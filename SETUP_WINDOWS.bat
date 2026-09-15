@echo off
REM WiFi-DensePose Windows 11 Deployment Script
REM Complete setup for real-time WiFi sensing on Windows

setlocal enabledelayedexpansion

echo.
echo ╔════════════════════════════════════════════════════════════╗
echo ║  RuView - WiFi-Based Sensing System for Windows 11        ║
echo ║  Real-Time Presence, Vitals & Activity Recognition        ║
echo ╚════════════════════════════════════════════════════════════╝
echo.

REM Check prerequisites
echo [1/6] Checking prerequisites...
where python >nul 2>&1
if errorlevel 1 (
    echo ERROR: Python 3.9+ not found. Install from https://www.python.org/
    exit /b 1
)

where cargo >nul 2>&1
if errorlevel 1 (
    echo ERROR: Rust not found. Install from https://rustup.rs/
    exit /b 1
)

REM Build Rust backend
echo.
echo [2/6] Building Rust backend (release mode)...
cd v2
cargo build --release --workspace --no-default-features
if errorlevel 1 (
    echo ERROR: Rust build failed
    exit /b 1
)
cd ..

REM Install Python dependencies
echo.
echo [3/6] Installing Python dependencies...
pip install -r requirements.txt --quiet
if errorlevel 1 (
    echo ERROR: Python package installation failed
    exit /b 1
)

REM Build visualization frontend
echo.
echo [4/6] Building visualization frontend...
if exist "ui\package.json" (
    cd ui
    npm install --silent
    npm run build --silent
    cd ..
)

REM Generate witness bundle (proof of functionality)
echo.
echo [5/6] Generating witness bundle...
bash scripts/generate-witness-bundle.sh 2>nul || echo WARNING: Witness bundle failed (non-critical)

REM Create startup launcher
echo.
echo [6/6] Creating Windows launcher...
goto :create_launcher

:create_launcher
cd %~dp0
set "LAUNCHER_PATH=%CD%\START_RUVIEW.bat"

(
    echo @echo off
    echo title RuView - WiFi Sensing System
    echo cd /d "%%~dp0"
    echo.
    echo echo.
    echo echo ╔════════════════════════════════════════════════════════════╗
    echo echo ║  RuView - WiFi Sensing Dashboard                          ║
    echo echo ║  Starting services...                                      ║
    echo echo ╚════════════════════════════════════════════════════════════╝
    echo echo.
    echo.
    echo REM Start API server
    echo start "" cmd /k "title RuView API Server ^& python -m uvicorn archive.v1.src.api.main:app --host 0.0.0.0 --port 8000 --reload"
    echo timeout /t 2
    echo.
    echo REM Start visualization server
    echo start "" cmd /k "title RuView UI Server ^& python -m http.server 3000 --directory ui"
    echo timeout /t 2
    echo.
    echo REM Open dashboard
    echo timeout /t 3
    echo start http://localhost:3000
    echo echo.
    echo echo ✓ RuView is running!
    echo echo   - Dashboard:     http://localhost:3000
    echo echo   - API:           http://localhost:8000
    echo echo   - API Docs:      http://localhost:8000/docs
    echo echo.
    echo pause
) > START_RUVIEW.bat

echo ✓ Launcher created: START_RUVIEW.bat
echo.
echo ════════════════════════════════════════════════════════════
echo  SETUP COMPLETE!
echo ════════════════════════════════════════════════════════════
echo.
echo Next steps:
echo.
echo 1. RUN THE APP:
echo    Double-click:  START_RUVIEW.bat
echo    Or from terminal: START_RUVIEW.bat
echo.
echo 2. CONNECT HARDWARE (if using real ESP32):
echo    - Flash firmware to ESP32-S3 or ESP32-C6
echo    - Configure WiFi SSID/password
echo    - Sensor will auto-discover the API server
echo.
echo 3. VIEW REAL-TIME DATA:
echo    - Open http://localhost:3000 in your browser
echo    - API documentation: http://localhost:8000/docs
echo.
echo 4. TEST WITH SYNTHETIC DATA:
echo    - Run: python archive/v1/data/proof/verify.py
echo    - Processes 1,000 synthetic CSI frames through pipeline
echo.
echo ════════════════════════════════════════════════════════════
echo.

goto :eof

:error
echo.
echo ERROR: Setup failed at step [%1/6]
echo.
exit /b 1
