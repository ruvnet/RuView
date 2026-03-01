"use strict";
/**
 * @ruvector/node/gnn - GNN-specific exports
 *
 * Import GNN capabilities directly:
 * ```typescript
 * import { RuvectorLayer, TensorCompress } from '@ruvector/node/gnn';
 * ```
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.init = exports.getCompressionLevel = exports.hierarchicalForward = exports.differentiableSearch = exports.TensorCompress = exports.RuvectorLayer = void 0;
var gnn_1 = require("@ruvector/gnn");
Object.defineProperty(exports, "RuvectorLayer", { enumerable: true, get: function () { return gnn_1.RuvectorLayer; } });
Object.defineProperty(exports, "TensorCompress", { enumerable: true, get: function () { return gnn_1.TensorCompress; } });
Object.defineProperty(exports, "differentiableSearch", { enumerable: true, get: function () { return gnn_1.differentiableSearch; } });
Object.defineProperty(exports, "hierarchicalForward", { enumerable: true, get: function () { return gnn_1.hierarchicalForward; } });
Object.defineProperty(exports, "getCompressionLevel", { enumerable: true, get: function () { return gnn_1.getCompressionLevel; } });
Object.defineProperty(exports, "init", { enumerable: true, get: function () { return gnn_1.init; } });
//# sourceMappingURL=gnn.js.map