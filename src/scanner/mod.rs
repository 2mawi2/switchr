use crate::config::Config;
use crate::models::ProjectList;
use anyhow::Result;
use std::sync::Arc;
use std::thread;

pub mod cursor;
pub mod github;
pub mod local;

pub trait ProjectScanner: Send + Sync {
    fn scan(&self, config: &Config) -> Result<ProjectList>;

    fn scanner_name(&self) -> &'static str;
}

pub struct ScanManager {
    scanners: Vec<Box<dyn ProjectScanner + Send + Sync>>,
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
    pub fn new_with_scanners(scanners: Vec<Box<dyn ProjectScanner + Send + Sync>>) -> Self {
        Self { scanners }
    }

    pub fn scan_all_verbose(&self, config: &Config, verbose: bool) -> Result<ProjectList> {
        let config = Arc::new(config.clone());
        let mut handles = Vec::new();

        let scanner_info: Vec<(String, String)> = self
            .scanners
            .iter()
            .map(|scanner| {
                (
                    scanner.scanner_name().to_string(),
                    scanner.scanner_name().to_string(),
                )
            })
            .collect();

        for (scanner_name, _) in scanner_info {
            let config_clone = Arc::clone(&config);
            let scanner_name_clone = scanner_name.clone();

            let handle = thread::spawn(move || {
                let start_time = std::time::Instant::now();

                let result = match scanner_name_clone.as_str() {
                    "local" => local::LocalScanner.scan(&config_clone),
                    "cursor" => cursor::CursorScanner.scan(&config_clone),
                    "github" => github::GitHubScanner.scan(&config_clone),
                    _ => Ok(ProjectList::new()),
                };

                let duration = start_time.elapsed();
                (scanner_name_clone, result, duration)
            });

            handles.push(handle);
        }

        if self
            .scanners
            .iter()
            .any(|s| !matches!(s.scanner_name(), "local" | "cursor" | "github"))
        {
            return self.scan_all_sequential(&config, verbose);
        }

        let mut all_projects = ProjectList::new();

        for handle in handles {
            match handle.join() {
                Ok((scanner_name, result, duration)) => match result {
                    Ok(projects) => {
                        let project_count = projects.len();

                        for project in projects.projects() {
                            all_projects.add_project(project.clone());
                        }

                        if verbose && (duration.as_millis() > 10 || project_count > 0) {
                            eprintln!(
                                "🔍 {} scanner: {} projects in {:.2?}",
                                scanner_name, project_count, duration
                            );
                        }
                    }
                    Err(e) => {
                        if verbose {
                            eprintln!(
                                "Warning: {} scanner failed in {:.2?}: {}",
                                scanner_name, duration, e
                            );
                        } else {
                            eprintln!("Warning: {} scanner failed: {}", scanner_name, e);
                        }
                    }
                },
                Err(_) => {
                    eprintln!("Warning: Scanner thread panicked");
                }
            }
        }

        all_projects.deduplicate();
        all_projects.sort_by_last_modified();
        Ok(all_projects)
    }

    fn scan_all_sequential(&self, config: &Config, verbose: bool) -> Result<ProjectList> {
        let mut all_projects = ProjectList::new();

        for scanner in &self.scanners {
            let scanner_start = std::time::Instant::now();
            match scanner.scan(config) {
                Ok(projects) => {
                    let scanner_duration = scanner_start.elapsed();
                    let project_count = projects.len();

                    for project in projects.projects() {
                        all_projects.add_project(project.clone());
                    }

                    if verbose && (scanner_duration.as_millis() > 10 || project_count > 0) {
                        eprintln!(
                            "🔍 {} scanner: {} projects in {:.2?}",
                            scanner.scanner_name(),
                            project_count,
                            scanner_duration
                        );
                    }
                }
                Err(e) => {
                    let scanner_duration = scanner_start.elapsed();
                    if verbose {
                        eprintln!(
                            "Warning: {} scanner failed in {:.2?}: {}",
                            scanner.scanner_name(),
                            scanner_duration,
                            e
                        );
                    } else {
                        eprintln!("Warning: {} scanner failed: {}", scanner.scanner_name(), e);
                    }
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

    unsafe impl Send for MockScanner {}
    unsafe impl Sync for MockScanner {}

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
            Box::new(scanner1) as Box<dyn ProjectScanner + Send + Sync>,
            Box::new(scanner2) as Box<dyn ProjectScanner + Send + Sync>,
        ]);

        let config = Config::default();
        let result = manager.scan_all_verbose(&config, false).unwrap();

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
            Box::new(good_scanner) as Box<dyn ProjectScanner + Send + Sync>,
            Box::new(bad_scanner) as Box<dyn ProjectScanner + Send + Sync>,
        ]);

        let config = Config::default();
        let result = manager.scan_all_verbose(&config, false).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result.projects()[0].name, "project1");
    }
}
