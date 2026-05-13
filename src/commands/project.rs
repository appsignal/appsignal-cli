use anyhow::Result;
use serde::Serialize;

use crate::config::Config;
use crate::output::Output;

#[derive(Serialize)]
struct ProjectInitResponse {
    path: String,
    message: String,
}

pub struct InitOptions {
    pub endpoint: Option<String>,
    pub oauth_client_id: Option<String>,
    pub org: Option<String>,
}

/// Initialize a project-local `.appsignal.toml`.
pub fn init(options: InitOptions, format: Output) -> Result<()> {
    let mut config = Config::load_local_only()?;
    let path = config
        .active_path()
        .expect("local config path should be set by load_local_only")
        .to_path_buf();

    if let Some(endpoint) = options.endpoint {
        config.endpoint = Some(endpoint);
    }

    if let Some(oauth_client_id) = options.oauth_client_id {
        config.oauth_client_id = Some(oauth_client_id);
    }

    if let Some(org) = options.org {
        config.org = Some(org);
    }

    config.save()?;

    let path_display = path.display().to_string();
    crate::output::print_with(
        ProjectInitResponse {
            path: path_display.clone(),
            message: format!("Created project config at {}", path_display),
        },
        format,
        |w| writeln!(w, "Created project config at {}", path_display),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_options_can_be_empty() {
        let options = InitOptions {
            endpoint: None,
            oauth_client_id: None,
            org: None,
        };

        assert!(options.endpoint.is_none());
        assert!(options.oauth_client_id.is_none());
        assert!(options.org.is_none());
    }
}
