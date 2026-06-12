use anyhow::Result;
use serde::Serialize;

use crate::api::AppSignalClient;
use crate::config::{AuthMethod, Config, OAuthCredentials};
use crate::error::CliError;
use crate::oauth;
use crate::output::Output;

#[derive(Serialize)]
struct AuthActionResult<'a> {
    authenticated: bool,
    method: Option<&'a str>,
    message: String,
}

#[derive(Serialize)]
struct AuthStatusResponse {
    authenticated: bool,
    method: Option<&'static str>,
    token: Option<String>,
    expires_at: Option<i64>,
    expired: bool,
    message: String,
}

pub struct LoginOptions {
    pub token: Option<String>,
    pub use_oauth: bool,
    pub endpoint: Option<String>,
    pub rest_endpoint: Option<String>,
    pub oauth_client_id: Option<String>,
    pub org: Option<String>,
}

impl LoginOptions {
    fn uses_oauth(&self) -> bool {
        self.use_oauth || self.token.is_none()
    }
}

/// Authenticate with AppSignal.
///
/// OAuth is the default login method. When a token is provided explicitly via
/// `--token`, the CLI stores that personal API token instead.
pub async fn login(options: LoginOptions, format: Output) -> Result<()> {
    let use_oauth = options.uses_oauth();
    let mut config = Config::load()?;
    apply_login_config_overrides(
        &mut config,
        options.endpoint,
        options.rest_endpoint,
        options.oauth_client_id,
        options.org,
    );

    let endpoint = config.endpoint_base_url()?;

    if use_oauth {
        let oauth_result =
            oauth::perform_oauth_flow(endpoint.as_deref(), config.oauth_client_id()).await?;

        crate::status!("Validating OAuth token...");

        let auth = AuthMethod::OAuth {
            access_token: oauth_result.credentials.access_token.clone(),
            refresh_token: oauth_result.credentials.refresh_token.clone(),
            expires_at: oauth_result.credentials.expires_at,
        };
        let client = AppSignalClient::with_auth(auth, endpoint.as_deref());
        match client.validate_token().await {
            Ok(_) => crate::status!("OK"),
            Err(e) => {
                crate::status!("FAILED");
                return Err(e);
            }
        }

        store_oauth_credentials(
            &mut config,
            oauth_result.client_id,
            oauth_result.credentials,
        );
        config.save()?;

        print_active_config_path(&config);
        return crate::output::print_with(
            AuthActionResult {
                authenticated: true,
                method: Some("oauth"),
                message: "OAuth credentials saved. You are now authenticated.".to_string(),
            },
            format,
            |w| writeln!(w, "OAuth credentials saved. You are now authenticated."),
        );
    }

    let token = options
        .token
        .expect("token login requires an explicit token");

    if token.is_empty() {
        anyhow::bail!(CliError::msg("Token cannot be empty"));
    }

    crate::status!("Validating token...");

    let client = AppSignalClient::new(&token, endpoint.as_deref());
    match client.validate_token().await {
        Ok(_) => crate::status!("OK"),
        Err(e) => {
            crate::status!("FAILED");
            return Err(e);
        }
    }

    config.oauth = None;
    config.token = Some(token);
    config.save()?;

    print_active_config_path(&config);
    crate::output::print_with(
        AuthActionResult {
            authenticated: true,
            method: Some("token"),
            message: "Token saved. You are now authenticated.".to_string(),
        },
        format,
        |w| writeln!(w, "Token saved. You are now authenticated."),
    )
}

fn store_oauth_credentials(config: &mut Config, client_id: String, credentials: OAuthCredentials) {
    config.token = None;
    config.oauth_client_id = Some(client_id);
    config.oauth = Some(credentials);
}

fn apply_login_config_overrides(
    config: &mut Config,
    endpoint: Option<String>,
    rest_endpoint: Option<String>,
    oauth_client_id: Option<String>,
    org: Option<String>,
) {
    if let Some(endpoint) = endpoint {
        config.endpoint = Some(endpoint);
    }

    if let Some(rest_endpoint) = rest_endpoint {
        config.rest_endpoint = Some(rest_endpoint);
    }

    if let Some(oauth_client_id) = oauth_client_id {
        config.oauth_client_id = Some(oauth_client_id);
    }

    if let Some(org) = org {
        config.org = Some(org);
    }
}

fn print_active_config_path(config: &Config) {
    if let Some(path) = config.active_path() {
        crate::status!("Saved config to {}", path.display());
    }
}

/// Remove stored credentials.
pub fn logout(format: Output) -> Result<()> {
    let mut config = Config::load()?;
    config.clear_credentials();
    config.save()?;
    print_active_config_path(&config);
    crate::output::print_with(
        AuthActionResult {
            authenticated: false,
            method: None,
            message: "Logged out. Credentials removed from active config.".to_string(),
        },
        format,
        |w| writeln!(w, "Logged out. Credentials removed from active config."),
    )
}

/// Show current authentication status.
pub fn status(format: Output) -> Result<()> {
    let config = Config::load()?;

    if let Some(path) = config.active_path() {
        crate::status!("Using config: {}", path.display());
    }

    let response = if let Some(ref oauth) = config.oauth {
        let masked = mask_token(&oauth.access_token);
        let expiry = oauth.expires_at.map(|ts| {
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });
        let expired = config.oauth_token_expired();
        let mut message = format!(
            "Authenticated via OAuth (token: {}, expires: {})",
            masked,
            expiry.as_deref().unwrap_or("no expiry")
        );
        if expired {
            message.push_str(
                "\n  Note: access token has expired and will be refreshed on next API call.",
            );
        }

        AuthStatusResponse {
            authenticated: true,
            method: Some("oauth"),
            token: Some(masked),
            expires_at: oauth.expires_at,
            expired,
            message,
        }
    } else {
        match config.token {
            Some(ref t) if !t.is_empty() => {
                let masked = mask_token(t);
                AuthStatusResponse {
                    authenticated: true,
                    method: Some("token"),
                    token: Some(masked.clone()),
                    expires_at: None,
                    expired: false,
                    message: format!("Authenticated via personal token (token: {})", masked),
                }
            }
            _ => AuthStatusResponse {
                authenticated: false,
                method: None,
                token: None,
                expires_at: None,
                expired: false,
                message: "Not authenticated. Run `appsignal-cli auth login` to set up.".to_string(),
            },
        }
    };

    let human_message = response.message.clone();
    crate::output::print_with(response, format, move |w| writeln!(w, "{}", human_message))
}

/// Mask a token for display: first 4 chars + "..." + last 4 chars.
fn mask_token(token: &str) -> String {
    if token.len() > 8 {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    } else {
        "****".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_login_config_overrides_updates_config_values() {
        let mut config = Config::default();

        apply_login_config_overrides(
            &mut config,
            Some("https://staging.lol".to_string()),
            Some("https://public-api.lol".to_string()),
            Some("staging-client-id".to_string()),
            Some("side-project".to_string()),
        );

        assert_eq!(config.endpoint, Some("https://staging.lol".to_string()));
        assert_eq!(
            config.rest_endpoint,
            Some("https://public-api.lol".to_string())
        );
        assert_eq!(
            config.oauth_client_id,
            Some("staging-client-id".to_string())
        );
        assert_eq!(config.org, Some("side-project".to_string()));
    }

    #[test]
    fn test_store_oauth_credentials_persists_client_id_and_clears_token() {
        let mut config = Config {
            token: Some("personal-token".to_string()),
            ..Config::default()
        };

        let credentials = OAuthCredentials {
            access_token: "oauth-access".to_string(),
            refresh_token: Some("oauth-refresh".to_string()),
            expires_at: Some(1_700_000_000),
        };

        store_oauth_credentials(
            &mut config,
            "registered-client-id".to_string(),
            credentials.clone(),
        );

        assert_eq!(config.token, None);
        assert_eq!(config.oauth_client_id(), Some("registered-client-id"));
        assert_eq!(config.oauth, Some(credentials));
    }

    #[test]
    fn test_login_defaults_to_oauth_without_token() {
        let options = LoginOptions {
            token: None,
            use_oauth: false,
            endpoint: None,
            rest_endpoint: None,
            oauth_client_id: None,
            org: None,
        };

        assert!(options.uses_oauth());
    }

    #[test]
    fn test_login_uses_token_flow_when_token_is_provided() {
        let options = LoginOptions {
            token: Some("personal-token".to_string()),
            use_oauth: false,
            endpoint: None,
            rest_endpoint: None,
            oauth_client_id: None,
            org: None,
        };

        assert!(!options.uses_oauth());
    }
}
