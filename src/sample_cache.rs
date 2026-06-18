//! A small on-disk cache of fetched transaction samples.
//!
//! `samples show`/`samples list` write each sample they fetch here (best-effort,
//! never fatal), so an investigator can re-inspect or search recently-seen
//! samples offline with `samples cache list` / `samples cache search` without
//! re-hitting the API. Each sample is one JSON file under the platform cache
//! directory (`dirs::cache_dir()/appsignal/samples/`).
//!
//! Caching can be turned off per-invocation with `--no-cache`, or globally with
//! the `APPSIGNAL_NO_CACHE` environment variable. Samples can contain request
//! parameters and session data, so the cache may hold sensitive values; clear it
//! with `samples cache clear`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::api::Sample;
use crate::error::CliError;

const ENV_DISABLE: &str = "APPSIGNAL_NO_CACHE";

/// One cached sample plus the context needed to find and re-render it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSample {
    pub app_id: String,
    pub incident_number: i64,
    /// `"performance"` or `"error"`.
    pub sample_type: String,
    /// When this entry was written (RFC 3339).
    pub cached_at: String,
    pub sample: Sample,
}

impl CachedSample {
    pub fn action(&self) -> Option<&str> {
        self.sample.action.as_deref()
    }

    pub fn user(&self) -> Option<String> {
        crate::sample_analysis::sample_user(&self.sample)
    }

    /// Lowercased searchable text: the whole entry serialized to JSON, which
    /// covers action, namespace, user, query bodies, params, and exceptions.
    fn haystack(&self) -> String {
        serde_json::to_string(self)
            .unwrap_or_default()
            .to_lowercase()
    }

    fn matches(&self, needle_lower: &str) -> bool {
        self.haystack().contains(needle_lower)
    }
}

/// Whether caching is disabled via the `APPSIGNAL_NO_CACHE` environment variable.
pub fn disabled_by_env() -> bool {
    disabled_from_env(std::env::var(ENV_DISABLE).ok().as_deref())
}

fn disabled_from_env(value: Option<&str>) -> bool {
    matches!(
        value.map(|raw| raw.trim().to_ascii_lowercase()),
        Some(value) if matches!(value.as_str(), "1" | "true" | "on" | "yes")
    )
}

/// The default cache directory, or `None` if the platform cache dir is unknown.
pub fn default_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("appsignal").join("samples"))
}

/// Resolve the default cache directory, erroring if it cannot be determined.
fn require_dir() -> Result<PathBuf> {
    default_dir().context(CliError::msg("Could not determine the cache directory."))
}

/// Best-effort store using the default directory and `now` as the timestamp.
/// Never returns an error: caching must never break the command that triggered it.
pub fn store_best_effort(
    app_id: &str,
    incident_number: i64,
    sample_type: &str,
    sample: &Sample,
    now_rfc3339: &str,
) {
    if let Some(dir) = default_dir() {
        let _ = store(
            dir.as_path(),
            app_id,
            incident_number,
            sample_type,
            sample,
            now_rfc3339,
        );
    }
}

/// Write one sample to `dir`, returning the file path written.
pub fn store(
    dir: &Path,
    app_id: &str,
    incident_number: i64,
    sample_type: &str,
    sample: &Sample,
    now_rfc3339: &str,
) -> Result<PathBuf> {
    fs::create_dir_all(dir)?;
    // Samples can hold sensitive request data, so keep the cache owner-only.
    restrict_to_owner(dir, 0o700);
    let entry = CachedSample {
        app_id: app_id.to_string(),
        incident_number,
        sample_type: sample_type.to_string(),
        cached_at: now_rfc3339.to_string(),
        sample: sample.clone(),
    };
    let path = dir.join(file_name(app_id, &sample.id));
    let json = serde_json::to_string_pretty(&entry)?;
    fs::write(&path, json)?;
    restrict_to_owner(&path, 0o600);
    Ok(path)
}

/// Best-effort tightening of cache file/dir permissions to the owner only.
/// No-op on non-Unix platforms; failures are ignored (caching is best-effort).
#[cfg(unix)]
fn restrict_to_owner(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path, _mode: u32) {}

/// All cached samples in `dir`, newest first, optionally filtered by app id.
pub fn load_all(dir: &Path, app_id: Option<&str>) -> Result<Vec<CachedSample>> {
    let mut entries: Vec<CachedSample> = read_entries(dir)?
        .into_iter()
        .filter(|entry| app_id.is_none_or(|wanted| entry.app_id == wanted))
        .collect();
    // RFC 3339 timestamps sort lexicographically; newest first.
    entries.sort_by(|a, b| b.cached_at.cmp(&a.cached_at));
    Ok(entries)
}

/// The most recently cached sample with the given sample id, if present.
/// Optionally scoped to one app id.
pub fn find_by_id(
    dir: &Path,
    sample_id: &str,
    app_id: Option<&str>,
) -> Result<Option<CachedSample>> {
    Ok(load_all(dir, app_id)?
        .into_iter()
        .find(|entry| entry.sample.id == sample_id))
}

/// Cached samples whose serialized contents contain `query` (case-insensitive).
pub fn search(dir: &Path, query: &str, app_id: Option<&str>) -> Result<Vec<CachedSample>> {
    let needle = query.to_lowercase();
    Ok(load_all(dir, app_id)?
        .into_iter()
        .filter(|entry| entry.matches(&needle))
        .collect())
}

/// Remove every cached sample file in `dir`; returns the number removed.
pub fn clear(dir: &Path) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut removed = 0;
    for path in cache_files(dir)? {
        if fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

/// The resolved default cache directory as a string, for display.
pub fn dir_display() -> Result<String> {
    Ok(require_dir()?.to_string_lossy().into_owned())
}

fn read_entries(dir: &Path) -> Result<Vec<CachedSample>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for path in cache_files(dir)? {
        // Skip unreadable or stale-format files rather than failing the whole list.
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(entry) = serde_json::from_str::<CachedSample>(&contents) {
                entries.push(entry);
            }
        }
    }
    Ok(entries)
}

fn cache_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            files.push(path);
        }
    }
    Ok(files)
}

/// Build a filesystem-safe file name from the app id and sample id.
fn file_name(app_id: &str, sample_id: &str) -> String {
    format!("{}-{}.json", sanitize(app_id), sanitize(sample_id))
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample(id: &str, action: &str) -> Sample {
        serde_json::from_value(json!({ "id": id, "action": action, "namespace": "web" })).unwrap()
    }

    #[test]
    fn disabled_from_env_honors_truthy_values() {
        for value in ["1", "true", "on", "yes", "TRUE"] {
            assert!(disabled_from_env(Some(value)));
        }
        assert!(!disabled_from_env(None));
        assert!(!disabled_from_env(Some("0")));
        assert!(!disabled_from_env(Some("")));
    }

    #[test]
    fn sanitize_replaces_unsafe_characters() {
        assert_eq!(sanitize("ab/cd:ef 12"), "ab_cd_ef_12");
        assert_eq!(sanitize("0123-abcd_EF"), "0123-abcd_EF");
    }

    #[test]
    fn store_then_load_roundtrips_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        store(
            dir.path(),
            "app1",
            1,
            "performance",
            &sample("s-old", "GET /a"),
            "2026-05-19T10:00:00Z",
        )
        .unwrap();
        store(
            dir.path(),
            "app1",
            2,
            "performance",
            &sample("s-new", "GET /b"),
            "2026-05-19T12:00:00Z",
        )
        .unwrap();

        let all = load_all(dir.path(), None).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].sample.id, "s-new");
        assert_eq!(all[1].sample.id, "s-old");
    }

    #[cfg(unix)]
    #[test]
    fn store_restricts_permissions_to_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("samples");
        let path = store(
            &cache,
            "app1",
            1,
            "performance",
            &sample("s-1", "GET /a"),
            "2026-05-19T10:00:00Z",
        )
        .unwrap();
        let file_mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let dir_mode = fs::metadata(&cache).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600, "cache file must be owner-only");
        assert_eq!(dir_mode, 0o700, "cache dir must be owner-only");
    }

    #[test]
    fn load_all_filters_by_app_id() {
        let dir = tempfile::tempdir().unwrap();
        store(
            dir.path(),
            "app1",
            1,
            "error",
            &sample("s1", "A"),
            "2026-05-19T10:00:00Z",
        )
        .unwrap();
        store(
            dir.path(),
            "app2",
            2,
            "error",
            &sample("s2", "B"),
            "2026-05-19T11:00:00Z",
        )
        .unwrap();

        let only = load_all(dir.path(), Some("app2")).unwrap();
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].app_id, "app2");
    }

    #[test]
    fn search_matches_serialized_contents_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        store(
            dir.path(),
            "app1",
            1,
            "performance",
            &sample("s1", "GET /Reports"),
            "2026-05-19T10:00:00Z",
        )
        .unwrap();
        store(
            dir.path(),
            "app1",
            2,
            "performance",
            &sample("s2", "GET /home"),
            "2026-05-19T11:00:00Z",
        )
        .unwrap();

        let hits = search(dir.path(), "reports", None).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].sample.id, "s1");
    }

    #[test]
    fn find_by_id_returns_matching_sample() {
        let dir = tempfile::tempdir().unwrap();
        store(
            dir.path(),
            "app1",
            1,
            "error",
            &sample("s1", "A"),
            "2026-05-19T10:00:00Z",
        )
        .unwrap();
        store(
            dir.path(),
            "app1",
            2,
            "error",
            &sample("s2", "B"),
            "2026-05-19T11:00:00Z",
        )
        .unwrap();

        let found = find_by_id(dir.path(), "s2", None).unwrap().unwrap();
        assert_eq!(found.sample.id, "s2");
        assert_eq!(found.incident_number, 2);
        assert!(find_by_id(dir.path(), "missing", None).unwrap().is_none());
    }

    #[test]
    fn clear_removes_all_entries() {
        let dir = tempfile::tempdir().unwrap();
        store(
            dir.path(),
            "app1",
            1,
            "error",
            &sample("s1", "A"),
            "2026-05-19T10:00:00Z",
        )
        .unwrap();
        store(
            dir.path(),
            "app1",
            2,
            "error",
            &sample("s2", "B"),
            "2026-05-19T11:00:00Z",
        )
        .unwrap();

        assert_eq!(clear(dir.path()).unwrap(), 2);
        assert!(load_all(dir.path(), None).unwrap().is_empty());
    }

    #[test]
    fn load_all_on_missing_dir_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        assert!(load_all(&missing, None).unwrap().is_empty());
        assert_eq!(clear(&missing).unwrap(), 0);
    }
}
