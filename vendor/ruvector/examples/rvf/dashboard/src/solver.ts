/**
 * Browser-native RVF Solver — loads the raw WASM binary directly.
 *
 * The rvf-solver-wasm crate compiles to a raw cdylib WASM module
 * (no wasm-bindgen). We fetch and instantiate it, then wrap the
 * C-style exports in a TypeScript API.
 */

export interface TrainResult {
  trained: number;
  correct: number;
  accuracy: number;
  patternsLearned: number;
}

export interface CycleMetric {
  cycle: number;
  accuracy: number;
  costPerSolve: number;
  noiseAccuracy: number;
  violations: number;
  patternsLearned: number;
}

export interface ModeResult {
  passed: boolean;
  accuracyMaintained: boolean;
  costImproved: boolean;
  robustnessImproved: boolean;
  zeroViolations: boolean;
  dimensionsImproved: number;
  cycles: CycleMetric[];
}

export interface AcceptanceManifest {
  version: number;
  modeA: ModeResult;
  modeB: ModeResult;
  modeC: ModeResult;
  allPassed: boolean;
  witnessEntries: number;
  witnessChainBytes: number;
}

export interface PolicyState {
  contextStats: Record<string, Record<string, unknown>>;
  earlyCommitPenalties: number;
  earlyCommitsTotal: number;
  earlyCommitsWrong: number;
  prepass: string;
  speculativeAttempts: number;
  speculativeArm2Wins: number;
}

// WASM exports interface
interface WasmExports {
  memory: WebAssembly.Memory;
  rvf_solver_alloc(len: number): number;
  rvf_solver_free(ptr: number, len: number): void;
  rvf_solver_create(): number;
  rvf_solver_destroy(handle: number): number;
  rvf_solver_train(h: number, count: number, minD: number, maxD: number, seedLo: number, seedHi: number): number;
  rvf_solver_acceptance(h: number, holdout: number, training: number, cycles: number, budget: number, seedLo: number, seedHi: number): number;
  rvf_solver_result_len(h: number): number;
  rvf_solver_result_read(h: number, ptr: number): number;
  rvf_solver_policy_len(h: number): number;
  rvf_solver_policy_read(h: number, ptr: number): number;
  rvf_solver_witness_len(h: number): number;
  rvf_solver_witness_read(h: number, ptr: number): number;
}

let wasmInstance: WasmExports | null = null;
let loadPromise: Promise<WasmExports | null> | null = null;

async function loadWasm(): Promise<WasmExports | null> {
  try {
    const response = await fetch('/rvf_solver_wasm.wasm');
    if (!response.ok) throw new Error(`HTTP ${response.status}`);

    const { instance } = await WebAssembly.instantiateStreaming(response, {
      env: {},
    });

    return instance.exports as unknown as WasmExports;
  } catch (e) {
    console.debug('[rvf-solver] WASM load failed, using demo mode:', e);
    return null;
  }
}

function readJson(wasm: WasmExports, handle: number, lenFn: (h: number) => number, readFn: (h: number, ptr: number) => number): unknown | null {
  const len = lenFn(handle);
  if (len <= 0) return null;
  const ptr = wasm.rvf_solver_alloc(len);
  if (ptr === 0) return null;
  try {
    readFn(handle, ptr);
    const buf = new Uint8Array(wasm.memory.buffer, ptr, len);
    const text = new TextDecoder().decode(buf);
    return JSON.parse(text);
  } finally {
    wasm.rvf_solver_free(ptr, len);
  }
}

function splitSeed(seed?: number | bigint): [number, number] {
  if (seed === undefined) {
    const s = BigInt(Math.floor(Math.random() * 2 ** 64));
    return [Number(s & 0xffffffffn), Number((s >> 32n) & 0xffffffffn)];
  }
  const s = typeof seed === 'number' ? BigInt(seed) : seed;
  return [Number(s & 0xffffffffn), Number((s >> 32n) & 0xffffffffn)];
}

/** Live WASM solver wrapper */
class WasmSolver {
  private handle: number;
  private wasm: WasmExports;

  constructor(handle: number, wasm: WasmExports) {
    this.handle = handle;
    this.wasm = wasm;
  }

  train(options: { count: number; minDifficulty?: number; maxDifficulty?: number; seed?: number }): TrainResult {
    const [seedLo, seedHi] = splitSeed(options.seed);
    const correct = this.wasm.rvf_solver_train(
      this.handle, options.count,
      options.minDifficulty ?? 1, options.maxDifficulty ?? 10,
      seedLo, seedHi,
    );
    if (correct < 0) throw new Error('Training failed');

    const raw = readJson(this.wasm, this.handle,
      (h) => this.wasm.rvf_solver_result_len(h),
      (h, p) => this.wasm.rvf_solver_result_read(h, p),
    ) as { trained: number; correct: number; accuracy: number; patterns_learned?: number } | null;

    return {
      trained: raw?.trained ?? options.count,
      correct: raw?.correct ?? correct,
      accuracy: raw?.accuracy ?? correct / options.count,
      patternsLearned: raw?.patterns_learned ?? 0,
    };
  }

  acceptance(options?: { cycles?: number; holdoutSize?: number; trainingPerCycle?: number; stepBudget?: number; seed?: number }): AcceptanceManifest {
    const opts = options ?? {};
    const [seedLo, seedHi] = splitSeed(opts.seed);
    const status = this.wasm.rvf_solver_acceptance(
      this.handle,
      opts.holdoutSize ?? 50, opts.trainingPerCycle ?? 200,
      opts.cycles ?? 5, opts.stepBudget ?? 500,
      seedLo, seedHi,
    );
    if (status < 0) throw new Error('Acceptance failed');

    const raw = readJson(this.wasm, this.handle,
      (h) => this.wasm.rvf_solver_result_len(h),
      (h, p) => this.wasm.rvf_solver_result_read(h, p),
    ) as Record<string, unknown> | null;

    if (!raw) throw new Error('Failed to read acceptance manifest');

    const mapMode = (m: Record<string, unknown>): ModeResult => ({
      passed: !!m.passed,
      accuracyMaintained: !!(m.accuracy_maintained ?? m.accuracyMaintained),
      costImproved: !!(m.cost_improved ?? m.costImproved),
      robustnessImproved: !!(m.robustness_improved ?? m.robustnessImproved),
      zeroViolations: !!(m.zero_violations ?? m.zeroViolations),
      dimensionsImproved: (m.dimensions_improved ?? m.dimensionsImproved ?? 0) as number,
      cycles: ((m.cycles ?? []) as Record<string, unknown>[]).map((c) => ({
        cycle: (c.cycle ?? 0) as number,
        accuracy: (c.accuracy ?? 0) as number,
        costPerSolve: (c.cost_per_solve ?? c.costPerSolve ?? 0) as number,
        noiseAccuracy: (c.noise_accuracy ?? c.noiseAccuracy ?? 0) as number,
        violations: (c.violations ?? 0) as number,
        patternsLearned: (c.patterns_learned ?? c.patternsLearned ?? 0) as number,
      })),
    });

    return {
      version: (raw.version ?? 2) as number,
      modeA: mapMode(raw.mode_a as Record<string, unknown>),
      modeB: mapMode(raw.mode_b as Record<string, unknown>),
      modeC: mapMode(raw.mode_c as Record<string, unknown>),
      allPassed: !!raw.all_passed,
      witnessEntries: (raw.witness_entries ?? 0) as number,
      witnessChainBytes: (raw.witness_chain_bytes ?? 0) as number,
    };
  }

  policy(): PolicyState | null {
    const raw = readJson(this.wasm, this.handle,
      (h) => this.wasm.rvf_solver_policy_len(h),
      (h, p) => this.wasm.rvf_solver_policy_read(h, p),
    ) as Record<string, unknown> | null;

    if (!raw) return null;
    return {
      contextStats: (raw.context_stats ?? raw.contextStats ?? {}) as Record<string, Record<string, unknown>>,
      earlyCommitPenalties: (raw.early_commit_penalties ?? raw.earlyCommitPenalties ?? 0) as number,
      earlyCommitsTotal: (raw.early_commits_total ?? raw.earlyCommitsTotal ?? 0) as number,
      earlyCommitsWrong: (raw.early_commits_wrong ?? raw.earlyCommitsWrong ?? 0) as number,
      prepass: (raw.prepass ?? '') as string,
      speculativeAttempts: (raw.speculative_attempts ?? raw.speculativeAttempts ?? 0) as number,
      speculativeArm2Wins: (raw.speculative_arm2_wins ?? raw.speculativeArm2Wins ?? 0) as number,
    };
  }

  destroy(): void {
    if (this.handle > 0) {
      this.wasm.rvf_solver_destroy(this.handle);
      this.handle = 0;
    }
  }
}

// Public API

export interface SolverInterface {
  train(options: { count: number; minDifficulty?: number; maxDifficulty?: number; seed?: number }): TrainResult;
  acceptance(options?: { cycles?: number; holdoutSize?: number; trainingPerCycle?: number; stepBudget?: number; seed?: number }): AcceptanceManifest;
  policy(): PolicyState | null;
  destroy(): void;
}

let solverInstance: SolverInterface | null = null;
let solverInitPromise: Promise<SolverInterface | null> | null = null;

async function initSolver(): Promise<SolverInterface | null> {
  if (!loadPromise) loadPromise = loadWasm();
  const wasm = await loadPromise;

  if (!wasm) return null;

  const handle = wasm.rvf_solver_create();
  if (handle < 0) {
    console.debug('[rvf-solver] Failed to create solver instance');
    return null;
  }

  return new WasmSolver(handle, wasm);
}

export async function getSolver(): Promise<SolverInterface | null> {
  if (solverInstance) return solverInstance;
  if (!solverInitPromise) solverInitPromise = initSolver();
  solverInstance = await solverInitPromise;
  return solverInstance;
}

/** Returns true if WASM solver is loaded. */
export async function isWasmAvailable(): Promise<boolean> {
  const s = await getSolver();
  return s !== null;
}

// ── Demo fallbacks ──

export function demoTrainResult(count: number, cycle: number): TrainResult {
  const baseAccuracy = 0.55 + cycle * 0.08;
  const accuracy = Math.min(0.98, baseAccuracy + (Math.random() - 0.5) * 0.04);
  const correct = Math.round(count * accuracy);
  return {
    trained: count,
    correct,
    accuracy,
    patternsLearned: Math.floor(count * 0.15 * (1 + cycle * 0.3)),
  };
}

export function demoAcceptanceManifest(): AcceptanceManifest {
  const makeCycles = (baseAcc: number): CycleMetric[] =>
    Array.from({ length: 5 }, (_, i) => ({
      cycle: i + 1,
      accuracy: Math.min(0.99, baseAcc + i * 0.03 + (Math.random() - 0.5) * 0.02),
      costPerSolve: 120 - i * 15 + Math.random() * 10,
      noiseAccuracy: baseAcc - 0.05 + Math.random() * 0.03,
      violations: i < 2 ? 1 : 0,
      patternsLearned: (i + 1) * 12,
    }));

  return {
    version: 2,
    modeA: { passed: true, accuracyMaintained: true, costImproved: false, robustnessImproved: false, zeroViolations: false, dimensionsImproved: 1, cycles: makeCycles(0.62) },
    modeB: { passed: true, accuracyMaintained: true, costImproved: true, robustnessImproved: false, zeroViolations: false, dimensionsImproved: 2, cycles: makeCycles(0.71) },
    modeC: { passed: true, accuracyMaintained: true, costImproved: true, robustnessImproved: true, zeroViolations: true, dimensionsImproved: 3, cycles: makeCycles(0.78) },
    allPassed: true,
    witnessEntries: 25,
    witnessChainBytes: 1825,
  };
}

export function demoPolicyState(): PolicyState {
  const buckets = ['easy', 'medium', 'hard', 'extreme'];
  const modes = ['none', 'weekday', 'hybrid'];
  const contextStats: Record<string, Record<string, unknown>> = {};
  for (const bucket of buckets) {
    contextStats[bucket] = {};
    for (const mode of modes) {
      contextStats[bucket][mode] = {
        attempts: Math.floor(Math.random() * 200) + 50,
        successes: Math.floor(Math.random() * 150) + 30,
        totalSteps: Math.floor(Math.random() * 5000) + 1000,
        alphaSafety: 1.0 + Math.random() * 2,
        betaSafety: 1.0 + Math.random(),
        costEma: 50 + Math.random() * 80,
        earlyCommitWrongs: Math.floor(Math.random() * 5),
      };
    }
  }
  return {
    contextStats,
    earlyCommitPenalties: 3,
    earlyCommitsTotal: 42,
    earlyCommitsWrong: 3,
    prepass: 'naked_singles',
    speculativeAttempts: 156,
    speculativeArm2Wins: 38,
  };
}
