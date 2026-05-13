use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const LOCAL_CONFIG_FILE_NAME: &str = ".appsignal.toml";

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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub token: Option<String>,
    pub org: Option<String>,
    pub endpoint: Option<String>,
    /// Optional OAuth client ID override. Defaults to the production client when unset.
    pub oauth_client_id: Option<String>,
    /// OAuth credentials (stored alongside the personal token; OAuth takes precedence).
    pub oauth: Option<OAuthCredentials>,
    /// When false in a local config, prevent falling back to inherited global credentials.
    pub inherit_auth: Option<bool>,
    #[serde(skip)]
    pub(crate) active_path: Option<PathBuf>,
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

    /// Returns the nearest project-local config override, if one exists.
    fn local_override_path() -> Result<Option<PathBuf>> {
        let current_dir =
            std::env::current_dir().context("Could not determine current directory")?;
        Ok(Self::local_override_path_from(&current_dir))
    }

    fn local_override_path_from(start_dir: &Path) -> Option<PathBuf> {
        let project_root = Self::project_root_from(start_dir);
        Self::local_override_path_with_project_root(start_dir, project_root.as_deref())
    }

    fn local_override_path_with_project_root(
        start_dir: &Path,
        project_root: Option<&Path>,
    ) -> Option<PathBuf> {
        for dir in start_dir.ancestors() {
            let path = dir.join(LOCAL_CONFIG_FILE_NAME);
            if path.exists() {
                return Some(path);
            }

            if project_root == Some(dir) || project_root.is_none() {
                break;
            }
        }

        None
    }

    fn project_root_from(start_dir: &Path) -> Option<PathBuf> {
        start_dir
            .ancestors()
            .find(|dir| dir.join(".git").exists())
            .map(Path::to_path_buf)
    }

    fn local_override_target_path() -> Result<PathBuf> {
        let current_dir =
            std::env::current_dir().context("Could not determine current directory")?;
        Ok(Self::local_override_target_path_from(&current_dir))
    }

    fn local_override_target_path_from(start_dir: &Path) -> PathBuf {
        let project_root = Self::project_root_from(start_dir);
        Self::local_override_target_path_with_project_root(start_dir, project_root.as_deref())
    }

    fn local_override_target_path_with_project_root(
        start_dir: &Path,
        project_root: Option<&Path>,
    ) -> PathBuf {
        if let Some(path) = Self::local_override_path_with_project_root(start_dir, project_root) {
            path
        } else if let Some(project_root) = project_root {
            project_root.join(LOCAL_CONFIG_FILE_NAME)
        } else {
            start_dir.join(LOCAL_CONFIG_FILE_NAME)
        }
    }

    /// Load the effective config from disk.
    ///
    /// Global config is loaded from `~/.config/appsignal/config.toml`. If the
    /// current directory (or one of its parents) contains `.appsignal.toml`,
    /// that file is layered on top as a project-local override.
    pub fn load() -> Result<Self> {
        let global_path = Self::default_path()?;
        let local_path = Self::local_override_path()?;
        Self::load_merged(&global_path, local_path.as_deref())
    }

    fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;
        let mut config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config at {}", path.display()))?;
        config.active_path = None;
        Ok(config)
    }

    fn load_merged(global_path: &Path, local_path: Option<&Path>) -> Result<Self> {
        let mut config = Self::load_from_path(global_path)?;

        if let Some(local_path) = local_path {
            let local_config = Self::load_from_path(local_path)?;
            config.apply_override(local_config);
            config.active_path = Some(local_path.to_path_buf());
        } else {
            config.active_path = Some(global_path.to_path_buf());
        }

        Ok(config)
    }

    pub(crate) fn load_local_only() -> Result<Self> {
        let path = Self::local_override_target_path()?;
        Self::load_local_only_at(&path)
    }

    fn load_local_only_at(path: &Path) -> Result<Self> {
        let mut config = Self::load_from_path(path)?;
        config.active_path = Some(path.to_path_buf());
        Ok(config)
    }

    fn apply_override(&mut self, override_config: Config) {
        let blocks_inherited_auth = override_config.inherit_auth == Some(false)
            && override_config.token.is_none()
            && override_config.oauth.is_none();

        if override_config.token.is_some() {
            self.token = override_config.token;
        }
        if override_config.org.is_some() {
            self.org = override_config.org;
        }
        if override_config.endpoint.is_some() {
            self.endpoint = override_config.endpoint;
        }
        if override_config.oauth_client_id.is_some() {
            self.oauth_client_id = override_config.oauth_client_id;
        }
        if override_config.oauth.is_some() {
            self.oauth = override_config.oauth;
        }
        if override_config.inherit_auth.is_some() {
            self.inherit_auth = override_config.inherit_auth;
        }

        if blocks_inherited_auth {
            self.token = None;
            self.oauth = None;
        }
    }

    /// Persist the config to the active config file.
    ///
    /// Config loaded through `Config::load()` is saved back to the project-local
    /// override when one is active, otherwise to the global config file.
    pub fn save(&self) -> Result<()> {
        let path = self.active_path.clone().unwrap_or(Self::default_path()?);
        self.save_to(&path)
    }

    /// Clear auth credentials from the active config scope.
    pub fn clear_credentials(&mut self) -> Result<()> {
        self.token = None;
        self.oauth = None;
        self.inherit_auth = if self.active_path_is_local()? {
            Some(false)
        } else {
            None
        };
        Ok(())
    }

    /// Returns the config file path currently in use.
    pub fn active_path(&self) -> Option<&Path> {
        self.active_path.as_deref()
    }

    fn active_path_is_local(&self) -> Result<bool> {
        match self.active_path.as_deref() {
            Some(path) => Ok(path != Self::default_path()?.as_path()),
            None => Ok(false),
        }
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
            if oauth.access_token.is_empty() {
                return self
                    .token
                    .as_deref()
                    .filter(|t| !t.is_empty())
                    .map(|token| AuthMethod::PersonalToken(token.to_string()))
                    .context("Not authenticated. Run `appsignal-cli auth login` first.");
            }

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

impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token
            && self.org == other.org
            && self.endpoint == other.endpoint
            && self.oauth_client_id == other.oauth_client_id
            && self.oauth == other.oauth
            && self.inherit_auth == other.inherit_auth
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

    fn local_config_path(dir: &TempDir) -> PathBuf {
        dir.path().join(LOCAL_CONFIG_FILE_NAME)
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
        let config = Config::load_from_path(&path).unwrap();
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

        let loaded = Config::load_from_path(&path).unwrap();
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

        fs::remove_file(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_delete_missing_file_is_ok() {
        let dir = TempDir::new().unwrap();
        let path = config_path(&dir);
        // Should not error when file doesn't exist
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
    }

    #[test]
    fn test_load_partial_config() {
        let dir = TempDir::new().unwrap();
        let path = config_path(&dir);
        fs::write(&path, "token = \"only-token\"\n").unwrap();

        let config = Config::load_from_path(&path).unwrap();
        assert_eq!(config.token, Some("only-token".to_string()));
        assert_eq!(config.org, None);
    }

    #[test]
    fn test_local_override_path_from_finds_nearest_project_config() {
        let dir = TempDir::new().unwrap();
        let project_root = dir.path().join("project");
        let nested_dir = project_root.join("src").join("bin");
        fs::create_dir_all(&nested_dir).unwrap();

        let local_path = project_root.join(LOCAL_CONFIG_FILE_NAME);
        fs::write(&local_path, "endpoint = \"https://staging.lol\"\n").unwrap();

        assert_eq!(
            Config::local_override_path_with_project_root(&nested_dir, Some(&project_root)),
            Some(local_path)
        );
    }

    #[test]
    fn test_load_merges_local_override_over_global() {
        let global_dir = TempDir::new().unwrap();
        let local_dir = TempDir::new().unwrap();
        let global_path = config_path(&global_dir);
        let local_path = local_config_path(&local_dir);

        fs::write(
            &global_path,
            concat!(
                "token = \"global-token\"\n",
                "org = \"global-org\"\n",
                "endpoint = \"https://appsignal.com\"\n"
            ),
        )
        .unwrap();
        fs::write(
            &local_path,
            concat!(
                "org = \"local-org\"\n",
                "endpoint = \"https://staging.lol\"\n",
                "oauth_client_id = \"staging-client-id\"\n",
                "[oauth]\n",
                "access_token = \"local-access\"\n",
                "refresh_token = \"local-refresh\"\n",
                "expires_at = 1700000000\n"
            ),
        )
        .unwrap();

        let config = Config::load_merged(&global_path, Some(&local_path)).unwrap();

        assert_eq!(config.token, Some("global-token".to_string()));
        assert_eq!(config.org, Some("local-org".to_string()));
        assert_eq!(
            config.endpoint_base_url().unwrap(),
            Some("https://staging.lol/".to_string())
        );
        assert_eq!(config.oauth_client_id(), Some("staging-client-id"));
        assert_eq!(
            config.oauth,
            Some(OAuthCredentials {
                access_token: "local-access".to_string(),
                refresh_token: Some("local-refresh".to_string()),
                expires_at: Some(1700000000),
            })
        );
    }

    #[test]
    fn test_load_local_only_at_does_not_copy_global_credentials() {
        let local_dir = TempDir::new().unwrap();
        let local_path = local_config_path(&local_dir);

        let config = Config::load_local_only_at(&local_path).unwrap();

        assert_eq!(config.token, None);
        assert_eq!(config.oauth, None);
        assert_eq!(config.active_path(), Some(local_path.as_path()));
    }

    #[test]
    fn test_inherit_auth_false_blocks_global_credentials() {
        let global_dir = TempDir::new().unwrap();
        let local_dir = TempDir::new().unwrap();
        let global_path = config_path(&global_dir);
        let local_path = local_config_path(&local_dir);

        fs::write(
            &global_path,
            concat!(
                "token = \"global-token\"\n",
                "[oauth]\n",
                "access_token = \"global-access\"\n"
            ),
        )
        .unwrap();
        fs::write(&local_path, "inherit_auth = false\n").unwrap();

        let config = Config::load_merged(&global_path, Some(&local_path)).unwrap();

        assert_eq!(config.token, None);
        assert_eq!(config.oauth, None);
        assert!(config.auth_method().is_err());
    }

    #[test]
    fn test_local_override_target_path_from_prefers_existing_override() {
        let dir = TempDir::new().unwrap();
        let project_root = dir.path().join("project");
        let nested_dir = project_root.join("src").join("bin");
        fs::create_dir_all(&nested_dir).unwrap();

        let local_path = project_root.join(LOCAL_CONFIG_FILE_NAME);
        fs::write(&local_path, "org = \"test-org\"\n").unwrap();

        assert_eq!(
            Config::local_override_target_path_with_project_root(&nested_dir, Some(&project_root)),
            local_path
        );
    }

    #[test]
    fn test_local_override_target_path_from_uses_git_root_when_no_override_exists() {
        let dir = TempDir::new().unwrap();
        let project_root = dir.path().join("project");
        let nested_dir = project_root.join("src").join("bin");
        fs::create_dir_all(project_root.join(".git")).unwrap();
        fs::create_dir_all(&nested_dir).unwrap();

        assert_eq!(
            Config::local_override_target_path_with_project_root(&nested_dir, Some(&project_root)),
            project_root.join(LOCAL_CONFIG_FILE_NAME)
        );
    }

    #[test]
    fn test_local_override_target_path_from_falls_back_to_current_dir() {
        let dir = TempDir::new().unwrap();
        let nested_dir = dir.path().join("scratch");
        fs::create_dir_all(&nested_dir).unwrap();

        assert_eq!(
            Config::local_override_target_path_with_project_root(&nested_dir, None),
            nested_dir.join(LOCAL_CONFIG_FILE_NAME)
        );
    }

    #[test]
    fn test_local_override_path_from_does_not_escape_current_dir_outside_git() {
        let dir = TempDir::new().unwrap();
        let nested_dir = dir.path().join("scratch");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(local_config_path(&dir), "org = \"temp-root\"\n").unwrap();

        assert_eq!(
            Config::local_override_path_with_project_root(&nested_dir, None),
            None
        );
    }

    #[test]
    fn test_save_persists_to_local_override_when_active() {
        let global_dir = TempDir::new().unwrap();
        let local_dir = TempDir::new().unwrap();
        let global_path = config_path(&global_dir);
        let local_path = local_config_path(&local_dir);

        fs::write(&global_path, "org = \"global-org\"\n").unwrap();
        fs::write(&local_path, "endpoint = \"https://staging.lol\"\n").unwrap();

        let mut config = Config::load_merged(&global_path, Some(&local_path)).unwrap();
        config.token = Some("local-token".to_string());
        config.save().unwrap();

        let global_config = Config::load_from_path(&global_path).unwrap();
        let local_config = Config::load_from_path(&local_path).unwrap();

        assert_eq!(global_config.token, None);
        assert_eq!(global_config.org, Some("global-org".to_string()));
        assert_eq!(local_config.token, Some("local-token".to_string()));
        assert_eq!(
            local_config.endpoint,
            Some("https://staging.lol".to_string())
        );
    }

    #[test]
    fn test_clear_credentials_sets_local_inherit_auth_override() {
        let dir = TempDir::new().unwrap();
        let local_path = local_config_path(&dir);

        let mut config = Config {
            token: Some("secret".to_string()),
            active_path: Some(local_path),
            ..Config::default()
        };

        config.clear_credentials().unwrap();

        assert_eq!(config.token, None);
        assert_eq!(config.oauth, None);
        assert_eq!(config.inherit_auth, Some(false));
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

        let loaded = Config::load_from_path(&path).unwrap();
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
            ..Config::default()
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
            ..Config::default()
        };
        config.save_to(&path).unwrap();

        let loaded = Config::load_from_path(&path).unwrap();
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
