/* tslint:disable */
/* eslint-disable */

export class WasmDag {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get number of edges
   */
  edge_count(): number;
  /**
   * Deserialize from bytes
   */
  static from_bytes(data: Uint8Array): WasmDag;
  /**
   * Get number of nodes
   */
  node_count(): number;
  /**
   * Find critical path (longest path by cost)
   * Returns JSON: {"path": [node_ids], "cost": total}
   */
  critical_path(): any;
  /**
   * Create new empty DAG
   */
  constructor();
  /**
   * Serialize to JSON
   */
  to_json(): string;
  /**
   * Add edge from -> to
   * Returns false if creates cycle (simple check)
   */
  add_edge(from: number, to: number): boolean;
  /**
   * Add a node with operator type and cost
   * Returns node ID
   */
  add_node(op: number, cost: number): number;
  /**
   * Serialize to bytes (bincode format)
   */
  to_bytes(): Uint8Array;
  /**
   * Compute attention scores for nodes
   * mechanism: 0=topological, 1=critical_path, 2=uniform
   */
  attention(mechanism: number): Float32Array;
  /**
   * Deserialize from JSON
   */
  static from_json(json: string): WasmDag;
  /**
   * Topological sort using Kahn's algorithm
   * Returns node IDs in topological order
   */
  topo_sort(): Uint32Array;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_wasmdag_free: (a: number, b: number) => void;
  readonly wasmdag_add_edge: (a: number, b: number, c: number) => number;
  readonly wasmdag_add_node: (a: number, b: number, c: number) => number;
  readonly wasmdag_attention: (a: number, b: number, c: number) => void;
  readonly wasmdag_critical_path: (a: number) => number;
  readonly wasmdag_edge_count: (a: number) => number;
  readonly wasmdag_from_bytes: (a: number, b: number, c: number) => void;
  readonly wasmdag_from_json: (a: number, b: number, c: number) => void;
  readonly wasmdag_new: () => number;
  readonly wasmdag_node_count: (a: number) => number;
  readonly wasmdag_to_bytes: (a: number, b: number) => void;
  readonly wasmdag_to_json: (a: number, b: number) => void;
  readonly wasmdag_topo_sort: (a: number, b: number) => void;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_export: (a: number, b: number) => number;
  readonly __wbindgen_export2: (a: number, b: number, c: number) => void;
  readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
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
