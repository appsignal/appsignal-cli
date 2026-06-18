use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Context, Result};
use reqwest::{Client, Method, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::client_headers::with_appsignal_headers;
use crate::config::AuthMethod;
use crate::error::CliError;

const DEFAULT_BASE_URL: &str = "https://appsignal.com";
const ACCOUNT_RESTRICTED_CODE: &str = "ACCOUNT_RESTRICTED";
const OAUTH_SCOPE_MESSAGE: &str =
    "Your OAuth token does not have the required scope for this operation.";

fn normalize_api_base_url(endpoint: Option<&str>) -> String {
    let endpoint = endpoint.unwrap_or(DEFAULT_BASE_URL);

    match url::Url::parse(endpoint) {
        Ok(mut url) => {
            if matches!(url.path(), "" | "/" | "/graphql") {
                url.set_path("");
            }
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => endpoint.to_string(),
    }
}

fn join_api_url(base_url: &str, path: &str) -> String {
    match url::Url::parse(base_url) {
        Ok(mut url) => {
            url.set_path(path);
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => format!("{}{}", base_url.trim_end_matches('/'), path),
    }
}

/// Client for the AppSignal API.
pub struct AppSignalClient {
    http: Client,
    auth: AuthMethod,
    base_url: String,
    rest_base_url: String,
}

// -- GraphQL response types --

#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
    extensions: Option<GraphQLErrorExtensions>,
}

#[derive(Debug, Deserialize)]
struct GraphQLErrorExtensions {
    code: Option<String>,
}

// -- App types --

#[derive(Debug, Deserialize, Serialize)]
pub struct App {
    pub id: String,
    pub name: Option<String>,
    pub environment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrganizationData {
    organization: Option<Organization>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Organization {
    slug: Option<String>,
    name: Option<String>,
    apps: Option<Vec<App>>,
}

#[derive(Debug, Deserialize)]
struct AppData {
    app: Option<App>,
}

#[derive(Debug, Deserialize)]
struct TokenInfoData {
    account: Option<TokenInfoAccount>,
}

#[derive(Debug, Deserialize)]
struct TokenInfoAccount {
    slug: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CurrentUserData {
    viewer: Option<User>,
}

// -- Shared types --

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KeyStringValue {
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TriggerSummary {
    pub id: String,
    pub name: String,
    #[serde(rename = "metricName")]
    pub metric_name: String,
    pub kind: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TriggerReference {
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ThresholdCondition {
    pub value: f64,
    #[serde(rename = "comparisonOperator")]
    pub comparison_operator: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Trigger {
    pub id: String,
    pub name: String,
    #[serde(rename = "metricName")]
    pub metric_name: String,
    pub field: String,
    #[serde(rename = "dashboardId")]
    pub dashboard_id: Option<String>,
    pub kind: String,
    #[serde(rename = "warmupDuration")]
    pub warmup_duration: i64,
    #[serde(rename = "cooldownDuration")]
    pub cooldown_duration: i64,
    pub description: Option<String>,
    #[serde(rename = "noMatchIsZero")]
    pub no_match_is_zero: bool,
    pub format: Option<String>,
    #[serde(rename = "formatInput")]
    pub format_input: Option<String>,
    #[serde(rename = "previousTrigger")]
    pub previous_trigger: Option<TriggerReference>,
    #[serde(rename = "thresholdCondition")]
    pub threshold_condition: ThresholdCondition,
    pub notifiers: Option<Vec<Notifier>>,
    pub tags: Option<Vec<KeyStringValue>>,
    pub user: Option<User>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct User {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Notifier {
    pub id: String,
    pub name: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Dashboard {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub label: Option<String>,
    pub source: Option<DashboardSource>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppResourceSection {
    Users,
    Notifiers,
    Namespaces,
    Dashboards,
    DeployMarkers,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DashboardSource {
    UserCreated,
}

impl DashboardSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserCreated => "USER_CREATED",
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Namespace {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeployMarker {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "shortRevision")]
    pub short_revision: Option<String>,
    pub revision: Option<String>,
    #[serde(rename = "gitCompareUrl")]
    pub git_compare_url: Option<String>,
    pub user: Option<String>,
    #[serde(rename = "liveForInWords")]
    pub live_for_in_words: Option<String>,
    #[serde(rename = "liveFor")]
    pub live_for: Option<i64>,
    #[serde(rename = "exceptionCount")]
    pub exception_count: Option<i64>,
    #[serde(rename = "exceptionRate")]
    pub exception_rate: Option<f64>,
}

/// Resources available for an application.
#[derive(Debug, Default, Serialize)]
pub struct AppResources {
    pub users: Option<Vec<User>>,
    pub notifiers: Option<Vec<Notifier>>,
    pub namespaces: Option<Vec<Namespace>>,
    pub dashboards: Option<Vec<Dashboard>>,
    pub deploy_markers: Option<Vec<DeployMarker>>,
}

// -- Incident types --

/// Incident types returned by the AppSignal GraphQL API.
/// Variant names must match the GraphQL `__typename` values exactly.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "__typename")]
#[allow(clippy::enum_variant_names)]
pub enum Incident {
    ExceptionIncident {
        id: String,
        number: i64,
        state: Option<String>,
        severity: Option<String>,
        description: Option<String>,
        count: i64,
        #[serde(rename = "createdAt")]
        created_at: Option<String>,
        #[serde(rename = "lastOccurredAt")]
        last_occurred_at: Option<String>,
        #[serde(rename = "updatedAt")]
        updated_at: Option<String>,
        #[serde(rename = "exceptionName")]
        exception_name: Option<String>,
        #[serde(rename = "exceptionMessage")]
        exception_message: Option<String>,
        #[serde(rename = "actionNames")]
        action_names: Option<Vec<String>>,
        namespace: Option<String>,
        #[serde(rename = "firstBacktraceLine")]
        first_backtrace_line: Option<String>,
        assignees: Option<Vec<User>>,
    },
    PerformanceIncident {
        id: String,
        number: i64,
        state: Option<String>,
        severity: Option<String>,
        description: Option<String>,
        count: i64,
        #[serde(rename = "createdAt")]
        created_at: Option<String>,
        #[serde(rename = "lastOccurredAt")]
        last_occurred_at: Option<String>,
        #[serde(rename = "updatedAt")]
        updated_at: Option<String>,
        #[serde(rename = "actionNames")]
        action_names: Option<Vec<String>>,
        namespace: Option<String>,
        mean: Option<f64>,
        #[serde(rename = "totalDuration")]
        total_duration: Option<f64>,
        assignees: Option<Vec<User>>,
    },
    AnomalyIncident {
        id: String,
        number: i64,
        state: Option<String>,
        severity: Option<String>,
        description: Option<String>,
        count: i64,
        #[serde(rename = "createdAt")]
        created_at: Option<String>,
        #[serde(rename = "lastOccurredAt")]
        last_occurred_at: Option<String>,
        #[serde(rename = "updatedAt")]
        updated_at: Option<String>,
        #[serde(rename = "alertState")]
        alert_state: Option<String>,
        trigger: Option<TriggerSummary>,
        tags: Option<Vec<KeyStringValue>>,
        assignees: Option<Vec<User>>,
    },
    LogIncident {
        id: String,
        number: i64,
        state: Option<String>,
        severity: Option<String>,
        description: Option<String>,
        count: i64,
        #[serde(rename = "createdAt")]
        created_at: Option<String>,
        #[serde(rename = "lastOccurredAt")]
        last_occurred_at: Option<String>,
        #[serde(rename = "updatedAt")]
        updated_at: Option<String>,
        assignees: Option<Vec<User>>,
    },
}

impl Incident {
    pub fn id(&self) -> &str {
        match self {
            Incident::ExceptionIncident { id, .. }
            | Incident::PerformanceIncident { id, .. }
            | Incident::AnomalyIncident { id, .. }
            | Incident::LogIncident { id, .. } => id,
        }
    }

    pub fn number(&self) -> i64 {
        match self {
            Incident::ExceptionIncident { number, .. }
            | Incident::PerformanceIncident { number, .. }
            | Incident::AnomalyIncident { number, .. }
            | Incident::LogIncident { number, .. } => *number,
        }
    }

    pub fn state(&self) -> &str {
        match self {
            Incident::ExceptionIncident { state, .. }
            | Incident::PerformanceIncident { state, .. }
            | Incident::AnomalyIncident { state, .. }
            | Incident::LogIncident { state, .. } => state.as_deref().unwrap_or("-"),
        }
    }

    pub fn severity(&self) -> &str {
        match self {
            Incident::ExceptionIncident { severity, .. }
            | Incident::PerformanceIncident { severity, .. }
            | Incident::AnomalyIncident { severity, .. }
            | Incident::LogIncident { severity, .. } => severity.as_deref().unwrap_or("-"),
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Incident::ExceptionIncident { description, .. }
            | Incident::PerformanceIncident { description, .. }
            | Incident::AnomalyIncident { description, .. }
            | Incident::LogIncident { description, .. } => description.as_deref().unwrap_or("-"),
        }
    }

    pub fn count(&self) -> i64 {
        match self {
            Incident::ExceptionIncident { count, .. }
            | Incident::PerformanceIncident { count, .. }
            | Incident::AnomalyIncident { count, .. }
            | Incident::LogIncident { count, .. } => *count,
        }
    }

    pub fn last_occurred_at(&self) -> &str {
        match self {
            Incident::ExceptionIncident {
                last_occurred_at, ..
            }
            | Incident::PerformanceIncident {
                last_occurred_at, ..
            }
            | Incident::AnomalyIncident {
                last_occurred_at, ..
            }
            | Incident::LogIncident {
                last_occurred_at, ..
            } => last_occurred_at.as_deref().unwrap_or("-"),
        }
    }

    pub fn created_at(&self) -> &str {
        match self {
            Incident::ExceptionIncident { created_at, .. }
            | Incident::PerformanceIncident { created_at, .. }
            | Incident::AnomalyIncident { created_at, .. }
            | Incident::LogIncident { created_at, .. } => created_at.as_deref().unwrap_or("-"),
        }
    }

    pub fn kind(&self) -> &str {
        match self {
            Incident::ExceptionIncident { .. } => "exception",
            Incident::PerformanceIncident { .. } => "performance",
            Incident::AnomalyIncident { .. } => "anomaly",
            Incident::LogIncident { .. } => "log",
        }
    }

    pub fn assignees(&self) -> &[User] {
        match self {
            Incident::ExceptionIncident { assignees, .. }
            | Incident::PerformanceIncident { assignees, .. }
            | Incident::AnomalyIncident { assignees, .. }
            | Incident::LogIncident { assignees, .. } => assignees.as_deref().unwrap_or(&[]),
        }
    }

    pub fn assignee_ids(&self) -> Vec<String> {
        self.assignees().iter().map(|u| u.id.clone()).collect()
    }

    /// Namespace for incident kinds that carry one (exception/performance).
    pub fn namespace(&self) -> Option<&str> {
        match self {
            Incident::ExceptionIncident { namespace, .. }
            | Incident::PerformanceIncident { namespace, .. } => namespace.as_deref(),
            _ => None,
        }
    }

    /// Action names for incident kinds that carry them (exception/performance).
    pub fn action_names(&self) -> &[String] {
        match self {
            Incident::ExceptionIncident { action_names, .. }
            | Incident::PerformanceIncident { action_names, .. } => {
                action_names.as_deref().unwrap_or(&[])
            }
            _ => &[],
        }
    }

    /// Mean request duration (ms) — performance incidents only.
    pub fn mean(&self) -> Option<f64> {
        match self {
            Incident::PerformanceIncident { mean, .. } => *mean,
            _ => None,
        }
    }

    /// Total request duration (ms) across occurrences — performance incidents only.
    pub fn total_duration(&self) -> Option<f64> {
        match self {
            Incident::PerformanceIncident { total_duration, .. } => *total_duration,
            _ => None,
        }
    }
}

// -- Sample types --

/// A transaction sample: the raw per-request data behind an incident.
///
/// Performance and exception samples share most fields; the type-specific ones
/// (`exception`/`error_causes` for errors, `has_n_plus_one` for performance)
/// are optional and only populated for the relevant sample type. Every field is
/// optional so the same struct deserializes either shape and tolerates the API
/// omitting fields.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Sample {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Total request duration, in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// Time spent queued before processing, in milliseconds.
    #[serde(rename = "queueDuration", skip_serializing_if = "Option::is_none")]
    pub queue_duration: Option<f64>,
    /// When the sample was recorded (ISO-8601).
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(rename = "originalId", skip_serializing_if = "Option::is_none")]
    pub original_id: Option<String>,
    /// Performance samples only: whether AppSignal detected an N+1 query.
    #[serde(rename = "hasNPlusOne", skip_serializing_if = "Option::is_none")]
    pub has_n_plus_one: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<KeyStringValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<Vec<KeyStringValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<Vec<KeyStringValue>>,
    /// Request parameters, session data, and custom data — arbitrary JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    #[serde(rename = "sessionData", skip_serializing_if = "Option::is_none")]
    pub session_data: Option<serde_json::Value>,
    #[serde(rename = "customData", skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<serde_json::Value>,
    /// Performance samples only: the event timeline (one entry per instrumented
    /// span). The richest source for the digest's breakdown and slow queries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<Vec<TimelineEvent>>,
    /// Performance samples only: number of timeline events dropped for size.
    #[serde(
        rename = "timelineTruncatedEvents",
        skip_serializing_if = "Option::is_none"
    )]
    pub timeline_truncated_events: Option<i64>,
    /// Performance samples only: total duration per event group.
    #[serde(rename = "groupDurations", skip_serializing_if = "Option::is_none")]
    pub group_durations: Option<Vec<KeyStringValue>>,
    /// Performance samples only: total allocations per event group.
    #[serde(rename = "groupAllocations", skip_serializing_if = "Option::is_none")]
    pub group_allocations: Option<Vec<KeyStringValue>>,
    /// Exception samples only: the raised error and its backtrace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception: Option<ExceptionDetail>,
    /// Exception samples only: the chain of underlying causes.
    #[serde(rename = "errorCauses", skip_serializing_if = "Option::is_none")]
    pub error_causes: Option<Vec<ErrorCause>>,
    /// Exception samples only: the trail of events leading to the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breadcrumbs: Option<Vec<Breadcrumb>>,
}

/// One event in a performance sample's timeline.
///
/// Only the fields the digest consumes are modelled; the AppSignal timeline
/// type carries more (allocation counts, relative offsets) that we don't select.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TimelineEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// The instrumentation group, e.g. `sql.active_record`, `view.render`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Duration of this event, in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
    /// A fingerprint shared by structurally identical events (e.g. the same
    /// query). Repeated digests are the signal for N+1 detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<TimelinePayload>,
}

/// The payload of a timeline event — for queries, `body` holds the statement.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TimelinePayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// A breadcrumb: an event recorded before an error occurred.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Breadcrumb {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<KeyStringValue>>,
}

/// The exception raised in an error sample.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExceptionDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtrace: Option<Vec<BacktraceLine>>,
}

/// A single frame of an exception backtrace.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BacktraceLine {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// An entry in an error sample's cause chain.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ErrorCause {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Which sample to fetch for an incident.
#[derive(Debug, Clone, Copy)]
pub enum SampleQuery<'a> {
    /// The most recent sample.
    Latest,
    /// A specific sample by id.
    Id(&'a str),
    /// The sample closest to an ISO-8601 timestamp.
    Timestamp(&'a str),
}

/// A sample together with the incident and incident type it belongs to.
#[derive(Debug, Serialize)]
pub struct IncidentSample {
    pub incident_number: i64,
    /// `"performance"` or `"error"`, taken from the incident `__typename`.
    #[serde(rename = "type")]
    pub sample_type: String,
    pub sample: Sample,
}

/// Samples for an incident together with the incident type.
#[derive(Debug, Serialize)]
pub struct IncidentSamples {
    pub incident_number: i64,
    #[serde(rename = "type")]
    pub sample_type: String,
    pub samples: Vec<Sample>,
}

/// A sample paired with the incident it belongs to, produced by a window scan
/// across multiple incidents.
#[derive(Debug, Serialize)]
pub struct WindowSample {
    pub incident_number: i64,
    #[serde(rename = "type")]
    pub sample_type: String,
    pub sample: Sample,
}

#[derive(Debug, Deserialize)]
struct AppIncidentSampleData {
    app: Option<AppIncidentSample>,
}

#[derive(Debug, Deserialize)]
struct AppIncidentSample {
    incident: Option<IncidentSampleEnvelope>,
}

/// The `incident` object after fragment flattening: `__typename` is always
/// present, and `number`/`sample`/`samples` come from whichever incident-type
/// fragment matched (so they are absent for anomaly/log incidents, which have
/// no samples).
#[derive(Debug, Deserialize)]
struct IncidentSampleEnvelope {
    #[serde(rename = "__typename")]
    typename: String,
    number: Option<i64>,
    sample: Option<Sample>,
    samples: Option<Vec<Sample>>,
}

/// Unwrap the `app.incident` envelope from a sample query, mapping the
/// "missing app" and "missing incident" cases to user-facing errors.
fn incident_envelope(
    data: AppIncidentSampleData,
    incident_number: i64,
) -> Result<IncidentSampleEnvelope> {
    let app = data.app.context(CliError::msg("Application not found"))?;
    app.incident
        .with_context(|| CliError::msg(format!("Incident #{} not found", incident_number)))
}

/// Map an incident `__typename` to a sample type, rejecting incident types that
/// don't carry transaction samples.
fn incident_sample_type(typename: &str, incident_number: i64) -> Result<String> {
    match typename {
        "PerformanceIncident" => Ok("performance".to_string()),
        "ExceptionIncident" => Ok("error".to_string()),
        other => {
            let kind = match other {
                "AnomalyIncident" => "an anomaly",
                "LogIncident" => "a log",
                _ => "this kind of",
            };
            anyhow::bail!(CliError::msg(format!(
                "Incident #{} is {} incident and has no transaction samples.",
                incident_number, kind
            )))
        }
    }
}

#[derive(Debug, Deserialize)]
struct AppIncidentsData {
    app: Option<AppIncidents>,
}

#[derive(Debug, Deserialize)]
struct AppIncidents {
    incidents: Option<Vec<Incident>>,
}

#[derive(Debug, Deserialize)]
struct AppIncidentData {
    app: Option<AppSingleIncident>,
}

#[derive(Debug, Deserialize)]
struct AppSingleIncident {
    incident: Option<Incident>,
}

#[derive(Debug, Deserialize)]
struct AppExceptionIncidentsData {
    app: Option<AppExceptionIncidents>,
}

#[derive(Debug, Deserialize)]
struct AppExceptionIncidents {
    #[serde(rename = "exceptionIncidents")]
    exception_incidents: Option<Vec<Incident>>,
}

#[derive(Debug, Deserialize)]
struct AppAnomalyIncidentsData {
    app: Option<AppAnomalyIncidents>,
}

#[derive(Debug, Deserialize)]
struct AppAnomalyIncidents {
    #[serde(rename = "anomalyIncidents")]
    anomaly_incidents: Option<Vec<Incident>>,
}

#[derive(Debug, Deserialize)]
struct AppPerformanceIncidentsData {
    app: Option<AppPerformanceIncidents>,
}

#[derive(Debug, Deserialize)]
struct AppPerformanceIncidents {
    #[serde(rename = "performanceIncidents")]
    performance_incidents: Option<Vec<Incident>>,
}

#[derive(Debug, Deserialize)]
struct AppTriggersData {
    app: Option<AppTriggers>,
}

#[derive(Debug, Deserialize)]
struct AppTriggers {
    triggers: Option<Vec<Trigger>>,
}

// -- App resource response types --

#[derive(Debug, Deserialize)]
struct AppUsersData {
    app: Option<AppUsers>,
}

#[derive(Debug, Deserialize)]
struct AppUsers {
    users: Option<Vec<User>>,
}

#[derive(Debug, Deserialize)]
struct AppResourcesData {
    app: Option<AppResourcesInner>,
}

#[derive(Debug, Deserialize)]
struct AppResourcesInner {
    users: Option<Vec<User>>,
    notifiers: Option<Vec<Notifier>>,
    namespaces: Option<Vec<Namespace>>,
    dashboards: Option<Vec<Dashboard>>,
    #[serde(rename = "deployMarkers")]
    deploy_markers: Option<Vec<DeployMarker>>,
}

// -- Mutation response types --

#[derive(Debug, Deserialize)]
struct UpdateIncidentData {
    #[serde(rename = "updateIncident")]
    update_incident: Option<Incident>,
}

#[derive(Debug, Deserialize)]
struct BulkUpdateIncidentsData {
    #[serde(rename = "bulkUpdateIncidents")]
    bulk_update_incidents: Option<Vec<Incident>>,
}

#[derive(Debug, Deserialize)]
struct CreateIncidentNoteData {
    #[serde(rename = "createIncidentNote")]
    create_incident_note: Option<Incident>,
}

#[derive(Debug, Deserialize)]
struct CreateTriggerData {
    #[serde(rename = "createTrigger")]
    create_trigger: Option<Trigger>,
}

#[derive(Debug, Deserialize)]
struct ArchiveTriggerData {
    #[serde(rename = "archiveTrigger")]
    archive_trigger: Option<Trigger>,
}

#[derive(Debug, Deserialize)]
struct CreateDashboardData {
    #[serde(rename = "createDashboard")]
    create_dashboard: Option<Dashboard>,
}

#[derive(Debug, Deserialize)]
struct UpdateDashboardData {
    #[serde(rename = "updateDashboard")]
    update_dashboard: Option<Dashboard>,
}

#[derive(Debug, Deserialize)]
struct AppLogLineActionsData {
    app: Option<AppLogLineActions>,
}

#[derive(Debug, Deserialize)]
struct AppLogLineActions {
    logs: Option<LogLineActionsInner>,
}

#[derive(Debug, Deserialize)]
struct LogLineActionsInner {
    #[serde(rename = "logLineActions")]
    log_line_actions: Option<Vec<LogLineAction>>,
}

#[derive(Debug, Deserialize)]
struct CreateLogLineActionData {
    #[serde(rename = "createLogLineAction")]
    create_log_line_action: Option<LogLineAction>,
}

#[derive(Debug, Deserialize)]
struct UpdateLogLineActionData {
    #[serde(rename = "updateLogLineAction")]
    update_log_line_action: Option<LogLineAction>,
}

#[derive(Debug, Deserialize)]
struct DeleteLogLineActionData {
    #[serde(rename = "deleteLogLineAction")]
    delete_log_line_action: Option<LogLineAction>,
}

const TRIGGER_SELECTION: &str = r#"
    id
    name
    metricName
    field
    dashboardId
    kind
    warmupDuration
    cooldownDuration
    description
    noMatchIsZero
    format
    formatInput
    previousTrigger { id }
    thresholdCondition { value comparisonOperator }
    notifiers { id name icon }
    tags { key value }
    user { id name }
"#;

const DASHBOARD_SELECTION: &str = r#"
    id
    title
    description
    label
    source
    createdAt
    updatedAt
"#;

const LOG_LINE_ACTION_SELECTION: &str = r#"
    __typename
    ... on LogLineActionTrigger {
        id
        name
        description
        query
        sourceIds
        severities
        actionType
        sources { id name type fmt }
        order
        user { id name email }
        previousTrigger { id }
        notificationOptions
        notificationTriggerValue
        notifiers { id name icon }
    }
    ... on LogLineActionFilter {
        id
        name
        query
        sourceIds
        actionType
        sources { id name type fmt }
        order
    }
    ... on LogLineActionMetrics {
        id
        name
        query
        sourceIds
        actionType
        sources { id name type fmt }
        logLineMetrics { id name field tags metricType }
        order
        user { id name email }
    }
"#;

/// Fields common to performance and exception samples.
const COMMON_SAMPLE_FIELDS: &str = r#"
    id
    action
    namespace
    duration
    queueDuration
    createdAt
    revision
    version
    originalId
    attributes { key value }
    overview { key value }
    environment { key value }
    params
    sessionData
    customData
"#;

/// Performance-sample field selection: N+1 detection plus the event timeline
/// and per-group rollups that the digest analyses.
const PERFORMANCE_SAMPLE_SELECTION: &str = r#"
    hasNPlusOne
    timelineTruncatedEvents
    groupDurations { key value }
    groupAllocations { key value }
    timeline { name action group duration count digest payload { name body } }
"#;

/// Exception-sample field selection: the error details plus the breadcrumb
/// trail leading up to it.
const EXCEPTION_SAMPLE_SELECTION: &str = r#"
    exception { name message backtrace { line path method column original type url } }
    errorCauses { name message }
    breadcrumbs { category action message metadata { key value } }
"#;

// -- Metric types --

/// A metric key as returned by `app.metrics.keys`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MetricKey {
    pub name: String,
    /// The metric type (`gauge`, `counter`, `measurement`, …). `type` is a
    /// reserved word, so it's exposed as `kind`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<KeyStringValue>>,
    /// The field names available for this metric (e.g. `COUNTER`, `MEAN`, `P90`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,
}

/// One field selector inside a [`MetricTimeseriesInput`].
#[derive(Debug, Serialize, Clone)]
pub struct MetricFieldInput {
    pub field: String,
}

/// A tag filter inside a [`MetricTimeseriesInput`].
#[derive(Debug, Serialize, Clone)]
pub struct MetricTagInput {
    pub key: String,
    pub value: String,
}

/// One entry in the `query: [MetricTimeseries!]!` argument of
/// `app.metrics.timeseries`.
#[derive(Debug, Serialize, Clone)]
pub struct MetricTimeseriesInput {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<MetricFieldInput>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<MetricTagInput>,
}

/// The result of `app.metrics.timeseries`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MetricTimeseries {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<MetricTimeseriesKey>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<MetricTimeseriesPoint>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MetricTimeseriesKey {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<KeyStringValue>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MetricTimeseriesPoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Field name → value. Field names come back lowercased (`mean`, `p95`,
    /// `counter`), matching the wire casing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<KeyStringValue>,
}

/// Error and performance datapoints for a historical window, from the
/// `timeDetective*DataPoints` fields.
#[derive(Debug, Serialize)]
pub struct TimeDetective {
    pub errors: Vec<ErrorDataPoint>,
    pub performance: Vec<PerformanceDataPoint>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ErrorDataPoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "actionName", skip_serializing_if = "Option::is_none")]
    pub action_name: Option<String>,
    #[serde(rename = "exceptionName", skip_serializing_if = "Option::is_none")]
    pub exception_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub throughput: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PerformanceDataPoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "actionName", skip_serializing_if = "Option::is_none")]
    pub action_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub throughput: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p90: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct AppMetricKeysData {
    app: Option<AppMetricKeys>,
}

#[derive(Debug, Deserialize)]
struct AppMetricKeys {
    metrics: Option<MetricKeysHolder>,
}

#[derive(Debug, Deserialize)]
struct MetricKeysHolder {
    keys: Option<Vec<MetricKey>>,
}

#[derive(Debug, Deserialize)]
struct AppMetricTimeseriesData {
    app: Option<AppMetricTimeseries>,
}

#[derive(Debug, Deserialize)]
struct AppMetricTimeseries {
    metrics: Option<MetricTimeseriesHolder>,
}

#[derive(Debug, Deserialize)]
struct MetricTimeseriesHolder {
    timeseries: Option<MetricTimeseries>,
}

#[derive(Debug, Deserialize)]
struct TimeDetectiveData {
    app: Option<TimeDetectiveApp>,
}

#[derive(Debug, Deserialize)]
struct TimeDetectiveApp {
    #[serde(rename = "timeDetectiveErrorDataPoints")]
    errors: Option<Vec<ErrorDataPoint>>,
    #[serde(rename = "timeDetectivePerformanceDataPoints")]
    performance: Option<Vec<PerformanceDataPoint>>,
}

// -- Log types --

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LogLine {
    pub id: String,
    pub timestamp: String,
    pub severity: String,
    pub hostname: String,
    pub group: Option<String>,
    pub message: String,
    pub attributes: Option<Vec<KeyStringValue>>,
    pub source: Option<LogSourceRef>,
}

/// Lightweight source reference returned inline with log lines.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LogSourceRef {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LogSource {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub fmt: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LogView {
    pub id: String,
    pub name: String,
    pub query: Option<String>,
    #[serde(rename = "sourceIds")]
    pub source_ids: Option<Vec<String>>,
    pub severities: Option<Vec<String>>,
    pub columns: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LogLineActionTriggerReference {
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LogLineMetricDefinition {
    pub id: Option<String>,
    pub name: String,
    pub field: Option<String>,
    #[serde(default)]
    pub tags: std::collections::BTreeMap<String, String>,
    #[serde(rename = "metricType")]
    pub metric_type: String,
}

/// Which flavor of log-line action the CLI is operating on.
///
/// The GraphQL API exposes a wider set including `FILTER`, but the CLI only
/// surfaces `METRICS` and `TRIGGER` today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLineActionKind {
    Metrics,
    Trigger,
}

impl LogLineActionKind {
    /// Canonical wire value sent to the GraphQL `LogLineActionTypeEnum`.
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::Metrics => "METRICS",
            Self::Trigger => "TRIGGER",
        }
    }

    /// Match this kind against an `actionType` value returned by the API.
    pub fn matches_api(self, api_value: &str) -> bool {
        api_value.eq_ignore_ascii_case(self.as_api_str())
    }
}

impl Serialize for LogLineActionKind {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_api_str())
    }
}

/// Fields shared by every `LogLineAction` variant. Flattened into each variant
/// so the accessors don't need a match arm per kind.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LogLineActionCommon {
    pub id: String,
    pub name: String,
    pub query: String,
    #[serde(rename = "sourceIds")]
    pub source_ids: Vec<String>,
    #[serde(rename = "actionType")]
    pub action_type: String,
    pub sources: Vec<LogSource>,
    pub order: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "__typename")]
pub enum LogLineAction {
    #[serde(rename = "LogLineActionTrigger")]
    Trigger {
        #[serde(flatten)]
        common: LogLineActionCommon,
        description: Option<String>,
        severities: Vec<String>,
        user: Option<User>,
        #[serde(rename = "previousTrigger")]
        previous_trigger: Option<LogLineActionTriggerReference>,
        #[serde(rename = "notificationOptions")]
        notification_options: Option<String>,
        #[serde(rename = "notificationTriggerValue")]
        notification_trigger_value: Option<i64>,
        notifiers: Option<Vec<Notifier>>,
    },
    #[serde(rename = "LogLineActionFilter")]
    Filter {
        #[serde(flatten)]
        common: LogLineActionCommon,
    },
    #[serde(rename = "LogLineActionMetrics")]
    Metrics {
        #[serde(flatten)]
        common: LogLineActionCommon,
        #[serde(rename = "logLineMetrics")]
        log_line_metrics: Vec<LogLineMetricDefinition>,
        user: Option<User>,
    },
}

impl LogLineAction {
    pub fn common(&self) -> &LogLineActionCommon {
        match self {
            LogLineAction::Trigger { common, .. }
            | LogLineAction::Filter { common, .. }
            | LogLineAction::Metrics { common, .. } => common,
        }
    }

    pub fn id(&self) -> &str {
        &self.common().id
    }

    pub fn name(&self) -> &str {
        &self.common().name
    }

    pub fn query(&self) -> &str {
        &self.common().query
    }

    pub fn action_type(&self) -> &str {
        &self.common().action_type
    }

    pub fn order(&self) -> i64 {
        self.common().order
    }

    pub fn source_ids(&self) -> &[String] {
        &self.common().source_ids
    }

    pub fn sources(&self) -> &[LogSource] {
        &self.common().sources
    }

    pub fn trigger_description(&self) -> Option<&str> {
        match self {
            LogLineAction::Trigger { description, .. } => description.as_deref(),
            _ => None,
        }
    }

    pub fn trigger_severities(&self) -> &[String] {
        match self {
            LogLineAction::Trigger { severities, .. } => severities,
            _ => &[],
        }
    }

    pub fn trigger_notifiers(&self) -> &[Notifier] {
        match self {
            LogLineAction::Trigger { notifiers, .. } => notifiers.as_deref().unwrap_or(&[]),
            _ => &[],
        }
    }

    pub fn metrics(&self) -> &[LogLineMetricDefinition] {
        match self {
            LogLineAction::Metrics {
                log_line_metrics, ..
            } => log_line_metrics,
            _ => &[],
        }
    }
}

/// Tri-state for fields on update mutations.
///
/// `Option<T>` collapses "absent" and "explicit null" into the same value;
/// `Patch` keeps them apart so the CLI can say "don't touch this" vs "clear
/// this" without ambiguity. Field-level `#[serde(skip_serializing_if =
/// "Patch::is_unchanged")]` ensures `Unchanged` is omitted from the request
/// entirely, `Clear` serializes as JSON `null`, and `Set(v)` serializes as
/// `v`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Patch<T> {
    /// Leave the field unchanged on the server (omit from request).
    #[default]
    Unchanged,
    /// Clear the field (send JSON `null`).
    Clear,
    /// Set the field to a new value.
    Set(T),
}

impl<T> Patch<T> {
    pub fn is_unchanged(&self) -> bool {
        matches!(self, Patch::Unchanged)
    }
}

impl<T: Serialize> Serialize for Patch<T> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self {
            // `Unchanged` is normally skipped via `skip_serializing_if`; if it
            // reaches here (e.g. inside a non-skipping container), emit null.
            Patch::Unchanged | Patch::Clear => serializer.serialize_none(),
            Patch::Set(value) => value.serialize(serializer),
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct LogLineMetricInput {
    pub name: String,
    pub field: Option<String>,
    #[serde(rename = "metricType")]
    pub metric_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<std::collections::BTreeMap<String, String>>,
}

impl FromStr for LogLineMetricInput {
    type Err = String;

    /// Parse a `--metric` spec like
    /// `name=log.error_count,type=counter` or
    /// `name=log.request_duration,type=distribution,field=duration_ms,tag.hostname=web-1`.
    ///
    /// Returns a `String` error so clap can surface parse failures at
    /// argument-parse time with its standard formatting.
    fn from_str(spec: &str) -> std::result::Result<Self, Self::Err> {
        let mut name: Option<String> = None;
        let mut field: Option<String> = None;
        let mut metric_type: Option<String> = None;
        let mut tags: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();

        for part in spec.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            let (key, value) = part
                .split_once('=')
                .ok_or_else(|| format!("Invalid metric component `{}` in `{}`", part, spec))?;
            let key = key.trim();
            let value = value.trim();

            if key.eq_ignore_ascii_case("name") {
                if value.is_empty() {
                    return Err(format!("Metric `name=` in `{}` cannot be empty.", spec));
                }
                name = Some(value.to_string());
            } else if key.eq_ignore_ascii_case("field") {
                if value.is_empty() {
                    return Err(format!("Metric `field=` in `{}` cannot be empty.", spec));
                }
                field = Some(value.to_string());
            } else if key.eq_ignore_ascii_case("type") || key.eq_ignore_ascii_case("metric_type") {
                metric_type = Some(normalize_metric_type(value)?);
            } else if let Some(tag_name) = key.strip_prefix("tag.") {
                let tag_name = tag_name.trim();
                if tag_name.is_empty() {
                    return Err(format!("Metric tag keys cannot be empty in `{}`", spec));
                }
                if value.is_empty() {
                    return Err(format!(
                        "Metric tag `{}` in `{}` cannot be empty.",
                        tag_name, spec
                    ));
                }
                tags.insert(tag_name.to_string(), value.to_string());
            } else {
                return Err(format!(
                    "Unsupported metric key `{}` in `{}`. Use name=..., type=..., field=..., and tag.<name>=...",
                    key, spec
                ));
            }
        }

        let name = name.ok_or_else(|| "Metric definitions require `name=...`".to_string())?;
        let metric_type =
            metric_type.ok_or_else(|| "Metric definitions require `type=...`".to_string())?;
        if matches!(metric_type.as_str(), "GAUGE" | "DISTRIBUTION") && field.is_none() {
            return Err(format!(
                "Metric `{}` uses type `{}` and requires `field=...`.",
                name,
                metric_type.to_ascii_lowercase()
            ));
        }

        Ok(LogLineMetricInput {
            name,
            field,
            metric_type,
            tags: (!tags.is_empty()).then_some(tags),
        })
    }
}

fn normalize_metric_type(metric_type: &str) -> std::result::Result<String, String> {
    match metric_type.trim().to_ascii_lowercase().as_str() {
        "counter" => Ok("COUNTER".to_string()),
        "gauge" => Ok("GAUGE".to_string()),
        "distribution" => Ok("DISTRIBUTION".to_string()),
        other => Err(format!(
            "Unsupported metric type '{}'. Use counter, gauge, or distribution.",
            other
        )),
    }
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct LogLineActionTriggerInput {
    #[serde(skip_serializing_if = "Patch::is_unchanged")]
    pub description: Patch<String>,
    #[serde(rename = "notifierIds", skip_serializing_if = "Option::is_none")]
    pub notifier_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severities: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AppLogViewsData {
    app: Option<AppLogViews>,
}

#[derive(Debug, Deserialize)]
struct AppLogViews {
    #[serde(rename = "logViews")]
    log_views: Option<Vec<LogView>>,
}

#[derive(Debug, Deserialize)]
struct AppLogSourcesData {
    app: Option<AppLogSources>,
}

#[derive(Debug, Deserialize)]
struct AppLogSources {
    logs: Option<LogSourcesInner>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct RestLogLine {
    pub id: Option<String>,
    pub timestamp: String,
    pub source_id: Option<String>,
    pub group: Option<String>,
    pub severity: Option<String>,
    pub message: Option<String>,
    pub hostname: Option<String>,
    #[serde(default)]
    pub attributes: serde_json::Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct LogSourcesInner {
    sources: Option<Vec<LogSource>>,
}

/// Resolve user identifiers (names or IDs) to user IDs.
/// Each identifier is matched case-insensitively against user names.
/// If no name matches, the identifier is assumed to be a raw user ID.
pub fn resolve_user_ids(identifiers: &[String], users: &[User]) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for ident in identifiers {
        let ident_lower = ident.to_lowercase();
        let matches: Vec<&User> = users
            .iter()
            .filter(|u| {
                u.name
                    .as_deref()
                    .map(|n| n.to_lowercase() == ident_lower)
                    .unwrap_or(false)
            })
            .collect();

        match matches.len() {
            0 => {
                // No name match — assume it's a raw ID
                ids.push(ident.clone());
            }
            1 => ids.push(matches[0].id.clone()),
            _ => {
                let descriptions: Vec<String> = matches
                    .iter()
                    .map(|u| {
                        format!(
                            "  {} ({}, {})",
                            u.id,
                            u.name.as_deref().unwrap_or("-"),
                            u.email.as_deref().unwrap_or("-"),
                        )
                    })
                    .collect();
                anyhow::bail!(CliError::msg(format!(
                    "Multiple users match '{}'. Use an email or ID to disambiguate:\n{}",
                    ident,
                    descriptions.join("\n")
                )))
            }
        }
    }
    Ok(ids)
}

/// Filter a list of apps by name and optional environment (case-insensitive).
/// Returns exactly one match or an error explaining what went wrong.
pub fn filter_apps(
    apps: Vec<App>,
    name: &str,
    environment: Option<&str>,
    org_slug: &str,
) -> Result<App> {
    let name_lower = name.to_lowercase();
    let matching: Vec<App> = apps
        .into_iter()
        .filter(|app| {
            let name_match = app
                .name
                .as_deref()
                .map(|n| n.to_lowercase() == name_lower)
                .unwrap_or(false);
            if !name_match {
                return false;
            }
            if let Some(env) = environment {
                let env_lower = env.to_lowercase();
                app.environment
                    .as_deref()
                    .map(|e| e.to_lowercase() == env_lower)
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .collect();

    match matching.len() {
        0 => {
            let msg = if let Some(env) = environment {
                format!(
                    "No app found with name '{}' and environment '{}' in organization '{}'",
                    name, env, org_slug
                )
            } else {
                format!(
                    "No app found with name '{}' in organization '{}'",
                    name, org_slug
                )
            };
            anyhow::bail!(CliError::msg(msg))
        }
        1 => Ok(matching.into_iter().next().unwrap()),
        _ => {
            let descriptions: Vec<String> = matching
                .iter()
                .map(|a| {
                    format!(
                        "  {} ({}, {})",
                        a.id,
                        a.name.as_deref().unwrap_or("-"),
                        a.environment.as_deref().unwrap_or("-"),
                    )
                })
                .collect();
            anyhow::bail!(CliError::msg(format!(
                "Multiple apps match name '{}'. Use --environment to disambiguate:\n{}",
                name,
                descriptions.join("\n")
            )))
        }
    }
}

impl AppSignalClient {
    /// Create a client using an OAuth access token.
    #[cfg(test)]
    pub fn new(access_token: &str, endpoint: Option<&str>) -> Self {
        Self::with_auth(
            AuthMethod::OAuth {
                access_token: access_token.to_string(),
                refresh_token: None,
                expires_at: None,
            },
            endpoint,
        )
    }

    /// Create a client from an [`AuthMethod`].
    pub fn with_auth(auth: AuthMethod, endpoint: Option<&str>) -> Self {
        Self::with_auth_endpoints(auth, endpoint, None)
    }

    /// Create a client from an [`AuthMethod`] with distinct GraphQL and REST
    /// base URLs. When `rest_endpoint` is omitted, REST calls use `endpoint`.
    pub fn with_auth_endpoints(
        auth: AuthMethod,
        endpoint: Option<&str>,
        rest_endpoint: Option<&str>,
    ) -> Self {
        Self {
            http: Client::new(),
            auth,
            base_url: normalize_api_base_url(endpoint),
            rest_base_url: normalize_api_base_url(rest_endpoint.or(endpoint)),
        }
    }

    /// Create a client pointing at a custom base or GraphQL endpoint.
    #[cfg(test)]
    pub fn with_endpoint(access_token: &str, endpoint: &str) -> Self {
        Self {
            http: Client::new(),
            auth: AuthMethod::OAuth {
                access_token: access_token.to_string(),
                refresh_token: None,
                expires_at: None,
            },
            base_url: normalize_api_base_url(Some(endpoint)),
            rest_base_url: normalize_api_base_url(Some(endpoint)),
        }
    }

    fn graphql_url(&self) -> String {
        join_api_url(&self.base_url, "/graphql")
    }

    fn rest_url(&self, path: &str) -> String {
        join_api_url(&self.rest_base_url, path)
    }

    fn graphql_request(&self, method: Method, url: &str) -> RequestBuilder {
        match &self.auth {
            AuthMethod::OAuth { access_token, .. } => {
                with_appsignal_headers(self.http.request(method, url)).bearer_auth(access_token)
            }
            // Personal API tokens authenticate via a `?token=` query parameter.
            AuthMethod::Token { token } => {
                with_appsignal_headers(self.http.request(method, url)).query(&[("token", token)])
            }
        }
    }

    fn rest_request(&self, method: Method, url: &str) -> RequestBuilder {
        match &self.auth {
            AuthMethod::OAuth { access_token, .. } => {
                with_appsignal_headers(self.http.request(method, url).bearer_auth(access_token))
            }
            AuthMethod::Token { token } => {
                with_appsignal_headers(self.http.request(method, url).query(&[("token", token)]))
            }
        }
    }

    /// Execute a GraphQL query against AppSignal.
    ///
    /// Authentication is applied using the stored OAuth access token via an
    /// `Authorization: Bearer <access_token>` header.
    async fn graphql<T: serde::de::DeserializeOwned>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T> {
        let body = json!({
            "query": query,
            "variables": variables,
        });

        let graphql_url = self.graphql_url();
        let request = self.graphql_request(Method::POST, &graphql_url).json(&body);

        let resp = request.send().await.context(CliError::NetworkUnreachable)?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!(graphql_http_error(status, &text));
        }

        let gql_resp: GraphQLResponse<T> =
            resp.json().await.context(CliError::UnexpectedResponse)?;

        if let Some(errors) = gql_resp.errors {
            anyhow::bail!(graphql_error(errors));
        }

        gql_resp.data.context(CliError::UnexpectedResponse)
    }

    /// Validate that the token is accepted by the API.
    pub async fn validate_token(&self) -> Result<()> {
        self.graphql::<serde_json::Value>("{ __typename }", json!({}))
            .await?;
        Ok(())
    }

    /// Get the organization slug associated with the current OAuth token.
    pub async fn current_org_slug(&self) -> Result<String> {
        let url = join_api_url(&self.base_url, "/oauth/token/info");
        let resp = self
            .graphql_request(Method::GET, &url)
            .send()
            .await
            .context(CliError::NetworkUnreachable)?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!(CliError::from_http(status, &text));
        }

        let token_info: TokenInfoData = resp.json().await.context(CliError::UnexpectedResponse)?;
        token_info
            .account
            .and_then(|account| account.slug)
            .filter(|slug| !slug.is_empty())
            .context(CliError::msg(
                "Could not determine organization from your OAuth token. Re-authenticate with `appsignal-cli auth login` or set it with `appsignal-cli apps set-org --org <slug>`.",
            ))
    }

    /// Get the authenticated user associated with the current OAuth token.
    pub async fn current_user(&self) -> Result<User> {
        let query = r#"
            {
                viewer {
                    id
                    name
                    email
                }
            }
        "#;
        let data: CurrentUserData = self.graphql(query, json!({})).await?;
        data.viewer
            .context(CliError::msg("Could not fetch the authenticated user"))
    }

    /// List all applications for an organization.
    pub async fn list_apps(&self, org_slug: &str) -> Result<Vec<App>> {
        let query = r#"
            query OrganizationApps($slug: String!) {
                organization(slug: $slug) {
                    apps {
                        id
                        name
                        environment
                    }
                }
            }
        "#;
        let data: OrganizationData = self.graphql(query, json!({ "slug": org_slug })).await?;
        let org = data
            .organization
            .with_context(|| CliError::msg(format!("Organization '{}' not found", org_slug)))?;
        Ok(org.apps.unwrap_or_default())
    }

    /// Get details for a specific application.
    pub async fn get_app(&self, app_id: &str) -> Result<App> {
        let query = r#"
            query AppQuery($appId: String!) {
                app(id: $appId) {
                    id
                    name
                    environment
                }
            }
        "#;
        let data: AppData = self.graphql(query, json!({ "appId": app_id })).await?;
        data.app
            .with_context(|| CliError::msg(format!("Application '{}' not found", app_id)))
    }

    /// Find an app by name and optional environment within an organization.
    pub async fn find_app(
        &self,
        org_slug: &str,
        name: &str,
        environment: Option<&str>,
    ) -> Result<App> {
        let apps = self.list_apps(org_slug).await?;
        filter_apps(apps, name, environment, org_slug)
    }

    /// Resolve an app ID from the flexible --app/--environment/--app-id options.
    /// Priority: --app-id wins if given, otherwise --app + --environment is used.
    pub async fn resolve_app_id(
        &self,
        org_slug: &str,
        app_id: Option<&str>,
        app_name: Option<&str>,
        environment: Option<&str>,
    ) -> Result<String> {
        if let Some(id) = app_id {
            return Ok(id.to_string());
        }
        if let Some(name) = app_name {
            let app = self.find_app(org_slug, name, environment).await?;
            return Ok(app.id);
        }
        anyhow::bail!(CliError::msg(
            "Provide either --app-id or --app (with optional --environment)"
        ))
    }

    /// List users for an application.
    pub async fn list_app_users(&self, app_id: &str) -> Result<Vec<User>> {
        let query = r#"
            query AppUsers($appId: String!) {
                app(id: $appId) {
                    users { id name email }
                }
            }
        "#;
        let data: AppUsersData = self.graphql(query, json!({ "appId": app_id })).await?;
        let app = data.app.context(CliError::msg("Application not found"))?;
        Ok(app.users.unwrap_or_default())
    }

    /// Get resources for an application (users, notifiers, namespaces, dashboards, deploy markers).
    pub async fn get_app_resources(
        &self,
        app_id: &str,
        sections: &[AppResourceSection],
    ) -> Result<AppResources> {
        // Build a dynamic query based on requested sections
        let include_all = sections.is_empty();
        let want = |section| include_all || sections.contains(&section);

        let mut fields = String::new();
        if want(AppResourceSection::Users) {
            fields.push_str("users { id name email } ");
        }
        if want(AppResourceSection::Notifiers) {
            fields.push_str("notifiers { id name icon } ");
        }
        if want(AppResourceSection::Namespaces) {
            fields.push_str("namespaces { id name } ");
        }
        if want(AppResourceSection::Dashboards) {
            fields.push_str("dashboards { id title description } ");
        }
        if want(AppResourceSection::DeployMarkers) {
            fields.push_str(
                "deployMarkers(limit: 20) { id createdAt shortRevision revision gitCompareUrl user liveForInWords liveFor exceptionCount exceptionRate } ",
            );
        }

        let query = format!(
            "query AppResources($appId: String!) {{ app(id: $appId) {{ {} }} }}",
            fields
        );

        let data: AppResourcesData = self.graphql(&query, json!({ "appId": app_id })).await?;
        let app = data.app.context(CliError::msg("Application not found"))?;

        Ok(AppResources {
            users: app.users,
            notifiers: app.notifiers,
            namespaces: app.namespaces,
            dashboards: app.dashboards,
            deploy_markers: app.deploy_markers,
        })
    }

    /// List incidents for an app (all types).
    #[allow(clippy::too_many_arguments)]
    pub async fn list_incidents(
        &self,
        app_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
        state: Option<&str>,
        order: Option<&str>,
        namespaces: Option<&[String]>,
        action_name: Option<&str>,
    ) -> Result<Vec<Incident>> {
        let query = r#"
            query AppIncidents($appId: String!, $limit: Int, $offset: Int, $state: IncidentStateEnum, $order: IncidentOrderEnum, $namespaces: [String], $actionName: String) {
                app(id: $appId) {
                    incidents(limit: $limit, offset: $offset, state: $state, order: $order, namespaces: $namespaces, actionName: $actionName) {
                        __typename
                        ... on ExceptionIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                            exceptionName exceptionMessage actionNames namespace firstBacktraceLine
                            assignees { id name }
                        }
                        ... on PerformanceIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                            actionNames namespace mean totalDuration
                            assignees { id name }
                        }
                        ... on AnomalyIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                            alertState
                            trigger { id name metricName kind }
                            tags { key value }
                            assignees { id name }
                        }
                        ... on LogIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                            assignees { id name }
                        }
                    }
                }
            }
        "#;

        let mut vars = json!({ "appId": app_id });
        if let Some(l) = limit {
            vars["limit"] = json!(l);
        }
        if let Some(o) = offset {
            vars["offset"] = json!(o);
        }
        if let Some(s) = state {
            vars["state"] = json!(s);
        }
        if let Some(o) = order {
            vars["order"] = json!(o);
        }
        if let Some(ns) = namespaces {
            vars["namespaces"] = json!(ns);
        }
        if let Some(a) = action_name {
            vars["actionName"] = json!(a);
        }

        let data: AppIncidentsData = self.graphql(query, vars).await?;
        let app = data.app.context(CliError::msg("Application not found"))?;
        Ok(app.incidents.unwrap_or_default())
    }

    /// List exception incidents for an app (with text search support).
    #[allow(clippy::too_many_arguments)]
    pub async fn list_exception_incidents(
        &self,
        app_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
        state: Option<&str>,
        order: Option<&str>,
        namespaces: Option<&[String]>,
        action_name: Option<&str>,
        query_str: Option<&str>,
    ) -> Result<Vec<Incident>> {
        let query = r#"
            query AppExceptionIncidents($appId: String!, $limit: Int, $offset: Int, $state: IncidentStateEnum, $order: IncidentOrderEnum, $namespaces: [String], $actionName: String, $query: String) {
                app(id: $appId) {
                    exceptionIncidents(limit: $limit, offset: $offset, state: $state, order: $order, namespaces: $namespaces, actionName: $actionName, query: $query) {
                        __typename
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        exceptionName exceptionMessage actionNames namespace firstBacktraceLine
                            assignees { id name }
                    }
                }
            }
        "#;

        let mut vars = json!({ "appId": app_id });
        if let Some(l) = limit {
            vars["limit"] = json!(l);
        }
        if let Some(o) = offset {
            vars["offset"] = json!(o);
        }
        if let Some(s) = state {
            vars["state"] = json!(s);
        }
        if let Some(o) = order {
            vars["order"] = json!(o);
        }
        if let Some(ns) = namespaces {
            vars["namespaces"] = json!(ns);
        }
        if let Some(a) = action_name {
            vars["actionName"] = json!(a);
        }
        if let Some(q) = query_str {
            vars["query"] = json!(q);
        }

        let data: AppExceptionIncidentsData = self.graphql(query, vars).await?;
        let app = data.app.context(CliError::msg("Application not found"))?;
        Ok(app.exception_incidents.unwrap_or_default())
    }

    /// List performance incidents for an app (with text search support).
    #[allow(clippy::too_many_arguments)]
    pub async fn list_performance_incidents(
        &self,
        app_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
        state: Option<&str>,
        order: Option<&str>,
        namespaces: Option<&[String]>,
        action_name: Option<&str>,
        query_str: Option<&str>,
    ) -> Result<Vec<Incident>> {
        let query = r#"
            query AppPerformanceIncidents($appId: String!, $limit: Int, $offset: Int, $state: IncidentStateEnum, $order: IncidentOrderEnum, $namespaces: [String], $actionName: String, $query: String) {
                app(id: $appId) {
                    performanceIncidents(limit: $limit, offset: $offset, state: $state, order: $order, namespaces: $namespaces, actionName: $actionName, query: $query) {
                        __typename
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        actionNames namespace mean totalDuration
                        assignees { id name }
                    }
                }
            }
        "#;

        let mut vars = json!({ "appId": app_id });
        if let Some(l) = limit {
            vars["limit"] = json!(l);
        }
        if let Some(o) = offset {
            vars["offset"] = json!(o);
        }
        if let Some(s) = state {
            vars["state"] = json!(s);
        }
        if let Some(o) = order {
            vars["order"] = json!(o);
        }
        if let Some(ns) = namespaces {
            vars["namespaces"] = json!(ns);
        }
        if let Some(a) = action_name {
            vars["actionName"] = json!(a);
        }
        if let Some(q) = query_str {
            vars["query"] = json!(q);
        }

        let data: AppPerformanceIncidentsData = self.graphql(query, vars).await?;
        let app = data.app.context(CliError::msg("Application not found"))?;
        Ok(app.performance_incidents.unwrap_or_default())
    }

    /// List anomaly incidents for an app.
    pub async fn list_anomaly_incidents(
        &self,
        app_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
        state: Option<&str>,
        order: Option<&str>,
    ) -> Result<Vec<Incident>> {
        let query = r#"
            query AppAnomalyIncidents($appId: String!, $limit: Int, $offset: Int, $state: IncidentStateEnum, $order: IncidentOrderEnum) {
                app(id: $appId) {
                    anomalyIncidents(limit: $limit, offset: $offset, state: $state, order: $order) {
                        __typename
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        alertState
                        trigger { id name metricName kind }
                        tags { key value }
                    }
                }
            }
        "#;

        let mut vars = json!({ "appId": app_id });
        if let Some(l) = limit {
            vars["limit"] = json!(l);
        }
        if let Some(o) = offset {
            vars["offset"] = json!(o);
        }
        if let Some(s) = state {
            vars["state"] = json!(s);
        }
        if let Some(o) = order {
            vars["order"] = json!(o);
        }

        let data: AppAnomalyIncidentsData = self.graphql(query, vars).await?;
        let app = data.app.context(CliError::msg("Application not found"))?;
        Ok(app.anomaly_incidents.unwrap_or_default())
    }

    /// List anomaly detection triggers for an app.
    pub async fn list_triggers(
        &self,
        app_id: &str,
        tags: Option<&[KeyStringValue]>,
    ) -> Result<Vec<Trigger>> {
        let query = format!(
            r#"
            query AppTriggers($appId: String!, $tags: [KeyStringValueInput!]) {{
                app(id: $appId) {{
                    triggers(tags: $tags) {{
                        {}
                    }}
                }}
            }}
        "#,
            TRIGGER_SELECTION
        );

        let mut vars = json!({ "appId": app_id });
        if let Some(tags) = tags {
            vars["tags"] = json!(tags);
        }

        let data: AppTriggersData = self.graphql(&query, vars).await?;
        let app = data.app.context(CliError::msg("Application not found"))?;
        Ok(app.triggers.unwrap_or_default())
    }

    /// Get a single incident by number.
    pub async fn get_incident(&self, app_id: &str, incident_number: i64) -> Result<Incident> {
        let query = r#"
            query AppIncident($appId: String!, $incidentNumber: Int!) {
                app(id: $appId) {
                    incident(incidentNumber: $incidentNumber) {
                        __typename
                        ... on ExceptionIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                            exceptionName exceptionMessage actionNames namespace firstBacktraceLine
                            assignees { id name }
                        }
                        ... on PerformanceIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                            actionNames namespace mean totalDuration
                            assignees { id name }
                        }
                        ... on AnomalyIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                            alertState
                            trigger { id name metricName kind }
                            tags { key value }
                            assignees { id name }
                        }
                        ... on LogIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                            assignees { id name }
                        }
                    }
                }
            }
        "#;

        let data: AppIncidentData = self
            .graphql(
                query,
                json!({ "appId": app_id, "incidentNumber": incident_number }),
            )
            .await?;
        let app = data.app.context(CliError::msg("Application not found"))?;
        app.incident
            .with_context(|| CliError::msg(format!("Incident #{} not found", incident_number)))
    }

    /// Fetch a single transaction sample for an incident.
    ///
    /// One query covers both performance and exception incidents via inline
    /// fragments; the sample type is taken from the incident `__typename` the
    /// API returns, never inferred from the caller's input — an exception
    /// incident can never be miscategorised as performance.
    ///
    /// `query` selects which sample: the latest, one by id, or the one closest
    /// to a timestamp. Note the timestamp variable is declared as `DateTime`
    /// even though an ISO-8601 string is sent — declaring it `String` returns a
    /// 400 type-mismatch from the API.
    pub async fn get_incident_sample(
        &self,
        app_id: &str,
        incident_number: i64,
        query: SampleQuery<'_>,
    ) -> Result<IncidentSample> {
        let (selector_decl, selector_args) = match query {
            SampleQuery::Latest => ("", String::new()),
            SampleQuery::Id(_) => (", $sampleId: String", "(id: $sampleId)".to_string()),
            SampleQuery::Timestamp(_) => (", $at: DateTime", "(timestamp: $at)".to_string()),
        };

        let query_str = format!(
            r#"
            query IncidentSample($appId: String!, $incidentNumber: Int!{selector_decl}) {{
                app(id: $appId) {{
                    incident(incidentNumber: $incidentNumber) {{
                        __typename
                        ... on PerformanceIncident {{
                            number
                            sample{selector_args} {{ {common}{perf} }}
                        }}
                        ... on ExceptionIncident {{
                            number
                            sample{selector_args} {{ {common}{exc} }}
                        }}
                    }}
                }}
            }}
            "#,
            common = COMMON_SAMPLE_FIELDS,
            perf = PERFORMANCE_SAMPLE_SELECTION,
            exc = EXCEPTION_SAMPLE_SELECTION,
        );

        let mut vars = json!({ "appId": app_id, "incidentNumber": incident_number });
        match query {
            SampleQuery::Latest => {}
            SampleQuery::Id(id) => vars["sampleId"] = json!(id),
            SampleQuery::Timestamp(at) => vars["at"] = json!(at),
        }

        let data: AppIncidentSampleData = self.graphql(&query_str, vars).await?;
        let envelope = incident_envelope(data, incident_number)?;
        let sample_type = incident_sample_type(&envelope.typename, incident_number)?;
        let sample = envelope.sample.with_context(|| {
            CliError::msg(format!(
                "No matching sample found for incident #{}",
                incident_number
            ))
        })?;

        Ok(IncidentSample {
            incident_number: envelope.number.unwrap_or(incident_number),
            sample_type,
            sample,
        })
    }

    /// Fetch the samples for an incident, optionally narrowed to a time window
    /// and capped by `limit`.
    ///
    /// `start`/`end` are ISO-8601 strings bound to `DateTime` GraphQL variables.
    /// As with [`get_incident_sample`], the sample type comes from the incident
    /// `__typename`.
    pub async fn get_incident_samples(
        &self,
        app_id: &str,
        incident_number: i64,
        start: Option<&str>,
        end: Option<&str>,
        limit: Option<i64>,
    ) -> Result<IncidentSamples> {
        let query_str = format!(
            r#"
            query IncidentSamples($appId: String!, $incidentNumber: Int!, $start: DateTime, $end: DateTime, $limit: Int) {{
                app(id: $appId) {{
                    incident(incidentNumber: $incidentNumber) {{
                        __typename
                        ... on PerformanceIncident {{
                            number
                            samples(start: $start, end: $end, limit: $limit) {{ {common}{perf} }}
                        }}
                        ... on ExceptionIncident {{
                            number
                            samples(start: $start, end: $end, limit: $limit) {{ {common}{exc} }}
                        }}
                    }}
                }}
            }}
            "#,
            common = COMMON_SAMPLE_FIELDS,
            perf = PERFORMANCE_SAMPLE_SELECTION,
            exc = EXCEPTION_SAMPLE_SELECTION,
        );

        let mut vars = json!({ "appId": app_id, "incidentNumber": incident_number });
        if let Some(start) = start {
            vars["start"] = json!(start);
        }
        if let Some(end) = end {
            vars["end"] = json!(end);
        }
        if let Some(limit) = limit {
            vars["limit"] = json!(limit);
        }

        let data: AppIncidentSampleData = self.graphql(&query_str, vars).await?;
        let envelope = incident_envelope(data, incident_number)?;
        let sample_type = incident_sample_type(&envelope.typename, incident_number)?;

        Ok(IncidentSamples {
            incident_number: envelope.number.unwrap_or(incident_number),
            sample_type,
            samples: envelope.samples.unwrap_or_default(),
        })
    }

    /// Scan recent incidents for samples that fall within a time window.
    ///
    /// The GraphQL `incidents` query has no time-range filter, so the window is
    /// applied at the sample level (`samples(start:, end:)`): the most recent
    /// incidents (capped by `incident_limit`) are listed, then each performance
    /// or exception incident's samples within `[start, end]` are collected.
    /// Anomaly and log incidents carry no samples and are skipped.
    pub async fn scan_samples_in_window(
        &self,
        app_id: &str,
        start: &str,
        end: &str,
        namespaces: Option<&[String]>,
        incident_limit: i64,
    ) -> Result<Vec<WindowSample>> {
        let incidents = self
            .list_incidents(
                app_id,
                Some(incident_limit),
                None,
                None,
                Some("LAST"),
                namespaces,
                None,
            )
            .await?;

        let mut samples = Vec::new();
        for incident in &incidents {
            if !matches!(
                incident,
                Incident::ExceptionIncident { .. } | Incident::PerformanceIncident { .. }
            ) {
                continue;
            }

            let result = self
                .get_incident_samples(app_id, incident.number(), Some(start), Some(end), None)
                .await?;
            for sample in result.samples {
                samples.push(WindowSample {
                    incident_number: result.incident_number,
                    sample_type: result.sample_type.clone(),
                    sample,
                });
            }
        }

        Ok(samples)
    }

    /// Discover metric keys for an app via `app.metrics.keys`.
    pub async fn list_metric_keys(
        &self,
        app_id: &str,
        name: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<MetricKey>> {
        let query = r#"
            query MetricKeys($appId: String!, $name: String, $limit: Int) {
                app(id: $appId) {
                    metrics {
                        keys(name: $name, limit: $limit) {
                            name
                            type
                            digest
                            tags { key value }
                            fields
                        }
                    }
                }
            }
        "#;

        let mut vars = json!({ "appId": app_id });
        if let Some(name) = name {
            vars["name"] = json!(name);
        }
        if let Some(limit) = limit {
            vars["limit"] = json!(limit);
        }

        let data: AppMetricKeysData = self.graphql(query, vars).await?;
        Ok(data
            .app
            .and_then(|app| app.metrics)
            .and_then(|metrics| metrics.keys)
            .unwrap_or_default())
    }

    /// Fetch a metric's timeseries via `app.metrics.timeseries`.
    ///
    /// `start`/`end` are ISO-8601 strings bound to `DateTime` variables (the
    /// usual gotcha). `timeframe` is a relative-window enum value (e.g. `R1H`);
    /// because its GraphQL enum type name isn't part of the public contract we
    /// rely on here, it is validated to be alphanumeric and interpolated as a
    /// literal enum rather than passed as a typed variable.
    pub async fn fetch_metric_timeseries(
        &self,
        app_id: &str,
        query: &[MetricTimeseriesInput],
        timeframe: Option<&str>,
        start: Option<&str>,
        end: Option<&str>,
    ) -> Result<MetricTimeseries> {
        let timeframe_arg = match timeframe {
            Some(value) => {
                if !value.chars().all(|c| c.is_ascii_alphanumeric()) {
                    anyhow::bail!(CliError::msg(format!(
                        "Invalid --timeframe '{}'. Expected a value like R1H or R7D.",
                        value
                    )));
                }
                format!(", timeframe: {value}")
            }
            None => String::new(),
        };

        let query_str = format!(
            r#"
            query MetricsTimeseries($appId: String!, $start: DateTime, $end: DateTime, $query: [MetricTimeseries!]!) {{
                app(id: $appId) {{
                    metrics {{
                        timeseries(start: $start, end: $end, query: $query{timeframe_arg}) {{
                            start
                            end
                            resolution
                            keys {{ name digest tags {{ key value }} }}
                            points {{ timestamp values {{ key value }} }}
                        }}
                    }}
                }}
            }}
            "#
        );

        let mut vars = json!({ "appId": app_id, "query": query });
        if let Some(start) = start {
            vars["start"] = json!(start);
        }
        if let Some(end) = end {
            vars["end"] = json!(end);
        }

        let data: AppMetricTimeseriesData = self.graphql(&query_str, vars).await?;
        data.app
            .and_then(|app| app.metrics)
            .and_then(|metrics| metrics.timeseries)
            .context(CliError::msg("No timeseries returned for this metric"))
    }

    /// Fetch error and performance datapoints for a historical window via the
    /// `timeDetective*DataPoints` fields. `start`/`end`/`namespaces` are
    /// non-null GraphQL arguments.
    pub async fn fetch_time_detective(
        &self,
        app_id: &str,
        start: &str,
        end: &str,
        namespaces: &[String],
    ) -> Result<TimeDetective> {
        let query = r#"
            query TimeDetective($appId: String!, $start: DateTime!, $end: DateTime!, $namespaces: [String!]!) {
                app(id: $appId) {
                    timeDetectiveErrorDataPoints(start: $start, end: $end, namespaces: $namespaces) {
                        namespace actionName exceptionName throughput
                    }
                    timeDetectivePerformanceDataPoints(start: $start, end: $end, namespaces: $namespaces) {
                        namespace actionName throughput mean p90
                    }
                }
            }
        "#;

        let vars = json!({
            "appId": app_id,
            "start": start,
            "end": end,
            "namespaces": namespaces,
        });

        let data: TimeDetectiveData = self.graphql(query, vars).await?;
        let app = data.app.context(CliError::msg("Application not found"))?;
        Ok(TimeDetective {
            errors: app.errors.unwrap_or_default(),
            performance: app.performance.unwrap_or_default(),
        })
    }

    /// Update a single incident (state, severity, assignees, description).
    #[allow(clippy::too_many_arguments)]
    pub async fn update_incident(
        &self,
        app_id: &str,
        incident_number: i64,
        state: Option<&str>,
        severity: Option<&str>,
        assignee_ids: Option<&[String]>,
        description: Option<&str>,
    ) -> Result<Incident> {
        let query = r#"
            mutation UpdateIncident($appId: String!, $number: Int!, $state: IncidentStateEnum, $severity: IncidentSeverityEnum, $assigneeIds: [String!], $description: String) {
                updateIncident(appId: $appId, number: $number, state: $state, severity: $severity, assigneeIds: $assigneeIds, description: $description) {
                    __typename
                    ... on ExceptionIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        exceptionName exceptionMessage actionNames namespace firstBacktraceLine
                            assignees { id name }
                    }
                    ... on PerformanceIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        actionNames namespace mean totalDuration
                            assignees { id name }
                    }
                    ... on AnomalyIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        alertState
                        trigger { id name metricName kind }
                        tags { key value }
                    }
                    ... on LogIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        assignees { id name }
                    }
                }
            }
        "#;

        let mut vars = json!({ "appId": app_id, "number": incident_number });
        if let Some(s) = state {
            vars["state"] = json!(s);
        }
        if let Some(s) = severity {
            vars["severity"] = json!(s);
        }
        if let Some(ids) = assignee_ids {
            vars["assigneeIds"] = json!(ids);
        }
        if let Some(d) = description {
            vars["description"] = json!(d);
        }

        let data: UpdateIncidentData = self.graphql(query, vars).await?;
        data.update_incident.with_context(|| {
            CliError::msg(format!("Failed to update incident #{}", incident_number))
        })
    }

    /// Update multiple incidents at once. Currently used for state changes.
    pub async fn bulk_update_incidents(
        &self,
        app_id: &str,
        incident_ids: &[String],
        state: &str,
    ) -> Result<Vec<Incident>> {
        let query = r#"
            mutation BulkUpdateIncidents($appId: String!, $ids: [String!]!, $state: IncidentStateEnum!) {
                bulkUpdateIncidents(appId: $appId, ids: $ids, state: $state) {
                    __typename
                    ... on ExceptionIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        exceptionName exceptionMessage actionNames namespace firstBacktraceLine
                        assignees { id name }
                    }
                    ... on PerformanceIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        actionNames namespace mean totalDuration
                        assignees { id name }
                    }
                    ... on AnomalyIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        alertState
                        trigger { id name metricName kind }
                        tags { key value }
                    }
                    ... on LogIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        assignees { id name }
                    }
                }
            }
        "#;

        let data: BulkUpdateIncidentsData = self
            .graphql(
                query,
                json!({
                    "appId": app_id,
                    "ids": incident_ids,
                    "state": state,
                }),
            )
            .await?;

        data.bulk_update_incidents
            .context(CliError::msg("Failed to bulk update incidents"))
    }

    /// Create a note on an incident.
    pub async fn create_incident_note(
        &self,
        app_id: &str,
        incident_number: i64,
        content: &str,
    ) -> Result<Incident> {
        let query = r#"
            mutation CreateIncidentNote($appId: String!, $incidentNumber: Int!, $content: String!) {
                createIncidentNote(appId: $appId, incidentNumber: $incidentNumber, content: $content) {
                    __typename
                    ... on ExceptionIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        exceptionName exceptionMessage actionNames namespace firstBacktraceLine
                            assignees { id name }
                    }
                    ... on PerformanceIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        actionNames namespace mean totalDuration
                            assignees { id name }
                    }
                    ... on AnomalyIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        alertState
                        trigger { id name metricName kind }
                        tags { key value }
                    }
                    ... on LogIncident {
                        id number state severity description count
                        createdAt lastOccurredAt updatedAt
                        assignees { id name }
                    }
                }
            }
        "#;

        let data: CreateIncidentNoteData = self
            .graphql(
                query,
                json!({
                    "appId": app_id,
                    "incidentNumber": incident_number,
                    "content": content,
                }),
            )
            .await?;
        data.create_incident_note.with_context(|| {
            CliError::msg(format!(
                "Failed to create note on incident #{}",
                incident_number
            ))
        })
    }

    /// Create a trigger, or create a new trigger version when `previous_trigger_id` is provided.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_trigger(
        &self,
        app_id: &str,
        previous_trigger_id: Option<&str>,
        name: Option<&str>,
        metric_name: &str,
        tags: Option<&[KeyStringValue]>,
        kind: &str,
        field: &str,
        comparison_operator: &str,
        condition_value: f64,
        warmup_duration: i64,
        cooldown_duration: i64,
        notifier_ids: Option<&[String]>,
        no_match_is_zero: bool,
        description: Option<&str>,
        dashboard_id: Option<&str>,
        format: Option<&str>,
        format_input: Option<&str>,
    ) -> Result<Trigger> {
        let query = format!(
            r#"
            mutation CreateTrigger(
                $previousTriggerId: String,
                $appId: String!,
                $name: String,
                $metricName: String!,
                $tags: [KeyStringValueInput!],
                $kind: String!,
                $field: MetricFieldEnum!,
                $comparisonOperator: ThresholdAlertSettingComparisonOperationEnum!,
                $conditionValue: Float!,
                $warmupDuration: Int!,
                $cooldownDuration: Int!,
                $notifierIds: [String!],
                $noMatchIsZero: Boolean,
                $description: String,
                $dashboardId: String,
                $format: String,
                $formatInput: String
            ) {{
                createTrigger(
                    previousTriggerId: $previousTriggerId,
                    appId: $appId,
                    name: $name,
                    metricName: $metricName,
                    tags: $tags,
                    kind: $kind,
                    field: $field,
                    condition: {{ comparisonOperator: $comparisonOperator, value: $conditionValue }},
                    warmupDuration: $warmupDuration,
                    cooldownDuration: $cooldownDuration,
                    notifierIds: $notifierIds,
                    noMatchIsZero: $noMatchIsZero,
                    description: $description,
                    dashboardId: $dashboardId,
                    format: $format,
                    formatInput: $formatInput
                ) {{
                    {}
                }}
            }}
        "#,
            TRIGGER_SELECTION
        );

        let mut vars = json!({
            "appId": app_id,
            "metricName": metric_name,
            "kind": kind,
            "field": field,
            "comparisonOperator": comparison_operator,
            "conditionValue": condition_value,
            "warmupDuration": warmup_duration,
            "cooldownDuration": cooldown_duration,
            "noMatchIsZero": no_match_is_zero,
        });

        if let Some(previous_trigger_id) = previous_trigger_id {
            vars["previousTriggerId"] = json!(previous_trigger_id);
        }
        if let Some(name) = name {
            vars["name"] = json!(name);
        }
        if let Some(tags) = tags {
            vars["tags"] = json!(tags);
        }
        if let Some(notifier_ids) = notifier_ids {
            vars["notifierIds"] = json!(notifier_ids);
        }
        if let Some(description) = description {
            vars["description"] = json!(description);
        }
        if let Some(dashboard_id) = dashboard_id {
            vars["dashboardId"] = json!(dashboard_id);
        }
        if let Some(format) = format {
            vars["format"] = json!(format);
        }
        if let Some(format_input) = format_input {
            vars["formatInput"] = json!(format_input);
        }

        let data: CreateTriggerData = self.graphql(&query, vars).await?;
        data.create_trigger
            .context(CliError::msg("Failed to create trigger"))
    }

    /// Archive a trigger.
    pub async fn archive_trigger(&self, app_id: &str, trigger_id: &str) -> Result<Trigger> {
        let query = format!(
            r#"
            mutation ArchiveTrigger($appId: String!, $id: String!) {{
                archiveTrigger(appId: $appId, id: $id) {{
                    {}
                }}
            }}
        "#,
            TRIGGER_SELECTION
        );

        let data: ArchiveTriggerData = self
            .graphql(&query, json!({ "appId": app_id, "id": trigger_id }))
            .await?;

        data.archive_trigger
            .with_context(|| CliError::msg(format!("Failed to archive trigger {}", trigger_id)))
    }

    /// Create a dashboard for an app.
    pub async fn create_dashboard(
        &self,
        app_id: &str,
        title: &str,
        description: Option<&str>,
    ) -> Result<Dashboard> {
        let query = format!(
            r#"
            mutation CreateDashboard($appId: String!, $title: String!, $description: String) {{
                createDashboard(appId: $appId, title: $title, description: $description) {{
                    {}
                }}
            }}
        "#,
            DASHBOARD_SELECTION
        );

        let mut vars = json!({
            "appId": app_id,
            "title": title,
        });

        if let Some(description) = description {
            vars["description"] = json!(description);
        }

        let data: CreateDashboardData = self.graphql(&query, vars).await?;
        data.create_dashboard
            .context(CliError::msg("Failed to create dashboard"))
    }

    /// Update a dashboard for an app.
    pub async fn update_dashboard(
        &self,
        app_id: &str,
        dashboard_id: &str,
        title: &str,
        description: Option<&str>,
    ) -> Result<Dashboard> {
        let query = format!(
            r#"
            mutation UpdateDashboard($id: String!, $appId: String!, $title: String!, $description: String) {{
                updateDashboard(id: $id, appId: $appId, title: $title, description: $description) {{
                    {}
                }}
            }}
        "#,
            DASHBOARD_SELECTION
        );

        let mut vars = json!({
            "id": dashboard_id,
            "appId": app_id,
            "title": title,
        });

        if let Some(description) = description {
            vars["description"] = json!(description);
        }

        let data: UpdateDashboardData = self.graphql(&query, vars).await?;
        data.update_dashboard
            .with_context(|| CliError::msg(format!("Failed to update dashboard {}", dashboard_id)))
    }

    /// List log line actions for an app.
    pub async fn list_log_line_actions(&self, app_id: &str) -> Result<Vec<LogLineAction>> {
        let query = format!(
            r#"
            query AppLogLineActions($appId: String!) {{
                app(id: $appId) {{
                    logs {{
                        logLineActions {{
                            {}
                        }}
                    }}
                }}
            }}
        "#,
            LOG_LINE_ACTION_SELECTION
        );

        let data: AppLogLineActionsData = self.graphql(&query, json!({ "appId": app_id })).await?;
        let app = data.app.context("Application not found")?;

        Ok(app
            .logs
            .and_then(|logs| logs.log_line_actions)
            .unwrap_or_default())
    }

    /// Create a log line action.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_log_line_action(
        &self,
        app_id: &str,
        name: &str,
        query_text: &str,
        action_type: LogLineActionKind,
        source_ids: Option<&[String]>,
        metrics: Option<&[LogLineMetricInput]>,
        trigger: Option<&LogLineActionTriggerInput>,
    ) -> Result<LogLineAction> {
        let query = format!(
            r#"
            mutation CreateLogLineAction(
                $appId: String!,
                $name: String!,
                $query: String!,
                $actionType: LogLineActionTypeEnum!,
                $sourceIds: [String!],
                $logLineMetrics: [LogLineMetricInput!],
                $trigger: LogLineTriggerInput
            ) {{
                createLogLineAction(
                    appId: $appId,
                    name: $name,
                    query: $query,
                    actionType: $actionType,
                    sourceIds: $sourceIds,
                    logLineMetrics: $logLineMetrics,
                    trigger: $trigger
                ) {{
                    {}
                }}
            }}
        "#,
            LOG_LINE_ACTION_SELECTION
        );

        let mut vars = json!({
            "appId": app_id,
            "name": name,
            "query": query_text,
            "actionType": action_type.as_api_str(),
        });

        if let Some(source_ids) = source_ids {
            vars["sourceIds"] = json!(source_ids);
        }
        if let Some(metrics) = metrics {
            vars["logLineMetrics"] = json!(metrics);
        }
        if let Some(trigger) = trigger {
            vars["trigger"] = json!(trigger);
        }

        let data: CreateLogLineActionData = self.graphql(&query, vars).await?;
        data.create_log_line_action
            .context("Failed to create log line action")
    }

    /// Update a log line action.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_log_line_action(
        &self,
        app_id: &str,
        id: &str,
        name: Option<&str>,
        query_text: Option<&str>,
        source_ids: Option<&[String]>,
        metrics: Option<&[LogLineMetricInput]>,
        trigger: Option<&LogLineActionTriggerInput>,
    ) -> Result<LogLineAction> {
        let query = format!(
            r#"
            mutation UpdateLogLineAction(
                $appId: String!,
                $id: String!,
                $name: String,
                $query: String,
                $sourceIds: [String!],
                $logLineMetrics: [LogLineMetricInput!],
                $trigger: LogLineTriggerInput
            ) {{
                updateLogLineAction(
                    appId: $appId,
                    id: $id,
                    name: $name,
                    query: $query,
                    sourceIds: $sourceIds,
                    logLineMetrics: $logLineMetrics,
                    trigger: $trigger
                ) {{
                    {}
                }}
            }}
        "#,
            LOG_LINE_ACTION_SELECTION
        );

        let mut vars = json!({
            "appId": app_id,
            "id": id,
        });

        if let Some(name) = name {
            vars["name"] = json!(name);
        }
        if let Some(query_text) = query_text {
            vars["query"] = json!(query_text);
        }
        if let Some(source_ids) = source_ids {
            vars["sourceIds"] = json!(source_ids);
        }
        if let Some(metrics) = metrics {
            vars["logLineMetrics"] = json!(metrics);
        }
        if let Some(trigger) = trigger {
            vars["trigger"] = json!(trigger);
        }

        let data: UpdateLogLineActionData = self.graphql(&query, vars).await?;
        data.update_log_line_action
            .with_context(|| format!("Failed to update log line action {}", id))
    }

    /// Delete a log line action.
    pub async fn delete_log_line_action(&self, app_id: &str, id: &str) -> Result<LogLineAction> {
        let query = format!(
            r#"
            mutation DeleteLogLineAction($appId: String!, $id: String!) {{
                deleteLogLineAction(appId: $appId, id: $id) {{
                    {}
                }}
            }}
        "#,
            LOG_LINE_ACTION_SELECTION
        );

        let data: DeleteLogLineActionData = self
            .graphql(&query, json!({ "appId": app_id, "id": id }))
            .await?;
        data.delete_log_line_action
            .with_context(|| format!("Failed to delete log line action {}", id))
    }

    // -- Log methods --

    /// Query log lines for an app via the REST `POST /api/v2/logs/lines` endpoint.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_log_lines_rest(
        &self,
        app_id: &str,
        start: Option<&str>,
        end: Option<&str>,
        source_ids: &[String],
        query: &str,
        limit: i64,
        order: &str,
        cursor_time: Option<&str>,
    ) -> Result<Vec<RestLogLine>> {
        let rest_url = self.rest_url("/api/v2/logs/lines");
        let body = json!({
            "site_id": app_id,
            "from": start,
            "to": end,
            "source_ids": source_ids,
            "query": query,
            "pagination": {
                "per_page": limit,
                "order": order.to_uppercase(),
                "cursor": { "time": cursor_time }
            }
        });

        let resp = self
            .rest_request(Method::POST, &rest_url)
            .json(&body)
            .send()
            .await
            .context(CliError::NetworkUnreachable)?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!(CliError::from_http(status, &text));
        }

        resp.json().await.context(CliError::UnexpectedResponse)
    }

    pub(crate) fn rest_log_lines_to_log_lines(
        lines: Vec<RestLogLine>,
        source_names: &HashMap<String, String>,
    ) -> Vec<LogLine> {
        lines
            .into_iter()
            .map(|line| {
                let attributes = if line.attributes.is_empty() {
                    None
                } else {
                    let mut entries: Vec<KeyStringValue> = line
                        .attributes
                        .into_iter()
                        .map(|(key, value)| KeyStringValue {
                            key,
                            value: match value {
                                Value::Null => None,
                                Value::String(value) => Some(value),
                                other => Some(other.to_string()),
                            },
                        })
                        .collect();
                    entries.sort_by(|left, right| left.key.cmp(&right.key));
                    Some(entries)
                };

                let source = line.source_id.as_ref().map(|source_id| LogSourceRef {
                    id: source_id.clone(),
                    name: source_names.get(source_id).cloned(),
                });

                LogLine {
                    id: line.id.unwrap_or_else(|| line.timestamp.clone()),
                    timestamp: line.timestamp,
                    severity: line.severity.unwrap_or_default(),
                    hostname: line.hostname.unwrap_or_default(),
                    group: line.group,
                    message: line.message.unwrap_or_default(),
                    attributes,
                    source,
                }
            })
            .collect()
    }

    /// List all log views (saved filter presets) for an app.
    pub async fn list_log_views(&self, app_id: &str) -> Result<Vec<LogView>> {
        let gql = r#"
            query AppLogViews($appId: String!) {
                app(id: $appId) {
                    logViews {
                        id
                        name
                        query
                        sourceIds
                        severities
                        columns
                    }
                }
            }
        "#;
        let data: AppLogViewsData = self.graphql(gql, json!({ "appId": app_id })).await?;
        let app = data.app.context(CliError::msg("Application not found"))?;
        Ok(app.log_views.unwrap_or_default())
    }

    /// Get a single log view by ID.
    pub async fn get_log_view(&self, app_id: &str, view_id: &str) -> Result<LogView> {
        let gql = r#"
            query AppLogView($appId: String!, $viewId: String!) {
                app(id: $appId) {
                    logView(id: $viewId) {
                        id
                        name
                        query
                        sourceIds
                        severities
                        columns
                    }
                }
            }
        "#;

        #[derive(Debug, Deserialize)]
        struct AppLogViewData {
            app: Option<AppLogViewInner>,
        }

        #[derive(Debug, Deserialize)]
        struct AppLogViewInner {
            #[serde(rename = "logView")]
            log_view: Option<LogView>,
        }

        let data: AppLogViewData = self
            .graphql(gql, json!({ "appId": app_id, "viewId": view_id }))
            .await?;
        let app = data.app.context(CliError::msg("Application not found"))?;
        app.log_view
            .with_context(|| CliError::msg(format!("Log view '{}' not found", view_id)))
    }

    /// List all log sources for an app.
    pub async fn list_log_sources(&self, app_id: &str) -> Result<Vec<LogSource>> {
        let gql = r#"
            query AppLogSources($appId: String!) {
                app(id: $appId) {
                    logs {
                        sources {
                            id
                            name
                            type
                            fmt
                        }
                    }
                }
            }
        "#;
        let data: AppLogSourcesData = self.graphql(gql, json!({ "appId": app_id })).await?;
        let app = data.app.context(CliError::msg("Application not found"))?;
        let logs = app
            .logs
            .context(CliError::msg("Logs not available for this app"))?;
        Ok(logs.sources.unwrap_or_default())
    }
}

fn graphql_error(errors: Vec<GraphQLError>) -> CliError {
    let account_restricted = errors.iter().find(|error| {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.code.as_deref())
            == Some(ACCOUNT_RESTRICTED_CODE)
    });

    if let Some(error) = account_restricted {
        return CliError::AccountRestricted(error.message.clone());
    }

    let oauth_scope_rejected = errors
        .iter()
        .any(|error| error.message.trim() == OAUTH_SCOPE_MESSAGE);

    if oauth_scope_rejected {
        return CliError::OAuthScopeRejected;
    }

    let msgs: Vec<String> = errors.into_iter().map(|e| e.message).collect();
    CliError::GraphQlRejected(msgs.join("; "))
}

fn graphql_http_error(status: reqwest::StatusCode, body: &str) -> CliError {
    if let Ok(response) = serde_json::from_str::<GraphQLResponse<serde_json::Value>>(body) {
        if let Some(errors) = response.errors {
            return graphql_error(errors);
        }
    }

    CliError::from_http(status, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_headers::{
        CLIENT_NAME, CLIENT_NAME_HEADER, CLIENT_VERSION, CLIENT_VERSION_HEADER, USER_AGENT_VALUE,
    };
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // -- Helper to build test apps --

    fn app(id: &str, name: &str, env: &str) -> App {
        App {
            id: id.to_string(),
            name: Some(name.to_string()),
            environment: Some(env.to_string()),
        }
    }

    fn sample_apps() -> Vec<App> {
        vec![
            app("id1", "Weekmenu", "development"),
            app("id2", "Weekmenu", "production"),
            app("id3", "SalonPelikaan", "production"),
        ]
    }

    // -- filter_apps tests --

    #[test]
    fn test_filter_apps_exact_match() {
        let result = filter_apps(sample_apps(), "SalonPelikaan", None, "org").unwrap();
        assert_eq!(result.id, "id3");
    }

    #[test]
    fn test_filter_apps_case_insensitive_name() {
        let result = filter_apps(sample_apps(), "salonpelikaan", None, "org").unwrap();
        assert_eq!(result.id, "id3");
    }

    #[test]
    fn test_filter_apps_case_insensitive_environment() {
        let result = filter_apps(sample_apps(), "Weekmenu", Some("PRODUCTION"), "org").unwrap();
        assert_eq!(result.id, "id2");
    }

    #[test]
    fn test_filter_apps_with_environment_disambiguates() {
        let result = filter_apps(sample_apps(), "Weekmenu", Some("development"), "org").unwrap();
        assert_eq!(result.id, "id1");
    }

    #[test]
    fn test_filter_apps_no_match() {
        let err = filter_apps(sample_apps(), "NonExistent", None, "myorg").unwrap_err();
        assert!(err.to_string().contains("No app found"));
        assert!(err.to_string().contains("NonExistent"));
        assert!(err.to_string().contains("myorg"));
    }

    #[test]
    fn test_filter_apps_no_match_with_env() {
        let err = filter_apps(sample_apps(), "Weekmenu", Some("staging"), "myorg").unwrap_err();
        assert!(err.to_string().contains("No app found"));
        assert!(err.to_string().contains("staging"));
    }

    #[test]
    fn test_filter_apps_multiple_matches_without_env() {
        let err = filter_apps(sample_apps(), "Weekmenu", None, "org").unwrap_err();
        assert!(err.to_string().contains("Multiple apps match"));
        assert!(err.to_string().contains("--environment"));
    }

    #[test]
    fn test_filter_apps_empty_list() {
        let err = filter_apps(vec![], "Anything", None, "org").unwrap_err();
        assert!(err.to_string().contains("No app found"));
    }

    #[test]
    fn test_filter_apps_app_with_no_name() {
        let apps = vec![App {
            id: "id1".to_string(),
            name: None,
            environment: Some("production".to_string()),
        }];
        let err = filter_apps(apps, "Anything", None, "org").unwrap_err();
        assert!(err.to_string().contains("No app found"));
    }

    // -- Incident accessor tests --

    fn exception_incident() -> Incident {
        Incident::ExceptionIncident {
            id: "exc1".to_string(),
            number: 42,
            state: Some("OPEN".to_string()),
            severity: Some("CRITICAL".to_string()),
            description: Some("Something broke".to_string()),
            count: 100,
            created_at: Some("2025-01-01T00:00:00Z".to_string()),
            last_occurred_at: Some("2025-06-01T12:00:00Z".to_string()),
            updated_at: Some("2025-06-01T12:00:00Z".to_string()),
            exception_name: Some("RuntimeError".to_string()),
            exception_message: Some("bad things".to_string()),
            action_names: Some(vec!["UsersController#show".to_string()]),
            namespace: Some("web".to_string()),
            first_backtrace_line: Some("app/models/user.rb:42".to_string()),
            assignees: Some(vec![User {
                id: "u1".to_string(),
                name: Some("Alice".to_string()),
                email: Some("alice@example.com".to_string()),
            }]),
        }
    }

    fn performance_incident() -> Incident {
        Incident::PerformanceIncident {
            id: "perf1".to_string(),
            number: 7,
            state: Some("CLOSED".to_string()),
            severity: None,
            description: None,
            count: 500,
            created_at: Some("2025-03-01T00:00:00Z".to_string()),
            last_occurred_at: None,
            updated_at: None,
            action_names: Some(vec!["PagesController#index".to_string()]),
            namespace: Some("web".to_string()),
            mean: Some(32.5),
            total_duration: Some(16250.0),
            assignees: None,
        }
    }

    fn anomaly_incident() -> Incident {
        Incident::AnomalyIncident {
            id: "anom1".to_string(),
            number: 3,
            state: None,
            severity: None,
            description: None,
            count: 1,
            created_at: None,
            last_occurred_at: None,
            updated_at: None,
            alert_state: Some("WARMUP".to_string()),
            trigger: Some(TriggerSummary {
                id: "t1".to_string(),
                name: "High CPU".to_string(),
                metric_name: "cpu_usage".to_string(),
                kind: "Advanced".to_string(),
            }),
            tags: Some(vec![KeyStringValue {
                key: "hostname".to_string(),
                value: Some("web-1".to_string()),
            }]),
            assignees: None,
        }
    }

    fn log_incident() -> Incident {
        Incident::LogIncident {
            id: "log1".to_string(),
            number: 99,
            state: Some("WIP".to_string()),
            severity: Some("WARNING".to_string()),
            description: Some("Too many logs".to_string()),
            count: 10,
            created_at: None,
            last_occurred_at: Some("2025-12-25T00:00:00Z".to_string()),
            updated_at: None,
            assignees: None,
        }
    }

    #[test]
    fn test_incident_number() {
        assert_eq!(exception_incident().number(), 42);
        assert_eq!(performance_incident().number(), 7);
        assert_eq!(anomaly_incident().number(), 3);
        assert_eq!(log_incident().number(), 99);
    }

    #[test]
    fn test_incident_state() {
        assert_eq!(exception_incident().state(), "OPEN");
        assert_eq!(performance_incident().state(), "CLOSED");
        assert_eq!(anomaly_incident().state(), "-");
        assert_eq!(log_incident().state(), "WIP");
    }

    #[test]
    fn test_incident_severity() {
        assert_eq!(exception_incident().severity(), "CRITICAL");
        assert_eq!(performance_incident().severity(), "-");
        assert_eq!(anomaly_incident().severity(), "-");
        assert_eq!(log_incident().severity(), "WARNING");
    }

    #[test]
    fn test_incident_description() {
        assert_eq!(exception_incident().description(), "Something broke");
        assert_eq!(performance_incident().description(), "-");
        assert_eq!(log_incident().description(), "Too many logs");
    }

    #[test]
    fn test_incident_count() {
        assert_eq!(exception_incident().count(), 100);
        assert_eq!(performance_incident().count(), 500);
    }

    #[test]
    fn test_incident_last_occurred_at() {
        assert_eq!(
            exception_incident().last_occurred_at(),
            "2025-06-01T12:00:00Z"
        );
        assert_eq!(performance_incident().last_occurred_at(), "-");
        assert_eq!(log_incident().last_occurred_at(), "2025-12-25T00:00:00Z");
    }

    #[test]
    fn test_incident_created_at() {
        assert_eq!(exception_incident().created_at(), "2025-01-01T00:00:00Z");
        assert_eq!(anomaly_incident().created_at(), "-");
    }

    #[test]
    fn test_incident_kind() {
        assert_eq!(exception_incident().kind(), "exception");
        assert_eq!(performance_incident().kind(), "performance");
        assert_eq!(anomaly_incident().kind(), "anomaly");
        assert_eq!(log_incident().kind(), "log");
    }

    // -- Incident deserialization tests --

    #[test]
    fn test_deserialize_exception_incident() {
        let json = r#"{
            "__typename": "ExceptionIncident",
            "id": "e1", "number": 5, "state": "OPEN", "severity": "CRITICAL",
            "description": "Oops", "count": 10,
            "createdAt": "2025-01-01T00:00:00Z",
            "lastOccurredAt": "2025-06-01T00:00:00Z",
            "updatedAt": null,
            "exceptionName": "RuntimeError",
            "exceptionMessage": "bad",
            "actionNames": ["FooController#bar"],
            "namespace": "web",
            "firstBacktraceLine": "app.rb:1",
            "assignees": [{ "id": "u1", "name": "Alice", "email": "alice@example.com" }]
        }"#;
        let incident: Incident = serde_json::from_str(json).unwrap();
        assert_eq!(incident.kind(), "exception");
        assert_eq!(incident.number(), 5);
        assert_eq!(incident.state(), "OPEN");
        assert_eq!(incident.assignees().len(), 1);
        assert_eq!(incident.assignees()[0].name.as_deref(), Some("Alice"));
        if let Incident::ExceptionIncident {
            exception_name,
            exception_message,
            ..
        } = &incident
        {
            assert_eq!(exception_name.as_deref(), Some("RuntimeError"));
            assert_eq!(exception_message.as_deref(), Some("bad"));
        } else {
            panic!("Expected ExceptionIncident");
        }
    }

    #[test]
    fn test_deserialize_performance_incident() {
        let json = r#"{
            "__typename": "PerformanceIncident",
            "id": "p1", "number": 3, "state": "CLOSED", "severity": null,
            "description": null, "count": 200,
            "createdAt": null, "lastOccurredAt": null, "updatedAt": null,
            "actionNames": [], "namespace": "web",
            "mean": 45.5, "totalDuration": 9100.0,
            "assignees": []
        }"#;
        let incident: Incident = serde_json::from_str(json).unwrap();
        assert_eq!(incident.kind(), "performance");
        assert_eq!(incident.count(), 200);
        if let Incident::PerformanceIncident {
            mean,
            total_duration,
            ..
        } = &incident
        {
            assert_eq!(*mean, Some(45.5));
            assert_eq!(*total_duration, Some(9100.0));
        } else {
            panic!("Expected PerformanceIncident");
        }
    }

    #[test]
    fn test_deserialize_anomaly_incident() {
        let json = r#"{
            "__typename": "AnomalyIncident",
            "id": "a1", "number": 1, "state": "OPEN", "severity": null,
            "description": "Spike detected", "count": 1,
            "createdAt": "2025-03-01T00:00:00Z",
            "lastOccurredAt": "2025-03-01T01:00:00Z",
            "updatedAt": null,
            "alertState": "WARMUP",
            "trigger": { "id": "t1", "name": "High CPU", "metricName": "cpu_usage", "kind": "Advanced" },
            "tags": [{ "key": "hostname", "value": "web-1" }],
            "assignees": []
        }"#;
        let incident: Incident = serde_json::from_str(json).unwrap();
        assert_eq!(incident.kind(), "anomaly");
        assert_eq!(incident.description(), "Spike detected");
        if let Incident::AnomalyIncident {
            alert_state,
            trigger,
            tags,
            ..
        } = &incident
        {
            assert_eq!(alert_state.as_deref(), Some("WARMUP"));
            assert_eq!(trigger.as_ref().unwrap().name, "High CPU");
            assert_eq!(tags.as_ref().unwrap()[0].key, "hostname");
        } else {
            panic!("Expected AnomalyIncident");
        }
    }

    #[test]
    fn test_deserialize_anomaly_incident_minimal() {
        // Anomaly incident with no trigger/tags/alertState (all optional)
        let json = r#"{
            "__typename": "AnomalyIncident",
            "id": "a2", "number": 2, "state": "CLOSED", "severity": null,
            "description": null, "count": 5,
            "createdAt": null, "lastOccurredAt": null, "updatedAt": null,
            "alertState": null, "trigger": null, "tags": null,
            "assignees": null
        }"#;
        let incident: Incident = serde_json::from_str(json).unwrap();
        assert_eq!(incident.kind(), "anomaly");
        assert_eq!(incident.number(), 2);
    }

    #[test]
    fn test_deserialize_log_incident() {
        let json = r#"{
            "__typename": "LogIncident",
            "id": "l1", "number": 10, "state": "WIP", "severity": "WARNING",
            "description": "Log flood", "count": 9999,
            "createdAt": null, "lastOccurredAt": null, "updatedAt": null,
            "assignees": null
        }"#;
        let incident: Incident = serde_json::from_str(json).unwrap();
        assert_eq!(incident.kind(), "log");
        assert_eq!(incident.state(), "WIP");
        assert_eq!(incident.count(), 9999);
    }

    // -- Assignee tests --

    #[test]
    fn test_incident_assignees() {
        let incident = exception_incident();
        assert_eq!(incident.assignees().len(), 1);
        assert_eq!(incident.assignees()[0].id, "u1");
        assert_eq!(incident.assignee_ids(), vec!["u1".to_string()]);
    }

    #[test]
    fn test_incident_assignees_empty() {
        let incident = performance_incident();
        assert!(incident.assignees().is_empty());
        assert!(incident.assignee_ids().is_empty());
    }

    // -- resolve_user_ids tests --

    fn sample_users() -> Vec<User> {
        vec![
            User {
                id: "u1".to_string(),
                name: Some("Alice Smith".to_string()),
                email: Some("alice@example.com".to_string()),
            },
            User {
                id: "u2".to_string(),
                name: Some("Bob Jones".to_string()),
                email: Some("bob@example.com".to_string()),
            },
        ]
    }

    #[test]
    fn test_resolve_user_ids_by_name() {
        let ids = resolve_user_ids(&["Alice Smith".to_string()], &sample_users()).unwrap();
        assert_eq!(ids, vec!["u1"]);
    }

    #[test]
    fn test_resolve_user_ids_case_insensitive() {
        let ids = resolve_user_ids(&["alice smith".to_string()], &sample_users()).unwrap();
        assert_eq!(ids, vec!["u1"]);
    }

    #[test]
    fn test_resolve_user_ids_falls_back_to_raw_id() {
        let ids = resolve_user_ids(&["some-raw-id-123".to_string()], &sample_users()).unwrap();
        assert_eq!(ids, vec!["some-raw-id-123"]);
    }

    #[test]
    fn test_resolve_user_ids_multiple() {
        let ids = resolve_user_ids(
            &["Alice Smith".to_string(), "Bob Jones".to_string()],
            &sample_users(),
        )
        .unwrap();
        assert_eq!(ids, vec!["u1", "u2"]);
    }

    #[test]
    fn test_resolve_user_ids_mixed_names_and_ids() {
        let ids = resolve_user_ids(
            &["Alice Smith".to_string(), "raw-id".to_string()],
            &sample_users(),
        )
        .unwrap();
        assert_eq!(ids, vec!["u1", "raw-id"]);
    }

    #[test]
    fn test_resolve_user_ids_duplicate_name_errors() {
        let users = vec![
            User {
                id: "u1".to_string(),
                name: Some("Alice".to_string()),
                email: Some("alice1@example.com".to_string()),
            },
            User {
                id: "u2".to_string(),
                name: Some("Alice".to_string()),
                email: Some("alice2@example.com".to_string()),
            },
        ];
        let err = resolve_user_ids(&["Alice".to_string()], &users).unwrap_err();
        assert!(err.to_string().contains("Multiple users match"));
    }

    // -- Wiremock integration tests for API client --

    fn graphql_response(data: serde_json::Value) -> serde_json::Value {
        json!({ "data": data })
    }

    fn log_line_action_filter_json(id: &str, order: i64) -> serde_json::Value {
        json!({
            "__typename": "LogLineActionFilter",
            "id": id,
            "name": "Drop health checks",
            "query": "message:\"GET /health\"",
            "sourceIds": ["src-1"],
            "actionType": "FILTER",
            "sources": [{ "id": "src-1", "name": "nginx", "type": "custom", "fmt": "json" }],
            "order": order
        })
    }

    fn log_line_action_metrics_json(id: &str, order: i64) -> serde_json::Value {
        json!({
            "__typename": "LogLineActionMetrics",
            "id": id,
            "name": "Track error count",
            "query": "severity:error",
            "sourceIds": [],
            "actionType": "METRICS",
            "sources": [],
            "logLineMetrics": [{
                "id": "metric-1",
                "name": "log.error_count",
                "field": null,
                "tags": { "hostname": "web-1" },
                "metricType": "COUNTER"
            }],
            "order": order,
            "user": { "id": "u1", "name": "Alice", "email": "alice@example.com" }
        })
    }

    #[tokio::test]
    async fn test_validate_token_uses_graphql_with_bearer_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(header("authorization", "Bearer test-token"))
            .and(header("user-agent", USER_AGENT_VALUE))
            .and(header("x-appsignal-client", CLIENT_NAME))
            .and(header("x-appsignal-client-version", CLIENT_VERSION))
            .and(body_string_contains("__typename"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "__typename": "Query"
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::new("test-token", Some(&server.uri()));
        client.validate_token().await.unwrap();
    }

    #[tokio::test]
    async fn test_token_auth_sends_token_query_param() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(query_param("token", "personal-api-token"))
            .and(header("x-appsignal-client", CLIENT_NAME))
            .and(body_string_contains("__typename"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "__typename": "Query"
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_auth_endpoints(
            AuthMethod::Token {
                token: "personal-api-token".to_string(),
            },
            Some(&server.uri()),
            None,
        );
        client.validate_token().await.unwrap();
    }

    #[tokio::test]
    async fn test_validate_token_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(header("authorization", "Bearer bad-token"))
            .and(body_string_contains("__typename"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&server)
            .await;

        let client = AppSignalClient::new("bad-token", Some(&server.uri()));
        let err = client.validate_token().await.unwrap_err();
        assert!(err.to_string().contains("Authentication failed"));
    }

    #[tokio::test]
    async fn test_validate_token_handles_http_400_account_restricted_graphql_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(header("authorization", "Bearer restricted-token"))
            .and(body_string_contains("__typename"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "errors": [{
                    "message": "You exceeded your free plan quota for this month. API access is restricted.",
                    "extensions": { "code": "ACCOUNT_RESTRICTED" }
                }]
            })))
            .mount(&server)
            .await;

        let client = AppSignalClient::new("restricted-token", Some(&server.uri()));
        let err = client.validate_token().await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "You exceeded your free plan quota for this month. API access is restricted."
        );
    }

    #[tokio::test]
    async fn test_validate_token_handles_free_plan_quota_graphql_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(header("authorization", "Bearer restricted-token"))
            .and(body_string_contains("__typename"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "errors": [{
                    "message": "You exceeded your free plan quota for this month. API access is restricted.",
                    "extensions": { "code": "ACCOUNT_RESTRICTED" }
                }]
            })))
            .mount(&server)
            .await;

        let client = AppSignalClient::new("restricted-token", Some(&server.uri()));
        let err = client.validate_token().await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "You exceeded your free plan quota for this month. API access is restricted."
        );
    }

    #[tokio::test]
    async fn test_validate_token_handles_locked_account_graphql_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(header("authorization", "Bearer locked-token"))
            .and(body_string_contains("__typename"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "errors": [{
                    "message": "This account is locked. API access is restricted.",
                    "extensions": { "code": "ACCOUNT_RESTRICTED" }
                }]
            })))
            .mount(&server)
            .await;

        let client = AppSignalClient::new("locked-token", Some(&server.uri()));
        let err = client.validate_token().await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "This account is locked. API access is restricted."
        );
    }

    #[tokio::test]
    async fn test_validate_token_handles_oauth_scope_graphql_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(header("authorization", "Bearer scope-token"))
            .and(body_string_contains("__typename"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "errors": [{
                    "message": "Your OAuth token does not have the required scope for this operation."
                }]
            })))
            .mount(&server)
            .await;

        let client = AppSignalClient::new("scope-token", Some(&server.uri()));
        let err = client.validate_token().await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "AppSignal rejected the request because your OAuth token is missing the required scope for this operation. Re-authenticate with `appsignal-cli auth login` to get an updated token."
        );
    }

    #[tokio::test]
    async fn test_current_org_slug_uses_token_info_account_slug() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/oauth/token/info"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "account": {
                    "slug": "my-org"
                }
            })))
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &server.uri());
        let slug = client.current_org_slug().await.unwrap();
        assert_eq!(slug, "my-org");
    }

    #[tokio::test]
    async fn test_validate_token_accepts_graphql_response_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(header("authorization", "Bearer bad-token"))
            .and(body_string_contains("__typename"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "__typename": "Query"
                }
            })))
            .mount(&server)
            .await;

        let client = AppSignalClient::new("bad-token", Some(&server.uri()));
        client.validate_token().await.unwrap();
    }

    #[tokio::test]
    async fn test_list_log_lines_rest_uses_bearer_auth() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/logs/lines"))
            .and(header("authorization", "Bearer tok"))
            .and(header(
                CLIENT_NAME_HEADER.to_ascii_lowercase().as_str(),
                CLIENT_NAME,
            ))
            .and(header(
                CLIENT_VERSION_HEADER.to_ascii_lowercase().as_str(),
                CLIENT_VERSION,
            ))
            .and(body_string_contains(r#""order":"DESC""#))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {
                    "id": "log-1",
                    "timestamp": "2025-06-01T12:00:00Z",
                    "source_id": "src-1",
                    "group": "web",
                    "severity": "error",
                    "message": "Request failed",
                    "hostname": "web-1",
                    "attributes": {"request_id": "123", "duration_ms": 42}
                }
            ])))
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let lines = client
            .list_log_lines_rest(
                "app1",
                Some("2025-06-01T00:00:00Z"),
                Some("2025-06-02T00:00:00Z"),
                &["src-1".to_string()],
                "severity=[error]",
                100,
                "desc",
                None,
            )
            .await
            .unwrap();

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].id.as_deref(), Some("log-1"));
        assert_eq!(lines[0].source_id.as_deref(), Some("src-1"));
        assert_eq!(lines[0].attributes.get("request_id"), Some(&json!("123")));
    }

    #[test]
    fn test_rest_log_lines_to_log_lines_maps_attributes_and_sources() {
        let source_names = HashMap::from([("src-1".to_string(), "Application".to_string())]);
        let lines = AppSignalClient::rest_log_lines_to_log_lines(
            vec![RestLogLine {
                id: Some("log-1".to_string()),
                timestamp: "2025-06-01T12:00:00Z".to_string(),
                source_id: Some("src-1".to_string()),
                group: Some("web".to_string()),
                severity: Some("error".to_string()),
                message: Some("Request failed".to_string()),
                hostname: Some("web-1".to_string()),
                attributes: serde_json::Map::from_iter([
                    ("duration_ms".to_string(), json!(42)),
                    ("request_id".to_string(), json!("123")),
                ]),
            }],
            &source_names,
        );

        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0]
                .source
                .as_ref()
                .and_then(|source| source.name.as_deref()),
            Some("Application")
        );
        assert_eq!(
            lines[0].attributes.as_ref().map(|attrs| attrs.len()),
            Some(2)
        );
        assert_eq!(lines[0].attributes.as_ref().unwrap()[0].key, "duration_ms");
        assert_eq!(
            lines[0].attributes.as_ref().unwrap()[0].value.as_deref(),
            Some("42")
        );
    }

    #[tokio::test]
    async fn test_current_user() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "viewer": {
                        "id": "user-1",
                        "name": "Ada",
                        "email": "ada@example.com"
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let user = client.current_user().await.unwrap();
        assert_eq!(user.id, "user-1");
        assert_eq!(user.name.as_deref(), Some("Ada"));
        assert_eq!(user.email.as_deref(), Some("ada@example.com"));
    }

    #[tokio::test]
    async fn test_list_apps() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "organization": {
                        "apps": [
                            { "id": "a1", "name": "MyApp", "environment": "production" }
                        ]
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let apps = client.list_apps("my-org").await.unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].id, "a1");
        assert_eq!(apps[0].name.as_deref(), Some("MyApp"));
    }

    #[tokio::test]
    async fn test_bulk_update_incidents() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("bulkUpdateIncidents"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "bulkUpdateIncidents": [
                        {
                            "__typename": "ExceptionIncident",
                            "id": "i1",
                            "number": 41,
                            "state": "CLOSED",
                            "severity": "HIGH",
                            "description": null,
                            "count": 1,
                            "createdAt": null,
                            "lastOccurredAt": null,
                            "updatedAt": null,
                            "exceptionName": null,
                            "exceptionMessage": null,
                            "actionNames": null,
                            "namespace": null,
                            "firstBacktraceLine": null,
                            "assignees": []
                        },
                        {
                            "__typename": "ExceptionIncident",
                            "id": "i2",
                            "number": 42,
                            "state": "CLOSED",
                            "severity": "HIGH",
                            "description": null,
                            "count": 1,
                            "createdAt": null,
                            "lastOccurredAt": null,
                            "updatedAt": null,
                            "exceptionName": null,
                            "exceptionMessage": null,
                            "actionNames": null,
                            "namespace": null,
                            "firstBacktraceLine": null,
                            "assignees": []
                        }
                    ]
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let incidents = client
            .bulk_update_incidents("app-1", &["i1".to_string(), "i2".to_string()], "CLOSED")
            .await
            .unwrap();
        assert_eq!(incidents.len(), 2);
        assert_eq!(incidents[0].number(), 41);
        assert_eq!(incidents[1].state(), "CLOSED");
    }

    #[tokio::test]
    async fn test_list_apps_org_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "organization": null
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let err = client.list_apps("bad-org").await.unwrap_err();
        assert!(err.to_string().contains("bad-org"));
    }

    #[tokio::test]
    async fn test_get_app() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": { "id": "a1", "name": "MyApp", "environment": "staging" }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let app = client.get_app("a1").await.unwrap();
        assert_eq!(app.id, "a1");
        assert_eq!(app.environment.as_deref(), Some("staging"));
    }

    #[tokio::test]
    async fn test_get_app_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": null
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let err = client.get_app("nope").await.unwrap_err();
        assert!(err.to_string().contains("nope"));
    }

    #[tokio::test]
    async fn test_get_app_resources_with_deploy_markers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "users": [{ "id": "u1", "name": "Jane", "email": "jane@example.com" }],
                        "notifiers": [{ "id": "n1", "name": "Slack", "icon": "slack" }],
                        "namespaces": [
                            { "id": "ns1", "name": "web" },
                            { "id": "ns2", "name": "background" }
                        ],
                        "dashboards": [{ "id": "d1", "title": "API", "description": "API metrics" }],
                        "deployMarkers": [{
                            "id": "m1",
                            "createdAt": "2025-06-01T12:00:00Z",
                            "shortRevision": "abc1234",
                            "revision": "abc1234567890",
                            "gitCompareUrl": "https://example.com/compare",
                            "user": "jeroen",
                            "liveForInWords": "2 hours",
                            "liveFor": 7200,
                            "exceptionCount": 3,
                            "exceptionRate": 0.25
                        }]
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let resources = client.get_app_resources("app1", &[]).await.unwrap();

        assert_eq!(resources.users.as_ref().unwrap().len(), 1);
        assert_eq!(resources.notifiers.as_ref().unwrap().len(), 1);
        let namespaces = resources.namespaces.as_ref().unwrap();
        assert_eq!(namespaces.len(), 2);
        assert_eq!(namespaces[0].name, "web");
        assert_eq!(namespaces[1].name, "background");
        assert_eq!(resources.dashboards.as_ref().unwrap().len(), 1);
        assert_eq!(resources.deploy_markers.as_ref().unwrap().len(), 1);

        let marker = &resources.deploy_markers.as_ref().unwrap()[0];
        assert_eq!(marker.id, "m1");
        assert_eq!(marker.short_revision.as_deref(), Some("abc1234"));
        assert_eq!(marker.revision.as_deref(), Some("abc1234567890"));
        assert_eq!(marker.user.as_deref(), Some("jeroen"));
        assert_eq!(marker.exception_count, Some(3));
        assert_eq!(marker.exception_rate, Some(0.25));
    }

    #[tokio::test]
    async fn test_list_incidents() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "incidents": [
                            {
                                "__typename": "ExceptionIncident",
                                "id": "e1", "number": 1, "state": "OPEN",
                                "severity": "CRITICAL", "description": "Boom",
                                "count": 5,
                                "createdAt": "2025-01-01T00:00:00Z",
                                "lastOccurredAt": "2025-06-01T00:00:00Z",
                                "updatedAt": null,
                                "exceptionName": "RuntimeError",
                                "exceptionMessage": "fail",
                                "actionNames": [],
                                "namespace": "web",
                                "firstBacktraceLine": "app.rb:1"
                            },
                            {
                                "__typename": "PerformanceIncident",
                                "id": "p1", "number": 2, "state": "CLOSED",
                                "severity": null, "description": null,
                                "count": 100,
                                "createdAt": null, "lastOccurredAt": null,
                                "updatedAt": null,
                                "actionNames": ["PagesController#index"],
                                "namespace": "web",
                                "mean": 50.0, "totalDuration": 5000.0
                            }
                        ]
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let incidents = client
            .list_incidents("app1", Some(10), None, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(incidents.len(), 2);
        assert_eq!(incidents[0].kind(), "exception");
        assert_eq!(incidents[0].number(), 1);
        assert_eq!(incidents[1].kind(), "performance");
        assert_eq!(incidents[1].number(), 2);
    }

    #[tokio::test]
    async fn test_list_incidents_empty() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": { "incidents": [] }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let incidents = client
            .list_incidents("app1", None, None, None, None, None, None)
            .await
            .unwrap();
        assert!(incidents.is_empty());
    }

    #[tokio::test]
    async fn test_get_incident() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "incident": {
                            "__typename": "ExceptionIncident",
                            "id": "e1", "number": 42, "state": "OPEN",
                            "severity": "CRITICAL", "description": "Bad",
                            "count": 10,
                            "createdAt": "2025-01-01T00:00:00Z",
                            "lastOccurredAt": "2025-06-01T00:00:00Z",
                            "updatedAt": null,
                            "exceptionName": "RuntimeError",
                            "exceptionMessage": "oops",
                            "actionNames": ["Foo#bar"],
                            "namespace": "web",
                            "firstBacktraceLine": "app.rb:99"
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let incident = client.get_incident("app1", 42).await.unwrap();
        assert_eq!(incident.number(), 42);
        assert_eq!(incident.kind(), "exception");
        assert_eq!(incident.state(), "OPEN");
    }

    #[tokio::test]
    async fn test_get_incident_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": { "incident": null }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let err = client.get_incident("app1", 999).await.unwrap_err();
        assert!(err.to_string().contains("999"));
    }

    fn performance_sample_json() -> serde_json::Value {
        json!({
            "id": "0123456789abcdef01234567-42",
            "action": "Web::OrdersController#create",
            "namespace": "web",
            "duration": 123.45,
            "queueDuration": 12.5,
            "createdAt": "2026-05-19T14:30:00Z",
            "revision": "abc123",
            "version": null,
            "originalId": null,
            "hasNPlusOne": true,
            "attributes": [{ "key": "request_id", "value": "req-1" }],
            "overview": [{ "key": "user_id", "value": "user-9" }],
            "environment": [{ "key": "framework", "value": "rails" }]
        })
    }

    fn exception_sample_json() -> serde_json::Value {
        json!({
            "id": "0123456789abcdef01234567-77",
            "action": "Web::OrdersController#show",
            "namespace": "web",
            "duration": 88.0,
            "queueDuration": null,
            "createdAt": "2026-05-19T15:00:00Z",
            "revision": "abc123",
            "version": null,
            "originalId": null,
            "attributes": [],
            "overview": [],
            "environment": [],
            "exception": {
                "name": "RuntimeError",
                "message": "boom",
                "backtrace": [
                    { "line": 42, "path": "app/controllers/orders_controller.rb", "method": "show",
                      "column": null, "original": null, "type": null, "url": null }
                ]
            },
            "errorCauses": [{ "name": "ArgumentError", "message": "bad input" }]
        })
    }

    #[tokio::test]
    async fn test_get_incident_sample_latest_performance() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "incident": {
                            "__typename": "PerformanceIncident",
                            "number": 42,
                            "sample": performance_sample_json()
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let result = client
            .get_incident_sample("app1", 42, SampleQuery::Latest)
            .await
            .unwrap();

        assert_eq!(result.incident_number, 42);
        assert_eq!(result.sample_type, "performance");
        assert_eq!(
            result.sample.action.as_deref(),
            Some("Web::OrdersController#create")
        );
        assert_eq!(result.sample.has_n_plus_one, Some(true));
    }

    #[tokio::test]
    async fn test_get_incident_sample_by_timestamp_declares_datetime_variable() {
        // Regression guard for the DateTime gotcha: the timestamp variable must
        // be declared `DateTime`, even though an ISO-8601 string is sent.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("$at: DateTime"))
            .and(body_string_contains("timestamp: $at"))
            .and(body_string_contains("2026-05-19T14:30:00Z"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "incident": {
                            "__typename": "PerformanceIncident",
                            "number": 42,
                            "sample": performance_sample_json()
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let result = client
            .get_incident_sample("app1", 42, SampleQuery::Timestamp("2026-05-19T14:30:00Z"))
            .await
            .unwrap();
        assert_eq!(result.sample_type, "performance");
    }

    #[tokio::test]
    async fn test_get_incident_sample_by_id_declares_string_variable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("$sampleId: String"))
            .and(body_string_contains("id: $sampleId"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "incident": {
                            "__typename": "ExceptionIncident",
                            "number": 77,
                            "sample": exception_sample_json()
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let result = client
            .get_incident_sample("app1", 77, SampleQuery::Id("0123456789abcdef01234567-77"))
            .await
            .unwrap();

        assert_eq!(result.sample_type, "error");
        let exception = result.sample.exception.unwrap();
        assert_eq!(exception.name.as_deref(), Some("RuntimeError"));
        assert_eq!(exception.backtrace.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_get_incident_sample_anomaly_has_no_samples() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "incident": { "__typename": "AnomalyIncident" }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let err = client
            .get_incident_sample("app1", 5, SampleQuery::Latest)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("anomaly"));
        assert!(err.to_string().contains("no transaction samples"));
    }

    #[tokio::test]
    async fn test_get_incident_samples_window_declares_datetime_variables() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("$start: DateTime"))
            .and(body_string_contains("$end: DateTime"))
            .and(body_string_contains("$limit: Int"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "incident": {
                            "__typename": "PerformanceIncident",
                            "number": 42,
                            "samples": [performance_sample_json(), performance_sample_json()]
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let result = client
            .get_incident_samples(
                "app1",
                42,
                Some("2026-05-19T00:00:00Z"),
                Some("2026-05-20T00:00:00Z"),
                Some(50),
            )
            .await
            .unwrap();

        assert_eq!(result.sample_type, "performance");
        assert_eq!(result.samples.len(), 2);
    }

    #[tokio::test]
    async fn test_scan_samples_in_window_collects_across_incidents() {
        let server = MockServer::start().await;

        // The incident listing: one performance incident (has samples) and one
        // anomaly incident (no samples, must be skipped).
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("AppIncidents"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "incidents": [
                            {
                                "__typename": "PerformanceIncident",
                                "id": "p1", "number": 7, "state": "OPEN",
                                "severity": "WARNING", "description": "slow",
                                "count": 3,
                                "createdAt": "2026-05-19T00:00:00Z",
                                "lastOccurredAt": "2026-05-19T12:00:00Z",
                                "updatedAt": null,
                                "actionNames": ["Web#index"], "namespace": "web",
                                "mean": 10.0, "totalDuration": 30.0
                            },
                            {
                                "__typename": "AnomalyIncident",
                                "id": "a1", "number": 8, "state": "OPEN",
                                "severity": "WARNING", "description": "anomaly",
                                "count": 1,
                                "createdAt": "2026-05-19T00:00:00Z",
                                "lastOccurredAt": "2026-05-19T12:00:00Z",
                                "updatedAt": null,
                                "alertState": "OPEN",
                                "trigger": null, "tags": []
                            }
                        ]
                    }
                }))),
            )
            .mount(&server)
            .await;

        // The per-incident samples query (only the performance incident reaches it).
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("IncidentSamples"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "incident": {
                            "__typename": "PerformanceIncident",
                            "number": 7,
                            "samples": [performance_sample_json()]
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let samples = client
            .scan_samples_in_window(
                "app1",
                "2026-05-19T00:00:00Z",
                "2026-05-20T00:00:00Z",
                None,
                20,
            )
            .await
            .unwrap();

        // Only the performance incident contributes a sample; the anomaly is skipped.
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].incident_number, 7);
        assert_eq!(samples[0].sample_type, "performance");
    }

    #[tokio::test]
    async fn test_list_metric_keys() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("MetricKeys"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "metrics": {
                            "keys": [
                                {
                                    "name": "database.query_count",
                                    "type": "counter",
                                    "digest": "abc",
                                    "tags": [{ "key": "hostname", "value": "web-1" }],
                                    "fields": ["COUNTER"]
                                }
                            ]
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let keys = client
            .list_metric_keys("app1", Some("database"), Some(50))
            .await
            .unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "database.query_count");
        assert_eq!(keys[0].kind.as_deref(), Some("counter"));
    }

    #[tokio::test]
    async fn test_fetch_metric_timeseries_declares_datetime_and_inlines_timeframe() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("$start: DateTime"))
            .and(body_string_contains("$query: [MetricTimeseries!]!"))
            .and(body_string_contains("timeframe: R1H"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "metrics": {
                            "timeseries": {
                                "start": "2026-05-19T00:00:00Z",
                                "end": "2026-05-19T01:00:00Z",
                                "resolution": "MINUTELY",
                                "keys": [{ "name": "latency", "digest": "d", "tags": [] }],
                                "points": [
                                    { "timestamp": "2026-05-19T00:00:00Z", "values": [{ "key": "mean", "value": "12.5" }] }
                                ]
                            }
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let query = vec![MetricTimeseriesInput {
            name: "latency".to_string(),
            fields: vec![MetricFieldInput {
                field: "MEAN".to_string(),
            }],
            tags: vec![],
        }];
        let series = client
            .fetch_metric_timeseries("app1", &query, Some("R1H"), None, None)
            .await
            .unwrap();
        assert_eq!(series.points.len(), 1);
        assert_eq!(series.points[0].values[0].key, "mean");
    }

    #[tokio::test]
    async fn test_fetch_metric_timeseries_rejects_unsafe_timeframe() {
        let client = AppSignalClient::new("tok", None);
        let err = client
            .fetch_metric_timeseries("app1", &[], Some("R1H) evil"), None, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Invalid --timeframe"));
    }

    #[tokio::test]
    async fn test_fetch_time_detective_declares_nonnull_datetime() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("$start: DateTime!"))
            .and(body_string_contains("$namespaces: [String!]!"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "timeDetectiveErrorDataPoints": [
                            { "namespace": "web", "actionName": "Web#index", "exceptionName": "RuntimeError", "throughput": 3.0 }
                        ],
                        "timeDetectivePerformanceDataPoints": [
                            { "namespace": "web", "actionName": "Web#index", "throughput": 100.0, "mean": 12.5, "p90": 30.0 }
                        ]
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let detective = client
            .fetch_time_detective(
                "app1",
                "2026-05-19T00:00:00Z",
                "2026-05-20T00:00:00Z",
                &["web".to_string()],
            )
            .await
            .unwrap();
        assert_eq!(detective.errors.len(), 1);
        assert_eq!(detective.performance.len(), 1);
        assert_eq!(detective.performance[0].p90, Some(30.0));
    }

    #[tokio::test]
    async fn test_resolve_app_id_with_explicit_id() {
        // No server needed -- should return immediately
        let client = AppSignalClient::new("tok", None);
        let id = client
            .resolve_app_id("org", Some("explicit-id"), None, None)
            .await
            .unwrap();
        assert_eq!(id, "explicit-id");
    }

    #[tokio::test]
    async fn test_resolve_app_id_prefers_app_id_over_name() {
        let client = AppSignalClient::new("tok", None);
        let id = client
            .resolve_app_id("org", Some("explicit-id"), Some("SomeName"), Some("prod"))
            .await
            .unwrap();
        assert_eq!(id, "explicit-id");
    }

    #[tokio::test]
    async fn test_resolve_app_id_neither_provided() {
        let client = AppSignalClient::new("tok", None);
        let err = client
            .resolve_app_id("org", None, None, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("--app-id"));
        assert!(err.to_string().contains("--app"));
    }

    #[test]
    fn test_normalize_api_base_url_uses_default() {
        assert_eq!(normalize_api_base_url(None), "https://appsignal.com/");
    }

    #[test]
    fn test_normalize_api_base_url_preserves_base_url() {
        assert_eq!(
            normalize_api_base_url(Some("https://staging.lol")),
            "https://staging.lol/"
        );
    }

    #[test]
    fn test_normalize_api_base_url_strips_graphql_path() {
        assert_eq!(
            normalize_api_base_url(Some("https://staging.lol/graphql")),
            "https://staging.lol/"
        );
    }

    #[test]
    fn test_client_builds_graphql_and_rest_urls_from_base_url() {
        let client = AppSignalClient::new("tok", Some("https://staging.lol"));

        assert_eq!(client.graphql_url(), "https://staging.lol/graphql");
        assert_eq!(
            client.rest_url("/api/v2/auth"),
            "https://staging.lol/api/v2/auth"
        );
    }

    #[test]
    fn test_client_builds_urls_from_graphql_endpoint_input() {
        let client = AppSignalClient::with_endpoint("tok", "https://staging.lol/graphql");

        assert_eq!(client.graphql_url(), "https://staging.lol/graphql");
        assert_eq!(
            client.rest_url("/api/v2/auth"),
            "https://staging.lol/api/v2/auth"
        );
    }

    #[test]
    fn test_client_can_use_distinct_rest_base_url() {
        let client = AppSignalClient::with_auth_endpoints(
            AuthMethod::OAuth {
                access_token: "tok".to_string(),
                refresh_token: None,
                expires_at: None,
            },
            Some("https://app.localhost"),
            Some("https://public-api.localhost"),
        );

        assert_eq!(client.graphql_url(), "https://app.localhost/graphql");
        assert_eq!(
            client.rest_url("/api/v2/auth"),
            "https://public-api.localhost/api/v2/auth"
        );
    }

    #[tokio::test]
    async fn test_list_exception_incidents() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "exceptionIncidents": [
                            {
                                "__typename": "ExceptionIncident",
                                "id": "e1", "number": 1, "state": "OPEN",
                                "severity": "CRITICAL", "description": "Boom",
                                "count": 5,
                                "createdAt": "2025-01-01T00:00:00Z",
                                "lastOccurredAt": "2025-06-01T00:00:00Z",
                                "updatedAt": null,
                                "exceptionName": "RuntimeError",
                                "exceptionMessage": "fail",
                                "actionNames": ["FooController#bar"],
                                "namespace": "web",
                                "firstBacktraceLine": "app.rb:1"
                            }
                        ]
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let incidents = client
            .list_exception_incidents("app1", Some(10), None, None, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].kind(), "exception");
    }

    #[tokio::test]
    async fn test_list_anomaly_incidents() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "anomalyIncidents": [
                            {
                                "__typename": "AnomalyIncident",
                                "id": "a1", "number": 10, "state": "OPEN",
                                "severity": null, "description": "CPU spike",
                                "count": 3,
                                "createdAt": "2025-01-01T00:00:00Z",
                                "lastOccurredAt": "2025-06-01T00:00:00Z",
                                "updatedAt": null,
                                "alertState": "OPEN",
                                "trigger": { "id": "t1", "name": "CPU Alert", "metricName": "cpu_usage", "kind": "Advanced" },
                                "tags": [{ "key": "hostname", "value": "web-1" }]
                            }
                        ]
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let incidents = client
            .list_anomaly_incidents("app1", Some(10), None, None, None)
            .await
            .unwrap();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].kind(), "anomaly");
        assert_eq!(incidents[0].number(), 10);
        if let Incident::AnomalyIncident {
            alert_state,
            trigger,
            ..
        } = &incidents[0]
        {
            assert_eq!(alert_state.as_deref(), Some("OPEN"));
            assert_eq!(trigger.as_ref().unwrap().name, "CPU Alert");
        } else {
            panic!("Expected AnomalyIncident");
        }
    }

    #[tokio::test]
    async fn test_update_incident() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "updateIncident": {
                        "__typename": "ExceptionIncident",
                        "id": "e1", "number": 42, "state": "CLOSED",
                        "severity": "CRITICAL", "description": "Fixed",
                        "count": 10,
                        "createdAt": "2025-01-01T00:00:00Z",
                        "lastOccurredAt": "2025-06-01T00:00:00Z",
                        "updatedAt": "2025-06-02T00:00:00Z",
                        "exceptionName": "RuntimeError",
                        "exceptionMessage": "oops",
                        "actionNames": [],
                        "namespace": "web",
                        "firstBacktraceLine": "app.rb:1"
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let incident = client
            .update_incident("app1", 42, Some("CLOSED"), Some("CRITICAL"), None, None)
            .await
            .unwrap();
        assert_eq!(incident.number(), 42);
        assert_eq!(incident.state(), "CLOSED");
        assert_eq!(incident.severity(), "CRITICAL");
    }

    #[tokio::test]
    async fn test_create_incident_note() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "createIncidentNote": {
                        "__typename": "ExceptionIncident",
                        "id": "e1", "number": 42, "state": "OPEN",
                        "severity": "CRITICAL", "description": "Bad",
                        "count": 10,
                        "createdAt": "2025-01-01T00:00:00Z",
                        "lastOccurredAt": "2025-06-01T00:00:00Z",
                        "updatedAt": "2025-06-02T00:00:00Z",
                        "exceptionName": "RuntimeError",
                        "exceptionMessage": "oops",
                        "actionNames": [],
                        "namespace": "web",
                        "firstBacktraceLine": "app.rb:1"
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let incident = client
            .create_incident_note("app1", 42, "Investigation notes here")
            .await
            .unwrap();
        assert_eq!(incident.number(), 42);
        assert_eq!(incident.kind(), "exception");
    }

    #[tokio::test]
    async fn test_list_triggers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "triggers": [
                            {
                                "id": "trig-1",
                                "name": "Slow requests",
                                "metricName": "response_time",
                                "field": "MEAN",
                                "dashboardId": "dash-1",
                                "kind": "Advanced",
                                "warmupDuration": 5,
                                "cooldownDuration": 2,
                                "description": "Investigate latency spikes",
                                "noMatchIsZero": false,
                                "format": "duration",
                                "formatInput": null,
                                "previousTrigger": null,
                                "thresholdCondition": {
                                    "value": 500.0,
                                    "comparisonOperator": "GREATER_THAN"
                                },
                                "notifiers": [
                                    { "id": "n1", "name": "Slack", "icon": "slack" }
                                ],
                                "tags": [
                                    { "key": "namespace", "value": "web" }
                                ],
                                "user": { "id": "u1", "name": "Alice" }
                            }
                        ]
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let triggers = client.list_triggers("app1", None).await.unwrap();

        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].id, "trig-1");
        assert_eq!(triggers[0].metric_name, "response_time");
        assert_eq!(
            triggers[0].threshold_condition.comparison_operator,
            "GREATER_THAN"
        );
    }

    #[tokio::test]
    async fn test_create_trigger() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "createTrigger": {
                        "id": "trig-2",
                        "name": "High CPU",
                        "metricName": "cpu_usage",
                        "field": "GAUGE",
                        "dashboardId": null,
                        "kind": "HostCPUUsage",
                        "warmupDuration": 3,
                        "cooldownDuration": 1,
                        "description": "CPU is too high",
                        "noMatchIsZero": false,
                        "format": "percent",
                        "formatInput": null,
                        "previousTrigger": { "id": "trig-1" },
                        "thresholdCondition": {
                            "value": 85.0,
                            "comparisonOperator": "GREATER_THAN"
                        },
                        "notifiers": [],
                        "tags": [],
                        "user": { "id": "u1", "name": "Alice" }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let trigger = client
            .create_trigger(
                "app1",
                Some("trig-1"),
                Some("High CPU"),
                "cpu_usage",
                None,
                "HostCPUUsage",
                "GAUGE",
                "GREATER_THAN",
                85.0,
                3,
                1,
                None,
                false,
                Some("CPU is too high"),
                None,
                Some("percent"),
                None,
            )
            .await
            .unwrap();

        assert_eq!(trigger.id, "trig-2");
        assert_eq!(trigger.previous_trigger.as_ref().unwrap().id, "trig-1");
        assert_eq!(trigger.threshold_condition.value, 85.0);
    }

    #[tokio::test]
    async fn test_archive_trigger() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "archiveTrigger": {
                        "id": "trig-9",
                        "name": "Old trigger",
                        "metricName": "response_time",
                        "field": "MEAN",
                        "dashboardId": null,
                        "kind": "Advanced",
                        "warmupDuration": 5,
                        "cooldownDuration": 2,
                        "description": null,
                        "noMatchIsZero": false,
                        "format": null,
                        "formatInput": null,
                        "previousTrigger": null,
                        "thresholdCondition": {
                            "value": 500.0,
                            "comparisonOperator": "GREATER_THAN"
                        },
                        "notifiers": [],
                        "tags": [],
                        "user": null
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let trigger = client.archive_trigger("app1", "trig-9").await.unwrap();

        assert_eq!(trigger.id, "trig-9");
        assert_eq!(trigger.kind, "Advanced");
    }

    #[tokio::test]
    async fn test_create_dashboard() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "createDashboard": {
                        "id": "dash-2",
                        "title": "Overview",
                        "description": "Main dashboard",
                        "label": null,
                        "source": "USER_CREATED",
                        "createdAt": "2026-06-12T10:00:00Z",
                        "updatedAt": "2026-06-12T10:00:00Z"
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let dashboard = client
            .create_dashboard("app1", "Overview", Some("Main dashboard"))
            .await
            .unwrap();

        assert_eq!(dashboard.id, "dash-2");
        assert_eq!(dashboard.title.as_deref(), Some("Overview"));
        assert_eq!(dashboard.source, Some(DashboardSource::UserCreated));
    }

    #[tokio::test]
    async fn test_update_dashboard() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "updateDashboard": {
                        "id": "dash-2",
                        "title": "Overview v2",
                        "description": "Updated dashboard",
                        "label": "beta",
                        "source": "USER_CREATED",
                        "createdAt": "2026-06-12T10:00:00Z",
                        "updatedAt": "2026-06-12T11:00:00Z"
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let dashboard = client
            .update_dashboard("app1", "dash-2", "Overview v2", Some("Updated dashboard"))
            .await
            .unwrap();

        assert_eq!(dashboard.id, "dash-2");
        assert_eq!(dashboard.title.as_deref(), Some("Overview v2"));
        assert_eq!(dashboard.label.as_deref(), Some("beta"));
    }

    #[tokio::test]
    async fn test_list_log_line_actions() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "logs": {
                            "logLineActions": [
                                log_line_action_filter_json("action-1", 0),
                                log_line_action_metrics_json("action-2", 1)
                            ]
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let actions = client.list_log_line_actions("app1").await.unwrap();

        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].action_type(), "FILTER");
        assert_eq!(actions[1].metrics()[0].name, "log.error_count");
    }

    #[tokio::test]
    async fn test_create_log_line_action() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "createLogLineAction": log_line_action_metrics_json("action-2", 1)
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let action = client
            .create_log_line_action(
                "app1",
                "Track error count",
                "severity:error",
                LogLineActionKind::Metrics,
                None,
                Some(&[LogLineMetricInput {
                    name: "log.error_count".to_string(),
                    field: None,
                    metric_type: "COUNTER".to_string(),
                    tags: None,
                }]),
                None,
            )
            .await
            .unwrap();

        assert_eq!(action.id(), "action-2");
        assert_eq!(action.action_type(), "METRICS");
    }

    #[tokio::test]
    async fn test_update_log_line_action() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "updateLogLineAction": log_line_action_metrics_json("action-2", 1)
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let action = client
            .update_log_line_action(
                "app1",
                "action-2",
                Some("Renamed metric"),
                None,
                Some(&[]),
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(action.id(), "action-2");
    }

    #[tokio::test]
    async fn test_delete_log_line_action() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "deleteLogLineAction": log_line_action_metrics_json("action-2", 1)
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let action = client
            .delete_log_line_action("app1", "action-2")
            .await
            .unwrap();

        assert_eq!(action.id(), "action-2");
    }

    // -- LogLine deserialization tests --

    #[test]
    fn test_deserialize_log_line() {
        let json = r#"{
            "id": "019cf56c-f226-76dd-8b29-a0c66f71d124",
            "timestamp": "2025-03-16T06:54:36.832Z",
            "severity": "ERROR",
            "hostname": "worker-ams1",
            "group": "notifiers",
            "message": "[Email] Sending notification",
            "attributes": [{ "key": "request_id", "value": "abc123" }],
            "source": { "id": "s1", "name": "application" }
        }"#;
        let line: LogLine = serde_json::from_str(json).unwrap();
        assert_eq!(line.id, "019cf56c-f226-76dd-8b29-a0c66f71d124");
        assert_eq!(line.severity, "ERROR");
        assert_eq!(line.hostname, "worker-ams1");
        assert_eq!(line.group.as_deref(), Some("notifiers"));
        assert_eq!(line.message, "[Email] Sending notification");
        let attrs = line.attributes.unwrap();
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs[0].key, "request_id");
        assert_eq!(attrs[0].value.as_deref(), Some("abc123"));
        let source = line.source.unwrap();
        assert_eq!(source.id, "s1");
        assert_eq!(source.name.as_deref(), Some("application"));
    }

    #[test]
    fn test_deserialize_log_line_minimal() {
        let json = r#"{
            "id": "line1",
            "timestamp": "2025-01-01T00:00:00Z",
            "severity": "INFO",
            "hostname": "web-1",
            "group": null,
            "message": "Hello",
            "attributes": null,
            "source": null
        }"#;
        let line: LogLine = serde_json::from_str(json).unwrap();
        assert_eq!(line.id, "line1");
        assert!(line.group.is_none());
        assert!(line.attributes.is_none());
        assert!(line.source.is_none());
    }

    #[test]
    fn test_deserialize_log_view() {
        let json = r#"{
            "id": "view1",
            "name": "Error logs",
            "query": "severity=[error,critical]",
            "sourceIds": ["s1", "s2"],
            "severities": ["ERROR", "CRITICAL"],
            "columns": ["timestamp", "message"]
        }"#;
        let view: LogView = serde_json::from_str(json).unwrap();
        assert_eq!(view.id, "view1");
        assert_eq!(view.name, "Error logs");
        assert_eq!(view.query.as_deref(), Some("severity=[error,critical]"));
        assert_eq!(view.source_ids.as_ref().unwrap(), &["s1", "s2"]);
        assert_eq!(view.severities.as_ref().unwrap(), &["ERROR", "CRITICAL"]);
        assert_eq!(view.columns.as_ref().unwrap(), &["timestamp", "message"]);
    }

    #[test]
    fn test_deserialize_log_view_minimal() {
        let json = r#"{
            "id": "view2",
            "name": "All logs",
            "query": null,
            "sourceIds": null,
            "severities": null,
            "columns": null
        }"#;
        let view: LogView = serde_json::from_str(json).unwrap();
        assert_eq!(view.id, "view2");
        assert_eq!(view.name, "All logs");
        assert!(view.query.is_none());
        assert!(view.source_ids.is_none());
    }

    #[test]
    fn test_deserialize_log_source() {
        let json = r#"{
            "id": "src1",
            "name": "Application",
            "type": "vector",
            "fmt": "JSON"
        }"#;
        let source: LogSource = serde_json::from_str(json).unwrap();
        assert_eq!(source.id, "src1");
        assert_eq!(source.name, "Application");
        assert_eq!(source.kind.as_deref(), Some("vector"));
        assert_eq!(source.fmt.as_deref(), Some("JSON"));
    }

    // -- Wiremock tests for log API methods --

    #[tokio::test]
    async fn test_list_log_views() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "logViews": [
                            {
                                "id": "v1",
                                "name": "Error logs",
                                "query": "severity=[error]",
                                "sourceIds": ["s1"],
                                "severities": ["ERROR"],
                                "columns": ["timestamp", "message"]
                            },
                            {
                                "id": "v2",
                                "name": "All logs",
                                "query": null,
                                "sourceIds": [],
                                "severities": [],
                                "columns": null
                            }
                        ]
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let views = client.list_log_views("app1").await.unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].id, "v1");
        assert_eq!(views[0].name, "Error logs");
        assert_eq!(views[0].query.as_deref(), Some("severity=[error]"));
        assert_eq!(views[1].id, "v2");
        assert!(views[1].query.is_none());
    }

    #[tokio::test]
    async fn test_list_log_views_empty() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": { "logViews": [] }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let views = client.list_log_views("app1").await.unwrap();
        assert!(views.is_empty());
    }

    #[tokio::test]
    async fn test_get_log_view() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "logView": {
                            "id": "v1",
                            "name": "Error logs",
                            "query": "severity=[error]",
                            "sourceIds": ["s1"],
                            "severities": ["ERROR"],
                            "columns": []
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let view = client.get_log_view("app1", "v1").await.unwrap();
        assert_eq!(view.id, "v1");
        assert_eq!(view.name, "Error logs");
    }

    #[tokio::test]
    async fn test_get_log_view_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": { "logView": null }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let err = client.get_log_view("app1", "nope").await.unwrap_err();
        assert!(err.to_string().contains("nope"));
    }

    #[tokio::test]
    async fn test_list_log_sources() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "logs": {
                            "sources": [
                                { "id": "s1", "name": "Application", "type": "vector", "fmt": "JSON" },
                                { "id": "s2", "name": "Custom", "type": "custom", "fmt": "PLAINTEXT" }
                            ]
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let sources = client.list_log_sources("app1").await.unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].id, "s1");
        assert_eq!(sources[0].name, "Application");
        assert_eq!(sources[0].kind.as_deref(), Some("vector"));
        assert_eq!(sources[1].fmt.as_deref(), Some("PLAINTEXT"));
    }

    #[tokio::test]
    async fn test_list_log_sources_empty() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": { "logs": { "sources": [] } }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let sources = client.list_log_sources("app1").await.unwrap();
        assert!(sources.is_empty());
    }
}
