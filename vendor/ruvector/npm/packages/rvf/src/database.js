"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.RvfDatabase = void 0;
const backend_1 = require("./backend");
const errors_1 = require("./errors");
/**
 * Main user-facing RVF database class.
 *
 * Wraps a backend implementation (`NodeBackend` or `WasmBackend`) and exposes
 * an ergonomic async API that mirrors the Rust `RvfStore` surface.
 *
 * Use the static factory methods (`create`, `open`, `openReadonly`) to obtain
 * an instance. Do not construct directly.
 */
class RvfDatabase {
    constructor(backend) {
        this.closed = false;
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
    static async create(path, options, backend = 'auto') {
        const impl = (0, backend_1.resolveBackend)(backend);
        await impl.create(path, options);
        return new RvfDatabase(impl);
    }
    /**
     * Open an existing RVF store for read-write access.
     *
     * @param path      File path to an existing `.rvf` file.
     * @param backend   Backend to use. Default: `'auto'`.
     */
    static async open(path, backend = 'auto') {
        const impl = (0, backend_1.resolveBackend)(backend);
        await impl.open(path);
        return new RvfDatabase(impl);
    }
    /**
     * Open an existing RVF store for read-only access (no lock required).
     *
     * @param path      File path to an existing `.rvf` file.
     * @param backend   Backend to use. Default: `'auto'`.
     */
    static async openReadonly(path, backend = 'auto') {
        const impl = (0, backend_1.resolveBackend)(backend);
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
    static fromBackend(backend) {
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
    async ingestBatch(entries) {
        this.ensureOpen();
        return this.backend.ingestBatch(entries);
    }
    /**
     * Soft-delete vectors by their IDs.
     *
     * @param ids  Vector IDs to delete.
     */
    async delete(ids) {
        this.ensureOpen();
        return this.backend.delete(ids);
    }
    /**
     * Soft-delete all vectors matching a filter expression.
     *
     * @param filter  The filter to match against vector metadata.
     */
    async deleteByFilter(filter) {
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
    async query(vector, k, options) {
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
    async compact() {
        this.ensureOpen();
        return this.backend.compact();
    }
    /**
     * Get the current store status (vector count, file size, epoch, etc.).
     */
    async status() {
        this.ensureOpen();
        return this.backend.status();
    }
    // -----------------------------------------------------------------------
    // Lineage
    // -----------------------------------------------------------------------
    /** Get this file's unique identifier as a hex string. */
    async fileId() {
        this.ensureOpen();
        return this.backend.fileId();
    }
    /** Get the parent file's identifier as a hex string (all zeros if root). */
    async parentId() {
        this.ensureOpen();
        return this.backend.parentId();
    }
    /** Get the lineage depth (0 for root files). */
    async lineageDepth() {
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
    async derive(childPath, options) {
        this.ensureOpen();
        const childBackend = await this.backend.derive(childPath, options);
        return RvfDatabase.fromBackend(childBackend);
    }
    // -----------------------------------------------------------------------
    // Kernel / eBPF
    // -----------------------------------------------------------------------
    /** Embed a kernel image. Returns the segment ID. */
    async embedKernel(arch, kernelType, flags, image, apiPort, cmdline) {
        this.ensureOpen();
        return this.backend.embedKernel(arch, kernelType, flags, image, apiPort, cmdline);
    }
    /** Extract the kernel image. Returns null if not present. */
    async extractKernel() {
        this.ensureOpen();
        return this.backend.extractKernel();
    }
    /** Embed an eBPF program. Returns the segment ID. */
    async embedEbpf(programType, attachType, maxDimension, bytecode, btf) {
        this.ensureOpen();
        return this.backend.embedEbpf(programType, attachType, maxDimension, bytecode, btf);
    }
    /** Extract the eBPF program. Returns null if not present. */
    async extractEbpf() {
        this.ensureOpen();
        return this.backend.extractEbpf();
    }
    // -----------------------------------------------------------------------
    // Inspection
    // -----------------------------------------------------------------------
    /** Get the list of segments in the store. */
    async segments() {
        this.ensureOpen();
        return this.backend.segments();
    }
    /** Get the vector dimensionality. */
    async dimension() {
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
    async close() {
        if (this.closed)
            return;
        this.closed = true;
        await this.backend.close();
    }
    /** True if the store has been closed. */
    get isClosed() {
        return this.closed;
    }
    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------
    ensureOpen() {
        if (this.closed) {
            throw new errors_1.RvfError(errors_1.RvfErrorCode.StoreClosed);
        }
    }
}
exports.RvfDatabase = RvfDatabase;
//# sourceMappingURL=database.js.map