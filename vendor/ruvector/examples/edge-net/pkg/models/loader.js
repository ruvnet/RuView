/**
 * @ruvector/edge-net Model Loader
 *
 * Tiered model loading with:
 * - Memory-aware model selection
 * - Streaming chunk verification
 * - Multi-source fallback (GCS → IPFS → P2P)
 * - IndexedDB caching
 *
 * Design: Registry returns manifest only, client derives URLs from manifest.
 *
 * @module @ruvector/edge-net/models/loader
 */

import { createHash } from 'crypto';
import { ManifestVerifier, verifyMerkleProof, computeMerkleRoot } from './integrity.js';

// ============================================================================
// MODEL TIERS
// ============================================================================

/**
 * Model tier definitions with memory requirements
 */
export const MODEL_TIERS = Object.freeze({
    micro: {
        name: 'micro',
        maxSize: 100 * 1024 * 1024, // 100MB
        minMemory: 256 * 1024 * 1024, // 256MB available
        description: 'Embeddings and small tasks',
        priority: 1,
    },
    small: {
        name: 'small',
        maxSize: 500 * 1024 * 1024, // 500MB
        minMemory: 1024 * 1024 * 1024, // 1GB available
        description: 'Balanced capability',
        priority: 2,
    },
    large: {
        name: 'large',
        maxSize: 1500 * 1024 * 1024, // 1.5GB
        minMemory: 4096 * 1024 * 1024, // 4GB available
        description: 'Full capability',
        priority: 3,
    },
});

/**
 * Capability priorities for model selection
 */
export const CAPABILITY_PRIORITIES = Object.freeze({
    embed: 1,      // Always prioritize embeddings
    retrieve: 2,   // Then retrieval
    generate: 3,   // Generation only when needed
    code: 4,       // Specialized capabilities
    multilingual: 5,
});

// ============================================================================
// MEMORY DETECTION
// ============================================================================

/**
 * Detect available memory for model loading
 */
export function detectAvailableMemory() {
    // Browser environment
    if (typeof navigator !== 'undefined' && navigator.deviceMemory) {
        return navigator.deviceMemory * 1024 * 1024 * 1024;
    }

    // Node.js environment
    if (typeof process !== 'undefined' && process.memoryUsage) {
        const usage = process.memoryUsage();
        // Estimate available as total minus current usage
        const total = require('os').totalmem?.() || 4 * 1024 * 1024 * 1024;
        return Math.max(0, total - usage.heapUsed);
    }

    // Default to 2GB as conservative estimate
    return 2 * 1024 * 1024 * 1024;
}

/**
 * Select appropriate tier based on device capabilities
 */
export function selectTier(requiredCapabilities = ['embed'], preferredTier = null) {
    const available = detectAvailableMemory();

    // Find highest tier that fits in memory
    const viableTiers = Object.values(MODEL_TIERS)
        .filter(tier => tier.minMemory <= available)
        .sort((a, b) => b.priority - a.priority);

    if (viableTiers.length === 0) {
        console.warn('[ModelLoader] Insufficient memory for any tier, using micro');
        return MODEL_TIERS.micro;
    }

    // Respect preferred tier if viable
    if (preferredTier && viableTiers.find(t => t.name === preferredTier)) {
        return MODEL_TIERS[preferredTier];
    }

    // Otherwise use highest viable
    return viableTiers[0];
}

// ============================================================================
// CACHE MANAGER
// ============================================================================

/**
 * IndexedDB-based cache for models and chunks
 */
export class ModelCache {
    constructor(options = {}) {
        this.dbName = options.dbName || 'ruvector-models';
        this.version = options.version || 1;
        this.db = null;
        this.maxCacheSize = options.maxCacheSize || 2 * 1024 * 1024 * 1024; // 2GB
    }

    async open() {
        if (this.db) return this.db;

        return new Promise((resolve, reject) => {
            const request = indexedDB.open(this.dbName, this.version);

            request.onerror = () => reject(request.error);

            request.onsuccess = () => {
                this.db = request.result;
                resolve(this.db);
            };

            request.onupgradeneeded = (event) => {
                const db = event.target.result;

                // Store for complete models
                if (!db.objectStoreNames.contains('models')) {
                    const store = db.createObjectStore('models', { keyPath: 'id' });
                    store.createIndex('hash', 'hash', { unique: true });
                    store.createIndex('lastAccess', 'lastAccess');
                }

                // Store for individual chunks (for streaming)
                if (!db.objectStoreNames.contains('chunks')) {
                    const store = db.createObjectStore('chunks', { keyPath: 'id' });
                    store.createIndex('modelId', 'modelId');
                }

                // Store for manifests
                if (!db.objectStoreNames.contains('manifests')) {
                    db.createObjectStore('manifests', { keyPath: 'modelId' });
                }
            };
        });
    }

    async get(modelId) {
        await this.open();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction('models', 'readonly');
            const store = tx.objectStore('models');
            const request = store.get(modelId);

            request.onsuccess = () => {
                const result = request.result;
                if (result) {
                    // Update last access
                    this.updateLastAccess(modelId);
                }
                resolve(result);
            };
            request.onerror = () => reject(request.error);
        });
    }

    async put(modelId, data, manifest) {
        await this.open();
        await this.ensureSpace(data.byteLength || data.length);

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction(['models', 'manifests'], 'readwrite');

            const modelStore = tx.objectStore('models');
            modelStore.put({
                id: modelId,
                data,
                hash: manifest.integrity?.merkleRoot || 'unknown',
                size: data.byteLength || data.length,
                lastAccess: Date.now(),
                cachedAt: Date.now(),
            });

            const manifestStore = tx.objectStore('manifests');
            manifestStore.put({
                modelId,
                manifest,
                cachedAt: Date.now(),
            });

            tx.oncomplete = () => resolve();
            tx.onerror = () => reject(tx.error);
        });
    }

    async getChunk(modelId, chunkIndex) {
        await this.open();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction('chunks', 'readonly');
            const store = tx.objectStore('chunks');
            const request = store.get(`${modelId}:${chunkIndex}`);

            request.onsuccess = () => resolve(request.result?.data);
            request.onerror = () => reject(request.error);
        });
    }

    async putChunk(modelId, chunkIndex, data, hash) {
        await this.open();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction('chunks', 'readwrite');
            const store = tx.objectStore('chunks');
            store.put({
                id: `${modelId}:${chunkIndex}`,
                modelId,
                chunkIndex,
                data,
                hash,
                cachedAt: Date.now(),
            });

            tx.oncomplete = () => resolve();
            tx.onerror = () => reject(tx.error);
        });
    }

    async updateLastAccess(modelId) {
        await this.open();

        return new Promise((resolve) => {
            const tx = this.db.transaction('models', 'readwrite');
            const store = tx.objectStore('models');
            const request = store.get(modelId);

            request.onsuccess = () => {
                if (request.result) {
                    request.result.lastAccess = Date.now();
                    store.put(request.result);
                }
                resolve();
            };
        });
    }

    async ensureSpace(needed) {
        await this.open();

        // Get current usage
        const estimate = await navigator.storage?.estimate?.();
        const used = estimate?.usage || 0;

        if (used + needed > this.maxCacheSize) {
            await this.evictLRU(needed);
        }
    }

    async evictLRU(needed) {
        await this.open();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction('models', 'readwrite');
            const store = tx.objectStore('models');
            const index = store.index('lastAccess');
            const request = index.openCursor();

            let freed = 0;

            request.onsuccess = (event) => {
                const cursor = event.target.result;
                if (cursor && freed < needed) {
                    freed += cursor.value.size || 0;
                    cursor.delete();
                    cursor.continue();
                } else {
                    resolve(freed);
                }
            };
            request.onerror = () => reject(request.error);
        });
    }

    async getCacheStats() {
        await this.open();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction('models', 'readonly');
            const store = tx.objectStore('models');
            const request = store.getAll();

            request.onsuccess = () => {
                const models = request.result;
                const totalSize = models.reduce((sum, m) => sum + (m.size || 0), 0);
                resolve({
                    modelCount: models.length,
                    totalSize,
                    models: models.map(m => ({
                        id: m.id,
                        size: m.size,
                        lastAccess: m.lastAccess,
                    })),
                });
            };
            request.onerror = () => reject(request.error);
        });
    }

    async clear() {
        await this.open();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction(['models', 'chunks', 'manifests'], 'readwrite');
            tx.objectStore('models').clear();
            tx.objectStore('chunks').clear();
            tx.objectStore('manifests').clear();
            tx.oncomplete = () => resolve();
            tx.onerror = () => reject(tx.error);
        });
    }
}

// ============================================================================
// MODEL LOADER
// ============================================================================

/**
 * Model loader with tiered selection and chunk verification
 */
export class ModelLoader {
    constructor(options = {}) {
        this.cache = new ModelCache(options.cache);
        this.verifier = new ManifestVerifier(options.trustRoot);
        this.registryUrl = options.registryUrl || 'https://models.ruvector.dev';

        // Loading state
        this.loadingModels = new Map();
        this.loadedModels = new Map();

        // Callbacks
        this.onProgress = options.onProgress || (() => {});
        this.onError = options.onError || console.error;

        // Source preference order
        this.sourceOrder = options.sourceOrder || ['cache', 'gcs', 'ipfs', 'p2p'];
    }

    /**
     * Fetch manifest from registry (registry only returns manifest, not URLs)
     */
    async fetchManifest(modelId) {
        // Check cache first
        const cached = await this.cache.get(modelId);
        if (cached?.manifest) {
            return cached.manifest;
        }

        // Fetch from registry
        const response = await fetch(`${this.registryUrl}/v2/manifests/${modelId}`);
        if (!response.ok) {
            throw new Error(`Failed to fetch manifest: ${response.status}`);
        }

        const manifest = await response.json();

        // Verify manifest
        const verification = this.verifier.verify(manifest);
        if (!verification.valid) {
            throw new Error(`Invalid manifest: ${verification.errors.join(', ')}`);
        }

        if (verification.warnings.length > 0) {
            console.warn('[ModelLoader] Manifest warnings:', verification.warnings);
        }

        return manifest;
    }

    /**
     * Select best model for required capabilities
     */
    async selectModel(requiredCapabilities, options = {}) {
        const tier = selectTier(requiredCapabilities, options.preferredTier);

        // Fetch model catalog for this tier
        const response = await fetch(`${this.registryUrl}/v2/catalog?tier=${tier.name}`);
        if (!response.ok) {
            throw new Error(`Failed to fetch catalog: ${response.status}`);
        }

        const catalog = await response.json();

        // Filter by capabilities
        const candidates = catalog.models.filter(m => {
            const hasCapabilities = requiredCapabilities.every(cap =>
                m.capabilities?.includes(cap)
            );
            const fitsMemory = m.memoryRequirement <= detectAvailableMemory();
            return hasCapabilities && fitsMemory;
        });

        if (candidates.length === 0) {
            throw new Error(`No model found for capabilities: ${requiredCapabilities.join(', ')}`);
        }

        // Sort by capability priority (prefer embeddings over generation)
        candidates.sort((a, b) => {
            const aPriority = Math.min(...a.capabilities.map(c => CAPABILITY_PRIORITIES[c] || 10));
            const bPriority = Math.min(...b.capabilities.map(c => CAPABILITY_PRIORITIES[c] || 10));
            return aPriority - bPriority;
        });

        return candidates[0];
    }

    /**
     * Load a model with chunk verification
     */
    async load(modelId, options = {}) {
        // Return if already loaded
        if (this.loadedModels.has(modelId)) {
            return this.loadedModels.get(modelId);
        }

        // Return existing promise if loading
        if (this.loadingModels.has(modelId)) {
            return this.loadingModels.get(modelId);
        }

        const loadPromise = this._loadInternal(modelId, options);
        this.loadingModels.set(modelId, loadPromise);

        try {
            const result = await loadPromise;
            this.loadedModels.set(modelId, result);
            return result;
        } finally {
            this.loadingModels.delete(modelId);
        }
    }

    async _loadInternal(modelId, options) {
        // 1. Get manifest
        const manifest = await this.fetchManifest(modelId);

        // 2. Memory check
        const available = detectAvailableMemory();
        if (manifest.model.memoryRequirement > available) {
            throw new Error(
                `Insufficient memory: need ${manifest.model.memoryRequirement}, have ${available}`
            );
        }

        // 3. Check cache
        const cached = await this.cache.get(modelId);
        if (cached?.data) {
            // Verify cached data against manifest
            if (cached.hash === manifest.integrity?.merkleRoot) {
                this.onProgress({ modelId, status: 'cached', progress: 1 });
                return { manifest, data: cached.data, source: 'cache' };
            }
            // Cache invalid, continue to download
        }

        // 4. Download with chunk verification
        const artifact = manifest.artifacts[0]; // Primary artifact
        const data = await this._downloadWithVerification(modelId, manifest, artifact, options);

        // 5. Cache the result
        await this.cache.put(modelId, data, manifest);

        return { manifest, data, source: options.source || 'remote' };
    }

    /**
     * Download with streaming chunk verification
     */
    async _downloadWithVerification(modelId, manifest, artifact, options) {
        const sources = this._getSourceUrls(manifest, artifact);

        for (const source of sources) {
            try {
                const data = await this._downloadFromSource(
                    modelId,
                    source,
                    manifest,
                    artifact
                );
                return data;
            } catch (error) {
                console.warn(`[ModelLoader] Source failed: ${source.type}`, error.message);
                continue;
            }
        }

        throw new Error('All download sources failed');
    }

    /**
     * Get ordered source URLs from manifest
     */
    _getSourceUrls(manifest, artifact) {
        const sources = [];
        const dist = manifest.distribution || {};

        for (const sourceType of this.sourceOrder) {
            if (sourceType === 'gcs' && dist.gcs) {
                sources.push({ type: 'gcs', url: dist.gcs });
            }
            if (sourceType === 'ipfs' && dist.ipfs) {
                sources.push({
                    type: 'ipfs',
                    url: `https://ipfs.io/ipfs/${dist.ipfs}`,
                    cid: dist.ipfs,
                });
            }
            if (sourceType === 'p2p') {
                // P2P would be handled separately
                sources.push({ type: 'p2p', url: null });
            }
        }

        // Add fallbacks
        if (dist.fallbackUrls) {
            for (const url of dist.fallbackUrls) {
                sources.push({ type: 'fallback', url });
            }
        }

        return sources;
    }

    /**
     * Download from a specific source with chunk verification
     */
    async _downloadFromSource(modelId, source, manifest, artifact) {
        if (source.type === 'p2p') {
            return this._downloadFromP2P(modelId, manifest, artifact);
        }

        const response = await fetch(source.url);
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
        }

        const contentLength = parseInt(response.headers.get('content-length') || '0');
        const chunking = manifest.integrity?.chunking;

        if (chunking && response.body) {
            // Streaming download with chunk verification
            return this._streamWithVerification(
                modelId,
                response.body,
                manifest,
                contentLength
            );
        } else {
            // Simple download
            const buffer = await response.arrayBuffer();

            // Verify full file hash
            if (artifact.sha256) {
                const hash = createHash('sha256')
                    .update(Buffer.from(buffer))
                    .digest('hex');
                if (hash !== artifact.sha256) {
                    throw new Error('File hash mismatch');
                }
            }

            return buffer;
        }
    }

    /**
     * Stream download with chunk-by-chunk verification
     */
    async _streamWithVerification(modelId, body, manifest, totalSize) {
        const chunking = manifest.integrity.chunking;
        const chunkSize = chunking.chunkSize;
        const expectedChunks = chunking.chunkCount;

        const reader = body.getReader();
        const chunks = [];
        let buffer = new Uint8Array(0);
        let chunkIndex = 0;
        let bytesReceived = 0;

        while (true) {
            const { done, value } = await reader.read();

            if (done) break;

            // Append to buffer
            const newBuffer = new Uint8Array(buffer.length + value.length);
            newBuffer.set(buffer);
            newBuffer.set(value, buffer.length);
            buffer = newBuffer;
            bytesReceived += value.length;

            // Process complete chunks
            while (buffer.length >= chunkSize || (bytesReceived === totalSize && buffer.length > 0)) {
                const isLastChunk = bytesReceived === totalSize && buffer.length <= chunkSize;
                const thisChunkSize = isLastChunk ? buffer.length : chunkSize;
                const chunkData = buffer.slice(0, thisChunkSize);
                buffer = buffer.slice(thisChunkSize);

                // Verify chunk
                const verification = this.verifier.verifyChunk(chunkData, chunkIndex, manifest);
                if (!verification.valid) {
                    throw new Error(`Chunk verification failed: ${verification.error}`);
                }

                chunks.push(chunkData);
                chunkIndex++;

                // Cache chunk for resume capability
                await this.cache.putChunk(
                    modelId,
                    chunkIndex - 1,
                    chunkData,
                    verification.hash
                );

                // Progress callback
                this.onProgress({
                    modelId,
                    status: 'downloading',
                    progress: bytesReceived / totalSize,
                    chunksVerified: chunkIndex,
                    totalChunks: expectedChunks,
                });

                if (isLastChunk) break;
            }
        }

        // Verify Merkle root
        const chunkHashes = chunking.chunkHashes;
        const computedRoot = computeMerkleRoot(chunkHashes);
        if (computedRoot !== manifest.integrity.merkleRoot) {
            throw new Error('Merkle root verification failed');
        }

        // Combine chunks
        const totalLength = chunks.reduce((sum, c) => sum + c.length, 0);
        const result = new Uint8Array(totalLength);
        let offset = 0;
        for (const chunk of chunks) {
            result.set(chunk, offset);
            offset += chunk.length;
        }

        this.onProgress({
            modelId,
            status: 'complete',
            progress: 1,
            verified: true,
        });

        return result.buffer;
    }

    /**
     * Download from P2P network (placeholder)
     */
    async _downloadFromP2P(modelId, manifest, artifact) {
        // Would integrate with WebRTC P2P network
        throw new Error('P2P download not implemented');
    }

    /**
     * Preload a model in the background
     */
    async preload(modelId) {
        try {
            await this.load(modelId);
        } catch (error) {
            console.warn(`[ModelLoader] Preload failed for ${modelId}:`, error.message);
        }
    }

    /**
     * Unload a model from memory
     */
    unload(modelId) {
        this.loadedModels.delete(modelId);
    }

    /**
     * Get cache statistics
     */
    async getCacheStats() {
        return this.cache.getCacheStats();
    }

    /**
     * Clear all cached models
     */
    async clearCache() {
        await this.cache.clear();
        this.loadedModels.clear();
    }
}

// ============================================================================
// EXPORTS
// ============================================================================

export default ModelLoader;
