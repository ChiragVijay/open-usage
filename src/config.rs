use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

pub const DEFAULT_REFRESH_SECONDS: u64 = 1800;
pub const REFRESH_INTERVALS: [u64; 4] = [300, 600, 1800, 3600];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub online: bool,
    pub enabled: [bool; 3],
    pub dark: bool,
    pub privacy: bool,
    pub refresh_seconds: u64,
    pub custom_homes: [Vec<PathBuf>; 3],
    pub cursor_imports: Vec<PathBuf>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            online: true,
            enabled: [true; 3],
            dark: false,
            privacy: false,
            refresh_seconds: DEFAULT_REFRESH_SECONDS,
            custom_homes: Default::default(),
            cursor_imports: vec![],
        }
    }
}
pub fn data_dir() -> PathBuf {
    std::env::var_os("OPEN_USAGE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("open-usage")
        })
}
impl Config {
    pub fn load() -> Result<Self> {
        let path = data_dir().join("settings.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let mut config: Self =
            serde_json::from_slice(&fs::read(&path)?).context("Settings could not be read")?;
        if !REFRESH_INTERVALS.contains(&config.refresh_seconds) {
            config.refresh_seconds = DEFAULT_REFRESH_SECONDS;
        }
        Ok(config)
    }
    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(data_dir())?;
        let path = data_dir().join("settings.json");
        let pending = data_dir().join("settings.pending.json");
        fs::write(&pending, serde_json::to_vec_pretty(self)?)?;
        fs::rename(pending, path).context("Settings could not be saved")
    }
}
