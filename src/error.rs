//! User-facing CLI errors.
//!
//! Any error wrapped (or chained) in [`CliError`] is shown verbatim to the
//! user. Anything else is treated as an internal failure and hidden behind a
//! generic message unless `APPSIGNAL_CLI_DEBUG=1` is set. See
//! [`crate::output::print_error`] for the rendering side.
//!
//! Construction patterns:
//!
//! - HTTP responses from the AppSignal API: [`CliError::from_http`].
//! - Domain messages that don't fit a category: [`CliError::msg`] (often via
//!   `.context(CliError::msg(...))` on a fallible call).
//! - Specific failure modes: pick the matching variant directly.
//!
//! Adding a new user-facing error is a one-line change in this file; nothing
//! else in the codebase needs to keep a string allowlist in sync.
//!
//! Variants live here even when they're constructed in a single place. The
//! point is that the *type* — not a prefix match on a formatted string — is
//! what marks a message as safe to show to a user.

use std::borrow::Cow;

use reqwest::StatusCode;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Authentication failed. Your AppSignal credentials were rejected. Run `appsignal-cli auth login` again.{}", fmt_detail(detail))]
    AuthRejected { detail: Option<String> },

    #[error(
        "AppSignal could not find the requested resource.{}",
        fmt_detail(detail)
    )]
    NotFound { detail: Option<String> },

    #[error(
        "AppSignal rate limited this request. Please try again in a moment.{}",
        fmt_detail(detail)
    )]
    RateLimited { detail: Option<String> },

    #[error(
        "AppSignal is unavailable right now. Please try again shortly.{}",
        fmt_detail(detail)
    )]
    Unavailable { detail: Option<String> },

    #[error("AppSignal request failed with HTTP {status}.{}", fmt_detail(detail))]
    UnexpectedHttp {
        status: StatusCode,
        detail: Option<String>,
    },

    #[error("AppSignal rejected the request. {0}")]
    GraphQlRejected(String),

    #[error("Could not reach AppSignal. Check your network connection and try again.")]
    NetworkUnreachable,

    #[error("AppSignal returned an unexpected response. Please try again.")]
    UnexpectedResponse,

    #[error(
        "Could not read or write the appsignal-cli config. Check file permissions and try again."
    )]
    ConfigIo,

    #[error(
        "OAuth login did not complete successfully. Try `appsignal-cli auth login --oauth` again."
    )]
    OAuthLocal,

    #[error("OAuth token exchange failed. {0}")]
    OAuthExchange(String),

    #[error(
        "OAuth token refresh failed. {0} Re-authenticate with `appsignal-cli auth login --oauth`."
    )]
    OAuthRefresh(String),

    /// Verbatim user-facing message. Use for domain-specific errors that
    /// don't fit a structured variant ("Application not found",
    /// "No log view found matching X", etc.).
    #[error("{0}")]
    Message(Cow<'static, str>),
}

impl CliError {
    /// Construct from an HTTP error response.
    pub fn from_http(status: StatusCode, body: &str) -> Self {
        let detail = extract_detail(body);
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Self::AuthRejected { detail },
            StatusCode::NOT_FOUND => Self::NotFound { detail },
            StatusCode::TOO_MANY_REQUESTS => Self::RateLimited { detail },
            s if s.is_server_error() => Self::Unavailable { detail },
            status => Self::UnexpectedHttp { status, detail },
        }
    }

    /// Wrap a verbatim, already user-friendly message.
    pub fn msg(text: impl Into<Cow<'static, str>>) -> Self {
        Self::Message(text.into())
    }
}

fn fmt_detail(detail: &Option<String>) -> String {
    match detail.as_deref().map(str::trim) {
        Some(d) if !d.is_empty() => format!(" Details: {}", d),
        _ => String::new(),
    }
}

/// Best-effort extraction of a human-readable message from an error response
/// body. Handles plain text, JSON `{message|error|detail|errors}` shapes, and
/// the legacy `GraphQL errors: ...` prefix.
pub(crate) fn extract_detail(body: &str) -> Option<String> {
    let body = body.trim();
    if let Some(rest) = body.strip_prefix("GraphQL errors: ") {
        return non_empty(rest);
    }
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        if let Some(detail) = extract_json_detail(&value) {
            return Some(detail);
        }
    }
    non_empty(body.lines().next().unwrap_or(body))
}

fn extract_json_detail(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => non_empty(s),
        Value::Array(values) => values.iter().find_map(extract_json_detail),
        Value::Object(map) => ["message", "error", "detail", "errors"]
            .iter()
            .find_map(|key| map.get(*key).and_then(extract_json_detail)),
        _ => None,
    }
}

fn non_empty(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_http_maps_auth_failure() {
        let err = CliError::from_http(StatusCode::UNAUTHORIZED, "Unauthorized");
        let message = err.to_string();
        assert!(message.starts_with("Authentication failed."));
        assert!(message.contains("auth login"));
        assert!(!message.contains("HTTP 401"));
    }

    #[test]
    fn from_http_maps_server_error_to_unavailable() {
        let err = CliError::from_http(StatusCode::BAD_GATEWAY, "");
        assert!(err.to_string().starts_with("AppSignal is unavailable"));
    }

    #[test]
    fn from_http_appends_json_detail() {
        let err = CliError::from_http(StatusCode::BAD_REQUEST, r#"{"message":"App slug missing"}"#);
        let message = err.to_string();
        assert!(message.contains("HTTP 400"));
        assert!(message.contains("App slug missing"));
    }

    #[test]
    fn from_http_handles_graphql_prefix() {
        let err = CliError::from_http(
            StatusCode::BAD_REQUEST,
            "GraphQL errors: missing required field",
        );
        assert!(err.to_string().contains("missing required field"));
    }

    #[test]
    fn from_http_no_detail_when_body_is_blank() {
        let err = CliError::from_http(StatusCode::BAD_REQUEST, "   ");
        assert!(!err.to_string().contains("Details:"));
    }
}
