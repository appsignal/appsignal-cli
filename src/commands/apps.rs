use anyhow::Result;

use crate::api::AppSignalClient;
use crate::config::Config;

/// List all applications in an organization.
pub async fn list(org_slug: &str) -> Result<()> {
    let config = Config::load()?;
    let token = config.require_token()?;
    let client = AppSignalClient::new(token);

    let apps = client.list_apps(org_slug).await?;

    if apps.is_empty() {
        println!("No applications found.");
        return Ok(());
    }

    println!(
        "{:<28} {:<30} {}",
        "ID", "NAME", "ENVIRONMENT"
    );
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
    let token = config.require_token()?;
    let client = AppSignalClient::new(token);

    let app = client.get_app(app_id).await?;

    println!("Application details:");
    println!("  ID:          {}", app.id);
    println!(
        "  Name:        {}",
        app.name.as_deref().unwrap_or("-")
    );
    println!(
        "  Environment: {}",
        app.environment.as_deref().unwrap_or("-")
    );

    Ok(())
}
