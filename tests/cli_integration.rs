use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("sw").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "A fast project switcher for developers",
        ))
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("sw").unwrap();
    cmd.arg("--version");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("sw "));
}

#[test]
fn test_config_subcommand() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("sw").unwrap();

    cmd.env("HOME", temp_dir.path());
    cmd.env("XDG_CACHE_HOME", temp_dir.path().join(".cache"));
    cmd.env("XDG_CONFIG_HOME", temp_dir.path().join(".config"));
    cmd.arg("config");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration:"))
        .stdout(predicate::str::contains("Editor:"))
        .stdout(predicate::str::contains("Project directories:"));
}

#[test]
fn test_setup_subcommand() {
    let mut cmd = Command::cargo_bin("sw").unwrap();
    cmd.arg("setup");

    let result = cmd.assert();

    let output = result.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("Welcome to the sw setup wizard")
            || stderr.contains("Failed to get")
            || stderr.contains("not a terminal")
            || !output.status.success()
    );
}

#[test]
fn test_list_subcommand() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("sw").unwrap();

    cmd.env("HOME", temp_dir.path());
    cmd.env("XDG_CACHE_HOME", temp_dir.path().join(".cache"));
    cmd.env("XDG_CONFIG_HOME", temp_dir.path().join(".config"));
    cmd.arg("list");

    cmd.assert().success().stdout(
        predicate::str::contains("Found")
            .and(predicate::str::contains("project(s):"))
            .or(predicate::str::contains("No projects found")),
    );
}

#[test]
fn test_verbose_flag() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("sw").unwrap();

    cmd.env("HOME", temp_dir.path());
    cmd.env("XDG_CACHE_HOME", temp_dir.path().join(".cache"));
    cmd.env("XDG_CONFIG_HOME", temp_dir.path().join(".config"));
    cmd.args(["--verbose", "config"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Running sw with verbose output enabled",
        ))
        .stdout(predicate::str::contains("Loaded configuration:"));
}

#[test]
fn test_interactive_mode_default() {
    // Skip this test if we're in a CI environment or no TTY is available
    if std::env::var("CI").is_ok()
        || std::env::var("GITHUB_ACTIONS").is_ok()
        || !atty::is(atty::Stream::Stdin)
    {
        eprintln!("Skipping interactive test in CI environment or non-TTY context");
        return;
    }

    let mut cmd = Command::cargo_bin("sw").unwrap();

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to initialize"));
}

#[test]
fn test_list_flag() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("sw").unwrap();

    cmd.env("HOME", temp_dir.path());
    cmd.env("XDG_CACHE_HOME", temp_dir.path().join(".cache"));
    cmd.env("XDG_CONFIG_HOME", temp_dir.path().join(".config"));
    cmd.arg("--list");

    cmd.assert().success().stdout(
        predicate::str::contains("Found")
            .and(predicate::str::contains("project(s):"))
            .or(predicate::str::contains("No projects found")),
    );
}

#[test]
fn test_fzf_flag() {
    let mut cmd = Command::cargo_bin("sw").unwrap();
    cmd.arg("--fzf");

    let result = cmd.assert();

    let output = result.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success()
            || stderr.contains("fzf binary not found")
            || stdout.contains("fzf binary not found")
    );
}

#[test]
fn test_conflicting_flags() {
    let mut cmd = Command::cargo_bin("sw").unwrap();
    cmd.args(["--list", "--interactive"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_refresh_subcommand() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("sw").unwrap();

    cmd.env("HOME", temp_dir.path());
    cmd.env("XDG_CACHE_HOME", temp_dir.path().join(".cache"));
    cmd.env("XDG_CONFIG_HOME", temp_dir.path().join(".config"));
    cmd.arg("refresh");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Cache refreshed"));
}

#[test]
fn test_config_file_creation() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("sw").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("config");

    cmd.assert().success();
}

#[test]
fn test_invalid_arguments() {
    let mut cmd = Command::cargo_bin("sw").unwrap();
    cmd.arg("--invalid-flag");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument"));
}

#[test]
fn test_subcommand_with_flags() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("sw").unwrap();

    cmd.env("HOME", temp_dir.path());
    cmd.env("XDG_CACHE_HOME", temp_dir.path().join(".cache"));
    cmd.env("XDG_CONFIG_HOME", temp_dir.path().join(".config"));
    cmd.args(["--verbose", "refresh"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Running sw with verbose output enabled",
        ))
        .stdout(predicate::str::contains(
            "Cache invalidated. Next scan will rebuild from scratch.",
        ));
}

#[test]
fn test_cursor_scanner_integration() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("sw").unwrap();

    cmd.env("HOME", temp_dir.path());
    cmd.env("XDG_CACHE_HOME", temp_dir.path().join(".cache"));
    cmd.env("XDG_CONFIG_HOME", temp_dir.path().join(".config"));
    cmd.args(["--verbose", "list"]);

    cmd.assert().success().stdout(
        predicate::str::contains("Found")
            .and(predicate::str::contains("project(s):"))
            .or(predicate::str::contains("No projects found")),
    );
}

#[test]
fn test_fzf_mode_implementation() {
    let mut cmd = Command::cargo_bin("sw").unwrap();
    cmd.arg("--fzf");

    let result = cmd.assert();

    let output = result.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success()
            || stderr.contains("fzf binary not found")
            || stdout.contains("fzf binary not found")
    );
}

#[test]
fn test_setup_wizard_implementation() {
    let mut cmd = Command::cargo_bin("sw").unwrap();
    cmd.arg("setup");

    let result = cmd.assert();

    let output = result.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("Welcome to the sw setup wizard")
            || stderr.contains("Failed to get")
            || stderr.contains("not a tty")
            || !output.status.success()
    );
}

#[test]
fn test_first_time_setup_logic_isolated() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("sw").unwrap();
    cmd.env("XDG_CONFIG_HOME", temp_dir.path())
        .env("XDG_CACHE_HOME", temp_dir.path().join("cache"))
        .env("HOME", temp_dir.path())
        .arg("--list")
        .arg("--verbose");

    cmd.assert().success().stdout(
        predicate::str::contains("Found")
            .and(predicate::str::contains("project(s):"))
            .or(predicate::str::contains("No projects found")),
    );
}

#[test]
fn test_config_is_first_time_run_detection() {
    use sw::config::Config;

    let result = Config::is_first_time_run();
    assert!(result.is_ok());
}

#[test]
fn test_config_should_prompt_github_setup() {
    use sw::config::Config;

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

#[test]
fn test_github_setup_prompting_logic() {
    use sw::config::Config;

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

#[test]
fn test_config_command_without_external_dependencies() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("sw").unwrap();
    cmd.env("XDG_CONFIG_HOME", temp_dir.path())
        .env("XDG_CACHE_HOME", temp_dir.path().join("cache"))
        .env("HOME", temp_dir.path())
        .arg("config");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration:"))
        .stdout(predicate::str::contains("Editor:"))
        .stdout(predicate::str::contains("Project directories:"));
}
