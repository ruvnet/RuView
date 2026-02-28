# monitor.ps1 - Open serial monitor for ESP32
# Auto-detects COM port

param(
    [string]$Port = "",
    [int]$Baud = 115200
)

$ErrorActionPreference = "Stop"

Write-Host "`n=== RuvLLM ESP32 Serial Monitor ===" -ForegroundColor Cyan
Write-Host ""

# Auto-detect COM port if not specified
if (-not $Port) {
    Add-Type -AssemblyName System.IO.Ports
    $ports = [System.IO.Ports.SerialPort]::GetPortNames() |
        Where-Object { $_ -match "COM\d+" } |
        Sort-Object { [int]($_ -replace "COM", "") }

    if ($ports.Count -eq 0) {
        Write-Error "No COM ports found. Is the ESP32 connected?"
    } elseif ($ports.Count -eq 1) {
        $Port = $ports[0]
        Write-Host "Auto-detected port: $Port" -ForegroundColor Green
    } else {
        Write-Host "Multiple COM ports found:" -ForegroundColor Yellow
        for ($i = 0; $i -lt $ports.Count; $i++) {
            Write-Host "  [$i] $($ports[$i])"
        }
        $selection = Read-Host "Select port (0-$($ports.Count - 1))"
        $Port = $ports[[int]$selection]
    }
}

Write-Host "Opening monitor on $Port at $Baud baud..." -ForegroundColor Cyan
Write-Host "Press Ctrl+C to exit" -ForegroundColor Gray
Write-Host ""

# Use espflash monitor
& espflash monitor --port $Port --baud $Baud
