use anyhow::Result;

use super::resolve_org;
use crate::api::AppSignalClient;
use crate::config::Config;

/// List all applications in an organization (and save the org slug to config).
pub async fn list(org_slug: &str) -> Result<()> {
    let mut config = Config::load()?;
    let token = config.require_token()?.to_string();
    let client = AppSignalClient::new(&token);

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
    let config = Config::load()?;
    let token = config.require_token()?.to_string();
    let client = AppSignalClient::new(&token);

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
    let config = Config::load()?;
    let token = config.require_token()?.to_string();
    let org_slug = resolve_org(org, &config)?;
    let client = AppSignalClient::new(&token);

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
    let token = config.require_token()?.to_string();
    let client = AppSignalClient::new(&token);

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
    let config = Config::load()?;
    let token = config.require_token()?.to_string();
    let client = AppSignalClient::new(&token);

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
