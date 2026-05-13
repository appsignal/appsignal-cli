use anyhow::Result;
use std::io::{self, Write};

use crate::api::AppSignalClient;
use crate::config::{AuthMethod, Config};
use crate::oauth;

pub struct LoginOptions {
    pub token: Option<String>,
    pub use_oauth: bool,
    pub endpoint: Option<String>,
    pub oauth_client_id: Option<String>,
    pub org: Option<String>,
}

/// Prompt the user for a token interactively if not provided via --token.
fn prompt_token() -> Result<String> {
    print!("Enter your AppSignal personal API token: ");
    io::stdout().flush()?;
    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    Ok(token.trim().to_string())
}

/// Authenticate with AppSignal.
///
/// When `use_oauth` is true the CLI runs the OAuth PKCE flow (opens a browser).
/// Otherwise, a personal API token is expected via `--token` or interactive prompt.
pub async fn login(options: LoginOptions) -> Result<()> {
    let mut config = Config::load()?;
    apply_login_config_overrides(
        &mut config,
        options.endpoint,
        options.oauth_client_id,
        options.org,
    );

    let endpoint = config.endpoint_base_url()?;

    if options.use_oauth {
        // --- OAuth flow ---
        let credentials =
            oauth::perform_oauth_flow(endpoint.as_deref(), config.oauth_client_id()).await?;

        // Validate the new OAuth token
        print!("Validating OAuth token... ");
        io::stdout().flush()?;

        let auth = AuthMethod::OAuth {
            access_token: credentials.access_token.clone(),
            refresh_token: credentials.refresh_token.clone(),
            expires_at: credentials.expires_at,
        };
        let client = AppSignalClient::with_auth(auth, endpoint.as_deref());
        match client.validate_token().await {
            Ok(_) => println!("OK"),
            Err(e) => {
                println!("FAILED");
                anyhow::bail!("OAuth token validation failed: {}", e);
            }
        }

        // Clear any existing personal token when switching to OAuth
        config.token = None;
        config.oauth = Some(credentials);
        config.save()?;

        println!("OAuth credentials saved. You are now authenticated.");
        print_active_config_path(&config);
    } else {
        // --- Personal token flow (existing behavior) ---
        let token = match options.token {
            Some(t) => t,
            None => prompt_token()?,
        };

        if token.is_empty() {
            anyhow::bail!("Token cannot be empty");
        }

        print!("Validating token... ");
        io::stdout().flush()?;

        let client = AppSignalClient::new(&token, endpoint.as_deref());
        match client.validate_token().await {
            Ok(_) => println!("OK"),
            Err(e) => {
                println!("FAILED");
                anyhow::bail!("Token validation failed: {}", e);
            }
        }

        // Clear any existing OAuth credentials when switching to a personal token
        config.oauth = None;
        config.token = Some(token);
        config.save()?;

        println!("Token saved. You are now authenticated.");
        print_active_config_path(&config);
    }

    Ok(())
}

fn apply_login_config_overrides(
    config: &mut Config,
    endpoint: Option<String>,
    oauth_client_id: Option<String>,
    org: Option<String>,
) {
    if let Some(endpoint) = endpoint {
        config.endpoint = Some(endpoint);
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
        println!("Saved config to {}", path.display());
    }
}

/// Remove stored credentials.
pub fn logout() -> Result<()> {
    let mut config = Config::load()?;
    config.clear_credentials();
    config.save()?;
    println!("Logged out. Credentials removed from active config.");
    Ok(())
}

/// Show current authentication status.
pub fn status() -> Result<()> {
    let config = Config::load()?;

    if let Some(path) = config.active_path() {
        println!("Using config: {}", path.display());
    }

    if let Some(ref oauth) = config.oauth {
        let masked = mask_token(&oauth.access_token);
        let expiry = oauth
            .expires_at
            .map(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            })
            .unwrap_or_else(|| "no expiry".to_string());

        println!(
            "Authenticated via OAuth (token: {}, expires: {})",
            masked, expiry
        );

        if config.oauth_token_expired() {
            println!("  Note: access token has expired and will be refreshed on next API call.");
        }
    } else {
        match config.token {
            Some(ref t) if !t.is_empty() => {
                let masked = mask_token(t);
                println!("Authenticated via personal token (token: {})", masked);
            }
            _ => {
                println!("Not authenticated. Run `appsignal-cli auth login` to set up.");
            }
        }
    }

    Ok(())
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
            Some("staging-client-id".to_string()),
            Some("side-project".to_string()),
        );

        assert_eq!(config.endpoint, Some("https://staging.lol".to_string()));
        assert_eq!(
            config.oauth_client_id,
            Some("staging-client-id".to_string())
        );
        assert_eq!(config.org, Some("side-project".to_string()));
    }
}
