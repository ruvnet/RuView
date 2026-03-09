# ADR-052 Appendix: DDD Bounded Contexts — Tauri Desktop Frontend

This document maps out the domain model for the RuView Tauri desktop application
described in ADR-052. It defines bounded contexts, their aggregates, entities,
value objects, and the domain events flowing between them.

## Context Map

```
+-------------------+       +---------------------+       +--------------------+
|                   |       |                     |       |                    |
|  Device Discovery |------>| Firmware Management |------>| Configuration /    |
|                   |       |                     |       | Provisioning       |
+-------------------+       +---------------------+       +--------------------+
        |                           |                             |
        |                           |                             |
        v                           v                             v
+-------------------+       +---------------------+       +--------------------+
|                   |       |                     |       |                    |
| Sensing Pipeline  |<------| Edge Module         |       | Visualization      |
|                   |       | (WASM)              |       |                    |
+-------------------+       +---------------------+       +--------------------+

Relationship types:
  -----> Upstream/Downstream (upstream publishes events, downstream consumes)
  <----- Conformist (downstream conforms to upstream's model)
```

---

## 1. Device Discovery Context

**Purpose**: Find, identify, and monitor ESP32 CSI nodes on the local network.

**Upstream of**: Firmware Management, Configuration, Sensing Pipeline, Visualization

### Aggregates

#### `NodeRegistry` (Aggregate Root)

Maintains the authoritative list of all known nodes. Merges discovery results
from multiple strategies (mDNS, UDP probe, HTTP sweep) and deduplicates by MAC
address.

| Field | Type | Description |
|-------|------|-------------|
| `nodes` | `Map<MacAddress, Node>` | All discovered nodes keyed by MAC |
| `scan_state` | `ScanState` | Idle, Scanning, Error |
| `last_scan` | `DateTime<Utc>` | Timestamp of last completed scan |

**Invariant**: No two nodes may share the same MAC address. If a node is
discovered via multiple strategies, the most recent data wins.

**Persistence**: The registry is persisted to `~/.ruview/nodes.db` (SQLite via
`rusqlite`). On startup, all previously known nodes are loaded as `Offline` and
reconciled against a fresh discovery scan. This means the app **remembers the
mesh** across restarts — critical for field deployments where nodes may be
temporarily powered off.

#### `Node` (Entity)

| Field | Type | Description |
|-------|------|-------------|
| `mac` | `MacAddress` (VO) | IEEE 802.11 MAC address (unique identity) |
| `ip` | `IpAddr` | Current IP address (may change on DHCP renewal) |
| `hostname` | `Option<String>` | mDNS hostname |
| `node_id` | `u8` | NVS-provisioned node ID |
| `firmware_version` | `Option<SemVer>` | Firmware version string |
| `health` | `HealthStatus` (VO) | Online / Offline / Degraded |
| `discovery_method` | `DiscoveryMethod` (VO) | How this node was found |
| `last_seen` | `DateTime<Utc>` | Last successful contact |
| `tdm_config` | `Option<TdmConfig>` (VO) | TDM slot assignment |
| `edge_tier` | `Option<u8>` | Edge processing tier (0/1/2) |

### Value Objects

- `MacAddress` — 6-byte hardware address, formatted as `AA:BB:CC:DD:EE:FF`
- `HealthStatus` — enum: `Online`, `Offline`, `Degraded(reason: String)`
- `DiscoveryMethod` — enum: `Mdns`, `UdpProbe`, `HttpSweep`, `Manual`
- `TdmConfig` — `{ slot_index: u8, total_nodes: u8 }`
- `SemVer` — semantic version `major.minor.patch`

### Domain Events

| Event | Payload | Consumers |
|-------|---------|-----------|
| `NodeDiscovered` | `{ node: Node }` | Firmware Mgmt (check for updates), Visualization (add to mesh graph) |
| `NodeWentOffline` | `{ mac: MacAddress, last_seen: DateTime }` | Visualization (gray out node), Sensing Pipeline (remove from active set) |
| `NodeCameOnline` | `{ node: Node }` | Visualization (restore node), Sensing Pipeline (re-add) |
| `NodeHealthChanged` | `{ mac: MacAddress, old: HealthStatus, new: HealthStatus }` | Visualization (update indicator) |
| `ScanCompleted` | `{ found: usize, new: usize, lost: usize }` | Dashboard (update summary) |

### Anti-Corruption Layer

When receiving data from the ESP32 OTA status endpoint (`GET /ota/status`), the
response format is owned by the firmware and may change across firmware versions.
The ACL translates the raw JSON response into `Node` entity fields:

```rust
/// ACL: Translate ESP32 OTA status response to Node fields.
fn translate_ota_status(raw: &serde_json::Value) -> Result<NodePatch, AclError> {
    NodePatch {
        firmware_version: raw["version"].as_str().map(SemVer::parse).transpose()?,
        uptime_secs: raw["uptime_s"].as_u64(),
        free_heap: raw["free_heap"].as_u64(),
        // Firmware may add fields in future versions — unknown fields are ignored
    }
}
```

---

## 2. Firmware Management Context

**Purpose**: Flash, update, and verify firmware on ESP32 nodes.

**Upstream of**: Configuration (a fresh flash triggers provisioning)
**Downstream of**: Device Discovery (needs node list and serial port info)

### Aggregates

#### `FlashSession` (Aggregate Root)

Represents a single firmware flashing operation from start to completion. Each
session has a lifecycle: Created -> Connecting -> Erasing -> Writing -> Verifying ->
Completed | Failed.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Session identifier |
| `port` | `SerialPort` (VO) | Target serial port |
| `firmware` | `FirmwareBinary` (Entity) | The binary being flashed |
| `chip` | `ChipType` (VO) | Target chip (ESP32, ESP32-S3, ESP32-C3) |
| `phase` | `FlashPhase` (VO) | Current phase of the flash operation |
| `progress` | `Progress` (VO) | Bytes written / total, speed |
| `started_at` | `DateTime<Utc>` | When the session started |
| `error` | `Option<String>` | Error message if failed |

**Invariant**: Only one `FlashSession` may be active per serial port at a time.

#### `FirmwareBinary` (Entity)

| Field | Type | Description |
|-------|------|-------------|
| `path` | `PathBuf` | Filesystem path to the `.bin` file |
| `size_bytes` | `u64` | Binary size |
| `version` | `Option<SemVer>` | Extracted from ESP32 image header |
| `chip_type` | `Option<ChipType>` | Detected from image magic bytes |
| `checksum` | `Sha256Hash` (VO) | SHA-256 of the binary |

#### `OtaSession` (Aggregate Root)

Represents an over-the-air firmware update to a running node.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Session identifier |
| `target_node` | `MacAddress` | Target node MAC |
| `target_ip` | `IpAddr` | Target node IP |
| `firmware` | `FirmwareBinary` | The binary being pushed |
| `psk` | `Option<SecureString>` | PSK for authentication (ADR-050) |
| `phase` | `OtaPhase` | Uploading / Rebooting / Verifying / Done / Failed |
| `progress` | `Progress` | Upload progress |

#### `BatchOtaSession` (Aggregate Root)

Coordinates rolling firmware updates across multiple mesh nodes. Prevents all
nodes from rebooting simultaneously, which would collapse the sensing network.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Batch session identifier |
| `firmware` | `FirmwareBinary` | The binary being deployed |
| `strategy` | `OtaStrategy` | `Sequential`, `TdmSafe`, `Parallel` |
| `max_concurrent` | `usize` | Max nodes updating at once |
| `batch_delay_secs` | `u64` | Delay between batches |
| `fail_fast` | `bool` | Abort remaining on first failure |
| `node_states` | `Map<MacAddress, BatchNodeState>` | Per-node progress |

**Invariant**: In `TdmSafe` mode, adjacent TDM slots are never updated
concurrently. Even-slot nodes update first, then odd-slot nodes.

**Lifecycle**: `Planning → InProgress → Completed | PartialFailure | Aborted`

- `BatchNodeState` — enum: `Queued`, `Uploading(Progress)`, `Rebooting`, `Verifying`, `Done`, `Failed(String)`, `Skipped`
- `OtaStrategy` — enum:
  - `Sequential` — one node at a time, wait for rejoin
  - `TdmSafe` — update non-adjacent slots to maintain sensing coverage
  - `Parallel` — all at once (development only)

### Value Objects

- `SerialPort` — `{ name: String, vid: u16, pid: u16, manufacturer: Option<String> }`
- `ChipType` — enum: `Esp32`, `Esp32s3`, `Esp32c3`
- `FlashPhase` — enum: `Connecting`, `Erasing`, `Writing`, `Verifying`, `Completed`, `Failed`
- `OtaPhase` — enum: `Uploading`, `Rebooting`, `Verifying`, `Completed`, `Failed`
- `Progress` — `{ bytes_done: u64, bytes_total: u64, speed_bps: u64 }`
- `Sha256Hash` — 32-byte hash
- `SecureString` — zeroized-on-drop string for PSK tokens

### Domain Events

| Event | Payload | Consumers |
|-------|---------|-----------|
| `FlashStarted` | `{ session_id, port, firmware_version }` | UI (show progress) |
| `FlashProgress` | `{ session_id, phase, progress }` | UI (update progress bar) |
| `FlashCompleted` | `{ session_id, duration_secs }` | Configuration (trigger provisioning prompt) |
| `FlashFailed` | `{ session_id, error }` | UI (show error) |
| `OtaStarted` | `{ session_id, target_mac, firmware_version }` | Discovery (mark node as updating) |
| `OtaCompleted` | `{ session_id, target_mac, new_version }` | Discovery (refresh node info) |
| `OtaFailed` | `{ session_id, target_mac, error }` | UI (show error) |
| `BatchOtaStarted` | `{ batch_id, strategy, node_count }` | UI (show batch progress) |
| `BatchNodeUpdated` | `{ batch_id, mac, state }` | UI (update per-node status), Discovery (refresh) |
| `BatchOtaCompleted` | `{ batch_id, succeeded, failed, skipped }` | UI (show summary), Discovery (full rescan) |

### Anti-Corruption Layer

The `espflash` crate has its own error types and progress reporting model. The
ACL translates these into domain events:

```rust
/// ACL: Translate espflash progress callbacks to domain FlashProgress events.
impl From<espflash::ProgressCallbackMessage> for FlashProgress {
    fn from(msg: espflash::ProgressCallbackMessage) -> Self {
        match msg {
            espflash::ProgressCallbackMessage::Connecting => FlashProgress {
                phase: FlashPhase::Connecting,
                progress: Progress::indeterminate(),
            },
            espflash::ProgressCallbackMessage::Erasing { addr, total } => FlashProgress {
                phase: FlashPhase::Erasing,
                progress: Progress::new(addr as u64, total as u64),
            },
            // ... etc
        }
    }
}
```

---

## 3. Configuration / Provisioning Context

**Purpose**: Manage NVS configuration for ESP32 nodes — WiFi credentials, network
targets, TDM mesh settings, edge intelligence parameters, WASM security keys.

**Downstream of**: Device Discovery (needs serial port), Firmware Management (post-flash provisioning)

### Aggregates

#### `ProvisioningSession` (Aggregate Root)

Represents a single NVS write or read operation on a connected ESP32.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Session identifier |
| `port` | `SerialPort` (VO) | Target serial port |
| `config` | `NodeConfig` (Entity) | Configuration to write |
| `direction` | `Direction` | Read or Write |
| `phase` | `ProvisionPhase` | Generating / Flashing / Verifying / Done |

#### `NodeConfig` (Entity)

The full set of NVS key-value pairs for a single node. Maps directly to the
firmware's `nvs_config_t` struct (see `firmware/esp32-csi-node/main/nvs_config.h`).

| Field | Type | NVS Key | Description |
|-------|------|---------|-------------|
| `wifi_ssid` | `Option<String>` | `ssid` | WiFi SSID |
| `wifi_password` | `Option<SecureString>` | `password` | WiFi password |
| `target_ip` | `Option<IpAddr>` | `target_ip` | Aggregator IP |
| `target_port` | `Option<u16>` | `target_port` | Aggregator UDP port |
| `node_id` | `Option<u8>` | `node_id` | Node identifier |
| `tdm_slot` | `Option<u8>` | `tdm_slot` | TDM slot index |
| `tdm_total` | `Option<u8>` | `tdm_nodes` | Total TDM nodes |
| `edge_tier` | `Option<u8>` | `edge_tier` | Processing tier |
| `hop_count` | `Option<u8>` | `hop_count` | Channel hop count |
| `channel_list` | `Option<Vec<u8>>` | `chan_list` | Channel sequence |
| `dwell_ms` | `Option<u32>` | `dwell_ms` | Hop dwell time |
| `power_duty` | `Option<u8>` | `power_duty` | Power duty cycle |
| `presence_thresh` | `Option<u16>` | `pres_thresh` | Presence threshold |
| `fall_thresh` | `Option<u16>` | `fall_thresh` | Fall detection threshold |
| `vital_window` | `Option<u16>` | `vital_win` | Vital sign window |
| `vital_interval_ms` | `Option<u16>` | `vital_int` | Vital sign interval |
| `top_k_count` | `Option<u8>` | `subk_count` | Top-K subcarriers |
| `wasm_max_modules` | `Option<u8>` | `wasm_max` | Max WASM modules |
| `wasm_verify` | `Option<bool>` | `wasm_verify` | Require WASM signature |
| `wasm_pubkey` | `Option<[u8; 32]>` | `wasm_pubkey` | Ed25519 public key |
| `ota_psk` | `Option<SecureString>` | `ota_psk` | OTA pre-shared key |

**Invariant**: `tdm_slot < tdm_total` when both are set.
**Invariant**: `channel_list.len() == hop_count` when both are set.
**Invariant**: `10 <= power_duty <= 100`.

#### `MeshConfig` (Entity)

A mesh-level configuration that generates per-node `NodeConfig` instances.
Corresponds to ADR-044 Phase 2 (config file provisioning).

| Field | Type | Description |
|-------|------|-------------|
| `common` | `NodeConfig` | Shared settings (WiFi, target IP, edge tier) |
| `nodes` | `Vec<MeshNodeEntry>` | Per-node overrides (port, node_id, tdm_slot) |

```rust
pub struct MeshNodeEntry {
    pub port: String,
    pub node_id: u8,
    pub tdm_slot: u8,
    // All other fields inherited from common
}
```

**Invariant**: `tdm_total` is automatically computed as `nodes.len()`.

### Value Objects

- `ProvisionPhase` — enum: `Generating`, `Flashing`, `Verifying`, `Completed`, `Failed`
- `Direction` — enum: `Read`, `Write`
- `Preset` — enum: `Basic`, `Vitals`, `Mesh3`, `Mesh6Vitals` (ADR-044 Phase 3)

### Domain Events

| Event | Payload | Consumers |
|-------|---------|-----------|
| `NodeProvisioned` | `{ port, node_id, config_summary }` | Discovery (trigger re-scan), UI (show success) |
| `NvsReadCompleted` | `{ port, config: NodeConfig }` | UI (populate form) |
| `ProvisionFailed` | `{ port, error }` | UI (show error) |
| `MeshProvisionStarted` | `{ node_count }` | UI (show batch progress) |
| `MeshProvisionCompleted` | `{ success_count, fail_count }` | UI (show summary) |

---

## 4. Sensing Pipeline Context

**Purpose**: Control the sensing server process, receive real-time CSI data, and
manage the signal processing pipeline.

**Downstream of**: Device Discovery (needs node IPs for data attribution)

### Aggregates

#### `SensingServer` (Aggregate Root)

Represents the managed sensing server child process.

| Field | Type | Description |
|-------|------|-------------|
| `state` | `ServerState` (VO) | Stopped / Starting / Running / Stopping / Crashed |
| `config` | `ServerConfig` (VO) | Port configuration, log level, model paths |
| `pid` | `Option<u32>` | OS process ID when running |
| `started_at` | `Option<DateTime<Utc>>` | Start timestamp |
| `log_buffer` | `RingBuffer<LogEntry>` | Last N log lines |
| `ws_url` | `Option<Url>` | WebSocket URL for live data |

**Invariant**: Only one `SensingServer` process may be managed at a time.

#### `SensingSession` (Entity)

An active connection to the sensing server's WebSocket for receiving real-time data.

| Field | Type | Description |
|-------|------|-------------|
| `connection_state` | `WsState` | Connecting / Connected / Disconnected |
| `frames_received` | `u64` | Total CSI frames received this session |
| `last_frame_at` | `Option<DateTime<Utc>>` | Timestamp of last received frame |
| `subscriptions` | `HashSet<DataChannel>` | Which data streams are active |

### Value Objects

- `ServerState` — enum: `Stopped`, `Starting`, `Running`, `Stopping`, `Crashed(exit_code: i32)`
- `ServerConfig` — `{ http_port: u16, ws_port: u16, udp_port: u16, model_dir: PathBuf, log_level: Level }`
- `LogEntry` — `{ timestamp: DateTime, level: Level, target: String, message: String }`
- `DataChannel` — enum: `CsiFrames`, `PoseUpdates`, `VitalSigns`, `ActivityClassification`
- `WsState` — enum: `Connecting`, `Connected`, `Disconnected(reason: String)`

### Domain Events

| Event | Payload | Consumers |
|-------|---------|-----------|
| `ServerStarted` | `{ pid, ports: ServerConfig }` | UI (enable sensing view), Discovery (start health polling via WS) |
| `ServerStopped` | `{ exit_code, uptime_secs }` | UI (disable sensing view) |
| `ServerCrashed` | `{ exit_code, last_log_lines }` | UI (show crash report) |
| `CsiFrameReceived` | `{ node_id, timestamp, subcarrier_count }` | Visualization (update charts) |
| `PoseUpdated` | `{ persons: Vec<PersonPose> }` | Visualization (draw skeletons) |
| `VitalSignUpdate` | `{ node_id, bpm, breath_rate }` | Visualization (update vitals chart) |
| `ActivityDetected` | `{ label, confidence }` | Visualization (show activity) |

---

## 5. Edge Module (WASM) Context

**Purpose**: Upload, manage, and monitor WASM edge processing modules running
on ESP32 nodes.

**Downstream of**: Device Discovery (needs node IPs and WASM capability info)
**Upstream of**: Sensing Pipeline (WASM modules emit edge-processed events)

### Aggregates

#### `ModuleRegistry` (Aggregate Root)

Tracks all WASM modules across all nodes.

| Field | Type | Description |
|-------|------|-------------|
| `modules` | `Map<(MacAddress, ModuleId), WasmModule>` | Per-node module inventory |

#### `WasmModule` (Entity)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `ModuleId` (VO) | Node-assigned module identifier |
| `name` | `String` | Filename of the uploaded `.wasm` |
| `size_bytes` | `u64` | Module size |
| `status` | `ModuleStatus` (VO) | Loaded / Running / Stopped / Error |
| `node_mac` | `MacAddress` | Which node this module runs on |
| `uploaded_at` | `DateTime<Utc>` | Upload timestamp |
| `signed` | `bool` | Whether the module has an Ed25519 signature |

### Value Objects

- `ModuleId` — string identifier assigned by the node firmware
- `ModuleStatus` — enum: `Loaded`, `Running`, `Stopped`, `Error(String)`

### Domain Events

| Event | Payload | Consumers |
|-------|---------|-----------|
| `ModuleUploaded` | `{ node_mac, module_id, name, size }` | UI (refresh list) |
| `ModuleStarted` | `{ node_mac, module_id }` | UI (update status) |
| `ModuleStopped` | `{ node_mac, module_id }` | UI (update status) |
| `ModuleUnloaded` | `{ node_mac, module_id }` | UI (remove from list) |
| `ModuleError` | `{ node_mac, module_id, error }` | UI (show error) |

### Anti-Corruption Layer

The ESP32 WASM management HTTP API (`/wasm/*` on port 8032) returns raw JSON
with firmware-specific field names. The ACL normalizes these:

```rust
/// ACL: Translate ESP32 WASM list response to domain WasmModule entities.
fn translate_wasm_list(raw: &[serde_json::Value]) -> Vec<WasmModule> {
    raw.iter().filter_map(|entry| {
        Some(WasmModule {
            id: ModuleId(entry["id"].as_str()?.to_string()),
            name: entry["name"].as_str().unwrap_or("unknown").to_string(),
            size_bytes: entry["size"].as_u64().unwrap_or(0),
            status: match entry["state"].as_str() {
                Some("running") => ModuleStatus::Running,
                Some("stopped") => ModuleStatus::Stopped,
                Some("loaded")  => ModuleStatus::Loaded,
                other => ModuleStatus::Error(
                    format!("Unknown state: {:?}", other)
                ),
            },
            // ...
        })
    }).collect()
}
```

---

## 6. Visualization Context

**Purpose**: Render real-time and historical sensing data — CSI heatmaps, pose
skeletons, vital sign charts, mesh topology graphs.

**Downstream of**: Sensing Pipeline (receives data events), Device Discovery (needs
node metadata for labeling)

This context is **purely presentational** and contains no domain logic. It
transforms domain events from other contexts into visual representations.

### Aggregates

None — this context is a **Query Model** (CQRS read side). It subscribes to
domain events and projects them into view models.

### View Models

#### `DashboardView`

| Field | Source Context | Description |
|-------|---------------|-------------|
| `nodes` | Device Discovery | Node cards with health, version, signal quality |
| `server` | Sensing Pipeline | Server status, uptime, port info |
| `recent_activity` | All contexts | Timeline of recent events |

#### `SignalView`

| Field | Source Context | Description |
|-------|---------------|-------------|
| `csi_heatmap` | Sensing Pipeline | Subcarrier amplitude x time matrix |
| `signal_field` | Sensing Pipeline | 2D signal strength grid |
| `activity_label` | Sensing Pipeline | Current classification |
| `confidence` | Sensing Pipeline | Classification confidence |

#### `PoseView`

| Field | Source Context | Description |
|-------|---------------|-------------|
| `persons` | Sensing Pipeline | Array of detected person skeletons |
| `zones` | Sensing Pipeline | Active zones in the sensing area |

#### `VitalsView`

| Field | Source Context | Description |
|-------|---------------|-------------|
| `breathing_rate_bpm` | Sensing Pipeline | Per-node breathing rate time series |
| `heart_rate_bpm` | Sensing Pipeline | Per-node heart rate time series |

#### `MeshView`

| Field | Source Context | Description |
|-------|---------------|-------------|
| `nodes` | Device Discovery | Positioned nodes for graph layout |
| `edges` | Device Discovery | Inter-node visibility/connectivity |
| `tdm_timeline` | Device Discovery | TDM slot schedule visualization |
| `sync_status` | Sensing Pipeline | Per-node sync status with server |

---

## Cross-Context Event Flow

```
                            NodeDiscovered
Device Discovery  ─────────────────────────────────> Firmware Management
        │                                                    │
        │  NodeDiscovered                                    │ FlashCompleted
        │  NodeHealthChanged                                 │
        ├──────────────────> Visualization                   v
        │                                           Configuration
        │  NodeDiscovered                                    │
        ├──────────────────> Sensing Pipeline                │ NodeProvisioned
        │                                                    │
        │                                                    v
        │                                           Device Discovery
        │                                           (re-scan triggered)
        │
        │  NodeDiscovered
        └──────────────────> Edge Module (WASM)
                                    │
                                    │ ModuleUploaded, ModuleStarted
                                    │
                                    v
                             Sensing Pipeline
                                    │
                                    │ CsiFrameReceived, PoseUpdated, VitalSignUpdate
                                    │
                                    v
                             Visualization
```

## Implementation Notes

1. **Event Bus**: Domain events are dispatched via Tauri's event system
   (`app_handle.emit("event-name", payload)`). The frontend subscribes using
   `listen("event-name", callback)`. This provides natural cross-context
   communication without coupling contexts directly.

2. **State Isolation**: Each bounded context maintains its own `State<'_, T>`
   managed by Tauri. Contexts do not share mutable state directly — they
   communicate exclusively through events.

3. **Module Organization**: Each bounded context maps to a Rust module under
   `src/commands/` and `src/domain/`:

   ```
   src/
     commands/           # Tauri command handlers (application layer)
       discovery.rs      # Device Discovery context commands
       flash.rs          # Firmware Management context commands
       ota.rs            # Firmware Management context commands
       provision.rs      # Configuration context commands
       server.rs         # Sensing Pipeline context commands
       wasm.rs           # Edge Module context commands
     domain/             # Domain models (pure Rust, no Tauri dependency)
       discovery/
         mod.rs
         node.rs         # Node entity, MacAddress VO
         registry.rs     # NodeRegistry aggregate
         events.rs       # Discovery domain events
       firmware/
         mod.rs
         binary.rs       # FirmwareBinary entity
         flash.rs        # FlashSession aggregate
         ota.rs          # OtaSession aggregate
         events.rs
       config/
         mod.rs
         nvs.rs          # NodeConfig entity
         mesh.rs         # MeshConfig entity
         provision.rs    # ProvisioningSession aggregate
         events.rs
       sensing/
         mod.rs
         server.rs       # SensingServer aggregate
         session.rs      # SensingSession entity
         events.rs
       wasm/
         mod.rs
         module.rs       # WasmModule entity
         registry.rs     # ModuleRegistry aggregate
         events.rs
     acl/                # Anti-corruption layers
       ota_status.rs     # ESP32 OTA status response translator
       wasm_api.rs       # ESP32 WASM API response translator
       espflash.rs       # espflash crate adapter
   ```

4. **Testing Strategy**: Domain modules under `src/domain/` have no Tauri
   dependency and can be tested with standard `cargo test`. Command handlers
   under `src/commands/` require Tauri test utilities for integration testing.

5. **Shared Kernel**: The `MacAddress`, `SemVer`, and `SecureString` value objects
   are shared across contexts. They live in a `src/domain/shared.rs` module.
   This is acceptable because they are immutable value objects with no behavior
   beyond validation and formatting.
