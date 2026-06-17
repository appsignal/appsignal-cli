use std::io::{self, Write};

use anyhow::Result;
use serde::Serialize;

use super::{authenticated_client, resolve_org};
use crate::api::{resolve_user_ids, Incident};
use crate::config::Config;
use crate::output::{self, Output};

#[derive(Serialize)]
struct IncidentListResponse<'a> {
    incidents: &'a [Incident],
}

#[derive(Serialize)]
struct IncidentResponse<'a> {
    incident: &'a Incident,
}

#[derive(Serialize)]
struct IncidentNoteResponse {
    incident_number: i64,
    message: String,
}

/// List incidents for an application (all types).
#[allow(clippy::too_many_arguments)]
pub async fn list(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: Option<&str>,
    order: Option<&str>,
    namespaces: Option<&str>,
    action_name: Option<&str>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let ns: Option<Vec<String>> =
        namespaces.map(|s| s.split(',').map(|n| n.trim().to_string()).collect());

    let incidents = client
        .list_incidents(
            &resolved_app_id,
            limit,
            offset,
            state,
            order,
            ns.as_deref(),
            action_name,
        )
        .await?;

    output::print_with(
        IncidentListResponse {
            incidents: &incidents,
        },
        format,
        |w| render_incident_table(w, &incidents),
    )
}

/// List exception incidents for an application.
#[allow(clippy::too_many_arguments)]
pub async fn list_exceptions(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: Option<&str>,
    order: Option<&str>,
    namespaces: Option<&str>,
    action_name: Option<&str>,
    query: Option<&str>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let ns: Option<Vec<String>> =
        namespaces.map(|s| s.split(',').map(|n| n.trim().to_string()).collect());

    let incidents = client
        .list_exception_incidents(
            &resolved_app_id,
            limit,
            offset,
            state,
            order,
            ns.as_deref(),
            action_name,
            query,
        )
        .await?;

    output::print_with(
        IncidentListResponse {
            incidents: &incidents,
        },
        format,
        |w| render_exception_table(w, &incidents),
    )
}

/// List anomaly incidents for an application.
#[allow(clippy::too_many_arguments)]
pub async fn list_anomalies(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: Option<&str>,
    order: Option<&str>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let incidents = client
        .list_anomaly_incidents(&resolved_app_id, limit, offset, state, order)
        .await?;

    output::print_with(
        IncidentListResponse {
            incidents: &incidents,
        },
        format,
        |w| render_anomaly_table(w, &incidents),
    )
}

/// List performance incidents for an application.
#[allow(clippy::too_many_arguments)]
pub async fn list_performance(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: Option<&str>,
    order: Option<&str>,
    namespaces: Option<&str>,
    action_name: Option<&str>,
    query: Option<&str>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let ns: Option<Vec<String>> =
        namespaces.map(|s| s.split(',').map(|n| n.trim().to_string()).collect());

    let incidents = client
        .list_performance_incidents(
            &resolved_app_id,
            limit,
            offset,
            state,
            order,
            ns.as_deref(),
            action_name,
            query,
        )
        .await?;

    output::print_with(
        IncidentListResponse {
            incidents: &incidents,
        },
        format,
        |w| render_performance_table(w, &incidents),
    )
}

/// Show details for a specific incident.
pub async fn show(
    incident_number: i64,
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

    let incident = client
        .get_incident(&resolved_app_id, incident_number)
        .await?;

    output::print_with(
        IncidentResponse {
            incident: &incident,
        },
        format,
        |w| render_incident_detail(w, &incident),
    )
}

/// Update an incident (state, severity, assignees, description).
/// Assign/unassign accept user names (resolved case-insensitively) or raw IDs.
#[allow(clippy::too_many_arguments)]
pub async fn update(
    incident_numbers: &[i64],
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    state: Option<&str>,
    severity: Option<&str>,
    assign: Option<&[String]>,
    assign_me: bool,
    unassign: Option<&[String]>,
    description: Option<&str>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    if incident_numbers.len() > 1 {
        if state.is_none() {
            anyhow::bail!("Bulk incident updates currently require `--state`.");
        }

        if severity.is_some()
            || assign.is_some()
            || assign_me
            || unassign.is_some()
            || description.is_some()
        {
            anyhow::bail!(
                "Bulk incident updates currently support only `--state`. Use a single `--number` for severity, assignee, or description changes."
            );
        }

        let mut incident_ids = Vec::with_capacity(incident_numbers.len());
        for incident_number in incident_numbers {
            let incident = client
                .get_incident(&resolved_app_id, *incident_number)
                .await?;
            incident_ids.push(incident.id().to_string());
        }

        let incidents = client
            .bulk_update_incidents(&resolved_app_id, &incident_ids, state.unwrap())
            .await?;

        crate::status!("{} incidents updated.", incidents.len());
        return output::print_with(
            IncidentListResponse {
                incidents: &incidents,
            },
            format,
            |w| render_incident_table(w, &incidents),
        );
    }

    let incident_number = incident_numbers[0];

    // If we need to assign/unassign, resolve names to IDs and merge with current assignees
    let final_assignee_ids = if assign.is_some() || assign_me || unassign.is_some() {
        // Fetch app users for name resolution
        let users = client.list_app_users(&resolved_app_id).await?;

        // Fetch current incident to get existing assignees
        let current = client
            .get_incident(&resolved_app_id, incident_number)
            .await?;
        let mut current_ids: Vec<String> = current.assignee_ids();

        // Add new assignees
        if let Some(to_add) = assign {
            let add_ids = resolve_user_ids(to_add, &users)?;
            for id in add_ids {
                if !current_ids.contains(&id) {
                    current_ids.push(id);
                }
            }
        }

        if assign_me {
            let current_user = client.current_user().await?;
            if !current_ids.contains(&current_user.id) {
                current_ids.push(current_user.id);
            }
        }

        // Remove unassigned
        if let Some(to_remove) = unassign {
            let remove_ids = resolve_user_ids(to_remove, &users)?;
            current_ids.retain(|id| !remove_ids.contains(id));
        }

        Some(current_ids)
    } else {
        None
    };

    let incident = client
        .update_incident(
            &resolved_app_id,
            incident_number,
            state,
            severity,
            final_assignee_ids.as_deref(),
            description,
        )
        .await?;

    crate::status!("Incident #{} updated.", incident_number);
    output::print_with(
        IncidentResponse {
            incident: &incident,
        },
        format,
        |w| render_incident_detail(w, &incident),
    )
}

/// Add a note to an incident.
pub async fn add_note(
    incident_number: i64,
    content: &str,
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

    let _incident = client
        .create_incident_note(&resolved_app_id, incident_number, content)
        .await?;

    output::print_with(
        IncidentNoteResponse {
            incident_number,
            message: format!("Note added to incident #{}.", incident_number),
        },
        format,
        |w| writeln!(w, "Note added to incident #{}.", incident_number),
    )
}

fn render_exception_table(w: &mut dyn Write, incidents: &[Incident]) -> io::Result<()> {
    if incidents.is_empty() {
        return writeln!(w, "No exception incidents found.");
    }

    writeln!(
        w,
        "{:<8} {:<10} {:<10} {:<8} {:<22} EXCEPTION",
        "#", "STATE", "SEVERITY", "COUNT", "LAST OCCURRED"
    )?;
    writeln!(w, "{}", "-".repeat(100))?;

    for incident in incidents {
        let exception = if let Incident::ExceptionIncident { exception_name, .. } = incident {
            exception_name.as_deref().unwrap_or("-")
        } else {
            "-"
        };
        writeln!(
            w,
            "{:<8} {:<10} {:<10} {:<8} {:<22} {}",
            incident.number(),
            incident.state(),
            incident.severity(),
            incident.count(),
            incident.last_occurred_at(),
            truncate(exception, 50),
        )?;
    }

    writeln!(w, "{} exception incident(s) found.", incidents.len())
}

fn render_performance_table(w: &mut dyn Write, incidents: &[Incident]) -> io::Result<()> {
    if incidents.is_empty() {
        return writeln!(w, "No performance incidents found.");
    }

    writeln!(
        w,
        "{:<8} {:<10} {:<10} {:<8} {:<22} ACTION",
        "#", "STATE", "SEVERITY", "COUNT", "LAST OCCURRED"
    )?;
    writeln!(w, "{}", "-".repeat(100))?;

    for incident in incidents {
        let action = if let Incident::PerformanceIncident { action_names, .. } = incident {
            action_names
                .as_ref()
                .and_then(|names| names.first())
                .map(|s| s.as_str())
                .unwrap_or("-")
        } else {
            "-"
        };
        writeln!(
            w,
            "{:<8} {:<10} {:<10} {:<8} {:<22} {}",
            incident.number(),
            incident.state(),
            incident.severity(),
            incident.count(),
            incident.last_occurred_at(),
            truncate(action, 50),
        )?;
    }

    writeln!(w, "{} performance incident(s) found.", incidents.len())
}

fn render_incident_table(w: &mut dyn Write, incidents: &[Incident]) -> io::Result<()> {
    if incidents.is_empty() {
        return writeln!(w, "No incidents found.");
    }

    writeln!(
        w,
        "{:<8} {:<12} {:<10} {:<10} {:<8} {:<22} DESCRIPTION",
        "#", "TYPE", "STATE", "SEVERITY", "COUNT", "LAST OCCURRED"
    )?;
    writeln!(w, "{}", "-".repeat(100))?;

    for incident in incidents {
        let desc = truncate(incident.description(), 40);
        writeln!(
            w,
            "{:<8} {:<12} {:<10} {:<10} {:<8} {:<22} {}",
            incident.number(),
            incident.kind(),
            incident.state(),
            incident.severity(),
            incident.count(),
            incident.last_occurred_at(),
            desc,
        )?;
    }

    writeln!(w, "{} incident(s) found.", incidents.len())
}

fn render_anomaly_table(w: &mut dyn Write, incidents: &[Incident]) -> io::Result<()> {
    if incidents.is_empty() {
        return writeln!(w, "No anomaly incidents found.");
    }

    writeln!(
        w,
        "{:<8} {:<10} {:<10} {:<10} {:<8} {:<22} TRIGGER",
        "#", "STATE", "SEVERITY", "ALERT", "COUNT", "LAST OCCURRED"
    )?;
    writeln!(w, "{}", "-".repeat(100))?;

    for incident in incidents {
        let trigger_name = if let Incident::AnomalyIncident { trigger, .. } = incident {
            trigger.as_ref().map(|t| t.name.as_str()).unwrap_or("-")
        } else {
            "-"
        };
        let alert_state = if let Incident::AnomalyIncident { alert_state, .. } = incident {
            alert_state.as_deref().unwrap_or("-")
        } else {
            "-"
        };
        writeln!(
            w,
            "{:<8} {:<10} {:<10} {:<10} {:<8} {:<22} {}",
            incident.number(),
            incident.state(),
            incident.severity(),
            alert_state,
            incident.count(),
            incident.last_occurred_at(),
            truncate(trigger_name, 40),
        )?;
    }

    writeln!(w, "{} anomaly incident(s) found.", incidents.len())
}

fn render_incident_detail(w: &mut dyn Write, incident: &Incident) -> io::Result<()> {
    let incident_label = format!("#{}", incident.number());
    let count = incident.count().to_string();
    output::detail(
        w,
        &[
            ("Incident", &incident_label),
            ("Type", incident.kind()),
            ("State", incident.state()),
            ("Severity", incident.severity()),
            ("Count", &count),
            ("Description", incident.description()),
            ("Created at", incident.created_at()),
            ("Last occurred", incident.last_occurred_at()),
        ],
    )?;

    let assignees = incident.assignees();
    if !assignees.is_empty() {
        let names: Vec<&str> = assignees
            .iter()
            .map(|u| u.name.as_deref().unwrap_or(&u.id))
            .collect();
        writeln!(w, "Assignees:      {}", names.join(", "))?;
    }

    match incident {
        Incident::ExceptionIncident {
            exception_name,
            exception_message,
            action_names,
            namespace,
            first_backtrace_line,
            ..
        } => {
            writeln!(
                w,
                "Exception:      {}",
                exception_name.as_deref().unwrap_or("-")
            )?;
            writeln!(
                w,
                "Message:        {}",
                exception_message.as_deref().unwrap_or("-")
            )?;
            writeln!(w, "Namespace:      {}", namespace.as_deref().unwrap_or("-"))?;
            if let Some(actions) = action_names {
                if !actions.is_empty() {
                    writeln!(w, "Actions:        {}", actions.join(", "))?;
                }
            }
            writeln!(
                w,
                "Backtrace:      {}",
                first_backtrace_line.as_deref().unwrap_or("-")
            )?;
        }
        Incident::PerformanceIncident {
            action_names,
            namespace,
            mean,
            total_duration,
            ..
        } => {
            writeln!(w, "Namespace:      {}", namespace.as_deref().unwrap_or("-"))?;
            if let Some(m) = mean {
                writeln!(w, "Mean duration:  {:.2} ms", m)?;
            }
            if let Some(td) = total_duration {
                writeln!(w, "Total duration: {:.2} ms", td)?;
            }
            if let Some(actions) = action_names {
                if !actions.is_empty() {
                    writeln!(w, "Actions:        {}", actions.join(", "))?;
                }
            }
        }
        Incident::AnomalyIncident {
            alert_state,
            trigger,
            tags,
            ..
        } => {
            if let Some(state) = alert_state {
                writeln!(w, "Alert state:    {}", state)?;
            }
            if let Some(t) = trigger {
                writeln!(w, "Trigger:        {} ({})", t.name, t.kind)?;
                writeln!(w, "Metric:         {}", t.metric_name)?;
            }
            if let Some(tags) = tags {
                if !tags.is_empty() {
                    let tag_strs: Vec<String> = tags
                        .iter()
                        .map(|t| format!("{}={}", t.key, t.value.as_deref().unwrap_or("")))
                        .collect();
                    writeln!(w, "Tags:           {}", tag_strs.join(", "))?;
                }
            }
        }
        Incident::LogIncident { .. } => {}
    }

    Ok(())
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

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_truncate_empty_string() {
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn test_truncate_with_small_max() {
        // max=3 means 0 chars + "..." = "..."
        assert_eq!(truncate("hello", 3), "...");
    }

    #[test]
    fn test_truncate_one_over() {
        assert_eq!(truncate("abcdef", 5), "ab...");
    }
}
