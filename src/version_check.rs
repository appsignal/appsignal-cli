use std::time::Duration;

use reqwest::header::{ACCEPT, USER_AGENT};
use semver::Version;
use serde::Deserialize;

use crate::client_headers::USER_AGENT_VALUE;

const GITHUB_TAGS_URL: &str =
    "https://api.github.com/repos/appsignal/homebrew-appsignal-cli/tags?per_page=1";

#[derive(Debug, PartialEq, Eq)]
pub enum VersionCheck {
    UpToDate,
    UpgradeAvailable(String),
    UpgradeRequired(String),
}

#[derive(Debug, Deserialize)]
struct GitHubTag {
    name: String,
}

pub async fn check() -> VersionCheck {
    check_at(GITHUB_TAGS_URL).await
}

async fn check_at(url: &str) -> VersionCheck {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(client) => client,
        Err(_) => return VersionCheck::UpToDate,
    };

    let response = match client
        .get(url)
        .header(USER_AGENT, USER_AGENT_VALUE)
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return VersionCheck::UpToDate,
    };

    if response.status() != reqwest::StatusCode::OK {
        return VersionCheck::UpToDate;
    }

    let tags: Vec<GitHubTag> = match response.json().await {
        Ok(tags) => tags,
        Err(_) => return VersionCheck::UpToDate,
    };

    classify_versions(
        env!("CARGO_PKG_VERSION"),
        match tags.first() {
            Some(tag) => tag.name.as_str(),
            None => return VersionCheck::UpToDate,
        },
    )
}

fn classify_versions(current: &str, latest: &str) -> VersionCheck {
    let current_version = match parse_version(current) {
        Some(version) => version,
        None => return VersionCheck::UpToDate,
    };
    let latest_version = match parse_version(latest) {
        Some(version) => version,
        None => return VersionCheck::UpToDate,
    };

    if latest_version <= current_version {
        return VersionCheck::UpToDate;
    }

    if latest_version.major > current_version.major {
        return VersionCheck::UpgradeRequired(latest_version.to_string());
    }

    VersionCheck::UpgradeAvailable(latest_version.to_string())
}

fn parse_version(raw: &str) -> Option<Version> {
    Version::parse(raw.trim_start_matches('v')).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn newer_same_major_version() -> String {
        let mut version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        version.minor += 1;
        version.patch = 0;
        version.to_string()
    }

    fn newer_major_version() -> String {
        let mut version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        version.major += 1;
        version.minor = 0;
        version.patch = 0;
        version.to_string()
    }

    #[test]
    fn classify_versions_ignores_same_version() {
        assert_eq!(classify_versions("0.2.1", "v0.2.1"), VersionCheck::UpToDate);
    }

    #[test]
    fn classify_versions_ignores_older_version() {
        assert_eq!(classify_versions("0.2.1", "v0.2.0"), VersionCheck::UpToDate);
    }

    #[test]
    fn classify_versions_warns_for_newer_same_major_version() {
        assert_eq!(
            classify_versions("0.2.1", "v0.3.0"),
            VersionCheck::UpgradeAvailable("0.3.0".to_string())
        );
    }

    #[test]
    fn classify_versions_blocks_for_newer_major_version() {
        assert_eq!(
            classify_versions("0.2.1", "v1.0.0"),
            VersionCheck::UpgradeRequired("1.0.0".to_string())
        );
    }

    #[tokio::test]
    async fn check_warns_when_github_returns_200_with_newer_same_major_version() {
        let server = MockServer::start().await;
        let latest_version = newer_same_major_version();

        Mock::given(method("GET"))
            .and(path("/tags"))
            .and(query_param("per_page", "1"))
            .and(header("user-agent", USER_AGENT_VALUE))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": format!("v{latest_version}") }
            ])))
            .mount(&server)
            .await;

        let result = check_at(&format!("{}/tags?per_page=1", server.uri())).await;

        assert_eq!(result, VersionCheck::UpgradeAvailable(latest_version));
    }

    #[tokio::test]
    async fn check_blocks_when_github_returns_200_with_newer_major_version() {
        let server = MockServer::start().await;
        let latest_version = newer_major_version();

        Mock::given(method("GET"))
            .and(path("/tags"))
            .and(query_param("per_page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": format!("v{latest_version}") }
            ])))
            .mount(&server)
            .await;

        let result = check_at(&format!("{}/tags?per_page=1", server.uri())).await;

        assert_eq!(result, VersionCheck::UpgradeRequired(latest_version));
    }

    #[tokio::test]
    async fn check_passes_when_github_does_not_respond_200() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tags"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let result = check_at(&format!("{}/tags?per_page=1", server.uri())).await;

        assert_eq!(result, VersionCheck::UpToDate);
    }

    #[tokio::test]
    async fn check_passes_when_tag_cannot_be_parsed() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": "not-a-version" }
            ])))
            .mount(&server)
            .await;

        let result = check_at(&format!("{}/tags?per_page=1", server.uri())).await;

        assert_eq!(result, VersionCheck::UpToDate);
    }
}
