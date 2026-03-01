/* tslint:disable */
/* eslint-disable */

export class AsyncQueryExecutor {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Execute query in a Web Worker for background processing
   */
  executeInWorker(_query: string): Promise<any>;
  /**
   * Execute query asynchronously with streaming results
   * This is useful for large result sets
   */
  executeStreaming(_query: string): Promise<any>;
  /**
   * Create a new async query executor
   */
  constructor(batch_size?: number | null);
  /**
   * Get batch size
   */
  batchSize: number;
}

export class AsyncTransaction {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Add operation to transaction
   */
  addOperation(operation: string): void;
  /**
   * Create a new transaction
   */
  constructor();
  /**
   * Commit transaction asynchronously
   */
  commit(): Promise<any>;
  /**
   * Rollback transaction
   */
  rollback(): void;
  /**
   * Check if committed
   */
  readonly isCommitted: boolean;
  /**
   * Get operation count
   */
  readonly operationCount: number;
}

export class BatchOperations {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Execute multiple Cypher statements in batch
   */
  executeBatch(statements: string[]): Promise<any>;
  /**
   * Create a new batch operations handler
   */
  constructor(max_batch_size?: number | null);
  /**
   * Get max batch size
   */
  readonly maxBatchSize: number;
}

export class GraphDB {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new edge (relationship)
   *
   * # Arguments
   * * `from` - Source node ID
   * * `to` - Target node ID
   * * `edge_type` - Relationship type
   * * `properties` - JavaScript object with edge properties
   *
   * # Returns
   * Edge ID
   */
  createEdge(from: string, to: string, edge_type: string, properties: any): string;
  /**
   * Create a new node
   *
   * # Arguments
   * * `labels` - Array of label strings
   * * `properties` - JavaScript object with node properties
   *
   * # Returns
   * Node ID
   */
  createNode(labels: string[], properties: any): string;
  /**
   * Delete an edge by ID
   */
  deleteEdge(id: string): boolean;
  /**
   * Delete a node by ID
   *
   * # Arguments
   * * `id` - Node ID
   *
   * # Returns
   * True if deleted, false if not found
   */
  deleteNode(id: string): boolean;
  /**
   * Export database as Cypher CREATE statements
   *
   * # Returns
   * String containing Cypher statements
   */
  exportCypher(): string;
  /**
   * Get a hyperedge by ID
   */
  getHyperedge(id: string): JsHyperedge | undefined;
  /**
   * Import Cypher statements
   *
   * # Arguments
   * * `statements` - Array of Cypher CREATE statements
   *
   * # Returns
   * Number of statements executed
   */
  importCypher(statements: string[]): Promise<number>;
  /**
   * Create a hyperedge (n-ary relationship)
   *
   * # Arguments
   * * `nodes` - Array of node IDs
   * * `description` - Natural language description of the relationship
   * * `embedding` - Optional embedding vector (auto-generated if not provided)
   * * `confidence` - Optional confidence score (0.0-1.0, defaults to 1.0)
   *
   * # Returns
   * Hyperedge ID
   */
  createHyperedge(nodes: string[], description: string, embedding?: Float32Array | null, confidence?: number | null): string;
  /**
   * Create a new GraphDB instance
   *
   * # Arguments
   * * `metric` - Distance metric for hypergraph embeddings ("euclidean", "cosine", "dotproduct", "manhattan")
   */
  constructor(metric?: string | null);
  /**
   * Execute a Cypher query (basic implementation)
   *
   * # Arguments
   * * `cypher` - Cypher query string
   *
   * # Returns
   * Promise<QueryResult> with matching nodes, edges, and hyperedges
   */
  query(cypher: string): Promise<QueryResult>;
  /**
   * Get database statistics
   */
  stats(): any;
  /**
   * Get an edge by ID
   */
  getEdge(id: string): JsEdge | undefined;
  /**
   * Get a node by ID
   *
   * # Arguments
   * * `id` - Node ID
   *
   * # Returns
   * JsNode or null if not found
   */
  getNode(id: string): JsNode | undefined;
}

export class JsEdge {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  getProperty(key: string): any;
  readonly properties: any;
  readonly id: string;
  readonly to: string;
  readonly from: string;
  readonly type: string;
}

export class JsHyperedge {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  readonly confidence: number;
  readonly properties: any;
  readonly description: string;
  readonly id: string;
  readonly nodes: string[];
  readonly order: number;
  readonly embedding: Float32Array;
}

export class JsNode {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get a specific property value
   */
  getProperty(key: string): any;
  /**
   * Check if node has a specific label
   */
  hasLabel(label: string): boolean;
  readonly properties: any;
  readonly id: string;
  readonly labels: string[];
  readonly embedding: Float32Array | undefined;
}

export class QueryResult {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Check if result is empty
   */
  isEmpty(): boolean;
  readonly hyperedges: JsHyperedge[];
  readonly data: any;
  readonly count: number;
  readonly edges: JsEdge[];
  readonly nodes: JsNode[];
}

export class ResultStream {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get next chunk of results
   */
  nextChunk(): Promise<any>;
  /**
   * Create a new result stream
   */
  constructor(chunk_size?: number | null);
  /**
   * Reset stream to beginning
   */
  reset(): void;
  /**
   * Get chunk size
   */
  readonly chunkSize: number;
  /**
   * Get current offset
   */
  readonly offset: number;
}

/**
 * Initialize panic hook for better error messages
 */
export function init(): void;

/**
 * Get version information
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_asyncqueryexecutor_free: (a: number, b: number) => void;
  readonly __wbg_asynctransaction_free: (a: number, b: number) => void;
  readonly __wbg_graphdb_free: (a: number, b: number) => void;
  readonly __wbg_jsedge_free: (a: number, b: number) => void;
  readonly __wbg_jshyperedge_free: (a: number, b: number) => void;
  readonly __wbg_jsnode_free: (a: number, b: number) => void;
  readonly __wbg_queryresult_free: (a: number, b: number) => void;
  readonly __wbg_resultstream_free: (a: number, b: number) => void;
  readonly asyncqueryexecutor_batchSize: (a: number) => number;
  readonly asyncqueryexecutor_executeInWorker: (a: number, b: number, c: number) => number;
  readonly asyncqueryexecutor_executeStreaming: (a: number, b: number, c: number) => number;
  readonly asyncqueryexecutor_new: (a: number) => number;
  readonly asyncqueryexecutor_set_batchSize: (a: number, b: number) => void;
  readonly asynctransaction_addOperation: (a: number, b: number, c: number) => void;
  readonly asynctransaction_commit: (a: number) => number;
  readonly asynctransaction_isCommitted: (a: number) => number;
  readonly asynctransaction_new: () => number;
  readonly asynctransaction_operationCount: (a: number) => number;
  readonly asynctransaction_rollback: (a: number) => void;
  readonly batchoperations_executeBatch: (a: number, b: number, c: number) => number;
  readonly batchoperations_new: (a: number) => number;
  readonly graphdb_createEdge: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
  readonly graphdb_createHyperedge: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
  readonly graphdb_createNode: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly graphdb_deleteEdge: (a: number, b: number, c: number) => number;
  readonly graphdb_deleteNode: (a: number, b: number, c: number) => number;
  readonly graphdb_exportCypher: (a: number, b: number) => void;
  readonly graphdb_getEdge: (a: number, b: number, c: number) => number;
  readonly graphdb_getHyperedge: (a: number, b: number, c: number) => number;
  readonly graphdb_getNode: (a: number, b: number, c: number) => number;
  readonly graphdb_importCypher: (a: number, b: number, c: number) => number;
  readonly graphdb_new: (a: number, b: number, c: number) => void;
  readonly graphdb_query: (a: number, b: number, c: number) => number;
  readonly graphdb_stats: (a: number) => number;
  readonly jsedge_from: (a: number, b: number) => void;
  readonly jsedge_getProperty: (a: number, b: number, c: number) => number;
  readonly jsedge_id: (a: number, b: number) => void;
  readonly jsedge_properties: (a: number) => number;
  readonly jsedge_to: (a: number, b: number) => void;
  readonly jsedge_type: (a: number, b: number) => void;
  readonly jshyperedge_confidence: (a: number) => number;
  readonly jshyperedge_description: (a: number, b: number) => void;
  readonly jshyperedge_embedding: (a: number, b: number) => void;
  readonly jshyperedge_id: (a: number, b: number) => void;
  readonly jshyperedge_nodes: (a: number, b: number) => void;
  readonly jshyperedge_order: (a: number) => number;
  readonly jshyperedge_properties: (a: number) => number;
  readonly jsnode_embedding: (a: number, b: number) => void;
  readonly jsnode_getProperty: (a: number, b: number, c: number) => number;
  readonly jsnode_hasLabel: (a: number, b: number, c: number) => number;
  readonly jsnode_id: (a: number, b: number) => void;
  readonly jsnode_labels: (a: number, b: number) => void;
  readonly jsnode_properties: (a: number) => number;
  readonly queryresult_count: (a: number) => number;
  readonly queryresult_data: (a: number) => number;
  readonly queryresult_edges: (a: number, b: number) => void;
  readonly queryresult_hyperedges: (a: number, b: number) => void;
  readonly queryresult_isEmpty: (a: number) => number;
  readonly queryresult_nodes: (a: number, b: number) => void;
  readonly resultstream_new: (a: number) => number;
  readonly resultstream_nextChunk: (a: number) => number;
  readonly resultstream_offset: (a: number) => number;
  readonly resultstream_reset: (a: number) => void;
  readonly version: (a: number) => void;
  readonly init: () => void;
  readonly batchoperations_maxBatchSize: (a: number) => number;
  readonly resultstream_chunkSize: (a: number) => number;
  readonly __wbg_batchoperations_free: (a: number, b: number) => void;
  readonly __wasm_bindgen_func_elem_446: (a: number, b: number, c: number) => void;
  readonly __wasm_bindgen_func_elem_445: (a: number, b: number) => void;
  readonly __wasm_bindgen_func_elem_774: (a: number, b: number, c: number, d: number) => void;
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
