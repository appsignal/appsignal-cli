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

    output::print_with(
        AppResourcesResponse {
            resources: &resources,
        },
        format,
        |w| {
            if let Some(users) = &resources.users {
                writeln!(w, "Users:")?;
                for user in users {
                    writeln!(
                        w,
                        "  {}  {}  {}",
                        user.id,
                        user.name.as_deref().unwrap_or("-"),
                        user.email.as_deref().unwrap_or("-")
                    )?;
                }
                writeln!(w)?;
            }

            if let Some(notifiers) = &resources.notifiers {
                writeln!(w, "Notifiers:")?;
                for notifier in notifiers {
                    writeln!(
                        w,
                        "  {}  {}",
                        notifier.id,
                        notifier.name.as_deref().unwrap_or("-")
                    )?;
                }
                writeln!(w)?;
            }

            if let Some(namespaces) = &resources.namespaces {
                writeln!(w, "Namespaces:")?;
                for ns in namespaces {
                    writeln!(w, "  {}", ns.name)?;
                }
                writeln!(w)?;
            }

            if let Some(dashboards) = &resources.dashboards {
                writeln!(w, "Dashboards:")?;
                for dashboard in dashboards {
                    writeln!(
                        w,
                        "  {}  {}",
                        dashboard.id,
                        dashboard.title.as_deref().unwrap_or("-")
                    )?;
                }
                writeln!(w)?;
            }

            if let Some(deploy_markers) = &resources.deploy_markers {
                writeln!(w, "Deploy markers:")?;
                writeln!(
                    w,
                    "  {:<28} {:<12} {:<20} {:<10} USER",
                    "ID", "REVISION", "CREATED AT", "ERRORS"
                )?;
                writeln!(w, "  {}", "-".repeat(90))?;
                for marker in deploy_markers {
                    writeln!(
                        w,
                        "  {:<28} {:<12} {:<20} {:<10} {}",
                        marker.id,
                        marker.short_revision.as_deref().unwrap_or("-"),
                        marker.created_at.as_deref().unwrap_or("-"),
                        marker
                            .exception_count
                            .map(|count| count.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        marker.user.as_deref().unwrap_or("-"),
                    )?;
                }
            }

            Ok(())
        },
    )
}
