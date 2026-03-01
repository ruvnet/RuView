/**
 * Temporal Tracking Module for RUVector
 *
 * Provides comprehensive version control, change tracking, and time-travel capabilities
 * for ontology and database evolution over time.
 *
 * @module temporal
 * @author ruv.io Team
 * @license MIT
 */
import { EventEmitter } from 'events';
/**
 * Represents the type of change in a version
 */
export declare enum ChangeType {
    ADDITION = "addition",
    DELETION = "deletion",
    MODIFICATION = "modification",
    METADATA = "metadata"
}
/**
 * Represents a single change in the database
 */
export interface Change {
    /** Type of change */
    type: ChangeType;
    /** Path to the changed entity (e.g., "nodes.User", "edges.FOLLOWS") */
    path: string;
    /** Previous value (null for additions) */
    before: any;
    /** New value (null for deletions) */
    after: any;
    /** Timestamp of the change */
    timestamp: number;
    /** Optional metadata about the change */
    metadata?: Record<string, any>;
}
/**
 * Represents a version snapshot with delta encoding
 */
export interface Version {
    /** Unique version identifier */
    id: string;
    /** Parent version ID (null for initial version) */
    parentId: string | null;
    /** Version creation timestamp */
    timestamp: number;
    /** Human-readable version description */
    description: string;
    /** List of changes from parent version (delta encoding) */
    changes: Change[];
    /** Version tags for easy reference */
    tags: string[];
    /** User or system that created the version */
    author?: string;
    /** Checksum for integrity verification */
    checksum: string;
    /** Additional metadata */
    metadata: Record<string, any>;
}
/**
 * Represents a diff between two versions
 */
export interface VersionDiff {
    /** Source version ID */
    fromVersion: string;
    /** Target version ID */
    toVersion: string;
    /** List of changes between versions */
    changes: Change[];
    /** Summary statistics */
    summary: {
        additions: number;
        deletions: number;
        modifications: number;
    };
    /** Timestamp of diff generation */
    generatedAt: number;
}
/**
 * Audit log entry for tracking all operations
 */
export interface AuditLogEntry {
    /** Unique log entry ID */
    id: string;
    /** Operation type */
    operation: 'create' | 'revert' | 'query' | 'compare' | 'tag' | 'prune';
    /** Target version ID */
    versionId?: string;
    /** Timestamp of the operation */
    timestamp: number;
    /** User or system that performed the operation */
    actor?: string;
    /** Operation result status */
    status: 'success' | 'failure' | 'partial';
    /** Error message if operation failed */
    error?: string;
    /** Additional operation details */
    details: Record<string, any>;
}
/**
 * Options for creating a new version
 */
export interface CreateVersionOptions {
    /** Version description */
    description: string;
    /** Optional tags for the version */
    tags?: string[];
    /** Author of the version */
    author?: string;
    /** Additional metadata */
    metadata?: Record<string, any>;
}
/**
 * Options for querying historical data
 */
export interface QueryOptions {
    /** Target timestamp for time-travel query */
    timestamp?: number;
    /** Target version ID */
    versionId?: string;
    /** Filter by path pattern */
    pathPattern?: RegExp;
    /** Include metadata in results */
    includeMetadata?: boolean;
}
/**
 * Visualization data for change history
 */
export interface VisualizationData {
    /** Version timeline */
    timeline: Array<{
        versionId: string;
        timestamp: number;
        description: string;
        changeCount: number;
        tags: string[];
    }>;
    /** Change frequency over time */
    changeFrequency: Array<{
        timestamp: number;
        count: number;
        type: ChangeType;
    }>;
    /** Most frequently changed paths */
    hotspots: Array<{
        path: string;
        changeCount: number;
        lastChanged: number;
    }>;
    /** Version graph (parent-child relationships) */
    versionGraph: {
        nodes: Array<{
            id: string;
            label: string;
            timestamp: number;
        }>;
        edges: Array<{
            from: string;
            to: string;
        }>;
    };
}
/**
 * Temporal Tracker Events
 */
export interface TemporalTrackerEvents {
    versionCreated: [version: Version];
    versionReverted: [fromVersion: string, toVersion: string];
    changeTracked: [change: Change];
    auditLogged: [entry: AuditLogEntry];
    error: [error: Error];
}
/**
 * TemporalTracker - Main class for temporal tracking functionality
 *
 * Provides version management, change tracking, time-travel queries,
 * and audit logging for database evolution over time.
 *
 * @example
 * ```typescript
 * const tracker = new TemporalTracker();
 *
 * // Create initial version
 * const v1 = await tracker.createVersion({
 *   description: 'Initial schema',
 *   tags: ['v1.0']
 * });
 *
 * // Track changes
 * tracker.trackChange({
 *   type: ChangeType.ADDITION,
 *   path: 'nodes.User',
 *   before: null,
 *   after: { name: 'User', properties: ['id', 'name'] },
 *   timestamp: Date.now()
 * });
 *
 * // Create new version with tracked changes
 * const v2 = await tracker.createVersion({
 *   description: 'Added User node',
 *   tags: ['v1.1']
 * });
 *
 * // Time-travel query
 * const snapshot = await tracker.queryAtTimestamp(v1.timestamp);
 *
 * // Compare versions
 * const diff = await tracker.compareVersions(v1.id, v2.id);
 * ```
 */
export declare class TemporalTracker extends EventEmitter {
    private versions;
    private currentState;
    private pendingChanges;
    private auditLog;
    private tagIndex;
    private pathIndex;
    constructor();
    /**
     * Initialize with a baseline empty version
     */
    private initializeBaseline;
    /**
     * Generate a unique ID
     */
    private generateId;
    /**
     * Calculate checksum for data integrity
     */
    private calculateChecksum;
    /**
     * Index a version for fast lookups
     */
    private indexVersion;
    /**
     * Track a change to be included in the next version
     *
     * @param change - The change to track
     * @emits changeTracked
     */
    trackChange(change: Change): void;
    /**
     * Create a new version with all pending changes
     *
     * @param options - Version creation options
     * @returns The created version
     * @emits versionCreated
     */
    createVersion(options: CreateVersionOptions): Promise<Version>;
    /**
     * Apply a change to the state object
     */
    private applyChange;
    /**
     * Get the current (latest) version
     */
    private getCurrentVersion;
    /**
     * List all versions, optionally filtered by tags
     *
     * @param tags - Optional tags to filter by
     * @returns Array of versions
     */
    listVersions(tags?: string[]): Version[];
    /**
     * Get a specific version by ID
     *
     * @param versionId - Version ID
     * @returns The version or null if not found
     */
    getVersion(versionId: string): Version | null;
    /**
     * Compare two versions and generate a diff
     *
     * @param fromVersionId - Source version ID
     * @param toVersionId - Target version ID
     * @returns Version diff
     */
    compareVersions(fromVersionId: string, toVersionId: string): Promise<VersionDiff>;
    /**
     * Generate diff between two states
     */
    private generateDiff;
    /**
     * Revert to a specific version
     *
     * @param versionId - Target version ID
     * @returns The new current version (revert creates a new version)
     * @emits versionReverted
     */
    revertToVersion(versionId: string): Promise<Version>;
    /**
     * Reconstruct the database state at a specific version
     *
     * @param versionId - Target version ID
     * @returns Reconstructed state
     */
    private reconstructStateAt;
    /**
     * Query the database state at a specific timestamp or version
     *
     * @param options - Query options
     * @returns Reconstructed state at the specified time/version
     */
    queryAtTimestamp(timestamp: number): Promise<any>;
    queryAtTimestamp(options: QueryOptions): Promise<any>;
    /**
     * Filter state by path pattern
     */
    private filterByPath;
    /**
     * Strip metadata from state
     */
    private stripMetadata;
    /**
     * Add tags to a version
     *
     * @param versionId - Version ID
     * @param tags - Tags to add
     */
    addTags(versionId: string, tags: string[]): void;
    /**
     * Get visualization data for change history
     *
     * @returns Visualization data
     */
    getVisualizationData(): VisualizationData;
    /**
     * Get audit log entries
     *
     * @param limit - Maximum number of entries to return
     * @returns Audit log entries
     */
    getAuditLog(limit?: number): AuditLogEntry[];
    /**
     * Log an audit entry
     */
    private logAudit;
    /**
     * Prune old versions to save space
     *
     * @param keepCount - Number of recent versions to keep
     * @param preserveTags - Tags to preserve regardless of age
     */
    pruneVersions(keepCount: number, preserveTags?: string[]): void;
    /**
     * Export all versions and audit log for backup
     *
     * @returns Serializable backup data
     */
    exportBackup(): {
        versions: Version[];
        auditLog: AuditLogEntry[];
        currentState: any;
        exportedAt: number;
    };
    /**
     * Import versions and state from backup
     *
     * @param backup - Backup data to import
     */
    importBackup(backup: ReturnType<typeof this.exportBackup>): void;
    /**
     * Get storage statistics
     *
     * @returns Storage statistics
     */
    getStorageStats(): {
        versionCount: number;
        totalChanges: number;
        auditLogSize: number;
        estimatedSizeBytes: number;
        oldestVersion: number;
        newestVersion: number;
    };
}
/**
 * Export singleton instance for convenience
 */
export declare const temporalTracker: TemporalTracker;
/**
 * Type guard for Change
 */
export declare function isChange(obj: any): obj is Change;
/**
 * Type guard for Version
 */
export declare function isVersion(obj: any): obj is Version;
//# sourceMappingURL=temporal.d.ts.map