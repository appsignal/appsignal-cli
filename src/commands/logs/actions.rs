//! `logs metrics` and `logs triggers` — CRUD over `LogLineAction`s.
//!
//! Both commands map to the same underlying GraphQL mutations
//! (`createLogLineAction` / `updateLogLineAction` / `deleteLogLineAction`),
//! discriminated by a typed [`LogLineActionKind`]. Keeping that dispatch typed
//! means the validators, renderers, and status messages all key off the kind
//! rather than re-parsing stringly-typed `action_type` values.

use std::io::{self, Write};

use anyhow::Result;
use serde::Serialize;

use super::super::{authenticated_client, resolve_org};
use super::truncate;
use crate::api::{
    AppSignalClient, LogLineAction, LogLineActionKind, LogLineActionTriggerInput,
    LogLineMetricInput, LogSource, Patch,
};
use crate::config::Config;
use crate::output::{self, Output};

// -- CLI input bundles --

/// The four flags every action command needs to resolve which app to talk to.
pub struct AppRef<'a> {
    pub app_id: Option<&'a str>,
    pub app_name: Option<&'a str>,
    pub environment: Option<&'a str>,
    pub org: Option<&'a str>,
}

/// Trigger-specific fields, in the shape an `update`/`create` cares about.
///
/// `description` uses [`Patch`] and `notifier_ids` / `severities` use
/// `Option<Vec<_>>`; both encode the same three states the server cares about
/// — leave alone, clear, or replace — but the scalar field uses `Patch` so
/// "set to empty string" and "clear" stay distinct.
#[derive(Default)]
pub struct TriggerFields {
    pub description: Patch<String>,
    pub notifier_ids: Option<Vec<String>>,
    pub severities: Option<Vec<String>>,
}

// -- Wire-format response envelopes --

#[derive(Serialize)]
struct ActionsResponse<'a> {
    actions: &'a [LogLineAction],
}

#[derive(Serialize)]
struct ActionResponse<'a> {
    action: &'a LogLineAction,
}

// -- Public commands --

/// List actions of a given kind for the resolved app.
pub async fn list(kind: LogLineActionKind, app: &AppRef<'_>, format: Output) -> Result<()> {
    let (client, app_id) = resolve_client(app).await?;
    let mut actions = client.list_log_line_actions(&app_id).await?;
    actions.retain(|action| kind.matches_api(action.action_type()));
    actions.sort_by_key(|action| action.order());

    output::print_with(ActionsResponse { actions: &actions }, format, |w| {
        render_action_table(w, kind, &actions)
    })
}

/// Create a metric action.
pub async fn create_metric(
    app: &AppRef<'_>,
    name: &str,
    query: &str,
    source_ids: &[String],
    metrics: &[LogLineMetricInput],
    format: Output,
) -> Result<()> {
    if metrics.is_empty() {
        anyhow::bail!("`logs metrics create` requires at least one `--metric` definition.");
    }
    let action = create_action(
        LogLineActionKind::Metrics,
        app,
        name,
        query,
        source_ids,
        Some(metrics),
        None,
    )
    .await?;
    finish_mutation(LogLineActionKind::Metrics, "created", &action, format)
}

/// Update a metric action.
pub async fn update_metric(
    app: &AppRef<'_>,
    id: &str,
    name: Option<&str>,
    query: Option<&str>,
    source_ids: Option<Vec<String>>,
    metrics: Option<Vec<LogLineMetricInput>>,
    format: Output,
) -> Result<()> {
    let action = update_action(
        app,
        id,
        name,
        query,
        source_ids.as_deref(),
        metrics.as_deref(),
        None,
    )
    .await?;
    finish_mutation(LogLineActionKind::Metrics, "updated", &action, format)
}

/// Create a trigger action.
pub async fn create_trigger(
    app: &AppRef<'_>,
    name: &str,
    query: &str,
    source_ids: &[String],
    trigger: TriggerFields,
    format: Output,
) -> Result<()> {
    let trigger_input = build_trigger_input(trigger);
    let action = create_action(
        LogLineActionKind::Trigger,
        app,
        name,
        query,
        source_ids,
        None,
        trigger_input.as_ref(),
    )
    .await?;
    finish_mutation(LogLineActionKind::Trigger, "created", &action, format)
}

/// Update a trigger action.
pub async fn update_trigger(
    app: &AppRef<'_>,
    id: &str,
    name: Option<&str>,
    query: Option<&str>,
    source_ids: Option<Vec<String>>,
    trigger: TriggerFields,
    format: Output,
) -> Result<()> {
    let trigger_input = build_trigger_input(trigger);
    let action = update_action(
        app,
        id,
        name,
        query,
        source_ids.as_deref(),
        None,
        trigger_input.as_ref(),
    )
    .await?;
    finish_mutation(LogLineActionKind::Trigger, "updated", &action, format)
}

/// Delete an action. The CLI namespace (`logs metrics` vs `logs triggers`) is
/// purely cosmetic for delete: the underlying mutation is keyed by ID.
pub async fn delete(
    kind: LogLineActionKind,
    app: &AppRef<'_>,
    id: &str,
    format: Output,
) -> Result<()> {
    let (client, app_id) = resolve_client(app).await?;
    let action = client.delete_log_line_action(&app_id, id).await?;
    finish_mutation(kind, "deleted", &action, format)
}

// -- Internal helpers --

async fn resolve_client(app: &AppRef<'_>) -> Result<(AppSignalClient, String)> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(app.org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let app_id = client
        .resolve_app_id(&org_slug, app.app_id, app.app_name, app.environment)
        .await?;
    Ok((client, app_id))
}

async fn create_action(
    kind: LogLineActionKind,
    app: &AppRef<'_>,
    name: &str,
    query: &str,
    source_ids: &[String],
    metrics: Option<&[LogLineMetricInput]>,
    trigger: Option<&LogLineActionTriggerInput>,
) -> Result<LogLineAction> {
    let (client, app_id) = resolve_client(app).await?;
    client
        .create_log_line_action(
            &app_id,
            name,
            query,
            kind,
            (!source_ids.is_empty()).then_some(source_ids),
            metrics,
            trigger,
        )
        .await
}

async fn update_action(
    app: &AppRef<'_>,
    id: &str,
    name: Option<&str>,
    query: Option<&str>,
    source_ids: Option<&[String]>,
    metrics: Option<&[LogLineMetricInput]>,
    trigger: Option<&LogLineActionTriggerInput>,
) -> Result<LogLineAction> {
    let (client, app_id) = resolve_client(app).await?;
    client
        .update_log_line_action(&app_id, id, name, query, source_ids, metrics, trigger)
        .await
}

fn finish_mutation(
    kind: LogLineActionKind,
    verb: &str,
    action: &LogLineAction,
    format: Output,
) -> Result<()> {
    crate::status!("{} {} {}.", display_label(kind), action.id(), verb);
    output::print_with(ActionResponse { action }, format, |w| {
        render_action_detail(w, kind, action)
    })
}

fn build_trigger_input(fields: TriggerFields) -> Option<LogLineActionTriggerInput> {
    let TriggerFields {
        description,
        notifier_ids,
        severities,
    } = fields;

    let severities = severities.map(|values| {
        values
            .into_iter()
            .map(|severity| severity.trim().to_ascii_uppercase())
            .collect::<Vec<_>>()
    });

    if description.is_unchanged() && notifier_ids.is_none() && severities.is_none() {
        None
    } else {
        Some(LogLineActionTriggerInput {
            description,
            notifier_ids,
            severities,
        })
    }
}

// -- Rendering --

fn display_label(kind: LogLineActionKind) -> &'static str {
    match kind {
        LogLineActionKind::Metrics => "Log metric",
        LogLineActionKind::Trigger => "Log trigger",
    }
}

fn render_action_table(
    w: &mut dyn Write,
    kind: LogLineActionKind,
    actions: &[LogLineAction],
) -> io::Result<()> {
    if actions.is_empty() {
        return writeln!(w, "No {} found.", plural_lowercase(kind));
    }

    match kind {
        LogLineActionKind::Metrics => {
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
                    truncate(&metric_names_summary(action), 24),
                    truncate(&sources_summary(action), 22),
                    truncate(action.query(), 16),
                    truncate(action.id(), 26),
                )?;
            }
            writeln!(w, "{} log metric configuration(s) found.", actions.len())
        }
        LogLineActionKind::Trigger => {
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
                    truncate(&trigger_severities_summary(action), 22),
                    truncate(&trigger_notifiers_summary(action), 16),
                    truncate(action.query(), 16),
                    truncate(action.id(), 26),
                )?;
            }
            writeln!(w, "{} log trigger(s) found.", actions.len())
        }
    }
}

fn render_action_detail(
    w: &mut dyn Write,
    kind: LogLineActionKind,
    action: &LogLineAction,
) -> io::Result<()> {
    let sources = sources_summary(action);

    match kind {
        LogLineActionKind::Metrics => {
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
        LogLineActionKind::Trigger => {
            let severities = trigger_severities_summary(action);
            let notifiers = trigger_notifiers_summary(action);
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
                .map(|(key, value)| format!("{}={}", key, value))
                .collect::<Vec<_>>()
                .join(", ");
            details.push(format!("tags: {}", tags));
        }
        writeln!(w, "  - {}", details.join(" | "))?;
    }
    Ok(())
}

fn plural_lowercase(kind: LogLineActionKind) -> &'static str {
    match kind {
        LogLineActionKind::Metrics => "log metrics",
        LogLineActionKind::Trigger => "log triggers",
    }
}

fn sources_summary(action: &LogLineAction) -> String {
    let source_names: Vec<&str> = action
        .sources()
        .iter()
        .map(|source: &LogSource| source.name.as_str())
        .collect();

    if !source_names.is_empty() {
        source_names.join(",")
    } else if !action.source_ids().is_empty() {
        action.source_ids().join(",")
    } else {
        "all".to_string()
    }
}

fn metric_names_summary(action: &LogLineAction) -> String {
    let names: Vec<String> = action
        .metrics()
        .iter()
        .map(|metric| metric.name.clone())
        .collect();
    if names.is_empty() {
        "-".to_string()
    } else {
        names.join(",")
    }
}

fn trigger_severities_summary(action: &LogLineAction) -> String {
    let severities = action.trigger_severities();
    if severities.is_empty() {
        "all".to_string()
    } else {
        severities.join(",")
    }
}

fn trigger_notifiers_summary(action: &LogLineAction) -> String {
    let names: Vec<String> = action
        .trigger_notifiers()
        .iter()
        .map(|notifier| notifier.name.clone().unwrap_or_else(|| notifier.id.clone()))
        .collect();
    if names.is_empty() {
        "-".to_string()
    } else {
        names.join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{LogLineActionCommon, LogSource};

    #[test]
    fn build_trigger_input_keeps_explicit_empty_lists_for_clear_operations() {
        let trigger = build_trigger_input(TriggerFields {
            description: Patch::Set("desc".to_string()),
            notifier_ids: Some(vec![]),
            severities: Some(vec![]),
        })
        .unwrap();

        assert_eq!(trigger.description, Patch::Set("desc".to_string()));
        assert_eq!(trigger.notifier_ids, Some(vec![]));
        assert_eq!(trigger.severities, Some(vec![]));
    }

    #[test]
    fn build_trigger_input_emits_clear_description() {
        let trigger = build_trigger_input(TriggerFields {
            description: Patch::Clear,
            notifier_ids: None,
            severities: None,
        })
        .unwrap();

        assert_eq!(trigger.description, Patch::Clear);
        let json = serde_json::to_value(&trigger).unwrap();
        assert_eq!(json, serde_json::json!({ "description": null }));
    }

    #[test]
    fn build_trigger_input_none_when_all_fields_unset() {
        let trigger = build_trigger_input(TriggerFields::default());
        assert!(trigger.is_none());
    }

    fn metrics_action_fixture() -> LogLineAction {
        LogLineAction::Metrics {
            common: LogLineActionCommon {
                id: "action-2".to_string(),
                name: "Track error count".to_string(),
                query: "severity:error".to_string(),
                source_ids: vec![],
                action_type: "METRICS".to_string(),
                sources: vec![],
                order: 1,
            },
            log_line_metrics: vec![],
            user: None,
        }
    }

    fn trigger_action_fixture() -> LogLineAction {
        LogLineAction::Trigger {
            common: LogLineActionCommon {
                id: "trigger-1".to_string(),
                name: "Root login".to_string(),
                query: "message:root".to_string(),
                source_ids: vec!["src-1".to_string()],
                action_type: "TRIGGER".to_string(),
                sources: vec![LogSource {
                    id: "src-1".to_string(),
                    name: "auth".to_string(),
                    kind: Some("custom".to_string()),
                    fmt: Some("json".to_string()),
                }],
                order: 0,
            },
            description: Some("Alert on root logins".to_string()),
            severities: vec!["ERROR".to_string()],
            user: None,
            previous_trigger: None,
            notification_options: None,
            notification_trigger_value: None,
            notifiers: None,
        }
    }

    #[test]
    fn render_action_table_metrics() {
        let actions = vec![metrics_action_fixture()];
        let mut buf = Vec::new();
        render_action_table(&mut buf, LogLineActionKind::Metrics, &actions).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("METRICS"));
        assert!(output.contains("Track error count"));
        assert!(output.contains("1 log metric configuration(s) found."));
    }

    #[test]
    fn render_action_table_triggers() {
        let actions = vec![trigger_action_fixture()];
        let mut buf = Vec::new();
        render_action_table(&mut buf, LogLineActionKind::Trigger, &actions).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("SEVERITIES"));
        assert!(output.contains("Root login"));
        assert!(output.contains("1 log trigger(s) found."));
    }
}
