#!/bin/bash
# install.sh — Install praxis on Linux/macOS
#
# Usage:
#   curl -fsSL https://praxis.dev/install.sh | bash
#   curl -fsSL https://praxis.dev/install.sh | bash -s -- --version v0.1.0
#
# Installs:
#   - praxis binary to /usr/local/bin
#   - systemd service (Linux) to /etc/systemd/system/praxis.service
#   - Default config to ~/.config/praxis/config.toml

set -e

REPO="ivan-cavero/praxis"
VERSION="latest"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="${HOME}/.config/praxis"

# Parse flags
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --install-dir) INSTALL_DIR="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

echo "┌────────────────────────────────────────────┐"
echo "│         praxis — Neural Command Center     │"
echo "└────────────────────────────────────────────┘"
echo ""

# ─── OS/Arch detection ────────────────────────────────────────
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    linux)  PLATFORM="linux" ;;
    darwin) PLATFORM="macos" ;;
    *)      echo "✖ Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
    x86_64)  ARCH_NAME="x86_64" ;;
    aarch64|arm64) ARCH_NAME="aarch64" ;;
    *)       echo "✖ Unsupported architecture: $ARCH"; exit 1 ;;
esac

BINARY_NAME="praxis-${PLATFORM}-${ARCH_NAME}"
if [ "$VERSION" = "latest" ]; then
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${BINARY_NAME}"
else
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${BINARY_NAME}"
fi

echo "  OS:          ${PLATFORM}"
echo "  Architecture: ${ARCH_NAME}"
echo "  Version:      ${VERSION}"
echo "  Binary:       ${INSTALL_DIR}/praxis"
echo ""

# ─── Download ─────────────────────────────────────────────────
echo "  ⇣ Downloading praxis..."
curl -fsSL "$DOWNLOAD_URL" -o /tmp/praxis 2>/dev/null || {
    echo "✖ Download failed. Check: ${DOWNLOAD_URL}"
    exit 1
}

# Verify checksum if available
CHECKSUM_URL="${DOWNLOAD_URL}.sha256"
CHECKSUM=$(curl -fsSL "$CHECKSUM_URL" 2>/dev/null || true)
if [ -n "$CHECKSUM" ]; then
    echo "  ✓ Verifying checksum..."
    COMPUTED=$(sha256sum /tmp/praxis | cut -d' ' -f1)
    if [ "$COMPUTED" != "$CHECKSUM" ]; then
        echo "✖ Checksum mismatch!"
        rm -f /tmp/praxis
        exit 1
    fi
    echo "  ✓ Checksum verified"
fi

chmod +x /tmp/praxis

# Install binary
if [ -w "$INSTALL_DIR" ]; then
    mv /tmp/praxis "${INSTALL_DIR}/praxis"
else
    echo "  ⚡ Escalating privileges to install to ${INSTALL_DIR}..."
    sudo mv /tmp/praxis "${INSTALL_DIR}/praxis"
fi

echo "  ✓ Binary installed to ${INSTALL_DIR}/praxis"

# ─── Create config directory ──────────────────────────────────
mkdir -p "$CONFIG_DIR"

# ─── Default config ──────────────────────────────────────────
CONFIG_FILE="${CONFIG_DIR}/config.toml"
if [ ! -f "$CONFIG_FILE" ]; then
    cat > "$CONFIG_FILE" << 'CONFIG_EOF'
# praxis configuration
# Located at ~/.config/praxis/config.toml

[server]
port = 8080
host = "0.0.0.0"

[auth]
# JWT secret is auto-generated on first run and stored in
# the data directory. You can override it here:
# jwt_secret = "your-256-bit-secret"

[data]
# Directory for projects, sessions, and vault data
dir = "~/.local/share/praxis"

[limits]
max_iterations_per_goal = 50
max_iterations_per_phase = 5
session_ttl_seconds = 3600
phase_timeout_seconds = 300
CONFIG_EOF
    echo "  ✓ Default config created at ${CONFIG_FILE}"
else
    echo "  - Config already exists at ${CONFIG_FILE} (skipped)"
fi

# ─── systemd service (Linux only) ────────────────────────────
if [ "$OS" = "linux" ]; then
    SERVICE_FILE="/etc/systemd/system/praxis.service"
    if [ ! -f "$SERVICE_FILE" ]; then
        echo "  ⚡ Installing systemd service (requires sudo)..."
        sudo tee "$SERVICE_FILE" > /dev/null << 'SERVICE_EOF'
[Unit]
Description=praxis — Neural Command Center
Documentation=https://praxis.dev/docs
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/praxis server
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info
Environment=PRAXIS_CONFIG_DIR=%h/.config/praxis
Environment=PRAXIS_DATA_DIR=%h/.local/share/praxis

[Install]
WantedBy=multi-user.target
SERVICE_EOF
        sudo systemctl daemon-reload 2>/dev/null || true
        echo "  ✓ systemd service installed"
        echo ""
        echo "  ────────────────────────────────────────────"
        echo "  Start praxis server:"
        echo "    sudo systemctl enable --now praxis"
        echo ""
        echo "  View logs:"
        echo "    journalctl -fu praxis"
        echo "  ────────────────────────────────────────────"
    else
        echo "  - systemd service already exists (skipped)"
    fi
fi

# ─── Verify ──────────────────────────────────────────────────
echo ""
echo "  ✓ Verifying installation..."
"${INSTALL_DIR}/praxis" --version 2>/dev/null || {
    echo "  ✖ Binary not executable or not found"
    echo "  Try: ${INSTALL_DIR}/praxis --version"
    exit 1
}

echo ""
echo "┌──────────────────────────────────────────────────────┐"
echo "│  ✓ praxis installed successfully!                    │"
echo "│                                                      │"
echo "│  Quick start:                                        │"
echo "│    praxis server              Start the API server   │"
echo "│    praxis run --goal \"...\"    Run a goal             │"
echo "│    praxis --help              See all commands       │"
echo "└──────────────────────────────────────────────────────┘"
