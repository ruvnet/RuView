# ADR-081: Adaptive CSI Mesh Firmware Kernel

| Field       | Value                                                                 |
|-------------|-----------------------------------------------------------------------|
| **Status**  | Accepted — Layers 1/2/3/4/5 implemented and host-tested; mesh RX path and Ed25519 signing tracked as Phase 3.5 polish |
| **Date**    | 2026-04-19                                                            |
| **Authors** | ruv                                                                   |
| **Depends** | ADR-018, ADR-028, ADR-029, ADR-031, ADR-032, ADR-039, ADR-066, ADR-073 |

## Context

RuView's firmware grew bottom-up. ADR-018 defined a binary CSI frame, ADR-029
added channel hopping and TDM, ADR-039 added a tiered edge-intelligence
pipeline, ADR-040 added programmable WASM modules, ADR-060 added per-node
channel and MAC overrides, ADR-066 added a swarm bridge to a coordinator, and
ADR-073 added multifrequency mesh scanning. Each one was a sound local
decision. Together they produced a firmware that works on ESP32-S3 but is
**implicitly coupled** to that chipset through `csi_collector.c` calling
`esp_wifi_*` directly and through hard-coded assumptions about the WiFi driver
callback shape.

This is a problem for three reasons:

1. **Portability.** Espressif exposes CSI through an official driver API. On
   locked Broadcom and Cypress chips, projects like Nexmon achieve the same
   thing by patching the firmware blob — but only for specific chip and
   firmware build combinations. Future RuView nodes will likely span both
   models plus eventually a custom silicon path. Today, none of the modules
   above can be reused unchanged on any non-ESP32 chip.

2. **Adaptivity.** The current firmware reacts to configuration, not to
   conditions. Channel hop intervals, edge tier, vitals cadence, top-K
   subcarriers, fall threshold, and power duty are all read from NVS at boot
   and never revisited. There is no closed-loop control: if a channel becomes
   congested, if motion spikes, if inter-node coherence drops, or if the
   environment is stable enough to coast at lower cadence, nothing changes
   onboard. The adaptive classifier in `wifi-densepose-sensing-server` does
   adapt — but only on the host side, after the data has already traversed the
   network at fixed rate.

3. **Mesh as an afterthought.** ADR-029 wired in a `TdmCoordinator` and ADR-066
   added a swarm bridge to a Cognitum Seed, but there is no first-class node
   role enumeration (anchor / observer / fusion-relay / coordinator), no
   role-assignment protocol, no `FEATURE_DELTA` message type, no
   coordinator-driven channel plan, and no automatic role re-election when a
   node drops. Multi-node deployments today are stitched together by manual
   per-node NVS provisioning.

The hard truth is that the firmware hack — getting raw CSI off a radio — is
not the moat. The moat is **adaptive control, multi-node fusion, compact
state encoding, persistent memory, and contrastive reasoning on top of the
radio layer**. The current architecture does not name those layers, so they
get reinvented inline by every new ADR.

## Decision

Adopt a **5-layer adaptive RF sensing kernel** as the canonical RuView
firmware architecture, and refactor the existing modules to fit underneath
it. The five layers, top to bottom:

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Layer 5 — Rust handoff                                                  │
│   Two streams only: feature_state (default) and debug_csi_frame (gated) │
└─────────────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────────────┐
│ Layer 4 — On-device feature extraction                                  │
│   100 ms motion, 1 s respiration, 5 s baseline windows                  │
│   Emits compact rv_feature_state_t (magic 0xC5110006)                   │
└─────────────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────────────┐
│ Layer 3 — Mesh sensing plane                                            │
│   Roles: Anchor / Observer / Fusion relay / Coordinator                 │
│   Messages: TIME_SYNC, ROLE_ASSIGN, CHANNEL_PLAN, CALIBRATION_START,    │
│             FEATURE_DELTA, HEALTH, ANOMALY_ALERT                        │
└─────────────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────────────┐
│ Layer 2 — Adaptive controller                                           │
│   Fast loop  ~200 ms — packet rate, active probing                      │
│   Medium loop ~1 s  — channel selection, role changes                   │
│   Slow loop ~30 s  — baseline recalibration                             │
└─────────────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────────────┐
│ Layer 1 — Radio Abstraction Layer (rv_radio_ops_t vtable)               │
│   ESP32 binding, future Nexmon binding, future custom silicon binding   │
└─────────────────────────────────────────────────────────────────────────┘
```

### Layer 1 — Radio Abstraction Layer

A single function-pointer vtable, `rv_radio_ops_t`, defined in
`firmware/esp32-csi-node/main/rv_radio_ops.h`:

```c
typedef struct {
    int (*init)(void);
    int (*set_channel)(uint8_t ch, uint8_t bw);
    int (*set_mode)(uint8_t mode);            /* RV_RADIO_MODE_* */
    int (*set_csi_enabled)(bool en);
    int (*set_capture_profile)(uint8_t profile_id);
    int (*get_health)(rv_radio_health_t *out);
} rv_radio_ops_t;
```

Capture profiles, named not numbered:

| Profile                        | Intent                                                |
|--------------------------------|-------------------------------------------------------|
| `RV_PROFILE_PASSIVE_LOW_RATE`  | Default idle: minimum cadence, presence only          |
| `RV_PROFILE_ACTIVE_PROBE`      | Inject NDP frames at high rate                        |
| `RV_PROFILE_RESP_HIGH_SENS`    | Quietest channel, longest window, vitals-only         |
| `RV_PROFILE_FAST_MOTION`       | Short window, high cadence                            |
| `RV_PROFILE_CALIBRATION`       | Synchronized burst across nodes                       |

Two bindings ship in this ADR:

- **ESP32 binding** (`rv_radio_ops_esp32.c`) wraps `csi_collector.c`,
  `esp_wifi_set_channel()`, `esp_wifi_set_csi()`, and
  `csi_inject_ndp_frame()`.
- **Mock binding** (`rv_radio_ops_mock.c`) wraps `mock_csi.c` so QEMU
  scenarios can exercise the controller and mesh plane without a radio.

A third binding (Nexmon-patched Broadcom) is reserved but not implemented
here.

### Layer 2 — Adaptive controller

`firmware/esp32-csi-node/main/adaptive_controller.{c,h}`. A single FreeRTOS
task with three cooperating timers:

| Loop   | Period  | Inputs                                                                 | Outputs                                              |
|--------|---------|------------------------------------------------------------------------|------------------------------------------------------|
| Fast   | ~200 ms | packet yield, retry/drop rate, motion score                            | cadence (vital_interval_ms), active vs passive probe |
| Medium | ~1 s    | CSI variance, RSSI median, channel occupancy, inter-node agreement     | channel selection (via radio ops), role transitions  |
| Slow   | ~30 s   | drift profile (Stable/Linear/StepChange), respiration confidence       | baseline recalibration, switch to delta-only mode    |

The controller publishes its decisions through the radio ops vtable
(`set_capture_profile`, `set_channel`) and through the mesh plane
(`CHANNEL_PLAN`, `ROLE_ASSIGN`). Default policy is conservative and matches
today's behavior; aggressive adaptation is opt-in via Kconfig.

### Layer 3 — Mesh sensing plane

Extends `swarm_bridge.c` with explicit node roles (Anchor / Observer /
Fusion relay / Coordinator) and a 7-message type protocol:

| Message              | Cadence            | Sender(s)        | Purpose                                       |
|----------------------|--------------------|------------------|-----------------------------------------------|
| `TIME_SYNC`          | 100 ms             | Anchor           | Reuse ADR-032 `SyncBeacon` (28 bytes, HMAC)   |
| `ROLE_ASSIGN`        | event-driven       | Coordinator      | Node ID → role mapping                         |
| `CHANNEL_PLAN`       | event-driven       | Coordinator      | Per-node channel + dwell schedule              |
| `CALIBRATION_START`  | event-driven       | Coordinator      | Synchronized calibration burst                 |
| `FEATURE_DELTA`      | 1–10 Hz            | Observer / Relay | Compact feature delta (see Layer 4)            |
| `HEALTH`             | 1 Hz               | All              | `rv_node_status_t` (see below)                 |
| `ANOMALY_ALERT`      | event-driven       | Observer         | Phase-physics violation, multi-link mismatch   |

Node status payload:

```c
typedef struct __attribute__((packed)) {
    uint8_t  node_id[8];
    uint64_t local_time_us;
    uint8_t  role;
    uint8_t  current_channel;
    uint8_t  current_bw;
    int8_t   noise_floor_dbm;
    uint16_t pkt_yield;
    uint16_t sync_error_us;
    uint16_t health_flags;
} rv_node_status_t;
```

Time-sync target is an engineering goal, not a guaranteed constant — it
depends on the clock quality of the chosen radio family. The first
acceptance test (Phase 2) measures it on real hardware.

### Layer 4 — On-device feature extraction

Defined in `firmware/esp32-csi-node/main/rv_feature_state.h`. Single
on-the-wire packet, **60 bytes packed** (verified by `_Static_assert` and
host unit test), magic `0xC5110006` (next free after ADR-039's
`0xC5110002`, ADR-069's `0xC5110003`, ADR-063's `0xC5110004`, and ADR-039's
compressed `0xC5110005`):

```c
#define RV_FEATURE_STATE_MAGIC  0xC5110006u

typedef struct __attribute__((packed)) {
    uint32_t magic;             /* RV_FEATURE_STATE_MAGIC */
    uint8_t  node_id;
    uint8_t  mode;              /* RV_PROFILE_* identifier */
    uint16_t seq;               /* monotonic per-node sequence */
    uint64_t ts_us;             /* node-local microseconds */
    float    motion_score;
    float    presence_score;
    float    respiration_bpm;
    float    respiration_conf;
    float    heartbeat_bpm;
    float    heartbeat_conf;
    float    anomaly_score;
    float    env_shift_score;
    float    node_coherence;
    uint16_t quality_flags;
    uint16_t reserved;
    uint32_t crc32;             /* IEEE polynomial over bytes [0..end-4] */
} rv_feature_state_t;

_Static_assert(sizeof(rv_feature_state_t) == 60,
               "rv_feature_state_t must be 60 bytes on the wire");
```

Three windows feed it: 100 ms (motion), 1 s (respiration), 5 s (baseline /
env shift). Each `rv_feature_state_t` represents the most recent state of
all three; mode field tells the receiver which window dominates this
update.

`rv_feature_state_t` does not replace ADR-039's `edge_vitals_pkt_t`
(0xC5110002) or ADR-063's `edge_fused_vitals_pkt_t` (0xC5110004). Those
remain the wire format for vitals-specific consumers. `rv_feature_state_t`
is the **default upstream payload** for the sensing pipeline; vitals
packets are now an alternate emission mode for backward compatibility.

### Layer 5 — Rust handoff

The Rust side sees only two streams from a node:

1. **`feature_state` stream** — `rv_feature_state_t`, default-on, 1–10 Hz.
2. **`debug_csi_frame` stream** — ADR-018 raw frames (magic 0xC5110001),
   default-off, opt-in via NVS or `CHANNEL_PLAN`. Used for calibration,
   debugging, training-set capture.

The Rust handoff is mirrored as a trait in
`crates/wifi-densepose-hardware/src/radio_ops.rs` so test harnesses (and
eventually the Rust-side controller for centralized coordinator nodes) can
swap radio backends without touching `wifi-densepose-signal`,
`wifi-densepose-ruvector`, `wifi-densepose-train`, or
`wifi-densepose-mat`. Rust-side mirror trait is **out of scope for the
firmware-only PR** that ships this ADR; tracked as Phase 4 follow-up.

## State Machine

```
BOOT → SELF_TEST → RADIO_INIT → TIME_SYNC → CALIBRATION → SENSE_IDLE
                                                            ↓ ↑
                                                         SENSE_ACTIVE
                                                            ↓
                                                          ALERT
                                                            ↓
                                                        DEGRADED
```

Transitions:

- **CALIBRATION** on boot, on role change, on sustained inter-node
  disagreement.
- **SENSE_ACTIVE** when motion or anomaly score crosses threshold.
- **DEGRADED** when packet yield, sync quality, or memory pressure drops
  below threshold; falls back to ADR-039 Tier-0 raw passthrough as the
  last-resort survivable mode.

## Data budgets

| Stream                  | Default rate                | Notes                                        |
|-------------------------|-----------------------------|----------------------------------------------|
| Raw capture (internal)  | 50–200 pps per observer     | Stays on-device unless debug stream enabled  |
| `rv_feature_state_t`    | 1–10 Hz per node            | Default upstream                             |
| `ANOMALY_ALERT`         | event-driven                | Burst-bounded                                |
| Debug ADR-018 raw CSI   | 0 (off by default)          | Burst-only via `CHANNEL_PLAN` debug flag     |

ADR-039 measured raw CSI at ~5 KB/frame and ~100 KB/s per node. The default
upstream with ADR-081's 60-byte `rv_feature_state_t` at 5 Hz is **300 B/s
per node — a 99.7% reduction**. A 50-node deployment at 5 Hz fits in
15 KB/s total, easily carried by a single-AP backhaul.

## Channel planning policy

Codified rules — these are constraints on the controller, not just defaults:

- Keep one anchor on a stable channel; observers distributed across the
  least-congested channels.
- Rotate **one** observer at a time. Never change all nodes simultaneously.
- Pin `RV_PROFILE_RESP_HIGH_SENS` to the quietest stable channel for the
  duration of a respiration window.
- Use a short active burst on a quiet channel for calibration, then return
  to passive capture.

This generalizes the per-deployment policy in ADR-073 ("node 1: ch 1/6/11,
node 2: ch 3/5/9") into a controller-driven plan that the coordinator can
publish via `CHANNEL_PLAN`. IEEE 802.11bf is the standards direction this
points toward.

## Security & integrity

- Every `FEATURE_DELTA` carries node id, monotonic seq, ts_us, and CRC32
  (IEEE polynomial), per the struct above.
- Every control message (`ROLE_ASSIGN`, `CHANNEL_PLAN`, `CALIBRATION_START`)
  carries sender role, epoch, replay window index, and authorization class,
  reusing the HMAC-SHA256 + 16-frame replay window from ADR-032
  (`secure_tdm.rs`).
- Optional Ed25519 signature at session/batch granularity for signed
  `CHANNEL_PLAN` and `CALIBRATION_START` messages, reusing the
  ADR-040/RVF Ed25519 path already shipping in firmware.

## Reuse map (do not rewrite)

| Concern                     | Existing component                                                                                       |
|-----------------------------|----------------------------------------------------------------------------------------------------------|
| ADR-018 binary frame        | `firmware/esp32-csi-node/main/csi_collector.c` (magic `0xC5110001`)                                      |
| ESP32 CSI driver glue       | `firmware/esp32-csi-node/main/csi_collector.c:225-303`                                                   |
| Channel hopping             | `csi_collector_set_hop_table()` and `csi_collector_start_hop_timer()`                                    |
| NDP injection               | `csi_inject_ndp_frame()` (placeholder, sufficient for L1 binding)                                        |
| TDM scheduling              | `crates/wifi-densepose-hardware/src/esp32/tdm.rs`                                                        |
| Secure beacons              | `crates/wifi-densepose-hardware/src/esp32/secure_tdm.rs` (HMAC + replay)                                 |
| Edge intelligence (Tier 1/2)| `firmware/esp32-csi-node/main/edge_processing.c` (magic `0xC5110002`/`0xC5110005`)                       |
| Fused vitals                | ADR-063 `edge_fused_vitals_pkt_t` (magic `0xC5110004`)                                                   |
| Swarm bridge                | `firmware/esp32-csi-node/main/swarm_bridge.c`                                                            |
| WASM Tier 3 modules         | `firmware/esp32-csi-node/main/wasm_runtime.c` (ADR-040)                                                  |
| Multistatic fusion          | `crates/wifi-densepose-ruvector/src/viewpoint/fusion.rs`                                                 |
| Adaptive classifier         | `crates/wifi-densepose-sensing-server/src/adaptive_classifier.rs:61-75`                                  |
| Feature primitives (Rust)   | `crates/wifi-densepose-signal/src/{motion.rs,features.rs,ruvsense/coherence.rs}`                         |

## Implementation status (2026-04-19)

This ADR ships **with** the initial implementation, not ahead of it.
Artifacts delivered alongside the ADR:

| Component                               | File                                                                    | State       |
|-----------------------------------------|-------------------------------------------------------------------------|-------------|
| L1 vtable + profile/mode/health enums   | `firmware/esp32-csi-node/main/rv_radio_ops.h`                           | Implemented |
| L1 ESP32 binding                        | `firmware/esp32-csi-node/main/rv_radio_ops_esp32.c`                     | Implemented |
| L1 Mock (QEMU) binding                  | `firmware/esp32-csi-node/main/rv_radio_ops_mock.c`                      | Implemented |
| L2 Controller FreeRTOS plumbing         | `firmware/esp32-csi-node/main/adaptive_controller.c`                    | Implemented |
| L2 Pure decision policy (testable)      | `firmware/esp32-csi-node/main/adaptive_controller_decide.c`             | Implemented |
| L3 Mesh-plane types + encoder/decoder   | `firmware/esp32-csi-node/main/rv_mesh.{h,c}`                            | Implemented |
| L3 HEALTH emit (slow loop, 30 s)        | `adaptive_controller.c:slow_loop_cb()`                                  | Implemented |
| L3 ANOMALY_ALERT on state transition    | `adaptive_controller.c:apply_decision()`                                | Implemented |
| L3 Role tracking + epoch monotonicity   | `adaptive_controller.c` (`s_role`, `s_mesh_epoch`)                      | Implemented |
| L4 Feature state packet + helpers       | `firmware/esp32-csi-node/main/rv_feature_state.{h,c}`                   | Implemented |
| L4 Emitter from fast loop (5 Hz)        | `adaptive_controller.c:emit_feature_state()`                            | Implemented |
| L1 Packet yield + send-fail accessors   | `csi_collector.c:csi_collector_get_pkt_yield_per_sec()` + send fail    | Implemented |
| L5 Rust mirror trait + mesh decoder     | `crates/wifi-densepose-hardware/src/radio_ops.rs`                       | Implemented |
| Host C unit tests (60 assertions)       | `firmware/esp32-csi-node/tests/host/`                                   | **60/60 ✓** |
| Rust unit tests (8 assertions)          | `crates/wifi-densepose-hardware` (`radio_ops::tests`)                   | **8/8 ✓**   |
| QEMU validator hooks (3 new checks)     | `scripts/validate_qemu_output.py` (check 17/18/19)                      | Passing     |
| L3 mesh RX path (receive + dispatch)    | —                                                                       | Phase 3.5   |
| Ed25519 signing for CHANNEL_PLAN etc.   | —                                                                       | Phase 3.5   |
| Hardware validation on COM7             | —                                                                       | Pending     |

## Measured performance

Host-side benchmarks (`firmware/esp32-csi-node/tests/host/`), x86-64,
gcc `-O2`, 2026-04-19. Numbers are illustrative of algorithmic cost on
a modern CPU; on-target ESP32-S3 Xtensa LX7 at 240 MHz is ~5–10×
slower for bit-by-bit CRC and broadly comparable for the decide
function after inlining.

| Operation                                   | Cost per call       | Notes                               |
|---------------------------------------------|---------------------|-------------------------------------|
| `adaptive_controller_decide()`              | **3.2 ns** (host)   | O(1) policy, 9 branches evaluated   |
| `rv_feature_state_crc32()` (56 B hashed)    | **612 ns** (host)   | 87 MB/s — bit-by-bit IEEE CRC32     |
| `rv_feature_state_finalize()` (full)        | **592 ns** (host)   | CRC-dominated                       |
| `rv_mesh_encode_health()` + `_decode()`     | **1010 ns** (host)  | Full roundtrip, hdr+payload+CRC     |

Projected on-target cost at 5 Hz cadence:

| Budget                                     | Value               |
|--------------------------------------------|---------------------|
| Controller fast-loop tick work (ESP32-S3)  | < 10 μs (est.)      |
| CRC32 per feature packet (ESP32-S3)        | ~3–6 μs (est.)      |
| Feature-state emit cost @ 5 Hz             | ~30 μs/sec (0.003%) |
| UDP send cost (existing stream_sender)     | — unchanged —       |

**Bandwidth:**

| Mode                                        | Rate        |
|---------------------------------------------|-------------|
| Raw ADR-018 CSI (pre-ADR-081)               | ~100 KB/s   |
| ADR-039 compressed CSI (Tier 1)             | ~50–70 KB/s |
| ADR-039 vitals packet (32 B @ 1 Hz)         | 32 B/s      |
| **ADR-081 feature state (60 B @ 5 Hz)**     | **300 B/s** |

**Memory:**

| Component                                   | Static RAM          |
|---------------------------------------------|---------------------|
| Controller state (s_cfg + s_last_obs + …)   | ~80 bytes           |
| Feature-state emit packet (stack, per tick) | 60 bytes            |
| CRC lookup table                            | 0 (bit-by-bit)      |
| Three FreeRTOS software timers              | ~3 × 56 B overhead  |

**Tests:**

| Suite                                       | Assertions | Result     |
|---------------------------------------------|-----------:|------------|
| `test_adaptive_controller` (host C)         |         18 | **PASS**   |
| `test_rv_feature_state` (host C)            |         15 | **PASS**   |
| `test_rv_mesh` (host C)                     |         27 | **PASS**   |
| `radio_ops::tests` (Rust)                   |          8 | **PASS**   |
| **Total**                                   |     **68** | **68/68**  |
| QEMU validator (`ADR-061` pipeline)         |  +3 checks | hooked     |

Cross-language parity: the Rust `crc32_ieee()` is verified against the
same known vectors used by the C test (`0xCBF43926` for `"123456789"`,
`0xD202EF8D` for a single zero byte), and the `mesh_constants_match_firmware`
test asserts `MESH_MAGIC`, `MESH_VERSION`, `MESH_HEADER_SIZE`, and
`MESH_MAX_PAYLOAD` match the C header byte-for-byte. Any drift between
the two implementations fails CI.

## New components this ADR authorizes

| New file                                                                                  | Purpose                                                |
|-------------------------------------------------------------------------------------------|--------------------------------------------------------|
| `firmware/esp32-csi-node/main/rv_radio_ops.h`                                             | `rv_radio_ops_t` vtable + profile/mode/health enums    |
| `firmware/esp32-csi-node/main/rv_radio_ops_esp32.c`                                       | ESP32 binding wrapping `csi_collector` + `esp_wifi_*`  |
| `firmware/esp32-csi-node/main/rv_feature_state.h`                                         | `rv_feature_state_t` packet + `RV_FEATURE_STATE_MAGIC` |
| `firmware/esp32-csi-node/main/adaptive_controller.h`                                      | Controller API + observation/decision structs           |
| `firmware/esp32-csi-node/main/adaptive_controller.c`                                      | 200 ms / 1 s / 30 s loops, FreeRTOS task               |
| `crates/wifi-densepose-hardware/src/radio_ops.rs` *(Phase 4 follow-up)*                  | Rust mirror trait for backend swapping                 |

## Roadmap

| Phase | Scope                                      | Status                                           |
|-------|--------------------------------------------|--------------------------------------------------|
| 1     | Single supported-CSI node + features → Rust | Largely done via ADR-018, ADR-039                |
| 2     | 3-node Seed v2 mesh + time-sync + plan     | Partially done (ADR-029, ADR-066, ADR-073)       |
| 3     | Adaptive controller, delta reporting, DEGRADED | **This ADR** authorizes the firmware skeleton |
| 4     | Cross-chipset bindings (Nexmon, custom)    | Reserved; gated by Phase 3 stability             |

## Acceptance criteria

1. **Portability gate.** A second `rv_radio_ops_t` binding (mock or
   alternate chipset) compiles and runs the controller + mesh plane code
   unchanged. The signal/ruvector/train/mat crates compile against a Rust
   mirror trait without modification.
2. **Mesh resilience benchmark.** A 3-node prototype maintains stable
   `presence_score` and `motion_score` when one observer changes channel
   or drops out for 5 seconds.
3. **Default upstream is compact.** Raw ADR-018 CSI is off by default; the
   default upstream is `rv_feature_state_t` at 1–10 Hz.
4. **Integrity.** Every `FEATURE_DELTA` carries node id, seq, ts_us, CRC32.
   Every control message carries epoch + replay-window + authorization
   class, verified against ADR-032's existing HMAC machinery.

## Consequences

### Positive

- The firmware hack is no longer the moat. The 5 layers are explicit and
  separately testable.
- Default upstream bandwidth drops ~99% vs. raw ADR-018, making 50+ node
  deployments practical.
- A documented vtable + Kconfig surface gates new features ("which layer
  does this belong in?") instead of letting them accrete inline.
- Adaptive control of cadence, channel, and role becomes a first-class
  firmware concern — the user-facing knob ("be smarter when busy, save
  power when idle") finally has a home.

### Negative

- An abstraction tax on the single-chipset case: `rv_radio_ops_t` is a
  vtable for a family currently of size 1.
- Adds ~5–8 KB SRAM for controller state and the new feature-state ring.
- Requires re-routing existing `swarm_bridge` traffic through the mesh
  plane message types over time (incremental, not breaking).

### Neutral

- This ADR introduces no new dependencies, no new networking stacks, and
  no new hardware requirements.
- ADR-039, ADR-063, ADR-066, ADR-069, ADR-073 are **not superseded**; they
  are reframed as components of Layer 3 / Layer 4.

## Verification

```bash
# Host-side C unit tests (no ESP-IDF, no QEMU required)
cd firmware/esp32-csi-node/tests/host
make check
# → test_adaptive_controller: 18/18 pass, decide() = 3.2 ns/call
# → test_rv_feature_state:    15/15 pass, CRC32(56 B) = 612 ns/pkt
# → test_rv_mesh:             27/27 pass, HEALTH roundtrip = 1.0 µs

# Rust-side radio_ops trait + mesh decoder tests
cd rust-port/wifi-densepose-rs
cargo test -p wifi-densepose-hardware --no-default-features --lib radio_ops
# → 8 passed; verifies MockRadio, CRC32 parity with firmware vectors,
#   HEALTH encode/decode roundtrip, bad-magic/short/CRC rejection,
#   and that MESH_MAGIC/VERSION/HEADER_SIZE match rv_mesh.h

# QEMU end-to-end (requires ESP-IDF + qemu-system-xtensa, see ADR-061)
bash scripts/qemu-esp32s3-test.sh
# → Validator now runs 19 checks; new ADR-081 checks 17/18/19 verify
#   adaptive_ctrl boot line, rv_radio_mock binding registration, and
#   slow-loop heartbeat.

# Full workspace
cargo test --workspace --no-default-features
```

## Related

ADR-018, ADR-028, ADR-029, ADR-030, ADR-031, ADR-032, ADR-039, ADR-040,
ADR-060, ADR-061, ADR-063, ADR-066, ADR-069, ADR-073, ADR-078.
