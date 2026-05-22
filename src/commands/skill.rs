use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::Serialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::output::Output;

const SKILL_NAME: &str = "appsignal";
const SKILL_VERSION: &str = env!("CARGO_PKG_VERSION");
const OPENCODE_SKILL_TEMPLATE: &str = include_str!("../../skills/opencode/SKILL.md");
const CODEX_SKILL_TEMPLATE: &str = include_str!("../../skills/codex/SKILL.md");
const CLAUDE_SKILL_TEMPLATE: &str = include_str!("../../skills/claude/SKILL.md");
const SHARED_SKILL_BODY_TEMPLATE: &str = include_str!("../../skills/shared/body.md");
const BODY_PLACEHOLDER: &str = "{{BODY}}";
const VERSION_MARKER_PREFIX: &str = "<!-- appsignal-cli skill version: ";
const VERSION_MARKER_SUFFIX: &str = " -->";

#[derive(Serialize)]
struct SkillInstallEntry {
    target: String,
    path: String,
}

#[derive(Serialize)]
struct SkillInstallResponse {
    skill_name: &'static str,
    bundled_version: &'static str,
    installed: Vec<SkillInstallEntry>,
}

#[derive(Serialize)]
struct SkillUpdateEntry {
    target: String,
    path: String,
    action: &'static str,
}

#[derive(Serialize)]
struct SkillUpdateResponse {
    skill_name: &'static str,
    bundled_version: &'static str,
    updated: Vec<SkillUpdateEntry>,
}

#[derive(Serialize)]
struct SkillStatusEntry {
    target: String,
    path: String,
    status: &'static str,
    installed: bool,
    installed_version: Option<String>,
    bundled_version: &'static str,
}

#[derive(Serialize)]
struct SkillStatusResponse {
    skill_name: &'static str,
    bundled_version: &'static str,
    targets: Vec<SkillStatusEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SkillFileStatus {
    Missing,
    UpToDate,
    UpdateAvailable,
    Unversioned,
}

impl SkillFileStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::UpToDate => "up_to_date",
            Self::UpdateAvailable => "update_available",
            Self::Unversioned => "unversioned",
        }
    }
}

struct SkillTargetPath {
    target: InstallTarget,
    skill_path: PathBuf,
}

struct ExistingSkill {
    status: SkillFileStatus,
    installed_version: Option<String>,
}

struct SkillUpdateResult {
    target: InstallTarget,
    path: PathBuf,
    action: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum InstallTarget {
    Opencode,
    Codex,
    Claude,
    All,
}

impl InstallTarget {
    fn label(&self) -> &'static str {
        match self {
            Self::Opencode => "OpenCode",
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::All => "all",
        }
    }
}

pub fn install(
    targets: &[InstallTarget],
    dir: Option<&str>,
    force: bool,
    format: Output,
) -> Result<()> {
    let targets = resolve_targets(targets, dir)?;
    let mut installed = Vec::new();

    for target in targets {
        let installed_path = install_skill(&target.target, &target.skill_path, force)?;
        installed.push((target.target, installed_path));
    }

    let response = SkillInstallResponse {
        skill_name: SKILL_NAME,
        bundled_version: SKILL_VERSION,
        installed: installed
            .iter()
            .map(|(target, path)| SkillInstallEntry {
                target: target.label().to_string(),
                path: path.display().to_string(),
            })
            .collect(),
    };

    crate::output::print_with(&response, format, |w| {
        for (target, path) in &installed {
            writeln!(
                w,
                "Installed {} skill v{} at {}",
                target.label(),
                SKILL_VERSION,
                path.display()
            )?;
        }
        writeln!(w, "Load it in your agent as `{}`.", SKILL_NAME)
    })
}

pub fn update(targets: &[InstallTarget], dir: Option<&str>, format: Output) -> Result<()> {
    let targets = resolve_targets(targets, dir)?;
    let mut updated = Vec::new();

    for target in targets {
        updated.push(update_skill(&target.target, &target.skill_path)?);
    }

    let response = SkillUpdateResponse {
        skill_name: SKILL_NAME,
        bundled_version: SKILL_VERSION,
        updated: updated
            .iter()
            .map(|entry| SkillUpdateEntry {
                target: entry.target.label().to_string(),
                path: entry.path.display().to_string(),
                action: entry.action,
            })
            .collect(),
    };

    crate::output::print_with(&response, format, |w| {
        for entry in &updated {
            writeln!(
                w,
                "{} {} skill to v{} at {}",
                capitalize(entry.action),
                entry.target.label(),
                SKILL_VERSION,
                entry.path.display()
            )?;
        }
        writeln!(w, "Load it in your agent as `{}`.", SKILL_NAME)
    })
}

pub fn status(targets: &[InstallTarget], dir: Option<&str>, format: Output) -> Result<()> {
    let targets = resolve_targets(targets, dir)?;
    let statuses: Vec<_> = targets
        .iter()
        .map(|target| {
            let existing = inspect_existing_skill(&target.skill_path)?;

            Ok(SkillStatusEntry {
                target: target.target.label().to_string(),
                path: target.skill_path.display().to_string(),
                status: existing.status.label(),
                installed: existing.status != SkillFileStatus::Missing,
                installed_version: existing.installed_version,
                bundled_version: SKILL_VERSION,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let response = SkillStatusResponse {
        skill_name: SKILL_NAME,
        bundled_version: SKILL_VERSION,
        targets: statuses,
    };

    crate::output::print_with(&response, format, |w| {
        for entry in &response.targets {
            let version_suffix = match entry.installed_version.as_deref() {
                Some(version) => format!(" (installed v{version})"),
                None if entry.installed => " (installed version unknown)".to_string(),
                None => String::new(),
            };

            writeln!(
                w,
                "{}: {}{} [{}]",
                entry.target, entry.path, version_suffix, entry.status
            )?;
        }

        writeln!(w, "Bundled skill version: {SKILL_VERSION}")
    })
}

fn expand_targets(targets: &[InstallTarget]) -> Vec<InstallTarget> {
    if targets.contains(&InstallTarget::All) {
        vec![
            InstallTarget::Opencode,
            InstallTarget::Codex,
            InstallTarget::Claude,
        ]
    } else {
        targets.to_vec()
    }
}

fn resolve_targets(targets: &[InstallTarget], dir: Option<&str>) -> Result<Vec<SkillTargetPath>> {
    let targets = expand_targets(targets);

    if dir.is_some() && targets.len() > 1 {
        anyhow::bail!(
            "`--dir` can only be used with a single target. Choose one target or omit `--dir`."
        );
    }

    targets
        .into_iter()
        .map(|target| {
            let skill_root = match dir {
                Some(path) => PathBuf::from(path),
                None => default_skill_root(&target)?,
            };

            Ok(SkillTargetPath {
                target,
                skill_path: skill_root.join(SKILL_NAME).join("SKILL.md"),
            })
        })
        .collect()
}

fn default_skill_root(target: &InstallTarget) -> Result<PathBuf> {
    let home_dir = dirs::home_dir().context("Could not determine home directory")?;
    let codex_home = env::var_os("CODEX_HOME").map(PathBuf::from);

    match target {
        InstallTarget::Opencode => Ok(opencode_skill_root_from(&home_dir)),
        InstallTarget::Codex => Ok(codex_skill_root_from(&home_dir, codex_home.as_deref())),
        InstallTarget::Claude => Ok(claude_skill_root_from(&home_dir)),
        InstallTarget::All => unreachable!("all target should be expanded before resolving paths"),
    }
}

fn opencode_skill_root_from(home_dir: &Path) -> PathBuf {
    home_dir.join(".agents").join("skills")
}

fn codex_skill_root_from(home_dir: &Path, codex_home: Option<&Path>) -> PathBuf {
    codex_home
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home_dir.join(".codex"))
        .join("skills")
}

fn claude_skill_root_from(home_dir: &Path) -> PathBuf {
    home_dir.join(".claude").join("skills")
}

fn install_skill(target: &InstallTarget, skill_path: &Path, force: bool) -> Result<PathBuf> {
    if skill_path.exists() && !force {
        anyhow::bail!(
            "Skill already exists at {}. Re-run with `--force` to overwrite it.",
            skill_path.display()
        );
    }

    write_skill(target, skill_path)?;

    Ok(skill_path.to_path_buf())
}

fn update_skill(target: &InstallTarget, skill_path: &Path) -> Result<SkillUpdateResult> {
    let action = if skill_path.exists() {
        "updated"
    } else {
        "installed"
    };
    write_skill(target, skill_path)?;

    Ok(SkillUpdateResult {
        target: target.clone(),
        path: skill_path.to_path_buf(),
        action,
    })
}

fn write_skill(target: &InstallTarget, skill_path: &Path) -> Result<()> {
    let skill_dir = skill_path
        .parent()
        .context("Skill path did not include a parent directory")?;

    fs::create_dir_all(skill_dir)
        .with_context(|| format!("Failed to create skill directory {}", skill_dir.display()))?;
    fs::write(skill_path, render_skill(target))
        .with_context(|| format!("Failed to write skill file {}", skill_path.display()))?;

    Ok(())
}

fn render_skill(target: &InstallTarget) -> String {
    let skill =
        match target {
            InstallTarget::Opencode => {
                OPENCODE_SKILL_TEMPLATE.replace(BODY_PLACEHOLDER, SHARED_SKILL_BODY_TEMPLATE)
            }
            InstallTarget::Codex => CODEX_SKILL_TEMPLATE
                .replace(BODY_PLACEHOLDER, &shared_skill_body_for_other_targets()),
            InstallTarget::Claude => CLAUDE_SKILL_TEMPLATE
                .replace(BODY_PLACEHOLDER, &shared_skill_body_for_other_targets()),
            InstallTarget::All => unreachable!("all target should be expanded before rendering"),
        };

    format!("{skill}\n\n{VERSION_MARKER_PREFIX}{SKILL_VERSION}{VERSION_MARKER_SUFFIX}\n")
}

fn shared_skill_body_for_other_targets() -> String {
    format!(
        "# AppSignal CLI Reference\n\n{}",
        SHARED_SKILL_BODY_TEMPLATE
    )
}

fn inspect_existing_skill(skill_path: &Path) -> Result<ExistingSkill> {
    if !skill_path.exists() {
        return Ok(ExistingSkill {
            status: SkillFileStatus::Missing,
            installed_version: None,
        });
    }

    let contents = fs::read_to_string(skill_path)
        .with_context(|| format!("Failed to read installed skill {}", skill_path.display()))?;
    let installed_version = extract_skill_version(&contents);

    let status = match installed_version.as_deref() {
        Some(version) if version == SKILL_VERSION => SkillFileStatus::UpToDate,
        Some(_) => SkillFileStatus::UpdateAvailable,
        None => SkillFileStatus::Unversioned,
    };

    Ok(ExistingSkill {
        status,
        installed_version,
    })
}

fn extract_skill_version(contents: &str) -> Option<String> {
    contents.lines().rev().find_map(|line| {
        line.trim()
            .strip_prefix(VERSION_MARKER_PREFIX)
            .and_then(|line| line.strip_suffix(VERSION_MARKER_SUFFIX))
            .map(str::to_string)
    })
}

fn capitalize(action: &str) -> String {
    let mut chars = action.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn opencode_skill_root_uses_agents_directory() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            opencode_skill_root_from(dir.path()),
            dir.path().join(".agents").join("skills")
        );
    }

    #[test]
    fn codex_skill_root_prefers_codex_home() {
        let dir = TempDir::new().unwrap();
        let codex_home = dir.path().join("custom-codex-home");

        assert_eq!(
            codex_skill_root_from(dir.path(), Some(&codex_home)),
            codex_home.join("skills")
        );
    }

    #[test]
    fn claude_skill_root_uses_claude_directory() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            claude_skill_root_from(dir.path()),
            dir.path().join(".claude").join("skills")
        );
    }

    #[test]
    fn render_opencode_skill_includes_opencode_frontmatter() {
        let rendered = render_skill(&InstallTarget::Opencode);

        assert!(rendered.starts_with("---\nname: appsignal\n"));
        assert!(rendered.contains("triggers:"));
        assert!(rendered.contains("invocable: true"));
        assert!(rendered.contains("# /appsignal - AppSignal CLI Reference"));
        assert!(rendered.contains("## Commands"));
        assert!(rendered.contains("appsignal-cli logs search"));
        assert!(rendered.contains(&format!(
            "{VERSION_MARKER_PREFIX}{SKILL_VERSION}{VERSION_MARKER_SUFFIX}"
        )));
    }

    #[test]
    fn render_codex_skill_includes_codex_metadata() {
        let rendered = render_skill(&InstallTarget::Codex);

        assert!(rendered.starts_with("---\nname: appsignal\ndescription:"));
        assert!(rendered.contains("metadata:\n  short-description: AppSignal CLI guide"));
        assert!(rendered.contains("# AppSignal CLI Reference"));
        assert!(rendered.contains("## Commands"));
        assert!(!rendered.contains("triggers:"));
    }

    #[test]
    fn render_claude_skill_uses_minimal_frontmatter() {
        let rendered = render_skill(&InstallTarget::Claude);

        assert!(rendered.starts_with("---\nname: appsignal\ndescription:"));
        assert!(rendered.contains("# AppSignal CLI Reference"));
        assert!(!rendered.contains("metadata:\n  short-description:"));
        assert!(!rendered.contains("triggers:"));
    }

    #[test]
    fn install_skill_writes_target_specific_file() {
        let dir = TempDir::new().unwrap();
        let skill_path = dir.path().join("appsignal").join("SKILL.md");

        let installed_path = install_skill(&InstallTarget::Codex, &skill_path, false).unwrap();
        let contents = fs::read_to_string(&installed_path).unwrap();

        assert_eq!(installed_path, skill_path);
        assert!(contents.contains("name: appsignal"));
        assert!(contents.contains("appsignal-cli incidents list"));
    }

    #[test]
    fn install_skill_requires_force_to_overwrite() {
        let dir = TempDir::new().unwrap();
        let skill_path = dir.path().join("appsignal").join("SKILL.md");

        let installed_path = install_skill(&InstallTarget::Opencode, &skill_path, false).unwrap();
        fs::write(&installed_path, "custom").unwrap();

        let error = install_skill(&InstallTarget::Opencode, &skill_path, false).unwrap_err();
        assert!(error.to_string().contains("Skill already exists"));

        install_skill(&InstallTarget::Opencode, &skill_path, true).unwrap();
        let contents = fs::read_to_string(&installed_path).unwrap();
        assert!(contents.contains("name: appsignal"));
    }

    #[test]
    fn update_skill_overwrites_existing_install() {
        let dir = TempDir::new().unwrap();
        let skill_path = dir.path().join("appsignal").join("SKILL.md");

        fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
        fs::write(&skill_path, "custom").unwrap();

        let result = update_skill(&InstallTarget::Claude, &skill_path).unwrap();
        let contents = fs::read_to_string(&skill_path).unwrap();

        assert_eq!(result.action, "updated");
        assert!(contents.contains("name: appsignal"));
        assert_eq!(
            extract_skill_version(&contents).as_deref(),
            Some(SKILL_VERSION)
        );
    }

    #[test]
    fn extract_skill_version_reads_embedded_version_marker() {
        let contents = format!(
            "---\nname: appsignal\n---\n\n{VERSION_MARKER_PREFIX}1.2.3{VERSION_MARKER_SUFFIX}\n"
        );

        assert_eq!(extract_skill_version(&contents).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn inspect_existing_skill_marks_unversioned_files() {
        let dir = TempDir::new().unwrap();
        let skill_path = dir.path().join("appsignal").join("SKILL.md");

        fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
        fs::write(&skill_path, "legacy skill").unwrap();

        let existing = inspect_existing_skill(&skill_path).unwrap();

        assert_eq!(existing.status, SkillFileStatus::Unversioned);
        assert_eq!(existing.installed_version, None);
    }

    #[test]
    fn inspect_existing_skill_marks_older_versions_as_update_available() {
        let dir = TempDir::new().unwrap();
        let skill_path = dir.path().join("appsignal").join("SKILL.md");

        fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
        fs::write(
            &skill_path,
            format!("skill\n{VERSION_MARKER_PREFIX}0.0.1{VERSION_MARKER_SUFFIX}\n"),
        )
        .unwrap();

        let existing = inspect_existing_skill(&skill_path).unwrap();

        assert_eq!(existing.status, SkillFileStatus::UpdateAvailable);
        assert_eq!(existing.installed_version.as_deref(), Some("0.0.1"));
    }

    #[test]
    fn install_rejects_custom_dir_for_multiple_targets() {
        let targets = vec![InstallTarget::Opencode, InstallTarget::Codex];
        let err = install(&targets, Some("/tmp/skills"), false, Output::Human).unwrap_err();

        assert!(err
            .to_string()
            .contains("`--dir` can only be used with a single target"));
    }

    #[test]
    fn expand_all_target_to_all_supported_targets() {
        let targets = expand_targets(&[InstallTarget::All]);

        assert_eq!(
            targets,
            vec![
                InstallTarget::Opencode,
                InstallTarget::Codex,
                InstallTarget::Claude,
            ]
        );
    }
}
