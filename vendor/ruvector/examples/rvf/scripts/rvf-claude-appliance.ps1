# rvf-claude-appliance.ps1 — Build & boot the Claude Code Appliance (Windows)
# Prerequisites: Docker Desktop, Rust 1.87+, WSL2 (for kernel build)
# Usage: .\scripts\rvf-claude-appliance.ps1
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

Write-Host "=== Claude Code Appliance Builder (Windows) ===" -ForegroundColor Cyan

# ── 1. Check prerequisites ──────────────────────────────────
Write-Host "[1/5] Checking prerequisites..." -ForegroundColor Yellow
try { $null = Get-Command cargo -ErrorAction Stop }
catch { Write-Error "Rust/cargo not found. Install from https://rustup.rs"; exit 1 }
try { $null = Get-Command docker -ErrorAction Stop }
catch { Write-Error "Docker Desktop not found. Install from https://docker.com"; exit 1 }
Write-Host "  cargo: $(cargo --version)"
Write-Host "  docker: $(docker --version)"

# ── 2. Build the appliance ──────────────────────────────────
Write-Host "[2/5] Building Claude Code Appliance (this builds a real Linux kernel via Docker)..." -ForegroundColor Yellow
Push-Location $ScriptDir
try {
    cargo run --example claude_code_appliance
} finally {
    Pop-Location
}
Write-Host "  Built: output\claude_code_appliance.rvf"

# ── 3. Inspect the result ───────────────────────────────────
Write-Host "[3/5] Inspecting appliance segments..." -ForegroundColor Yellow
$Appliance = Join-Path $ScriptDir "output\claude_code_appliance.rvf"
Get-Item $Appliance | Select-Object Name, @{N="Size (MB)";E={[math]::Round($_.Length/1MB,1)}}
try { rvf inspect $Appliance } catch { Write-Host "  (install rvf-cli for detailed inspection)" }

# ── 4. Query the embedded vector store ──────────────────────
Write-Host "[4/5] Querying package database..." -ForegroundColor Yellow
try {
    rvf query $Appliance --vector "0.1,0.2,0.3" --k 3
} catch {
    Write-Host "  (install rvf-cli to query, or use the Rust API)"
}

# ── 5. Boot instructions ───────────────────────────────────
Write-Host "[5/5] Boot instructions:" -ForegroundColor Yellow
Write-Host ""
Write-Host "  Windows requires WSL2 or Windows QEMU for the kernel launcher." -ForegroundColor DarkYellow
Write-Host ""
Write-Host "  # Option A: WSL2 (recommended)"
Write-Host "  wsl -d Ubuntu -- rvf launch $Appliance"
Write-Host ""
Write-Host "  # Option B: Windows QEMU"
Write-Host "  qemu-system-x86_64.exe -M microvm -kernel kernel.bin -append 'console=ttyS0' -nographic"
Write-Host ""
Write-Host "  # Option C: Docker (no QEMU needed)"
Write-Host "  docker run --rm -v ${Appliance}:/app.rvf -p 2222:22 -p 8080:8080 rvf-boot /app.rvf"
Write-Host ""
Write-Host "  # Connect:"
Write-Host "  ssh -p 2222 deploy@localhost"
Write-Host "  Invoke-RestMethod -Uri http://localhost:8080/query -Method Post -Body '{`"vector`":[0.1,...],`"k`":5}'"

Write-Host ""
Write-Host "=== Claude Code Appliance ready ===" -ForegroundColor Green
$Size = [math]::Round((Get-Item $Appliance).Length / 1MB, 1)
Write-Host "  File: $Appliance"
Write-Host "  Size: ${Size} MB"
Write-Host "  5.1 MB single .rvf — boots Linux, serves queries, runs Claude Code."
