/**
 * RVF Wrapper - Persistent vector store via @ruvector/rvf
 *
 * Wraps @ruvector/rvf RvfDatabase through thin convenience functions.
 * Falls back to clear error messages when the package is not installed.
 */
export declare function isRvfAvailable(): boolean;
export interface RvfStoreOptions {
    dimensions: number;
    metric?: 'l2' | 'cosine' | 'dotproduct';
    compression?: 'none' | 'scalar' | 'product';
    m?: number;
    efConstruction?: number;
}
export interface RvfEntry {
    id: string;
    vector: Float32Array | number[];
    metadata?: Record<string, any>;
}
export interface RvfResult {
    id: string;
    distance: number;
}
export interface RvfStoreStatus {
    totalVectors: number;
    totalSegments: number;
    fileSizeBytes: number;
    epoch: number;
    compactionState: string;
    deadSpaceRatio: number;
    readOnly: boolean;
}
export interface RvfQueryOpts {
    efSearch?: number;
    filter?: any;
    timeoutMs?: number;
}
export type RvfStore = any;
/**
 * Create a new RVF store at the given path.
 */
export declare function createRvfStore(path: string, options: RvfStoreOptions): Promise<RvfStore>;
/**
 * Open an existing RVF store for read-write access.
 */
export declare function openRvfStore(path: string): Promise<RvfStore>;
/**
 * Ingest a batch of vectors into an open store.
 */
export declare function rvfIngest(store: RvfStore, entries: RvfEntry[]): Promise<{
    accepted: number;
    rejected: number;
    epoch: number;
}>;
/**
 * Query for the k nearest neighbors.
 */
export declare function rvfQuery(store: RvfStore, vector: Float32Array | number[], k: number, options?: RvfQueryOpts): Promise<RvfResult[]>;
/**
 * Soft-delete vectors by their IDs.
 */
export declare function rvfDelete(store: RvfStore, ids: string[]): Promise<{
    deleted: number;
    epoch: number;
}>;
/**
 * Get the current store status.
 */
export declare function rvfStatus(store: RvfStore): Promise<RvfStoreStatus>;
/**
 * Run compaction to reclaim dead space.
 */
export declare function rvfCompact(store: RvfStore): Promise<{
    segmentsCompacted: number;
    bytesReclaimed: number;
    epoch: number;
}>;
/**
 * Derive a child store from a parent for lineage tracking.
 */
export declare function rvfDerive(store: RvfStore, childPath: string): Promise<RvfStore>;
/**
 * Close the store, releasing the writer lock and flushing data.
 */
export declare function rvfClose(store: RvfStore): Promise<void>;
//# sourceMappingURL=rvf-wrapper.d.ts.map