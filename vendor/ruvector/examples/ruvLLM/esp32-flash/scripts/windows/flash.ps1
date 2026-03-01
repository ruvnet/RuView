# flash.ps1 - Auto-detect COM port and flash RuvLLM ESP32
# Automatically finds connected ESP32 devices

param(
    [string]$Port = "",
    [switch]$Monitor = $true,
    [string]$Target = "xtensa-esp32-espidf",
    [switch]$Release = $true
)

$ErrorActionPreference = "Stop"

Write-Host "`n=== RuvLLM ESP32 Flash ===" -ForegroundColor Cyan
Write-Host ""

# Auto-detect COM port if not specified
if (-not $Port) {
    # Get available COM ports
    Add-Type -AssemblyName System.IO.Ports
    $ports = [System.IO.Ports.SerialPort]::GetPortNames() |
        Where-Object { $_ -match "COM\d+" } |
        Sort-Object { [int]($_ -replace "COM", "") }

    if ($ports.Count -eq 0) {
        Write-Error "No COM ports found. Is the ESP32 connected via USB?"
    } elseif ($ports.Count -eq 1) {
        $Port = $ports[0]
        Write-Host "Auto-detected port: $Port" -ForegroundColor Green
    } else {
        Write-Host "Multiple COM ports found:" -ForegroundColor Yellow
        Write-Host ""
        for ($i = 0; $i -lt $ports.Count; $i++) {
            Write-Host "  [$i] $($ports[$i])"
        }
        Write-Host ""
        $selection = Read-Host "Select port (0-$($ports.Count - 1))"

        if ($selection -match "^\d+$" -and [int]$selection -lt $ports.Count) {
            $Port = $ports[[int]$selection]
        } else {
            Write-Error "Invalid selection"
        }
    }
}

Write-Host "Using port: $Port" -ForegroundColor Cyan
Write-Host ""

# Find binary
$projectDir = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$buildDir = if ($Release) { "release" } else { "debug" }
$targetDir = "$projectDir\target\$Target\$buildDir"

# Look for ELF or binary file
$binary = Get-ChildItem $targetDir -Filter "*.elf" -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -notmatch "deps" } |
    Select-Object -First 1

if (-not $binary) {
    $binary = Get-ChildItem $targetDir -Filter "ruvllm-esp32*" -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -notmatch "\." -or $_.Name -match "\.elf$" } |
        Select-Object -First 1
}

if (-not $binary) {
    Write-Host "Available files in $targetDir`:" -ForegroundColor Yellow
    Get-ChildItem $targetDir -ErrorAction SilentlyContinue | ForEach-Object { Write-Host "  $($_.Name)" }
    Write-Error "No binary found. Run .\build.ps1 first"
}

Write-Host "Binary: $($binary.Name)" -ForegroundColor Gray
Write-Host ""

# Check for espflash
$espflash = Get-Command espflash -ErrorAction SilentlyContinue
if (-not $espflash) {
    Write-Error "espflash not found. Run .\setup.ps1 first"
}

# Build espflash command
$espflashArgs = @("flash", "--port", $Port, $binary.FullName)

if ($Monitor) {
    $espflashArgs += "--monitor"
}

Write-Host "Flashing..." -ForegroundColor Cyan
Write-Host "Command: espflash $($espflashArgs -join ' ')" -ForegroundColor Gray
Write-Host ""

# Flash the device
& espflash @espflashArgs

if ($LASTEXITCODE -ne 0) {
    Write-Error "Flash failed with exit code $LASTEXITCODE"
}

Write-Host ""
Write-Host "Flash complete!" -ForegroundColor Green
