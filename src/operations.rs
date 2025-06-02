use crate::config::Config;
use crate::opener::ProjectOpener;
use crate::project_manager;
use crate::scanner;
use crate::tui::run_interactive_mode;
use anyhow::{Context, Result};
use clap_complete::{generate, Shell};
use dialoguer::{Confirm, Input};
use std::io;
use std::path::PathBuf;

/// Handle the setup wizard operation
pub fn handle_setup_wizard(config: &Config, verbose: bool) -> Result<()> {
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
            gitlab_username: None,
            cache_ttl_seconds: config.cache_ttl_seconds,
        };

        new_config.save().context("Failed to save configuration")?;
        println!("\n✅ Configuration saved successfully!");
        return Ok(());
    }

    let is_authenticated = scanner::github::is_gh_authenticated().unwrap_or(false);

    let new_config = if is_authenticated {
        println!("✅ GitHub CLI is authenticated");

        let current_username = scanner::github::get_gh_username()
            .unwrap_or_else(|_| config.github_username.as_deref().unwrap_or("").to_string());

        let use_github = Confirm::new()
            .with_prompt(format!(
                "Enable GitHub repository discovery for user '{}'?",
                current_username
            ))
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
            gitlab_username: None,
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

                match scanner::github::get_gh_username() {
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
            gitlab_username: None,
            cache_ttl_seconds: config.cache_ttl_seconds,
        };

        if github_username.is_some() {
            println!("🐙 GitHub integration enabled - your repositories will be discovered automatically");
        }

        config
    };

    println!("\n🦊 GitLab configuration:");

    let gitlab_username = if which::which("glab").is_err() {
        println!("⚠️  GitLab CLI (glab) is not installed.");
        println!("To enable GitLab repository discovery, please install it with:");
        println!("  brew install glab");

        let skip_gitlab = Confirm::new()
            .with_prompt("Continue without GitLab integration?")
            .default(true)
            .interact()
            .context("Failed to get GitLab skip confirmation")?;

        if !skip_gitlab {
            println!("Please install GitLab CLI and run setup again.");
            return Ok(());
        }

        None
    } else {
        let setup_gitlab = Confirm::new()
            .with_prompt("Would you like to configure GitLab integration?")
            .default(false)
            .interact()
            .context("Failed to get GitLab setup confirmation")?;

        if setup_gitlab {
            let gitlab_username_input: String = Input::new()
                .with_prompt("GitLab username")
                .allow_empty(true)
                .interact()
                .context("Failed to get GitLab username input")?;

            let username = if gitlab_username_input.trim().is_empty() {
                None
            } else {
                Some(gitlab_username_input.trim().to_string())
            };

            if username.is_some() {
                println!(
                    "🦊 GitLab integration enabled for user '{}'",
                    username.as_ref().unwrap()
                );
            }

            username
        } else {
            None
        }
    };

    let final_config = Config {
        editor_command: new_config.editor_command,
        project_dirs: new_config.project_dirs,
        github_username: new_config.github_username,
        gitlab_username,
        cache_ttl_seconds: new_config.cache_ttl_seconds,
    };

    final_config
        .save()
        .context("Failed to save configuration")?;
    println!("\n✅ Configuration saved successfully!");

    if let Err(e) = final_config.validate() {
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
        println!("  Editor: {}", final_config.editor_command);
        println!(
            "  Project directories: {} entries",
            final_config.project_dirs.len()
        );
        if let Some(ref username) = final_config.github_username {
            println!("  GitHub username: {}", username);
        }
    }

    println!("\n🎉 Setup complete! You can now use 'sw' to switch between projects.");
    println!("Try running 'sw --list' to see your projects.");

    Ok(())
}

/// Handle showing the current configuration
pub fn handle_show_config(config: &Config, _verbose: bool) -> Result<()> {
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
    } else if scanner::github::is_gh_installed() {
        match scanner::github::is_gh_authenticated() {
            Ok(true) => {
                println!("  GitHub: ⚠️  Authenticated but not configured");
                println!("    💡 Run 'sw setup' to enable GitHub integration");
            }
            Ok(false) => {
                println!("  GitHub: ❌ Not configured");
            }
            Err(e) => {
                println!("  GitHub: ❌ Not configured (error checking auth: {})", e);
            }
        }
    } else {
        println!("  GitHub: ❌ Not configured");
        println!("  GitHub CLI: ❌ Not installed");
    }

    if let Some(ref username) = config.gitlab_username {
        println!("  GitLab username: {}", username);

        if scanner::gitlab::is_glab_installed() {
            if scanner::gitlab::is_glab_accessible() {
                println!("  GitLab status: ✅ Accessible");
            } else {
                println!("  GitLab status: ❌ Not accessible (check VPN/auth)");
            }
        } else {
            println!("  GitLab status: ⚠️  GitLab CLI not installed");
        }
    } else if scanner::gitlab::is_glab_installed() {
        println!("  GitLab: ❌ Not configured");
        println!("    💡 Run 'sw setup' to enable GitLab integration");
    } else {
        println!("  GitLab: ❌ Not configured");
        println!("  GitLab CLI: ❌ Not installed");
    }

    Ok(())
}

/// Handle listing projects
pub fn handle_list_projects(config: &Config, verbose: bool) -> Result<()> {
    let project_list = project_manager::get_projects_with_cache(config, verbose)?;

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

/// Handle refreshing the cache
pub fn handle_refresh_cache(config: &Config, verbose: bool) -> Result<()> {
    if verbose {
        println!("Refreshing project cache...");
    }

    let project_list = project_manager::get_projects_fresh(config, verbose)?;

    println!("Cache refreshed! Found {} projects.", project_list.len());
    Ok(())
}

/// Handle opening a project by name
pub fn handle_open_project_by_name(
    project_name: &str,
    config: &Config,
    verbose: bool,
) -> Result<()> {
    let opener = ProjectOpener::new();

    let projects = project_manager::get_projects_with_cache(config, verbose)?;

    let matching_project = projects
        .projects()
        .iter()
        .find(|p| p.name.to_lowercase().contains(&project_name.to_lowercase()))
        .cloned();

    if let Some(project) = matching_project {
        if verbose {
            println!(
                "Found project: {} at {}",
                project.name,
                project.path.display()
            );
        }

        opener.open_project(&project, config)?;
        println!("Opened project: {}", project.name);
    } else {
        // Try fresh scan if not found in cache
        if verbose {
            println!("Project not found in cache, trying fresh scan...");
        }
        let fresh_projects = project_manager::get_projects_fresh(config, verbose)?;

        let fresh_matching = fresh_projects
            .projects()
            .iter()
            .find(|p| p.name.to_lowercase().contains(&project_name.to_lowercase()))
            .cloned();

        if let Some(project) = fresh_matching {
            if verbose {
                println!(
                    "Found project in fresh scan: {} at {}",
                    project.name,
                    project.path.display()
                );
            }
            opener.open_project(&project, config)?;
            println!("Opened project: {}", project.name);
        } else {
            println!("No project found matching '{}'", project_name);
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Handle interactive mode
pub fn handle_interactive_mode(config: &Config, verbose: bool) -> Result<()> {
    let opener = ProjectOpener::new();

    let projects = project_manager::get_projects_with_cache(config, verbose)?;

    if projects.is_empty() {
        println!(
            "No projects found. Try running with --refresh to rescan or check your configuration."
        );
        return Ok(());
    }

    if verbose {
        println!("Starting interactive mode with {} projects", projects.len());
    }

    if let Some(selected_project) = run_interactive_mode(projects.projects().to_vec())? {
        if verbose {
            println!(
                "Selected project: {} at {}",
                selected_project.name,
                selected_project.path.display()
            );
        }

        opener.open_project(&selected_project, config)?;
        println!("Opened project: {}", selected_project.name);
    } else if verbose {
        println!("No project selected");
    }

    Ok(())
}

/// Handle fzf mode
pub fn handle_fzf_mode(config: &Config, verbose: bool) -> Result<()> {
    use crate::models;
    use std::io::Write;
    use std::process::{Command, Stdio};

    if which::which("fzf").is_err() {
        anyhow::bail!("fzf binary not found. Please install fzf to use this mode.");
    }

    let opener = ProjectOpener::new();

    let projects = project_manager::get_projects_with_cache(config, verbose)?;

    if projects.is_empty() {
        println!(
            "No projects found. Try running with --refresh to rescan or check your configuration."
        );
        return Ok(());
    }

    if verbose {
        println!("Piping {} projects to fzf", projects.len());
    }

    let project_lines: Vec<String> = projects
        .projects()
        .iter()
        .map(|project| {
            let source_indicator = match project.source {
                models::ProjectSource::Local => "📁",
                models::ProjectSource::Cursor => "🎯",
                models::ProjectSource::GitHub => "🐙",
                models::ProjectSource::GitLab => "🦊",
            };

            let time_str = if let Some(timestamp) = project.last_modified {
                format!(" ({})", timestamp.format("%Y-%m-%d %H:%M"))
            } else {
                String::new()
            };

            format!("{} {}{}", source_indicator, project.name, time_str)
        })
        .collect();

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
            writeln!(stdin, "{}", line).context("Failed to write to fzf stdin")?;
        }
    }

    let output = fzf_process
        .wait_with_output()
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

    let selected_project = projects
        .projects()
        .iter()
        .zip(project_lines.iter())
        .find(|(_, line)| **line == selected_line)
        .map(|(project, _)| project)
        .cloned();

    if let Some(project) = selected_project {
        if verbose {
            println!(
                "Selected project: {} at {}",
                project.name,
                project.path.display()
            );
        }

        opener.open_project(&project, config)?;
        println!("Opened project: {}", project.name);
    } else {
        anyhow::bail!("Failed to find selected project");
    }

    Ok(())
}

/// Handle generating shell completions
pub fn handle_generate_completions(shell: Shell, cli_command: &mut clap::Command) -> Result<()> {
    generate(shell, cli_command, "sw", &mut io::stdout());
    Ok(())
}
