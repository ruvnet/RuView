"use strict";
/**
 * Browser-specific exports for @ruvector/wasm
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
exports.VectorDB = void 0;
exports.detectSIMD = detectSIMD;
exports.version = version;
exports.benchmark = benchmark;
let wasmModule = null;
/**
 * Initialize WASM module for browser
 */
async function initWasm() {
    if (!wasmModule) {
        wasmModule = await Promise.resolve().then(() => __importStar(require('../pkg/ruvector_wasm.js')));
        await wasmModule.default();
    }
    return wasmModule;
}
/**
 * VectorDB class for browser
 */
class VectorDB {
    constructor(options) {
        this.dimensions = options.dimensions;
    }
    async init() {
        const module = await initWasm();
        this.db = new module.VectorDB(this.dimensions, 'cosine', true);
    }
    insert(vector, id, metadata) {
        if (!this.db)
            throw new Error('Database not initialized. Call init() first.');
        const vectorArray = vector instanceof Float32Array ? vector : new Float32Array(vector);
        return this.db.insert(vectorArray, id, metadata);
    }
    insertBatch(entries) {
        if (!this.db)
            throw new Error('Database not initialized. Call init() first.');
        const processedEntries = entries.map(entry => ({
            id: entry.id,
            vector: entry.vector instanceof Float32Array ? entry.vector : new Float32Array(entry.vector),
            metadata: entry.metadata
        }));
        return this.db.insertBatch(processedEntries);
    }
    search(query, k, filter) {
        if (!this.db)
            throw new Error('Database not initialized. Call init() first.');
        const queryArray = query instanceof Float32Array ? query : new Float32Array(query);
        const results = this.db.search(queryArray, k, filter);
        return results.map((r) => ({
            id: r.id,
            score: r.score,
            vector: r.vector,
            metadata: r.metadata
        }));
    }
    delete(id) {
        if (!this.db)
            throw new Error('Database not initialized. Call init() first.');
        return this.db.delete(id);
    }
    get(id) {
        if (!this.db)
            throw new Error('Database not initialized. Call init() first.');
        const entry = this.db.get(id);
        if (!entry)
            return null;
        return { id: entry.id, vector: entry.vector, metadata: entry.metadata };
    }
    len() {
        if (!this.db)
            throw new Error('Database not initialized. Call init() first.');
        return this.db.len();
    }
    isEmpty() {
        if (!this.db)
            throw new Error('Database not initialized. Call init() first.');
        return this.db.isEmpty();
    }
    getDimensions() {
        return this.dimensions;
    }
    async saveToIndexedDB() {
        if (!this.db)
            throw new Error('Database not initialized. Call init() first.');
        await this.db.saveToIndexedDB();
    }
    static async loadFromIndexedDB(dbName, options) {
        const db = new VectorDB(options);
        await db.init();
        await db.db.loadFromIndexedDB(dbName);
        return db;
    }
}
exports.VectorDB = VectorDB;
async function detectSIMD() {
    const module = await initWasm();
    return module.detectSIMD();
}
async function version() {
    const module = await initWasm();
    return module.version();
}
async function benchmark(name, iterations, dimensions) {
    const module = await initWasm();
    return module.benchmark(name, iterations, dimensions);
}
exports.default = VectorDB;
//# sourceMappingURL=browser.js.map