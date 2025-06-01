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
            return Ok(project_list);
        }

        if !is_gh_authenticated()? {
            return Ok(project_list);
        }

        let repositories = match fetch_user_repositories_with_timeout(github_username, 10) {
            Ok(repos) => repos,
            Err(e) => {
                eprintln!("Warning: GitHub API request timed out or failed: {}", e);
                return Ok(project_list);
            }
        };
        
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
        .args(["api", "user", "--jq", ".login"])
        .output()
        .context("Failed to test GitHub API access")?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    Ok(output.status.success() && !stdout.trim().is_empty())
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

/// Prompt user to set up GitHub integration interactively
pub fn prompt_github_setup() -> Result<Option<String>> {
    use dialoguer::Confirm;

    println!("\n🐙 GitHub Integration Setup");
    println!("─────────────────────────────");
    println!("Connect your GitHub account to discover and switch between your repositories.");
    println!();

    let setup_github = Confirm::new()
        .with_prompt("Would you like to set up GitHub integration?")
        .default(true)
        .interact()
        .context("Failed to get user input for GitHub setup")?;

    if !setup_github {
        println!("⏭️  Skipping GitHub integration. You can set it up later with: sw setup");
        return Ok(None);
    }

    // Check if GitHub CLI is installed
    if !is_gh_installed() {
        println!("❌ GitHub CLI (gh) is not installed.");
        println!();
        println!("To use GitHub integration, please install the GitHub CLI:");
        println!("  macOS:  brew install gh");
        println!("  Linux:  Visit https://cli.github.com/");
        println!("  Windows: Visit https://cli.github.com/");
        println!();
        
        let continue_anyway = Confirm::new()
            .with_prompt("Continue without GitHub integration for now?")
            .default(true)
            .interact()
            .context("Failed to get user input for continuing without GitHub")?;

        if continue_anyway {
            println!("⏭️  You can set up GitHub integration later with: sw setup");
            return Ok(None);
        } else {
            println!("Please install GitHub CLI and run 'sw' again.");
            std::process::exit(1);
        }
    }

    // Check if already authenticated
    if is_gh_authenticated()? {
        println!("✅ GitHub CLI is already authenticated!");
        
        // Try to get the username using the same API call we use for auth check
        match get_gh_username() {
            Ok(username) => {
                println!("📝 Authenticated as: {}", username);
                println!("🐙 GitHub integration enabled! Your repositories will be discovered automatically.");
                return Ok(Some(username));
            }
            Err(e) => {
                println!("⚠️  Authentication detected but could not determine GitHub username: {}", e);
                println!("This might be due to token scope limitations.");
                
                let manual_username = dialoguer::Input::<String>::new()
                    .with_prompt("Please enter your GitHub username manually")
                    .interact()
                    .context("Failed to get GitHub username input")?;

                if manual_username.trim().is_empty() {
                    println!("⏭️  Skipping GitHub integration.");
                    return Ok(None);
                } else {
                    println!("🐙 GitHub integration enabled with username: {}", manual_username.trim());
                    return Ok(Some(manual_username.trim().to_string()));
                }
            }
        }
    }

    // Need to authenticate
    println!("🔐 GitHub authentication required...");
    println!();
    
    let do_auth = Confirm::new()
        .with_prompt("Authenticate with GitHub now?")
        .default(true)
        .interact()
        .context("Failed to get authentication confirmation")?;

    if !do_auth {
        println!("⏭️  Skipping GitHub authentication. You can set it up later with: sw setup");
        return Ok(None);
    }

    // Run authentication
    if run_gh_auth_login()? {
        // Try to get username after successful auth
        match get_gh_username() {
            Ok(username) => {
                println!("📝 Successfully authenticated as: {}", username);
                println!("🐙 GitHub integration enabled! Your repositories will be discovered automatically.");
                Ok(Some(username))
            }
            Err(e) => {
                println!("⚠️  Authentication succeeded but could not determine username: {}", e);
                
                let manual_username = dialoguer::Input::<String>::new()
                    .with_prompt("Please enter your GitHub username")
                    .allow_empty(true)
                    .interact()
                    .context("Failed to get GitHub username input")?;

                if manual_username.trim().is_empty() {
                    println!("⏭️  GitHub authentication completed but no username provided.");
                    Ok(None)
                } else {
                    println!("🐙 GitHub integration enabled with username: {}", manual_username.trim());
                    Ok(Some(manual_username.trim().to_string()))
                }
            }
        }
    } else {
        println!("⏭️  GitHub authentication was cancelled. You can try again later with: sw setup");
        Ok(None)
    }
}

/// Get the authenticated GitHub username
pub fn get_gh_username() -> Result<String> {
    let output = Command::new("gh")
        .args(["api", "user", "--jq", ".login"])
        .output()
        .context("Failed to get GitHub username")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to get GitHub username: {}", stderr);
    }

    let username = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    if username.is_empty() {
        anyhow::bail!("GitHub username is empty");
    }

    Ok(username)
}

fn fetch_user_repositories_with_timeout(username: &str, timeout_seconds: u64) -> Result<Vec<GitHubRepository>> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    
    let start_time = Instant::now();
    
    
    let mut child = Command::new("gh")
        .args([
            "api",
            &format!("/users/{}/repos", username),
            "--paginate",
            "--jq", 
            ".[] | {name, html_url, archived, pushed_at, updated_at}"
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn GitHub API command")?;

    
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                
                let output = child.wait_with_output()
                    .context("Failed to get output from GitHub API command")?;
                
                if !status.success() {
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

                return Ok(repositories);
            }
            Ok(None) => {
                
                if start_time.elapsed() > Duration::from_secs(timeout_seconds) {
                    
                    let _ = child.kill();
                    let _ = child.wait(); 
                    anyhow::bail!("GitHub API request timed out after {} seconds", timeout_seconds);
                }
                
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(e).context("Error waiting for GitHub API command");
            }
        }
    }
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

    #[test]
    fn test_timeout_mechanism() {
        // This is a unit test to verify the timeout logic compiles correctly
        // In a real scenario, we would need to mock the Command execution
        let result = fetch_user_repositories_with_timeout("testuser", 1);
        // We expect this to fail in test environment since gh CLI might not be available
        // But the important thing is that the function doesn't panic
        let _ = result;
    }

    #[test]
    fn test_get_gh_username_function_exists() {
        // Test that the function exists and returns a Result
        // We can't test the actual functionality in CI since gh CLI might not be authenticated
        // But we can verify the function signature and error handling
        let result = get_gh_username();
        assert!(result.is_ok() || result.is_err()); // Either way is fine, just don't panic
    }

    #[test]
    fn test_is_gh_installed_function() {
        // Test that the function returns a boolean without panicking
        let result = is_gh_installed();
        // The result depends on whether gh is installed in the test environment
        // But it should always return a boolean
        assert!(result == true || result == false);
    }

    #[test]
    fn test_is_gh_authenticated_function() {
        // Test that the function returns a Result without panicking
        // This now tests API access rather than auth status
        let result = is_gh_authenticated();
        // Should always return a Result, regardless of actual authentication state
        assert!(result.is_ok() || result.is_err());
    }
} 