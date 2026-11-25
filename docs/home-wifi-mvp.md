# 家用 WiFi MVP 一鍵安裝（RSSI-only）

分支：`feat/home-wifi-rssi-mvp`（fork `mtlam1028/RuView`）。開頁即看：
腳本編譯並啟動 `sensing-server`，瀏覽器自動開啟 observatory 頁面。

## 各 OS 一鍵步驟

### Windows（PowerShell）

```powershell
powershell -ExecutionPolicy Bypass -File scripts\run-home-wifi.ps1
powershell -ExecutionPolicy Bypass -File scripts\run-home-wifi.ps1 -Source auto
```

未安裝 Git 時經 winget 取 `Git.Git`；未安裝 Rust 時下載 `rustup-init.exe` 安裝。
工作區預設為 `%USERPROFILE%\RuView`。

### macOS

```bash
bash scripts/run-home-wifi-mac.sh         # --source wifi（預設）
bash scripts/run-home-wifi-mac.sh auto    # --source auto
```

未安裝 Git 時提示 `xcode-select --install`（或經 Homebrew 安裝）；
未安裝 Rust 時經 rustup 安裝。工作區預設為 `~/RuView`。

### Linux

```bash
bash scripts/run-home-wifi-linux.sh         # --source wifi（預設）
bash scripts/run-home-wifi-linux.sh auto    # --source auto
```

未安裝 Git 時經 `apt-get`／`dnf`／`pacman` 安裝；
未安裝 Rust 時經 rustup 安裝（Debian/Ubuntu 會先裝編譯依賴）。
工作區預設為 `~/RuView`。

三者共通流程：檢查 Git → 檢查 Rust（缺則安裝）→ clone 或更新本分支 →
`cargo build --release -p wifi-densepose-sensing-server` →
以 `--bind-addr 0.0.0.0` 啟動 → 印出本機與 LAN 查看地址：

- 本機：`http://localhost:8080/ui/observatory.html`
- 區域網路：`http://<本機 LAN IP>:8080/ui/observatory.html`

可用環境變數覆寫：`SOURCE`（`wifi`｜`auto`）、`HTTP_PORT`、`WORK_DIR`、
`REPO_URL`、`BRANCH`、`BIND_ADDR`。PowerShell 版以 `-Source`／`-HttpPort`
參數覆寫前兩者。

## `--source wifi` 與 `auto` 的差別

| 來源參數 | 行為 |
|---|---|
| `wifi` | 強制使用本機 WiFi 介面掃描，走 RSSI 路徑。 |
| `auto` | 由 server 自動選擇可用來源；家用 PC 無 ESP32／CSI 硬體時即落回 `wifi`（RSSI）。 |

## RSSI-only 上限（本 MVP 能力邊界）

1. **僅有訊號強度，無通道狀態**：`wifi` 來源只提供 per-BSSID RSSI，
   無 CSI 子載波相位／振幅，人物姿態、呼吸、心率等細粒度推論不可用。
2. **存在／移動二元輸出為主**：`motion_level`（`present_moving`／
   `present_still`／`absent`）與人數估計為啟發式結果，精度低於 ESP32 CSI。
3. **多徑與鄰居干擾**：RSSI 易受牆體、家電、鄰居 AP 影響；跨房間定位僅供參考。
4. **掃描 cadence 受 OS 限制**：Windows（`netsh`／WlanAPI）、macOS
   （`airport`／CoreWLAN）、Linux（`iw`／`nmcli`）掃描間隔各異，
   幀率不保證 10 fps；UI 以 `stale` 徽章標示凍結幀。
5. **需授與 WiFi／位置權限**：macOS 需位置權限方可掃描；Linux 掃描多需
   `sudo` 或 NetworkManager 權限；企業受管裝置可能被群組原則封鎖。
6. **契約以凍結版為準**：狀態語義見 `docs/api-contract-freeze-home-mvp.md`；
   本 MVP 永不產出 `live_verified`（僅 `live_unverified`），無幀即
   `disconnected`／`waiting_for_frames=true`。
