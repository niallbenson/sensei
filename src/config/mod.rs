//! Configuration management for Sensei

pub mod progress;
pub mod session;

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::theme::Theme;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Selected theme name
    pub theme: String,

    /// Custom theme overrides (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_theme: Option<Theme>,

    /// Animation speed multiplier (1.0 = normal, 0.0 = instant)
    pub animation_speed: f32,

    /// Vim mode enabled
    pub vim_mode: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "Tokyo Night".to_string(),
            custom_theme: None,
            animation_speed: 1.0,
            vim_mode: true,
        }
    }
}

impl Config {
    /// Load configuration from disk, or create default if not exists
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config from {:?}", config_path))?;
            serde_json::from_str(&contents).with_context(|| "Failed to parse config.json")
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to disk
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory {:?}", parent))?;
        }

        let contents =
            serde_json::to_string_pretty(self).with_context(|| "Failed to serialize config")?;

        std::fs::write(&config_path, contents)
            .with_context(|| format!("Failed to write config to {:?}", config_path))?;

        Ok(())
    }

    /// Get the path to the config file
    pub fn config_path() -> Result<PathBuf> {
        let proj_dirs =
            ProjectDirs::from("", "", "sensei").context("Failed to determine config directory")?;
        Ok(proj_dirs.config_dir().join("config.json"))
    }

    /// Get the data directory path
    pub fn data_dir() -> Result<PathBuf> {
        let proj_dirs =
            ProjectDirs::from("", "", "sensei").context("Failed to determine data directory")?;
        Ok(proj_dirs.data_dir().to_path_buf())
    }

    /// Get the books directory path
    pub fn books_dir() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("books"))
    }

    /// Get the notes directory path
    pub fn notes_dir() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("notes"))
    }

    /// Get the active theme
    pub fn active_theme(&self) -> Theme {
        self.custom_theme.clone().unwrap_or_else(Theme::tokyo_night)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_tokyo_night_theme() {
        let config = Config::default();
        assert_eq!(config.theme, "Tokyo Night");
    }

    #[test]
    fn default_config_has_vim_mode_enabled() {
        let config = Config::default();
        assert!(config.vim_mode);
    }

    #[test]
    fn config_serializes_to_json() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("Tokyo Night"));
    }

    #[test]
    fn config_deserializes_from_json() {
        let json = r#"{"theme":"Custom","animation_speed":0.5,"vim_mode":false}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, "Custom");
        assert!(!config.vim_mode);
    }
}
