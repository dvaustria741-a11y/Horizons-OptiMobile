//! Where the user's own files live: mods, packs, shaders, worlds, reports.
//! Android's external app folder (`Android/data/<package>/files`) is used when
//! it is writable, so a file manager can reach it; otherwise the private home.
use std::path::{Path, PathBuf};

const MARKER: &str = "content-root.txt";

/// The folder holding profile 0's game files. The provider reads the same
/// marker, so it works even when the game is not running.
pub fn content_root(home: &Path) -> PathBuf {
    std::fs::read_to_string(home.join(MARKER))
        .ok()
        .map(|text| PathBuf::from(text.trim()))
        .filter(|path| !path.as_os_str().is_empty() && path.is_dir())
        .unwrap_or_else(|| home.to_owned())
}

/// Record the external folder if it can be written, otherwise fall back to home.
pub fn publish(home: &Path, external: Option<&Path>) {
    let usable = external.filter(|dir| {
        std::fs::create_dir_all(dir).is_ok() && {
            let probe = dir.join(".write-test");
            let ok = std::fs::write(&probe, b"ok").is_ok();
            let _ = std::fs::remove_file(probe);
            ok
        }
    });
    match usable {
        Some(dir) => {
            if let Err(error) = std::fs::write(home.join(MARKER), dir.to_string_lossy().as_bytes())
            {
                log::warn!("Content folder marker: {error}");
            }
        }
        None => {
            let _ = std::fs::remove_file(home.join(MARKER));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_marker_and_falls_back_to_home() {
        let base = std::env::temp_dir().join(format!("horizon-root-{}", std::process::id()));
        let (home, external) = (base.join("home"), base.join("external"));
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(content_root(&home), home);
        publish(&home, Some(&external));
        assert_eq!(content_root(&home), external);
        publish(&home, None);
        assert_eq!(content_root(&home), home);
        let _ = std::fs::remove_dir_all(base);
    }
}
