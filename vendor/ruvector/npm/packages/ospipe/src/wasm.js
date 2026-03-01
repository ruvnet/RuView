"use strict";
/**
 * WASM bindings for OsPipe - use in browser-based pipes.
 *
 * This module provides a thin wrapper around the @ruvector/ospipe-wasm package,
 * exposing vector search, embedding, deduplication, and safety checking
 * capabilities that run entirely client-side via WebAssembly.
 *
 * @packageDocumentation
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
exports.initOsPipeWasm = initOsPipeWasm;
/**
 * Load and initialize the OsPipe WASM module.
 *
 * This function dynamically imports the @ruvector/ospipe-wasm package,
 * initializes the WebAssembly module, and returns a typed wrapper
 * around the raw WASM bindings.
 *
 * @param options - WASM initialization options
 * @returns Initialized WASM instance with typed methods
 * @throws {Error} If the WASM module fails to load or initialize
 *
 * @example
 * ```typescript
 * import { initOsPipeWasm } from "@ruvector/ospipe/wasm";
 *
 * const wasm = await initOsPipeWasm({ dimension: 384 });
 *
 * // Embed and insert
 * const embedding = wasm.embedText("hello world");
 * wasm.insert("doc-1", embedding, JSON.stringify({ app: "test" }));
 *
 * // Search
 * const query = wasm.embedText("greetings");
 * const results = wasm.search(query, 5);
 * ```
 */
async function initOsPipeWasm(options = {}) {
    const dimension = options.dimension ?? 384;
    // Dynamic import so the WASM package is not required at bundle time.
    // This allows the main @ruvector/ospipe package to work without WASM.
    // The @ruvector/ospipe-wasm package provides the compiled WASM bindings.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let wasm;
    try {
        // Use a variable to prevent TypeScript from resolving the module statically
        const wasmPkg = "@ruvector/ospipe-wasm";
        wasm = await Promise.resolve(`${wasmPkg}`).then(s => __importStar(require(s)));
    }
    catch {
        throw new Error("Failed to load @ruvector/ospipe-wasm. " +
            "Install it with: npm install @ruvector/ospipe-wasm");
    }
    await wasm.default();
    const instance = new wasm.OsPipeWasm(dimension);
    return {
        insert(id, embedding, metadata, timestamp) {
            instance.insert(id, embedding, metadata, timestamp ?? Date.now());
        },
        search(queryEmbedding, k = 10) {
            return instance.search(queryEmbedding, k);
        },
        searchFiltered(queryEmbedding, k, startTime, endTime) {
            return instance.search_filtered(queryEmbedding, k, startTime, endTime);
        },
        isDuplicate(embedding, threshold = 0.95) {
            return instance.is_duplicate(embedding, threshold);
        },
        embedText(text) {
            return new Float32Array(instance.embed_text(text));
        },
        safetyCheck(content) {
            return instance.safety_check(content);
        },
        routeQuery(query) {
            return instance.route_query(query);
        },
        get size() {
            return instance.len();
        },
        stats() {
            return instance.stats();
        },
    };
}
//# sourceMappingURL=wasm.js.map