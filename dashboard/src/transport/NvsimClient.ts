/* Common NvsimClient interface — both WasmClient and WsClient implement it.
 * Dashboard binds to this interface and never to a concrete client.
 * Aligns with ADR-092 §5.2.
 */

export interface PipelineConfigJson {
  digitiser?: {
    f_s_hz: number;
    f_mod_hz: number;
    lp_cutoff_hz?: number;
  };
  sensor?: {
    gamma_fwhm_hz?: number;
    t1_s?: number;
    t2_s?: number;
    t2_star_s?: number;
    contrast?: number;
    n_spins?: number;
    n_centers?: number;
    shot_noise_disabled?: boolean;
  };
  dt_s?: number | null;
}

export interface SceneJson {
  dipoles: { position: [number, number, number]; moment: [number, number, number] }[];
  loops: {
    centre: [number, number, number];
    normal: [number, number, number];
    radius: number;
    current: number;
    n_segments: number;
  }[];
  ferrous: {
    position: [number, number, number];
    volume: number;
    susceptibility: number;
  }[];
  eddy: unknown[];
  sensors: [number, number, number][];
  ambient_field: [number, number, number];
}

export interface MagFrameRecord {
  magic: number;
  version: number;
  flags: number;
  sensorId: number;
  tUs: bigint;
  bPt: [number, number, number];
  sigmaPt: [number, number, number];
  noiseFloorPtSqrtHz: number;
  temperatureK: number;
  raw: Uint8Array;
}

export interface MagFrameBatch {
  frames: MagFrameRecord[];
  bytes: Uint8Array;
}

export type NvsimEvent =
  | { type: 'log'; level: 'info' | 'warn' | 'err' | 'dbg' | 'ok'; msg: string }
  | { type: 'witness'; hex: string }
  | { type: 'fps'; value: number }
  | { type: 'state'; running: boolean; t: number; framesEmitted: number };

export interface RunOpts { frames?: number }

/** One-shot pipeline run for "what would the sensor recover at this scene?"
 * use cases. Doesn't disturb the running pipeline. */
export interface TransientRunResult {
  bRecoveredT: [number, number, number];
  bMagT: number;
  noiseFloorPtSqrtHz: number;
  sigmaPt: [number, number, number];
  nFrames: number;
  witnessHex: string;
}

export interface NvsimClient {
  loadScene(scene: SceneJson): Promise<void>;
  setConfig(cfg: PipelineConfigJson): Promise<void>;
  setSeed(seed: bigint): Promise<void>;
  reset(): Promise<void>;
  run(opts?: RunOpts): Promise<void>;
  pause(): Promise<void>;
  step(direction: 'fwd' | 'back', dtMs: number): Promise<void>;

  onFrames(cb: (batch: MagFrameBatch) => void): void;
  onEvent(cb: (ev: NvsimEvent) => void): void;

  generateWitness(samples: number): Promise<Uint8Array>;
  verifyWitness(expected: Uint8Array): Promise<{ ok: true } | { ok: false; actual: Uint8Array }>;
  exportProofBundle(): Promise<Blob>;
  runTransient(scene: SceneJson, config: PipelineConfigJson, seed: bigint, samples: number): Promise<TransientRunResult>;

  buildId(): Promise<string>;
  close(): Promise<void>;
}

/** Parse one MagFrame from a 60-byte slice. Layout matches `nvsim::frame`. */
export function parseMagFrame(view: DataView, offset: number, raw: Uint8Array): MagFrameRecord {
  // v1 layout: magic(u32) | version(u16) | flags(u16) | sensor_id(u16) | _reserved(u16) |
  //            t_us(u64) | b_pt[3](f32) | sigma_pt[3](f32) | noise_floor_pt_sqrt_hz(f32) |
  //            temperature_k(f32) — 60 bytes total. All little-endian.
  const magic = view.getUint32(offset + 0, true);
  const version = view.getUint16(offset + 4, true);
  const flags = view.getUint16(offset + 6, true);
  const sensorId = view.getUint16(offset + 8, true);
  // skip 2 bytes reserved at offset+10
  const tUs = view.getBigUint64(offset + 12, true);
  const bx = view.getFloat32(offset + 20, true);
  const by = view.getFloat32(offset + 24, true);
  const bz = view.getFloat32(offset + 28, true);
  const sx = view.getFloat32(offset + 32, true);
  const sy = view.getFloat32(offset + 36, true);
  const sz = view.getFloat32(offset + 40, true);
  const noiseFloorPtSqrtHz = view.getFloat32(offset + 44, true);
  const temperatureK = view.getFloat32(offset + 48, true);
  return {
    magic,
    version,
    flags,
    sensorId,
    tUs,
    bPt: [bx, by, bz],
    sigmaPt: [sx, sy, sz],
    noiseFloorPtSqrtHz,
    temperatureK,
    raw: raw.subarray(offset, offset + 60),
  };
}

export function parseFrameBatch(bytes: Uint8Array): MagFrameRecord[] {
  const frameSize = 60;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const out: MagFrameRecord[] = [];
  for (let off = 0; off + frameSize <= bytes.byteLength; off += frameSize) {
    out.push(parseMagFrame(view, off, bytes));
  }
  return out;
}
