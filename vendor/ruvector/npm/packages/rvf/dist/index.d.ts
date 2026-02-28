/**
 * @ruvector/rvf — Unified TypeScript SDK for the RuVector Format.
 *
 * Works with both the native Node.js backend (`@ruvector/rvf-node`) and
 * the browser WASM backend (`@ruvector/rvf-wasm`).
 *
 * @example
 * ```ts
 * import { RvfDatabase } from '@ruvector/rvf';
 *
 * const db = await RvfDatabase.create('./my.rvf', { dimensions: 128 });
 * await db.ingestBatch([
 *   { id: '1', vector: new Float32Array(128) },
 * ]);
 * const results = await db.query(new Float32Array(128), 10);
 * await db.close();
 * ```
 */
export type { DistanceMetric, CompressionProfile, HardwareProfile, RvfOptions, RvfFilterValue, RvfFilterExpr, RvfQueryOptions, RvfSearchResult, RvfIngestResult, RvfIngestEntry, RvfDeleteResult, RvfCompactionResult, CompactionState, RvfStatus, DerivationType, RvfKernelData, RvfEbpfData, RvfSegmentInfo, BackendType, RvfIndexStats, RvfWitnessResult, } from './types';
export { RvfError, RvfErrorCode } from './errors';
export type { RvfBackend } from './backend';
export { NodeBackend, WasmBackend, resolveBackend } from './backend';
export { RvfDatabase } from './database';
export { RvfSolver } from '@ruvector/rvf-solver';
export type { TrainOptions, TrainResult, AcceptanceOptions, AcceptanceManifest, AcceptanceModeResult, CycleMetrics, PolicyState, SkipMode, SkipModeStats, CompiledConfig, } from '@ruvector/rvf-solver';
