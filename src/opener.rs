use crate::config::Config;
use crate::models::{Project, ProjectSource};
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub struct ProjectOpener;

impl ProjectOpener {
    pub fn new() -> Self {
        Self
    }

    pub fn open_project(&self, project: &Project, config: &Config) -> Result<()> {
        if project.source == ProjectSource::GitHub && !project.path.exists() {
            self.clone_github_project(project)?;
        }

        self.open_project_path(&project.path, config)
    }

    fn clone_github_project(&self, project: &Project) -> Result<()> {
        let github_url = project
            .github_url
            .as_ref()
            .context("GitHub project missing URL")?;

        if let Some(parent) = project.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        println!("Cloning GitHub repository: {}", github_url);

        let output = Command::new("git")
            .args(["clone", github_url, &project.path.to_string_lossy()])
            .output()
            .context("Failed to execute git clone command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git clone failed: {}", stderr);
        }

        println!(
            "Successfully cloned {} to {}",
            project.name,
            project.path.display()
        );
        Ok(())
    }

    pub fn open_project_path<P: AsRef<Path>>(&self, path: P, config: &Config) -> Result<()> {
        let path = path.as_ref();

        if config.editor_command.trim().is_empty() {
            anyhow::bail!("Editor command is empty");
        }

        if !path.exists() {
            anyhow::bail!("Project path does not exist: {}", path.display());
        }

        let parts: Vec<&str> = config.editor_command.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("Editor command is empty");
        }

        let editor = parts[0];
        let args = &parts[1..];

        let mut cmd = Command::new(editor);
        cmd.args(args);
        cmd.arg(path.as_os_str());

        if is_background_editor(editor) {
            cmd.spawn()
                .with_context(|| format!("Failed to launch editor: {}", config.editor_command))?;
        } else {
            let output = cmd.output().with_context(|| {
                format!(
                    "Failed to execute editor command: {}",
                    config.editor_command
                )
            })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Editor command failed: {}", stderr);
            }
        }

        Ok(())
    }
}

impl Default for ProjectOpener {
    fn default() -> Self {
        Self::new()
    }
}

fn is_background_editor(editor: &str) -> bool {
    matches!(editor, "cursor" | "code" | "subl" | "atom")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_project(path: &Path) -> Project {
        Project::new_local("test-project".to_string(), path)
    }

    fn create_github_project(name: &str, path: &Path, url: &str) -> Project {
        Project::new_github(name.to_string(), path, url.to_string())
    }

    #[test]
    fn test_opener_creation() {
        let _opener = ProjectOpener::new();
    }

    #[test]
    fn test_open_nonexistent_project() {
        let opener = ProjectOpener::new();
        let mut config = Config::default();
        config.set_editor("echo".to_string());
        let nonexistent_path = PathBuf::from("/nonexistent/path/that/does/not/exist");
        let project = Project::new_local("nonexistent".to_string(), &nonexistent_path);

        let result = opener.open_project(&project, &config);
        assert!(result.is_err(), "Should fail to open nonexistent project");
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_open_with_empty_editor_command() {
        let opener = ProjectOpener::new();
        let mut config = Config::default();
        config.set_editor("".to_string());
        let temp_dir = TempDir::new().unwrap();
        let project = create_test_project(temp_dir.path());

        let result = opener.open_project(&project, &config);
        assert!(result.is_err(), "Should fail with empty editor command");
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_open_with_whitespace_only_editor_command() {
        let opener = ProjectOpener::new();
        let mut config = Config::default();
        config.set_editor("   ".to_string());
        let temp_dir = TempDir::new().unwrap();
        let project = create_test_project(temp_dir.path());

        let result = opener.open_project(&project, &config);
        assert!(
            result.is_err(),
            "Should fail with whitespace-only editor command"
        );
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_is_background_editor() {
        assert!(is_background_editor("cursor"));
        assert!(is_background_editor("code"));
        assert!(is_background_editor("subl"));
        assert!(is_background_editor("atom"));

        assert!(!is_background_editor("vim"));
        assert!(!is_background_editor("nano"));
        assert!(!is_background_editor("emacs"));
    }

    #[test]
    fn test_github_project_missing_url() {
        let opener = ProjectOpener::new();
        let temp_dir = TempDir::new().unwrap();
        let nonexistent_path = temp_dir.path().join("nonexistent");

        let mut project = Project::new_local("test".to_string(), &nonexistent_path);
        project.source = ProjectSource::GitHub;

        let result = opener.clone_github_project(&project);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing URL"));
    }

    #[test]
    fn test_existing_github_project() {
        let opener = ProjectOpener::new();
        let mut config = Config::default();
        config.set_editor("echo".to_string());

        let temp_dir = TempDir::new().unwrap();
        let project = create_github_project(
            "existing-repo",
            temp_dir.path(),
            "https://github.com/user/existing-repo",
        );

        let result = opener.open_project(&project, &config);

        let _ = result;
    }
}
