use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub layouts: HashMap<String, String>,
    #[serde(default)]
    pub conversion: ConversionConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default)]
    pub autostart: bool,
    #[serde(default = "default_autostart_scope")]
    pub autostart_scope: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConversionConfig {
    #[serde(default = "default_conversion_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_conversion_mode")]
    pub mode: String,
}

fn default_autostart_scope() -> String {
    "user".to_string()
}

fn default_conversion_hotkey() -> String {
    "Pause".to_string()
}

fn default_conversion_mode() -> String {
    "auto".to_string()
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            autostart: false,
            autostart_scope: default_autostart_scope(),
        }
    }
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            hotkey: default_conversion_hotkey(),
            mode: default_conversion_mode(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            layouts: HashMap::new(),
            conversion: ConversionConfig::default(),
        }
    }
}

impl Config {
    /// Returns the config file path: %APPDATA%\LightSwitch\config.toml
    pub fn path() -> PathBuf {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(appdata)
            .join("LightSwitch")
            .join("config.toml")
    }

    /// Loads config from disk, or returns default if file doesn't exist.
    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Saves config to disk.
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content =
            toml::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(path, content)
    }
}
