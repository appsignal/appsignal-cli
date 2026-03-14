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

#[derive(Debug, Deserialize)]
struct Organization {
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
}
