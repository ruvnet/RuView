"use strict";
/**
 * RVF Wrapper - Persistent vector store via @ruvector/rvf
 *
 * Wraps @ruvector/rvf RvfDatabase through thin convenience functions.
 * Falls back to clear error messages when the package is not installed.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.isRvfAvailable = isRvfAvailable;
exports.createRvfStore = createRvfStore;
exports.openRvfStore = openRvfStore;
exports.rvfIngest = rvfIngest;
exports.rvfQuery = rvfQuery;
exports.rvfDelete = rvfDelete;
exports.rvfStatus = rvfStatus;
exports.rvfCompact = rvfCompact;
exports.rvfDerive = rvfDerive;
exports.rvfClose = rvfClose;
let rvfModule = null;
let loadError = null;
function getRvfModule() {
    if (rvfModule)
        return rvfModule;
    if (loadError)
        throw loadError;
    try {
        rvfModule = require('@ruvector/rvf');
        return rvfModule;
    }
    catch (e) {
        loadError = new Error('@ruvector/rvf is not installed. Run: npm install @ruvector/rvf');
        throw loadError;
    }
}
function isRvfAvailable() {
    try {
        getRvfModule();
        return true;
    }
    catch {
        return false;
    }
}
// ---------------------------------------------------------------------------
// Wrapper functions
// ---------------------------------------------------------------------------
/**
 * Create a new RVF store at the given path.
 */
async function createRvfStore(path, options) {
    const mod = getRvfModule();
    return mod.RvfDatabase.create(path, options);
}
/**
 * Open an existing RVF store for read-write access.
 */
async function openRvfStore(path) {
    const mod = getRvfModule();
    return mod.RvfDatabase.open(path);
}
/**
 * Ingest a batch of vectors into an open store.
 */
async function rvfIngest(store, entries) {
    return store.ingestBatch(entries);
}
/**
 * Query for the k nearest neighbors.
 */
async function rvfQuery(store, vector, k, options) {
    return store.query(vector, k, options);
}
/**
 * Soft-delete vectors by their IDs.
 */
async function rvfDelete(store, ids) {
    return store.delete(ids);
}
/**
 * Get the current store status.
 */
async function rvfStatus(store) {
    return store.status();
}
/**
 * Run compaction to reclaim dead space.
 */
async function rvfCompact(store) {
    return store.compact();
}
/**
 * Derive a child store from a parent for lineage tracking.
 */
async function rvfDerive(store, childPath) {
    return store.derive(childPath);
}
/**
 * Close the store, releasing the writer lock and flushing data.
 */
async function rvfClose(store) {
    return store.close();
}
//# sourceMappingURL=rvf-wrapper.js.map