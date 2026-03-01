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
