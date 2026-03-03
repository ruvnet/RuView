# Hardware Platform Domain Model

The Hardware Platform domain covers everything from the ESP32-S3 silicon to the server-side aggregator: collecting raw CSI, processing it on-device, running programmable WASM modules at the edge, and provisioning fleets of sensor nodes. It is the physical foundation that all higher-level domains (RuvSense, WiFi-Mat, Pose Tracking) depend on for real radio data.

This document defines the system using [Domain-Driven Design](https://martinfowler.com/bliki/DomainDrivenDesign.html) (DDD): bounded contexts that own their data and rules, aggregate roots that enforce invariants, value objects that carry meaning, and domain events that connect everything. The goal is to make the firmware and hardware layer's structure match the electronics it controls -- so that anyone reading the code (or an AI agent modifying it) understands *why* each piece exists, not just *what* it does.

**Bounded Contexts:**

| # | Context | Responsibility | Key ADRs | Code |
|---|---------|----------------|----------|------|
| 1 | [Sensor Node](#1-sensor-node-context) | WiFi CSI collection, channel hopping, TDM scheduling, UDP streaming | [ADR-012](../adr/ADR-012-esp32-csi-sensor-mesh.md), [ADR-018](../adr/ADR-018-dev-implementation.md) | `firmware/esp32-csi-node/main/{csi_collector,stream_sender,nvs_config}.c` |
| 2 | [Edge Processing](#2-edge-processing-context) | On-device DSP pipeline (Tiers 0-2): phase unwrap, presence, vitals, fall detection | [ADR-039](../adr/ADR-039-esp32-edge-intelligence.md) | `firmware/esp32-csi-node/main/edge_processing.c` |
| 3 | [WASM Runtime](#3-wasm-runtime-context) | Tier 3 programmable sensing: module management, host API, budget control, RVF containers | [ADR-040](../adr/ADR-040-wasm-programmable-sensing.md), [ADR-041](../adr/ADR-041-wasm-module-collection.md) | `firmware/esp32-csi-node/main/{wasm_runtime,wasm_upload,rvf_parser}.c` |
| 4 | [Aggregation](#4-aggregation-context) | Server-side CSI frame reception, timestamp alignment, multi-node feature fusion | [ADR-012](../adr/ADR-012-esp32-csi-sensor-mesh.md) | `crates/wifi-densepose-hardware/src/esp32/` |
| 5 | [Provisioning](#5-provisioning-context) | NVS configuration, firmware lifecycle, fleet management, deployment presets | [ADR-044](../adr/ADR-044-provisioning-tool-enhancements.md) | `firmware/esp32-csi-node/provision.py` |

All firmware paths are relative to the repository root. Rust crate paths are relative to `rust-port/wifi-densepose-rs/`.

---

## Domain-Driven Design Specification

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Sensor Node** | An ESP32-S3 device that captures WiFi CSI frames and streams them to an aggregator via UDP |
| **CSI Frame** | A snapshot of Channel State Information: amplitude and phase per subcarrier, extracted from WiFi preambles |
| **Subcarrier** | One of 52-56 OFDM frequency bins whose complex response encodes the radio channel; the atomic unit of CSI |
| **Edge Tier** | Processing level on the ESP32: 0 = raw passthrough, 1 = basic DSP, 2 = vitals pipeline, 3 = WASM programmable |
| **Core 0 / Core 1** | The two Xtensa LX7 cores on ESP32-S3; Core 0 runs WiFi + CSI callback, Core 1 runs the DSP pipeline |
| **SPSC Ring Buffer** | Single-producer single-consumer lock-free queue between Core 0 (CSI callback) and Core 1 (DSP task) |
| **Vitals Packet** | 32-byte UDP packet (magic `0xC5110002`) containing presence, breathing BPM, heart rate BPM, fall flag |
| **Compressed Frame** | Delta-compressed CSI frame (magic `0xC5110003`) using XOR + RLE for 30-50% bandwidth reduction |
| **WASM Module** | A `no_std` Rust program compiled to `wasm32-unknown-unknown`, executed on-device via WASM3 interpreter |
| **Module Slot** | One of 4 pre-allocated PSRAM arenas (160 KB each) that host a WASM module instance |
| **Host API** | 12 functions in the `csi` namespace that WASM modules call to read sensor data and emit events |
| **RVF Container** | Signed binary envelope (192-byte overhead) wrapping a WASM payload with manifest, capabilities, and Ed25519 signature |
| **Budget Guard** | Per-frame execution time limit (default 10 ms); modules exceeding 10 consecutive faults are auto-stopped |
| **Adaptive Budget** | Mincut-eigenvalue-gap-driven compute allocation: scene complexity drives how much CPU time WASM modules get |
| **Aggregator** | Server (laptop, RPi, or cloud) that receives UDP streams from all nodes, aligns timestamps, and fuses features |
| **Feature-Level Fusion** | Combining per-node extracted features (not raw I/Q) to avoid cross-node clock synchronization |
| **Fused Frame** | Aggregated observation from all nodes for one time window, with cross-node correlation and fused motion energy |
| **NVS** | Non-Volatile Storage on ESP32 flash; stores runtime configuration (WiFi creds, edge tier, TDM slot, etc.) |
| **Provisioning** | Writing NVS key-value pairs to a device without recompiling firmware |
| **TDM Slot** | Time-Division Multiplexing slot assignment for coordinated multi-node transmission |
| **Channel Hopping** | Switching the ESP32 radio across WiFi channels (e.g., 1, 6, 11) for multi-band CSI diversity |
| **OTA Update** | Over-the-air firmware update via HTTP endpoint on port 8032 |

---

## Bounded Contexts

### 1. Sensor Node Context

**Responsibility:** Capture raw WiFi CSI frames via the ESP-IDF CSI API, serialize them into the ADR-018 binary format, and stream to the aggregator over UDP. Handle channel hopping, TDM scheduling, and rate limiting.

```
+--------------------------------------------------------------+
|                    Sensor Node Context                          |
+--------------------------------------------------------------+
|                                                                |
|  +----------------+    +----------------+                      |
|  | CSI Collector  |    | NVS Config     |                      |
|  | (promiscuous   |    | (20+ keys:     |                      |
|  |  mode, I/Q     |    |  ssid, ip,     |                      |
|  |  extraction)   |    |  tier, tdm...) |                      |
|  +-------+--------+    +-------+--------+                      |
|          |                     |                               |
|          |  CSI callback       |  Boot config                  |
|          |  (Core 0, 50 Hz    |                               |
|          |   rate limit)       |                               |
|          v                     |                               |
|  +----------------+            |                               |
|  | Stream Sender  |<-----------+                               |
|  | (UDP to agg,   |                                            |
|  |  seq numbers,  |                                            |
|  |  ENOMEM        |                                            |
|  |  backoff)      |---> UDP frames (magic 0xC5110001)          |
|  +-------+--------+                                            |
|          |                                                     |
|          | SPSC ring buffer (to Core 1)                        |
|          v                                                     |
|  [Edge Processing Context]                                     |
|                                                                |
+--------------------------------------------------------------+
```

**Aggregates:**
- `SensorNode` (Aggregate Root)

**Value Objects:**
- `CsiFrame`
- `NodeIdentity`
- `NvsConfig`
- `TdmSchedule`
- `ChannelHopConfig`

**Domain Services:**
- `CsiCollectionService` -- Registers ESP-IDF CSI callback, extracts I/Q, enforces 50 Hz rate limit
- `StreamSendService` -- Serializes frames to ADR-018 binary format, sends UDP with sequence numbers
- `NvsConfigService` -- Reads 20+ NVS keys at boot, provides typed config to all firmware components

---

### 2. Edge Processing Context

**Responsibility:** On-device signal processing pipeline running on Core 1. Implements Tiers 0-2: phase extraction, Welford running statistics, top-K subcarrier selection, bandpass filtering, BPM estimation, presence detection, and fall detection.

```
+--------------------------------------------------------------+
|                  Edge Processing Context                       |
+--------------------------------------------------------------+
|                                                                |
|  SPSC ring buffer (from Core 0)                               |
|          |                                                     |
|          v                                                     |
|  +----------------+                                            |
|  | Phase Extract  |   Tier 1                                   |
|  | + Unwrap       |                                            |
|  +-------+--------+                                            |
|          |                                                     |
|          v                                                     |
|  +----------------+    +----------------+                      |
|  | Welford Stats  |    | Top-K Select   |                      |
|  | (per-subcarrier|    | (by variance)  |                      |
|  |  running var)  |    +-------+--------+                      |
|  +-------+--------+            |                               |
|          |                     |                               |
|          +----------+----------+                               |
|                     |                                          |
|                     v                                          |
|  +------------------+-----------+   Tier 2                     |
|  | Biquad IIR Bandpass Filters  |                              |
|  | breathing: 0.1-0.5 Hz        |                              |
|  | heart rate: 0.8-2.0 Hz       |                              |
|  +-------+----------------------+                              |
|          |                                                     |
|          v                                                     |
|  +----------------+    +----------------+                      |
|  | Zero-Crossing  |    | Presence       |                      |
|  | BPM Estimator  |    | Detector       |                      |
|  |                |    | (adaptive      |                      |
|  |                |    |  threshold,    |                      |
|  |                |    |  3-sigma cal)  |                      |
|  +-------+--------+    +-------+--------+                      |
|          |                     |                               |
|          +----------+----------+                               |
|                     |                                          |
|                     v                                          |
|  +------------------+--------+                                 |
|  | Fall Detector             |                                 |
|  | (phase acceleration       |                                 |
|  |  threshold)               |                                 |
|  +------------------+--------+                                 |
|                     |                                          |
|                     v                                          |
|  +------------------+--------+                                 |
|  | Multi-Person Clustering   |                                 |
|  | (subcarrier groups, <=4)  |----> VitalsPacket (0xC5110002)  |
|  +---------------------------+----> CompressedFrame (0xC5110003)|
|                                                                |
+--------------------------------------------------------------+
```

**Aggregates:**
- `EdgeProcessingState` (Aggregate Root)

**Value Objects:**
- `VitalsPacket`
- `CompressedFrame`
- `PresenceState`
- `BpmEstimate`
- `FallAlert`
- `EdgeTier`

**Domain Services:**
- `PhaseExtractionService` -- Converts raw I/Q to amplitude + phase, applies unwrapping
- `WelfordStatsService` -- Maintains per-subcarrier running mean and variance
- `TopKSelectionService` -- Selects K subcarriers with highest variance for downstream processing
- `BandpassFilterService` -- Biquad IIR filters for breathing and heart rate frequency bands
- `PresenceDetectionService` -- Adaptive threshold with 1200-frame, 3-sigma calibration
- `FallDetectionService` -- Phase acceleration exceeding configurable threshold (default 2.0 rad/s^2)
- `DeltaCompressionService` -- XOR + RLE delta encoding for 30-50% bandwidth reduction

---

### 3. WASM Runtime Context

**Responsibility:** Manage the Tier 3 WASM programmable sensing layer. Load, validate, execute, and monitor WASM modules compiled from Rust. Enforce budget guards, handle RVF container verification, expose Host API, and provide HTTP management endpoints.

```
+--------------------------------------------------------------+
|                   WASM Runtime Context                         |
+--------------------------------------------------------------+
|                                                                |
|  +--------------------+    +--------------------+              |
|  | Module Manager     |    | RVF Verifier       |              |
|  | (4 slots, load/    |    | (Ed25519 sig,      |              |
|  |  unload/start/     |    |  SHA-256 hash,     |              |
|  |  stop lifecycle)   |    |  host API compat)  |              |
|  +--------+-----------+    +--------+-----------+              |
|           |                         |                          |
|           +----------+--------------+                          |
|                      |                                         |
|                      v                                         |
|  +-------------------+------------------+                      |
|  |            WASM3 Interpreter          |                     |
|  |  +-----------+ +-----------+          |                     |
|  |  | Slot 0    | | Slot 1    | ...x4    |                     |
|  |  | 160 KB    | | 160 KB    |          |                     |
|  |  | arena     | | arena     |          |                     |
|  |  +-----------+ +-----------+          |                     |
|  +-------------------+------------------+                      |
|                      |                                         |
|                      v                                         |
|  +-------------------+------------------+                      |
|  |           Host API (12 funcs)         |                     |
|  |  csi_get_phase, csi_get_amplitude,    |                     |
|  |  csi_get_variance, csi_get_bpm_*,     |                     |
|  |  csi_emit_event, csi_log, ...         |                     |
|  +-------------------+------------------+                      |
|                      |                                         |
|                      v                                         |
|  +-------------------+------------------+                      |
|  |         Budget Controller             |                     |
|  |  B = clamp(B0 + k1*dL + k2*A         |                     |
|  |           - k3*T - k4*P,             |                     |
|  |       B_min, B_max)                   |                     |
|  |  10 consecutive faults -> auto-stop   |                     |
|  +-------------------+------------------+                      |
|                      |                                         |
|                      +----> WASM events (magic 0xC5110004)     |
|                                                                |
|  +--------------------+                                        |
|  | HTTP Upload Server |                                        |
|  | (port 8032)        |                                        |
|  | POST /wasm/upload  |                                        |
|  | GET  /wasm/list    |                                        |
|  | POST /wasm/start/N |                                        |
|  | POST /wasm/stop/N  |                                        |
|  | DELETE /wasm/N     |                                        |
|  +--------------------+                                        |
|                                                                |
+--------------------------------------------------------------+
```

**Aggregates:**
- `WasmModuleSlot` (Aggregate Root)

**Value Objects:**
- `RvfContainer`
- `RvfManifest`
- `WasmTelemetry`
- `HostApiVersion`
- `CapabilityBitmask`
- `BudgetAllocation`
- `ModuleState`

**Domain Services:**
- `RvfVerificationService` -- Parses RVF header, verifies SHA-256 hash and Ed25519 signature
- `ModuleLifecycleService` -- Handles load -> start -> run -> stop -> unload transitions
- `BudgetControllerService` -- Computes per-frame budget from mincut eigenvalue gap, thermal, and battery pressure
- `HostApiBindingService` -- Links 12 host functions to WASM3 imports in the "csi" namespace
- `WasmUploadService` -- HTTP server on port 8032 for module management endpoints

---

### 4. Aggregation Context

**Responsibility:** Receive UDP CSI streams from multiple ESP32 nodes on the server side. Align timestamps across nodes (without cross-node phase synchronization), compute cross-node correlations, and produce fused feature frames for downstream pipeline consumption.

```
+--------------------------------------------------------------+
|                   Aggregation Context                          |
+--------------------------------------------------------------+
|                                                                |
|  UDP socket (:5005)                                           |
|     |          |          |                                    |
|     v          v          v                                    |
|  +--------+ +--------+ +--------+                             |
|  | Node 0 | | Node 1 | | Node 2 |  ... (up to 6)             |
|  | State  | | State  | | State  |                             |
|  | (ring  | | (ring  | | (ring  |                             |
|  |  buf,  | |  buf,  | |  buf,  |                             |
|  |  drift)| |  drift)| |  drift)|                             |
|  +---+----+ +---+----+ +---+----+                             |
|      |          |          |                                   |
|      +-----+----+-----+---+                                   |
|            |          |                                        |
|            v          v                                        |
|  +--------------------+--+    +-----------------------+        |
|  | Timestamp Aligner     |    | Cross-Node Correlator |        |
|  | (per-node monotonic,  |    | (amplitude ratios,    |        |
|  |  no NTP needed)       |    |  fused motion energy) |        |
|  +-----------+-----------+    +----------+------------+        |
|              |                           |                     |
|              +----------+----------------+                     |
|                         |                                      |
|                         v                                      |
|  +----------------------+-----+                                |
|  |      Fused Frame           |                                |
|  |  per_node_features[]       |                                |
|  |  cross_node_correlation    |--> pipeline_tx (mpsc channel)  |
|  |  fused_motion_energy       |                                |
|  |  fused_breathing_band      |                                |
|  +----------------------------+                                |
|                                                                |
+--------------------------------------------------------------+
```

**Aggregates:**
- `Esp32Aggregator` (Aggregate Root)

**Value Objects:**
- `FusedFrame`
- `NodeState`
- `CrossNodeCorrelation`
- `FusedMotionEnergy`

**Domain Services:**
- `UdpReceiverService` -- Listens on UDP port 5005, demuxes by magic number and node ID
- `TimestampAlignmentService` -- Maps per-node monotonic timestamps to aggregator-local time
- `FeatureFusionService` -- Computes cross-node correlation, fused motion (max across nodes), fused breathing (highest SNR)
- `PipelineBridgeService` -- Feeds fused frames into the wifi-densepose Rust pipeline via mpsc channel

---

### 5. Provisioning Context

**Responsibility:** Configure ESP32 sensor nodes by writing NVS key-value pairs without recompiling firmware. Support fleet provisioning via config files, deployment presets, read-back verification, and auto-detection of connected devices.

```
+--------------------------------------------------------------+
|                   Provisioning Context                         |
+--------------------------------------------------------------+
|                                                                |
|  +--------------------+    +--------------------+              |
|  | CLI Interface      |    | Config File Loader |              |
|  | (--ssid, --port,   |    | (JSON mesh config, |              |
|  |  --edge-tier,      |    |  common + per-node |              |
|  |  --preset, ...)    |    |  settings)         |              |
|  +--------+-----------+    +--------+-----------+              |
|           |                         |                          |
|           +----------+--------------+                          |
|                      |                                         |
|                      v                                         |
|  +-------------------+------------------+                      |
|  |        Preset Resolver               |                      |
|  |  basic, vitals, mesh-3,              |                      |
|  |  mesh-6-vitals                       |                      |
|  +-------------------+------------------+                      |
|                      |                                         |
|                      v                                         |
|  +-------------------+------------------+                      |
|  |        NVS Writer                    |                      |
|  |  esptool partition write             |                      |
|  |  20+ keys: ssid, password,           |                      |
|  |  target_ip, edge_tier, tdm_slot,     |                      |
|  |  hop_count, wasm_max, ...            |                      |
|  +-------------------+------------------+                      |
|                      |                                         |
|                      v                                         |
|  +-------------------+------------------+                      |
|  |      Verifier (optional)             |                      |
|  |  serial monitor for 5s,             |                      |
|  |  check for "CSI streaming active"    |                      |
|  +--------------------------------------+                      |
|                                                                |
|  +--------------------+                                        |
|  | Read-Back           |                                       |
|  | (--read: dump NVS   |                                       |
|  |  partition, parse    |                                       |
|  |  key-value pairs)   |                                       |
|  +--------------------+                                        |
|                                                                |
|  +--------------------+                                        |
|  | Auto-Detect         |                                       |
|  | (scan serial ports  |                                       |
|  |  for ESP32-S3)      |                                       |
|  +--------------------+                                        |
|                                                                |
+--------------------------------------------------------------+
```

**Aggregates:**
- `ProvisioningSession` (Aggregate Root)

**Value Objects:**
- `NvsConfig`
- `DeploymentPreset`
- `MeshConfig`
- `PortIdentity`
- `VerificationResult`

**Domain Services:**
- `NvsWriteService` -- Writes typed NVS key-value pairs to the ESP32 flash partition via esptool
- `PresetResolverService` -- Maps named presets (basic, vitals, mesh-3, mesh-6-vitals) to NVS key sets
- `MeshProvisionerService` -- Iterates over nodes in a config file, computing TDM slots automatically
- `ReadBackService` -- Reads NVS partition, parses binary format, returns typed config
- `BootVerificationService` -- Opens serial monitor post-provision, checks for expected log lines

---

## Aggregates

### SensorNode (Aggregate Root)

```rust
/// A physical ESP32-S3 device configured for CSI collection.
/// Owns its identity, configuration, firmware version, and current edge tier.
pub struct SensorNode {
    /// Unique node identifier (0-255, assigned during provisioning)
    node_id: u8,
    /// WiFi MAC address of the ESP32-S3
    mac: MacAddress,
    /// Current WiFi channel
    channel: u8,
    /// Firmware version string (e.g., "1.2.0")
    firmware_version: FirmwareVersion,
    /// Current edge processing tier (0-3)
    edge_tier: EdgeTier,
    /// Full NVS configuration snapshot
    config: NvsConfig,
    /// TDM slot assignment (None if standalone)
    tdm_slot: Option<TdmSchedule>,
    /// Channel hopping configuration
    hop_config: Option<ChannelHopConfig>,
    /// Current operational status
    status: NodeStatus,
    /// Monotonic boot timestamp (ms since power-on)
    uptime_ms: u64,
}

impl SensorNode {
    /// Invariant: node_id must be unique within a mesh deployment
    /// Invariant: edge_tier 3 requires WASM runtime to be initialized
    /// Invariant: tdm_slot.slot < tdm_slot.total_nodes
    pub fn new(node_id: u8, mac: MacAddress, config: NvsConfig) -> Self { /* ... */ }

    pub fn transition_tier(&mut self, new_tier: EdgeTier) -> Result<(), TierError> {
        // Cannot go to Tier 3 if WASM runtime is not available
        // Cannot downgrade while WASM modules are running
        /* ... */
    }
}
```

### EdgeProcessingState (Aggregate Root)

```rust
/// Maintains the full on-device DSP pipeline state for one sensor node.
/// Runs exclusively on Core 1.
pub struct EdgeProcessingState {
    /// Current processing tier
    tier: EdgeTier,
    /// Per-subcarrier running statistics (Welford)
    subcarrier_stats: [WelfordAccumulator; 56],
    /// Top-K selected subcarrier indices
    top_k_indices: Vec<u8>,
    /// Biquad IIR filter states
    breathing_filter: BiquadState,
    heartrate_filter: BiquadState,
    /// Current presence detection state
    presence: PresenceState,
    /// Latest BPM estimates
    breathing_bpm: Option<BpmEstimate>,
    heartrate_bpm: Option<BpmEstimate>,
    /// Fall detection state
    fall_detector: FallDetectorState,
    /// Multi-person clustering state (up to 4 persons)
    person_clusters: Vec<PersonCluster>,
    /// Calibration state (1200-frame adaptive threshold)
    calibration: CalibrationState,
}

impl EdgeProcessingState {
    /// Invariant: Only processes frames on Core 1 (never Core 0)
    /// Invariant: Tier 0 performs no processing (passthrough only)
    /// Invariant: Tier 2 includes all of Tier 1 processing
    /// Invariant: person_clusters.len() <= 4
    pub fn process_frame(&mut self, frame: &RawCsiFrame) -> ProcessingResult { /* ... */ }
}
```

### WasmModuleSlot (Aggregate Root)

```rust
/// One of 4 pre-allocated WASM execution slots on the ESP32-S3.
/// Each slot owns its PSRAM arena, WASM3 runtime instance, and telemetry.
pub struct WasmModuleSlot {
    /// Slot index (0-3)
    slot_id: u8,
    /// Pre-allocated PSRAM arena (160 KB, fixed at boot)
    arena: FixedArena,
    /// Loaded module metadata (None if slot is empty)
    module: Option<LoadedModule>,
    /// Current slot state
    state: ModuleState,
    /// Per-module telemetry counters
    telemetry: WasmTelemetry,
    /// Budget allocation for this slot (microseconds per frame)
    budget_us: u32,
}

/// Metadata for a loaded WASM module
pub struct LoadedModule {
    /// Module name from RVF manifest (up to 32 chars)
    name: String,
    /// SHA-256 hash of the WASM payload
    build_hash: [u8; 32],
    /// Declared capability bitmask
    capabilities: CapabilityBitmask,
    /// Author string from manifest
    author: String,
    /// WASM3 function pointers for lifecycle
    fn_on_init: WasmFunction,
    fn_on_frame: WasmFunction,
    fn_on_timer: WasmFunction,
}

impl WasmModuleSlot {
    /// Invariant: Arena is pre-allocated at boot and never freed (prevents fragmentation)
    /// Invariant: Module auto-stopped after 10 consecutive budget faults
    /// Invariant: RVF signature must be verified before loading (when wasm_verify=1)
    /// Invariant: Module binary + WASM3 heap must fit within 160 KB arena
    pub fn load(&mut self, rvf: &RvfContainer) -> Result<(), WasmLoadError> { /* ... */ }

    pub fn on_frame(&mut self, n_sc: i32) -> Result<Vec<WasmEvent>, WasmExecError> {
        // Measure execution time
        // Record telemetry
        // Check budget guard
        /* ... */
    }
}
```

### Esp32Aggregator (Aggregate Root)

```rust
/// Server-side aggregator that receives CSI streams from multiple ESP32 nodes,
/// aligns timestamps, and produces fused feature frames.
pub struct Esp32Aggregator {
    /// UDP socket listening for node streams (port 5005)
    socket: UdpSocket,
    /// Per-node state: ring buffer, last timestamp, drift estimate
    nodes: HashMap<u8, NodeState>,
    /// Ring buffer of fused feature frames
    fused_buffer: VecDeque<FusedFrame>,
    /// Channel to downstream pipeline
    pipeline_tx: mpsc::Sender<CsiData>,
    /// Configuration
    config: AggregatorConfig,
}

impl Esp32Aggregator {
    /// Invariant: Fuses features, never raw phases (clock drift makes cross-node
    ///            phase alignment impossible with 20-50 ppm crystal oscillators)
    /// Invariant: Handles missing nodes gracefully (partial fused frames are valid)
    /// Invariant: Sequence number gaps < 100ms are interpolated, not dropped
    pub fn receive_and_fuse(&mut self) -> Result<FusedFrame, AggError> { /* ... */ }
}
```

### ProvisioningSession (Aggregate Root)

```rust
/// A provisioning session that configures one or more ESP32 nodes.
/// Tracks which nodes have been provisioned and their verification status.
pub struct ProvisioningSession {
    /// Session identifier
    session_id: SessionId,
    /// Common configuration shared across all nodes
    common_config: CommonConfig,
    /// Per-node provisioning state
    node_results: Vec<NodeProvisionResult>,
    /// Preset used (if any)
    preset: Option<DeploymentPreset>,
    /// Mesh configuration (if provisioning multiple nodes)
    mesh_config: Option<MeshConfig>,
}

impl ProvisioningSession {
    /// Invariant: WiFi credentials must be non-empty
    /// Invariant: target_ip must be a valid IPv4 address
    /// Invariant: TDM slot indices must be unique and contiguous within a mesh
    /// Invariant: hop_count must match the length of the channel list
    pub fn provision_node(&mut self, port: &PortIdentity) -> Result<(), ProvisionError> {
        /* ... */
    }
}
```

---

## Value Objects

### CsiFrame

```rust
/// A single CSI observation from one ESP32 node.
/// Immutable snapshot of the radio channel at one instant.
pub struct CsiFrame {
    /// Monotonic timestamp (ms since node boot)
    timestamp_ms: u32,
    /// Source node identifier
    node_id: u8,
    /// RSSI in dBm (typically -90 to -20)
    rssi: i8,
    /// WiFi channel number (1-13)
    channel: u8,
    /// Per-subcarrier amplitude (|CSI|, 52-56 values)
    amplitude: Vec<f32>,
    /// Per-subcarrier phase (arg(CSI), 52-56 values, radians)
    phase: Vec<f32>,
    /// Sequence number for loss detection
    seq_num: u32,
}
```

### VitalsPacket

```rust
/// 32-byte Tier 2 output packet sent at configurable intervals.
/// Contains all vital sign estimates from on-device processing.
pub struct VitalsPacket {
    /// Presence state
    presence: PresenceState,
    /// Motion score (0-255, higher = more motion)
    motion_score: u8,
    /// Breathing rate estimate (BPM, None if not confident)
    breathing_bpm: Option<f32>,
    /// Heart rate estimate (BPM, None if not confident)
    heart_rate_bpm: Option<f32>,
    /// Fall detected flag
    fall_flag: bool,
    /// Number of detected persons (0-8)
    n_persons: u8,
    /// Motion energy scalar
    motion_energy: f32,
    /// Presence confidence score
    presence_score: f32,
    /// RSSI at time of measurement
    rssi: i8,
    /// Timestamp (ms since boot)
    timestamp_ms: u32,
}
```

### EdgeTier

```rust
/// Processing tier on the ESP32-S3. Each tier includes all functionality
/// of lower tiers.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeTier {
    /// Tier 0: Raw CSI passthrough (magic 0xC5110001). No on-device processing.
    Disabled = 0,
    /// Tier 1: Phase unwrap, Welford stats, top-K selection, delta compression.
    /// Adds ~30 KB binary overhead.
    BasicDsp = 1,
    /// Tier 2: All of Tier 1 + biquad bandpass, BPM estimation, presence,
    /// fall detection, multi-person clustering. Adds ~3 KB over Tier 1.
    FullPipeline = 2,
    /// Tier 3: All of Tier 2 + WASM3 runtime for programmable sensing modules.
    /// Adds ~100 KB binary (WASM3 interpreter).
    WasmProgrammable = 3,
}
```

### NvsConfig

```rust
/// Complete NVS configuration for one ESP32 sensor node.
/// Covers all 20+ firmware-readable keys.
pub struct NvsConfig {
    // -- Network --
    pub ssid: String,
    pub password: String,
    pub target_ip: Ipv4Addr,
    pub target_port: u16,       // default: 5005
    pub node_id: u8,            // default: 0

    // -- TDM --
    pub tdm_slot: u8,           // default: 0
    pub tdm_total: u8,          // default: 1 (no TDM)

    // -- Channel Hopping --
    pub hop_count: u8,          // default: 1 (no hop)
    pub chan_list: Vec<u8>,     // default: [1, 6, 11]
    pub dwell_ms: u32,          // default: 100

    // -- Edge Processing --
    pub edge_tier: EdgeTier,    // default: Tier 2
    pub pres_thresh: u16,       // default: 0 (auto-calibrate)
    pub fall_thresh: u16,       // default: 2000 (2.0 rad/s^2)
    pub vital_win: u16,         // default: 256
    pub vital_int: u16,         // default: 1000 ms
    pub subk_count: u8,         // default: 8

    // -- Power --
    pub power_duty: u8,         // default: 100 (always on)

    // -- WASM --
    pub wasm_max: u8,           // default: 4
    pub wasm_verify: bool,      // default: true (secure-by-default)
    pub wasm_pubkey: Option<[u8; 32]>, // Ed25519 public key

    // -- MAC Filter --
    pub filter_mac: Option<MacAddress>,
}
```

### RvfContainer

```rust
/// RVF (RuVector Format) container for signed WASM deployment.
/// Total overhead: 192 bytes (32 header + 96 manifest + 64 signature).
pub struct RvfContainer {
    /// Format version (currently 1)
    pub format_version: u16,
    /// Feature flags (bit 0: has_signature, bit 1: has_test_vectors)
    pub flags: u16,
    /// Module manifest
    pub manifest: RvfManifest,
    /// Raw WASM payload (starts with "\0asm" magic)
    pub wasm_payload: Vec<u8>,
    /// Ed25519 signature over header + manifest + payload (64 bytes)
    pub signature: Option<[u8; 64]>,
    /// Optional test vectors for self-verification
    pub test_vectors: Option<Vec<u8>>,
}

/// 96-byte packed manifest describing the WASM module.
pub struct RvfManifest {
    pub module_name: String,         // up to 32 chars
    pub required_host_api: u16,      // version (1 = current)
    pub capabilities: CapabilityBitmask,
    pub max_frame_us: u32,           // requested per-frame budget
    pub max_events_per_sec: u16,     // rate limit
    pub memory_limit_kb: u16,        // max WASM heap
    pub event_schema_version: u16,
    pub build_hash: [u8; 32],        // SHA-256 of WASM payload
    pub min_subcarriers: u16,
    pub max_subcarriers: u16,
    pub author: String,              // up to 10 chars
}
```

### WasmTelemetry

```rust
/// Per-module execution telemetry, exposed via /wasm/list endpoint.
pub struct WasmTelemetry {
    /// Total on_frame() calls since module start
    pub frame_count: u32,
    /// Total csi_emit_event() calls
    pub event_count: u32,
    /// WASM3 runtime errors
    pub error_count: u32,
    /// Cumulative execution time (microseconds)
    pub total_us: u32,
    /// Worst-case single-frame execution time (microseconds)
    pub max_us: u32,
    /// Number of times frame budget was exceeded
    pub budget_faults: u32,
}
```

### FusedFrame

```rust
/// Aggregated observation from all nodes for one time window.
/// Product of feature-level fusion (not signal-level).
pub struct FusedFrame {
    /// Aggregator-local monotonic timestamp
    timestamp: Instant,
    /// Per-node features (None if node dropped frames)
    node_features: Vec<Option<CsiFrame>>,
    /// Cross-node correlation matrix (N x N)
    cross_node_correlation: Array2<f64>,
    /// Fused motion energy (max across all nodes)
    fused_motion_energy: f64,
    /// Fused breathing band (coherent sum from highest-SNR node)
    fused_breathing_band: f64,
}
```

### DeploymentPreset

```rust
/// Named provisioning presets for common deployment scenarios.
pub enum DeploymentPreset {
    /// Single node, Tier 0, no TDM, no hopping
    Basic,
    /// Single node, Tier 2, vital_int=1000, subk_count=32
    Vitals,
    /// 3-node TDM, Tier 1, hop_count=3, channels=[1,6,11]
    Mesh3,
    /// 6-node TDM, Tier 2, hop_count=3, channels=[1,6,11], vital_int=500
    Mesh6Vitals,
}
```

### PresenceState

```rust
/// Tri-state presence classification from the edge DSP pipeline.
pub enum PresenceState {
    /// No motion detected; room appears empty
    Empty,
    /// Static human presence (breathing motion only)
    Present,
    /// Active motion detected
    Moving,
}
```

### ModuleState

```rust
/// Lifecycle state of a WASM module slot.
pub enum ModuleState {
    /// Slot is empty (arena allocated but no module loaded)
    Empty,
    /// Module loaded into arena but not yet started
    Loaded,
    /// Module running: on_frame() called per CSI frame, on_timer() at interval
    Running,
    /// Module explicitly stopped by user
    Stopped,
    /// Module auto-stopped due to error (10 consecutive budget faults or runtime error)
    Error,
}
```

---

## Domain Events

### Sensor Node Events

```rust
/// Emitted when an ESP32 node completes boot and begins CSI collection.
pub struct NodeBooted {
    pub node_id: u8,
    pub mac: MacAddress,
    pub firmware_version: FirmwareVersion,
    pub edge_tier: EdgeTier,
    pub uptime_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Emitted each time a CSI frame is received by the aggregator.
pub struct CsiFrameReceived {
    pub node_id: u8,
    pub seq_num: u32,
    pub subcarrier_count: u8,
    pub rssi: i8,
    pub channel: u8,
    pub timestamp: DateTime<Utc>,
}
```

### Edge Processing Events

```rust
/// Emitted when presence detection state transitions.
pub struct PresenceChanged {
    pub node_id: u8,
    pub previous: PresenceState,
    pub current: PresenceState,
    pub motion_energy: f32,
    pub timestamp: DateTime<Utc>,
}

/// Emitted at each vitals interval (default 1 Hz) with latest estimates.
pub struct VitalsUpdated {
    pub node_id: u8,
    pub breathing_bpm: Option<f32>,
    pub heart_rate_bpm: Option<f32>,
    pub n_persons: u8,
    pub timestamp: DateTime<Utc>,
}

/// Emitted when phase acceleration exceeds the fall detection threshold.
pub struct FallDetected {
    pub node_id: u8,
    pub motion_energy: f32,
    pub phase_acceleration: f32,
    pub threshold: f32,
    pub timestamp: DateTime<Utc>,
}
```

### WASM Runtime Events

```rust
/// Emitted when a WASM module is loaded into a slot and passes verification.
pub struct WasmModuleLoaded {
    pub slot_id: u8,
    pub module_name: String,
    pub build_hash: [u8; 32],
    pub capabilities: CapabilityBitmask,
    pub author: String,
    pub timestamp: DateTime<Utc>,
}

/// Emitted when a WASM module is auto-stopped or encounters a runtime error.
pub struct WasmModuleFaulted {
    pub slot_id: u8,
    pub module_name: String,
    pub fault_type: WasmFaultType,
    pub fault_count: u32,
    pub telemetry: WasmTelemetry,
    pub timestamp: DateTime<Utc>,
}

pub enum WasmFaultType {
    /// Exceeded per-frame budget 10 consecutive times
    BudgetExhausted,
    /// WASM3 runtime trap (stack overflow, OOB memory, etc.)
    RuntimeTrap,
    /// Module called an unavailable host API function
    MissingImport,
    /// RVF signature verification failed
    SignatureInvalid,
}

/// Emitted when a WASM module calls csi_emit_event().
pub struct WasmEventEmitted {
    pub slot_id: u8,
    pub module_name: String,
    pub event_type: u8,
    pub value: f32,
    pub timestamp: DateTime<Utc>,
}
```

### Provisioning Events

```rust
/// Emitted when a node's NVS configuration has been successfully written.
pub struct NodeProvisioningComplete {
    pub node_id: u8,
    pub port: String,
    pub config_keys: Vec<String>,
    pub preset: Option<DeploymentPreset>,
    pub verified: bool,
    pub timestamp: DateTime<Utc>,
}

/// Emitted when mesh provisioning completes for all nodes in a config file.
pub struct MeshProvisioningComplete {
    pub session_id: SessionId,
    pub node_count: usize,
    pub failed_nodes: Vec<u8>,
    pub timestamp: DateTime<Utc>,
}
```

---

## Invariants

### Firmware Architecture Invariants

| # | Invariant | Rationale | Enforcement |
|---|-----------|-----------|-------------|
| 1 | Core 0 handles WiFi + CSI callback only; Core 1 handles all DSP | Prevents WiFi stack corruption from compute-heavy DSP. CSI callback runs in ISR context on Core 0. | FreeRTOS task pinning: `xTaskCreatePinnedToCore(..., 1)` for DSP task |
| 2 | SPSC ring buffer between cores prevents memory contention | Lock-free single-producer single-consumer avoids mutexes between ISR and task contexts | `csi_spsc_ring` implementation with atomic read/write indices |
| 3 | CSI callback rate-limited to 50 Hz | Prevents lwIP pbuf exhaustion at high CSI rates (100-500 Hz in promiscuous mode). Issue #127 root cause. | 20 ms minimum interval check in `csi_collector.c` |
| 4 | `sendto()` uses 100 ms ENOMEM backoff | UDP sends can fail when lwIP pbuf pool is temporarily exhausted; immediate retry amplifies the problem | `stream_sender.c` checks `errno == ENOMEM` and delays |
| 5 | Binary must fit within 1 MB OTA partition | ESP32-S3 partition table allocates 1 MB for the factory app. Exceeding this prevents OTA updates. | CI size gate at 950 KB in `firmware-ci.yml` |

### WASM Runtime Invariants

| # | Invariant | Rationale | Enforcement |
|---|-----------|-----------|-------------|
| 6 | WASM modules get max 10 ms per frame | Prevents a runaway module from blocking the Tier 2 DSP pipeline and missing CSI frames | `esp_timer_get_time()` measurement + budget fault counter |
| 7 | Auto-stop after 10 consecutive budget faults | Graceful degradation: faulted module is stopped, Tier 2 pipeline continues unaffected | Fault counter in `WasmModuleSlot`, state transition to `Error` |
| 8 | RVF signature verification enabled by default | WASM upload is remote code execution; signatures ensure authenticity | `wasm_verify=1` default in Kconfig and NVS fallback |
| 9 | WASM arenas are pre-allocated at boot (640 KB PSRAM) | Dynamic malloc/free cycles fragment PSRAM over days of continuous operation | Fixed 160 KB arenas per slot, zeroed on unload but never freed |
| 10 | Maximum 4 concurrent WASM module slots | Bounds PSRAM usage and prevents compute exhaustion | `WASM_MAX_SLOTS` constant, validated at load time |

### Aggregation Invariants

| # | Invariant | Rationale | Enforcement |
|---|-----------|-----------|-------------|
| 11 | Feature-level fusion only (never raw phase alignment) | ESP32 crystal drift of 20-50 ppm makes cross-node phase coherence impossible | Aggregator extracts per-node features independently, then correlates |
| 12 | Missing nodes produce partial fused frames, not errors | Nodes may drop offline; the system must degrade gracefully | `Option<CsiFrame>` per node in `FusedFrame.node_features` |
| 13 | Sequence number gaps < 100 ms are interpolated | Brief UDP losses should not create discontinuities in downstream processing | Gap detection + linear interpolation in `NodeState` ring buffer |

### Provisioning Invariants

| # | Invariant | Rationale | Enforcement |
|---|-----------|-----------|-------------|
| 14 | WiFi credentials must never appear in tracked files | Prevents credential leakage via git history | `.gitignore` for `sdkconfig`, `provision.py` writes to NVS only |
| 15 | TDM slot indices must be unique within a mesh | Duplicate slots cause transmission collisions | Validation in `MeshProvisionerService`, config file schema check |
| 16 | `hop_count` must equal `chan_list.len()` | Mismatch causes firmware to read uninitialized channel values | CLI validation + NVS write-time assertion |

---

## Domain Services

### CsiCollectionService

Manages the ESP-IDF CSI API lifecycle on Core 0.

```rust
pub trait CsiCollectionService {
    /// Register CSI callback, configure promiscuous mode, set channel.
    /// Rate-limits callback invocations to 50 Hz.
    fn start_collection(&mut self, config: &NvsConfig) -> Result<(), CsiError>;

    /// Stop CSI collection and deregister callback.
    fn stop_collection(&mut self) -> Result<(), CsiError>;

    /// Get current collection statistics (frames/sec, drops, errors).
    fn stats(&self) -> CollectionStats;
}
```

### EdgeProcessingPipeline

Orchestrates the Tier 1-2 DSP chain on Core 1.

```rust
pub trait EdgeProcessingPipeline {
    /// Process a single CSI frame through the configured tier pipeline.
    /// Returns vitals packet (if Tier 2 interval elapsed) and/or compressed frame.
    fn process_frame(
        &mut self,
        raw: &RawCsiFrame,
    ) -> Result<ProcessingOutput, ProcessingError>;

    /// Reconfigure the pipeline tier at runtime (e.g., via NVS update).
    fn set_tier(&mut self, tier: EdgeTier) -> Result<(), TierError>;

    /// Get calibration status (0.0-1.0, 1.0 = fully calibrated after 1200 frames).
    fn calibration_progress(&self) -> f32;
}

pub struct ProcessingOutput {
    pub vitals: Option<VitalsPacket>,
    pub compressed: Option<CompressedFrame>,
    pub events: Vec<EdgeEvent>,
}
```

### WasmModuleManager

Manages the lifecycle of WASM modules across 4 slots.

```rust
pub trait WasmModuleManager {
    /// Load an RVF container into the next available slot.
    /// Verifies signature (if wasm_verify=1), checks host API compatibility,
    /// validates binary fits within arena.
    fn load_module(&mut self, rvf: &RvfContainer) -> Result<u8, WasmLoadError>;

    /// Start a loaded module (calls on_init()).
    fn start_module(&mut self, slot_id: u8) -> Result<(), WasmExecError>;

    /// Stop a running module.
    fn stop_module(&mut self, slot_id: u8) -> Result<(), WasmExecError>;

    /// Unload a module from its slot (zeroes the arena).
    fn unload_module(&mut self, slot_id: u8) -> Result<(), WasmLoadError>;

    /// Get telemetry for all slots.
    fn list_modules(&self) -> Vec<(u8, Option<&LoadedModule>, &ModuleState, &WasmTelemetry)>;

    /// Execute on_frame() for all running modules within the budget.
    fn dispatch_frame(&mut self, n_sc: i32) -> Vec<WasmEvent>;
}
```

### FeatureFusionService

Server-side fusion of per-node features into a coherent multi-node observation.

```rust
pub trait FeatureFusionService {
    /// Fuse features from N nodes for one time window.
    /// - Motion energy: max across nodes
    /// - Breathing band: highest-SNR node as primary
    /// - Location: cross-node amplitude ratios
    fn fuse(
        &self,
        node_features: &[Option<CsiFrame>],
    ) -> Result<FusedFrame, FusionError>;

    /// Compute cross-node correlation matrix.
    fn cross_correlate(
        &self,
        features: &[Option<CsiFrame>],
    ) -> Array2<f64>;
}
```

### ProvisioningService

Orchestrates the full provisioning workflow for individual nodes and meshes.

```rust
pub trait ProvisioningService {
    /// Provision a single node with the given configuration.
    fn provision_node(
        &mut self,
        port: &PortIdentity,
        config: &NvsConfig,
    ) -> Result<NodeProvisionResult, ProvisionError>;

    /// Provision all nodes defined in a mesh config file.
    fn provision_mesh(
        &mut self,
        mesh: &MeshConfig,
    ) -> Result<Vec<NodeProvisionResult>, ProvisionError>;

    /// Read back the current NVS configuration from a connected device.
    fn read_config(
        &self,
        port: &PortIdentity,
    ) -> Result<NvsConfig, ProvisionError>;

    /// Verify a provisioned node booted successfully.
    fn verify_boot(
        &self,
        port: &PortIdentity,
        timeout_secs: u32,
    ) -> Result<VerificationResult, ProvisionError>;

    /// Auto-detect connected ESP32-S3 devices.
    fn detect_ports(&self) -> Vec<PortIdentity>;
}
```

---

## Context Map

```
+------------------------------------------------------------------+
|                     Hardware Platform Domain                       |
+------------------------------------------------------------------+
|                                                                    |
|  +------------------+                                              |
|  |  Provisioning    |                                              |
|  |  Context         |--(writes NVS)---+                            |
|  |  (provision.py)  |                 |                            |
|  +------------------+                 |                            |
|                                       v                            |
|  +------------------+    SPSC    +------------------+              |
|  |  Sensor Node     |---------->|  Edge Processing  |              |
|  |  Context         |  ring buf |  Context          |              |
|  |  (Core 0)        |           |  (Core 1)         |              |
|  +--------+---------+           +--------+----------+              |
|           |                              |                         |
|           | UDP raw (0x01)               | feeds CSI data          |
|           |                              v                         |
|           |                     +------------------+               |
|           |                     |  WASM Runtime    |               |
|           |                     |  Context         |               |
|           |                     |  (Tier 3, Core 1)|               |
|           |                     +--------+---------+               |
|           |                              |                         |
|           | UDP raw   UDP vitals (0x02)  | UDP events (0x04)       |
|           | (0x01)    UDP compressed     |                         |
|           |           (0x03)             |                         |
|           +----------+------------------+                          |
|                      |                                             |
|                      v                                             |
|           +------------------+                                     |
|           |  Aggregation     |                                     |
|           |  Context         |                                     |
|           |  (Server-side)   |                                     |
|           +--------+---------+                                     |
|                    |                                               |
|                    | mpsc channel                                  |
|                    v                                               |
+------------------------------------------------------------------+
|                 DOWNSTREAM (Customer/Supplier)                    |
|  +-----------------+  +-----------------+  +-----------------+    |
|  | wifi-densepose  |  | wifi-densepose  |  | wifi-densepose  |    |
|  |   -signal       |  |   -nn           |  |   -mat          |    |
|  | (RuvSense)      |  | (Inference)     |  | (Disaster)      |    |
|  +-----------------+  +-----------------+  +-----------------+    |
+------------------------------------------------------------------+
```

**Relationship Types:**

| Upstream | Downstream | Relationship | Description |
|----------|------------|-------------|-------------|
| Provisioning | Sensor Node | **Customer/Supplier** | Provisioning writes NVS config that the node reads at boot |
| Sensor Node | Edge Processing | **Partnership** | Tightly coupled via SPSC ring buffer on the same chip |
| Edge Processing | WASM Runtime | **Customer/Supplier** | Edge pipeline feeds CSI data to WASM modules via Host API |
| Sensor Node | Aggregation | **Published Language** | ADR-018 binary wire format (magic bytes, fixed offsets) |
| Edge Processing | Aggregation | **Published Language** | Vitals (0xC5110002) and compressed (0xC5110003) wire formats |
| WASM Runtime | Aggregation | **Published Language** | WASM events (0xC5110004) wire format |
| Aggregation | Downstream crates | **Customer/Supplier** | Aggregator produces `FusedFrame` consumed by signal/nn/mat |

---

## Anti-Corruption Layers

### Aggregator-to-Pipeline ACL

The aggregator translates between the hardware-specific ESP32 binary wire format and the wifi-densepose Rust pipeline types.

```rust
/// Adapts raw ESP32 UDP packets to the wifi-densepose-signal CsiData type.
pub struct Esp32ToPipelineAdapter {
    /// Maps ADR-018 magic bytes to frame type
    frame_parser: FrameParser,
    /// Converts ESP32 I/Q byte pairs to f32 amplitude/phase
    iq_converter: IqConverter,
}

impl Esp32ToPipelineAdapter {
    /// Parse a raw UDP datagram from an ESP32 node into a pipeline-ready frame.
    /// Handles magic byte demuxing:
    ///   0xC5110001 -> raw CSI frame
    ///   0xC5110002 -> vitals packet
    ///   0xC5110003 -> compressed frame (decompress first)
    ///   0xC5110004 -> WASM event packet
    pub fn parse_datagram(
        &self,
        data: &[u8],
        src_addr: SocketAddr,
    ) -> Result<ParsedFrame, ParseError> {
        /* ... */
    }
}

pub enum ParsedFrame {
    RawCsi(CsiFrame),
    Vitals(VitalsPacket),
    CompressedCsi(CsiFrame), // decompressed
    WasmEvent(WasmEventEmitted),
}
```

### WASM Host API ACL

The Host API acts as an anti-corruption layer between the WASM module world (no_std, wasm32 ABI) and the firmware's C data structures.

```rust
/// Translates between WASM3 linear memory and firmware C structs.
/// Each Host API function validates indices, clamps values, and converts types.
pub struct WasmHostApiAdapter {
    /// Pointer to current CSI frame data (set before each on_frame dispatch)
    current_frame: *const EdgeProcessingState,
    /// Event buffer for this dispatch cycle
    event_buffer: Vec<WasmEvent>,
}

impl WasmHostApiAdapter {
    /// csi_get_phase(sc_idx: i32) -> f32
    /// Validates sc_idx is within [0, n_subcarriers), returns 0.0 if out of bounds.
    pub fn get_phase(&self, sc_idx: i32) -> f32 { /* ... */ }

    /// csi_emit_event(event_type: i32, value: f32) -> void
    /// Validates event_type is within the module's declared event ID range.
    /// Applies dead-band filter: suppresses if |value - last_emitted| < threshold.
    pub fn emit_event(&mut self, event_type: i32, value: f32) { /* ... */ }
}
```

### Provisioning-to-NVS ACL

The provisioning tool translates between human-readable CLI arguments / JSON config files and the ESP-IDF NVS binary format.

```rust
/// Adapts CLI/JSON configuration to ESP32 NVS binary partition format.
/// Handles type conversions, validation, and encoding.
pub struct ProvisioningAdapter {
    /// Maps CLI flag names to NVS key names and types
    key_registry: HashMap<String, NvsKeySpec>,
}

pub struct NvsKeySpec {
    pub nvs_key: String,       // e.g., "edge_tier"
    pub nvs_type: NvsType,     // u8, u16, u32, string, blob
    pub default: Option<String>,
    pub validator: Box<dyn Fn(&str) -> bool>,
}

impl ProvisioningAdapter {
    /// Convert a typed NvsConfig struct to a list of NVS binary writes.
    pub fn to_nvs_entries(&self, config: &NvsConfig) -> Vec<NvsEntry> { /* ... */ }

    /// Parse an NVS binary partition dump into a typed NvsConfig.
    pub fn from_nvs_partition(&self, data: &[u8]) -> Result<NvsConfig, ParseError> { /* ... */ }
}
```

---

## Wire Protocol Summary

All ESP32 UDP packets share a 4-byte magic prefix for demuxing at the aggregator.

| Magic | Name | Source | Size | Rate | Description |
|-------|------|--------|------|------|-------------|
| `0xC5110001` | Raw CSI | Tier 0+ | ~128-404 B | 20-28.5 Hz | Full I/Q per subcarrier |
| `0xC5110002` | Vitals | Tier 2+ | 32 B | 1 Hz (configurable) | Presence, BPM, fall flag |
| `0xC5110003` | Compressed | Tier 1+ | variable | 20-28.5 Hz | XOR+RLE delta-compressed CSI |
| `0xC5110004` | WASM Events | Tier 3 | variable | event-driven | Module event_type + value tuples |

---

## Hardware Constraints and Measured Performance

| Metric | Value | Source |
|--------|-------|--------|
| CSI frame rate | 28.5 Hz (measured) | ADR-039 hardware benchmark |
| Boot to ready | 3.9 s | WiFi connect dominates |
| Binary size | 925 KB (10% free in 1 MB) | Includes full WASM3 runtime |
| WASM init time | 106 ms | 4 slots, 160 KB arenas |
| WASM binary size (7 modules) | 13.8 KB | wasm32-unknown-unknown release |
| Internal RAM available | 316 KiB | No PSRAM on test board |
| Crystal drift | 20-50 ppm | 72-180 ms divergence per hour |
| BOM (3-node starter kit) | $54 | ADR-012 bill of materials |

---

## References

- [ADR-012: ESP32 CSI Sensor Mesh](../adr/ADR-012-esp32-csi-sensor-mesh.md) -- Hardware selection, mesh architecture, BOM
- [ADR-018: Dev Implementation](../adr/ADR-018-dev-implementation.md) -- Binary frame format, ADR-018 wire protocol
- [ADR-039: ESP32-S3 Edge Intelligence](../adr/ADR-039-esp32-edge-intelligence.md) -- Tiered processing, DSP pipeline, hardware benchmarks
- [ADR-040: WASM Programmable Sensing](../adr/ADR-040-wasm-programmable-sensing.md) -- WASM3 runtime, Host API, RVF container, adaptive budget
- [ADR-041: WASM Module Collection](../adr/ADR-041-wasm-module-collection.md) -- 60-module catalog, event ID registry, budget tiers
- [ADR-044: Provisioning Tool Enhancements](../adr/ADR-044-provisioning-tool-enhancements.md) -- NVS coverage, presets, mesh config, read-back
- [RuvSense Domain Model](ruvsense-domain-model.md) -- Upstream signal processing domain
- [WiFi-Mat Domain Model](wifi-mat-domain-model.md) -- Downstream disaster response domain
