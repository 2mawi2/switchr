# Build and test commands for sw project

# Run all tests
test:
    cargo test

# Build the project
build:
    cargo build

# Build optimized release
release:
    cargo build --release

# Run clippy linter
lint:
    cargo clippy -- -D warnings

# Format code
fmt:
    cargo fmt

# Run all checks (fmt, lint, test)
check: fmt lint test

# Check if sw is already installed
check-install:
    @echo "Checking for existing sw installations..."
    @if command -v sw >/dev/null 2>&1; then \
        echo "Found existing sw at: $(which sw)"; \
        echo "Version: $(sw --version 2>/dev/null || echo 'unknown')"; \
    else \
        echo "No existing sw installation found"; \
    fi
    @echo "Cargo bin directory: ~/.cargo/bin"
    @echo "Make sure ~/.cargo/bin is in your PATH"

# Install binary locally to ~/.cargo/bin
install: check-install
    #!/usr/bin/env bash
    echo "Installing sw to ~/.cargo/bin..."
    cargo install --path .
    echo "✅ Installation complete!"
    echo "Verifying installation..."
    if command -v sw >/dev/null 2>&1; then
        echo "✅ sw is now available: $(which sw)"
        echo "Version: $(sw --version)"
    else
        echo "❌ Installation failed or ~/.cargo/bin is not in PATH"
        echo "Add ~/.cargo/bin to your PATH in your shell config"
        exit 1
    fi
    echo "Installing shell completions..."
    CURRENT_SHELL=$(basename "$SHELL" 2>/dev/null || echo "unknown")
    echo "Detected shell: $CURRENT_SHELL"
    case "$CURRENT_SHELL" in
        bash)
            just install-completions-bash
            echo "✅ Bash completions installed"
            ;;
        zsh)
            just install-completions-zsh
            echo "✅ Zsh completions installed"
            ;;
        fish)
            just install-completions-fish
            echo "✅ Fish completions installed"
            ;;
        *)
            echo "Shell $CURRENT_SHELL not auto-detected. Install completions manually:"
            echo "  just install-completions-bash   # for bash"
            echo "  just install-completions-zsh    # for zsh"
            echo "  just install-completions-fish   # for fish"
            ;;
    esac

# Uninstall the locally installed binary
uninstall:
    @echo "Uninstalling sw..."
    cargo uninstall sw
    @echo "✅ sw has been uninstalled"
    @if command -v sw >/dev/null 2>&1; then \
        echo "⚠️  Another sw installation still exists at: $(which sw)"; \
    else \
        echo "✅ No sw command found"; \
    fi

# Run in development mode
dev:
    cargo run

# Watch and run tests on changes
watch:
    cargo watch -x test

# Clean build artifacts
clean:
    cargo clean 

run:
    cargo run

# Generate shell completions
completions shell="bash":
    cargo run -- completions {{shell}}

# Generate and install bash completions
install-completions-bash:
    mkdir -p ~/.local/share/bash-completion/completions
    cargo run -- completions bash > ~/.local/share/bash-completion/completions/sw

# Generate and install zsh completions
install-completions-zsh:
    mkdir -p ~/.local/share/zsh/site-functions
    cargo run -- completions zsh > ~/.local/share/zsh/site-functions/_sw

# Generate and install fish completions
install-completions-fish:
    mkdir -p ~/.config/fish/completions
    cargo run -- completions fish > ~/.config/fish/completions/sw.fish