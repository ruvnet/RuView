# 家用 MVP 對外契約凍結（fde 凍結版）

凍結範圍：`GET /health`、`GET /api/v1/status`、`GET /api/v1/sensing/latest`、
`WS /ws/sensing` 訊框欄位、三態徽章映射。**認證（auth）行為維持現狀，本次不凍結、不更動。**

實作依據：`v2/crates/wifi-densepose-sensing-server/src/main.rs`
（`health`、`health_ready`、`latest`、`SensingUpdate`）、
`v2/crates/wifi-densepose-sensing-server/src/provenance.rs`
（`SourceState::as_str`）、`ui/services/sensing.service.js`、
`ui/utils/data-source-banner.js`、`ui/observatory/js/hud-controller.js`。

## 一、契約表

### 1. `GET /health`（存活探測）

| 欄位 | 型別 | 說明 |
|---|---|---|
| `status` | string | 固定 `"ok"` |
| `source` | string | `effective_source()`（如 `esp32`、`wifi:…`、`simulated`、失效後綴 `:offline`） |
| `tick` | u64 | 伺服器訊框計數器 |
| `clients` | number | WS 連線數 |
| `processing` | object | 由最新更新時間戳推導之處理健康狀態 |

### 2. `GET /api/v1/status`（即 `health_ready`；家用 MVP 唯一狀態真相來源）

| 欄位 | 型別 | 說明 |
|---|---|---|
| `status` | string | 固定 `"ready"` |
| `source` | string | `effective_source()`；逾 `ESP32_OFFLINE_TIMEOUT`（5 s）未收到幀即帶 `:offline` 後綴 |
| `source_state` | string | ADR-295 典範狀態，僅取以下五值：`synthetic`、`live_verified`、`live_unverified`、`stale`、`disconnected` |
| `waiting_for_frames` | boolean | 恆等於 `source_state == "disconnected"`（#2097：已設定 live 來源但尚未收到首幀，不得偽裝為可用） |
| `last_frame_age_ms` | u64 \| null | 最新幀年齡；從未收幀即為 `null` |
| `trust` | object | 治理信任路徑狀態（見證、隱私等級、重校準旗標，僅供診斷） |
| `node_positions` | object | `configured_node_ids`、`default_position`、`default_position_node_ids` |

`source_state` 推導（`SourceState::from_source_label`，新鮮窗 = 5 s）：

| 條件 | `source_state` |
|---|---|
| `source` 以 `:offline` 結尾 | `disconnected` |
| `source` ∈ {`simulated`, `simulate`, `synthetic`, `test`} | `synthetic`（恆為合成，與幀年齡無關） |
| 無幀（`last_frame_age` 為空） | `disconnected` |
| 幀齡 > 5 s | `stale` |
| 有新鮮幀（本路徑無鑑別，僅達 `Unverified`） | `live_unverified`（本路徑永不產出 `live_verified`） |

### 3. `GET /api/v1/sensing/latest`（儀表板一次性快照）

| 狀態 | 形狀 |
|---|---|
| 有新鮮更新（齡 ≤ 10 s，`LATEST_UPDATE_STALE_AFTER`） | 完整 `SensingUpdate`（同 §5），外加 `age_ms`（float） |
| 更新凍結（齡 > 10 s） | `{"status":"stale","age_ms":<float>,"source":<effective_source>}`，不得再作即時值使用 |
| 從未有更新 | `{"status":"no data yet"}` |

### 4. `GET /api/v1/sensing/latest` 之 `source` 與 `modality`（rssi 或 csi）

線路上無獨立 `modality` 欄位； modality 由 `source` 前綴衍生，UI 依以下規則標示（`sensing.service.js liveSourceLabel`）：

| `source` | modality | UI 標示 |
|---|---|---|
| `wifi` 或 `wifi:…` | **rssi** | `HOST WIFI (RSSI ONLY)` |
| `esp32` 或 `esp32:…` | **csi** | `ESP32 HARDWARE` |
| `realtek_csi…`、`mediatek…`、`qualcomm…`、`vendor:…` 等 | **csi** | 供應商 CSI 來源 |
| 其他／缺失 | 未知 | `SOURCE UNSPECIFIED`，不得顯示為 live |

### 5. `WS /ws/sensing` 訊框欄位（`SensingUpdate` 序列化形）

**必要欄位**（§七退回規則之標的）：

| 欄位 | 型別 | 說明 |
|---|---|---|
| `type` | string | 固定 `"sensing_update"` |
| `timestamp` | float | 牆鐘秒（Unix epoch） |
| `source` | string | 來源標籤（同 §2；離線帶 `:offline` 後綴） |
| `tick` | u64 | 單調遞增訊框序號 |
| `features` | object | `mean_rssi`、`variance`、`motion_band_power`、`breathing_band_power`、`dominant_freq_hz`、`change_points`、`spectral_power`（全為 number） |
| `classification` | object | `motion_level`（`present_moving`／`present_still`／`absent`）、`presence`（恆等於 `motion_level != "absent"`，#1442）、`confidence`（number） |
| `signal_field` | object | 訊號場網格 |

**可選欄位**（缺失不得觸發退回）：`source_state`、`nodes`、`node_features`、
`vital_signs`、`persons`、`estimated_persons`、`room_inference`、
`calibrated_presence_evidence`、`pose_keypoints`、`enhanced_motion` 等。

傳輸語義：廣播落後之客戶端僅跳幀、不斷線；每 30 s 一次 WS Ping 保活；
客戶端 `ping` 回 `pong`。

### 6. 三態徽章映射（UI 唯一顯示規則）

UI 僅依 `source_state`（`_applyServerSource`／`data-source-banner.js`／
observatory `updateSourceBadge`）決定徽章；`source` 字串本身不得直接判 live。

| `source_state` | 徽章 | UI 行為 |
|---|---|---|
| `live_verified`、`live_unverified` | **LIVE** | index 不顯示橫幅；observatory 綠點 `LIVE` |
| `stale` | **STALE** | 凍結上一幀＋警示（`RECONNECTING — the display is frozen at the last live reading`）；observatory 回 `DEMO` 顯示，不得標 `LIVE` |
| `synthetic`、`disconnected` | **OFFLINE／DEMO** | 合成水印橫幅（`SYNTHETIC DATA — …`／`SIMULATED — …`）；observatory 灰點 `DEMO`；`waiting_for_frames == true` 時必須落此態（#2097） |

未通過 `/api/v1/status` 探測（unreachable／401／異常）一律視為未知，
保守落入合成／離線態，**不得塌陷為 `live`**（#1526）。

## 二、退回規則（任一必要欄位缺失即退回）

1. `/api/v1/status` 回應缺 `source`、`source_state`、`waiting_for_frames`、
   `last_frame_age_ms` 任一者 → **退回**：dev 不得合併，UI 視同未知來源，
   強制合成／離線徽章。
2. `source_state` 出現五值以外字串 → **退回**：視為契約破壞，不得以字串
   前綴或 `source` 回退判 live。
3. `waiting_for_frames` 與 `source_state == "disconnected"` 不一致 →
   **退回**（恆等式破壞）。
4. `last_frame_age_ms` 為 `null` 但 `source_state` 為任一 live 態 →
   **退回**（無幀不得為 live，#2097）。
5. `GET /api/v1/sensing/latest` 新鮮回應缺 §5 任一必要欄位，或缺 `age_ms` →
   **退回**；`{"status":"stale"}` 回應缺 `age_ms` 或 `source` → **退回**。
6. WS 訊框缺 §5 任一必要欄位 → **該幀丟棄**，不得推進徽章、不得改寫
   `_serverSource`；`classification.presence != (motion_level != "absent")` →
   該幀丟棄（#1442）。
7. WS 訊框缺 `source` → 不得由該幀晉升至 LIVE；缺 `source_state` 屬合法
   （可選欄位），沿用既有狀態。
8. 任何把 `simulated`／`simulate`／`synthetic`／`test` 顯示為 LIVE 的 UI
   行為 → **退回**（#1557：合成來源只能重建為 live 來源並提出已驗證幀，
   方可轉 live）。
9. 本次凍結**不含 auth 變更**：任何附帶修改認證行為（token、ticket、
   中介軟體）之實作 → **退回**，另案處理。
