use std::io::{self, Write};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use tabled::Tabled;

use super::{authenticated_client, resolve_org};
use crate::api::{KeyStringValue, Trigger};
use crate::config::Config;
use crate::output::{self, Output};

#[derive(Serialize)]
struct TriggerListResponse<'a> {
    triggers: &'a [Trigger],
}

#[derive(Serialize)]
struct TriggerResponse<'a> {
    trigger: &'a Trigger,
}

#[derive(Tabled)]
struct TriggerRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "TRIGGER NAME")]
    name: String,
    #[tabled(rename = "METRIC NAME")]
    metric: String,
    #[tabled(rename = "FIELD")]
    field: String,
    #[tabled(rename = "KIND")]
    kind: String,
    #[tabled(rename = "DESCRIPTION")]
    description: String,
    #[tabled(rename = "THRESHOLD")]
    threshold: String,
    #[tabled(rename = "WARMUP")]
    warmup: String,
    #[tabled(rename = "COOLDOWN")]
    cooldown: String,
}

pub async fn list(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    metric_name: Option<&str>,
    kind: Option<&str>,
    tags: &[String],
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let parsed_tags = parse_tags(tags)?;
    let mut triggers = client
        .list_triggers(&resolved_app_id, parsed_tags.as_deref())
        .await?;

    if let Some(metric_name) = metric_name.filter(|value| !value.trim().is_empty()) {
        triggers.retain(|trigger| trigger.metric_name.eq_ignore_ascii_case(metric_name.trim()));
    }

    if let Some(kind) = kind.filter(|value| !value.trim().is_empty()) {
        triggers.retain(|trigger| trigger.kind.eq_ignore_ascii_case(kind.trim()));
    }

    output::print_with(
        TriggerListResponse {
            triggers: &triggers,
        },
        format,
        |w| render_trigger_table(w, &triggers),
    )
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    name: Option<&str>,
    metric_name: &str,
    kind: &str,
    field: &str,
    comparison_operator: &str,
    condition_value: f64,
    warmup_duration: i64,
    cooldown_duration: i64,
    notifier_ids: Option<&str>,
    tags: &[String],
    description: Option<&str>,
    no_match_is_zero: bool,
    dashboard_id: Option<&str>,
    format_name: Option<&str>,
    format_input: Option<&str>,
    output_format: Output,
) -> Result<()> {
    upsert(
        None,
        app_id,
        app_name,
        environment,
        org,
        name,
        metric_name,
        kind,
        field,
        comparison_operator,
        condition_value,
        warmup_duration,
        cooldown_duration,
        notifier_ids,
        tags,
        description,
        no_match_is_zero,
        dashboard_id,
        format_name,
        format_input,
        output_format,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    trigger_id: &str,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    name: Option<&str>,
    metric_name: &str,
    kind: &str,
    field: &str,
    comparison_operator: &str,
    condition_value: f64,
    warmup_duration: i64,
    cooldown_duration: i64,
    notifier_ids: Option<&str>,
    tags: &[String],
    description: Option<&str>,
    no_match_is_zero: bool,
    dashboard_id: Option<&str>,
    format_name: Option<&str>,
    format_input: Option<&str>,
    output_format: Output,
) -> Result<()> {
    upsert(
        Some(trigger_id),
        app_id,
        app_name,
        environment,
        org,
        name,
        metric_name,
        kind,
        field,
        comparison_operator,
        condition_value,
        warmup_duration,
        cooldown_duration,
        notifier_ids,
        tags,
        description,
        no_match_is_zero,
        dashboard_id,
        format_name,
        format_input,
        output_format,
    )
    .await
}

pub async fn archive(
    trigger_id: &str,
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

    let trigger = client.archive_trigger(&resolved_app_id, trigger_id).await?;

    crate::status!("Trigger {} archived.", trigger_id);
    output::print_with(TriggerResponse { trigger: &trigger }, format, |w| {
        render_trigger_detail(w, &trigger)
    })
}

#[allow(clippy::too_many_arguments)]
async fn upsert(
    previous_trigger_id: Option<&str>,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    name: Option<&str>,
    metric_name: &str,
    kind: &str,
    field: &str,
    comparison_operator: &str,
    condition_value: f64,
    warmup_duration: i64,
    cooldown_duration: i64,
    notifier_ids: Option<&str>,
    tags: &[String],
    description: Option<&str>,
    no_match_is_zero: bool,
    dashboard_id: Option<&str>,
    format_name: Option<&str>,
    format_input: Option<&str>,
    output_format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let parsed_tags = parse_tags(tags)?;
    let parsed_notifier_ids = parse_csv(notifier_ids);
    let trigger = client
        .create_trigger(
            &resolved_app_id,
            previous_trigger_id,
            name,
            metric_name,
            parsed_tags.as_deref(),
            kind,
            &normalize_metric_field(field)?,
            &normalize_comparison_operator(comparison_operator)?,
            condition_value,
            warmup_duration,
            cooldown_duration,
            parsed_notifier_ids.as_deref(),
            no_match_is_zero,
            description,
            dashboard_id,
            format_name,
            format_input,
        )
        .await?;

    if let Some(previous_trigger_id) = previous_trigger_id {
        crate::status!(
            "Trigger {} updated as new version {}.",
            previous_trigger_id,
            trigger.id
        );
    } else {
        crate::status!("Trigger {} created.", trigger.id);
    }

    output::print_with(TriggerResponse { trigger: &trigger }, output_format, |w| {
        render_trigger_detail(w, &trigger)
    })
}

fn render_trigger_table(w: &mut dyn Write, triggers: &[Trigger]) -> io::Result<()> {
    if triggers.is_empty() {
        return writeln!(w, "No triggers found.");
    }

    let rows = triggers.iter().map(|trigger| TriggerRow {
        id: truncate(&trigger.id, 22),
        name: truncate(&trigger.name, 28),
        metric: truncate(&trigger.metric_name, 24),
        field: trigger.field.clone(),
        kind: truncate(&trigger.kind, 22),
        description: truncate(trigger.description.as_deref().unwrap_or("-"), 36),
        threshold: threshold_summary(trigger),
        warmup: format!("{}m", trigger.warmup_duration),
        cooldown: format!("{}m", trigger.cooldown_duration),
    });

    output::table(w, rows)?;
    writeln!(w, "{} trigger(s) found.", triggers.len())
}

fn render_trigger_detail(w: &mut dyn Write, trigger: &Trigger) -> io::Result<()> {
    let threshold = threshold_summary(trigger);
    let tags = tags_summary(trigger.tags.as_deref());
    let notifiers = notifiers_summary(trigger);
    let previous = trigger
        .previous_trigger
        .as_ref()
        .map(|trigger| trigger.id.as_str())
        .unwrap_or("-");

    output::detail(
        w,
        &[
            ("ID", trigger.id.as_str()),
            ("Trigger name", trigger.name.as_str()),
            ("Metric name", trigger.metric_name.as_str()),
            ("Field", trigger.field.as_str()),
            ("Kind", trigger.kind.as_str()),
            ("Threshold", threshold.as_str()),
            ("Warmup", &format!("{} minutes", trigger.warmup_duration)),
            (
                "Cooldown",
                &format!("{} minutes", trigger.cooldown_duration),
            ),
            (
                "No match is zero",
                if trigger.no_match_is_zero {
                    "true"
                } else {
                    "false"
                },
            ),
            ("Dashboard", trigger.dashboard_id.as_deref().unwrap_or("-")),
            ("Notifiers", notifiers.as_str()),
            ("Tags", tags.as_str()),
            ("Previous", previous),
            ("Format", trigger.format.as_deref().unwrap_or("-")),
            (
                "Format input",
                trigger.format_input.as_deref().unwrap_or("-"),
            ),
            ("Description", trigger.description.as_deref().unwrap_or("-")),
        ],
    )
}

fn parse_tags(tags: &[String]) -> Result<Option<Vec<KeyStringValue>>> {
    if tags.is_empty() {
        return Ok(None);
    }

    let parsed = tags
        .iter()
        .map(|tag| {
            let (key, value) = tag
                .split_once('=')
                .with_context(|| format!("Invalid --tag value '{}'. Use key=value.", tag))?;
            let key = key.trim();
            let value = value.trim();

            if key.is_empty() || value.is_empty() {
                bail!("Invalid --tag value '{}'. Use key=value.", tag);
            }

            Ok(KeyStringValue {
                key: key.to_string(),
                value: Some(value.to_string()),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(parsed))
}

fn parse_csv(value: Option<&str>) -> Option<Vec<String>> {
    value.and_then(|value| {
        let values: Vec<String> = value
            .split(',')
            .map(|entry| entry.trim())
            .filter(|entry| !entry.is_empty())
            .map(|entry| entry.to_string())
            .collect();

        if values.is_empty() {
            None
        } else {
            Some(values)
        }
    })
}

fn normalize_metric_field(field: &str) -> Result<String> {
    match field.trim().to_ascii_uppercase().as_str() {
        "COUNT" => Ok("COUNT".to_string()),
        "COUNTER" => Ok("COUNTER".to_string()),
        "GAUGE" => Ok("GAUGE".to_string()),
        "MEAN" => Ok("MEAN".to_string()),
        "P90" => Ok("P90".to_string()),
        "P95" => Ok("P95".to_string()),
        _ => bail!("Invalid --field value. Use one of: count, counter, gauge, mean, p90, p95."),
    }
}

fn normalize_comparison_operator(operator: &str) -> Result<String> {
    match operator.trim() {
        ">" => Ok("GREATER_THAN".to_string()),
        ">=" => Ok("GREATER_THAN_OR_EQUAL".to_string()),
        "<" => Ok("LESS_THAN".to_string()),
        "<=" => Ok("LESS_THAN_OR_EQUAL".to_string()),
        "==" => Ok("EQUAL".to_string()),
        "!=" => Ok("NOT_EQUAL".to_string()),
        _ => bail!("Invalid --comparison-operator value. Use one of: >, >=, <, <=, ==, !=."),
    }
}

fn comparison_symbol(operator: &str) -> &str {
    match operator {
        "GREATER_THAN" => ">",
        "GREATER_THAN_OR_EQUAL" => ">=",
        "LESS_THAN" => "<",
        "LESS_THAN_OR_EQUAL" => "<=",
        "EQUAL" => "==",
        "NOT_EQUAL" => "!=",
        _ => operator,
    }
}

fn threshold_summary(trigger: &Trigger) -> String {
    format!(
        "{} {} {}",
        trigger.field,
        comparison_symbol(&trigger.threshold_condition.comparison_operator),
        trigger.threshold_condition.value,
    )
}

fn tags_summary(tags: Option<&[KeyStringValue]>) -> String {
    tags.map(|tags| {
        tags.iter()
            .map(|tag| match tag.value.as_deref() {
                Some(value) => format!("{}={}", tag.key, value),
                None => tag.key.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    })
    .filter(|summary| !summary.is_empty())
    .unwrap_or_else(|| "-".to_string())
}

fn notifiers_summary(trigger: &Trigger) -> String {
    trigger
        .notifiers
        .as_deref()
        .map(|notifiers| {
            notifiers
                .iter()
                .map(|notifier| notifier.name.as_deref().unwrap_or(notifier.id.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|summary| !summary.is_empty())
        .unwrap_or_else(|| "-".to_string())
}

fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        value.to_string()
    } else if max <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &value[..max - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_comparison_operator, normalize_metric_field, parse_tags};

    #[test]
    fn parse_tags_accepts_key_value_pairs() {
        let tags = parse_tags(&["hostname=web-1".to_string(), "region=eu".to_string()])
            .unwrap()
            .unwrap();

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].key, "hostname");
        assert_eq!(tags[0].value.as_deref(), Some("web-1"));
    }

    #[test]
    fn parse_tags_rejects_invalid_values() {
        let err = parse_tags(&["hostname".to_string()]).unwrap_err();
        assert!(err.to_string().contains("Invalid --tag value"));
    }

    #[test]
    fn normalize_metric_field_supports_known_values() {
        assert_eq!(normalize_metric_field("mean").unwrap(), "MEAN");
        assert_eq!(normalize_metric_field("P95").unwrap(), "P95");
    }

    #[test]
    fn normalize_comparison_operator_supports_symbols() {
        assert_eq!(
            normalize_comparison_operator(">=").unwrap(),
            "GREATER_THAN_OR_EQUAL"
        );
        assert_eq!(normalize_comparison_operator("!=").unwrap(), "NOT_EQUAL");
    }
}
