//! Parsing of AppSignal references (full URLs, paths, or bare sample ids) into
//! the pieces the `samples` command needs: app id, incident number, and which
//! sample to fetch.
//!
//! AppSignal sample URLs come in a handful of shapes:
//!
//! ```text
//! .../sites/<app_id>/performance/incidents/<N>
//! .../sites/<app_id>/exceptions/incidents/<N>
//! .../sites/<app_id>/performance/incidents/<N>/samples/<sample_id>
//! .../sites/<app_id>/exceptions/incidents/<N>/samples/timestamp/<ISO8601>
//! ```
//!
//! Plus a bare sample id, which is `<app_id>-<digits>` (the app id is a
//! 24-character hex string).
//!
//! Two lessons are baked in here:
//!
//! - The `/samples/timestamp/<ISO>` shape must be recognized *before* a generic
//!   sample-id segment, or the ISO timestamp gets mistaken for an id.
//! - The performance-vs-error distinction in the URL is only a *hint*. Callers
//!   should dispatch off the incident `__typename` the API actually returns, not
//!   off the URL path, so an exception URL can never be miscategorised as
//!   performance.

use crate::error::CliError;
use anyhow::Result;

/// The sample type hinted by an AppSignal URL path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleKind {
    Performance,
    Error,
}

/// Which sample to fetch for a reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SampleSelector {
    /// No specific sample requested — fetch the latest.
    Latest,
    /// A specific sample by id.
    Id(String),
    /// The sample closest to an ISO-8601 timestamp.
    Timestamp(String),
}

/// A parsed AppSignal sample reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleRef {
    pub app_id: String,
    /// Incident number, when the reference points at an incident. A bare sample
    /// id carries no incident context, so this is `None`.
    pub incident_number: Option<i64>,
    /// Performance vs error, when the URL path makes it explicit. Treat as a
    /// hint only — the incident `__typename` is authoritative.
    pub kind: Option<SampleKind>,
    pub selector: SampleSelector,
}

/// Parse a user-supplied reference: a full AppSignal URL, a bare path, or a
/// bare sample id.
pub fn parse(input: &str) -> Result<SampleRef> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(invalid(input));
    }

    let segments = path_segments(trimmed);

    if segments.iter().any(|s| s == "sites") {
        parse_path_segments(&segments).ok_or_else(|| invalid(input))
    } else if looks_like_sample_id(trimmed) {
        parse_bare_sample_id(trimmed).ok_or_else(|| invalid(input))
    } else {
        Err(invalid(input))
    }
}

fn invalid(input: &str) -> anyhow::Error {
    CliError::msg(format!(
        "Could not parse '{}' as an AppSignal sample URL or id. Expected a URL like \
         https://appsignal.com/<account>/sites/<app-id>/performance/incidents/<n> \
         or a sample id like <app-id>-<digits>.",
        input.trim()
    ))
    .into()
}

/// Split a URL or path into decoded path segments, dropping the scheme, host,
/// and any query/fragment.
fn path_segments(input: &str) -> Vec<String> {
    // Try a full URL first so the `url` crate decodes percent-escapes for us.
    if let Ok(url) = url::Url::parse(input) {
        if url.has_host() {
            if let Some(segments) = url.path_segments() {
                return segments
                    .filter(|s| !s.is_empty())
                    .map(decode_segment)
                    .collect();
            }
        }
    }

    // Otherwise treat the input as a bare path. Strip any query/fragment, then
    // split on '/'.
    let path = input
        .split(['?', '#'])
        .next()
        .unwrap_or(input)
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    path.split('/')
        .filter(|s| !s.is_empty())
        .map(decode_segment)
        .collect()
}

/// Decode the small set of percent-escapes that show up in path segments (most
/// importantly the `:` in an ISO-8601 timestamp).
fn decode_segment(segment: &str) -> String {
    if !segment.contains('%') {
        return segment.to_string();
    }

    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(byte) = hex_pair(bytes[i + 1], bytes[i + 2]) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(out).unwrap_or_else(|_| segment.to_string())
}

fn hex_pair(hi: u8, lo: u8) -> Option<u8> {
    Some(hex_digit(hi)? << 4 | hex_digit(lo)?)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_path_segments(segments: &[String]) -> Option<SampleRef> {
    let app_id = segment_after(segments, "sites")?.to_string();
    if !is_app_id(&app_id) {
        return None;
    }

    let kind = if segments.iter().any(|s| s == "performance") {
        Some(SampleKind::Performance)
    } else if segments.iter().any(|s| s == "exceptions") {
        Some(SampleKind::Error)
    } else {
        None
    };

    let incident_number = segment_after(segments, "incidents").and_then(|s| s.parse::<i64>().ok());

    let selector = match position(segments, "samples") {
        Some(idx) => match segments.get(idx + 1).map(String::as_str) {
            Some("timestamp") => SampleSelector::Timestamp(segments.get(idx + 2)?.to_string()),
            Some(id) => SampleSelector::Id(id.to_string()),
            None => SampleSelector::Latest,
        },
        None => SampleSelector::Latest,
    };

    Some(SampleRef {
        app_id,
        incident_number,
        kind,
        selector,
    })
}

fn parse_bare_sample_id(input: &str) -> Option<SampleRef> {
    let app_id = app_id_from_sample_id(input)?;
    Some(SampleRef {
        app_id,
        incident_number: None,
        kind: None,
        selector: SampleSelector::Id(input.to_string()),
    })
}

/// Extract the 24-character hex app id prefix from a `<app_id>-<rest>` sample id.
pub fn app_id_from_sample_id(sample_id: &str) -> Option<String> {
    let (app_id, _rest) = sample_id.split_once('-')?;
    is_app_id(app_id).then(|| app_id.to_string())
}

fn looks_like_sample_id(input: &str) -> bool {
    !input.contains("://") && !input.contains('/') && app_id_from_sample_id(input).is_some()
}

/// AppSignal app ids are 24-character lowercase hex strings.
fn is_app_id(value: &str) -> bool {
    value.len() == 24
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn position(segments: &[String], needle: &str) -> Option<usize> {
    segments.iter().position(|s| s == needle)
}

fn segment_after<'a>(segments: &'a [String], needle: &str) -> Option<&'a String> {
    let idx = position(segments, needle)?;
    segments.get(idx + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "0123456789abcdef01234567";

    #[test]
    fn parses_performance_incident_url() {
        let parsed = parse(&format!(
            "https://appsignal.com/my-account/sites/{APP}/performance/incidents/105"
        ))
        .unwrap();
        assert_eq!(parsed.app_id, APP);
        assert_eq!(parsed.incident_number, Some(105));
        assert_eq!(parsed.kind, Some(SampleKind::Performance));
        assert_eq!(parsed.selector, SampleSelector::Latest);
    }

    #[test]
    fn parses_exception_incident_url() {
        let parsed = parse(&format!(
            "https://appsignal.com/my-account/sites/{APP}/exceptions/incidents/197"
        ))
        .unwrap();
        assert_eq!(parsed.incident_number, Some(197));
        assert_eq!(parsed.kind, Some(SampleKind::Error));
    }

    #[test]
    fn parses_sample_by_id_url() {
        let parsed = parse(&format!(
            "https://appsignal.com/acct/sites/{APP}/performance/incidents/12/samples/{APP}-99887766"
        ))
        .unwrap();
        assert_eq!(parsed.incident_number, Some(12));
        assert_eq!(
            parsed.selector,
            SampleSelector::Id(format!("{APP}-99887766"))
        );
    }

    #[test]
    fn parses_sample_by_timestamp_url_before_id() {
        // The `/samples/timestamp/<ISO>` shape must win over the generic
        // sample-id branch.
        let parsed = parse(&format!(
            "https://appsignal.com/acct/sites/{APP}/exceptions/incidents/2323/samples/timestamp/2026-05-19T14:30:00Z"
        ))
        .unwrap();
        assert_eq!(parsed.incident_number, Some(2323));
        assert_eq!(parsed.kind, Some(SampleKind::Error));
        assert_eq!(
            parsed.selector,
            SampleSelector::Timestamp("2026-05-19T14:30:00Z".to_string())
        );
    }

    #[test]
    fn decodes_percent_encoded_timestamp() {
        let parsed = parse(&format!(
            "https://appsignal.com/acct/sites/{APP}/performance/incidents/7/samples/timestamp/2026-05-19T14%3A30%3A00Z"
        ))
        .unwrap();
        assert_eq!(
            parsed.selector,
            SampleSelector::Timestamp("2026-05-19T14:30:00Z".to_string())
        );
    }

    #[test]
    fn parses_bare_path_without_scheme() {
        let parsed = parse(&format!("/acct/sites/{APP}/performance/incidents/42")).unwrap();
        assert_eq!(parsed.app_id, APP);
        assert_eq!(parsed.incident_number, Some(42));
    }

    #[test]
    fn parses_bare_sample_id() {
        let parsed = parse(&format!("{APP}-163830565690137776091764161460")).unwrap();
        assert_eq!(parsed.app_id, APP);
        assert_eq!(parsed.incident_number, None);
        assert_eq!(parsed.kind, None);
        assert_eq!(
            parsed.selector,
            SampleSelector::Id(format!("{APP}-163830565690137776091764161460"))
        );
    }

    #[test]
    fn rejects_non_hex_app_id_sample() {
        assert!(parse("not-a-sample-id").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("https://example.com/random/path").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn app_id_from_sample_id_requires_24_hex() {
        assert_eq!(
            app_id_from_sample_id(&format!("{APP}-1")),
            Some(APP.to_string())
        );
        assert_eq!(app_id_from_sample_id("short-1"), None);
        assert_eq!(app_id_from_sample_id("nodash"), None);
        // App ids are lowercase hex; uppercase must not be treated as an id.
        assert_eq!(app_id_from_sample_id("0123456789ABCDEF01234567-1"), None);
    }
}
