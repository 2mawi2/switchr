use anyhow::{Context, Result};
use clap::{Parser, Subcommand, CommandFactory};
use clap_complete::{generate, Shell};
use std::io;

mod models;
mod config;
mod cache;
mod scanner;
mod opener;
mod tui;

use config::Config;
use cache::Cache;
use scanner::ScanManager;
use opener::ProjectOpener;
use tui::run_interactive_mode;

#[derive(Parser)]
#[command(name = "sw")]
#[command(about = "A fast project switcher for developers")]
#[command(version)]
pub struct Cli {
    #[arg(value_name = "PROJECT")]
    pub project_name: Option<String>,

    #[arg(long, short, conflicts_with_all = ["list", "fzf"])]
    pub interactive: bool,

    #[arg(long, short, conflicts_with_all = ["interactive", "fzf"])]
    pub list: bool,

    #[arg(long, conflicts_with_all = ["interactive", "list"])]
    pub fzf: bool,

    #[arg(long, short)]
    pub refresh: bool,

    #[arg(long, short)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Setup,
    List,
    Refresh,
    Config,

    Completions {

        #[arg(value_enum)]
        shell: Shell,
    },
}

impl Cli {
    pub fn operation_mode(&self) -> OperationMode {
        if let Some(ref project_name) = self.project_name {
            return OperationMode::Direct(project_name.clone());
        }

        match &self.command {
            Some(Commands::Setup) => OperationMode::Setup,
            Some(Commands::List) => OperationMode::List,
            Some(Commands::Refresh) => OperationMode::Refresh,
            Some(Commands::Config) => OperationMode::ShowConfig,
            Some(Commands::Completions { shell }) => OperationMode::Completions(*shell),
            None => {
                if self.list {
                    OperationMode::List
                } else if self.fzf {
                    OperationMode::Fzf
                } else {
                    OperationMode::Interactive
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum OperationMode {
    Direct(String),
    Interactive,
    List,
    Fzf,
    Setup,
    Refresh,
    ShowConfig,
    Completions(Shell),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        println!("Running sw with verbose output enabled");
    }

    let config = Config::load()?;
    config.validate()?;

    if cli.verbose {
        println!("Loaded configuration: editor={}, dirs={}",
                config.editor_command, config.project_dirs.len());
    }

    match cli.operation_mode() {
        OperationMode::Setup => {
            handle_setup_wizard(&config, cli.verbose)
        }
        OperationMode::ShowConfig => {
            println!("Configuration:");
            println!("  Editor: {}", config.editor_command);
            println!("  Project directories:");
            for dir in &config.project_dirs {
                println!("    {}", dir.display());
            }
            println!("  Cache TTL: {} seconds", config.cache_ttl_seconds);


            if let Some(ref username) = config.github_username {
                println!("  GitHub username: {}", username);


                if scanner::github::is_gh_installed() {
                    match scanner::github::is_gh_authenticated() {
                        Ok(true) => println!("  GitHub status: ✅ Authenticated"),
                        Ok(false) => println!("  GitHub status: ❌ Not authenticated"),
                        Err(e) => println!("  GitHub status: ⚠️  Error checking status: {}", e),
                    }
                } else {
                    println!("  GitHub status: ⚠️  GitHub CLI not installed");
                }
            } else {
                println!("  GitHub: ❌ Not configured");
                if !scanner::github::is_gh_installed() {
                    println!("  GitHub CLI: ❌ Not installed");
                }
            }

            Ok(())
        }
        OperationMode::List => {
            list_projects(&config, cli.verbose)
        }
        OperationMode::Interactive => {
            handle_interactive_mode(&config, cli.verbose)
        }
        OperationMode::Fzf => {
            handle_fzf_mode(&config, cli.verbose)
        }
        OperationMode::Refresh => {
            refresh_cache(&config, cli.verbose)
        }
        OperationMode::Direct(project_name) => {
            open_project_by_name(&project_name, &config, cli.verbose)
        }
        OperationMode::Completions(shell) => {
            generate_completions(shell)
        }
    }
}

fn list_projects(config: &Config, verbose: bool) -> Result<()> {
    let cache = Cache::new(config)?;
    let scan_manager = ScanManager::new();

    let cached_projects = cache.load_projects()?;
    let should_scan = cached_projects.is_none() || !cache.is_cache_valid(cache.projects_cache_path());

    if let Some(ref cached) = cached_projects {
        if !should_scan {
            if verbose {
                println!("Using cached projects");
            }

            if cached.is_empty() {
                println!("No projects found in configured directories:");
                for dir in &config.project_dirs {
                    println!("  {}", dir.display());
                }
                return Ok(());
            }

            println!("Found {} project(s):", cached.len());
            for project in cached.projects() {
                println!("  {}", project.display_string());
            }
            return Ok(());
        } else if verbose {
            println!("Cache is stale, refreshing...");
        }
    } else if verbose {
        println!("Cache miss, scanning for projects...");
    }

    let scan_start = std::time::Instant::now();
    let project_list = scan_manager.scan_all_verbose(config, verbose)?;
    let scan_duration = scan_start.elapsed();
    
    cache.save_projects(&project_list)?;

    if verbose {
        println!("Found {} projects in {:.2?}", project_list.len(), scan_duration);
    }

    if project_list.is_empty() {
        println!("No projects found in configured directories:");
        for dir in &config.project_dirs {
            println!("  {}", dir.display());
        }
        return Ok(());
    }

    println!("Found {} project(s):", project_list.len());
    for project in project_list.projects() {
        println!("  {}", project.display_string());
    }

    Ok(())
}

fn refresh_cache(config: &Config, verbose: bool) -> Result<()> {
    let cache = Cache::new(config)?;

    if verbose {
        println!("Invalidating cache...");
    }

    cache.invalidate_all()?;

    if verbose {
        println!("Cache invalidated. Next scan will rebuild from scratch.");
    } else {
        println!("Cache refreshed");
    }

    Ok(())
}

fn open_project_by_name(project_name: &str, config: &Config, verbose: bool) -> Result<()> {
    let cache = Cache::new(config)?;
    let scan_manager = ScanManager::new();
    let opener = ProjectOpener::new();

    let projects = if let Some(cached_projects) = cache.load_projects()? {
        if verbose {
            println!("Searching in cached projects");
        }
        cached_projects.projects().to_vec()
    } else {
        if verbose {
            println!("No cached projects found, scanning...");
        }
        let project_list = scan_manager.scan_all_verbose(config, verbose)?;
        cache.save_projects(&project_list)?;
        project_list.projects().to_vec()
    };

    let matching_project = projects.iter()
        .find(|p| p.name.to_lowercase().contains(&project_name.to_lowercase()))
        .cloned();

    if let Some(project) = matching_project {
        if verbose {
            println!("Found project: {} at {}", project.name, project.path.display());
        }

        opener.open_project(&project, config)?;
        println!("Opened project: {}", project.name);

        if !cache.is_cache_valid(cache.projects_cache_path()) && verbose {
            println!("Refreshing project cache in background...");
        }
    } else {
        if !cache.is_cache_valid(cache.projects_cache_path()) {
            if verbose {
                println!("Project not found in cache, trying fresh scan...");
            }
            let fresh_projects = scan_manager.scan_all_verbose(config, verbose)?;
            cache.save_projects(&fresh_projects)?;

            let fresh_matching = fresh_projects.projects().iter()
                .find(|p| p.name.to_lowercase().contains(&project_name.to_lowercase()))
                .cloned();

            if let Some(project) = fresh_matching {
                if verbose {
                    println!("Found project in fresh scan: {} at {}", project.name, project.path.display());
                }
                opener.open_project(&project, config)?;
                println!("Opened project: {}", project.name);
                return Ok(());
            }
        }

        println!("No project found matching '{}'", project_name);
        std::process::exit(1);
    }

    Ok(())
}

fn handle_interactive_mode(config: &Config, verbose: bool) -> Result<()> {
    let cache = Cache::new(config)?;
    let scan_manager = ScanManager::new();
    let opener = ProjectOpener::new();

    let mut projects = if let Some(cached_projects) = cache.load_projects()? {
        if verbose {
            println!("Starting with cached projects");
        }
        cached_projects.projects().to_vec()
    } else {
        if verbose {
            println!("No cached projects found, scanning...");
        }
        let project_list = scan_manager.scan_all_verbose(config, verbose)?;
        cache.save_projects(&project_list)?;
        project_list.projects().to_vec()
    };

    if !cache.is_cache_valid(cache.projects_cache_path()) {
        if verbose {
            println!("Cache is stale, refreshing...");
        }
        let fresh_projects = scan_manager.scan_all_verbose(config, verbose)?;
        cache.save_projects(&fresh_projects)?;
        projects = fresh_projects.projects().to_vec();
    }

    if projects.is_empty() {
        println!("No projects found. Try running with --refresh to rescan or check your configuration.");
        return Ok(());
    }

    if verbose {
        println!("Starting interactive mode with {} projects", projects.len());
    }

    if let Some(selected_project) = run_interactive_mode(projects)? {
        if verbose {
            println!("Selected project: {} at {}", selected_project.name, selected_project.path.display());
        }

        opener.open_project(&selected_project, config)?;
        println!("Opened project: {}", selected_project.name);
    } else if verbose {
        println!("No project selected");
    }

    Ok(())
}

fn handle_fzf_mode(config: &Config, verbose: bool) -> Result<()> {
    use std::process::{Command, Stdio};
    use std::io::Write;

    if which::which("fzf").is_err() {
        anyhow::bail!("fzf binary not found. Please install fzf to use this mode.");
    }

    let cache = Cache::new(config)?;
    let scan_manager = ScanManager::new();
    let opener = ProjectOpener::new();

    let mut projects = if let Some(cached_projects) = cache.load_projects()? {
        if verbose {
            println!("Using cached projects for fzf");
        }
        cached_projects.projects().to_vec()
    } else {
        if verbose {
            println!("No cached projects found, scanning...");
        }
        let project_list = scan_manager.scan_all_verbose(config, verbose)?;
        cache.save_projects(&project_list)?;
        project_list.projects().to_vec()
    };

    if !cache.is_cache_valid(cache.projects_cache_path()) {
        if verbose {
            println!("Refreshing project list...");
        }
        let fresh_projects = scan_manager.scan_all_verbose(config, verbose)?;
        cache.save_projects(&fresh_projects)?;
        projects = fresh_projects.projects().to_vec();
    }

    if projects.is_empty() {
        println!("No projects found. Try running with --refresh to rescan or check your configuration.");
        return Ok(());
    }

    if verbose {
        println!("Piping {} projects to fzf", projects.len());
    }


    let project_lines: Vec<String> = projects.iter().map(|project| {
        let source_indicator = match project.source {
            models::ProjectSource::Local => "📁",
            models::ProjectSource::Cursor => "🎯",
            models::ProjectSource::GitHub => "🐙",
        };

        let time_str = if let Some(timestamp) = project.last_modified {
            format!(" ({})", timestamp.format("%Y-%m-%d %H:%M"))
        } else {
            String::new()
        };

        format!("{} {}{}", source_indicator, project.name, time_str)
    }).collect();


    let mut fzf_process = Command::new("fzf")
        .arg("--prompt=Select project: ")
        .arg("--height=40%")
        .arg("--reverse")
        .arg("--border")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn fzf process")?;


    if let Some(stdin) = fzf_process.stdin.as_mut() {
        for line in &project_lines {
            writeln!(stdin, "{}", line)
                .context("Failed to write to fzf stdin")?;
        }
    }


    let output = fzf_process.wait_with_output()
        .context("Failed to wait for fzf process")?;

    if !output.status.success() {

        if verbose {
            println!("fzf cancelled or failed");
        }
        return Ok(());
    }

    let selected_line = String::from_utf8(output.stdout)
        .context("Failed to parse fzf output")?
        .trim()
        .to_string();

    if selected_line.is_empty() {
        if verbose {
            println!("No project selected");
        }
        return Ok(());
    }


    let selected_project = projects.iter()
        .zip(project_lines.iter())
        .find(|(_, line)| **line == selected_line)
        .map(|(project, _)| project)
        .cloned();

    if let Some(project) = selected_project {
        if verbose {
            println!("Selected project: {} at {}", project.name, project.path.display());
        }

        opener.open_project(&project, config)?;
        println!("Opened project: {}", project.name);
    } else {
        anyhow::bail!("Failed to find selected project");
    }

    Ok(())
}

fn handle_setup_wizard(config: &Config, verbose: bool) -> Result<()> {
    use dialoguer::{Input, Confirm};
    use std::path::PathBuf;

    println!("🚀 Welcome to the sw setup wizard!");
    println!("This will help you configure your project switcher.\n");

    if verbose {
        println!("Current configuration will be used as defaults");
    }


    let editor_command: String = Input::new()
        .with_prompt("Editor command")
        .default(config.editor_command.clone())
        .interact()
        .context("Failed to get editor command input")?;


    println!("\n📁 Project directories configuration:");
    println!("Current directories: {:?}", config.project_dirs);

    let add_more_dirs = Confirm::new()
        .with_prompt("Would you like to add more project directories?")
        .default(false)
        .interact()
        .context("Failed to get directory confirmation")?;

    let mut project_dirs = config.project_dirs.clone();

    if add_more_dirs {
        loop {
            let dir_input: String = Input::new()
                .with_prompt("Enter project directory path (or press Enter to finish)")
                .allow_empty(true)
                .interact()
                .context("Failed to get directory input")?;

            if dir_input.trim().is_empty() {
                break;
            }

            let path = PathBuf::from(dir_input.trim());
            if path.exists() {
                project_dirs.push(path);
                println!("✅ Added directory");
            } else {
                println!("⚠️  Directory does not exist, but added anyway");
                project_dirs.push(path);
            }
        }
    }


    println!("\n🐙 GitHub configuration:");


    if which::which("gh").is_err() {
        println!("⚠️  GitHub CLI (gh) is not installed.");
        println!("To enable GitHub repository discovery, please install it with:");
        println!("  brew install gh");

        let skip_github = Confirm::new()
            .with_prompt("Continue without GitHub integration?")
            .default(true)
            .interact()
            .context("Failed to get GitHub skip confirmation")?;

        if !skip_github {
            println!("Please install GitHub CLI and run setup again.");
            return Ok(());
        }


        let new_config = Config {
            editor_command,
            project_dirs,
            github_username: None,
            cache_ttl_seconds: config.cache_ttl_seconds,
        };

        new_config.save().context("Failed to save configuration")?;
        println!("\n✅ Configuration saved successfully!");
        return Ok(());
    }


    let is_authenticated = scanner::github::is_gh_authenticated()
        .unwrap_or(false);

    let new_config = if is_authenticated {
        println!("✅ GitHub CLI is authenticated");


        let current_username = get_gh_username().unwrap_or_else(|_|
            config.github_username.as_deref().unwrap_or("").to_string()
        );

        let use_github = Confirm::new()
            .with_prompt(format!("Enable GitHub repository discovery for user '{}'?", current_username))
            .default(true)
            .interact()
            .context("Failed to get GitHub usage confirmation")?;

        let github_username = if use_github {
            Some(current_username)
        } else {
            None
        };


        let config = Config {
            editor_command,
            project_dirs,
            github_username: github_username.clone(),
            cache_ttl_seconds: config.cache_ttl_seconds,
        };

        if use_github {
            println!("🐙 GitHub integration enabled - your repositories will be discovered automatically");
        }

        config

    } else {
        println!("❌ GitHub CLI is not authenticated");

        let setup_github = Confirm::new()
            .with_prompt("Would you like to authenticate with GitHub now?")
            .default(true)
            .interact()
            .context("Failed to get GitHub authentication confirmation")?;

        let github_username = if setup_github {
            println!("\n🔐 Starting GitHub authentication...");

            if scanner::github::run_gh_auth_login()? {
                println!("✅ GitHub authentication successful!");


                match get_gh_username() {
                    Ok(username) => {
                        println!("📝 Authenticated as: {}", username);
                        Some(username)
                    }
                    Err(e) => {
                        println!("⚠️  Could not determine GitHub username: {}", e);
                        let manual_username: String = Input::new()
                            .with_prompt("Please enter your GitHub username")
                            .allow_empty(true)
                            .interact()
                            .context("Failed to get manual GitHub username")?;

                        if manual_username.trim().is_empty() {
                            None
                        } else {
                            Some(manual_username.trim().to_string())
                        }
                    }
                }
            } else {
                println!("❌ GitHub authentication failed or cancelled");
                None
            }
        } else {
            None
        };


        let config = Config {
            editor_command,
            project_dirs,
            github_username: github_username.clone(),
            cache_ttl_seconds: config.cache_ttl_seconds,
        };

        if github_username.is_some() {
            println!("🐙 GitHub integration enabled - your repositories will be discovered automatically");
        }

        config
    };


    new_config.save().context("Failed to save configuration")?;
    println!("\n✅ Configuration saved successfully!");


    if let Err(e) = new_config.validate() {
        println!("⚠️  Configuration validation failed: {}", e);
        let continue_anyway = Confirm::new()
            .with_prompt("Save configuration anyway?")
            .default(false)
            .interact()
            .context("Failed to get validation confirmation")?;

        if !continue_anyway {
            println!("Setup cancelled.");
            return Ok(());
        }
    }

    println!("📝 Configuration file: {:?}", Config::config_file_path()?);

    if verbose {
        println!("\nNew configuration:");
        println!("  Editor: {}", new_config.editor_command);
        println!("  Project directories: {} entries", new_config.project_dirs.len());
        if let Some(ref username) = new_config.github_username {
            println!("  GitHub username: {}", username);
        }
    }

    println!("\n🎉 Setup complete! You can now use 'sw' to switch between projects.");
    println!("Try running 'sw --list' to see your projects.");

    Ok(())
}

fn get_gh_username() -> Result<String> {
    use std::process::Command;

    let output = Command::new("gh")
        .args(["api", "user", "--jq", ".login"])
        .output()
        .context("Failed to get GitHub username from gh CLI")?;

    if !output.status.success() {
        anyhow::bail!("Failed to get GitHub username: {}", String::from_utf8_lossy(&output.stderr));
    }

    let username = String::from_utf8(output.stdout)
        .context("Failed to parse GitHub username")?
        .trim()
        .to_string();

    if username.is_empty() {
        anyhow::bail!("Empty username returned from GitHub API");
    }

    Ok(username)
}

fn generate_completions(shell: Shell) -> Result<()> {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "sw", &mut io::stdout());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_basic_parsing() {
        let cli = Cli::try_parse_from(&["sw"]).unwrap();

        assert!(cli.project_name.is_none());
        assert!(!cli.interactive);
        assert!(!cli.list);
        assert!(!cli.fzf);
        assert!(!cli.refresh);
        assert!(!cli.verbose);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_cli_project_name() {
        let cli = Cli::try_parse_from(&["sw", "my-project"]).unwrap();

        assert_eq!(cli.project_name, Some("my-project".to_string()));
        assert_eq!(cli.operation_mode(), OperationMode::Direct("my-project".to_string()));
    }

    #[test]
    fn test_cli_flags() {
        let cli = Cli::try_parse_from(&["sw", "--list", "--verbose"]).unwrap();

        assert!(cli.list);
        assert!(cli.verbose);
        assert_eq!(cli.operation_mode(), OperationMode::List);
    }

    #[test]
    fn test_cli_interactive_flag() {
        let cli = Cli::try_parse_from(&["sw", "--interactive"]).unwrap();

        assert!(cli.interactive);
        assert_eq!(cli.operation_mode(), OperationMode::Interactive);
    }

    #[test]
    fn test_cli_fzf_flag() {
        let cli = Cli::try_parse_from(&["sw", "--fzf"]).unwrap();

        assert!(cli.fzf);
        assert_eq!(cli.operation_mode(), OperationMode::Fzf);
    }

    #[test]
    fn test_cli_subcommands() {
        let cli = Cli::try_parse_from(&["sw", "setup"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Setup)));
        assert_eq!(cli.operation_mode(), OperationMode::Setup);

        let cli = Cli::try_parse_from(&["sw", "list"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::List)));
        assert_eq!(cli.operation_mode(), OperationMode::List);

        let cli = Cli::try_parse_from(&["sw", "refresh"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Refresh)));
        assert_eq!(cli.operation_mode(), OperationMode::Refresh);

        let cli = Cli::try_parse_from(&["sw", "config"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Config)));
        assert_eq!(cli.operation_mode(), OperationMode::ShowConfig);
    }

    #[test]
    fn test_cli_conflicting_flags() {
        assert!(Cli::try_parse_from(&["sw", "--list", "--interactive"]).is_err());
        assert!(Cli::try_parse_from(&["sw", "--list", "--fzf"]).is_err());
        assert!(Cli::try_parse_from(&["sw", "--interactive", "--fzf"]).is_err());
    }

    #[test]
    fn test_operation_mode_defaults() {
        let cli = Cli::try_parse_from(&["sw"]).unwrap();
        assert_eq!(cli.operation_mode(), OperationMode::Interactive);
    }

    #[test]
    fn test_operation_mode_precedence() {
        let cli = Cli::try_parse_from(&["sw", "project-name"]).unwrap();
        assert_eq!(cli.operation_mode(), OperationMode::Direct("project-name".to_string()));

        let cli = Cli::try_parse_from(&["sw", "setup"]).unwrap();
        assert_eq!(cli.operation_mode(), OperationMode::Setup);
    }

    #[test]
    fn test_cli_completions_subcommand() {
        let cli = Cli::try_parse_from(&["sw", "completions", "bash"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Completions { shell: Shell::Bash })));
        assert_eq!(cli.operation_mode(), OperationMode::Completions(Shell::Bash));

        let cli = Cli::try_parse_from(&["sw", "completions", "zsh"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Completions { shell: Shell::Zsh })));
        assert_eq!(cli.operation_mode(), OperationMode::Completions(Shell::Zsh));

        let cli = Cli::try_parse_from(&["sw", "completions", "fish"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Completions { shell: Shell::Fish })));
        assert_eq!(cli.operation_mode(), OperationMode::Completions(Shell::Fish));

        let cli = Cli::try_parse_from(&["sw", "completions", "powershell"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Completions { shell: Shell::PowerShell })));
        assert_eq!(cli.operation_mode(), OperationMode::Completions(Shell::PowerShell));
    }
}