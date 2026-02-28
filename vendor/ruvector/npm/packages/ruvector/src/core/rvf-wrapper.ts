/**
 * RVF Wrapper - Persistent vector store via @ruvector/rvf
 *
 * Wraps @ruvector/rvf RvfDatabase through thin convenience functions.
 * Falls back to clear error messages when the package is not installed.
 */

let rvfModule: any = null;
let loadError: Error | null = null;

function getRvfModule() {
  if (rvfModule) return rvfModule;
  if (loadError) throw loadError;

  try {
    rvfModule = require('@ruvector/rvf');
    return rvfModule;
  } catch (e: any) {
    loadError = new Error(
      '@ruvector/rvf is not installed. Run: npm install @ruvector/rvf'
    );
    throw loadError;
  }
}

export function isRvfAvailable(): boolean {
  try {
    getRvfModule();
    return true;
  } catch {
    return false;
  }
}

// ---------------------------------------------------------------------------
// Minimal inline types (mirrors @ruvector/rvf/types when package absent)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Store handle (opaque to callers)
// ---------------------------------------------------------------------------

export type RvfStore = any;

// ---------------------------------------------------------------------------
// Wrapper functions
// ---------------------------------------------------------------------------

/**
 * Create a new RVF store at the given path.
 */
export async function createRvfStore(
  path: string,
  options: RvfStoreOptions,
): Promise<RvfStore> {
  const mod = getRvfModule();
  return mod.RvfDatabase.create(path, options);
}

/**
 * Open an existing RVF store for read-write access.
 */
export async function openRvfStore(path: string): Promise<RvfStore> {
  const mod = getRvfModule();
  return mod.RvfDatabase.open(path);
}

/**
 * Ingest a batch of vectors into an open store.
 */
export async function rvfIngest(
  store: RvfStore,
  entries: RvfEntry[],
): Promise<{ accepted: number; rejected: number; epoch: number }> {
  return store.ingestBatch(entries);
}

/**
 * Query for the k nearest neighbors.
 */
export async function rvfQuery(
  store: RvfStore,
  vector: Float32Array | number[],
  k: number,
  options?: RvfQueryOpts,
): Promise<RvfResult[]> {
  return store.query(vector, k, options);
}

/**
 * Soft-delete vectors by their IDs.
 */
export async function rvfDelete(
  store: RvfStore,
  ids: string[],
): Promise<{ deleted: number; epoch: number }> {
  return store.delete(ids);
}

/**
 * Get the current store status.
 */
export async function rvfStatus(store: RvfStore): Promise<RvfStoreStatus> {
  return store.status();
}

/**
 * Run compaction to reclaim dead space.
 */
export async function rvfCompact(
  store: RvfStore,
): Promise<{ segmentsCompacted: number; bytesReclaimed: number; epoch: number }> {
  return store.compact();
}

/**
 * Derive a child store from a parent for lineage tracking.
 */
export async function rvfDerive(
  store: RvfStore,
  childPath: string,
): Promise<RvfStore> {
  return store.derive(childPath);
}

/**
 * Close the store, releasing the writer lock and flushing data.
 */
export async function rvfClose(store: RvfStore): Promise<void> {
  return store.close();
}
