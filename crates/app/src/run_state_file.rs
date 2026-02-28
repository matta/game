use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RunStateFile {
    pub format_version: u32,
    pub run_seed: u64,
    pub snapshot_hash_hex: String,
    pub tick: u64,
    pub floor_index: u8,
    pub branch_profile: String,
    pub active_god: String,
    pub updated_at_unix_ms: u64,
}

impl RunStateFile {
    pub fn get_default_path() -> Option<PathBuf> {
        ProjectDirs::from("", "", "Roguelike").map(|proj_dirs| {
            let mut path = proj_dirs.data_dir().to_path_buf();
            path.push("last_run_state.json");
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
    fn test_json_roundtrip() {
        let state = RunStateFile {
            format_version: 1,
            run_seed: 12345,
            snapshot_hash_hex: "0x00000000deadbeef".to_string(),
            tick: 100,
            floor_index: 2,
            branch_profile: "BranchA".to_string(),
            active_god: "Veil".to_string(),
            updated_at_unix_ms: 1645956000000,
        };

        let json = serde_json::to_string(&state).unwrap();
        let decoded: RunStateFile = serde_json::from_str(&json).unwrap();
        assert_eq!(state, decoded);
    }

    #[test]
    fn test_atomic_write_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");

        let state = RunStateFile {
            format_version: 1,
            run_seed: 99,
            snapshot_hash_hex: "0x123".to_string(),
            tick: 0,
            floor_index: 1,
            branch_profile: "None".to_string(),
            active_god: "None".to_string(),
            updated_at_unix_ms: 0,
        };

        state.write_atomic(&path).unwrap();
        assert!(path.exists());

        let loaded = RunStateFile::load(&path).unwrap();
        assert_eq!(state, loaded);

        // Verify tmp file is gone
        let tmp_path = path.with_extension("json.tmp");
        assert!(!tmp_path.exists());
    }
}
