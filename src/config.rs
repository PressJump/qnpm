// config.rs

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub cache_dir: PathBuf,
}

impl Config {
    pub fn new() -> Self {
        Self {
            cache_dir: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
        }
    }

    // Function to load config from a file
    pub fn load(config_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if config_path.exists() {
            let config_str = std::fs::read_to_string(config_path)?;
            Ok(serde_json::from_str(&config_str)?)
        } else {
            Ok(Self::new())
        }
    }

    // Function to save config to a file
    pub fn save(&self, config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let config_str = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, config_str)?;
        Ok(())
    }
}
