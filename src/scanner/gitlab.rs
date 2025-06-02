use crate::config::Config;
use crate::models::{Project, ProjectList};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct GitLabScanner;

impl GitLabScanner {
    
            return false;
        }

        
        is_glab_accessible()
    }

    ame: &str) -> PathBuf {
        
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("gitlab")
            .join(username)
            .join(repo_name)
    }

    e, username: &str) -> Result<Project> {
        let name = repo_json["name"]
            .as_str()
            .context("Repository name not found")?
            .to_string();

        let web_url = repo_json["web_url"]
            .as_str()
            .context("Repository web_url not found")?
            .to_string();

        let clone_path = Self::get_clone_path(username, &name);

        
        let last_modified = repo_json["last_activity_at"]
            .as_str()
            .and_then(|s| parse_gitlab_timestamp(s));

        Ok(Project::new_gitlab(name, clone_path, web_url).with_last_modified(
            last_modified.unwrap_or_else(Utc::now),
        ))
    }
}

impl crate::scanner::ProjectScanner for GitLabScanner {
    fn scanner_name(&self) -> &'static str {
        "gitlab"
    }

    fn scan(&self, config: &Config) -> Result<ProjectList> {
        
        let username = match &config.gitlab_username {
            Some(u) => u,
            None => return Ok(ProjectList::new()),
        };

        
        if !Self::can_connect() {
            return Ok(ProjectList::new());
        }

        
        let output = Command::new("glab")
            .args(["repo", "list", "--mine", "-F", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute glab command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("glab command failed: {}", stderr));
        }

        let stdout = String::from_utf8(output.stdout)
            .context("Failed to parse glab output as UTF-8")?;

        if stdout.trim().is_empty() {
            return Ok(ProjectList::new());
        }

        let repos: Vec<Value> = serde_json::from_str(&stdout)
            .context("Failed to parse glab JSON output")?;

        let mut projects = Vec::new();
        for repo in repos {
            
            if repo["archived"].as_bool().unwrap_or(false) {
                continue;
            }

            match Self::repository_to_project(&repo, username) {
                Ok(project) => projects.push(project),
                Err(e) => {
                    eprintln!("Warning: Failed to parse GitLab repository: {}", e);
                }
            }
        }

        Ok(ProjectList::from_projects(projects))
    }
}

 {
    which::which("glab").is_ok()
}

:new("timeout")
        .args(["10", "glab", "repo", "list", "--mine", "-F", "json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(result) => result.status.success(),
        Err(_) => {
            
            let output = Command::new("glab")
                .args(["repo", "list", "--mine", "-F", "json"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();

            match output {
                Ok(result) => result.status.success(),
                Err(_) => false,
            }
        }
    }
}

tamp_str: &str) -> Option<DateTime<Utc>> {
    
    DateTime::parse_from_rfc3339(timestamp_str)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::scanner::ProjectScanner;
    use chrono::{Datelike, Timelike};

    fn create_test_config() -> Config {
        Config {
            editor_command: "code".to_string(),
            project_dirs: vec![],
            github_username: None,
            gitlab_username: Some("testuser".to_string()),
            cache_ttl_seconds: 1800,
        }
    }

    #[test]
    fn test_gitlab_scanner_name() {
        let scanner = GitLabScanner;
        assert_eq!(scanner.scanner_name(), "gitlab");
    }

    #[test]
    fn test_get_clone_path() {
        let path = GitLabScanner::get_clone_path("testuser", "my-project");
        let path_str = path.to_string_lossy();
        
        assert!(path_str.contains("gitlab"));
        assert!(path_str.contains("testuser"));
        assert!(path_str.contains("my-project"));
    }

    #[test]
    fn test_parse_gitlab_timestamp_valid() {
        let timestamp = "2024-01-15T10:30:00.000Z";
        let parsed = parse_gitlab_timestamp(timestamp);
        
        assert!(parsed.is_some());
        let dt = parsed.unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 15);
        assert_eq!(dt.hour(), 10);
        assert_eq!(dt.minute(), 30);
    }

    #[test]
    fn test_parse_gitlab_timestamp_invalid() {
        let timestamp = "invalid-timestamp";
        let parsed = parse_gitlab_timestamp(timestamp);
        assert!(parsed.is_none());
    }

    #[test]
    fn test_parse_gitlab_timestamp_none() {
        let parsed = parse_gitlab_timestamp("");
        assert!(parsed.is_none());
    }

    #[test]
    fn test_repository_to_project_normal_repo() {
        let repo_json = serde_json::json!({
            "name": "test-project",
            "web_url": "https://gitlab.example.com/testuser/test-project",
            "last_activity_at": "2024-01-15T10:30:00.000Z",
            "archived": false
        });
        
        let project = GitLabScanner::repository_to_project(&repo_json, "testuser").unwrap();
        
        assert_eq!(project.name, "test-project");
        assert_eq!(project.source, crate::models::ProjectSource::GitLab);
        assert_eq!(project.gitlab_url, Some("https://gitlab.example.com/testuser/test-project".to_string()));
        assert!(project.github_url.is_none());
        assert!(project.last_modified.is_some());
    }

    #[test]
    fn test_repository_to_project_no_timestamp() {
        let repo_json = serde_json::json!({
            "name": "test-project",
            "web_url": "https://gitlab.example.com/testuser/test-project",
            "archived": false
        });
        
        let project = GitLabScanner::repository_to_project(&repo_json, "testuser").unwrap();
        
        assert_eq!(project.name, "test-project");
        assert!(project.last_modified.is_some()); 
    }

    #[test]
    fn test_is_glab_installed_function() {
        
        
        let _result = is_glab_installed();
    }

    #[test]
    fn test_scan_no_username() {
        let config = Config {
            editor_command: "code".to_string(),
            project_dirs: vec![],
            github_username: None,
            gitlab_username: None,
            cache_ttl_seconds: 1800,
        };
        
        let scanner = GitLabScanner;
        let result = scanner.scan(&config).unwrap();
        assert!(result.is_empty());
    }
} 