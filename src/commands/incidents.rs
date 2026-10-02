use std::io::{self, Write};

use anyhow::Result;
use serde::Serialize;
use tabled::Tabled;

use super::{authenticated_client, resolve_org};
use crate::api::{
    resolve_user_ids, AppSignalClient, Incident, IncidentNote, IncidentNotificationFrequency,
};
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
struct IncidentShowResponse<'a> {
    incident: &'a Incident,
    #[serde(skip_serializing_if = "Option::is_none")]
    exception_error: Option<&'a ExceptionErrorView>,
    #[serde(skip_serializing_if = "is_empty_exception_errors")]
    error_causes: &'a [ExceptionErrorView],
}

#[derive(Serialize)]
pub(super) struct ExceptionErrorView {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_line: Option<String>,
}

fn is_empty_exception_errors(errors: &&[ExceptionErrorView]) -> bool {
    errors.is_empty()
}

#[derive(Serialize)]
struct IncidentNoteResponse {
    incident_number: i64,
    message: String,
}

#[derive(Serialize)]
struct IncidentNotesResponse<'a> {
    incident_number: i64,
    notes: &'a [IncidentNote],
}

#[derive(Tabled)]
struct IncidentNoteRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "AUTHOR")]
    author: String,
    #[tabled(rename = "SOURCE")]
    source: String,
    #[tabled(rename = "CAN EDIT")]
    can_edit: &'static str,
    #[tabled(rename = "CAN DELETE")]
    can_delete: &'static str,
    #[tabled(rename = "UPDATED")]
    updated_at: String,
    #[tabled(rename = "CONTENT")]
    content: String,
}

#[derive(Tabled)]
struct ExceptionIncidentRow {
    #[tabled(rename = "#")]
    number: i64,
    #[tabled(rename = "STATE")]
    state: String,
    #[tabled(rename = "SEVERITY")]
    severity: String,
    #[tabled(rename = "COUNT")]
    count: i64,
    #[tabled(rename = "LAST OCCURRED")]
    last_occurred: String,
    #[tabled(rename = "EXCEPTION")]
    exception: String,
}

#[derive(Tabled)]
struct PerformanceIncidentRow {
    #[tabled(rename = "#")]
    number: i64,
    #[tabled(rename = "STATE")]
    state: String,
    #[tabled(rename = "SEVERITY")]
    severity: String,
    #[tabled(rename = "COUNT")]
    count: i64,
    #[tabled(rename = "LAST OCCURRED")]
    last_occurred: String,
    #[tabled(rename = "ACTION")]
    action: String,
}

#[derive(Tabled)]
struct IncidentRow {
    #[tabled(rename = "#")]
    number: i64,
    #[tabled(rename = "TYPE")]
    kind: String,
    #[tabled(rename = "STATE")]
    state: String,
    #[tabled(rename = "SEVERITY")]
    severity: String,
    #[tabled(rename = "COUNT")]
    count: i64,
    #[tabled(rename = "LAST OCCURRED")]
    last_occurred: String,
    #[tabled(rename = "DESCRIPTION")]
    description: String,
}

#[derive(Tabled)]
struct AnomalyIncidentRow {
    #[tabled(rename = "#")]
    number: i64,
    #[tabled(rename = "STATE")]
    state: String,
    #[tabled(rename = "SEVERITY")]
    severity: String,
    #[tabled(rename = "ALERT")]
    alert: String,
    #[tabled(rename = "COUNT")]
    count: i64,
    #[tabled(rename = "LAST OCCURRED")]
    last_occurred: String,
    #[tabled(rename = "TRIGGER")]
    trigger: String,
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
    marker: Option<&str>,
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
            marker,
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

    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    show_with_client(
        &client,
        &resolved_app_id,
        incident_number,
        format,
        &mut stdout,
        &mut stderr,
    )
    .await
}

async fn show_with_client(
    client: &AppSignalClient,
    app_id: &str,
    incident_number: i64,
    format: Output,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<()> {
    let incident = client.get_incident(app_id, incident_number).await?;
    let enrichment = super::incident_enrichment::fetch(client, app_id, &incident).await;
    if let Some(warning) = enrichment.warning {
        writeln!(stderr, "Warning: {warning}")?;
    }
    let causes: Vec<_> = enrichment
        .causes
        .into_iter()
        .map(|cause| cause.error)
        .collect();
    let response = IncidentShowResponse {
        incident: &incident,
        exception_error: enrichment.exception.as_ref(),
        error_causes: &causes,
    };
    output::write_with(response, format, stdout, |w| {
        render_incident_detail(w, &incident, enrichment.exception.as_ref(), &causes)
    })
}

/// Update an incident (state, severity, notification frequency, assignees, description).
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
    notification_frequency: Option<IncidentNotificationFrequency>,
    notification_threshold: Option<i64>,
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
            || notification_frequency.is_some()
            || notification_threshold.is_some()
            || assign.is_some()
            || assign_me
            || unassign.is_some()
            || description.is_some()
        {
            anyhow::bail!(
                "Bulk incident updates currently support only `--state`. Use a single `--number` for severity, notification settings, assignee, or description changes."
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
            notification_frequency,
            notification_threshold,
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
        |w| render_incident_detail(w, &incident, None, &[]),
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

/// List notes on an incident.
pub async fn list_notes(
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
    let notes = client
        .list_incident_notes(&resolved_app_id, incident_number)
        .await?;

    output::print_with(
        IncidentNotesResponse {
            incident_number,
            notes: &notes,
        },
        format,
        |w| render_incident_notes(w, &notes),
    )
}

/// Update a note on an incident.
#[allow(clippy::too_many_arguments)]
pub async fn update_note(
    incident_number: i64,
    note_id: &str,
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
    let incident = client
        .get_incident(&resolved_app_id, incident_number)
        .await?;

    client
        .update_incident_note(&resolved_app_id, incident.id(), note_id, content)
        .await?;

    output::print_with(
        IncidentNoteResponse {
            incident_number,
            message: format!("Note updated on incident #{}.", incident_number),
        },
        format,
        |w| writeln!(w, "Note updated on incident #{}.", incident_number),
    )
}

/// Delete a note from an incident.
pub async fn delete_note(
    incident_number: i64,
    note_id: &str,
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

    client
        .delete_incident_note(&resolved_app_id, incident.id(), note_id)
        .await?;

    output::print_with(
        IncidentNoteResponse {
            incident_number,
            message: format!("Note deleted from incident #{}.", incident_number),
        },
        format,
        |w| writeln!(w, "Note deleted from incident #{}.", incident_number),
    )
}

fn render_incident_notes(w: &mut dyn Write, notes: &[IncidentNote]) -> io::Result<()> {
    if notes.is_empty() {
        return writeln!(w, "No notes found.");
    }

    let rows = notes.iter().map(|note| IncidentNoteRow {
        id: note.id.clone(),
        author: note
            .author
            .as_ref()
            .and_then(|author| author.name.clone())
            .unwrap_or_else(|| "-".to_string()),
        source: note.via.clone().unwrap_or_else(|| "-".to_string()),
        can_edit: if note.viewer_can_edit { "yes" } else { "no" },
        can_delete: if note.viewer_can_delete { "yes" } else { "no" },
        updated_at: note
            .updated_at
            .as_deref()
            .or(note.created_at.as_deref())
            .unwrap_or("-")
            .to_string(),
        content: output::truncate(&note.content.replace('\n', " "), 60),
    });

    output::table(w, rows)?;
    writeln!(w, "{} note(s) found.", notes.len())
}

fn render_exception_table(w: &mut dyn Write, incidents: &[Incident]) -> io::Result<()> {
    if incidents.is_empty() {
        return writeln!(w, "No exception incidents found.");
    }

    let rows = incidents.iter().map(|incident| {
        let exception = if let Incident::ExceptionIncident { exception_name, .. } = incident {
            exception_name.as_deref().unwrap_or("-")
        } else {
            "-"
        };
        ExceptionIncidentRow {
            number: incident.number(),
            state: incident.state().to_string(),
            severity: incident.severity().to_string(),
            count: incident.count(),
            last_occurred: incident.last_occurred_at().to_string(),
            exception: output::truncate(exception, 50),
        }
    });

    output::table(w, rows)?;

    writeln!(w, "{} exception incident(s) found.", incidents.len())
}

fn render_performance_table(w: &mut dyn Write, incidents: &[Incident]) -> io::Result<()> {
    if incidents.is_empty() {
        return writeln!(w, "No performance incidents found.");
    }

    let rows = incidents.iter().map(|incident| {
        let action = if let Incident::PerformanceIncident { action_names, .. } = incident {
            action_names
                .as_ref()
                .and_then(|names| names.first())
                .map(|s| s.as_str())
                .unwrap_or("-")
        } else {
            "-"
        };
        PerformanceIncidentRow {
            number: incident.number(),
            state: incident.state().to_string(),
            severity: incident.severity().to_string(),
            count: incident.count(),
            last_occurred: incident.last_occurred_at().to_string(),
            action: output::truncate(action, 50),
        }
    });

    output::table(w, rows)?;

    writeln!(w, "{} performance incident(s) found.", incidents.len())
}

fn render_incident_table(w: &mut dyn Write, incidents: &[Incident]) -> io::Result<()> {
    if incidents.is_empty() {
        return writeln!(w, "No incidents found.");
    }

    let rows = incidents.iter().map(|incident| IncidentRow {
        number: incident.number(),
        kind: incident.kind().to_string(),
        state: incident.state().to_string(),
        severity: incident.severity().to_string(),
        count: incident.count(),
        last_occurred: incident.last_occurred_at().to_string(),
        description: output::truncate(incident.description(), 40),
    });

    output::table(w, rows)?;

    writeln!(w, "{} incident(s) found.", incidents.len())
}

fn render_anomaly_table(w: &mut dyn Write, incidents: &[Incident]) -> io::Result<()> {
    if incidents.is_empty() {
        return writeln!(w, "No anomaly incidents found.");
    }

    let rows = incidents.iter().map(|incident| {
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
        AnomalyIncidentRow {
            number: incident.number(),
            state: incident.state().to_string(),
            severity: incident.severity().to_string(),
            alert: alert_state.to_string(),
            count: incident.count(),
            last_occurred: incident.last_occurred_at().to_string(),
            trigger: output::truncate(trigger_name, 40),
        }
    });

    output::table(w, rows)?;

    writeln!(w, "{} anomaly incident(s) found.", incidents.len())
}

fn render_error_causes(w: &mut dyn Write, error_causes: &[ExceptionErrorView]) -> io::Result<()> {
    if error_causes.is_empty() {
        return Ok(());
    }

    writeln!(w, "Error causes:")?;
    for (index, cause) in error_causes.iter().enumerate() {
        let details = [cause.message.as_deref(), cause.first_line.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" - ");

        if details.is_empty() {
            writeln!(w, "  {}. {}", index + 1, cause.name)?;
        } else {
            writeln!(w, "  {}. {}: {}", index + 1, cause.name, details)?;
        }
    }

    Ok(())
}

fn render_incident_detail(
    w: &mut dyn Write,
    incident: &Incident,
    exception_error: Option<&ExceptionErrorView>,
    error_causes: &[ExceptionErrorView],
) -> io::Result<()> {
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
            let rendered_exception = exception_error
                .map(|error| error.name.as_str())
                .or(exception_name.as_deref())
                .unwrap_or("-");
            let rendered_message = exception_error
                .and_then(|error| error.message.as_deref())
                .or(exception_message.as_deref())
                .unwrap_or("-");
            writeln!(w, "Exception:      {}", rendered_exception)?;
            writeln!(w, "Message:        {}", rendered_message)?;
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
            render_error_causes(w, error_causes)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn mount_show_incident(server: &MockServer, incident: &Incident) {
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("query AppIncident("))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"data": {"app": {"incident": incident}}})),
            )
            .expect(1)
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn show_preserves_enriched_human_and_json_output() {
        for format in [Output::Human, Output::Json] {
            let server = MockServer::start().await;
            let mut incident = exception_incident();
            if let Incident::ExceptionIncident { digests, .. } = &mut incident {
                *digests = Some(vec!["digest".into()]);
            }
            mount_show_incident(&server, &incident).await;
            Mock::given(path("/api/v2/tracing/traces/errors"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(json!([{"trace_id": "trace", "span_id": "span"}])),
                )
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(path("/api/v2/tracing/trace/error"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                    "trace_id": "trace", "span_id": "span",
                    "events.name": ["exception", "exception"],
                    "events.attributes": [
                        {"appsignal.incident_digest": "digest", "exception.type": "LatestError", "exception.message": "fresh message", "appsignal.exception.stacktrace_index": "0_0"},
                        {"exception.type": "CauseError", "exception.message": "cause message", "appsignal.exception.stacktrace_index": "0_1", "appsignal.stacktrace_id": "stack"}
                    ]
                }])))
                .expect(1).mount(&server).await;
            Mock::given(path("/graphql")).and(body_string_contains("query IncidentCauseBacktrace("))
                .and(body_partial_json(json!({"variables": {"appId": "app", "id": "stack", "revision": null}})))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {"app": {"backtrace": [{"type": "app", "original": "app.rb:42"}]}}})))
                .expect(1).mount(&server).await;
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            show_with_client(
                &AppSignalClient::with_endpoint("tok", &server.uri()),
                "app",
                7,
                format,
                &mut stdout,
                &mut stderr,
            )
            .await
            .unwrap();
            assert!(stderr.is_empty());
            match format {
                Output::Json => {
                    let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
                    assert_eq!(
                        value["incident"]["exceptionMessage"],
                        "stale incident message"
                    );
                    assert_eq!(
                        value["exception_error"],
                        json!({"name": "LatestError", "message": "fresh message"})
                    );
                    assert_eq!(
                        value["error_causes"],
                        json!([{"name": "CauseError", "message": "cause message", "first_line": "app.rb:42"}])
                    );
                }
                Output::Human => {
                    let output = String::from_utf8(stdout).unwrap();
                    assert!(output.contains("Exception:      LatestError"));
                    assert!(output.contains("Message:        fresh message"));
                    assert!(output.contains("CauseError: cause message - app.rb:42"));
                    assert!(!output.contains("stale incident message"));
                }
            }
            for request in server.received_requests().await.unwrap() {
                if request.url.path() == "/graphql" {
                    assert!(!request.body_json::<serde_json::Value>().unwrap()["query"]
                        .as_str()
                        .unwrap()
                        .contains("sample"));
                }
            }
        }
    }

    #[tokio::test]
    async fn show_optional_failures_warn_only_on_stderr_and_still_succeed() {
        for format in [Output::Human, Output::Json] {
            let server = MockServer::start().await;
            let mut incident = exception_incident();
            if let Incident::ExceptionIncident { digests, .. } = &mut incident {
                *digests = Some(vec!["digest".into()]);
            }
            mount_show_incident(&server, &incident).await;
            Mock::given(path("/api/v2/tracing/traces/errors"))
                .respond_with(
                    ResponseTemplate::new(403).set_body_string("sensitive server details"),
                )
                .expect(1)
                .mount(&server)
                .await;
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            show_with_client(
                &AppSignalClient::with_endpoint("tok", &server.uri()),
                "app",
                7,
                format,
                &mut stdout,
                &mut stderr,
            )
            .await
            .unwrap();
            let warning = String::from_utf8(stderr).unwrap();
            assert_eq!(warning.lines().count(), 1);
            assert!(warning.starts_with("Warning: "));
            assert!(warning.contains("authentication rejected"));
            assert!(!warning.contains("sensitive"));
            if matches!(format, Output::Json) {
                let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
                assert!(value.get("exception_error").is_none());
                assert!(value.get("error_causes").is_none());
                assert_eq!(value["incident"]["number"], 7);
            } else {
                assert!(String::from_utf8(stdout)
                    .unwrap()
                    .contains("stale incident message"));
            }
        }
    }

    #[tokio::test]
    async fn show_non_exception_does_not_enrich_and_main_fetch_errors_remain_fatal() {
        let server = MockServer::start().await;
        let incident: Incident = serde_json::from_value(json!({
            "__typename": "PerformanceIncident", "id": "p", "number": 8, "count": 1
        }))
        .unwrap();
        mount_show_incident(&server, &incident).await;
        let client = AppSignalClient::with_endpoint("tok", &server.uri());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        show_with_client(&client, "app", 8, Output::Json, &mut stdout, &mut stderr)
            .await
            .unwrap();
        assert!(stderr.is_empty());
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
        server.reset().await;
        Mock::given(path("/graphql"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        stdout.clear();
        assert!(
            show_with_client(&client, "app", 8, Output::Json, &mut stdout, &mut stderr)
                .await
                .is_err()
        );
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
    }

    fn exception_incident() -> Incident {
        Incident::ExceptionIncident {
            id: "exc-1".to_string(),
            number: 7,
            state: Some("OPEN".to_string()),
            severity: Some("CRITICAL".to_string()),
            description: Some("Something broke".to_string()),
            count: 3,
            created_at: Some("2026-07-01T00:00:00Z".to_string()),
            last_occurred_at: Some("2026-07-02T00:00:00Z".to_string()),
            updated_at: None,
            exception_name: Some("RuntimeError".to_string()),
            exception_message: Some("stale incident message".to_string()),
            action_names: Some(vec!["UsersController#show".to_string()]),
            namespace: Some("web".to_string()),
            first_backtrace_line: Some("app/models/user.rb:10".to_string()),
            digests: Some(vec!["digest-1".to_string()]),
            assignees: None,
        }
    }

    #[test]
    fn render_incident_table_uses_shared_table_format() {
        let incidents = vec![Incident::LogIncident {
            id: "log-1".to_string(),
            number: 42,
            state: Some("OPEN".to_string()),
            severity: None,
            description: Some("Log matched".to_string()),
            count: 3,
            created_at: None,
            last_occurred_at: Some("2026-06-22T10:00:00Z".to_string()),
            updated_at: None,
            assignees: None,
        }];
        let mut buf = Vec::new();

        render_incident_table(&mut buf, &incidents).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("+"));
        assert!(output.contains("| # "));
        assert!(output.contains("| TYPE"));
        assert!(output.contains("42"));
        assert!(output.contains("1 incident(s) found."));
    }

    #[test]
    fn render_incident_detail_uses_sample_error_and_causes() {
        let incident = exception_incident();
        let exception_error = Some(ExceptionErrorView {
            name: "NoMethodError".to_string(),
            message: Some("fresher sample message".to_string()),
            first_line: None,
        });
        let error_causes = vec![ExceptionErrorView {
            name: "ArgumentError".to_string(),
            message: Some("argument out of range".to_string()),
            first_line: Some("app/models/report.rb:42".to_string()),
        }];
        let mut buf = Vec::new();

        render_incident_detail(&mut buf, &incident, exception_error.as_ref(), &error_causes)
            .unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Exception:      NoMethodError"));
        assert!(output.contains("Message:        fresher sample message"));
        assert!(!output.contains("stale incident message"));
        assert!(output.contains("Error causes:"));
        assert!(output.contains("ArgumentError: argument out of range - app/models/report.rb:42"));
    }

    #[test]
    fn render_incident_detail_falls_back_to_incident_error() {
        let incident = exception_incident();
        let mut buf = Vec::new();

        render_incident_detail(&mut buf, &incident, None, &[]).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Exception:      RuntimeError"));
        assert!(output.contains("Message:        stale incident message"));
        assert!(!output.contains("Error causes:"));
    }

    #[test]
    fn render_incident_notes_includes_ids_and_content() {
        let notes = vec![IncidentNote {
            id: "note-1".to_string(),
            content: "Investigated the failure".to_string(),
            created_at: Some("2026-07-22T10:00:00Z".to_string()),
            updated_at: None,
            viewer_can_edit: true,
            viewer_can_delete: true,
            via: Some("cli".to_string()),
            author: Some(crate::api::User {
                id: "user-1".to_string(),
                name: Some("Ada".to_string()),
                email: None,
            }),
        }];
        let mut buf = Vec::new();

        render_incident_notes(&mut buf, &notes).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("note-1"));
        assert!(output.contains("Ada"));
        assert!(output.contains("cli"));
        assert!(output.contains("CAN EDIT"));
        assert!(output.contains("CAN DELETE"));
        assert!(output.contains("Investigated the failure"));
        assert!(output.contains("1 note(s) found."));

        let json = serde_json::to_value(IncidentNotesResponse {
            incident_number: 42,
            notes: &notes,
        })
        .unwrap();
        assert_eq!(json["notes"][0]["viewerCanEdit"], true);
        assert_eq!(json["notes"][0]["viewerCanDelete"], true);
    }
}
