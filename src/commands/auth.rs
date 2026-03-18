use anyhow::Result;
use std::io::{self, Write};

use crate::api::AppSignalClient;
use crate::config::{AuthMethod, Config};
use crate::oauth;

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
pub async fn login(token: Option<String>, use_oauth: bool) -> Result<()> {
    let mut config = Config::load()?;

    if use_oauth {
        // --- OAuth flow ---
        // Derive the OAuth base URL from a custom GraphQL endpoint if set.
        let base_url = config.endpoint.as_ref().and_then(|ep| {
            url::Url::parse(ep).ok().map(|u| {
                format!(
                    "{}://{}{}",
                    u.scheme(),
                    u.host_str().unwrap_or("appsignal.com"),
                    u.port().map(|p| format!(":{}", p)).unwrap_or_default()
                )
            })
        });

        let credentials = oauth::perform_oauth_flow(base_url.as_deref()).await?;

        // Validate the new OAuth token
        print!("Validating OAuth token... ");
        io::stdout().flush()?;

        let auth = AuthMethod::OAuth {
            access_token: credentials.access_token.clone(),
            refresh_token: credentials.refresh_token.clone(),
            expires_at: credentials.expires_at,
        };
        let client = AppSignalClient::with_auth(auth, config.endpoint.as_deref());
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
    } else {
        // --- Personal token flow (existing behavior) ---
        let token = match token {
            Some(t) => t,
            None => prompt_token()?,
        };

        if token.is_empty() {
            anyhow::bail!("Token cannot be empty");
        }

        print!("Validating token... ");
        io::stdout().flush()?;

        let client = AppSignalClient::new(&token, config.endpoint.as_deref());
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
    }

    Ok(())
}

/// Remove stored credentials.
pub fn logout() -> Result<()> {
    Config::delete()?;
    println!("Logged out. Credentials removed.");
    Ok(())
}

/// Show current authentication status.
pub fn status() -> Result<()> {
    let config = Config::load()?;

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
