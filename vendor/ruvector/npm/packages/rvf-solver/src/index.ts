/**
 * @ruvector/rvf-solver — Self-learning temporal solver with AGI capabilities.
 *
 * Provides Thompson Sampling policy learning, ReasoningBank pattern
 * distillation, and SHAKE-256 tamper-evident witness chains.
 *
 * @example
 * ```ts
 * import { RvfSolver } from '@ruvector/rvf-solver';
 *
 * const solver = await RvfSolver.create();
 *
 * // Train on 100 puzzles
 * const result = solver.train({ count: 100, minDifficulty: 1, maxDifficulty: 5 });
 * console.log(`Accuracy: ${(result.accuracy * 100).toFixed(1)}%`);
 *
 * // Run full acceptance test
 * const manifest = solver.acceptance({ cycles: 3 });
 * console.log(`Mode C passed: ${manifest.allPassed}`);
 *
 * // Get policy state
 * const policy = solver.policy();
 * console.log(`Context buckets: ${Object.keys(policy?.contextStats ?? {}).length}`);
 *
 * // Get witness chain
 * const chain = solver.witnessChain();
 * console.log(`Witness chain: ${chain?.length ?? 0} bytes`);
 *
 * solver.destroy();
 * ```
 */

export { RvfSolver } from './solver';

export type {
  TrainOptions,
  TrainResult,
  AcceptanceOptions,
  AcceptanceManifest,
  AcceptanceModeResult,
  CycleMetrics,
  PolicyState,
  SkipMode,
  SkipModeStats,
  CompiledConfig,
} from './types';
