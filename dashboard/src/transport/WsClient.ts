/* WebSocket transport client — talks to a `nvsim-server` Axum host
 * (v2/crates/nvsim-server). REST for control plane, binary WebSocket
 * for the MagFrame stream. Mirrors the WasmClient interface so the
 * dashboard can swap transports at runtime without code changes.
 *
 * ADR-092 §5.2 / §6.2.
 */

import {
  type NvsimClient,
  type SceneJson,
  type PipelineConfigJson,
  type RunOpts,
  type MagFrameBatch,
  type NvsimEvent,
  type TransientRunResult,
  parseFrameBatch,
} from './NvsimClient';

interface HealthBody {
  nvsim_version: string;
  magic: number;
  frame_bytes: number;
  expected_witness_hex: string;
}

interface VerifyBody {
  ok: boolean;
  actual_hex: string;
  expected_hex: string;
}

interface WitnessBody {
  witness_hex: string;
  samples: number;
  seed_hex: string;
}

export interface WsBootInfo {
  buildVersion: string;
  frameMagic: number;
  frameBytes: number;
  expectedWitnessHex: string;
}

/** Convert a base URL (e.g. `http://host:7878`) to its WebSocket peer (`ws://host:7878`). */
function toWsUrl(baseUrl: string): string {
  if (baseUrl.startsWith('ws://') || baseUrl.startsWith('wss://')) return baseUrl;
  return baseUrl.replace(/^http/, 'ws');
}

export class WsClient implements NvsimClient {
  private baseUrl: string;
  private wsUrl: string;
  private ws: WebSocket | null = null;
  private bootInfo: WsBootInfo | null = null;
  private frameSubs = new Set<(b: MagFrameBatch) => void>();
  private eventSubs = new Set<(e: NvsimEvent) => void>();
  private running = false;
  private framesEmitted = 0;
  private fpsLast = performance.now();
  private fpsCount = 0;

  /** @param baseUrl e.g. `http://localhost:7878` */
  constructor(baseUrl: string) {
    this.baseUrl = baseUrl.replace(/\/$/, '');
    this.wsUrl = `${toWsUrl(this.baseUrl)}/ws/stream`;
  }

  private async json<T>(path: string, init?: RequestInit): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      ...init,
      headers: { 'content-type': 'application/json', ...(init?.headers ?? {}) },
    });
    if (!res.ok) throw new Error(`${path}: ${res.status} ${res.statusText}`);
    return (await res.json()) as T;
  }

  async boot(): Promise<WsBootInfo> {
    if (this.bootInfo) return this.bootInfo;
    const h = await this.json<HealthBody>('/api/health');
    this.bootInfo = {
      buildVersion: h.nvsim_version,
      frameMagic: h.magic,
      frameBytes: h.frame_bytes,
      expectedWitnessHex: h.expected_witness_hex,
    };
    this.openWs();
    return this.bootInfo;
  }

  private openWs(): void {
    if (this.ws) return;
    const ws = new WebSocket(this.wsUrl);
    ws.binaryType = 'arraybuffer';
    ws.onopen = () => {
      this.eventSubs.forEach((s) =>
        s({ type: 'log', level: 'ok', msg: `ws/stream connected · ${this.wsUrl}` }),
      );
    };
    ws.onclose = () => {
      this.ws = null;
      this.eventSubs.forEach((s) =>
        s({ type: 'log', level: 'warn', msg: 'ws/stream closed' }),
      );
    };
    ws.onerror = () => {
      this.eventSubs.forEach((s) =>
        s({ type: 'log', level: 'err', msg: `ws/stream error · ${this.wsUrl}` }),
      );
    };
    ws.onmessage = (ev: MessageEvent) => {
      if (!(ev.data instanceof ArrayBuffer)) return;
      const bytes = new Uint8Array(ev.data);
      const frames = parseFrameBatch(bytes);
      if (frames.length === 0) return;
      const batch: MagFrameBatch = { frames, bytes };
      this.frameSubs.forEach((s) => s(batch));
      this.framesEmitted += frames.length;
      this.fpsCount += frames.length;
      const now = performance.now();
      if (now - this.fpsLast >= 1000) {
        const fps = (this.fpsCount * 1000) / (now - this.fpsLast);
        this.eventSubs.forEach((s) => s({ type: 'fps', value: fps }));
        this.fpsLast = now;
        this.fpsCount = 0;
      }
    };
    this.ws = ws;
  }

  async loadScene(scene: SceneJson): Promise<void> {
    await this.json('/api/scene', { method: 'PUT', body: JSON.stringify(scene) });
  }
  async setConfig(cfg: PipelineConfigJson): Promise<void> {
    await this.json('/api/config', { method: 'PUT', body: JSON.stringify(cfg) });
  }
  async setSeed(seed: bigint): Promise<void> {
    await this.json('/api/seed', {
      method: 'PUT',
      body: JSON.stringify({ seed_hex: '0x' + seed.toString(16).toUpperCase().padStart(16, '0') }),
    });
  }
  async reset(): Promise<void> {
    await this.json('/api/reset', { method: 'POST' });
    this.running = false;
    this.framesEmitted = 0;
    this.eventSubs.forEach((s) => s({ type: 'state', running: false, t: 0, framesEmitted: 0 }));
  }
  async run(_opts?: RunOpts): Promise<void> {
    await this.json('/api/run', { method: 'POST' });
    this.running = true;
    this.eventSubs.forEach((s) =>
      s({ type: 'state', running: true, t: 0, framesEmitted: this.framesEmitted }),
    );
  }
  async pause(): Promise<void> {
    await this.json('/api/pause', { method: 'POST' });
    this.running = false;
    this.eventSubs.forEach((s) =>
      s({ type: 'state', running: false, t: 0, framesEmitted: this.framesEmitted }),
    );
  }
  async step(direction: 'fwd' | 'back', dtMs: number): Promise<void> {
    await this.json('/api/step', { method: 'POST', body: JSON.stringify({ direction, dt_ms: dtMs }) });
  }

  onFrames(cb: (b: MagFrameBatch) => void): void { this.frameSubs.add(cb); }
  onEvent(cb: (e: NvsimEvent) => void): void { this.eventSubs.add(cb); }

  async generateWitness(samples: number): Promise<Uint8Array> {
    const r = await this.json<WitnessBody>('/api/witness/generate', {
      method: 'POST',
      body: JSON.stringify({ samples }),
    });
    const out = new Uint8Array(32);
    for (let i = 0; i < 32; i++) out[i] = parseInt(r.witness_hex.slice(i * 2, i * 2 + 2), 16);
    return out;
  }

  async verifyWitness(expected: Uint8Array): Promise<{ ok: true } | { ok: false; actual: Uint8Array }> {
    const expected_hex = Array.from(expected).map((b) => b.toString(16).padStart(2, '0')).join('');
    const r = await this.json<VerifyBody>('/api/witness/verify', {
      method: 'POST',
      body: JSON.stringify({ expected_hex, samples: 256 }),
    });
    if (r.ok) return { ok: true };
    const actual = new Uint8Array(32);
    for (let i = 0; i < 32; i++) actual[i] = parseInt(r.actual_hex.slice(i * 2, i * 2 + 2), 16);
    return { ok: false, actual };
  }

  async exportProofBundle(): Promise<Blob> {
    const text = await fetch(`${this.baseUrl}/api/export-proof`, { method: 'POST' }).then((r) => r.text());
    return new Blob([text], { type: 'application/json' });
  }

  async runTransient(
    scene: SceneJson,
    config: PipelineConfigJson,
    _seed: bigint,
    samples: number,
  ): Promise<TransientRunResult> {
    // Server doesn't expose a transient route in V1 — the dashboard's
    // Ghost Murmur sandbox falls back to the WASM client when transport
    // is WS. Stub here returns a zero-result so the caller can detect.
    void scene; void config; void samples;
    return {
      bRecoveredT: [0, 0, 0],
      bMagT: 0,
      noiseFloorPtSqrtHz: 0,
      sigmaPt: [0, 0, 0],
      nFrames: 0,
      witnessHex: '(transient route not available in WS transport — V1 limitation)',
    };
  }

  async buildId(): Promise<string> {
    const info = this.bootInfo ?? (await this.boot());
    return `nvsim@${info.buildVersion} (ws)`;
  }

  async close(): Promise<void> {
    this.ws?.close();
    this.ws = null;
  }
}
