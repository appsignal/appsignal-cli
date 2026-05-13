use anyhow::{Context, Result};
use clap::ValueEnum;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const SKILL_NAME: &str = "appsignal";
const OPENCODE_SKILL_TEMPLATE: &str = include_str!("../../skills/opencode/SKILL.md");
const CODEX_SKILL_TEMPLATE: &str = include_str!("../../skills/codex/SKILL.md");
const CLAUDE_SKILL_TEMPLATE: &str = include_str!("../../skills/claude/SKILL.md");
const SHARED_SKILL_BODY_TEMPLATE: &str = include_str!("../../skills/shared/body.md");
const BODY_PLACEHOLDER: &str = "{{BODY}}";

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

pub fn install(targets: &[InstallTarget], dir: Option<&str>, force: bool) -> Result<()> {
    let targets = expand_targets(targets);

    if dir.is_some() && targets.len() > 1 {
        anyhow::bail!(
            "`--dir` can only be used with a single target. Choose one target or omit `--dir`."
        );
    }

    let mut installed = Vec::new();

    for target in targets {
        let skill_root = match dir {
            Some(path) => PathBuf::from(path),
            None => default_skill_root(&target)?,
        };

        let installed_path = install_skill(&target, &skill_root, force)?;
        installed.push((target, installed_path));
    }

    for (target, path) in &installed {
        println!("Installed {} skill at {}", target.label(), path.display());
    }
    println!("Load it in your agent as `{}`.", SKILL_NAME);

    Ok(())
}

fn expand_targets(targets: &[InstallTarget]) -> Vec<InstallTarget> {
    if targets.iter().any(|target| *target == InstallTarget::All) {
        vec![
            InstallTarget::Opencode,
            InstallTarget::Codex,
            InstallTarget::Claude,
        ]
    } else {
        targets.to_vec()
    }
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

fn install_skill(target: &InstallTarget, skill_root: &Path, force: bool) -> Result<PathBuf> {
    let skill_dir = skill_root.join(SKILL_NAME);
    let skill_path = skill_dir.join("SKILL.md");

    if skill_path.exists() && !force {
        anyhow::bail!(
            "Skill already exists at {}. Re-run with `--force` to overwrite it.",
            skill_path.display()
        );
    }

    fs::create_dir_all(&skill_dir)
        .with_context(|| format!("Failed to create skill directory {}", skill_dir.display()))?;
    fs::write(&skill_path, render_skill(target))
        .with_context(|| format!("Failed to write skill file {}", skill_path.display()))?;

    Ok(skill_path)
}

fn render_skill(target: &InstallTarget) -> String {
    match target {
        InstallTarget::Opencode => {
            OPENCODE_SKILL_TEMPLATE.replace(BODY_PLACEHOLDER, SHARED_SKILL_BODY_TEMPLATE)
        }
        InstallTarget::Codex => {
            CODEX_SKILL_TEMPLATE.replace(BODY_PLACEHOLDER, &shared_skill_body_for_other_targets())
        }
        InstallTarget::Claude => {
            CLAUDE_SKILL_TEMPLATE.replace(BODY_PLACEHOLDER, &shared_skill_body_for_other_targets())
        }
        InstallTarget::All => unreachable!("all target should be expanded before rendering"),
    }
}

fn shared_skill_body_for_other_targets() -> String {
    format!(
        "# AppSignal CLI Reference\n\n{}",
        SHARED_SKILL_BODY_TEMPLATE
    )
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

        let installed_path = install_skill(&InstallTarget::Codex, dir.path(), false).unwrap();
        let contents = fs::read_to_string(&installed_path).unwrap();

        assert_eq!(
            installed_path,
            dir.path().join("appsignal").join("SKILL.md")
        );
        assert!(contents.contains("name: appsignal"));
        assert!(contents.contains("appsignal-cli incidents list"));
    }

    #[test]
    fn install_skill_requires_force_to_overwrite() {
        let dir = TempDir::new().unwrap();

        let installed_path = install_skill(&InstallTarget::Opencode, dir.path(), false).unwrap();
        fs::write(&installed_path, "custom").unwrap();

        let error = install_skill(&InstallTarget::Opencode, dir.path(), false).unwrap_err();
        assert!(error.to_string().contains("Skill already exists"));

        install_skill(&InstallTarget::Opencode, dir.path(), true).unwrap();
        let contents = fs::read_to_string(&installed_path).unwrap();
        assert!(contents.contains("name: appsignal"));
    }

    #[test]
    fn install_rejects_custom_dir_for_multiple_targets() {
        let targets = vec![InstallTarget::Opencode, InstallTarget::Codex];
        let err = install(&targets, Some("/tmp/skills"), false).unwrap_err();

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
