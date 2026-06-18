//! The `metrics` command: discover metric keys and pull raw timeseries.
//!
//! AppSignal exposes metrics over GraphQL (`app.metrics.keys` /
//! `app.metrics.timeseries`) and historical per-action throughput via the
//! `timeDetective*DataPoints` fields — no REST endpoint is needed. This command
//! surfaces all three: `metrics list` (key discovery), `metrics timeseries`
//! (raw values over time), and `metrics history` (error/performance datapoints
//! over an arbitrary window).

use std::io::{self, Write};

use anyhow::Result;

use super::{authenticated_client, resolve_org};
use crate::api::{
    ErrorDataPoint, MetricFieldInput, MetricKey, MetricTagInput, MetricTimeseries,
    MetricTimeseriesInput, PerformanceDataPoint, TimeDetective,
};
use crate::config::Config;
use crate::error::CliError;
use crate::output::{self, Output};

/// `metrics list` — discover metric keys.
pub async fn list(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    name: Option<&str>,
    limit: Option<i64>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let keys = client
        .list_metric_keys(&resolved_app_id, name, limit)
        .await?;

    output::print_with(MetricKeysResponse { keys: &keys }, format, |w| {
        render_metric_keys(w, &keys)
    })
}

/// `metrics timeseries` — fetch a metric's values over time.
#[allow(clippy::too_many_arguments)]
pub async fn timeseries(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    metric: &str,
    fields: &[String],
    tags: &[String],
    timeframe: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    format: Output,
) -> Result<()> {
    if timeframe.is_none() && (start.is_none() || end.is_none()) {
        anyhow::bail!(CliError::msg(
            "Provide a window: either --timeframe (e.g. R1H) or both --start and --end.",
        ));
    }

    let query = vec![MetricTimeseriesInput {
        name: metric.to_string(),
        fields: fields
            .iter()
            .map(|field| MetricFieldInput {
                field: field.clone(),
            })
            .collect(),
        tags: parse_tags(tags)?,
    }];

    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let series = client
        .fetch_metric_timeseries(&resolved_app_id, &query, timeframe, start, end)
        .await?;

    output::print_with(&series, format, |w| render_timeseries(w, metric, &series))
}

/// `metrics history` — error and performance datapoints over a window.
#[allow(clippy::too_many_arguments)]
pub async fn history(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    start: &str,
    end: &str,
    namespaces: &[String],
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let detective = client
        .fetch_time_detective(&resolved_app_id, start, end, namespaces)
        .await?;

    output::print_with(&detective, format, |w| render_history(w, &detective))
}

#[derive(serde::Serialize)]
struct MetricKeysResponse<'a> {
    keys: &'a [MetricKey],
}

/// Parse `key=value` tag filters.
fn parse_tags(tags: &[String]) -> Result<Vec<MetricTagInput>> {
    tags.iter()
        .map(|tag| {
            tag.split_once('=')
                .map(|(key, value)| MetricTagInput {
                    key: key.trim().to_string(),
                    value: value.trim().to_string(),
                })
                .ok_or_else(|| {
                    CliError::msg(format!("Invalid --tag '{}'. Expected key=value.", tag)).into()
                })
        })
        .collect()
}

fn render_metric_keys(w: &mut dyn Write, keys: &[MetricKey]) -> io::Result<()> {
    if keys.is_empty() {
        return writeln!(w, "No metrics found.");
    }

    writeln!(w, "{:<40} {:<14} FIELDS", "NAME", "TYPE")?;
    writeln!(w, "{}", "-".repeat(90))?;
    for key in keys {
        let kind = key.kind.as_deref().unwrap_or("-");
        let fields = key
            .fields
            .as_ref()
            .map(|f| f.join(", "))
            .unwrap_or_else(|| "-".to_string());
        writeln!(w, "{:<40} {:<14} {}", truncate(&key.name, 40), kind, fields)?;
        if let Some(tags) = &key.tags {
            if !tags.is_empty() {
                writeln!(w, "    tags: {}", format_tags(tags))?;
            }
        }
    }
    writeln!(w, "{} metric(s) found.", keys.len())
}

fn render_timeseries(w: &mut dyn Write, metric: &str, series: &MetricTimeseries) -> io::Result<()> {
    writeln!(w, "Metric: {}", metric)?;
    if let (Some(start), Some(end)) = (&series.start, &series.end) {
        writeln!(w, "Window: {} → {}", start, end)?;
    }
    if let Some(resolution) = &series.resolution {
        writeln!(w, "Resolution: {}", resolution)?;
    }

    if !series.keys.is_empty() {
        writeln!(w, "Series:")?;
        for key in &series.keys {
            let tags = key
                .tags
                .as_ref()
                .filter(|t| !t.is_empty())
                .map(|t| format!(" [{}]", format_tags(t)))
                .unwrap_or_default();
            writeln!(w, "  {}{}", key.name.as_deref().unwrap_or("-"), tags)?;
        }
    }

    if series.points.is_empty() {
        return writeln!(w, "No datapoints in this window.");
    }

    writeln!(w, "\n{:<26} VALUES", "TIMESTAMP")?;
    writeln!(w, "{}", "-".repeat(70))?;
    for point in &series.points {
        let timestamp = point.timestamp.as_deref().unwrap_or("-");
        let values = point
            .values
            .iter()
            .map(|kv| format!("{}={}", kv.key, kv.value.as_deref().unwrap_or("-")))
            .collect::<Vec<_>>()
            .join("  ");
        writeln!(w, "{:<26} {}", timestamp, values)?;
    }
    writeln!(w, "{} datapoint(s).", series.points.len())
}

fn render_history(w: &mut dyn Write, detective: &TimeDetective) -> io::Result<()> {
    render_error_datapoints(w, &detective.errors)?;
    writeln!(w)?;
    render_performance_datapoints(w, &detective.performance)
}

fn render_error_datapoints(w: &mut dyn Write, points: &[ErrorDataPoint]) -> io::Result<()> {
    writeln!(w, "Errors")?;
    if points.is_empty() {
        return writeln!(w, "  No error datapoints in this window.");
    }
    writeln!(
        w,
        "  {:<12} {:<34} {:<28} THROUGHPUT",
        "NAMESPACE", "ACTION", "EXCEPTION"
    )?;
    for point in points {
        writeln!(
            w,
            "  {:<12} {:<34} {:<28} {}",
            point.namespace.as_deref().unwrap_or("-"),
            truncate(point.action_name.as_deref().unwrap_or("-"), 34),
            truncate(point.exception_name.as_deref().unwrap_or("-"), 28),
            format_number(point.throughput),
        )?;
    }
    Ok(())
}

fn render_performance_datapoints(
    w: &mut dyn Write,
    points: &[PerformanceDataPoint],
) -> io::Result<()> {
    writeln!(w, "Performance")?;
    if points.is_empty() {
        return writeln!(w, "  No performance datapoints in this window.");
    }
    writeln!(
        w,
        "  {:<12} {:<40} {:>12} {:>10} {:>10}",
        "NAMESPACE", "ACTION", "THROUGHPUT", "MEAN", "P90"
    )?;
    for point in points {
        writeln!(
            w,
            "  {:<12} {:<40} {:>12} {:>10} {:>10}",
            point.namespace.as_deref().unwrap_or("-"),
            truncate(point.action_name.as_deref().unwrap_or("-"), 40),
            format_number(point.throughput),
            format_number(point.mean),
            format_number(point.p90),
        )?;
    }
    Ok(())
}

fn format_tags(tags: &[crate::api::KeyStringValue]) -> String {
    tags.iter()
        .map(|kv| format!("{}={}", kv.key, kv.value.as_deref().unwrap_or("")))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_number(value: Option<f64>) -> String {
    value
        .map(|v| format!("{:.2}", v))
        .unwrap_or_else(|| "-".to_string())
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
    use crate::api::{KeyStringValue, MetricTimeseriesPoint};

    #[test]
    fn parse_tags_parses_key_value_pairs() {
        let parsed =
            parse_tags(&["hostname=web-1".to_string(), "region = eu ".to_string()]).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].key, "hostname");
        assert_eq!(parsed[0].value, "web-1");
        assert_eq!(parsed[1].key, "region");
        assert_eq!(parsed[1].value, "eu");
    }

    #[test]
    fn parse_tags_rejects_missing_equals() {
        assert!(parse_tags(&["bad".to_string()]).is_err());
    }

    #[test]
    fn renders_metric_keys_table() {
        let keys = vec![MetricKey {
            name: "database.query_count".to_string(),
            kind: Some("counter".to_string()),
            digest: Some("abc".to_string()),
            tags: Some(vec![KeyStringValue {
                key: "hostname".to_string(),
                value: Some("web-1".to_string()),
            }]),
            fields: Some(vec!["COUNTER".to_string()]),
        }];
        let mut buf = Vec::new();
        render_metric_keys(&mut buf, &keys).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("database.query_count"));
        assert!(out.contains("counter"));
        assert!(out.contains("hostname=web-1"));
    }

    #[test]
    fn renders_timeseries_points() {
        let series = MetricTimeseries {
            start: Some("2026-05-19T00:00:00Z".to_string()),
            end: Some("2026-05-19T01:00:00Z".to_string()),
            resolution: Some("MINUTELY".to_string()),
            keys: vec![],
            points: vec![MetricTimeseriesPoint {
                timestamp: Some("2026-05-19T00:00:00Z".to_string()),
                values: vec![KeyStringValue {
                    key: "mean".to_string(),
                    value: Some("12.50".to_string()),
                }],
            }],
        };
        let mut buf = Vec::new();
        render_timeseries(&mut buf, "latency", &series).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("latency"));
        assert!(out.contains("mean=12.50"));
    }
}
