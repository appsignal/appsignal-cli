use std::time::Duration;

use reqwest::header::{ACCEPT, USER_AGENT};
use semver::Version;
use serde::Deserialize;

const GITHUB_TAGS_URL: &str =
    "https://api.github.com/repos/appsignal/homebrew-appsignal-cli/tags?per_page=1";
const USER_AGENT_VALUE: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Deserialize)]
struct GitHubTag {
    name: String,
}

pub async fn newer_version_available() -> Option<String> {
    newer_version_available_at(GITHUB_TAGS_URL).await
}

async fn newer_version_available_at(url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;

    let response = client
        .get(url)
        .header(USER_AGENT, USER_AGENT_VALUE)
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .ok()?;

    if response.status() != reqwest::StatusCode::OK {
        return None;
    }

    let tags: Vec<GitHubTag> = response.json().await.ok()?;
    let latest_version = parse_version(tags.first()?.name.as_str())?;
    let current_version = parse_version(env!("CARGO_PKG_VERSION"))?;

    (latest_version > current_version).then(|| latest_version.to_string())
}

fn parse_version(raw: &str) -> Option<Version> {
    Version::parse(raw.trim_start_matches('v')).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn returns_newer_version_when_github_responds_200_with_newer_tag() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tags"))
            .and(query_param("per_page", "1"))
            .and(header("user-agent", USER_AGENT_VALUE))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": "v9.9.9" }
            ])))
            .mount(&server)
            .await;

        let result = newer_version_available_at(&format!("{}/tags?per_page=1", server.uri())).await;

        assert_eq!(result.as_deref(), Some("9.9.9"));
    }

    #[tokio::test]
    async fn returns_none_for_same_version() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": format!("v{}", env!("CARGO_PKG_VERSION")) }
            ])))
            .mount(&server)
            .await;

        let result = newer_version_available_at(&format!("{}/tags?per_page=1", server.uri())).await;

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn returns_none_for_older_version() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": "v0.0.1" }
            ])))
            .mount(&server)
            .await;

        let result = newer_version_available_at(&format!("{}/tags?per_page=1", server.uri())).await;

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn returns_none_when_github_does_not_respond_200() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tags"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let result = newer_version_available_at(&format!("{}/tags?per_page=1", server.uri())).await;

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn returns_none_when_tag_version_cannot_be_parsed() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": "not-a-version" }
            ])))
            .mount(&server)
            .await;

        let result = newer_version_available_at(&format!("{}/tags?per_page=1", server.uri())).await;

        assert_eq!(result, None);
    }
}
