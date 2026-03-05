# Deployment Platform Domain Model

The Deployment Platform domain covers everything from cross-compiling the sensing server for ARM targets to managing TV box appliances running Armbian: provisioning devices, deploying binaries, configuring kiosk displays, and coordinating multi-room installations. It bridges the gap between the Sensing Server domain (which produces the binary) and the physical hardware it runs on.

This document defines the system using [Domain-Driven Design](https://martinfowler.com/bliki/DomainDrivenDesign.html) (DDD): bounded contexts that own their data and rules, aggregate roots that enforce invariants, value objects that carry meaning, and domain events that connect everything.

**Bounded Contexts:**

| # | Context | Responsibility | Key ADRs | Code |
|---|---------|----------------|----------|------|
| 1 | [Appliance Management](#1-appliance-management-context) | Device inventory, provisioning, health monitoring, OTA updates for TV box deployments | [ADR-046](../adr/ADR-046-android-tv-box-armbian-deployment.md) | `scripts/deploy/`, `config/armbian/` |
| 2 | [Cross-Compilation](#2-cross-compilation-context) | Build pipeline for aarch64, binary packaging, CI/CD release artifacts | [ADR-046](../adr/ADR-046-android-tv-box-armbian-deployment.md) | `.github/workflows/`, `Cross.toml` |
| 3 | [Display Kiosk](#3-display-kiosk-context) | HDMI output management, Chromium kiosk mode, screen rotation, auto-start | [ADR-046](../adr/ADR-046-android-tv-box-armbian-deployment.md) | `config/armbian/kiosk/` |
| 4 | [WiFi CSI Bridge](#4-wifi-csi-bridge-context) | Custom WiFi driver CSI extraction, protocol translation to ESP32 binary format | [ADR-046](../adr/ADR-046-android-tv-box-armbian-deployment.md) | `tools/csi-bridge/` |
| 5 | [Network Topology](#5-network-topology-context) | ESP32 mesh ↔ TV box connectivity, dedicated AP mode, multi-room routing | [ADR-046](../adr/ADR-046-android-tv-box-armbian-deployment.md), [ADR-012](../adr/ADR-012-esp32-csi-sensor-mesh.md) | `config/armbian/network/` |

---

## Domain-Driven Design Specification

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Appliance** | A TV box running Armbian with the sensing server deployed, treated as a managed device in the fleet |
| **Fleet** | The set of all appliances across a multi-room or multi-site installation |
| **Deployment Package** | A self-contained archive containing the sensing-server binary, systemd unit, configuration, and setup script for a target architecture |
| **Kiosk Mode** | Chromium running in full-screen, no-UI mode pointing at `localhost:3000`, auto-started by systemd on HDMI-connected appliances |
| **CSI Bridge** | A userspace daemon that reads CSI data from a patched WiFi driver and re-encodes it as ESP32-compatible UDP frames for the sensing server |
| **Dedicated AP** | An optional `hostapd`-managed WiFi access point on the TV box that creates an isolated network for ESP32 nodes |
| **OTA Update** | Over-the-air binary replacement: download new sensing-server binary, validate checksum, swap via atomic rename, restart service |
| **Reference Device** | A TV box model that has been tested and validated for Armbian + sensing-server deployment (e.g., T95 Max+ / S905X3) |
| **Provisioning** | First-time setup of an appliance: flash Armbian to SD, deploy package, configure WiFi, start services |
| **Health Beacon** | Periodic JSON payload sent by each appliance to a central coordinator (if multi-room) containing uptime, CPU temp, memory usage, inference latency, connected ESP32 count |

---

## Bounded Contexts

### 1. Appliance Management Context

**Responsibility:** Track deployed TV box appliances, provision new devices, monitor health, and coordinate OTA updates across the fleet.

```
+------------------------------------------------------------+
|            Appliance Management Context                    |
+------------------------------------------------------------+
|                                                            |
|  +----------------+    +----------------+                  |
|  |  Device        |    |  Provisioning  |                  |
|  |  Registry      |    |  Service       |                  |
|  |  (fleet state) |    |  (first-time   |                  |
|  |                |    |   setup)       |                  |
|  +-------+--------+    +-------+--------+                  |
|          |                     |                           |
|          +----------+----------+                           |
|                     v                                      |
|          +-------------------+                             |
|          |  Health Monitor   |                             |
|          |  (beacon receiver,|                             |
|          |   thermal alerts, |                             |
|          |   connectivity)   |                             |
|          +--------+----------+                             |
|                   v                                        |
|          +-------------------+                             |
|          |  OTA Updater      |                             |
|          |  (binary swap,    |                             |
|          |   rollback,       |                             |
|          |   checksum verify)|                             |
|          +-------------------+                             |
|                                                            |
+------------------------------------------------------------+
```

**Aggregates:**

```rust
/// Aggregate Root: A managed TV box appliance in the fleet.
/// Identified by MAC address of the primary Ethernet interface.
pub struct Appliance {
    /// Unique device identifier (Ethernet MAC address).
    pub device_id: DeviceId,
    /// Human-readable name (e.g., "living-room", "bedroom-1").
    pub name: String,
    /// Hardware model (e.g., "T95 Max+ S905X3").
    pub hardware_model: HardwareModel,
    /// Current deployment state.
    pub state: ApplianceState,
    /// Installed sensing-server version.
    pub server_version: SemanticVersion,
    /// Network configuration.
    pub network: NetworkConfig,
    /// Last received health beacon.
    pub last_health: Option<HealthBeacon>,
    /// Provisioning timestamp.
    pub provisioned_at: DateTime<Utc>,
    /// Connected ESP32 node IDs (from last beacon).
    pub connected_nodes: Vec<u8>,
}

/// Lifecycle states for an appliance.
pub enum ApplianceState {
    /// SD card prepared, not yet booted.
    Provisioned,
    /// Booted and running, health beacons received.
    Online,
    /// No health beacon for >5 minutes.
    Unreachable,
    /// OTA update in progress.
    Updating,
    /// Manual maintenance / stopped.
    Offline,
    /// Thermal throttling or hardware issue detected.
    Degraded,
}
```

**Value Objects:**

```rust
/// Hardware model specification for a TV box.
pub struct HardwareModel {
    /// Marketing name (e.g., "T95 Max+").
    pub name: String,
    /// SoC identifier (e.g., "Amlogic S905X3").
    pub soc: String,
    /// WiFi chipset (e.g., "RTL8822CS").
    pub wifi_chipset: String,
    /// Total RAM in MB.
    pub ram_mb: u32,
    /// eMMC storage in GB.
    pub emmc_gb: u32,
    /// Whether CSI bridge is supported for this WiFi chipset.
    pub csi_bridge_supported: bool,
    /// Armbian device tree name (e.g., "meson-sm1-sei610").
    pub armbian_dtb: String,
}

/// Periodic health report from an appliance.
pub struct HealthBeacon {
    pub device_id: DeviceId,
    pub timestamp: DateTime<Utc>,
    pub uptime_secs: u64,
    pub cpu_temp_celsius: f32,
    pub cpu_usage_percent: f32,
    pub memory_used_mb: u32,
    pub memory_total_mb: u32,
    pub disk_used_percent: f32,
    pub inference_latency_ms: f32,
    pub connected_esp32_nodes: Vec<u8>,
    pub server_version: SemanticVersion,
    pub csi_frames_per_sec: f32,
    pub websocket_clients: u32,
}

/// Network configuration for an appliance.
pub struct NetworkConfig {
    /// Primary IP address (Ethernet or WiFi client).
    pub ip_address: IpAddr,
    /// Whether the appliance runs a dedicated AP for ESP32 nodes.
    pub dedicated_ap: Option<DedicatedApConfig>,
    /// UDP port for ESP32 CSI reception.
    pub csi_udp_port: u16,  // default: 5005
    /// HTTP port for sensing server.
    pub http_port: u16,     // default: 3000
}

/// Configuration for a dedicated WiFi AP hosted by the appliance.
pub struct DedicatedApConfig {
    /// SSID for the ESP32 mesh network.
    pub ssid: String,
    /// WPA2 passphrase.
    pub passphrase: String,
    /// Channel (1-11 for 2.4 GHz).
    pub channel: u8,
    /// DHCP range for connected ESP32 nodes.
    pub dhcp_range: (IpAddr, IpAddr),
}

/// Unique device identifier (Ethernet MAC).
pub struct DeviceId(pub [u8; 6]);

/// Semantic version for tracking installed software.
pub struct SemanticVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub pre: Option<String>,
}
```

**Domain Services:**
- `ProvisioningService` — Generates Armbian SD card image with pre-configured deployment package, WiFi credentials, and systemd units
- `HealthMonitorService` — Listens for UDP health beacons from fleet appliances, triggers alerts on thermal throttling (>80°C), unreachable (>5 min), or high memory usage (>90%)
- `OtaUpdateService` — Downloads new binary from release URL, verifies SHA-256 checksum, performs atomic swap (`rename(new, current)`), restarts systemd service, rolls back if health beacon fails within 60s

**Invariants:**
- Device ID (MAC address) is immutable after provisioning
- OTA update refuses to proceed if current CPU temperature >75°C (thermal headroom)
- Rollback is automatic if no healthy beacon is received within 60 seconds of restart
- Dedicated AP SSID must not match the upstream WiFi SSID

---

### 2. Cross-Compilation Context

**Responsibility:** Build the sensing-server binary for ARM64 targets, package deployment archives, and manage CI/CD release artifacts.

```
+------------------------------------------------------------+
|           Cross-Compilation Context                        |
+------------------------------------------------------------+
|                                                            |
|  +----------------+    +----------------+                  |
|  |  Cross.toml    |    |  GitHub Actions|                  |
|  |  (target cfg)  |    |  CI Matrix     |                  |
|  +-------+--------+    +-------+--------+                  |
|          |                     |                           |
|          +----------+----------+                           |
|                     v                                      |
|          +-------------------+                             |
|          |  Build Pipeline   |                             |
|          |  (cross build     |                             |
|          |   --target        |                             |
|          |   aarch64-unknown-|                             |
|          |   linux-gnu)      |                             |
|          +--------+----------+                             |
|                   v                                        |
|          +-------------------+                             |
|          |  Binary Packager  |                             |
|          |  (strip, compress,|---> .tar.gz artifact        |
|          |   bundle assets,  |                             |
|          |   systemd units)  |                             |
|          +-------------------+                             |
|                                                            |
+------------------------------------------------------------+
```

**Value Objects:**

```rust
/// A packaged deployment archive for a target platform.
pub struct DeploymentPackage {
    /// Target triple (e.g., "aarch64-unknown-linux-gnu").
    pub target: String,
    /// Sensing server binary (stripped).
    pub binary: PathBuf,
    /// Binary size in bytes.
    pub binary_size: u64,
    /// SHA-256 checksum of the binary.
    pub checksum: String,
    /// Systemd service unit file.
    pub service_unit: String,
    /// Static web UI assets directory.
    pub ui_assets: PathBuf,
    /// Armbian configuration files (kiosk, network, etc.).
    pub config_files: Vec<PathBuf>,
    /// Setup script (runs on first boot).
    pub setup_script: PathBuf,
    /// Version being packaged.
    pub version: SemanticVersion,
}

/// Build target specification.
pub struct BuildTarget {
    /// Rust target triple.
    pub triple: String,
    /// CPU architecture description.
    pub arch: String,
    /// Whether NEON SIMD is available.
    pub has_neon: bool,
    /// Cross-compilation Docker image.
    pub cross_image: String,
    /// Binary size limit in bytes.
    pub size_limit: u64,
}
```

**Supported Targets:**

| Target Triple | Architecture | Use Case | Size Limit |
|---------------|-------------|----------|------------|
| `x86_64-unknown-linux-gnu` | x86-64 | PC/laptop (existing) | 30 MB |
| `aarch64-unknown-linux-gnu` | ARM64 | TV box (Armbian) | 15 MB |
| `armv7-unknown-linux-gnueabihf` | ARMv7 | Older TV boxes (32-bit) | 12 MB |
| `x86_64-pc-windows-msvc` | x86-64 | Windows (existing) | 30 MB |

**Invariants:**
- Stripped binary must be under size limit for target
- SHA-256 checksum is computed and included in every deployment package
- UI assets are embedded in binary via `include_dir!` or bundled alongside
- No native GPU dependencies — CPU-only inference (candle or ONNX Runtime)

---

### 3. Display Kiosk Context

**Responsibility:** Manage HDMI output on TV box appliances, running Chromium in kiosk mode to display the sensing dashboard full-screen on boot.

```
+------------------------------------------------------------+
|              Display Kiosk Context                         |
+------------------------------------------------------------+
|                                                            |
|  +----------------+    +----------------+                  |
|  |  systemd       |    |  Chromium      |                  |
|  |  autologin +   |    |  Kiosk Launch  |                  |
|  |  X11/Wayland   |    |  (full-screen, |                  |
|  |  session       |    |   no-UI bars)  |                  |
|  +-------+--------+    +-------+--------+                  |
|          |                     |                           |
|          +----------+----------+                           |
|                     v                                      |
|          +-------------------+                             |
|          |  Display Manager  |                             |
|          |  (resolution,     |                             |
|          |   rotation,       |                             |
|          |   overscan,       |                             |
|          |   sleep/wake)     |                             |
|          +-------------------+                             |
|                                                            |
+------------------------------------------------------------+
```

**Value Objects:**

```rust
/// Display configuration for kiosk mode.
pub struct KioskConfig {
    /// URL to display (default: "http://localhost:3000").
    pub url: String,
    /// Screen rotation in degrees (0, 90, 180, 270).
    pub rotation: u16,
    /// Whether to hide the mouse cursor.
    pub hide_cursor: bool,
    /// Auto-refresh interval in seconds (0 = disabled).
    pub auto_refresh_secs: u32,
    /// Display sleep schedule (e.g., off 23:00-06:00).
    pub sleep_schedule: Option<SleepSchedule>,
    /// Overscan compensation percentage (0-10).
    pub overscan_percent: u8,
}

/// Sleep schedule for display power management.
pub struct SleepSchedule {
    /// Time to turn display off (HH:MM local time).
    pub sleep_time: String,
    /// Time to turn display on (HH:MM local time).
    pub wake_time: String,
}
```

**Invariants:**
- Chromium kiosk starts only after sensing-server systemd unit is `active`
- If Chromium crashes, systemd restarts it within 5 seconds (`Restart=always`)
- Display sleep/wake uses CEC commands (HDMI-CEC) to control TV power when available
- No browser UI elements are visible (address bar, scrollbars, etc.)

---

### 4. WiFi CSI Bridge Context

**Responsibility:** Extract CSI data from patched WiFi drivers on the TV box and translate it into ESP32-compatible binary frames for the sensing server. This is the Phase 2 custom firmware path.

```
+------------------------------------------------------------+
|              WiFi CSI Bridge Context                       |
+------------------------------------------------------------+
|                                                            |
|  +----------------+    +----------------+                  |
|  |  Patched WiFi  |    |  CSI Reader    |                  |
|  |  Driver        |    |  (Netlink /    |                  |
|  |  (kernel space)|    |   procfs /     |                  |
|  |  CSI hooks     |    |   UDP socket)  |                  |
|  +-------+--------+    +-------+--------+                  |
|          |                     |                           |
|          +----------+----------+                           |
|                     v                                      |
|          +-------------------+                             |
|          |  Protocol         |                             |
|          |  Translator       |                             |
|          |  (chipset CSI →   |                             |
|          |   ESP32 binary    |                             |
|          |   0xC5100001)     |                             |
|          +--------+----------+                             |
|                   v                                        |
|          +-------------------+                             |
|          |  UDP Sender       |                             |
|          |  (localhost:5005) |---> sensing-server           |
|          +-------------------+                             |
|                                                            |
+------------------------------------------------------------+
```

**Value Objects:**

```rust
/// Raw CSI extraction from a WiFi chipset.
pub struct ChipsetCsiFrame {
    /// Source chipset type.
    pub chipset: WifiChipset,
    /// Timestamp of extraction (kernel monotonic clock).
    pub timestamp_us: u64,
    /// Number of subcarriers (varies by chipset and bandwidth).
    pub n_subcarriers: u16,
    /// Number of spatial streams / antennas.
    pub n_streams: u8,
    /// Channel frequency in MHz.
    pub freq_mhz: u16,
    /// Bandwidth (20/40/80/160 MHz).
    pub bandwidth_mhz: u16,
    /// RSSI in dBm.
    pub rssi_dbm: i8,
    /// Noise floor estimate in dBm.
    pub noise_floor_dbm: i8,
    /// Complex CSI values (I/Q pairs) per subcarrier per stream.
    pub csi_matrix: Vec<Complex<f32>>,
    /// Source MAC address (BSSID of the AP being measured).
    pub source_mac: [u8; 6],
}

/// Supported WiFi chipsets for CSI extraction.
pub enum WifiChipset {
    /// Broadcom BCM43455 via Nexmon CSI patches.
    BroadcomBcm43455,
    /// Realtek RTL8822CS via modified rtw88 driver.
    RealtekRtl8822cs,
    /// MediaTek MT7661 via mt76 driver modification.
    MediatekMt7661,
}

/// Translated frame in ESP32 binary protocol (ADR-018).
pub struct Esp32CompatFrame {
    /// Magic: 0xC5100001
    pub magic: u32,
    /// Virtual node ID assigned to this WiFi interface.
    pub node_id: u8,
    /// Number of antennas / spatial streams.
    pub n_antennas: u8,
    /// Number of subcarriers (resampled to match ESP32 format).
    pub n_subcarriers: u8,
    /// Frequency in MHz.
    pub freq_mhz: u16,
    /// Sequence number (monotonic counter).
    pub sequence: u32,
    /// RSSI in dBm.
    pub rssi: i8,
    /// Noise floor in dBm.
    pub noise_floor: i8,
    /// Amplitude values (extracted from complex CSI).
    pub amplitudes: Vec<f32>,
    /// Phase values (extracted from complex CSI).
    pub phases: Vec<f32>,
}
```

**Domain Services:**
- `CsiExtractionService` — Reads raw CSI from patched driver via Netlink socket (BCM43455), procfs (RTL8822CS), or UDP (MT7661)
- `SubcarrierResamplerService` — Resamples chipset-specific subcarrier counts to match ESP32 format (e.g., 256 → 128 via decimation or interpolation)
- `ProtocolTranslatorService` — Converts `ChipsetCsiFrame` to `Esp32CompatFrame` with ADR-018 binary encoding
- `CalibrationService` — Compensates for chipset-specific phase offsets, antenna spacing, and gain differences relative to ESP32 CSI

**Invariants:**
- Bridge assigns virtual `node_id` in range 200-254 (reserved for non-ESP32 sources) to avoid collision with physical ESP32 node IDs (1-199)
- Subcarrier resampling preserves frequency ordering (lowest to highest)
- Phase values are unwrapped before encoding (continuous, not wrapped to ±π)
- Bridge daemon starts only if a compatible patched driver is detected at boot

---

### 5. Network Topology Context

**Responsibility:** Manage network connectivity between ESP32 sensor nodes and TV box appliances, including optional dedicated AP mode and multi-room routing.

```
+------------------------------------------------------------+
|            Network Topology Context                        |
+------------------------------------------------------------+
|                                                            |
|  +----------------+    +----------------+                  |
|  |  hostapd       |    |  DHCP Server   |                  |
|  |  (dedicated AP |    |  (dnsmasq for  |                  |
|  |   for ESP32    |    |   ESP32 nodes) |                  |
|  |   mesh)        |    |                |                  |
|  +-------+--------+    +-------+--------+                  |
|          |                     |                           |
|          +----------+----------+                           |
|                     v                                      |
|          +-------------------+                             |
|          |  Topology Manager |                             |
|          |  (node discovery, |                             |
|          |   IP assignment,  |                             |
|          |   route config)   |                             |
|          +--------+----------+                             |
|                   v                                        |
|          +-------------------+                             |
|          |  Firewall Rules   |                             |
|          |  (iptables/nft:   |                             |
|          |   allow UDP 5005, |                             |
|          |   block external  |                             |
|          |   access to ESP32 |                             |
|          |   subnet)         |                             |
|          +-------------------+                             |
|                                                            |
+------------------------------------------------------------+
```

**Value Objects:**

```rust
/// Network topology for a single-room deployment.
pub struct RoomTopology {
    /// Appliance acting as the aggregator.
    pub appliance: DeviceId,
    /// Whether the appliance runs a dedicated AP.
    pub dedicated_ap: bool,
    /// Connected ESP32 nodes with their assigned IPs.
    pub nodes: Vec<EspNodeConnection>,
    /// Upstream network interface (Ethernet or WiFi client).
    pub uplink_interface: String,
    /// Sensing network interface (dedicated AP or same as uplink).
    pub sensing_interface: String,
}

/// An ESP32 node's network connection to the appliance.
pub struct EspNodeConnection {
    /// ESP32 node ID (from firmware NVS).
    pub node_id: u8,
    /// MAC address of the ESP32.
    pub mac: [u8; 6],
    /// Assigned IP address (via DHCP or static).
    pub ip: IpAddr,
    /// Last CSI frame received timestamp.
    pub last_seen: DateTime<Utc>,
    /// Average CSI frames per second from this node.
    pub fps: f32,
}
```

**Domain Services:**
- `DedicatedApService` — Configures `hostapd` to create a WPA2 AP on the TV box's WiFi interface, assigns DHCP range via `dnsmasq`, sets up IP forwarding
- `NodeDiscoveryService` — Monitors UDP port 5005 for new ESP32 node IDs, registers them in the topology, alerts on node departure (no frames for >30s)
- `FirewallService` — Configures `nftables`/`iptables` to isolate the ESP32 subnet from the upstream LAN, allowing only UDP 5005 inbound and HTTP 3000 outbound

**Invariants:**
- Dedicated AP uses a separate WiFi interface or virtual interface (not the uplink)
- ESP32 subnet is isolated from upstream LAN by default (firewall rules)
- If dedicated AP is disabled, ESP32 nodes must be on the same LAN subnet as the appliance
- Node discovery does not require mDNS or any discovery protocol — ESP32 nodes are configured with the appliance's IP via NVS provisioning (ADR-044)

---

## Domain Events

| Event | Published By | Consumed By | Payload |
|-------|-------------|-------------|---------|
| `ApplianceProvisioned` | Appliance Mgmt | Fleet Dashboard | `{ device_id, name, hardware_model, ip }` |
| `ApplianceOnline` | Appliance Mgmt | Fleet Dashboard | `{ device_id, server_version, uptime }` |
| `ApplianceUnreachable` | Appliance Mgmt | Fleet Dashboard, Alerting | `{ device_id, last_seen, reason }` |
| `ApplianceDegraded` | Appliance Mgmt | Fleet Dashboard, Alerting | `{ device_id, cpu_temp, reason }` |
| `OtaUpdateStarted` | Appliance Mgmt | Fleet Dashboard | `{ device_id, from_version, to_version }` |
| `OtaUpdateCompleted` | Appliance Mgmt | Fleet Dashboard | `{ device_id, new_version, duration_secs }` |
| `OtaUpdateRolledBack` | Appliance Mgmt | Fleet Dashboard, Alerting | `{ device_id, attempted_version, rollback_version, reason }` |
| `BinaryBuilt` | Cross-Compilation | Release Pipeline | `{ target, version, binary_size, checksum }` |
| `DeploymentPackageCreated` | Cross-Compilation | Appliance Mgmt | `{ target, version, package_url }` |
| `KioskStarted` | Display Kiosk | Appliance Mgmt | `{ device_id, url, resolution }` |
| `KioskCrashed` | Display Kiosk | Appliance Mgmt | `{ device_id, exit_code, restart_count }` |
| `CsiBridgeStarted` | WiFi CSI Bridge | Appliance Mgmt, Sensing Server | `{ device_id, chipset, virtual_node_id }` |
| `CsiBridgeFailed` | WiFi CSI Bridge | Appliance Mgmt | `{ device_id, chipset, error }` |
| `EspNodeDiscovered` | Network Topology | Appliance Mgmt | `{ appliance_id, node_id, mac, ip }` |
| `EspNodeLost` | Network Topology | Appliance Mgmt, Alerting | `{ appliance_id, node_id, last_seen }` |
| `DedicatedApStarted` | Network Topology | Appliance Mgmt | `{ appliance_id, ssid, channel }` |

---

## Context Map

```
+-------------------+          +---------------------+
| Appliance         |--------->|  Fleet Dashboard    |
| Management        | events   |  (external UI for   |
| (fleet state)     | -------> |   multi-room mgmt)  |
+--------+----------+          +---------------------+
         |
         | provisions, monitors
         v
+-------------------+          +---------------------+
| Cross-Compilation |--------->| GitHub Releases     |
| (build pipeline)  | uploads  | (binary artifacts)  |
+-------------------+          +---------------------+
         |
         | provides binary
         v
+-------------------+          +---------------------+
| Display Kiosk     |--------->| Sensing Server      |
| (Chromium on      | loads    | (upstream domain,   |
|  HDMI output)     | UI from  |  produces web UI)   |
+-------------------+          +----------+----------+
                                          ^
+-------------------+                     |
| WiFi CSI Bridge   |-----UDP 5005------>|
| (patched driver)  |  ESP32 compat      |
+-------------------+  frames            |
                                          |
+-------------------+                     |
| Network Topology  |-----UDP 5005------>|
| (ESP32 mesh       |  ESP32 frames      |
|  connectivity)    |                     |
+-------------------+                     |
```

**Relationships:**

| Upstream | Downstream | Relationship | Mechanism |
|----------|-----------|--------------|-----------|
| Cross-Compilation | Appliance Mgmt | Supplier-Consumer | Build produces binary; Appliance Mgmt deploys it |
| Appliance Mgmt | Display Kiosk | Customer-Supplier | Appliance Mgmt starts kiosk after server is healthy |
| WiFi CSI Bridge | Sensing Server (external) | Conformist | Bridge adapts its output to match ESP32 binary protocol (ADR-018) |
| Network Topology | Sensing Server (external) | Shared Kernel | Both depend on UDP port 5005 and ESP32 node ID scheme |
| Appliance Mgmt | Network Topology | Customer-Supplier | Appliance config determines whether dedicated AP is enabled |

---

## Anti-Corruption Layers

### ESP32 Protocol ACL (CSI Bridge)

The WiFi CSI Bridge translates chipset-specific CSI formats (Nexmon, rtw88, mt76) into the ESP32 binary protocol (ADR-018). The sensing server never knows whether frames came from a real ESP32 or a TV box WiFi chipset. Virtual node IDs (200-254) prevent collision with physical ESP32 IDs but are otherwise treated identically by the ingestion context.

### Armbian Platform ACL

Appliance Management abstracts over Armbian specifics (device tree names, boot configuration, dtb overlays) through the `HardwareModel` value object. Higher-level contexts (Cross-Compilation, Display Kiosk) depend only on the target triple (`aarch64-unknown-linux-gnu`) and systemd service interface, not on Amlogic/Allwinner/Rockchip kernel specifics.

### Fleet Coordination ACL

For multi-room deployments, each appliance is self-contained (runs its own sensing server, display, and network). The fleet dashboard reads health beacons but never controls individual appliances directly. OTA updates are pulled by each appliance (not pushed), maintaining the appliance as the authority over its own state.

---

## Related

- [ADR-046: Android TV Box / Armbian Deployment](../adr/ADR-046-android-tv-box-armbian-deployment.md) — Primary architectural decision
- [ADR-012: ESP32 CSI Sensor Mesh](../adr/ADR-012-esp32-csi-sensor-mesh.md) — ESP32 mesh network design
- [ADR-018: Dev Implementation](../adr/ADR-018-dev-implementation.md) — ESP32 binary CSI protocol
- [ADR-039: Edge Intelligence](../adr/ADR-039-esp32-edge-intelligence.md) — On-device processing tiers
- [ADR-044: Provisioning Tool](../adr/ADR-044-provisioning-tool-enhancements.md) — NVS provisioning for ESP32 nodes
- [Hardware Platform Domain Model](hardware-platform-domain-model.md) — Upstream domain (ESP32 hardware)
- [Sensing Server Domain Model](sensing-server-domain-model.md) — Upstream domain (server software)
