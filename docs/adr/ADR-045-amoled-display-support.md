# ADR-045: AMOLED Display Support for ESP32-S3 CSI Node

## Status

Proposed

## Context

The ESP32-S3 board (LilyGO T-Display-S3 AMOLED) has an integrated RM67162 QSPI AMOLED display (536x240) and 8MB octal PSRAM that were unused by the CSI firmware. Users want real-time on-device visualization of CSI statistics, vital signs, and system health without relying on an external server.

### Constraints

- Binary was 947 KB in a 1 MB partition — needed 8MB flash + custom partition table
- SPIRAM was disabled in sdkconfig despite hardware having 8MB PSRAM
- Core 1 is pinned to DSP (edge processing) — display must use Core 0
- Existing CSI pipeline must not be affected

### Available APIs

Thread-safe edge APIs already exist (`edge_get_vitals()`, `edge_get_multi_person()`) — the display task only reads from these, no new synchronization needed.

## Decision

Add optional AMOLED display support with the following architecture:

### Hardware Abstraction Layer

- `display_hal.c/h`: RM67162 QSPI panel driver + CST816S capacitive touch via I2C
- Auto-detect at boot: probe RM67162 and check SPIRAM; log warning and skip if absent

### UI Layer

- `display_ui.c/h`: LVGL 8.3 with 4 swipeable views via tileview widget
- Dark theme (#0a0a0f) with cyan (#00d4ff) accent for three.js-like aesthetic
- Views: Dashboard (CSI amplitude chart + stats), Vitals (breathing + HR line graphs), Presence (4x4 occupancy grid), System (CPU, heap, PSRAM, WiFi, uptime, FPS)

### Task Layer

- `display_task.c/h`: FreeRTOS task on Core 0, priority 1 (lowest)
- LVGL pump loop at configurable FPS (default 30)
- Double-buffered draw buffers allocated in SPIRAM

### Compile-Time Control

- `CONFIG_DISPLAY_ENABLE=y` (default): compiles display code, auto-detects hardware at boot
- `CONFIG_DISPLAY_ENABLE=n`: zero-cost — no display code compiled
- `CONFIG_SPIRAM_IGNORE_NOTFOUND=y`: boots fine on boards without PSRAM

### Flash Layout

8MB partition table (`partitions_display.csv`):
- Dual OTA partitions: 2 x 2MB (supports larger binaries with LVGL)
- SPIFFS: 1.9MB (for future font/asset storage)
- NVS + otadata + phy: standard sizes

### Core/Task Layout

| Task | Core | Priority | Impact |
|------|------|----------|--------|
| WiFi/LwIP | 0 | 18-23 | unchanged |
| OTA httpd | 0 | 5 | unchanged |
| **display_task** | **0** | **1** | **NEW — lowest priority** |
| edge_task (DSP) | 1 | 5 | unchanged |

### Dependencies

- LVGL ~8.3 (via ESP-IDF managed components)
- espressif/esp_lcd_touch_cst816s ^1.0
- espressif/esp_lcd_touch ^1.0

## Consequences

### Positive

- Real-time on-device stats without network dependency
- Zero impact on CSI pipeline (display reads thread-safe APIs, runs at lowest priority)
- Graceful degradation: works on boards without display or PSRAM
- SPIRAM enabled for all boards (benefits WASM runtime too)
- 8MB flash + dual OTA 2MB partitions give headroom for future features

### Negative

- Binary size increase (~200-300 KB with LVGL)
- SPIRAM + 8MB flash config is specific to T-Display-S3 AMOLED boards
- Boards with only 4MB flash need `CONFIG_DISPLAY_ENABLE=n` and the old partition table

### Risks

- RM67162 init sequence is board-specific; other AMOLED panels may need different commands
- QSPI bus conflicts if other peripherals use SPI2_HOST (currently unused)

## New Files

| File | Purpose |
|------|---------|
| `main/display_hal.c/h` | RM67162 QSPI + CST816S touch HAL |
| `main/display_ui.c/h` | LVGL 4-view UI |
| `main/display_task.c/h` | FreeRTOS task, LVGL pump |
| `main/lv_conf.h` | LVGL compile config |
| `partitions_display.csv` | 8MB partition table |
| `idf_component.yml` | Managed component deps |

## Modified Files

| File | Change |
|------|--------|
| `sdkconfig.defaults` | 8MB flash, SPIRAM, custom partitions |
| `main/CMakeLists.txt` | Conditional display sources + deps |
| `main/main.c` | +1 include, +5 lines guarded init |
| `main/Kconfig.projbuild` | "AMOLED Display" menu |
