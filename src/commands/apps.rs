use std::io::{self, Write};

use anyhow::Result;
use serde::Serialize;
use tabled::Tabled;

use super::{authenticated_client, resolve_org};
use crate::api::{App, AppResources, ViewerOrganization};
use crate::config::Config;
use crate::output::{self, Output, Render};

/// Response shape for `apps list`. Owns its JSON representation.
#[derive(Serialize)]
pub struct AppListing {
    pub apps: Vec<App>,
}

#[derive(Serialize)]
struct AppResponse<'a> {
    app: &'a App,
}

#[derive(Serialize)]
struct OrganizationStatus<'a> {
    org: Option<&'a str>,
    message: String,
}

#[derive(Serialize)]
struct OrganizationListing<'a> {
    organizations: &'a [ViewerOrganization],
}

#[derive(Serialize)]
struct AppResourcesResponse<'a> {
    resources: &'a AppResources,
}

#[derive(Tabled)]
struct UserRow<'a> {
    #[tabled(rename = "ID")]
    id: &'a str,
    #[tabled(rename = "NAME")]
    name: &'a str,
    #[tabled(rename = "EMAIL")]
    email: &'a str,
}

#[derive(Tabled)]
struct NotifierRow<'a> {
    #[tabled(rename = "ID")]
    id: &'a str,
    #[tabled(rename = "NAME")]
    name: &'a str,
}

#[derive(Tabled)]
struct NamespaceRow<'a> {
    #[tabled(rename = "ID")]
    id: &'a str,
    #[tabled(rename = "NAME")]
    name: &'a str,
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

#[derive(Tabled)]
struct DeployMarkerRow<'a> {
    #[tabled(rename = "ID")]
    id: &'a str,
    #[tabled(rename = "REVISION")]
    revision: &'a str,
    #[tabled(rename = "CREATED AT")]
    created_at: &'a str,
    #[tabled(rename = "ERRORS")]
    errors: String,
    #[tabled(rename = "USER")]
    user: &'a str,
}

/// Table view of a single app. Kept separate from `App` so the JSON payload
/// and the human columns can evolve independently.
#[derive(Tabled)]
struct AppRow<'a> {
    #[tabled(rename = "ID")]
    id: &'a str,
    #[tabled(rename = "NAME")]
    name: &'a str,
    #[tabled(rename = "ENVIRONMENT")]
    environment: &'a str,
}

#[derive(Tabled)]
struct OrganizationRow<'a> {
    #[tabled(rename = "SLUG")]
    slug: &'a str,
    #[tabled(rename = "NAME")]
    name: &'a str,
}

impl Render for AppListing {
    fn render_human(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.apps.is_empty() {
            return writeln!(w, "No applications found.");
        }
        let rows = self.apps.iter().map(|a| AppRow {
            id: &a.id,
            name: a.name.as_deref().unwrap_or("-"),
            environment: a.environment.as_deref().unwrap_or("-"),
        });
        output::table(w, rows)?;
        writeln!(w, "{} application(s) found.", self.apps.len())
    }
}

impl Render for AppResourcesResponse<'_> {
    fn render_human(&self, w: &mut dyn Write) -> io::Result<()> {
        let resources = self.resources;
        let mut wrote_section = false;

        if let Some(users) = &resources.users {
            writeln!(w, "Users:")?;
            let rows = users.iter().map(|user| UserRow {
                id: &user.id,
                name: user.name.as_deref().unwrap_or("-"),
                email: user.email.as_deref().unwrap_or("-"),
            });
            output::table(w, rows)?;
            wrote_section = true;
        }

        if let Some(notifiers) = &resources.notifiers {
            if wrote_section {
                writeln!(w)?;
            }
            writeln!(w, "Notifiers:")?;
            let rows = notifiers.iter().map(|notifier| NotifierRow {
                id: &notifier.id,
                name: notifier.name.as_deref().unwrap_or("-"),
            });
            output::table(w, rows)?;
            wrote_section = true;
        }

        if let Some(namespaces) = &resources.namespaces {
            if wrote_section {
                writeln!(w)?;
            }
            writeln!(w, "Namespaces:")?;
            let rows = namespaces.iter().map(|namespace| NamespaceRow {
                id: &namespace.id,
                name: &namespace.name,
            });
            output::table(w, rows)?;
            wrote_section = true;
        }

        if let Some(dashboards) = &resources.dashboards {
            if wrote_section {
                writeln!(w)?;
            }
            writeln!(w, "Dashboards:")?;
            let rows = dashboards.iter().map(|dashboard| DashboardRow {
                id: &dashboard.id,
                title: dashboard.title.as_deref().unwrap_or("-"),
                description: dashboard.description.as_deref().unwrap_or("-"),
            });
            output::table(w, rows)?;
            wrote_section = true;
        }

        if let Some(deploy_markers) = &resources.deploy_markers {
            if wrote_section {
                writeln!(w)?;
            }
            writeln!(w, "Deploy markers:")?;
            let rows = deploy_markers.iter().map(|marker| DeployMarkerRow {
                id: &marker.id,
                revision: marker.short_revision.as_deref().unwrap_or("-"),
                created_at: marker.created_at.as_deref().unwrap_or("-"),
                errors: marker
                    .exception_count
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                user: marker.user.as_deref().unwrap_or("-"),
            });
            output::table(w, rows)?;
        }

        Ok(())
    }
}

/// List all applications in an organization (and save the org slug to config).
pub async fn list(org_slug: &str, format: Output) -> Result<()> {
    let mut config = Config::load()?;
    let client = authenticated_client(&mut config).await?;

    let apps = client.list_apps(org_slug).await?;

    config.org = Some(org_slug.to_string());
    config.save()?;

    output::print(&AppListing { apps }, format)
}

/// Show details for a specific application.
pub async fn info(app_id: &str, format: Output) -> Result<()> {
    let mut config = Config::load()?;
    let client = authenticated_client(&mut config).await?;

    let app = client.get_app(app_id).await?;

    output::print_with(AppResponse { app: &app }, format, |w| {
        output::detail(
            w,
            &[
                ("ID", app.id.as_str()),
                ("Name", app.name.as_deref().unwrap_or("-")),
                ("Environment", app.environment.as_deref().unwrap_or("-")),
            ],
        )
    })
}

/// Find an application by name and optional environment.
pub async fn find(
    name: &str,
    environment: Option<&str>,
    org: Option<&str>,
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let app = client.find_app(&org_slug, name, environment).await?;

    output::print_with(AppResponse { app: &app }, format, |w| {
        output::detail(
            w,
            &[
                ("ID", app.id.as_str()),
                ("Name", app.name.as_deref().unwrap_or("-")),
                ("Environment", app.environment.as_deref().unwrap_or("-")),
            ],
        )
    })
}

/// Set the default organization slug.
pub async fn set_org(org_slug: &str, format: Output) -> Result<()> {
    let mut config = Config::load()?;
    let client = authenticated_client(&mut config).await?;

    let _apps = client.list_apps(org_slug).await?;

    config.org = Some(org_slug.to_string());
    config.save()?;
    output::print_with(
        OrganizationStatus {
            org: Some(org_slug),
            message: format!("Default organization set to '{}'.", org_slug),
        },
        format,
        |w| writeln!(w, "Default organization set to '{}'.", org_slug),
    )
}

/// Show current default organization.
pub fn show_org(format: Output) -> Result<()> {
    let config = Config::load()?;
    let org = config.org.as_deref().filter(|o| !o.is_empty());
    let message = match org {
        Some(org) => format!("Default organization: {}", org),
        None => "No default organization set.".to_string(),
    };
    let human_message = message.clone();
    output::print_with(OrganizationStatus { org, message }, format, move |w| {
        writeln!(w, "{}", human_message)
    })
}

/// List all organizations the authenticated user has access to.
pub async fn orgs(format: Output) -> Result<()> {
    let mut config = Config::load()?;
    let client = authenticated_client(&mut config).await?;

    let orgs = client.list_organizations().await?;

    output::print_with(
        OrganizationListing {
            organizations: &orgs,
        },
        format,
        |w| {
            if orgs.is_empty() {
                return writeln!(w, "No organizations found.");
            }
            let rows = orgs.iter().map(|org| OrganizationRow {
                slug: &org.slug,
                name: &org.name,
            });
            output::table(w, rows)?;
            writeln!(w, "{} organization(s) found.", orgs.len())
        },
    )
}

/// Show resources for an application (users, notifiers, namespaces, dashboards, deploy markers).
pub async fn resources(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    sections: &[&str],
    format: Output,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let section_list: Vec<String> = sections.iter().map(|section| section.to_string()).collect();

    let resources = client
        .get_app_resources(&resolved_app_id, &section_list)
        .await?;

    output::print(
        &AppResourcesResponse {
            resources: &resources,
        },
        format,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Dashboard, DashboardSource, DeployMarker, Namespace, Notifier, User};

    #[test]
    fn app_resources_render_human_uses_tables_for_sections() {
        let resources = AppResources {
            users: Some(vec![User {
                id: "user-1".to_string(),
                name: Some("Ada".to_string()),
                email: Some("ada@example.com".to_string()),
            }]),
            notifiers: Some(vec![Notifier {
                id: "notifier-1".to_string(),
                name: Some("Slack".to_string()),
                icon: None,
            }]),
            namespaces: Some(vec![Namespace {
                id: "namespace-1".to_string(),
                name: "web".to_string(),
            }]),
            dashboards: Some(vec![Dashboard {
                id: "dashboard-1".to_string(),
                title: Some("Overview".to_string()),
                description: Some("Main dashboard".to_string()),
                label: None,
                source: Some(DashboardSource::UserCreated),
                created_at: None,
                updated_at: None,
            }]),
            deploy_markers: Some(vec![DeployMarker {
                id: "marker-1".to_string(),
                created_at: Some("2026-05-13T12:00:00Z".to_string()),
                short_revision: Some("abc123".to_string()),
                revision: None,
                git_compare_url: None,
                user: Some("jeroen".to_string()),
                live_for_in_words: None,
                live_for: None,
                exception_count: Some(2),
                exception_rate: None,
            }]),
        };
        let response = AppResourcesResponse {
            resources: &resources,
        };

        let mut buf = Vec::new();
        response.render_human(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Users:"));
        assert!(output.contains("| ID"));
        assert!(output.contains("ada@example.com"));
        assert!(output.contains("Notifiers:"));
        assert!(output.contains("Namespaces:"));
        assert!(output.contains("Dashboards:"));
        assert!(output.contains("Main dashboard"));
        assert!(output.contains("Deploy markers:"));
        assert!(output.contains("abc123"));
    }

    #[test]
    fn app_resources_response_serializes_nested_resources_shape() {
        let resources = AppResources {
            users: Some(vec![User {
                id: "user-1".to_string(),
                name: Some("Ada".to_string()),
                email: Some("ada@example.com".to_string()),
            }]),
            ..AppResources::default()
        };
        let response = AppResourcesResponse {
            resources: &resources,
        };

        let json = serde_json::to_value(&response).unwrap();

        assert_eq!(json["resources"]["users"][0]["id"], "user-1");
        assert_eq!(json["resources"]["users"][0]["name"], "Ada");
    }
}
