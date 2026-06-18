//! The `performance` command: rank an app's performance incidents and surface
//! the slow database work behind the worst offenders.
//!
//! Both subcommands build on data the public GraphQL API already exposes:
//! `app.performanceIncidents` is **action-level** — it reports each slow action's
//! mean/total duration and throughput, but not per-SQL timing. `performance
//! actions` ranks those actions directly. `performance queries` goes one step
//! further by pulling the latest *sample* for each of the slowest actions and
//! distilling the slow queries and N+1 suspects out of its timeline (see
//! `sample_analysis`). That per-query view is therefore derived from the latest
//! sampled request for each action, not a full aggregate across every request —
//! the output says so explicitly.

use std::io::{self, Write};

use anyhow::Result;
use clap::ValueEnum;
use serde::Serialize;

use super::{authenticated_client, resolve_org};
use crate::api::{AppSignalClient, Incident, SampleQuery};
use crate::config::Config;
use crate::output::{self, Output};
use crate::sample_analysis::{self, NPlusOneSuspect, SlowQuery};

/// Pool of recent performance incidents `queries` ranks before drilling.
const QUERY_SCAN_POOL: i64 = 50;

/// Metric used to rank performance actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ValueEnum)]
pub enum PerfSort {
    /// Mean request duration (slowest typical request).
    Mean,
    /// Total request duration across occurrences (biggest overall cost).
    Total,
    /// Throughput — number of occurrences.
    Count,
}

impl PerfSort {
    fn label(self) -> &'static str {
        match self {
            PerfSort::Mean => "mean duration",
            PerfSort::Total => "total duration",
            PerfSort::Count => "throughput",
        }
    }

    fn key(self, incident: &Incident) -> f64 {
        match self {
            PerfSort::Mean => incident.mean().unwrap_or(0.0),
            PerfSort::Total => incident.total_duration().unwrap_or(0.0),
            PerfSort::Count => incident.count() as f64,
        }
    }
}

/// `performance actions` — rank recent performance incidents by a chosen metric.
#[allow(clippy::too_many_arguments)]
pub async fn actions(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    namespaces: &[String],
    action: Option<&str>,
    state: Option<&str>,
    sort: PerfSort,
    limit: i64,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let mut incidents = client
        .list_performance_incidents(
            &resolved_app_id,
            Some(limit.max(1)),
            None,
            state,
            Some("LAST"),
            namespaces_opt(namespaces).as_deref(),
            action,
            None,
        )
        .await?;

    sort_incidents(&mut incidents, sort);

    let rows: Vec<ActionRow> = incidents.iter().map(ActionRow::from_incident).collect();
    output::print_with(
        ActionsResponse {
            sort,
            scanned: rows.len(),
            actions: &rows,
        },
        format,
        |w| render_actions(w, sort, &rows),
    )
}

/// `performance queries` — slow queries and N+1 suspects behind the slowest actions.
#[allow(clippy::too_many_arguments)]
pub async fn queries(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    namespaces: &[String],
    action: Option<&str>,
    state: Option<&str>,
    limit: i64,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let mut incidents = client
        .list_performance_incidents(
            &resolved_app_id,
            Some(QUERY_SCAN_POOL),
            None,
            state,
            Some("LAST"),
            namespaces_opt(namespaces).as_deref(),
            action,
            None,
        )
        .await?;

    sort_incidents(&mut incidents, PerfSort::Mean);
    let scanned = incidents.len();
    let top = limit.max(1) as usize;

    let mut drilled = Vec::new();
    for incident in incidents.into_iter().take(top) {
        drilled.push(drill_incident(&client, &resolved_app_id, &incident).await);
    }

    output::print_with(
        QueriesResponse {
            source: "latest-sample-per-action",
            scanned,
            drilled_into: drilled.len(),
            actions: &drilled,
        },
        format,
        |w| render_queries(w, &drilled),
    )
}

/// Fetch the latest sample for one performance incident and distil its slow work.
async fn drill_incident(
    client: &AppSignalClient,
    app_id: &str,
    incident: &Incident,
) -> DrilledAction {
    let action = first_action(incident);
    let namespace = incident.namespace().map(str::to_string);
    let mean_ms = incident.mean();

    match client
        .get_incident_sample(app_id, incident.number(), SampleQuery::Latest)
        .await
    {
        Ok(found) => {
            let analysis =
                sample_analysis::analyze(&found.sample, &found.sample_type, found.incident_number);
            let (slow_queries, n_plus_one) = match analysis.performance {
                Some(perf) => (perf.slow_queries, perf.n_plus_one_suspects),
                None => (Vec::new(), Vec::new()),
            };
            DrilledAction {
                number: incident.number(),
                action,
                namespace,
                mean_ms,
                sample_id: Some(found.sample.id),
                note: None,
                slow_queries,
                n_plus_one: n_plus_one.into_iter().map(NPlusOneRow::from).collect(),
            }
        }
        Err(err) => DrilledAction {
            number: incident.number(),
            action,
            namespace,
            mean_ms,
            sample_id: None,
            note: Some(format!("no sample available ({})", err)),
            slow_queries: Vec::new(),
            n_plus_one: Vec::new(),
        },
    }
}

fn sort_incidents(incidents: &mut [Incident], sort: PerfSort) {
    incidents.sort_by(|a, b| {
        sort.key(b)
            .partial_cmp(&sort.key(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn namespaces_opt(namespaces: &[String]) -> Option<Vec<String>> {
    if namespaces.is_empty() {
        None
    } else {
        Some(namespaces.to_vec())
    }
}

fn first_action(incident: &Incident) -> Option<String> {
    incident.action_names().first().cloned()
}

#[derive(Serialize)]
struct ActionsResponse<'a> {
    sort: PerfSort,
    scanned: usize,
    actions: &'a [ActionRow],
}

#[derive(Serialize)]
struct ActionRow {
    number: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mean_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_ms: Option<f64>,
    count: i64,
    severity: String,
    state: String,
    last_occurred_at: String,
}

impl ActionRow {
    fn from_incident(incident: &Incident) -> Self {
        ActionRow {
            number: incident.number(),
            action: first_action(incident),
            namespace: incident.namespace().map(str::to_string),
            mean_ms: incident.mean(),
            total_ms: incident.total_duration(),
            count: incident.count(),
            severity: incident.severity().to_string(),
            state: incident.state().to_string(),
            last_occurred_at: incident.last_occurred_at().to_string(),
        }
    }
}

#[derive(Serialize)]
struct QueriesResponse<'a> {
    /// Provenance marker: this view is distilled from the latest sampled request
    /// per action, not a full aggregate. Mirrors the caveat in the human output.
    source: &'static str,
    scanned: usize,
    drilled_into: usize,
    actions: &'a [DrilledAction],
}

#[derive(Serialize)]
struct DrilledAction {
    number: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mean_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    slow_queries: Vec<SlowQuery>,
    n_plus_one: Vec<NPlusOneRow>,
}

#[derive(Serialize)]
struct NPlusOneRow {
    digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<String>,
    count: i64,
    total_ms: f64,
}

impl From<NPlusOneSuspect> for NPlusOneRow {
    fn from(suspect: NPlusOneSuspect) -> Self {
        NPlusOneRow {
            digest: suspect.digest,
            group: suspect.group,
            count: suspect.count,
            total_ms: suspect.total_ms,
        }
    }
}

fn render_actions(w: &mut dyn Write, sort: PerfSort, rows: &[ActionRow]) -> io::Result<()> {
    if rows.is_empty() {
        return writeln!(w, "No performance incidents found.");
    }

    writeln!(
        w,
        "Performance actions ranked by {} ({} scanned)",
        sort.label(),
        rows.len()
    )?;
    writeln!(
        w,
        "{:<4} {:<40} {:<10} {:>10} {:>12} {:>10} {:<8}",
        "#", "ACTION", "NAMESPACE", "MEAN(ms)", "TOTAL(ms)", "COUNT", "STATE"
    )?;
    writeln!(w, "{}", "-".repeat(98))?;
    for (index, row) in rows.iter().enumerate() {
        writeln!(
            w,
            "{:<4} {:<40} {:<10} {:>10} {:>12} {:>10} {:<8}",
            index + 1,
            truncate(row.action.as_deref().unwrap_or("-"), 40),
            truncate(row.namespace.as_deref().unwrap_or("-"), 10),
            format_number(row.mean_ms),
            format_number(row.total_ms),
            row.count,
            row.state,
        )?;
    }
    Ok(())
}

fn render_queries(w: &mut dyn Write, actions: &[DrilledAction]) -> io::Result<()> {
    if actions.is_empty() {
        return writeln!(w, "No performance incidents found.");
    }

    writeln!(
        w,
        "Slow queries across the {} slowest performance action(s)",
        actions.len()
    )?;
    writeln!(
        w,
        "(derived from the latest sampled request per action — not a full aggregate)\n"
    )?;

    for (index, action) in actions.iter().enumerate() {
        let mean = action
            .mean_ms
            .map(|m| format!("  mean={:.1}ms", m))
            .unwrap_or_default();
        writeln!(
            w,
            "#{} {}  [namespace={}, incident #{}]{}",
            index + 1,
            action.action.as_deref().unwrap_or("-"),
            action.namespace.as_deref().unwrap_or("-"),
            action.number,
            mean,
        )?;

        if let Some(note) = &action.note {
            writeln!(w, "    {}", note)?;
            writeln!(w)?;
            continue;
        }

        if action.n_plus_one.is_empty() && action.slow_queries.is_empty() {
            writeln!(
                w,
                "    No slow queries or N+1 patterns in the latest sample."
            )?;
            writeln!(w)?;
            continue;
        }

        if !action.n_plus_one.is_empty() {
            writeln!(w, "    N+1 suspects:")?;
            for suspect in &action.n_plus_one {
                writeln!(
                    w,
                    "      {:>4}× [{}] {} (total {:.1}ms)",
                    suspect.count,
                    short_digest(&suspect.digest),
                    suspect.group.as_deref().unwrap_or("-"),
                    suspect.total_ms,
                )?;
            }
        }

        if !action.slow_queries.is_empty() {
            writeln!(w, "    Slow queries:")?;
            for query in &action.slow_queries {
                writeln!(
                    w,
                    "      {:>9.1}ms [{}] {}",
                    query.duration_ms,
                    query.group.as_deref().unwrap_or("-"),
                    truncate(&one_line(&query.body), 90),
                )?;
            }
        }
        writeln!(w)?;
    }
    Ok(())
}

fn format_number(value: Option<f64>) -> String {
    value
        .map(|v| format!("{:.1}", v))
        .unwrap_or_else(|| "-".to_string())
}

fn short_digest(digest: &str) -> String {
    digest.chars().take(8).collect()
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let kept: String = value.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::User;

    fn perf_incident(number: i64, action: &str, mean: f64, total: f64, count: i64) -> Incident {
        Incident::PerformanceIncident {
            id: format!("perf-{number}"),
            number,
            state: Some("OPEN".to_string()),
            severity: None,
            description: None,
            count,
            created_at: None,
            last_occurred_at: Some("2026-05-19T14:00:00Z".to_string()),
            updated_at: None,
            action_names: Some(vec![action.to_string()]),
            namespace: Some("web".to_string()),
            mean: Some(mean),
            total_duration: Some(total),
            assignees: None::<Vec<User>>,
        }
    }

    #[test]
    fn sort_incidents_orders_by_mean_descending() {
        let mut incidents = [
            perf_incident(1, "GET /a", 100.0, 1000.0, 10),
            perf_incident(2, "GET /b", 300.0, 600.0, 2),
            perf_incident(3, "GET /c", 200.0, 9000.0, 45),
        ];
        sort_incidents(&mut incidents, PerfSort::Mean);
        let order: Vec<i64> = incidents.iter().map(Incident::number).collect();
        assert_eq!(order, vec![2, 3, 1]);
    }

    #[test]
    fn sort_incidents_orders_by_total_descending() {
        let mut incidents = [
            perf_incident(1, "GET /a", 100.0, 1000.0, 10),
            perf_incident(2, "GET /b", 300.0, 600.0, 2),
            perf_incident(3, "GET /c", 200.0, 9000.0, 45),
        ];
        sort_incidents(&mut incidents, PerfSort::Total);
        let order: Vec<i64> = incidents.iter().map(Incident::number).collect();
        assert_eq!(order, vec![3, 1, 2]);
    }

    #[test]
    fn render_actions_lists_ranked_rows() {
        let incidents = [
            perf_incident(2, "GET /slow", 300.0, 600.0, 2),
            perf_incident(1, "GET /fast", 100.0, 1000.0, 10),
        ];
        let rows: Vec<ActionRow> = incidents.iter().map(ActionRow::from_incident).collect();
        let mut buf = Vec::new();
        render_actions(&mut buf, PerfSort::Mean, &rows).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("ranked by mean duration"));
        assert!(out.contains("GET /slow"));
        assert!(out.contains("300.0"));
    }

    #[test]
    fn render_queries_includes_caveat_and_slow_query() {
        let actions = [DrilledAction {
            number: 7,
            action: Some("GET /reports".to_string()),
            namespace: Some("web".to_string()),
            mean_ms: Some(1234.5),
            sample_id: Some("sample-1".to_string()),
            note: None,
            slow_queries: vec![SlowQuery {
                body: "SELECT  *  FROM   reports".to_string(),
                group: Some("sql".to_string()),
                duration_ms: 812.0,
            }],
            n_plus_one: vec![NPlusOneRow {
                digest: "abcdef1234567890".to_string(),
                group: Some("sql".to_string()),
                count: 42,
                total_ms: 700.0,
            }],
        }];
        let mut buf = Vec::new();
        render_queries(&mut buf, &actions).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("not a full aggregate"));
        assert!(out.contains("GET /reports"));
        assert!(out.contains("SELECT * FROM reports"));
        assert!(out.contains("42×"));
        assert!(out.contains("abcdef12"));
    }

    #[test]
    fn queries_response_json_states_sample_provenance() {
        let drilled: Vec<DrilledAction> = Vec::new();
        let json = serde_json::to_value(QueriesResponse {
            source: "latest-sample-per-action",
            scanned: 3,
            drilled_into: 0,
            actions: &drilled,
        })
        .unwrap();
        // The JSON output must state the same provenance as the human output.
        assert_eq!(json["source"], "latest-sample-per-action");
    }

    #[test]
    fn render_queries_notes_missing_sample() {
        let actions = [DrilledAction {
            number: 9,
            action: Some("GET /gone".to_string()),
            namespace: Some("web".to_string()),
            mean_ms: None,
            sample_id: None,
            note: Some("no sample available (boom)".to_string()),
            slow_queries: Vec::new(),
            n_plus_one: Vec::new(),
        }];
        let mut buf = Vec::new();
        render_queries(&mut buf, &actions).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("no sample available"));
    }
}
