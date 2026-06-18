mod api;
mod appsignal_url;
mod client_headers;
mod commands;
mod config;
mod error;
mod oauth;
mod output;
mod sample_analysis;
mod telemetry;
mod version_check;

use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand};

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
    /// Fetch transaction samples behind an incident
    Samples {
        #[command(subcommand)]
        action: SamplesAction,
    },
    /// Discover metric keys and pull raw metric timeseries
    Metrics {
        #[command(subcommand)]
        action: MetricsAction,
    },
    /// Rank slow performance actions and the queries behind them
    Performance {
        #[command(subcommand)]
        action: PerformanceAction,
    },
    /// Stream, search, and inspect application logs
    Logs {
        #[command(subcommand)]
        action: LogsAction,
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

#[derive(Args)]
struct SamplesAppArgs {
    /// Application ID (alternative to --app + --environment).
    /// Taken from the URL automatically when a reference is given.
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
enum SamplesAction {
    /// Show a single sample for an incident (latest, by id, or by timestamp)
    Show {
        /// An AppSignal incident/sample URL or a sample id. When given, it
        /// supplies the app, incident, and which sample to fetch.
        reference: Option<String>,
        #[command(flatten)]
        app: SamplesAppArgs,
        /// Incident number (when not using a URL reference)
        #[arg(long)]
        incident: Option<i64>,
        /// Fetch a specific sample by id
        #[arg(long)]
        sample_id: Option<String>,
        /// Fetch the sample closest to this ISO-8601 timestamp
        #[arg(long)]
        at: Option<String>,
        /// Show the unprocessed sample instead of the analysed digest
        #[arg(long)]
        raw: bool,
    },
    /// List the samples for an incident, or scan a time window across incidents
    List {
        /// An AppSignal incident URL (supplies the app and incident)
        reference: Option<String>,
        #[command(flatten)]
        app: SamplesAppArgs,
        /// Incident number. Omit to scan multiple incidents in a time window
        /// (requires --start and --end).
        #[arg(long)]
        incident: Option<i64>,
        /// Only include samples at or after this ISO-8601 timestamp
        #[arg(long)]
        start: Option<String>,
        /// Only include samples at or before this ISO-8601 timestamp
        #[arg(long)]
        end: Option<String>,
        /// With --incident: max samples. In window mode: max incidents to scan.
        #[arg(long)]
        limit: Option<i64>,
        /// Window mode only: filter scanned incidents by namespace
        /// (comma-separated, e.g. "web,background"). Ignored with --incident.
        #[arg(long)]
        namespaces: Option<String>,
        /// Only keep samples whose user identity matches (id, email, or substring)
        #[arg(long)]
        user: Option<String>,
    },
}

#[derive(Args)]
struct MetricAppArgs {
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
enum MetricsAction {
    /// Discover metric keys for an app
    List {
        #[command(flatten)]
        app: MetricAppArgs,
        /// Filter metric keys by name (substring)
        #[arg(long)]
        name: Option<String>,
        /// Maximum number of metric keys to return
        #[arg(long)]
        limit: Option<i64>,
    },
    /// Fetch a metric's timeseries over a window
    Timeseries {
        #[command(flatten)]
        app: MetricAppArgs,
        /// Metric name (see `metrics list`)
        #[arg(long)]
        metric: String,
        /// Field(s) to fetch, e.g. COUNTER, MEAN, P90 (repeatable or comma-separated)
        #[arg(long, value_delimiter = ',')]
        field: Vec<String>,
        /// Tag filter as key=value (repeatable or comma-separated)
        #[arg(long, value_delimiter = ',')]
        tag: Vec<String>,
        /// Relative window enum, e.g. R1H or R7D (alternative to --start/--end)
        #[arg(long, conflicts_with_all = ["start", "end"])]
        timeframe: Option<String>,
        /// Window start (ISO-8601); use with --end
        #[arg(long)]
        start: Option<String>,
        /// Window end (ISO-8601); use with --start
        #[arg(long)]
        end: Option<String>,
    },
    /// Error and performance datapoints over a historical window
    History {
        #[command(flatten)]
        app: MetricAppArgs,
        /// Window start (ISO-8601)
        #[arg(long)]
        start: String,
        /// Window end (ISO-8601)
        #[arg(long)]
        end: String,
        /// Namespaces to include (comma-separated, e.g. "web,background")
        #[arg(long, value_delimiter = ',', default_value = "web")]
        namespaces: Vec<String>,
    },
}

#[derive(Args)]
struct PerformanceAppArgs {
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
    /// Namespaces to include (comma-separated, e.g. "web,background")
    #[arg(long, value_delimiter = ',')]
    namespaces: Vec<String>,
    /// Only consider this action name
    #[arg(long)]
    action: Option<String>,
    /// Incident state filter (e.g. OPEN, CLOSED)
    #[arg(long)]
    state: Option<String>,
}

#[derive(Subcommand)]
enum PerformanceAction {
    /// Rank recent performance incidents by mean/total duration or throughput
    Actions {
        #[command(flatten)]
        app: PerformanceAppArgs,
        /// Metric to rank by
        #[arg(long, value_enum, default_value_t = commands::performance::PerfSort::Mean)]
        sort: commands::performance::PerfSort,
        /// Number of recent performance incidents to scan and rank
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    /// Surface the slow queries and N+1 suspects behind the slowest actions
    Queries {
        #[command(flatten)]
        app: PerformanceAppArgs,
        /// Number of slowest actions to drill into (one sample fetched per action)
        #[arg(long, default_value_t = 5)]
        limit: i64,
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
    Self::Samples { action } => action.telemetry_command(),
    Self::Metrics { action } => action.telemetry_command(),
    Self::Performance { action } => action.telemetry_command(),
    Self::Logs { action } => action.telemetry_command(),
    Self::Dashboards { action } => action.telemetry_command(),
    Self::Triggers { action } => action.telemetry_command(),
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

impl_telemetry_command!(SamplesAction {
    Self::Show { .. } => telemetry::TelemetryCommand::SamplesShow,
    Self::List { .. } => telemetry::TelemetryCommand::SamplesList
});

impl_telemetry_command!(MetricsAction {
    Self::List { .. } => telemetry::TelemetryCommand::MetricsList,
    Self::Timeseries { .. } => telemetry::TelemetryCommand::MetricsTimeseries,
    Self::History { .. } => telemetry::TelemetryCommand::MetricsHistory
});

impl_telemetry_command!(PerformanceAction {
    Self::Actions { .. } => telemetry::TelemetryCommand::PerformanceActions,
    Self::Queries { .. } => telemetry::TelemetryCommand::PerformanceQueries
});

impl_telemetry_command!(LogsAction {
    Self::Tail { .. } => telemetry::TelemetryCommand::LogsTail,
    Self::Search { .. } => telemetry::TelemetryCommand::LogsSearch,
    Self::Views { .. } => telemetry::TelemetryCommand::LogsViews,
    Self::Sources { .. } => telemetry::TelemetryCommand::LogsSources,
    Self::Metrics { action } => action.telemetry_command(),
    Self::Triggers { action } => action.telemetry_command()
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
        Commands::Samples { action } => match action {
            SamplesAction::Show {
                reference,
                app,
                incident,
                sample_id,
                at,
                raw,
            } => {
                commands::samples::show(
                    reference.as_deref(),
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    incident,
                    sample_id.as_deref(),
                    at.as_deref(),
                    raw,
                    cli.output,
                )
                .await?
            }
            SamplesAction::List {
                reference,
                app,
                incident,
                start,
                end,
                limit,
                namespaces,
                user,
            } => {
                commands::samples::list(
                    reference.as_deref(),
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    incident,
                    start.as_deref(),
                    end.as_deref(),
                    limit,
                    namespaces.as_deref(),
                    user.as_deref(),
                    cli.output,
                )
                .await?
            }
        },
        Commands::Metrics { action } => match action {
            MetricsAction::List { app, name, limit } => {
                commands::metrics::list(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    name.as_deref(),
                    limit,
                    cli.output,
                )
                .await?
            }
            MetricsAction::Timeseries {
                app,
                metric,
                field,
                tag,
                timeframe,
                start,
                end,
            } => {
                commands::metrics::timeseries(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    &metric,
                    &field,
                    &tag,
                    timeframe.as_deref(),
                    start.as_deref(),
                    end.as_deref(),
                    cli.output,
                )
                .await?
            }
            MetricsAction::History {
                app,
                start,
                end,
                namespaces,
            } => {
                commands::metrics::history(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    &start,
                    &end,
                    &namespaces,
                    cli.output,
                )
                .await?
            }
        },
        Commands::Performance { action } => match action {
            PerformanceAction::Actions { app, sort, limit } => {
                commands::performance::actions(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    &app.namespaces,
                    app.action.as_deref(),
                    app.state.as_deref(),
                    sort,
                    limit,
                    cli.output,
                )
                .await?
            }
            PerformanceAction::Queries { app, limit } => {
                commands::performance::queries(
                    app.app_id.as_deref(),
                    app.app.as_deref(),
                    app.environment.as_deref(),
                    app.org.as_deref(),
                    &app.namespaces,
                    app.action.as_deref(),
                    app.state.as_deref(),
                    limit,
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
