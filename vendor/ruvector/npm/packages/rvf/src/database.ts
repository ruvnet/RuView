import type {
  RvfOptions,
  RvfQueryOptions,
  RvfSearchResult,
  RvfIngestResult,
  RvfIngestEntry,
  RvfDeleteResult,
  RvfCompactionResult,
  RvfStatus,
  RvfFilterExpr,
  RvfKernelData,
  RvfEbpfData,
  RvfSegmentInfo,
  BackendType,
} from './types';
import type { RvfBackend } from './backend';
import { resolveBackend } from './backend';
import { RvfError, RvfErrorCode } from './errors';

/**
 * Main user-facing RVF database class.
 *
 * Wraps a backend implementation (`NodeBackend` or `WasmBackend`) and exposes
 * an ergonomic async API that mirrors the Rust `RvfStore` surface.
 *
 * Use the static factory methods (`create`, `open`, `openReadonly`) to obtain
 * an instance. Do not construct directly.
 */
export class RvfDatabase {
  private backend: RvfBackend;
  private closed = false;

  private constructor(backend: RvfBackend) {
    this.backend = backend;
  }

  // -----------------------------------------------------------------------
  // Factory methods
  // -----------------------------------------------------------------------

  /**
   * Create a new RVF store at `path`.
   *
   * @param path      File path for the new store.
   * @param options   Store creation options (dimensions is required).
   * @param backend   Backend to use. Default: `'auto'`.
   */
  static async create(
    path: string,
    options: RvfOptions,
    backend: BackendType = 'auto',
  ): Promise<RvfDatabase> {
    const impl = resolveBackend(backend);
    await impl.create(path, options);
    return new RvfDatabase(impl);
  }

  /**
   * Open an existing RVF store for read-write access.
   *
   * @param path      File path to an existing `.rvf` file.
   * @param backend   Backend to use. Default: `'auto'`.
   */
  static async open(
    path: string,
    backend: BackendType = 'auto',
  ): Promise<RvfDatabase> {
    const impl = resolveBackend(backend);
    await impl.open(path);
    return new RvfDatabase(impl);
  }

  /**
   * Open an existing RVF store for read-only access (no lock required).
   *
   * @param path      File path to an existing `.rvf` file.
   * @param backend   Backend to use. Default: `'auto'`.
   */
  static async openReadonly(
    path: string,
    backend: BackendType = 'auto',
  ): Promise<RvfDatabase> {
    const impl = resolveBackend(backend);
    await impl.openReadonly(path);
    return new RvfDatabase(impl);
  }

  /**
   * Create an RvfDatabase from an already-initialized backend.
   *
   * Used internally (e.g. by `derive()`) to wrap a child backend that was
   * created by the native layer without going through the normal open/create
   * flow.
   */
  static fromBackend(backend: RvfBackend): RvfDatabase {
    return new RvfDatabase(backend);
  }

  // -----------------------------------------------------------------------
  // Write operations
  // -----------------------------------------------------------------------

  /**
   * Ingest a batch of vectors into the store.
   *
   * @param entries  Array of `{ id, vector, metadata? }` entries.
   * @returns        Counts of accepted/rejected vectors and the new epoch.
   */
  async ingestBatch(entries: RvfIngestEntry[]): Promise<RvfIngestResult> {
    this.ensureOpen();
    return this.backend.ingestBatch(entries);
  }

  /**
   * Soft-delete vectors by their IDs.
   *
   * @param ids  Vector IDs to delete.
   */
  async delete(ids: string[]): Promise<RvfDeleteResult> {
    this.ensureOpen();
    return this.backend.delete(ids);
  }

  /**
   * Soft-delete all vectors matching a filter expression.
   *
   * @param filter  The filter to match against vector metadata.
   */
  async deleteByFilter(filter: RvfFilterExpr): Promise<RvfDeleteResult> {
    this.ensureOpen();
    return this.backend.deleteByFilter(filter);
  }

  // -----------------------------------------------------------------------
  // Read operations
  // -----------------------------------------------------------------------

  /**
   * Query for the `k` nearest neighbors of a given vector.
   *
   * @param vector   The query embedding.
   * @param k        Number of results to return.
   * @param options  Optional query parameters (efSearch, filter, timeout).
   * @returns        Sorted search results (closest first).
   */
  async query(
    vector: Float32Array | number[],
    k: number,
    options?: RvfQueryOptions,
  ): Promise<RvfSearchResult[]> {
    this.ensureOpen();
    const f32 = vector instanceof Float32Array ? vector : new Float32Array(vector);
    return this.backend.query(f32, k, options);
  }

  // -----------------------------------------------------------------------
  // Maintenance
  // -----------------------------------------------------------------------

  /**
   * Run compaction to reclaim dead space from soft-deleted vectors.
   */
  async compact(): Promise<RvfCompactionResult> {
    this.ensureOpen();
    return this.backend.compact();
  }

  /**
   * Get the current store status (vector count, file size, epoch, etc.).
   */
  async status(): Promise<RvfStatus> {
    this.ensureOpen();
    return this.backend.status();
  }

  // -----------------------------------------------------------------------
  // Lineage
  // -----------------------------------------------------------------------

  /** Get this file's unique identifier as a hex string. */
  async fileId(): Promise<string> {
    this.ensureOpen();
    return this.backend.fileId();
  }

  /** Get the parent file's identifier as a hex string (all zeros if root). */
  async parentId(): Promise<string> {
    this.ensureOpen();
    return this.backend.parentId();
  }

  /** Get the lineage depth (0 for root files). */
  async lineageDepth(): Promise<number> {
    this.ensureOpen();
    return this.backend.lineageDepth();
  }

  /**
   * Derive a child store from this parent.
   *
   * Creates a new RVF file at `childPath` that records this store as its
   * parent for provenance tracking. Returns a new `RvfDatabase` wrapping
   * the child store.
   */
  async derive(childPath: string, options?: RvfOptions): Promise<RvfDatabase> {
    this.ensureOpen();
    const childBackend = await this.backend.derive(childPath, options);
    return RvfDatabase.fromBackend(childBackend);
  }

  // -----------------------------------------------------------------------
  // Kernel / eBPF
  // -----------------------------------------------------------------------

  /** Embed a kernel image. Returns the segment ID. */
  async embedKernel(
    arch: number, kernelType: number, flags: number,
    image: Uint8Array, apiPort: number, cmdline?: string,
  ): Promise<number> {
    this.ensureOpen();
    return this.backend.embedKernel(arch, kernelType, flags, image, apiPort, cmdline);
  }

  /** Extract the kernel image. Returns null if not present. */
  async extractKernel(): Promise<RvfKernelData | null> {
    this.ensureOpen();
    return this.backend.extractKernel();
  }

  /** Embed an eBPF program. Returns the segment ID. */
  async embedEbpf(
    programType: number, attachType: number, maxDimension: number,
    bytecode: Uint8Array, btf?: Uint8Array,
  ): Promise<number> {
    this.ensureOpen();
    return this.backend.embedEbpf(programType, attachType, maxDimension, bytecode, btf);
  }

  /** Extract the eBPF program. Returns null if not present. */
  async extractEbpf(): Promise<RvfEbpfData | null> {
    this.ensureOpen();
    return this.backend.extractEbpf();
  }

  // -----------------------------------------------------------------------
  // Inspection
  // -----------------------------------------------------------------------

  /** Get the list of segments in the store. */
  async segments(): Promise<RvfSegmentInfo[]> {
    this.ensureOpen();
    return this.backend.segments();
  }

  /** Get the vector dimensionality. */
  async dimension(): Promise<number> {
    this.ensureOpen();
    return this.backend.dimension();
  }

  // -----------------------------------------------------------------------
  // Lifecycle
  // -----------------------------------------------------------------------

  /**
   * Close the store, releasing the writer lock and flushing pending data.
   *
   * After calling `close()`, all other methods will throw `RvfError` with
   * code `StoreClosed`.
   */
  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    await this.backend.close();
  }

  /** True if the store has been closed. */
  get isClosed(): boolean {
    return this.closed;
  }

  // -----------------------------------------------------------------------
  // Internal
  // -----------------------------------------------------------------------

  private ensureOpen(): void {
    if (this.closed) {
      throw new RvfError(RvfErrorCode.StoreClosed);
    }
  }
}
