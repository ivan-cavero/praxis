# Installation Guide

## Quick Install

```bash
# Linux / macOS
curl -fsSL https://praxis.dev/install.sh | bash

# Windows (PowerShell)
iwr -useb https://praxis.dev/install.ps1 | iex
```

## Manual Install

### From GitHub Releases

Download the latest binary for your platform from [GitHub Releases](https://github.com/ivan-cavero/praxis/releases):

| Platform | Binary |
|----------|--------|
| Linux x86_64 | `praxis-linux-x86_64` |
| macOS x86_64 | `praxis-macos-x86_64` |
| macOS ARM64 | `praxis-macos-aarch64` |
| Windows x86_64 | `praxis-windows-x86_64.exe` |

```bash
# Linux
chmod +x praxis-linux-x86_64
sudo mv praxis-linux-x86_64 /usr/local/bin/praxis

# macOS
chmod +x praxis-macos-x86_64
sudo mv praxis-macos-x86_64 /usr/local/bin/praxis
```

### From Source

```bash
# Install Rust nightly
rustup toolchain install nightly

# Clone the repository
git clone https://github.com/ivan-cavero/praxis.git
cd praxis

# Build
cargo build --release

# Install
sudo cp target/release/praxis /usr/local/bin/
```

### Homebrew (macOS)

```bash
brew tap ivan-cavero/tap
brew install praxis
```

## Verify Installation

```bash
praxis --version
# praxis v0.1.0
```

## System Requirements

- **OS:** Linux, macOS, or Windows
- **RAM:** 512MB minimum, 2GB recommended
- **Disk:** 100MB for binary + project data
- **Network:** Required for LLM API calls (OpenAI, Anthropic, etc.)

## Next Steps

- [Quick Start Tutorial](./quickstart.md)
- [Configuration Reference](./configuration.md)
- [CLI Reference](./cli.md)