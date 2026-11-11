@echo off
chcp 65001 >nul
title RuView - Real WiFi Sensing (built-in NIC)
set "RUVIEW_ROOT=%~dp0"
set "RUVIEW_ROOT=%RUVIEW_ROOT:~0,-1%"
echo ============================================================
echo   RuView - Real WiFi sensing via built-in WiFi card
echo ============================================================
echo.
echo  [1/3] Starting sensing server  (ws://localhost:8765) ...
start "RuView Sensing (WLAN 2)" /min python "%RUVIEW_ROOT%\run_win_wifi.py"
timeout /t 3 /nobreak >nul

echo  [2/3] Starting UI server        (http://localhost:8080) ...
start "RuView UI" /min python -m http.server 8080 --directory "%RUVIEW_ROOT%\ui"
timeout /t 2 /nobreak >nul

echo  [3/3] Opening Observatory dashboard ...
start "" "http://localhost:8080/observatory.html"
echo.
echo  In the Observatory, top-right gear (Settings) -^> Data tab:
echo     Data Source : Live WebSocket
echo     WS URL      : ws://localhost:8765
echo.
echo  Watch RSSI / Presence / Motion update from your WiFi card.
echo  Tip: move around or walk past the laptop - Presence should
echo  trigger when the signal varies (RSSI-only coarse sensing).
echo.
pause
