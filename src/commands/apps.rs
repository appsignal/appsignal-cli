use anyhow::Result;

use super::{authenticated_client, resolve_org};
use crate::config::Config;

/// List all applications in an organization (and save the org slug to config).
pub async fn list(org_slug: &str) -> Result<()> {
    let mut config = Config::load()?;
    let client = authenticated_client(&mut config).await?;

    let apps = client.list_apps(org_slug).await?;

    // Save the org slug so future commands don't need it
    config.org = Some(org_slug.to_string());
    config.save()?;

    if apps.is_empty() {
        println!("No applications found.");
        return Ok(());
    }

    println!("{:<28} {:<30} ENVIRONMENT", "ID", "NAME");
    println!("{}", "-".repeat(73));

    for app in &apps {
        println!(
            "{:<28} {:<30} {}",
            app.id,
            app.name.as_deref().unwrap_or("-"),
            app.environment.as_deref().unwrap_or("-"),
        );
    }

    println!("\n{} application(s) found.", apps.len());
    Ok(())
}

/// Show details for a specific application.
pub async fn info(app_id: &str) -> Result<()> {
    let mut config = Config::load()?;
    let client = authenticated_client(&mut config).await?;

    let app = client.get_app(app_id).await?;

    println!("Application details:");
    println!("  ID:          {}", app.id);
    println!("  Name:        {}", app.name.as_deref().unwrap_or("-"));
    println!(
        "  Environment: {}",
        app.environment.as_deref().unwrap_or("-")
    );

    Ok(())
}

/// Find an application by name and optional environment.
pub async fn find(name: &str, environment: Option<&str>, org: Option<&str>) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let app = client.find_app(&org_slug, name, environment).await?;

    println!("Application details:");
    println!("  ID:          {}", app.id);
    println!("  Name:        {}", app.name.as_deref().unwrap_or("-"));
    println!(
        "  Environment: {}",
        app.environment.as_deref().unwrap_or("-")
    );

    Ok(())
}

/// Set the default organization slug.
pub async fn set_org(org_slug: &str) -> Result<()> {
    let mut config = Config::load()?;
    let client = authenticated_client(&mut config).await?;

    // Validate the org exists by listing apps
    let _apps = client.list_apps(org_slug).await?;

    config.org = Some(org_slug.to_string());
    config.save()?;
    println!("Default organization set to '{}'.", org_slug);
    Ok(())
}

/// Show current default organization.
pub fn show_org() -> Result<()> {
    let config = Config::load()?;
    match config.org.as_deref().filter(|o| !o.is_empty()) {
        Some(org) => println!("Default organization: {}", org),
        None => println!("No default organization set."),
    }
    Ok(())
}

/// List all organizations the authenticated user has access to.
pub async fn orgs() -> Result<()> {
    let mut config = Config::load()?;
    let client = authenticated_client(&mut config).await?;

    let orgs = client.list_organizations().await?;

    if orgs.is_empty() {
        println!("No organizations found.");
        return Ok(());
    }

    println!("{:<40} NAME", "SLUG");
    println!("{}", "-".repeat(60));

    for org in &orgs {
        println!("{:<40} {}", org.slug, org.name);
    }

    println!("\n{} organization(s) found.", orgs.len());
    Ok(())
}

/// Show resources for an application (users, notifiers, namespaces, dashboards).
pub async fn resources(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    sections: Option<&str>,
) -> Result<()> {
    let mut config = Config::load()?;
    let org_slug = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;

    let resolved_app_id = client
        .resolve_app_id(&org_slug, app_id, app_name, environment)
        .await?;

    let section_list: Vec<String> = sections
        .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
        .unwrap_or_default();

    let resources = client
        .get_app_resources(&resolved_app_id, &section_list)
        .await?;

    if let Some(users) = &resources.users {
        println!("Users:");
        println!("  {:<28} {:<25} EMAIL", "ID", "NAME");
        println!("  {}", "-".repeat(80));
        for user in users {
            println!(
                "  {:<28} {:<25} {}",
                user.id,
                user.name.as_deref().unwrap_or("-"),
                user.email.as_deref().unwrap_or("-"),
            );
        }
        println!();
    }

    if let Some(notifiers) = &resources.notifiers {
        println!("Notifiers:");
        println!("  {:<28} NAME", "ID");
        println!("  {}", "-".repeat(60));
        for notifier in notifiers {
            println!(
                "  {:<28} {}",
                notifier.id,
                notifier.name.as_deref().unwrap_or("-"),
            );
        }
        println!();
    }

    if let Some(namespaces) = &resources.namespaces {
        println!("Namespaces:");
        for ns in namespaces {
            println!("  {}", ns);
        }
        println!();
    }

    if let Some(dashboards) = &resources.dashboards {
        println!("Dashboards:");
        println!("  {:<28} TITLE", "ID");
        println!("  {}", "-".repeat(60));
        for dashboard in dashboards {
            println!(
                "  {:<28} {}",
                dashboard.id,
                dashboard.title.as_deref().unwrap_or("-"),
            );
        }
        println!();
    }

    Ok(())
}
