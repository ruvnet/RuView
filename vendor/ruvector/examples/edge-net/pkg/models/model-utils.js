/**
 * @ruvector/edge-net Model Utilities
 *
 * Helper functions for model management, optimization, and deployment.
 *
 * @module @ruvector/edge-net/models/utils
 */

import { createHash, randomBytes } from 'crypto';
import { existsSync, readFileSync, writeFileSync, mkdirSync, statSync, createReadStream } from 'fs';
import { join, dirname } from 'path';
import { homedir } from 'os';
import { pipeline } from 'stream/promises';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// ============================================
// CONFIGURATION
// ============================================

export const DEFAULT_CACHE_DIR = process.env.ONNX_CACHE_DIR ||
    join(homedir(), '.ruvector', 'models', 'onnx');

export const REGISTRY_PATH = join(__dirname, 'registry.json');

export const GCS_CONFIG = {
    bucket: process.env.GCS_MODEL_BUCKET || 'ruvector-models',
    projectId: process.env.GCS_PROJECT_ID || 'ruvector',
};

export const IPFS_CONFIG = {
    gateway: process.env.IPFS_GATEWAY || 'https://ipfs.io/ipfs',
    pinataApiKey: process.env.PINATA_API_KEY,
    pinataSecret: process.env.PINATA_SECRET,
};

// ============================================
// REGISTRY MANAGEMENT
// ============================================

/**
 * Load the model registry
 * @returns {Object} Registry object
 */
export function loadRegistry() {
    try {
        if (existsSync(REGISTRY_PATH)) {
            return JSON.parse(readFileSync(REGISTRY_PATH, 'utf-8'));
        }
    } catch (error) {
        console.error('[Registry] Failed to load:', error.message);
    }
    return { version: '1.0.0', models: {}, profiles: {}, adapters: {} };
}

/**
 * Save the model registry
 * @param {Object} registry - Registry object to save
 */
export function saveRegistry(registry) {
    registry.updated = new Date().toISOString();
    writeFileSync(REGISTRY_PATH, JSON.stringify(registry, null, 2));
}

/**
 * Get a model from the registry
 * @param {string} modelId - Model identifier
 * @returns {Object|null} Model metadata or null
 */
export function getModel(modelId) {
    const registry = loadRegistry();
    return registry.models[modelId] || null;
}

/**
 * Get a deployment profile
 * @param {string} profileId - Profile identifier
 * @returns {Object|null} Profile configuration or null
 */
export function getProfile(profileId) {
    const registry = loadRegistry();
    return registry.profiles[profileId] || null;
}

// ============================================
// FILE UTILITIES
// ============================================

/**
 * Format bytes to human-readable size
 * @param {number} bytes - Size in bytes
 * @returns {string} Formatted size string
 */
export function formatSize(bytes) {
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let size = bytes;
    let unitIndex = 0;
    while (size >= 1024 && unitIndex < units.length - 1) {
        size /= 1024;
        unitIndex++;
    }
    return `${size.toFixed(1)}${units[unitIndex]}`;
}

/**
 * Parse size string to bytes
 * @param {string} sizeStr - Size string like "100MB"
 * @returns {number} Size in bytes
 */
export function parseSize(sizeStr) {
    const units = { 'B': 1, 'KB': 1024, 'MB': 1024**2, 'GB': 1024**3, 'TB': 1024**4 };
    const match = sizeStr.match(/^([\d.]+)\s*(B|KB|MB|GB|TB)?$/i);
    if (!match) return 0;
    const value = parseFloat(match[1]);
    const unit = (match[2] || 'B').toUpperCase();
    return value * (units[unit] || 1);
}

/**
 * Calculate SHA256 hash of a file
 * @param {string} filePath - Path to file
 * @returns {Promise<string>} Hex-encoded hash
 */
export async function hashFile(filePath) {
    const hash = createHash('sha256');
    const stream = createReadStream(filePath);

    return new Promise((resolve, reject) => {
        stream.on('data', (data) => hash.update(data));
        stream.on('end', () => resolve(hash.digest('hex')));
        stream.on('error', reject);
    });
}

/**
 * Calculate SHA256 hash of a buffer
 * @param {Buffer} buffer - Data buffer
 * @returns {string} Hex-encoded hash
 */
export function hashBuffer(buffer) {
    return createHash('sha256').update(buffer).digest('hex');
}

/**
 * Get the cache directory for a model
 * @param {string} modelId - HuggingFace model ID
 * @returns {string} Cache directory path
 */
export function getModelCacheDir(modelId) {
    return join(DEFAULT_CACHE_DIR, modelId.replace(/\//g, '--'));
}

/**
 * Check if a model is cached locally
 * @param {string} modelId - Model identifier
 * @returns {boolean} True if cached
 */
export function isModelCached(modelId) {
    const model = getModel(modelId);
    if (!model) return false;
    const cacheDir = getModelCacheDir(model.huggingface);
    return existsSync(cacheDir);
}

/**
 * Get cached model size
 * @param {string} modelId - Model identifier
 * @returns {number} Size in bytes or 0
 */
export function getCachedModelSize(modelId) {
    const model = getModel(modelId);
    if (!model) return 0;
    const cacheDir = getModelCacheDir(model.huggingface);
    if (!existsSync(cacheDir)) return 0;
    return getDirectorySize(cacheDir);
}

/**
 * Get directory size recursively
 * @param {string} dir - Directory path
 * @returns {number} Total size in bytes
 */
export function getDirectorySize(dir) {
    let size = 0;
    try {
        const { readdirSync } = require('fs');
        const entries = readdirSync(dir, { withFileTypes: true });
        for (const entry of entries) {
            const fullPath = join(dir, entry.name);
            if (entry.isDirectory()) {
                size += getDirectorySize(fullPath);
            } else {
                size += statSync(fullPath).size;
            }
        }
    } catch (error) {
        // Ignore errors
    }
    return size;
}

// ============================================
// MODEL OPTIMIZATION
// ============================================

/**
 * Quantization configurations
 */
export const QUANTIZATION_CONFIGS = {
    int4: {
        bits: 4,
        blockSize: 32,
        expectedReduction: 0.25, // 4x smaller
        description: 'Aggressive quantization, some quality loss',
    },
    int8: {
        bits: 8,
        blockSize: 128,
        expectedReduction: 0.5, // 2x smaller
        description: 'Balanced quantization, minimal quality loss',
    },
    fp16: {
        bits: 16,
        blockSize: null,
        expectedReduction: 0.5, // 2x smaller than fp32
        description: 'Half precision, no quality loss',
    },
    fp32: {
        bits: 32,
        blockSize: null,
        expectedReduction: 1.0, // No change
        description: 'Full precision, original quality',
    },
};

/**
 * Estimate quantized model size
 * @param {string} modelId - Model identifier
 * @param {string} quantType - Quantization type
 * @returns {number} Estimated size in bytes
 */
export function estimateQuantizedSize(modelId, quantType) {
    const model = getModel(modelId);
    if (!model) return 0;

    const originalSize = parseSize(model.size);
    const config = QUANTIZATION_CONFIGS[quantType] || QUANTIZATION_CONFIGS.fp32;

    return Math.floor(originalSize * config.expectedReduction);
}

/**
 * Get recommended quantization for a device profile
 * @param {Object} deviceProfile - Device capabilities
 * @returns {string} Recommended quantization type
 */
export function getRecommendedQuantization(deviceProfile) {
    const { memory, isEdge, requiresSpeed } = deviceProfile;

    if (memory < 512 * 1024 * 1024) { // < 512MB
        return 'int4';
    } else if (memory < 2 * 1024 * 1024 * 1024 || isEdge) { // < 2GB or edge
        return 'int8';
    } else if (requiresSpeed) {
        return 'fp16';
    }
    return 'fp32';
}

// ============================================
// DOWNLOAD UTILITIES
// ============================================

/**
 * Download progress callback type
 * @callback ProgressCallback
 * @param {Object} progress - Progress information
 * @param {number} progress.loaded - Bytes loaded
 * @param {number} progress.total - Total bytes
 * @param {string} progress.file - Current file name
 */

/**
 * Download a file with progress reporting
 * @param {string} url - URL to download
 * @param {string} destPath - Destination path
 * @param {ProgressCallback} [onProgress] - Progress callback
 * @returns {Promise<string>} Downloaded file path
 */
export async function downloadFile(url, destPath, onProgress) {
    const destDir = dirname(destPath);
    if (!existsSync(destDir)) {
        mkdirSync(destDir, { recursive: true });
    }

    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const totalSize = parseInt(response.headers.get('content-length') || '0', 10);
    let loadedSize = 0;

    const { createWriteStream } = await import('fs');
    const fileStream = createWriteStream(destPath);
    const reader = response.body.getReader();

    try {
        while (true) {
            const { done, value } = await reader.read();
            if (done) break;

            fileStream.write(value);
            loadedSize += value.length;

            if (onProgress) {
                onProgress({
                    loaded: loadedSize,
                    total: totalSize,
                    file: destPath,
                });
            }
        }
    } finally {
        fileStream.end();
    }

    return destPath;
}

// ============================================
// IPFS UTILITIES
// ============================================

/**
 * Pin a file to IPFS via Pinata
 * @param {string} filePath - Path to file to pin
 * @param {Object} metadata - Metadata for the pin
 * @returns {Promise<string>} IPFS CID
 */
export async function pinToIPFS(filePath, metadata = {}) {
    if (!IPFS_CONFIG.pinataApiKey || !IPFS_CONFIG.pinataSecret) {
        throw new Error('Pinata API credentials not configured');
    }

    const FormData = (await import('form-data')).default;
    const form = new FormData();

    form.append('file', createReadStream(filePath));
    form.append('pinataMetadata', JSON.stringify({
        name: metadata.name || filePath,
        keyvalues: metadata,
    }));

    const response = await fetch('https://api.pinata.cloud/pinning/pinFileToIPFS', {
        method: 'POST',
        headers: {
            'pinata_api_key': IPFS_CONFIG.pinataApiKey,
            'pinata_secret_api_key': IPFS_CONFIG.pinataSecret,
        },
        body: form,
    });

    if (!response.ok) {
        throw new Error(`Pinata error: ${response.statusText}`);
    }

    const result = await response.json();
    return result.IpfsHash;
}

/**
 * Get IPFS gateway URL for a CID
 * @param {string} cid - IPFS CID
 * @returns {string} Gateway URL
 */
export function getIPFSUrl(cid) {
    return `${IPFS_CONFIG.gateway}/${cid}`;
}

// ============================================
// GCS UTILITIES
// ============================================

/**
 * Generate GCS URL for a model
 * @param {string} modelId - Model identifier
 * @param {string} fileName - File name
 * @returns {string} GCS URL
 */
export function getGCSUrl(modelId, fileName) {
    return `https://storage.googleapis.com/${GCS_CONFIG.bucket}/${modelId}/${fileName}`;
}

/**
 * Check if a model exists in GCS
 * @param {string} modelId - Model identifier
 * @param {string} fileName - File name
 * @returns {Promise<boolean>} True if exists
 */
export async function checkGCSExists(modelId, fileName) {
    const url = getGCSUrl(modelId, fileName);
    try {
        const response = await fetch(url, { method: 'HEAD' });
        return response.ok;
    } catch {
        return false;
    }
}

// ============================================
// ADAPTER UTILITIES
// ============================================

/**
 * MicroLoRA adapter configuration
 */
export const LORA_DEFAULTS = {
    rank: 8,
    alpha: 16,
    dropout: 0.1,
    targetModules: ['q_proj', 'v_proj'],
};

/**
 * Create adapter metadata
 * @param {string} name - Adapter name
 * @param {string} baseModel - Base model identifier
 * @param {Object} options - Training options
 * @returns {Object} Adapter metadata
 */
export function createAdapterMetadata(name, baseModel, options = {}) {
    return {
        id: `${name}-${randomBytes(4).toString('hex')}`,
        name,
        baseModel,
        rank: options.rank || LORA_DEFAULTS.rank,
        alpha: options.alpha || LORA_DEFAULTS.alpha,
        targetModules: options.targetModules || LORA_DEFAULTS.targetModules,
        created: new Date().toISOString(),
        size: null, // Set after training
    };
}

/**
 * Get adapter save path
 * @param {string} adapterName - Adapter name
 * @returns {string} Save path
 */
export function getAdapterPath(adapterName) {
    return join(DEFAULT_CACHE_DIR, 'adapters', adapterName);
}

// ============================================
// BENCHMARK UTILITIES
// ============================================

/**
 * Create a benchmark result object
 * @param {string} modelId - Model identifier
 * @param {number[]} times - Latency measurements in ms
 * @returns {Object} Benchmark results
 */
export function createBenchmarkResult(modelId, times) {
    times.sort((a, b) => a - b);

    return {
        model: modelId,
        timestamp: new Date().toISOString(),
        iterations: times.length,
        stats: {
            avg: times.reduce((a, b) => a + b, 0) / times.length,
            median: times[Math.floor(times.length / 2)],
            p95: times[Math.floor(times.length * 0.95)],
            p99: times[Math.floor(times.length * 0.99)],
            min: times[0],
            max: times[times.length - 1],
            stddev: calculateStdDev(times),
        },
        rawTimes: times,
    };
}

/**
 * Calculate standard deviation
 * @param {number[]} values - Array of values
 * @returns {number} Standard deviation
 */
function calculateStdDev(values) {
    const mean = values.reduce((a, b) => a + b, 0) / values.length;
    const squareDiffs = values.map(v => Math.pow(v - mean, 2));
    const avgSquareDiff = squareDiffs.reduce((a, b) => a + b, 0) / squareDiffs.length;
    return Math.sqrt(avgSquareDiff);
}

// ============================================
// EXPORTS
// ============================================

export default {
    // Configuration
    DEFAULT_CACHE_DIR,
    REGISTRY_PATH,
    GCS_CONFIG,
    IPFS_CONFIG,
    QUANTIZATION_CONFIGS,
    LORA_DEFAULTS,

    // Registry
    loadRegistry,
    saveRegistry,
    getModel,
    getProfile,

    // Files
    formatSize,
    parseSize,
    hashFile,
    hashBuffer,
    getModelCacheDir,
    isModelCached,
    getCachedModelSize,
    getDirectorySize,

    // Optimization
    estimateQuantizedSize,
    getRecommendedQuantization,

    // Download
    downloadFile,

    // IPFS
    pinToIPFS,
    getIPFSUrl,

    // GCS
    getGCSUrl,
    checkGCSExists,

    // Adapters
    createAdapterMetadata,
    getAdapterPath,

    // Benchmarks
    createBenchmarkResult,
};
