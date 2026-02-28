# RuvLLM ESP32 - Cluster Flash Script (Windows)
# Flashes multiple ESP32s with configured roles

param(
    [string]$ConfigFile = "cluster.toml"
)

$ErrorActionPreference = "Stop"

Write-Host @"

╔══════════════════════════════════════════════════════════╗
║          RuvLLM ESP32 - Cluster Flash Tool               ║
╚══════════════════════════════════════════════════════════╝

"@ -ForegroundColor Cyan

if (-not (Test-Path $ConfigFile)) {
    Write-Host "Error: $ConfigFile not found" -ForegroundColor Red
    Write-Host "Run: .\install.ps1 cluster <num_chips>"
    exit 1
}

# Parse config
$config = Get-Content $ConfigFile -Raw
$clusterName = [regex]::Match($config, 'name = "([^"]+)"').Groups[1].Value
$numChips = [regex]::Match($config, 'chips = (\d+)').Groups[1].Value
$topology = [regex]::Match($config, 'topology = "([^"]+)"').Groups[1].Value

Write-Host "Cluster: $clusterName" -ForegroundColor Green
Write-Host "Chips: $numChips"
Write-Host "Topology: $topology"
Write-Host ""

# Build with federation
Write-Host "Building with federation support..." -ForegroundColor Yellow
cargo build --release --features federation

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

# Extract ports
$ports = [regex]::Matches($config, 'port = "([^"]+)"') | ForEach-Object { $_.Groups[1].Value }

$chipId = 1
foreach ($port in $ports) {
    Write-Host ""
    Write-Host "═══════════════════════════════════════════" -ForegroundColor Yellow
    Write-Host "Flashing Chip $chipId to $port" -ForegroundColor Yellow
    Write-Host "═══════════════════════════════════════════" -ForegroundColor Yellow

    # Check if port exists
    $portExists = [System.IO.Ports.SerialPort]::GetPortNames() -contains $port
    if (-not $portExists) {
        Write-Host "Warning: $port not found, skipping..." -ForegroundColor Red
        $chipId++
        continue
    }

    # Flash
    $env:RUVLLM_CHIP_ID = $chipId
    $env:RUVLLM_TOTAL_CHIPS = $numChips

    espflash flash --port $port target\xtensa-esp32-espidf\release\ruvllm-esp32-flash

    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ Chip $chipId flashed successfully" -ForegroundColor Green
    } else {
        Write-Host "✗ Chip $chipId flash failed" -ForegroundColor Red
    }

    $chipId++

    # Wait between flashes
    Start-Sleep -Seconds 2
}

Write-Host ""
Write-Host "═══════════════════════════════════════════" -ForegroundColor Green
Write-Host "Cluster flash complete!" -ForegroundColor Green
Write-Host "═══════════════════════════════════════════" -ForegroundColor Green
Write-Host ""
Write-Host "To monitor: Open separate terminals and run:"
foreach ($port in $ports) {
    Write-Host "  espflash monitor --port $port"
}
