use anyhow::Result;

use crate::config::Config;

pub struct InitOptions {
    pub endpoint: Option<String>,
    pub oauth_client_id: Option<String>,
    pub org: Option<String>,
}

/// Initialize a project-local `.appsignal.toml`.
pub fn init(options: InitOptions) -> Result<()> {
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

    println!("Created project config at {}", path.display());
    Ok(())
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
