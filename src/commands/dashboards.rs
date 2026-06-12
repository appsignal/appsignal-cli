use std::io::{self, Write};

use anyhow::Result;
use serde::Serialize;
use tabled::Tabled;

use super::{authenticated_client, resolve_org};
use crate::api::Dashboard;
use crate::config::Config;
use crate::output::{self, Output};

#[derive(Serialize)]
struct DashboardListResponse<'a> {
    dashboards: &'a [Dashboard],
}

#[derive(Serialize)]
struct DashboardResponse<'a> {
    dashboard: &'a Dashboard,
}

#[derive(Tabled)]
struct DashboardRow<'a> {
    #[tabled(rename = "ID")]
    id: &'a str,
    #[tabled(rename = "TITLE")]
    title: &'a str,
    #[tabled(rename = "DESCRIPTION")]
    description: &'a str,
}

pub async fn list(
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

    let resources = client
        .get_app_resources(&resolved_app_id, &["dashboards".to_string()])
        .await?;
    let dashboards = resources.dashboards.unwrap_or_default();

    output::print_with(
        DashboardListResponse {
            dashboards: &dashboards,
        },
        format,
        |w| render_dashboard_table(w, &dashboards),
    )
}

pub async fn create(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    title: &str,
    description: Option<&str>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let dashboard = client
        .create_dashboard(&resolved_app_id, title, description)
        .await?;

    crate::status!("Dashboard {} created.", dashboard.id);
    output::print_with(
        DashboardResponse {
            dashboard: &dashboard,
        },
        format,
        |w| render_dashboard_detail(w, &dashboard),
    )
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    dashboard_id: &str,
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    title: &str,
    description: Option<&str>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let dashboard = client
        .update_dashboard(&resolved_app_id, dashboard_id, title, description)
        .await?;

    crate::status!("Dashboard {} updated.", dashboard_id);
    output::print_with(
        DashboardResponse {
            dashboard: &dashboard,
        },
        format,
        |w| render_dashboard_detail(w, &dashboard),
    )
}

fn render_dashboard_detail(w: &mut dyn Write, dashboard: &Dashboard) -> io::Result<()> {
    output::detail(
        w,
        &[
            ("ID", dashboard.id.as_str()),
            ("Title", dashboard.title.as_deref().unwrap_or("-")),
            (
                "Description",
                dashboard.description.as_deref().unwrap_or("-"),
            ),
            ("Label", dashboard.label.as_deref().unwrap_or("-")),
            ("Source", dashboard.source.as_deref().unwrap_or("-")),
            ("Created at", dashboard.created_at.as_deref().unwrap_or("-")),
            ("Updated at", dashboard.updated_at.as_deref().unwrap_or("-")),
        ],
    )
}

fn render_dashboard_table(w: &mut dyn Write, dashboards: &[Dashboard]) -> io::Result<()> {
    if dashboards.is_empty() {
        return writeln!(w, "No dashboards found.");
    }

    let rows = dashboards.iter().map(|dashboard| DashboardRow {
        id: &dashboard.id,
        title: dashboard.title.as_deref().unwrap_or("-"),
        description: dashboard.description.as_deref().unwrap_or("-"),
    });

    output::table(w, rows)?;
    writeln!(w, "{} dashboard(s) found.", dashboards.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::AppResources;

    fn sample_resources() -> AppResources {
        AppResources {
            dashboards: Some(vec![sample_dashboard()]),
            ..AppResources::default()
        }
    }

    fn sample_dashboard() -> Dashboard {
        Dashboard {
            id: "dash-1".to_string(),
            title: Some("Overview".to_string()),
            description: Some("Main dashboard".to_string()),
            label: Some("beta".to_string()),
            source: Some("USER_CREATED".to_string()),
            created_at: Some("2026-06-12T10:00:00Z".to_string()),
            updated_at: Some("2026-06-12T11:00:00Z".to_string()),
        }
    }

    #[test]
    fn render_dashboard_detail_includes_management_fields() {
        let dashboard = sample_dashboard();

        let mut buf = Vec::new();
        render_dashboard_detail(&mut buf, &dashboard).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("ID:"));
        assert!(output.contains("Overview"));
        assert!(output.contains("Main dashboard"));
        assert!(output.contains("USER_CREATED"));
        assert!(output.contains("2026-06-12T11:00:00Z"));
    }

    #[test]
    fn render_dashboard_table_shows_table_and_count() {
        let resources = sample_resources();
        let dashboards = resources.dashboards.as_ref().unwrap();

        let mut buf = Vec::new();
        render_dashboard_table(&mut buf, dashboards).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("| ID"));
        assert!(output.contains("Overview"));
        assert!(output.contains("Main dashboard"));
        assert!(output.contains("1 dashboard(s) found."));
    }

    #[test]
    fn render_dashboard_table_handles_empty_state() {
        let mut buf = Vec::new();
        render_dashboard_table(&mut buf, &[]).unwrap();

        assert_eq!(String::from_utf8(buf).unwrap(), "No dashboards found.\n");
    }

    #[test]
    fn dashboard_response_serializes_nested_shape() {
        let dashboard = sample_dashboard();
        let response = DashboardResponse {
            dashboard: &dashboard,
        };

        let json = serde_json::to_value(&response).unwrap();

        assert_eq!(json["dashboard"]["id"], "dash-1");
        assert_eq!(json["dashboard"]["title"], "Overview");
        assert_eq!(json["dashboard"]["source"], "USER_CREATED");
    }

    #[test]
    fn dashboard_list_response_serializes_nested_shape() {
        let resources = sample_resources();
        let dashboards = resources.dashboards.as_ref().unwrap();
        let response = DashboardListResponse { dashboards };

        let json = serde_json::to_value(&response).unwrap();

        assert_eq!(json["dashboards"][0]["id"], "dash-1");
        assert_eq!(json["dashboards"][0]["title"], "Overview");
    }
}
