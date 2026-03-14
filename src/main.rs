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
    /// List, find, and inspect your AppSignal applications
    Apps {
        #[command(subcommand)]
        action: AppsAction,
    },
    /// List and inspect incidents
    Incidents {
        #[command(subcommand)]
        action: IncidentsAction,
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
    /// List all applications in an organization (also saves the org as default)
    List {
        /// Organization slug (from your AppSignal URL: appsignal.com/<org-slug>)
        #[arg(long)]
        org: String,
    },
    /// Show details for a specific application by ID
    Info {
        /// The application ID
        #[arg(long)]
        app_id: String,
    },
    /// Find an application by name and optional environment
    Find {
        /// Application name (case-insensitive)
        #[arg(long)]
        name: String,
        /// Environment filter (e.g. "production", "staging") — case-insensitive
        #[arg(long)]
        environment: Option<String>,
        /// Organization slug (uses saved default if omitted)
        #[arg(long)]
        org: Option<String>,
    },
    /// Set the default organization slug
    SetOrg {
        /// Organization slug
        #[arg(long)]
        org: String,
    },
    /// Show the current default organization
    ShowOrg,
    /// List all organizations you have access to
    Orgs,
}

#[derive(Subcommand)]
enum IncidentsAction {
    /// List incidents for an application (all types)
    List {
        /// Application ID (alternative to --app + --environment)
        #[arg(long)]
        app_id: Option<String>,
        /// Application name — used with optional --environment to find the app
        #[arg(long)]
        app: Option<String>,
        /// Environment filter (e.g. "production") — used with --app
        #[arg(long)]
        environment: Option<String>,
        /// Organization slug (uses saved default if omitted)
        #[arg(long)]
        org: Option<String>,
        /// Maximum number of incidents to return
        #[arg(long, default_value = "10")]
        limit: Option<i64>,
        /// Offset for pagination
        #[arg(long)]
        offset: Option<i64>,
        /// Filter by state: OPEN, CLOSED, or WIP
        #[arg(long)]
        state: Option<String>,
        /// Sort order: LAST (most recent activity) or ID (creation order)
        #[arg(long)]
        order: Option<String>,
        /// Filter by namespaces (comma-separated, e.g. "web,background")
        #[arg(long)]
        namespaces: Option<String>,
        /// Filter by action name (e.g. "UsersController#show")
        #[arg(long)]
        action: Option<String>,
    },
    /// List exception incidents (with text search support)
    ListExceptions {
        /// Application ID (alternative to --app + --environment)
        #[arg(long)]
        app_id: Option<String>,
        /// Application name — used with optional --environment to find the app
        #[arg(long)]
        app: Option<String>,
        /// Environment filter (e.g. "production") — used with --app
        #[arg(long)]
        environment: Option<String>,
        /// Organization slug (uses saved default if omitted)
        #[arg(long)]
        org: Option<String>,
        /// Maximum number of incidents to return
        #[arg(long, default_value = "10")]
        limit: Option<i64>,
        /// Offset for pagination
        #[arg(long)]
        offset: Option<i64>,
        /// Filter by state: OPEN, CLOSED, or WIP
        #[arg(long)]
        state: Option<String>,
        /// Sort order: LAST (most recent activity) or ID (creation order)
        #[arg(long)]
        order: Option<String>,
        /// Filter by namespaces (comma-separated, e.g. "web,background")
        #[arg(long)]
        namespaces: Option<String>,
        /// Filter by action name (e.g. "UsersController#show")
        #[arg(long)]
        action: Option<String>,
        /// Search query to filter exception incidents by name or message
        #[arg(long)]
        query: Option<String>,
    },
    /// List anomaly detection incidents
    ListAnomalies {
        /// Application ID (alternative to --app + --environment)
        #[arg(long)]
        app_id: Option<String>,
        /// Application name — used with optional --environment to find the app
        #[arg(long)]
        app: Option<String>,
        /// Environment filter (e.g. "production") — used with --app
        #[arg(long)]
        environment: Option<String>,
        /// Organization slug (uses saved default if omitted)
        #[arg(long)]
        org: Option<String>,
        /// Maximum number of incidents to return
        #[arg(long, default_value = "10")]
        limit: Option<i64>,
        /// Offset for pagination
        #[arg(long)]
        offset: Option<i64>,
        /// Filter by state: OPEN, CLOSED, or WIP
        #[arg(long)]
        state: Option<String>,
        /// Sort order: LAST (most recent activity) or ID (creation order)
        #[arg(long)]
        order: Option<String>,
    },
    /// Show details for a specific incident by number
    Show {
        /// Incident number
        #[arg(long)]
        number: i64,
        /// Application ID (alternative to --app + --environment)
        #[arg(long)]
        app_id: Option<String>,
        /// Application name — used with optional --environment to find the app
        #[arg(long)]
        app: Option<String>,
        /// Environment filter (e.g. "production") — used with --app
        #[arg(long)]
        environment: Option<String>,
        /// Organization slug (uses saved default if omitted)
        #[arg(long)]
        org: Option<String>,
    },
    /// Update an incident (state, severity, assignees)
    Update {
        /// Incident number
        #[arg(long)]
        number: i64,
        /// Application ID (alternative to --app + --environment)
        #[arg(long)]
        app_id: Option<String>,
        /// Application name — used with optional --environment to find the app
        #[arg(long)]
        app: Option<String>,
        /// Environment filter (e.g. "production") — used with --app
        #[arg(long)]
        environment: Option<String>,
        /// Organization slug (uses saved default if omitted)
        #[arg(long)]
        org: Option<String>,
        /// New state: OPEN, CLOSED, or WIP
        #[arg(long)]
        state: Option<String>,
        /// New severity: UNTRIAGED, CRITICAL, HIGH, LOW, NONE, or INFORMATIONAL
        #[arg(long)]
        severity: Option<String>,
        /// Comma-separated user IDs to assign (use `apps resources` to find IDs)
        #[arg(long)]
        assign: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
    },
    /// Add a note to an incident
    AddNote {
        /// Incident number
        #[arg(long)]
        number: i64,
        /// Note content (markdown supported)
        #[arg(long)]
        content: String,
        /// Application ID (alternative to --app + --environment)
        #[arg(long)]
        app_id: Option<String>,
        /// Application name — used with optional --environment to find the app
        #[arg(long)]
        app: Option<String>,
        /// Environment filter (e.g. "production") — used with --app
        #[arg(long)]
        environment: Option<String>,
        /// Organization slug (uses saved default if omitted)
        #[arg(long)]
        org: Option<String>,
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
            AppsAction::Find {
                name,
                environment,
                org,
            } => commands::apps::find(&name, environment.as_deref(), org.as_deref()).await?,
            AppsAction::SetOrg { org } => commands::apps::set_org(&org).await?,
            AppsAction::ShowOrg => commands::apps::show_org()?,
            AppsAction::Orgs => commands::apps::orgs().await?,
        },
        Commands::Incidents { action } => match action {
            IncidentsAction::List {
                app_id,
                app,
                environment,
                org,
                limit,
                offset,
                state,
                order,
                namespaces,
                action,
            } => {
                commands::incidents::list(
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                    limit,
                    offset,
                    state.as_deref(),
                    order.as_deref(),
                    namespaces.as_deref(),
                    action.as_deref(),
                )
                .await?
            }
            IncidentsAction::ListExceptions {
                app_id,
                app,
                environment,
                org,
                limit,
                offset,
                state,
                order,
                namespaces,
                action,
                query,
            } => {
                commands::incidents::list_exceptions(
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                    limit,
                    offset,
                    state.as_deref(),
                    order.as_deref(),
                    namespaces.as_deref(),
                    action.as_deref(),
                    query.as_deref(),
                )
                .await?
            }
            IncidentsAction::ListAnomalies {
                app_id,
                app,
                environment,
                org,
                limit,
                offset,
                state,
                order,
            } => {
                commands::incidents::list_anomalies(
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                    limit,
                    offset,
                    state.as_deref(),
                    order.as_deref(),
                )
                .await?
            }
            IncidentsAction::Show {
                number,
                app_id,
                app,
                environment,
                org,
            } => {
                commands::incidents::show(
                    number,
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                )
                .await?
            }
            IncidentsAction::Update {
                number,
                app_id,
                app,
                environment,
                org,
                state,
                severity,
                assign,
                description,
            } => {
                let assignee_ids: Option<Vec<String>> =
                    assign.map(|s| s.split(',').map(|id| id.trim().to_string()).collect());
                commands::incidents::update(
                    number,
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                    state.as_deref(),
                    severity.as_deref(),
                    assignee_ids.as_deref(),
                    description.as_deref(),
                )
                .await?
            }
            IncidentsAction::AddNote {
                number,
                content,
                app_id,
                app,
                environment,
                org,
            } => {
                commands::incidents::add_note(
                    number,
                    &content,
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                )
                .await?
            }
        },
    }

    Ok(())
}
