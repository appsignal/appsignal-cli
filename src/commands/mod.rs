pub mod apps;
pub mod auth;
pub mod incidents;

use crate::config::Config;
use anyhow::Result;

/// Resolve the organization slug: use explicit --org if given, fall back to config.
pub fn resolve_org(explicit: Option<&str>, config: &Config) -> Result<String> {
    if let Some(org) = explicit {
        return Ok(org.to_string());
    }
    config
        .org
        .as_deref()
        .filter(|o| !o.is_empty())
        .map(|o| o.to_string())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No organization configured. Run `appsignal-cli apps list --org <slug>` first, \
                 or set it with `appsignal-cli apps set-org --org <slug>`."
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_org_explicit() {
        let config = Config::default();
        let result = resolve_org(Some("explicit-org"), &config).unwrap();
        assert_eq!(result, "explicit-org");
    }

    #[test]
    fn test_resolve_org_explicit_overrides_config() {
        let config = Config {
            org: Some("config-org".to_string()),
            ..Config::default()
        };
        let result = resolve_org(Some("explicit-org"), &config).unwrap();
        assert_eq!(result, "explicit-org");
    }

    #[test]
    fn test_resolve_org_from_config() {
        let config = Config {
            org: Some("config-org".to_string()),
            ..Config::default()
        };
        let result = resolve_org(None, &config).unwrap();
        assert_eq!(result, "config-org");
    }

    #[test]
    fn test_resolve_org_empty_config() {
        let config = Config {
            org: Some("".to_string()),
            ..Config::default()
        };
        let err = resolve_org(None, &config).unwrap_err();
        assert!(err.to_string().contains("No organization configured"));
    }

    #[test]
    fn test_resolve_org_none() {
        let config = Config::default();
        let err = resolve_org(None, &config).unwrap_err();
        assert!(err.to_string().contains("No organization configured"));
    }
}
