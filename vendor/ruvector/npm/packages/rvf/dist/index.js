"use strict";
/**
 * @ruvector/rvf — Unified TypeScript SDK for the RuVector Format.
 *
 * Works with both the native Node.js backend (`@ruvector/rvf-node`) and
 * the browser WASM backend (`@ruvector/rvf-wasm`).
 *
 * @example
 * ```ts
 * import { RvfDatabase } from '@ruvector/rvf';
 *
 * const db = await RvfDatabase.create('./my.rvf', { dimensions: 128 });
 * await db.ingestBatch([
 *   { id: '1', vector: new Float32Array(128) },
 * ]);
 * const results = await db.query(new Float32Array(128), 10);
 * await db.close();
 * ```
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.RvfSolver = exports.RvfDatabase = exports.resolveBackend = exports.WasmBackend = exports.NodeBackend = exports.RvfErrorCode = exports.RvfError = void 0;
// Re-export error types
var errors_1 = require("./errors");
Object.defineProperty(exports, "RvfError", { enumerable: true, get: function () { return errors_1.RvfError; } });
Object.defineProperty(exports, "RvfErrorCode", { enumerable: true, get: function () { return errors_1.RvfErrorCode; } });
var backend_1 = require("./backend");
Object.defineProperty(exports, "NodeBackend", { enumerable: true, get: function () { return backend_1.NodeBackend; } });
Object.defineProperty(exports, "WasmBackend", { enumerable: true, get: function () { return backend_1.WasmBackend; } });
Object.defineProperty(exports, "resolveBackend", { enumerable: true, get: function () { return backend_1.resolveBackend; } });
// Re-export the main database class
var database_1 = require("./database");
Object.defineProperty(exports, "RvfDatabase", { enumerable: true, get: function () { return database_1.RvfDatabase; } });
// Re-export solver (AGI components)
var rvf_solver_1 = require("@ruvector/rvf-solver");
Object.defineProperty(exports, "RvfSolver", { enumerable: true, get: function () { return rvf_solver_1.RvfSolver; } });
//# sourceMappingURL=index.js.map