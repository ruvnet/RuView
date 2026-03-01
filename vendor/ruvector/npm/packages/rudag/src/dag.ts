/**
 * High-level DAG API with WASM acceleration
 * Provides a TypeScript-friendly interface to the WASM DAG implementation
 *
 * @security All inputs are validated to prevent injection attacks
 * @performance Results are cached to minimize WASM calls
 */

import { createStorage, DagStorage, MemoryStorage, StoredDag } from './storage';

// WASM module type definitions
interface WasmDagModule {
  WasmDag: {
    new(): WasmDagInstance;
    from_bytes(data: Uint8Array): WasmDagInstance;
    from_json(json: string): WasmDagInstance;
  };
}

interface WasmDagInstance {
  add_node(op: number, cost: number): number;
  add_edge(from: number, to: number): boolean;
  node_count(): number;
  edge_count(): number;
  topo_sort(): Uint32Array;
  critical_path(): string | CriticalPath;
  attention(mechanism: number): Float32Array;
  to_bytes(): Uint8Array;
  to_json(): string;
  free(): void;
}

/**
 * Operator types for DAG nodes
 */
export enum DagOperator {
  /** Table scan operation */
  SCAN = 0,
  /** Filter/WHERE clause */
  FILTER = 1,
  /** Column projection/SELECT */
  PROJECT = 2,
  /** Join operation */
  JOIN = 3,
  /** Aggregation (GROUP BY) */
  AGGREGATE = 4,
  /** Sort/ORDER BY */
  SORT = 5,
  /** Limit/TOP N */
  LIMIT = 6,
  /** Union of results */
  UNION = 7,
  /** Custom user-defined operator */
  CUSTOM = 255,
}

/**
 * Attention mechanism types for node scoring
 */
export enum AttentionMechanism {
  /** Score by position in topological order */
  TOPOLOGICAL = 0,
  /** Score by distance from critical path */
  CRITICAL_PATH = 1,
  /** Equal scores for all nodes */
  UNIFORM = 2,
}

/**
 * Node representation in the DAG
 */
export interface DagNode {
  /** Unique identifier for this node */
  id: number;
  /** The operator type (e.g., SCAN, FILTER, JOIN) */
  operator: DagOperator | number;
  /** Execution cost estimate for this node */
  cost: number;
  /** Optional arbitrary metadata attached to the node */
  metadata?: Record<string, unknown>;
}

/**
 * Edge representation (directed connection between nodes)
 */
export interface DagEdge {
  /** Source node ID */
  from: number;
  /** Target node ID */
  to: number;
}

/**
 * Critical path result from DAG analysis
 */
export interface CriticalPath {
  /** Node IDs in the critical path */
  path: number[];
  /** Total cost of the critical path */
  cost: number;
}

/**
 * DAG configuration options
 */
export interface RuDagOptions {
  /** Custom ID for the DAG (auto-generated if not provided) */
  id?: string;
  /** Human-readable name */
  name?: string;
  /** Storage backend (IndexedDB/Memory/null for no persistence) */
  storage?: DagStorage | MemoryStorage | null;
  /** Auto-save changes to storage (default: true) */
  autoSave?: boolean;
  /** Error handler for background save failures */
  onSaveError?: (error: unknown) => void;
}

// WASM module singleton with loading promise for concurrent access
let wasmModule: WasmDagModule | null = null;
let wasmLoadPromise: Promise<WasmDagModule> | null = null;

/**
 * Initialize WASM module (singleton pattern with concurrent safety)
 * @throws {Error} If WASM module fails to load
 */
async function initWasm(): Promise<WasmDagModule> {
  if (wasmModule) return wasmModule;

  // Prevent concurrent loading
  if (wasmLoadPromise) return wasmLoadPromise;

  wasmLoadPromise = (async () => {
    try {
      // Try browser bundler version first
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const mod = await import('../pkg/ruvector_dag_wasm.js') as any;
      if (typeof mod.default === 'function') {
        await mod.default();
      }
      wasmModule = mod as WasmDagModule;
      return wasmModule;
    } catch {
      try {
        // Fallback to Node.js version
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const mod = await import('../pkg-node/ruvector_dag_wasm.js') as any;
        wasmModule = mod as WasmDagModule;
        return wasmModule;
      } catch (e) {
        wasmLoadPromise = null; // Allow retry on failure
        throw new Error(`Failed to load WASM module: ${e}`);
      }
    }
  })();

  return wasmLoadPromise;
}

/**
 * Type guard for CriticalPath validation
 * @security Prevents prototype pollution from untrusted WASM output
 */
function isCriticalPath(obj: unknown): obj is CriticalPath {
  if (typeof obj !== 'object' || obj === null) return false;
  if (Object.getPrototypeOf(obj) !== Object.prototype && Object.getPrototypeOf(obj) !== null) return false;

  const candidate = obj as Record<string, unknown>;

  if (!('path' in candidate) || !Array.isArray(candidate.path)) return false;
  if (!candidate.path.every((item: unknown) => typeof item === 'number' && Number.isFinite(item))) return false;
  if (!('cost' in candidate) || typeof candidate.cost !== 'number') return false;
  if (!Number.isFinite(candidate.cost)) return false;

  return true;
}

/**
 * Validate DAG ID to prevent injection attacks
 * @security Prevents path traversal and special character injection
 */
function isValidDagId(id: string): boolean {
  if (typeof id !== 'string' || id.length === 0 || id.length > 256) return false;
  // Only allow alphanumeric, dash, underscore
  return /^[a-zA-Z0-9_-]+$/.test(id);
}

/**
 * Sanitize ID or generate a safe one
 */
function sanitizeOrGenerateId(id?: string): string {
  if (id && isValidDagId(id)) return id;
  // Generate safe ID
  const timestamp = Date.now();
  const random = Math.random().toString(36).slice(2, 8);
  return `dag-${timestamp}-${random}`;
}

/**
 * RuDag - High-performance DAG with WASM acceleration and persistence
 *
 * @example
 * ```typescript
 * const dag = await new RuDag({ name: 'my-query' }).init();
 * const scan = dag.addNode(DagOperator.SCAN, 10.0);
 * const filter = dag.addNode(DagOperator.FILTER, 2.0);
 * dag.addEdge(scan, filter);
 * const { path, cost } = dag.criticalPath();
 * ```
 */
export class RuDag {
  private wasm: WasmDagInstance | null = null;
  private nodes: Map<number, DagNode> = new Map();
  private storage: DagStorage | MemoryStorage | null;
  private readonly id: string;
  private name?: string;
  private autoSave: boolean;
  private initialized = false;
  private onSaveError?: (error: unknown) => void;

  // Cache for expensive operations
  private _topoCache: number[] | null = null;
  private _criticalPathCache: CriticalPath | null = null;
  private _dirty = true;

  constructor(options: RuDagOptions = {}) {
    this.id = sanitizeOrGenerateId(options.id);
    this.name = options.name;
    this.storage = options.storage === undefined ? createStorage() : options.storage;
    this.autoSave = options.autoSave ?? true;
    this.onSaveError = options.onSaveError;
  }

  /**
   * Initialize the DAG with WASM module and storage
   * @returns This instance for chaining
   * @throws {Error} If WASM module fails to load
   * @throws {Error} If storage initialization fails
   */
  async init(): Promise<this> {
    if (this.initialized) return this;

    const mod = await initWasm();

    try {
      this.wasm = new mod.WasmDag();
    } catch (error) {
      throw new Error(`Failed to create WASM DAG instance: ${error}`);
    }

    try {
      if (this.storage) {
        await this.storage.init();
      }
    } catch (error) {
      // Cleanup WASM on storage failure
      if (this.wasm) {
        this.wasm.free();
        this.wasm = null;
      }
      throw new Error(`Failed to initialize storage: ${error}`);
    }

    this.initialized = true;
    return this;
  }

  /**
   * Ensure DAG is initialized
   * @throws {Error} If DAG not initialized
   */
  private ensureInit(): WasmDagInstance {
    if (!this.wasm) {
      throw new Error('DAG not initialized. Call init() first.');
    }
    return this.wasm;
  }

  /**
   * Handle background save errors
   */
  private handleSaveError(error: unknown): void {
    if (this.onSaveError) {
      this.onSaveError(error);
    } else {
      console.warn('[RuDag] Background save failed:', error);
    }
  }

  /**
   * Invalidate caches (called when DAG structure changes)
   */
  private invalidateCache(): void {
    this._dirty = true;
    this._topoCache = null;
    this._criticalPathCache = null;
  }

  /**
   * Add a node to the DAG
   * @param operator - The operator type
   * @param cost - Execution cost estimate (must be non-negative)
   * @param metadata - Optional metadata
   * @returns The new node ID
   * @throws {Error} If cost is invalid
   */
  addNode(operator: DagOperator | number, cost: number, metadata?: Record<string, unknown>): number {
    // Input validation
    if (!Number.isFinite(cost) || cost < 0) {
      throw new Error(`Invalid cost: ${cost}. Must be a non-negative finite number.`);
    }
    if (!Number.isInteger(operator) || operator < 0 || operator > 255) {
      throw new Error(`Invalid operator: ${operator}. Must be an integer 0-255.`);
    }

    const wasm = this.ensureInit();
    const id = wasm.add_node(operator, cost);

    this.nodes.set(id, {
      id,
      operator,
      cost,
      metadata,
    });

    this.invalidateCache();

    if (this.autoSave) {
      this.save().catch((e) => this.handleSaveError(e));
    }

    return id;
  }

  /**
   * Add an edge between nodes
   * @param from - Source node ID
   * @param to - Target node ID
   * @returns true if edge was added, false if it would create a cycle
   * @throws {Error} If node IDs are invalid
   */
  addEdge(from: number, to: number): boolean {
    // Input validation
    if (!Number.isInteger(from) || from < 0) {
      throw new Error(`Invalid 'from' node ID: ${from}`);
    }
    if (!Number.isInteger(to) || to < 0) {
      throw new Error(`Invalid 'to' node ID: ${to}`);
    }
    if (from === to) {
      throw new Error('Self-loops are not allowed in a DAG');
    }

    const wasm = this.ensureInit();
    const success = wasm.add_edge(from, to);

    if (success) {
      this.invalidateCache();

      if (this.autoSave) {
        this.save().catch((e) => this.handleSaveError(e));
      }
    }

    return success;
  }

  /**
   * Get node count
   */
  get nodeCount(): number {
    return this.ensureInit().node_count();
  }

  /**
   * Get edge count
   */
  get edgeCount(): number {
    return this.ensureInit().edge_count();
  }

  /**
   * Get topological sort (cached)
   * @returns Array of node IDs in topological order
   */
  topoSort(): number[] {
    if (!this._dirty && this._topoCache) {
      return [...this._topoCache]; // Return copy to prevent mutation
    }

    const result = this.ensureInit().topo_sort();
    this._topoCache = Array.from(result);
    return [...this._topoCache];
  }

  /**
   * Find critical path (cached)
   * @returns Object with path (node IDs) and total cost
   * @throws {Error} If WASM returns invalid data
   */
  criticalPath(): CriticalPath {
    if (!this._dirty && this._criticalPathCache) {
      return { ...this._criticalPathCache, path: [...this._criticalPathCache.path] };
    }

    const result = this.ensureInit().critical_path();

    let parsed: unknown;
    if (typeof result === 'string') {
      try {
        parsed = JSON.parse(result);
      } catch (e) {
        throw new Error(`Invalid critical path JSON from WASM: ${e}`);
      }
    } else {
      parsed = result;
    }

    if (!isCriticalPath(parsed)) {
      throw new Error('Invalid critical path structure from WASM');
    }

    this._criticalPathCache = parsed;
    this._dirty = false;

    return { ...parsed, path: [...parsed.path] };
  }

  /**
   * Compute attention scores for nodes
   * @param mechanism - Attention mechanism to use
   * @returns Array of scores (one per node)
   */
  attention(mechanism: AttentionMechanism = AttentionMechanism.CRITICAL_PATH): number[] {
    if (!Number.isInteger(mechanism) || mechanism < 0 || mechanism > 2) {
      throw new Error(`Invalid attention mechanism: ${mechanism}`);
    }
    const result = this.ensureInit().attention(mechanism);
    return Array.from(result);
  }

  /**
   * Get node by ID
   */
  getNode(id: number): DagNode | undefined {
    return this.nodes.get(id);
  }

  /**
   * Get all nodes
   */
  getNodes(): DagNode[] {
    return Array.from(this.nodes.values());
  }

  /**
   * Serialize to bytes (bincode format)
   */
  toBytes(): Uint8Array {
    return this.ensureInit().to_bytes();
  }

  /**
   * Serialize to JSON string
   */
  toJSON(): string {
    return this.ensureInit().to_json();
  }

  /**
   * Save DAG to storage
   * @returns StoredDag record or null if no storage configured
   */
  async save(): Promise<StoredDag | null> {
    if (!this.storage) return null;

    const data = this.toBytes();
    return this.storage.save(this.id, data, {
      name: this.name,
      metadata: {
        nodeCount: this.nodeCount,
        edgeCount: this.edgeCount,
        nodes: Object.fromEntries(this.nodes),
      },
    });
  }

  /**
   * Load DAG from storage by ID
   * @param id - DAG ID to load
   * @param storage - Storage backend (creates default if not provided)
   * @returns Loaded DAG or null if not found
   * @throws {Error} If ID contains invalid characters
   */
  static async load(id: string, storage?: DagStorage | MemoryStorage): Promise<RuDag | null> {
    if (!isValidDagId(id)) {
      throw new Error(`Invalid DAG ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
    }

    const isOwnedStorage = !storage;
    const store = storage || createStorage();

    try {
      await store.init();
      const record = await store.get(id);

      if (!record) {
        if (isOwnedStorage) store.close();
        return null;
      }

      return RuDag.fromBytes(record.data, {
        id: record.id,
        name: record.name,
        storage: store,
      });
    } catch (error) {
      if (isOwnedStorage) store.close();
      throw error;
    }
  }

  /**
   * Create DAG from bytes
   * @param data - Serialized DAG data
   * @param options - Configuration options
   * @throws {Error} If data is empty or invalid
   */
  static async fromBytes(data: Uint8Array, options: RuDagOptions = {}): Promise<RuDag> {
    if (!data || data.length === 0) {
      throw new Error('Cannot create DAG from empty or null data');
    }

    const mod = await initWasm();
    const dag = new RuDag(options);

    try {
      dag.wasm = mod.WasmDag.from_bytes(data);
    } catch (error) {
      throw new Error(`Failed to deserialize DAG from bytes: ${error}`);
    }

    dag.initialized = true;

    if (dag.storage) {
      try {
        await dag.storage.init();
      } catch (error) {
        dag.wasm?.free();
        dag.wasm = null;
        throw new Error(`Failed to initialize storage: ${error}`);
      }
    }

    return dag;
  }

  /**
   * Create DAG from JSON
   * @param json - JSON string
   * @param options - Configuration options
   * @throws {Error} If JSON is empty or invalid
   */
  static async fromJSON(json: string, options: RuDagOptions = {}): Promise<RuDag> {
    if (!json || json.trim().length === 0) {
      throw new Error('Cannot create DAG from empty or null JSON');
    }

    const mod = await initWasm();
    const dag = new RuDag(options);

    try {
      dag.wasm = mod.WasmDag.from_json(json);
    } catch (error) {
      throw new Error(`Failed to deserialize DAG from JSON: ${error}`);
    }

    dag.initialized = true;

    if (dag.storage) {
      try {
        await dag.storage.init();
      } catch (error) {
        dag.wasm?.free();
        dag.wasm = null;
        throw new Error(`Failed to initialize storage: ${error}`);
      }
    }

    return dag;
  }

  /**
   * List all stored DAGs
   * @param storage - Storage backend (creates default if not provided)
   */
  static async listStored(storage?: DagStorage | MemoryStorage): Promise<StoredDag[]> {
    const isOwnedStorage = !storage;
    const store = storage || createStorage();

    try {
      await store.init();
      const result = await store.list();
      if (isOwnedStorage) store.close();
      return result;
    } catch (error) {
      if (isOwnedStorage) store.close();
      throw error;
    }
  }

  /**
   * Delete a stored DAG
   * @param id - DAG ID to delete
   * @param storage - Storage backend (creates default if not provided)
   * @throws {Error} If ID contains invalid characters
   */
  static async deleteStored(id: string, storage?: DagStorage | MemoryStorage): Promise<boolean> {
    if (!isValidDagId(id)) {
      throw new Error(`Invalid DAG ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
    }

    const isOwnedStorage = !storage;
    const store = storage || createStorage();

    try {
      await store.init();
      const result = await store.delete(id);
      if (isOwnedStorage) store.close();
      return result;
    } catch (error) {
      if (isOwnedStorage) store.close();
      throw error;
    }
  }

  /**
   * Get storage statistics
   * @param storage - Storage backend (creates default if not provided)
   */
  static async storageStats(storage?: DagStorage | MemoryStorage): Promise<{ count: number; totalSize: number }> {
    const isOwnedStorage = !storage;
    const store = storage || createStorage();

    try {
      await store.init();
      const result = await store.stats();
      if (isOwnedStorage) store.close();
      return result;
    } catch (error) {
      if (isOwnedStorage) store.close();
      throw error;
    }
  }

  /**
   * Get DAG ID
   */
  getId(): string {
    return this.id;
  }

  /**
   * Get DAG name
   */
  getName(): string | undefined {
    return this.name;
  }

  /**
   * Set DAG name
   * @param name - New name for the DAG
   */
  setName(name: string): void {
    this.name = name;
    if (this.autoSave) {
      this.save().catch((e) => this.handleSaveError(e));
    }
  }

  /**
   * Cleanup resources (WASM memory and storage connection)
   * Always call this when done with a DAG to prevent memory leaks
   */
  dispose(): void {
    if (this.wasm) {
      this.wasm.free();
      this.wasm = null;
    }
    if (this.storage) {
      this.storage.close();
      this.storage = null;
    }
    this.nodes.clear();
    this._topoCache = null;
    this._criticalPathCache = null;
    this.initialized = false;
  }
}
