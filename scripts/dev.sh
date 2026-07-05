#!/bin/bash
# dev.sh — Start praxis development environment (Linux/macOS)
#
# Starts both backend (API server) and frontend (Vite dashboard) with hot reload.
# Backend: cargo-watch recompiles on .rs changes (if installed)
# Frontend: Vite HMR reloads on .vue/.ts changes
#
# Usage:
#   ./scripts/dev.sh
#   ./scripts/dev.sh --backend-only
#   ./scripts/dev.sh --frontend-only

set -e

BACKEND_ONLY=false
FRONTEND_ONLY=false

for arg in "$@"; do
    case "$arg" in
        --backend-only) BACKEND_ONLY=true ;;
        --frontend-only) FRONTEND_ONLY=true ;;
        *) echo "Unknown option: $arg"; exit 1 ;;
    esac
done

echo ""
echo -e "  \033[36mpraxis — Development Environment\033[0m"
echo -e "  \033[90m─────────────────────────────────\033[0m"
echo ""

# ─── Check prerequisites ──────────────────────────────────────────

missing=()

if ! command -v cargo &> /dev/null; then
    missing+=("cargo (Rust)")
fi
if ! command -v bun &> /dev/null; then
    missing+=("bun")
fi

if [ ${#missing[@]} -gt 0 ]; then
    echo -e "  \033[31mMissing prerequisites:\033[0m"
    for m in "${missing[@]}"; do
        echo -e "    \033[31m- $m\033[0m"
    done
    echo ""
    echo -e "  \033[33mInstall:\033[0m"
    echo -e "    \033[90mRust:  https://rustup.rs\033[0m"
    echo -e "    \033[90mbun:   https://bun.sh\033[0m"
    exit 1
fi

# ─── Check cargo-watch (optional, for backend hot reload) ────────

CARGO_WATCH=""
if cargo watch --help &> /dev/null; then
    CARGO_WATCH="yes"
fi

# ─── Start processes ──────────────────────────────────────────────

BACKEND_PID=""
FRONTEND_PID=""

cleanup() {
    echo ""
    if [ -n "$BACKEND_PID" ]; then
        echo -e "  \033[33mStopping backend...\033[0m"
        kill "$BACKEND_PID" 2>/dev/null || true
    fi
    if [ -n "$FRONTEND_PID" ]; then
        echo -e "  \033[33mStopping frontend...\033[0m"
        kill "$FRONTEND_PID" 2>/dev/null || true
    fi
    echo -e "  \033[32mDone.\033[0m"
    exit 0
}

trap cleanup SIGINT SIGTERM

# ─── Start backend ────────────────────────────────────────────────

if [ "$FRONTEND_ONLY" = false ]; then
    echo -e "  \033[36mStarting backend (API server on :8080)...\033[0m"

    if [ -n "$CARGO_WATCH" ]; then
        echo -e "    \033[32mcargo-watch detected — hot reload enabled\033[0m"
        cargo watch -x 'run --bin praxis -- server' &
        BACKEND_PID=$!
    else
        echo -e "    \033[90mcargo-watch not installed — using plain cargo run\033[0m"
        echo -e "    \033[90mInstall cargo-watch for hot reload: cargo install cargo-watch\033[0m"
        cargo run --bin praxis -- server &
        BACKEND_PID=$!
    fi
    echo -e "    \033[90mPID: $BACKEND_PID\033[0m"
fi

# ─── Start frontend ───────────────────────────────────────────────

if [ "$BACKEND_ONLY" = false ]; then
    echo ""
    echo -e "  \033[36mStarting frontend (Vite on :3000)...\033[0m"

    DASHBOARD_DIR="$(cd "$(dirname "$0")/.." && pwd)/dashboard"
    (cd "$DASHBOARD_DIR" && bun run dev) &
    FRONTEND_PID=$!
    echo -e "    \033[90mPID: $FRONTEND_PID\033[0m"
fi

# ─── Summary ──────────────────────────────────────────────────────

echo ""
echo -e "  \033[90m─────────────────────────────────────────\033[0m"
if [ "$FRONTEND_ONLY" = false ]; then
    echo -e "  \033[32mBackend:  http://localhost:8080\033[0m"
fi
if [ "$BACKEND_ONLY" = false ]; then
    echo -e "  \033[32mFrontend: http://localhost:3000\033[0m"
fi
echo -e "  \033[90m─────────────────────────────────────────\033[0m"
echo -e "  \033[33mPress Ctrl+C to stop\033[0m"
echo ""

# Wait for Ctrl+C
wait
