#!/usr/bin/env bash
# install.sh — Install praxis CLI from GitHub Releases
# Usage: curl -fsSL https://raw.githubusercontent.com/ivan-cavero/praxis/main/scripts/install.sh | bash
#
# Environment variables:
#   PRAXIS_INSTALL_DIR  — install directory (default: /usr/local/bin or ~/.local/bin)
#   PRAXIS_VERSION      — specific version tag (default: latest)
#   PRAXIS_FORCE        — overwrite existing binary (default: no)

set -euo pipefail

# ─── Configuration ────────────────────────────────────────────────
OWNER="ivan-cavero"
REPO="praxis"
RELEASE_BASE="https://github.com/${OWNER}/${REPO}/releases"
INSTALL_DIR="${PRAXIS_INSTALL_DIR:-}"
VERSION="${PRAXIS_VERSION:-}"
FORCE="${PRAXIS_FORCE:-}"

# ─── Helpers ──────────────────────────────────────────────────────
info()  { echo -e "\033[1;34m→\033[0m $*"; }
warn()  { echo -e "\033[1;33m⚠\033[0m $*" >&2; }
error() { echo -e "\033[1;31m✗\033[0m $*" >&2; exit 1; }

# ─── Detect OS and architecture ───────────────────────────────────
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux*)  os="linux" ;;
        Darwin*) os="macos" ;;
        *)       error "Unsupported OS: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64)  arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *)       error "Unsupported architecture: $(uname -m)" ;;
    esac

    echo "${os}-${arch}"
}

# ─── Resolve version ──────────────────────────────────────────────
resolve_version() {
    if [[ -n "$VERSION" ]]; then
        echo "$VERSION"
        return
    fi

    info "Fetching latest release..."
    local latest
    latest=$(curl -fsSL "https://api.github.com/repos/${OWNER}/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed -E 's/.*"([^"]+)".*/\1/')

    if [[ -z "$latest" ]]; then
        error "Failed to resolve latest release version"
    fi

    echo "$latest"
}

# ─── Determine install directory ──────────────────────────────────
resolve_install_dir() {
    if [[ -n "$INSTALL_DIR" ]]; then
        echo "$INSTALL_DIR"
        return
    fi

    # Try system directory first
    if [[ -w "/usr/local/bin" ]] || [[ -w "/usr/bin" ]]; then
        echo "/usr/local/bin"
        return
    fi

    # Fallback to user directory
    local user_dir="${HOME}/.local/bin"
    if [[ ! -d "$user_dir" ]]; then
        mkdir -p "$user_dir"
    fi
    echo "$user_dir"
}

# ─── Main ─────────────────────────────────────────────────────────
main() {
    local platform
    platform=$(detect_platform)
    info "Detected platform: ${platform}"

    local version
    version=$(resolve_version)
    info "Installing version: ${version}"

    local install_dir
    install_dir=$(resolve_install_dir)
    info "Installing to: ${install_dir}"

    local binary_name="praxis-${platform}"
    local url="${RELEASE_BASE}/download/${version}/${binary_name}"
    local sha_url="${url}.sha256"

    # Check if binary already exists
    if [[ -x "${install_dir}/praxis" ]] && [[ "$FORCE" != "1" ]]; then
        warn "Binary already exists at ${install_dir}/praxis"
        warn "Set PRAXIS_FORCE=1 to overwrite"
        exit 1
    fi

    # Download binary
    info "Downloading ${binary_name}..."
    local tmpfile
    tmpfile=$(mktemp)
    if ! curl -fsSL --output "$tmpfile" "$url"; then
        rm -f "$tmpfile"
        error "Download failed: ${url}"
    fi

    # Verify checksum if available
    if curl -fsSL --output - "$sha_url" 2>/dev/null | grep -q .; then
        info "Verifying checksum..."
        local expected_hash
        expected_hash=$(curl -fsSL "$sha_url" | awk '{print $1}')
        local actual_hash
        actual_hash=$(sha256sum "$tmpfile" | awk '{print $1}')
        if [[ "$expected_hash" != "$actual_hash" ]]; then
            rm -f "$tmpfile"
            error "Checksum mismatch: expected ${expected_hash}, got ${actual_hash}"
        fi
    else
        info "No checksum file available — skipping verification"
    fi

    # Install
    chmod +x "$tmpfile"
    install "$tmpfile" "${install_dir}/praxis"
    rm -f "$tmpfile"

    info "Installed praxis ${version} to ${install_dir}/praxis"

    # Add to PATH if using user directory
    if [[ "$install_dir" == "${HOME}/.local/bin" ]]; then
        if ! echo "$PATH" | grep -q "$install_dir"; then
            warn "Add ${install_dir} to your PATH:"
            warn "  export PATH=\"${install_dir}:\$PATH\""
            warn "Add the above line to ~/.bashrc or ~/.zshrc"
        fi
    fi

    # Confirm
    info "Version:"
    "${install_dir}/praxis" --version 2>/dev/null || echo "  (version command not available)"
}

main "$@"
