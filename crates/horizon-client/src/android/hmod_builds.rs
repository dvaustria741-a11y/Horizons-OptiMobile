//! Sandboxed `.hmod` builds of registry mods that later turned native (`.hnmod`).
//!
//! A native mod is compiled into the game, which an already-built APK cannot do. Each of the mods
//! below still publishes its last WebAssembly build in the registry's releases, and Android runs
//! those in the same sandbox as every other `.hmod`. The registry's index deliberately leaves
//! these builds out (a launcher never installs a `.hmod` as a native mod), so they are pinned
//! here by the SHA-256 the registry entry lists. The live index is still asked about takedowns.
use crate::browser;
use horizon_client::mod_list::{DIR, ModList};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path, time::Duration};

/// Far larger than any of these files (the biggest is about 50 KiB).
const MAX_BYTES: u64 = 8 * 1024 * 1024;

/// One pinned `.hmod` release.
pub struct Build {
    pub id: &'static str,
    pub version: &'static str,
    pub sha256: &'static str,
}

/// The newest `.hmod` version of every native registry mod that has one, as its entry in
/// `horizon-rust/horizon-mods` lists it.
pub const BUILDS: [Build; 9] = [
    Build {
        id: "blockscope",
        version: "0.2.0",
        sha256: "dcb8a3d3eefe37ebc7fd1886412dc9799c5fa4dee88fd419e7f46a712df00c12",
    },
    Build {
        id: "classic_zoom",
        version: "1.0.0",
        sha256: "d8bc299202d9760299a3901841adef950d46cfcc1a54e0729379d24885e9ca5d",
    },
    Build {
        id: "coords_hud",
        version: "0.1.0",
        sha256: "612335e4e00d638f8b97b6c2f5c718c069c7a13811ba922557c27cb2612daf59",
    },
    Build {
        id: "fps_overlay",
        version: "1.0.0",
        sha256: "46109c3da01c058c0c15a9e43206b749627e69cd5cd023167621bb5dbdb88bcb",
    },
    Build {
        id: "keystrokes",
        version: "1.0.0",
        sha256: "fc7ab8fffba4d9df9da2b5aa84dd9ed33be4712fe65a0b518094f5feb64966d4",
    },
    Build {
        id: "lookat",
        version: "0.1.0",
        sha256: "f3da841f96ffc4d43a636c4aa6503907281ed36cd8711c498447ef5402943970",
    },
    Build {
        id: "minimap",
        version: "1.0.0",
        sha256: "2906eb2832f4e64075be2aa55da0829f40b2eccf26727951dd19d7607e9fe704",
    },
    Build {
        id: "recipes",
        version: "1.0.0",
        sha256: "d393b6f0061377113eeb3bfd710dbf68f564de1c2949da694c8e4de1b6b983c6",
    },
    Build {
        id: "zoom",
        version: "1.0.0",
        sha256: "447f7cc49b4e8535c8252a871ac6fbdafcc7bfcf881297429dc93f185b2f9340",
    },
];

pub fn build_for(id: &str) -> Option<&'static Build> {
    BUILDS.iter().find(|b| b.id == id)
}

impl Build {
    pub fn file(&self) -> String {
        format!("{}-{}.hmod", self.id, self.version)
    }
    pub fn url(&self) -> String {
        format!(
            "{}{}-{}/{}",
            horizon_mods::index::FILE_HOST,
            self.id,
            self.version,
            self.file()
        )
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Downloads `build` over HTTPS, checks it against its pinned hash and the live takedown list,
/// reads its manifest, installs it into the game folder and turns it on. Returns the mod's name.
pub fn install(game: &Path, build: &Build, progress: &dyn Fn(f32)) -> Result<String, String> {
    let url = build.url();
    let response = ureq::AgentBuilder::new()
        .https_only(true)
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(90))
        .build()
        .get(&url)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("The mod download answered {code}."),
            ureq::Error::Transport(t) => format!("The mod download failed: {t}"),
        })?;
    let total = response
        .header("Content-Length")
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|n| *n > 0.0);
    let mut reader = response.into_reader().take(MAX_BYTES + 1);
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(total) = total {
            progress((bytes.len() as f32 / total).min(1.0));
        }
    }
    if bytes.len() as u64 > MAX_BYTES {
        return Err("The mod file is larger than expected".into());
    }
    let sha256 = hex(&Sha256::digest(&bytes));
    if !build.sha256.eq_ignore_ascii_case(&sha256) {
        return Err(format!(
            "{} does not match the reviewed file (checksum differs); nothing was installed.",
            build.file()
        ));
    }
    if let Ok(index) = browser::horizon::index()
        && let Some(taken) = index.revoked(build.id, &sha256)
    {
        return Err(format!(
            "{} was taken down: {}",
            build.file(),
            taken.reason
        ));
    }
    let dir = game.join(DIR);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stage = dir.join("import.partial");
    std::fs::write(&stage, &bytes).map_err(|e| e.to_string())?;
    let result: Result<String, String> = (|| {
        let package = horizon_mods::Package::read(&stage)?;
        if package.manifest.id != build.id || package.manifest.version != build.version {
            return Err(format!(
                "{} holds {} {}, not {} {}",
                build.file(),
                package.manifest.id,
                package.manifest.version,
                build.id,
                build.version
            ));
        }
        let destination = dir.join(format!("{}.hmod", build.id));
        if destination.exists() {
            std::fs::copy(&destination, destination.with_extension("hmod.previous"))
                .map_err(|e| e.to_string())?;
        }
        std::fs::rename(&stage, &destination).map_err(|e| e.to_string())?;
        let mut on = ModList::load(game);
        on.enabled.insert(build.id.to_owned());
        on.save(game);
        Ok(package.manifest.name)
    })();
    let _ = std::fs::remove_file(&stage);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_are_well_formed() {
        for b in &BUILDS {
            assert!(horizon_mods::index::valid_id(b.id));
            assert_eq!(b.sha256.len(), 64);
            assert!(
                b.sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            );
            assert!(b.url().starts_with("https://"));
            assert!(b.file().ends_with(".hmod"));
        }
    }
}
