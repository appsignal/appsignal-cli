use std::io::{self, IsTerminal, Write};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::api::dashboard_visuals::{DashboardDetail, VisualInput, VisualType};
use crate::commands::{authenticated_client, resolve_org};
use crate::config::Config;
use crate::error::CliError;
use crate::output::{self, Output, Render};

pub enum VisualCommand {
    Show {
        id: String,
    },
    Add {
        dashboard_id: String,
        kind: VisualType,
        file: String,
    },
    Update {
        dashboard_id: String,
        id: String,
        file: String,
    },
}

#[derive(Serialize)]
struct DashboardResponse {
    dashboard: DashboardDetail,
}

impl Render for DashboardResponse {
    fn render_human(&self, w: &mut dyn Write) -> io::Result<()> {
        super::render_dashboard_detail(w, &self.dashboard.dashboard)?;
        if self.dashboard.visuals.is_empty() {
            return writeln!(w, "\nNo charts found.");
        }
        for visual in &self.dashboard.visuals {
            let kind = match visual.kind() {
                VisualType::Timeseries => "timeseries",
                VisualType::Number => "number",
            };
            writeln!(
                w,
                "\nChart {} ({kind}): {}",
                visual.common().id,
                visual.common().title
            )?;
            writeln!(
                w,
                "{}",
                serde_json::to_string_pretty(visual).map_err(io::Error::other)?
            )?;
        }
        Ok(())
    }
}

fn read_input(file: &str) -> Result<VisualInput> {
    if file == "-" {
        let stdin = io::stdin();
        if stdin.is_terminal() {
            return Err(
                CliError::msg("Pipe a chart JSON object to stdin when using --file -.").into(),
            );
        }
        VisualInput::parse(stdin.lock())
    } else {
        let reader = std::fs::File::open(file)
            .with_context(|| CliError::msg(format!("Could not read chart JSON file {file}.")))?;
        VisualInput::parse(reader)
    }
}

pub async fn run(
    app_id: Option<&str>,
    app_name: Option<&str>,
    environment: Option<&str>,
    org: Option<&str>,
    command: VisualCommand,
    format: Output,
) -> Result<()> {
    // Read local input before authentication or network requests.
    let input = match &command {
        VisualCommand::Show { .. } => None,
        VisualCommand::Add { file, kind, .. } => {
            let input = read_input(file)?;
            input.validate(*kind, true)?;
            Some(input)
        }
        VisualCommand::Update { file, .. } => Some(read_input(file)?),
    };
    let mut config = Config::load()?;
    let org = resolve_org(org, &config)?;
    let client = authenticated_client(&mut config).await?;
    let app_id = client
        .resolve_app_id(&org, app_id, app_name, environment)
        .await?;
    let dashboard = match command {
        VisualCommand::Show { id } => client.get_dashboard_detail(&app_id, &id).await?,
        VisualCommand::Add {
            dashboard_id, kind, ..
        } => {
            let dashboard = client
                .create_dashboard_visual(
                    &app_id,
                    &dashboard_id,
                    kind,
                    input.expect("add input parsed"),
                )
                .await?;
            crate::status!("Chart added to dashboard {}.", dashboard_id);
            dashboard
        }
        VisualCommand::Update {
            dashboard_id, id, ..
        } => {
            let dashboard = client
                .update_dashboard_visual(
                    &app_id,
                    &dashboard_id,
                    &id,
                    input.expect("update input parsed"),
                )
                .await?;
            crate::status!("Chart {} updated.", id);
            dashboard
        }
    };
    output::print(&DashboardResponse { dashboard }, format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_renders_both_chart_types_and_configuration() {
        let response = DashboardResponse {
            dashboard: serde_json::from_value(crate::api::dashboard_visuals::tests::dashboard())
                .unwrap(),
        };
        let mut output = Vec::new();
        response.render_human(&mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        for expected in [
            "line-1 (timeseries)",
            "number-1 (number)",
            "MEAN",
            "namespace",
            "SUM",
        ] {
            assert!(text.contains(expected), "{text}");
        }
        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["dashboard"]["visuals"][0]["id"], "line-1");
        assert_eq!(
            json["dashboard"]["visuals"][1]["metric"]["aggregate"],
            "SUM"
        );
    }

    #[test]
    fn unreadable_file_has_user_facing_context() {
        let dir = tempfile::tempdir().unwrap();
        let error = read_input(dir.path().join("missing.json").to_str().unwrap()).unwrap_err();
        assert!(error.downcast_ref::<CliError>().is_some());
        assert!(error.to_string().contains("Could not read chart JSON file"));
    }

    #[test]
    fn file_input_is_parsed() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("chart.json");
        std::fs::write(&file, r#"{"title":"Requests"}"#).unwrap();
        read_input(file.to_str().unwrap())
            .unwrap()
            .validate(VisualType::Timeseries, true)
            .unwrap();
    }

    #[test]
    fn detail_renders_empty_dashboard_and_json_envelope() {
        let response = DashboardResponse {
            dashboard: serde_json::from_value(serde_json::json!({
                "id": "dash-1", "title": "Overview", "visuals": []
            }))
            .unwrap(),
        };
        let mut output = Vec::new();
        response.render_human(&mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("dash-1"));
        assert!(text.contains("No charts found."));
        assert_eq!(
            serde_json::to_value(response).unwrap()["dashboard"]["visuals"],
            serde_json::json!([])
        );
    }
}
