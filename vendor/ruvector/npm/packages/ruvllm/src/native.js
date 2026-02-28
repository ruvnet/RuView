"use strict";
/**
 * Native bindings loader for RuvLLM
 *
 * Automatically loads the correct native binary for the current platform.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.getNativeModule = getNativeModule;
exports.version = version;
exports.hasSimdSupport = hasSimdSupport;
const path_1 = require("path");
// Try to load the native module
let nativeModule = null;
// Platform-specific package names
const PLATFORM_PACKAGES = {
    'darwin-x64': '@ruvector/ruvllm-darwin-x64',
    'darwin-arm64': '@ruvector/ruvllm-darwin-arm64',
    'linux-x64': '@ruvector/ruvllm-linux-x64-gnu',
    'linux-arm64': '@ruvector/ruvllm-linux-arm64-gnu',
    'win32-x64': '@ruvector/ruvllm-win32-x64-msvc',
};
function getPlatformKey() {
    const platform = process.platform;
    const arch = process.arch;
    return `${platform}-${arch}`;
}
function loadNativeModule() {
    if (nativeModule) {
        return nativeModule;
    }
    const platformKey = getPlatformKey();
    const packageName = PLATFORM_PACKAGES[platformKey];
    if (!packageName) {
        // Silently fail - JS fallback will be used
        return null;
    }
    // Try loading from optional dependencies
    const attempts = [
        // Try the platform-specific package
        () => require(packageName),
        // Try loading from local .node file (CJS build)
        () => require((0, path_1.join)(__dirname, '..', '..', 'ruvllm.node')),
        // Try loading from local .node file (root)
        () => require((0, path_1.join)(__dirname, '..', 'ruvllm.node')),
    ];
    for (const attempt of attempts) {
        try {
            const raw = attempt();
            // Normalize: native exports RuvLlmEngine, we expose as RuvLLMEngine
            nativeModule = {
                RuvLLMEngine: raw.RuvLLMEngine ?? raw.RuvLlmEngine,
                SimdOperations: raw.SimdOperations,
                version: raw.version,
                hasSimdSupport: raw.hasSimdSupport,
            };
            return nativeModule;
        }
        catch {
            // Continue to next attempt
        }
    }
    // Silently fall back to JS implementation
    return null;
}
// Export functions to get native bindings
function getNativeModule() {
    return loadNativeModule();
}
function version() {
    const mod = loadNativeModule();
    return mod?.version() ?? '0.1.0-js';
}
function hasSimdSupport() {
    const mod = loadNativeModule();
    return mod?.hasSimdSupport() ?? false;
}
//# sourceMappingURL=native.js.map