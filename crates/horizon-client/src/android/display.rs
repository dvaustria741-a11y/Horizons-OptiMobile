//! Whole-frame resolution, independent of the world's graphics settings.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RenderScale(u32);

impl Default for RenderScale {
    fn default() -> Self {
        Self(100)
    }
}

impl RenderScale {
    pub const PRESETS: [u32; 6] = [100, 85, 75, 67, 50, 25];

    pub fn new(percent: u32) -> Self {
        Self(percent.clamp(25, 100))
    }

    pub fn parse(text: &str) -> Result<Self, &'static str> {
        let percent = text
            .trim()
            .trim_end_matches('%')
            .trim()
            .parse::<u32>()
            .map_err(|_| "Enter a whole percentage from 25 to 100")?;
        if !(25..=100).contains(&percent) {
            return Err("Enter a percentage from 25 to 100");
        }
        Ok(Self(percent))
    }

    pub fn percent(self) -> u32 {
        self.0.clamp(25, 100)
    }

    pub fn buffer_size(self, view: [u32; 2]) -> [u32; 2] {
        let [w, h] = view.map(|v| v.max(1));
        // Keep standard menus usable even on a narrow keypad screen. Never
        // enlarge a surface that is already smaller than the GUI minimum.
        let minimum = (320.0 / w as f64).max(240.0 / h as f64).min(1.0);
        let scale = (self.percent() as f64 / 100.0).max(minimum);
        [w, h].map(|v| (v as f64 * scale).round().max(1.0) as u32)
    }
}

/// Fixed render resolutions offered as presets (landscape width x height).
pub const RESOLUTION_PRESETS: [[u32; 2]; 5] =
    [[1920, 1080], [1600, 900], [1280, 720], [960, 540], [854, 480]];

pub fn resolution_label(size: [u32; 2]) -> String {
    format!("{} x {}", size[0], size[1])
}

/// Parse "1920x1080", "1920 x 1080", "1920*1080" or "1920\u{d7}1080".
pub fn parse_resolution(text: &str) -> Result<[u32; 2], &'static str> {
    const ERROR: &str = "Enter a resolution like 1920 x 1080";
    let cleaned = text.trim().replace(['\u{d7}', '*', 'X'], "x");
    let (w, h) = cleaned.split_once('x').ok_or(ERROR)?;
    let w = w.trim().parse::<u32>().map_err(|_| ERROR)?;
    let h = h.trim().parse::<u32>().map_err(|_| ERROR)?;
    if !(320..=8192).contains(&w) || !(240..=8192).contains(&h) {
        return Err("Width must be 320-8192 and height 240-8192");
    }
    Ok([w, h])
}

/// The fixed resolution fitted inside the view without stretching or
/// enlarging. Portrait screens swap width and height.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn fit_resolution(fixed: Option<[u32; 2]>, view: [u32; 2]) -> Option<[u32; 2]> {
    let [mut fw, mut fh] = fixed?;
    let [w, h] = view.map(|v| v.max(1));
    if h > w {
        std::mem::swap(&mut fw, &mut fh);
    }
    let minimum = (320.0 / w as f64).max(240.0 / h as f64).min(1.0);
    let factor = (fw as f64 / w as f64)
        .min(fh as f64 / h as f64)
        .min(1.0)
        .max(minimum);
    Some([w, h].map(|v| (v as f64 * factor).round().max(1.0) as u32))
}

#[cfg(target_os = "android")]
static REQUESTED_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(100);

/// Packed width << 32 | height; zero means no fixed resolution.
#[cfg(target_os = "android")]
static REQUESTED_FIXED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(target_os = "android")]
pub fn set_fixed(size: Option<[u32; 2]>) {
    let packed = size.map_or(0, |[w, h]| (u64::from(w) << 32) | u64::from(h));
    REQUESTED_FIXED.store(packed, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(target_os = "android")]
fn requested_fixed() -> Option<[u32; 2]> {
    let packed = REQUESTED_FIXED.load(std::sync::atomic::Ordering::Relaxed);
    (packed != 0).then(|| [(packed >> 32) as u32, (packed & 0xffff_ffff) as u32])
}

#[cfg(target_os = "android")]
pub fn set_scale(scale: RenderScale) {
    REQUESTED_SCALE.store(scale.percent(), std::sync::atomic::Ordering::Relaxed);
}

#[cfg(target_os = "android")]
fn desired(window: &winit::window::Window) -> Option<[u32; 2]> {
    let scale = RenderScale::new(REQUESTED_SCALE.load(std::sync::atomic::Ordering::Relaxed));
    crate::native_ime::refresh_bounds();
    let current = window.inner_size();
    let physical = window.outer_size();
    if physical.width == 0 || physical.height == 0 {
        return None;
    }
    let [_, _, width, height] =
        winit::platform::android::native_content_rect([physical.width, physical.height]);
    let view = [width, height];
    // The percentage sets only how many pixels the frame has (sharpness). The
    // screen size the UI is laid out for follows the resolution alone.
    let layout = fit_resolution(requested_fixed(), view).unwrap_or(view);
    horizon_client::hud::set_layout_reference(Some(layout));
    let size = scale.buffer_size(view);
    if size == [current.width, current.height] {
        winit::platform::android::set_native_input_scale([
            size[0] as f64 / view[0] as f64,
            size[1] as f64 / view[1] as f64,
        ]);
        None
    } else {
        Some(size)
    }
}

#[cfg(target_os = "android")]
fn set_render_size(window: &winit::window::Window, size: [u32; 2]) -> Result<[u32; 2], String> {
    use winit::platform::android::WindowExtAndroid;
    let native = REQUESTED_SCALE.load(std::sync::atomic::Ordering::Relaxed) == 100;
    let physical = window.outer_size();
    let rect = winit::platform::android::native_content_rect([physical.width, physical.height]);
    let target = if native && rect == [0, 0, physical.width, physical.height] {
        [0, 0]
    } else {
        size
    };
    let actual = window
        .set_render_size(winit::dpi::PhysicalSize::new(target[0], target[1]))
        .map_err(|e| e.to_string())?;
    log::info!("Android frame size {}x{}", actual.width, actual.height);
    Ok([actual.width, actual.height])
}

/// Set logical dimensions before a renderer recreates its native surface.
#[cfg(target_os = "android")]
pub fn prepare(window: &winit::window::Window) -> Result<Option<[u32; 2]>, String> {
    desired(window)
        .map(|size| set_render_size(window, size))
        .transpose()
}

/// Update the logical frame target; native presentation keeps its real dimensions.
#[cfg(target_os = "android")]
pub fn apply(
    window: &winit::window::Window,
    renderer: &mut horizon_client::gfx::Renderer,
) -> Result<(), String> {
    if renderer.suspended {
        return Ok(());
    }
    let size = if let Some(size) = desired(window) {
        set_render_size(window, size)?
    } else {
        let size = window.inner_size();
        [size.width, size.height]
    };
    // Some Android firmware updates the native window without delivering a
    // matching Resized event. Reconcile even at 100%, where no scale changes.
    if size[0] > 0 && size[1] > 0 && size != [renderer.config.width, renderer.config.height] {
        renderer.resize(size[0], size[1]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_surface_area_dimensions_without_changing_aspect() {
        assert_eq!(RenderScale::new(50).buffer_size([2772, 1280]), [1386, 640]);
        assert_eq!(
            RenderScale::default().buffer_size([2772, 1280]),
            [2772, 1280]
        );
    }

    #[test]
    fn keypad_screen_keeps_usable_menu_width() {
        assert_eq!(RenderScale::new(25).buffer_size([480, 854]), [320, 569]);
        assert_eq!(RenderScale::new(25).buffer_size([240, 320]), [240, 320]);
        assert_eq!(RenderScale::new(25).buffer_size([0, 0]), [1, 1]);
    }

    #[test]
    fn fit_resolution_fits_without_stretching_or_enlarging() {
        assert_eq!(fit_resolution(None, [2772, 1280]), None);
        assert_eq!(fit_resolution(Some([1920, 1080]), [2400, 1350]), Some([1920, 1080]));
        assert_eq!(fit_resolution(Some([1920, 1080]), [2772, 1280]), Some([1920, 887]));
        assert_eq!(fit_resolution(Some([1920, 1080]), [1280, 720]), Some([1280, 720]));
        assert_eq!(fit_resolution(Some([1920, 1080]), [1080, 1920]), Some([1080, 1920]));
    }

    #[test]
    fn resolution_text_is_parsed_and_bounded() {
        assert_eq!(parse_resolution("1920x1080"), Ok([1920, 1080]));
        assert_eq!(parse_resolution(" 1280 X 720 "), Ok([1280, 720]));
        assert_eq!(parse_resolution("1600*900"), Ok([1600, 900]));
        assert!(parse_resolution("1920").is_err());
        assert!(parse_resolution("10x10").is_err());
        assert!(parse_resolution("9000x9000").is_err());
    }

    #[test]
    fn edited_and_old_settings_have_bounded_values() {
        assert_eq!(RenderScale::parse(" 67% ").unwrap().percent(), 67);
        assert!(RenderScale::parse("0").is_err());
        assert!(RenderScale::parse("101").is_err());
        assert_eq!(
            serde_json::from_str::<RenderScale>("0").unwrap().percent(),
            25
        );
    }
}

/// Fixed presentation ratios; logical rendering and input share the same bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Aspect {
    #[default]
    Native,
    Wide,
    Classic,
    Square,
}
impl Aspect {
    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "Native",
            Self::Wide => "16:9",
            Self::Classic => "4:3",
            Self::Square => "1:1",
        }
    }
    pub fn ratio(self) -> [u32; 2] {
        match self {
            Self::Native => [0, 0],
            Self::Wide => [16, 9],
            Self::Classic => [4, 3],
            Self::Square => [1, 1],
        }
    }
}

/// Client-wide screen choices. Missing preferences start in navigation-free mode.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreenPreferences {
    pub immersive: bool,
    pub avoid_cutout: bool,
    pub aspect: Aspect,
}
impl Default for ScreenPreferences {
    fn default() -> Self {
        Self {
            immersive: true,
            avoid_cutout: false,
            aspect: Aspect::Native,
        }
    }
}
#[cfg(test)]
mod screen_tests {
    use super::*;
    #[test]
    fn missing_screen_preferences_hide_navigation_but_keep_explicit_off() {
        let defaults: ScreenPreferences = serde_json::from_str("{}").unwrap();
        assert!(defaults.immersive);
        let saved: ScreenPreferences =
            serde_json::from_str(r#"{"immersive":false,"aspect":"classic","avoid_cutout":true}"#)
                .unwrap();
        assert!(!saved.immersive);
        assert!(saved.avoid_cutout);
        assert_eq!(saved.aspect, Aspect::Classic);
    }
}
