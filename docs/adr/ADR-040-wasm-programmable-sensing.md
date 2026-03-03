# ADR-040: WASM Programmable Sensing (Tier 3)

**Status**: Accepted
**Date**: 2026-03-02
**Deciders**: @ruvnet

## Context

ADR-039 implemented Tiers 0-2 of the ESP32-S3 edge intelligence pipeline:
- **Tier 0**: Raw CSI passthrough (magic `0xC5110001`)
- **Tier 1**: Basic DSP — phase unwrap, Welford stats, top-K, delta compression
- **Tier 2**: Full pipeline — vitals, presence, fall detection, multi-person

The firmware uses ~820 KB of flash, leaving ~80 KB headroom in the 1 MB OTA partition. The ESP32-S3 has 8 MB PSRAM available for runtime data. New sensing algorithms (gesture recognition, signal coherence monitoring, adversarial detection) currently require a full firmware reflash — impractical for deployed sensor networks.

The project already has 35+ RuVector WASM crates and 28 pre-built `.wasm` binaries, but none are integrated into the ESP32 firmware.

## Decision

Add a **Tier 3 WASM programmable sensing layer** that executes hot-loadable algorithms compiled from Rust to `wasm32-unknown-unknown`, interpreted on-device via the WASM3 runtime.

### Architecture

```
Core 1 (DSP Task)
┌──────────────────────────────────────────────────┐
│ Tier 2 Pipeline (existing)                       │
│   Phase extract → Welford → Top-K → Biquad →    │
│   BPM → Presence → Fall → Multi-person           │
│                                                  │
│ ┌──────────────────────────────────────────────┐ │
│ │ Tier 3 WASM Runtime (new)                    │ │
│ │   WASM3 Interpreter (MIT, ~100 KB flash)     │ │
│ │   ┌────────────┐ ┌────────────┐              │ │
│ │   │ Module 0   │ │ Module 1   │ ...×4        │ │
│ │   │ gesture.wm │ │ coherence  │              │ │
│ │   └─────┬──────┘ └─────┬──────┘              │ │
│ │         │               │                     │ │
│ │    Host API ("csi" namespace)                 │ │
│ │    csi_get_phase, csi_get_amplitude, ...      │ │
│ └──────────────────────────────────────────────┘ │
│                      │                           │
│              UDP output (0xC5110004)              │
└──────────────────────────────────────────────────┘
```

### Components

| Component | File | Description |
|-----------|------|-------------|
| WASM3 component | `components/wasm3/CMakeLists.txt` | ESP-IDF managed component, fetches WASM3 from GitHub |
| Runtime host | `main/wasm_runtime.c/h` | WASM3 environment, module slots, host API bindings |
| HTTP upload | `main/wasm_upload.c/h` | REST endpoints for module management on port 8032 |
| Rust WASM crate | `wifi-densepose-wasm-edge/` | `no_std` sensing algorithms compiled to WASM |

### Host API (namespace "csi")

| Import | Signature | Description |
|--------|-----------|-------------|
| `csi_get_phase` | `(i32) -> f32` | Current phase for subcarrier index |
| `csi_get_amplitude` | `(i32) -> f32` | Current amplitude |
| `csi_get_variance` | `(i32) -> f32` | Welford running variance |
| `csi_get_bpm_breathing` | `() -> f32` | Breathing BPM from Tier 2 |
| `csi_get_bpm_heartrate` | `() -> f32` | Heart rate BPM from Tier 2 |
| `csi_get_presence` | `() -> i32` | Presence flag (0/1) |
| `csi_get_motion_energy` | `() -> f32` | Motion energy scalar |
| `csi_get_n_persons` | `() -> i32` | Detected person count |
| `csi_get_timestamp` | `() -> i32` | Milliseconds since boot |
| `csi_emit_event` | `(i32, f32) -> void` | Emit custom event to host |
| `csi_log` | `(i32, i32) -> void` | Debug log from WASM memory |
| `csi_get_phase_history` | `(i32, i32) -> i32` | Copy phase history ring buffer |

### Module Lifecycle

| Export | Called | Description |
|--------|--------|-------------|
| `on_init()` | Once, when module starts | Initialize module state |
| `on_frame(n_sc: i32)` | Per CSI frame (~20 Hz) | Process current frame |
| `on_timer()` | At configurable interval | Periodic tasks |

### Wire Protocol (magic `0xC5110004`)

| Offset | Type | Field |
|--------|------|-------|
| 0-3 | u32 LE | Magic `0xC5110004` |
| 4 | u8 | Node ID |
| 5 | u8 | Module ID (slot index) |
| 6-7 | u16 LE | Event count |
| 8+ | Event[] | Array of (u8 type, f32 value) tuples |

### HTTP Endpoints (port 8032)

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/wasm/upload` | Upload .wasm binary (max 128 KB) |
| `GET` | `/wasm/list` | List loaded modules with status |
| `POST` | `/wasm/start/:id` | Start a module |
| `POST` | `/wasm/stop/:id` | Stop a module |
| `DELETE` | `/wasm/:id` | Unload a module |

### WASM Crate Modules

| Module | Source | Events | Description |
|--------|--------|--------|-------------|
| `gesture.rs` | `ruvsense/gesture.rs` | 1 (Core) | DTW template matching for gesture recognition |
| `coherence.rs` | `ruvector/viewpoint/coherence.rs` | 2 (Core) | Phase phasor coherence monitoring |
| `adversarial.rs` | `ruvsense/adversarial.rs` | 3 (Core) | Signal anomaly/adversarial detection |
| `vital_trend.rs` | ADR-041 Phase 1 | 100-111 (Medical) | Clinical vital sign trend analysis (bradypnea, tachypnea, bradycardia, tachycardia, apnea) |
| `occupancy.rs` | ADR-041 Phase 1 | 300-302 (Building) | Spatial occupancy zone detection with per-zone variance analysis |
| `intrusion.rs` | ADR-041 Phase 1 | 200-203 (Security) | State-machine intrusion detector (calibrate-monitor-arm-alert) |

### Memory Budget

| Component | SRAM | PSRAM | Flash |
|-----------|------|-------|-------|
| WASM3 interpreter | ~10 KB | — | ~100 KB |
| WASM module storage (×4) | — | 512 KB | — |
| WASM execution stack | 8 KB | — | — |
| Host API bindings | 2 KB | — | ~15 KB |
| HTTP upload handler | 1 KB | — | ~8 KB |
| RVF parser + verifier | 1 KB | — | ~6 KB |
| **Total Tier 3** | **~22 KB** | **512 KB** | **~129 KB** |
| **Running total (Tier 0-3)** | **~34 KB** | **512 KB** | **~925 KB** |

**Measured binary size**: 925 KB (0xE7440 bytes), 10% free in 1 MB OTA partition.

### NVS Configuration

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `wasm_max` | u8 | 4 | Maximum concurrent WASM modules |
| `wasm_verify` | u8 | 1 | Require signature verification (secure-by-default) |
| `wasm_pubkey` | blob(32) | — | Signing public key for WASM verification |

## Consequences

### Positive
- Deploy new sensing algorithms to 1000+ nodes without reflashing firmware
- 20-year extensibility horizon — new algorithms via .wasm uploads
- Algorithms developed/tested in Rust, compiled to portable WASM
- PSRAM utilization (previously unused 8 MB) for module storage
- Hot-swap algorithms for A/B testing in production deployments
- Same `no_std` Rust code runs on ESP32 (WASM3) and in browser (wasm-pack)

### Negative
- WASM3 interpreter overhead: ~10× slower than native C for compute-heavy code
- Adds ~123 KB flash footprint (firmware approaches 950 KB of 1 MB limit)
- Additional attack surface via WASM module upload endpoint
- Debugging WASM modules on ESP32 is harder than native C

### Risks

| Risk | Mitigation |
|------|------------|
| WASM3 memory management may fragment PSRAM over time | Fixed 160 KB arenas pre-allocated at boot per slot — no runtime malloc/free cycles |
| Complex WASM modules (>64 KB) may cause stack overflow in interpreter | `WASM_STACK_SIZE` = 8 KB, `d_m3MaxFunctionStackHeight` = 128; modules validated at load time |
| HTTP upload endpoint requires network security | Ed25519 signature verification enabled by default (`wasm_verify=1`); disable only via NVS for lab/dev |
| Runaway WASM module blocks DSP pipeline | Per-frame budget guard (10 ms default); module auto-stopped after 10 consecutive faults |
| Denial-of-service via rapid upload/unload cycles | Max 4 concurrent slots; upload handler validates size before PSRAM copy |

## Implementation

- `firmware/esp32-csi-node/components/wasm3/CMakeLists.txt` — WASM3 ESP-IDF component
- `firmware/esp32-csi-node/main/wasm_runtime.c/h` — Runtime host with 12 API bindings + manifest
- `firmware/esp32-csi-node/main/wasm_upload.c/h` — HTTP REST endpoints (RVF-aware)
- `firmware/esp32-csi-node/main/rvf_parser.c/h` — RVF container parser and verifier
- `rust-port/.../wifi-densepose-wasm-edge/` — Rust WASM crate (gesture, coherence, adversarial, rvf, occupancy, vital_trend, intrusion)
- `rust-port/.../wifi-densepose-sensing-server/src/main.rs` — `0xC5110004` parser
- `docs/adr/ADR-039-esp32-edge-intelligence.md` — Updated with Tier 3 reference

---

## Appendix A: Production Hardening

The initial Tier 3 implementation addresses five production-readiness concerns:

### A.1 Fixed PSRAM Arenas

Dynamic `heap_caps_malloc` / `free` cycles on PSRAM fragment memory over days of
continuous operation. Instead, each module slot pre-allocates a **160 KB fixed arena**
at boot (`WASM_ARENA_SIZE`). The WASM binary and WASM3 runtime heap both live inside
this arena. Unloading a module zeroes the arena but never frees it — the slot is
reused on the next `wasm_runtime_load()`.

```
Boot:  [arena0: 160 KB][arena1: 160 KB][arena2: 160 KB][arena3: 160 KB]
                                                   Total: 640 KB PSRAM
Load:  [module0 binary | wasm3 heap | ...padding... ]
Unload:[zeroed .......................................]  ← slot reusable
```

This eliminates fragmentation at the cost of reserving 640 KB PSRAM at boot
(8% of 8 MB). The remaining 7.36 MB is available for future use.

### A.2 Per-Frame Budget Guard

Each `on_frame()` call is measured with `esp_timer_get_time()`. If execution
exceeds `WASM_FRAME_BUDGET_US` (default 10 ms = 10,000 us), a budget fault is
recorded. After **10 consecutive faults**, the module is auto-stopped with
`WASM_MODULE_ERROR` state. This prevents a runaway WASM module from blocking the
Tier 2 DSP pipeline.

```c
int64_t t_start = esp_timer_get_time();
m3_CallV(slot->fn_on_frame, n_sc);
uint32_t elapsed_us = (uint32_t)(esp_timer_get_time() - t_start);

slot->total_us += elapsed_us;
if (elapsed_us > slot->max_us) slot->max_us = elapsed_us;

if (elapsed_us > WASM_FRAME_BUDGET_US) {
    slot->budget_faults++;
    if (slot->budget_faults >= 10) {
        slot->state = WASM_MODULE_ERROR;  // auto-stop
    }
}
```

The budget is configurable via `WASM_FRAME_BUDGET_US` (Kconfig or NVS override).

### A.3 Per-Module Telemetry

The `/wasm/list` endpoint and `wasm_module_info_t` struct expose per-module
telemetry:

| Field | Type | Description |
|-------|------|-------------|
| `frame_count` | u32 | Total on_frame calls since start |
| `event_count` | u32 | Total csi_emit_event calls |
| `error_count` | u32 | WASM3 runtime errors |
| `total_us` | u32 | Cumulative execution time (microseconds) |
| `max_us` | u32 | Worst-case single frame execution time |
| `budget_faults` | u32 | Times frame budget was exceeded |

Mean execution time = `total_us / frame_count`. This enables remote monitoring
of module health and performance regression detection.

### A.4 Secure-by-Default

`wasm_verify` defaults to **1** in both Kconfig and the NVS fallback path.
Uploaded `.wasm` binaries must include a valid Ed25519 signature (same key as
OTA firmware). Disable only for lab/dev use via:

```bash
python provision.py --port COM7 --wasm-verify  # NVS: wasm_verify=1 (default)
# To disable in dev: write wasm_verify=0 to NVS directly
```

---

## Appendix B: Adaptive Budget Architecture (Mincut-Driven)

### B.1 Design Principle

One control loop turns **sensing into a bounded compute budget**, spends that
budget on **sparse or spiking inference**, and exports **only deltas**. The
budget is driven by the **mincut eigenvalue gap** (Δλ = λ₂ − λ₁ of the CSI
graph Laplacian), which reflects scene complexity: a quiet room has Δλ ≈ 0,
a busy room has large Δλ.

### B.2 Control Loop

```
                  ┌─────────────────────────────────┐
  CSI frames ───→ │ Tier 2 DSP (existing)            │
                  │  Welford stats, top-K, presence   │
                  └──────────┬────────────────────────┘
                             │
              ┌──────────────▼──────────────────────┐
              │ Budget Controller                    │
              │                                      │
              │  Inputs:                             │
              │    Δλ  = mincut eigenvalue gap        │
              │    A   = anomaly_score (adversarial)  │
              │    T   = thermal_pressure (0.0-1.0)   │
              │    P   = battery_pressure (0.0-1.0)   │
              │                                      │
              │  Output:                             │
              │    B   = frame compute budget (μs)    │
              │                                      │
              │  B = clamp(B₀ + k₁·max(0,Δλ)        │
              │            + k₂·A                    │
              │            − k₃·T                    │
              │            − k₄·P,                   │
              │       B_min, B_max)                   │
              └──────────────┬──────────────────────┘
                             │
              ┌──────────────▼──────────────────────┐
              │ WASM Module Dispatch                 │
              │  Budget B split across active modules│
              │  Each module gets B/N μs per frame   │
              └──────────────┬──────────────────────┘
                             │
              ┌──────────────▼──────────────────────┐
              │ Delta Export                          │
              │  Only emit events when Δ > threshold │
              │  Quiet room → near-zero UDP traffic   │
              └─────────────────────────────────────┘
```

### B.3 Budget Formula

```
B = clamp(B₀ + k₁·max(0, Δλ) + k₂·A − k₃·T − k₄·P, B_min, B_max)
```

| Symbol | Default | Description |
|--------|---------|-------------|
| B₀ | 5,000 μs | Base budget (5 ms) |
| k₁ | 2,000 | Δλ sensitivity (more scene change → more budget) |
| k₂ | 3,000 | Anomaly boost (detected anomaly → more compute) |
| k₃ | 4,000 | Thermal penalty (chip hot → less compute) |
| k₄ | 3,000 | Battery penalty (low SoC → less compute) |
| B_min | 1,000 μs | Floor: always run at least 1 ms |
| B_max | 15,000 μs | Ceiling: never exceed 15 ms |

### B.4 Where Δλ Comes From

The mincut graph is the **top-K subcarrier correlation graph** already
maintained by Tier 1/2 DSP. Subcarriers are nodes; edge weights are
pairwise Pearson correlation magnitudes over the Welford window. The
algebraic connectivity (Fiedler value λ₂) of this graph's Laplacian
approximates the mincut value. On ESP32-S3 with K=8 subcarriers, this
is an 8×8 eigenvalue problem — solvable with power iteration in <100 μs.

### B.5 Spiking and Sparse Optimizations

When the budget is tight (Δλ ≈ 0, quiet room), WASM modules should:

1. **Skip on_frame entirely** if Δλ < ε (no scene change → no computation)
2. **Sparse inference**: Only process the top-K subcarriers that changed
   (already tracked by Tier 1 delta compression)
3. **Spiking semantics**: Modules emit events only when state transitions
   occur, not on every frame. The host tracks a per-module "last emitted"
   state and suppresses duplicate events.

### B.6 Thermal and Power Hooks

ESP32-S3 provides:
- `temp_sensor_read()` — on-chip temperature (°C)
- ADC reading of battery voltage (if wired)

Thermal pressure: `T = clamp((temp_celsius - 60) / 20, 0, 1)` — ramps
from 0 at 60°C to 1.0 at 80°C (thermal throttle zone).

Battery pressure: `P = clamp((3.3 - battery_volts) / 0.6, 0, 1)` — ramps
from 0 at 3.3V to 1.0 at 2.7V (brownout zone).

### B.7 Transport Strategy

WASM output packets (`0xC5110004`) adopt **delta-only export**:

- Events are only emitted when the value changes by more than a
  configurable dead-band (default: 5% of previous value)
- Quiet room = zero WASM UDP packets (only Tier 2 vitals at 1 Hz)
- Busy room = bursty WASM events, naturally rate-limited by budget B

Future work: QUIC-lite transport with 0-RTT connection resumption and
congestion-aware pacing, replacing raw UDP for WASM event streams.

---

## Appendix C: Hardware Benchmark (RuView ESP32-S3)

Measured on ESP32-S3 (QFN56 rev v0.2, 8 MB flash, 160 MHz, ESP-IDF v5.2,
board without PSRAM). WiFi connected to AP at RSSI -25 dBm, channel 5 BW20.

### WASM Runtime Performance

| Metric | Value |
|--------|-------|
| WASM runtime init | **106 ms** |
| Total boot to ready | **3.9 s** (including WiFi connect) |
| Module slots | 4 × 160 KB (heap fallback, no PSRAM) |
| WASM binary size (7 modules) | **13.8 KB** (wasm32-unknown-unknown release) |
| Frame budget | 10,000 µs (10 ms) |
| Timer interval | 1,000 ms (1 Hz) |

### CSI Throughput

| Metric | Value |
|--------|-------|
| Frame rate | **28.5 Hz** (exceeds 20 Hz estimate) |
| Frame sizes | 128 / 256 bytes |
| Per-frame interval | 30.6 ms avg |
| RSSI range | -83 to -32 dBm (mean -62 dBm) |

### Rust Test Results

| Crate | Tests | Status |
|-------|-------|--------|
| wifi-densepose-wasm-edge (std) | 14 | All pass, 0 warnings |
| Full workspace | 1,411 | All pass, 0 failed |

### Known Issues

1. **Fall threshold too sensitive** — default 2.0 rad/s² produces 6.7 false positives/s in static environment. Recommend 5.0-8.0 for deployment.
2. **No PSRAM on test board** — WASM arenas fall back to internal heap (316 KiB total). Production boards with 8 MB PSRAM will use dedicated PSRAM arenas.
3. **WiFi-Ethernet isolation** — some consumer routers block bridging between WiFi and wired clients. Verify network path during deployment.

### B.8 Implementation Plan

| Step | Scope | Effort |
|------|-------|--------|
| 1 | Add `edge_compute_fiedler()` in `edge_processing.c` — power iteration on 8×8 Laplacian | ~50 lines C |
| 2 | Add budget controller struct and update formula in `wasm_runtime.c` | ~30 lines C |
| 3 | Wire thermal/battery sensors into budget inputs | ~20 lines C |
| 4 | Add delta-export dead-band filter in `wasm_runtime_on_frame()` | ~15 lines C |
| 5 | NVS keys for k₁-k₄, B_min, B_max, dead-band threshold | ~10 lines C |

Total: ~125 lines of C, no new files. All constants configurable via NVS.

### B.9 Failure Modes

| Failure | Behavior |
|---------|----------|
| Δλ estimate wrong (correlation noise) | Budget oscillates — clamped by B_min/B_max |
| Thermal sensor absent | T defaults to 0 (no throttle) |
| Battery ADC not wired | P defaults to 0 (always-on mode) |
| All WASM modules budget-faulted | DSP pipeline runs Tier 2 only — graceful degradation |

---

## Appendix C: RVF Container Format

### C.1 Problem

Raw `.wasm` uploads over HTTP are remote code execution. Signatures solve
authenticity, but without a manifest the host has no way to enforce budgets,
check API compatibility, or identify what it's running. RVF wraps the WASM
payload with governance metadata in a single artifact.

### C.2 Binary Layout

```
Offset  Size   Type     Field
────────────────────────────────────────────
0       4      [u8;4]   Magic "RVF\x01" (0x01465652 LE)
4       2      u16 LE   format_version (1)
6       2      u16 LE   flags (bit 0: has_signature, bit 1: has_test_vectors)
8       4      u32 LE   manifest_len (always 96)
12      4      u32 LE   wasm_len
16      4      u32 LE   signature_len (0 or 64)
20      4      u32 LE   test_vectors_len (0 if none)
24      4      u32 LE   total_len (header + manifest + wasm + sig + tvec)
28      4      u32 LE   reserved (0)
────────────────────────────────────────────
32      96     struct   Manifest (see below)
128     N      bytes    WASM payload ("\0asm" magic)
128+N   0|64   bytes    Ed25519 signature (signs bytes 0..128+N-1)
128+N+S M      bytes    Test vectors (optional)
```

Total overhead: 32 (header) + 96 (manifest) + 64 (signature) = **192 bytes**.

### C.3 Manifest (96 bytes, packed)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0 | 32 | char[] | `module_name` — null-terminated ASCII |
| 32 | 2 | u16 | `required_host_api` — version (1 = current) |
| 34 | 4 | u32 | `capabilities` — RVF_CAP_* bitmask |
| 38 | 4 | u32 | `max_frame_us` — requested per-frame budget (0 = use default) |
| 42 | 2 | u16 | `max_events_per_sec` — rate limit (0 = unlimited) |
| 44 | 2 | u16 | `memory_limit_kb` — max WASM heap (0 = use default) |
| 46 | 2 | u16 | `event_schema_version` — for receiver compatibility |
| 48 | 32 | [u8;32] | `build_hash` — SHA-256 of WASM payload |
| 80 | 2 | u16 | `min_subcarriers` — minimum required (0 = any) |
| 82 | 2 | u16 | `max_subcarriers` — maximum expected (0 = any) |
| 84 | 10 | char[] | `author` — null-padded ASCII |
| 94 | 2 | [u8;2] | reserved (0) |

### C.4 Capability Bitmask

| Bit | Flag | Host API functions |
|-----|------|--------------------|
| 0 | `READ_PHASE` | `csi_get_phase` |
| 1 | `READ_AMPLITUDE` | `csi_get_amplitude` |
| 2 | `READ_VARIANCE` | `csi_get_variance` |
| 3 | `READ_VITALS` | `csi_get_bpm_*`, `csi_get_presence`, `csi_get_n_persons` |
| 4 | `READ_HISTORY` | `csi_get_phase_history` |
| 5 | `EMIT_EVENTS` | `csi_emit_event` |
| 6 | `LOG` | `csi_log` |

Modules declare which host APIs they need. Future firmware versions may
refuse to link imports that aren't declared in capabilities — defense in
depth against supply-chain attacks.

### C.5 On-Device Flow

```
HTTP POST /wasm/upload
     │
     ▼
 ┌────────────────────────┐
 │ Check first 4 bytes    │
 │  "RVF\x01" → RVF path │
 │  "\0asm"   → raw path  │
 └───────┬────────────────┘
         │
    ┌────▼────┐     ┌───────────┐
    │ RVF     │     │ Raw WASM  │
    │ parse   │     │ (dev only,│
    │ header  │     │ verify=0) │
    └────┬────┘     └─────┬─────┘
         │                │
    ┌────▼────┐           │
    │ Verify  │           │
    │ SHA-256 │           │
    │ hash    │           │
    └────┬────┘           │
         │                │
    ┌────▼────┐           │
    │ Verify  │           │
    │ Ed25519 │           │
    │ sig     │           │
    └────┬────┘           │
         │                │
    ┌────▼────┐           │
    │ Check   │           │
    │ host API│           │
    │ version │           │
    └────┬────┘           │
         │                │
         ├────────────────┘
         ▼
 ┌───────────────────┐
 │ wasm_runtime_load │
 │ set_manifest      │
 │ start module      │
 └───────────────────┘
```

### C.6 Rollback Support

Each slot stores the SHA-256 build hash from the manifest. The `/wasm/list`
endpoint returns this hash. Fleet management systems can:

1. Push an RVF to a node
2. Verify the installed hash matches via GET `/wasm/list`
3. Roll back by pushing the previous RVF (same slot reused after unload)

Two-slot strategy: maintain slot 0 as "last known good" and slot 1 as
"candidate". Promote by stopping slot 0 and starting slot 1.

### C.7 Rust Builder

The `wifi-densepose-wasm-edge` crate provides `rvf::builder::build_rvf()`
(behind the `std` feature) to package a `.wasm` binary into an `.rvf`:

```rust
use wifi_densepose_wasm_edge::rvf::builder::{build_rvf, RvfConfig};

let wasm = std::fs::read("target/wasm32-unknown-unknown/release/module.wasm")?;
let rvf = build_rvf(&wasm, &RvfConfig {
    module_name: "gesture".into(),
    author: "rUv".into(),
    capabilities: CAP_READ_PHASE | CAP_EMIT_EVENTS,
    max_frame_us: 5000,
    ..Default::default()
});
std::fs::write("gesture.rvf", &rvf)?;
// Then sign externally with Ed25519 and patch_signature()
```

### C.8 Implementation Files

| File | Description |
|------|-------------|
| `firmware/.../main/rvf_parser.h` | RVF types, capability flags, parse/verify API |
| `firmware/.../main/rvf_parser.c` | Header/manifest parser, SHA-256 hash check |
| `wifi-densepose-wasm-edge/src/rvf.rs` | Format constants, builder (std), tests |

### C.9 Failure Modes

| Failure | Behavior |
|---------|----------|
| RVF too large for PSRAM buffer | Rejected at receive with 400 |
| Build hash mismatch | Rejected at parse with `ESP_ERR_INVALID_CRC` |
| Signature absent when `wasm_verify=1` | Rejected with 403 |
| Host API version too new | Rejected with `ESP_ERR_NOT_SUPPORTED` |
| Raw WASM when `wasm_verify=1` | Rejected with 403 |
