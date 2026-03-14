use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub token: Option<String>,
    pub org: Option<String>,
}

impl Config {
    /// Returns the default path to the config file: ~/.config/appsignal/config.toml
    fn default_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("appsignal");
        Ok(config_dir.join("config.toml"))
    }

    /// Load the config from disk, or return defaults if it doesn't exist.
    pub fn load() -> Result<Self> {
        Self::load_from(&Self::default_path()?)
    }

    /// Load the config from a specific path.
    pub fn load_from(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;
        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config at {}", path.display()))?;
        Ok(config)
    }

    /// Persist the config to disk.
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::default_path()?)
    }

    /// Persist the config to a specific path.
    pub fn save_to(&self, path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config dir {}", parent.display()))?;
        }
        let contents = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(path, contents)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;
        Ok(())
    }

    /// Delete the config file.
    pub fn delete() -> Result<()> {
        Self::delete_at(&Self::default_path()?)
    }

    /// Delete the config file at a specific path.
    pub fn delete_at(path: &PathBuf) -> Result<()> {
        if path.exists() {
            fs::remove_file(path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn config_path(dir: &TempDir) -> PathBuf {
        dir.path().join("config.toml")
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.token, None);
        assert_eq!(config.org, None);
    }

    #[test]
    fn test_require_token_with_token() {
        let config = Config {
            token: Some("abc123".to_string()),
            org: None,
        };
        assert_eq!(config.require_token().unwrap(), "abc123");
    }

    #[test]
    fn test_require_token_without_token() {
        let config = Config::default();
        let err = config.require_token().unwrap_err();
        assert!(err.to_string().contains("Not authenticated"));
    }

    #[test]
    fn test_require_token_with_empty_token() {
        let config = Config {
            token: Some("".to_string()),
            org: None,
        };
        let err = config.require_token().unwrap_err();
        assert!(err.to_string().contains("Not authenticated"));
    }

    #[test]
    fn test_serde_round_trip() {
        let config = Config {
            token: Some("my-token".to_string()),
            org: Some("my-org".to_string()),
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_serde_round_trip_empty() {
        let config = Config::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_load_from_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = config_path(&dir);
        let config = Config::load_from(&path).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn test_save_and_load() {
        let dir = TempDir::new().unwrap();
        let path = config_path(&dir);

        let config = Config {
            token: Some("test-token".to_string()),
            org: Some("test-org".to_string()),
        };
        config.save_to(&path).unwrap();

        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_save_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("dir").join("config.toml");

        let config = Config {
            token: Some("tok".to_string()),
            org: None,
        };
        config.save_to(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_delete_existing_file() {
        let dir = TempDir::new().unwrap();
        let path = config_path(&dir);

        let config = Config {
            token: Some("tok".to_string()),
            org: None,
        };
        config.save_to(&path).unwrap();
        assert!(path.exists());

        Config::delete_at(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_delete_missing_file_is_ok() {
        let dir = TempDir::new().unwrap();
        let path = config_path(&dir);
        // Should not error when file doesn't exist
        Config::delete_at(&path).unwrap();
    }

    #[test]
    fn test_load_partial_config() {
        let dir = TempDir::new().unwrap();
        let path = config_path(&dir);
        fs::write(&path, "token = \"only-token\"\n").unwrap();

        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.token, Some("only-token".to_string()));
        assert_eq!(config.org, None);
    }

    #[test]
    fn test_save_overwrites_existing() {
        let dir = TempDir::new().unwrap();
        let path = config_path(&dir);

        let config1 = Config {
            token: Some("first".to_string()),
            org: None,
        };
        config1.save_to(&path).unwrap();

        let config2 = Config {
            token: Some("second".to_string()),
            org: Some("new-org".to_string()),
        };
        config2.save_to(&path).unwrap();

        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded, config2);
    }
}
