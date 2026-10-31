//! `klaub skills install`: copy a packaged skill into an agent's configuration.
//!
//! An existing skill whose text was changed is left untouched unless `--force` is given.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context as _, Result, bail};

use crate::cli::{Harness, SkillName, SkillsCommand};
use crate::output;

const FLOW_SKILL: &str = include_str!("../../../../skills/klaub-flow/SKILL.md");
const FEEDBACK_SKILL: &str = include_str!("../../../../skills/klaub-feedback/SKILL.md");

pub(super) fn run(command: &SkillsCommand) -> Result<ExitCode> {
    match command {
        SkillsCommand::Install {
            skill,
            harness,
            force,
        } => install(*skill, *harness, *force),
    }
}

fn install(skill: SkillName, harness: Harness, force: bool) -> Result<ExitCode> {
    let destination = skills_dir(harness)?.join(skill.dir_name());
    install_at(&destination, skill.body(), force)?;
    Ok(ExitCode::SUCCESS)
}

/// Writes `SKILL.md` under `destination`, which is the skill's own directory.
///
/// Identical content is reported as already installed. Different content is refused, naming the
/// path, unless `force` is set.
pub(super) fn install_at(destination: &Path, body: &str, force: bool) -> Result<()> {
    let path = destination.join("SKILL.md");
    if let Some(existing) = read_if_present(&path)? {
        if existing == body {
            output::line(&format!("already installed: {}", path.display()));
            return Ok(());
        }
        if !force {
            bail!(
                "{} already exists and differs from the packaged skill; pass --force to replace it",
                path.display()
            );
        }
    }
    fs::create_dir_all(destination)
        .with_context(|| format!("cannot create {}", destination.display()))?;
    write_skill(&path, body)?;
    output::line(&format!("installed {}", path.display()));
    Ok(())
}

fn read_if_present(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
    }
}

fn write_skill(path: &Path, body: &str) -> Result<()> {
    let mut file =
        fs::File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
    let permissions = file_mode(0o644);
    file.set_permissions(permissions)
        .with_context(|| format!("cannot set permissions on {}", path.display()))?;
    file.write_all(body.as_bytes())
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

fn skills_dir(harness: Harness) -> Result<PathBuf> {
    let home = dirs_home().context("cannot find the home directory")?;
    Ok(match harness {
        Harness::Claude => home.join(".claude/skills"),
        Harness::Codex => std::env::var_os("CODEX_HOME")
            .map_or_else(|| home.join(".codex"), PathBuf::from)
            .join("skills"),
    })
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn file_mode(mode: u32) -> fs::Permissions {
    use std::os::unix::fs::PermissionsExt;
    fs::Permissions::from_mode(mode)
}

impl SkillName {
    fn dir_name(self) -> &'static str {
        match self {
            Self::Flow => "klaub-flow",
            Self::Feedback => "klaub-feedback",
        }
    }

    fn body(self) -> &'static str {
        match self {
            Self::Flow => FLOW_SKILL,
            Self::Feedback => FEEDBACK_SKILL,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_skill_is_installed() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("klaub-flow");
        install_at(&destination, "skill body\n", false).unwrap();
        let installed = fs::read_to_string(destination.join("SKILL.md")).unwrap();
        assert_eq!(installed, "skill body\n");
    }

    #[test]
    fn identical_content_is_left_as_already_installed() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("klaub-feedback");
        install_at(&destination, FEEDBACK_SKILL, false).unwrap();
        install_at(&destination, FEEDBACK_SKILL, false).unwrap();
        assert_eq!(
            fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            FEEDBACK_SKILL
        );
    }

    #[test]
    fn a_customised_skill_is_kept_unless_forced() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("klaub-flow");
        install_at(&destination, "my own notes\n", false).unwrap();
        let error = install_at(&destination, FLOW_SKILL, false).unwrap_err();
        let path = destination.join("SKILL.md");
        assert!(
            error.to_string().contains(&path.display().to_string()),
            "{error}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "my own notes\n");

        install_at(&destination, FLOW_SKILL, true).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), FLOW_SKILL);
    }
}
