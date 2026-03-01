/**
 * IndexedDB-based persistence layer for DAG storage
 * Provides browser-compatible persistent storage for DAGs
 *
 * @performance Single-transaction pattern for atomic operations
 * @security ID validation to prevent injection
 */
export interface StoredDag {
    /** Unique identifier */
    id: string;
    /** Human-readable name */
    name?: string;
    /** Serialized DAG data */
    data: Uint8Array;
    /** Creation timestamp */
    createdAt: number;
    /** Last update timestamp */
    updatedAt: number;
    /** Optional metadata */
    metadata?: Record<string, unknown>;
}
export interface DagStorageOptions {
    /** Custom database name */
    dbName?: string;
    /** Database version for migrations */
    version?: number;
}
/**
 * Check if IndexedDB is available (browser environment)
 */
export declare function isIndexedDBAvailable(): boolean;
/**
 * IndexedDB storage class for DAG persistence
 *
 * @performance Uses single-transaction pattern for save operations
 */
export declare class DagStorage {
    private dbName;
    private version;
    private db;
    private initialized;
    constructor(options?: DagStorageOptions);
    /**
     * Initialize the database connection
     * @throws {Error} If IndexedDB is not available
     * @throws {Error} If database is blocked by another tab
     */
    init(): Promise<void>;
    /**
     * Ensure database is initialized
     * @throws {Error} If database not initialized
     */
    private ensureInit;
    /**
     * Save a DAG to storage (single-transaction pattern)
     * @performance Uses single transaction for atomic read-modify-write
     */
    save(id: string, data: Uint8Array, options?: {
        name?: string;
        metadata?: Record<string, unknown>;
    }): Promise<StoredDag>;
    /**
     * Save multiple DAGs in a single transaction (batch operation)
     * @performance Much faster than individual saves for bulk operations
     */
    saveBatch(dags: Array<{
        id: string;
        data: Uint8Array;
        name?: string;
        metadata?: Record<string, unknown>;
    }>): Promise<StoredDag[]>;
    /**
     * Get a DAG from storage
     */
    get(id: string): Promise<StoredDag | null>;
    /**
     * Delete a DAG from storage
     */
    delete(id: string): Promise<boolean>;
    /**
     * List all DAGs in storage
     */
    list(): Promise<StoredDag[]>;
    /**
     * Search DAGs by name
     */
    findByName(name: string): Promise<StoredDag[]>;
    /**
     * Clear all DAGs from storage
     */
    clear(): Promise<void>;
    /**
     * Get storage statistics
     */
    stats(): Promise<{
        count: number;
        totalSize: number;
    }>;
    /**
     * Close the database connection
     */
    close(): void;
}
/**
 * In-memory storage fallback for Node.js or environments without IndexedDB
 */
export declare class MemoryStorage {
    private store;
    private initialized;
    init(): Promise<void>;
    save(id: string, data: Uint8Array, options?: {
        name?: string;
        metadata?: Record<string, unknown>;
    }): Promise<StoredDag>;
    saveBatch(dags: Array<{
        id: string;
        data: Uint8Array;
        name?: string;
        metadata?: Record<string, unknown>;
    }>): Promise<StoredDag[]>;
    get(id: string): Promise<StoredDag | null>;
    delete(id: string): Promise<boolean>;
    list(): Promise<StoredDag[]>;
    findByName(name: string): Promise<StoredDag[]>;
    clear(): Promise<void>;
    stats(): Promise<{
        count: number;
        totalSize: number;
    }>;
    close(): void;
}
/**
 * Create appropriate storage based on environment
 */
export declare function createStorage(options?: DagStorageOptions): DagStorage | MemoryStorage;
//# sourceMappingURL=storage.d.ts.map