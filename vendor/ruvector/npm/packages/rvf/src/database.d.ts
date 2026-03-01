import type { RvfOptions, RvfQueryOptions, RvfSearchResult, RvfIngestResult, RvfIngestEntry, RvfDeleteResult, RvfCompactionResult, RvfStatus, RvfFilterExpr, RvfKernelData, RvfEbpfData, RvfSegmentInfo, BackendType } from './types';
import type { RvfBackend } from './backend';
/**
 * Main user-facing RVF database class.
 *
 * Wraps a backend implementation (`NodeBackend` or `WasmBackend`) and exposes
 * an ergonomic async API that mirrors the Rust `RvfStore` surface.
 *
 * Use the static factory methods (`create`, `open`, `openReadonly`) to obtain
 * an instance. Do not construct directly.
 */
export declare class RvfDatabase {
    private backend;
    private closed;
    private constructor();
    /**
     * Create a new RVF store at `path`.
     *
     * @param path      File path for the new store.
     * @param options   Store creation options (dimensions is required).
     * @param backend   Backend to use. Default: `'auto'`.
     */
    static create(path: string, options: RvfOptions, backend?: BackendType): Promise<RvfDatabase>;
    /**
     * Open an existing RVF store for read-write access.
     *
     * @param path      File path to an existing `.rvf` file.
     * @param backend   Backend to use. Default: `'auto'`.
     */
    static open(path: string, backend?: BackendType): Promise<RvfDatabase>;
    /**
     * Open an existing RVF store for read-only access (no lock required).
     *
     * @param path      File path to an existing `.rvf` file.
     * @param backend   Backend to use. Default: `'auto'`.
     */
    static openReadonly(path: string, backend?: BackendType): Promise<RvfDatabase>;
    /**
     * Create an RvfDatabase from an already-initialized backend.
     *
     * Used internally (e.g. by `derive()`) to wrap a child backend that was
     * created by the native layer without going through the normal open/create
     * flow.
     */
    static fromBackend(backend: RvfBackend): RvfDatabase;
    /**
     * Ingest a batch of vectors into the store.
     *
     * @param entries  Array of `{ id, vector, metadata? }` entries.
     * @returns        Counts of accepted/rejected vectors and the new epoch.
     */
    ingestBatch(entries: RvfIngestEntry[]): Promise<RvfIngestResult>;
    /**
     * Soft-delete vectors by their IDs.
     *
     * @param ids  Vector IDs to delete.
     */
    delete(ids: string[]): Promise<RvfDeleteResult>;
    /**
     * Soft-delete all vectors matching a filter expression.
     *
     * @param filter  The filter to match against vector metadata.
     */
    deleteByFilter(filter: RvfFilterExpr): Promise<RvfDeleteResult>;
    /**
     * Query for the `k` nearest neighbors of a given vector.
     *
     * @param vector   The query embedding.
     * @param k        Number of results to return.
     * @param options  Optional query parameters (efSearch, filter, timeout).
     * @returns        Sorted search results (closest first).
     */
    query(vector: Float32Array | number[], k: number, options?: RvfQueryOptions): Promise<RvfSearchResult[]>;
    /**
     * Run compaction to reclaim dead space from soft-deleted vectors.
     */
    compact(): Promise<RvfCompactionResult>;
    /**
     * Get the current store status (vector count, file size, epoch, etc.).
     */
    status(): Promise<RvfStatus>;
    /** Get this file's unique identifier as a hex string. */
    fileId(): Promise<string>;
    /** Get the parent file's identifier as a hex string (all zeros if root). */
    parentId(): Promise<string>;
    /** Get the lineage depth (0 for root files). */
    lineageDepth(): Promise<number>;
    /**
     * Derive a child store from this parent.
     *
     * Creates a new RVF file at `childPath` that records this store as its
     * parent for provenance tracking. Returns a new `RvfDatabase` wrapping
     * the child store.
     */
    derive(childPath: string, options?: RvfOptions): Promise<RvfDatabase>;
    /** Embed a kernel image. Returns the segment ID. */
    embedKernel(arch: number, kernelType: number, flags: number, image: Uint8Array, apiPort: number, cmdline?: string): Promise<number>;
    /** Extract the kernel image. Returns null if not present. */
    extractKernel(): Promise<RvfKernelData | null>;
    /** Embed an eBPF program. Returns the segment ID. */
    embedEbpf(programType: number, attachType: number, maxDimension: number, bytecode: Uint8Array, btf?: Uint8Array): Promise<number>;
    /** Extract the eBPF program. Returns null if not present. */
    extractEbpf(): Promise<RvfEbpfData | null>;
    /** Get the list of segments in the store. */
    segments(): Promise<RvfSegmentInfo[]>;
    /** Get the vector dimensionality. */
    dimension(): Promise<number>;
    /**
     * Close the store, releasing the writer lock and flushing pending data.
     *
     * After calling `close()`, all other methods will throw `RvfError` with
     * code `StoreClosed`.
     */
    close(): Promise<void>;
    /** True if the store has been closed. */
    get isClosed(): boolean;
    private ensureOpen;
}
//# sourceMappingURL=database.d.ts.map