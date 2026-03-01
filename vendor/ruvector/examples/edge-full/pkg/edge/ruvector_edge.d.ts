/* tslint:disable */
/* eslint-disable */

export class WasmAdaptiveCompressor {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Update network metrics (bandwidth in Mbps, latency in ms)
   */
  updateMetrics(bandwidth_mbps: number, latency_ms: number): void;
  /**
   * Create new adaptive compressor
   */
  constructor();
  /**
   * Compress vector based on network conditions
   */
  compress(data: Float32Array): any;
  /**
   * Get current network condition
   */
  condition(): string;
}

export class WasmCrypto {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Generate a local CID for data
   */
  static generateCid(data: Uint8Array): string;
  /**
   * SHA-256 hash of string as hex
   */
  static sha256String(text: string): string;
  /**
   * SHA-256 hash as hex string
   */
  static sha256(data: Uint8Array): string;
  /**
   * Decrypt data with AES-256-GCM
   */
  static decrypt(encrypted: any, key_hex: string): Uint8Array;
  /**
   * Encrypt data with AES-256-GCM (key as hex)
   */
  static encrypt(data: Uint8Array, key_hex: string): any;
}

export class WasmHnswIndex {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create with custom parameters (m = connections per node, ef = search width)
   */
  static withParams(m: number, ef_construction: number): WasmHnswIndex;
  /**
   * Get number of vectors in index
   */
  len(): number;
  /**
   * Create new HNSW index with default parameters
   */
  constructor();
  /**
   * Insert a vector with an ID
   */
  insert(id: string, vector: Float32Array): void;
  /**
   * Search for k nearest neighbors, returns JSON array of {id, distance}
   */
  search(query: Float32Array, k: number): any;
  /**
   * Check if index is empty
   */
  isEmpty(): boolean;
}

export class WasmHybridKeyPair {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get public key bytes as hex
   */
  publicKeyHex(): string;
  /**
   * Generate new hybrid keypair
   */
  constructor();
  /**
   * Sign message with hybrid signature
   */
  sign(message: Uint8Array): string;
  /**
   * Verify hybrid signature (pubkey and signature both as JSON)
   */
  static verify(public_key_json: string, message: Uint8Array, signature_json: string): boolean;
}

export class WasmIdentity {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Sign raw bytes and return signature as hex
   */
  signBytes(data: Uint8Array): string;
  /**
   * Generate a random nonce
   */
  static generateNonce(): string;
  /**
   * Get Ed25519 public key as hex string
   */
  publicKeyHex(): string;
  /**
   * Create a signed registration for this identity
   */
  createRegistration(agent_id: string, capabilities: any): any;
  /**
   * Get X25519 public key as hex string (for key exchange)
   */
  x25519PublicKeyHex(): string;
  /**
   * Create a new identity with generated keys
   */
  constructor();
  /**
   * Sign a message and return signature as hex
   */
  sign(message: string): string;
  /**
   * Verify a signature (static method)
   */
  static verify(public_key_hex: string, message: string, signature_hex: string): boolean;
}

export class WasmQuantizer {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Binary quantize a vector (32x compression)
   */
  static binaryQuantize(vector: Float32Array): Uint8Array;
  /**
   * Scalar quantize a vector (4x compression)
   */
  static scalarQuantize(vector: Float32Array): any;
  /**
   * Compute hamming distance between binary quantized vectors
   */
  static hammingDistance(a: Uint8Array, b: Uint8Array): number;
  /**
   * Reconstruct from scalar quantized
   */
  static scalarDequantize(quantized: any): Float32Array;
}

export class WasmRaftNode {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Append entry to log (leader only), returns log index or null
   */
  appendEntry(data: Uint8Array): any;
  /**
   * Get log length
   */
  getLogLength(): number;
  /**
   * Start an election (returns vote request as JSON)
   */
  startElection(): any;
  /**
   * Get commit index
   */
  getCommitIndex(): bigint;
  /**
   * Handle a vote request (returns vote response as JSON)
   */
  handleVoteRequest(request: any): any;
  /**
   * Handle a vote response (returns true if we became leader)
   */
  handleVoteResponse(response: any): boolean;
  /**
   * Create new Raft node with cluster members
   */
  constructor(node_id: string, members: any);
  /**
   * Get current term
   */
  term(): bigint;
  /**
   * Get current state (Follower, Candidate, Leader)
   */
  state(): string;
  /**
   * Check if this node is the leader
   */
  isLeader(): boolean;
}

export class WasmSemanticMatcher {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get number of registered agents
   */
  agentCount(): number;
  /**
   * Find best matching agent for a task, returns {agentId, score} or null
   */
  matchAgent(task_description: string): any;
  /**
   * Register an agent with capability description
   */
  registerAgent(agent_id: string, capabilities: string): void;
  /**
   * Create new semantic matcher
   */
  constructor();
}

export class WasmSpikingNetwork {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Apply STDP learning rule
   */
  stdpUpdate(pre: Uint8Array, post: Uint8Array, learning_rate: number): void;
  /**
   * Create new spiking network
   */
  constructor(input_size: number, hidden_size: number, output_size: number);
  /**
   * Reset network state
   */
  reset(): void;
  /**
   * Process input spikes and return output spikes
   */
  forward(inputs: Uint8Array): Uint8Array;
}

/**
 * Initialize the WASM module (call once on load)
 */
export function init(): void;

/**
 * Get library version
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_wasmadaptivecompressor_free: (a: number, b: number) => void;
  readonly __wbg_wasmcrypto_free: (a: number, b: number) => void;
  readonly __wbg_wasmhnswindex_free: (a: number, b: number) => void;
  readonly __wbg_wasmhybridkeypair_free: (a: number, b: number) => void;
  readonly __wbg_wasmidentity_free: (a: number, b: number) => void;
  readonly __wbg_wasmquantizer_free: (a: number, b: number) => void;
  readonly __wbg_wasmraftnode_free: (a: number, b: number) => void;
  readonly __wbg_wasmsemanticmatcher_free: (a: number, b: number) => void;
  readonly __wbg_wasmspikingnetwork_free: (a: number, b: number) => void;
  readonly version: () => [number, number];
  readonly wasmadaptivecompressor_compress: (a: number, b: number, c: number) => any;
  readonly wasmadaptivecompressor_condition: (a: number) => [number, number];
  readonly wasmadaptivecompressor_new: () => number;
  readonly wasmadaptivecompressor_updateMetrics: (a: number, b: number, c: number) => void;
  readonly wasmcrypto_decrypt: (a: any, b: number, c: number) => [number, number, number, number];
  readonly wasmcrypto_encrypt: (a: number, b: number, c: number, d: number) => [number, number, number];
  readonly wasmcrypto_generateCid: (a: number, b: number) => [number, number];
  readonly wasmcrypto_sha256: (a: number, b: number) => [number, number];
  readonly wasmhnswindex_insert: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly wasmhnswindex_isEmpty: (a: number) => number;
  readonly wasmhnswindex_len: (a: number) => number;
  readonly wasmhnswindex_new: () => number;
  readonly wasmhnswindex_search: (a: number, b: number, c: number, d: number) => any;
  readonly wasmhnswindex_withParams: (a: number, b: number) => number;
  readonly wasmhybridkeypair_new: () => number;
  readonly wasmhybridkeypair_publicKeyHex: (a: number) => [number, number];
  readonly wasmhybridkeypair_sign: (a: number, b: number, c: number) => [number, number];
  readonly wasmhybridkeypair_verify: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
  readonly wasmidentity_createRegistration: (a: number, b: number, c: number, d: any) => [number, number, number];
  readonly wasmidentity_generateNonce: () => [number, number];
  readonly wasmidentity_new: () => number;
  readonly wasmidentity_publicKeyHex: (a: number) => [number, number];
  readonly wasmidentity_sign: (a: number, b: number, c: number) => [number, number];
  readonly wasmidentity_verify: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
  readonly wasmidentity_x25519PublicKeyHex: (a: number) => [number, number];
  readonly wasmquantizer_binaryQuantize: (a: number, b: number) => [number, number];
  readonly wasmquantizer_hammingDistance: (a: number, b: number, c: number, d: number) => number;
  readonly wasmquantizer_scalarDequantize: (a: any) => [number, number, number, number];
  readonly wasmquantizer_scalarQuantize: (a: number, b: number) => any;
  readonly wasmraftnode_appendEntry: (a: number, b: number, c: number) => any;
  readonly wasmraftnode_getCommitIndex: (a: number) => bigint;
  readonly wasmraftnode_getLogLength: (a: number) => number;
  readonly wasmraftnode_handleVoteRequest: (a: number, b: any) => [number, number, number];
  readonly wasmraftnode_handleVoteResponse: (a: number, b: any) => [number, number, number];
  readonly wasmraftnode_isLeader: (a: number) => number;
  readonly wasmraftnode_new: (a: number, b: number, c: any) => [number, number, number];
  readonly wasmraftnode_startElection: (a: number) => any;
  readonly wasmraftnode_state: (a: number) => [number, number];
  readonly wasmraftnode_term: (a: number) => bigint;
  readonly wasmsemanticmatcher_agentCount: (a: number) => number;
  readonly wasmsemanticmatcher_matchAgent: (a: number, b: number, c: number) => any;
  readonly wasmsemanticmatcher_new: () => number;
  readonly wasmsemanticmatcher_registerAgent: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly wasmspikingnetwork_forward: (a: number, b: number, c: number) => [number, number];
  readonly wasmspikingnetwork_new: (a: number, b: number, c: number) => number;
  readonly wasmspikingnetwork_reset: (a: number) => void;
  readonly wasmspikingnetwork_stdpUpdate: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly init: () => void;
  readonly wasmidentity_signBytes: (a: number, b: number, c: number) => [number, number];
  readonly wasmcrypto_sha256String: (a: number, b: number) => [number, number];
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __externref_table_dealloc: (a: number) => void;
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
