//! The `samples` command: fetch the underlying transaction samples behind an
//! incident.
//!
//! `incidents show` is incident-level aggregate only; this command pulls the
//! per-request sample data (action, duration, queue time, params, exception +
//! backtrace) that real triage works from. Inputs can be a full AppSignal URL,
//! a bare sample id, or explicit `--incident`/`--app-id` flags.

use std::io::{self, Write};

use anyhow::{Context, Result};
use serde::Serialize;

use super::{authenticated_client, resolve_org};
use crate::api::{
    AppSignalClient, BacktraceLine, IncidentSample, IncidentSamples, Sample, SampleQuery,
    WindowSample,
};
use crate::appsignal_url::{self, SampleSelector};
use crate::config::Config;
use crate::error::CliError;
use crate::output::{self, Output};
use crate::sample_analysis::{self, SampleAnalysis};
use crate::sample_cache::{self, CachedSample};

/// A single sample with its analysis, for `--output json`.
#[derive(Serialize)]
struct SampleShowResponse<'a> {
    incident_number: i64,
    #[serde(rename = "type")]
    sample_type: &'a str,
    sample: &'a Sample,
    analysis: &'a SampleAnalysis,
}

/// Fetch a single sample for an incident, by latest / id / timestamp.
#[allow(clippy::too_many_arguments)]
pub async fn show(
    reference: Option<&str>,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    incident: Option<i64>,
    sample_id: Option<&str>,
    at: Option<&str>,
    raw: bool,
    no_cache: bool,
    format: Output,
) -> Result<()> {
    let parsed = reference.map(appsignal_url::parse).transpose()?;

    // Reference wins over flags for app id, incident, and which sample.
    let explicit_app_id = parsed
        .as_ref()
        .map(|p| p.app_id.clone())
        .or_else(|| app_id.map(str::to_string));

    let incident_number = parsed.as_ref().and_then(|p| p.incident_number).or(incident);

    let selector = match &parsed {
        Some(p) => p.selector.clone(),
        None => match (sample_id, at) {
            (Some(id), _) => SampleSelector::Id(id.to_string()),
            (None, Some(ts)) => SampleSelector::Timestamp(ts.to_string()),
            (None, None) => SampleSelector::Latest,
        },
    };

    let incident_number = incident_number.context(CliError::msg(
        "Fetching a sample needs its incident. Pass an incident URL, or use --incident <number>.",
    ))?;

    let mut config = Config::load()?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = resolve_target_app_id(
        &client,
        &config,
        explicit_app_id.as_deref(),
        app_name,
        environment,
        org,
    )
    .await?;

    let query = match &selector {
        SampleSelector::Latest => SampleQuery::Latest,
        SampleSelector::Id(id) => SampleQuery::Id(id),
        SampleSelector::Timestamp(ts) => SampleQuery::Timestamp(ts),
    };

    let result = client
        .get_incident_sample(&resolved_app_id, incident_number, query)
        .await?;

    cache_samples(
        no_cache,
        &resolved_app_id,
        [(
            result.incident_number,
            result.sample_type.as_str(),
            &result.sample,
        )],
    );

    // Default to the analysed digest; `--raw` returns the unprocessed sample.
    if raw {
        return output::print_with(&result, format, |w| render_sample_detail(w, &result));
    }

    let analysis =
        sample_analysis::analyze(&result.sample, &result.sample_type, result.incident_number);
    let response = SampleShowResponse {
        incident_number: result.incident_number,
        sample_type: &result.sample_type,
        sample: &result.sample,
        analysis: &analysis,
    };

    output::print_with(response, format, |w| analysis.render_digest(w))
}

/// Default number of recent incidents scanned in window mode.
const DEFAULT_WINDOW_INCIDENT_SCAN: i64 = 20;

/// A window scan's samples, for `--output json`.
#[derive(Serialize)]
struct WindowSamplesResponse<'a> {
    count: usize,
    samples: &'a [WindowSample],
}

/// List samples for one incident, or — when no incident is given — scan a time
/// window across incidents.
#[allow(clippy::too_many_arguments)]
pub async fn list(
    reference: Option<&str>,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    incident: Option<i64>,
    start: Option<&str>,
    end: Option<&str>,
    limit: Option<i64>,
    namespaces: Option<&str>,
    user: Option<&str>,
    no_cache: bool,
    format: Output,
) -> Result<()> {
    let parsed = reference.map(appsignal_url::parse).transpose()?;

    let explicit_app_id = parsed
        .as_ref()
        .map(|p| p.app_id.clone())
        .or_else(|| app_id.map(str::to_string));

    let incident_number = parsed.as_ref().and_then(|p| p.incident_number).or(incident);

    let mut config = Config::load()?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = resolve_target_app_id(
        &client,
        &config,
        explicit_app_id.as_deref(),
        app_name,
        environment,
        org,
    )
    .await?;

    match incident_number {
        // Single-incident mode (Feature 1 behaviour). `--user` optionally filters.
        Some(number) => {
            let mut result = client
                .get_incident_samples(&resolved_app_id, number, start, end, limit)
                .await?;
            if let Some(user) = user {
                result.samples.retain(|s| sample_matches_user(s, user));
            }
            cache_samples(
                no_cache,
                &resolved_app_id,
                result
                    .samples
                    .iter()
                    .map(|s| (result.incident_number, result.sample_type.as_str(), s)),
            );
            output::print_with(&result, format, |w| render_sample_list(w, &result))
        }
        // Window mode: scan incidents and collect samples in [start, end].
        None => {
            let (start, end) = match (start, end) {
                (Some(start), Some(end)) => (start, end),
                _ => anyhow::bail!(CliError::msg(
                    "Listing samples needs either --incident, or both --start and --end to scan a \
                     time window across incidents.",
                )),
            };

            let namespaces = parse_namespaces(namespaces);
            let incident_limit = limit.unwrap_or(DEFAULT_WINDOW_INCIDENT_SCAN);

            let mut samples = client
                .scan_samples_in_window(
                    &resolved_app_id,
                    start,
                    end,
                    namespaces.as_deref(),
                    incident_limit,
                )
                .await?;

            if let Some(user) = user {
                samples.retain(|w| sample_matches_user(&w.sample, user));
            }
            cache_samples(
                no_cache,
                &resolved_app_id,
                samples
                    .iter()
                    .map(|w| (w.incident_number, w.sample_type.as_str(), &w.sample)),
            );
            sort_window_samples(&mut samples);

            let response = WindowSamplesResponse {
                count: samples.len(),
                samples: &samples,
            };
            output::print_with(response, format, |w| {
                render_window_samples(w, &samples, start, end)
            })
        }
    }
}

/// Best-effort cache of every sample a fetch returned. Honours `--no-cache` and
/// the `APPSIGNAL_NO_CACHE` environment variable, and never fails the command.
fn cache_samples<'a>(
    no_cache: bool,
    app_id: &str,
    entries: impl IntoIterator<Item = (i64, &'a str, &'a Sample)>,
) {
    if no_cache || sample_cache::disabled_by_env() {
        return;
    }
    let now = chrono::Utc::now().to_rfc3339();
    for (incident_number, sample_type, sample) in entries {
        sample_cache::store_best_effort(app_id, incident_number, sample_type, sample, &now);
    }
}

/// A cache listing or search result, for `--output json`.
#[derive(Serialize)]
struct CacheListResponse<'a> {
    count: usize,
    entries: &'a [CachedSample],
}

/// `samples cache list` — show recently cached samples.
pub fn cache_list(app_id: Option<&str>, limit: Option<usize>, format: Output) -> Result<()> {
    let dir = sample_cache::default_dir()
        .context(CliError::msg("Could not determine the cache directory."))?;
    let mut entries = sample_cache::load_all(&dir, app_id)?;
    if let Some(limit) = limit {
        entries.truncate(limit);
    }
    output::print_with(
        CacheListResponse {
            count: entries.len(),
            entries: &entries,
        },
        format,
        |w| render_cache_entries(w, &entries, None),
    )
}

/// `samples cache search <query>` — search cached samples by their contents.
pub fn cache_search(
    query: &str,
    app_id: Option<&str>,
    limit: Option<usize>,
    format: Output,
) -> Result<()> {
    let dir = sample_cache::default_dir()
        .context(CliError::msg("Could not determine the cache directory."))?;
    let mut entries = sample_cache::search(&dir, query, app_id)?;
    if let Some(limit) = limit {
        entries.truncate(limit);
    }
    output::print_with(
        CacheListResponse {
            count: entries.len(),
            entries: &entries,
        },
        format,
        |w| render_cache_entries(w, &entries, Some(query)),
    )
}

/// `samples cache clear` — delete every cached sample.
pub fn cache_clear(format: Output) -> Result<()> {
    let dir = sample_cache::default_dir()
        .context(CliError::msg("Could not determine the cache directory."))?;
    let removed = sample_cache::clear(&dir)?;
    output::print_with(serde_json::json!({ "removed": removed }), format, |w| {
        writeln!(w, "Removed {} cached sample(s).", removed)
    })
}

/// `samples cache path` — print the cache directory.
pub fn cache_path(format: Output) -> Result<()> {
    let path = sample_cache::dir_display()?;
    output::print_with(serde_json::json!({ "path": path }), format, |w| {
        writeln!(w, "{}", path)
    })
}

fn render_cache_entries(
    w: &mut dyn Write,
    entries: &[CachedSample],
    query: Option<&str>,
) -> io::Result<()> {
    if entries.is_empty() {
        return match query {
            Some(q) => writeln!(w, "No cached samples matched \"{}\".", q),
            None => writeln!(
                w,
                "No cached samples. Run `samples show`/`samples list` first."
            ),
        };
    }

    if let Some(q) = query {
        writeln!(w, "Cached samples matching \"{}\":", q)?;
    }
    writeln!(
        w,
        "{:<22} {:<26} {:<6} {:<5} {:<28} {:<22} SAMPLE",
        "CACHED", "APP", "INC", "TYPE", "ACTION", "USER"
    )?;
    writeln!(w, "{}", "-".repeat(120))?;
    for entry in entries {
        writeln!(
            w,
            "{:<22} {:<26} {:<6} {:<5} {:<28} {:<22} {}",
            truncate(&entry.cached_at, 22),
            truncate(&entry.app_id, 26),
            entry.incident_number,
            short_type(&entry.sample_type),
            truncate(entry.action().unwrap_or("-"), 28),
            truncate(entry.user().as_deref().unwrap_or("-"), 22),
            entry.sample.id,
        )?;
    }
    writeln!(w, "{} cached sample(s).", entries.len())
}

fn parse_namespaces(namespaces: Option<&str>) -> Option<Vec<String>> {
    namespaces.and_then(|raw| {
        let parts: Vec<String> = raw
            .split(',')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect();
        // An all-empty/all-comma value would otherwise become `Some([])`, which
        // serialises as an empty namespace filter and silently returns nothing.
        if parts.is_empty() {
            None
        } else {
            Some(parts)
        }
    })
}

fn sample_matches_user(sample: &Sample, query: &str) -> bool {
    sample_analysis::sample_user(sample)
        .map(|user| user.to_lowercase().contains(&query.to_lowercase()))
        .unwrap_or(false)
}

/// Sort window samples chronologically; ISO-8601 strings sort lexicographically,
/// and samples with no timestamp sort last.
fn sort_window_samples(samples: &mut [WindowSample]) {
    samples.sort_by(|a, b| match (&a.sample.created_at, &b.sample.created_at) {
        (Some(a), Some(b)) => a.cmp(b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
}

/// Resolve the target app id without forcing an organization to be configured
/// when the id is already known (from a URL or `--app-id`).
async fn resolve_target_app_id(
    client: &AppSignalClient,
    config: &Config,
    explicit_app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
) -> Result<String> {
    if let Some(id) = explicit_app_id {
        return Ok(id.to_string());
    }
    let org_slug = resolve_org(org, config)?;
    client
        .resolve_app_id(&org_slug, None, app_name, environment)
        .await
}

fn render_sample_detail(w: &mut dyn Write, result: &IncidentSample) -> io::Result<()> {
    let sample = &result.sample;
    let incident_label = format!("#{}", result.incident_number);

    let mut pairs: Vec<(&str, String)> = vec![
        ("Sample", sample.id.clone()),
        ("Type", result.sample_type.clone()),
        ("Incident", incident_label),
    ];
    push_common_pairs(&mut pairs, sample);

    let detail_pairs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    output::detail(w, &detail_pairs)?;

    if let Some(exception) = &sample.exception {
        writeln!(w)?;
        writeln!(
            w,
            "Exception:    {}",
            exception.name.as_deref().unwrap_or("-")
        )?;
        if let Some(message) = &exception.message {
            writeln!(w, "Message:      {}", message)?;
        }
        render_backtrace(w, exception.backtrace.as_deref().unwrap_or(&[]))?;
    }

    if let Some(causes) = &sample.error_causes {
        if !causes.is_empty() {
            writeln!(w)?;
            writeln!(w, "Error causes:")?;
            for cause in causes {
                writeln!(
                    w,
                    "  {}: {}",
                    cause.name.as_deref().unwrap_or("-"),
                    cause.message.as_deref().unwrap_or("-")
                )?;
            }
        }
    }

    Ok(())
}

fn push_common_pairs(pairs: &mut Vec<(&'static str, String)>, sample: &Sample) {
    if let Some(action) = &sample.action {
        pairs.push(("Action", action.clone()));
    }
    if let Some(namespace) = &sample.namespace {
        pairs.push(("Namespace", namespace.clone()));
    }
    if let Some(created_at) = &sample.created_at {
        pairs.push(("Occurred at", created_at.clone()));
    }
    if let Some(duration) = sample.duration {
        pairs.push(("Duration", format!("{:.2} ms", duration)));
    }
    if let Some(queue) = sample.queue_duration {
        pairs.push(("Queue", format!("{:.2} ms", queue)));
    }
    if let Some(revision) = &sample.revision {
        pairs.push(("Revision", revision.clone()));
    }
    if sample.has_n_plus_one == Some(true) {
        pairs.push(("N+1", "detected".to_string()));
    }
}

fn render_backtrace(w: &mut dyn Write, backtrace: &[BacktraceLine]) -> io::Result<()> {
    if backtrace.is_empty() {
        return Ok(());
    }
    const MAX_FRAMES: usize = 10;
    writeln!(w, "Backtrace:")?;
    for frame in backtrace.iter().take(MAX_FRAMES) {
        writeln!(w, "  {}", sample_analysis::format_backtrace_frame(frame))?;
    }
    if backtrace.len() > MAX_FRAMES {
        writeln!(w, "  ... {} more frame(s)", backtrace.len() - MAX_FRAMES)?;
    }
    Ok(())
}

fn render_sample_list(w: &mut dyn Write, result: &IncidentSamples) -> io::Result<()> {
    if result.samples.is_empty() {
        return writeln!(
            w,
            "No samples found for incident #{}.",
            result.incident_number
        );
    }

    writeln!(
        w,
        "{:<34} {:<12} {:<24} ACTION",
        "SAMPLE ID", "DURATION", "OCCURRED AT"
    )?;
    writeln!(w, "{}", "-".repeat(100))?;

    for sample in &result.samples {
        let duration = sample
            .duration
            .map(|d| format!("{:.2} ms", d))
            .unwrap_or_else(|| "-".to_string());
        let occurred_at = sample.created_at.as_deref().unwrap_or("-");
        let action = sample.action.as_deref().unwrap_or("-");
        writeln!(
            w,
            "{:<34} {:<12} {:<24} {}",
            sample.id,
            duration,
            occurred_at,
            truncate(action, 50),
        )?;
    }

    writeln!(
        w,
        "{} {} sample(s) for incident #{}.",
        result.samples.len(),
        result.sample_type,
        result.incident_number
    )
}

fn render_window_samples(
    w: &mut dyn Write,
    samples: &[WindowSample],
    start: &str,
    end: &str,
) -> io::Result<()> {
    if samples.is_empty() {
        return writeln!(w, "No samples found between {} and {}.", start, end);
    }

    writeln!(
        w,
        "{:<34} {:<9} {:<6} {:<12} {:<24} ACTION",
        "SAMPLE ID", "INCIDENT", "TYPE", "DURATION", "OCCURRED AT"
    )?;
    writeln!(w, "{}", "-".repeat(110))?;

    for entry in samples {
        let duration = entry
            .sample
            .duration
            .map(|d| format!("{:.2} ms", d))
            .unwrap_or_else(|| "-".to_string());
        let occurred_at = entry.sample.created_at.as_deref().unwrap_or("-");
        let action = entry.sample.action.as_deref().unwrap_or("-");
        writeln!(
            w,
            "{:<34} {:<9} {:<6} {:<12} {:<24} {}",
            entry.sample.id,
            format!("#{}", entry.incident_number),
            short_type(&entry.sample_type),
            duration,
            occurred_at,
            truncate(action, 40),
        )?;
    }

    writeln!(w, "{} sample(s) found in window.", samples.len())
}

fn short_type(sample_type: &str) -> &str {
    match sample_type {
        "performance" => "perf",
        "error" => "err",
        other => other,
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let truncated: String = value.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ExceptionDetail;

    fn sample_with(action: &str) -> Sample {
        Sample {
            id: "0123456789abcdef01234567-99".to_string(),
            action: Some(action.to_string()),
            namespace: Some("web".to_string()),
            duration: Some(123.456),
            queue_duration: Some(12.3),
            created_at: Some("2026-05-19T14:30:00Z".to_string()),
            revision: Some("abc123".to_string()),
            version: None,
            original_id: None,
            has_n_plus_one: Some(true),
            attributes: None,
            overview: None,
            environment: None,
            params: None,
            session_data: None,
            custom_data: None,
            timeline: None,
            timeline_truncated_events: None,
            group_durations: None,
            group_allocations: None,
            exception: None,
            error_causes: None,
            breadcrumbs: None,
        }
    }

    #[test]
    fn renders_performance_sample_detail() {
        let result = IncidentSample {
            incident_number: 105,
            sample_type: "performance".to_string(),
            sample: sample_with("Web::OrdersController#create"),
        };
        let mut buf = Vec::new();
        render_sample_detail(&mut buf, &result).unwrap();
        let out = String::from_utf8(buf).unwrap();

        assert!(out.contains("Web::OrdersController#create"));
        assert!(out.contains("#105"));
        assert!(out.contains("123.46 ms"));
        assert!(out.contains("N+1"));
    }

    #[test]
    fn renders_exception_sample_with_backtrace() {
        let mut sample = sample_with("Web::OrdersController#show");
        sample.has_n_plus_one = None;
        sample.exception = Some(ExceptionDetail {
            name: Some("RuntimeError".to_string()),
            message: Some("boom".to_string()),
            backtrace: Some(vec![BacktraceLine {
                line: Some(42),
                path: Some("app/controllers/orders_controller.rb".to_string()),
                method: Some("show".to_string()),
                column: None,
                original: None,
                kind: None,
                url: None,
            }]),
        });
        let result = IncidentSample {
            incident_number: 7,
            sample_type: "error".to_string(),
            sample,
        };

        let mut buf = Vec::new();
        render_sample_detail(&mut buf, &result).unwrap();
        let out = String::from_utf8(buf).unwrap();

        assert!(out.contains("RuntimeError"));
        assert!(out.contains("boom"));
        assert!(out.contains("app/controllers/orders_controller.rb:42 in show"));
    }

    #[test]
    fn list_renders_empty_message() {
        let result = IncidentSamples {
            incident_number: 9,
            sample_type: "performance".to_string(),
            samples: vec![],
        };
        let mut buf = Vec::new();
        render_sample_list(&mut buf, &result).unwrap();
        assert!(String::from_utf8(buf).unwrap().contains("No samples found"));
    }

    #[test]
    fn parse_namespaces_splits_and_trims() {
        assert_eq!(
            parse_namespaces(Some("web, background ,, ")),
            Some(vec!["web".to_string(), "background".to_string()])
        );
        assert_eq!(parse_namespaces(None), None);
        // All-empty input collapses to `None`, not `Some([])`.
        assert_eq!(parse_namespaces(Some("")), None);
        assert_eq!(parse_namespaces(Some(" , ,")), None);
    }

    #[test]
    fn user_filter_matches_case_insensitive_substring() {
        let mut sample = sample_with("Web::OrdersController#index");
        sample.overview = Some(vec![crate::api::KeyStringValue {
            key: "user_id".to_string(),
            value: Some("User-42".to_string()),
        }]);
        assert!(sample_matches_user(&sample, "user-42"));
        assert!(sample_matches_user(&sample, "42"));
        assert!(!sample_matches_user(&sample, "user-99"));
    }

    #[test]
    fn window_render_shows_incident_column_and_sorts() {
        let mut a = sample_with("A#a");
        a.created_at = Some("2026-05-19T10:00:00Z".to_string());
        let mut b = sample_with("B#b");
        b.created_at = Some("2026-05-19T09:00:00Z".to_string());

        let mut samples = vec![
            WindowSample {
                incident_number: 1,
                sample_type: "performance".to_string(),
                sample: a,
            },
            WindowSample {
                incident_number: 2,
                sample_type: "error".to_string(),
                sample: b,
            },
        ];
        sort_window_samples(&mut samples);
        assert_eq!(samples[0].incident_number, 2); // earlier timestamp first

        let mut buf = Vec::new();
        render_window_samples(
            &mut buf,
            &samples,
            "2026-05-19T00:00:00Z",
            "2026-05-20T00:00:00Z",
        )
        .unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("INCIDENT"));
        assert!(out.contains("#1"));
        assert!(out.contains("#2"));
    }
}
