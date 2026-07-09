# praxis - Launch All Services
# Usage: .\launch.ps1

$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$exe = Join-Path $root "target\release\praxis.exe"
if (-not (Test-Path $exe)) {
    $exe = Join-Path $root "target\debug\praxis.exe"
}

Write-Host ""
Write-Host "  praxis - Launching All Services" -ForegroundColor Cyan
Write-Host ""

# 1. Backend
Write-Host "  [1/3] Starting backend API server..." -ForegroundColor Yellow
Write-Host "        Port: 8080" -ForegroundColor Gray
$backend = Start-Process -FilePath $exe -ArgumentList "server" -WorkingDirectory $root -PassThru -WindowStyle Hidden
Write-Host "  OK Backend started (PID: $($backend.Id))" -ForegroundColor Green

Start-Sleep -Seconds 2

# 2. Frontend
Write-Host "  [2/3] Starting frontend dev server..." -ForegroundColor Yellow
Write-Host "        Port: 3000" -ForegroundColor Gray
$frontend = Join-Path $root "desktop\frontend"
$frontendProc = Start-Process -FilePath "bun" -ArgumentList "run", "dev" -WorkingDirectory $frontend -PassThru -WindowStyle Hidden
Write-Host "  OK Frontend started (PID: $($frontendProc.Id))" -ForegroundColor Green

Start-Sleep -Seconds 2

# 3. Open browser
Write-Host "  [3/3] Opening dashboard in browser..." -ForegroundColor Yellow
Start-Process "http://localhost:3000"

Write-Host ""
Write-Host "  Dashboard: http://localhost:3000" -ForegroundColor Green
Write-Host "  API:       http://localhost:8080" -ForegroundColor Green
Write-Host "  Health:    http://localhost:8080/api/health" -ForegroundColor Green
Write-Host ""
Write-Host "  API keys: Manage them in Settings (no env vars needed)" -ForegroundColor Gray
Write-Host "  Press Ctrl+C in this terminal to stop everything." -ForegroundColor Gray
Write-Host ""

Wait-Process -Id $backend.Id -ErrorAction SilentlyContinue
Write-Host ""
Write-Host "  Backend stopped. Shutting down..." -ForegroundColor Yellow
if ($frontendProc) {
    Stop-Process -Id $frontendProc.Id -Force -ErrorAction SilentlyContinue
    Write-Host "  Frontend stopped." -ForegroundColor Yellow
}
Write-Host ""
