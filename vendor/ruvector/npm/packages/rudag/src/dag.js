"use strict";
/**
 * High-level DAG API with WASM acceleration
 * Provides a TypeScript-friendly interface to the WASM DAG implementation
 *
 * @security All inputs are validated to prevent injection attacks
 * @performance Results are cached to minimize WASM calls
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.RuDag = exports.AttentionMechanism = exports.DagOperator = void 0;
const storage_1 = require("./storage");
/**
 * Operator types for DAG nodes
 */
var DagOperator;
(function (DagOperator) {
    /** Table scan operation */
    DagOperator[DagOperator["SCAN"] = 0] = "SCAN";
    /** Filter/WHERE clause */
    DagOperator[DagOperator["FILTER"] = 1] = "FILTER";
    /** Column projection/SELECT */
    DagOperator[DagOperator["PROJECT"] = 2] = "PROJECT";
    /** Join operation */
    DagOperator[DagOperator["JOIN"] = 3] = "JOIN";
    /** Aggregation (GROUP BY) */
    DagOperator[DagOperator["AGGREGATE"] = 4] = "AGGREGATE";
    /** Sort/ORDER BY */
    DagOperator[DagOperator["SORT"] = 5] = "SORT";
    /** Limit/TOP N */
    DagOperator[DagOperator["LIMIT"] = 6] = "LIMIT";
    /** Union of results */
    DagOperator[DagOperator["UNION"] = 7] = "UNION";
    /** Custom user-defined operator */
    DagOperator[DagOperator["CUSTOM"] = 255] = "CUSTOM";
})(DagOperator || (exports.DagOperator = DagOperator = {}));
/**
 * Attention mechanism types for node scoring
 */
var AttentionMechanism;
(function (AttentionMechanism) {
    /** Score by position in topological order */
    AttentionMechanism[AttentionMechanism["TOPOLOGICAL"] = 0] = "TOPOLOGICAL";
    /** Score by distance from critical path */
    AttentionMechanism[AttentionMechanism["CRITICAL_PATH"] = 1] = "CRITICAL_PATH";
    /** Equal scores for all nodes */
    AttentionMechanism[AttentionMechanism["UNIFORM"] = 2] = "UNIFORM";
})(AttentionMechanism || (exports.AttentionMechanism = AttentionMechanism = {}));
// WASM module singleton with loading promise for concurrent access
let wasmModule = null;
let wasmLoadPromise = null;
/**
 * Initialize WASM module (singleton pattern with concurrent safety)
 * @throws {Error} If WASM module fails to load
 */
async function initWasm() {
    if (wasmModule)
        return wasmModule;
    // Prevent concurrent loading
    if (wasmLoadPromise)
        return wasmLoadPromise;
    wasmLoadPromise = (async () => {
        try {
            // Try browser bundler version first
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            const mod = await Promise.resolve().then(() => __importStar(require('../pkg/ruvector_dag_wasm.js')));
            if (typeof mod.default === 'function') {
                await mod.default();
            }
            wasmModule = mod;
            return wasmModule;
        }
        catch {
            try {
                // Fallback to Node.js version
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                const mod = await Promise.resolve().then(() => __importStar(require('../pkg-node/ruvector_dag_wasm.js')));
                wasmModule = mod;
                return wasmModule;
            }
            catch (e) {
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
function isCriticalPath(obj) {
    if (typeof obj !== 'object' || obj === null)
        return false;
    if (Object.getPrototypeOf(obj) !== Object.prototype && Object.getPrototypeOf(obj) !== null)
        return false;
    const candidate = obj;
    if (!('path' in candidate) || !Array.isArray(candidate.path))
        return false;
    if (!candidate.path.every((item) => typeof item === 'number' && Number.isFinite(item)))
        return false;
    if (!('cost' in candidate) || typeof candidate.cost !== 'number')
        return false;
    if (!Number.isFinite(candidate.cost))
        return false;
    return true;
}
/**
 * Validate DAG ID to prevent injection attacks
 * @security Prevents path traversal and special character injection
 */
function isValidDagId(id) {
    if (typeof id !== 'string' || id.length === 0 || id.length > 256)
        return false;
    // Only allow alphanumeric, dash, underscore
    return /^[a-zA-Z0-9_-]+$/.test(id);
}
/**
 * Sanitize ID or generate a safe one
 */
function sanitizeOrGenerateId(id) {
    if (id && isValidDagId(id))
        return id;
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
class RuDag {
    constructor(options = {}) {
        this.wasm = null;
        this.nodes = new Map();
        this.initialized = false;
        // Cache for expensive operations
        this._topoCache = null;
        this._criticalPathCache = null;
        this._dirty = true;
        this.id = sanitizeOrGenerateId(options.id);
        this.name = options.name;
        this.storage = options.storage === undefined ? (0, storage_1.createStorage)() : options.storage;
        this.autoSave = options.autoSave ?? true;
        this.onSaveError = options.onSaveError;
    }
    /**
     * Initialize the DAG with WASM module and storage
     * @returns This instance for chaining
     * @throws {Error} If WASM module fails to load
     * @throws {Error} If storage initialization fails
     */
    async init() {
        if (this.initialized)
            return this;
        const mod = await initWasm();
        try {
            this.wasm = new mod.WasmDag();
        }
        catch (error) {
            throw new Error(`Failed to create WASM DAG instance: ${error}`);
        }
        try {
            if (this.storage) {
                await this.storage.init();
            }
        }
        catch (error) {
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
    ensureInit() {
        if (!this.wasm) {
            throw new Error('DAG not initialized. Call init() first.');
        }
        return this.wasm;
    }
    /**
     * Handle background save errors
     */
    handleSaveError(error) {
        if (this.onSaveError) {
            this.onSaveError(error);
        }
        else {
            console.warn('[RuDag] Background save failed:', error);
        }
    }
    /**
     * Invalidate caches (called when DAG structure changes)
     */
    invalidateCache() {
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
    addNode(operator, cost, metadata) {
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
    addEdge(from, to) {
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
    get nodeCount() {
        return this.ensureInit().node_count();
    }
    /**
     * Get edge count
     */
    get edgeCount() {
        return this.ensureInit().edge_count();
    }
    /**
     * Get topological sort (cached)
     * @returns Array of node IDs in topological order
     */
    topoSort() {
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
    criticalPath() {
        if (!this._dirty && this._criticalPathCache) {
            return { ...this._criticalPathCache, path: [...this._criticalPathCache.path] };
        }
        const result = this.ensureInit().critical_path();
        let parsed;
        if (typeof result === 'string') {
            try {
                parsed = JSON.parse(result);
            }
            catch (e) {
                throw new Error(`Invalid critical path JSON from WASM: ${e}`);
            }
        }
        else {
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
    attention(mechanism = AttentionMechanism.CRITICAL_PATH) {
        if (!Number.isInteger(mechanism) || mechanism < 0 || mechanism > 2) {
            throw new Error(`Invalid attention mechanism: ${mechanism}`);
        }
        const result = this.ensureInit().attention(mechanism);
        return Array.from(result);
    }
    /**
     * Get node by ID
     */
    getNode(id) {
        return this.nodes.get(id);
    }
    /**
     * Get all nodes
     */
    getNodes() {
        return Array.from(this.nodes.values());
    }
    /**
     * Serialize to bytes (bincode format)
     */
    toBytes() {
        return this.ensureInit().to_bytes();
    }
    /**
     * Serialize to JSON string
     */
    toJSON() {
        return this.ensureInit().to_json();
    }
    /**
     * Save DAG to storage
     * @returns StoredDag record or null if no storage configured
     */
    async save() {
        if (!this.storage)
            return null;
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
    static async load(id, storage) {
        if (!isValidDagId(id)) {
            throw new Error(`Invalid DAG ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
        }
        const isOwnedStorage = !storage;
        const store = storage || (0, storage_1.createStorage)();
        try {
            await store.init();
            const record = await store.get(id);
            if (!record) {
                if (isOwnedStorage)
                    store.close();
                return null;
            }
            return RuDag.fromBytes(record.data, {
                id: record.id,
                name: record.name,
                storage: store,
            });
        }
        catch (error) {
            if (isOwnedStorage)
                store.close();
            throw error;
        }
    }
    /**
     * Create DAG from bytes
     * @param data - Serialized DAG data
     * @param options - Configuration options
     * @throws {Error} If data is empty or invalid
     */
    static async fromBytes(data, options = {}) {
        if (!data || data.length === 0) {
            throw new Error('Cannot create DAG from empty or null data');
        }
        const mod = await initWasm();
        const dag = new RuDag(options);
        try {
            dag.wasm = mod.WasmDag.from_bytes(data);
        }
        catch (error) {
            throw new Error(`Failed to deserialize DAG from bytes: ${error}`);
        }
        dag.initialized = true;
        if (dag.storage) {
            try {
                await dag.storage.init();
            }
            catch (error) {
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
    static async fromJSON(json, options = {}) {
        if (!json || json.trim().length === 0) {
            throw new Error('Cannot create DAG from empty or null JSON');
        }
        const mod = await initWasm();
        const dag = new RuDag(options);
        try {
            dag.wasm = mod.WasmDag.from_json(json);
        }
        catch (error) {
            throw new Error(`Failed to deserialize DAG from JSON: ${error}`);
        }
        dag.initialized = true;
        if (dag.storage) {
            try {
                await dag.storage.init();
            }
            catch (error) {
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
    static async listStored(storage) {
        const isOwnedStorage = !storage;
        const store = storage || (0, storage_1.createStorage)();
        try {
            await store.init();
            const result = await store.list();
            if (isOwnedStorage)
                store.close();
            return result;
        }
        catch (error) {
            if (isOwnedStorage)
                store.close();
            throw error;
        }
    }
    /**
     * Delete a stored DAG
     * @param id - DAG ID to delete
     * @param storage - Storage backend (creates default if not provided)
     * @throws {Error} If ID contains invalid characters
     */
    static async deleteStored(id, storage) {
        if (!isValidDagId(id)) {
            throw new Error(`Invalid DAG ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
        }
        const isOwnedStorage = !storage;
        const store = storage || (0, storage_1.createStorage)();
        try {
            await store.init();
            const result = await store.delete(id);
            if (isOwnedStorage)
                store.close();
            return result;
        }
        catch (error) {
            if (isOwnedStorage)
                store.close();
            throw error;
        }
    }
    /**
     * Get storage statistics
     * @param storage - Storage backend (creates default if not provided)
     */
    static async storageStats(storage) {
        const isOwnedStorage = !storage;
        const store = storage || (0, storage_1.createStorage)();
        try {
            await store.init();
            const result = await store.stats();
            if (isOwnedStorage)
                store.close();
            return result;
        }
        catch (error) {
            if (isOwnedStorage)
                store.close();
            throw error;
        }
    }
    /**
     * Get DAG ID
     */
    getId() {
        return this.id;
    }
    /**
     * Get DAG name
     */
    getName() {
        return this.name;
    }
    /**
     * Set DAG name
     * @param name - New name for the DAG
     */
    setName(name) {
        this.name = name;
        if (this.autoSave) {
            this.save().catch((e) => this.handleSaveError(e));
        }
    }
    /**
     * Cleanup resources (WASM memory and storage connection)
     * Always call this when done with a DAG to prevent memory leaks
     */
    dispose() {
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
exports.RuDag = RuDag;
//# sourceMappingURL=dag.js.map