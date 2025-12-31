# Installation

Install Fabricks on your system.

---

## Quick Install (Recommended)

The fastest way to install Fabricks:

```bash
curl -fsSL https://get.fabricks.dev | sh
```

This script automatically detects your OS and architecture, downloads the appropriate binary, and installs it to `/usr/local/bin`.

---

## Manual Installation

### Download Binary

Download the latest release for your platform:

```bash
# Linux (x86_64)
curl -LO https://github.com/Liquescent-Development/fabricks/releases/latest/download/fabricks-linux-x86_64
chmod +x fabricks-linux-x86_64
sudo mv fabricks-linux-x86_64 /usr/local/bin/fabricks

# Linux (ARM64)
curl -LO https://github.com/Liquescent-Development/fabricks/releases/latest/download/fabricks-linux-aarch64
chmod +x fabricks-linux-aarch64
sudo mv fabricks-linux-aarch64 /usr/local/bin/fabricks

# macOS (Intel)
curl -LO https://github.com/Liquescent-Development/fabricks/releases/latest/download/fabricks-darwin-x86_64
chmod +x fabricks-darwin-x86_64
sudo mv fabricks-darwin-x86_64 /usr/local/bin/fabricks

# macOS (Apple Silicon)
curl -LO https://github.com/Liquescent-Development/fabricks/releases/latest/download/fabricks-darwin-aarch64
chmod +x fabricks-darwin-aarch64
sudo mv fabricks-darwin-aarch64 /usr/local/bin/fabricks
```

### Install from Source

Requires Rust 1.75 or later:

```bash
# Install Rust if needed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Fabricks
cargo install fabricks

# Or build from source
git clone https://github.com/Liquescent-Development/fabricks.git
cd fabricks
cargo build --release
sudo cp target/release/fabricks /usr/local/bin/
```

---

## Verify Installation

```bash
fabricks version
```

Expected output:
```
fabricks 1.0.0
commit: abc123def456
built: 2025-01-15T10:00:00Z
rust: 1.75.0
```

---

## Install the Daemon

The Fabricks daemon (`fabricksd`) is required for multi-service orchestration:

```bash
# Install daemon (included in main package)
sudo fabricks daemon install

# Start the daemon
sudo fabricks daemon start

# Verify daemon is running
fabricks daemon status
```

### Daemon as a System Service

#### systemd (Linux)

```bash
# Install systemd service
sudo fabricks daemon install --systemd

# Enable and start
sudo systemctl enable fabricksd
sudo systemctl start fabricksd

# Check status
sudo systemctl status fabricksd
```

#### launchd (macOS)

```bash
# Install launchd service
sudo fabricks daemon install --launchd

# Load and start
sudo launchctl load /Library/LaunchDaemons/io.fabricks.daemon.plist

# Check status
sudo launchctl list | grep fabricks
```

---

## WASM Toolchain Setup

To build WASM modules from source, install the appropriate toolchain:

### Rust

```bash
# Add WASM targets
rustup target add wasm32-wasi
rustup target add wasm32-unknown-unknown

# Optional: Install wasm-opt for optimization
cargo install wasm-opt
```

### Go

```bash
# Go 1.21+ has built-in WASM support
# Build with:
GOOS=wasip1 GOARCH=wasm go build -o app.wasm
```

### JavaScript/TypeScript

```bash
# Install a WASM-compatible JavaScript runtime
npm install -g wasm-pack
```

---

## Shell Completions

Generate shell completions for your shell:

```bash
# Bash
fabricks completions bash > /etc/bash_completion.d/fabricks

# Zsh
fabricks completions zsh > ~/.zfunc/_fabricks

# Fish
fabricks completions fish > ~/.config/fish/completions/fabricks.fish

# PowerShell
fabricks completions powershell > fabricks.ps1
```

---

## Configuration

Fabricks stores configuration in `~/.fabricks/`:

```
~/.fabricks/
├── config.toml         # User configuration
├── credentials.json    # Registry credentials
├── cache/              # Build cache
└── registry/           # Local OCI storage
```

### Example Configuration

Create `~/.fabricks/config.toml`:

```toml
[registry]
default = "registry.fabricks.io"

[build]
cache_dir = "~/.fabricks/cache"
max_parallel = 4

[runtime]
engine = "wasmtime"

[daemon]
socket = "/var/run/fabricks.sock"
auto_start = true
```

---

## Uninstall

```bash
# Stop and remove daemon
sudo fabricks daemon stop
sudo fabricks daemon uninstall

# Remove binary
sudo rm /usr/local/bin/fabricks

# Remove user data (optional)
rm -rf ~/.fabricks
```

---

## Next Steps

- [Quick Start](quick-start.md) - Create your first fabrick
- [Tutorial](tutorial.md) - Build a complete application
- [CLI Reference](cli-reference.md) - Full command documentation
