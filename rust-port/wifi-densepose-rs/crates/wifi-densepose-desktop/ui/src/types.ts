// =============================================================================
// types.ts — TypeScript types matching the Rust domain model for RuView
// =============================================================================

// ---------------------------------------------------------------------------
// Node Discovery & Registry
// ---------------------------------------------------------------------------

export type MacAddress = string; // "AA:BB:CC:DD:EE:FF"

export type HealthStatus = "online" | "offline" | "degraded" | "unknown";

export type DiscoveryMethod = "mdns" | "udp_probe" | "http_sweep" | "manual";

export type MeshRole = "coordinator" | "node" | "aggregator";

export type Chip = "esp32" | "esp32s2" | "esp32s3" | "esp32c3" | "esp32c6";

export interface TdmConfig {
  slot: number;
  total: number;
}

export interface NodeCapabilities {
  wasm: boolean;
  ota: boolean;
  csi: boolean;
}

export interface Node {
  ip: string;
  mac: MacAddress | null;
  hostname: string | null;
  node_id: number;
  firmware_version: string | null;
  tdm_slot: number | null;
  tdm_total: number | null;
  edge_tier: number | null;
  uptime_secs: number | null;
  discovery_method: DiscoveryMethod;
  last_seen: string; // ISO 8601 datetime
  health: HealthStatus;
  chip: Chip;
  mesh_role: MeshRole;
  capabilities: NodeCapabilities | null;
  friendly_name: string | null;
  notes: string | null;
}

// ---------------------------------------------------------------------------
// Firmware Flashing
// ---------------------------------------------------------------------------

export type FlashPhase =
  | "connecting"
  | "erasing"
  | "writing"
  | "verifying"
  | "done"
  | "error";

export interface FlashProgress {
  phase: FlashPhase;
  progress_pct: number; // 0.0 - 100.0
  bytes_written: number;
  bytes_total: number;
  speed_bps: number;
}

export interface FirmwareBinary {
  path: string;
  filename: string;
  size_bytes: number;
  chip: Chip | null;
}

export interface FlashSession {
  port: string;
  firmware: FirmwareBinary;
  chip: Chip;
  baud: number;
  progress: FlashProgress | null;
  started_at: string | null;
  finished_at: string | null;
  error: string | null;
}

export interface FlashResult {
  success: boolean;
  duration_ms: number;
  bytes_written: number;
  error: string | null;
}

export interface ChipInfo {
  chip: Chip;
  mac: MacAddress;
  flash_size_bytes: number;
  crystal_freq_mhz: number;
}

// ---------------------------------------------------------------------------
// OTA Updates
// ---------------------------------------------------------------------------

export type OtaStrategy = "sequential" | "tdm_safe" | "parallel";

export type BatchNodeState =
  | "queued"
  | "uploading"
  | "rebooting"
  | "verifying"
  | "done"
  | "failed"
  | "skipped";

export interface OtaSession {
  node_ip: string;
  firmware_path: string;
  progress_pct: number;
  state: BatchNodeState;
  error: string | null;
}

export interface BatchOtaSession {
  strategy: OtaStrategy;
  max_concurrent: number;
  batch_delay_secs: number;
  fail_fast: boolean;
  nodes: OtaSession[];
  started_at: string | null;
  finished_at: string | null;
}

export interface OtaResult {
  node_ip: string;
  success: boolean;
  previous_version: string | null;
  new_version: string | null;
  duration_ms: number;
  error: string | null;
}

export interface OtaStatus {
  current_version: string;
  partition: string;
  update_available: boolean;
}

// ---------------------------------------------------------------------------
// WASM Modules
// ---------------------------------------------------------------------------

export type WasmModuleState = "running" | "stopped" | "error" | "loading";

export interface WasmModule {
  module_id: string;
  name: string;
  size_bytes: number;
  state: WasmModuleState;
  node_ip: string;
  loaded_at: string | null;
  error: string | null;
  memory_used_kb: number | null;
  cpu_usage_pct: number | null;
  exec_count: number | null;
}

// ---------------------------------------------------------------------------
// Sensing Server
// ---------------------------------------------------------------------------

export interface ServerConfig {
  http_port: number;
  ws_port: number;
  udp_port: number;
  static_dir: string | null;
  model_dir: string | null;
  log_level: string;
}

export interface ServerStatus {
  running: boolean;
  pid: number | null;
  http_port: number | null;
  ws_port: number | null;
  udp_port: number | null;
  uptime_secs: number | null;
  error: string | null;
}

export interface SensingUpdate {
  timestamp: string;
  node_id: number;
  subcarrier_count: number;
  rssi: number;
  activity: string | null;
  confidence: number | null;
}

// ---------------------------------------------------------------------------
// Serial Port
// ---------------------------------------------------------------------------

export interface SerialPort {
  name: string;          // e.g. "COM3" or "/dev/ttyUSB0"
  description: string;   // e.g. "Silicon Labs CP210x"
  chip: Chip | null;     // detected chip type, if any
  manufacturer: string | null;
  vid: number | null;    // USB vendor ID
  pid: number | null;    // USB product ID
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

export interface AppSettings {
  server_http_port: number;
  server_ws_port: number;
  server_udp_port: number;
  bind_address: string;
  ui_path: string;
  ota_psk: string;
  auto_discover: boolean;
  discover_interval_ms: number;
  theme: "dark" | "light";
}
