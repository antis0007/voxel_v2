//! Server files kept beside the world: settings and access lists.
//!
//! Both are plain text so an operator can edit them by hand. A missing file means
//! defaults, and an unreadable one is reported rather than fatal: a broken file
//! must not stop the host from starting.

use engine_net::admin::{AccessList, ServerSettings};
use std::path::Path;

pub const SETTINGS_FILE: &str = "server.properties";
pub const ACCESS_FILE: &str = "server.access";

pub struct Loaded {
    pub settings: ServerSettings,
    pub access: AccessList,
    pub warnings: Vec<String>,
}

/// Reads both files, falling back to defaults and collecting anything ignored.
pub fn load(dir: &Path) -> Loaded {
    let mut warnings = Vec::new();
    let settings = match std::fs::read_to_string(dir.join(SETTINGS_FILE)) {
        Ok(text) => {
            let (settings, problems) = ServerSettings::parse(&text);
            warnings.extend(problems.into_iter().map(|p| format!("{SETTINGS_FILE}: {p}")));
            settings
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => ServerSettings::default(),
        Err(error) => {
            warnings.push(format!("{SETTINGS_FILE}: {error}"));
            ServerSettings::default()
        }
    };
    let access = match std::fs::read_to_string(dir.join(ACCESS_FILE)) {
        Ok(text) => {
            let (access, problems) = AccessList::parse(&text);
            warnings.extend(problems.into_iter().map(|p| format!("{ACCESS_FILE}: {p}")));
            access
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => AccessList::default(),
        Err(error) => {
            warnings.push(format!("{ACCESS_FILE}: {error}"));
            AccessList::default()
        }
    };
    Loaded { settings, access, warnings }
}

/// Writes both files, creating the directory if needed.
pub fn save(dir: &Path, settings: &ServerSettings, access: &AccessList) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(SETTINGS_FILE), settings.to_text())?;
    std::fs::write(dir.join(ACCESS_FILE), access.to_text())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_directory_yields_defaults() {
        let dir = std::env::temp_dir().join("voxel-server-files-missing");
        let _ = std::fs::remove_dir_all(&dir);
        let loaded = load(&dir);
        assert_eq!(loaded.settings, ServerSettings::default());
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    }

    #[test]
    fn both_files_survive_a_round_trip() {
        let dir = std::env::temp_dir().join("voxel-server-files-round");
        let _ = std::fs::remove_dir_all(&dir);
        let mut settings = ServerSettings::default();
        settings.motd = "quiet valley".to_owned();
        settings.whitelist = true;
        let mut access = AccessList::default();
        access.set_op("Ana", true).expect("op");
        access.ban("Bo", "grief").expect("ban");
        save(&dir, &settings, &access).expect("save");

        let loaded = load(&dir);
        assert_eq!(loaded.settings, settings);
        assert!(loaded.access.is_op("ana"));
        assert_eq!(loaded.access.ban_reason("BO"), Some("grief"));
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
