use std::io::{self, IsTerminal, Read, Write};

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;
use serde_json::json;

use crate::client_headers::{with_appsignal_headers, CLIENT_VERSION};
use crate::config::Config;
use crate::error::CliError;
use crate::output::Output;

const DEFAULT_BASE_URL: &str = "https://appsignal.com";
const FEEDBACK_PATH: &str = "/api/cli/feedback";

#[derive(Debug)]
pub struct FeedbackOptions {
    pub message: Option<String>,
    pub text: Vec<String>,
    pub email: Option<String>,
    pub no_email: bool,
}

#[derive(Serialize)]
struct FeedbackResponse {
    message: String,
    email: Option<String>,
    email_saved: bool,
}

pub async fn send(options: FeedbackOptions, format: Output) -> Result<()> {
    let mut config = Config::load()?;
    let endpoint = config.endpoint_base_url()?;
    super::refresh_oauth_if_needed(&mut config, endpoint.as_deref()).await?;

    let (email, should_save_email) = resolve_email(
        &config,
        options.email,
        options.no_email,
        matches!(format, Output::Human),
    )?;
    let feedback = feedback_text(options.message, &options.text)?;

    submit_feedback(&config, endpoint.as_deref(), &feedback, email.as_deref()).await?;

    if should_save_email {
        config.feedback_email = email.clone();
        config.save()?;
    }

    crate::output::print_with(
        FeedbackResponse {
            message: "Feedback sent".to_string(),
            email,
            email_saved: should_save_email,
        },
        format,
        |w| writeln!(w, "Thanks, feedback sent to AppSignal."),
    )
}

fn feedback_text(message: Option<String>, text: &[String]) -> Result<String> {
    let feedback = match (message, text.is_empty()) {
        (Some(message), _) => message,
        (None, false) => text.join(" "),
        (None, true) => read_feedback_from_stdin()?,
    };

    let feedback = feedback.trim().to_string();
    if feedback.is_empty() {
        anyhow::bail!(CliError::msg(
            "Feedback cannot be empty. Pass text as an argument or pipe it on stdin."
        ));
    }

    Ok(feedback)
}

fn read_feedback_from_stdin() -> Result<String> {
    if io::stdin().is_terminal() {
        crate::status!("Enter feedback, then press Ctrl-D when finished:");
    }

    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context(CliError::ConfigIo)?;
    Ok(input)
}

fn resolve_email(
    config: &Config,
    explicit_email: Option<String>,
    no_email: bool,
    prompt_if_missing: bool,
) -> Result<(Option<String>, bool)> {
    if no_email {
        return Ok((None, false));
    }

    let email = if let Some(email) = explicit_email {
        clean_email(&email)
    } else if let Some(email) = config.feedback_email.as_deref().and_then(clean_email) {
        Some(email)
    } else if prompt_if_missing && io::stdin().is_terminal() {
        prompt_email()?
    } else {
        None
    };

    let should_save_email = email
        .as_ref()
        .is_some_and(|email| config.feedback_email.as_deref() != Some(email.as_str()));

    Ok((email, should_save_email))
}

fn clean_email(email: &str) -> Option<String> {
    let email = email.trim().to_string();
    if email.is_empty() {
        None
    } else {
        Some(email)
    }
}

fn prompt_email() -> Result<Option<String>> {
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    write!(
        stderr,
        "Contact email for follow-up (optional, press Enter to skip): "
    )
    .context(CliError::ConfigIo)?;
    stderr.flush().context(CliError::ConfigIo)?;

    let mut email = String::new();
    io::stdin()
        .read_line(&mut email)
        .context(CliError::ConfigIo)?;
    Ok(clean_email(&email))
}

async fn submit_feedback(
    config: &Config,
    endpoint: Option<&str>,
    feedback: &str,
    email: Option<&str>,
) -> Result<()> {
    let feedback_url = feedback_url(endpoint.unwrap_or(DEFAULT_BASE_URL));
    let mut request = with_appsignal_headers(Client::new().post(feedback_url));

    if let Some(oauth) = config.oauth.as_ref() {
        request = request.bearer_auth(&oauth.access_token);
    }

    let response = request
        .json(&json!({
            "source": "appsignal-cli",
            "feedback": feedback,
            "email": email,
            "cli_version": CLIENT_VERSION,
        }))
        .send()
        .await
        .context(CliError::NetworkUnreachable)?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!(CliError::from_http(status, &text));
    }

    Ok(())
}

fn feedback_url(base_url: &str) -> String {
    match url::Url::parse(base_url) {
        Ok(mut url) => {
            url.set_path(FEEDBACK_PATH);
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => format!("{}{}", base_url.trim_end_matches('/'), FEEDBACK_PATH),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OAuthCredentials;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn feedback_text_prefers_message_flag() {
        let text =
            feedback_text(Some("flag text".to_string()), &["positional".to_string()]).unwrap();

        assert_eq!(text, "flag text");
    }

    #[test]
    fn feedback_text_joins_positional_words() {
        let text = feedback_text(None, &["missing".to_string(), "endpoint".to_string()]).unwrap();

        assert_eq!(text, "missing endpoint");
    }

    #[test]
    fn feedback_url_uses_feedback_path() {
        assert_eq!(
            feedback_url("https://staging.lol/"),
            "https://staging.lol/api/cli/feedback"
        );
    }

    #[test]
    fn resolve_email_uses_explicit_email_and_marks_it_for_save() {
        let config = Config::default();

        let (email, should_save_email) =
            resolve_email(&config, Some(" ada@example.com ".to_string()), false, false).unwrap();

        assert_eq!(email.as_deref(), Some("ada@example.com"));
        assert!(should_save_email);
    }

    #[test]
    fn resolve_email_uses_saved_email_without_resaving() {
        let config = Config {
            feedback_email: Some("ada@example.com".to_string()),
            ..Config::default()
        };

        let (email, should_save_email) = resolve_email(&config, None, false, false).unwrap();

        assert_eq!(email.as_deref(), Some("ada@example.com"));
        assert!(!should_save_email);
    }

    #[test]
    fn resolve_email_no_email_suppresses_saved_email() {
        let config = Config {
            feedback_email: Some("ada@example.com".to_string()),
            ..Config::default()
        };

        let (email, should_save_email) = resolve_email(&config, None, true, true).unwrap();

        assert_eq!(email, None);
        assert!(!should_save_email);
    }

    #[tokio::test]
    async fn submit_feedback_posts_payload_with_oauth_when_available() {
        let server = MockServer::start().await;
        let config = Config {
            endpoint: Some(server.uri()),
            oauth: Some(OAuthCredentials {
                access_token: "oauth-token".to_string(),
                refresh_token: None,
                expires_at: None,
            }),
            ..Config::default()
        };

        Mock::given(method("POST"))
            .and(path("/api/cli/feedback"))
            .and(header("authorization", "Bearer oauth-token"))
            .and(body_json(json!({
                "source": "appsignal-cli",
                "feedback": "Please add metrics support",
                "email": "ada@example.com",
                "cli_version": CLIENT_VERSION,
            })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let endpoint = config.endpoint_base_url().unwrap();
        submit_feedback(
            &config,
            endpoint.as_deref(),
            "Please add metrics support",
            Some("ada@example.com"),
        )
        .await
        .unwrap();
    }
}
