import type { RvfSolverWasmExports } from '../pkg/rvf_solver';
import type {
  TrainOptions,
  TrainResult,
  AcceptanceOptions,
  AcceptanceManifest,
  PolicyState,
} from './types';

let wasmExports: RvfSolverWasmExports | null = null;

async function getWasm(): Promise<RvfSolverWasmExports> {
  if (wasmExports) return wasmExports;
  // Dynamic import to support both CJS and ESM
  const initModule = await import('../pkg/rvf_solver');
  const init = initModule.default || initModule;
  wasmExports = await init();
  return wasmExports;
}

function splitSeed(seed?: bigint | number): [number, number] {
  if (seed === undefined) {
    const s = BigInt(Math.floor(Math.random() * 2 ** 64));
    return [Number(s & 0xFFFFFFFFn), Number((s >> 32n) & 0xFFFFFFFFn)];
  }
  const s = typeof seed === 'number' ? BigInt(seed) : seed;
  return [Number(s & 0xFFFFFFFFn), Number((s >> 32n) & 0xFFFFFFFFn)];
}

function readJson<T>(
  wasm: RvfSolverWasmExports,
  handle: number,
  lenFn: (h: number) => number,
  readFn: (h: number, ptr: number) => number,
): T | null {
  const len = lenFn(handle);
  if (len <= 0) return null;
  const ptr = wasm.rvf_solver_alloc(len);
  if (ptr === 0) return null;
  try {
    readFn(handle, ptr);
    const buf = new Uint8Array(wasm.memory.buffer, ptr, len);
    const text = new TextDecoder().decode(buf);
    return JSON.parse(text) as T;
  } finally {
    wasm.rvf_solver_free(ptr, len);
  }
}

/**
 * RVF Self-Learning Solver.
 *
 * Wraps the rvf-solver-wasm WASM module providing:
 * - PolicyKernel with Thompson Sampling (two-signal model)
 * - Context-bucketed bandit (18 buckets)
 * - KnowledgeCompiler with signature-based pattern cache
 * - Speculative dual-path execution
 * - Three-loop adaptive solver (fast/medium/slow)
 * - SHAKE-256 tamper-evident witness chain
 */
export class RvfSolver {
  private handle: number;
  private wasm: RvfSolverWasmExports;

  private constructor(handle: number, wasm: RvfSolverWasmExports) {
    this.handle = handle;
    this.wasm = wasm;
  }

  /**
   * Create a new solver instance.
   * Initializes the WASM module on first call.
   */
  static async create(): Promise<RvfSolver> {
    const wasm = await getWasm();
    const handle = wasm.rvf_solver_create();
    if (handle < 0) {
      throw new Error('Failed to create solver instance (max 8 concurrent instances)');
    }
    return new RvfSolver(handle, wasm);
  }

  /**
   * Train the solver on generated puzzles.
   *
   * Uses the three-loop architecture:
   * - Fast loop: constraint propagation solver
   * - Medium loop: PolicyKernel skip-mode selection
   * - Slow loop: KnowledgeCompiler pattern distillation
   */
  train(options: TrainOptions): TrainResult {
    const [seedLo, seedHi] = splitSeed(options.seed);
    const correct = this.wasm.rvf_solver_train(
      this.handle,
      options.count,
      options.minDifficulty ?? 1,
      options.maxDifficulty ?? 10,
      seedLo,
      seedHi,
    );
    if (correct < 0) {
      throw new Error('Training failed: invalid handle');
    }
    const result = readJson<TrainResult>(
      this.wasm,
      this.handle,
      (h) => this.wasm.rvf_solver_result_len(h),
      (h, p) => this.wasm.rvf_solver_result_read(h, p),
    );
    return result ?? {
      trained: options.count,
      correct,
      accuracy: correct / options.count,
      patternsLearned: 0,
    };
  }

  /**
   * Run the full acceptance test with training/holdout cycles.
   *
   * Runs all three ablation modes:
   * - Mode A: Fixed heuristic policy
   * - Mode B: Compiler-suggested policy
   * - Mode C: Learned Thompson Sampling policy
   *
   * Returns the full manifest with per-cycle metrics and witness chain.
   */
  acceptance(options?: AcceptanceOptions): AcceptanceManifest {
    const opts = options ?? {};
    const [seedLo, seedHi] = splitSeed(opts.seed);
    const status = this.wasm.rvf_solver_acceptance(
      this.handle,
      opts.holdoutSize ?? 50,
      opts.trainingPerCycle ?? 200,
      opts.cycles ?? 5,
      opts.stepBudget ?? 500,
      seedLo,
      seedHi,
    );
    if (status < 0) {
      throw new Error('Acceptance test failed: invalid handle');
    }
    const manifest = readJson<any>(
      this.wasm,
      this.handle,
      (h) => this.wasm.rvf_solver_result_len(h),
      (h, p) => this.wasm.rvf_solver_result_read(h, p),
    );
    if (!manifest) {
      throw new Error('Failed to read acceptance manifest');
    }
    return {
      version: manifest.version,
      modeA: manifest.mode_a,
      modeB: manifest.mode_b,
      modeC: manifest.mode_c,
      allPassed: manifest.all_passed,
      witnessEntries: manifest.witness_entries,
      witnessChainBytes: manifest.witness_chain_bytes,
    };
  }

  /**
   * Get the current policy state (Thompson Sampling parameters,
   * context buckets, KnowledgeCompiler cache stats).
   */
  policy(): PolicyState | null {
    return readJson<PolicyState>(
      this.wasm,
      this.handle,
      (h) => this.wasm.rvf_solver_policy_len(h),
      (h, p) => this.wasm.rvf_solver_policy_read(h, p),
    );
  }

  /**
   * Get the raw SHAKE-256 witness chain bytes.
   *
   * The witness chain is 73 bytes per entry and provides
   * tamper-evident proof of all training/acceptance operations.
   * Verifiable using `rvf_witness_verify` from `@ruvector/rvf-wasm`.
   */
  witnessChain(): Uint8Array | null {
    const len = this.wasm.rvf_solver_witness_len(this.handle);
    if (len <= 0) return null;
    const ptr = this.wasm.rvf_solver_alloc(len);
    if (ptr === 0) return null;
    try {
      this.wasm.rvf_solver_witness_read(this.handle, ptr);
      const buf = new Uint8Array(this.wasm.memory.buffer, ptr, len);
      // Copy to avoid referencing WASM memory after free
      return new Uint8Array(buf);
    } finally {
      this.wasm.rvf_solver_free(ptr, len);
    }
  }

  /**
   * Destroy the solver instance and free WASM resources.
   */
  destroy(): void {
    if (this.handle > 0) {
      this.wasm.rvf_solver_destroy(this.handle);
      this.handle = 0;
    }
  }
}
