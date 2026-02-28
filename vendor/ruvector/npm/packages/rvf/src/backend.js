"use strict";
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
exports.WasmBackend = exports.NodeBackend = void 0;
exports.resolveBackend = resolveBackend;
const errors_1 = require("./errors");
// ---------------------------------------------------------------------------
// NodeBackend — wraps @ruvector/rvf-node (N-API)
// ---------------------------------------------------------------------------
/**
 * Backend that delegates to the `@ruvector/rvf-node` native N-API addon.
 *
 * The native addon is loaded lazily on first use so that the SDK package can
 * be imported in environments where the native build is unavailable (e.g.
 * browsers) without throwing at import time.
 */
class NodeBackend {
    constructor() {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        this.native = null;
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        this.handle = null;
        // String ID <-> Numeric Label mappings (N-API layer requires i64 labels)
        this.idToLabel = new Map();
        this.labelToId = new Map();
        this.nextLabel = 1; // RVF uses 1-based labels
        this.storePath = '';
    }
    async loadNative() {
        if (this.native)
            return;
        try {
            // Dynamic import so the SDK can be bundled for browsers without
            // pulling in the native addon at compile time.
            // The NAPI addon exports a `RvfDatabase` class with factory methods.
            const mod = await Promise.resolve().then(() => __importStar(require('@ruvector/rvf-node')));
            this.native = mod.RvfDatabase ?? mod.default?.RvfDatabase ?? mod;
        }
        catch {
            throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'Could not load @ruvector/rvf-node — is it installed?');
        }
    }
    ensureHandle() {
        if (!this.handle) {
            throw new errors_1.RvfError(errors_1.RvfErrorCode.StoreClosed);
        }
    }
    async create(path, options) {
        await this.loadNative();
        try {
            this.handle = await this.native.create(path, mapOptionsToNative(options));
            this.storePath = path;
            this.idToLabel.clear();
            this.labelToId.clear();
            this.nextLabel = 1;
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async open(path) {
        await this.loadNative();
        try {
            this.handle = await this.native.open(path);
            this.storePath = path;
            await this.loadMappings();
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async openReadonly(path) {
        await this.loadNative();
        try {
            this.handle = await this.native.openReadonly(path);
            this.storePath = path;
            await this.loadMappings();
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async ingestBatch(entries) {
        this.ensureHandle();
        try {
            // NAPI signature: ingestBatch(vectors: Float32Array, ids: i64[], metadata?)
            // Flatten individual vectors into a single contiguous Float32Array.
            const n = entries.length;
            if (n === 0)
                return { accepted: 0, rejected: 0, epoch: 0 };
            const first = entries[0].vector;
            const dim = first instanceof Float32Array ? first.length : first.length;
            const flat = new Float32Array(n * dim);
            for (let i = 0; i < n; i++) {
                const v = entries[i].vector;
                const f32 = v instanceof Float32Array ? v : new Float32Array(v);
                flat.set(f32, i * dim);
            }
            // Map string IDs to numeric labels for the N-API layer.
            // The native Rust HNSW expects i64 labels — non-numeric strings cause
            // silent data loss (NaN → dropped).  We maintain a bidirectional
            // string↔label mapping and persist it as a sidecar JSON file.
            const ids = entries.map((e) => this.resolveLabel(e.id));
            const result = this.handle.ingestBatch(flat, ids);
            // Persist mappings after every ingest so they survive crashes.
            await this.saveMappings();
            return {
                accepted: Number(result.accepted),
                rejected: Number(result.rejected),
                epoch: result.epoch,
            };
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async query(vector, k, options) {
        this.ensureHandle();
        try {
            const nativeOpts = options ? mapQueryOptionsToNative(options) : undefined;
            const results = this.handle.query(vector, k, nativeOpts);
            // Map numeric labels back to original string IDs.
            return results.map((r) => ({
                id: this.labelToId.get(Number(r.id)) ?? String(r.id),
                distance: r.distance,
            }));
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async delete(ids) {
        this.ensureHandle();
        try {
            // Resolve string IDs to numeric labels for the N-API layer.
            const numIds = ids
                .map((id) => this.idToLabel.get(id))
                .filter((label) => label !== undefined);
            if (numIds.length === 0) {
                return { deleted: 0, epoch: 0 };
            }
            const result = this.handle.delete(numIds);
            // Remove deleted entries from the mapping.
            for (const id of ids) {
                const label = this.idToLabel.get(id);
                if (label !== undefined) {
                    this.idToLabel.delete(id);
                    this.labelToId.delete(label);
                }
            }
            await this.saveMappings();
            return { deleted: Number(result.deleted), epoch: result.epoch };
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async deleteByFilter(filter) {
        this.ensureHandle();
        try {
            // NAPI takes a JSON string for the filter expression.
            const result = this.handle.deleteByFilter(JSON.stringify(filter));
            return { deleted: Number(result.deleted), epoch: result.epoch };
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async compact() {
        this.ensureHandle();
        try {
            const result = this.handle.compact();
            return {
                segmentsCompacted: result.segmentsCompacted ?? result.segments_compacted,
                bytesReclaimed: Number(result.bytesReclaimed ?? result.bytes_reclaimed),
                epoch: result.epoch,
            };
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async status() {
        this.ensureHandle();
        try {
            const s = this.handle.status();
            return mapNativeStatus(s);
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async close() {
        if (!this.handle)
            return;
        try {
            await this.saveMappings();
            this.handle.close();
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
        finally {
            this.handle = null;
            this.idToLabel.clear();
            this.labelToId.clear();
            this.nextLabel = 1;
            this.storePath = '';
        }
    }
    async fileId() {
        this.ensureHandle();
        try {
            return this.handle.fileId();
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async parentId() {
        this.ensureHandle();
        try {
            return this.handle.parentId();
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async lineageDepth() {
        this.ensureHandle();
        try {
            return this.handle.lineageDepth();
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async derive(childPath, options) {
        this.ensureHandle();
        try {
            const nativeOpts = options ? mapOptionsToNative(options) : undefined;
            const childHandle = this.handle.derive(childPath, nativeOpts);
            const child = new NodeBackend();
            child.native = this.native;
            child.handle = childHandle;
            child.storePath = childPath;
            // Copy parent mappings to child (COW semantics)
            child.idToLabel = new Map(this.idToLabel);
            child.labelToId = new Map(this.labelToId);
            child.nextLabel = this.nextLabel;
            await child.saveMappings();
            return child;
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async embedKernel(arch, kernelType, flags, image, apiPort, cmdline) {
        this.ensureHandle();
        try {
            return this.handle.embedKernel(arch, kernelType, flags, Buffer.from(image), apiPort, cmdline);
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async extractKernel() {
        this.ensureHandle();
        try {
            const result = this.handle.extractKernel();
            if (!result)
                return null;
            return {
                header: new Uint8Array(result.header),
                image: new Uint8Array(result.image),
            };
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async embedEbpf(programType, attachType, maxDimension, bytecode, btf) {
        this.ensureHandle();
        try {
            return this.handle.embedEbpf(programType, attachType, maxDimension, Buffer.from(bytecode), btf ? Buffer.from(btf) : undefined);
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async extractEbpf() {
        this.ensureHandle();
        try {
            const result = this.handle.extractEbpf();
            if (!result)
                return null;
            return {
                header: new Uint8Array(result.header),
                payload: new Uint8Array(result.payload),
            };
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async segments() {
        this.ensureHandle();
        try {
            const segs = this.handle.segments();
            return segs.map((s) => ({
                id: s.id,
                offset: s.offset,
                payloadLength: s.payloadLength ?? s.payload_length,
                segType: s.segType ?? s.seg_type,
            }));
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async dimension() {
        this.ensureHandle();
        try {
            return this.handle.dimension();
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    // ─── String ID ↔ Numeric Label mapping helpers ───
    /**
     * Get or allocate a numeric label for a string ID.
     * If the ID was already seen, returns the existing label.
     */
    resolveLabel(id) {
        let label = this.idToLabel.get(id);
        if (label !== undefined)
            return label;
        label = this.nextLabel++;
        this.idToLabel.set(id, label);
        this.labelToId.set(label, id);
        return label;
    }
    /** Path to the sidecar mappings file. */
    mappingsPath() {
        return this.storePath ? this.storePath + '.idmap.json' : '';
    }
    /** Persist the string↔label mapping to a sidecar JSON file. */
    async saveMappings() {
        const mp = this.mappingsPath();
        if (!mp)
            return;
        try {
            const fs = await Promise.resolve().then(() => __importStar(require('fs')));
            const data = JSON.stringify({
                idToLabel: Object.fromEntries(this.idToLabel),
                labelToId: Object.fromEntries(Array.from(this.labelToId.entries()).map(([k, v]) => [String(k), v])),
                nextLabel: this.nextLabel,
            });
            fs.writeFileSync(mp, data, 'utf-8');
        }
        catch {
            // Non-fatal: mapping persistence is best-effort (e.g. read-only FS).
        }
    }
    /** Load the string↔label mapping from the sidecar JSON file if it exists. */
    async loadMappings() {
        const mp = this.mappingsPath();
        if (!mp)
            return;
        try {
            const fs = await Promise.resolve().then(() => __importStar(require('fs')));
            if (!fs.existsSync(mp))
                return;
            const raw = JSON.parse(fs.readFileSync(mp, 'utf-8'));
            this.idToLabel = new Map(Object.entries(raw.idToLabel ?? {}).map(([k, v]) => [k, Number(v)]));
            this.labelToId = new Map(Object.entries(raw.labelToId ?? {}).map(([k, v]) => [Number(k), v]));
            this.nextLabel = raw.nextLabel ?? this.idToLabel.size + 1;
        }
        catch {
            // Non-fatal: start with empty mappings.
        }
    }
}
exports.NodeBackend = NodeBackend;
// ---------------------------------------------------------------------------
// WasmBackend — wraps @ruvector/rvf-wasm
// ---------------------------------------------------------------------------
/**
 * Backend that delegates to the `@ruvector/rvf-wasm` WASM build.
 *
 * The WASM microkernel exposes C-ABI store functions (`rvf_store_create`,
 * `rvf_store_query`, etc.) operating on integer handles. This backend wraps
 * them behind the same `RvfBackend` interface.
 *
 * Suitable for browser environments. The WASM module is loaded lazily.
 */
class WasmBackend {
    constructor() {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        this.wasm = null;
        /** Integer store handle returned by `rvf_store_create` / `rvf_store_open`. */
        this.handle = 0;
        this.dim = 0;
    }
    async loadWasm() {
        if (this.wasm)
            return;
        try {
            const mod = await Promise.resolve().then(() => __importStar(require('@ruvector/rvf-wasm')));
            // wasm-pack default export is the init function
            if (typeof mod.default === 'function') {
                this.wasm = await mod.default();
            }
            else {
                this.wasm = mod;
            }
        }
        catch {
            throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'Could not load @ruvector/rvf-wasm — is it installed?');
        }
    }
    ensureHandle() {
        if (!this.handle) {
            throw new errors_1.RvfError(errors_1.RvfErrorCode.StoreClosed);
        }
    }
    metricCode(metric) {
        switch (metric) {
            case 'Cosine': return 2;
            case 'InnerProduct': return 1;
            default: return 0; // L2
        }
    }
    async create(_path, options) {
        await this.loadWasm();
        try {
            const nativeOpts = mapOptionsToNative(options);
            const dim = nativeOpts.dimension;
            const metric = this.metricCode(nativeOpts.metric);
            const h = this.wasm.rvf_store_create(dim, metric);
            if (h <= 0)
                throw new Error('rvf_store_create returned ' + h);
            this.handle = h;
            this.dim = dim;
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async open(_path) {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'WASM backend does not support file-based open (in-memory only)');
    }
    async openReadonly(_path) {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'WASM backend does not support file-based openReadonly (in-memory only)');
    }
    async ingestBatch(entries) {
        this.ensureHandle();
        try {
            const n = entries.length;
            if (n === 0)
                return { accepted: 0, rejected: 0, epoch: 0 };
            const dim = this.dim || (entries[0].vector instanceof Float32Array
                ? entries[0].vector.length : entries[0].vector.length);
            const flat = new Float32Array(n * dim);
            const ids = new BigUint64Array(n);
            for (let i = 0; i < n; i++) {
                const v = entries[i].vector;
                const f32 = v instanceof Float32Array ? v : new Float32Array(v);
                flat.set(f32, i * dim);
                ids[i] = BigInt(entries[i].id);
            }
            // Allocate in WASM memory and call
            const vecsPtr = this.wasm.rvf_alloc(flat.byteLength);
            const idsPtr = this.wasm.rvf_alloc(ids.byteLength);
            new Float32Array(this.wasm.memory.buffer, vecsPtr, flat.length).set(flat);
            new BigUint64Array(this.wasm.memory.buffer, idsPtr, ids.length).set(ids);
            const accepted = this.wasm.rvf_store_ingest(this.handle, vecsPtr, idsPtr, n);
            this.wasm.rvf_free(vecsPtr, flat.byteLength);
            this.wasm.rvf_free(idsPtr, ids.byteLength);
            return { accepted: accepted > 0 ? accepted : 0, rejected: accepted < 0 ? n : 0, epoch: 0 };
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async query(vector, k, _options) {
        this.ensureHandle();
        try {
            const queryPtr = this.wasm.rvf_alloc(vector.byteLength);
            new Float32Array(this.wasm.memory.buffer, queryPtr, vector.length).set(vector);
            // Each result = 8 bytes id + 4 bytes dist = 12 bytes
            const outSize = k * 12;
            const outPtr = this.wasm.rvf_alloc(outSize);
            const count = this.wasm.rvf_store_query(this.handle, queryPtr, k, 0, outPtr);
            const results = [];
            const view = new DataView(this.wasm.memory.buffer);
            for (let i = 0; i < count; i++) {
                const off = outPtr + i * 12;
                const id = view.getBigUint64(off, true);
                const dist = view.getFloat32(off + 8, true);
                results.push({ id: String(id), distance: dist });
            }
            this.wasm.rvf_free(queryPtr, vector.byteLength);
            this.wasm.rvf_free(outPtr, outSize);
            return results;
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async delete(ids) {
        this.ensureHandle();
        try {
            const arr = new BigUint64Array(ids.map((id) => BigInt(id)));
            const ptr = this.wasm.rvf_alloc(arr.byteLength);
            new BigUint64Array(this.wasm.memory.buffer, ptr, arr.length).set(arr);
            const deleted = this.wasm.rvf_store_delete(this.handle, ptr, ids.length);
            this.wasm.rvf_free(ptr, arr.byteLength);
            return { deleted: deleted > 0 ? deleted : 0, epoch: 0 };
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async deleteByFilter(_filter) {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'deleteByFilter not supported in WASM backend');
    }
    async compact() {
        return { segmentsCompacted: 0, bytesReclaimed: 0, epoch: 0 };
    }
    async status() {
        this.ensureHandle();
        try {
            const outPtr = this.wasm.rvf_alloc(20);
            this.wasm.rvf_store_status(this.handle, outPtr);
            const view = new DataView(this.wasm.memory.buffer);
            const totalVectors = view.getUint32(outPtr, true);
            const dim = view.getUint32(outPtr + 4, true);
            this.wasm.rvf_free(outPtr, 20);
            return {
                totalVectors,
                totalSegments: 1,
                fileSizeBytes: 0,
                epoch: 0,
                profileId: 0,
                compactionState: 'idle',
                deadSpaceRatio: 0,
                readOnly: false,
            };
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
    }
    async close() {
        if (!this.handle)
            return;
        try {
            this.wasm.rvf_store_close(this.handle);
        }
        catch (err) {
            throw errors_1.RvfError.fromNative(err);
        }
        finally {
            this.handle = 0;
        }
    }
    async fileId() {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'fileId not supported in WASM backend');
    }
    async parentId() {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'parentId not supported in WASM backend');
    }
    async lineageDepth() {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'lineageDepth not supported in WASM backend');
    }
    async derive(_childPath, _options) {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'derive not supported in WASM backend');
    }
    async embedKernel() {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'embedKernel not supported in WASM backend');
    }
    async extractKernel() {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'extractKernel not supported in WASM backend');
    }
    async embedEbpf() {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'embedEbpf not supported in WASM backend');
    }
    async extractEbpf() {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'extractEbpf not supported in WASM backend');
    }
    async segments() {
        throw new errors_1.RvfError(errors_1.RvfErrorCode.BackendNotFound, 'segments not supported in WASM backend');
    }
    async dimension() {
        this.ensureHandle();
        const d = this.wasm.rvf_store_dimension(this.handle);
        if (d < 0)
            throw new errors_1.RvfError(errors_1.RvfErrorCode.StoreClosed);
        return d;
    }
}
exports.WasmBackend = WasmBackend;
// ---------------------------------------------------------------------------
// Backend resolution
// ---------------------------------------------------------------------------
/**
 * Resolve a `BackendType` to a concrete `RvfBackend` instance.
 *
 * - `'node'`  Always returns a `NodeBackend`.
 * - `'wasm'`  Always returns a `WasmBackend`.
 * - `'auto'`  Tries `node` first, falls back to `wasm`.
 */
function resolveBackend(type) {
    switch (type) {
        case 'node':
            return new NodeBackend();
        case 'wasm':
            return new WasmBackend();
        case 'auto': {
            // In Node.js environments, prefer native; in browsers, prefer WASM.
            const isNode = typeof process !== 'undefined' &&
                typeof process.versions !== 'undefined' &&
                typeof process.versions.node === 'string';
            return isNode ? new NodeBackend() : new WasmBackend();
        }
    }
}
// ---------------------------------------------------------------------------
// Mapping helpers (TS options -> native/wasm shapes)
// ---------------------------------------------------------------------------
function mapMetricToNative(metric) {
    switch (metric) {
        case 'cosine':
            return 'Cosine';
        case 'dotproduct':
            return 'InnerProduct';
        case 'l2':
        default:
            return 'L2';
    }
}
function mapCompressionToNative(compression) {
    switch (compression) {
        case 'scalar':
            return 'Scalar';
        case 'product':
            return 'Product';
        case 'none':
        default:
            return 'None';
    }
}
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function mapOptionsToNative(options) {
    return {
        dimension: options.dimensions,
        metric: mapMetricToNative(options.metric),
        profile: options.profile ?? 0,
        compression: mapCompressionToNative(options.compression),
        signing: options.signing ?? false,
        m: options.m ?? 16,
        ef_construction: options.efConstruction ?? 200,
    };
}
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function mapQueryOptionsToNative(options) {
    return {
        ef_search: options.efSearch ?? 100,
        // NAPI accepts the filter as a JSON string, not an object.
        filter: options.filter ? JSON.stringify(options.filter) : undefined,
        timeout_ms: options.timeoutMs ?? 0,
    };
}
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function mapNativeStatus(s) {
    return {
        totalVectors: s.total_vectors ?? s.totalVectors ?? 0,
        totalSegments: s.total_segments ?? s.totalSegments ?? 0,
        fileSizeBytes: s.file_size ?? s.fileSizeBytes ?? 0,
        epoch: s.current_epoch ?? s.epoch ?? 0,
        profileId: s.profile_id ?? s.profileId ?? 0,
        compactionState: mapCompactionState(s.compaction_state ?? s.compactionState),
        deadSpaceRatio: s.dead_space_ratio ?? s.deadSpaceRatio ?? 0,
        readOnly: s.read_only ?? s.readOnly ?? false,
    };
}
function mapCompactionState(state) {
    if (typeof state === 'string') {
        const lower = state.toLowerCase();
        if (lower === 'running')
            return 'running';
        if (lower === 'emergency')
            return 'emergency';
    }
    return 'idle';
}
//# sourceMappingURL=backend.js.map