mod api;
mod client_headers;
mod commands;
mod config;
mod error;
mod oauth;
mod output;
mod telemetry;
mod version_check;

use anyhow::{bail, Result};
use clap::{ArgGroup, Args, Parser, Subcommand};

use crate::api::AppResourceSection;
use crate::commands::skill::InstallTarget;
use crate::error::CliError;
use crate::output::Output;

#[derive(Parser)]
#[command(name = "appsignal-cli")]
#[command(about = "CLI for interacting with AppSignal", long_about = None)]
#[command(version)]
struct Cli {
    /// Output format for command results. `human` is the default; `json` is
    /// machine-readable. Status messages always go to stderr regardless.
    /// `--format` is supported as a synonym for `--output`.
    #[arg(
        long,
        visible_alias = "format",
        short = 'o',
        global = true,
        value_enum,
        default_value_t = Output::Human
    )]
    output: Output,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show a more playful overview of the CLI
    About,
    /// Configure AppSignal authentication
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// List, find, and inspect your AppSignal applications
    Apps {
        #[command(subcommand)]
        action: AppsAction,
    },
    /// Initialize a project-local AppSignal config
    Project {
        #[command(subcommand)]
        action: ProjectAction,
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
    /// Fetch and inspect performance samples/traces
    #[command(visible_aliases = ["samples", "sample"])]
    Traces {
        #[command(subcommand)]
        action: TracesAction,
    },
    /// Create and update dashboards
    Dashboards {
        #[command(subcommand)]
        action: DashboardAction,
    },
    /// List and manage anomaly detection triggers
    Triggers {
        #[command(subcommand)]
        action: TriggerAction,
    },
    /// Send feedback about appsignal-cli to AppSignal
    Feedback(FeedbackArgs),
    /// Install the bundled AppSignal LLM skill
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
}

#[derive(Args)]
#[command(group(ArgGroup::new("contact").args(["email", "no_email"])))]
struct FeedbackArgs {
    /// Feedback text. If omitted, reads from stdin.
    #[arg(short = 'm', long, value_name = "TEXT", conflicts_with = "text")]
    message: Option<String>,
    /// Contact email for follow-up. Saved to the active config for next time.
    #[arg(long)]
    email: Option<String>,
    /// Do not include a contact email, even if one is saved.
    #[arg(long)]
    no_email: bool,
    /// Feedback text as positional arguments. Use quotes for multi-word feedback.
    #[arg(value_name = "MESSAGE")]
    text: Vec<String>,
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
    /// Update an installed AppSignal skill to the bundled version
    Update {
        /// Update target(s): opencode, codex, claude, or all
        #[arg(long, value_delimiter = ',', default_value = "opencode")]
        target: Vec<InstallTarget>,
        /// Update a skill installed in this skills root directory instead of the target's default
        #[arg(long)]
        dir: Option<String>,
    },
    /// Show whether installed AppSignal skills are current
    Status {
        /// Status target(s): opencode, codex, claude, or all
        #[arg(long, value_delimiter = ',', default_value = "all")]
        target: Vec<InstallTarget>,
        /// Check a skill installed in this skills root directory instead of the target's default
        #[arg(long)]
        dir: Option<String>,
    },
}

#[derive(Subcommand)]
enum AuthAction {
    /// Authenticate with AppSignal via OAuth
    Login {
        /// Override the AppSignal base URL (for example `https://staging.lol`)
        #[arg(long)]
        endpoint: Option<String>,
        /// Override the AppSignal REST/public API base URL
        #[arg(long)]
        rest_endpoint: Option<String>,
        /// Override the OAuth client ID used during login
        #[arg(long)]
        oauth_client_id: Option<String>,
        /// Set the default organization slug during login
        #[arg(long)]
        org: Option<String>,
    },
    /// Remove stored credentials
    Logout,
    /// Show the current authentication status
    Status,
}

#[derive(Subcommand)]
enum AppsAction {
    /// List all applications for the organization attached to the current OAuth token
    List,
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
    /// Show resources for an app
    Resources {
        #[command(subcommand)]
        action: AppResourceAction,
    },
}

#[derive(Args)]
#[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
struct AppResourceArgs {
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
}

#[derive(Subcommand)]
enum AppResourceAction {
    /// Show all supported app resources
    All(AppResourceArgs),
    /// Show app users
    Users(AppResourceArgs),
    /// Show app notifiers
    Notifiers(AppResourceArgs),
    /// Show app namespaces
    Namespaces(AppResourceArgs),
    /// Show app dashboards
    Dashboards(AppResourceArgs),
    /// Show recent deploy markers
    DeployMarkers(AppResourceArgs),
}

#[derive(Subcommand)]
enum ProjectAction {
    /// Create or update the project-local `.appsignal.toml`
    Init {
        /// Override the AppSignal base URL (for example `https://staging.lol`)
        #[arg(long)]
        endpoint: Option<String>,
        /// Override the AppSignal REST/public API base URL
        #[arg(long)]
        rest_endpoint: Option<String>,
        /// Override the OAuth client ID for this project
        #[arg(long)]
        oauth_client_id: Option<String>,
        /// Set the default organization slug for this project
        #[arg(long)]
        org: Option<String>,
    },
}

#[derive(Subcommand)]
enum IncidentsAction {
    /// List incidents for an application (all types)
    #[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
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
    #[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
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
    #[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
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
    #[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
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
    #[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
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
    #[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
    Update {
        /// Incident number. Repeat or pass a comma-separated list for bulk state changes.
        #[arg(long, value_delimiter = ',', num_args = 1.., required = true)]
        number: Vec<i64>,
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
        /// Assign the incident to the authenticated CLI user
        #[arg(long)]
        assign_me: bool,
        /// Comma-separated user names or IDs to remove from assignees
        #[arg(long)]
        unassign: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
    },
    /// Add a note to an incident
    #[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
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
    #[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
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
    /// Search log lines (one-shot query). Use --output json or --format json for machine-readable output.
    #[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
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
        /// Automatically paginate to fetch all results (requires --start; ignores --limit and --order)
        #[arg(long)]
        page_all: bool,
    },
    /// List saved log views (filter presets) for an app
    #[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
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
    #[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
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
    /// Create and manage log-derived metrics
    #[command(
        after_help = "Examples:\n  appsignal-cli logs metrics list --app \"MyApp\" --environment production\n  appsignal-cli logs metrics create --app \"MyApp\" --environment production --name \"Track error count\" --query 'severity:error' --metric 'name=log.error_count,type=counter'\n  appsignal-cli logs metrics update --app \"MyApp\" --environment production --id metric_rule_123 --clear-sources\n  appsignal-cli logs metrics delete --app \"MyApp\" --environment production --id metric_rule_123"
    )]
    Metrics {
        #[command(subcommand)]
        action: LogMetricAction,
    },
    /// Create and manage log-based triggers
    #[command(
        after_help = "Examples:\n  appsignal-cli logs triggers list --app \"MyApp\" --environment production\n  appsignal-cli logs triggers create --app \"MyApp\" --environment production --name \"Root login\" --query 'message:root' --severity ERROR --notifier-id notifier_123\n  appsignal-cli logs triggers update --app \"MyApp\" --environment production --id trigger_rule_123 --clear-notifiers\n  appsignal-cli logs triggers delete --app \"MyApp\" --environment production --id trigger_rule_123"
    )]
    Triggers {
        #[command(subcommand)]
        action: LogTriggerAction,
    },
}

#[derive(Args)]
#[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
struct TraceAppArgs {
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
}

#[derive(Subcommand)]
enum TracesAction {
    /// List performance samples/traces for an action
    List {
        #[command(flatten)]
        app: TraceAppArgs,
        /// Namespace to search in (e.g. web, background, graphql)
        #[arg(long)]
        namespace: String,
        /// Action name to fetch traces for (e.g. UsersController#show)
        #[arg(long = "action")]
        action_name: String,
        /// Start time (ISO 8601, defaults to 24 hours ago)
        #[arg(long)]
        start: Option<String>,
        /// End time (ISO 8601, defaults to now)
        #[arg(long)]
        end: Option<String>,
        /// Minimum trace duration in milliseconds
        #[arg(long)]
        min_duration_ms: Option<f64>,
        /// Maximum number of samples/traces to return (1-100, default 25)
        #[arg(long, default_value = "25")]
        limit: Option<i64>,
        /// Automatically paginate to fetch all samples/traces; ignores --limit
        #[arg(long)]
        page_all: bool,
    },
    /// List performance samples/traces for an incident
    Incident {
        #[command(flatten)]
        app: TraceAppArgs,
        /// Performance incident number
        #[arg(long)]
        number: i64,
        /// Restrict lookup to one action if the incident has multiple actions
        #[arg(long = "action")]
        action_name: Option<String>,
        /// Start time (ISO 8601, defaults to 24 hours ago)
        #[arg(long)]
        start: Option<String>,
        /// End time (ISO 8601, defaults to now)
        #[arg(long)]
        end: Option<String>,
        /// Minimum trace duration in milliseconds
        #[arg(long)]
        min_duration_ms: Option<f64>,
        /// Maximum number of samples/traces to return per action (1-100, default 25)
        #[arg(long, default_value = "25")]
        limit: Option<i64>,
        /// Automatically paginate to fetch all samples/traces for each action; ignores --limit
        #[arg(long)]
        page_all: bool,
    },
    /// List error traces for an exception digest
    Errors {
        #[command(flatten)]
        app: TraceAppArgs,
        /// Exception incident digest
        #[arg(long)]
        digest: String,
        /// Maximum number of error traces to return (1-100, default 25)
        #[arg(long, default_value = "25")]
        limit: Option<i64>,
        /// Automatically paginate to fetch all error traces; ignores --limit
        #[arg(long)]
        page_all: bool,
    },
    /// Show a performance sample/trace span tree, or one span with --span-id
    Show {
        #[command(flatten)]
        app: TraceAppArgs,
        /// Namespace to search in (e.g. web, background, graphql)
        #[arg(long)]
        namespace: String,
        /// Action name for the trace (e.g. UsersController#show)
        #[arg(long = "action")]
        action_name: String,
        /// Trace ID returned by `traces list` or `samples list`
        #[arg(long)]
        trace_id: String,
        /// Span ID to inspect within the trace
        #[arg(long)]
        span_id: Option<String>,
        /// Include HTTP headers, request parameters, session data, and function parameters
        #[arg(long)]
        include_sensitive: bool,
        /// Start time (ISO 8601, defaults to 24 hours ago)
        #[arg(long)]
        start: Option<String>,
        /// End time (ISO 8601, defaults to now)
        #[arg(long)]
        end: Option<String>,
    },
    /// Show an error trace span tree, or one span with --span-id
    ShowError {
        #[command(flatten)]
        app: TraceAppArgs,
        /// Exception incident digest
        #[arg(long)]
        digest: String,
        /// Trace ID returned by `samples errors` or `samples incident` for an exception incident
        #[arg(long)]
        trace_id: String,
        /// Span ID to inspect within the trace
        #[arg(long)]
        span_id: Option<String>,
        /// Include HTTP headers, request parameters, session data, and function parameters
        #[arg(long)]
        include_sensitive: bool,
    },
    /// Show a trace from a performance or exception incident without passing namespace/action/digest
    ShowIncident {
        #[command(flatten)]
        app: TraceAppArgs,
        /// Performance or exception incident number
        #[arg(long)]
        number: i64,
        /// Trace ID returned by `samples incident`
        #[arg(long)]
        trace_id: String,
        /// Span ID to inspect within the trace
        #[arg(long)]
        span_id: Option<String>,
        /// Include HTTP headers, request parameters, session data, and function parameters
        #[arg(long)]
        include_sensitive: bool,
        /// Start time for performance trace lookup (ISO 8601, defaults to 24 hours ago)
        #[arg(long)]
        start: Option<String>,
        /// End time for performance trace lookup (ISO 8601, defaults to now)
        #[arg(long)]
        end: Option<String>,
    },
}

#[derive(Args)]
#[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
struct LogActionAppArgs {
    /// Application ID (required unless --app is used)
    #[arg(long)]
    app_id: Option<String>,
    /// Application name (required unless --app-id is used)
    #[arg(long)]
    app: Option<String>,
    /// Environment filter (recommended with --app; required when the app name is ambiguous)
    #[arg(long)]
    environment: Option<String>,
    /// Organization slug (uses saved default if omitted)
    #[arg(long)]
    org: Option<String>,
}

impl LogActionAppArgs {
    fn as_ref(&self) -> commands::logs::actions::AppRef<'_> {
        commands::logs::actions::AppRef {
            app_id: self.app_id.as_deref(),
            app_name: self.app.as_deref(),
            environment: self.environment.as_deref(),
            org: self.org.as_deref(),
        }
    }
}

/// Resolve a `Vec<T>` flag into the three states our action mutations care
/// about: leave the field untouched (`None`), clear it (`Some(empty)`), or
/// replace it (`Some(non-empty)`).
fn replace_or_clear<T>(values: Vec<T>, clear: bool) -> Option<Vec<T>> {
    if clear {
        Some(Vec::new())
    } else if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

/// Same tri-state as `replace_or_clear`, but for scalar fields. The `clear`
/// flag wins if both are set (clap already enforces `conflicts_with`).
fn replace_or_clear_scalar<T>(value: Option<T>, clear: bool) -> api::Patch<T> {
    if clear {
        api::Patch::Clear
    } else {
        match value {
            Some(v) => api::Patch::Set(v),
            None => api::Patch::Unchanged,
        }
    }
}

#[derive(Subcommand)]
enum LogMetricAction {
    /// List log-derived metrics for an app
    List {
        #[command(flatten)]
        app: LogActionAppArgs,
    },
    /// Create a new log-derived metric
    #[command(
        after_help = "Example:\n  appsignal-cli logs metrics create --app \"MyApp\" --environment production --name \"Track error count\" --query 'severity:error' --metric 'name=log.error_count,type=counter'"
    )]
    Create {
        #[command(flatten)]
        app: LogActionAppArgs,
        /// Metric configuration name (required)
        #[arg(long)]
        name: String,
        /// Query expression to match against log lines (required)
        #[arg(long)]
        query: String,
        /// Scope the action to a specific source ID. Repeat to add more.
        #[arg(long = "source-id")]
        source_ids: Vec<String>,
        /// Metric definition in key=value form (required, repeat for multiple metrics). Example: `name=log.error_count,type=counter` or `name=log.request_duration,type=distribution,field=duration_ms,tag.hostname=web-1`
        #[arg(long = "metric")]
        metrics: Vec<api::LogLineMetricInput>,
    },
    /// Update a log-derived metric
    #[command(
        after_help = "Examples:\n  appsignal-cli logs metrics update --app \"MyApp\" --environment production --id metric_rule_123 --name \"Track API errors\"\n  appsignal-cli logs metrics update --app \"MyApp\" --environment production --id metric_rule_123 --clear-metrics"
    )]
    Update {
        #[command(flatten)]
        app: LogActionAppArgs,
        /// ID of the metric configuration to update (required)
        #[arg(long)]
        id: String,
        /// New metric configuration name
        #[arg(long)]
        name: Option<String>,
        /// New query expression
        #[arg(long)]
        query: Option<String>,
        /// Replace source IDs with these values. Repeat to add more.
        #[arg(long = "source-id", conflicts_with = "clear_sources")]
        source_ids: Vec<String>,
        /// Remove all source IDs from the action
        #[arg(long, conflicts_with = "source_ids")]
        clear_sources: bool,
        /// Replace metric definitions with these values. Repeat for multiple metrics.
        #[arg(long = "metric", conflicts_with = "clear_metrics")]
        metrics: Vec<api::LogLineMetricInput>,
        /// Remove all metric definitions from this metric configuration
        #[arg(long, conflicts_with = "metrics")]
        clear_metrics: bool,
    },
    /// Delete a log-derived metric
    Delete {
        #[command(flatten)]
        app: LogActionAppArgs,
        /// ID of the metric configuration to delete (required)
        #[arg(long)]
        id: String,
    },
}

#[derive(Subcommand)]
enum LogTriggerAction {
    /// List log-based triggers for an app
    List {
        #[command(flatten)]
        app: LogActionAppArgs,
    },
    /// Create a new log-based trigger
    #[command(
        after_help = "Example:\n  appsignal-cli logs triggers create --app \"MyApp\" --environment production --name \"Root login\" --query 'message:root' --severity ERROR --notifier-id notifier_123"
    )]
    Create {
        #[command(flatten)]
        app: LogActionAppArgs,
        /// Trigger name (required)
        #[arg(long)]
        name: String,
        /// Query expression to match against log lines (required)
        #[arg(long)]
        query: String,
        /// Scope the trigger to a specific source ID. Repeat to add more.
        #[arg(long = "source-id")]
        source_ids: Vec<String>,
        /// Trigger description
        #[arg(long)]
        description: Option<String>,
        /// Attach a notifier to this trigger. Repeat to add more.
        #[arg(long = "notifier-id")]
        notifier_ids: Vec<String>,
        /// Match only these severities. Repeat to add more.
        #[arg(long = "severity")]
        severities: Vec<String>,
    },
    /// Update an existing log-based trigger
    #[command(
        after_help = "Examples:\n  appsignal-cli logs triggers update --app \"MyApp\" --environment production --id trigger_rule_123 --name \"Root login attempts\"\n  appsignal-cli logs triggers update --app \"MyApp\" --environment production --id trigger_rule_123 --clear-severities --clear-notifiers"
    )]
    Update {
        #[command(flatten)]
        app: LogActionAppArgs,
        /// ID of the trigger to update (required)
        #[arg(long)]
        id: String,
        /// New trigger name
        #[arg(long)]
        name: Option<String>,
        /// New query expression
        #[arg(long)]
        query: Option<String>,
        /// Replace source IDs with these values. Repeat to add more.
        #[arg(long = "source-id", conflicts_with = "clear_sources")]
        source_ids: Vec<String>,
        /// Remove all source IDs from the trigger
        #[arg(long, conflicts_with = "source_ids")]
        clear_sources: bool,
        /// Update trigger description
        #[arg(long, conflicts_with = "clear_description")]
        description: Option<String>,
        /// Clear the trigger description
        #[arg(long, conflicts_with = "description")]
        clear_description: bool,
        /// Replace notifier IDs with these values. Repeat to add more.
        #[arg(long = "notifier-id", conflicts_with = "clear_notifiers")]
        notifier_ids: Vec<String>,
        /// Remove all trigger notifier IDs
        #[arg(long, conflicts_with = "notifier_ids")]
        clear_notifiers: bool,
        /// Replace trigger severities with these values. Repeat to add more.
        #[arg(long = "severity", conflicts_with = "clear_severities")]
        severities: Vec<String>,
        /// Remove all trigger severities
        #[arg(long, conflicts_with = "severities")]
        clear_severities: bool,
    },
    /// Delete a log-based trigger
    Delete {
        #[command(flatten)]
        app: LogActionAppArgs,
        /// ID of the trigger to delete (required)
        #[arg(long)]
        id: String,
    },
}

#[derive(Args)]
#[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
struct TriggerAppArgs {
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
}

#[derive(Args)]
#[command(group(ArgGroup::new("app_ref").required(true).args(["app_id", "app"])))]
struct DashboardAppArgs {
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
}

#[derive(Args)]
struct DashboardDefinitionArgs {
    /// Dashboard title
    #[arg(long)]
    title: String,
    /// Optional dashboard description
    #[arg(long)]
    description: Option<String>,
}

#[derive(Args)]
struct TriggerDefinitionArgs {
    /// Display name for the trigger. Defaults to the metric name if omitted.
    #[arg(long)]
    name: Option<String>,
    /// Metric name to monitor
    #[arg(long)]
    metric_name: String,
    /// Trigger kind/classification (for example: Advanced, Performance, HostCPUUsage)
    #[arg(long)]
    kind: String,
    /// Metric field to compare: count, counter, gauge, mean, p90, or p95
    #[arg(long)]
    field: String,
    /// Comparison operator: >, >=, <, <=, ==, !=
    #[arg(long)]
    comparison_operator: String,
    /// Threshold value to compare against
    #[arg(long)]
    condition_value: f64,
    /// Warmup duration in minutes before opening an alert
    #[arg(long)]
    warmup_duration: i64,
    /// Cooldown duration in minutes before closing an alert
    #[arg(long)]
    cooldown_duration: i64,
    /// Comma-separated notifier IDs to attach to the trigger
    #[arg(long)]
    notifier_ids: Option<String>,
    /// Tag filter(s) in key=value form. Repeat the flag or use commas.
    #[arg(long = "tag", value_delimiter = ',')]
    tags: Vec<String>,
    /// Optional description shown with the trigger
    #[arg(long)]
    description: Option<String>,
    /// Treat missing datapoints as 0
    #[arg(long, default_value_t = false)]
    no_match_is_zero: bool,
    /// Dashboard ID to link from notifications
    #[arg(long)]
    dashboard_id: Option<String>,
    /// Output format for the metric value (for example: duration, number, percent)
    #[arg(long)]
    format: Option<String>,
    /// Input unit for the size format (for example: byte, kilobyte, megabyte)
    #[arg(long)]
    format_input: Option<String>,
}

#[derive(Subcommand)]
enum TriggerAction {
    /// List triggers for an application
    List {
        #[command(flatten)]
        app: TriggerAppArgs,
        /// Filter by metric name
        #[arg(long)]
        metric_name: Option<String>,
        /// Filter by trigger kind
        #[arg(long)]
        kind: Option<String>,
        /// Tag filter(s) in key=value form. Repeat the flag or use commas.
        #[arg(long = "tag", value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// Create a new anomaly detection trigger
    Create {
        #[command(flatten)]
        app: TriggerAppArgs,
        #[command(flatten)]
        definition: TriggerDefinitionArgs,
    },
    /// Update a trigger by creating a new version linked to the existing trigger
    Update {
        #[command(flatten)]
        app: TriggerAppArgs,
        /// ID of the existing trigger to update
        #[arg(long)]
        id: String,
        #[command(flatten)]
        definition: TriggerDefinitionArgs,
    },
    /// Archive a trigger and close its associated alerts/incidents
    Archive {
        #[command(flatten)]
        app: TriggerAppArgs,
        /// ID of the trigger to archive
        #[arg(long)]
        id: String,
    },
}

#[derive(Subcommand)]
enum DashboardAction {
    /// List dashboards for an application
    List {
        #[command(flatten)]
        app: DashboardAppArgs,
    },
    /// Create a new dashboard
    Create {
        #[command(flatten)]
        app: DashboardAppArgs,
        #[command(flatten)]
        definition: DashboardDefinitionArgs,
    },
    /// Update an existing dashboard
    Update {
        #[command(flatten)]
        app: DashboardAppArgs,
        /// ID of the dashboard to update
        #[arg(long)]
        id: String,
        #[command(flatten)]
        definition: DashboardDefinitionArgs,
    },
}

impl Cli {
    fn telemetry_command(&self) -> telemetry::TelemetryCommand {
        self.command.telemetry_command()
    }
}

trait ToTelemetryCommand {
    fn telemetry_command(&self) -> telemetry::TelemetryCommand;
}

macro_rules! impl_telemetry_command {
    ($ty:ty { $($pattern:pat => $command:expr),+ $(,)? }) => {
        impl ToTelemetryCommand for $ty {
            fn telemetry_command(&self) -> telemetry::TelemetryCommand {
                match self {
                    $($pattern => $command),+
                }
            }
        }
    };
}

impl_telemetry_command!(Commands {
    Self::About => telemetry::TelemetryCommand::About,
    Self::Auth { action } => action.telemetry_command(),
    Self::Apps { action } => action.telemetry_command(),
    Self::Project { action } => action.telemetry_command(),
    Self::Incidents { action } => action.telemetry_command(),
    Self::Logs { action } => action.telemetry_command(),
    Self::Traces { action } => action.telemetry_command(),
    Self::Dashboards { action } => action.telemetry_command(),
    Self::Triggers { action } => action.telemetry_command(),
    Self::Feedback(_) => telemetry::TelemetryCommand::Feedback,
    Self::Skill { action } => action.telemetry_command()
});

impl_telemetry_command!(SkillAction {
    Self::Install { .. } => telemetry::TelemetryCommand::SkillInstall,
    Self::Update { .. } => telemetry::TelemetryCommand::SkillUpdate,
    Self::Status { .. } => telemetry::TelemetryCommand::SkillStatus
});

impl_telemetry_command!(AuthAction {
    Self::Login { .. } => telemetry::TelemetryCommand::AuthLogin,
    Self::Logout => telemetry::TelemetryCommand::AuthLogout,
    Self::Status => telemetry::TelemetryCommand::AuthStatus
});

impl_telemetry_command!(AppsAction {
    Self::List => telemetry::TelemetryCommand::AppsList,
    Self::Info { .. } => telemetry::TelemetryCommand::AppsInfo,
    Self::Find { .. } => telemetry::TelemetryCommand::AppsFind,
    Self::SetOrg { .. } => telemetry::TelemetryCommand::AppsSetOrg,
    Self::ShowOrg => telemetry::TelemetryCommand::AppsShowOrg,
    Self::Resources { action } => action.telemetry_command()
});

impl_telemetry_command!(AppResourceAction {
    Self::All(_) => telemetry::TelemetryCommand::AppsResourcesAll,
    Self::Users(_) => telemetry::TelemetryCommand::AppsResourcesUsers,
    Self::Notifiers(_) => telemetry::TelemetryCommand::AppsResourcesNotifiers,
    Self::Namespaces(_) => telemetry::TelemetryCommand::AppsResourcesNamespaces,
    Self::Dashboards(_) => telemetry::TelemetryCommand::AppsResourcesDashboards,
    Self::DeployMarkers(_) => telemetry::TelemetryCommand::AppsResourcesDeployMarkers
});

impl_telemetry_command!(ProjectAction {
    Self::Init { .. } => telemetry::TelemetryCommand::ProjectInit
});

impl_telemetry_command!(IncidentsAction {
    Self::List { .. } => telemetry::TelemetryCommand::IncidentsList,
    Self::ListExceptions { .. } => telemetry::TelemetryCommand::IncidentsListExceptions,
    Self::ListPerformance { .. } => telemetry::TelemetryCommand::IncidentsListPerformance,
    Self::ListAnomalies { .. } => telemetry::TelemetryCommand::IncidentsListAnomalies,
    Self::Show { .. } => telemetry::TelemetryCommand::IncidentsShow,
    Self::Update { .. } => telemetry::TelemetryCommand::IncidentsUpdate,
    Self::AddNote { .. } => telemetry::TelemetryCommand::IncidentsAddNote
});

impl_telemetry_command!(LogsAction {
    Self::Tail { .. } => telemetry::TelemetryCommand::LogsTail,
    Self::Search { .. } => telemetry::TelemetryCommand::LogsSearch,
    Self::Views { .. } => telemetry::TelemetryCommand::LogsViews,
    Self::Sources { .. } => telemetry::TelemetryCommand::LogsSources,
    Self::Metrics { action } => action.telemetry_command(),
    Self::Triggers { action } => action.telemetry_command()
});

impl_telemetry_command!(TracesAction {
    Self::List { .. } => telemetry::TelemetryCommand::TracesList,
    Self::Incident { .. } => telemetry::TelemetryCommand::TracesIncident,
    Self::Errors { .. } => telemetry::TelemetryCommand::TracesErrors,
    Self::Show { .. } => telemetry::TelemetryCommand::TracesShow,
    Self::ShowError { .. } => telemetry::TelemetryCommand::TracesShowError,
    Self::ShowIncident { .. } => telemetry::TelemetryCommand::TracesShowIncident
});

impl_telemetry_command!(LogMetricAction {
    Self::List { .. } => telemetry::TelemetryCommand::LogsMetricsList,
    Self::Create { .. } => telemetry::TelemetryCommand::LogsMetricsCreate,
    Self::Update { .. } => telemetry::TelemetryCommand::LogsMetricsUpdate,
    Self::Delete { .. } => telemetry::TelemetryCommand::LogsMetricsDelete
});

impl_telemetry_command!(LogTriggerAction {
    Self::List { .. } => telemetry::TelemetryCommand::LogsTriggersList,
    Self::Create { .. } => telemetry::TelemetryCommand::LogsTriggersCreate,
    Self::Update { .. } => telemetry::TelemetryCommand::LogsTriggersUpdate,
    Self::Delete { .. } => telemetry::TelemetryCommand::LogsTriggersDelete
});

impl_telemetry_command!(TriggerAction {
    Self::List { .. } => telemetry::TelemetryCommand::TriggersList,
    Self::Create { .. } => telemetry::TelemetryCommand::TriggersCreate,
    Self::Update { .. } => telemetry::TelemetryCommand::TriggersUpdate,
    Self::Archive { .. } => telemetry::TelemetryCommand::TriggersArchive
});

impl_telemetry_command!(DashboardAction {
    Self::List { .. } => telemetry::TelemetryCommand::DashboardsList,
    Self::Create { .. } => telemetry::TelemetryCommand::DashboardsCreate,
    Self::Update { .. } => telemetry::TelemetryCommand::DashboardsUpdate
});

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let telemetry_command = cli.telemetry_command();
    let output = cli.output;
    let started_at = std::time::Instant::now();

    let result = run(cli).await;

    telemetry::track_command(
        telemetry_command,
        result.is_ok(),
        started_at.elapsed(),
        output,
    )
    .await;

    if let Err(err) = result {
        let _ = output::print_error(&err, output);
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    match version_check::check().await {
        version_check::VersionCheck::UpToDate => {}
        version_check::VersionCheck::UpgradeAvailable(latest_version) => {
            crate::output::status_box(&[
                "Newer appsignal-cli version available".to_string(),
                format!("Current: {}", env!("CARGO_PKG_VERSION")),
                format!("Latest:  {latest_version}"),
            ]);
        }
        version_check::VersionCheck::UpgradeRequired(latest_version) => {
            crate::output::status_box(&[
                "Upgrade required".to_string(),
                "A new major appsignal-cli version is available.".to_string(),
                format!("Current: {}", env!("CARGO_PKG_VERSION")),
                format!("Latest:  {latest_version}"),
                "Install the latest major version to continue.".to_string(),
            ]);
            bail!(CliError::msg("Please upgrade appsignal-cli to continue."));
        }
    }

    match cli.command {
        Commands::About => commands::about::show(cli.output)?,
        Commands::Auth { action } => match action {
            AuthAction::Login {
                endpoint,
                rest_endpoint,
                oauth_client_id,
                org,
            } => {
                commands::auth::login(
                    commands::auth::LoginOptions {
                        endpoint,
                        rest_endpoint,
                        oauth_client_id,
                        org,
                    },
                    cli.output,
                )
                .await?
            }
            AuthAction::Logout => commands::auth::logout(cli.output)?,
            AuthAction::Status => commands::auth::status(cli.output)?,
        },
        Commands::Apps { action } => match action {
            AppsAction::List => commands::apps::list(cli.output).await?,
            AppsAction::Info { app_id } => commands::apps::info(&app_id, cli.output).await?,
            AppsAction::Find {
                name,
                environment,
                org,
            } => {
                commands::apps::find(&name, environment.as_deref(), org.as_deref(), cli.output)
                    .await?
            }
            AppsAction::SetOrg { org } => commands::apps::set_org(&org, cli.output).await?,
            AppsAction::ShowOrg => commands::apps::show_org(cli.output)?,
            AppsAction::Resources { action } => match action {
                AppResourceAction::All(args) => {
                    commands::apps::resources(
                        args.app_id.as_deref(),
                        args.app.as_deref(),
                        args.environment.as_deref(),
                        args.org.as_deref(),
                        &[],
                        cli.output,
                    )
                    .await?
                }
                AppResourceAction::Users(args) => {
                    commands::apps::resources(
                        args.app_id.as_deref(),
                        args.app.as_deref(),
                        args.environment.as_deref(),
                        args.org.as_deref(),
                        &[AppResourceSection::Users],
                        cli.output,
                    )
                    .await?
                }
                AppResourceAction::Notifiers(args) => {
                    commands::apps::resources(
                        args.app_id.as_deref(),
                        args.app.as_deref(),
                        args.environment.as_deref(),
                        args.org.as_deref(),
                        &[AppResourceSection::Notifiers],
                        cli.output,
                    )
                    .await?
                }
                AppResourceAction::Namespaces(args) => {
                    commands::apps::resources(
                        args.app_id.as_deref(),
                        args.app.as_deref(),
                        args.environment.as_deref(),
                        args.org.as_deref(),
                        &[AppResourceSection::Namespaces],
                        cli.output,
                    )
                    .await?
                }
                AppResourceAction::Dashboards(args) => {
                    commands::apps::resources(
                        args.app_id.as_deref(),
                        args.app.as_deref(),
                        args.environment.as_deref(),
                        args.org.as_deref(),
                        &[AppResourceSection::Dashboards],
                        cli.output,
                    )
                    .await?
                }
                AppResourceAction::DeployMarkers(args) => {
                    commands::apps::resources(
                        args.app_id.as_deref(),
                        args.app.as_deref(),
                        args.environment.as_deref(),
                        args.org.as_deref(),
                        &[AppResourceSection::DeployMarkers],
                        cli.output,
                    )
                    .await?
                }
            },
        },
        Commands::Project { action } => match action {
            ProjectAction::Init {
                endpoint,
                rest_endpoint,
                oauth_client_id,
                org,
            } => commands::project::init(
                commands::project::InitOptions {
                    endpoint,
                    rest_endpoint,
                    oauth_client_id,
                    org,
                },
                cli.output,
            )?,
        },
        Commands::Dashboards { action } => match action {
            DashboardAction::List { app } => {
                commands::dashboards::list(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    cli.output,
                )
                .await?
            }
            DashboardAction::Create { app, definition } => {
                commands::dashboards::create(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    &definition.title,
                    definition.description.as_deref(),
                    cli.output,
                )
                .await?
            }
            DashboardAction::Update {
                app,
                id,
                definition,
            } => {
                commands::dashboards::update(
                    &id,
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    &definition.title,
                    definition.description.as_deref(),
                    cli.output,
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
                    cli.output,
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
                    cli.output,
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
                    cli.output,
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
                    cli.output,
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
                    cli.output,
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
                assign_me,
                unassign,
                description,
            } => {
                let assign_list: Option<Vec<String>> =
                    assign.map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
                let unassign_list: Option<Vec<String>> =
                    unassign.map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
                commands::incidents::update(
                    &number,
                    app_id.as_deref(),
                    app.as_deref(),
                    environment.as_deref(),
                    org.as_deref(),
                    state.as_deref(),
                    severity.as_deref(),
                    assign_list.as_deref(),
                    assign_me,
                    unassign_list.as_deref(),
                    description.as_deref(),
                    cli.output,
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
                    cli.output,
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
                    cli.output,
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
                    page_all,
                    cli.output,
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
                    cli.output,
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
                    cli.output,
                )
                .await?
            }
            LogsAction::Metrics { action } => match action {
                LogMetricAction::List { app } => {
                    commands::logs::actions::list(
                        api::LogLineActionKind::Metrics,
                        &app.as_ref(),
                        cli.output,
                    )
                    .await?
                }
                LogMetricAction::Create {
                    app,
                    name,
                    query,
                    source_ids,
                    metrics,
                } => {
                    commands::logs::actions::create_metric(
                        &app.as_ref(),
                        &name,
                        &query,
                        &source_ids,
                        &metrics,
                        cli.output,
                    )
                    .await?
                }
                LogMetricAction::Update {
                    app,
                    id,
                    name,
                    query,
                    source_ids,
                    clear_sources,
                    metrics,
                    clear_metrics,
                } => {
                    commands::logs::actions::update_metric(
                        &app.as_ref(),
                        &id,
                        name.as_deref(),
                        query.as_deref(),
                        replace_or_clear(source_ids, clear_sources),
                        replace_or_clear(metrics, clear_metrics),
                        cli.output,
                    )
                    .await?
                }
                LogMetricAction::Delete { app, id } => {
                    commands::logs::actions::delete(
                        api::LogLineActionKind::Metrics,
                        &app.as_ref(),
                        &id,
                        cli.output,
                    )
                    .await?
                }
            },
            LogsAction::Triggers { action } => match action {
                LogTriggerAction::List { app } => {
                    commands::logs::actions::list(
                        api::LogLineActionKind::Trigger,
                        &app.as_ref(),
                        cli.output,
                    )
                    .await?
                }
                LogTriggerAction::Create {
                    app,
                    name,
                    query,
                    source_ids,
                    description,
                    notifier_ids,
                    severities,
                } => {
                    commands::logs::actions::create_trigger(
                        &app.as_ref(),
                        &name,
                        &query,
                        &source_ids,
                        commands::logs::actions::TriggerFields {
                            description: replace_or_clear_scalar(description, false),
                            notifier_ids: (!notifier_ids.is_empty()).then_some(notifier_ids),
                            severities: (!severities.is_empty()).then_some(severities),
                        },
                        cli.output,
                    )
                    .await?
                }
                LogTriggerAction::Update {
                    app,
                    id,
                    name,
                    query,
                    source_ids,
                    clear_sources,
                    description,
                    clear_description,
                    notifier_ids,
                    clear_notifiers,
                    severities,
                    clear_severities,
                } => {
                    commands::logs::actions::update_trigger(
                        &app.as_ref(),
                        &id,
                        name.as_deref(),
                        query.as_deref(),
                        replace_or_clear(source_ids, clear_sources),
                        commands::logs::actions::TriggerFields {
                            description: replace_or_clear_scalar(description, clear_description),
                            notifier_ids: replace_or_clear(notifier_ids, clear_notifiers),
                            severities: replace_or_clear(severities, clear_severities),
                        },
                        cli.output,
                    )
                    .await?
                }
                LogTriggerAction::Delete { app, id } => {
                    commands::logs::actions::delete(
                        api::LogLineActionKind::Trigger,
                        &app.as_ref(),
                        &id,
                        cli.output,
                    )
                    .await?
                }
            },
        },
        Commands::Traces { action } => match action {
            TracesAction::List {
                app,
                namespace,
                action_name,
                start,
                end,
                min_duration_ms,
                limit,
                page_all,
            } => {
                commands::traces::list(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    &namespace,
                    &action_name,
                    start.as_deref(),
                    end.as_deref(),
                    min_duration_ms,
                    limit,
                    page_all,
                    cli.output,
                )
                .await?
            }
            TracesAction::Incident {
                app,
                number,
                action_name,
                start,
                end,
                min_duration_ms,
                limit,
                page_all,
            } => {
                commands::traces::incident(
                    number,
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    action_name.as_deref(),
                    start.as_deref(),
                    end.as_deref(),
                    min_duration_ms,
                    limit,
                    page_all,
                    cli.output,
                )
                .await?
            }
            TracesAction::Errors {
                app,
                digest,
                limit,
                page_all,
            } => {
                commands::traces::errors(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    &digest,
                    limit,
                    page_all,
                    cli.output,
                )
                .await?
            }
            TracesAction::Show {
                app,
                namespace,
                action_name,
                trace_id,
                span_id,
                include_sensitive,
                start,
                end,
            } => {
                commands::traces::show(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    &namespace,
                    &action_name,
                    &trace_id,
                    span_id.as_deref(),
                    include_sensitive,
                    start.as_deref(),
                    end.as_deref(),
                    cli.output,
                )
                .await?
            }
            TracesAction::ShowError {
                app,
                digest,
                trace_id,
                span_id,
                include_sensitive,
            } => {
                commands::traces::show_error(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    &digest,
                    &trace_id,
                    span_id.as_deref(),
                    include_sensitive,
                    cli.output,
                )
                .await?
            }
            TracesAction::ShowIncident {
                app,
                number,
                trace_id,
                span_id,
                include_sensitive,
                start,
                end,
            } => {
                commands::traces::show_incident(
                    number,
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    &trace_id,
                    span_id.as_deref(),
                    include_sensitive,
                    start.as_deref(),
                    end.as_deref(),
                    cli.output,
                )
                .await?
            }
        },
        Commands::Triggers { action } => match action {
            TriggerAction::List {
                app,
                metric_name,
                kind,
                tags,
            } => {
                commands::triggers::list(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    metric_name.as_deref(),
                    kind.as_deref(),
                    &tags,
                    cli.output,
                )
                .await?
            }
            TriggerAction::Create { app, definition } => {
                commands::triggers::create(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    definition.name.as_deref(),
                    &definition.metric_name,
                    &definition.kind,
                    &definition.field,
                    &definition.comparison_operator,
                    definition.condition_value,
                    definition.warmup_duration,
                    definition.cooldown_duration,
                    definition.notifier_ids.as_deref(),
                    &definition.tags,
                    definition.description.as_deref(),
                    definition.no_match_is_zero,
                    definition.dashboard_id.as_deref(),
                    definition.format.as_deref(),
                    definition.format_input.as_deref(),
                    cli.output,
                )
                .await?
            }
            TriggerAction::Update {
                app,
                id,
                definition,
            } => {
                commands::triggers::update(
                    &id,
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    definition.name.as_deref(),
                    &definition.metric_name,
                    &definition.kind,
                    &definition.field,
                    &definition.comparison_operator,
                    definition.condition_value,
                    definition.warmup_duration,
                    definition.cooldown_duration,
                    definition.notifier_ids.as_deref(),
                    &definition.tags,
                    definition.description.as_deref(),
                    definition.no_match_is_zero,
                    definition.dashboard_id.as_deref(),
                    definition.format.as_deref(),
                    definition.format_input.as_deref(),
                    cli.output,
                )
                .await?
            }
            TriggerAction::Archive { app, id } => {
                commands::triggers::archive(
                    &id,
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    cli.output,
                )
                .await?
            }
        },
        Commands::Feedback(args) => {
            commands::feedback::send(
                commands::feedback::FeedbackOptions {
                    message: args.message,
                    text: args.text,
                    email: args.email,
                    no_email: args.no_email,
                },
                cli.output,
            )
            .await?
        }
        Commands::Skill { action } => match action {
            SkillAction::Install { target, dir, force } => {
                commands::skill::install(&target, dir.as_deref(), force, cli.output)?
            }
            SkillAction::Update { target, dir } => {
                commands::skill::update(&target, dir.as_deref(), cli.output)?
            }
            SkillAction::Status { target, dir } => {
                commands::skill::status(&target, dir.as_deref(), cli.output)?
            }
        },
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_command_maps_nested_app_resource_commands() {
        let cli = Cli {
            output: Output::Human,
            command: Commands::Apps {
                action: AppsAction::Resources {
                    action: AppResourceAction::DeployMarkers(AppResourceArgs {
                        app_id: None,
                        app: None,
                        environment: None,
                        org: None,
                    }),
                },
            },
        };

        assert_eq!(
            cli.telemetry_command(),
            telemetry::TelemetryCommand::AppsResourcesDeployMarkers
        );
    }

    #[test]
    fn samples_alias_maps_to_traces_command() {
        let cli = Cli::try_parse_from([
            "appsignal-cli",
            "samples",
            "list",
            "--app-id",
            "app-1",
            "--namespace",
            "web",
            "--action",
            "PostsController#index",
        ])
        .unwrap();

        assert_eq!(
            cli.telemetry_command(),
            telemetry::TelemetryCommand::TracesList
        );
        match cli.command {
            Commands::Traces {
                action:
                    TracesAction::List {
                        app,
                        namespace,
                        action_name,
                        ..
                    },
            } => {
                assert_eq!(app.app_id.as_deref(), Some("app-1"));
                assert_eq!(namespace, "web");
                assert_eq!(action_name, "PostsController#index");
            }
            _ => panic!("samples alias did not parse as traces list"),
        }
    }

    #[test]
    fn incidents_show_requires_app_reference() {
        let err =
            match Cli::try_parse_from(["appsignal-cli", "incidents", "show", "--number", "42"]) {
                Ok(_) => panic!("incidents show unexpectedly parsed without an app reference"),
                Err(err) => err,
            };

        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
        let message = err.to_string();
        assert!(message.contains("--app-id"));
        assert!(message.contains("--app"));
    }

    #[test]
    fn feedback_parses_email_after_positional_message() {
        let cli = Cli::try_parse_from([
            "appsignal-cli",
            "feedback",
            "please add uptime monitors",
            "--email",
            "ada@example.com",
        ])
        .unwrap();

        match cli.command {
            Commands::Feedback(args) => {
                assert_eq!(args.text, vec!["please add uptime monitors"]);
                assert_eq!(args.email.as_deref(), Some("ada@example.com"));
            }
            _ => panic!("feedback command did not parse as feedback"),
        }
    }

    #[test]
    fn samples_list_requires_app_reference() {
        let err = match Cli::try_parse_from([
            "appsignal-cli",
            "samples",
            "list",
            "--namespace",
            "web",
            "--action",
            "PostsController#index",
        ]) {
            Ok(_) => panic!("samples list unexpectedly parsed without an app reference"),
            Err(err) => err,
        };

        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
        let message = err.to_string();
        assert!(message.contains("--app-id"));
        assert!(message.contains("--app"));
    }

    #[test]
    fn samples_list_page_all_parses() {
        let cli = Cli::try_parse_from([
            "appsignal-cli",
            "samples",
            "list",
            "--app-id",
            "app-1",
            "--namespace",
            "web",
            "--action",
            "PostsController#index",
            "--page-all",
        ])
        .unwrap();

        match cli.command {
            Commands::Traces {
                action: TracesAction::List { page_all, .. },
            } => assert!(page_all),
            _ => panic!("samples list did not parse as traces list"),
        }
    }

    #[test]
    fn samples_incident_command_parses() {
        let cli = Cli::try_parse_from([
            "appsignal-cli",
            "samples",
            "incident",
            "--app-id",
            "app-1",
            "--number",
            "42",
        ])
        .unwrap();

        assert_eq!(
            cli.telemetry_command(),
            telemetry::TelemetryCommand::TracesIncident
        );
        match cli.command {
            Commands::Traces {
                action: TracesAction::Incident { app, number, .. },
            } => {
                assert_eq!(app.app_id.as_deref(), Some("app-1"));
                assert_eq!(number, 42);
            }
            _ => panic!("samples incident did not parse as traces incident"),
        }
    }

    #[test]
    fn samples_errors_command_parses() {
        let cli = Cli::try_parse_from([
            "appsignal-cli",
            "samples",
            "errors",
            "--app-id",
            "app-1",
            "--digest",
            "digest-1",
            "--page-all",
        ])
        .unwrap();

        assert_eq!(
            cli.telemetry_command(),
            telemetry::TelemetryCommand::TracesErrors
        );
        match cli.command {
            Commands::Traces {
                action:
                    TracesAction::Errors {
                        app,
                        digest,
                        page_all,
                        ..
                    },
            } => {
                assert_eq!(app.app_id.as_deref(), Some("app-1"));
                assert_eq!(digest, "digest-1");
                assert!(page_all);
            }
            _ => panic!("samples errors did not parse as traces errors"),
        }
    }

    #[test]
    fn samples_show_error_command_parses() {
        let cli = Cli::try_parse_from([
            "appsignal-cli",
            "samples",
            "show-error",
            "--app-id",
            "app-1",
            "--digest",
            "digest-1",
            "--trace-id",
            "trace-1",
        ])
        .unwrap();

        assert_eq!(
            cli.telemetry_command(),
            telemetry::TelemetryCommand::TracesShowError
        );
        match cli.command {
            Commands::Traces {
                action:
                    TracesAction::ShowError {
                        app,
                        digest,
                        trace_id,
                        ..
                    },
            } => {
                assert_eq!(app.app_id.as_deref(), Some("app-1"));
                assert_eq!(digest, "digest-1");
                assert_eq!(trace_id, "trace-1");
            }
            _ => panic!("samples show-error did not parse as traces show-error"),
        }
    }

    #[test]
    fn samples_show_incident_command_parses() {
        let cli = Cli::try_parse_from([
            "appsignal-cli",
            "samples",
            "show-incident",
            "--app-id",
            "app-1",
            "--number",
            "42",
            "--trace-id",
            "trace-1",
        ])
        .unwrap();

        assert_eq!(
            cli.telemetry_command(),
            telemetry::TelemetryCommand::TracesShowIncident
        );
        match cli.command {
            Commands::Traces {
                action:
                    TracesAction::ShowIncident {
                        app,
                        number,
                        trace_id,
                        ..
                    },
            } => {
                assert_eq!(app.app_id.as_deref(), Some("app-1"));
                assert_eq!(number, 42);
                assert_eq!(trace_id, "trace-1");
            }
            _ => panic!("samples show-incident did not parse as traces show-incident"),
        }
    }

    #[test]
    fn telemetry_command_maps_deeply_nested_log_trigger_commands() {
        let cli = Cli {
            output: Output::Json,
            command: Commands::Logs {
                action: LogsAction::Triggers {
                    action: LogTriggerAction::Update {
                        app: LogActionAppArgs {
                            app_id: None,
                            app: None,
                            environment: None,
                            org: None,
                        },
                        id: "trigger_rule_123".to_string(),
                        name: None,
                        query: None,
                        source_ids: Vec::new(),
                        clear_sources: false,
                        description: None,
                        clear_description: false,
                        notifier_ids: Vec::new(),
                        clear_notifiers: false,
                        severities: Vec::new(),
                        clear_severities: false,
                    },
                },
            },
        };

        assert_eq!(
            cli.telemetry_command(),
            telemetry::TelemetryCommand::LogsTriggersUpdate
        );
    }
}
