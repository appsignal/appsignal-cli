use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use serde::Serialize;
use serde_json::Value;

use super::{authenticated_client, resolve_org};
use crate::api::{AppSignalClient, Incident, TraceSpan, TraceSummary};
use crate::config::Config;
use crate::error::CliError;
use crate::output::{self, Output};

#[derive(Serialize)]
struct TraceListResponse<'a> {
    traces: &'a [TraceSummary],
}

#[derive(Serialize)]
struct IncidentSamplesResponse<'a> {
    incident_number: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actions: Option<&'a [ActionSamples]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    digests: Option<&'a [DigestSamples]>,
}

#[derive(Serialize)]
struct ActionSamples {
    action_name: String,
    traces: Vec<TraceSummary>,
}

#[derive(Serialize)]
struct DigestSamples {
    digest: String,
    traces: Vec<TraceSummary>,
}

#[derive(Serialize)]
struct TraceShowResponse<'a> {
    trace_id: &'a str,
    spans: &'a [TraceSpan],
    #[serde(skip_serializing_if = "Option::is_none")]
    span: Option<&'a TraceSpan>,
}

/// List performance sample traces for a namespace/action.
#[allow(clippy::too_many_arguments)]
pub async fn list(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    namespace: &str,
    action_name: &str,
    start: Option<&str>,
    end: Option<&str>,
    min_duration_ms: Option<f64>,
    limit: Option<i64>,
    page_all: bool,
    format: Output,
) -> Result<()> {
    let namespace = clean_required_arg("namespace", namespace)?;
    let action_name = clean_required_arg("action", action_name)?;
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;
    let (start, end) = resolve_time_range(start, end);

    let traces = if page_all {
        fetch_all_trace_pages(
            &client,
            &resolved_app_id,
            namespace,
            action_name,
            &start,
            &end,
            min_duration_ms,
        )
        .await?
    } else {
        client
            .list_performance_traces(
                &resolved_app_id,
                namespace,
                action_name,
                &start,
                &end,
                min_duration_ms,
                limit.unwrap_or(25),
                "DESC",
                None,
            )
            .await?
    };

    output::print_with(TraceListResponse { traces: &traces }, format, |w| {
        render_trace_list(w, &traces, namespace, action_name, &start, &end)
    })
}

/// List performance samples/traces for the action(s) attached to a performance incident.
#[allow(clippy::too_many_arguments)]
pub async fn incident(
    incident_number: i64,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    action_name: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    min_duration_ms: Option<f64>,
    limit: Option<i64>,
    page_all: bool,
    format: Output,
) -> Result<()> {
    let action_name = action_name
        .map(str::trim)
        .filter(|action| !action.is_empty());
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;
    let incident = client
        .get_incident(&resolved_app_id, incident_number)
        .await?;
    let (start, end) = resolve_time_range(start, end);

    match &incident {
        Incident::PerformanceIncident { .. } => {
            let (namespace, actions) = performance_incident_trace_query(&incident, action_name)?;
            let action_samples = fetch_action_samples(
                &client,
                &resolved_app_id,
                &namespace,
                actions,
                &start,
                &end,
                min_duration_ms,
                limit,
                page_all,
            )
            .await?;

            output::print_with(
                IncidentSamplesResponse {
                    incident_number,
                    namespace: Some(&namespace),
                    actions: Some(&action_samples),
                    digests: None,
                },
                format,
                |w| {
                    render_performance_incident_samples(
                        w,
                        incident_number,
                        &namespace,
                        &action_samples,
                        &start,
                        &end,
                    )
                },
            )
        }
        Incident::ExceptionIncident { .. } => {
            if action_name.is_some() {
                anyhow::bail!(CliError::msg(
                    "`--action` is only supported for performance incident sample lookup."
                ));
            }

            let digests = exception_incident_digests(&incident)?;
            let digest_samples = fetch_digest_samples(
                &client,
                &resolved_app_id,
                digests,
                limit,
                page_all,
            )
            .await?;

            output::print_with(
                IncidentSamplesResponse {
                    incident_number,
                    namespace: None,
                    actions: None,
                    digests: Some(&digest_samples),
                },
                format,
                |w| render_exception_incident_samples(w, incident_number, &digest_samples),
            )
        }
        other => anyhow::bail!(CliError::msg(format!(
            "Incident #{} is a {} incident. Sample/trace lookup by incident supports performance and exception incidents.",
            other.number(),
            other.kind()
        ))),
    }
}

/// List error traces for an exception digest.
#[allow(clippy::too_many_arguments)]
pub async fn errors(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    digest: &str,
    limit: Option<i64>,
    page_all: bool,
    format: Output,
) -> Result<()> {
    let digest = clean_required_arg("digest", digest)?;
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let traces = if page_all {
        fetch_all_error_trace_pages(&client, &resolved_app_id, digest).await?
    } else {
        client
            .list_error_traces(&resolved_app_id, digest, limit.unwrap_or(25), "DESC", None)
            .await?
    };

    output::print_with(TraceListResponse { traces: &traces }, format, |w| {
        render_error_trace_list(w, &traces, digest)
    })
}

/// Show the span tree for an error trace, or details for one span.
#[allow(clippy::too_many_arguments)]
pub async fn show_error(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    digest: &str,
    trace_id: &str,
    span_id: Option<&str>,
    include_sensitive: bool,
    format: Output,
) -> Result<()> {
    let digest = clean_required_arg("digest", digest)?;
    let trace_id = clean_required_arg("trace-id", trace_id)?;
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let spans = client
        .get_error_trace(&resolved_app_id, digest, trace_id)
        .await?;
    let span = if let Some(span_id) = span_id {
        Some(
            spans
                .iter()
                .find(|span| span.span_id == span_id)
                .with_context(|| {
                    CliError::msg(format!(
                        "Span {} not found in trace {}. Rerun without `--span-id` to see available span IDs.",
                        span_id, trace_id
                    ))
                })?,
        )
    } else {
        None
    };

    output::print_with(
        TraceShowResponse {
            trace_id,
            spans: &spans,
            span,
        },
        format,
        |w| {
            if let Some(span) = span {
                render_span_detail(w, span, include_sensitive)
            } else {
                render_trace_tree(w, trace_id, &spans)
            }
        },
    )
}

/// Show a trace from an incident without requiring namespace/action or digest arguments.
#[allow(clippy::too_many_arguments)]
pub async fn show_incident(
    incident_number: i64,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    trace_id: &str,
    span_id: Option<&str>,
    include_sensitive: bool,
    start: Option<&str>,
    end: Option<&str>,
    format: Output,
) -> Result<()> {
    let trace_id = clean_required_arg("trace-id", trace_id)?;
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;
    let incident = client
        .get_incident(&resolved_app_id, incident_number)
        .await?;
    let (start, end) = resolve_time_range(start, end);

    let spans = match &incident {
        Incident::PerformanceIncident { .. } => {
            let (namespace, actions) = performance_incident_trace_query(&incident, None)?;
            fetch_performance_trace_for_incident(
                &client,
                &resolved_app_id,
                &namespace,
                &actions,
                trace_id,
                &start,
                &end,
            )
            .await?
        }
        Incident::ExceptionIncident { .. } => {
            let digests = exception_incident_digests(&incident)?;
            fetch_error_trace_for_incident(&client, &resolved_app_id, &digests, trace_id).await?
        }
        other => {
            anyhow::bail!(CliError::msg(format!(
                "Incident #{} is a {} incident. Trace lookup by incident supports performance and exception incidents.",
                other.number(),
                other.kind()
            )))
        }
    };

    let span = if let Some(span_id) = span_id {
        Some(
            spans
                .iter()
                .find(|span| span.span_id == span_id)
                .with_context(|| {
                    CliError::msg(format!(
                        "Span {} not found in trace {}. Rerun without `--span-id` to see available span IDs.",
                        span_id, trace_id
                    ))
                })?,
        )
    } else {
        None
    };

    output::print_with(
        TraceShowResponse {
            trace_id,
            spans: &spans,
            span,
        },
        format,
        |w| {
            if let Some(span) = span {
                render_span_detail(w, span, include_sensitive)
            } else {
                render_trace_tree(w, trace_id, &spans)
            }
        },
    )
}

/// Show the span tree for a performance sample trace, or details for one span.
#[allow(clippy::too_many_arguments)]
pub async fn show(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    namespace: &str,
    action_name: &str,
    trace_id: &str,
    span_id: Option<&str>,
    include_sensitive: bool,
    start: Option<&str>,
    end: Option<&str>,
    format: Output,
) -> Result<()> {
    let namespace = clean_required_arg("namespace", namespace)?;
    let action_name = clean_required_arg("action", action_name)?;
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;
    let (start, end) = resolve_time_range(start, end);

    let spans = client
        .get_performance_trace(
            &resolved_app_id,
            namespace,
            action_name,
            trace_id,
            &start,
            &end,
        )
        .await?;

    let span = if let Some(span_id) = span_id {
        Some(
            spans
                .iter()
                .find(|span| span.span_id == span_id)
                .with_context(|| {
                    CliError::msg(format!(
                        "Span {} not found in trace {}. Rerun without `--span-id` to see available span IDs.",
                        span_id, trace_id
                    ))
                })?,
        )
    } else {
        None
    };

    output::print_with(
        TraceShowResponse {
            trace_id,
            spans: &spans,
            span,
        },
        format,
        |w| {
            if let Some(span) = span {
                render_span_detail(w, span, include_sensitive)
            } else {
                render_trace_tree(w, trace_id, &spans)
            }
        },
    )
}

fn resolve_time_range(start: Option<&str>, end: Option<&str>) -> (String, String) {
    let end = end
        .map(ToString::to_string)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let start = start
        .map(ToString::to_string)
        .unwrap_or_else(|| (Utc::now() - Duration::hours(24)).to_rfc3339());
    (start, end)
}

#[allow(clippy::too_many_arguments)]
async fn fetch_action_samples(
    client: &AppSignalClient,
    app_id: &str,
    namespace: &str,
    actions: Vec<String>,
    start: &str,
    end: &str,
    min_duration_ms: Option<f64>,
    limit: Option<i64>,
    page_all: bool,
) -> Result<Vec<ActionSamples>> {
    let mut action_samples = Vec::with_capacity(actions.len());
    for action in actions {
        let traces = if page_all {
            fetch_all_trace_pages(
                client,
                app_id,
                namespace,
                &action,
                start,
                end,
                min_duration_ms,
            )
            .await?
        } else {
            client
                .list_performance_traces(
                    app_id,
                    namespace,
                    &action,
                    start,
                    end,
                    min_duration_ms,
                    limit.unwrap_or(25),
                    "DESC",
                    None,
                )
                .await?
        };
        action_samples.push(ActionSamples {
            action_name: action,
            traces,
        });
    }
    Ok(action_samples)
}

async fn fetch_digest_samples(
    client: &AppSignalClient,
    app_id: &str,
    digests: Vec<String>,
    limit: Option<i64>,
    page_all: bool,
) -> Result<Vec<DigestSamples>> {
    let mut digest_samples = Vec::with_capacity(digests.len());
    for digest in digests {
        let traces = if page_all {
            fetch_all_error_trace_pages(client, app_id, &digest).await?
        } else {
            client
                .list_error_traces(app_id, &digest, limit.unwrap_or(25), "DESC", None)
                .await?
        };
        digest_samples.push(DigestSamples { digest, traces });
    }
    Ok(digest_samples)
}

async fn fetch_performance_trace_for_incident(
    client: &AppSignalClient,
    app_id: &str,
    namespace: &str,
    actions: &[String],
    trace_id: &str,
    start: &str,
    end: &str,
) -> Result<Vec<TraceSpan>> {
    let mut not_found_count = 0;
    for action in actions {
        match client
            .get_performance_trace(app_id, namespace, action, trace_id, start, end)
            .await
        {
            Ok(spans) if !spans.is_empty() => return Ok(spans),
            Ok(_) => not_found_count += 1,
            Err(error) if is_not_found_error(&error) => not_found_count += 1,
            Err(error) => return Err(error),
        }
    }

    anyhow::bail!(CliError::msg(format!(
        "Trace {} was not found for this performance incident across {} action(s). Try expanding --start/--end if the trace is older than 24 hours.",
        trace_id, not_found_count
    )))
}

async fn fetch_error_trace_for_incident(
    client: &AppSignalClient,
    app_id: &str,
    digests: &[String],
    trace_id: &str,
) -> Result<Vec<TraceSpan>> {
    let mut not_found_count = 0;
    for digest in digests {
        match client.get_error_trace(app_id, digest, trace_id).await {
            Ok(spans) if !spans.is_empty() => return Ok(spans),
            Ok(_) => not_found_count += 1,
            Err(error) if is_not_found_error(&error) => not_found_count += 1,
            Err(error) => return Err(error),
        }
    }

    anyhow::bail!(CliError::msg(format!(
        "Trace {} was not found for this exception incident across {} digest(s).",
        trace_id, not_found_count
    )))
}

fn is_not_found_error(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<CliError>(),
        Some(CliError::NotFound { .. })
    )
}

async fn fetch_all_trace_pages(
    client: &AppSignalClient,
    app_id: &str,
    namespace: &str,
    action_name: &str,
    start: &str,
    end: &str,
    min_duration_ms: Option<f64>,
) -> Result<Vec<TraceSummary>> {
    let mut all_traces = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut cursor_time: Option<String> = None;
    let mut page = 0;

    loop {
        page += 1;
        let batch = client
            .list_performance_traces(
                app_id,
                namespace,
                action_name,
                start,
                end,
                min_duration_ms,
                100,
                "DESC",
                cursor_time.as_deref(),
            )
            .await?;

        let batch_len = batch.len();
        let next_cursor = batch.last().and_then(|trace| trace.time.clone());

        for trace in batch {
            let id = trace_identity(&trace);
            if seen_ids.insert(id) {
                all_traces.push(trace);
            }
        }

        crate::status!(
            "Page {}: fetched {} samples/traces ({} total unique)",
            page,
            batch_len,
            all_traces.len()
        );

        if batch_len < 100 || next_cursor.is_none() {
            break;
        }

        if next_cursor == cursor_time {
            anyhow::bail!(CliError::msg(format!(
                "Trace pagination cursor did not advance at {}. Re-run with a narrower --start/--end time range.",
                cursor_time.as_deref().unwrap_or("the current cursor")
            )));
        }

        cursor_time = next_cursor;
    }

    Ok(all_traces)
}

async fn fetch_all_error_trace_pages(
    client: &AppSignalClient,
    app_id: &str,
    digest: &str,
) -> Result<Vec<TraceSummary>> {
    let mut all_traces = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut cursor_time: Option<String> = None;
    let mut page = 0;

    loop {
        page += 1;
        let batch = client
            .list_error_traces(app_id, digest, 100, "DESC", cursor_time.as_deref())
            .await?;

        let batch_len = batch.len();
        let next_cursor = batch.last().and_then(|trace| trace.time.clone());

        for trace in batch {
            let id = trace_identity(&trace);
            if seen_ids.insert(id) {
                all_traces.push(trace);
            }
        }

        crate::status!(
            "Page {}: fetched {} error traces ({} total unique)",
            page,
            batch_len,
            all_traces.len()
        );

        if batch_len < 100 || next_cursor.is_none() {
            break;
        }

        if next_cursor == cursor_time {
            anyhow::bail!(CliError::msg(format!(
                "Trace pagination cursor did not advance at {}. Re-run with a narrower digest or inspect recent traces without --page-all.",
                cursor_time.as_deref().unwrap_or("the current cursor")
            )));
        }

        cursor_time = next_cursor;
    }

    Ok(all_traces)
}

fn trace_identity(trace: &TraceSummary) -> String {
    trace
        .span_id
        .as_deref()
        .unwrap_or(&trace.trace_id)
        .to_string()
}

fn clean_required_arg<'a>(label: &str, value: &'a str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!(CliError::msg(format!("`--{}` cannot be empty.", label)));
    }
    Ok(value)
}

fn performance_incident_trace_query(
    incident: &Incident,
    action_name: Option<&str>,
) -> Result<(String, Vec<String>)> {
    match incident {
        Incident::PerformanceIncident {
            number,
            action_names,
            namespace,
            ..
        } => {
            let namespace = namespace
                .as_deref()
                .map(str::trim)
                .filter(|namespace| !namespace.is_empty())
                .with_context(|| {
                    CliError::msg(format!(
                        "Performance incident #{} has no namespace to query samples/traces.",
                        number
                    ))
                })?
                .to_string();

            let available_actions = action_names.as_deref().unwrap_or(&[]);
            let action_name = action_name.map(str::trim).filter(|action| !action.is_empty());
            let actions = if let Some(action_name) = action_name {
                if !available_actions.is_empty()
                    && !available_actions
                        .iter()
                        .any(|available| available.trim() == action_name)
                {
                    anyhow::bail!(CliError::msg(format!(
                        "Action {:?} is not attached to performance incident #{}. Available actions: {}.",
                        action_name,
                        number,
                        available_actions.join(", ")
                    )));
                }
                vec![action_name.to_string()]
            } else {
                available_actions
                    .iter()
                    .map(|action| action.trim())
                    .filter(|action| !action.is_empty())
                    .map(ToString::to_string)
                    .collect()
            };

            if actions.is_empty() {
                anyhow::bail!(CliError::msg(format!(
                    "Performance incident #{} has no action names to query samples/traces.",
                    number
                )));
            }

            Ok((namespace, actions))
        }
        other => anyhow::bail!(CliError::msg(format!(
            "Incident #{} is a {} incident. Sample/trace lookup by incident only supports performance incidents.",
            other.number(),
            other.kind()
        ))),
    }
}

fn exception_incident_digests(incident: &Incident) -> Result<Vec<String>> {
    match incident {
        Incident::ExceptionIncident {
            number, digests, ..
        } => {
            let digests: Vec<String> = digests
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|digest| digest.trim())
                .filter(|digest| !digest.is_empty())
                .map(ToString::to_string)
                .collect();

            if digests.is_empty() {
                anyhow::bail!(CliError::msg(format!(
                    "Exception incident #{} has no digests to query error traces.",
                    number
                )));
            }

            Ok(digests)
        }
        other => anyhow::bail!(CliError::msg(format!(
            "Incident #{} is a {} incident. Digest lookup only supports exception incidents.",
            other.number(),
            other.kind()
        ))),
    }
}

fn render_performance_incident_samples(
    w: &mut dyn Write,
    incident_number: i64,
    namespace: &str,
    action_samples: &[ActionSamples],
    start: &str,
    end: &str,
) -> io::Result<()> {
    writeln!(
        w,
        "Performance samples/traces for incident #{}",
        incident_number
    )?;
    writeln!(w, "Namespace: {}", namespace)?;
    writeln!(w, "Time range: {} to {}", start, end)?;

    let mut total = 0;
    for samples in action_samples {
        writeln!(w)?;
        writeln!(w, "Action: {}", samples.action_name)?;
        if samples.traces.is_empty() {
            writeln!(w, "No samples/traces found.")?;
            continue;
        }

        total += samples.traces.len();
        render_trace_rows(w, &samples.traces)?;
    }

    writeln!(w, "{} sample(s)/trace(s) found.", total)
}

fn render_exception_incident_samples(
    w: &mut dyn Write,
    incident_number: i64,
    digest_samples: &[DigestSamples],
) -> io::Result<()> {
    writeln!(
        w,
        "Error traces for exception incident #{}",
        incident_number
    )?;

    let mut total = 0;
    for samples in digest_samples {
        writeln!(w)?;
        writeln!(w, "Digest: {}", samples.digest)?;
        if samples.traces.is_empty() {
            writeln!(w, "No error traces found.")?;
            continue;
        }

        total += samples.traces.len();
        render_trace_rows(w, &samples.traces)?;
    }

    writeln!(w, "{} error trace(s) found.", total)
}

fn render_trace_list(
    w: &mut dyn Write,
    traces: &[TraceSummary],
    namespace: &str,
    action_name: &str,
    start: &str,
    end: &str,
) -> io::Result<()> {
    writeln!(
        w,
        "Performance samples/traces for {} / {}",
        namespace, action_name
    )?;
    writeln!(w, "Time range: {} to {}", start, end)?;

    if traces.is_empty() {
        return writeln!(w, "No samples/traces found.");
    }

    render_trace_rows(w, traces)?;

    writeln!(w, "{} sample(s)/trace(s) found.", traces.len())
}

fn render_error_trace_list(
    w: &mut dyn Write,
    traces: &[TraceSummary],
    digest: &str,
) -> io::Result<()> {
    writeln!(w, "Error traces for digest {}", digest)?;

    if traces.is_empty() {
        return writeln!(w, "No error traces found.");
    }

    render_trace_rows(w, traces)?;
    writeln!(w, "{} error trace(s) found.", traces.len())
}

fn render_trace_rows(w: &mut dyn Write, traces: &[TraceSummary]) -> io::Result<()> {
    writeln!(
        w,
        "{:<60} {:>12} {:<22} ACTION",
        "TRACE ID", "DURATION", "TIME"
    )?;
    writeln!(w, "{}", "-".repeat(100))?;

    for trace in traces {
        writeln!(
            w,
            "{:<60} {:>12} {:<22} {}",
            trace.trace_id,
            format_duration(trace.duration),
            trace.time.as_deref().unwrap_or("-"),
            truncate(trace.action_name.as_deref().unwrap_or("-"), 40),
        )?;
    }

    Ok(())
}

fn render_trace_tree(w: &mut dyn Write, trace_id: &str, spans: &[TraceSpan]) -> io::Result<()> {
    if spans.is_empty() {
        return writeln!(w, "No spans found for sample/trace {}.", trace_id);
    }

    writeln!(w, "Span tree for sample/trace {}", trace_id)?;
    let children = children_by_parent(spans);
    let roots: Vec<usize> = spans
        .iter()
        .enumerate()
        .filter_map(|(idx, span)| {
            let parent = span.parent_span_id.as_deref().unwrap_or_default();
            (parent.is_empty() || !spans.iter().any(|candidate| candidate.span_id == parent))
                .then_some(idx)
        })
        .collect();

    for idx in roots {
        render_span_tree_line(w, spans, &children, idx, 0)?;
    }

    let error_count = spans
        .iter()
        .filter(|span| span.status_code.as_deref() == Some("error"))
        .count();
    if let Some(slowest) = spans.iter().max_by(|left, right| {
        left.duration
            .unwrap_or(0.0)
            .total_cmp(&right.duration.unwrap_or(0.0))
    }) {
        writeln!(w)?;
        writeln!(w, "Total spans: {}", spans.len())?;
        writeln!(w, "Error spans: {}", error_count)?;
        writeln!(
            w,
            "Slowest span: {} ({})",
            span_label(slowest),
            format_duration(slowest.duration),
        )?;
    }

    Ok(())
}

fn children_by_parent(spans: &[TraceSpan]) -> HashMap<&str, Vec<usize>> {
    let mut children: HashMap<&str, Vec<usize>> = HashMap::new();
    for (idx, span) in spans.iter().enumerate() {
        if let Some(parent) = span
            .parent_span_id
            .as_deref()
            .filter(|parent| !parent.is_empty())
        {
            children.entry(parent).or_default().push(idx);
        }
    }
    children
}

fn render_span_tree_line(
    w: &mut dyn Write,
    spans: &[TraceSpan],
    children: &HashMap<&str, Vec<usize>>,
    idx: usize,
    depth: usize,
) -> io::Result<()> {
    let span = &spans[idx];
    let indent = "  ".repeat(depth);
    let event_hint = if span.event_names.is_empty() {
        String::new()
    } else {
        format!("  {} event(s)", span.event_names.len())
    };
    let error_hint = span_error_hint(span).unwrap_or_default();
    writeln!(
        w,
        "{}{} [{}] {}{}{}  span:{}",
        indent,
        span_label(span),
        span.span_kind.as_deref().unwrap_or("unknown"),
        format_duration(span.duration),
        event_hint,
        error_hint,
        span.span_id,
    )?;

    if let Some(child_indexes) = children.get(span.span_id.as_str()) {
        for child_idx in child_indexes {
            render_span_tree_line(w, spans, children, *child_idx, depth + 1)?;
        }
    }

    Ok(())
}

fn render_span_detail(
    w: &mut dyn Write,
    span: &TraceSpan,
    include_sensitive: bool,
) -> io::Result<()> {
    writeln!(w, "Span details")?;
    let label = span_label(span);
    let duration = format_duration(span.duration);
    output::detail(
        w,
        &[
            ("Span ID", &span.span_id),
            ("Trace ID", &span.trace_id),
            ("Parent", span.parent_span_id.as_deref().unwrap_or("-")),
            ("Name", &label),
            ("Kind", span.span_kind.as_deref().unwrap_or("-")),
            ("Status", span.status_code.as_deref().unwrap_or("-")),
            ("Duration", &duration),
            ("Start", span.start_time.as_deref().unwrap_or("-")),
            ("End", span.end_time.as_deref().unwrap_or("-")),
        ],
    )?;

    render_attribute_section(w, "Tags", &tag_attributes(span), true)?;
    render_attribute_section(
        w,
        "Span Attributes",
        &filtered_span_attributes(span, include_sensitive),
        false,
    )?;
    render_events(w, span)?;
    render_attribute_section(
        w,
        "Resource Attributes",
        &filtered_resource_attributes(span),
        false,
    )?;

    if include_sensitive {
        writeln!(
            w,
            "Sensitive HTTP/request/session attributes shown where present."
        )?;
    } else {
        writeln!(w, "Sensitive HTTP/request/session attributes omitted. Pass `--include-sensitive` to include them.")?;
    }

    Ok(())
}

fn render_attribute_section(
    w: &mut dyn Write,
    title: &str,
    attributes: &[(String, String)],
    strip_tag_prefix: bool,
) -> io::Result<()> {
    if attributes.is_empty() {
        return Ok(());
    }

    writeln!(w)?;
    writeln!(w, "{}", title)?;
    for (key, value) in attributes {
        let key = if strip_tag_prefix {
            key.strip_prefix("appsignal.tag.").unwrap_or(key)
        } else {
            key
        };
        writeln!(w, "{}: {}", key, truncate(value, 500))?;
    }
    Ok(())
}

fn render_events(w: &mut dyn Write, span: &TraceSpan) -> io::Result<()> {
    if span.event_names.is_empty() {
        return Ok(());
    }

    writeln!(w)?;
    writeln!(w, "Events")?;
    for (idx, name) in span.event_names.iter().enumerate() {
        let timestamp = span
            .event_timestamps
            .get(idx)
            .map(String::as_str)
            .unwrap_or("-");
        writeln!(w, "{} ({})", name, timestamp)?;
        if let Some(attributes) = span.event_attributes.get(idx) {
            let mut attributes = map_to_pairs(attributes);
            attributes.retain(|(key, _)| key != "appsignal.stacktrace_id");
            for (key, value) in attributes {
                writeln!(w, "{}: {}", key, truncate(&value, 500))?;
            }
        }
    }
    Ok(())
}

fn tag_attributes(span: &TraceSpan) -> Vec<(String, String)> {
    map_to_pairs(&span.span_attributes)
        .into_iter()
        .filter(|(key, _)| key.starts_with("appsignal.tag."))
        .collect()
}

fn filtered_span_attributes(span: &TraceSpan, include_sensitive: bool) -> Vec<(String, String)> {
    map_to_pairs(&span.span_attributes)
        .into_iter()
        .filter(|(key, _)| !key.starts_with("appsignal.tag."))
        .filter(|(key, _)| include_sensitive || !is_sensitive_attribute(key))
        .filter(|(key, _)| !matches!(key.as_str(), "idle_ns" | "busy_ns" | "trace_id"))
        .collect()
}

fn filtered_resource_attributes(span: &TraceSpan) -> Vec<(String, String)> {
    map_to_pairs(&span.resource_attributes)
        .into_iter()
        .filter(|(key, _)| !key.starts_with("appsignal.config."))
        .collect()
}

fn is_sensitive_attribute(key: &str) -> bool {
    [
        "http.request.header.",
        "http.response.header.",
        "appsignal.request.query_parameters",
        "appsignal.request.payload",
        "appsignal.request.session_data",
        "appsignal.function.parameters",
    ]
    .iter()
    .any(|prefix| key.starts_with(prefix))
}

fn map_to_pairs(map: &serde_json::Map<String, Value>) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = map
        .iter()
        .map(|(key, value)| (key.clone(), value_to_string(value)))
        .collect();
    pairs.sort_by(|left, right| left.0.cmp(&right.0));
    pairs
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "".to_string(),
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn span_label(span: &TraceSpan) -> String {
    span.span_name
        .as_deref()
        .or(span.action_name.as_deref())
        .filter(|name| !name.is_empty())
        .unwrap_or("(unknown)")
        .to_string()
}

fn span_error_hint(span: &TraceSpan) -> Option<String> {
    let idx = span
        .event_names
        .iter()
        .position(|name| name == "exception")?;
    let attributes = span.event_attributes.get(idx)?;
    attributes
        .get("exception.type")
        .map(|value| format!("  !! {}", value_to_string(value)))
        .or_else(|| Some("  !! exception".to_string()))
}

fn format_duration(duration: Option<f64>) -> String {
    match duration {
        Some(duration) if duration.is_finite() => format!("{duration:.1}ms"),
        _ => "-".to_string(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::AppSignalClient;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    fn rest_trace_json(id: &str, time: &str) -> serde_json::Value {
        json!({
            "span_id": id,
            "trace_id": id,
            "site_id": "app1",
            "namespace": "web",
            "revision": null,
            "action_name": "PostsController#index",
            "time": time,
            "duration": 123.4,
            "tags": {},
            "has_npo": false
        })
    }

    #[test]
    fn sensitive_attributes_are_filtered_by_default() {
        assert!(is_sensitive_attribute("http.request.header.accept"));
        assert!(is_sensitive_attribute("appsignal.request.session_data"));
        assert!(!is_sensitive_attribute("db.system"));
    }

    #[test]
    fn trace_identity_prefers_span_id() {
        let trace = TraceSummary {
            span_id: Some("span-1".to_string()),
            trace_id: "trace-1".to_string(),
            site_id: None,
            namespace: None,
            revision: None,
            action_name: None,
            time: None,
            duration: None,
            tags: serde_json::Map::new(),
            has_npo: None,
        };

        assert_eq!(trace_identity(&trace), "span-1");
    }

    #[tokio::test]
    async fn fetch_all_trace_pages_fetches_multiple_pages() {
        let call_count = std::sync::Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v2/tracing/traces/performance"))
            .respond_with(move |_req: &Request| {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    let traces: Vec<_> = (0..100)
                        .map(|i| rest_trace_json(&format!("trace-{}", i), "2026-06-22T10:00:00Z"))
                        .collect();
                    ResponseTemplate::new(200).set_body_json(traces)
                } else {
                    ResponseTemplate::new(200)
                        .set_body_json(vec![rest_trace_json("trace-100", "2026-06-22T09:00:00Z")])
                }
            })
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let traces = fetch_all_trace_pages(
            &client,
            "app1",
            "web",
            "PostsController#index",
            "2026-06-22T00:00:00Z",
            "2026-06-23T00:00:00Z",
            None,
        )
        .await
        .unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 2);
        assert_eq!(traces.len(), 101);
        assert_eq!(traces[100].trace_id, "trace-100");
    }

    #[tokio::test]
    async fn fetch_all_trace_pages_deduplicates_boundary_traces() {
        let call_count = std::sync::Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v2/tracing/traces/performance"))
            .respond_with(move |_req: &Request| {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    let traces: Vec<_> = (0..100)
                        .map(|i| rest_trace_json(&format!("trace-{}", i), "2026-06-22T10:00:00Z"))
                        .collect();
                    ResponseTemplate::new(200).set_body_json(traces)
                } else {
                    ResponseTemplate::new(200).set_body_json(vec![
                        rest_trace_json("trace-99", "2026-06-22T09:00:00Z"),
                        rest_trace_json("trace-100", "2026-06-22T09:00:00Z"),
                    ])
                }
            })
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let traces = fetch_all_trace_pages(
            &client,
            "app1",
            "web",
            "PostsController#index",
            "2026-06-22T00:00:00Z",
            "2026-06-23T00:00:00Z",
            None,
        )
        .await
        .unwrap();

        assert_eq!(traces.len(), 101);
        assert_eq!(
            traces
                .iter()
                .filter(|trace| trace.trace_id == "trace-99")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn fetch_all_trace_pages_errors_when_cursor_does_not_advance() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/tracing/traces/performance"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    (0..100)
                        .map(|i| rest_trace_json(&format!("trace-{}", i), "2026-06-22T10:00:00Z"))
                        .collect::<Vec<_>>(),
                ),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let err = fetch_all_trace_pages(
            &client,
            "app1",
            "web",
            "PostsController#index",
            "2026-06-22T00:00:00Z",
            "2026-06-23T00:00:00Z",
            None,
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("cursor did not advance"));
    }

    #[test]
    fn span_error_hint_uses_exception_type() {
        let span = TraceSpan {
            span_id: "span-1".to_string(),
            trace_id: "trace-1".to_string(),
            parent_span_id: None,
            span_name: None,
            span_kind: None,
            duration: None,
            start_time: None,
            end_time: None,
            status_code: None,
            status_message: None,
            service_name: None,
            namespace: None,
            revision: None,
            action_name: None,
            span_attributes: serde_json::Map::new(),
            event_names: vec!["exception".to_string()],
            event_timestamps: Vec::new(),
            event_attributes: vec![json!({ "exception.type": "RuntimeError" })
                .as_object()
                .unwrap()
                .clone()],
            resource_attributes: serde_json::Map::new(),
        };

        assert_eq!(span_error_hint(&span).as_deref(), Some("  !! RuntimeError"));
    }

    #[test]
    fn performance_incident_trace_query_uses_incident_actions() {
        let incident = Incident::PerformanceIncident {
            id: "incident-1".to_string(),
            number: 42,
            state: Some("OPEN".to_string()),
            severity: None,
            description: None,
            count: 10,
            created_at: None,
            last_occurred_at: None,
            updated_at: None,
            action_names: Some(vec![
                "PostsController#index".to_string(),
                "PostsController#show".to_string(),
            ]),
            namespace: Some("web".to_string()),
            mean: Some(250.0),
            total_duration: Some(2500.0),
            assignees: None,
        };

        let (namespace, actions) = performance_incident_trace_query(&incident, None).unwrap();

        assert_eq!(namespace, "web");
        assert_eq!(
            actions,
            vec!["PostsController#index", "PostsController#show"]
        );
    }

    #[test]
    fn performance_incident_trace_query_can_filter_action() {
        let incident = Incident::PerformanceIncident {
            id: "incident-1".to_string(),
            number: 42,
            state: Some("OPEN".to_string()),
            severity: None,
            description: None,
            count: 10,
            created_at: None,
            last_occurred_at: None,
            updated_at: None,
            action_names: Some(vec!["PostsController#index".to_string()]),
            namespace: Some("web".to_string()),
            mean: Some(250.0),
            total_duration: Some(2500.0),
            assignees: None,
        };

        let (_namespace, actions) =
            performance_incident_trace_query(&incident, Some("PostsController#index")).unwrap();

        assert_eq!(actions, vec!["PostsController#index"]);
    }

    #[test]
    fn performance_incident_trace_query_trims_whitespace() {
        let incident = Incident::PerformanceIncident {
            id: "incident-1".to_string(),
            number: 42,
            state: Some("OPEN".to_string()),
            severity: None,
            description: None,
            count: 10,
            created_at: None,
            last_occurred_at: None,
            updated_at: None,
            action_names: Some(vec!["WaitlistsController#create ".to_string()]),
            namespace: Some(" development ".to_string()),
            mean: Some(250.0),
            total_duration: Some(2500.0),
            assignees: None,
        };

        let (namespace, actions) =
            performance_incident_trace_query(&incident, Some("WaitlistsController#create "))
                .unwrap();

        assert_eq!(namespace, "development");
        assert_eq!(actions, vec!["WaitlistsController#create"]);
    }

    #[test]
    fn exception_incident_digests_trims_values() {
        let incident = Incident::ExceptionIncident {
            id: "incident-1".to_string(),
            number: 42,
            state: Some("OPEN".to_string()),
            severity: None,
            description: None,
            count: 10,
            created_at: None,
            last_occurred_at: None,
            updated_at: None,
            exception_name: Some("RuntimeError".to_string()),
            exception_message: Some("boom".to_string()),
            action_names: Some(vec!["PostsController#index".to_string()]),
            namespace: Some("web".to_string()),
            first_backtrace_line: None,
            digests: Some(vec![" digest-1 ".to_string(), "".to_string()]),
            assignees: None,
        };

        let digests = exception_incident_digests(&incident).unwrap();

        assert_eq!(digests, vec!["digest-1"]);
    }
}
