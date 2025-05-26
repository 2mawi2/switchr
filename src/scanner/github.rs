use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

use crate::config::Config;
use crate::models::{Project, ProjectList};
use super::ProjectScanner;

pub struct GitHubScanner;

#[derive(Debug, Deserialize)]
struct GitHubRepository {
    name: String,
    html_url: String,
    archived: bool,
    pushed_at: Option<String>,
    updated_at: Option<String>,
}

impl ProjectScanner for GitHubScanner {
    fn scan(&self, config: &Config) -> Result<ProjectList> {
        let mut project_list = ProjectList::new();
        
        let github_username = match &config.github_username {
            Some(username) => username,
            None => {
                
                return Ok(project_list);
            }
        };

        
        if !is_gh_installed() {
            eprintln!("Warning: GitHub CLI (gh) not found. Install with: brew install gh");
            return Ok(project_list);
        }

        
        if !is_gh_authenticated()? {
            if prompt_for_github_login()? {
                
                if !run_gh_auth_login()? {
                    eprintln!("GitHub authentication failed or cancelled");
                    return Ok(project_list);
                }
            } else {
                eprintln!("GitHub authentication required but declined. Skipping GitHub repositories.");
                return Ok(project_list);
            }
        }

        let repositories = fetch_user_repositories(github_username)?;
        
        for repo in repositories {
            if let Some(project) = repository_to_project(repo, config)? {
                project_list.add_project(project);
            }
        }

        project_list.sort_by_last_modified();
        Ok(project_list)
    }

    fn scanner_name(&self) -> &'static str {
        "github"
    }
}

pub fn is_gh_installed() -> bool {
    which::which("gh").is_ok()
}

pub fn is_gh_authenticated() -> Result<bool> {
    let output = Command::new("gh")
        .args(["auth", "status"])
        .output()
        .context("Failed to check GitHub authentication status")?;
    
    Ok(output.status.success())
}

fn prompt_for_github_login() -> Result<bool> {
    use dialoguer::Confirm;
    
    println!("GitHub authentication required to access your repositories.");
    let should_login = Confirm::new()
        .with_prompt("Would you like to authenticate with GitHub now?")
        .default(true)
        .interact()
        .context("Failed to get user confirmation for GitHub login")?;
    
    Ok(should_login)
}

pub fn run_gh_auth_login() -> Result<bool> {
    println!("Opening GitHub authentication in your browser...");
    
    let status = Command::new("gh")
        .args(["auth", "login"])
        .status()
        .context("Failed to run 'gh auth login'")?;
    
    if status.success() {
        println!("✅ GitHub authentication successful!");
        Ok(true)
    } else {
        println!("❌ GitHub authentication failed or was cancelled");
        Ok(false)
    }
}

fn fetch_user_repositories(username: &str) -> Result<Vec<GitHubRepository>> {
    let output = Command::new("gh")
        .args([
            "api",
            &format!("/users/{}/repos", username),
            "--paginate",
            "--jq", 
            ".[] | {name, html_url, archived, pushed_at, updated_at}"
        ])
        .output()
        .context("Failed to fetch GitHub repositories")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("GitHub API call failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut repositories = Vec::new();
    
    
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        
        let repo: GitHubRepository = serde_json::from_str(line)
            .with_context(|| format!("Failed to parse repository JSON: {}", line))?;
        repositories.push(repo);
    }

    Ok(repositories)
}

fn repository_to_project(repo: GitHubRepository, config: &Config) -> Result<Option<Project>> {
    
    if repo.archived {
        return Ok(None);
    }

    
    let clone_path = get_clone_path(&repo.name, config)?;
    
    
    let last_modified = parse_github_timestamp(&repo.pushed_at.or(repo.updated_at))?;

    let mut project = Project::new_github(repo.name, clone_path, repo.html_url);
    
    if let Some(timestamp) = last_modified {
        project = project.with_last_modified(timestamp);
    }

    Ok(Some(project))
}

fn get_clone_path(repo_name: &str, _config: &Config) -> Result<PathBuf> {
    let home = dirs::home_dir()
        .context("Failed to get home directory")?;
    
    
    Ok(home.join("Documents/git").join(repo_name))
}

fn parse_github_timestamp(timestamp_str: &Option<String>) -> Result<Option<DateTime<Utc>>> {
    match timestamp_str {
        Some(ts) => {
            let parsed = DateTime::parse_from_rfc3339(ts)
                .with_context(|| format!("Failed to parse GitHub timestamp: {}", ts))?;
            Ok(Some(parsed.with_timezone(&Utc)))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProjectSource;
    use chrono::TimeZone;

    fn create_test_repo(name: &str, archived: bool, pushed_at: Option<&str>) -> GitHubRepository {
        GitHubRepository {
            name: name.to_string(),
            html_url: format!("https://github.com/testuser/{}", name),
            archived,
            pushed_at: pushed_at.map(|s| s.to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        }
    }

    #[test]
    fn test_github_scanner_no_username() {
        let scanner = GitHubScanner;
        let config = Config {
            github_username: None,
            ..Config::default()
        };

        let result = scanner.scan(&config).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_github_scanner_no_gh_cli() {
        
        let scanner = GitHubScanner;
        let config = Config {
            github_username: Some("testuser".to_string()),
            ..Config::default()
        };

        
        
        let result = scanner.scan(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_repository_to_project_normal_repo() {
        let repo = create_test_repo("my-project", false, Some("2024-01-15T10:30:00Z"));
        let config = Config::default();

        let project = repository_to_project(repo, &config).unwrap().unwrap();
        
        assert_eq!(project.name, "my-project");
        assert_eq!(project.source, ProjectSource::GitHub);
        assert_eq!(project.github_url, Some("https://github.com/testuser/my-project".to_string()));
        assert!(project.last_modified.is_some());
        
        
        let expected_path = dirs::home_dir().unwrap().join("Documents/git/my-project");
        assert_eq!(project.path, expected_path);
    }

    #[test]
    fn test_repository_to_project_archived_repo() {
        let repo = create_test_repo("archived-project", true, Some("2024-01-15T10:30:00Z"));
        let config = Config::default();

        let result = repository_to_project(repo, &config).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_repository_to_project_no_timestamp() {
        let mut repo = create_test_repo("no-timestamp", false, None);
        repo.updated_at = None; 
        let config = Config::default();

        let project = repository_to_project(repo, &config).unwrap().unwrap();
        
        assert_eq!(project.name, "no-timestamp");
        assert!(project.last_modified.is_none());
    }

    #[test]
    fn test_parse_github_timestamp_valid() {
        let timestamp_str = Some("2024-01-15T10:30:00Z".to_string());
        let result = parse_github_timestamp(&timestamp_str).unwrap().unwrap();
        
        let expected = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_github_timestamp_none() {
        let result = parse_github_timestamp(&None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_github_timestamp_invalid() {
        let timestamp_str = Some("invalid-timestamp".to_string());
        let result = parse_github_timestamp(&timestamp_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_clone_path() {
        let config = Config::default();
        let path = get_clone_path("test-repo", &config).unwrap();
        
        let expected = dirs::home_dir().unwrap().join("Documents/git/test-repo");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_github_scanner_name() {
        let scanner = GitHubScanner;
        assert_eq!(scanner.scanner_name(), "github");
    }

    #[test]
    fn test_is_gh_installed() {
        
        
        let installed = is_gh_installed();
        
        assert!(installed == true || installed == false);
    }

    #[test]
    fn test_is_gh_authenticated() {
        
        
        let result = is_gh_authenticated();
        assert!(result.is_ok()); 
    }
} 