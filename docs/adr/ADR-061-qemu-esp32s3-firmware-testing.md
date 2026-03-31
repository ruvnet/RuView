# ADR-061: QEMU ESP32-S3 Emulation for Firmware Testing & Development

| Field       | Value                                          |
|-------------|------------------------------------------------|
| **Status**  | Accepted                                       |
| **Date**    | 2026-03-13 (updated 2026-03-14)                |
| **Authors** | RuView Team                                    |
| **Relates** | ADR-018 (binary frame), ADR-039 (edge intel), ADR-040 (WASM), ADR-057 (build guard), ADR-060 (channel/MAC filter) |

## Context

The ESP32-S3 CSI node firmware (`firmware/esp32-csi-node/`) has grown to 16 source files spanning:

| Module | File | Testable in QEMU? |
|--------|------|--------------------|
| NVS config load | `nvs_config.c` | Yes - NVS partition in flash image |
| Edge processing (DSP) | `edge_processing.c` | Yes - all math, no HW dependency |
| ADR-018 frame serialization | `csi_collector.c:csi_serialize_frame()` | Yes - pure buffer ops |
| UDP stream sender | `stream_sender.c` | Yes - QEMU has lwIP via SLIRP |
| WASM runtime | `wasm_runtime.c` | Yes - CPU only |
| OTA update | `ota_update.c` | Partial - needs HTTP mock |
| Power management | `power_mgmt.c` | Partial - no real light-sleep |
| Display (OLED) | `display_*.c` | No - I2C hardware |
| WiFi CSI callback | `csi_collector.c:wifi_csi_callback()` | **No** - requires RF PHY |
| Channel hopping | `csi_collector.c:hop_timer_cb()` | **No** - requires `esp_wifi_set_channel()` |

Currently, **every code change requires flashing to physical hardware** on COM7. This creates a bottleneck:
- Build + flash cycle: ~20 seconds
- Serial monitor: manual inspection
- No automated CI (no ESP32-S3 in GitHub Actions runners)
- Contributors without hardware cannot test firmware changes

Espressif maintains an official QEMU fork (`github.com/espressif/qemu`) with ESP32-S3 machine support, including dual-core Xtensa LX7, flash mapping, UART, GPIO, timers, and FreeRTOS.

## Glossary

| Term | Definition |
|------|-----------|
| CSI | Channel State Information - per-subcarrier amplitude/phase from WiFi |
| NVS | Non-Volatile Storage - ESP-IDF key-value flash partition |
| TDM | Time-Division Multiplexing - nodes transmit in assigned time slots |
| UART | Universal Asynchronous Receiver-Transmitter - serial console output |
| SLIRP | User-mode TCP/IP stack - enables networking without root/TAP |
| QEMU | Quick Emulator - runs ESP32-S3 firmware without physical hardware |
| QMP | QEMU Machine Protocol - JSON-based control interface |
| LFSR | Linear Feedback Shift Register - deterministic pseudo-random generator |
| SPSC | Single Producer Single Consumer - lock-free ring buffer pattern |
| FreeRTOS | Real-time OS used by ESP-IDF for task scheduling |
| gcov/lcov | GCC code coverage tools for line/branch analysis |
| libFuzzer | LLVM coverage-guided fuzzer for finding crashes |
| ASAN | AddressSanitizer - detects buffer overflows and use-after-free |
| UBSAN | UndefinedBehaviorSanitizer - detects undefined C behavior |

## Quick Start

### Prerequisites

Install required tools:

```bash
# QEMU (Espressif fork with ESP32-S3 support)
git clone https://github.com/espressif/qemu.git
cd qemu && ./configure --target-list=xtensa-softmmu && make -j$(nproc)
export QEMU_PATH=/path/to/qemu/build/qemu-system-xtensa

# ESP-IDF (for building firmware)
# See https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/get-started/

# Python tools
pip install esptool esp-idf-nvs-partition-gen

# Coverage tools (optional, Layer 5)
sudo apt install lcov          # Debian/Ubuntu
brew install lcov              # macOS

# Fuzz testing (optional, Layer 6)
sudo apt install clang         # Debian/Ubuntu

# Mesh testing (optional, Layer 3 - requires root)
sudo apt install socat bridge-utils iproute2
```

### Run the Full Test Suite

```bash
# Layer 2: Single-node test (build + run + validate)
bash scripts/qemu-esp32s3-test.sh

# Layer 3: Multi-node mesh (3 nodes, requires root)
sudo bash scripts/qemu-mesh-test.sh 3

# Layer 6: Fuzz testing (60 seconds per target)
cd firmware/esp32-csi-node/test && make all CC=clang
make run_serialize FUZZ_DURATION=60

# Layer 7: Generate NVS test matrix
python3 scripts/generate_nvs_matrix.py --output-dir build/nvs_matrix

# Layer 8: Snapshot regression tests
bash scripts/qemu-snapshot-test.sh --create
bash scripts/qemu-snapshot-test.sh --restore csi-streaming

# Layer 9: Chaos/fault injection
bash scripts/qemu-chaos-test.sh --faults all --duration 120
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `QEMU_PATH` | `qemu-system-xtensa` | Path to Espressif QEMU binary |
| `QEMU_TIMEOUT` | `60` (single) / `45` (mesh) / `120` (chaos) | Test timeout in seconds |
| `SKIP_BUILD` | unset | Set to `1` to skip firmware build step |
| `NVS_BIN` | unset | Path to pre-built NVS partition binary |
| `QEMU_NET` | `1` | Set to `0` to disable SLIRP networking |
| `CHAOS_SEED` | current time | Seed for reproducible chaos testing |

### Exit Codes (all scripts)

| Code | Meaning | Action |
|------|---------|--------|
| 0 | PASS | All checks passed |
| 1 | WARN | Non-critical issues; review output |
| 2 | FAIL | Critical checks failed; fix and re-run |
| 3 | FATAL | Build error, crash, or missing tool; check prerequisites |

## Decision

Introduce a **comprehensive QEMU testing platform** for the ESP32-S3 CSI node firmware with nine capability layers:

1. **Mock CSI generator** - compile-time synthetic CSI frame injection
2. **QEMU runner** - automated build, run, and validation
3. **Multi-node mesh simulation** - TDM and aggregation testing across QEMU instances
4. **GDB remote debugging** - zero-cost breakpoint debugging without JTAG
5. **Code coverage** - gcov/lcov integration for path analysis
6. **Fuzz testing** - malformed input resilience for CSI parser, NVS, WASM
7. **NVS provisioning matrix** - exhaustive config combination testing
8. **Snapshot & replay** - sub-100ms state restore for fast iteration
9. **Chaos testing** - fault injection for resilience validation

---

## Layer 1: Mock CSI Generator

### Architecture

```
┌─────────────────────────────────────────────────────┐
│                  ESP32-S3 Firmware                    │
│                                                       │
│  ┌─────────────┐    ┌──────────────────────────────┐ │
│  │  Real WiFi   │    │  Mock CSI Generator          │ │
│  │  CSI Callback │ OR │  (timer → synthetic frames)  │ │
│  │  (HW only)   │    │  (QEMU + unit tests)         │ │
│  └──────┬───────┘    └──────────┬───────────────────┘ │
│         │                       │                     │
│         └───────────┬───────────┘                     │
│                     ▼                                 │
│  ┌──────────────────────────────────────────────────┐ │
│  │  edge_enqueue_csi() → SPSC ring → DSP Core 1    │ │
│  │  ├── Biquad bandpass (breathing / heart rate)    │ │
│  │  ├── Phase unwrapping + Welford stats            │ │
│  │  ├── Top-K subcarrier selection                  │ │
│  │  ├── Presence detection (adaptive threshold)     │ │
│  │  ├── Fall detection (phase acceleration)         │ │
│  │  └── Multi-person vitals clustering              │ │
│  └──────────────────┬───────────────────────────────┘ │
│                     ▼                                 │
│  ┌──────────────────────────────────────────────────┐ │
│  │  csi_serialize_frame() → ADR-018 binary format   │ │
│  │  stream_sender_send() → UDP to aggregator        │ │
│  │  edge vitals packet   → 0xC5110002 (32 bytes)    │ │
│  └──────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

### Mock CSI Generator Design

When `CONFIG_CSI_MOCK_ENABLED=y` (Kconfig option), the build replaces `esp_wifi_set_csi_config()` / `esp_wifi_set_csi_rx_cb()` with a periodic timer that injects synthetic CSI frames:

```c
// mock_csi.c - synthetic CSI frame generator

#define MOCK_CSI_INTERVAL_MS   50   // 20 Hz (matches real CSI rate)
#define MOCK_N_SUBCARRIERS     52   // HT20 mode
#define MOCK_IQ_LEN            (MOCK_N_SUBCARRIERS * 2)  // I + Q bytes

typedef struct {
    uint8_t  scenario;        // 0=empty, 1=person_static, 2=person_walking, 3=fall
    uint32_t frame_count;
    float    person_x;        // Simulated position [0..1]
    float    person_speed;    // Movement speed per frame
    uint8_t  breathing_phase; // Simulated breathing cycle
} mock_state_t;

// Generates realistic CSI I/Q data:
// - Empty room: Gaussian noise + stable phase (low variance)
// - Static person: Phase shift proportional to distance, breathing modulation
// - Walking person: Progressive phase drift + Doppler-like amplitude change
// - Fall event: Sudden phase acceleration spike
void mock_generate_csi_frame(mock_state_t *state, wifi_csi_info_t *out_info);
```

### Signal Model

The synthetic CSI generator models subcarrier amplitude and phase as:

```
A_k(t) = A_base + A_person * exp(-d_k²/σ²) + noise
φ_k(t) = φ_base + (2π * d / λ) + breathing_mod(t) + noise

where:
  k         = subcarrier index
  d_k       = simulated distance effect on subcarrier k
  A_person  = amplitude perturbation from human body (scenario-dependent)
  d         = simulated person-to-antenna distance
  λ         = wavelength at subcarrier frequency
  breathing_mod(t) = sin(2π * f_breath * t) * amplitude_breath
  noise     = Gaussian, σ tuned to match real ESP32-S3 CSI noise floor (~-90 dBm)
```

This model exercises:
- Presence detection (amplitude variance exceeds threshold)
- Breathing rate extraction (periodic phase modulation at 0.1-0.5 Hz)
- Fall detection (sudden phase acceleration exceeding `fall_thresh`)
- Multi-person separation (distinct subcarrier groups with different breathing frequencies)

### Scenarios

| ID | Scenario | Duration | Expected Output |
|----|----------|----------|-----------------|
| 0 | Empty room | 10s | `presence=0`, `motion_energy < thresh` |
| 1 | Static person | 10s | `presence=1`, `breathing_rate ∈ [10,25]`, `fall=0` |
| 2 | Walking person | 10s | `presence=1`, `motion_energy > 0.5`, `fall=0` |
| 3 | Fall event | 5s | `fall=1` flag set, `motion_energy` spike |
| 4 | Multi-person | 15s | `n_persons=2`, independent breathing rates |
| 5 | Channel sweep | 5s | Frames on channels 1, 6, 11 in sequence |
| 6 | MAC filter test | 5s | Frames with wrong MAC are dropped (counter check) |
| 7 | Ring buffer overflow | 3s | 1000 frames in 100ms burst, graceful drop |
| 8 | Boundary RSSI | 5s | RSSI sweeps -90 to -10 dBm, no crash |
| 9 | Zero-length frame | 2s | `iq_len=0` frames, serialize returns 0 |

---

## Layer 2: QEMU Runner & CI

### QEMU Runner Script

```bash
#!/bin/bash
# scripts/qemu-esp32s3-test.sh

set -euo pipefail

FIRMWARE_DIR="firmware/esp32-csi-node"
BUILD_DIR="$FIRMWARE_DIR/build"
QEMU_BIN="${QEMU_PATH:-qemu-system-xtensa}"
FLASH_IMAGE="$BUILD_DIR/qemu_flash.bin"
LOG_FILE="$BUILD_DIR/qemu_output.log"
TIMEOUT_SEC="${QEMU_TIMEOUT:-60}"

echo "=== QEMU ESP32-S3 Firmware Test ==="

# 1. Build with mock CSI enabled
echo "[1/4] Building firmware (mock CSI mode)..."
idf.py -C "$FIRMWARE_DIR" \
  -D SDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.qemu" \
  build

# 2. Merge binaries into single flash image
echo "[2/4] Creating merged flash image..."
esptool.py --chip esp32s3 merge_bin -o "$FLASH_IMAGE" \
  --flash_mode dio --flash_freq 80m --flash_size 8MB \
  0x0     "$BUILD_DIR/bootloader/bootloader.bin" \
  0x8000  "$BUILD_DIR/partition_table/partition-table.bin" \
  0xf000  "$BUILD_DIR/ota_data_initial.bin" \
  0x20000 "$BUILD_DIR/esp32-csi-node.bin"

# 3. Optionally inject pre-provisioned NVS partition
if [ -f "$BUILD_DIR/nvs_test.bin" ]; then
  echo "[2b] Injecting pre-provisioned NVS partition..."
  dd if="$BUILD_DIR/nvs_test.bin" of="$FLASH_IMAGE" \
    bs=1 seek=$((0x9000)) conv=notrunc
fi

# 4. Run in QEMU with timeout, capture UART output
echo "[3/4] Running QEMU (timeout: ${TIMEOUT_SEC}s)..."
timeout "$TIMEOUT_SEC" "$QEMU_BIN" \
  -machine esp32s3 \
  -nographic \
  -drive file="$FLASH_IMAGE",if=mtd,format=raw \
  -serial mon:stdio \
  -no-reboot \
  2>&1 | tee "$LOG_FILE" || true

# 5. Validate expected output
echo "[4/4] Validating output..."
python3 scripts/validate_qemu_output.py "$LOG_FILE"
```

### QEMU sdkconfig overlay (`sdkconfig.qemu`)

```
# Enable mock CSI generator (disables real WiFi CSI)
CONFIG_CSI_MOCK_ENABLED=y

# Skip WiFi STA connection (no AP in QEMU)
CONFIG_CSI_MOCK_SKIP_WIFI_CONNECT=y

# Run all scenarios sequentially
CONFIG_CSI_MOCK_SCENARIO=255

# Use loopback for UDP (QEMU SLIRP provides 10.0.2.x network)
CONFIG_CSI_TARGET_IP="10.0.2.2"

# Shorter test durations
CONFIG_CSI_MOCK_SCENARIO_DURATION_MS=5000

# Enable verbose logging for validation
CONFIG_LOG_DEFAULT_LEVEL_INFO=y
CONFIG_CSI_MOCK_LOG_FRAMES=y
```

### Output Validation Script

`scripts/validate_qemu_output.py` parses the UART log and checks:

| Check | Pass Criteria | Severity |
|-------|---------------|----------|
| Boot | `app_main()` called, no panic/assert | FATAL |
| NVS load | `nvs_config:` log line present | FATAL |
| Mock CSI init | `mock_csi: Starting mock CSI generator` | FATAL |
| Frame generation | `mock_csi: Generated N frames` where N > 0 | ERROR |
| Edge pipeline | `edge_processing: DSP task started on Core 1` | ERROR |
| Vitals output | At least one `vitals:` log line with valid BPM | ERROR |
| Presence detection | `presence=1` appears during person scenarios | WARN |
| Fall detection | `fall=1` appears during fall scenario | WARN |
| MAC filter | `csi_collector: MAC filter dropped N frames` where N > 0 | WARN |
| ADR-018 serialize | `csi_collector: Serialized N frames` where N > 0 | ERROR |
| No crash | No `Guru Meditation Error`, no `assert failed`, no `abort()` | FATAL |
| Clean exit | Firmware reaches end of scenario sequence | ERROR |
| Heap OK | No `HEAP_ERROR` or `out of memory` | FATAL |
| Stack OK | No `Stack overflow` detected | FATAL |

Exit codes: `0` = all pass, `1` = WARN only, `2` = ERROR, `3` = FATAL

### CI Workflow

```yaml
# .github/workflows/firmware-qemu.yml
name: Firmware QEMU Tests
on:
  push:
    paths: ['firmware/**']
  pull_request:
    paths: ['firmware/**']

jobs:
  qemu-test:
    runs-on: ubuntu-latest
    container:
      image: espressif/idf:v5.4
    strategy:
      matrix:
        scenario: [default, nvs-full, nvs-edge-tier0, nvs-tdm-3node]
    steps:
      - uses: actions/checkout@v4

      - name: Install Espressif QEMU
        run: |
          apt-get update && apt-get install -y libslirp-dev libglib2.0-dev ninja-build
          git clone --depth 1 https://github.com/espressif/qemu.git /tmp/qemu
          cd /tmp/qemu
          ./configure --target-list=xtensa-softmmu --enable-slirp
          make -j$(nproc)
          cp build/qemu-system-xtensa /usr/local/bin/
        env:
          QEMU_PATH: /usr/local/bin/qemu-system-xtensa

      - name: Prepare NVS for scenario
        run: |
          case "${{ matrix.scenario }}" in
            nvs-full)
              python firmware/esp32-csi-node/provision.py --dry-run \
                --port dummy --ssid "TestWiFi" --password "test1234" \
                --target-ip "10.0.2.2" --target-port 5005 \
                --channel 6 --filter-mac AA:BB:CC:DD:EE:FF \
                --node-id 1 --edge-tier 2
              cp nvs_provision.bin firmware/esp32-csi-node/build/nvs_test.bin
              ;;
            nvs-edge-tier0)
              python firmware/esp32-csi-node/provision.py --dry-run \
                --port dummy --edge-tier 0 --node-id 5
              cp nvs_provision.bin firmware/esp32-csi-node/build/nvs_test.bin
              ;;
            nvs-tdm-3node)
              python firmware/esp32-csi-node/provision.py --dry-run \
                --port dummy --tdm-slot 1 --tdm-total 3 --node-id 1
              cp nvs_provision.bin firmware/esp32-csi-node/build/nvs_test.bin
              ;;
          esac

      - name: Build firmware (mock CSI mode)
        run: |
          cd firmware/esp32-csi-node
          idf.py -D SDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.qemu" set-target esp32s3
          idf.py build

      - name: Run QEMU tests
        run: bash scripts/qemu-esp32s3-test.sh
        env:
          QEMU_PATH: /usr/local/bin/qemu-system-xtensa
          QEMU_TIMEOUT: 90

      - name: Upload QEMU log
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: qemu-output-${{ matrix.scenario }}
          path: firmware/esp32-csi-node/build/qemu_output.log
```

---

## Layer 3: Multi-Node Mesh Simulation

Run multiple QEMU instances with TAP networking to test TDM slot coordination and multi-node aggregation.

### Architecture

```
┌──────────┐   ┌──────────┐   ┌──────────┐
│ QEMU #0  │   │ QEMU #1  │   │ QEMU #2  │
│ slot=0   │   │ slot=1   │   │ slot=2   │
│ node_id=0│   │ node_id=1│   │ node_id=2│
└────┬─────┘   └────┬─────┘   └────┬─────┘
     │              │              │
     └──────────┬───┴──────────────┘
                ▼
        ┌───────────────┐
        │ TAP bridge    │
        │ (10.0.0.0/24) │
        └───────┬───────┘
                ▼
        ┌───────────────┐
        │ Rust aggregator│
        │ (UDP :5005)   │
        └───────────────┘
```

### Multi-Node Runner

```bash
#!/bin/bash
# scripts/qemu-mesh-test.sh - run 3 QEMU nodes + Rust aggregator

set -euo pipefail

N_NODES=${1:-3}
AGGREGATOR_PORT=5005
BRIDGE="qemu-br0"

# Create bridge
ip link add "$BRIDGE" type bridge
ip addr add 10.0.0.1/24 dev "$BRIDGE"
ip link set "$BRIDGE" up

# Build flash images with per-node NVS
for i in $(seq 0 $((N_NODES - 1))); do
  python firmware/esp32-csi-node/provision.py --dry-run \
    --port dummy --node-id "$i" --tdm-slot "$i" --tdm-total "$N_NODES" \
    --target-ip 10.0.0.1 --target-port "$AGGREGATOR_PORT"
  cp nvs_provision.bin "build/nvs_node${i}.bin"

  # Inject NVS into per-node flash image
  cp build/qemu_flash.bin "build/qemu_flash_node${i}.bin"
  dd if="build/nvs_node${i}.bin" of="build/qemu_flash_node${i}.bin" \
    bs=1 seek=$((0x9000)) conv=notrunc
done

# Start Rust aggregator in background
cargo run -p wifi-densepose-hardware --bin aggregator - \
  --listen 0.0.0.0:${AGGREGATOR_PORT} \
  --expect-nodes "$N_NODES" \
  --output build/mesh_test_results.json &
AGGREGATOR_PID=$!

# Launch QEMU nodes
for i in $(seq 0 $((N_NODES - 1))); do
  TAP="tap${i}"
  ip tuntap add "$TAP" mode tap
  ip link set "$TAP" master "$BRIDGE"
  ip link set "$TAP" up

  qemu-system-xtensa \
    -machine esp32s3 \
    -nographic \
    -drive file="build/qemu_flash_node${i}.bin",if=mtd,format=raw \
    -serial file:"build/qemu_node${i}.log" \
    -nic tap,ifname="$TAP",script=no,downscript=no \
    -no-reboot &
  echo "Started QEMU node $i (PID: $!)"
done

# Wait for test duration
sleep 30

# Validate results
kill $AGGREGATOR_PID 2>/dev/null || true
python3 scripts/validate_mesh_test.py build/mesh_test_results.json --nodes "$N_NODES"
```

### Mesh Validation Checks

| Check | Pass Criteria |
|-------|---------------|
| All nodes booted | N distinct `node_id` values in received frames |
| TDM ordering | Slot 0 frames arrive before slot 1 within each TDM cycle |
| No slot collision | No two frames from different nodes with overlapping timestamps within TDM window |
| Frame count balance | Each node contributes ±10% of total frames |
| ADR-018 compliance | All frames have valid magic `0xC5110001` and correct node IDs |
| Vitals per node | Each node produces independent vitals packets |

---

## Layer 4: GDB Remote Debugging

QEMU provides a built-in GDB stub for zero-cost debugging without JTAG hardware.

### Usage

```bash
# Launch QEMU with GDB stub (paused at boot)
qemu-system-xtensa \
  -machine esp32s3 \
  -nographic \
  -drive file=build/qemu_flash.bin,if=mtd,format=raw \
  -serial mon:stdio \
  -s -S   # -s = GDB on :1234, -S = pause at start

# In another terminal: attach GDB
xtensa-esp-elf-gdb build/esp32-csi-node.elf \
  -ex "target remote :1234" \
  -ex "b edge_processing.c:dsp_task" \
  -ex "b csi_collector.c:wifi_csi_callback" \
  -ex "b mock_csi.c:mock_generate_csi_frame" \
  -ex "watch g_nvs_config.csi_channel" \
  -ex "continue"
```

### Debugging Walkthrough

**1. Start QEMU with GDB stub (paused at reset vector):**

```bash
qemu-system-xtensa \
  -machine esp32s3 \
  -nographic \
  -drive file=build/qemu_flash.bin,if=mtd,format=raw \
  -serial mon:stdio \
  -s -S
# -s  opens GDB server on localhost:1234
# -S  pauses CPU until GDB sends "continue"
```

**2. Connect from a second terminal:**

```bash
xtensa-esp-elf-gdb build/esp32-csi-node.elf \
  -ex "target remote :1234" \
  -ex "b app_main" \
  -ex "continue"
```

**3. Set a breakpoint on DSP processing and inspect state:**

```
(gdb) b edge_processing.c:dsp_task
(gdb) continue
# ...breakpoint hit...
(gdb) print g_nvs_config
(gdb) print ring->head - ring->tail
(gdb) continue
```

**4. Connect from VS Code** using the `launch.json` config below (set breakpoints in the editor gutter, then press F5).

**5. Dump gcov coverage data (requires `sdkconfig.coverage` overlay):**

```
(gdb) monitor gcov dump
# Writes .gcda files to the build directory.
# Then generate the HTML report on the host:
#   lcov --capture --directory build --output-file coverage.info
#   genhtml coverage.info --output-directory build/coverage_report
```

### Key Breakpoint Locations

| Breakpoint | Purpose |
|-----------|---------|
| `edge_processing.c:dsp_task` | DSP consumer loop entry |
| `edge_processing.c:presence_detect` | Threshold comparison |
| `edge_processing.c:fall_detect` | Phase acceleration check |
| `csi_collector.c:wifi_csi_callback` | Frame ingestion (or mock injection point) |
| `csi_collector.c:csi_serialize_frame` | ADR-018 serialization |
| `nvs_config.c:nvs_config_load` | NVS parse logic |
| `wasm_runtime.c:wasm_on_csi` | WASM module dispatch |
| `mock_csi.c:mock_generate_csi_frame` | Synthetic frame generation |

### VS Code Integration

```json
// .vscode/launch.json
{
  "version": "0.2.0",
  "configurations": [{
    "name": "QEMU ESP32-S3 Debug",
    "type": "cppdbg",
    "request": "launch",
    "program": "${workspaceFolder}/firmware/esp32-csi-node/build/esp32-csi-node.elf",
    "miDebuggerPath": "xtensa-esp-elf-gdb",
    "miDebuggerServerAddress": "localhost:1234",
    "setupCommands": [
      { "text": "set remote hardware-breakpoint-limit 2" },
      { "text": "set remote hardware-watchpoint-limit 2" }
    ]
  }]
}
```

---

## Layer 5: Code Coverage (gcov/lcov)

### Build with Coverage

```
# sdkconfig.coverage (overlay)
CONFIG_COMPILER_OPTIMIZATION_NONE=y
CONFIG_GCOV_ENABLE=y
CONFIG_APPTRACE_GCOV_ENABLE=y
```

### Coverage Collection

```bash
# After QEMU run, extract gcov data from flash dump
esptool.py --chip esp32s3 read_flash 0x300000 0x100000 gcov_data.bin

# Or use ESP-IDF's app_trace + gcov integration:
# QEMU + GDB → "monitor gcov dump" → .gcda files

# Generate HTML report
lcov --capture --directory build --output-file coverage.info
lcov --remove coverage.info '*/esp-idf/*' '*/test/*' --output-file coverage_filtered.info
genhtml coverage_filtered.info --output-directory build/coverage_report
```

### Coverage Targets

| Module | Target | Critical Paths |
|--------|--------|---------------|
| `edge_processing.c` | ≥80% | `dsp_task`, `biquad_filter`, `fall_detect`, `multi_person_cluster` |
| `csi_collector.c` | ≥90% | `csi_serialize_frame`, `wifi_csi_callback`, MAC filter branch |
| `nvs_config.c` | ≥95% | Every NVS key read path, default fallback paths |
| `mock_csi.c` | ≥95% | All scenarios, all signal model branches |
| `stream_sender.c` | ≥80% | Init, send, error paths |
| `wasm_runtime.c` | ≥70% | Module load, dispatch, signature verify |

---

## Layer 6: Fuzz Testing

### Fuzz Targets

| Target | Input | Mutation Strategy | Looking For |
|--------|-------|-------------------|-------------|
| `csi_serialize_frame()` | Random `wifi_csi_info_t` | Extreme `len` (0, 65535), NULL `buf`, negative RSSI, channel 255 | Buffer overflow, NULL deref |
| `nvs_config_load()` | Crafted NVS partition binary | Truncated strings, out-of-range u8/u16, missing keys, corrupt headers | Kconfig fallback, no crash |
| `edge_enqueue_csi()` | Rapid-fire 10,000 frames | Vary `iq_len` (0 to `EDGE_MAX_IQ_BYTES+1`), randomize RSSI | Ring overflow, no data corruption |
| `rvf_parser.c` | Malformed RVF network packets | Bad magic, truncated headers, oversized payloads | Parse rejection, no crash |
| `wasm_upload.c` | Corrupt WASM blobs | Invalid magic, oversized modules, bad Ed25519 signatures, truncated | Rejection without crash, no code execution |
| `csi_serialize_frame()` + `edge_enqueue_csi()` | Chained: generate → serialize → enqueue | End-to-end with random data | Pipeline integrity |

### Implementation Approach

```c
// test/fuzz_csi_serialize.c - runs on host (not ESP32)
// Compiled with: clang -fsanitize=fuzzer,address

#include "csi_collector.h"

int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size) {
    if (size < sizeof(wifi_csi_info_t)) return 0;

    wifi_csi_info_t info;
    memcpy(&info, data, sizeof(info));

    // Point buf at remaining fuzz data
    size_t remaining = size - sizeof(info);
    uint8_t iq_buf[2048];
    if (remaining > sizeof(iq_buf)) remaining = sizeof(iq_buf);
    memcpy(iq_buf, data + sizeof(info), remaining);
    info.buf = iq_buf;
    info.len = (int)remaining;

    uint8_t out[4096];
    csi_serialize_frame(&info, out, sizeof(out));
    return 0;
}
```

### Fuzz CI Job

```yaml
  fuzz-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build fuzz targets
        run: |
          cd firmware/esp32-csi-node/test
          clang -fsanitize=fuzzer,address -I../main \
            fuzz_csi_serialize.c ../main/csi_collector.c \
            -o fuzz_serialize
      - name: Run fuzz (5 min per target)
        run: |
          cd firmware/esp32-csi-node/test
          timeout 300 ./fuzz_serialize corpus/ || true
      - name: Upload crashes
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: fuzz-crashes
          path: firmware/esp32-csi-node/test/crash-*
```

---

## Layer 7: NVS Provisioning Matrix

### Config Combinations

| Config | NVS Values | Validates |
|--------|-----------|-----------|
| `default` | (empty NVS) | Kconfig fallback paths |
| `wifi-only` | ssid, password | Basic provisioning |
| `full-adr060` | channel=6, filter_mac=AA:BB:CC:DD:EE:FF | Channel override + MAC filter |
| `edge-tier0` | edge_tier=0 | Raw CSI passthrough (no DSP) |
| `edge-tier1` | edge_tier=1, pres_thresh=100, fall_thresh=2000 | Stats-only mode |
| `edge-tier2-custom` | edge_tier=2, vital_win=128, vital_int=500, subk_count=16 | Full vitals with custom params |
| `tdm-3node` | tdm_slot=1, tdm_nodes=3, node_id=1 | TDM mesh timing |
| `wasm-signed` | wasm_max=4, wasm_verify=1, wasm_pubkey=<32 bytes> | WASM with Ed25519 verification |
| `wasm-unsigned` | wasm_max=2, wasm_verify=0 | WASM without signature check |
| `5ghz-channel` | channel=36, filter_mac=... | 5 GHz CSI collection |
| `boundary-max` | target_port=65535, node_id=255, top_k=32, vital_win=256 | Max-range values |
| `boundary-min` | target_port=1, node_id=0, top_k=1, vital_win=32 | Min-range values |
| `power-save` | power_duty=10, edge_tier=0 | Low-power mode |
| `corrupt-nvs` | (manually crafted partial/corrupt partition) | Graceful fallback to defaults |

### Automated Matrix Generation

```python
# scripts/generate_nvs_matrix.py
# Generates all 14 NVS partition binaries for CI matrix

CONFIGS = [
    {"name": "default", "args": []},
    {"name": "wifi-only", "args": ["--ssid", "Test", "--password", "test1234"]},
    {"name": "full-adr060", "args": ["--channel", "6", "--filter-mac", "AA:BB:CC:DD:EE:FF",
                                      "--ssid", "Test", "--password", "test"]},
    {"name": "edge-tier0", "args": ["--edge-tier", "0"]},
    # ... all 14 configs
]
```

---

## Layer 8: Snapshot & Replay

### QEMU Snapshot Commands

```bash
# Save snapshot after boot + NVS load (skip 3s boot time)
(qemu) savevm post_boot

# Save after WiFi connect + first CSI frame
(qemu) savevm post_connect

# Save after edge pipeline calibration complete (~60s)
(qemu) savevm post_calibration

# Restore any snapshot (< 100ms)
(qemu) loadvm post_connect
```

### Automated Snapshot Pipeline

```bash
# scripts/qemu-snapshot-test.sh

# Phase 1: Create base snapshots (one-time, cached in CI)
qemu-system-xtensa ... -monitor unix:qemu.sock,server,nowait &
sleep 5
echo "savevm post_boot" | socat - UNIX-CONNECT:qemu.sock
sleep 10
echo "savevm post_first_frame" | socat - UNIX-CONNECT:qemu.sock

# Phase 2: Run quick tests from snapshots (< 1s each)
for test in test_presence test_fall test_multi_person; do
  echo "loadvm post_first_frame" | socat - UNIX-CONNECT:qemu.sock
  echo "cont" | socat - UNIX-CONNECT:qemu.sock
  sleep 2  # Run test scenario
  # Validate output
done
```

### Performance Impact

| Operation | Without Snapshots | With Snapshots |
|-----------|-------------------|----------------|
| Full boot + NVS + WiFi mock | ~5 seconds | ~5 seconds (first run) |
| Run single scenario | ~5s boot + ~5s test = 10s | ~0.1s restore + ~5s test = 5.1s |
| Run all 10 scenarios | ~100 seconds | ~51 seconds (49% faster) |
| Run 14 NVS configs × 10 scenarios | ~23 minutes | ~12 minutes (48% faster) |

---

## Layer 9: Chaos Testing

### Fault Injection Table

| Fault | Injection Method | Expected Behavior | Severity |
|-------|-----------------|-------------------|----------|
| WiFi disconnect | Timer kills mock WiFi connection after N frames | Reconnect attempt, CSI pauses and resumes | HIGH |
| Ring buffer overflow | Burst 1000 frames in 100ms | Frame drop counter increments, no crash, no data corruption | HIGH |
| NVS corruption | Flash image with partial-write NVS partition | Falls back to Kconfig defaults, logs warning | MEDIUM |
| Stack overflow | Deep recursion in WASM module callback | Watchdog fires, task restarts, no hang | HIGH |
| Heap exhaustion | `malloc` returns NULL after N allocations | Graceful degradation, logs OOM, continues operation | HIGH |
| Timer starvation | Block DSP task for 500ms | Frames dropped from ring, no deadlock, recovers | MEDIUM |
| UDP send failure | SLIRP network down | `stream_sender_send` returns -1, error counter increments | LOW |
| Corrupt CSI frame | Inject frame with invalid magic in I/Q data | Edge pipeline rejects, increments error counter | LOW |
| NVS write during read | Concurrent NVS open for write while config loads | No corruption, NVS handle isolation | MEDIUM |

### Chaos Runner

```bash
# scripts/qemu-chaos-test.sh

# Run with fault injection enabled
qemu-system-xtensa ... \
  -monitor unix:qemu.sock,server,nowait &

# Inject faults via GDB or monitor commands
for fault in wifi_kill heap_exhaust ring_flood; do
  echo "[CHAOS] Injecting: $fault"
  python3 scripts/inject_fault.py --socket qemu.sock --fault "$fault"
  sleep 5
  python3 scripts/check_health.py --log "$LOG_FILE" --after-fault "$fault"
done
```

---

## Implementation Plan

| Phase | Layer | Deliverables | Effort | Priority |
|-------|-------|-------------|--------|----------|
| **P1** | L1 + L2 | `mock_csi.c`, `mock_csi.h`, `Kconfig.projbuild`, `sdkconfig.qemu`, `qemu-esp32s3-test.sh`, `validate_qemu_output.py`, `firmware-qemu.yml` | 2 days | Critical |
| **P2** | L4 + L5 | GDB launch config, `sdkconfig.coverage`, lcov integration, coverage CI job | 1 day | High |
| **P3** | L7 | `generate_nvs_matrix.py`, 14 NVS configs, CI matrix expansion | 1 day | High |
| **P4** | L6 | `fuzz_csi_serialize.c`, `fuzz_nvs_config.c`, `fuzz_edge_enqueue.c`, fuzz CI job | 2 days | High |
| **P5** | L3 | `qemu-mesh-test.sh`, TAP bridge setup, `validate_mesh_test.py`, Rust aggregator integration | 3 days | High |
| **P6** | L8 | Snapshot pipeline, cached base images in CI | 0.5 day | Medium |
| **P7** | L9 | `inject_fault.py`, `check_health.py`, `qemu-chaos-test.sh`, 9 fault scenarios | 2 days | Medium |
| **P8** | Performance | Instruction counting, DSP cycle profiling, optimization report | 1 day | Low |

**Total**: ~12.5 days across 8 phases

---

## File Layout

```
firmware/esp32-csi-node/
├── main/
│   ├── mock_csi.c              # NEW - synthetic CSI frame generator
│   ├── mock_csi.h              # NEW - mock API + scenario definitions
│   ├── Kconfig.projbuild       # MODIFIED - CONFIG_CSI_MOCK_* options
│   ├── CMakeLists.txt          # MODIFIED - conditional mock_csi.c inclusion
│   └── ... (existing files unchanged)
├── test/
│   ├── fuzz_csi_serialize.c    # NEW - libFuzzer target for serialization
│   ├── fuzz_nvs_config.c       # NEW - libFuzzer target for NVS parsing
│   ├── fuzz_edge_enqueue.c     # NEW - libFuzzer target for ring buffer
│   └── corpus/                 # NEW - seed inputs for fuzz targets
├── sdkconfig.qemu             # NEW - QEMU-specific sdkconfig overlay
├── sdkconfig.coverage         # NEW - gcov-enabled sdkconfig overlay
└── ...

scripts/
├── qemu-esp32s3-test.sh       # NEW - single-node QEMU runner
├── qemu-mesh-test.sh          # NEW - multi-node mesh runner
├── qemu-chaos-test.sh         # NEW - chaos/fault injection runner
├── validate_qemu_output.py    # NEW - UART log validation
├── validate_mesh_test.py      # NEW - mesh test validation
├── generate_nvs_matrix.py     # NEW - NVS config matrix generator
├── inject_fault.py            # NEW - QEMU fault injection
└── check_health.py            # NEW - post-fault health checker

.vscode/
└── launch.json                # MODIFIED - add QEMU GDB debug config

.github/workflows/
└── firmware-qemu.yml          # NEW - CI workflow with matrix
```

---

## Consequences

### Benefits

1. **No hardware required** - contributors validate firmware changes with QEMU alone
2. **Automated CI** - every PR touching `firmware/` runs 14 NVS configs × 10 scenarios in parallel
3. **10× faster iteration** - snapshot restore in <100ms vs 20s flash cycle
4. **Security hardening** - fuzz testing catches buffer overflows, NULL derefs, and parser bugs before they reach hardware
5. **Mesh validation** - multi-node TDM tested without 3 physical ESP32s
6. **Coverage visibility** - lcov reports show untested edge processing paths
7. **Resilience proof** - chaos tests verify firmware recovers from WiFi drops, OOM, and ring overflow
8. **GDB debugging** - set breakpoints on DSP pipeline without JTAG adapter
9. **Regression detection** - boot failures, NVS parsing errors, and FreeRTOS deadlocks caught in CI

### Limitations

1. **No real WiFi/CSI** - QEMU cannot emulate the ESP32-S3 WiFi radio or CSI extraction hardware
2. **Synthetic CSI fidelity** - mock frames approximate real CSI patterns but don't capture real-world multipath, interference, or antenna characteristics
3. **Timing differences** - QEMU timing is not cycle-accurate; FreeRTOS tick rates may differ from hardware
4. **No peripheral testing** - I2C display, real GPIO, and light-sleep power management cannot be tested
5. **QEMU build requirement** - Espressif's QEMU fork must be built from source (not in Ubuntu packages)
6. **Coverage overhead** - gcov-enabled builds are ~2× slower in QEMU

### What QEMU Testing Covers vs Requires Hardware

| Test Domain | QEMU | Hardware |
|-------------|------|----------|
| Boot + NVS config (14 configs) | Full | Full |
| Edge DSP pipeline (biquad, Welford, top-K) | Full | Full |
| ADR-018 frame serialization | Full | Full |
| Vitals packet generation (0xC5110002) | Full | Full |
| WASM module loading + execution | Full | Full |
| Multi-node TDM mesh (3+ nodes) | Full (TAP) | Full |
| Fuzz testing (CSI parser, NVS) | Full | N/A |
| Code coverage analysis | Full | Partial |
| GDB breakpoint debugging | Full | Full (JTAG) |
| Chaos/fault injection | Full | Manual |
| OTA update flow | Partial (HTTP mock) | Full |
| Real WiFi connection | No | Full |
| Real CSI data quality | No | Full |
| Channel hopping on RF | No | Full |
| MAC filter on real frames | No | Full |
| Power management (light-sleep) | No | Full |
| Display rendering (OLED) | No | Full |
| UDP over real network | No | Full |

---

## Alternatives Considered

### 1. Host-native unit tests (no QEMU)
Extract pure C functions (`csi_serialize_frame`, edge DSP math) and compile/test on host with CMock/Unity. Simpler but doesn't test FreeRTOS integration, NVS, or boot sequence.

**Verdict**: Complementary - do both. Host unit tests for math, QEMU for integration. Fuzz targets (Layer 6) already use host-native compilation.

### 2. Hardware-in-the-loop CI (real ESP32 on runner)
Use a self-hosted GitHub Actions runner with a physical ESP32-S3 attached.

**Verdict**: Valuable but expensive and fragile. QEMU covers ~85% of test cases (up from 70% with all 9 layers). Add HIL later for real CSI validation only.

### 3. Docker-based ESP-IDF build only (no runtime test)
Just verify the firmware compiles in CI without running it.

**Verdict**: Already possible but insufficient - compilation doesn't catch runtime bugs (stack overflow, NVS parsing errors, FreeRTOS deadlocks).

### 4. Renode emulator
Alternative to QEMU with better peripheral modeling for some platforms.

**Verdict**: Renode has ESP32 support but ESP32-S3 support is less mature than Espressif's own QEMU fork. Revisit if Renode adds full S3 support.

---

## References

- [Espressif QEMU fork](https://github.com/espressif/qemu) - official ESP32/S3/C3/H2 support
- [ESP-IDF QEMU guide](https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/api-guides/tools/qemu.html)
- [libFuzzer documentation](https://llvm.org/docs/LibFuzzer.html) - LLVM-based coverage-guided fuzzing
- [lcov](https://github.com/linux-test-project/lcov) - Linux test coverage visualization
- ADR-018: Binary CSI frame format (magic `0xC5110001`)
- ADR-039: Edge intelligence pipeline (biquad, vitals, fall detection)
- ADR-040: WASM programmable sensing runtime
- ADR-057: Build-time CSI guard (`CONFIG_ESP_WIFI_CSI_ENABLED`)
- ADR-060: Channel override and MAC address filter

---

## Optimization Log (2026-03-14)

### Bugs Fixed

1. **LFSR float bias** - `lfsr_float()` used divisor 32767.5 producing range [-1.0, 1.00002]; fixed to 32768.0 for exact [-1.0, +1.0)
2. **MAC filter initialization** - `gen_mac_filter()` compared `frame_count == scenario_start_ms` (count vs timestamp); replaced with boolean flag
3. **Scenario infinite loop** - `advance_scenario()` looped to scenario 0 when all completed; now sets `s_all_done=true` and timer callback exits early
4. **Boot check severity** - `validate_qemu_output.py` reported no-boot as ERROR; upgraded to FATAL (nothing works without boot)
5. **NVS boundary configs** - `boundary-max` used `vital_win=65535` which firmware silently rejects (valid: 32-256); fixed to 256
6. **NVS boundary-min** - `vital_win=1` also invalid; fixed to 32 (firmware min)
7. **edge-tier2-custom** - `vital_win=512` exceeded firmware max of 256; fixed to 256
8. **power-save config** - Described as "10% duty cycle" but didn't set `power_duty=10`; fixed
9. **wasm-signed/unsigned** - Both configs were identical; signed now includes pubkey blob, unsigned sets `wasm_verify=0`

### Optimizations Applied

1. **SLIRP networking** - QEMU runner now passes `-nic user,model=open_eth` for UDP testing
2. **Scenario completion tracking** - Validator now checks `All N scenarios complete` log marker (check 15)
3. **Frame rate monitoring** - Validator extracts `scenario=N frames=M` counters for rate analysis (check 16)
4. **Watchdog tuning** - `sdkconfig.qemu` relaxes WDT to 30s / INT_WDT to 800ms for QEMU timing variance
5. **Timer stack depth** - Increased `FREERTOS_TIMER_TASK_STACK_DEPTH=4096` to prevent overflow from math-heavy mock callback
6. **Display disabled** - `CONFIG_DISPLAY_ENABLE=n` in QEMU overlay (no I2C hardware)
7. **CI fuzz job** - Added `fuzz-test` job running all 3 fuzz targets for 60s each with crash artifact upload
8. **CI NVS validation** - Added `nvs-matrix-validate` job that generates all 14 binaries and verifies sizes
9. **CI matrix expanded** - Added `edge-tier1`, `boundary-max`, `boundary-min` to QEMU test matrix (4 → 7 configs)
10. **QEMU cache key** - Uses `github.run_id` with restore-keys fallback to prevent stale QEMU builds
