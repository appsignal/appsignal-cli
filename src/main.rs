mod api;
mod commands;
mod config;
mod error;
mod oauth;
mod output;
mod version_check;

use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand};

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
    /// List and manage anomaly detection triggers
    Triggers {
        #[command(subcommand)]
        action: TriggerAction,
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
    /// Authenticate with AppSignal (personal token or OAuth)
    Login {
        /// Your AppSignal personal API token
        #[arg(long, conflicts_with = "oauth")]
        token: Option<String>,
        /// Authenticate via OAuth (opens your browser)
        #[arg(long)]
        oauth: bool,
        /// Override the AppSignal base URL (for example `https://staging.lol`)
        #[arg(long)]
        endpoint: Option<String>,
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
    /// Show resources for an app
    Resources {
        #[command(subcommand)]
        action: AppResourceAction,
    },
}

#[derive(Args)]
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
    /// Search log lines (one-shot query). Use --output json or --format json for machine-readable output.
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let output = cli.output;

    if let Err(err) = run(cli).await {
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
                token,
                oauth,
                endpoint,
                oauth_client_id,
                org,
            } => {
                commands::auth::login(
                    commands::auth::LoginOptions {
                        token,
                        use_oauth: oauth,
                        endpoint,
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
            AppsAction::List { org } => commands::apps::list(&org, cli.output).await?,
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
            AppsAction::Orgs => commands::apps::orgs(cli.output).await?,
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
                        &["users"],
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
                        &["notifiers"],
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
                        &["namespaces"],
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
                        &["dashboards"],
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
                        &["deploy_markers"],
                        cli.output,
                    )
                    .await?
                }
            },
        },
        Commands::Project { action } => match action {
            ProjectAction::Init {
                endpoint,
                oauth_client_id,
                org,
            } => commands::project::init(
                commands::project::InitOptions {
                    endpoint,
                    oauth_client_id,
                    org,
                },
                cli.output,
            )?,
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
