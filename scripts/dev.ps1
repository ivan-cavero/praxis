#!/usr/bin/env pwsh
# dev.ps1 — Start praxis development environment (Windows)
#
# Starts both backend (API server) and frontend (Vite dashboard) with hot reload.
# Backend: cargo-watch recompiles on .rs changes (if installed)
# Frontend: Vite HMR reloads on .vue/.ts changes
#
# Usage:
#   .\scripts\dev.ps1
#   .\scripts\dev.ps1 -BackendOnly
#   .\scripts\dev.ps1 -FrontendOnly

param(
    [switch]$BackendOnly,
    [switch]$FrontendOnly
)

$ErrorActionPreference = "Stop"

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

# ─── Check cargo-watch (optional, for backend hot reload) ────────

$hasCargoWatch = $null -ne (Get-Command cargo-watch -ErrorAction SilentlyContinue)
$cargoWatchInstalled = $false
if (-not $hasCargoWatch) {
    # Also check if it's a cargo subcommand
    $testResult = cargo watch --help 2>&1
    if ($LASTEXITCODE -eq 0) { $cargoWatchInstalled = $true }
} else {
    $cargoWatchInstalled = $true
}

# ─── Start backend ────────────────────────────────────────────────

$backendProcess = $null
$frontendProcess = $null

function Stop-DevProcesses {
    if ($null -ne $script:backendProcess -and -not $script:backendProcess.HasExited) {
        Write-Host "  Stopping backend..." -ForegroundColor Yellow
        Stop-Process -Id $script:backendProcess.Id -Force -ErrorAction SilentlyContinue
    }
    if ($null -ne $script:frontendProcess -and -not $script:frontendProcess.HasExited) {
        Write-Host "  Stopping frontend..." -ForegroundColor Yellow
        Stop-Process -Id $script:frontendProcess.Id -Force -ErrorAction SilentlyContinue
    }
    Write-Host "  Done." -ForegroundColor Green
}

try {
    if (-not $FrontendOnly) {
        Write-Host "  Starting backend (API server on :8080)..." -ForegroundColor Cyan

        if ($cargoWatchInstalled) {
            Write-Host "    cargo-watch detected — hot reload enabled" -ForegroundColor Green
            $script:backendProcess = Start-Process -FilePath "cargo" -ArgumentList "watch","-x","run --bin praxis -- server" -PassThru -NoNewWindow
        } else {
            Write-Host "    cargo-watch not installed — using plain cargo run" -ForegroundColor DarkGray
            Write-Host "    Install cargo-watch for hot reload: cargo install cargo-watch" -ForegroundColor DarkGray
            $script:backendProcess = Start-Process -FilePath "cargo" -ArgumentList "run","--bin","praxis","--","server" -PassThru -NoNewWindow
        }
        Write-Host "    PID: $($script:backendProcess.Id)" -ForegroundColor DarkGray
    }

    # ─── Start frontend ───────────────────────────────────────────

    if (-not $BackendOnly) {
        Write-Host ""
        Write-Host "  Starting frontend (Vite on :3000)..." -ForegroundColor Cyan

        $dashboardDir = Join-Path $PSScriptRoot ".." "dashboard" | Resolve-Path
        $script:frontendProcess = Start-Process -FilePath "bun" -ArgumentList "run","dev" -WorkingDirectory $dashboardDir -PassThru -NoNewWindow
        Write-Host "    PID: $($script:frontendProcess.Id)" -ForegroundColor DarkGray
    }

    # ─── Summary ─────────────────────────────────────────────────

    Write-Host ""
    Write-Host "  ─────────────────────────────────────────" -ForegroundColor DarkGray
    if (-not $FrontendOnly) {
        Write-Host "  Backend:  http://localhost:8080" -ForegroundColor Green
    }
    if (-not $BackendOnly) {
        Write-Host "  Frontend: http://localhost:3000" -ForegroundColor Green
    }
    Write-Host "  ─────────────────────────────────────────" -ForegroundColor DarkGray
    Write-Host "  Press Ctrl+C to stop" -ForegroundColor Yellow
    Write-Host ""

    # Wait for Ctrl+C
    while ($true) {
        Start-Sleep -Milliseconds 500

        # Check if processes exited on their own
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
    }
} finally {
    Stop-DevProcesses
}
