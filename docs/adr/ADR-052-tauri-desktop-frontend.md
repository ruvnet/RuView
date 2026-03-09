# ADR-052: Tauri Desktop Frontend — RuView Hardware Management & Visualization

| Field | Value |
|-------|-------|
| Status | Proposed |
| Date | 2026-03-06 |
| Deciders | ruv |
| Depends on | ADR-012 (ESP32 CSI Mesh), ADR-039 (Edge Intelligence), ADR-040 (WASM Programmable Sensing), ADR-044 (Provisioning Enhancements), ADR-050 (Security Hardening), ADR-051 (Server Decomposition) |
| Issue | [#177](https://github.com/ruvnet/RuView/issues/177) |

## Context

RuView currently requires users to interact with multiple disconnected tools to manage a WiFi DensePose deployment:

| Task | Current Tool | Pain Point |
|------|-------------|------------|
| Flash firmware | `esptool.py` CLI | Requires Python, pip, correct chip/baud flags |
| Provision NVS | `provision.py` CLI | 13+ flags, no GUI, no read-back |
| OTA update | `curl POST :8032/ota` | Manual HTTP, PSK header construction |
| WASM modules | `curl` to `:8032/wasm/*` | No visibility into module state |
| Start sensing server | `cargo run` or binary | Manual port configuration, no log viewer |
| View sensing data | Browser at `localhost:8080` | Separate window, no hardware context |
| Mesh topology | Mental model | No visualization of TDM slots, sync, health |
| Node discovery | Manual IP tracking | No mDNS/UDP broadcast discovery |

There is no single tool that provides a unified view of the entire deployment — from ESP32 hardware through the sensing pipeline to pose visualization. Field operators deploying multi-node meshes must context-switch between terminals, browsers, and serial monitors.

### Why a Desktop App

A browser-based UI cannot access serial ports (for flashing), raw UDP sockets (for node discovery), or the local filesystem (for firmware binaries). A desktop application is required for hardware management. Tauri v2 is the natural choice because:

1. **Rust backend** — integrates directly with the existing Rust workspace (`wifi-densepose-rs`). Crates like `wifi-densepose-hardware` (serial port parsing), `wifi-densepose-config`, and `wifi-densepose-sensing-server` can be linked as library dependencies.
2. **Small binary** — Tauri bundles the system webview rather than shipping Chromium (~150 MB savings vs Electron).
3. **Cross-platform** — Windows, macOS, Linux from the same codebase.
4. **Security model** — Tauri's capability-based permissions system restricts frontend access to explicitly allowed Rust commands.

### Why Not Electron / Flutter / Native

| Option | Rejected Because |
|--------|-----------------|
| Electron | 150+ MB bundle, no Rust integration, duplicates webview |
| Flutter | No serial port plugins, Dart FFI to Rust is awkward |
| Native (GTK/Qt) | Platform-specific UI code, no web component reuse |
| Web-only (PWA) | Cannot access serial ports or raw UDP |

## Decision

Build a Tauri v2 desktop application as a new crate in the Rust workspace. The frontend uses TypeScript with React and Vite. The Rust backend exposes Tauri commands that bridge the frontend to serial ports, UDP sockets, HTTP management endpoints, and the sensing server process.

### 1. Workspace Integration

Add a new crate to the workspace:

```
rust-port/wifi-densepose-rs/
  Cargo.toml                          # Add "crates/wifi-densepose-desktop" to members
  crates/
    wifi-densepose-desktop/           # NEW — Tauri app crate
      Cargo.toml
      tauri.conf.json
      capabilities/
        default.json                  # Tauri v2 capability permissions
      icons/                          # App icons (all platforms)
      src/
        main.rs                       # Tauri entry point
        lib.rs                        # Command module re-exports
        commands/
          mod.rs
          discovery.rs                # Node discovery commands
          flash.rs                    # Firmware flashing commands
          ota.rs                      # OTA update commands
          wasm.rs                     # WASM module management commands
          server.rs                   # Sensing server lifecycle commands
          provision.rs                # NVS provisioning commands
          serial.rs                   # Serial port enumeration
        state.rs                      # Tauri managed state
        discovery/
          mod.rs
          mdns.rs                     # mDNS service discovery
          udp_broadcast.rs            # UDP broadcast probe
        flash/
          mod.rs
          espflash.rs                 # Rust-native ESP32 flashing (via espflash crate)
          esptool.rs                  # Fallback: bundled esptool.py wrapper
      frontend/
        package.json
        tsconfig.json
        vite.config.ts
        index.html
        src/
          main.tsx
          App.tsx
          routes.tsx
          hooks/
            useNodes.ts               # Node discovery and status polling
            useServer.ts              # Sensing server state
            useWebSocket.ts           # WS connection to sensing server
          stores/
            nodeStore.ts              # Zustand store for discovered nodes
            serverStore.ts            # Sensing server process state
            settingsStore.ts          # User preferences (dark mode, ports)
          pages/
            Dashboard.tsx             # Hardware management overview
            NodeDetail.tsx            # Single node detail + config
            FlashFirmware.tsx         # Firmware flashing wizard
            WasmModules.tsx           # WASM module manager
            SensingView.tsx           # Live sensing data visualization
            MeshTopology.tsx          # Multi-node mesh topology view
            Settings.tsx              # App settings and preferences
          components/
            NodeCard.tsx              # Node status card (health, version, signal)
            NodeList.tsx              # Discovered node list
            FirmwareProgress.tsx      # Flash/OTA progress indicator
            LogViewer.tsx             # Scrolling log output
            SignalChart.tsx           # Real-time CSI signal chart
            PoseOverlay.tsx           # Pose skeleton overlay
            MeshGraph.tsx             # D3/force-graph mesh topology
            SerialPortSelect.tsx      # Serial port dropdown
            ProvisionForm.tsx         # NVS provisioning form
          lib/
            tauri.ts                  # Typed Tauri invoke wrappers
            types.ts                  # Shared TypeScript types
```

### 2. Rust Backend — Tauri Commands

#### 2.1 Node Discovery

```rust
// commands/discovery.rs

/// Discover ESP32 CSI nodes on the local network.
/// Strategy 1: mDNS — nodes announce _ruview._tcp service
/// Strategy 2: UDP broadcast probe on port 5005 (CSI aggregator port)
/// Strategy 3: HTTP health check sweep on port 8032 (OTA server)
#[tauri::command]
async fn discover_nodes(timeout_ms: u64) -> Result<Vec<DiscoveredNode>, String>;

/// Get detailed status from a specific node via HTTP.
/// Calls GET /ota/status on port 8032.
#[tauri::command]
async fn get_node_status(ip: String) -> Result<NodeStatus, String>;

/// Subscribe to node health updates (periodic polling).
#[tauri::command]
async fn watch_nodes(interval_ms: u64, state: State<'_, AppState>) -> Result<(), String>;
```

The `DiscoveredNode` struct:

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct DiscoveredNode {
    pub ip: String,
    pub mac: Option<String>,
    pub hostname: Option<String>,
    pub node_id: u8,
    pub firmware_version: Option<String>,
    pub tdm_slot: Option<u8>,
    pub tdm_total: Option<u8>,
    pub edge_tier: Option<u8>,
    pub uptime_secs: Option<u64>,
    pub discovery_method: DiscoveryMethod, // Mdns | UdpProbe | HttpSweep
    pub last_seen: chrono::DateTime<chrono::Utc>,
}
```

#### 2.2 Firmware Flashing

```rust
// commands/flash.rs

/// List available serial ports with chip detection.
#[tauri::command]
async fn list_serial_ports() -> Result<Vec<SerialPortInfo>, String>;

/// Flash firmware binary to an ESP32 via serial port.
/// Uses the `espflash` crate for Rust-native flashing (no Python dependency).
/// Falls back to bundled esptool.py if espflash fails.
/// Emits progress events via Tauri event system.
#[tauri::command]
async fn flash_firmware(
    port: String,
    firmware_path: String,
    chip: Chip, // Esp32, Esp32s3, Esp32c3
    baud: Option<u32>,
    app_handle: AppHandle,
) -> Result<FlashResult, String>;

/// Read firmware info from a connected ESP32 (chip type, flash size, MAC).
#[tauri::command]
async fn read_chip_info(port: String) -> Result<ChipInfo, String>;
```

Flash progress is emitted as Tauri events:

```rust
#[derive(Serialize, Clone)]
pub struct FlashProgress {
    pub phase: FlashPhase,   // Connecting | Erasing | Writing | Verifying
    pub progress_pct: f32,   // 0.0 - 100.0
    pub bytes_written: u64,
    pub bytes_total: u64,
    pub speed_bps: u64,
}
```

#### 2.3 OTA Updates

```rust
// commands/ota.rs

/// Push firmware to a node via HTTP OTA (port 8032).
/// Includes PSK authentication per ADR-050.
#[tauri::command]
async fn ota_update(
    node_ip: String,
    firmware_path: String,
    psk: Option<String>,
    app_handle: AppHandle,
) -> Result<OtaResult, String>;

/// Get OTA status from a node (current version, partition info).
#[tauri::command]
async fn ota_status(node_ip: String, psk: Option<String>) -> Result<OtaStatus, String>;

/// Batch OTA update — push firmware to multiple nodes sequentially.
/// Skips nodes already running the target version.
#[tauri::command]
async fn ota_batch_update(
    nodes: Vec<String>, // IPs
    firmware_path: String,
    psk: Option<String>,
    app_handle: AppHandle,
) -> Result<Vec<OtaResult>, String>;
```

#### 2.4 WASM Module Management

```rust
// commands/wasm.rs

/// List WASM modules loaded on a node.
/// Calls GET /wasm/list on port 8032.
#[tauri::command]
async fn wasm_list(node_ip: String) -> Result<Vec<WasmModule>, String>;

/// Upload a WASM module to a node.
/// Calls POST /wasm/upload on port 8032 with binary payload.
#[tauri::command]
async fn wasm_upload(
    node_ip: String,
    wasm_path: String,
    app_handle: AppHandle,
) -> Result<WasmUploadResult, String>;

/// Start/stop a WASM module on a node.
#[tauri::command]
async fn wasm_control(
    node_ip: String,
    module_id: String,
    action: WasmAction, // Start | Stop | Unload
) -> Result<(), String>;
```

#### 2.5 Sensing Server Lifecycle

```rust
// commands/server.rs

/// Start the sensing server as a managed child process.
/// The server binary is either bundled with the Tauri app (sidecar)
/// or discovered on PATH.
#[tauri::command]
async fn start_server(
    config: ServerConfig,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String>;

/// Stop the managed sensing server process.
#[tauri::command]
async fn stop_server(state: State<'_, AppState>) -> Result<(), String>;

/// Get sensing server status (running/stopped, PID, ports, uptime).
#[tauri::command]
async fn server_status(state: State<'_, AppState>) -> Result<ServerStatus, String>;

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    pub http_port: u16,       // Default: 8080
    pub ws_port: u16,         // Default: 8765
    pub udp_port: u16,        // Default: 5005
    pub static_dir: Option<String>, // Path to UI static files
    pub model_dir: Option<String>,  // Path to ML models
    pub log_level: String,    // trace, debug, info, warn, error
}
```

The sensing server is bundled as a Tauri sidecar binary. Tauri v2 supports sidecar binaries via `externalBin` in `tauri.conf.json`:

```json
{
  "bundle": {
    "externalBin": ["sensing-server"]
  }
}
```

#### 2.6 NVS Provisioning

```rust
// commands/provision.rs

/// Provision NVS configuration to an ESP32 via serial port.
/// Replaces the Python provision.py script with a Rust-native implementation.
/// Generates NVS partition binary and flashes it to the NVS partition offset.
#[tauri::command]
async fn provision_node(
    port: String,
    config: NvsConfig,
    app_handle: AppHandle,
) -> Result<ProvisionResult, String>;

/// Read current NVS configuration from a connected ESP32.
/// Reads the NVS partition and parses key-value pairs.
#[tauri::command]
async fn read_nvs(port: String) -> Result<NvsConfig, String>;

#[derive(Serialize, Deserialize, Clone)]
pub struct NvsConfig {
    pub wifi_ssid: Option<String>,
    pub wifi_password: Option<String>,
    pub target_ip: Option<String>,
    pub target_port: Option<u16>,
    pub node_id: Option<u8>,
    pub tdm_slot: Option<u8>,
    pub tdm_total: Option<u8>,
    pub edge_tier: Option<u8>,
    pub presence_thresh: Option<u16>,
    pub fall_thresh: Option<u16>,
    pub vital_window: Option<u16>,
    pub vital_interval_ms: Option<u16>,
    pub top_k_count: Option<u8>,
    pub hop_count: Option<u8>,
    pub channel_list: Option<Vec<u8>>,
    pub dwell_ms: Option<u32>,
    pub power_duty: Option<u8>,
    pub wasm_max_modules: Option<u8>,
    pub wasm_verify: Option<bool>,
    pub wasm_pubkey: Option<Vec<u8>>,
    pub ota_psk: Option<String>,
}
```

### 3. Frontend Architecture

#### 3.1 Tech Stack

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Framework | React 19 | Component model, ecosystem, team familiarity |
| Build | Vite 6 | Fast HMR, Tauri plugin support |
| State | Zustand | Lightweight, no boilerplate, works with Tauri events |
| Routing | React Router v7 | File-based routes, type-safe |
| UI Components | shadcn/ui + Tailwind CSS | Accessible, customizable, no runtime CSS-in-JS |
| Charts | Recharts or visx | Real-time signal visualization |
| Topology Graph | D3 force-directed | Mesh network visualization |
| Serial UI | Custom | Tauri command integration |
| Icons | Lucide React | Consistent, tree-shakeable |

#### 3.2 Page Layout

```
+------------------------------------------+
|  RuView                    [Settings] [?] |
+-------+----------------------------------+
|       |                                  |
| Nav   |  Dashboard / Active Page         |
|       |                                  |
| [D]   |  +--------+ +--------+ +------+ |
| [F]   |  | Node 1 | | Node 2 | | +Add | |
| [W]   |  +--------+ +--------+ +------+ |
| [S]   |                                  |
| [M]   |  Server Status: Running          |
| [T]   |  +--------------------------+   |
|       |  | Live Signal / Pose View  |   |
|       |  +--------------------------+   |
+-------+----------------------------------+
|  Status Bar: 3 nodes | Server: :8080     |
+------------------------------------------+

Nav items:
  [D] Dashboard — overview of all nodes and server
  [F] Flash — firmware flashing wizard
  [W] WASM — edge module management
  [S] Sensing — live sensing data view
  [M] Mesh — topology visualization
  [T] Settings — ports, paths, preferences
```

#### 3.3 Dashboard Page

The dashboard is the primary landing page showing:

1. **Node Grid** — cards for each discovered ESP32 node showing:
   - IP address and hostname
   - Firmware version (with update indicator if newer available)
   - Node ID and TDM slot assignment
   - Edge processing tier (raw / stats / vitals)
   - Signal quality indicator (last CSI frame age)
   - Health status (online/offline/degraded)
   - Quick actions: OTA update, configure, view logs

2. **Sensing Server Panel** — start/stop button, port configuration, log tail

3. **Discovery Controls** — scan button, auto-discovery toggle, network range filter

#### 3.4 Flash Firmware Page

A wizard-style flow:

1. **Select Port** — dropdown of detected serial ports with chip info
2. **Select Firmware** — file picker for `.bin` files, or select from bundled builds
3. **Configure** — chip type, baud rate, flash mode
4. **Flash** — progress bar with phase indicators (connecting, erasing, writing, verifying)
5. **Provision** — optional NVS provisioning form (WiFi, target IP, TDM, edge tier)
6. **Verify** — serial monitor showing boot log, success/fail indicator

#### 3.5 WASM Module Manager Page

| Column | Content |
|--------|---------|
| Module ID | Auto-assigned by node |
| Name | Filename of uploaded `.wasm` |
| Size | Module size in KB |
| Status | Running / Stopped / Error |
| Node | Which ESP32 node it runs on |
| Actions | Start / Stop / Unload / View Logs |

Upload panel: drag-and-drop `.wasm` file, select target node(s), upload button.

#### 3.6 Sensing View Page

Embeds the existing web UI (`ui/`) via an iframe pointing at the sensing server's static file route, or builds native React components that connect to the same WebSocket API. The native approach is preferred because it allows:

- Tighter integration with the node status sidebar
- Shared state between hardware management and visualization
- Offline access to recorded data

Key visualization components:
- **CSI Heatmap** — subcarrier amplitude over time
- **Signal Field** — 2D signal strength visualization
- **Pose Skeleton** — detected body keypoints and connections
- **Vital Signs** — real-time breathing rate and heart rate charts
- **Activity Classification** — current activity label with confidence

#### 3.7 Mesh Topology Page

A force-directed graph showing:
- Nodes as circles (color = health status, size = edge tier)
- Edges between nodes that can see each other
- TDM slot labels on each node
- Sync status indicators (in-sync / drifting / lost)
- Click a node to navigate to its detail page

### 4. Platform-Specific Considerations

#### 4.1 macOS

- **Serial driver signing**: CP210x and CH340 drivers require user approval in System Preferences > Security
- **App signing**: Tauri apps must be signed and notarized for distribution outside the App Store
- **USB permissions**: No special permissions needed beyond driver installation
- **CoreWLAN**: The sensing server can use CoreWLAN for WiFi scanning (ADR-025); the desktop app inherits this capability

#### 4.2 Windows

- **COM port access**: Windows assigns COM port numbers; the app lists them via the Windows Registry or `SetupDi` API
- **Driver installation**: USB-to-serial drivers (CP210x, CH340, FTDI) must be installed; the app can detect missing drivers and link to downloads
- **Firewall**: The sensing server's UDP listener may trigger Windows Firewall prompts; the app should pre-configure rules or guide the user
- **Code signing**: EV certificate required for SmartScreen trust; unsigned apps trigger warnings

#### 4.3 Linux

- **udev rules**: ESP32 serial ports (`/dev/ttyUSB*`, `/dev/ttyACM*`) require udev rules for non-root access. The app bundles a `99-ruview-esp32.rules` file and offers to install it:
  ```
  SUBSYSTEM=="tty", ATTRS{idVendor}=="10c4", MODE="0666"  # CP210x
  SUBSYSTEM=="tty", ATTRS{idVendor}=="1a86", MODE="0666"  # CH340
  ```
- **AppImage/deb/rpm**: Tauri supports all three packaging formats
- **Wayland vs X11**: Tauri uses webkit2gtk which works on both

### 5. Cargo.toml for the Desktop Crate

```toml
[package]
name = "wifi-densepose-desktop"
version.workspace = true
edition.workspace = true
description = "Tauri desktop frontend for RuView WiFi DensePose"
license.workspace = true
authors.workspace = true

[lib]
name = "wifi_densepose_desktop"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"        # Sidecar process management
tauri-plugin-dialog = "2"       # File picker dialogs
tauri-plugin-fs = "2"           # Filesystem access
tauri-plugin-process = "2"      # Process management
tauri-plugin-notification = "2" # Desktop notifications

# Workspace crates
wifi-densepose-hardware = { workspace = true }
wifi-densepose-config = { workspace = true }
wifi-densepose-core = { workspace = true }

# Serial port access
serialport = { workspace = true }

# ESP32 flashing (Rust-native, replaces esptool.py)
espflash = "3"

# Network discovery
mdns-sd = "0.11"               # mDNS/DNS-SD service discovery

# HTTP client for OTA and WASM management
reqwest = { version = "0.12", features = ["json", "multipart", "stream"] }

# Async runtime
tokio = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# Logging
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

# Time
chrono = { version = "0.4", features = ["serde"] }
```

### 6. Tauri Configuration

```json
{
  "$schema": "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-config-schema/schema.json",
  "productName": "RuView",
  "version": "0.3.0",
  "identifier": "net.ruv.ruview",
  "build": {
    "frontendDist": "../frontend/dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "cd frontend && npm run dev",
    "beforeBuildCommand": "cd frontend && npm run build"
  },
  "app": {
    "windows": [
      {
        "title": "RuView - WiFi DensePose",
        "width": 1280,
        "height": 800,
        "minWidth": 900,
        "minHeight": 600
      }
    ]
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "externalBin": ["sensing-server"],
    "linux": {
      "deb": { "depends": ["libwebkit2gtk-4.1-0"] },
      "appimage": { "bundleMediaFramework": true }
    },
    "windows": {
      "wix": { "language": "en-US" }
    }
  }
}
```

### 7. Tauri v2 Capabilities (Permissions)

```json
{
  "identifier": "default",
  "description": "RuView default capability set",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-execute",
    "shell:allow-open",
    "dialog:allow-open",
    "dialog:allow-save",
    "fs:allow-read",
    "fs:allow-write",
    "process:allow-exit",
    "notification:default"
  ]
}
```

### 8. Development Workflow

```bash
# Prerequisites
cargo install tauri-cli@^2
cd rust-port/wifi-densepose-rs/crates/wifi-densepose-desktop/frontend
npm install

# Development (hot-reload frontend + Rust rebuild)
cd rust-port/wifi-densepose-rs/crates/wifi-densepose-desktop
cargo tauri dev

# Production build
cargo tauri build

# Build sensing-server sidecar (must be done before tauri build)
cargo build --release -p wifi-densepose-sensing-server
# Copy to sidecar location:
# target/release/sensing-server -> crates/wifi-densepose-desktop/binaries/sensing-server-{arch}
```

### 9. Persistent Node Registry

Discovery alone is transient — nodes appear when they broadcast, disappear when they don't. A persistent local registry transforms discovery into **reconciliation**.

```
~/.ruview/nodes.db   (SQLite via rusqlite)
```

**Schema:**

```sql
CREATE TABLE nodes (
    mac         TEXT PRIMARY KEY,        -- e.g. "AA:BB:CC:DD:EE:FF"
    last_ip     TEXT,                    -- last known IP
    last_seen   INTEGER NOT NULL,        -- Unix timestamp
    firmware    TEXT,                    -- e.g. "0.3.1"
    chip        TEXT DEFAULT 'esp32s3',  -- esp32, esp32s3, esp32c3
    mesh_role   TEXT DEFAULT 'node',     -- 'coordinator' | 'node' | 'aggregator'
    tdm_slot    INTEGER,                -- assigned TDM slot index
    capabilities TEXT,                  -- JSON: {"wasm": true, "ota": true, "csi": true}
    friendly_name TEXT,                 -- user-assigned label
    notes       TEXT                    -- free-form notes
);
```

**Behavior:**

- On discovery broadcast, upsert into registry (update `last_ip`, `last_seen`, `firmware`)
- Dashboard shows **all registered nodes**, dimming those not seen recently
- User can manually add nodes by MAC/IP (for networks without mDNS)
- Export/import registry as JSON for fleet management across machines
- Node health history (uptime, last OTA, error count) tracked over time

This means the desktop app **remembers the mesh** across restarts, which is critical for field deployments where nodes may be offline temporarily.

### 10. OTA Safety Gate — Rolling Updates

Mesh deployments cannot tolerate all nodes rebooting simultaneously. The OTA subsystem includes a **rolling update mode** that preserves sensing continuity:

```rust
#[derive(Serialize, Deserialize)]
pub struct BatchOtaConfig {
    /// Update strategy
    pub strategy: OtaStrategy,
    /// Max nodes updating concurrently
    pub max_concurrent: usize,
    /// Delay between batches (seconds)
    pub batch_delay_secs: u64,
    /// Abort if any node fails
    pub fail_fast: bool,
}

#[derive(Serialize, Deserialize)]
pub enum OtaStrategy {
    /// Update one node at a time, wait for it to rejoin mesh
    Sequential,
    /// Update non-adjacent TDM slots to maintain coverage
    TdmSafe,
    /// Update all nodes simultaneously (development only)
    Parallel,
}
```

**`TdmSafe` strategy:**

1. Sort nodes by TDM slot index
2. Update even-slot nodes first (slots 0, 2, 4...)
3. Wait for each to reboot and rejoin mesh (verified via beacon)
4. Then update odd-slot nodes (slots 1, 3, 5...)
5. At no point are adjacent nodes offline simultaneously

**UI flow:**

- User selects target firmware + target nodes
- App shows pre-update diff (current vs new version per node)
- Progress bar per node with states: `queued → uploading → rebooting → verifying → done`
- Abort button halts remaining updates without rolling back completed ones
- Post-update health check confirms all nodes are sensing

### 11. Plugin Architecture (Future)

This desktop tool is quietly becoming the **control plane for RuView**. Once it manages discovery, firmware, OTA, WASM, sensing, and mesh topology, plugin extensibility becomes inevitable:

- **Firmware management** today → **swarm orchestration** tomorrow
- **WASM upload** today → **edge module marketplace** tomorrow
- **Sensing view** today → **activity classification dashboard** tomorrow

The Tauri command surface should be designed with this trajectory in mind:

- Commands are grouped by bounded context (already done)
- Each context can be extended by loading additional Tauri plugins
- The node registry becomes the source of truth for all plugins
- Event bus (Tauri's `emit`/`listen`) provides cross-plugin communication

This does NOT mean building a plugin system in Phase 1. It means keeping the architecture open to it: no hardcoded views, state flows through the registry, commands are typed and versioned.

### 12. Security Considerations

1. **PSK Storage**: OTA PSK tokens are stored in the OS keychain via `tauri-plugin-stronghold` or the platform's native credential store, never in plaintext config files.

2. **Serial Port Access**: Tauri's capability system restricts which commands the frontend can invoke. Serial port access is only available through the typed `flash_firmware` and `provision_node` commands, not raw serial I/O.

3. **Network Requests**: OTA and WASM management commands only communicate with nodes on the local network. The app does not make external network requests except for update checks (opt-in).

4. **Firmware Validation**: Before flashing, the app validates the firmware binary header (ESP32 image magic bytes, partition table offset) to prevent bricking.

5. **WASM Signature Verification**: The desktop app can sign WASM modules before upload using a locally stored Ed25519 key pair, complementing the node-side verification (ADR-040).

### 13. Implementation Phases

| Phase | Scope | Effort | Priority |
|-------|-------|--------|----------|
| **Phase 1: Skeleton** | Tauri project scaffolding, workspace integration, basic window with React | 1 week | P0 |
| **Phase 2: Discovery** | Serial port listing, UDP/mDNS node discovery, dashboard with node cards | 1 week | P0 |
| **Phase 3: Flash** | espflash integration, firmware flashing wizard with progress events | 1 week | P0 |
| **Phase 4: Server** | Sidecar sensing server start/stop, log viewer, status panel | 1 week | P1 |
| **Phase 5: OTA** | HTTP OTA with PSK auth, batch update, version comparison | 1 week | P1 |
| **Phase 6: Provisioning** | NVS read/write via serial, provisioning form, mesh config file | 1 week | P1 |
| **Phase 7: WASM** | Module upload/list/start/stop, drag-and-drop, per-module logs | 1 week | P2 |
| **Phase 8: Sensing** | WebSocket integration, live signal charts, pose overlay | 2 weeks | P2 |
| **Phase 9: Mesh View** | Force-directed topology graph, TDM slot visualization, sync status | 1 week | P2 |
| **Phase 10: Polish** | App signing, auto-update, udev rules installer, onboarding wizard | 1 week | P3 |

Total estimated effort: ~11 weeks for a single developer.

## Consequences

### Positive

- **Single pane of glass** — all hardware management, sensing, and visualization in one app
- **No Python dependency** — Rust-native `espflash` replaces `esptool.py` for firmware flashing
- **Replaces 6+ CLI tools** — flash, provision, OTA, WASM management, server control, visualization
- **Accessible to non-developers** — GUI replaces CLI flags and curl commands
- **Cross-platform** — one codebase for Windows, macOS, Linux
- **Workspace integration** — shares types, config, and hardware crates with sensing server
- **Small binary** — ~15-20 MB vs ~150 MB for Electron equivalent

### Negative

- **New frontend dependency** — introduces Node.js/npm build step into the Rust workspace
- **Tauri version churn** — Tauri v2 is recent; API stability is not yet proven at scale
- **webkit2gtk on Linux** — depends on system webview version; old distros may have stale webkit
- **espflash limitations** — the `espflash` crate may not support all chip variants or flash modes that `esptool.py` handles; fallback to bundled Python is needed
- **Maintenance surface** — adds ~5,000 lines of TypeScript and ~2,000 lines of Rust

### Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| espflash cannot flash all ESP32 variants | Medium | High | Bundle esptool.py as fallback sidecar |
| Tauri v2 breaking changes | Low | Medium | Pin to specific Tauri version; update in dedicated PRs |
| Serial port access fails on macOS Sequoia+ | Medium | Medium | Test on latest macOS; document driver requirements |
| webkit2gtk version mismatch on Linux | Medium | Low | Set minimum version in deb/rpm dependencies |
| Sidecar sensing server fails to start | Low | Medium | Detect failure and show manual start instructions |

## References

- Tauri v2 documentation: https://v2.tauri.app/
- espflash crate: https://crates.io/crates/espflash
- mdns-sd crate: https://crates.io/crates/mdns-sd
- ADR-012: ESP32 CSI Sensor Mesh
- ADR-039: ESP32 Edge Intelligence
- ADR-040: WASM Programmable Sensing
- ADR-044: Provisioning Tool Enhancements
- ADR-050: Quality Engineering — Security Hardening
- ADR-051: Sensing Server Decomposition
- `firmware/esp32-csi-node/` — ESP32 firmware source
- `firmware/esp32-csi-node/provision.py` — Current provisioning script
- `rust-port/wifi-densepose-rs/crates/wifi-densepose-sensing-server/` — Sensing server
- `rust-port/wifi-densepose-rs/crates/wifi-densepose-hardware/` — Hardware crate
- `ui/` — Existing web UI
