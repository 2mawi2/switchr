use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use git2::Repository;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::fs;
use std::path::Path;

use crate::config::Config;
use crate::models::{Project, ProjectList};
use super::ProjectScanner;

pub struct LocalScanner;

impl ProjectScanner for LocalScanner {
    fn scan(&self, config: &Config) -> Result<ProjectList> {
        let all_projects: Result<Vec<_>> = config
            .project_dirs
            .par_iter()
            .map(|dir| scan_directory(dir))
            .collect();

        let mut project_list = ProjectList::new();
        for projects in all_projects? {
            for project in projects {
                project_list.add_project(project);
            }
        }

        project_list.sort_by_last_modified();
        Ok(project_list)
    }

    fn scanner_name(&self) -> &'static str {
        "local"
    }
}

fn scan_directory(base_dir: &Path) -> Result<Vec<Project>> {
    if !base_dir.exists() {
        return Ok(vec![]);
    }

    let mut projects = Vec::new();
    
    let walker = WalkBuilder::new(base_dir)
        .max_depth(Some(3))
        .hidden(false)
        .ignore(false)
        .git_ignore(false)
        .build();

    for entry in walker {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        if !entry.file_type().is_some_and(|ft| ft.is_dir()) {
            continue;
        }

        if is_hidden_directory(path) {
            continue;
        }

        if is_project_directory(path) {
            let project_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let mut project = Project::new_local(project_name, path.to_path_buf());
            
            if let Some(timestamp) = get_project_timestamp(path) {
                project = project.with_last_modified(timestamp);
            }

            projects.push(project);
        }
    }

    Ok(projects)
}

fn is_hidden_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
}

fn is_project_directory(path: &Path) -> bool {
    
    has_git_directory(path)
}

fn has_git_directory(path: &Path) -> bool {
    path.join(".git").exists()
}

fn get_project_timestamp(path: &Path) -> Option<DateTime<Utc>> {
    if let Some(git_timestamp) = get_git_last_commit_time(path) {
        return Some(git_timestamp);
    }

    if let Some(dir_timestamp) = get_directory_modified_time(path) {
        return Some(dir_timestamp);
    }

    None
}

fn get_git_last_commit_time(path: &Path) -> Option<DateTime<Utc>> {
    let repo = Repository::open(path).ok()?;
    let head = repo.head().ok()?;
    let commit = head.peel_to_commit().ok()?;
    let timestamp = commit.time();
    
    DateTime::from_timestamp(timestamp.seconds(), 0)
}

fn get_directory_modified_time(path: &Path) -> Option<DateTime<Utc>> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    
    DateTime::from_timestamp(
        modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
        0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProjectSource;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_project(base_dir: &Path, name: &str, project_file: &str) -> PathBuf {
        let project_dir = base_dir.join(name);
        fs::create_dir_all(&project_dir).unwrap();
        
        
        Repository::init(&project_dir).unwrap();
        
        
        fs::write(project_dir.join(project_file), "").unwrap();
        
        
        match project_file {
            "Cargo.toml" => {
                fs::create_dir_all(project_dir.join("src")).unwrap();
                fs::write(project_dir.join("src/main.rs"), "fn main() {}").unwrap();
            }
            "package.json" => {
                fs::write(project_dir.join("index.js"), "console.log('hello')").unwrap();
            }
            _ => {
                fs::write(project_dir.join("README.md"), "# Test Project").unwrap();
            }
        }
        
        project_dir
    }

    fn create_git_project(base_dir: &Path, name: &str) -> PathBuf {
        let project_dir = base_dir.join(name);
        fs::create_dir_all(&project_dir).unwrap();
        
        
        Repository::init(&project_dir).unwrap();
        
        
        fs::write(project_dir.join("README.md"), "# Git Project").unwrap();
        
        project_dir
    }

    #[test]
    fn test_is_project_directory() {
        let temp_dir = TempDir::new().unwrap();
        
        
        let rust_project = create_test_project(temp_dir.path(), "rust-project", "Cargo.toml");
        assert!(is_project_directory(&rust_project));

        
        let node_project = create_test_project(temp_dir.path(), "node-project", "package.json");
        assert!(is_project_directory(&node_project));

        
        let git_project = create_git_project(temp_dir.path(), "git-project");
        assert!(is_project_directory(&git_project));

        
        let empty_dir = temp_dir.path().join("empty");
        fs::create_dir_all(&empty_dir).unwrap();
        assert!(!is_project_directory(&empty_dir));
    }

    #[test]
    fn test_is_hidden_directory() {
        let temp_dir = TempDir::new().unwrap();
        
        let hidden_dir = temp_dir.path().join(".hidden");
        fs::create_dir_all(&hidden_dir).unwrap();
        assert!(is_hidden_directory(&hidden_dir));

        let normal_dir = temp_dir.path().join("normal");
        fs::create_dir_all(&normal_dir).unwrap();
        assert!(!is_hidden_directory(&normal_dir));
    }

    #[test]
    fn test_scan_directory() {
        let temp_dir = TempDir::new().unwrap();
        
        
        create_test_project(temp_dir.path(), "rust-app", "Cargo.toml");
        create_test_project(temp_dir.path(), "node-app", "package.json");
        create_git_project(temp_dir.path(), "git-repo");
        
        
        let hidden_dir = temp_dir.path().join(".hidden");
        fs::create_dir_all(&hidden_dir).unwrap();
        fs::write(hidden_dir.join("Cargo.toml"), "").unwrap();

        
        let empty_dir = temp_dir.path().join("empty");
        fs::create_dir_all(&empty_dir).unwrap();

        let projects = scan_directory(temp_dir.path()).unwrap();
        
        assert_eq!(projects.len(), 3);
        
        let project_names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
        assert!(project_names.contains(&"rust-app"));
        assert!(project_names.contains(&"node-app"));
        assert!(project_names.contains(&"git-repo"));
        assert!(!project_names.contains(&".hidden")); 
        assert!(!project_names.contains(&"empty"));   

        
        assert!(projects.iter().all(|p| p.source == ProjectSource::Local));
    }

    #[test]
    fn test_local_scanner() {
        let temp_dir = TempDir::new().unwrap();
        
        
        create_test_project(temp_dir.path(), "project1", "Cargo.toml");
        create_test_project(temp_dir.path(), "project2", "package.json");

        let mut config = Config::default();
        config.project_dirs = vec![temp_dir.path().to_path_buf()];

        let scanner = LocalScanner;
        let result = scanner.scan(&config).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(scanner.scanner_name(), "local");
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("does-not-exist");

        let projects = scan_directory(&nonexistent).unwrap();
        assert!(projects.is_empty());
    }

    #[test]
    fn test_get_directory_modified_time() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test");
        fs::create_dir_all(&test_dir).unwrap();

        let timestamp = get_directory_modified_time(&test_dir);
        assert!(timestamp.is_some());
    }

    #[test]
    fn test_project_file_detection() {
        let temp_dir = TempDir::new().unwrap();
        
        
        let project_files = [
            "Cargo.toml", "package.json", "pyproject.toml", "setup.py",
            "requirements.txt", "go.mod", "pom.xml", "build.gradle",
            "Makefile", "justfile", "Dockerfile", "README.md",
        ];

        for file in &project_files {
            let project_dir = create_test_project(temp_dir.path(), &format!("test-{}", file), file);
            assert!(
                is_project_directory(&project_dir),
                "Failed to detect Git repository with file: {}",
                file
            );
        }
        
        
        let non_git_dir = temp_dir.path().join("not-a-git-repo");
        fs::create_dir_all(&non_git_dir).unwrap();
        fs::write(non_git_dir.join("Cargo.toml"), "").unwrap();
        assert!(!is_project_directory(&non_git_dir));
    }
} 