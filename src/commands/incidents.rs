use anyhow::Result;

use crate::api::{AppSignalClient, Incident};
use crate::config::Config;

/// Resolve the organization slug: use explicit --org if given, fall back to config.
fn resolve_org(explicit: Option<&str>, config: &Config) -> Result<String> {
    if let Some(org) = explicit {
        return Ok(org.to_string());
    }
    config
        .org
        .as_deref()
        .filter(|o| !o.is_empty())
        .map(|o| o.to_string())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No organization configured. Run `appsignal-cli apps list --org <slug>` first, \
                 or set it with `appsignal-cli apps set-org --org <slug>`."
            )
        })
}

/// List incidents for an application.
pub async fn list(
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
    let client = AppSignalClient::new(&token);

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let incidents = client
        .list_incidents(&resolved_app_id, limit, offset, state, order)
        .await?;

    if incidents.is_empty() {
        println!("No incidents found.");
        return Ok(());
    }

    println!(
        "{:<8} {:<12} {:<10} {:<10} {:<8} {:<22} {}",
        "#", "TYPE", "STATE", "SEVERITY", "COUNT", "LAST OCCURRED", "DESCRIPTION"
    );
    println!("{}", "-".repeat(100));

    for incident in &incidents {
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
    let client = AppSignalClient::new(&token);

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let incident = client
        .get_incident(&resolved_app_id, incident_number)
        .await?;

    print_incident_detail(&incident);
    Ok(())
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
            println!(
                "  Namespace:      {}",
                namespace.as_deref().unwrap_or("-")
            );
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
            println!(
                "  Namespace:      {}",
                namespace.as_deref().unwrap_or("-")
            );
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
        Incident::AnomalyIncident { .. } => {}
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
