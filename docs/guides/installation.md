# Installation Guide

## Quick Install

```bash
# Linux / macOS
curl -fsSL https://project-x.dev/install.sh | bash

# Windows (PowerShell)
iwr -useb https://project-x.dev/install.ps1 | iex
```

## Manual Install

### From GitHub Releases

Download the latest binary for your platform from [GitHub Releases](https://github.com/ivan-cavero/project-x/releases):

| Platform | Binary |
|----------|--------|
| Linux x86_64 | `project-x-linux-x86_64` |
| macOS x86_64 | `project-x-macos-x86_64` |
| macOS ARM64 | `project-x-macos-aarch64` |
| Windows x86_64 | `project-x-windows-x86_64.exe` |

```bash
# Linux
chmod +x project-x-linux-x86_64
sudo mv project-x-linux-x86_64 /usr/local/bin/project-x

# macOS
chmod +x project-x-macos-x86_64
sudo mv project-x-macos-x86_64 /usr/local/bin/project-x
```

### From Source

```bash
# Install Rust nightly
rustup toolchain install nightly

# Clone the repository
git clone https://github.com/ivan-cavero/project-x.git
cd project-x

# Build
cargo build --release

# Install
sudo cp target/release/project-x /usr/local/bin/
```

### Homebrew (macOS)

```bash
brew tap ivan-cavero/tap
brew install project-x
```

## Verify Installation

```bash
project-x --version
# Project-X v0.1.0
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