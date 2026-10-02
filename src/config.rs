use crate::pricing::Price;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub online: bool,
    pub enabled: [bool; 3],
    pub dark: bool,
    pub privacy: bool,
    pub monthly_budget: f64,
    pub refresh_seconds: u64,
    pub custom_homes: [Vec<PathBuf>; 3],
    pub cursor_imports: Vec<PathBuf>,
    pub prices: BTreeMap<String, Price>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            online: false,
            enabled: [true; 3],
            dark: false,
            privacy: false,
            monthly_budget: 100.0,
            refresh_seconds: 300,
            custom_homes: Default::default(),
            cursor_imports: vec![],
            prices: BTreeMap::new(),
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
        if !config.monthly_budget.is_finite() || config.monthly_budget <= 0.0 {
            config.monthly_budget = 100.0;
        }
        config.refresh_seconds = config.refresh_seconds.clamp(60, 3600);
        config.prices.retain(|_, price| price.valid());
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
