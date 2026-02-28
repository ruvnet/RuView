/**
 * High-level DAG API with WASM acceleration
 * Provides a TypeScript-friendly interface to the WASM DAG implementation
 *
 * @security All inputs are validated to prevent injection attacks
 * @performance Results are cached to minimize WASM calls
 */
import { DagStorage, MemoryStorage, StoredDag } from './storage';
/**
 * Operator types for DAG nodes
 */
export declare enum DagOperator {
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
    CUSTOM = 255
}
/**
 * Attention mechanism types for node scoring
 */
export declare enum AttentionMechanism {
    /** Score by position in topological order */
    TOPOLOGICAL = 0,
    /** Score by distance from critical path */
    CRITICAL_PATH = 1,
    /** Equal scores for all nodes */
    UNIFORM = 2
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
export declare class RuDag {
    private wasm;
    private nodes;
    private storage;
    private readonly id;
    private name?;
    private autoSave;
    private initialized;
    private onSaveError?;
    private _topoCache;
    private _criticalPathCache;
    private _dirty;
    constructor(options?: RuDagOptions);
    /**
     * Initialize the DAG with WASM module and storage
     * @returns This instance for chaining
     * @throws {Error} If WASM module fails to load
     * @throws {Error} If storage initialization fails
     */
    init(): Promise<this>;
    /**
     * Ensure DAG is initialized
     * @throws {Error} If DAG not initialized
     */
    private ensureInit;
    /**
     * Handle background save errors
     */
    private handleSaveError;
    /**
     * Invalidate caches (called when DAG structure changes)
     */
    private invalidateCache;
    /**
     * Add a node to the DAG
     * @param operator - The operator type
     * @param cost - Execution cost estimate (must be non-negative)
     * @param metadata - Optional metadata
     * @returns The new node ID
     * @throws {Error} If cost is invalid
     */
    addNode(operator: DagOperator | number, cost: number, metadata?: Record<string, unknown>): number;
    /**
     * Add an edge between nodes
     * @param from - Source node ID
     * @param to - Target node ID
     * @returns true if edge was added, false if it would create a cycle
     * @throws {Error} If node IDs are invalid
     */
    addEdge(from: number, to: number): boolean;
    /**
     * Get node count
     */
    get nodeCount(): number;
    /**
     * Get edge count
     */
    get edgeCount(): number;
    /**
     * Get topological sort (cached)
     * @returns Array of node IDs in topological order
     */
    topoSort(): number[];
    /**
     * Find critical path (cached)
     * @returns Object with path (node IDs) and total cost
     * @throws {Error} If WASM returns invalid data
     */
    criticalPath(): CriticalPath;
    /**
     * Compute attention scores for nodes
     * @param mechanism - Attention mechanism to use
     * @returns Array of scores (one per node)
     */
    attention(mechanism?: AttentionMechanism): number[];
    /**
     * Get node by ID
     */
    getNode(id: number): DagNode | undefined;
    /**
     * Get all nodes
     */
    getNodes(): DagNode[];
    /**
     * Serialize to bytes (bincode format)
     */
    toBytes(): Uint8Array;
    /**
     * Serialize to JSON string
     */
    toJSON(): string;
    /**
     * Save DAG to storage
     * @returns StoredDag record or null if no storage configured
     */
    save(): Promise<StoredDag | null>;
    /**
     * Load DAG from storage by ID
     * @param id - DAG ID to load
     * @param storage - Storage backend (creates default if not provided)
     * @returns Loaded DAG or null if not found
     * @throws {Error} If ID contains invalid characters
     */
    static load(id: string, storage?: DagStorage | MemoryStorage): Promise<RuDag | null>;
    /**
     * Create DAG from bytes
     * @param data - Serialized DAG data
     * @param options - Configuration options
     * @throws {Error} If data is empty or invalid
     */
    static fromBytes(data: Uint8Array, options?: RuDagOptions): Promise<RuDag>;
    /**
     * Create DAG from JSON
     * @param json - JSON string
     * @param options - Configuration options
     * @throws {Error} If JSON is empty or invalid
     */
    static fromJSON(json: string, options?: RuDagOptions): Promise<RuDag>;
    /**
     * List all stored DAGs
     * @param storage - Storage backend (creates default if not provided)
     */
    static listStored(storage?: DagStorage | MemoryStorage): Promise<StoredDag[]>;
    /**
     * Delete a stored DAG
     * @param id - DAG ID to delete
     * @param storage - Storage backend (creates default if not provided)
     * @throws {Error} If ID contains invalid characters
     */
    static deleteStored(id: string, storage?: DagStorage | MemoryStorage): Promise<boolean>;
    /**
     * Get storage statistics
     * @param storage - Storage backend (creates default if not provided)
     */
    static storageStats(storage?: DagStorage | MemoryStorage): Promise<{
        count: number;
        totalSize: number;
    }>;
    /**
     * Get DAG ID
     */
    getId(): string;
    /**
     * Get DAG name
     */
    getName(): string | undefined;
    /**
     * Set DAG name
     * @param name - New name for the DAG
     */
    setName(name: string): void;
    /**
     * Cleanup resources (WASM memory and storage connection)
     * Always call this when done with a DAG to prevent memory leaks
     */
    dispose(): void;
}
//# sourceMappingURL=dag.d.ts.map