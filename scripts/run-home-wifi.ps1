#Requires -Version 5.1
<#
.SYNOPSIS
  RuView 家用 WiFi MVP 一鍵安裝啟動（Windows 版）。

.DESCRIPTION
  依序執行：檢查 Git（缺則經 winget 安裝）→ 檢查 Rust（缺則安裝 rustup）→
  clone 或更新 fork 分支 feat/home-wifi-rssi-mvp → cargo build --release →
  以 --source wifi（或 auto）啟動 sensing-server → 印出本機與 LAN 查看地址並開瀏覽器。

.PARAMETER Source
  感測來源：wifi（本機 WiFi RSSI，預設）或 auto（自動選擇）。

.PARAMETER HttpPort
  HTTP / UI 連接埠，預設 8080。

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\run-home-wifi.ps1
  powershell -ExecutionPolicy Bypass -File scripts\run-home-wifi.ps1 -Source auto
#>
param(
  [ValidateSet('wifi', 'auto')]
  [string]$Source = 'wifi',
  [int]$HttpPort = 8080
)

$ErrorActionPreference = 'Stop'
$RepoUrl = 'https://github.com/mtlam1028/RuView.git'
$Branch  = 'feat/home-wifi-rssi-mvp'
$WorkDir = Join-Path $HOME 'RuView'
$BindAddr = '0.0.0.0'

function Require-Command($Name) {
  return ($null -ne (Get-Command $Name -ErrorAction SilentlyContinue))
}

# ── 1. 檢查 Git ──────────────────────────────────────────────
if (-not (Require-Command 'git')) {
  Write-Host '[1/5] 未偵測到 Git，嘗試經 winget 安裝…'
  if (Require-Command 'winget') {
    winget install --id Git.Git -e --source winget --accept-package-agreements --accept-source-agreements
    $env:Path = [System.Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
                [System.Environment]::GetEnvironmentVariable('Path', 'User')
  } else {
    Write-Error '找不到 git，且無 winget 可自動安裝。請先至 https://git-scm.com/download/win 安裝 Git 後重試。'
  }
}
git --version
Write-Host '[1/5] Git 就緒。'

# ── 2. 檢查 Rust（缺則安裝）──────────────────────────────────
if (-not (Require-Command 'cargo')) {
  Write-Host '[2/5] 未偵測到 Rust，正在安裝 rustup…'
  $Installer = Join-Path $env:TEMP 'rustup-init.exe'
  Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile $Installer
  Start-Process -FilePath $Installer -ArgumentList '-y' -Wait
  Remove-Item $Installer -ErrorAction SilentlyContinue
  $CargoBin = Join-Path $HOME '.cargo\bin'
  if ($env:Path -notlike "*$CargoBin*") { $env:Path = "$CargoBin;$env:Path" }
}
cargo --version
Write-Host '[2/5] Rust 就緒。'

# ── 3. clone 或更新分支 ──────────────────────────────────────
if ((Test-Path (Join-Path $WorkDir '.git'))) {
  Write-Host "[3/5] 更新既有工作區 $WorkDir …"
  git -C $WorkDir fetch origin
  git -C $WorkDir checkout $Branch
  git -C $WorkDir pull --ff-only origin $Branch
} else {
  Write-Host "[3/5] 複製 $RepoUrl ($Branch) 至 $WorkDir …"
  git clone -b $Branch $RepoUrl $WorkDir
}
Write-Host '[3/5] 原始碼就緒。'

# ── 4. cargo build --release ─────────────────────────────────
Write-Host '[4/5] 編譯 sensing-server（release，需時數分鐘）…'
Push-Location (Join-Path $WorkDir 'v2')
cargo build --release -p wifi-densepose-sensing-server
Pop-Location
$ServerBin = Join-Path $WorkDir 'v2\target\release\sensing-server.exe'
if (-not (Test-Path $ServerBin)) { Write-Error "編譯完成但找不到 $ServerBin。" }
Write-Host '[4/5] 編譯完成。'

# ── 5. 啟動 server＋印出地址 ────────────────────────────────
$LanIps = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
  Where-Object { $_.IPAddress -ne '127.0.0.1' -and $_.PrefixOrigin -ne 'WellKnown' } |
  Select-Object -ExpandProperty IPAddress -Unique

Write-Host "[5/5] 啟動 server（--source $Source，port $HttpPort）…"
$ServerArgs = @('--source', $Source, '--http-port', "$HttpPort", '--bind-addr', $BindAddr)
$Proc = Start-Process -FilePath $ServerBin -ArgumentList $ServerArgs -WorkingDirectory (Join-Path $WorkDir 'v2') -PassThru
Start-Sleep -Seconds 3
if ($Proc.HasExited) { Write-Error "server 啟動後即退出（exit $($Proc.ExitCode)），請檢查終端機輸出。" }

$LocalUrl = "http://localhost:${HttpPort}/ui/observatory.html"
Write-Host ''
Write-Host '=== RuView 家用 WiFi MVP 已啟動 ==='
Write-Host "本機查看：$LocalUrl"
foreach ($Ip in $LanIps) { Write-Host "區域網路查看：http://${Ip}:${HttpPort}/ui/observatory.html" }
Write-Host "來源：--source $Source（wifi = 本機 WiFi，RSSI-only）"
Write-Host "停止：工作管理員結束 sensing-server，或執行 Stop-Process -Id $($Proc.Id)"
Write-Host ''
Start-Process $LocalUrl
