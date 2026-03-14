mod api;
mod commands;
mod config;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "appsignal-cli")]
#[command(about = "CLI for interacting with AppSignal", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure your AppSignal API token
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// List and inspect your AppSignal applications
    Apps {
        #[command(subcommand)]
        action: AppsAction,
    },
}

#[derive(Subcommand)]
enum AuthAction {
    /// Set your personal API token
    Login {
        /// Your AppSignal personal API token
        #[arg(long)]
        token: Option<String>,
    },
    /// Remove stored credentials
    Logout,
    /// Show the current authentication status
    Status,
}

#[derive(Subcommand)]
enum AppsAction {
    /// List all applications in an organization
    List {
        /// Organization slug (from your AppSignal URL: appsignal.com/<org-slug>)
        #[arg(long)]
        org: String,
    },
    /// Show details for a specific application
    Info {
        /// The application ID
        #[arg(long)]
        app_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { action } => match action {
            AuthAction::Login { token } => commands::auth::login(token).await?,
            AuthAction::Logout => commands::auth::logout()?,
            AuthAction::Status => commands::auth::status()?,
        },
        Commands::Apps { action } => match action {
            AppsAction::List { org } => commands::apps::list(&org).await?,
            AppsAction::Info { app_id } => commands::apps::info(&app_id).await?,
        },
    }

    Ok(())
}
