//! The `samples` command: fetch the underlying transaction samples behind an
//! incident.
//!
//! `incidents show` is incident-level aggregate only; this command pulls the
//! per-request sample data (action, duration, queue time, params, exception +
//! backtrace) that real triage works from. Inputs can be a full AppSignal URL,
//! a bare sample id, or explicit `--incident`/`--app-id` flags.

use std::io::{self, Write};

use anyhow::{Context, Result};

use super::{authenticated_client, resolve_org};
use crate::api::{
    AppSignalClient, BacktraceLine, IncidentSample, IncidentSamples, Sample, SampleQuery,
};
use crate::appsignal_url::{self, SampleSelector};
use crate::config::Config;
use crate::error::CliError;
use crate::output::{self, Output};

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

    output::print_with(&result, format, |w| render_sample_detail(w, &result))
}

/// List the samples for an incident, optionally within a time window.
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
    format: Output,
) -> Result<()> {
    let parsed = reference.map(appsignal_url::parse).transpose()?;

    let explicit_app_id = parsed
        .as_ref()
        .map(|p| p.app_id.clone())
        .or_else(|| app_id.map(str::to_string));

    let incident_number = parsed
        .as_ref()
        .and_then(|p| p.incident_number)
        .or(incident)
        .context(CliError::msg(
            "Listing samples needs an incident. Pass an incident URL, or use --incident <number>.",
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

    let result = client
        .get_incident_samples(&resolved_app_id, incident_number, start, end, limit)
        .await?;

    output::print_with(&result, format, |w| render_sample_list(w, &result))
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
        writeln!(w, "  {}", format_backtrace_line(frame))?;
    }
    if backtrace.len() > MAX_FRAMES {
        writeln!(w, "  ... {} more frame(s)", backtrace.len() - MAX_FRAMES)?;
    }
    Ok(())
}

fn format_backtrace_line(frame: &BacktraceLine) -> String {
    if let Some(original) = &frame.original {
        if !original.is_empty() {
            return original.clone();
        }
    }
    let location = match (&frame.path, frame.line) {
        (Some(path), Some(line)) => format!("{}:{}", path, line),
        (Some(path), None) => path.clone(),
        _ => "?".to_string(),
    };
    match &frame.method {
        Some(method) => format!("{} in {}", location, method),
        None => location,
    }
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
            exception: None,
            error_causes: None,
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
    fn backtrace_line_prefers_original() {
        let frame = BacktraceLine {
            line: Some(10),
            path: Some("a.rb".to_string()),
            method: Some("m".to_string()),
            column: None,
            original: Some("a.rb:10:in `m'".to_string()),
            kind: None,
            url: None,
        };
        assert_eq!(format_backtrace_line(&frame), "a.rb:10:in `m'");
    }
}
