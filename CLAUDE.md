# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

sw (switchr) is a fast project switcher CLI tool written in Rust that helps developers quickly switch between projects by typing part of the project name. It discovers projects from multiple sources (local Git repos, Cursor workspaces, GitHub, GitLab) and opens them in the preferred editor.

## Development Commands

All development tasks use `just` (justfile):

```bash
# Core development
just build          # Build the project (cargo build)
just test           # Run all tests (cargo test)
just lint           # Run clippy linter
just fmt            # Format code with rustfmt
just check          # Run fmt, lint, and test in sequence
just dev            # Run in development mode
just watch          # Watch and run tests on file changes

# Installation
just install        # Install to ~/.cargo/bin
just build-release  # Build optimized release binary

# Testing specific features
cargo test --test cli_integration  # Run integration tests only
cargo test scanner::               # Run scanner module tests
```

## Architecture

### Core Components

1. **Scanner System** (`src/scanner/`): Trait-based architecture where each scanner implements `ProjectScanner`. The `ScanManager` orchestrates concurrent scanning from multiple sources:
   - `local.rs`: Discovers Git repositories in configured directories
   - `cursor.rs`: Reads Cursor editor workspace configuration
   - `github.rs`: Fetches GitHub repos via `gh` CLI
   - `gitlab.rs`: Fetches GitLab projects via API

2. **Project Management** (`src/project_manager.rs`): Central coordinator that:
   - Manages the TTL-based cache (30 min default)
   - Deduplicates projects from multiple sources
   - Sorts by last modified time

3. **UI Modes** (`src/operations.rs`, `src/tui.rs`):
   - Interactive TUI with real-time fuzzy search (default)
   - List mode for shell completion
   - FZF integration mode
   - Direct project open mode

4. **Configuration** (`src/config.rs`): JSON config at `~/.config/sw/config.json` storing editor preferences, scan directories, and API credentials.

### Key Design Patterns

- **Parallel Scanning**: All scanners run concurrently using threads, results merged in `ScanManager`
- **Caching**: Binary serialization with bincode, stored in platform-specific cache dirs
- **Error Handling**: Uses `anyhow` for application errors, maintains error context throughout
- **Testing**: Integration tests use temp directories and mock configurations

### Performance Considerations

- Target: <200ms with cache, <5s full scan
- Release builds use LTO and single codegen unit
- Efficient binary cache format
- Minimal dependencies for fast compilation

## Common Development Tasks

```bash
# Add a new scanner
# 1. Create new file in src/scanner/
# 2. Implement ProjectScanner trait
# 3. Add to ScanManager in src/scanner/mod.rs

# Test CLI behavior
cargo test --test cli_integration -- --nocapture

# Debug performance
just build-release && time ./target/release/sw --list

# Test specific scanner
SW_LOG=debug cargo run -- --no-cache --list
```