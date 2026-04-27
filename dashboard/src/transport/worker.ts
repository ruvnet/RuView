/* Web Worker hosting the nvsim WASM module.
 *
 * Boots `/nvsim-pkg/nvsim.js`, instantiates `WasmPipeline`, then
 * postMessage-RPCs with the main thread. Frame batches are returned
 * as `ArrayBuffer` transfers so we don't pay a copy on the hot path.
 *
 * ADR-092 §5.4.
 */

/// <reference lib="WebWorker" />

const ws = self as unknown as DedicatedWorkerGlobalScope;

interface WasmPipelineApi {
  run(n: number): Uint8Array;
  runWithWitness(n: number): { frames: Uint8Array; witness: Uint8Array; frameCount: number };
  free?: () => void;
}
type WasmPipelineCtor = new (sceneJson: string, configJson: string, seed: number) => WasmPipelineApi;
type WasmPipelineStatic = WasmPipelineCtor & {
  buildVersion(): string;
  frameMagic(): number;
  frameBytes(): number;
};

interface TransientResult {
  bRecoveredT: Float64Array;
  bMagT: number;
  noiseFloorPtSqrtHz: number;
  sigmaPt: Float64Array;
  nFrames: number;
  witnessHex: string;
}

interface NvsimPkg {
  default: (input?: unknown) => Promise<unknown>;
  WasmPipeline: WasmPipelineStatic;
  referenceSceneJson: () => string;
  expectedReferenceWitnessHex: () => string;
  hexWitness: (b: Uint8Array) => string;
  referenceWitness: () => Uint8Array;
  runTransient: (sceneJson: string, configJson: string, seed: number, nSamples: number) => TransientResult;
}

let _WasmPipeline!: WasmPipelineStatic;
let referenceSceneJson!: () => string;
let expectedReferenceWitnessHex!: () => string;
let hexWitness!: (b: Uint8Array) => string;
let referenceWitness!: () => Uint8Array;
let runTransient!: (sceneJson: string, configJson: string, seed: number, nSamples: number) => TransientResult;

async function loadPkg(base: string): Promise<void> {
  // `base` is the dashboard's BASE_URL injected by Vite, prefixed with the
  // origin so we get an absolute URL the dynamic import can resolve. In dev
  // this is "/", in prod under GitHub Pages it's "/RuView/nvsim/".
  const absoluteBase = new URL(base, ws.location.origin).href;
  const pkgUrl = new URL('nvsim-pkg/nvsim.js', absoluteBase).href;
  const pkg = (await import(/* @vite-ignore */ pkgUrl)) as NvsimPkg;
  await pkg.default();
  _WasmPipeline = pkg.WasmPipeline;
  referenceSceneJson = pkg.referenceSceneJson;
  expectedReferenceWitnessHex = pkg.expectedReferenceWitnessHex;
  hexWitness = pkg.hexWitness;
  referenceWitness = pkg.referenceWitness;
  runTransient = pkg.runTransient;
}

let pipeline: WasmPipelineApi | null = null;
let configJson = '';
let sceneJson = '';
let seed = BigInt(0xCAFEBABE);

let running = false;
let timer: number | null = null;
let framesEmitted = 0;
let tStart = 0;

function ensureRebuild(): void {
  if (!sceneJson) sceneJson = referenceSceneJson();
  if (!configJson) {
    configJson = JSON.stringify({
      digitiser: { f_s_hz: 10000, f_mod_hz: 1000 },
      sensor: {
        gamma_fwhm_hz: 1.0e6,
        t1_s: 5.0e-3,
        t2_s: 1.0e-6,
        t2_star_s: 200e-9,
        contrast: 0.03,
        n_spins: 1.0e12,
        shot_noise_disabled: false,
      },
      dt_s: null,
    });
  }
  pipeline?.free?.();
  pipeline = new _WasmPipeline(sceneJson, configJson, Number(seed & 0xFFFFFFFFn));
}

function post(msg: unknown, transfer: Transferable[] = []): void {
  // postMessage Transferable overload: pass transfer list as 2nd arg
  (ws.postMessage as (msg: unknown, t: Transferable[]) => void)(msg, transfer);
}

function startTimer(): void {
  if (timer !== null) return;
  tStart = performance.now();
  framesEmitted = 0;
  const tick = (): void => {
    if (!running || !pipeline) return;
    // Per-tick: simulate 32 frames; push as one batch.
    const n = 32;
    const bytes = pipeline.run(n);
    framesEmitted += n;
    const elapsed = (performance.now() - tStart) / 1000;
    const fps = elapsed > 0 ? framesEmitted / elapsed : 0;
    post(
      { type: 'frames', batch: bytes.buffer, count: n, fps, framesEmitted },
      [bytes.buffer],
    );
    timer = ws.setTimeout(tick, 16);
  };
  timer = ws.setTimeout(tick, 0);
}

function stopTimer(): void {
  if (timer !== null) {
    ws.clearTimeout(timer);
    timer = null;
  }
}

ws.addEventListener('message', async (ev: MessageEvent): Promise<void> => {
  const m = ev.data as { type: string; id?: number; [k: string]: unknown };
  try {
    switch (m.type) {
      case 'boot': {
        const base = (m.base as string | undefined) ?? '/';
        await loadPkg(base);
        ensureRebuild();
        post({
          type: 'booted',
          id: m.id,
          buildVersion: _WasmPipeline.buildVersion(),
          frameMagic: _WasmPipeline.frameMagic(),
          frameBytes: _WasmPipeline.frameBytes(),
          expectedWitnessHex: expectedReferenceWitnessHex(),
        });
        break;
      }
      case 'setScene': {
        sceneJson = m.json as string;
        ensureRebuild();
        post({ type: 'ack', id: m.id });
        break;
      }
      case 'setConfig': {
        configJson = m.json as string;
        ensureRebuild();
        post({ type: 'ack', id: m.id });
        break;
      }
      case 'setSeed': {
        seed = BigInt(m.seed as string | number | bigint);
        ensureRebuild();
        post({ type: 'ack', id: m.id });
        break;
      }
      case 'reset': {
        stopTimer();
        running = false;
        ensureRebuild();
        framesEmitted = 0;
        post({ type: 'ack', id: m.id });
        post({ type: 'state', running: false, framesEmitted });
        break;
      }
      case 'run': {
        if (!pipeline) ensureRebuild();
        running = true;
        startTimer();
        post({ type: 'ack', id: m.id });
        post({ type: 'state', running: true, framesEmitted });
        break;
      }
      case 'pause': {
        running = false;
        stopTimer();
        post({ type: 'ack', id: m.id });
        post({ type: 'state', running: false, framesEmitted });
        break;
      }
      case 'step': {
        if (!pipeline) ensureRebuild();
        const bytes = pipeline!.run(1);
        framesEmitted += 1;
        post(
          { type: 'frames', batch: bytes.buffer, count: 1, fps: 0, framesEmitted },
          [bytes.buffer],
        );
        post({ type: 'ack', id: m.id });
        break;
      }
      case 'witnessGenerate': {
        if (!pipeline) ensureRebuild();
        const samples = (m.samples as number) ?? 256;
        const result = pipeline!.runWithWitness(samples) as {
          frames: Uint8Array;
          witness: Uint8Array;
          frameCount: number;
        };
        const hex = hexWitness(result.witness);
        post(
          {
            type: 'witness',
            id: m.id,
            witness: result.witness.buffer,
            hex,
            frameCount: result.frameCount,
          },
          [result.witness.buffer],
        );
        break;
      }
      case 'witnessVerify': {
        // Verify always runs the *canonical* reference scene at seed=42, N=256
        // so the witness matches Proof::EXPECTED_WITNESS_HEX byte-for-byte.
        // The user's working scene/config/seed don't affect the witness.
        const expectedBuf = m.expected as ArrayBuffer;
        const expected = new Uint8Array(expectedBuf);
        const actual = referenceWitness();
        let ok = actual.length === expected.length;
        if (ok) {
          for (let i = 0; i < expected.length; i++) {
            if (actual[i] !== expected[i]) { ok = false; break; }
          }
        }
        const actualBuf = actual.slice().buffer;
        post(
          {
            type: 'verify',
            id: m.id,
            ok,
            actual: actualBuf,
            actualHex: hexWitness(actual),
          },
          [actualBuf],
        );
        break;
      }
      case 'runTransient': {
        const sceneJson = m.scene as string;
        const configJson = m.config as string;
        const seed = (m.seed as number) ?? 0;
        const samples = (m.samples as number) ?? 64;
        const r = runTransient(sceneJson, configJson, seed, samples);
        post({
          type: 'transient',
          id: m.id,
          bRecoveredT: Array.from(r.bRecoveredT),
          bMagT: r.bMagT,
          noiseFloorPtSqrtHz: r.noiseFloorPtSqrtHz,
          sigmaPt: Array.from(r.sigmaPt),
          nFrames: r.nFrames,
          witnessHex: r.witnessHex,
        });
        break;
      }
      case 'buildId': {
        post({
          type: 'buildId',
          id: m.id,
          buildId: `nvsim@${_WasmPipeline.buildVersion()}`,
        });
        break;
      }
      default:
        post({ type: 'err', id: m.id, msg: `unknown op ${m.type}` });
    }
  } catch (e) {
    post({ type: 'err', id: m.id, msg: (e as Error).message ?? String(e) });
  }
});

post({ type: 'ready' });
