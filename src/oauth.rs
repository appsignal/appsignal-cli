use anyhow::{Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::Rng;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Write;

use crate::config::OAuthCredentials;

const DEFAULT_OAUTH_BASE: &str = "https://appsignal.com";
const CLIENT_ID: &str = "IimKhAcp18_CojT108_KnthRqTpyT8BCSCddBzSpCZs";
const REDIRECT_URI: &str = "appsignal://callback";
const SCOPES: &str = "app:read app:write";

/// OAuth configuration for the AppSignal CLI application.
pub struct OAuthConfig {
    pub client_id: String,
    pub authorize_url: String,
    pub token_url: String,
    pub redirect_uri: String,
    pub scopes: String,
}

impl OAuthConfig {
    pub fn new(base_url: Option<&str>) -> Self {
        let base = base_url.unwrap_or(DEFAULT_OAUTH_BASE).trim_end_matches('/');
        Self {
            client_id: CLIENT_ID.to_string(),
            authorize_url: format!("{}/oauth/authorize", base),
            token_url: format!("{}/oauth/token", base),
            redirect_uri: REDIRECT_URI.to_string(),
            scopes: SCOPES.to_string(),
        }
    }
}

/// Generate a cryptographically random code verifier for PKCE.
pub fn generate_code_verifier() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    URL_SAFE_NO_PAD.encode(&bytes)
}

/// Derive the S256 code challenge from a code verifier.
pub fn generate_code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

/// Generate a random state parameter to prevent CSRF.
pub fn generate_state() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..16).map(|_| rng.gen()).collect();
    URL_SAFE_NO_PAD.encode(&bytes)
}

/// Build the full authorization URL for the browser.
fn build_authorize_url(config: &OAuthConfig, state: &str, code_challenge: &str) -> String {
    format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256&scope={}",
        config.authorize_url,
        urlencoding::encode(&config.client_id),
        urlencoding::encode(&config.redirect_uri),
        urlencoding::encode(state),
        urlencoding::encode(code_challenge),
        urlencoding::encode(&config.scopes),
    )
}

/// Parse the callback URL (e.g. `appsignal://callback?code=...&state=...`)
/// and extract the authorization code after validating the state parameter.
fn parse_callback_url(callback_url: &str, expected_state: &str) -> Result<String> {
    let url = url::Url::parse(callback_url).context(
        "Invalid callback URL. Expected a URL like: appsignal://callback?code=...&state=...",
    )?;

    let params: std::collections::HashMap<String, String> =
        url.query_pairs().into_owned().collect();

    // Check for an error response from the OAuth server
    if let Some(error) = params.get("error") {
        let description = params.get("error_description").cloned().unwrap_or_default();
        anyhow::bail!("OAuth authorization failed: {} ({})", error, description);
    }

    // Validate state
    let state = params
        .get("state")
        .context("Missing 'state' parameter in callback URL")?;

    if state != expected_state {
        anyhow::bail!("OAuth state mismatch — possible CSRF attack. Please try again.");
    }

    let code = params
        .get("code")
        .context("Missing 'code' parameter in callback URL")?
        .clone();

    Ok(code)
}

/// Response from the OAuth token endpoint.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    /// Number of seconds until the access token expires.
    expires_in: Option<i64>,
    token_type: String,
}

/// Exchange an authorization code for access and refresh tokens.
async fn exchange_code(
    config: &OAuthConfig,
    code: &str,
    code_verifier: &str,
) -> Result<OAuthCredentials> {
    let client = Client::new();
    let resp = client
        .post(&config.token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", &config.client_id),
            ("code", code),
            ("redirect_uri", config.redirect_uri.as_str()),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .context("Failed to exchange authorization code for tokens")?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Token exchange failed (HTTP {}): {}", status, text);
    }

    let token_resp: TokenResponse = resp
        .json()
        .await
        .context("Failed to parse token response")?;

    if token_resp.token_type.to_lowercase() != "bearer" {
        anyhow::bail!(
            "Unexpected token type: {} (expected 'bearer')",
            token_resp.token_type
        );
    }

    let expires_at = token_resp
        .expires_in
        .map(|secs| chrono::Utc::now().timestamp() + secs);

    Ok(OAuthCredentials {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_at,
    })
}

/// Refresh an OAuth access token using a refresh token.
pub async fn refresh_access_token(
    base_url: Option<&str>,
    refresh_token: &str,
) -> Result<OAuthCredentials> {
    let config = OAuthConfig::new(base_url);
    let client = Client::new();

    let resp = client
        .post(&config.token_url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &config.client_id),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .context("Failed to refresh OAuth token")?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "Token refresh failed (HTTP {}): {}. Please re-authenticate with `appsignal-cli auth login --oauth`.",
            status,
            text
        );
    }

    let token_resp: TokenResponse = resp
        .json()
        .await
        .context("Failed to parse token refresh response")?;

    let expires_at = token_resp
        .expires_in
        .map(|secs| chrono::Utc::now().timestamp() + secs);

    Ok(OAuthCredentials {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_at,
    })
}

/// Prompt the user to paste the callback URL from the browser.
fn prompt_callback_url() -> Result<String> {
    print!("Paste the callback URL here: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

/// Run the full OAuth PKCE authorization flow.
///
/// 1. Open the browser to AppSignal's authorization page.
/// 2. After the user authorizes, the browser redirects to the custom scheme
///    `appsignal://callback?code=...&state=...`.
/// 3. The user pastes the callback URL back into the terminal.
/// 4. Exchange the authorization code for access and refresh tokens.
pub async fn perform_oauth_flow(base_url: Option<&str>) -> Result<OAuthCredentials> {
    let config = OAuthConfig::new(base_url);

    // Step 1: PKCE parameters
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    let state = generate_state();

    // Step 2: Build and open the authorization URL
    let authorize_url = build_authorize_url(&config, &state, &code_challenge);

    println!("Opening your browser to authenticate with AppSignal...");
    println!();

    if open::that(&authorize_url).is_err() {
        println!("Could not open browser automatically.");
        println!("Please open the following URL in your browser:");
        println!();
        println!("  {}", authorize_url);
        println!();
    }

    println!("After you authorize, your browser will redirect to a URL starting");
    println!("with \"appsignal://callback\". Copy that full URL and paste it below.");
    println!();

    // Step 3: Get the callback URL from the user
    let callback_url = prompt_callback_url()?;

    if callback_url.is_empty() {
        anyhow::bail!("No callback URL provided. Please try again.");
    }

    // Step 4: Parse the callback URL and extract the authorization code
    let code = parse_callback_url(&callback_url, &state)?;

    // Step 5: Exchange the code for tokens
    print!("Exchanging authorization code for tokens... ");
    std::io::stdout().flush()?;

    let credentials = exchange_code(&config, &code, &code_verifier).await?;
    println!("OK");

    Ok(credentials)
}

// -- urlencoding helper (minimal, avoids another dependency) --
mod urlencoding {
    use std::fmt;

    pub struct Encoded<'a>(pub &'a str);

    impl<'a> fmt::Display for Encoded<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for byte in self.0.bytes() {
                match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        write!(f, "{}", byte as char)?;
                    }
                    _ => {
                        write!(f, "%{:02X}", byte)?;
                    }
                }
            }
            Ok(())
        }
    }

    pub fn encode(input: &str) -> Encoded<'_> {
        Encoded(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_verifier_length() {
        let verifier = generate_code_verifier();
        // 32 bytes base64url-encoded ≈ 43 chars
        assert!(verifier.len() >= 32);
    }

    #[test]
    fn test_code_verifier_is_url_safe() {
        let verifier = generate_code_verifier();
        for c in verifier.chars() {
            assert!(
                c.is_ascii_alphanumeric() || c == '-' || c == '_',
                "Invalid character in code verifier: {}",
                c
            );
        }
    }

    #[test]
    fn test_code_challenge_deterministic() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = generate_code_challenge(verifier);
        let challenge2 = generate_code_challenge(verifier);
        assert_eq!(challenge, challenge2);
    }

    #[test]
    fn test_code_challenge_differs_from_verifier() {
        let verifier = generate_code_verifier();
        let challenge = generate_code_challenge(&verifier);
        assert_ne!(verifier, challenge);
    }

    #[test]
    fn test_state_is_unique() {
        let state1 = generate_state();
        let state2 = generate_state();
        assert_ne!(state1, state2);
    }

    #[test]
    fn test_state_is_url_safe() {
        let state = generate_state();
        for c in state.chars() {
            assert!(
                c.is_ascii_alphanumeric() || c == '-' || c == '_',
                "Invalid character in state: {}",
                c
            );
        }
    }

    #[test]
    fn test_build_authorize_url() {
        let config = OAuthConfig::new(Some("https://test.appsignal.com"));
        let url = build_authorize_url(&config, "mystate", "mychallenge");

        assert!(url.starts_with("https://test.appsignal.com/oauth/authorize?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains(&format!("client_id={}", CLIENT_ID)));
        assert!(url.contains("redirect_uri=appsignal%3A%2F%2Fcallback"));
        assert!(url.contains("state=mystate"));
        assert!(url.contains("code_challenge=mychallenge"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("scope=app%3Aread%20app%3Awrite"));
    }

    #[test]
    fn test_oauth_config_default_urls() {
        let config = OAuthConfig::new(None);
        assert_eq!(
            config.authorize_url,
            "https://appsignal.com/oauth/authorize"
        );
        assert_eq!(config.token_url, "https://appsignal.com/oauth/token");
        assert_eq!(config.client_id, CLIENT_ID);
        assert_eq!(config.redirect_uri, "appsignal://callback");
        assert_eq!(config.scopes, "app:read app:write");
    }

    #[test]
    fn test_oauth_config_custom_base() {
        let config = OAuthConfig::new(Some("https://staging.appsignal.com"));
        assert_eq!(
            config.authorize_url,
            "https://staging.appsignal.com/oauth/authorize"
        );
        assert_eq!(
            config.token_url,
            "https://staging.appsignal.com/oauth/token"
        );
        // redirect_uri and client_id are the same regardless of base
        assert_eq!(config.redirect_uri, "appsignal://callback");
        assert_eq!(config.client_id, CLIENT_ID);
    }

    #[test]
    fn test_parse_callback_url_success() {
        let url = "appsignal://callback?code=abc123&state=mystate";
        let code = parse_callback_url(url, "mystate").unwrap();
        assert_eq!(code, "abc123");
    }

    #[test]
    fn test_parse_callback_url_state_mismatch() {
        let url = "appsignal://callback?code=abc123&state=wrong";
        let result = parse_callback_url(url, "mystate");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("state mismatch"));
    }

    #[test]
    fn test_parse_callback_url_error_response() {
        let url = "appsignal://callback?error=access_denied&error_description=User+denied";
        let result = parse_callback_url(url, "mystate");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("access_denied"));
    }

    #[test]
    fn test_parse_callback_url_missing_code() {
        let url = "appsignal://callback?state=mystate";
        let result = parse_callback_url(url, "mystate");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing 'code'"));
    }

    #[test]
    fn test_parse_callback_url_missing_state() {
        let url = "appsignal://callback?code=abc123";
        let result = parse_callback_url(url, "mystate");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing 'state'"));
    }

    #[test]
    fn test_parse_callback_url_invalid_url() {
        let result = parse_callback_url("not a url", "mystate");
        assert!(result.is_err());
    }

    #[test]
    fn test_urlencoding_basic() {
        let encoded = format!("{}", urlencoding::encode("hello world"));
        assert_eq!(encoded, "hello%20world");
    }

    #[test]
    fn test_urlencoding_special_chars() {
        let encoded = format!("{}", urlencoding::encode("a=b&c=d"));
        assert_eq!(encoded, "a%3Db%26c%3Dd");
    }

    #[test]
    fn test_urlencoding_safe_chars_preserved() {
        let encoded = format!("{}", urlencoding::encode("abc-123_test.value~ok"));
        assert_eq!(encoded, "abc-123_test.value~ok");
    }

    #[tokio::test]
    async fn test_exchange_code_success() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/oauth/token"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "new-access-token",
                    "refresh_token": "new-refresh-token",
                    "expires_in": 3600,
                    "token_type": "Bearer"
                })),
            )
            .mount(&server)
            .await;

        let config = OAuthConfig::new(Some(&server.uri()));
        let result = exchange_code(&config, "auth-code", "verifier").await;

        let creds = result.unwrap();
        assert_eq!(creds.access_token, "new-access-token");
        assert_eq!(creds.refresh_token, Some("new-refresh-token".to_string()));
        assert!(creds.expires_at.is_some());
    }

    #[tokio::test]
    async fn test_exchange_code_failure() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/oauth/token"))
            .respond_with(wiremock::ResponseTemplate::new(400).set_body_string("invalid_grant"))
            .mount(&server)
            .await;

        let config = OAuthConfig::new(Some(&server.uri()));
        let result = exchange_code(&config, "bad-code", "verifier").await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Token exchange failed"));
    }

    #[tokio::test]
    async fn test_refresh_access_token_success() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/oauth/token"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "refreshed-token",
                    "refresh_token": "new-refresh",
                    "expires_in": 7200,
                    "token_type": "Bearer"
                })),
            )
            .mount(&server)
            .await;

        let creds = refresh_access_token(Some(&server.uri()), "old-refresh")
            .await
            .unwrap();
        assert_eq!(creds.access_token, "refreshed-token");
        assert_eq!(creds.refresh_token, Some("new-refresh".to_string()));
    }

    #[tokio::test]
    async fn test_refresh_access_token_failure() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/oauth/token"))
            .respond_with(wiremock::ResponseTemplate::new(401).set_body_string("invalid_token"))
            .mount(&server)
            .await;

        let result = refresh_access_token(Some(&server.uri()), "bad-refresh").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Token refresh failed"));
    }
}
