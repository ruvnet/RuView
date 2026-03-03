# Edge Intelligence Modules — WiFi-DensePose

> 60 WASM modules that run directly on an ESP32 sensor. No internet needed, no cloud fees, instant response. Each module is a tiny file (5-30 KB) that reads WiFi signal data and makes decisions locally in under 10 ms.

## Quick Start

```bash
# Build all modules for ESP32
cd rust-port/wifi-densepose-rs/crates/wifi-densepose-wasm-edge
cargo build --target wasm32-unknown-unknown --release

# Run all 632 tests
cargo test --features std

# Upload a module to your ESP32
python scripts/wasm_upload.py --port COM7 --module target/wasm32-unknown-unknown/release/module_name.wasm
```

## Module Categories

| | Category | Modules | Tests | Documentation |
|---|----------|---------|-------|---------------|
| | **Core** | 7 | 81 | [core.md](core.md) |
| | **Medical & Health** | 5 | 38 | [medical.md](medical.md) |
| | **Security & Safety** | 6 | 42 | [security.md](security.md) |
| | **Smart Building** | 5 | 38 | [building.md](building.md) |
| | **Retail & Hospitality** | 5 | 38 | [retail.md](retail.md) |
| | **Industrial** | 5 | 38 | [industrial.md](industrial.md) |
| | **Exotic & Research** | 10 | ~60 | [exotic.md](exotic.md) |
| | **Signal Intelligence** | 6 | 54 | [signal-intelligence.md](signal-intelligence.md) |
| | **Adaptive Learning** | 4 | 42 | [adaptive-learning.md](adaptive-learning.md) |
| | **Spatial & Temporal** | 6 | 56 | [spatial-temporal.md](spatial-temporal.md) |
| | **AI Security** | 2 | 20 | [ai-security.md](ai-security.md) |
| | **Quantum & Autonomous** | 4 | 30 | [autonomous.md](autonomous.md) |
| | **Total** | **65** | **632** | |

## How It Works

1. **WiFi signals bounce off people and objects** in a room, creating a unique pattern
2. **The ESP32 chip reads these patterns** as Channel State Information (CSI) — 52 numbers that describe how each WiFi channel changed
3. **WASM modules analyze the patterns** to detect specific things: someone fell, a room is occupied, breathing rate changed
4. **Events are emitted locally** — no cloud round-trip, response time under 10 ms

## Architecture

```
WiFi Router ──── radio waves ────→ ESP32-S3 Sensor
                                      │
                                      ▼
                              ┌──────────────┐
                              │  Tier 0-2    │  C firmware: phase unwrap,
                              │  DSP Engine  │  stats, top-K selection
                              └──────┬───────┘
                                      │ CSI frame (52 subcarriers)
                                      ▼
                              ┌──────────────┐
                              │   WASM3      │  Tiny interpreter
                              │   Runtime    │  (60 KB overhead)
                              └──────┬───────┘
                                      │
                          ┌───────────┼───────────┐
                          ▼           ▼           ▼
                    ┌──────────┐ ┌──────────┐ ┌──────────┐
                    │ Module A │ │ Module B │ │ Module C │
                    │ (5-30KB) │ │ (5-30KB) │ │ (5-30KB) │
                    └────┬─────┘ └────┬─────┘ └────┬─────┘
                         │           │           │
                         └───────────┼───────────┘
                                     ▼
                              Events + Alerts
                         (UDP to aggregator or local)
```

## Host API

Every module talks to the ESP32 through 12 functions:

| Function | Returns | Description |
|----------|---------|-------------|
| `csi_get_phase(i)` | `f32` | WiFi signal phase angle for subcarrier `i` |
| `csi_get_amplitude(i)` | `f32` | Signal strength for subcarrier `i` |
| `csi_get_variance(i)` | `f32` | How much subcarrier `i` fluctuates |
| `csi_get_bpm_breathing()` | `f32` | Breathing rate (BPM) |
| `csi_get_bpm_heartrate()` | `f32` | Heart rate (BPM) |
| `csi_get_presence()` | `i32` | Is anyone there? (0/1) |
| `csi_get_motion_energy()` | `f32` | Overall movement level |
| `csi_get_n_persons()` | `i32` | Estimated number of people |
| `csi_get_timestamp()` | `i32` | Current timestamp (ms) |
| `csi_emit_event(id, val)` | — | Send a detection result to the host |
| `csi_log(ptr, len)` | — | Log a message to serial console |
| `csi_get_phase_history(buf, max)` | `i32` | Past phase values for trend analysis |

## Event ID Registry

| Range | Category | Example Events |
|-------|----------|---------------|
| 0-99 | Core | Gesture detected, coherence score, anomaly |
| 100-199 | Medical | Apnea, bradycardia, tachycardia, seizure |
| 200-299 | Security | Intrusion, perimeter breach, loitering, panic |
| 300-399 | Smart Building | Zone occupied, HVAC, lighting, elevator, meeting |
| 400-499 | Retail | Queue length, dwell zone, customer flow, turnover |
| 500-599 | Industrial | Proximity warning, confined space, vibration |
| 600-699 | Exotic | Sleep stage, emotion, gesture language, rain |
| 700-729 | Signal Intelligence | Attention, coherence gate, compression, recovery |
| 730-759 | Adaptive Learning | Gesture learned, attractor, adaptation, EWC |
| 760-789 | Spatial Reasoning | Influence, HNSW match, spike tracking |
| 790-819 | Temporal Analysis | Pattern, LTL violation, GOAP goal |
| 820-849 | AI Security | Replay attack, injection, jamming, behavior |
| 850-879 | Quantum-Inspired | Entanglement, decoherence, hypothesis |
| 880-899 | Autonomous | Inference, rule fired, mesh reconfigure |

## Module Development

### Adding a New Module

1. Create `src/your_module.rs` following the pattern:
   ```rust
   #![cfg_attr(not(feature = "std"), no_std)]
   #[cfg(not(feature = "std"))]
   use libm::fabsf;

   pub struct YourModule { /* fixed-size fields only */ }

   impl YourModule {
       pub const fn new() -> Self { /* ... */ }
       pub fn process_frame(&mut self, /* inputs */) -> &[(i32, f32)] { /* ... */ }
   }
   ```

2. Add `pub mod your_module;` to `lib.rs`
3. Add event constants to `event_types` in `lib.rs`
4. Add tests with `#[cfg(test)] mod tests { ... }`
5. Run `cargo test --features std`

### Constraints

- **No heap allocation**: Use fixed-size arrays, not `Vec` or `String`
- **No `std`**: Use `libm` for math functions
- **Budget tiers**: L (<2ms), S (<5ms), H (<10ms) per frame
- **Binary size**: Each module should be 5-30 KB as WASM

## References

- [ADR-039](../adr/ADR-039-esp32-edge-intelligence.md) — Edge processing tiers
- [ADR-040](../adr/ADR-040-wasm-programmable-sensing.md) — WASM runtime design
- [ADR-041](../adr/ADR-041-wasm-module-collection.md) — Full module specification
- [Source code](../../rust-port/wifi-densepose-rs/crates/wifi-densepose-wasm-edge/src/)
