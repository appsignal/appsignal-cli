use anyhow::Result;
use std::io::{self, Write};

use crate::api::AppSignalClient;
use crate::config::Config;

/// Prompt the user for a token interactively if not provided via --token.
fn prompt_token() -> Result<String> {
    print!("Enter your AppSignal personal API token: ");
    io::stdout().flush()?;
    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    Ok(token.trim().to_string())
}

/// Authenticate with AppSignal by storing (and validating) a personal API token.
pub async fn login(token: Option<String>) -> Result<()> {
    let token = match token {
        Some(t) => t,
        None => prompt_token()?,
    };

    if token.is_empty() {
        anyhow::bail!("Token cannot be empty");
    }

    // Validate by making a test API call
    print!("Validating token... ");
    io::stdout().flush()?;

    let config = Config::load()?;
    let client = AppSignalClient::new(&token, config.endpoint.as_deref());
    match client.validate_token().await {
        Ok(_) => {
            println!("OK");
        }
        Err(e) => {
            println!("FAILED");
            anyhow::bail!("Token validation failed: {}", e);
        }
    }

    let mut config = config;
    config.token = Some(token);
    config.save()?;

    println!("Token saved. You are now authenticated.");
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
    match config.token {
        Some(ref t) if !t.is_empty() => {
            // Show only the last 4 characters
            let masked = if t.len() > 4 {
                format!("{}...{}", &t[..4], &t[t.len() - 4..])
            } else {
                "****".to_string()
            };
            println!("Authenticated (token: {})", masked);
        }
        _ => {
            println!("Not authenticated. Run `appsignal-cli auth login` to set up.");
        }
    }
    Ok(())
}
