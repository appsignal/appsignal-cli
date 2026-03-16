use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

const DEFAULT_GRAPHQL_ENDPOINT: &str = "https://appsignal.com/graphql";

/// Client for the AppSignal API.
pub struct AppSignalClient {
    http: Client,
    token: String,
    endpoint: String,
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
struct TypenameData {
    #[serde(rename = "__typename")]
    #[allow(dead_code)]
    typename: Option<String>,
}

// -- Viewer types --

#[derive(Debug, Deserialize, Serialize)]
pub struct ViewerOrganization {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct Viewer {
    organizations: Option<Vec<ViewerOrganization>>,
}

#[derive(Debug, Deserialize)]
struct ViewerData {
    viewer: Option<Viewer>,
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
}

/// Resources available for an application.
#[derive(Debug, Default, Serialize)]
pub struct AppResources {
    pub users: Option<Vec<User>>,
    pub notifiers: Option<Vec<Notifier>>,
    pub namespaces: Option<Vec<String>>,
    pub dashboards: Option<Vec<Dashboard>>,
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
    namespaces: Option<Vec<String>>,
    dashboards: Option<Vec<Dashboard>>,
}

// -- Mutation response types --

#[derive(Debug, Deserialize)]
struct UpdateIncidentData {
    #[serde(rename = "updateIncident")]
    update_incident: Option<Incident>,
}

#[derive(Debug, Deserialize)]
struct CreateIncidentNoteData {
    #[serde(rename = "createIncidentNote")]
    create_incident_note: Option<Incident>,
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

#[derive(Debug, Deserialize)]
struct AppLogsData {
    app: Option<AppLogs>,
}

#[derive(Debug, Deserialize)]
struct AppLogs {
    logs: Option<LogsInner>,
}

#[derive(Debug, Deserialize)]
struct LogsInner {
    lines: Option<Vec<LogLine>>,
    #[allow(dead_code)]
    sources: Option<Vec<LogSource>>,
    #[serde(rename = "queryWindow")]
    #[allow(dead_code)]
    query_window: Option<i64>,
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
                anyhow::bail!(
                    "Multiple users match '{}'. Use an email or ID to disambiguate:\n{}",
                    ident,
                    descriptions.join("\n")
                )
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
            anyhow::bail!(msg)
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
            anyhow::bail!(
                "Multiple apps match name '{}'. Use --environment to disambiguate:\n{}",
                name,
                descriptions.join("\n")
            )
        }
    }
}

impl AppSignalClient {
    pub fn new(token: &str, endpoint: Option<&str>) -> Self {
        Self {
            http: Client::new(),
            token: token.to_string(),
            endpoint: endpoint.unwrap_or(DEFAULT_GRAPHQL_ENDPOINT).to_string(),
        }
    }

    /// Create a client pointing at a custom endpoint.
    #[cfg(test)]
    pub fn with_endpoint(token: &str, endpoint: &str) -> Self {
        Self {
            http: Client::new(),
            token: token.to_string(),
            endpoint: endpoint.to_string(),
        }
    }

    /// Execute a GraphQL query against AppSignal.
    async fn graphql<T: serde::de::DeserializeOwned>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T> {
        let url = format!("{}?token={}", self.endpoint, self.token);
        let body = json!({
            "query": query,
            "variables": variables,
        });

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to send request to AppSignal")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("AppSignal API returned HTTP {}: {}", status, text);
        }

        let gql_resp: GraphQLResponse<T> = resp
            .json()
            .await
            .context("Failed to parse AppSignal response")?;

        if let Some(errors) = gql_resp.errors {
            let msgs: Vec<String> = errors.into_iter().map(|e| e.message).collect();
            anyhow::bail!("GraphQL errors: {}", msgs.join("; "));
        }

        gql_resp
            .data
            .context("No data in AppSignal GraphQL response")
    }

    /// Validate that the token is accepted by the API.
    pub async fn validate_token(&self) -> Result<()> {
        let query = "{ __typename }";
        let _data: TypenameData = self.graphql(query, json!({})).await?;
        Ok(())
    }

    /// Get all organizations the authenticated user has access to.
    pub async fn list_organizations(&self) -> Result<Vec<ViewerOrganization>> {
        let query = r#"
            {
                viewer {
                    organizations {
                        slug
                        name
                    }
                }
            }
        "#;
        let data: ViewerData = self.graphql(query, json!({})).await?;
        let viewer = data.viewer.context("Could not fetch viewer data")?;
        Ok(viewer.organizations.unwrap_or_default())
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
            .with_context(|| format!("Organization '{}' not found", org_slug))?;
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
            .with_context(|| format!("Application '{}' not found", app_id))
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
        anyhow::bail!("Provide either --app-id or --app (with optional --environment)")
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
        let app = data.app.context("Application not found")?;
        Ok(app.users.unwrap_or_default())
    }

    /// Get resources for an application (users, notifiers, namespaces, dashboards).
    pub async fn get_app_resources(
        &self,
        app_id: &str,
        sections: &[String],
    ) -> Result<AppResources> {
        // Build a dynamic query based on requested sections
        let include_all = sections.is_empty();
        let want = |s: &str| include_all || sections.iter().any(|x| x == s);

        let mut fields = String::new();
        if want("users") {
            fields.push_str("users { id name email } ");
        }
        if want("notifiers") {
            fields.push_str("notifiers { id name icon } ");
        }
        if want("namespaces") {
            fields.push_str("namespaces ");
        }
        if want("dashboards") {
            fields.push_str("dashboards { id title description } ");
        }

        let query = format!(
            "query AppResources($appId: String!) {{ app(id: $appId) {{ {} }} }}",
            fields
        );

        let data: AppResourcesData = self.graphql(&query, json!({ "appId": app_id })).await?;
        let app = data.app.context("Application not found")?;

        Ok(AppResources {
            users: app.users,
            notifiers: app.notifiers,
            namespaces: app.namespaces,
            dashboards: app.dashboards,
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
        let app = data.app.context("Application not found")?;
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
        let app = data.app.context("Application not found")?;
        Ok(app.exception_incidents.unwrap_or_default())
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
        let app = data.app.context("Application not found")?;
        Ok(app.anomaly_incidents.unwrap_or_default())
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
        let app = data.app.context("Application not found")?;
        app.incident
            .with_context(|| format!("Incident #{} not found", incident_number))
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
        data.update_incident
            .with_context(|| format!("Failed to update incident #{}", incident_number))
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
        data.create_incident_note
            .with_context(|| format!("Failed to create note on incident #{}", incident_number))
    }

    // -- Log methods --

    /// Query log lines for an app via the GraphQL `logs.lines` field.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_log_lines(
        &self,
        app_id: &str,
        start: Option<&str>,
        end: Option<&str>,
        source_ids: Option<&[String]>,
        severities: Option<&[String]>,
        query: Option<&str>,
        limit: Option<i64>,
        order: Option<&str>,
    ) -> Result<Vec<LogLine>> {
        let gql = r#"
            query AppLogLines($appId: String!, $start: PreciseDateTime, $end: PreciseDateTime,
                              $sourceIds: [String!], $severities: [SeverityEnum!],
                              $query: String, $limit: Int, $order: OrderEnum) {
                app(id: $appId) {
                    logs {
                        lines(start: $start, end: $end, sourceIds: $sourceIds,
                              severities: $severities, query: $query, limit: $limit, order: $order) {
                            id
                            timestamp
                            severity
                            hostname
                            group
                            message
                            attributes { key value }
                            source { id name }
                        }
                    }
                }
            }
        "#;

        let mut vars = json!({ "appId": app_id });
        if let Some(s) = start {
            vars["start"] = json!(s);
        }
        if let Some(e) = end {
            vars["end"] = json!(e);
        }
        if let Some(sids) = source_ids {
            vars["sourceIds"] = json!(sids);
        }
        if let Some(sevs) = severities {
            vars["severities"] = json!(sevs);
        }
        if let Some(q) = query {
            vars["query"] = json!(q);
        }
        if let Some(l) = limit {
            vars["limit"] = json!(l);
        }
        if let Some(o) = order {
            vars["order"] = json!(o);
        }

        let data: AppLogsData = self.graphql(gql, vars).await?;
        let app = data.app.context("Application not found")?;
        let logs = app.logs.context("Logs not available for this app")?;
        Ok(logs.lines.unwrap_or_default())
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
        let app = data.app.context("Application not found")?;
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
        let app = data.app.context("Application not found")?;
        app.log_view
            .with_context(|| format!("Log view '{}' not found", view_id))
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
        let app = data.app.context("Application not found")?;
        let logs = app.logs.context("Logs not available for this app")?;
        Ok(logs.sources.unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
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

    #[tokio::test]
    async fn test_validate_token_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(graphql_response(json!({ "__typename": "Query" }))),
            )
            .mount(&server)
            .await;

        let client =
            AppSignalClient::with_endpoint("test-token", &format!("{}/graphql", server.uri()));
        client.validate_token().await.unwrap();
    }

    #[tokio::test]
    async fn test_validate_token_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&server)
            .await;

        let client =
            AppSignalClient::with_endpoint("bad-token", &format!("{}/graphql", server.uri()));
        let err = client.validate_token().await.unwrap_err();
        assert!(err.to_string().contains("401"));
    }

    #[tokio::test]
    async fn test_validate_token_graphql_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": null,
                "errors": [{"message": "Invalid token"}]
            })))
            .mount(&server)
            .await;

        let client =
            AppSignalClient::with_endpoint("bad-token", &format!("{}/graphql", server.uri()));
        let err = client.validate_token().await.unwrap_err();
        assert!(err.to_string().contains("Invalid token"));
    }

    #[tokio::test]
    async fn test_list_organizations() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "viewer": {
                        "organizations": [
                            { "slug": "org-one", "name": "Org One" },
                            { "slug": "org-two", "name": "Org Two" }
                        ]
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let orgs = client.list_organizations().await.unwrap();
        assert_eq!(orgs.len(), 2);
        assert_eq!(orgs[0].slug, "org-one");
        assert_eq!(orgs[1].name, "Org Two");
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
    async fn test_list_log_lines() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "logs": {
                            "lines": [
                                {
                                    "id": "line1",
                                    "timestamp": "2025-06-01T12:00:00Z",
                                    "severity": "ERROR",
                                    "hostname": "web-1",
                                    "group": "notifiers",
                                    "message": "Something broke",
                                    "attributes": [{ "key": "req", "value": "123" }],
                                    "source": { "id": "s1", "name": "app" }
                                },
                                {
                                    "id": "line2",
                                    "timestamp": "2025-06-01T12:01:00Z",
                                    "severity": "INFO",
                                    "hostname": "web-2",
                                    "group": null,
                                    "message": "All good",
                                    "attributes": [],
                                    "source": null
                                }
                            ]
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let lines = client
            .list_log_lines("app1", None, None, None, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].id, "line1");
        assert_eq!(lines[0].severity, "ERROR");
        assert_eq!(lines[0].message, "Something broke");
        assert_eq!(lines[1].id, "line2");
        assert_eq!(lines[1].severity, "INFO");
    }

    #[tokio::test]
    async fn test_list_log_lines_empty() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": { "logs": { "lines": [] } }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let lines = client
            .list_log_lines("app1", None, None, None, None, None, None, None)
            .await
            .unwrap();
        assert!(lines.is_empty());
    }

    #[tokio::test]
    async fn test_list_log_lines_with_filters() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(graphql_response(json!({
                    "app": {
                        "logs": {
                            "lines": [{
                                "id": "filtered1",
                                "timestamp": "2025-06-01T12:00:00Z",
                                "severity": "ERROR",
                                "hostname": "web-1",
                                "group": null,
                                "message": "Filtered result",
                                "attributes": [],
                                "source": null
                            }]
                        }
                    }
                }))),
            )
            .mount(&server)
            .await;

        let client = AppSignalClient::with_endpoint("tok", &format!("{}/graphql", server.uri()));
        let sids = vec!["s1".to_string()];
        let sevs = vec!["ERROR".to_string()];
        let lines = client
            .list_log_lines(
                "app1",
                Some("2025-06-01T00:00:00Z"),
                Some("2025-06-02T00:00:00Z"),
                Some(&sids),
                Some(&sevs),
                Some("timeout"),
                Some(50),
                Some("ASC"),
            )
            .await
            .unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].id, "filtered1");
    }

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
