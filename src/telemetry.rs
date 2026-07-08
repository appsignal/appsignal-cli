use std::time::Duration;

use reqwest::Client;
use serde::Serialize;

use crate::client_headers::{with_appsignal_headers, CLIENT_VERSION};
use crate::config::Config;
use crate::output::Output;

const DEFAULT_BASE_URL: &str = "https://appsignal.com";
const TELEMETRY_PATH: &str = "/api/cli_telemetry";
const TELEMETRY_ENV_VAR: &str = "APPSIGNAL_CLI_TELEMETRY";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
enum Event {
    #[serde(rename = "command.run")]
    CommandRun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum TelemetryCommand {
    #[serde(rename = "about")]
    About,
    #[serde(rename = "auth.login")]
    AuthLogin,
    #[serde(rename = "auth.logout")]
    AuthLogout,
    #[serde(rename = "auth.status")]
    AuthStatus,
    #[serde(rename = "apps.list")]
    AppsList,
    #[serde(rename = "apps.info")]
    AppsInfo,
    #[serde(rename = "apps.find")]
    AppsFind,
    #[serde(rename = "apps.set-org")]
    AppsSetOrg,
    #[serde(rename = "apps.show-org")]
    AppsShowOrg,
    #[serde(rename = "apps.resources.all")]
    AppsResourcesAll,
    #[serde(rename = "apps.resources.users")]
    AppsResourcesUsers,
    #[serde(rename = "apps.resources.notifiers")]
    AppsResourcesNotifiers,
    #[serde(rename = "apps.resources.namespaces")]
    AppsResourcesNamespaces,
    #[serde(rename = "apps.resources.dashboards")]
    AppsResourcesDashboards,
    #[serde(rename = "apps.resources.deploy-markers")]
    AppsResourcesDeployMarkers,
    #[serde(rename = "project.init")]
    ProjectInit,
    #[serde(rename = "dashboards.list")]
    DashboardsList,
    #[serde(rename = "dashboards.create")]
    DashboardsCreate,
    #[serde(rename = "dashboards.update")]
    DashboardsUpdate,
    #[serde(rename = "incidents.list")]
    IncidentsList,
    #[serde(rename = "incidents.list-exceptions")]
    IncidentsListExceptions,
    #[serde(rename = "incidents.list-performance")]
    IncidentsListPerformance,
    #[serde(rename = "incidents.list-anomalies")]
    IncidentsListAnomalies,
    #[serde(rename = "incidents.show")]
    IncidentsShow,
    #[serde(rename = "incidents.update")]
    IncidentsUpdate,
    #[serde(rename = "incidents.add-note")]
    IncidentsAddNote,
    #[serde(rename = "logs.tail")]
    LogsTail,
    #[serde(rename = "logs.search")]
    LogsSearch,
    #[serde(rename = "logs.views")]
    LogsViews,
    #[serde(rename = "logs.sources")]
    LogsSources,
    #[serde(rename = "logs.metrics.list")]
    LogsMetricsList,
    #[serde(rename = "logs.metrics.create")]
    LogsMetricsCreate,
    #[serde(rename = "logs.metrics.update")]
    LogsMetricsUpdate,
    #[serde(rename = "logs.metrics.delete")]
    LogsMetricsDelete,
    #[serde(rename = "logs.triggers.list")]
    LogsTriggersList,
    #[serde(rename = "logs.triggers.create")]
    LogsTriggersCreate,
    #[serde(rename = "logs.triggers.update")]
    LogsTriggersUpdate,
    #[serde(rename = "logs.triggers.delete")]
    LogsTriggersDelete,
    #[serde(rename = "traces.list")]
    TracesList,
    #[serde(rename = "traces.incident")]
    TracesIncident,
    #[serde(rename = "traces.errors")]
    TracesErrors,
    #[serde(rename = "traces.show")]
    TracesShow,
    #[serde(rename = "traces.show-error")]
    TracesShowError,
    #[serde(rename = "traces.show-incident")]
    TracesShowIncident,
    #[serde(rename = "triggers.list")]
    TriggersList,
    #[serde(rename = "triggers.create")]
    TriggersCreate,
    #[serde(rename = "triggers.update")]
    TriggersUpdate,
    #[serde(rename = "triggers.archive")]
    TriggersArchive,
    #[serde(rename = "feedback")]
    Feedback,
    #[serde(rename = "skill.install")]
    SkillInstall,
    #[serde(rename = "skill.update")]
    SkillUpdate,
    #[serde(rename = "skill.status")]
    SkillStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
enum OutputFormat {
    #[serde(rename = "human")]
    Human,
    #[serde(rename = "json")]
    Json,
}

impl From<Output> for OutputFormat {
    fn from(output: Output) -> Self {
        match output {
            Output::Human => Self::Human,
            Output::Json => Self::Json,
        }
    }
}

#[derive(Serialize)]
struct CommandRunEvent {
    event: Event,
    command: TelemetryCommand,
    success: bool,
    duration_ms: u64,
    cli_version: &'static str,
    output_format: OutputFormat,
}

pub async fn track_command(
    command: TelemetryCommand,
    success: bool,
    duration: Duration,
    output: Output,
) {
    if !telemetry_enabled() {
        return;
    }

    let base_url = telemetry_base_url();
    let client = match Client::builder().timeout(Duration::from_secs(1)).build() {
        Ok(client) => client,
        Err(_) => return,
    };

    let _ = with_appsignal_headers(client.post(telemetry_url(&base_url)))
        .json(&CommandRunEvent {
            event: Event::CommandRun,
            command,
            success,
            duration_ms: duration.as_millis().min(u64::MAX as u128) as u64,
            cli_version: CLIENT_VERSION,
            output_format: output.into(),
        })
        .send()
        .await;
}

fn telemetry_base_url() -> String {
    Config::load()
        .ok()
        .and_then(|config| config.endpoint_base_url().ok().flatten())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
}

fn telemetry_url(base_url: &str) -> String {
    format!("{}{}", base_url.trim_end_matches('/'), TELEMETRY_PATH)
}

fn telemetry_enabled() -> bool {
    telemetry_enabled_from_env(std::env::var(TELEMETRY_ENV_VAR).ok().as_deref())
}

fn telemetry_enabled_from_env(value: Option<&str>) -> bool {
    !matches!(
        value.map(|raw| raw.trim().to_ascii_lowercase()),
        Some(value) if matches!(value.as_str(), "0" | "false" | "off" | "no")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn telemetry_command_serializes_to_command_path() {
        assert_eq!(
            serde_json::to_value(TelemetryCommand::AppsResourcesDeployMarkers).unwrap(),
            serde_json::json!("apps.resources.deploy-markers")
        );
    }

    #[test]
    fn output_format_serializes_to_expected_value() {
        assert_eq!(
            serde_json::to_value(OutputFormat::Json).unwrap(),
            serde_json::json!("json")
        );
    }

    #[test]
    fn telemetry_enabled_defaults_to_true() {
        assert!(telemetry_enabled_from_env(None));
    }

    #[test]
    fn telemetry_enabled_honors_opt_out_values() {
        for value in ["0", "false", "off", "no", "FALSE"] {
            assert!(!telemetry_enabled_from_env(Some(value)));
        }
    }

    #[test]
    fn telemetry_url_appends_endpoint_path() {
        assert_eq!(
            telemetry_url("https://appsignal.com/"),
            "https://appsignal.com/api/cli_telemetry"
        );
    }

    #[tokio::test]
    async fn track_command_posts_expected_payload() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/cli_telemetry"))
            .and(body_json(serde_json::json!({
                "event": "command.run",
                "command": "apps.list",
                "success": true,
                "duration_ms": 250,
                "cli_version": CLIENT_VERSION,
                "output_format": "json"
            })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = Client::builder()
            .timeout(Duration::from_secs(1))
            .build()
            .unwrap();
        let _ = with_appsignal_headers(client.post(telemetry_url(&server.uri())))
            .json(&CommandRunEvent {
                event: Event::CommandRun,
                command: TelemetryCommand::AppsList,
                success: true,
                duration_ms: 250,
                cli_version: CLIENT_VERSION,
                output_format: OutputFormat::Json,
            })
            .send()
            .await
            .unwrap();
    }
}
