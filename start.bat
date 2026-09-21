@echo off
setlocal
chcp 65001 >nul

set "SCRIPT_PATH=%~dp0start_ruview.ps1"
set "RUVIEW_CLOSE_CONFLICTS=1"

if not exist "%SCRIPT_PATH%" (
    echo [ERRO] Nao encontrei o launcher PowerShell do RuView.
    echo [ERRO] Caminho esperado: "%SCRIPT_PATH%"
    pause
    exit /b 1
)

powershell -NoLogo -NoExit -ExecutionPolicy Bypass -File "%SCRIPT_PATH%" -CloseConflicts
exit /b %errorlevel%
