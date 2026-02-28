/* tslint:disable */
/* eslint-disable */

export class BTSPAssociativeMemory {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get memory dimensions
   */
  dimensions(): any;
  /**
   * Store key-value association in one shot
   */
  store_one_shot(key: Float32Array, value: Float32Array): void;
  /**
   * Create new associative memory
   *
   * # Arguments
   * * `input_size` - Dimension of key vectors
   * * `output_size` - Dimension of value vectors
   */
  constructor(input_size: number, output_size: number);
  /**
   * Retrieve value from key
   */
  retrieve(query: Float32Array): Float32Array;
}

export class BTSPLayer {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get weights as Float32Array
   */
  get_weights(): Float32Array;
  /**
   * One-shot association: learn pattern -> target in single step
   *
   * This is the key BTSP capability: immediate learning without iteration.
   * Uses gradient normalization for single-step convergence.
   */
  one_shot_associate(pattern: Float32Array, target: number): void;
  /**
   * Create a new BTSP layer
   *
   * # Arguments
   * * `size` - Number of synapses (input dimension)
   * * `tau` - Time constant in milliseconds (2000ms default)
   */
  constructor(size: number, tau: number);
  /**
   * Reset layer to initial state
   */
  reset(): void;
  /**
   * Forward pass: compute layer output
   */
  forward(input: Float32Array): number;
  /**
   * Get number of synapses
   */
  readonly size: number;
}

export class BTSPSynapse {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new BTSP synapse
   *
   * # Arguments
   * * `initial_weight` - Starting weight (0.0 to 1.0)
   * * `tau_btsp` - Time constant in milliseconds (1000-3000ms recommended)
   */
  constructor(initial_weight: number, tau_btsp: number);
  /**
   * Update synapse based on activity and plateau signal
   *
   * # Arguments
   * * `presynaptic_active` - Is presynaptic neuron firing?
   * * `plateau_signal` - Dendritic plateau potential detected?
   * * `dt` - Time step in milliseconds
   */
  update(presynaptic_active: boolean, plateau_signal: boolean, dt: number): void;
  /**
   * Compute synaptic output
   */
  forward(input: number): number;
  /**
   * Get eligibility trace
   */
  readonly eligibility_trace: number;
  /**
   * Get current weight
   */
  readonly weight: number;
}

export class GlobalWorkspace {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get current load (0.0 to 1.0)
   */
  current_load(): number;
  /**
   * Get most salient item
   */
  most_salient(): WorkspaceItem | undefined;
  /**
   * Retrieve top-k most salient representations
   */
  retrieve_top_k(k: number): any;
  /**
   * Set salience decay rate
   */
  set_decay_rate(decay: number): void;
  /**
   * Create with custom threshold
   */
  static with_threshold(capacity: number, threshold: number): GlobalWorkspace;
  /**
   * Get available slots
   */
  available_slots(): number;
  /**
   * Get average salience
   */
  average_salience(): number;
  /**
   * Create a new global workspace
   *
   * # Arguments
   * * `capacity` - Maximum number of representations (typically 4-7)
   */
  constructor(capacity: number);
  /**
   * Clear all representations
   */
  clear(): void;
  /**
   * Run competitive dynamics (salience decay and pruning)
   */
  compete(): void;
  /**
   * Check if workspace is at capacity
   */
  is_full(): boolean;
  /**
   * Check if workspace is empty
   */
  is_empty(): boolean;
  /**
   * Retrieve all current representations as JSON
   */
  retrieve(): any;
  /**
   * Broadcast a representation to the workspace
   *
   * Returns true if accepted, false if rejected.
   */
  broadcast(item: WorkspaceItem): boolean;
  /**
   * Get current number of representations
   */
  readonly len: number;
  /**
   * Get workspace capacity
   */
  readonly capacity: number;
}

export class HdcMemory {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get a vector by label
   */
  get(label: string): Hypervector | undefined;
  /**
   * Check if a label exists
   */
  has(label: string): boolean;
  /**
   * Create a new empty HDC memory
   */
  constructor();
  /**
   * Clear all stored vectors
   */
  clear(): void;
  /**
   * Store a hypervector with a label
   */
  store(label: string, vector: Hypervector): void;
  /**
   * Find the k most similar vectors to query
   */
  top_k(query: Hypervector, k: number): any;
  /**
   * Retrieve vectors similar to query above threshold
   *
   * Returns array of [label, similarity] pairs
   */
  retrieve(query: Hypervector, threshold: number): any;
  /**
   * Get number of stored vectors
   */
  readonly size: number;
}

export class Hypervector {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create from raw bytes
   */
  static from_bytes(bytes: Uint8Array): Hypervector;
  /**
   * Compute similarity between two hypervectors
   *
   * Returns a value in [-1.0, 1.0] where:
   * - 1.0 = identical vectors
   * - 0.0 = random/orthogonal vectors
   * - -1.0 = completely opposite vectors
   */
  similarity(other: Hypervector): number;
  /**
   * Compute Hamming distance (number of differing bits)
   */
  hamming_distance(other: Hypervector): number;
  /**
   * Create a zero hypervector
   */
  constructor();
  /**
   * Bind two hypervectors using XOR
   *
   * Binding is associative, commutative, and self-inverse:
   * - a.bind(b) == b.bind(a)
   * - a.bind(b).bind(b) == a
   */
  bind(other: Hypervector): Hypervector;
  /**
   * Create a random hypervector with ~50% bits set
   */
  static random(): Hypervector;
  /**
   * Bundle multiple vectors by majority voting on each bit
   */
  static bundle_3(a: Hypervector, b: Hypervector, c: Hypervector): Hypervector;
  /**
   * Count the number of set bits (population count)
   */
  popcount(): number;
  /**
   * Get the raw bits as Uint8Array (for serialization)
   */
  to_bytes(): Uint8Array;
  /**
   * Create a hypervector from a seed for reproducibility
   */
  static from_seed(seed: bigint): Hypervector;
  /**
   * Get number of bits
   */
  readonly dimension: number;
}

export class KWTALayer {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Set activation threshold
   */
  with_threshold(threshold: number): void;
  /**
   * Select top-k neurons with their activation values
   *
   * Returns array of [index, value] pairs.
   */
  select_with_values(inputs: Float32Array): any;
  /**
   * Create sparse activation vector (only top-k preserved)
   */
  sparse_activations(inputs: Float32Array): Float32Array;
  /**
   * Create a new K-WTA layer
   *
   * # Arguments
   * * `size` - Total number of neurons
   * * `k` - Number of winners to select
   */
  constructor(size: number, k: number);
  /**
   * Select top-k neurons
   *
   * Returns indices of k neurons with highest activations, sorted descending.
   */
  select(inputs: Float32Array): Uint32Array;
  /**
   * Get number of winners
   */
  readonly k: number;
  /**
   * Get layer size
   */
  readonly size: number;
}

export class WTALayer {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Soft competition with normalized activations
   *
   * Returns activation levels for all neurons after softmax-like normalization.
   */
  compete_soft(inputs: Float32Array): Float32Array;
  /**
   * Get current membrane potentials
   */
  get_membranes(): Float32Array;
  /**
   * Set refractory period
   */
  set_refractory_period(period: number): void;
  /**
   * Create a new WTA layer
   *
   * # Arguments
   * * `size` - Number of competing neurons
   * * `threshold` - Activation threshold for firing
   * * `inhibition` - Lateral inhibition strength (0.0-1.0)
   */
  constructor(size: number, threshold: number, inhibition: number);
  /**
   * Reset layer state
   */
  reset(): void;
  /**
   * Run winner-take-all competition
   *
   * Returns the index of the winning neuron, or -1 if no neuron exceeds threshold.
   */
  compete(inputs: Float32Array): number;
  /**
   * Get layer size
   */
  readonly size: number;
}

export class WorkspaceItem {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Check if expired
   */
  is_expired(current_time: bigint): boolean;
  /**
   * Create with custom decay and lifetime
   */
  static with_decay(content: Float32Array, salience: number, source_module: number, timestamp: bigint, decay_rate: number, lifetime: bigint): WorkspaceItem;
  /**
   * Apply temporal decay
   */
  apply_decay(dt: number): void;
  /**
   * Get content as Float32Array
   */
  get_content(): Float32Array;
  /**
   * Update salience
   */
  update_salience(new_salience: number): void;
  /**
   * Create a new workspace item
   */
  constructor(content: Float32Array, salience: number, source_module: number, timestamp: bigint);
  /**
   * Compute content magnitude (L2 norm)
   */
  magnitude(): number;
  /**
   * Get source module
   */
  readonly source_module: number;
  /**
   * Get ID
   */
  readonly id: bigint;
  /**
   * Get salience
   */
  readonly salience: number;
  /**
   * Get timestamp
   */
  readonly timestamp: bigint;
}

/**
 * Get information about available bio-inspired mechanisms
 */
export function available_mechanisms(): any;

/**
 * Get biological references for the mechanisms
 */
export function biological_references(): any;

/**
 * Initialize the WASM module with panic hook
 */
export function init(): void;

/**
 * Get performance targets for each mechanism
 */
export function performance_targets(): any;

/**
 * Get the version of the crate
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_btspassociativememory_free: (a: number, b: number) => void;
  readonly __wbg_btsplayer_free: (a: number, b: number) => void;
  readonly __wbg_btspsynapse_free: (a: number, b: number) => void;
  readonly __wbg_globalworkspace_free: (a: number, b: number) => void;
  readonly __wbg_hdcmemory_free: (a: number, b: number) => void;
  readonly __wbg_hypervector_free: (a: number, b: number) => void;
  readonly __wbg_kwtalayer_free: (a: number, b: number) => void;
  readonly __wbg_workspaceitem_free: (a: number, b: number) => void;
  readonly __wbg_wtalayer_free: (a: number, b: number) => void;
  readonly available_mechanisms: () => number;
  readonly biological_references: () => number;
  readonly btspassociativememory_dimensions: (a: number) => number;
  readonly btspassociativememory_new: (a: number, b: number) => number;
  readonly btspassociativememory_retrieve: (a: number, b: number, c: number, d: number) => void;
  readonly btspassociativememory_store_one_shot: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly btsplayer_forward: (a: number, b: number, c: number, d: number) => void;
  readonly btsplayer_get_weights: (a: number) => number;
  readonly btsplayer_new: (a: number, b: number) => number;
  readonly btsplayer_one_shot_associate: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly btsplayer_reset: (a: number) => void;
  readonly btsplayer_size: (a: number) => number;
  readonly btspsynapse_eligibility_trace: (a: number) => number;
  readonly btspsynapse_forward: (a: number, b: number) => number;
  readonly btspsynapse_new: (a: number, b: number, c: number) => void;
  readonly btspsynapse_update: (a: number, b: number, c: number, d: number) => void;
  readonly btspsynapse_weight: (a: number) => number;
  readonly globalworkspace_available_slots: (a: number) => number;
  readonly globalworkspace_average_salience: (a: number) => number;
  readonly globalworkspace_broadcast: (a: number, b: number) => number;
  readonly globalworkspace_capacity: (a: number) => number;
  readonly globalworkspace_clear: (a: number) => void;
  readonly globalworkspace_compete: (a: number) => void;
  readonly globalworkspace_current_load: (a: number) => number;
  readonly globalworkspace_is_empty: (a: number) => number;
  readonly globalworkspace_is_full: (a: number) => number;
  readonly globalworkspace_len: (a: number) => number;
  readonly globalworkspace_most_salient: (a: number) => number;
  readonly globalworkspace_new: (a: number) => number;
  readonly globalworkspace_retrieve: (a: number) => number;
  readonly globalworkspace_retrieve_top_k: (a: number, b: number) => number;
  readonly globalworkspace_set_decay_rate: (a: number, b: number) => void;
  readonly globalworkspace_with_threshold: (a: number, b: number) => number;
  readonly hdcmemory_clear: (a: number) => void;
  readonly hdcmemory_get: (a: number, b: number, c: number) => number;
  readonly hdcmemory_has: (a: number, b: number, c: number) => number;
  readonly hdcmemory_new: () => number;
  readonly hdcmemory_retrieve: (a: number, b: number, c: number) => number;
  readonly hdcmemory_size: (a: number) => number;
  readonly hdcmemory_store: (a: number, b: number, c: number, d: number) => void;
  readonly hdcmemory_top_k: (a: number, b: number, c: number) => number;
  readonly hypervector_bind: (a: number, b: number) => number;
  readonly hypervector_bundle_3: (a: number, b: number, c: number) => number;
  readonly hypervector_dimension: (a: number) => number;
  readonly hypervector_from_bytes: (a: number, b: number, c: number) => void;
  readonly hypervector_from_seed: (a: bigint) => number;
  readonly hypervector_hamming_distance: (a: number, b: number) => number;
  readonly hypervector_new: () => number;
  readonly hypervector_popcount: (a: number) => number;
  readonly hypervector_random: () => number;
  readonly hypervector_similarity: (a: number, b: number) => number;
  readonly hypervector_to_bytes: (a: number) => number;
  readonly kwtalayer_k: (a: number) => number;
  readonly kwtalayer_new: (a: number, b: number, c: number) => void;
  readonly kwtalayer_select: (a: number, b: number, c: number, d: number) => void;
  readonly kwtalayer_select_with_values: (a: number, b: number, c: number, d: number) => void;
  readonly kwtalayer_size: (a: number) => number;
  readonly kwtalayer_sparse_activations: (a: number, b: number, c: number, d: number) => void;
  readonly kwtalayer_with_threshold: (a: number, b: number) => void;
  readonly performance_targets: () => number;
  readonly version: (a: number) => void;
  readonly workspaceitem_apply_decay: (a: number, b: number) => void;
  readonly workspaceitem_get_content: (a: number) => number;
  readonly workspaceitem_id: (a: number) => bigint;
  readonly workspaceitem_is_expired: (a: number, b: bigint) => number;
  readonly workspaceitem_magnitude: (a: number) => number;
  readonly workspaceitem_new: (a: number, b: number, c: number, d: number, e: bigint) => number;
  readonly workspaceitem_salience: (a: number) => number;
  readonly workspaceitem_source_module: (a: number) => number;
  readonly workspaceitem_timestamp: (a: number) => bigint;
  readonly workspaceitem_update_salience: (a: number, b: number) => void;
  readonly workspaceitem_with_decay: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => number;
  readonly wtalayer_compete: (a: number, b: number, c: number, d: number) => void;
  readonly wtalayer_compete_soft: (a: number, b: number, c: number, d: number) => void;
  readonly wtalayer_get_membranes: (a: number) => number;
  readonly wtalayer_new: (a: number, b: number, c: number, d: number) => void;
  readonly wtalayer_reset: (a: number) => void;
  readonly wtalayer_set_refractory_period: (a: number, b: number) => void;
  readonly init: () => void;
  readonly wtalayer_size: (a: number) => number;
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
