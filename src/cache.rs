use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::io::Write;

use crate::config::Config;
use crate::models::{Project, ProjectList};

#[derive(Debug)]
pub struct Cache {
    cache_dir: PathBuf,
    ttl_seconds: u64,
}

impl Cache {
    pub fn new(config: &Config) -> Result<Self> {
        let cache_dir = Config::cache_dir_path()?;
        
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)
                .with_context(|| format!("Failed to create cache directory: {}", cache_dir.display()))?;
        }

        Ok(Self {
            cache_dir,
            ttl_seconds: config.cache_ttl_seconds,
        })
    }

    pub fn projects_cache_path(&self) -> PathBuf {
        self.cache_dir.join("sw_projects.cache")
    }

    pub fn github_cache_path(&self) -> PathBuf {
        self.cache_dir.join("sw_github.cache")
    }

    pub fn is_cache_valid<P: AsRef<Path>>(&self, cache_path: P) -> bool {
        let path = cache_path.as_ref();
        
        if !path.exists() {
            return false;
        }

        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default();
                    
                    let age_seconds = now.as_secs() - duration.as_secs();
                    return age_seconds < self.ttl_seconds;
                }
            }
        }

        false
    }

    pub fn load_projects(&self) -> Result<Option<ProjectList>> {
        let cache_path = self.projects_cache_path();
        
        if !self.is_cache_valid(&cache_path) {
            return Ok(None);
        }

        let data = fs::read(&cache_path)
            .with_context(|| format!("Failed to read cache file: {}", cache_path.display()))?;

        let projects: Vec<Project> = bincode::deserialize(&data)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize cache: {}", e))?;

        Ok(Some(ProjectList::from_projects(projects)))
    }

    
    fn atomic_write<P: AsRef<Path>>(&self, target_path: P, data: &[u8]) -> Result<()> {
        let target_path = target_path.as_ref();
        
        
        if let Some(parent_dir) = target_path.parent() {
            fs::create_dir_all(parent_dir)
                .with_context(|| format!("Failed to create parent directory: {}", parent_dir.display()))?;
        }
        
        
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        
        let temp_filename = format!(
            "{}.tmp.{}.{}",
            target_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("cache"),
            std::process::id(),
            now.as_nanos()
        );
        
        let temp_path = target_path.with_file_name(temp_filename);
        
        
        {
            let mut temp_file = fs::File::create(&temp_path)
                .with_context(|| format!("Failed to create temporary file: {}", temp_path.display()))?;
            
            temp_file.write_all(data)
                .with_context(|| format!("Failed to write to temporary file: {}", temp_path.display()))?;
            
            
            temp_file.sync_all()
                .with_context(|| format!("Failed to sync temporary file: {}", temp_path.display()))?;
        } 
        
        
        if let Err(e) = fs::rename(&temp_path, target_path) {
            
            let _ = fs::remove_file(&temp_path);
            return Err(e).with_context(|| format!("Failed to rename {} to {}", temp_path.display(), target_path.display()));
        }
        
        Ok(())
    }

    pub fn save_projects(&self, projects: &ProjectList) -> Result<()> {
        let cache_path = self.projects_cache_path();
        
        let data = bincode::serialize(projects.projects()).map_err(|e| anyhow::anyhow!("Failed to serialize cache: {}", e))?;

        
        let mut last_error = None;
        for attempt in 0..3 {
            match self.atomic_write(&cache_path, &data) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < 2 {
                        
                        std::thread::sleep(std::time::Duration::from_millis(1 << attempt));
                    }
                }
            }
        }

        Err(last_error.unwrap())
            .with_context(|| format!("Failed to write cache file after 3 attempts: {}", cache_path.display()))
    }

    #[allow(dead_code)]
    pub fn load_github_projects(&self) -> Result<Option<ProjectList>> {
        let cache_path = self.github_cache_path();
        
        if !self.is_cache_valid(&cache_path) {
            return Ok(None);
        }

        let data = fs::read(&cache_path)
            .with_context(|| format!("Failed to read GitHub cache: {}", cache_path.display()))?;

        let projects: Vec<Project> = bincode::deserialize(&data)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize GitHub cache: {}", e))?;

        Ok(Some(ProjectList::from_projects(projects)))
    }

    #[allow(dead_code)]
    pub fn save_github_projects(&self, projects: &ProjectList) -> Result<()> {
        let cache_path = self.github_cache_path();
        
        let data = bincode::serialize(projects.projects()).map_err(|e| anyhow::anyhow!("Failed to serialize GitHub cache: {}", e))?;

        
        let mut last_error = None;
        for attempt in 0..3 {
            match self.atomic_write(&cache_path, &data) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < 2 {
                        
                        std::thread::sleep(std::time::Duration::from_millis(1 << attempt));
                    }
                }
            }
        }

        Err(last_error.unwrap())
            .with_context(|| format!("Failed to write GitHub cache file after 3 attempts: {}", cache_path.display()))
    }

    pub fn invalidate_all(&self) -> Result<()> {
        let paths = [self.projects_cache_path(), self.github_cache_path()];
        
        for path in &paths {
            if path.exists() {
                fs::remove_file(path)
                    .with_context(|| format!("Failed to remove cache file: {}", path.display()))?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Project, ProjectSource};
    use tempfile::TempDir;
    use std::thread;
    use std::time::Duration;

    fn create_test_config(_cache_dir: &Path) -> Config {
        use crate::config::Config;
        let mut config = Config::default();
        config.cache_ttl_seconds = 1; 
        config
    }

    #[test]
    fn test_cache_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(temp_dir.path());
        
        
        let cache = Cache {
            cache_dir: temp_dir.path().to_path_buf(),
            ttl_seconds: config.cache_ttl_seconds,
        };

        assert!(cache.projects_cache_path().to_string_lossy().contains("sw_projects.cache"));
        assert!(cache.github_cache_path().to_string_lossy().contains("sw_github.cache"));
    }

    #[test]
    fn test_cache_validity() {
        let temp_dir = TempDir::new().unwrap();
        let cache = Cache {
            cache_dir: temp_dir.path().to_path_buf(),
            ttl_seconds: 1,
        };

        let cache_file = temp_dir.path().join("test.cache");
        
        
        assert!(!cache.is_cache_valid(&cache_file));

        
        fs::write(&cache_file, "test").unwrap();
        assert!(cache.is_cache_valid(&cache_file));

        
        thread::sleep(Duration::from_secs(2));
        assert!(!cache.is_cache_valid(&cache_file));
    }

    #[test]
    fn test_project_cache_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let cache = Cache {
            cache_dir: temp_dir.path().to_path_buf(),
            ttl_seconds: 60,
        };

        let mut project_list = ProjectList::new();
        project_list.add_project(Project::new_local("test-project".to_string(), "/test/path"));
        project_list.add_project(Project::new_github(
            "gh-project".to_string(),
            "/gh/path",
            "https://github.com/user/repo".to_string(),
        ));

        
        cache.save_projects(&project_list).unwrap();

        
        let loaded = cache.load_projects().unwrap().unwrap();
        
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.projects()[0].name, "test-project");
        assert_eq!(loaded.projects()[0].source, ProjectSource::Local);
        assert_eq!(loaded.projects()[1].name, "gh-project");
        assert_eq!(loaded.projects()[1].source, ProjectSource::GitHub);
    }

    #[test]
    fn test_cache_invalidation() {
        let temp_dir = TempDir::new().unwrap();
        let cache = Cache {
            cache_dir: temp_dir.path().to_path_buf(),
            ttl_seconds: 60,
        };

        let project_list = ProjectList::new();
        
        
        cache.save_projects(&project_list).unwrap();
        cache.save_github_projects(&project_list).unwrap();

        assert!(cache.projects_cache_path().exists());
        assert!(cache.github_cache_path().exists());

        
        cache.invalidate_all().unwrap();

        assert!(!cache.projects_cache_path().exists());
        assert!(!cache.github_cache_path().exists());
    }

    #[test]
    fn test_atomic_write_basic() {
        let temp_dir = TempDir::new().unwrap();
        let cache = Cache {
            cache_dir: temp_dir.path().to_path_buf(),
            ttl_seconds: 60,
        };

        let test_path = temp_dir.path().join("atomic_test.dat");
        let test_data = b"test data for atomic write";

        
        cache.atomic_write(&test_path, test_data).unwrap();
        
        
        assert!(test_path.exists());
        let read_data = fs::read(&test_path).unwrap();
        assert_eq!(read_data, test_data);

        
        let temp_files: Vec<_> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.file_name()
                    .to_string_lossy()
                    .contains(".tmp.")
            })
            .collect();
        assert!(temp_files.is_empty(), "Temporary files should be cleaned up");
    }

    #[test]
    fn test_atomic_write_overwrites_existing() {
        let temp_dir = TempDir::new().unwrap();
        let cache = Cache {
            cache_dir: temp_dir.path().to_path_buf(),
            ttl_seconds: 60,
        };

        let test_path = temp_dir.path().join("overwrite_test.dat");
        
        
        fs::write(&test_path, b"initial data").unwrap();
        assert_eq!(fs::read(&test_path).unwrap(), b"initial data");

        
        let new_data = b"new data via atomic write";
        cache.atomic_write(&test_path, new_data).unwrap();
        
        
        assert_eq!(fs::read(&test_path).unwrap(), new_data);
    }

    #[test]
    fn test_concurrent_cache_writes() {
        use std::sync::Arc;
        use std::thread;

        let temp_dir = TempDir::new().unwrap();
        
        
        let cache = Arc::new(Cache {
            cache_dir: temp_dir.path().to_path_buf(),
            ttl_seconds: 60,
        });

        
        fs::create_dir_all(&cache.cache_dir).unwrap();

        let mut handles = vec![];
        
        
        for i in 0..3 {
            let cache_clone = Arc::clone(&cache);
            let handle = thread::spawn(move || -> Result<()> {
                let mut project_list = ProjectList::new();
                project_list.add_project(Project::new_local(
                    format!("thread-{}-project", i),
                    format!("/thread/{}/path", i),
                ));
                
                
                for j in 0..2 {
                    project_list.add_project(Project::new_local(
                        format!("thread-{}-project-{}", i, j),
                        format!("/thread/{}/path/{}", i, j),
                    ));
                    if let Err(e) = cache_clone.save_projects(&project_list) {
                        eprintln!("Thread {} iteration {}: {}", i, j, e);
                        return Err(e);
                    }
                    
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Ok(())
            });
            handles.push(handle);
        }

        
        let mut errors = Vec::new();
        for (i, handle) in handles.into_iter().enumerate() {
            match handle.join() {
                Ok(Ok(())) => {}, 
                Ok(Err(e)) => errors.push(format!("Thread {} error: {}", i, e)),
                Err(_) => errors.push(format!("Thread {} panicked", i)),
            }
        }

        
        if !errors.is_empty() {
            eprintln!("Errors during concurrent writes: {:?}", errors);
        }

        
        let cache_path = cache.projects_cache_path();
        assert!(cache_path.exists(), "Cache file should exist after concurrent writes");
        
        
        let loaded_projects = cache.load_projects().unwrap();
        assert!(loaded_projects.is_some(), "Should be able to load cache");
        
        
        let projects = loaded_projects.unwrap();
        assert!(!projects.is_empty(), "Cache should contain projects");
    }
} 