use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub scan: ScanSettings,
    #[serde(default)]
    pub playback: PlaybackSettings,
    #[serde(default)]
    pub equalizer: EqualizerSettings,
    #[serde(default)]
    pub renumber: RenumberSettings,
    #[serde(default)]
    pub replacer: ReplacerSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSettings {
    pub roots: Vec<String>,
    #[serde(default)]
    pub source_root: String,
}

impl Default for ScanSettings {
    fn default() -> Self {
        let cwd = std::env::current_dir().ok();
        let default_dest = cwd
            .as_ref()
            .map(|p| p.join("music"))
            .unwrap_or_else(|| PathBuf::from("./music"));
        let default_source = cwd
            .as_ref()
            .map(|p| p.join("music_original"))
            .unwrap_or_else(|| PathBuf::from("./music_original"));
        Self {
            roots: vec![default_dest.to_string_lossy().to_string()],
            source_root: default_source.to_string_lossy().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackSettings {
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: String,
    pub crossfade_ms: u32,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            volume: 0.8,
            shuffle: false,
            repeat: "off".into(),
            crossfade_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqualizerSettings {
    pub enabled: bool,
    pub bands: [f32; 10],
    pub bass_boost: f32,
}

impl Default for EqualizerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bands: [0.0; 10],
            bass_boost: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenumberSettings {
    pub enabled: bool,
    pub threshold: f32,
}

impl Default for RenumberSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReplacerSettings {
    #[serde(default)]
    pub youtube_api_key: String,
    #[serde(default)]
    pub cookies_browser: String,
}

impl Settings {
    pub fn load_or_default() -> Self {
        match Self::load() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("settings load failed, using defaults: {e:#}");
                Self::default()
            }
        }
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let parsed: Self = toml::from_str(&text).with_context(|| "parse settings.toml")?;
        Ok(parsed)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let text = toml::to_string_pretty(self).context("serialize settings")?;
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("dev", "Recurate", "Recurate")
            .context("could not resolve config directory")?;
        Ok(dirs.config_dir().join("settings.toml"))
    }
}
