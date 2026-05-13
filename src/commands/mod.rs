pub mod about;
pub mod apps;
pub mod auth;
pub mod incidents;
pub mod logs;

use crate::api::AppSignalClient;
use crate::config::Config;
use crate::oauth;
use anyhow::Result;

/// Resolve the organization slug: use explicit --org if given, fall back to config.
pub fn resolve_org(explicit: Option<&str>, config: &Config) -> Result<String> {
    if let Some(org) = explicit {
        return Ok(org.to_string());
    }
    config
        .org
        .as_deref()
        .filter(|o| !o.is_empty())
        .map(|o| o.to_string())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No organization configured. Run `appsignal-cli apps list --org <slug>` first, \
                 or set it with `appsignal-cli apps set-org --org <slug>`."
            )
        })
}

/// Create an authenticated [`AppSignalClient`] from the current config.
///
/// If OAuth credentials are present and the access token has expired, this
/// function automatically refreshes the token and persists the updated
/// credentials before returning the client.
pub async fn authenticated_client(config: &mut Config) -> Result<AppSignalClient> {
    let endpoint = config.endpoint_base_url()?;

    // Auto-refresh expired OAuth tokens
    if config.oauth.is_some() && config.oauth_token_expired() {
        let oauth = config.oauth.as_ref().unwrap();
        if let Some(ref refresh_token) = oauth.refresh_token {
            let new_creds = oauth::refresh_access_token(
                endpoint.as_deref(),
                config.oauth_client_id(),
                refresh_token,
            )
            .await?;
            config.oauth = Some(new_creds);
            config.save()?;
        } else {
            anyhow::bail!(
                "OAuth access token has expired and no refresh token is available.\n\
                 Please re-authenticate with `appsignal-cli auth login --oauth`."
            );
        }
    }

    let auth = config.auth_method()?;
    Ok(AppSignalClient::with_auth(auth, endpoint.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_org_explicit() {
        let config = Config::default();
        let result = resolve_org(Some("explicit-org"), &config).unwrap();
        assert_eq!(result, "explicit-org");
    }

    #[test]
    fn test_resolve_org_explicit_overrides_config() {
        let config = Config {
            org: Some("config-org".to_string()),
            ..Config::default()
        };
        let result = resolve_org(Some("explicit-org"), &config).unwrap();
        assert_eq!(result, "explicit-org");
    }

    #[test]
    fn test_resolve_org_from_config() {
        let config = Config {
            org: Some("config-org".to_string()),
            ..Config::default()
        };
        let result = resolve_org(None, &config).unwrap();
        assert_eq!(result, "config-org");
    }

    #[test]
    fn test_resolve_org_empty_config() {
        let config = Config {
            org: Some("".to_string()),
            ..Config::default()
        };
        let err = resolve_org(None, &config).unwrap_err();
        assert!(err.to_string().contains("No organization configured"));
    }

    #[test]
    fn test_resolve_org_none() {
        let config = Config::default();
        let err = resolve_org(None, &config).unwrap_err();
        assert!(err.to_string().contains("No organization configured"));
    }
}
