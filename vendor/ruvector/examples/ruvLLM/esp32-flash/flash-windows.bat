@echo off
REM RuvLLM ESP32 Flash Script for Windows
REM Usage: flash-windows.bat COM6

setlocal enabledelayedexpansion

set PORT=%1
if "%PORT%"=="" set PORT=COM6

echo ========================================
echo   RuvLLM ESP32 Flash Tool
echo ========================================
echo.

REM Check if espflash is installed
where espflash >nul 2>&1
if errorlevel 1 (
    echo [ERROR] espflash not found. Installing...
    cargo install espflash
    if errorlevel 1 (
        echo [ERROR] Failed to install espflash
        echo Please run: cargo install espflash
        pause
        exit /b 1
    )
)

REM Check if espup is installed (for ESP32 Rust toolchain)
where espup >nul 2>&1
if errorlevel 1 (
    echo [WARNING] ESP32 Rust toolchain may not be installed.
    echo Installing espup...
    cargo install espup
    espup install
)

echo.
echo Building for ESP32...
echo.

cargo build --release
if errorlevel 1 (
    echo [ERROR] Build failed!
    pause
    exit /b 1
)

echo.
echo Flashing to %PORT%...
echo.

espflash flash --port %PORT% --monitor target\xtensa-esp32-espidf\release\ruvllm-esp32-flash
if errorlevel 1 (
    echo [ERROR] Flash failed!
    echo Make sure:
    echo   1. ESP32 is connected to %PORT%
    echo   2. You have write permission to the port
    echo   3. No other program is using the port
    pause
    exit /b 1
)

echo.
echo ========================================
echo   Flash complete! Monitor starting...
echo ========================================
pause
