import type { RvfOptions, RvfQueryOptions, RvfSearchResult, RvfIngestResult, RvfIngestEntry, RvfDeleteResult, RvfCompactionResult, RvfStatus, RvfFilterExpr, RvfKernelData, RvfEbpfData, RvfSegmentInfo, BackendType } from './types';
/**
 * Abstract backend that wraps either the native (N-API) or WASM build of
 * rvf-runtime.  The `RvfDatabase` class delegates all I/O to a backend
 * instance, keeping the public API identical regardless of runtime.
 */
export interface RvfBackend {
    /** Create a new store file at `path` with the given options. */
    create(path: string, options: RvfOptions): Promise<void>;
    /** Open an existing store at `path` for read-write access. */
    open(path: string): Promise<void>;
    /** Open an existing store at `path` for read-only access. */
    openReadonly(path: string): Promise<void>;
    /** Ingest a batch of vectors. */
    ingestBatch(entries: RvfIngestEntry[]): Promise<RvfIngestResult>;
    /** Query the k nearest neighbors. */
    query(vector: Float32Array, k: number, options?: RvfQueryOptions): Promise<RvfSearchResult[]>;
    /** Soft-delete vectors by ID. */
    delete(ids: string[]): Promise<RvfDeleteResult>;
    /** Soft-delete vectors matching a filter. */
    deleteByFilter(filter: RvfFilterExpr): Promise<RvfDeleteResult>;
    /** Run compaction to reclaim dead space. */
    compact(): Promise<RvfCompactionResult>;
    /** Get the current store status. */
    status(): Promise<RvfStatus>;
    /** Close the store, releasing locks. */
    close(): Promise<void>;
    fileId(): Promise<string>;
    parentId(): Promise<string>;
    lineageDepth(): Promise<number>;
    derive(childPath: string, options?: RvfOptions): Promise<RvfBackend>;
    embedKernel(arch: number, kernelType: number, flags: number, image: Uint8Array, apiPort: number, cmdline?: string): Promise<number>;
    extractKernel(): Promise<RvfKernelData | null>;
    embedEbpf(programType: number, attachType: number, maxDimension: number, bytecode: Uint8Array, btf?: Uint8Array): Promise<number>;
    extractEbpf(): Promise<RvfEbpfData | null>;
    segments(): Promise<RvfSegmentInfo[]>;
    dimension(): Promise<number>;
}
/**
 * Backend that delegates to the `@ruvector/rvf-node` native N-API addon.
 *
 * The native addon is loaded lazily on first use so that the SDK package can
 * be imported in environments where the native build is unavailable (e.g.
 * browsers) without throwing at import time.
 */
export declare class NodeBackend implements RvfBackend {
    private native;
    private handle;
    private idToLabel;
    private labelToId;
    private nextLabel;
    private storePath;
    private loadNative;
    private ensureHandle;
    create(path: string, options: RvfOptions): Promise<void>;
    open(path: string): Promise<void>;
    openReadonly(path: string): Promise<void>;
    ingestBatch(entries: RvfIngestEntry[]): Promise<RvfIngestResult>;
    query(vector: Float32Array, k: number, options?: RvfQueryOptions): Promise<RvfSearchResult[]>;
    delete(ids: string[]): Promise<RvfDeleteResult>;
    deleteByFilter(filter: RvfFilterExpr): Promise<RvfDeleteResult>;
    compact(): Promise<RvfCompactionResult>;
    status(): Promise<RvfStatus>;
    close(): Promise<void>;
    fileId(): Promise<string>;
    parentId(): Promise<string>;
    lineageDepth(): Promise<number>;
    derive(childPath: string, options?: RvfOptions): Promise<RvfBackend>;
    embedKernel(arch: number, kernelType: number, flags: number, image: Uint8Array, apiPort: number, cmdline?: string): Promise<number>;
    extractKernel(): Promise<RvfKernelData | null>;
    embedEbpf(programType: number, attachType: number, maxDimension: number, bytecode: Uint8Array, btf?: Uint8Array): Promise<number>;
    extractEbpf(): Promise<RvfEbpfData | null>;
    segments(): Promise<RvfSegmentInfo[]>;
    dimension(): Promise<number>;
    /**
     * Get or allocate a numeric label for a string ID.
     * If the ID was already seen, returns the existing label.
     */
    private resolveLabel;
    /** Path to the sidecar mappings file. */
    private mappingsPath;
    /** Persist the string↔label mapping to a sidecar JSON file. */
    private saveMappings;
    /** Load the string↔label mapping from the sidecar JSON file if it exists. */
    private loadMappings;
}
/**
 * Backend that delegates to the `@ruvector/rvf-wasm` WASM build.
 *
 * The WASM microkernel exposes C-ABI store functions (`rvf_store_create`,
 * `rvf_store_query`, etc.) operating on integer handles. This backend wraps
 * them behind the same `RvfBackend` interface.
 *
 * Suitable for browser environments. The WASM module is loaded lazily.
 */
export declare class WasmBackend implements RvfBackend {
    private wasm;
    /** Integer store handle returned by `rvf_store_create` / `rvf_store_open`. */
    private handle;
    private dim;
    private loadWasm;
    private ensureHandle;
    private metricCode;
    create(_path: string, options: RvfOptions): Promise<void>;
    open(_path: string): Promise<void>;
    openReadonly(_path: string): Promise<void>;
    ingestBatch(entries: RvfIngestEntry[]): Promise<RvfIngestResult>;
    query(vector: Float32Array, k: number, _options?: RvfQueryOptions): Promise<RvfSearchResult[]>;
    delete(ids: string[]): Promise<RvfDeleteResult>;
    deleteByFilter(_filter: RvfFilterExpr): Promise<RvfDeleteResult>;
    compact(): Promise<RvfCompactionResult>;
    status(): Promise<RvfStatus>;
    close(): Promise<void>;
    fileId(): Promise<string>;
    parentId(): Promise<string>;
    lineageDepth(): Promise<number>;
    derive(_childPath: string, _options?: RvfOptions): Promise<RvfBackend>;
    embedKernel(): Promise<number>;
    extractKernel(): Promise<RvfKernelData | null>;
    embedEbpf(): Promise<number>;
    extractEbpf(): Promise<RvfEbpfData | null>;
    segments(): Promise<RvfSegmentInfo[]>;
    dimension(): Promise<number>;
}
/**
 * Resolve a `BackendType` to a concrete `RvfBackend` instance.
 *
 * - `'node'`  Always returns a `NodeBackend`.
 * - `'wasm'`  Always returns a `WasmBackend`.
 * - `'auto'`  Tries `node` first, falls back to `wasm`.
 */
export declare function resolveBackend(type: BackendType): RvfBackend;
//# sourceMappingURL=backend.d.ts.map