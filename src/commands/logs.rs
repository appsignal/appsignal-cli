use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use tokio::time::{sleep, Duration};

use super::{authenticated_client, resolve_org};
use crate::api::{
    AppSignalClient, LogLine, LogLineAction, LogLineActionTriggerInput, LogLineMetricInput, LogView,
};
use crate::config::Config;
use crate::output::{self, Output};

#[derive(Serialize)]
struct LogLinesResponse<'a> {
    lines: &'a [LogLine],
}

#[derive(Serialize)]
struct LogViewsResponse<'a> {
    views: &'a [LogView],
}

#[derive(Serialize)]
struct LogSourcesResponse<'a> {
    sources: &'a [crate::api::LogSource],
}

#[derive(Serialize)]
struct LogMetricsResponse<'a> {
    metrics: &'a [LogLineAction],
}

#[derive(Serialize)]
struct LogMetricResponse<'a> {
    metric: &'a LogLineAction,
}

#[derive(Serialize)]
struct LogTriggersResponse<'a> {
    triggers: &'a [LogLineAction],
}

#[derive(Serialize)]
struct LogTriggerResponse<'a> {
    trigger: &'a LogLineAction,
}

/// Tail (stream) log lines for an application, polling every second.
/// Supports filtering by query, severities, source IDs, and log view.
#[allow(clippy::too_many_arguments)]
pub async fn tail(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    query: Option<&str>,
    severities: Option<&str>,
    source_ids: Option<&str>,
    view: Option<&str>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let (view_query, view_source_ids, view_severities) = if let Some(view_name) = view {
        let lv = resolve_log_view(&client, &resolved_app_id, view_name).await?;
        (
            lv.query.clone(),
            lv.source_ids.clone(),
            lv.severities.clone(),
        )
    } else {
        (None, None, None)
    };

    let effective_query = query.map(|q| q.to_string()).or(view_query);
    let effective_source_ids = source_ids
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .collect::<Vec<_>>()
        })
        .or(view_source_ids);
    let effective_severities = severities
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_uppercase())
                .collect::<Vec<_>>()
        })
        .or(view_severities);

    let (resolved_source_ids, source_names) =
        resolve_search_sources(&client, &resolved_app_id, effective_source_ids.as_deref()).await?;
    let effective_query =
        merge_rest_log_query(effective_query.as_deref(), effective_severities.as_deref());

    crate::status!(
        "Tailing logs{} (Ctrl+C to stop)...",
        if let Some(v) = view {
            format!(" [view: {}]", v)
        } else {
            String::new()
        }
    );

    // Start 60 seconds in the past for initial context
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut start = Utc::now() - chrono::Duration::seconds(60);

    loop {
        let end = Utc::now();
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();

        let lines = AppSignalClient::rest_log_lines_to_log_lines(
            client
                .list_log_lines_rest(
                    &resolved_app_id,
                    Some(&start_str),
                    Some(&end_str),
                    &resolved_source_ids,
                    &effective_query,
                    100,
                    "ASC",
                    None,
                )
                .await?,
            &source_names,
        );

        for line in &lines {
            if seen_ids.insert(line.id.clone()) {
                print_log_line(line, format)?;
            }
        }

        start = end - chrono::Duration::seconds(120);

        if seen_ids.len() > 1000 {
            seen_ids.clear();
            for line in &lines {
                seen_ids.insert(line.id.clone());
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}

/// Search log lines (one-shot query). Designed for both human use and LLM consumption.
/// With --page-all, automatically paginates by slicing the time window to collect all results.
#[allow(clippy::too_many_arguments)]
pub async fn search(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    query: Option<&str>,
    severities: Option<&str>,
    source_ids: Option<&str>,
    view: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    limit: Option<i64>,
    order: Option<&str>,
    page_all: bool,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let (view_query, view_source_ids, view_severities) = if let Some(view_name) = view {
        let lv = resolve_log_view(&client, &resolved_app_id, view_name).await?;
        (
            lv.query.clone(),
            lv.source_ids.clone(),
            lv.severities.clone(),
        )
    } else {
        (None, None, None)
    };

    let effective_query = query.map(|q| q.to_string()).or(view_query);
    let effective_source_ids = source_ids
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .collect::<Vec<_>>()
        })
        .or(view_source_ids);
    let effective_severities = severities
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_uppercase())
                .collect::<Vec<_>>()
        })
        .or(view_severities);

    let (resolved_source_ids, source_names) =
        resolve_search_sources(&client, &resolved_app_id, effective_source_ids.as_deref()).await?;
    let effective_query =
        merge_rest_log_query(effective_query.as_deref(), effective_severities.as_deref());

    let lines = if page_all {
        fetch_all_pages(
            &client,
            &resolved_app_id,
            start,
            end,
            &resolved_source_ids,
            &effective_query,
            &source_names,
        )
        .await?
    } else {
        let raw_lines = client
            .list_log_lines_rest(
                &resolved_app_id,
                start,
                end,
                &resolved_source_ids,
                &effective_query,
                limit.unwrap_or(100),
                order.unwrap_or("DESC"),
                None,
            )
            .await?;
        AppSignalClient::rest_log_lines_to_log_lines(raw_lines, &source_names)
    };

    output::print_with(LogLinesResponse { lines: &lines }, format, |w| {
        if lines.is_empty() {
            return writeln!(w, "No log lines found.");
        }

        for line in &lines {
            render_log_line_human(w, line)?;
        }

        writeln!(w, "{} log line(s) returned.", lines.len())
    })
}

/// Fetch all log lines in a time range by paginating with time-window slicing.
/// Fetches in ASC order, and when a page returns 100 results (the API cap),
/// uses the last timestamp as the start of the next page.
/// Deduplicates by log line ID to handle boundary overlap.
async fn fetch_all_pages(
    client: &AppSignalClient,
    app_id: &str,
    start: Option<&str>,
    end: Option<&str>,
    source_ids: &[String],
    query: &str,
    source_names: &HashMap<String, String>,
) -> Result<Vec<LogLine>> {
    let mut all_lines: Vec<LogLine> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut current_cursor: Option<String> = None;
    let page_limit: i64 = 100;
    let mut page = 0;

    loop {
        page += 1;
        let batch = AppSignalClient::rest_log_lines_to_log_lines(
            client
                .list_log_lines_rest(
                    app_id,
                    start,
                    end,
                    source_ids,
                    query,
                    page_limit,
                    "ASC",
                    current_cursor.as_deref(),
                )
                .await?,
            source_names,
        );

        let batch_len = batch.len();

        for line in batch {
            if seen_ids.insert(line.id.clone()) {
                all_lines.push(line);
            }
        }

        crate::status!(
            "Page {}: fetched {} lines ({} total unique)",
            page,
            batch_len,
            all_lines.len()
        );

        // If we got fewer than the limit, we've exhausted the range
        if batch_len < page_limit as usize {
            break;
        }

        // Use the last line's timestamp as the new start boundary
        let last_timestamp = all_lines
            .last()
            .map(|l| l.timestamp.clone())
            .context("Unexpected empty result after pagination")?;
        current_cursor = Some(last_timestamp);
    }

    Ok(all_lines)
}

async fn resolve_search_sources(
    client: &AppSignalClient,
    app_id: &str,
    requested_source_ids: Option<&[String]>,
) -> Result<(Vec<String>, HashMap<String, String>)> {
    let sources = client.list_log_sources(app_id).await?;
    let source_names: HashMap<String, String> = sources
        .iter()
        .map(|source| (source.id.clone(), source.name.clone()))
        .collect();

    let resolved_source_ids = match requested_source_ids {
        Some(source_ids) => source_ids.to_vec(),
        None => sources.into_iter().map(|source| source.id).collect(),
    };

    Ok((resolved_source_ids, source_names))
}

fn merge_rest_log_query(query: Option<&str>, severities: Option<&[String]>) -> String {
    let mut parts = Vec::new();

    if let Some(query) = query.filter(|query| !query.trim().is_empty()) {
        parts.push(query.trim().to_string());
    }

    if let Some(severities) = severities.filter(|severities| !severities.is_empty()) {
        let joined = severities
            .iter()
            .map(|severity| severity.to_lowercase())
            .collect::<Vec<_>>()
            .join(",");
        parts.push(format!("severity=[{}]", joined));
    }

    parts.join(" ")
}

/// List all log views (saved filter presets) for an app.
pub async fn views(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let views = client.list_log_views(&resolved_app_id).await?;

    output::print_with(LogViewsResponse { views: &views }, format, |w| {
        render_log_views(w, &views)
    })
}

/// List all log sources for an app.
pub async fn sources(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let sources = client.list_log_sources(&resolved_app_id).await?;

    output::print_with(LogSourcesResponse { sources: &sources }, format, |w| {
        render_log_sources(w, &sources)
    })
}

pub async fn list_metrics(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    format: Output,
) -> Result<()> {
    let actions = load_actions(app_id, app_name, environment, org, Some("metrics")).await?;

    output::print_with(LogMetricsResponse { metrics: &actions }, format, |w| {
        render_log_metrics(w, &actions)
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn create_metric(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    name: &str,
    query_text: &str,
    source_ids: &[String],
    metrics: &[String],
    format: Output,
) -> Result<()> {
    let action = create_action(
        app_id,
        app_name,
        environment,
        org,
        "metrics",
        name,
        query_text,
        source_ids,
        metrics,
        None,
        &[],
        &[],
    )
    .await?;

    crate::status!("Log metric {} created.", action.id());
    output::print_with(LogMetricResponse { metric: &action }, format, |w| {
        render_log_metric_detail(w, &action)
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn update_metric(
    id: &str,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    name: Option<&str>,
    query_text: Option<&str>,
    source_ids: Option<Vec<String>>,
    metrics: Option<Vec<String>>,
    format: Output,
) -> Result<()> {
    let action = update_action(
        id,
        app_id,
        app_name,
        environment,
        org,
        "metrics",
        name,
        query_text,
        source_ids,
        metrics,
        None,
        None,
        None,
    )
    .await?;

    crate::status!("Log metric {} updated.", action.id());
    output::print_with(LogMetricResponse { metric: &action }, format, |w| {
        render_log_metric_detail(w, &action)
    })
}

pub async fn delete_metric(
    id: &str,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    format: Output,
) -> Result<()> {
    let action = delete_action(id, app_id, app_name, environment, org, "metrics").await?;

    crate::status!("Log metric {} deleted.", action.id());
    output::print_with(LogMetricResponse { metric: &action }, format, |w| {
        render_log_metric_detail(w, &action)
    })
}

pub async fn list_log_triggers(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    format: Output,
) -> Result<()> {
    let actions = load_actions(app_id, app_name, environment, org, Some("trigger")).await?;

    output::print_with(LogTriggersResponse { triggers: &actions }, format, |w| {
        render_log_triggers(w, &actions)
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn create_log_trigger(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    name: &str,
    query_text: &str,
    source_ids: &[String],
    description: Option<&str>,
    notifier_ids: &[String],
    severities: &[String],
    format: Output,
) -> Result<()> {
    let action = create_action(
        app_id,
        app_name,
        environment,
        org,
        "trigger",
        name,
        query_text,
        source_ids,
        &[],
        description,
        notifier_ids,
        severities,
    )
    .await?;

    crate::status!("Log trigger {} created.", action.id());
    output::print_with(LogTriggerResponse { trigger: &action }, format, |w| {
        render_log_trigger_detail(w, &action)
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn update_log_trigger(
    id: &str,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    name: Option<&str>,
    query_text: Option<&str>,
    source_ids: Option<Vec<String>>,
    description: Option<&str>,
    notifier_ids: Option<Vec<String>>,
    severities: Option<Vec<String>>,
    format: Output,
) -> Result<()> {
    let action = update_action(
        id,
        app_id,
        app_name,
        environment,
        org,
        "trigger",
        name,
        query_text,
        source_ids,
        None,
        description,
        notifier_ids,
        severities,
    )
    .await?;

    crate::status!("Log trigger {} updated.", action.id());
    output::print_with(LogTriggerResponse { trigger: &action }, format, |w| {
        render_log_trigger_detail(w, &action)
    })
}

pub async fn delete_log_trigger(
    id: &str,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    format: Output,
) -> Result<()> {
    let action = delete_action(id, app_id, app_name, environment, org, "trigger").await?;

    crate::status!("Log trigger {} deleted.", action.id());
    output::print_with(LogTriggerResponse { trigger: &action }, format, |w| {
        render_log_trigger_detail(w, &action)
    })
}

// -- Helpers --

/// Resolve a log view by name or ID. Tries ID first, then case-insensitive name match.
async fn resolve_log_view(
    client: &AppSignalClient,
    app_id: &str,
    name_or_id: &str,
) -> Result<LogView> {
    // Try by ID first
    if let Ok(view) = client.get_log_view(app_id, name_or_id).await {
        return Ok(view);
    }

    // Fall back to name match
    let views = client.list_log_views(app_id).await?;
    let name_lower = name_or_id.to_lowercase();
    let matches: Vec<&LogView> = views
        .iter()
        .filter(|v| v.name.to_lowercase() == name_lower)
        .collect();

    match matches.len() {
        0 => anyhow::bail!(
            "No log view found matching '{}'. Use `logs views` to list available views.",
            name_or_id
        ),
        1 => Ok(matches[0].clone()),
        _ => {
            let descriptions: Vec<String> = matches
                .iter()
                .map(|v| format!("  {} ({})", v.id, v.name))
                .collect();
            anyhow::bail!(
                "Multiple log views match '{}'. Use the view ID to disambiguate:\n{}",
                name_or_id,
                descriptions.join("\n")
            )
        }
    }
}

fn build_trigger_input(
    description: Option<&str>,
    notifier_ids: Option<&[String]>,
    severities: Option<&[String]>,
) -> Option<LogLineActionTriggerInput> {
    let notifier_ids = notifier_ids.map(|notifier_ids| notifier_ids.to_vec());
    let severities = severities.map(|severities| {
        severities
            .iter()
            .map(|severity| severity.trim().to_ascii_uppercase())
            .collect::<Vec<_>>()
    });

    let description = description.map(str::to_string);
    if description.is_none() && notifier_ids.is_none() && severities.is_none() {
        return None;
    }

    Some(LogLineActionTriggerInput {
        description,
        notifier_ids,
        severities,
    })
}

async fn resolve_log_actions_client(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
) -> Result<(AppSignalClient, String)> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    Ok((client, resolved_app_id))
}

async fn load_actions(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    action_type: Option<&str>,
) -> Result<Vec<LogLineAction>> {
    let (client, resolved_app_id) =
        resolve_log_actions_client(app_id, app_name, environment, org).await?;
    let mut actions = client.list_log_line_actions(&resolved_app_id).await?;
    if let Some(action_type) = action_type {
        actions.retain(|action| action.action_type().eq_ignore_ascii_case(action_type));
    }
    actions.sort_by_key(|action| action.order());
    Ok(actions)
}

#[allow(clippy::too_many_arguments)]
async fn create_action(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    action_type: &str,
    name: &str,
    query_text: &str,
    source_ids: &[String],
    metrics: &[String],
    description: Option<&str>,
    notifier_ids: &[String],
    severities: &[String],
) -> Result<LogLineAction> {
    let (client, resolved_app_id) =
        resolve_log_actions_client(app_id, app_name, environment, org).await?;
    let metric_inputs = parse_metric_specs(metrics)?;
    let trigger_input = build_trigger_input(
        description,
        (!notifier_ids.is_empty()).then_some(notifier_ids),
        (!severities.is_empty()).then_some(severities),
    );
    validate_action_payload(action_type, &metric_inputs, trigger_input.as_ref())?;

    client
        .create_log_line_action(
            &resolved_app_id,
            name,
            query_text,
            action_type,
            (!source_ids.is_empty()).then_some(source_ids),
            (!metric_inputs.is_empty()).then_some(metric_inputs.as_slice()),
            trigger_input.as_ref(),
        )
        .await
}

#[allow(clippy::too_many_arguments)]
async fn update_action(
    id: &str,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    expected_type: &str,
    name: Option<&str>,
    query_text: Option<&str>,
    source_ids: Option<Vec<String>>,
    metrics: Option<Vec<String>>,
    description: Option<&str>,
    notifier_ids: Option<Vec<String>>,
    severities: Option<Vec<String>>,
) -> Result<LogLineAction> {
    let (client, resolved_app_id) =
        resolve_log_actions_client(app_id, app_name, environment, org).await?;
    let existing = find_action_by_id(&client, &resolved_app_id, id).await?;
    ensure_action_type(&existing, expected_type)?;

    let metric_inputs = metrics.as_deref().map(parse_metric_specs).transpose()?;
    let trigger_input =
        build_trigger_input(description, notifier_ids.as_deref(), severities.as_deref());
    validate_optional_action_payload(
        expected_type,
        metric_inputs.as_deref(),
        trigger_input.as_ref(),
    )?;

    client
        .update_log_line_action(
            &resolved_app_id,
            id,
            name,
            query_text,
            source_ids.as_deref(),
            metric_inputs.as_deref(),
            trigger_input.as_ref(),
        )
        .await
}

async fn delete_action(
    id: &str,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    expected_type: &str,
) -> Result<LogLineAction> {
    let (client, resolved_app_id) =
        resolve_log_actions_client(app_id, app_name, environment, org).await?;
    let existing = find_action_by_id(&client, &resolved_app_id, id).await?;
    ensure_action_type(&existing, expected_type)?;
    client.delete_log_line_action(&resolved_app_id, id).await
}

async fn find_action_by_id(
    client: &AppSignalClient,
    app_id: &str,
    id: &str,
) -> Result<LogLineAction> {
    client
        .list_log_line_actions(app_id)
        .await?
        .into_iter()
        .find(|action| action.id() == id)
        .with_context(|| format!("No log rule found with ID {}.", id))
}

fn ensure_action_type(action: &LogLineAction, expected_type: &str) -> Result<()> {
    if action.action_type().eq_ignore_ascii_case(expected_type) {
        Ok(())
    } else {
        anyhow::bail!(
            "Action {} is a {} configuration, not a {} configuration.",
            action.id(),
            action.action_type().to_ascii_lowercase(),
            expected_type
        )
    }
}

fn validate_action_payload(
    action_type: &str,
    metrics: &[LogLineMetricInput],
    trigger: Option<&LogLineActionTriggerInput>,
) -> Result<()> {
    match action_type.trim().to_ascii_lowercase().as_str() {
        "metrics" if metrics.is_empty() => {
            anyhow::bail!("Metrics actions require at least one `--metric` definition.")
        }
        "trigger" | "filter" if !metrics.is_empty() => {
            anyhow::bail!("`--metric` is only valid with `--type metrics`.")
        }
        "filter" | "metrics" if trigger.is_some() => {
            anyhow::bail!("Trigger-only flags are only valid with `--type trigger`.")
        }
        _ => Ok(()),
    }
}

fn validate_optional_action_payload(
    action_type: &str,
    metrics: Option<&[LogLineMetricInput]>,
    trigger: Option<&LogLineActionTriggerInput>,
) -> Result<()> {
    if let Some(metrics) = metrics {
        match action_type.trim().to_ascii_lowercase().as_str() {
            "metrics" => {}
            _ => anyhow::bail!("`--metric` is only valid for log metrics commands."),
        }

        if metrics.is_empty() {
            return Ok(());
        }
    }

    if trigger.is_some() && !action_type.eq_ignore_ascii_case("trigger") {
        anyhow::bail!("Trigger-specific flags are only valid for log trigger commands.")
    } else {
        Ok(())
    }
}

fn parse_metric_specs(specs: &[String]) -> Result<Vec<LogLineMetricInput>> {
    specs.iter().map(|spec| parse_metric_spec(spec)).collect()
}

fn parse_metric_spec(spec: &str) -> Result<LogLineMetricInput> {
    let mut name = None;
    let mut field = None;
    let mut metric_type = None;
    let mut tags = serde_json::Map::new();

    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let (key, value) = part
            .split_once('=')
            .with_context(|| format!("Invalid metric component `{}` in `{}`", part, spec))?;
        let key = key.trim();
        let value = value.trim();

        if key.eq_ignore_ascii_case("name") {
            name = Some(value.to_string());
        } else if key.eq_ignore_ascii_case("field") {
            field = Some(value.to_string());
        } else if key.eq_ignore_ascii_case("type") || key.eq_ignore_ascii_case("metric_type") {
            metric_type = Some(normalize_metric_type(value)?);
        } else if let Some(tag_name) = key.strip_prefix("tag.") {
            if tag_name.trim().is_empty() {
                anyhow::bail!("Metric tag keys cannot be empty in `{}`", spec);
            }
            tags.insert(
                tag_name.trim().to_string(),
                Value::String(value.to_string()),
            );
        } else {
            anyhow::bail!(
                "Unsupported metric key `{}` in `{}`. Use name=..., type=..., field=..., and tag.<name>=...",
                key,
                spec
            );
        }
    }

    let name = name.context("Metric definitions require `name=...`")?;
    let metric_type = metric_type.context("Metric definitions require `type=...`")?;
    if matches!(metric_type.as_str(), "GAUGE" | "DISTRIBUTION") && field.is_none() {
        anyhow::bail!(
            "Metric `{}` uses type `{}` and requires `field=...`.",
            name,
            metric_type.to_ascii_lowercase()
        );
    }

    Ok(LogLineMetricInput {
        name,
        field,
        metric_type,
        tags: (!tags.is_empty()).then_some(tags),
    })
}

fn normalize_metric_type(metric_type: &str) -> Result<String> {
    match metric_type.trim().to_ascii_lowercase().as_str() {
        "counter" => Ok("COUNTER".to_string()),
        "gauge" => Ok("GAUGE".to_string()),
        "distribution" => Ok("DISTRIBUTION".to_string()),
        other => anyhow::bail!(
            "Unsupported metric type '{}'. Use counter, gauge, or distribution.",
            other
        ),
    }
}

fn print_log_line(line: &LogLine, format: Output) -> Result<()> {
    let stdout = io::stdout();
    let mut w = stdout.lock();
    match format {
        Output::Human => render_log_line_human(&mut w, line)?,
        Output::Json => output::json_line(&mut w, line)?,
    }
    Ok(())
}

fn render_log_line_human(w: &mut dyn Write, line: &LogLine) -> io::Result<()> {
    let source_name = line
        .source
        .as_ref()
        .and_then(|s| s.name.as_deref())
        .unwrap_or("-");

    let attrs = line
        .attributes
        .as_ref()
        .map(|attrs| {
            if attrs.is_empty() {
                String::new()
            } else {
                let kv: Vec<String> = attrs
                    .iter()
                    .map(|a| format!("{}={}", a.key, a.value.as_deref().unwrap_or("")))
                    .collect();
                format!(" [{}]", kv.join(" "))
            }
        })
        .unwrap_or_default();

    writeln!(
        w,
        "{} {:<8} {:<16} {:<24} {}{}",
        &line.timestamp,
        line.severity.to_uppercase(),
        source_name,
        line.hostname,
        line.message,
        attrs,
    )
}

fn render_log_views(w: &mut dyn Write, views: &[LogView]) -> io::Result<()> {
    if views.is_empty() {
        return writeln!(w, "No log views found.");
    }

    writeln!(w, "{:<28} {:<40} {:<20} SEVERITIES", "ID", "NAME", "QUERY")?;
    writeln!(w, "{}", "-".repeat(100))?;

    for v in views {
        let query_str = v.query.as_deref().unwrap_or("-");
        let sevs = v
            .severities
            .as_ref()
            .map(|s| s.join(","))
            .unwrap_or_else(|| "-".to_string());
        writeln!(
            w,
            "{:<28} {:<40} {:<20} {}",
            truncate(&v.id, 26),
            truncate(&v.name, 38),
            truncate(query_str, 18),
            sevs,
        )?;
    }

    writeln!(w, "{} log view(s) found.", views.len())
}

fn render_log_sources(w: &mut dyn Write, sources: &[crate::api::LogSource]) -> io::Result<()> {
    if sources.is_empty() {
        return writeln!(w, "No log sources found.");
    }

    writeln!(w, "{:<28} {:<30} {:<12} FORMAT", "ID", "NAME", "TYPE")?;
    writeln!(w, "{}", "-".repeat(80))?;

    for s in sources {
        writeln!(
            w,
            "{:<28} {:<30} {:<12} {}",
            truncate(&s.id, 26),
            truncate(&s.name, 28),
            s.kind.as_deref().unwrap_or("-"),
            s.fmt.as_deref().unwrap_or("-"),
        )?;
    }

    writeln!(w, "{} log source(s) found.", sources.len())
}

fn render_log_metrics(w: &mut dyn Write, actions: &[LogLineAction]) -> io::Result<()> {
    if actions.is_empty() {
        return writeln!(w, "No log metrics found.");
    }

    writeln!(
        w,
        "{:<28} {:<26} {:<24} {:<18} ID",
        "NAME", "METRICS", "SOURCES", "QUERY"
    )?;
    writeln!(w, "{}", "-".repeat(120))?;

    for action in actions {
        writeln!(
            w,
            "{:<28} {:<26} {:<24} {:<18} {}",
            truncate(action.name(), 26),
            truncate(&render_metric_names(action), 24),
            truncate(&render_action_sources(action), 22),
            truncate(action.query(), 16),
            truncate(action.id(), 26),
        )?;
    }

    writeln!(w, "{} log metric configuration(s) found.", actions.len())
}

fn render_log_metric_detail(w: &mut dyn Write, action: &LogLineAction) -> io::Result<()> {
    let sources = render_action_sources(action);

    output::detail(
        w,
        &[
            ("ID", action.id()),
            ("Name", action.name()),
            ("Query", action.query()),
            ("Sources", &sources),
        ],
    )?;
    render_metric_definitions(w, action)
}

fn render_log_triggers(w: &mut dyn Write, actions: &[LogLineAction]) -> io::Result<()> {
    if actions.is_empty() {
        return writeln!(w, "No log triggers found.");
    }

    writeln!(
        w,
        "{:<28} {:<24} {:<18} {:<18} ID",
        "NAME", "SEVERITIES", "NOTIFIERS", "QUERY"
    )?;
    writeln!(w, "{}", "-".repeat(120))?;

    for action in actions {
        writeln!(
            w,
            "{:<28} {:<24} {:<18} {:<18} {}",
            truncate(action.name(), 26),
            truncate(&render_trigger_severities(action), 22),
            truncate(&render_trigger_notifier_names(action), 16),
            truncate(action.query(), 16),
            truncate(action.id(), 26),
        )?;
    }

    writeln!(w, "{} log trigger(s) found.", actions.len())
}

fn render_log_trigger_detail(w: &mut dyn Write, action: &LogLineAction) -> io::Result<()> {
    let sources = render_action_sources(action);
    let severities = render_trigger_severities(action);
    let notifiers = render_trigger_notifier_names(action);

    output::detail(
        w,
        &[
            ("ID", action.id()),
            ("Name", action.name()),
            ("Query", action.query()),
            ("Sources", &sources),
            ("Severities", &severities),
            ("Notifiers", &notifiers),
        ],
    )?;

    if let Some(description) = action.trigger_description() {
        writeln!(w, "Description: {}", description)?;
    }

    Ok(())
}

fn render_action_sources(action: &LogLineAction) -> String {
    let source_names = action
        .sources()
        .iter()
        .map(|source| source.name.as_str())
        .collect::<Vec<_>>();

    if !source_names.is_empty() {
        source_names.join(",")
    } else if !action.source_ids().is_empty() {
        action.source_ids().join(",")
    } else {
        "all".to_string()
    }
}

fn render_metric_names(action: &LogLineAction) -> String {
    let names = action
        .metrics()
        .iter()
        .map(|metric| metric.name.clone())
        .collect::<Vec<_>>();

    if names.is_empty() {
        "-".to_string()
    } else {
        names.join(",")
    }
}

fn render_metric_definitions(w: &mut dyn Write, action: &LogLineAction) -> io::Result<()> {
    let metrics = action.metrics();
    if metrics.is_empty() {
        return Ok(());
    }

    writeln!(w, "Metrics:")?;
    for metric in metrics {
        let mut details = vec![format!(
            "{} ({})",
            metric.name,
            metric.metric_type.to_ascii_lowercase()
        )];
        if let Some(field) = &metric.field {
            details.push(format!("field={}", field));
        }
        if !metric.tags.is_empty() {
            let tags = metric
                .tags
                .iter()
                .map(|(key, value)| match value {
                    Value::String(value) => format!("{}={}", key, value),
                    _ => format!("{}={}", key, value),
                })
                .collect::<Vec<_>>()
                .join(", ");
            details.push(format!("tags: {}", tags));
        }
        writeln!(w, "  - {}", details.join(" | "))?;
    }

    Ok(())
}

fn render_trigger_severities(action: &LogLineAction) -> String {
    let severities = action.trigger_severities();
    if severities.is_empty() {
        "all".to_string()
    } else {
        severities.join(",")
    }
}

fn render_trigger_notifier_names(action: &LogLineAction) -> String {
    let names = action
        .trigger_notifiers()
        .iter()
        .map(|notifier| notifier.name.clone().unwrap_or_else(|| notifier.id.clone()))
        .collect::<Vec<_>>();

    if names.is_empty() {
        "-".to_string()
    } else {
        names.join(",")
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
    use crate::api::{AppSignalClient, KeyStringValue, LogLineAction, LogSource, LogSourceRef};
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_print_log_line_basic() {
        // Just ensure it doesn't panic
        let line = LogLine {
            id: "abc".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            severity: "error".to_string(),
            hostname: "web-1".to_string(),
            group: None,
            message: "Something went wrong".to_string(),
            attributes: Some(vec![KeyStringValue {
                key: "request_id".to_string(),
                value: Some("123".to_string()),
            }]),
            source: Some(LogSourceRef {
                id: "s1".to_string(),
                name: Some("Application".to_string()),
            }),
        };
        print_log_line(&line, Output::Human).unwrap();
    }

    #[test]
    fn test_print_log_line_no_source() {
        let line = LogLine {
            id: "def".to_string(),
            timestamp: "2025-06-15T12:30:00Z".to_string(),
            severity: "info".to_string(),
            hostname: "worker-3".to_string(),
            group: Some("background".to_string()),
            message: "Job completed".to_string(),
            attributes: None,
            source: None,
        };
        print_log_line(&line, Output::Human).unwrap();
    }

    #[test]
    fn test_print_log_line_empty_attributes() {
        let line = LogLine {
            id: "ghi".to_string(),
            timestamp: "2025-06-15T12:30:00Z".to_string(),
            severity: "warn".to_string(),
            hostname: "api-1".to_string(),
            group: None,
            message: "Rate limited".to_string(),
            attributes: Some(vec![]),
            source: Some(LogSourceRef {
                id: "s2".to_string(),
                name: None,
            }),
        };
        print_log_line(&line, Output::Human).unwrap();
    }

    fn graphql_log_sources_response(sources: Vec<serde_json::Value>) -> serde_json::Value {
        json!({
            "data": {
                "app": {
                    "logs": {
                        "sources": sources
                    }
                }
            }
        })
    }

    fn log_source_json(id: &str, name: &str) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "type": "custom",
            "fmt": "json"
        })
    }

    fn rest_log_line_json(
        id: &str,
        timestamp: &str,
        severity: &str,
        message: &str,
    ) -> serde_json::Value {
        json!({
            "id": id,
            "timestamp": timestamp,
            "severity": severity,
            "hostname": "web-1",
            "group": null,
            "message": message,
            "attributes": {},
            "source_id": "src-1"
        })
    }

    #[test]
    fn test_merge_rest_log_query_adds_severities() {
        let query = merge_rest_log_query(
            Some("message:timeout"),
            Some(&["ERROR".to_string(), "WARN".to_string()]),
        );
        assert_eq!(query, "message:timeout severity=[error,warn]");
    }

    #[test]
    fn test_merge_rest_log_query_handles_missing_query() {
        let query = merge_rest_log_query(None, Some(&["ERROR".to_string()]));
        assert_eq!(query, "severity=[error]");
    }

    #[test]
    fn test_parse_metric_spec_counter() {
        let metric = parse_metric_spec("name=log.error_count,type=counter").unwrap();

        assert_eq!(metric.name, "log.error_count");
        assert_eq!(metric.metric_type, "COUNTER");
        assert!(metric.field.is_none());
    }

    #[test]
    fn test_parse_metric_spec_distribution_with_tags() {
        let metric = parse_metric_spec(
            "name=log.request_duration,type=distribution,field=duration_ms,tag.hostname=web-1",
        )
        .unwrap();

        assert_eq!(metric.metric_type, "DISTRIBUTION");
        assert_eq!(metric.field.as_deref(), Some("duration_ms"));
        assert_eq!(
            metric
                .tags
                .as_ref()
                .and_then(|tags| tags.get("hostname"))
                .and_then(Value::as_str),
            Some("web-1")
        );
    }

    #[test]
    fn test_parse_metric_spec_requires_field_for_distribution() {
        let err = parse_metric_spec("name=log.request_duration,type=distribution").unwrap_err();
        assert!(err.to_string().contains("requires `field=...`"));
    }

    #[test]
    fn test_build_trigger_input_keeps_explicit_empty_lists_for_clear_operations() {
        let trigger = build_trigger_input(Some("desc"), Some(&[]), Some(&[])).unwrap();

        assert_eq!(trigger.description.as_deref(), Some("desc"));
        assert_eq!(trigger.notifier_ids, Some(vec![]));
        assert_eq!(trigger.severities, Some(vec![]));
    }

    #[test]
    fn test_render_log_metrics() {
        let actions = vec![LogLineAction::LogLineActionMetrics {
            id: "action-2".to_string(),
            name: "Track error count".to_string(),
            query: "severity:error".to_string(),
            source_ids: vec![],
            action_type: "METRICS".to_string(),
            sources: vec![],
            log_line_metrics: vec![],
            order: 1,
            user: None,
        }];

        let mut buf = Vec::new();
        render_log_metrics(&mut buf, &actions).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("METRICS"));
        assert!(output.contains("Track error count"));
        assert!(output.contains("1 log metric configuration(s) found."));
    }

    #[test]
    fn test_render_log_triggers() {
        let actions = vec![LogLineAction::LogLineActionTrigger {
            id: "trigger-1".to_string(),
            name: "Root login".to_string(),
            description: Some("Alert on root logins".to_string()),
            query: "message:root".to_string(),
            source_ids: vec!["src-1".to_string()],
            severities: vec!["ERROR".to_string()],
            action_type: "TRIGGER".to_string(),
            sources: vec![LogSource {
                id: "src-1".to_string(),
                name: "auth".to_string(),
                kind: Some("custom".to_string()),
                fmt: Some("json".to_string()),
            }],
            order: 0,
            user: None,
            previous_trigger: None,
            notification_options: None,
            notification_trigger_value: None,
            notifiers: None,
        }];

        let mut buf = Vec::new();
        render_log_triggers(&mut buf, &actions).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("SEVERITIES"));
        assert!(output.contains("Root login"));
        assert!(output.contains("1 log trigger(s) found."));
    }

    // -- fetch_all_pages tests --

    #[tokio::test]
    async fn test_fetch_all_pages_single_page() {
        // When results < 100, should return in one page
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_log_sources_response(vec![
                    log_source_json("src-1", "Application"),
                ])),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v2/logs/lines"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![
                rest_log_line_json("l1", "2025-06-01T12:00:00Z", "INFO", "msg1"),
                rest_log_line_json("l2", "2025-06-01T12:01:00Z", "ERROR", "msg2"),
            ]))
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let (source_ids, source_names) =
            resolve_search_sources(&client, "app1", None).await.unwrap();
        let lines = fetch_all_pages(
            &client,
            "app1",
            Some("2025-06-01T00:00:00Z"),
            Some("2025-06-02T00:00:00Z"),
            &source_ids,
            "",
            &source_names,
        )
        .await
        .unwrap();

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].id, "l1");
        assert_eq!(lines[1].id, "l2");
    }

    #[tokio::test]
    async fn test_fetch_all_pages_empty() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_log_sources_response(vec![
                    log_source_json("src-1", "Application"),
                ])),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v2/logs/lines"))
            .respond_with(ResponseTemplate::new(200).set_body_json(Vec::<serde_json::Value>::new()))
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let (source_ids, source_names) =
            resolve_search_sources(&client, "app1", None).await.unwrap();
        let lines = fetch_all_pages(&client, "app1", None, None, &source_ids, "", &source_names)
            .await
            .unwrap();

        assert!(lines.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_all_pages_multi_page() {
        // Simulate pagination: first call returns 100 lines, second returns 30
        let call_count = std::sync::Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_log_sources_response(vec![
                    log_source_json("src-1", "Application"),
                ])),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v2/logs/lines"))
            .respond_with(move |_req: &Request| {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    // First page: exactly 100 lines (triggers pagination)
                    let lines: Vec<serde_json::Value> = (0..100)
                        .map(|i| {
                            rest_log_line_json(
                                &format!("page1-{}", i),
                                &format!("2025-06-01T12:{:02}:00Z", i % 60),
                                "INFO",
                                &format!("message {}", i),
                            )
                        })
                        .collect();
                    ResponseTemplate::new(200).set_body_json(lines)
                } else {
                    // Second page: fewer than 100 (terminates pagination)
                    let lines: Vec<serde_json::Value> = (0..30)
                        .map(|i| {
                            rest_log_line_json(
                                &format!("page2-{}", i),
                                &format!("2025-06-01T13:{:02}:00Z", i % 60),
                                "ERROR",
                                &format!("page2 message {}", i),
                            )
                        })
                        .collect();
                    ResponseTemplate::new(200).set_body_json(lines)
                }
            })
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let (source_ids, source_names) =
            resolve_search_sources(&client, "app1", None).await.unwrap();
        let lines = fetch_all_pages(
            &client,
            "app1",
            Some("2025-06-01T00:00:00Z"),
            Some("2025-06-02T00:00:00Z"),
            &source_ids,
            "",
            &source_names,
        )
        .await
        .unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 2);
        assert_eq!(lines.len(), 130); // 100 + 30
        assert_eq!(lines[0].id, "page1-0");
        assert_eq!(lines[99].id, "page1-99");
        assert_eq!(lines[100].id, "page2-0");
    }

    #[tokio::test]
    async fn test_fetch_all_pages_deduplicates() {
        // Second page includes some IDs from first page (boundary overlap)
        let call_count = std::sync::Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_log_sources_response(vec![
                    log_source_json("src-1", "Application"),
                ])),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v2/logs/lines"))
            .respond_with(move |_req: &Request| {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    // First page: exactly 100 lines
                    let lines: Vec<serde_json::Value> = (0..100)
                        .map(|i| {
                            rest_log_line_json(
                                &format!("line-{}", i),
                                &format!("2025-06-01T12:{:02}:00Z", i % 60),
                                "INFO",
                                &format!("msg {}", i),
                            )
                        })
                        .collect();
                    ResponseTemplate::new(200).set_body_json(lines)
                } else {
                    // Second page: 5 duplicates from page 1 + 15 new lines = 20 total
                    let mut lines: Vec<serde_json::Value> = (95..100)
                        .map(|i| {
                            rest_log_line_json(
                                &format!("line-{}", i), // duplicate IDs
                                &format!("2025-06-01T12:{:02}:00Z", i % 60),
                                "INFO",
                                &format!("msg {}", i),
                            )
                        })
                        .collect();
                    for i in 0..15 {
                        lines.push(rest_log_line_json(
                            &format!("new-{}", i),
                            &format!("2025-06-01T13:{:02}:00Z", i % 60),
                            "WARN",
                            &format!("new msg {}", i),
                        ));
                    }
                    ResponseTemplate::new(200).set_body_json(lines)
                }
            })
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let (source_ids, source_names) =
            resolve_search_sources(&client, "app1", None).await.unwrap();
        let lines = fetch_all_pages(&client, "app1", None, None, &source_ids, "", &source_names)
            .await
            .unwrap();

        // 100 from page 1 + 15 new from page 2 (5 dupes removed)
        assert_eq!(lines.len(), 115);
        // Verify no duplicate IDs
        let ids: HashSet<String> = lines.iter().map(|l| l.id.clone()).collect();
        assert_eq!(ids.len(), 115);
    }
}
