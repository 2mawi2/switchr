# sw - A fast project switcher for developers

A fast project switcher for developers

## Installation

### Homebrew (Recommended)

```bash
# Add the tap
brew tap 2mawi2/homebrew-tap

# Install sw
brew install sw
```

## Quick Start
This is a Rust project switcher tool. Use [justfile](mdc:justfile) for common development tasks:

```bash
just build    # Build the project
just test     # Run all tests
just run      # Build and run with default args
just dev      # Development mode with cargo watch
```

## Development

```bash
# Run tests
just test

# Build and check
just check

# Run in development mode
just dev
```

## Release

To create a new release:

```bash
just release
```

This will:
- Ensure you're on the main branch
- Pull latest changes
- Create/update the release branch
- Trigger GitHub Actions to build cross-platform binaries
- Auto-increment version and publish to GitHub Releases
- Update Homebrew tap for easy installation

Monitor the release at: https://github.com/2mawi2/switchr/actions

## Usage

### Manual Installation Steps
```bash
# 1. Install the binary to ~/.cargo/bin
cargo install --path .

# 2. Install shell completions (choose your shell)
# For Bash
sw completions bash > ~/.local/share/bash-completion/completions/sw

# For Zsh  
mkdir -p ~/.local/share/zsh/site-functions
sw completions zsh > ~/.local/share/zsh/site-functions/_sw

# For Fish
mkdir -p ~/.config/fish/completions  
sw completions fish > ~/.config/fish/completions/sw.fish
```

### Shell Completion
The tool supports shell completion for all major shells. After installation, you'll have tab completion for:

- All command options (`--list`, `--interactive`, `--fzf`, etc.)
- All subcommands (`setup`, `list`, `refresh`, `config`, `completions`)
- Shell types for the `completions` subcommand (`bash`, `zsh`, `fish`, `powershell`, `elvish`)

Generate completions manually:
```bash
sw completions bash    # Generate bash completions
sw completions zsh     # Generate zsh completions  
sw completions fish    # Generate fish completions
sw completions powershell  # Generate PowerShell completions
```

## GitHub Integration
The tool integrates with GitHub to discover your repositories:

### Prerequisites
- Install GitHub CLI: `brew install gh` (macOS) or equivalent for your OS
- Authenticate: `gh auth login` (run this manually or the tool will prompt you)

The tool will:
1. Check if `gh` is installed (warns if not found)
2. Check if you're authenticated with `gh auth status`
3. Prompt you to run `gh auth login` if not authenticated
4. Use `gh api` to fetch your repositories 

## Project Structure
- [src/main.rs](mdc:src/main.rs) - CLI entry point with clap parsing
- [src/scanner/](mdc:src/scanner) - Project discovery modules (local, cursor, github)
- [src/cache.rs](mdc:src/cache.rs) - TTL-based caching with bincode serialization
- [src/config.rs](mdc:src/config.rs) - Configuration management
- [src/models.rs](mdc:src/models.rs) - Core data structures

## Testing
- Unit tests: `cargo test` (81 tests)
- Integration tests: `cargo test --test integration` (17 tests)
- Watch mode: `just dev` 
