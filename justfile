# Build and test commands for sw project

# Run all tests
test:
    cargo test

# Build the project
build:
    cargo build

# Build optimized release
build-release:
    cargo build --release

# Run clippy linter
lint:
    cargo clippy --all-features --all-targets -- -D warnings

# Format code
fmt:
    cargo fmt

# Run all checks (fmt, lint, test)
check: fmt lint test

# Create a release - triggers GitHub Actions to build and publish
release:
    #!/usr/bin/env bash
    set -e
    
    # Check if we're on main branch
    current_branch=$(git branch --show-current)
    if [ "$current_branch" != "main" ]; then
        echo "Error: Must be on main branch to create a release"
        exit 1
    fi
    
    # Pull latest changes and check working directory is clean
    git pull origin main
    if [ -n "$(git status --porcelain)" ]; then
        echo "Error: Working directory is not clean. Commit or stash changes first."
        exit 1
    fi
    
    # Create or update release branch and push to trigger GitHub Actions
    if git show-ref --verify --quiet refs/heads/release; then
        git checkout release
        git merge main
    else
        git checkout -b release
    fi
    
    git push origin release
    git checkout main
    
    echo "✅ Release triggered! Monitor at: https://github.com/2mawi2/switchr/actions"

# Install binary locally to ~/.cargo/bin
install:
    cargo install --path .

# Uninstall the locally installed binary
uninstall:
    cargo uninstall sw

# Run in development mode
dev:
    cargo run

# Watch and run tests on changes
watch:
    cargo watch -x test

# Clean build artifacts
clean:
    cargo clean

# Generate shell completions
completions shell="bash":
    cargo run -- completions {{shell}}