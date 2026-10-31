use super::*;
use horizon_client::{
    osk::Output,
    ui_theme::{Icon, skin as color},
};
use winit::{
    event::{KeyEvent, Touch},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Action {
    Page(Page),
    Play,
    Search,
    ShaderSource(crate::shader_catalog::Origin),
    ModSource(crate::shader_catalog::Origin),
    Field(&'static str),
    FinishNameEdit,
    Install(usize),
    Import(horizon_client::content_files::Kind),
    OpenFolder(horizon_client::content_files::Kind),
    ToggleSection(usize),
    ToggleLocal(usize),
    DeleteLocal(usize),
    ConfirmDeleteLocal,
    Next,
    Previous,
    Profile,
    Logs,
    RecordPerformance,
    ExportPerformance,
    ExportDiagnostics,
    Account,
    SignIn,
    DeviceSignIn,
    CopyLoginCode,
    CancelLogin,
    ReviewApp,
    Repository,
    SignOut,
    LoginLink,
    CloseLogin,
    Renderer,
    RenderScale,
    ChooseRenderScale(crate::display::RenderScale),
    Orientation,
    InputHost,
    ChooseInputHost(crate::catalog::InputHost),
    ChooseOrientation(crate::catalog::Orientation),
    ChooseRenderer(crate::catalog::RendererMode),
    ChooseProfile(u32),
    Dismiss,
    Immersive,
    Cutout,
    Aspect,
    ChooseAspect(crate::display::Aspect),
    DisplayHz,
    ChooseDisplayHz(u16),
    PerformanceHints,
    GpuProfile,
}

pub(super) fn button(
    ui: &mut Ui,
    targets: &mut Vec<([f32; 4], Action)>,
    rect: [f32; 4],
    label: &str,
    action: Action,
    accent: bool,
) {
    let [x, y, w, h] = rect;
    ui.rounded(
        x,
        y,
        w,
        h,
        (h / 2.0).min(12.0),
        if accent { color::ACCENT } else { color::RAISED },
    );
    if !accent {
        ui.border(x, y, w, h, (h / 2.0).min(12.0), tint(color::ACCENT, 50));
    }
    let padding = (w * 0.12).clamp(4.0, 12.0);
    let base = if w < 80.0 { 13.0 } else { 16.0 };
    let font = if matches!(action, Action::Page(_)) {
        (base * ((w - 2.0 * padding) / ui.measure(Face::Medium, base, label).max(1.0)).min(1.0))
            .max(9.0)
    } else {
        base
    };
    let label = ui.fit_text(Face::Medium, font, label, w - 2.0 * padding);
    ui.text(
        Face::Medium,
        font,
        x + padding,
        y + 11.0,
        if accent {
            color::ON_ACCENT
        } else {
            color::TEXT
        },
        &label,
    );
    targets.push((rect, action));
}

/// `c` with its alpha replaced (straight alpha, HUD colour encoding).
fn tint(c: u32, a: u8) -> u32 {
    (c & 0x00ff_ffff) | (u32::from(a) << 24)
}
/// `0xRRGGBB` in the HUD's colour encoding, opaque.
fn rgb(c: u32) -> u32 {
    0xff00_0000 | (c & 0xff) << 16 | (c & 0xff00) | (c >> 16 & 0xff)
}
/// Linear blend of two `0xRRGGBB` colours, in the HUD encoding.
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |shift: u32| {
        let (x, y) = ((a >> shift & 0xff) as f32, (b >> shift & 0xff) as f32);
        (x + (y - x) * t.clamp(0.0, 1.0)).round() as u32
    };
    rgb(ch(16) << 16 | ch(8) << 8 | ch(0))
}
/// A vertical gradient from `top` to `bottom` (`0xRRGGBB`) filling a rounded rectangle,
/// drawn as thin opaque bands whose ends follow the rounded corners.
fn gradient(ui: &mut Ui, rect: [f32; 4], radius: f32, top: u32, bottom: u32) {
    let [x, y, w, h] = rect;
    let bands = ((h / 3.0) as usize).clamp(6, 72);
    for i in 0..bands {
        let (t0, t1) = (i as f32 / bands as f32, (i + 1) as f32 / bands as f32);
        let mid = h * (t0 + t1) / 2.0;
        let edge = mid.min(h - mid);
        let cut = if edge < radius {
            radius - (radius * radius - (radius - edge) * (radius - edge)).sqrt()
        } else {
            0.0
        };
        ui.rounded(
            x + cut,
            y + h * t0,
            w - 2.0 * cut,
            h * (t1 - t0) + 0.6,
            0.0,
            mix(top, bottom, (t0 + t1) / 2.0),
        );
    }
}
/// The hero card's sky: night teal at the top, warm at the horizon.
fn sky(t: f32) -> u32 {
    const STOPS: [(f32, u32); 4] = [
        (0.0, 0x0b2623),
        (0.55, 0x15473a),
        (0.82, 0x6b6a3a),
        (1.0, 0xc98a3d),
    ];
    for pair in STOPS.windows(2) {
        let ((t0, c0), (t1, c1)) = (pair[0], pair[1]);
        if t <= t1 {
            return mix(c0, c1, (t - t0) / (t1 - t0));
        }
    }
    rgb(STOPS[3].1)
}
/// A point `d` dp along the outline of a stadium (a pill) whose top-left is (`x`, `y`),
/// starting at the top-left of the top edge and running clockwise.
fn orbit_point(d: f32, x: f32, y: f32, w: f32, h: f32) -> (f32, f32) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let r = h / 2.0;
    let straight = (w - h).max(0.0);
    let arc = PI * r;
    if d < straight {
        (x + r + d, y)
    } else if d < straight + arc {
        let a = -FRAC_PI_2 + (d - straight) / r;
        (x + w - r + r * a.cos(), y + r + r * a.sin())
    } else if d < 2.0 * straight + arc {
        (x + w - r - (d - straight - arc), y + h)
    } else {
        let a = FRAC_PI_2 + (d - 2.0 * straight - arc) / r;
        (x + r + r * a.cos(), y + r + r * a.sin())
    }
}
fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}
fn page_icon(page: Page) -> Icon {
    match page {
        Page::Play => Icon::Play,
        Page::Mods => Icon::Mods,
        Page::Packs => Icon::Packs,
        Page::Shaders => Icon::Shaders,
        Page::Installed => Icon::Installed,
        Page::Settings => Icon::Settings,
    }
}
/// A pill button with an optional leading icon and a vertically centred label.
pub(super) fn pill(
    ui: &mut Ui,
    targets: &mut Vec<([f32; 4], Action)>,
    rect: [f32; 4],
    icon: Option<Icon>,
    label: &str,
    action: Action,
    accent: bool,
) {
    let [x, y, w, h] = rect;
    if accent && h >= 56.0 {
        gradient(ui, rect, (h / 2.0).min(30.0), 0x62e89b, 0x1f9d5c);
    } else {
        ui.rounded(
            x,
            y,
            w,
            h,
            (h / 2.0).min(14.0),
            if accent { color::ACCENT } else { color::RAISED },
        );
    }
    if !accent {
        ui.border(x, y, w, h, (h / 2.0).min(14.0), tint(color::ACCENT, 70));
    }
    let ink = if accent {
        color::ON_ACCENT
    } else {
        color::TEXT
    };
    let font = if h < 36.0 { 14.0 } else { 16.0 };
    let pad = (h * 0.38).clamp(10.0, 16.0);
    let glyph = (h * 0.5).clamp(16.0, 24.0);
    let mut left = x + pad;
    if let Some(icon) = icon {
        ui.icon(icon, left, y + (h - glyph) / 2.0, glyph, ink);
        left += glyph + 8.0;
    }
    let label = ui.fit_text(Face::Medium, font, label, (x + w - pad - left).max(8.0));
    let line = (font * 1.45).round();
    ui.text(
        Face::Medium,
        font,
        left,
        y + ((h - line) / 2.0).max(0.0),
        ink,
        &label,
    );
    targets.push((rect, action));
}
fn toggle(
    ui: &mut Ui,
    targets: &mut Vec<([f32; 4], Action)>,
    rect: [f32; 4],
    on: bool,
    action: Action,
) {
    let [x, y, w, h] = rect;
    ui.text(
        Face::Regular,
        14.0,
        x,
        y + 11.0,
        color::TEXT,
        if on { "On" } else { "Off" },
    );
    ui.rounded(
        x + w - 52.0,
        y + 8.0,
        48.0,
        24.0,
        12.0,
        if on { color::TEXT } else { color::HOVER },
    );
    ui.rounded(
        x + w - if on { 26.0 } else { 50.0 },
        y + 10.0,
        20.0,
        20.0,
        10.0,
        if on { color::BG } else { color::TEXT },
    );
    targets.push(([x, y, w, h], action));
}
impl Launcher {
    /// The same spatial focus algorithm as the game's controller menus.
    pub(super) fn navigate(&mut self, key: &KeyEvent) -> bool {
        self.navigate_key(&key.logical_key)
    }
    pub(super) fn navigate_key(&mut self, key: &winit::keyboard::Key) -> bool {
        use horizon_client::gamepad::{self, Button};
        use winit::keyboard::{Key, NamedKey};
        if self.pending_focus.is_some() {
            return true;
        }
        if self.catalog.editing.is_some() || self.targets.is_empty() {
            return false;
        }
        let button = match key {
            Key::Named(NamedKey::ArrowUp) => Button::DPadUp,
            Key::Named(NamedKey::ArrowDown) => Button::DPadDown,
            Key::Named(NamedKey::ArrowLeft) => Button::DPadLeft,
            Key::Named(NamedKey::ArrowRight) => Button::DPadRight,
            Key::Named(NamedKey::Tab) => {
                self.focus = Some(self.focus.map_or(0, |i| (i + 1) % self.targets.len()));
                return true;
            }
            _ => return false,
        };
        if matches!(button, Button::DPadUp | Button::DPadDown)
            && let Some((_, action)) = self.focus.and_then(|i| self.targets.get(i))
            && let Some(index) = self.list_targets.iter().position(|(_, a)| a == action)
        {
            let rects: Vec<_> = self.list_targets.iter().map(|(r, _)| *r).collect();
            if let Some(next) = gamepad::focus_step_strict(rects[index], button, &rects)
                && let Some(view) = self.list_view
            {
                let (rect, action) = self.list_targets[next];
                let scroll = if let Some(chooser) = &mut self.chooser {
                    &mut chooser.scroll
                } else {
                    &mut self.catalog.scroll
                };
                *scroll = super::gesture::reveal(*scroll, rect, view);
                self.pending_focus = Some(action);
                self.focus = None;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                return true;
            }
        }
        self.focus = Some(
            if let Some(index) = self.focus.filter(|i| *i < self.targets.len()) {
                let rects: Vec<_> = self.targets.iter().map(|(rect, _)| *rect).collect();
                gamepad::focus_step_rect(rects[index], button, &rects).unwrap_or(index)
            } else {
                0
            },
        );
        true
    }

    pub(super) fn draw_pages(&mut self) -> Result<()> {
        let (Some(r), Some(gui)) = (&mut self.renderer, &self.gui) else {
            return Ok(());
        };
        if r.suspended {
            return Ok(());
        }
        self.targets.clear();
        self.list_targets.clear();
        self.list_view = None;
        let mut b = HudBatch::new([r.config.width, r.config.height]);
        let mut ui = Ui::mobile(&gui.atlas, &mut b, (0.0, 0.0));
        // OEM navigation bars can remain visible even in fullscreen (keypad phones).
        // Inset the launcher layout only; keep surface and input coordinates intact.
        if let Some(window) = &self.window {
            use winit::platform::android::WindowExtAndroid;
            let rect = window.content_rect();
            let input = winit::platform::android::native_input_scale();
            if rect.bottom > 0 {
                ui.h = ui.h.min(rect.bottom as f32 * input[1] as f32 / ui.density);
            }
            if rect.right > 0 {
                ui.w = ui.w.min(rect.right as f32 * input[0] as f32 / ui.density);
            }
        }
        if let Some(bottom) = crate::native_ime::viewport_bottom() {
            ui.h = ui.h.min(bottom as f32 / ui.density);
        }
        let (w, h) = (ui.w, ui.h);
        let scale = ui.density / ui.batch.scale;
        ui.batch
            .fill(&gui.atlas, 0.0, 0.0, w * scale, h * scale, color::BG);
        gradient(&mut ui, [0.0, 0.0, w, h], 0.0, 0x0f2a20, 0x040b08);
        let full_h = h;
        let wide = w >= 700.0;
        let rail = if wide { 92.0 } else { 0.0 };
        let dock = if wide { 0.0 } else { 62.0 };
        let h = h - dock;
        let lx = rail + 28.0;
        let cw = w - lx - 28.0;
        let oy = -60.0_f32;
        // ---- animation clock ----
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.ui_clock).as_secs_f32().min(0.1);
        self.ui_clock = now;
        if self.page_shown != self.catalog.page {
            self.page_shown = self.catalog.page;
            self.page_since = now;
        }
        let wanted = Page::ALL
            .iter()
            .position(|p| *p == self.catalog.page)
            .unwrap_or(0) as f32;
        self.nav_pos += (wanted - self.nav_pos) * (1.0 - (-dt * 14.0).exp());
        let sliding = (wanted - self.nav_pos).abs() > 0.003;
        if !sliding {
            self.nav_pos = wanted;
        }
        let fade = ease_out(now.duration_since(self.page_since).as_secs_f32() / 0.24);
        let clock = now.duration_since(self.epoch).as_secs_f32();
        self.animating = sliding || fade < 1.0;
        // ---- account chip (first target, like the old header button) ----
        let chip_w = if wide { 250.0 } else { 150.0 };
        pill(
            &mut ui,
            &mut self.targets,
            [
                w - 28.0 - chip_w,
                if wide { 20.0 } else { 10.0 },
                chip_w,
                40.0,
            ],
            Some(Icon::Account),
            &self.login.status(&self.catalog.config.name),
            Action::Account,
            self.login.signed_in,
        );
        // ---- title block ----
        let page = self.catalog.page;
        let title = match page {
            Page::Play => "Play",
            Page::Mods => "Mods",
            Page::Packs => "Resource packs",
            Page::Shaders => "Shaders",
            Page::Installed => "Installed",
            Page::Settings => "Settings",
        };
        ui.text(
            Face::Medium,
            if wide { 28.0 } else { 22.0 },
            lx,
            if wide { 16.0 } else { 10.0 },
            color::TEXT,
            title,
        );
        if wide {
            ui.text(
                Face::Regular,
                13.0,
                lx,
                54.0,
                color::MUTED,
                &format!("Horizon Android {}", crate::backend::VERSION),
            );
        }
        // ---- navigation ----
        if wide {
            gradient(&mut ui, [0.0, 0.0, rail, full_h], 0.0, 0x0e231a, 0x061009);
            ui.rounded(rail - 1.0, 0.0, 1.0, full_h, 0.0, color::BORDER);
            ui.rounded(14.0, 14.0, 64.0, 64.0, 22.0, tint(color::ACCENT, 34));
            ui.rounded(18.0, 18.0, 56.0, 56.0, 18.0, color::ACCENT);
            let mark = ui.measure(Face::Medium, 28.0, "H");
            ui.text(
                Face::Medium,
                28.0,
                46.0 - mark / 2.0,
                30.0,
                color::ON_ACCENT,
                "H",
            );
            let item = ((full_h - 100.0 - 16.0) / 6.0).clamp(52.0, 74.0);
            let slide = 96.0 + self.nav_pos * item;
            ui.rounded(
                8.0,
                slide,
                rail - 16.0,
                item - 6.0,
                18.0,
                tint(color::ACCENT, 48),
            );
            ui.border(8.0, slide, rail - 16.0, item - 6.0, 18.0, tint(color::ACCENT, 90));
            for (i, p) in Page::ALL.iter().enumerate() {
                let y = 96.0 + i as f32 * item;
                let on = *p == page;
                let glow = (1.0 - (self.nav_pos - i as f32).abs()).clamp(0.0, 1.0);
                let ink = if glow > 0.5 {
                    color::ACCENT
                } else {
                    color::MUTED
                };
                let lift = -3.0 * glow;
                ui.icon(
                    page_icon(*p),
                    (rail - 28.0) / 2.0,
                    y + 8.0 + lift,
                    28.0,
                    ink,
                );
                let name = p.name();
                let tw = ui.measure(Face::Medium, 12.0, name);
                ui.text(
                    Face::Medium,
                    12.0,
                    (rail - tw) / 2.0,
                    y + 40.0 + lift,
                    if on { color::TEXT } else { color::MUTED },
                    name,
                );
                self.targets
                    .push(([8.0, y, rail - 16.0, item - 6.0], Action::Page(*p)));
            }
        } else {
            let top = full_h - dock;
            ui.rounded(0.0, top, w, dock, 0.0, color::RAISED);
            ui.rounded(0.0, top, w, 1.0, 0.0, color::BORDER);
            let item = w / 6.0;
            ui.rounded(
                self.nav_pos * item + 12.0,
                top,
                item - 24.0,
                3.0,
                1.5,
                color::ACCENT,
            );
            for (i, p) in Page::ALL.iter().enumerate() {
                let x = i as f32 * item;
                let glow = (1.0 - (self.nav_pos - i as f32).abs()).clamp(0.0, 1.0);
                let ink = if glow > 0.5 {
                    color::ACCENT
                } else {
                    color::MUTED
                };
                ui.icon(
                    page_icon(*p),
                    x + (item - 24.0) / 2.0,
                    top + 9.0 - 2.0 * glow,
                    24.0,
                    ink,
                );
                let name = match p {
                    Page::Installed => "Files",
                    Page::Settings => "Setup",
                    _ => p.name(),
                };
                let tw = ui.measure(Face::Medium, 10.0, name);
                ui.text(
                    Face::Medium,
                    10.0,
                    x + (item - tw) / 2.0,
                    top + 37.0 - 2.0 * glow,
                    if *p == page {
                        color::TEXT
                    } else {
                        color::MUTED
                    },
                    name,
                );
                self.targets.push(([x, top, item, dock], Action::Page(*p)));
            }
        }
        let compact = w < 700.0;
        if page == Page::Play {
            use horizon_client::official_install::InstallStage;
            let installed = horizon_client::official_install::ready(&self.home);
            let installing = self.worker.is_some();
            let progress = *self.install.lock().unwrap();
            let top = 160.0 + oy;
            self.animating |= installed || installing;
            let card = [lx - 14.0, top - 20.0, cw + 28.0, (h - top - 92.0).max(80.0)];
            let (card_top, card_bottom) = (0x153629, 0x0b1a13);
            gradient(&mut ui, card, 22.0, card_top, card_bottom);
            if !installing && !compact {
                let (hx, right) = (card[0] + card[2] * 0.40, card[0] + card[2]);
                let (hw, radius) = (right - hx, 22.0_f32);
                // How far the card's rounded right corners cut in, `edge` dp from the top or bottom.
                let inset = |edge: f32| {
                    if edge < radius {
                        radius - (radius * radius - (radius - edge) * (radius - edge)).sqrt()
                    } else {
                        0.0
                    }
                };
                let bands = 36;
                let band_h = card[3] / bands as f32;
                for i in 0..bands {
                    let y0 = card[1] + band_h * i as f32;
                    let mid = band_h * (i as f32 + 0.5);
                    let cut = inset(mid.min(card[3] - mid));
                    ui.rounded(
                        hx,
                        y0,
                        hw - cut,
                        band_h + 0.6,
                        0.0,
                        sky((i as f32 + 0.5) / bands as f32),
                    );
                }
                ui.hero(hx, card[1], hw, card[3]);
                // Square picture corners poke out of the card's rounded corners: cover them.
                let page_at = |y: f32| mix(0x0f2a20, 0x040b08, y / full_h);
                for k in 0..22 {
                    let cut = inset(k as f32 + 0.5);
                    let (top_y, bottom_y) = (card[1] + k as f32, card[1] + card[3] - 1.0 - k as f32);
                    ui.rounded(right - cut, top_y, cut + 0.5, 1.0, 0.0, page_at(top_y));
                    ui.rounded(right - cut, bottom_y, cut + 0.5, 1.0, 0.0, page_at(bottom_y));
                }
                // Melt the picture's left edge into the card.
                let (strips, rows, fade) = (24, 12, hw * 0.38);
                for i in 0..strips {
                    let (s0, s1) = (i as f32 / strips as f32, (i + 1) as f32 / strips as f32);
                    let alpha = ((1.0 - (s0 + s1) / 2.0).powf(1.6) * 255.0) as u8;
                    for j in 0..rows {
                        let (r0, r1) = (j as f32 / rows as f32, (j + 1) as f32 / rows as f32);
                        ui.rounded(
                            hx + fade * s0,
                            card[1] + card[3] * r0,
                            fade * (s1 - s0),
                            card[3] * (r1 - r0),
                            0.0,
                            tint(mix(card_top, card_bottom, (r0 + r1) / 2.0), alpha),
                        );
                    }
                }
            }
            ui.border(card[0], card[1], card[2], card[3], 22.0, tint(color::ACCENT, 80));
            ui.text(
                Face::Medium,
                32.0,
                lx,
                top,
                color::TEXT,
                if installing {
                    "Getting your game ready"
                } else if installed {
                    "Ready to play"
                } else {
                    if compact {
                        "Your Java worlds"
                    } else {
                        "Your Java worlds, on Android"
                    }
                },
            );
            let sub = format!("Minecraft 26.3  /  Profile {}", self.catalog.config.profile);
            let sub_w = ui.measure(Face::Regular, 15.0, &sub);
            ui.rounded(
                lx,
                top + 46.0,
                sub_w + 28.0,
                30.0,
                15.0,
                tint(color::ACCENT, 40),
            );
            ui.text(
                Face::Regular,
                15.0,
                lx + 14.0,
                top + 50.0,
                color::TEXT,
                &sub,
            );
            if installing {
                let current = InstallStage::ALL
                    .iter()
                    .position(|s| *s == progress.stage)
                    .unwrap_or(0);
                let row_height = ((h - 360.0) / 6.0).clamp(25.0, 40.0);
                if h >= 620.0 {
                    for (index, stage) in InstallStage::ALL.iter().enumerate() {
                        let y = top + 90.0 + index as f32 * row_height;
                        let label = if index < current {
                            format!("{:02}  {}  - done", index + 1, stage.label())
                        } else {
                            format!("{:02}  {}", index + 1, stage.label())
                        };
                        ui.text(
                            Face::Regular,
                            16.0,
                            lx,
                            y,
                            if index == current {
                                color::TEXT
                            } else {
                                color::MUTED
                            },
                            &label,
                        );
                    }
                } else {
                    ui.text(
                        Face::Regular,
                        18.0,
                        lx,
                        top + 100.0,
                        color::TEXT,
                        progress.stage.label(),
                    );
                    ui.text(
                        Face::Regular,
                        15.0,
                        lx,
                        top + 138.0,
                        color::MUTED,
                        &format!("Step {} of 6", current + 1),
                    );
                }
                let fraction = if progress.total > 0 {
                    progress.done as f32 / progress.total as f32
                } else {
                    0.0
                };
                let overall = (current as f32 + fraction.clamp(0.0, 1.0)) / 6.0;
                let bar_y = h - 152.0;
                ui.rounded(lx, bar_y, cw, 6.0, 3.0, color::HOVER);
                if overall > 0.0 {
                    let filled = cw * overall;
                    ui.rounded(lx, bar_y, filled, 6.0, 3.0, color::ACCENT);
                    let sweep = 70.0_f32.min(filled);
                    let at = ((clock * 0.7) % 1.0) * (filled + sweep) - sweep;
                    let (a, b) = (at.max(0.0), (at + sweep).min(filled));
                    if b > a {
                        ui.rounded(lx + a, bar_y, b - a, 6.0, 3.0, tint(0x00ff_ffff, 70));
                    }
                }
                let detail = match progress.stage {
                    InstallStage::Download => format!(
                        "{:.1} / {:.1} MiB  - verified downloads",
                        progress.done as f64 / 1048576.0,
                        progress.total as f64 / 1048576.0
                    ),
                    InstallStage::Controls => format!(
                        "{} / {} control files verified",
                        progress.done, progress.total
                    ),
                    _ => "Please keep Horizon open. Downloaded files are kept for retry.".into(),
                };
                ui.text(Face::Regular, 14.0, lx, bar_y + 14.0, color::MUTED, &detail);
            } else {
                let rows = if installed {
                    [
                        "01  Open a world or join a Java server",
                        "02  Touch setup: Options > Controls",
                        "03  Find packs and .hmod in the tabs",
                    ]
                } else {
                    [
                        "01  Download resources from Mojang",
                        "02  Install and verify on this phone",
                        "03  Play offline or sign in for servers",
                    ]
                };
                for (index, line) in rows.iter().enumerate() {
                    let y = top + 100.0 + index as f32 * 46.0;
                    let (num, text) = line.split_at(2);
                    let text_w = ui.measure(Face::Regular, 17.0, text.trim_start());
                    ui.rounded(lx + 30.0, y - 3.0, text_w + 48.0, 32.0, 16.0, color::RAISED);
                    ui.rounded(lx, y - 3.0, 32.0, 32.0, 16.0, rgb(0x1f9d5c));
                    let nw = ui.measure(Face::Medium, 14.0, num);
                    ui.text(
                        Face::Medium,
                        14.0,
                        lx + 16.0 - nw / 2.0,
                        y + 1.0,
                        color::TEXT,
                        num,
                    );
                    ui.text(
                        Face::Regular,
                        17.0,
                        lx + 46.0,
                        y,
                        color::TEXT,
                        text.trim_start(),
                    );
                }
            }
            if installing {
                ui.text(
                    Face::Medium,
                    19.0,
                    lx,
                    h - 90.0,
                    color::TEXT,
                    progress.stage.label(),
                );
            } else {
                let play = [lx, h - 100.0, cw.min(420.0), 60.0];
                if installed {
                    let pulse = 0.5 + 0.5 * (clock * 2.4).sin();
                    let r = play[3] / 2.0;
                    ui.rounded(
                        play[0] - 7.0,
                        play[1] - 7.0,
                        play[2] + 14.0,
                        play[3] + 14.0,
                        r + 7.0,
                        tint(color::ACCENT, (10.0 + 24.0 * pulse) as u8),
                    );
                    ui.rounded(
                        play[0] - 3.0,
                        play[1] - 3.0,
                        play[2] + 6.0,
                        play[3] + 6.0,
                        r + 3.0,
                        tint(color::ACCENT, (22.0 + 44.0 * pulse) as u8),
                    );
                }
                pill(
                    &mut ui,
                    &mut self.targets,
                    play,
                    Some(if installed {
                        Icon::Play
                    } else {
                        Icon::Installed
                    }),
                    if installed {
                        "Play"
                    } else if self.error.is_some() {
                        "Retry installation"
                    } else {
                        "Download Minecraft 26.3"
                    },
                    Action::Play,
                    true,
                );
                if installed {
                    // A comet of light running round the button's edge.
                    let [x, y, w, bh] = play;
                    ui.rounded(x + 2.0, y + 2.0, w - 4.0, bh / 2.0 - 2.0, bh / 4.0, tint(0x00ff_ffff, 30));
                    ui.border(x, y, w, bh, bh / 2.0, tint(rgb(0xd6ffe6), 70));
                    let perimeter = 2.0 * (w - bh) + std::f32::consts::PI * bh;
                    let head = (clock * 0.3).fract() * perimeter;
                    let tail = (perimeter * 0.3).min(260.0);
                    let dots = 40;
                    for i in 0..dots {
                        let f = i as f32 / dots as f32;
                        let (px, py) =
                            orbit_point((head - f * tail).rem_euclid(perimeter), x, y, w, bh);
                        let size = 6.0 - 3.0 * f;
                        ui.rounded(
                            px - size / 2.0,
                            py - size / 2.0,
                            size,
                            size,
                            size / 2.0,
                            tint(rgb(0xd6ffe6), (230.0 * (1.0 - f).powf(1.8)) as u8),
                        );
                    }
                }
            }
        } else if page.is_catalog() {
            ui.text(
                Face::Medium,
                18.0,
                lx,
                142.0 + oy,
                color::TEXT,
                if page == Page::Mods {
                    "Horizon Mods"
                } else if page == Page::Shaders {
                    if w < 360.0 { "Shaders" } else { "Shader Packs" }
                } else {
                    "Resource Packs"
                },
            );
            button(
                &mut ui,
                &mut self.targets,
                [lx, 174.0 + oy, w - lx - 210.0, 44.0],
                &format!(
                    "{}{}",
                    if page == Page::Mods {
                        "Search / .hmod URL: "
                    } else {
                        "Search: "
                    },
                    self.catalog.query
                ),
                Action::Field("search"),
                false,
            );
            pill(
                &mut ui,
                &mut self.targets,
                [w - 28.0 - 170.0, 174.0 + oy, 170.0, 44.0],
                Some(Icon::Search),
                if page == Page::Mods && self.catalog.query.trim().starts_with("https://") {
                    "Import .hmod"
                } else {
                    "Search"
                },
                Action::Search,
                true,
            );
            if matches!(page, Page::Shaders | Page::Mods) {
                use crate::shader_catalog::Origin;
                let width = ((w - 185.0) / 2.0).clamp(58.0, 120.0);
                for (i, origin) in [Origin::Horizon, Origin::Modrinth].into_iter().enumerate() {
                    let rect = [
                        w - 28.0 - (2 - i) as f32 * width,
                        134.0 + oy,
                        width - 5.0,
                        32.0,
                    ];
                    button(
                        &mut ui,
                        &mut self.targets,
                        rect,
                        origin.source().label(),
                        if page == Page::Mods {
                            Action::ModSource(origin)
                        } else {
                            Action::ShaderSource(origin)
                        },
                        false,
                    );
                    if (if page == Page::Mods {
                        self.catalog.config.mod_source
                    } else {
                        self.catalog.config.shader_source
                    }) == origin
                    {
                        ui.border(rect[0], rect[1], rect[2], rect[3], 1.0, color::TEXT);
                    }
                }
            }
            let hint = match page {
                Page::Mods
                    if self.catalog.config.mod_source
                        == crate::shader_catalog::Origin::Modrinth =>
                {
                    "Modrinth Java mods are browse-only; Fabric/Forge/NeoForge cannot run here"
                }
                Page::Mods => "Search Horizon mods or paste an HTTPS .hmod link",
                Page::Shaders
                    if self.catalog.config.shader_source
                        == crate::shader_catalog::Origin::Modrinth =>
                {
                    "Modrinth shader ZIPs / experimental compatibility; some packs may not work"
                }
                Page::Shaders => "Horizon native .hshader packs",
                _ => "Verified downloads / enable in Installed",
            };
            let hint = ui.fit_text(Face::Regular, 14.0, hint, cw);
            ui.text(Face::Regular, 14.0, lx, 270.0 + oy, color::MUTED, &hint);
            let kind = match page {
                Page::Mods => horizon_client::content_files::Kind::Mods,
                Page::Shaders => horizon_client::content_files::Kind::Shaders,
                _ => horizon_client::content_files::Kind::Packs,
            };
            let file_button_width = ((w - 72.0) / 2.0).min(158.0);
            pill(
                &mut ui,
                &mut self.targets,
                [lx, 222.0 + oy, file_button_width, 38.0],
                Some(Icon::Folder),
                "Open folder",
                Action::OpenFolder(kind),
                false,
            );
            pill(
                &mut ui,
                &mut self.targets,
                [
                    w - 28.0 - file_button_width,
                    222.0 + oy,
                    file_button_width,
                    38.0,
                ],
                Some(Icon::Import),
                "Import file",
                Action::Import(kind),
                false,
            );
            let top = 296.0 + oy;
            let bottom = h - 112.0;
            self.catalog.scroll = self.catalog.scroll.clamp(
                0.0,
                (self.catalog.results.len() as f32 * 78.0 - (bottom - top)).max(0.0),
            );
            self.list_view = Some([
                top,
                bottom,
                (self.catalog.results.len() as f32 * 78.0 - (bottom - top)).max(0.0),
            ]);
            for (i, p) in self.catalog.results.iter().enumerate() {
                let y = top + i as f32 * 78.0 - self.catalog.scroll;
                let download = self.catalog.download_label(p);
                if download.is_none()
                    && !crate::catalog::source_only_mod(p)
                    && !(p.kind == crate::browser::Kind::Mod
                        && p.source == crate::browser::SourceId::Modrinth)
                {
                    self.list_targets
                        .push(([w - 150.0, y, 118.0, 70.0], Action::Install(i)));
                }
                if y < top || y + 70.0 > bottom {
                    continue;
                }
                let tx = lx + 60.0;
                let room = (w - 150.0 - tx - 16.0).max(40.0);
                ui.rounded(lx - 12.0, y - 8.0, w - lx - 8.0, 74.0, 16.0, color::RAISED);
                ui.rounded(lx, y + 2.0, 46.0, 46.0, 14.0, tint(color::ACCENT, 40));
                ui.icon(page_icon(page), lx + 11.0, y + 13.0, 24.0, color::ACCENT);
                let title = ui.fit_text(Face::Medium, 19.0, &p.title, room);
                let summary = ui.fit_text(Face::Regular, 14.0, &p.summary, room);
                ui.text(Face::Medium, 19.0, tx, y, color::TEXT, &title);
                ui.text(Face::Regular, 14.0, tx, y + 29.0, color::MUTED, &summary);
                if let Some(state) = download {
                    ui.text(Face::Medium, 16.0, w - 150.0, y + 12.0, color::TEXT, &state);
                } else if p.kind == crate::browser::Kind::Mod
                    && p.source == crate::browser::SourceId::Modrinth
                {
                    ui.text(
                        Face::Regular,
                        14.0,
                        w - 150.0,
                        y + 12.0,
                        color::MUTED,
                        "Java mod",
                    );
                } else if crate::catalog::source_only_mod(p) {
                    ui.text(
                        Face::Regular,
                        14.0,
                        w - 150.0,
                        y + 12.0,
                        color::MUTED,
                        ".hnmod: later",
                    );
                } else if p.kind == crate::browser::Kind::Mod && !cfg!(target_pointer_width = "64")
                {
                    ui.text(
                        Face::Regular,
                        14.0,
                        w - 150.0,
                        y + 12.0,
                        color::MUTED,
                        "64-bit required",
                    );
                } else {
                    button(
                        &mut ui,
                        &mut self.targets,
                        [w - 150.0, y, 118.0, 42.0],
                        self.catalog.install_button(p),
                        Action::Install(i),
                        false,
                    );
                }
            }
            pill(
                &mut ui,
                &mut self.targets,
                [lx, h - 100.0, 120.0, 40.0],
                None,
                "Previous",
                Action::Previous,
                false,
            );
            pill(
                &mut ui,
                &mut self.targets,
                [lx + 134.0, h - 100.0, 120.0, 40.0],
                None,
                "Next",
                Action::Next,
                false,
            );
        } else if page == Page::Installed {
            use horizon_client::content_files::Kind;
            let top = 156.0 + oy;
            let bottom = h - 100.0;
            let row_height = if compact { 110.0 } else { 72.0 };
            let total = Kind::ALL
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    48.0 + if self.catalog.sections[i] {
                        self.catalog.local.iter().filter(|e| e.kind == *k).count() as f32
                            * row_height
                    } else {
                        0.0
                    }
                })
                .sum::<f32>();
            let maximum = (total - (bottom - top)).max(0.0);
            self.catalog.scroll = self.catalog.scroll.clamp(0.0, maximum);
            self.list_view = Some([top, bottom, maximum]);
            let mut offset = 0.0;
            for (section, kind) in Kind::ALL.into_iter().enumerate() {
                let count = self.catalog.local.iter().filter(|e| e.kind == kind).count();
                let y = top + offset - self.catalog.scroll;
                let label = format!(
                    "{} {} ({count})",
                    if self.catalog.sections[section] {
                        "-"
                    } else {
                        "+"
                    },
                    kind.label()
                );
                self.list_targets.push((
                    [lx, y, w - lx - 178.0, 40.0],
                    Action::ToggleSection(section),
                ));
                self.list_targets
                    .push(([w - 165.0, y, 133.0, 40.0], Action::Import(kind)));
                if y >= top && y + 40.0 <= bottom {
                    button(
                        &mut ui,
                        &mut self.targets,
                        [lx, y, w - lx - 178.0, 40.0],
                        &label,
                        Action::ToggleSection(section),
                        false,
                    );
                    pill(
                        &mut ui,
                        &mut self.targets,
                        [w - 165.0, y, 133.0, 40.0],
                        Some(Icon::Import),
                        "Import",
                        Action::Import(kind),
                        false,
                    );
                }
                offset += 48.0;
                if !self.catalog.sections[section] {
                    continue;
                }
                for (i, item) in self
                    .catalog
                    .local
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.kind == kind)
                {
                    let y = top + offset - self.catalog.scroll;
                    offset += row_height;
                    self.list_targets.push((
                        [
                            if compact { lx } else { w - 300.0 },
                            y,
                            120.0,
                            row_height - 4.0,
                        ],
                        Action::ToggleLocal(i),
                    ));
                    self.list_targets.push((
                        [w - 165.0, y, 133.0, row_height - 4.0],
                        Action::DeleteLocal(i),
                    ));
                    if y < top || y + row_height - 4.0 > bottom {
                        continue;
                    }
                    let title = ui.fit_text(
                        Face::Medium,
                        19.0,
                        &item.title,
                        if compact { cw } else { w - lx - 332.0 },
                    );
                    ui.text(Face::Medium, 19.0, lx, y, color::TEXT, &title);
                    ui.text(
                        Face::Regular,
                        14.0,
                        lx,
                        y + 28.0,
                        color::MUTED,
                        kind.label(),
                    );
                    if kind == Kind::Mods && !cfg!(target_pointer_width = "64") {
                        ui.text(
                            Face::Regular,
                            14.0,
                            if compact { lx } else { w - 300.0 },
                            y + if compact { 62.0 } else { 12.0 },
                            color::MUTED,
                            "64-bit required",
                        );
                    } else {
                        toggle(
                            &mut ui,
                            &mut self.targets,
                            [
                                if compact { lx } else { w - 300.0 },
                                y + if compact { 55.0 } else { 0.0 },
                                120.0,
                                40.0,
                            ],
                            item.enabled,
                            Action::ToggleLocal(i),
                        );
                    }
                    pill(
                        &mut ui,
                        &mut self.targets,
                        [w - 165.0, y + if compact { 55.0 } else { 0.0 }, 133.0, 40.0],
                        Some(Icon::Delete),
                        "Remove",
                        Action::DeleteLocal(i),
                        false,
                    );
                }
            }
        } else {
            let c = &self.catalog.config;
            ui.text(
                Face::Medium,
                20.0,
                lx,
                156.0 + oy,
                color::TEXT,
                "Launch settings",
            );
            let row_height = if compact { 82.0 } else { 66.0 };
            self.catalog.scroll = self.catalog.scroll.clamp(
                0.0,
                (15.0 * row_height - (h - 130.0 - (195.0 + oy))).max(0.0),
            );
            self.list_view = Some([
                (195.0 + oy),
                h - 130.0,
                (15.0 * row_height - (h - 130.0 - (195.0 + oy))).max(0.0),
            ]);
            for (i, (title, value, action)) in [
                ("Rendering", c.renderer.label().to_owned(), Action::Renderer),
                (
                    "Client resolution",
                    if self.catalog.editing == Some("render_scale") {
                        self.catalog.render_scale_text.clone()
                    } else {
                        format!("{}%", c.render_scale.percent())
                    },
                    Action::RenderScale,
                ),
                ("Profile", format!("Profile {}", c.profile), Action::Profile),
                (
                    "Orientation",
                    c.orientation.label().to_owned(),
                    Action::Orientation,
                ),
                ("Input", c.input_host.label().to_owned(), Action::InputHost),
                (
                    "Immersive fullscreen",
                    if c.screen.immersive { "On" } else { "Off" }.into(),
                    Action::Immersive,
                ),
                (
                    "Avoid camera cutout",
                    if c.screen.avoid_cutout { "On" } else { "Off" }.into(),
                    Action::Cutout,
                ),
                (
                    "Aspect ratio",
                    c.screen.aspect.label().into(),
                    Action::Aspect,
                ),
                (
                    "Display refresh rate",
                    c.device.display_label(),
                    Action::DisplayHz,
                ),
                (
                    "Performance hints",
                    if c.device.performance_hints {
                        "On (ADPF)"
                    } else {
                        "Off"
                    }
                    .into(),
                    Action::PerformanceHints,
                ),
                (
                    "GPU profiling",
                    if c.device.gpu_profile {
                        "On / included in report"
                    } else {
                        "Off"
                    }
                    .into(),
                    Action::GpuProfile,
                ),
                (
                    "Performance recording",
                    if c.record_performance {
                        "On / up to 30 min"
                    } else {
                        "Off"
                    }
                    .into(),
                    Action::RecordPerformance,
                ),
                (
                    "Diagnostic report",
                    "Export one ZIP to share".into(),
                    Action::ExportDiagnostics,
                ),
                (
                    "Raw performance data",
                    "Advanced / JSONL in Reports".into(),
                    Action::ExportPerformance,
                ),
                (
                    "Server",
                    if c.server.is_empty() {
                        "Singleplayer / tap to enter server".into()
                    } else {
                        c.server.clone()
                    },
                    Action::Field("server"),
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let y = (195.0 + oy) + i as f32 * row_height - self.catalog.scroll;
                self.list_targets.push((
                    [
                        if compact { lx } else { lx + (w - lx) * 0.34 },
                        y,
                        if compact { cw } else { (w - lx) * 0.66 - 28.0 },
                        if compact { 70.0 } else { 46.0 },
                    ],
                    action,
                ));
                if y < (185.0 + oy) || y + if compact { 70.0 } else { 46.0 } > h - 130.0 {
                    continue;
                }
                ui.text(
                    Face::Regular,
                    16.0,
                    lx,
                    y + if compact { 0.0 } else { 13.0 },
                    color::MUTED,
                    title,
                );
                if matches!(
                    action,
                    Action::PerformanceHints | Action::Immersive | Action::Cutout
                ) {
                    toggle(
                        &mut ui,
                        &mut self.targets,
                        [
                            if compact { lx } else { lx + (w - lx) * 0.34 },
                            y + if compact { 24.0 } else { 0.0 },
                            if compact { cw } else { (w - lx) * 0.66 - 28.0 },
                            46.0,
                        ],
                        match action {
                            Action::PerformanceHints => c.device.performance_hints,
                            Action::Immersive => c.screen.immersive,
                            Action::Cutout => c.screen.avoid_cutout,
                            _ => c.device.gpu_profile,
                        },
                        action,
                    );
                } else {
                    button(
                        &mut ui,
                        &mut self.targets,
                        [
                            if compact { lx } else { lx + (w - lx) * 0.34 },
                            y + if compact { 24.0 } else { 0.0 },
                            if compact { cw } else { (w - lx) * 0.66 - 28.0 },
                            46.0,
                        ],
                        &format!(
                            "{} {}",
                            value,
                            if matches!(
                                action,
                                Action::Renderer
                                    | Action::Aspect
                                    | Action::DisplayHz
                                    | Action::RenderScale
                                    | Action::Profile
                                    | Action::Orientation
                                    | Action::InputHost
                            ) {
                                "v"
                            } else {
                                ""
                            }
                        ),
                        action,
                        false,
                    );
                }
                ui.rounded(
                    lx,
                    y + if compact { 78.0 } else { 62.0 },
                    cw,
                    1.0,
                    0.0,
                    color::BORDER,
                );
            }
            pill(
                &mut ui,
                &mut self.targets,
                [lx + cw / 2.0 + 6.0, h - 114.0, cw / 2.0 - 6.0, 42.0],
                Some(Icon::Account),
                "Microsoft / Xbox account",
                Action::Account,
                true,
            );
            pill(
                &mut ui,
                &mut self.targets,
                [lx, h - 114.0, cw / 2.0 - 6.0, 42.0],
                Some(Icon::Diagnostics),
                "Diagnostics",
                Action::Logs,
                false,
            );
        }
        if fade < 1.0 {
            ui.rounded(
                rail,
                70.0,
                w - rail,
                h - 70.0,
                0.0,
                tint(color::BG, ((1.0 - fade) * 255.0) as u8),
            );
        }
        let status = self.error.as_deref().unwrap_or(&self.catalog.status);
        for (i, line) in status
            .chars()
            .collect::<Vec<_>>()
            .chunks(110)
            .take(2)
            .enumerate()
        {
            ui.text(
                Face::Regular,
                14.0,
                lx,
                h - 47.0 + i as f32 * 18.0,
                color::MUTED,
                &ui.fit_text(
                    Face::Regular,
                    14.0,
                    &line.iter().collect::<String>(),
                    (w - lx - 190.0).max(40.0),
                ),
            );
        }
        pill(
            &mut ui,
            &mut self.targets,
            [w - 28.0 - 120.0, h - 50.0, 120.0, 34.0],
            Some(Icon::Link),
            "GitHub",
            Action::Repository,
            false,
        );
        if self.login.visible {
            self.list_targets.clear();
            self.list_view = None;
            self.login.draw(
                &mut ui,
                &mut self.targets,
                &self.catalog.config.name,
                self.catalog.editing == Some("name"),
            );
        }
        if let Some(chooser) = &mut self.chooser {
            chooser.draw(&mut ui, &mut self.targets);
            self.list_targets = chooser.navigation(ui.w);
            self.list_view = Some(chooser.viewport(ui.h));
        }
        if let Some([top, bottom, maximum]) = self.list_view
            && maximum > 0.0
            && bottom > top
        {
            let scroll = self
                .chooser
                .as_ref()
                .map_or(self.catalog.scroll, |c| c.scroll);
            let height = bottom - top;
            let thumb = (height * height / (height + maximum)).max(20.0).min(height);
            let y = top + (height - thumb) * (scroll / maximum).clamp(0.0, 1.0);
            ui.rounded(w - 12.0, top, 4.0, height, 2.0, color::BORDER);
            ui.rounded(w - 12.0, y, 4.0, thumb, 2.0, color::MUTED);
        }
        let caret_visible = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .is_ok_and(|t| t.as_millis() % 1000 < 500);
        if caret_visible && let Some(name) = self.catalog.editing {
            let value = match name {
                "name" => self.catalog.config.name.clone(),
                "server" => self.catalog.config.server.clone(),
                "render_scale" => self.catalog.render_scale_text.clone(),
                _ => format!("Search: {}", self.catalog.query),
            };
            for ([x, y, w, _], action) in &self.targets {
                if *action == Action::Field(name)
                    || (name == "render_scale" && *action == Action::RenderScale)
                {
                    let text = ui.fit_text(Face::Medium, 16.0, &value, w - 24.0);
                    let caret_x = x + 12.0 + ui.measure(Face::Medium, 16.0, &text);
                    ui.rounded(
                        caret_x.min(x + w - 10.0),
                        y + 12.0,
                        1.0,
                        18.0,
                        0.0,
                        color::TEXT,
                    );
                }
            }
        }
        if let Some(action) = self.pending_focus.take() {
            self.focus = self.targets.iter().position(|(_, a)| *a == action);
        }
        if let Some(index) = self.focus
            && let Some(([x, y, w, h], _)) = self.targets.get(index)
        {
            ui.border(*x - 2.0, *y - 2.0, *w + 4.0, *h + 4.0, 2.0, color::TEXT);
            self.cursor = (
                f64::from((x + w / 2.0) * ui.density),
                f64::from((y + h / 2.0) * ui.density),
            );
        }
        if self.mouse_down || self.drag.as_ref().is_some_and(|drag| !drag.moved) {
            let point = [
                self.cursor.0 as f32 / ui.density,
                self.cursor.1 as f32 / ui.density,
            ];
            if let Some(([x, y, w, h], _)) = self.targets.iter().rev().find(|(rect, _)| {
                point[0] >= rect[0]
                    && point[0] <= rect[0] + rect[2]
                    && point[1] >= rect[1]
                    && point[1] <= rect[1] + rect[3]
            }) {
                ui.rounded(
                    *x,
                    *y,
                    *w,
                    *h,
                    12.0,
                    horizon_client::hud::rgba(255, 255, 255, 48),
                );
            }
        }
        if !self.mouse_down
            && self.drag.is_none()
            && let Some((action, time)) = self.feedback
            && time.elapsed() < std::time::Duration::from_millis(160)
            && let Some(([x, y, w, h], _)) = self.targets.iter().find(|(_, a)| *a == action)
        {
            ui.rounded(
                *x,
                *y,
                *w,
                *h,
                12.0,
                horizon_client::hud::rgba(255, 255, 255, 48),
            );
        }
        crate::documents::draw_ui(&mut ui);
        ui.finish();

        if !r.render(&game::loading_frame(&b))? && !b.patches.is_empty() {
            // A suspended/lost surface skips the frame, including its glyph
            // uploads. Rebuild them next time instead of caching invisible text.
            horizon_client::ui_theme::reset_atlas();
        }
        Ok(())
    }
    pub(super) fn activate(&mut self, events: &ActiveEventLoop) -> Result<()> {
        if self.session_worker.is_some() {
            return Ok(());
        }
        let Some(r) = &self.renderer else {
            return Ok(());
        };
        let mut b = HudBatch::new([r.config.width, r.config.height]);
        let ui = Ui::mobile(&self.gui.as_ref().unwrap().atlas, &mut b, (0.0, 0.0));
        let p = self.focus.and_then(|index| self.targets.get(index)).map_or(
            [
                self.cursor.0 as f32 / ui.density,
                self.cursor.1 as f32 / ui.density,
            ],
            |([x, y, w, h], _)| [x + w / 2.0, y + h / 2.0],
        );
        let action = self
            .targets
            .iter()
            .find(|([x, y, w, h], _)| {
                (*x..*x + *w).contains(&p[0]) && (*y..*y + *h).contains(&p[1])
            })
            .map(|(_, a)| *a);
        if self.catalog.busy() || crate::documents::busy() {
            return Ok(());
        }
        self.feedback = action.map(|action| (action, std::time::Instant::now()));
        self.focus = None;
        if action.is_some() && horizon_client::official_install::ready(&self.home) {
            let audio = self.audio.get_or_insert_with(|| {
                let mut audio = horizon_client::audio::Audio::new(
                    &self.home.join(".local/horizon-client/26.3"),
                );
                let settings = horizon_client::settings::Settings::load(
                    &self
                        .catalog
                        .config
                        .game_home(&self.home)
                        .join("options.txt"),
                );
                audio.master = settings.volumes[0];
                let mut volumes = settings.volumes;
                volumes[0] = 1.0;
                horizon_client::settings::set_source_volumes(volumes);
                audio
            });
            audio.play_button_click();
        }
        match action {
            Some(Action::Immersive) => {
                self.catalog.config.screen.immersive ^= true;
                self.catalog.config.save(&self.home)?;
                self.configure_screen();
            }
            Some(Action::Cutout) => {
                self.catalog.config.screen.avoid_cutout ^= true;
                self.catalog.config.save(&self.home)?;
                self.configure_screen();
            }
            Some(Action::Aspect) => self.open_chooser(chooser::Kind::Aspect),
            Some(Action::ChooseAspect(aspect)) => {
                self.catalog.config.screen.aspect = aspect;
                self.catalog.config.save(&self.home)?;
                self.configure_screen();
                self.chooser = None;
            }
            Some(Action::DisplayHz) => self.open_chooser(chooser::Kind::DisplayHz),
            Some(Action::ChooseDisplayHz(hz)) => {
                self.catalog.config.device.display_hz = hz;
                self.catalog.config.device.sanitize();
                self.catalog.config.save(&self.home)?;
                crate::device_runtime::configure(self.catalog.config.device);
                self.chooser = None;
            }
            Some(Action::PerformanceHints) => {
                self.catalog.config.device.performance_hints ^= true;
                self.catalog.status="ADPF gives Android CPU frame-work hints on supported devices. It does not cap FPS or guarantee a speedup.".into();
                self.catalog.config.save(&self.home)?;
                crate::device_runtime::configure(self.catalog.config.device);
            }
            Some(Action::GpuProfile) => {
                self.catalog.config.device.gpu_profile ^= true;
                self.catalog.status="Detailed GPU profiling records diagnostics and has overhead. Reports remain on this device.".into();
                self.catalog.config.save(&self.home)?;
                crate::device_runtime::configure(self.catalog.config.device);
            }

            Some(Action::ModSource(origin)) => {
                self.catalog.config.mod_source = origin;
                self.catalog.config.save(&self.home)?;
                self.catalog.offset = 0;
                self.catalog.browse(&self.home);
            }
            Some(Action::ShaderSource(origin)) => {
                self.catalog.config.shader_source = origin;
                self.catalog.config.save(&self.home)?;
                self.catalog.offset = 0;
                self.catalog.browse(&self.home);
            }
            Some(Action::OpenFolder(kind)) => {
                crate::platform::hide();
                let folder = match kind {
                    horizon_client::content_files::Kind::Mods => "mods",
                    horizon_client::content_files::Kind::Packs => "packs",
                    horizon_client::content_files::Kind::Shaders => "shaders",
                };
                let id = crate::document_store::folder_id(self.catalog.config.profile, folder);
                self.catalog.status = if crate::platform::open_documents(&id) {
                    "Opened in Files. Return here after copying files.".into()
                } else {
                    "Open the system Files app and choose Horizon Android.".into()
                };
            }
            Some(Action::Import(kind)) => {
                crate::platform::hide();
                let home = self.catalog.config.game_home(&self.home);
                if let Err(e) = crate::documents::begin(kind, home) {
                    self.catalog.status = e;
                }
            }
            Some(Action::ToggleSection(i)) => {
                self.catalog.sections[i] = !self.catalog.sections[i];
                self.catalog.scroll = 0.0;
            }
            Some(Action::ToggleLocal(i)) => self.catalog.toggle_local(&self.home, i),
            Some(Action::DeleteLocal(i)) => {
                self.catalog.pending_delete = self.catalog.local.get(i).cloned();
                if let Some(item) = &self.catalog.pending_delete {
                    self.chooser = Some(super::chooser::Chooser::confirm_delete(item.file.clone()));
                }
            }
            Some(Action::ConfirmDeleteLocal) => {
                self.catalog.remove_local(&self.home);
                self.chooser = None;
            }
            Some(Action::Repository) => {
                self.catalog.status = if crate::platform::open_repository() {
                    "Repository opened in browser."
                } else {
                    "No browser available to open the repository."
                }
                .into();
            }
            Some(Action::Account) => {
                self.login.visible = true;
            }
            Some(Action::CloseLogin) => {
                self.login.hide();
                crate::platform::hide();
                self.catalog.editing = None;
            }
            Some(Action::DeviceSignIn) => self
                .login
                .start_device(&self.catalog.config.microsoft_client_id),
            Some(Action::CopyLoginCode) => self.login.copy_code(),
            Some(Action::CancelLogin) => {
                self.login.close();
                self.login.visible = true;
            }
            Some(Action::ReviewApp) => self.login.review_app(),
            Some(Action::SignIn) => self.login.start(&self.catalog.config.microsoft_client_id),
            Some(Action::SignOut) => self.login.sign_out(),
            Some(Action::LoginLink) => self.login.open_link(),
            Some(Action::Page(page)) => {
                self.catalog.editing = None;
                if crate::platform::active() {
                    crate::platform::hide();
                }
                self.catalog.page = page;
                self.catalog.scroll = 0.0;
                self.error = None;
                self.catalog.offset = 0;
                self.catalog.browse(&self.home);
            }
            Some(Action::Play) => self.action(events)?,
            Some(Action::Search) => {
                crate::platform::hide();
                self.catalog.offset = 0;
                self.catalog.search(&self.home);
            }
            Some(Action::FinishNameEdit) => {
                self.catalog.editing = None;
                crate::platform::hide();
                self.catalog.config.save(&self.home)?;
            }
            Some(Action::Field(name)) => {
                if name == "render_scale" {
                    self.catalog.render_scale_text =
                        self.catalog.config.render_scale.percent().to_string();
                    self.chooser = None;
                }
                self.catalog.editing = Some(name);
                let initial = match name {
                    "name" => &self.catalog.config.name,
                    "server" => &self.catalog.config.server,
                    "render_scale" => &self.catalog.render_scale_text,
                    "client_id" => &self.catalog.config.microsoft_client_id,
                    _ => &self.catalog.query,
                };
                crate::platform::show(initial);
            }
            Some(Action::Install(i)) => self.catalog.install(&self.home, i),
            Some(Action::Next) => {
                self.catalog.offset += 12;
                self.catalog.search(&self.home);
            }
            Some(Action::Previous) => {
                self.catalog.offset = self.catalog.offset.saturating_sub(12);
                self.catalog.search(&self.home);
            }
            Some(Action::Profile) => self.open_chooser(chooser::Kind::Profile),
            Some(Action::Renderer) => self.open_chooser(chooser::Kind::Renderer),
            Some(Action::RenderScale) => self.open_chooser(chooser::Kind::RenderScale),
            Some(Action::ChooseRenderScale(scale)) => {
                self.catalog.config.render_scale = scale;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
            }
            Some(Action::Orientation) => self.open_chooser(chooser::Kind::Orientation),
            Some(Action::InputHost) => self.open_chooser(chooser::Kind::InputHost),
            Some(Action::ChooseInputHost(mode)) => {
                self.catalog.config.input_host = mode;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                crate::configure_input_mode(mode);
            }
            Some(Action::ChooseOrientation(mode)) => {
                self.catalog.config.orientation = mode;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                crate::platform::set_orientation(mode);
            }
            Some(Action::ChooseRenderer(mode)) => {
                self.catalog.config.renderer = mode;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
            }
            Some(Action::ChooseProfile(profile)) => {
                std::fs::create_dir_all(if profile == 0 {
                    self.home.clone()
                } else {
                    self.home
                        .join("profiles")
                        .join(format!("profile-{profile}"))
                })?;
                self.catalog.config.profile = profile;
                self.catalog.config.save(&self.home)?;
                self.catalog.refresh(&self.home);
                self.chooser = None;
            }
            Some(Action::Dismiss) => self.chooser = None,
            Some(Action::Logs) => {
                self.catalog.status = crate::diagnostics::summary(&self.home);
                if let Ok(error) = std::fs::read_to_string(self.home.join("android-error.txt")) {
                    self.catalog.status = format!(
                        "Last handled error: {}. {}",
                        crate::diagnostics::redact(&error),
                        self.catalog.status
                    );
                }
            }
            Some(Action::ExportDiagnostics) if self.diagnostic_export.is_none() => {
                let home = self.home.clone();
                let (sender, receiver) = mpsc::channel();
                self.diagnostic_export = Some(receiver);
                self.catalog.status =
                    "Preparing local diagnostic ZIP. No upload; review before sharing.".into();
                std::thread::spawn(move || {
                    let result = crate::diagnostics::export(&home).map(|p| {
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    });
                    let _ = sender.send(result);
                });
            }
            Some(Action::ExportDiagnostics) => {}
            Some(Action::RecordPerformance) => {
                self.catalog.config.record_performance = !self.catalog.config.record_performance;
                if !self.catalog.config.record_performance {
                    crate::performance::stop();
                }
                self.catalog.config.save(&self.home)?;
                self.catalog.status = if self.catalog.config.record_performance {
                    "Recording starts on Play. Detailed timings add overhead. Return here to save the report."
                } else { "Performance recording off." }.into();
            }
            Some(Action::ExportPerformance) => {
                self.catalog.status = match crate::performance::export(&self.home) {
                    Ok(()) => "Reports opened in Files. Copy or share a report from there.".into(),
                    Err(e) => e,
                };
            }
            None => {}
        }
        Ok(())
    }
    pub(super) fn ui_density(&self) -> f32 {
        let (Some(r), Some(gui)) = (&self.renderer, &self.gui) else {
            return 1.0;
        };
        let mut batch = HudBatch::new([r.config.width, r.config.height]);
        Ui::mobile(&gui.atlas, &mut batch, (0.0, 0.0)).density
    }

    pub(super) fn touch_event(&mut self, events: &ActiveEventLoop, t: Touch) -> Result<()> {
        self.focus = None;
        self.pending_focus = None;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        self.cursor = (t.location.x, t.location.y);
        match t.phase {
            TouchPhase::Started => {
                self.drag = Some(super::gesture::Drag::new(t.location.x, t.location.y));
            }
            TouchPhase::Moved => {
                let density = self.ui_density() as f64;
                if let Some(drag) = &mut self.drag {
                    let delta = drag.update(t.location.x, t.location.y, density);
                    if drag.moved {
                        if let Some(chooser) = &mut self.chooser {
                            chooser.scroll = (chooser.scroll + delta).max(0.0);
                        } else if self
                            .list_view
                            .is_some_and(|view| drag.origin_y() / density >= view[0] as f64)
                        {
                            self.catalog.scroll = (self.catalog.scroll + delta).max(0.0);
                        }
                    }
                }
            }
            TouchPhase::Ended => {
                if self.drag.take().is_some_and(|drag| !drag.moved) {
                    self.activate(events)?;
                }
            }
            TouchPhase::Cancelled => {
                self.drag = None;
            }
        }
        Ok(())
    }
    pub(super) fn type_key(&mut self, key: KeyEvent) {
        if key.physical_key == PhysicalKey::Code(KeyCode::Escape)
            || key.logical_key
                == winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrowserBack)
        {
            if self.login.visible {
                self.login.hide();
                crate::platform::hide();
                self.catalog.editing = None;
                return;
            }
            if self.chooser.take().is_some() {
                return;
            }
            crate::platform::hide();
            self.catalog.editing = None;
            let _ = self.catalog.config.save(&self.home);
            return;
        }
        if self.catalog.editing.is_none() {
            return;
        }
        let out = match key.physical_key {
            PhysicalKey::Code(KeyCode::Backspace) => Some(Output::Backspace),
            PhysicalKey::Code(KeyCode::Enter) => {
                crate::platform::hide();
                Some(Output::Enter)
            }
            _ => key.text.map(|s| Output::Text(s.into())),
        };
        if let Some(out) = out {
            self.catalog.edit(out, &self.home);
        }
    }
}
