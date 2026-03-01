//! Persistent UI scale settings.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::APP_NAME;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UiScaleFile {
    pub format_version: u32,
    pub ui_scale: f32,
}

impl UiScaleFile {
    pub fn get_default_path() -> Option<PathBuf> {
        ProjectDirs::from("", "", APP_NAME).map(|proj_dirs| {
            let mut path = proj_dirs.data_dir().to_path_buf();
            path.push("ui_scale.json");
            path
        })
    }

    pub fn write_atomic(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(&tmp_path, json)?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        let state: Self = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ui_scale_file_roundtrip() {
        let state = UiScaleFile { format_version: 1, ui_scale: 1.35 };
        let json = serde_json::to_string(&state).expect("serialize");
        let decoded: UiScaleFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(state, decoded);
    }

    #[test]
    fn ui_scale_file_atomic_write_and_load() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("ui_scale.json");
        let state = UiScaleFile { format_version: 1, ui_scale: 1.6 };

        state.write_atomic(&path).expect("write");
        let loaded = UiScaleFile::load(&path).expect("load");
        assert_eq!(state, loaded);

        let tmp_path = path.with_extension("json.tmp");
        assert!(!tmp_path.exists());
    }
}
