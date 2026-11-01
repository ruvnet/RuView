// SPDX-License-Identifier: MIT
// Type declarations for @ruvnet/ruview/sdk (ADR-369).

export type FirmwareVariant = 's3-8mb' | 's3-4mb' | 'c6' | 'c5';
export type TrainMode = 'pose-smoke' | 'pose' | 'room';
export type Split = 'chronological' | 'blocked-gap' | 'grouped-subject' | 'grouped-session' | 'random-frame';
export type DoctorGroup = 'runtime' | 'harness' | 'hosts' | 'repo' | 'rust' | 'python' | 'firmware' | 'serial' | 'devices' | 'remote' | 'sensing' | 'kernel';

/** Every tool resolves to structured JSON; failures are `ok: false` with a reason. */
export interface ToolResult {
  ok: boolean;
  reason?: string;
  detail?: string;
  remedy?: string;
  dryRun?: boolean;
  [key: string]: unknown;
}

export interface DoctorCheck {
  group: DoctorGroup;
  id: string;
  status: 'pass' | 'warn' | 'fail' | 'skip';
  detail: string;
  remedy?: string;
}

export interface DoctorReport {
  ok: boolean;
  summary: { pass: number; warn: number; fail: number; skip: number };
  checks: DoctorCheck[];
  nextSteps: string[];
}

export interface FlashArgs {
  port: string;
  bundle: string;
  variant?: FirmwareVariant;
  baud?: 115200 | 230400 | 460800 | 921600;
  allow_unverified?: boolean;
  boot_log_seconds?: number;
  confirm?: boolean;
}

export interface TrainArgs {
  mode?: TrainMode;
  config?: string;
  data_dir?: string;
  checkpoint_dir?: string;
  enrollment?: string;
  output?: string;
  samples?: number;
  cuda?: boolean;
  confirm?: boolean;
}

export interface TrainingReport {
  metric?: string;
  model_score: number;
  baseline_score?: number;
  split?: Split;
  train_subjects?: string[];
  test_subjects?: string[];
  train_end?: string;
  test_start?: string;
  n_test?: number;
  reproducer?: string;
  data?: 'real' | 'synthetic';
}

export interface GateResult extends ToolResult {
  verdict: 'PASS' | 'FAIL';
  evidence: 'MEASURED' | 'CLAIMED' | 'SYNTHETIC' | null;
  deltaPp: number | null;
  findings: { severity: 'fail' | 'warn'; code: string; message: string }[];
  statement: string | null;
}

export interface RuView {
  call(tool: string, args?: Record<string, unknown>): Promise<ToolResult>;
  tools(): { name: string; description: string; inputSchema: object; annotations: object }[];
  doctor(options?: { groups?: DoctorGroup[]; port?: string; probe_port?: boolean; sensing_url?: string }): Promise<DoctorReport>;
  onboard(path?: 'docker-demo' | 'repo-build' | 'live-esp32'): Promise<ToolResult>;
  guidance(args?: { topic?: string; query?: string; limit?: number }): Promise<ToolResult>;
  memorySearch(query: string, limit?: number): Promise<ToolResult>;
  claimCheck(text: string): Promise<ToolResult>;
  verify(repo?: string): Promise<ToolResult>;
  spaces(args?: Record<string, unknown>): Promise<ToolResult>;
  firmware: {
    variants: Readonly<Record<FirmwareVariant, { chip: string; chipName: string; flashSize: string }>>;
    ports(): Promise<ToolResult>;
    plan(args: FlashArgs): Promise<ToolResult>;
    flash(args: FlashArgs): Promise<ToolResult>;
    monitor(port: string, seconds?: number, baud?: 115200 | 230400 | 460800 | 921600 | 1500000): Promise<ToolResult>;
  };
  training: {
    modes: readonly TrainMode[];
    plan(args?: TrainArgs): Promise<ToolResult>;
    run(args?: TrainArgs): Promise<ToolResult>;
    gate(report: TrainingReport): Promise<GateResult>;
    calibrate(args?: { step?: 'baseline' | 'enroll' | 'train-room' | 'room-watch'; args?: string[]; confirm?: boolean }): Promise<ToolResult>;
  };
  devices: {
    scan(): Promise<ToolResult>;
    esp32(args?: {
      udp_port?: number; bind?: '0.0.0.0' | '127.0.0.1' | '::' | '::1'; seconds?: number; max_packets?: number;
      /** Run live single-antenna CSI through @ruvnet/ruview-kernel (optional package). */
      analyze?: boolean; node_id?: number; backend?: 'wasm' | 'napi' | 'auto'; analyze_max_frames?: number;
      /** Return recent binned amplitude frames for the waterfall view. */
      spectrum?: boolean; spectrum_bins?: number; spectrum_frames?: number;
    }): Promise<ToolResult>;
    mmwave(args: { source?: 'serial'; port: string; model?: 'auto' | 'mr60bha2' | 'ld2410'; seconds?: number } | { source: 'esphome'; host: string; api_port?: number; seconds?: number }): Promise<ToolResult>;
    lidar(args: { source: 'rplidar'; port: string; baud?: 115200 | 256000 | 460800 | 921600; seconds?: number } | { source: 'iphone'; url: string; seconds?: number; max_frames?: number }): Promise<ToolResult>;
  };
  hosts: {
    list(): Promise<ToolResult>;
    run(host: string, tool: string, args?: Record<string, unknown>): Promise<ToolResult>;
  };
  kernel: {
    selfTest(args?: { backend?: 'wasm' | 'napi' | 'auto'; seconds?: number }): Promise<ToolResult>;
    load(options?: { backend?: 'wasm' | 'napi' | 'auto' }): Promise<unknown>;
  };
  startMcpServer(): Promise<void>;
}

export class RuViewError extends Error {
  tool: string;
  reason?: string;
  result: ToolResult;
}

export const FIRMWARE_VARIANTS: RuView['firmware']['variants'];
export const TRAIN_MODES: readonly TrainMode[];
export const SPLITS: readonly Split[];
export const DOCTOR_GROUPS: readonly DoctorGroup[];
export const GUIDANCE_TOPICS: readonly string[];

export function createRuView(options?: { strict?: boolean }): RuView;
export default createRuView;
