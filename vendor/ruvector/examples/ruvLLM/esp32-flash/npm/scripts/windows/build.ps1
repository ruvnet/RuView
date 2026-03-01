# build.ps1 - Auto-configure and build RuvLLM ESP32
# Automatically detects toolchain paths - no manual configuration needed

param(
    [string]$Target = "xtensa-esp32-espidf",
    [switch]$Release = $true,
    [string]$Features = ""
)

$ErrorActionPreference = "Stop"

Write-Host "`n=== RuvLLM ESP32 Build ===" -ForegroundColor Cyan
Write-Host ""

# Auto-detect paths
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { "$env:USERPROFILE\.rustup" }
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { "$env:USERPROFILE\.cargo" }

# Find ESP toolchain
$espToolchain = (Get-ChildItem "$rustupHome\toolchains" -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like "esp*" } |
    Select-Object -First 1)

if (-not $espToolchain) {
    Write-Error "ESP toolchain not found. Run .\setup.ps1 first"
}

$espToolchainPath = $espToolchain.FullName

# Find libclang dynamically
$libclang = Get-ChildItem "$espToolchainPath" -Recurse -Filter "libclang.dll" -ErrorAction SilentlyContinue |
    Select-Object -First 1

if (-not $libclang) {
    Write-Error "libclang.dll not found in $espToolchainPath"
}

# Find Python
$python = Get-Command python -ErrorAction SilentlyContinue
if (-not $python) {
    $python = Get-Command python3 -ErrorAction SilentlyContinue
}
if (-not $python) {
    Write-Error "Python not found. Please install Python 3.8+"
}
$pythonPath = Split-Path $python.Source

# Find clang and xtensa-esp-elf paths
$clangBin = Get-ChildItem "$espToolchainPath" -Recurse -Directory -Filter "esp-clang" -ErrorAction SilentlyContinue |
    Select-Object -First 1
$clangBinPath = if ($clangBin) { "$($clangBin.FullName)\bin" } else { "" }

$xtensaBin = Get-ChildItem "$espToolchainPath" -Recurse -Directory -Filter "xtensa-esp-elf" -ErrorAction SilentlyContinue |
    Select-Object -First 1
$xtensaBinPath = if ($xtensaBin) { "$($xtensaBin.FullName)\bin" } else { "" }

# Set environment variables
$env:LIBCLANG_PATH = Split-Path $libclang.FullName
$env:RUSTUP_TOOLCHAIN = "esp"
$env:ESP_IDF_VERSION = "v5.1.2"

# Build PATH with all required directories
$pathParts = @(
    $pythonPath,
    "$pythonPath\Scripts",
    $clangBinPath,
    $xtensaBinPath,
    "$cargoHome\bin"
) | Where-Object { $_ -ne "" }

$env:PATH = ($pathParts -join ";") + ";" + $env:PATH

Write-Host "Build Configuration:" -ForegroundColor Gray
Write-Host "  Target:        $Target"
Write-Host "  Release:       $Release"
Write-Host "  Toolchain:     $($espToolchain.Name)"
Write-Host "  LIBCLANG_PATH: $($env:LIBCLANG_PATH)"
Write-Host ""

# Navigate to project directory
$projectDir = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $projectDir

try {
    # Build cargo command
    $cargoArgs = @("build")

    if ($Release) {
        $cargoArgs += "--release"
    }

    if ($Features) {
        $cargoArgs += "--features"
        $cargoArgs += $Features
    }

    Write-Host "Running: cargo $($cargoArgs -join ' ')" -ForegroundColor Gray
    Write-Host ""

    & cargo @cargoArgs

    if ($LASTEXITCODE -ne 0) {
        throw "Build failed with exit code $LASTEXITCODE"
    }

    Write-Host ""
    Write-Host "Build successful!" -ForegroundColor Green

    # Find the built binary
    $buildDir = if ($Release) { "release" } else { "debug" }
    $binary = Get-ChildItem "$projectDir\target\$Target\$buildDir" -Filter "*.elf" -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -notmatch "deps" } |
        Select-Object -First 1

    if ($binary) {
        Write-Host "Binary: $($binary.FullName)" -ForegroundColor Cyan
    }

    Write-Host ""
    Write-Host "Next: Run .\flash.ps1 to flash to device" -ForegroundColor Yellow

} finally {
    Pop-Location
}
