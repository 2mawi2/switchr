use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use config::Config;

mod cache;
mod config;
mod models;
mod opener;
mod operations;
mod project_manager;
mod scanner;
mod tui;

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
    let config = Config::load()?;

    let is_first_time = Config::is_first_time_run().unwrap_or(false);
    let should_setup_github = config.should_prompt_github_setup();

    if is_first_time && should_setup_github {
        match cli.operation_mode() {
            OperationMode::Interactive | OperationMode::Fzf => {
                if let Ok(Some(github_username)) = scanner::github::prompt_github_setup() {
                    let updated_config = Config {
                        github_username: Some(github_username),
                        ..config.clone()
                    };
                    if let Err(e) = updated_config.save() {
                        eprintln!("Warning: Failed to save GitHub configuration: {}", e);
                    }
                    println!(); // Add some spacing
                }
            }
            OperationMode::List | OperationMode::ShowConfig => {
                if cli.verbose {
                    println!("💡 Tip: Run 'sw setup' to configure GitHub integration for repository discovery");
                }
            }
            _ => {}
        }
    }

    if cli.verbose {
        println!("Running sw with verbose output enabled");
    }

    config.validate()?;

    if cli.verbose {
        println!(
            "Loaded configuration: editor={}, dirs={}",
            config.editor_command,
            config.project_dirs.len()
        );
    }

    match cli.operation_mode() {
        OperationMode::Setup => operations::handle_setup_wizard(&config, cli.verbose),
        OperationMode::ShowConfig => operations::handle_show_config(&config, cli.verbose),
        OperationMode::List => operations::handle_list_projects(&config, cli.verbose),
        OperationMode::Interactive => operations::handle_interactive_mode(&config, cli.verbose),
        OperationMode::Fzf => operations::handle_fzf_mode(&config, cli.verbose),
        OperationMode::Refresh => operations::handle_refresh_cache(&config, cli.verbose),
        OperationMode::Direct(project_name) => {
            operations::handle_open_project_by_name(&project_name, &config, cli.verbose)
        }
        OperationMode::Completions(shell) => {
            let mut cmd = Cli::command();
            operations::handle_generate_completions(shell, &mut cmd)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_basic_parsing() {
        let cli = Cli::try_parse_from(["sw"]).unwrap();

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
        let cli = Cli::try_parse_from(["sw", "my-project"]).unwrap();

        assert_eq!(cli.project_name, Some("my-project".to_string()));
        assert_eq!(
            cli.operation_mode(),
            OperationMode::Direct("my-project".to_string())
        );
    }

    #[test]
    fn test_cli_flags() {
        let cli = Cli::try_parse_from(["sw", "--list", "--verbose"]).unwrap();

        assert!(cli.list);
        assert!(cli.verbose);
        assert_eq!(cli.operation_mode(), OperationMode::List);
    }

    #[test]
    fn test_cli_interactive_flag() {
        let cli = Cli::try_parse_from(["sw", "--interactive"]).unwrap();

        assert!(cli.interactive);
        assert_eq!(cli.operation_mode(), OperationMode::Interactive);
    }

    #[test]
    fn test_cli_fzf_flag() {
        let cli = Cli::try_parse_from(["sw", "--fzf"]).unwrap();

        assert!(cli.fzf);
        assert_eq!(cli.operation_mode(), OperationMode::Fzf);
    }

    #[test]
    fn test_cli_subcommands() {
        let cli = Cli::try_parse_from(["sw", "setup"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Setup)));
        assert_eq!(cli.operation_mode(), OperationMode::Setup);

        let cli = Cli::try_parse_from(["sw", "list"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::List)));
        assert_eq!(cli.operation_mode(), OperationMode::List);

        let cli = Cli::try_parse_from(["sw", "refresh"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Refresh)));
        assert_eq!(cli.operation_mode(), OperationMode::Refresh);

        let cli = Cli::try_parse_from(["sw", "config"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Config)));
        assert_eq!(cli.operation_mode(), OperationMode::ShowConfig);
    }

    #[test]
    fn test_cli_conflicting_flags() {
        assert!(Cli::try_parse_from(["sw", "--list", "--interactive"]).is_err());
        assert!(Cli::try_parse_from(["sw", "--list", "--fzf"]).is_err());
        assert!(Cli::try_parse_from(["sw", "--interactive", "--fzf"]).is_err());
    }

    #[test]
    fn test_operation_mode_defaults() {
        let cli = Cli::try_parse_from(["sw"]).unwrap();
        assert_eq!(cli.operation_mode(), OperationMode::Interactive);
    }

    #[test]
    fn test_operation_mode_precedence() {
        let cli = Cli::try_parse_from(["sw", "project-name"]).unwrap();
        assert_eq!(
            cli.operation_mode(),
            OperationMode::Direct("project-name".to_string())
        );

        let cli = Cli::try_parse_from(["sw", "setup"]).unwrap();
        assert_eq!(cli.operation_mode(), OperationMode::Setup);
    }

    #[test]
    fn test_cli_completions_subcommand() {
        let cli = Cli::try_parse_from(["sw", "completions", "bash"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Completions { shell: Shell::Bash })
        ));
        assert_eq!(
            cli.operation_mode(),
            OperationMode::Completions(Shell::Bash)
        );

        let cli = Cli::try_parse_from(["sw", "completions", "zsh"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Completions { shell: Shell::Zsh })
        ));
        assert_eq!(cli.operation_mode(), OperationMode::Completions(Shell::Zsh));

        let cli = Cli::try_parse_from(["sw", "completions", "fish"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Completions { shell: Shell::Fish })
        ));
        assert_eq!(
            cli.operation_mode(),
            OperationMode::Completions(Shell::Fish)
        );

        let cli = Cli::try_parse_from(["sw", "completions", "powershell"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Completions {
                shell: Shell::PowerShell
            })
        ));
        assert_eq!(
            cli.operation_mode(),
            OperationMode::Completions(Shell::PowerShell)
        );
    }
}
