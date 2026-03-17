use anyhow::Result;

use super::resolve_org;
use crate::api::{resolve_user_ids, AppSignalClient, Incident};
use crate::config::Config;

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
) -> Result<()> {
    let config = Config::load()?;
    let token = config.require_token()?.to_string();
    let org_slug = resolve_org(org, &config)?;
    let client = AppSignalClient::new(&token, config.endpoint.as_deref());

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

    print_incident_table(&incidents);
    Ok(())
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
) -> Result<()> {
    let config = Config::load()?;
    let token = config.require_token()?.to_string();
    let org_slug = resolve_org(org, &config)?;
    let client = AppSignalClient::new(&token, config.endpoint.as_deref());

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

    print_exception_table(&incidents);
    Ok(())
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
) -> Result<()> {
    let config = Config::load()?;
    let token = config.require_token()?.to_string();
    let org_slug = resolve_org(org, &config)?;
    let client = AppSignalClient::new(&token, config.endpoint.as_deref());

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let incidents = client
        .list_anomaly_incidents(&resolved_app_id, limit, offset, state, order)
        .await?;

    if incidents.is_empty() {
        println!("No anomaly incidents found.");
        return Ok(());
    }

    println!(
        "{:<8} {:<10} {:<10} {:<10} {:<8} {:<22} TRIGGER",
        "#", "STATE", "SEVERITY", "ALERT", "COUNT", "LAST OCCURRED"
    );
    println!("{}", "-".repeat(100));

    for incident in &incidents {
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
        println!(
            "{:<8} {:<10} {:<10} {:<10} {:<8} {:<22} {}",
            incident.number(),
            incident.state(),
            incident.severity(),
            alert_state,
            incident.count(),
            incident.last_occurred_at(),
            truncate(trigger_name, 40),
        );
    }

    println!("\n{} anomaly incident(s) found.", incidents.len());
    Ok(())
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
) -> Result<()> {
    let config = Config::load()?;
    let token = config.require_token()?.to_string();
    let org_slug = resolve_org(org, &config)?;
    let client = AppSignalClient::new(&token, config.endpoint.as_deref());

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

    print_performance_table(&incidents);
    Ok(())
}

/// Show details for a specific incident.
pub async fn show(
    incident_number: i64,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let token = config.require_token()?.to_string();
    let org_slug = resolve_org(org, &config)?;
    let client = AppSignalClient::new(&token, config.endpoint.as_deref());

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let incident = client
        .get_incident(&resolved_app_id, incident_number)
        .await?;

    print_incident_detail(&incident);
    Ok(())
}

/// Update an incident (state, severity, assignees, description).
/// Assign/unassign accept user names (resolved case-insensitively) or raw IDs.
#[allow(clippy::too_many_arguments)]
pub async fn update(
    incident_number: i64,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    state: Option<&str>,
    severity: Option<&str>,
    assign: Option<&[String]>,
    unassign: Option<&[String]>,
    description: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let token = config.require_token()?.to_string();
    let org_slug = resolve_org(org, &config)?;
    let client = AppSignalClient::new(&token, config.endpoint.as_deref());

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    // If we need to assign/unassign, resolve names to IDs and merge with current assignees
    let final_assignee_ids = if assign.is_some() || unassign.is_some() {
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

    println!("Incident #{} updated.", incident_number);
    print_incident_detail(&incident);
    Ok(())
}

/// Add a note to an incident.
pub async fn add_note(
    incident_number: i64,
    content: &str,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    let token = config.require_token()?.to_string();
    let org_slug = resolve_org(org, &config)?;
    let client = AppSignalClient::new(&token, config.endpoint.as_deref());

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let _incident = client
        .create_incident_note(&resolved_app_id, incident_number, content)
        .await?;

    println!("Note added to incident #{}.", incident_number);
    Ok(())
}

fn print_exception_table(incidents: &[Incident]) {
    if incidents.is_empty() {
        println!("No exception incidents found.");
        return;
    }

    println!(
        "{:<8} {:<10} {:<10} {:<8} {:<22} EXCEPTION",
        "#", "STATE", "SEVERITY", "COUNT", "LAST OCCURRED"
    );
    println!("{}", "-".repeat(100));

    for incident in incidents {
        let exception = if let Incident::ExceptionIncident { exception_name, .. } = incident {
            exception_name.as_deref().unwrap_or("-")
        } else {
            "-"
        };
        println!(
            "{:<8} {:<10} {:<10} {:<8} {:<22} {}",
            incident.number(),
            incident.state(),
            incident.severity(),
            incident.count(),
            incident.last_occurred_at(),
            truncate(exception, 50),
        );
    }

    println!("\n{} exception incident(s) found.", incidents.len());
}

fn print_performance_table(incidents: &[Incident]) {
    if incidents.is_empty() {
        println!("No performance incidents found.");
        return;
    }

    println!(
        "{:<8} {:<10} {:<10} {:<8} {:<22} ACTION",
        "#", "STATE", "SEVERITY", "COUNT", "LAST OCCURRED"
    );
    println!("{}", "-".repeat(100));

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
        println!(
            "{:<8} {:<10} {:<10} {:<8} {:<22} {}",
            incident.number(),
            incident.state(),
            incident.severity(),
            incident.count(),
            incident.last_occurred_at(),
            truncate(action, 50),
        );
    }

    println!("\n{} performance incident(s) found.", incidents.len());
}

fn print_incident_table(incidents: &[Incident]) {
    if incidents.is_empty() {
        println!("No incidents found.");
        return;
    }

    println!(
        "{:<8} {:<12} {:<10} {:<10} {:<8} {:<22} DESCRIPTION",
        "#", "TYPE", "STATE", "SEVERITY", "COUNT", "LAST OCCURRED"
    );
    println!("{}", "-".repeat(100));

    for incident in incidents {
        let desc = truncate(incident.description(), 40);
        println!(
            "{:<8} {:<12} {:<10} {:<10} {:<8} {:<22} {}",
            incident.number(),
            incident.kind(),
            incident.state(),
            incident.severity(),
            incident.count(),
            incident.last_occurred_at(),
            desc,
        );
    }

    println!("\n{} incident(s) found.", incidents.len());
}

fn print_incident_detail(incident: &Incident) {
    println!("Incident #{}", incident.number());
    println!("  Type:           {}", incident.kind());
    println!("  State:          {}", incident.state());
    println!("  Severity:       {}", incident.severity());
    println!("  Count:          {}", incident.count());
    println!("  Description:    {}", incident.description());
    println!("  Created at:     {}", incident.created_at());
    println!("  Last occurred:  {}", incident.last_occurred_at());

    let assignees = incident.assignees();
    if !assignees.is_empty() {
        let names: Vec<&str> = assignees
            .iter()
            .map(|u| u.name.as_deref().unwrap_or(&u.id))
            .collect();
        println!("  Assignees:      {}", names.join(", "));
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
            println!(
                "  Exception:      {}",
                exception_name.as_deref().unwrap_or("-")
            );
            println!(
                "  Message:        {}",
                exception_message.as_deref().unwrap_or("-")
            );
            println!("  Namespace:      {}", namespace.as_deref().unwrap_or("-"));
            if let Some(actions) = action_names {
                if !actions.is_empty() {
                    println!("  Actions:        {}", actions.join(", "));
                }
            }
            println!(
                "  Backtrace:      {}",
                first_backtrace_line.as_deref().unwrap_or("-")
            );
        }
        Incident::PerformanceIncident {
            action_names,
            namespace,
            mean,
            total_duration,
            ..
        } => {
            println!("  Namespace:      {}", namespace.as_deref().unwrap_or("-"));
            if let Some(m) = mean {
                println!("  Mean duration:  {:.2} ms", m);
            }
            if let Some(td) = total_duration {
                println!("  Total duration: {:.2} ms", td);
            }
            if let Some(actions) = action_names {
                if !actions.is_empty() {
                    println!("  Actions:        {}", actions.join(", "));
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
                println!("  Alert state:    {}", state);
            }
            if let Some(t) = trigger {
                println!("  Trigger:        {} ({})", t.name, t.kind);
                println!("  Metric:         {}", t.metric_name);
            }
            if let Some(tags) = tags {
                if !tags.is_empty() {
                    let tag_strs: Vec<String> = tags
                        .iter()
                        .map(|t| format!("{}={}", t.key, t.value.as_deref().unwrap_or("")))
                        .collect();
                    println!("  Tags:           {}", tag_strs.join(", "));
                }
            }
        }
        Incident::LogIncident { .. } => {}
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
