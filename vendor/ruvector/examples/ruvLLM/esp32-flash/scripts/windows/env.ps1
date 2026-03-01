# env.ps1 - Set up ESP32 Rust environment for the current session
# Source this script: . .\env.ps1

$ErrorActionPreference = "SilentlyContinue"

# Find paths
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { "$env:USERPROFILE\.rustup" }
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { "$env:USERPROFILE\.cargo" }

# Find ESP toolchain
$espToolchain = (Get-ChildItem "$rustupHome\toolchains" -Directory |
    Where-Object { $_.Name -like "esp*" } |
    Select-Object -First 1)

if (-not $espToolchain) {
    Write-Host "ESP toolchain not found. Run setup.ps1 first." -ForegroundColor Red
    return
}

$espToolchainPath = $espToolchain.FullName

# Find libclang
$libclang = Get-ChildItem "$espToolchainPath" -Recurse -Filter "libclang.dll" |
    Select-Object -First 1

# Find clang bin
$clangBin = Get-ChildItem "$espToolchainPath" -Recurse -Directory -Filter "esp-clang" |
    Select-Object -First 1

# Find xtensa-esp-elf bin
$xtensaBin = Get-ChildItem "$espToolchainPath" -Recurse -Directory -Filter "xtensa-esp-elf" |
    Select-Object -First 1

# Find Python
$python = Get-Command python -ErrorAction SilentlyContinue
$pythonPath = if ($python) { Split-Path $python.Source } else { "" }

# Set environment variables
$env:LIBCLANG_PATH = if ($libclang) { Split-Path $libclang.FullName } else { "" }
$env:RUSTUP_TOOLCHAIN = "esp"
$env:ESP_IDF_VERSION = "v5.1.2"

# Build PATH
$pathAdditions = @()
if ($pythonPath) { $pathAdditions += $pythonPath; $pathAdditions += "$pythonPath\Scripts" }
if ($clangBin) { $pathAdditions += "$($clangBin.FullName)\bin" }
if ($xtensaBin) { $pathAdditions += "$($xtensaBin.FullName)\bin" }
$pathAdditions += "$cargoHome\bin"

$env:PATH = ($pathAdditions -join ";") + ";" + $env:PATH

# Display status
Write-Host ""
Write-Host "ESP32 Rust environment loaded" -ForegroundColor Green
Write-Host ""
Write-Host "  RUSTUP_TOOLCHAIN: $($env:RUSTUP_TOOLCHAIN)" -ForegroundColor Gray
Write-Host "  LIBCLANG_PATH:    $($env:LIBCLANG_PATH)" -ForegroundColor Gray
Write-Host "  ESP_IDF_VERSION:  $($env:ESP_IDF_VERSION)" -ForegroundColor Gray
Write-Host ""
Write-Host "Ready to build! Run: .\build.ps1" -ForegroundColor Cyan
