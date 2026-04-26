/* Default `NvsimClient` implementation. Talks to the Web Worker that
 * hosts the nvsim WASM module. ADR-092 §5.4 + §6.3. */

import {
  type NvsimClient,
  type SceneJson,
  type PipelineConfigJson,
  type RunOpts,
  type MagFrameBatch,
  type NvsimEvent,
  parseFrameBatch,
} from './NvsimClient';

interface PendingRequest<T = unknown> {
  resolve: (v: T) => void;
  reject: (err: Error) => void;
}

export interface WasmBootInfo {
  buildVersion: string;
  frameMagic: number;
  frameBytes: number;
  expectedWitnessHex: string;
}

export class WasmClient implements NvsimClient {
  private worker: Worker;
  private nextId = 1;
  private pending = new Map<number, PendingRequest<unknown>>();
  private frameSubs = new Set<(b: MagFrameBatch) => void>();
  private eventSubs = new Set<(e: NvsimEvent) => void>();
  private bootInfo: WasmBootInfo | null = null;

  constructor() {
    this.worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
    this.worker.addEventListener('message', (ev) => this.onMessage(ev));
    this.worker.addEventListener('error', (e) =>
      this.eventSubs.forEach((s) => s({ type: 'log', level: 'err', msg: String(e.message) })),
    );
  }

  private onMessage(ev: MessageEvent): void {
    const m = ev.data as { type: string; id?: number; [k: string]: unknown };
    if (m.type === 'frames') {
      const buf = m.batch as ArrayBuffer;
      const bytes = new Uint8Array(buf);
      const frames = parseFrameBatch(bytes);
      const batch: MagFrameBatch = { frames, bytes };
      this.frameSubs.forEach((s) => s(batch));
      const fps = m.fps as number;
      if (fps > 0) {
        this.eventSubs.forEach((s) => s({ type: 'fps', value: fps }));
      }
      return;
    }
    if (m.type === 'state') {
      this.eventSubs.forEach((s) =>
        s({
          type: 'state',
          running: Boolean(m.running),
          t: 0,
          framesEmitted: Number(m.framesEmitted ?? 0),
        }),
      );
      return;
    }
    if (m.type === 'ready') {
      return;
    }
    if (m.type === 'err' && m.id == null) {
      this.eventSubs.forEach((s) =>
        s({ type: 'log', level: 'err', msg: String(m.msg) }),
      );
      return;
    }
    if (typeof m.id === 'number' && this.pending.has(m.id)) {
      const p = this.pending.get(m.id)!;
      this.pending.delete(m.id);
      if (m.type === 'err') p.reject(new Error(String(m.msg)));
      else p.resolve(m);
    }
  }

  private rpc<T = unknown>(msg: Record<string, unknown>, transfer: Transferable[] = []): Promise<T> {
    const id = this.nextId++;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
      this.worker.postMessage({ ...msg, id }, transfer);
    });
  }

  async boot(): Promise<WasmBootInfo> {
    if (this.bootInfo) return this.bootInfo;
    const r = await this.rpc<{ buildVersion: string; frameMagic: number; frameBytes: number; expectedWitnessHex: string }>(
      { type: 'boot' },
    );
    this.bootInfo = {
      buildVersion: r.buildVersion,
      frameMagic: r.frameMagic,
      frameBytes: r.frameBytes,
      expectedWitnessHex: r.expectedWitnessHex,
    };
    return this.bootInfo;
  }

  async loadScene(scene: SceneJson): Promise<void> {
    await this.rpc({ type: 'setScene', json: JSON.stringify(scene) });
  }

  async setConfig(cfg: PipelineConfigJson): Promise<void> {
    await this.rpc({ type: 'setConfig', json: JSON.stringify(cfg) });
  }

  async setSeed(seed: bigint): Promise<void> {
    await this.rpc({ type: 'setSeed', seed: Number(seed & 0xFFFFFFFFn) });
  }

  async reset(): Promise<void> {
    await this.rpc({ type: 'reset' });
  }

  async run(_opts?: RunOpts): Promise<void> {
    await this.rpc({ type: 'run' });
  }

  async pause(): Promise<void> {
    await this.rpc({ type: 'pause' });
  }

  async step(_direction: 'fwd' | 'back', _dtMs: number): Promise<void> {
    await this.rpc({ type: 'step' });
  }

  onFrames(cb: (batch: MagFrameBatch) => void): void { this.frameSubs.add(cb); }
  onEvent(cb: (ev: NvsimEvent) => void): void { this.eventSubs.add(cb); }

  async generateWitness(samples: number): Promise<Uint8Array> {
    const r = await this.rpc<{ witness: ArrayBuffer; hex: string }>({ type: 'witnessGenerate', samples });
    return new Uint8Array(r.witness);
  }

  async verifyWitness(expected: Uint8Array): Promise<{ ok: true } | { ok: false; actual: Uint8Array }> {
    const buf = expected.slice().buffer;
    const r = await this.rpc<{ ok: boolean; actual: ArrayBuffer; actualHex: string }>(
      { type: 'witnessVerify', samples: 256, expected: buf },
      [buf],
    );
    if (r.ok) return { ok: true };
    return { ok: false, actual: new Uint8Array(r.actual) };
  }

  async exportProofBundle(): Promise<Blob> {
    // Bundle = REFERENCE_SCENE_JSON + computed witness hex + version. Wraps
    // the same artifacts `Proof::generate` produces natively. ADR-092 §6.1.
    const w = await this.generateWitness(256);
    const hex = Array.from(w).map((b) => b.toString(16).padStart(2, '0')).join('');
    const info = this.bootInfo ?? (await this.boot());
    const manifest = JSON.stringify(
      {
        kind: 'nvsim-proof-bundle',
        version: info.buildVersion,
        seed: '0x0000002A',
        nSamples: 256,
        witness: hex,
        expected: info.expectedWitnessHex,
        ok: hex === info.expectedWitnessHex,
        ts: new Date().toISOString(),
      },
      null,
      2,
    );
    return new Blob([manifest], { type: 'application/json' });
  }

  async buildId(): Promise<string> {
    const r = await this.rpc<{ buildId: string }>({ type: 'buildId' });
    return r.buildId;
  }

  async close(): Promise<void> {
    this.worker.terminate();
  }
}
