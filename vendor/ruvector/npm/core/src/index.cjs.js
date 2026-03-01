"use strict";
/**
 * @ruvector/core - CommonJS wrapper
 *
 * This file provides CommonJS compatibility for projects using require()
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.DistanceMetric = void 0;
const node_os_1 = require("node:os");
/**
 * Distance metric for similarity calculation
 */
var DistanceMetric;
(function (DistanceMetric) {
    /** Euclidean (L2) distance */
    DistanceMetric["Euclidean"] = "euclidean";
    /** Cosine similarity (1 - cosine distance) */
    DistanceMetric["Cosine"] = "cosine";
    /** Dot product similarity */
    DistanceMetric["DotProduct"] = "dot";
})(DistanceMetric || (exports.DistanceMetric = DistanceMetric = {}));
/**
 * Get platform-specific package name
 */
function getPlatformPackage() {
    const plat = (0, node_os_1.platform)();
    const architecture = (0, node_os_1.arch)();
    // Map Node.js platform names to package names
    const packageMap = {
        'linux-x64': 'ruvector-core-linux-x64-gnu',
        'linux-arm64': 'ruvector-core-linux-arm64-gnu',
        'darwin-x64': 'ruvector-core-darwin-x64',
        'darwin-arm64': 'ruvector-core-darwin-arm64',
        'win32-x64': 'ruvector-core-win32-x64-msvc',
    };
    const key = `${plat}-${architecture}`;
    const packageName = packageMap[key];
    if (!packageName) {
        throw new Error(`Unsupported platform: ${plat}-${architecture}. ` +
            `Supported platforms: ${Object.keys(packageMap).join(', ')}`);
    }
    return packageName;
}
/**
 * Load the native binding for the current platform
 */
function loadNativeBinding() {
    const packageName = getPlatformPackage();
    try {
        // Try to require the platform-specific package
        return require(packageName);
    }
    catch (error) {
        // Fallback: try loading from local platforms directory
        try {
            const plat = (0, node_os_1.platform)();
            const architecture = (0, node_os_1.arch)();
            const platformKey = `${plat}-${architecture}`;
            const platformMap = {
                'linux-x64': 'linux-x64-gnu',
                'linux-arm64': 'linux-arm64-gnu',
                'darwin-x64': 'darwin-x64',
                'darwin-arm64': 'darwin-arm64',
                'win32-x64': 'win32-x64-msvc',
            };
            const localPath = `../platforms/${platformMap[platformKey]}/ruvector.node`;
            return require(localPath);
        }
        catch (fallbackError) {
            throw new Error(`Failed to load native binding: ${error.message}\n` +
                `Fallback also failed: ${fallbackError.message}\n` +
                `Platform: ${(0, node_os_1.platform)()}-${(0, node_os_1.arch)()}\n` +
                `Expected package: ${packageName}`);
        }
    }
}
// Load the native module
const nativeBinding = loadNativeBinding();
// Try to load optional attention module
let attention = null;
try {
    attention = require('@ruvector/attention');
}
catch {
    // Attention module not installed - this is optional
}
// Export everything from the native binding
module.exports = nativeBinding;
// Add VectorDB alias (native exports as VectorDb)
if (nativeBinding.VectorDb && !nativeBinding.VectorDB) {
    module.exports.VectorDB = nativeBinding.VectorDb;
}
// Also export as default
module.exports.default = nativeBinding;
// Re-export DistanceMetric
module.exports.DistanceMetric = DistanceMetric;
// Export attention if available
if (attention) {
    module.exports.attention = attention;
}
//# sourceMappingURL=index.cjs.js.map