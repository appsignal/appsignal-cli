use std::time::Duration;

use reqwest::Client;
use serde::Serialize;

use crate::client_headers::{with_appsignal_headers, CLIENT_VERSION};
use crate::config::Config;
use crate::output::Output;

const DEFAULT_BASE_URL: &str = "https://appsignal.com";
const TELEMETRY_PATH: &str = "/api/cli_telemetry";
const TELEMETRY_ENV_VAR: &str = "APPSIGNAL_CLI_TELEMETRY";

#[derive(Serialize)]
struct CommandRunEvent<'a> {
    event: &'a str,
    command: &'a str,
    success: bool,
    duration_ms: u64,
    cli_version: &'a str,
    output_format: &'a str,
}

pub fn command_path_from_env_args() -> Option<String> {
    let mut command_parts = Vec::new();
    let mut skip_next = false;

    for arg in std::env::args().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }

        match arg.as_str() {
            "-o" | "--output" | "--format" => {
                skip_next = true;
            }
            _ if arg.starts_with('-') => {
                if !command_parts.is_empty() {
                    break;
                }
            }
            _ => command_parts.push(arg),
        }
    }

    if command_parts.is_empty() {
        None
    } else {
        Some(command_parts.join("."))
    }
}

pub async fn track_command(command: &str, success: bool, duration: Duration, output: Output) {
    if !telemetry_enabled() || command.is_empty() {
        return;
    }

    let base_url = telemetry_base_url();
    let client = match Client::builder().timeout(Duration::from_secs(1)).build() {
        Ok(client) => client,
        Err(_) => return,
    };

    let _ = with_appsignal_headers(client.post(telemetry_url(&base_url)))
        .json(&CommandRunEvent {
            event: "command.run",
            command,
            success,
            duration_ms: duration.as_millis().min(u64::MAX as u128) as u64,
            cli_version: CLIENT_VERSION,
            output_format: output_label(output),
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

fn output_label(output: Output) -> &'static str {
    match output {
        Output::Human => "human",
        Output::Json => "json",
    }
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
    fn command_path_skips_global_output_flags() {
        let command = [
            "appsignal-cli",
            "--output",
            "json",
            "apps",
            "resources",
            "deploy-markers",
            "--org",
            "my-org",
        ];

        let mut command_parts = Vec::new();
        let mut skip_next = false;

        for arg in command.into_iter().skip(1) {
            if skip_next {
                skip_next = false;
                continue;
            }

            match arg {
                "-o" | "--output" | "--format" => skip_next = true,
                _ if arg.starts_with('-') => {
                    if !command_parts.is_empty() {
                        break;
                    }
                }
                _ => command_parts.push(arg),
            }
        }

        assert_eq!(command_parts.join("."), "apps.resources.deploy-markers");
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
                event: "command.run",
                command: "apps.list",
                success: true,
                duration_ms: 250,
                cli_version: CLIENT_VERSION,
                output_format: "json",
            })
            .send()
            .await
            .unwrap();
    }
}
