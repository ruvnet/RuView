/** Configuration for solver training. */
export interface TrainOptions {
  /** Number of puzzles to generate and solve. */
  count: number;
  /** Minimum puzzle difficulty (1-10). Default: 1. */
  minDifficulty?: number;
  /** Maximum puzzle difficulty (1-10). Default: 10. */
  maxDifficulty?: number;
  /** RNG seed (BigInt or number). Default: random. */
  seed?: bigint | number;
}

/** Result of a training run. */
export interface TrainResult {
  /** Number of puzzles trained on. */
  trained: number;
  /** Number solved correctly. */
  correct: number;
  /** Accuracy (correct / trained). */
  accuracy: number;
  /** Number of patterns learned by the ReasoningBank. */
  patternsLearned: number;
}

/** Configuration for acceptance testing. */
export interface AcceptanceOptions {
  /** Number of holdout puzzles per cycle. Default: 50. */
  holdoutSize?: number;
  /** Number of training puzzles per cycle. Default: 200. */
  trainingPerCycle?: number;
  /** Number of train/test cycles. Default: 5. */
  cycles?: number;
  /** Maximum steps per puzzle. Default: 500. */
  stepBudget?: number;
  /** RNG seed (BigInt or number). Default: random. */
  seed?: bigint | number;
}

/** Per-cycle metrics from an acceptance mode. */
export interface CycleMetrics {
  cycle: number;
  accuracy: number;
  costPerSolve: number;
}

/** Result of a single acceptance mode (A, B, or C). */
export interface AcceptanceModeResult {
  passed: boolean;
  finalAccuracy: number;
  cycles: CycleMetrics[];
}

/** Full acceptance test manifest. */
export interface AcceptanceManifest {
  version: number;
  /** Mode A: fixed heuristic policy. */
  modeA: AcceptanceModeResult;
  /** Mode B: compiler-suggested policy. */
  modeB: AcceptanceModeResult;
  /** Mode C: learned Thompson Sampling policy. */
  modeC: AcceptanceModeResult;
  /** True if Mode C passed (the full learned mode). */
  allPassed: boolean;
  /** Number of witness entries in the chain. */
  witnessEntries: number;
  /** Total witness chain bytes. */
  witnessChainBytes: number;
}

/** Skip mode for the PolicyKernel. */
export type SkipMode = 'none' | 'weekday' | 'hybrid';

/** Per-arm stats from Thompson Sampling. */
export interface SkipModeStats {
  attempts: number;
  successes: number;
  totalSteps: number;
  alphaSafety: number;
  betaSafety: number;
  costEma: number;
  earlyCommitWrongs: number;
}

/** Compiled knowledge entry. */
export interface CompiledConfig {
  maxSteps: number;
  avgSteps: number;
  observations: number;
  expectedCorrect: boolean;
  hitCount: number;
  counterexampleCount: number;
  compiledSkip: SkipMode;
}

/** Full policy state from the PolicyKernel. */
export interface PolicyState {
  contextStats: Record<string, Record<string, SkipModeStats>>;
  earlyCommitPenalties: number;
  earlyCommitsTotal: number;
  earlyCommitsWrong: number;
  prepass: string;
  speculativeAttempts: number;
  speculativeArm2Wins: number;
}
