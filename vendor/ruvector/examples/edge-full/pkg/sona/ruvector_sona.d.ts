/* tslint:disable */
/* eslint-disable */

export class WasmEphemeralAgent {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Force learning cycle on agent's engine
   */
  forceLearn(): string;
  /**
   * Create agent with custom configuration
   *
   * # Arguments
   * * `agent_id` - Unique identifier
   * * `config` - JSON configuration object
   *
   * # Example
   * ```javascript
   * const config = {
   *   hidden_dim: 256,
   *   trajectory_capacity: 500,
   *   pattern_clusters: 25
   * };
   * const agent = WasmEphemeralAgent.with_config("agent-1", config);
   * ```
   */
  static withConfig(agent_id: string, config: any): WasmEphemeralAgent;
  /**
   * Export agent state for coordinator aggregation
   *
   * # Returns
   * JSON object containing agent state, trajectories, and statistics
   *
   * # Example
   * ```javascript
   * const state = agent.export_state();
   * console.log('Trajectories:', state.trajectories.length);
   * coordinator.aggregate(state);
   * ```
   */
  exportState(): any;
  /**
   * Get learned patterns from agent
   */
  getPatterns(): any;
  /**
   * Process a task and record trajectory
   *
   * # Arguments
   * * `embedding` - Query embedding as Float32Array
   * * `quality` - Task quality score [0.0, 1.0]
   *
   * # Example
   * ```javascript
   * const embedding = new Float32Array(256).fill(0.1);
   * agent.process_task(embedding, 0.85);
   * ```
   */
  processTask(embedding: Float32Array, quality: number): void;
  /**
   * Get agent uptime in seconds
   */
  uptimeSeconds(): bigint;
  /**
   * Get average quality of collected trajectories
   */
  averageQuality(): number;
  /**
   * Get number of collected trajectories
   */
  trajectoryCount(): number;
  /**
   * Process task with model route information
   *
   * # Arguments
   * * `embedding` - Query embedding
   * * `quality` - Quality score
   * * `route` - Model route used (e.g., "gpt-4", "claude-3")
   */
  processTaskWithRoute(embedding: Float32Array, quality: number, route: string): void;
  /**
   * Create a new ephemeral agent with default config
   *
   * # Arguments
   * * `agent_id` - Unique identifier for this agent
   *
   * # Example
   * ```javascript
   * const agent = new WasmEphemeralAgent("agent-1");
   * ```
   */
  constructor(agent_id: string);
  /**
   * Clear collected trajectories (after export)
   */
  clear(): void;
  /**
   * Get agent statistics
   *
   * # Returns
   * JSON object with trajectory count, quality stats, uptime
   */
  getStats(): any;
}

export class WasmFederatedCoordinator {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Apply coordinator's learned LoRA to input
   */
  applyLora(input: Float32Array): Float32Array;
  /**
   * Get total number of contributing agents
   */
  agentCount(): number;
  /**
   * Consolidate learning from all aggregated trajectories
   *
   * Should be called periodically after aggregating multiple agents.
   *
   * # Returns
   * Learning result as JSON string
   */
  consolidate(): string;
  /**
   * Create coordinator with custom configuration
   *
   * # Arguments
   * * `coordinator_id` - Unique identifier
   * * `config` - JSON configuration object
   *
   * # Example
   * ```javascript
   * const config = {
   *   hidden_dim: 256,
   *   trajectory_capacity: 50000,
   *   pattern_clusters: 200,
   *   ewc_lambda: 2000.0
   * };
   * const coordinator = WasmFederatedCoordinator.with_config("central", config);
   * ```
   */
  static withConfig(coordinator_id: string, config: any): WasmFederatedCoordinator;
  /**
   * Get all learned patterns from coordinator
   */
  getPatterns(): any;
  /**
   * Find similar patterns to query
   *
   * # Arguments
   * * `query_embedding` - Query vector
   * * `k` - Number of patterns to return
   */
  findPatterns(query_embedding: Float32Array, k: number): any;
  /**
   * Get total trajectories aggregated
   */
  totalTrajectories(): number;
  /**
   * Set quality threshold for accepting trajectories
   *
   * # Arguments
   * * `threshold` - Minimum quality [0.0, 1.0], default 0.4
   */
  setQualityThreshold(threshold: number): void;
  /**
   * Create a new federated coordinator with default config
   *
   * # Arguments
   * * `coordinator_id` - Unique identifier for this coordinator
   *
   * # Example
   * ```javascript
   * const coordinator = new WasmFederatedCoordinator("central");
   * ```
   */
  constructor(coordinator_id: string);
  /**
   * Clear all agent contributions (reset coordinator)
   */
  clear(): void;
  /**
   * Aggregate agent export into coordinator
   *
   * # Arguments
   * * `agent_export` - JSON export from agent.export_state()
   *
   * # Returns
   * JSON aggregation result with accepted/rejected counts
   *
   * # Example
   * ```javascript
   * const agentState = agent.export_state();
   * const result = coordinator.aggregate(agentState);
   * console.log('Accepted:', result.accepted);
   * ```
   */
  aggregate(agent_export: any): any;
  /**
   * Get coordinator statistics
   *
   * # Returns
   * JSON object with agent count, trajectory count, quality stats
   */
  getStats(): any;
}

export class WasmSonaEngine {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Apply LoRA transformation to input vector
   *
   * # Arguments
   * * `input` - Input vector as Float32Array
   *
   * # Returns
   * Transformed vector as Float32Array
   *
   * # Example
   * ```javascript
   * const input = new Float32Array(256).fill(1.0);
   * const output = engine.apply_lora(input);
   * ```
   */
  applyLora(input: Float32Array): Float32Array;
  /**
   * Get configuration
   *
   * # Returns
   * Configuration as JSON object
   */
  getConfig(): any;
  /**
   * Check if engine is enabled
   *
   * # Returns
   * true if enabled, false otherwise
   */
  isEnabled(): boolean;
  /**
   * Force background learning cycle
   *
   * # Returns
   * Learning statistics as JSON string
   *
   * # Example
   * ```javascript
   * const stats = engine.force_learn();
   * console.log('Learning results:', stats);
   * ```
   */
  forceLearn(): string;
  /**
   * Record a step in the trajectory
   *
   * # Arguments
   * * `trajectory_id` - ID returned from start_trajectory
   * * `node_id` - Graph node visited
   * * `score` - Step quality score [0.0, 1.0]
   * * `latency_us` - Step latency in microseconds
   *
   * # Example
   * ```javascript
   * engine.record_step(trajectoryId, 42, 0.8, 1000);
   * ```
   */
  recordStep(trajectory_id: bigint, node_id: number, score: number, latency_us: bigint): void;
  /**
   * Enable or disable the engine
   *
   * # Arguments
   * * `enabled` - Whether to enable the engine
   *
   * # Example
   * ```javascript
   * engine.set_enabled(false); // Pause learning
   * ```
   */
  setEnabled(enabled: boolean): void;
  /**
   * Create engine with custom configuration
   *
   * # Arguments
   * * `config` - JSON configuration object
   *
   * # Example
   * ```javascript
   * const config = {
   *   hidden_dim: 256,
   *   embedding_dim: 256,
   *   micro_lora_rank: 2,
   *   base_lora_rank: 16,
   *   micro_lora_lr: 0.001,
   *   base_lora_lr: 0.0001,
   *   ewc_lambda: 1000.0,
   *   pattern_clusters: 128,
   *   trajectory_capacity: 10000,
   *   quality_threshold: 0.6
   * };
   * const engine = WasmSonaEngine.with_config(config);
   * ```
   */
  static withConfig(config: any): WasmSonaEngine;
  /**
   * Find similar patterns to query
   *
   * # Arguments
   * * `query_embedding` - Query vector as Float32Array
   * * `k` - Number of patterns to return
   *
   * # Returns
   * Array of similar patterns as JSON
   *
   * # Example
   * ```javascript
   * const query = new Float32Array(256).fill(0.5);
   * const patterns = engine.find_patterns(query, 5);
   * console.log('Similar patterns:', patterns);
   * ```
   */
  findPatterns(query_embedding: Float32Array, k: number): any;
  /**
   * End the trajectory and submit for learning
   *
   * # Arguments
   * * `trajectory_id` - ID returned from start_trajectory
   * * `final_score` - Overall trajectory quality [0.0, 1.0]
   *
   * # Example
   * ```javascript
   * engine.end_trajectory(trajectoryId, 0.85);
   * ```
   */
  endTrajectory(trajectory_id: bigint, final_score: number): void;
  /**
   * Apply LoRA transformation to specific layer
   *
   * # Arguments
   * * `layer_idx` - Layer index
   * * `input` - Input vector as Float32Array
   *
   * # Returns
   * Transformed vector as Float32Array
   */
  applyLoraLayer(layer_idx: number, input: Float32Array): Float32Array;
  /**
   * Start recording a new trajectory
   *
   * # Arguments
   * * `query_embedding` - Query vector as Float32Array
   *
   * # Returns
   * Trajectory ID (u64)
   *
   * # Example
   * ```javascript
   * const embedding = new Float32Array(256).fill(0.1);
   * const trajectoryId = engine.start_trajectory(embedding);
   * ```
   */
  startTrajectory(query_embedding: Float32Array): bigint;
  /**
   * Run instant learning cycle
   *
   * Flushes accumulated micro-LoRA updates
   *
   * # Example
   * ```javascript
   * engine.run_instant_cycle();
   * ```
   */
  runInstantCycle(): void;
  /**
   * Apply learning from user feedback
   *
   * # Arguments
   * * `success` - Whether the operation succeeded
   * * `latency_ms` - Operation latency in milliseconds
   * * `quality` - User-perceived quality [0.0, 1.0]
   *
   * # Example
   * ```javascript
   * engine.learn_from_feedback(true, 50.0, 0.9);
   * ```
   */
  learnFromFeedback(success: boolean, latency_ms: number, quality: number): void;
  /**
   * Create a new SONA engine with specified hidden dimension
   *
   * # Arguments
   * * `hidden_dim` - Size of hidden layer (typically 256, 512, or 1024)
   *
   * # Example
   * ```javascript
   * const engine = new WasmSonaEngine(256);
   * ```
   */
  constructor(hidden_dim: number);
  /**
   * Try to run background learning cycle
   *
   * Returns true if cycle was executed, false if not due yet
   *
   * # Example
   * ```javascript
   * if (engine.tick()) {
   *   console.log('Background learning completed');
   * }
   * ```
   */
  tick(): boolean;
  /**
   * Get engine statistics
   *
   * # Returns
   * Statistics as JSON object
   *
   * # Example
   * ```javascript
   * const stats = engine.get_stats();
   * console.log('Trajectories buffered:', stats.trajectories_buffered);
   * console.log('Patterns learned:', stats.patterns_learned);
   * ```
   */
  getStats(): any;
}

/**
 * Initialize WASM module (called automatically)
 */
export function wasm_init(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_wasmephemeralagent_free: (a: number, b: number) => void;
  readonly __wbg_wasmfederatedcoordinator_free: (a: number, b: number) => void;
  readonly __wbg_wasmsonaengine_free: (a: number, b: number) => void;
  readonly wasmephemeralagent_averageQuality: (a: number) => number;
  readonly wasmephemeralagent_clear: (a: number) => void;
  readonly wasmephemeralagent_exportState: (a: number) => number;
  readonly wasmephemeralagent_forceLearn: (a: number, b: number) => void;
  readonly wasmephemeralagent_getPatterns: (a: number) => number;
  readonly wasmephemeralagent_getStats: (a: number) => number;
  readonly wasmephemeralagent_new: (a: number, b: number, c: number) => void;
  readonly wasmephemeralagent_processTask: (a: number, b: number, c: number, d: number) => void;
  readonly wasmephemeralagent_processTaskWithRoute: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly wasmephemeralagent_trajectoryCount: (a: number) => number;
  readonly wasmephemeralagent_uptimeSeconds: (a: number) => bigint;
  readonly wasmephemeralagent_withConfig: (a: number, b: number, c: number, d: number) => void;
  readonly wasmfederatedcoordinator_agentCount: (a: number) => number;
  readonly wasmfederatedcoordinator_aggregate: (a: number, b: number) => number;
  readonly wasmfederatedcoordinator_applyLora: (a: number, b: number, c: number, d: number) => void;
  readonly wasmfederatedcoordinator_clear: (a: number) => void;
  readonly wasmfederatedcoordinator_consolidate: (a: number, b: number) => void;
  readonly wasmfederatedcoordinator_findPatterns: (a: number, b: number, c: number, d: number) => number;
  readonly wasmfederatedcoordinator_getPatterns: (a: number) => number;
  readonly wasmfederatedcoordinator_getStats: (a: number) => number;
  readonly wasmfederatedcoordinator_new: (a: number, b: number, c: number) => void;
  readonly wasmfederatedcoordinator_setQualityThreshold: (a: number, b: number) => void;
  readonly wasmfederatedcoordinator_totalTrajectories: (a: number) => number;
  readonly wasmfederatedcoordinator_withConfig: (a: number, b: number, c: number, d: number) => void;
  readonly wasmsonaengine_applyLora: (a: number, b: number, c: number, d: number) => void;
  readonly wasmsonaengine_applyLoraLayer: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly wasmsonaengine_endTrajectory: (a: number, b: bigint, c: number) => void;
  readonly wasmsonaengine_findPatterns: (a: number, b: number, c: number, d: number) => number;
  readonly wasmsonaengine_forceLearn: (a: number, b: number) => void;
  readonly wasmsonaengine_getConfig: (a: number) => number;
  readonly wasmsonaengine_getStats: (a: number) => number;
  readonly wasmsonaengine_isEnabled: (a: number) => number;
  readonly wasmsonaengine_learnFromFeedback: (a: number, b: number, c: number, d: number) => void;
  readonly wasmsonaengine_new: (a: number, b: number) => void;
  readonly wasmsonaengine_recordStep: (a: number, b: bigint, c: number, d: number, e: bigint) => void;
  readonly wasmsonaengine_runInstantCycle: (a: number) => void;
  readonly wasmsonaengine_setEnabled: (a: number, b: number) => void;
  readonly wasmsonaengine_startTrajectory: (a: number, b: number, c: number) => bigint;
  readonly wasmsonaengine_tick: (a: number) => number;
  readonly wasmsonaengine_withConfig: (a: number, b: number) => void;
  readonly wasm_init: () => void;
  readonly __wbindgen_export: (a: number, b: number) => number;
  readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_export3: (a: number) => void;
  readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
