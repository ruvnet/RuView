"use strict";
/**
 * Node.js-specific entry point with filesystem support
 *
 * @security Path traversal prevention via ID validation
 */
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
var __exportStar = (this && this.__exportStar) || function(m, exports) {
    for (var p in m) if (p !== "default" && !Object.prototype.hasOwnProperty.call(exports, p)) __createBinding(exports, m, p);
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.NodeDagManager = exports.FileDagStorage = void 0;
exports.createNodeDag = createNodeDag;
__exportStar(require("./index"), exports);
const index_1 = require("./index");
const fs_1 = require("fs");
const path_1 = require("path");
/**
 * Validate storage ID to prevent path traversal attacks
 * @security Only allows alphanumeric, dash, underscore characters
 */
function isValidStorageId(id) {
    if (typeof id !== 'string' || id.length === 0 || id.length > 256)
        return false;
    // Strictly alphanumeric with dash/underscore - no dots, slashes, etc.
    return /^[a-zA-Z0-9_-]+$/.test(id);
}
/**
 * Ensure path is within base directory
 * @security Prevents path traversal via realpath comparison
 */
async function ensureWithinBase(basePath, targetPath) {
    const resolvedBase = (0, path_1.resolve)(basePath);
    const resolvedTarget = (0, path_1.resolve)(targetPath);
    if (!resolvedTarget.startsWith(resolvedBase + '/') && resolvedTarget !== resolvedBase) {
        throw new Error('Path traversal detected: target path outside base directory');
    }
    return resolvedTarget;
}
/**
 * Create a Node.js DAG with memory storage
 */
async function createNodeDag(name) {
    const storage = new index_1.MemoryStorage();
    const dag = new index_1.RuDag({ name, storage });
    await dag.init();
    return dag;
}
/**
 * File-based storage for Node.js environments
 * @security All file operations validate paths to prevent traversal attacks
 */
class FileDagStorage {
    constructor(basePath = '.rudag') {
        this.initialized = false;
        // Normalize and resolve base path
        this.basePath = (0, path_1.resolve)((0, path_1.normalize)(basePath));
    }
    async init() {
        if (this.initialized)
            return;
        try {
            await fs_1.promises.mkdir(this.basePath, { recursive: true });
            this.initialized = true;
        }
        catch (error) {
            throw new Error(`Failed to create storage directory: ${error}`);
        }
    }
    async getFilePath(id) {
        if (!isValidStorageId(id)) {
            throw new Error(`Invalid storage ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
        }
        const targetPath = (0, path_1.join)(this.basePath, `${id}.dag`);
        return ensureWithinBase(this.basePath, targetPath);
    }
    async getMetaPath(id) {
        if (!isValidStorageId(id)) {
            throw new Error(`Invalid storage ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
        }
        const targetPath = (0, path_1.join)(this.basePath, `${id}.meta.json`);
        return ensureWithinBase(this.basePath, targetPath);
    }
    async save(id, data, options = {}) {
        await this.init();
        const filePath = await this.getFilePath(id);
        const metaPath = await this.getMetaPath(id);
        // Load existing metadata for createdAt preservation
        let existingMeta = null;
        try {
            const metaContent = await fs_1.promises.readFile(metaPath, 'utf-8');
            existingMeta = JSON.parse(metaContent);
        }
        catch {
            // File doesn't exist or invalid - will create new
        }
        const now = Date.now();
        const meta = {
            id,
            name: options.name,
            metadata: options.metadata,
            createdAt: existingMeta?.createdAt || now,
            updatedAt: now,
        };
        // Write both files atomically (as much as possible)
        await Promise.all([
            fs_1.promises.writeFile(filePath, Buffer.from(data)),
            fs_1.promises.writeFile(metaPath, JSON.stringify(meta, null, 2)),
        ]);
    }
    async load(id) {
        await this.init();
        const filePath = await this.getFilePath(id);
        try {
            const data = await fs_1.promises.readFile(filePath);
            return new Uint8Array(data);
        }
        catch (error) {
            if (error.code === 'ENOENT') {
                return null;
            }
            throw error;
        }
    }
    async loadMeta(id) {
        await this.init();
        const metaPath = await this.getMetaPath(id);
        try {
            const content = await fs_1.promises.readFile(metaPath, 'utf-8');
            return JSON.parse(content);
        }
        catch (error) {
            if (error.code === 'ENOENT') {
                return null;
            }
            throw error;
        }
    }
    async delete(id) {
        await this.init();
        const filePath = await this.getFilePath(id);
        const metaPath = await this.getMetaPath(id);
        const results = await Promise.allSettled([
            fs_1.promises.unlink(filePath),
            fs_1.promises.unlink(metaPath),
        ]);
        // Return true if at least one file was deleted
        return results.some(r => r.status === 'fulfilled');
    }
    async list() {
        await this.init();
        try {
            const files = await fs_1.promises.readdir(this.basePath);
            return files
                .filter(f => f.endsWith('.dag'))
                .map(f => f.slice(0, -4)) // Remove .dag extension
                .filter(id => isValidStorageId(id)); // Extra safety filter
        }
        catch (error) {
            if (error.code === 'ENOENT') {
                return [];
            }
            throw error;
        }
    }
    async clear() {
        await this.init();
        const ids = await this.list();
        await Promise.all(ids.map(id => this.delete(id)));
    }
    async stats() {
        await this.init();
        const ids = await this.list();
        let totalSize = 0;
        for (const id of ids) {
            try {
                const filePath = await this.getFilePath(id);
                const stat = await fs_1.promises.stat(filePath);
                totalSize += stat.size;
            }
            catch {
                // Skip files that can't be accessed
            }
        }
        return { count: ids.length, totalSize };
    }
}
exports.FileDagStorage = FileDagStorage;
/**
 * Node.js DAG manager with file persistence
 */
class NodeDagManager {
    constructor(basePath) {
        this.storage = new FileDagStorage(basePath);
    }
    async init() {
        await this.storage.init();
    }
    async createDag(name) {
        const dag = new index_1.RuDag({ name, storage: null, autoSave: false });
        await dag.init();
        return dag;
    }
    async saveDag(dag) {
        const data = dag.toBytes();
        await this.storage.save(dag.getId(), data, { name: dag.getName() });
    }
    async loadDag(id) {
        const data = await this.storage.load(id);
        if (!data)
            return null;
        const meta = await this.storage.loadMeta(id);
        return index_1.RuDag.fromBytes(data, { id, name: meta?.name });
    }
    async deleteDag(id) {
        return this.storage.delete(id);
    }
    async listDags() {
        return this.storage.list();
    }
    async clearAll() {
        return this.storage.clear();
    }
    async getStats() {
        return this.storage.stats();
    }
}
exports.NodeDagManager = NodeDagManager;
//# sourceMappingURL=node.js.map