mod api;
mod commands;
mod config;
mod oauth;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands::skill::InstallTarget;

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
    /// Show a more playful overview of the CLI
    About,
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
    /// Stream, search, and inspect application logs
    Logs {
        #[command(subcommand)]
        action: LogsAction,
    },
    /// Install the bundled AppSignal LLM skill
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
}

#[derive(Subcommand)]
enum SkillAction {
    /// Install the bundled AppSignal skill into an agent skills directory
    Install {
        /// Install target(s): opencode, codex, claude, or all
        #[arg(long, value_delimiter = ',', default_value = "opencode")]
        target: Vec<InstallTarget>,
        /// Install into this skills root directory instead of the target's default
        #[arg(long)]
        dir: Option<String>,
        /// Overwrite an existing installed skill
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum AuthAction {
    /// Authenticate with AppSignal (personal token or OAuth)
    Login {
        /// Your AppSignal personal API token
        #[arg(long, conflicts_with = "oauth")]
        token: Option<String>,
        /// Authenticate via OAuth (opens your browser)
        #[arg(long)]
        oauth: bool,
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
    /// Show resources for an app (users, notifiers, namespaces, dashboards)
    Resources {
        /// Application ID (alternative to --app + --environment)
        #[arg(long)]
        app_id: Option<String>,
        /// Application name
        #[arg(long)]
        app: Option<String>,
        /// Environment filter
        #[arg(long)]
        environment: Option<String>,
        /// Organization slug (uses saved default if omitted)
        #[arg(long)]
        org: Option<String>,
        /// Comma-separated sections to include: users, notifiers, namespaces, dashboards (default: all)
        #[arg(long)]
        sections: Option<String>,
    },
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
        #[arg(long, default_value = "LAST")]
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
        #[arg(long, default_value = "LAST")]
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
    /// List performance incidents (with text search support)
    ListPerformance {
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
        #[arg(long, default_value = "LAST")]
        order: Option<String>,
        /// Filter by namespaces (comma-separated, e.g. "web,background")
        #[arg(long)]
        namespaces: Option<String>,
        /// Filter by action name (e.g. "UsersController#show")
        #[arg(long)]
        action: Option<String>,
        /// Search query to filter performance incidents by action name
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
        #[arg(long, default_value = "LAST")]
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
        /// Comma-separated user names or IDs to add as assignees
        #[arg(long)]
        assign: Option<String>,
        /// Comma-separated user names or IDs to remove from assignees
        #[arg(long)]
        unassign: Option<String>,
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

#[derive(Subcommand)]
enum LogsAction {
    /// Tail (stream) log lines in real time, with optional filters
    Tail {
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
        /// Log query filter. Supports field filters (group=notifiers, severity=error,
        /// message:"[Email]", hostname:web-1) and free text. Use quotes for literal
        /// special characters. See https://docs.appsignal.com/logging/query-syntax
        #[arg(long)]
        query: Option<String>,
        /// Comma-separated severity levels (e.g. "ERROR,CRITICAL")
        #[arg(long)]
        severities: Option<String>,
        /// Comma-separated source IDs to filter by
        #[arg(long)]
        source_ids: Option<String>,
        /// Log view name or ID — applies the view's saved filters as defaults
        #[arg(long)]
        view: Option<String>,
    },
    /// Search log lines (one-shot query). Use --json for LLM-friendly output.
    Search {
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
        /// Log query filter. Supports field filters (group=notifiers, severity=error,
        /// message:"[Email]", hostname:web-1) and free text. Use quotes for literal
        /// special characters. See https://docs.appsignal.com/logging/query-syntax
        #[arg(long)]
        query: Option<String>,
        /// Comma-separated severity levels (e.g. "ERROR,CRITICAL")
        #[arg(long)]
        severities: Option<String>,
        /// Comma-separated source IDs to filter by
        #[arg(long)]
        source_ids: Option<String>,
        /// Log view name or ID — applies the view's saved filters as defaults
        #[arg(long)]
        view: Option<String>,
        /// Start time (ISO 8601, e.g. "2025-01-01T00:00:00Z")
        #[arg(long)]
        start: Option<String>,
        /// End time (ISO 8601, e.g. "2025-01-01T12:00:00Z")
        #[arg(long)]
        end: Option<String>,
        /// Maximum number of log lines to return (max 100)
        #[arg(long, default_value = "100")]
        limit: Option<i64>,
        /// Sort order: ASC (oldest first) or DESC (newest first)
        #[arg(long, default_value = "DESC")]
        order: Option<String>,
        /// Output results as JSON (useful for LLM/programmatic consumption)
        #[arg(long)]
        json: bool,
        /// Automatically paginate to fetch all results (requires --start; ignores --limit and --order)
        #[arg(long)]
        page_all: bool,
    },
    /// List saved log views (filter presets) for an app
    Views {
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
    /// List log sources for an app
    Sources {
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
        Commands::About => commands::about::show()?,
        Commands::Auth { action } => match action {
            AuthAction::Login { token, oauth } => commands::auth::login(token, oauth).await?,
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
            AppsAction::Resources {
                app_id,
                app,
                environment,
                org,
                sections,
            } => {
                commands::apps::resources(
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                    sections.as_deref(),
                )
                .await?
            }
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
            IncidentsAction::ListPerformance {
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
                commands::incidents::list_performance(
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
                unassign,
                description,
            } => {
                let assign_list: Option<Vec<String>> =
                    assign.map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
                let unassign_list: Option<Vec<String>> =
                    unassign.map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
                commands::incidents::update(
                    number,
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                    state.as_deref(),
                    severity.as_deref(),
                    assign_list.as_deref(),
                    unassign_list.as_deref(),
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
        Commands::Logs { action } => match action {
            LogsAction::Tail {
                app_id,
                app,
                environment,
                org,
                query,
                severities,
                source_ids,
                view,
            } => {
                commands::logs::tail(
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                    query.as_deref(),
                    severities.as_deref(),
                    source_ids.as_deref(),
                    view.as_deref(),
                )
                .await?
            }
            LogsAction::Search {
                app_id,
                app,
                environment,
                org,
                query,
                severities,
                source_ids,
                view,
                start,
                end,
                limit,
                order,
                json,
                page_all,
            } => {
                commands::logs::search(
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                    query.as_deref(),
                    severities.as_deref(),
                    source_ids.as_deref(),
                    view.as_deref(),
                    start.as_deref(),
                    end.as_deref(),
                    limit,
                    order.as_deref(),
                    json,
                    page_all,
                )
                .await?
            }
            LogsAction::Views {
                app_id,
                app,
                environment,
                org,
            } => {
                commands::logs::views(
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                )
                .await?
            }
            LogsAction::Sources {
                app_id,
                app,
                environment,
                org,
            } => {
                commands::logs::sources(
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                )
                .await?
            }
        },
        Commands::Skill { action } => match action {
            SkillAction::Install { target, dir, force } => {
                commands::skill::install(&target, dir.as_deref(), force)?
            }
        },
    }

    Ok(())
}
