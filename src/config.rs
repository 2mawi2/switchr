use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {

    pub editor_command: String,

    pub project_dirs: Vec<PathBuf>,

    pub github_username: Option<String>,

    pub cache_ttl_seconds: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            editor_command: detect_default_editor(),
            project_dirs: default_project_dirs(),
            github_username: None,
            cache_ttl_seconds: 1800,
        }
    }
}

impl Config {

    pub fn load() -> Result<Self> {
        let config_path = Self::config_file_path()?;
        Self::load_from_path(&config_path)
    }


    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {

            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Self = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }


    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_file_path()?;
        self.save_to_path(&config_path)
    }


    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();


        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }

        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize config")?;

        fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }


    pub fn config_file_path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("", "", "sw")
            .context("Failed to determine config directory")?;

        Ok(project_dirs.config_dir().join("config.json"))
    }


    pub fn cache_dir_path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("", "", "sw")
            .context("Failed to determine cache directory")?;

        Ok(project_dirs.cache_dir().to_path_buf())
    }


    pub fn validate(&self) -> Result<()> {
        if self.editor_command.trim().is_empty() {
            anyhow::bail!("Editor command cannot be empty");
        }

        for dir in &self.project_dirs {
            if !dir.exists() {
                eprintln!("Warning: Project directory does not exist: {}", dir.display());
            }
        }

        if self.cache_ttl_seconds == 0 {
            anyhow::bail!("Cache TTL must be greater than 0");
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn add_project_dir<P: Into<PathBuf>>(&mut self, path: P) {
        let path = path.into();
        if !self.project_dirs.contains(&path) {
            self.project_dirs.push(path);
        }
    }

    #[allow(dead_code)]
    pub fn remove_project_dir<P: AsRef<Path>>(&mut self, path: P) -> bool {
        let path = path.as_ref();
        if let Some(pos) = self.project_dirs.iter().position(|p| p == path) {
            self.project_dirs.remove(pos);
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn set_editor(&mut self, editor: String) {
        self.editor_command = editor;
    }

    /// Check if this is likely the first time the user is running the application.
    /// This is determined by checking if the config file exists.
    pub fn is_first_time_run() -> Result<bool> {
        let config_path = Self::config_file_path()?;
        Ok(!config_path.exists())
    }

    /// Check if GitHub integration needs to be configured
    pub fn should_prompt_github_setup(&self) -> bool {
        self.github_username.is_none()
    }
}


fn detect_default_editor() -> String {

    if let Ok(editor) = std::env::var("EDITOR") {
        return editor;
    }

    if let Ok(visual) = std::env::var("VISUAL") {
        return visual;
    }


    let editors = ["cursor", "code", "vim", "nvim", "nano"];
    for editor in &editors {
        if which::which(editor).is_ok() {
            return editor.to_string();
        }
    }


    "vim".to_string()
}


fn default_project_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(home) = dirs::home_dir() {
        let candidates = [
            "Code",           // VS Code default
            "Projects",       // Common name
            "Documents/git",  // Git convention
            "src",           // Development convention
            "workspace",     // IDE convention
            "Documents/projects", // Alternative location
        ];

        for candidate in &candidates {
            let path = home.join(candidate);
            if path.exists() && path.is_dir() {
                dirs.push(path);
                if dirs.len() >= 2 {
                    break;
                }
            }
        }

        if dirs.is_empty() {
            let fallback_candidates = ["Code", "Projects", "Documents/git"];
            for candidate in &fallback_candidates {
                let path = home.join(candidate);
                if path.exists() {
                    dirs.push(path);
                    break;
                }
            }

            if dirs.is_empty() {
                dirs.push(home.join("Documents/git"));
            }
        }
    }

    dirs
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();

        assert!(!config.editor_command.is_empty());
        assert!(!config.project_dirs.is_empty());
        assert_eq!(config.cache_ttl_seconds, 1800);
        assert!(config.github_username.is_none());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            editor_command: "cursor".to_string(),
            project_dirs: vec![PathBuf::from("/home/user/projects")],
            github_username: Some("testuser".to_string()),
            cache_ttl_seconds: 600,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let original_config = Config {
            editor_command: "test-editor".to_string(),
            project_dirs: vec![PathBuf::from("/test/path")],
            github_username: Some("testuser".to_string()),
            cache_ttl_seconds: 900,
        };


        original_config.save_to_path(&config_path).unwrap();


        let loaded_config = Config::load_from_path(&config_path).unwrap();

        assert_eq!(original_config, loaded_config);
    }

    #[test]
    fn test_load_nonexistent_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.json");

        let config = Config::load_from_path(&config_path).unwrap();


        assert_eq!(config, Config::default());
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();


        config.validate().unwrap();


        config.editor_command = "".to_string();
        assert!(config.validate().is_err());

        config.editor_command = "vim".to_string();


        config.cache_ttl_seconds = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_add_remove_project_dir() {
        let mut config = Config::default();
        let initial_count = config.project_dirs.len();

        let new_dir = PathBuf::from("/new/project/dir");


        config.add_project_dir(&new_dir);
        assert_eq!(config.project_dirs.len(), initial_count + 1);
        assert!(config.project_dirs.contains(&new_dir));


        config.add_project_dir(&new_dir);
        assert_eq!(config.project_dirs.len(), initial_count + 1);


        assert!(config.remove_project_dir(&new_dir));
        assert_eq!(config.project_dirs.len(), initial_count);
        assert!(!config.project_dirs.contains(&new_dir));


        assert!(!config.remove_project_dir(&new_dir));
    }

    #[test]
    fn test_detect_default_editor() {
        let editor = detect_default_editor();
        assert!(!editor.is_empty());
    }

    #[test]
    fn test_default_project_dirs() {
        let dirs = default_project_dirs();
        assert!(!dirs.is_empty());


        for dir in &dirs {
            assert!(dir.is_absolute());
        }
    }

    #[test]
    fn test_config_with_invalid_json() {
        use tempfile::NamedTempFile;
        use std::io::Write;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "invalid json content").unwrap();

        let result = Config::load_from_path(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_is_first_time_run_no_config() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("sw").join("config.json");

        assert!(!config_path.exists());

        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(&config_path, "{}").unwrap();
        assert!(config_path.exists());
    }

    #[test]
    fn test_first_time_run_with_temp_config() {
        use tempfile::NamedTempFile;
        use std::io::Write;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, r#"{{"editor_command": "vim", "project_dirs": [], "github_username": null, "cache_ttl_seconds": 1800}}"#).unwrap();
        let temp_path = temp_file.path();

        let config = Config::load_from_path(temp_path).unwrap();
        assert_eq!(config.editor_command, "vim");
        assert_eq!(config.cache_ttl_seconds, 1800);
    }

    #[test]
    fn test_should_prompt_github_setup() {
        let config_without_github = Config {
            editor_command: "vim".to_string(),
            project_dirs: vec![],
            github_username: None,
            cache_ttl_seconds: 1800,
        };
        assert!(config_without_github.should_prompt_github_setup());

        let config_with_github = Config {
            editor_command: "vim".to_string(),
            project_dirs: vec![],
            github_username: Some("testuser".to_string()),
            cache_ttl_seconds: 1800,
        };
        assert!(!config_with_github.should_prompt_github_setup());
    }
}