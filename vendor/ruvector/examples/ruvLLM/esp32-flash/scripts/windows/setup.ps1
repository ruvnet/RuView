# setup.ps1 - One-time Windows setup for RuvLLM ESP32
# Run this once to install/configure the ESP32 Rust toolchain

$ErrorActionPreference = "Stop"

Write-Host "`n=== RuvLLM ESP32 Windows Setup ===" -ForegroundColor Cyan
Write-Host ""

# Find Rust ESP toolchain dynamically
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { "$env:USERPROFILE\.rustup" }
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { "$env:USERPROFILE\.cargo" }

# Check if Rust is installed
$rustc = Get-Command rustc -ErrorAction SilentlyContinue
if (-not $rustc) {
    Write-Host "Rust not found. Installing rustup..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile rustup-init.exe
    .\rustup-init.exe -y --default-toolchain stable
    Remove-Item rustup-init.exe
    $env:PATH = "$cargoHome\bin;" + $env:PATH
    Write-Host "Rust installed successfully" -ForegroundColor Green
}

# Find or install ESP toolchain
$espToolchain = Get-ChildItem "$rustupHome\toolchains" -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like "esp*" } |
    Select-Object -First 1

if (-not $espToolchain) {
    Write-Host "ESP toolchain not found. Installing espup..." -ForegroundColor Yellow

    # Download espup
    $espupUrl = "https://github.com/esp-rs/espup/releases/latest/download/espup-x86_64-pc-windows-msvc.exe"
    $espupPath = "$env:TEMP\espup.exe"

    Write-Host "Downloading espup..." -ForegroundColor Gray
    Invoke-WebRequest -Uri $espupUrl -OutFile $espupPath

    Write-Host "Running espup install (this may take several minutes)..." -ForegroundColor Gray
    & $espupPath install

    if ($LASTEXITCODE -ne 0) {
        Write-Error "espup install failed with exit code $LASTEXITCODE"
    }

    Remove-Item $espupPath -ErrorAction SilentlyContinue

    # Re-check for toolchain
    $espToolchain = Get-ChildItem "$rustupHome\toolchains" -Directory |
        Where-Object { $_.Name -like "esp*" } |
        Select-Object -First 1
}

if (-not $espToolchain) {
    Write-Error "ESP toolchain installation failed. Please install manually: https://esp-rs.github.io/book/"
}

Write-Host "Found ESP toolchain: $($espToolchain.Name)" -ForegroundColor Green

# Find Python
$python = Get-Command python -ErrorAction SilentlyContinue
if (-not $python) {
    $python = Get-Command python3 -ErrorAction SilentlyContinue
}
if (-not $python) {
    Write-Error "Python not found. Please install Python 3.8+ from https://python.org"
}
Write-Host "Found Python: $($python.Source)" -ForegroundColor Green

# Find libclang
$libclang = Get-ChildItem "$($espToolchain.FullName)" -Recurse -Filter "libclang.dll" -ErrorAction SilentlyContinue |
    Select-Object -First 1

if ($libclang) {
    Write-Host "Found libclang: $($libclang.FullName)" -ForegroundColor Green
} else {
    Write-Host "Warning: libclang.dll not found in toolchain" -ForegroundColor Yellow
}

# Install espflash if not present
$espflash = Get-Command espflash -ErrorAction SilentlyContinue
if (-not $espflash) {
    Write-Host "Installing espflash..." -ForegroundColor Yellow
    cargo install espflash
    if ($LASTEXITCODE -ne 0) {
        Write-Error "espflash installation failed"
    }
    Write-Host "espflash installed successfully" -ForegroundColor Green
} else {
    Write-Host "Found espflash: $($espflash.Source)" -ForegroundColor Green
}

# Install ldproxy if not present
$ldproxy = Get-Command ldproxy -ErrorAction SilentlyContinue
if (-not $ldproxy) {
    Write-Host "Installing ldproxy..." -ForegroundColor Yellow
    cargo install ldproxy
    if ($LASTEXITCODE -ne 0) {
        Write-Error "ldproxy installation failed"
    }
    Write-Host "ldproxy installed successfully" -ForegroundColor Green
}

Write-Host ""
Write-Host "=== Setup Complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "Summary:" -ForegroundColor Cyan
Write-Host "  Toolchain: $($espToolchain.Name)"
Write-Host "  Python:    $($python.Source)"
if ($libclang) {
    Write-Host "  Libclang:  $($libclang.FullName)"
}
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "  1. Run: .\build.ps1"
Write-Host "  2. Connect ESP32 via USB"
Write-Host "  3. Run: .\flash.ps1"
Write-Host ""
