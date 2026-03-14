use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub token: Option<String>,
}

impl Config {
    /// Returns the path to the config file: ~/.config/appsignal/config.toml
    fn path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("appsignal");
        Ok(config_dir.join("config.toml"))
    }

    /// Load the config from disk, or return defaults if it doesn't exist.
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;
        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config at {}", path.display()))?;
        Ok(config)
    }

    /// Persist the config to disk.
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config dir {}", parent.display()))?;
        }
        let contents = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&path, contents)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;
        Ok(())
    }

    /// Delete the config file.
    pub fn delete() -> Result<()> {
        let path = Self::path()?;
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete config at {}", path.display()))?;
        }
        Ok(())
    }

    /// Return the stored token, or error with a helpful message.
    pub fn require_token(&self) -> Result<&str> {
        self.token
            .as_deref()
            .filter(|t| !t.is_empty())
            .context("Not authenticated. Run `appsignal-cli auth login` first.")
    }
}
