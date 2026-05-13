use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Describes how the CLI authenticates with the AppSignal API.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthMethod {
    /// Personal API token passed as a query parameter.
    PersonalToken(String),
    /// OAuth access token sent as a Bearer header, with optional refresh support.
    OAuth {
        access_token: String,
        refresh_token: Option<String>,
        /// Seconds since UNIX epoch when the access token expires.
        expires_at: Option<i64>,
    },
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub token: Option<String>,
    pub org: Option<String>,
    pub endpoint: Option<String>,
    /// Optional OAuth client ID override. Defaults to the production client when unset.
    pub oauth_client_id: Option<String>,
    /// OAuth credentials (stored alongside the personal token; OAuth takes precedence).
    pub oauth: Option<OAuthCredentials>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuthCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Seconds since UNIX epoch when the access token expires.
    pub expires_at: Option<i64>,
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

    /// Determine the authentication method to use.
    /// OAuth credentials take precedence over a personal token when present.
    pub fn auth_method(&self) -> Result<AuthMethod> {
        if let Some(ref oauth) = self.oauth {
            return Ok(AuthMethod::OAuth {
                access_token: oauth.access_token.clone(),
                refresh_token: oauth.refresh_token.clone(),
                expires_at: oauth.expires_at,
            });
        }

        let token = self.require_token()?;
        Ok(AuthMethod::PersonalToken(token.to_string()))
    }

    /// Return the configured OAuth client ID override, if present and non-empty.
    pub fn oauth_client_id(&self) -> Option<&str> {
        self.oauth_client_id
            .as_deref()
            .filter(|client_id| !client_id.is_empty())
    }

    /// Return the configured AppSignal base URL, if present.
    ///
    /// The `endpoint` config must be a base URL without a path, for example
    /// `https://staging.lol`.
    pub fn endpoint_base_url(&self) -> Result<Option<String>> {
        self.endpoint
            .as_deref()
            .filter(|endpoint| !endpoint.is_empty())
            .map(normalize_base_url)
            .transpose()
    }

    /// Returns true if the stored OAuth access token has expired (or will expire within 60 s).
    pub fn oauth_token_expired(&self) -> bool {
        if let Some(ref oauth) = self.oauth {
            if let Some(expires_at) = oauth.expires_at {
                let now = chrono::Utc::now().timestamp();
                return now >= expires_at - 60; // 60 s grace window
            }
        }
        false
    }
}

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let mut url =
        url::Url::parse(endpoint).with_context(|| format!("Invalid endpoint URL: {}", endpoint))?;

    if !matches!(url.path(), "" | "/") {
        anyhow::bail!(
            "Invalid endpoint URL: {}. `endpoint` must be a base URL without a path, for example `https://staging.lol`.",
            endpoint
        );
    }

    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
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
            ..Config::default()
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
            ..Config::default()
        };
        let err = config.require_token().unwrap_err();
        assert!(err.to_string().contains("Not authenticated"));
    }

    #[test]
    fn test_serde_round_trip() {
        let config = Config {
            token: Some("my-token".to_string()),
            org: Some("my-org".to_string()),
            ..Config::default()
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
            ..Config::default()
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
            ..Config::default()
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
            ..Config::default()
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
            ..Config::default()
        };
        config1.save_to(&path).unwrap();

        let config2 = Config {
            token: Some("second".to_string()),
            org: Some("new-org".to_string()),
            ..Config::default()
        };
        config2.save_to(&path).unwrap();

        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded, config2);
    }

    #[test]
    fn test_auth_method_personal_token() {
        let config = Config {
            token: Some("my-token".to_string()),
            ..Config::default()
        };
        let method = config.auth_method().unwrap();
        assert_eq!(method, AuthMethod::PersonalToken("my-token".to_string()));
    }

    #[test]
    fn test_auth_method_oauth_takes_precedence() {
        let config = Config {
            token: Some("my-token".to_string()),
            oauth: Some(OAuthCredentials {
                access_token: "oauth-access".to_string(),
                refresh_token: Some("oauth-refresh".to_string()),
                expires_at: Some(9999999999),
            }),
            ..Config::default()
        };
        let method = config.auth_method().unwrap();
        assert_eq!(
            method,
            AuthMethod::OAuth {
                access_token: "oauth-access".to_string(),
                refresh_token: Some("oauth-refresh".to_string()),
                expires_at: Some(9999999999),
            }
        );
    }

    #[test]
    fn test_auth_method_no_credentials() {
        let config = Config::default();
        let err = config.auth_method().unwrap_err();
        assert!(err.to_string().contains("Not authenticated"));
    }

    #[test]
    fn test_oauth_credentials_serde_round_trip() {
        let config = Config {
            token: None,
            org: Some("my-org".to_string()),
            endpoint: None,
            oauth_client_id: None,
            oauth: Some(OAuthCredentials {
                access_token: "access-tok".to_string(),
                refresh_token: Some("refresh-tok".to_string()),
                expires_at: Some(1700000000),
            }),
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_oauth_token_expired_when_expired() {
        let config = Config {
            oauth: Some(OAuthCredentials {
                access_token: "tok".to_string(),
                refresh_token: None,
                expires_at: Some(0), // epoch = long expired
            }),
            ..Config::default()
        };
        assert!(config.oauth_token_expired());
    }

    #[test]
    fn test_oauth_token_not_expired_when_future() {
        let config = Config {
            oauth: Some(OAuthCredentials {
                access_token: "tok".to_string(),
                refresh_token: None,
                expires_at: Some(9999999999),
            }),
            ..Config::default()
        };
        assert!(!config.oauth_token_expired());
    }

    #[test]
    fn test_oauth_token_expired_no_oauth() {
        let config = Config::default();
        assert!(!config.oauth_token_expired());
    }

    #[test]
    fn test_save_and_load_oauth_credentials() {
        let dir = TempDir::new().unwrap();
        let path = config_path(&dir);

        let config = Config {
            token: None,
            org: Some("test-org".to_string()),
            endpoint: None,
            oauth_client_id: None,
            oauth: Some(OAuthCredentials {
                access_token: "acc-tok".to_string(),
                refresh_token: Some("ref-tok".to_string()),
                expires_at: Some(1700000000),
            }),
        };
        config.save_to(&path).unwrap();

        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_oauth_client_id_override_returns_none_when_missing() {
        let config = Config::default();
        assert_eq!(config.oauth_client_id(), None);
    }

    #[test]
    fn test_oauth_client_id_override_returns_none_when_empty() {
        let config = Config {
            oauth_client_id: Some(String::new()),
            ..Config::default()
        };
        assert_eq!(config.oauth_client_id(), None);
    }

    #[test]
    fn test_oauth_client_id_override_returns_value_when_present() {
        let config = Config {
            oauth_client_id: Some("staging-client-id".to_string()),
            ..Config::default()
        };
        assert_eq!(config.oauth_client_id(), Some("staging-client-id"));
    }

    #[test]
    fn test_endpoint_base_url_returns_none_when_missing() {
        let config = Config::default();
        assert_eq!(config.endpoint_base_url().unwrap(), None);
    }

    #[test]
    fn test_endpoint_base_url_returns_base_url_when_valid() {
        let config = Config {
            endpoint: Some("https://staging.lol".to_string()),
            ..Config::default()
        };
        assert_eq!(
            config.endpoint_base_url().unwrap(),
            Some("https://staging.lol/".to_string())
        );
    }

    #[test]
    fn test_endpoint_base_url_rejects_graphql_path() {
        let config = Config {
            endpoint: Some("https://staging.lol/graphql".to_string()),
            ..Config::default()
        };
        let err = config.endpoint_base_url().unwrap_err();
        assert!(err
            .to_string()
            .contains("must be a base URL without a path"));
    }
}
