use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::models::{Project, ProjectList};
use super::ProjectScanner;

pub struct CursorScanner;

#[derive(Debug, Deserialize)]
struct WorkspaceStorage {
    #[serde(rename = "workspaceIdentifier")]
    workspace_identifier: Option<WorkspaceIdentifier>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceIdentifier {
    #[serde(rename = "configPath")]
    config_path: Option<String>,
}

impl ProjectScanner for CursorScanner {
    fn scan(&self, _config: &Config) -> Result<ProjectList> {
        let mut project_list = ProjectList::new();
        
        let cursor_storage_path = get_cursor_storage_path()?;
        if !cursor_storage_path.exists() {
            return Ok(project_list);
        }

        let workspaces = scan_cursor_workspaces(&cursor_storage_path)?;
        
        for workspace in workspaces {
            if let Some(project) = workspace_to_project(workspace)? {
                project_list.add_project(project);
            }
        }

        project_list.sort_by_last_modified();
        Ok(project_list)
    }

    fn scanner_name(&self) -> &'static str {
        "cursor"
    }
}

fn get_cursor_storage_path() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .context("Failed to get home directory")?;
    
    #[cfg(target_os = "macos")]
    let storage_path = home.join("Library/Application Support/Cursor/User/workspaceStorage");
    
    #[cfg(target_os = "linux")]
    let storage_path = home.join(".config/Cursor/User/workspaceStorage");
    
    #[cfg(target_os = "windows")]
    let storage_path = home.join("AppData/Roaming/Cursor/User/workspaceStorage");
    
    Ok(storage_path)
}

fn scan_cursor_workspaces(storage_path: &Path) -> Result<Vec<WorkspaceInfo>> {
    let mut workspaces = Vec::new();
    
    if !storage_path.exists() {
        return Ok(workspaces);
    }

    for entry in fs::read_dir(storage_path)
        .with_context(|| format!("Failed to read Cursor storage directory: {}", storage_path.display()))?
    {
        let entry = entry.context("Failed to read directory entry")?;
        let workspace_dir = entry.path();
        
        if !workspace_dir.is_dir() {
            continue;
        }

        if let Some(workspace_info) = parse_workspace_directory(&workspace_dir)? {
            workspaces.push(workspace_info);
        }
    }

    Ok(workspaces)
}

#[derive(Debug)]
struct WorkspaceInfo {
    path: PathBuf,
    name: String,
    last_modified: Option<DateTime<Utc>>,
}

fn parse_workspace_directory(workspace_dir: &Path) -> Result<Option<WorkspaceInfo>> {
    let workspace_json_path = workspace_dir.join("workspace.json");
    
    if !workspace_json_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&workspace_json_path)
        .with_context(|| format!("Failed to read workspace.json: {}", workspace_json_path.display()))?;

    let storage: WorkspaceStorage = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse workspace.json: {}", workspace_json_path.display()))?;

    if let Some(workspace_id) = storage.workspace_identifier {
        if let Some(config_path) = workspace_id.config_path {
            let project_path = PathBuf::from(&config_path);
            
            
            let project_name = project_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            
            let last_modified = fs::metadata(&workspace_json_path)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| {
                    DateTime::from_timestamp(
                        modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                        0,
                    )
                });

            return Ok(Some(WorkspaceInfo {
                path: project_path,
                name: project_name,
                last_modified,
            }));
        }
    }

    Ok(None)
}

fn workspace_to_project(workspace: WorkspaceInfo) -> Result<Option<Project>> {
    
    if !workspace.path.exists() {
        return Ok(None);
    }

    let mut project = Project::new_cursor(workspace.name, workspace.path);
    
    if let Some(timestamp) = workspace.last_modified {
        project = project.with_last_modified(timestamp);
    }

    Ok(Some(project))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProjectSource;
    use std::fs;
    use tempfile::TempDir;
    use chrono::TimeZone;

    fn create_test_workspace_storage(base_dir: &Path, workspace_id: &str, config_path: &str) -> PathBuf {
        let workspace_dir = base_dir.join(workspace_id);
        fs::create_dir_all(&workspace_dir).unwrap();
        
        let workspace_json = serde_json::json!({
            "workspaceIdentifier": {
                "configPath": config_path,
            }
        });
        
        let workspace_json_path = workspace_dir.join("workspace.json");
        fs::write(&workspace_json_path, workspace_json.to_string()).unwrap();
        
        workspace_dir
    }

    fn create_test_project_dir(path: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().join(path.trim_start_matches('/'));
        fs::create_dir_all(&project_path).unwrap();
        
        
        fs::write(project_path.join("README.md"), "# Test Project").unwrap();
        
        temp_dir
    }

    #[test]
    fn test_cursor_scanner_empty_storage() {
        let temp_dir = TempDir::new().unwrap();
        let _scanner = CursorScanner;
        let _config = Config::default();

        
        let result = scan_cursor_workspaces(temp_dir.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_cursor_scanner_nonexistent_storage() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("does-not-exist");
        
        let result = scan_cursor_workspaces(&nonexistent).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_workspace_directory_valid() {
        let temp_dir = TempDir::new().unwrap();
        let project_temp = create_test_project_dir("/Users/test/my-project");
        let project_path = project_temp.path().join("Users/test/my-project");
        
        let workspace_dir = create_test_workspace_storage(
            temp_dir.path(),
            "workspace123",
            project_path.to_str().unwrap()
        );

        let workspace_info = parse_workspace_directory(&workspace_dir).unwrap().unwrap();
        
        assert_eq!(workspace_info.name, "my-project");
        assert_eq!(workspace_info.path, project_path);
        assert!(workspace_info.last_modified.is_some());
    }

    #[test]
    fn test_parse_workspace_directory_no_workspace_json() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_dir = temp_dir.path().join("workspace123");
        fs::create_dir_all(&workspace_dir).unwrap();

        let result = parse_workspace_directory(&workspace_dir).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_workspace_directory_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_dir = temp_dir.path().join("workspace123");
        fs::create_dir_all(&workspace_dir).unwrap();
        
        let workspace_json_path = workspace_dir.join("workspace.json");
        fs::write(&workspace_json_path, "{ invalid json }").unwrap();

        let result = parse_workspace_directory(&workspace_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_workspace_directory_missing_config_path() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_dir = temp_dir.path().join("workspace123");
        fs::create_dir_all(&workspace_dir).unwrap();
        
        let workspace_json = serde_json::json!({
            "workspaceIdentifier": {
                
            }
        });
        
        let workspace_json_path = workspace_dir.join("workspace.json");
        fs::write(&workspace_json_path, workspace_json.to_string()).unwrap();

        let result = parse_workspace_directory(&workspace_dir).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_workspace_to_project_existing_path() {
        let project_temp = create_test_project_dir("/Users/test/existing-project");
        let project_path = project_temp.path().join("Users/test/existing-project");
        
        let workspace_info = WorkspaceInfo {
            path: project_path.clone(),
            name: "existing-project".to_string(),
            last_modified: Some(Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap()),
        };

        let project = workspace_to_project(workspace_info).unwrap().unwrap();
        
        assert_eq!(project.name, "existing-project");
        assert_eq!(project.path, project_path);
        assert_eq!(project.source, ProjectSource::Cursor);
        assert!(project.last_modified.is_some());
    }

    #[test]
    fn test_workspace_to_project_nonexistent_path() {
        let workspace_info = WorkspaceInfo {
            path: PathBuf::from("/nonexistent/path"),
            name: "nonexistent-project".to_string(),
            last_modified: None,
        };

        let result = workspace_to_project(workspace_info).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_workspace_to_project_non_git_directory() {
        let temp_dir = TempDir::new().unwrap();
        let non_git_path = temp_dir.path().join("regular-project");
        fs::create_dir_all(&non_git_path).unwrap();
        fs::write(non_git_path.join("some-file.txt"), "content").unwrap();
        
        let workspace_info = WorkspaceInfo {
            path: non_git_path.clone(),
            name: "regular-project".to_string(),
            last_modified: None,
        };

        let result = workspace_to_project(workspace_info).unwrap();
        assert!(result.is_some());
        
        let project = result.unwrap();
        assert_eq!(project.name, "regular-project");
        assert_eq!(project.path, non_git_path);
        assert_eq!(project.source, crate::models::ProjectSource::Cursor);
    }

    #[test]
    fn test_cursor_scanner_integration() {
        let temp_dir = TempDir::new().unwrap();
        let project_temp1 = create_test_project_dir("/Users/test/project1");
        let project_temp2 = create_test_project_dir("/Users/test/project2");
        
        let project_path1 = project_temp1.path().join("Users/test/project1");
        let project_path2 = project_temp2.path().join("Users/test/project2");
        
        
        create_test_workspace_storage(
            temp_dir.path(),
            "workspace1",
            project_path1.to_str().unwrap()
        );
        create_test_workspace_storage(
            temp_dir.path(),
            "workspace2", 
            project_path2.to_str().unwrap()
        );
        
        
        create_test_workspace_storage(
            temp_dir.path(),
            "workspace3",
            "/nonexistent/path"
        );

        let workspaces = scan_cursor_workspaces(temp_dir.path()).unwrap();
        assert_eq!(workspaces.len(), 3); 

        
        let mut projects = Vec::new();
        for workspace in workspaces {
            if let Some(project) = workspace_to_project(workspace).unwrap() {
                projects.push(project);
            }
        }
        
        assert_eq!(projects.len(), 2); 
        
        let project_names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
        assert!(project_names.contains(&"project1"));
        assert!(project_names.contains(&"project2"));
        
        
        assert!(projects.iter().all(|p| p.source == ProjectSource::Cursor));
    }

    #[test]
    fn test_get_cursor_storage_path() {
        let path = get_cursor_storage_path().unwrap();
        
        #[cfg(target_os = "macos")]
        assert!(path.to_string_lossy().contains("Library/Application Support/Cursor/User/workspaceStorage"));
        
        #[cfg(target_os = "linux")]
        assert!(path.to_string_lossy().contains(".config/Cursor/User/workspaceStorage"));
        
        #[cfg(target_os = "windows")]
        assert!(path.to_string_lossy().contains("AppData/Roaming/Cursor/User/workspaceStorage"));
    }

    #[test]
    fn test_cursor_scanner_name() {
        let scanner = CursorScanner;
        assert_eq!(scanner.scanner_name(), "cursor");
    }
} 