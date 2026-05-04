use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::domain::RepeatMode;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Snapshot {
    pub queue_paths: Vec<PathBuf>,
    pub current: Option<usize>,
    pub position_ms: u64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub volume: f32,
}

impl Snapshot {
    pub fn path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("dev", "Recurate", "Recurate")
            .context("project dirs")?;
        Ok(dirs.data_local_dir().join("snapshot.json"))
    }

    pub fn load() -> Result<Option<Self>> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let parsed: Self = serde_json::from_str(&text).context("parse snapshot.json")?;
        Ok(Some(parsed))
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let text = serde_json::to_string_pretty(self).context("serialize snapshot")?;
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }
}
