use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

const GRAPHQL_ENDPOINT: &str = "https://appsignal.com/graphql";

/// Client for the AppSignal API.
pub struct AppSignalClient {
    http: Client,
    token: String,
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

// -- Incident types --

/// Common fields shared across all incident types.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "__typename")]
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
            Incident::ExceptionIncident { last_occurred_at, .. }
            | Incident::PerformanceIncident { last_occurred_at, .. }
            | Incident::AnomalyIncident { last_occurred_at, .. }
            | Incident::LogIncident { last_occurred_at, .. } => {
                last_occurred_at.as_deref().unwrap_or("-")
            }
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

impl AppSignalClient {
    pub fn new(token: &str) -> Self {
        Self {
            http: Client::new(),
            token: token.to_string(),
        }
    }

    /// Execute a GraphQL query against AppSignal.
    async fn graphql<T: serde::de::DeserializeOwned>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T> {
        let url = format!("{}?token={}", GRAPHQL_ENDPOINT, self.token);
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
        let data: OrganizationData = self
            .graphql(query, json!({ "slug": org_slug }))
            .await?;
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
        let data: AppData = self
            .graphql(query, json!({ "appId": app_id }))
            .await?;
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

    /// List incidents for an app.
    pub async fn list_incidents(
        &self,
        app_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
        state: Option<&str>,
        order: Option<&str>,
    ) -> Result<Vec<Incident>> {
        let query = r#"
            query AppIncidents($appId: String!, $limit: Int, $offset: Int, $state: IncidentStateEnum, $order: IncidentOrderEnum) {
                app(id: $appId) {
                    incidents(limit: $limit, offset: $offset, state: $state, order: $order) {
                        __typename
                        ... on ExceptionIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                            exceptionName exceptionMessage actionNames namespace firstBacktraceLine
                        }
                        ... on PerformanceIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                            actionNames namespace mean totalDuration
                        }
                        ... on AnomalyIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                        }
                        ... on LogIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
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

        let data: AppIncidentsData = self.graphql(query, vars).await?;
        let app = data.app.context("Application not found")?;
        Ok(app.incidents.unwrap_or_default())
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
                        }
                        ... on PerformanceIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                            actionNames namespace mean totalDuration
                        }
                        ... on AnomalyIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
                        }
                        ... on LogIncident {
                            id number state severity description count
                            createdAt lastOccurredAt updatedAt
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
}
