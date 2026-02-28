"use strict";
/**
 * RuvLTRA Model Registry and Downloader
 *
 * Automatically downloads GGUF models from HuggingFace Hub.
 *
 * @example
 * ```typescript
 * import { ModelDownloader, RUVLTRA_MODELS } from '@ruvector/ruvllm';
 *
 * // Download the Claude Code optimized model
 * const downloader = new ModelDownloader();
 * const modelPath = await downloader.download('claude-code');
 *
 * // Or download all models
 * await downloader.downloadAll();
 * ```
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
exports.ModelDownloader = exports.MODEL_ALIASES = exports.RUVLTRA_MODELS = void 0;
exports.getDefaultModelsDir = getDefaultModelsDir;
exports.resolveModelId = resolveModelId;
exports.getModelInfo = getModelInfo;
exports.listModels = listModels;
const fs_1 = require("fs");
const path_1 = require("path");
const os_1 = require("os");
/** HuggingFace repository */
const HF_REPO = 'ruv/ruvltra';
const HF_BASE_URL = `https://huggingface.co/${HF_REPO}/resolve/main`;
/** Available RuvLTRA models */
exports.RUVLTRA_MODELS = {
    'claude-code': {
        id: 'claude-code',
        name: 'RuvLTRA Claude Code',
        filename: 'ruvltra-claude-code-0.5b-q4_k_m.gguf',
        sizeBytes: 398000000,
        size: '398 MB',
        parameters: '0.5B',
        useCase: 'Claude Code workflows, agentic coding',
        quantization: 'Q4_K_M',
        contextLength: 4096,
        url: `${HF_BASE_URL}/ruvltra-claude-code-0.5b-q4_k_m.gguf`,
    },
    'small': {
        id: 'small',
        name: 'RuvLTRA Small',
        filename: 'ruvltra-small-0.5b-q4_k_m.gguf',
        sizeBytes: 398000000,
        size: '398 MB',
        parameters: '0.5B',
        useCase: 'Edge devices, IoT, resource-constrained environments',
        quantization: 'Q4_K_M',
        contextLength: 4096,
        url: `${HF_BASE_URL}/ruvltra-small-0.5b-q4_k_m.gguf`,
    },
    'medium': {
        id: 'medium',
        name: 'RuvLTRA Medium',
        filename: 'ruvltra-medium-1.1b-q4_k_m.gguf',
        sizeBytes: 669000000,
        size: '669 MB',
        parameters: '1.1B',
        useCase: 'General purpose, balanced performance',
        quantization: 'Q4_K_M',
        contextLength: 8192,
        url: `${HF_BASE_URL}/ruvltra-medium-1.1b-q4_k_m.gguf`,
    },
};
/** Model aliases for convenience */
exports.MODEL_ALIASES = {
    'cc': 'claude-code',
    'claudecode': 'claude-code',
    'claude': 'claude-code',
    's': 'small',
    'sm': 'small',
    'm': 'medium',
    'med': 'medium',
    'default': 'claude-code',
};
/**
 * Get the default models directory
 */
function getDefaultModelsDir() {
    return (0, path_1.join)((0, os_1.homedir)(), '.ruvllm', 'models');
}
/**
 * Resolve model ID from alias or direct ID
 */
function resolveModelId(modelIdOrAlias) {
    const normalized = modelIdOrAlias.toLowerCase().trim();
    // Direct match
    if (exports.RUVLTRA_MODELS[normalized]) {
        return normalized;
    }
    // Alias match
    if (exports.MODEL_ALIASES[normalized]) {
        return exports.MODEL_ALIASES[normalized];
    }
    return null;
}
/**
 * Get model info by ID or alias
 */
function getModelInfo(modelIdOrAlias) {
    const id = resolveModelId(modelIdOrAlias);
    return id ? exports.RUVLTRA_MODELS[id] : null;
}
/**
 * List all available models
 */
function listModels() {
    return Object.values(exports.RUVLTRA_MODELS);
}
/**
 * Model downloader for RuvLTRA GGUF models
 */
class ModelDownloader {
    constructor(modelsDir) {
        this.modelsDir = modelsDir || getDefaultModelsDir();
    }
    /**
     * Get the path where a model would be saved
     */
    getModelPath(modelIdOrAlias) {
        const model = getModelInfo(modelIdOrAlias);
        if (!model)
            return null;
        return (0, path_1.join)(this.modelsDir, model.filename);
    }
    /**
     * Check if a model is already downloaded
     */
    isDownloaded(modelIdOrAlias) {
        const path = this.getModelPath(modelIdOrAlias);
        if (!path)
            return false;
        if (!(0, fs_1.existsSync)(path))
            return false;
        // Verify size matches expected
        const model = getModelInfo(modelIdOrAlias);
        if (!model)
            return false;
        const stats = (0, fs_1.statSync)(path);
        // Allow 5% variance for size check
        const minSize = model.sizeBytes * 0.95;
        return stats.size >= minSize;
    }
    /**
     * Get download status for all models
     */
    getStatus() {
        return listModels().map(model => ({
            model,
            downloaded: this.isDownloaded(model.id),
            path: this.getModelPath(model.id),
        }));
    }
    /**
     * Download a model from HuggingFace
     */
    async download(modelIdOrAlias, options = {}) {
        const model = getModelInfo(modelIdOrAlias);
        if (!model) {
            const available = listModels().map(m => m.id).join(', ');
            throw new Error(`Unknown model: ${modelIdOrAlias}. Available models: ${available}`);
        }
        const destDir = options.modelsDir || this.modelsDir;
        const destPath = (0, path_1.join)(destDir, model.filename);
        // Check if already downloaded
        if (!options.force && this.isDownloaded(model.id)) {
            return destPath;
        }
        // Ensure directory exists
        if (!(0, fs_1.existsSync)(destDir)) {
            (0, fs_1.mkdirSync)(destDir, { recursive: true });
        }
        // Download with progress tracking
        const tempPath = `${destPath}.tmp`;
        let startTime = Date.now();
        let lastProgressTime = startTime;
        let lastDownloaded = 0;
        try {
            // Use dynamic import for node-fetch if native fetch not available
            const fetchFn = globalThis.fetch || (await Promise.resolve().then(() => __importStar(require('node:https')))).default;
            const response = await fetch(model.url, {
                headers: {
                    'User-Agent': 'RuvLLM/2.3.0',
                },
            });
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }
            const contentLength = parseInt(response.headers.get('content-length') || String(model.sizeBytes));
            // Create write stream
            const fileStream = (0, fs_1.createWriteStream)(tempPath);
            let downloaded = 0;
            // Stream with progress
            const reader = response.body?.getReader();
            if (!reader) {
                throw new Error('Response body is not readable');
            }
            while (true) {
                const { done, value } = await reader.read();
                if (done)
                    break;
                downloaded += value.length;
                fileStream.write(value);
                // Report progress
                if (options.onProgress) {
                    const now = Date.now();
                    const elapsed = (now - lastProgressTime) / 1000;
                    const bytesThisInterval = downloaded - lastDownloaded;
                    const speedBps = elapsed > 0 ? bytesThisInterval / elapsed : 0;
                    const remaining = contentLength - downloaded;
                    const etaSeconds = speedBps > 0 ? remaining / speedBps : 0;
                    options.onProgress({
                        modelId: model.id,
                        downloaded,
                        total: contentLength,
                        percent: Math.round((downloaded / contentLength) * 100),
                        speedBps,
                        etaSeconds,
                    });
                    lastProgressTime = now;
                    lastDownloaded = downloaded;
                }
            }
            fileStream.end();
            // Wait for file to be fully written
            await new Promise((resolve, reject) => {
                fileStream.on('finish', resolve);
                fileStream.on('error', reject);
            });
            // Move temp file to final destination
            if ((0, fs_1.existsSync)(destPath)) {
                (0, fs_1.unlinkSync)(destPath);
            }
            (0, fs_1.renameSync)(tempPath, destPath);
            return destPath;
        }
        catch (error) {
            // Clean up temp file on error
            if ((0, fs_1.existsSync)(tempPath)) {
                try {
                    (0, fs_1.unlinkSync)(tempPath);
                }
                catch { }
            }
            throw error;
        }
    }
    /**
     * Download all available models
     */
    async downloadAll(options = {}) {
        const paths = [];
        for (const model of listModels()) {
            const path = await this.download(model.id, options);
            paths.push(path);
        }
        return paths;
    }
    /**
     * Delete a downloaded model
     */
    delete(modelIdOrAlias) {
        const path = this.getModelPath(modelIdOrAlias);
        if (!path || !(0, fs_1.existsSync)(path)) {
            return false;
        }
        (0, fs_1.unlinkSync)(path);
        return true;
    }
    /**
     * Delete all downloaded models
     */
    deleteAll() {
        let count = 0;
        for (const model of listModels()) {
            if (this.delete(model.id)) {
                count++;
            }
        }
        return count;
    }
}
exports.ModelDownloader = ModelDownloader;
exports.default = ModelDownloader;
//# sourceMappingURL=models.js.map