#!/usr/bin/env pwsh
# dev.ps1 — Start praxis development environment (Windows)
#
# Modes:
#   .\scripts\dev.ps1              → Tauri desktop dev (default — hot reload Rust + Vue)
#   .\scripts\dev.ps1 -Web         → backend (API :8080) + frontend (Vite :3000) separately
#   .\scripts\dev.ps1 -BackendOnly  → backend only (implies -Web)
#   .\scripts\dev.ps1 -FrontendOnly → frontend only (implies -Web)
#
# Tauri dev: compiles desktop binary (embeds API server) + starts Vite via beforeDevCommand
# Web mode:  cargo-watch recompiles backend on .rs changes, Vite HMR for .vue/.ts changes

param(
    [switch]$Web,
    [switch]$BackendOnly,
    [switch]$FrontendOnly
)

$ErrorActionPreference = "Stop"

# -BackendOnly and -FrontendOnly imply web mode (Tauri embeds both)
$isWebMode = $Web -or $BackendOnly -or $FrontendOnly

Write-Host ""
Write-Host "  praxis — Development Environment" -ForegroundColor Cyan
Write-Host "  ─────────────────────────────────" -ForegroundColor DarkGray
Write-Host ""

# ─── Check prerequisites ──────────────────────────────────────────

$missing = @()

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $missing += "cargo (Rust)"
}
if (-not (Get-Command bun -ErrorAction SilentlyContinue)) {
    $missing += "bun"
}

if ($missing.Count -gt 0) {
    Write-Host "  Missing prerequisites:" -ForegroundColor Red
    foreach ($m in $missing) {
        Write-Host "    - $m" -ForegroundColor Red
    }
    Write-Host ""
    Write-Host "  Install:" -ForegroundColor Yellow
    Write-Host "    Rust:  https://rustup.rs" -ForegroundColor Gray
    Write-Host "    bun:   https://bun.sh" -ForegroundColor Gray
    exit 1
}

# ─── Resolve directories ──────────────────────────────────────────

$frontendDir     = Join-Path $PSScriptRoot ".." "desktop" "frontend" | Resolve-Path
$desktopCrateDir = Join-Path $PSScriptRoot ".." "desktop"           | Resolve-Path

# ─── Pre-flight: frontend dependencies (bun install) ─────────────

if (-not $BackendOnly) {
    $nodeModules = Join-Path $frontendDir "node_modules"
    $lockfile    = Join-Path $frontendDir "bun.lock"
    $needInstall = $false

    if (-not (Test-Path $nodeModules)) {
        Write-Host "  Frontend: node_modules missing — running bun install..." -ForegroundColor Yellow
        $needInstall = $true
    } elseif ((Test-Path $lockfile) -and
              (Get-Item $lockfile).LastWriteTime -gt (Get-Item $nodeModules).LastWriteTime) {
        Write-Host "  Frontend: bun.lock newer than node_modules — running bun install..." -ForegroundColor Yellow
        $needInstall = $true
    }

    if ($needInstall) {
        Push-Location $frontendDir
        try {
            & bun install
            $installExit = $LASTEXITCODE
        } finally {
            Pop-Location
        }
        if ($installExit -ne 0) {
            Write-Host "  bun install failed (exit $installExit)." -ForegroundColor Red
            exit 1
        }
        Write-Host "  Frontend dependencies installed." -ForegroundColor Green
        Write-Host ""
    }
}

# ─── Pre-flight: cargo check ─────────────────────────────────────

if ($isWebMode -and -not $FrontendOnly) {
    Write-Host "  Backend: running cargo check (pre-flight)..." -ForegroundColor Yellow
    & cargo check --bin praxis
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  cargo check failed (exit $LASTEXITCODE)." -ForegroundColor Red
        Write-Host "  Fix compile errors before starting the dev server." -ForegroundColor Yellow
        exit 1
    }
    Write-Host "  Backend: cargo check passed." -ForegroundColor Green
    Write-Host ""
} elseif (-not $isWebMode) {
    Write-Host "  Desktop: running cargo check --bin desktop (pre-flight)..." -ForegroundColor Yellow
    & cargo check --bin desktop
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  cargo check failed (exit $LASTEXITCODE)." -ForegroundColor Red
        Write-Host "  Fix compile errors before starting Tauri dev." -ForegroundColor Yellow
        exit 1
    }
    Write-Host "  Desktop: cargo check passed." -ForegroundColor Green
    Write-Host ""
}

# ─── Check cargo-watch (optional, for web mode backend hot reload) ──

$cargoWatchInstalled = $null -ne (Get-Command cargo-watch -ErrorAction SilentlyContinue)

# ─── Process management ───────────────────────────────────────────

$script:backendProcess  = $null
$script:frontendProcess = $null
$script:desktopProcess  = $null

function Stop-DevProcesses {
    if ($null -ne $script:backendProcess -and -not $script:backendProcess.HasExited) {
        Write-Host "  Stopping backend..." -ForegroundColor Yellow
        Stop-Process -Id $script:backendProcess.Id -Force -ErrorAction SilentlyContinue
    }
    if ($null -ne $script:frontendProcess -and -not $script:frontendProcess.HasExited) {
        Write-Host "  Stopping frontend..." -ForegroundColor Yellow
        Stop-Process -Id $script:frontendProcess.Id -Force -ErrorAction SilentlyContinue
    }
    if ($null -ne $script:desktopProcess -and -not $script:desktopProcess.HasExited) {
        Write-Host "  Stopping Tauri desktop..." -ForegroundColor Yellow
        Stop-Process -Id $script:desktopProcess.Id -Force -ErrorAction SilentlyContinue
    }
    Write-Host "  Done." -ForegroundColor Green
}

try {
    if (-not $isWebMode) {
        # ─── Tauri desktop mode (default) ────────────────────────
        # Tauri dev starts Vite (via beforeDevCommand) and compiles
        # the desktop binary which embeds its own API server.
        # No separate backend or frontend needed.

        Write-Host "  Starting Tauri desktop dev..." -ForegroundColor Cyan
        Write-Host "    Tauri will start Vite (:3000) and compile desktop binary" -ForegroundColor DarkGray
        Write-Host "    First run may download @tauri-apps/cli via bunx..." -ForegroundColor DarkGray

        $script:desktopProcess = Start-Process -FilePath "bun" -ArgumentList 'x @tauri-apps/cli dev' -WorkingDirectory $desktopCrateDir -PassThru -NoNewWindow
        Write-Host "    PID: $($script:desktopProcess.Id)" -ForegroundColor DarkGray

    } else {
        # ─── Web mode: backend + frontend separately ──────────────

        if (-not $FrontendOnly) {
            Write-Host "  Starting backend (API server on :8080)..." -ForegroundColor Cyan

            if ($cargoWatchInstalled) {
                Write-Host "    cargo-watch detected — hot reload enabled" -ForegroundColor Green
                $script:backendProcess = Start-Process -FilePath "cargo" -ArgumentList 'watch -x "run --bin praxis -- server"' -PassThru -NoNewWindow
            } else {
                Write-Host "    cargo-watch not installed — using plain cargo run" -ForegroundColor DarkGray
                Write-Host "    Install cargo-watch for hot reload: cargo install cargo-watch" -ForegroundColor DarkGray
                $script:backendProcess = Start-Process -FilePath "cargo" -ArgumentList "run","--bin","praxis","--","server" -PassThru -NoNewWindow
            }
            Write-Host "    PID: $($script:backendProcess.Id)" -ForegroundColor DarkGray
        }

        if (-not $BackendOnly) {
            Write-Host ""
            Write-Host "  Starting frontend (Vite on :3000)..." -ForegroundColor Cyan

            $script:frontendProcess = Start-Process -FilePath "bun" -ArgumentList "run","dev" -WorkingDirectory $frontendDir -PassThru -NoNewWindow
            Write-Host "    PID: $($script:frontendProcess.Id)" -ForegroundColor DarkGray
        }
    }

    # ─── Summary ─────────────────────────────────────────────────

    Write-Host ""
    Write-Host "  ─────────────────────────────────────────" -ForegroundColor DarkGray
    if (-not $isWebMode) {
        Write-Host "  Desktop:  Tauri dev (Vite :3000 + embedded API)" -ForegroundColor Green
    } else {
        if (-not $FrontendOnly) {
            Write-Host "  Backend:  http://localhost:8080" -ForegroundColor Green
        }
        if (-not $BackendOnly) {
            Write-Host "  Frontend: http://localhost:3000" -ForegroundColor Green
        }
    }
    Write-Host "  ─────────────────────────────────────────" -ForegroundColor DarkGray
    Write-Host "  Press Ctrl+C to stop" -ForegroundColor Yellow
    Write-Host ""

    # Wait for Ctrl+C
    while ($true) {
        Start-Sleep -Milliseconds 500

        if ($null -ne $script:backendProcess -and $script:backendProcess.HasExited) {
            Write-Host "  Backend exited unexpectedly." -ForegroundColor Red
            Stop-DevProcesses
            exit 1
        }
        if ($null -ne $script:frontendProcess -and $script:frontendProcess.HasExited) {
            Write-Host "  Frontend exited unexpectedly." -ForegroundColor Red
            Stop-DevProcesses
            exit 1
        }
        if ($null -ne $script:desktopProcess -and $script:desktopProcess.HasExited) {
            Write-Host "  Tauri desktop exited unexpectedly." -ForegroundColor Red
            Stop-DevProcesses
            exit 1
        }
    }
} finally {
    Stop-DevProcesses
}
