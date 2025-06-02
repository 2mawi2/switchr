use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectSource {
    /// Project found in local filesystem
    Local,
    /// Project found in Cursor's workspace storage
    Cursor,
    /// Project found in GitHub repositories
    GitHub,
    /// Project found in GitLab repositories
    GitLab,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// The project name (usually the directory name)
    pub name: String,
    /// The path to the project directory
    pub path: PathBuf,
    /// When the project was last modified (if available)
    pub last_modified: Option<DateTime<Utc>>,
    /// Where this project was discovered
    pub source: ProjectSource,
    /// GitHub URL if this is a GitHub project
    pub github_url: Option<String>,
    /// GitLab URL if this is a GitLab project
    pub gitlab_url: Option<String>,
}

impl Project {
    pub fn new_local<P: Into<PathBuf>>(name: String, path: P) -> Self {
        Self {
            name,
            path: path.into(),
            last_modified: None,
            source: ProjectSource::Local,
            github_url: None,
            gitlab_url: None,
        }
    }

    pub fn new_cursor<P: Into<PathBuf>>(name: String, path: P) -> Self {
        Self {
            name,
            path: path.into(),
            last_modified: None,
            source: ProjectSource::Cursor,
            github_url: None,
            gitlab_url: None,
        }
    }

    pub fn new_github<P: Into<PathBuf>>(name: String, path: P, github_url: String) -> Self {
        Self {
            name,
            path: path.into(),
            last_modified: None,
            source: ProjectSource::GitHub,
            github_url: Some(github_url),
            gitlab_url: None,
        }
    }

    pub fn new_gitlab<P: Into<PathBuf>>(name: String, path: P, gitlab_url: String) -> Self {
        Self {
            name,
            path: path.into(),
            last_modified: None,
            source: ProjectSource::GitLab,
            github_url: None,
            gitlab_url: Some(gitlab_url),
        }
    }

    pub fn with_last_modified(mut self, timestamp: DateTime<Utc>) -> Self {
        self.last_modified = Some(timestamp);
        self
    }

    #[allow(dead_code)]
    pub fn exists_locally(&self) -> bool {
        self.path.exists()
    }

    pub fn display_string(&self) -> String {
        let source_indicator = match self.source {
            ProjectSource::Local => "📁",
            ProjectSource::Cursor => "🎯",
            ProjectSource::GitHub => "🐙",
            ProjectSource::GitLab => "🦊",
        };

        let time_str = if let Some(timestamp) = self.last_modified {
            format!(" ({})", timestamp.format("%Y-%m-%d %H:%M"))
        } else {
            String::new()
        };

        format!(
            "{} {}{} - {}",
            source_indicator,
            self.name,
            time_str,
            self.path.display()
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProjectList {
    projects: Vec<Project>,
}

impl ProjectList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_projects(projects: Vec<Project>) -> Self {
        Self { projects }
    }

    pub fn add_project(&mut self, project: Project) {
        self.projects.push(project);
    }

    pub fn projects(&self) -> &[Project] {
        &self.projects
    }

    pub fn len(&self) -> usize {
        self.projects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.projects.is_empty()
    }

    pub fn sort_by_last_modified(&mut self) {
        self.projects
            .sort_by(|a, b| match (a.last_modified, b.last_modified) {
                (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.name.cmp(&b.name),
            });
    }

    #[allow(dead_code)]
    pub fn filter_by_source(&self, source: ProjectSource) -> Vec<&Project> {
        self.projects
            .iter()
            .filter(|p| p.source == source)
            .collect()
    }

    pub fn deduplicate(&mut self) {
        let mut to_remove = Vec::new();

        let local_paths: std::collections::HashSet<_> = self
            .projects
            .iter()
            .filter(|p| p.source == ProjectSource::Local)
            .map(|p| &p.path)
            .collect();

        for (i, project) in self.projects.iter().enumerate() {
            if project.source == ProjectSource::GitHub && local_paths.contains(&project.path) {
                to_remove.push(i);
            }
        }

        for &i in to_remove.iter().rev() {
            self.projects.remove(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_project_creation() {
        let project = Project::new_local("test-project".to_string(), "/path/to/project");

        assert_eq!(project.name, "test-project");
        assert_eq!(project.path, PathBuf::from("/path/to/project"));
        assert_eq!(project.source, ProjectSource::Local);
        assert!(project.last_modified.is_none());
        assert!(project.github_url.is_none());
    }

    #[test]
    fn test_cursor_project_creation() {
        let project = Project::new_cursor("cursor-project".to_string(), "/cursor/path");

        assert_eq!(project.source, ProjectSource::Cursor);
        assert_eq!(project.name, "cursor-project");
    }

    #[test]
    fn test_github_project_creation() {
        let project = Project::new_github(
            "github-project".to_string(),
            "/github/path",
            "https://github.com/user/repo".to_string(),
        );

        assert_eq!(project.source, ProjectSource::GitHub);
        assert_eq!(
            project.github_url,
            Some("https://github.com/user/repo".to_string())
        );
        assert!(project.gitlab_url.is_none());
    }

    #[test]
    fn test_gitlab_project_creation() {
        let project = Project::new_gitlab(
            "gitlab-project".to_string(),
            "/gitlab/path",
            "https://gitlab.example.com/user/repo".to_string(),
        );

        assert_eq!(project.name, "gitlab-project");
        assert_eq!(project.path, PathBuf::from("/gitlab/path"));
        assert_eq!(project.source, ProjectSource::GitLab);
        assert_eq!(
            project.gitlab_url,
            Some("https://gitlab.example.com/user/repo".to_string())
        );
        assert!(project.github_url.is_none());
        assert!(project.last_modified.is_none());
    }

    #[test]
    fn test_project_with_timestamp() {
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let project = Project::new_local("test".to_string(), "/path").with_last_modified(timestamp);

        assert_eq!(project.last_modified, Some(timestamp));
    }

    #[test]
    fn test_display_string() {
        // Test local project
        let local_project = Project::new_local("local-proj".to_string(), "/path/to/local");
        assert!(local_project.display_string().starts_with("📁 local-proj"));

        // Test cursor project
        let cursor_project = Project::new_cursor("cursor-proj".to_string(), "/path/to/cursor");
        assert!(cursor_project
            .display_string()
            .starts_with("🎯 cursor-proj"));

        // Test GitHub project
        let github_project = Project::new_github(
            "github-proj".to_string(),
            "/path/to/github",
            "https://github.com/user/repo".to_string(),
        );
        assert!(github_project
            .display_string()
            .starts_with("🐙 github-proj"));

        // Test GitLab project
        let gitlab_project = Project::new_gitlab(
            "gitlab-proj".to_string(),
            "/path/to/gitlab",
            "https://gitlab.example.com/user/repo".to_string(),
        );
        assert!(gitlab_project
            .display_string()
            .starts_with("🦊 gitlab-proj"));

        // Test with timestamp
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let project_with_time =
            Project::new_local("timed-proj".to_string(), "/path").with_last_modified(timestamp);

        let display = project_with_time.display_string();
        assert!(display.contains("📁 timed-proj"));
        assert!(display.contains("(2024-01-15 10:30)"));
    }

    #[test]
    fn test_project_list_operations() {
        let mut list = ProjectList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);

        let project = Project::new_local("test".to_string(), "/path");
        list.add_project(project.clone());

        assert!(!list.is_empty());
        assert_eq!(list.len(), 1);
        assert_eq!(list.projects()[0], project);
    }

    #[test]
    fn test_project_list_sorting() {
        let mut list = ProjectList::new();

        let old_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let new_time = Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap();

        let old_project =
            Project::new_local("old".to_string(), "/old").with_last_modified(old_time);
        let new_project =
            Project::new_local("new".to_string(), "/new").with_last_modified(new_time);
        let no_time_project = Project::new_local("no-time".to_string(), "/no-time");

        list.add_project(old_project.clone());
        list.add_project(new_project.clone());
        list.add_project(no_time_project.clone());

        list.sort_by_last_modified();

        assert_eq!(list.projects()[0], new_project);
        assert_eq!(list.projects()[1], old_project);
        assert_eq!(list.projects()[2], no_time_project);
    }

    #[test]
    fn test_filter_by_source() {
        let projects = vec![
            Project::new_local("local1".to_string(), "/path1"),
            Project::new_cursor("cursor1".to_string(), "/path2"),
            Project::new_github(
                "github1".to_string(),
                "/path3",
                "https://github.com/user/repo".to_string(),
            ),
        ];

        let project_list = ProjectList::from_projects(projects);

        let local_projects = project_list.filter_by_source(ProjectSource::Local);
        assert_eq!(local_projects.len(), 1);
        assert_eq!(local_projects[0].name, "local1");

        let cursor_projects = project_list.filter_by_source(ProjectSource::Cursor);
        assert_eq!(cursor_projects.len(), 1);
        assert_eq!(cursor_projects[0].name, "cursor1");

        let github_projects = project_list.filter_by_source(ProjectSource::GitHub);
        assert_eq!(github_projects.len(), 1);
        assert_eq!(github_projects[0].name, "github1");
    }

    #[test]
    fn test_deduplicate_projects() {
        let shared_path = PathBuf::from("/Users/test/my-project");

        let projects = vec![
            Project::new_local("my-project".to_string(), shared_path.clone()),
            Project::new_github(
                "my-project".to_string(),
                shared_path.clone(),
                "https://github.com/user/my-project".to_string(),
            ),
            Project::new_cursor("other-project".to_string(), "/different/path"),
        ];

        let mut project_list = ProjectList::from_projects(projects);
        assert_eq!(project_list.len(), 3);

        project_list.deduplicate();
        assert_eq!(project_list.len(), 2);

        let remaining_projects: Vec<_> = project_list
            .projects()
            .iter()
            .map(|p| (&p.name, &p.source))
            .collect();
        assert!(remaining_projects.contains(&(&"my-project".to_string(), &ProjectSource::Local)));
        assert!(
            remaining_projects.contains(&(&"other-project".to_string(), &ProjectSource::Cursor))
        );
        assert!(!remaining_projects
            .iter()
            .any(|(_, source)| **source == ProjectSource::GitHub));
    }

    #[test]
    fn test_deduplicate_no_duplicates() {
        let projects = vec![
            Project::new_local("project1".to_string(), "/path1"),
            Project::new_cursor("project2".to_string(), "/path2"),
            Project::new_github(
                "project3".to_string(),
                "/path3",
                "https://github.com/user/project3".to_string(),
            ),
        ];

        let mut project_list = ProjectList::from_projects(projects);
        let original_len = project_list.len();

        project_list.deduplicate();
        assert_eq!(project_list.len(), original_len);
    }
}
