/**
 * Database Persistence Module for ruvector-extensions
 *
 * Provides comprehensive database persistence capabilities including:
 * - Multiple save formats (JSON, Binary/MessagePack, SQLite)
 * - Incremental saves (only changed data)
 * - Snapshot management (create, list, restore, delete)
 * - Export/import functionality
 * - Compression support
 * - Progress callbacks for large operations
 *
 * @module persistence
 */
import type { VectorEntry, DbOptions, DbStats } from 'ruvector';
type VectorDBInstance = any;
/**
 * Supported persistence formats
 */
export type PersistenceFormat = 'json' | 'binary' | 'sqlite';
/**
 * Compression algorithms
 */
export type CompressionType = 'none' | 'gzip' | 'brotli';
/**
 * Progress callback for long-running operations
 */
export type ProgressCallback = (progress: {
    /** Operation being performed */
    operation: string;
    /** Current progress (0-100) */
    percentage: number;
    /** Number of items processed */
    current: number;
    /** Total items to process */
    total: number;
    /** Human-readable message */
    message: string;
}) => void;
/**
 * Persistence configuration options
 */
export interface PersistenceOptions {
    /** Base directory for persistence files */
    baseDir: string;
    /** Default format for saves */
    format?: PersistenceFormat;
    /** Enable compression */
    compression?: CompressionType;
    /** Enable incremental saves */
    incremental?: boolean;
    /** Auto-save interval in milliseconds (0 = disabled) */
    autoSaveInterval?: number;
    /** Maximum number of snapshots to keep */
    maxSnapshots?: number;
    /** Batch size for large operations */
    batchSize?: number;
}
/**
 * Database snapshot metadata
 */
export interface SnapshotMetadata {
    /** Snapshot identifier */
    id: string;
    /** Human-readable name */
    name: string;
    /** Creation timestamp */
    timestamp: number;
    /** Vector count at snapshot time */
    vectorCount: number;
    /** Database dimension */
    dimension: number;
    /** Format used */
    format: PersistenceFormat;
    /** Whether compressed */
    compressed: boolean;
    /** File size in bytes */
    fileSize: number;
    /** Checksum for integrity */
    checksum: string;
    /** Additional metadata */
    metadata?: Record<string, any>;
}
/**
 * Serialized database state
 */
export interface DatabaseState {
    /** Format version for compatibility */
    version: string;
    /** Database configuration */
    options: DbOptions;
    /** Database statistics */
    stats: DbStats;
    /** Vector entries */
    vectors: VectorEntry[];
    /** Index state (opaque) */
    indexState?: any;
    /** Additional metadata */
    metadata?: Record<string, any>;
    /** Timestamp of save */
    timestamp: number;
    /** Checksum for integrity */
    checksum?: string;
}
/**
 * Export options
 */
export interface ExportOptions {
    /** Output file path */
    path: string;
    /** Export format */
    format?: PersistenceFormat;
    /** Enable compression */
    compress?: boolean;
    /** Include index state */
    includeIndex?: boolean;
    /** Progress callback */
    onProgress?: ProgressCallback;
}
/**
 * Import options
 */
export interface ImportOptions {
    /** Input file path */
    path: string;
    /** Expected format (auto-detect if not specified) */
    format?: PersistenceFormat;
    /** Whether to clear database before import */
    clear?: boolean;
    /** Verify checksum */
    verifyChecksum?: boolean;
    /** Progress callback */
    onProgress?: ProgressCallback;
}
/**
 * Main persistence manager for VectorDB instances
 *
 * @example
 * ```typescript
 * const db = new VectorDB({ dimension: 384 });
 * const persistence = new DatabasePersistence(db, {
 *   baseDir: './data',
 *   format: 'binary',
 *   compression: 'gzip',
 *   incremental: true
 * });
 *
 * // Save database
 * await persistence.save({ onProgress: (p) => console.log(p.message) });
 *
 * // Create snapshot
 * const snapshot = await persistence.createSnapshot('before-update');
 *
 * // Restore from snapshot
 * await persistence.restoreSnapshot(snapshot.id);
 * ```
 */
export declare class DatabasePersistence {
    private db;
    private options;
    private incrementalState;
    private autoSaveTimer;
    /**
     * Create a new database persistence manager
     *
     * @param db - VectorDB instance to manage
     * @param options - Persistence configuration
     */
    constructor(db: VectorDBInstance, options: PersistenceOptions);
    /**
     * Initialize persistence system
     */
    private initialize;
    /**
     * Save database to disk
     *
     * @param options - Save options
     * @returns Path to saved file
     */
    save(options?: {
        path?: string;
        format?: PersistenceFormat;
        compress?: boolean;
        onProgress?: ProgressCallback;
    }): Promise<string>;
    /**
     * Save only changed data (incremental save)
     *
     * @param options - Save options
     * @returns Path to saved file or null if no changes
     */
    saveIncremental(options?: {
        path?: string;
        format?: PersistenceFormat;
        onProgress?: ProgressCallback;
    }): Promise<string | null>;
    /**
     * Load database from disk
     *
     * @param options - Load options
     */
    load(options: {
        path: string;
        format?: PersistenceFormat;
        verifyChecksum?: boolean;
        onProgress?: ProgressCallback;
    }): Promise<void>;
    /**
     * Create a snapshot of the current database state
     *
     * @param name - Human-readable snapshot name
     * @param metadata - Additional metadata to store
     * @returns Snapshot metadata
     */
    createSnapshot(name: string, metadata?: Record<string, any>): Promise<SnapshotMetadata>;
    /**
     * List all available snapshots
     *
     * @returns Array of snapshot metadata, sorted by timestamp (newest first)
     */
    listSnapshots(): Promise<SnapshotMetadata[]>;
    /**
     * Restore database from a snapshot
     *
     * @param snapshotId - Snapshot ID to restore
     * @param options - Restore options
     */
    restoreSnapshot(snapshotId: string, options?: {
        verifyChecksum?: boolean;
        onProgress?: ProgressCallback;
    }): Promise<void>;
    /**
     * Delete a snapshot
     *
     * @param snapshotId - Snapshot ID to delete
     */
    deleteSnapshot(snapshotId: string): Promise<void>;
    /**
     * Export database to a file
     *
     * @param options - Export options
     */
    export(options: ExportOptions): Promise<void>;
    /**
     * Import database from a file
     *
     * @param options - Import options
     */
    import(options: ImportOptions): Promise<void>;
    /**
     * Start automatic saves at configured interval
     */
    startAutoSave(): void;
    /**
     * Stop automatic saves
     */
    stopAutoSave(): void;
    /**
     * Cleanup and shutdown
     */
    shutdown(): Promise<void>;
    /**
     * Serialize database to state object
     */
    private serializeDatabase;
    /**
     * Deserialize state object into database
     */
    private deserializeDatabase;
    /**
     * Write state to file in specified format
     */
    private writeStateToFile;
    /**
     * Read state from file in specified format
     */
    private readStateFromFile;
    /**
     * Get all vector IDs from database
     */
    private getAllVectorIds;
    /**
     * Compute checksum of state object
     */
    private computeChecksum;
    /**
     * Compute checksum of file
     */
    private computeFileChecksum;
    /**
     * Detect file format from extension
     */
    private detectFormat;
    /**
     * Check if data is compressed
     */
    private isCompressed;
    /**
     * Get default save path
     */
    private getDefaultSavePath;
    /**
     * Load incremental state
     */
    private loadIncrementalState;
    /**
     * Update incremental state after save
     */
    private updateIncrementalState;
    /**
     * Clean up old snapshots beyond max limit
     */
    private cleanupOldSnapshots;
}
/**
 * Format file size in human-readable format
 *
 * @param bytes - File size in bytes
 * @returns Formatted string (e.g., "1.5 MB")
 */
export declare function formatFileSize(bytes: number): string;
/**
 * Format timestamp as ISO string
 *
 * @param timestamp - Unix timestamp in milliseconds
 * @returns ISO formatted date string
 */
export declare function formatTimestamp(timestamp: number): string;
/**
 * Estimate memory usage of database state
 *
 * @param state - Database state
 * @returns Estimated memory usage in bytes
 */
export declare function estimateMemoryUsage(state: DatabaseState): number;
export {};
//# sourceMappingURL=persistence.d.ts.map