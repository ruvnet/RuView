# WiFi-DensePose Test Runner
# Runs cargo tests with full output capture

$ErrorActionPreference = "Continue"
$originalDir = Get-Location
$testDir = Join-Path $PSScriptRoot "v2"

Write-Host "Starting WiFi-DensePose Rust test suite..."
Write-Host "Working directory: $testDir"
Write-Host ""

try {
    Set-Location $testDir
    
    # Run cargo test with no-default-features
    & cargo test --workspace --no-default-features 2>&1
    
    $exitCode = $LASTEXITCODE
    Write-Host ""
    Write-Host "Test suite completed with exit code: $exitCode"
    
    exit $exitCode
}
finally {
    Set-Location $originalDir
}
