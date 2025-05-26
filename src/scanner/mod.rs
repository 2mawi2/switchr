use anyhow::Result;
use crate::models::ProjectList;
use crate::config::Config;

pub mod local;
pub mod cursor;
pub mod github;

pub trait ProjectScanner {
    fn scan(&self, config: &Config) -> Result<ProjectList>;
    
    fn scanner_name(&self) -> &'static str;
}

pub struct ScanManager {
    scanners: Vec<Box<dyn ProjectScanner>>,
}

impl ScanManager {
    pub fn new() -> Self {
        Self {
            scanners: vec![
                Box::new(local::LocalScanner),
                Box::new(cursor::CursorScanner),
                Box::new(github::GitHubScanner),
            ],
        }
    }

    #[cfg(test)]
    pub fn new_with_scanners(scanners: Vec<Box<dyn ProjectScanner>>) -> Self {
        Self { scanners }
    }

    pub fn scan_all(&self, config: &Config) -> Result<ProjectList> {
        let mut all_projects = ProjectList::new();

        for scanner in &self.scanners {
            match scanner.scan(config) {
                Ok(projects) => {
                    for project in projects.projects() {
                        all_projects.add_project(project.clone());
                    }
                }
                Err(e) => {
                    eprintln!("Warning: {} scanner failed: {}", scanner.scanner_name(), e);
                }
            }
        }

        
        all_projects.deduplicate();
        all_projects.sort_by_last_modified();
        Ok(all_projects)
    }
}

impl Default for ScanManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Project;

    struct MockScanner {
        name: &'static str,
        projects: Vec<Project>,
        should_fail: bool,
    }

    impl MockScanner {
        fn new(name: &'static str, projects: Vec<Project>) -> Self {
            Self {
                name,
                projects,
                should_fail: false,
            }
        }

        fn new_failing(name: &'static str) -> Self {
            Self {
                name,
                projects: vec![],
                should_fail: true,
            }
        }
    }

    impl ProjectScanner for MockScanner {
        fn scan(&self, _config: &Config) -> Result<ProjectList> {
            if self.should_fail {
                anyhow::bail!("Mock scanner failure");
            }
            Ok(ProjectList::from_projects(self.projects.clone()))
        }

        fn scanner_name(&self) -> &'static str {
            self.name
        }
    }

    #[test]
    fn test_scan_manager_with_mock_scanners() {
        let scanner1 = MockScanner::new(
            "mock1",
            vec![Project::new_local("project1".to_string(), "/path1")],
        );
        let scanner2 = MockScanner::new(
            "mock2",
            vec![Project::new_local("project2".to_string(), "/path2")],
        );

        let manager = ScanManager::new_with_scanners(vec![
            Box::new(scanner1),
            Box::new(scanner2),
        ]);

        let config = Config::default();
        let result = manager.scan_all(&config).unwrap();

        assert_eq!(result.len(), 2);
        let project_names: Vec<&str> = result.projects().iter().map(|p| p.name.as_str()).collect();
        assert!(project_names.contains(&"project1"));
        assert!(project_names.contains(&"project2"));
    }

    #[test]
    fn test_scan_manager_with_failing_scanner() {
        let good_scanner = MockScanner::new(
            "good",
            vec![Project::new_local("project1".to_string(), "/path1")],
        );
        let bad_scanner = MockScanner::new_failing("bad");

        let manager = ScanManager::new_with_scanners(vec![
            Box::new(good_scanner),
            Box::new(bad_scanner),
        ]);

        let config = Config::default();
        let result = manager.scan_all(&config).unwrap();

        
        assert_eq!(result.len(), 1);
        assert_eq!(result.projects()[0].name, "project1");
    }
} 