/**
 * @ruvector/edge-net Model Loader
 *
 * Smart model loading with:
 * - IndexedDB caching
 * - Automatic source selection (CDN -> GCS -> IPFS -> fallback)
 * - Streaming download with progress
 * - Model validation before use
 * - Lazy loading support
 *
 * @module @ruvector/edge-net/models/model-loader
 */

import { EventEmitter } from 'events';
import { createHash, randomBytes } from 'crypto';
import { promises as fs } from 'fs';
import path from 'path';

import { ModelRegistry } from './model-registry.js';
import { DistributionManager, ProgressTracker } from './distribution.js';

// ============================================
// CONSTANTS
// ============================================

const DEFAULT_CACHE_DIR = process.env.HOME
    ? `${process.env.HOME}/.ruvector/models/cache`
    : '/tmp/.ruvector/models/cache';

const CACHE_VERSION = 1;
const MAX_CACHE_SIZE_BYTES = 10 * 1024 * 1024 * 1024; // 10GB default
const CACHE_CLEANUP_THRESHOLD = 0.9; // Cleanup when 90% full

// ============================================
// CACHE STORAGE INTERFACE
// ============================================

/**
 * Cache storage interface for different backends
 */
class CacheStorage {
    async get(key) { throw new Error('Not implemented'); }
    async set(key, value, metadata) { throw new Error('Not implemented'); }
    async delete(key) { throw new Error('Not implemented'); }
    async has(key) { throw new Error('Not implemented'); }
    async list() { throw new Error('Not implemented'); }
    async getMetadata(key) { throw new Error('Not implemented'); }
    async clear() { throw new Error('Not implemented'); }
    async getSize() { throw new Error('Not implemented'); }
}

// ============================================
// FILE SYSTEM CACHE
// ============================================

/**
 * File system-based cache storage for Node.js
 */
class FileSystemCache extends CacheStorage {
    constructor(cacheDir) {
        super();
        this.cacheDir = cacheDir;
        this.metadataDir = path.join(cacheDir, '.metadata');
        this.initialized = false;
    }

    async init() {
        if (this.initialized) return;
        await fs.mkdir(this.cacheDir, { recursive: true });
        await fs.mkdir(this.metadataDir, { recursive: true });
        this.initialized = true;
    }

    _getFilePath(key) {
        // Sanitize key for filesystem
        const safeKey = key.replace(/[^a-zA-Z0-9._-]/g, '_');
        return path.join(this.cacheDir, safeKey);
    }

    _getMetadataPath(key) {
        const safeKey = key.replace(/[^a-zA-Z0-9._-]/g, '_');
        return path.join(this.metadataDir, `${safeKey}.json`);
    }

    async get(key) {
        await this.init();
        const filePath = this._getFilePath(key);

        try {
            const data = await fs.readFile(filePath);

            // Update access time in metadata
            await this._updateAccessTime(key);

            return data;
        } catch (error) {
            if (error.code === 'ENOENT') return null;
            throw error;
        }
    }

    async set(key, value, metadata = {}) {
        await this.init();
        const filePath = this._getFilePath(key);
        const metadataPath = this._getMetadataPath(key);

        // Write data
        await fs.writeFile(filePath, value);

        // Write metadata
        const fullMetadata = {
            key,
            size: value.length,
            hash: `sha256:${createHash('sha256').update(value).digest('hex')}`,
            createdAt: new Date().toISOString(),
            accessedAt: new Date().toISOString(),
            accessCount: 1,
            cacheVersion: CACHE_VERSION,
            ...metadata,
        };

        await fs.writeFile(metadataPath, JSON.stringify(fullMetadata, null, 2));

        return fullMetadata;
    }

    async delete(key) {
        await this.init();
        const filePath = this._getFilePath(key);
        const metadataPath = this._getMetadataPath(key);

        try {
            await fs.unlink(filePath);
            await fs.unlink(metadataPath).catch(() => {});
            return true;
        } catch (error) {
            if (error.code === 'ENOENT') return false;
            throw error;
        }
    }

    async has(key) {
        await this.init();
        const filePath = this._getFilePath(key);

        try {
            await fs.access(filePath);
            return true;
        } catch {
            return false;
        }
    }

    async list() {
        await this.init();

        try {
            const files = await fs.readdir(this.cacheDir);
            return files.filter(f => !f.startsWith('.'));
        } catch {
            return [];
        }
    }

    async getMetadata(key) {
        await this.init();
        const metadataPath = this._getMetadataPath(key);

        try {
            const data = await fs.readFile(metadataPath, 'utf-8');
            return JSON.parse(data);
        } catch {
            return null;
        }
    }

    async _updateAccessTime(key) {
        const metadataPath = this._getMetadataPath(key);

        try {
            const data = await fs.readFile(metadataPath, 'utf-8');
            const metadata = JSON.parse(data);

            metadata.accessedAt = new Date().toISOString();
            metadata.accessCount = (metadata.accessCount || 0) + 1;

            await fs.writeFile(metadataPath, JSON.stringify(metadata, null, 2));
        } catch {
            // Ignore metadata update errors
        }
    }

    async clear() {
        await this.init();
        const files = await this.list();

        for (const file of files) {
            await this.delete(file);
        }
    }

    async getSize() {
        await this.init();
        const files = await this.list();
        let totalSize = 0;

        for (const file of files) {
            const filePath = this._getFilePath(file);
            try {
                const stats = await fs.stat(filePath);
                totalSize += stats.size;
            } catch {
                // Ignore missing files
            }
        }

        return totalSize;
    }

    async getEntriesWithMetadata() {
        await this.init();
        const files = await this.list();
        const entries = [];

        for (const file of files) {
            const metadata = await this.getMetadata(file);
            if (metadata) {
                entries.push(metadata);
            }
        }

        return entries;
    }
}

// ============================================
// INDEXEDDB CACHE (BROWSER)
// ============================================

/**
 * IndexedDB-based cache storage for browsers
 */
class IndexedDBCache extends CacheStorage {
    constructor(dbName = 'ruvector-models') {
        super();
        this.dbName = dbName;
        this.storeName = 'models';
        this.metadataStoreName = 'metadata';
        this.db = null;
    }

    async init() {
        if (this.db) return;

        if (typeof indexedDB === 'undefined') {
            throw new Error('IndexedDB not available');
        }

        return new Promise((resolve, reject) => {
            const request = indexedDB.open(this.dbName, CACHE_VERSION);

            request.onerror = () => reject(request.error);

            request.onsuccess = () => {
                this.db = request.result;
                resolve();
            };

            request.onupgradeneeded = (event) => {
                const db = event.target.result;

                // Models store
                if (!db.objectStoreNames.contains(this.storeName)) {
                    db.createObjectStore(this.storeName);
                }

                // Metadata store
                if (!db.objectStoreNames.contains(this.metadataStoreName)) {
                    const metaStore = db.createObjectStore(this.metadataStoreName);
                    metaStore.createIndex('accessedAt', 'accessedAt');
                    metaStore.createIndex('size', 'size');
                }
            };
        });
    }

    async get(key) {
        await this.init();

        return new Promise((resolve, reject) => {
            const transaction = this.db.transaction([this.storeName], 'readonly');
            const store = transaction.objectStore(this.storeName);
            const request = store.get(key);

            request.onerror = () => reject(request.error);
            request.onsuccess = () => {
                if (request.result) {
                    this._updateAccessTime(key);
                }
                resolve(request.result || null);
            };
        });
    }

    async set(key, value, metadata = {}) {
        await this.init();

        const fullMetadata = {
            key,
            size: value.length || value.byteLength,
            hash: await this._computeHash(value),
            createdAt: new Date().toISOString(),
            accessedAt: new Date().toISOString(),
            accessCount: 1,
            cacheVersion: CACHE_VERSION,
            ...metadata,
        };

        return new Promise((resolve, reject) => {
            const transaction = this.db.transaction(
                [this.storeName, this.metadataStoreName],
                'readwrite'
            );

            const modelStore = transaction.objectStore(this.storeName);
            const metaStore = transaction.objectStore(this.metadataStoreName);

            modelStore.put(value, key);
            metaStore.put(fullMetadata, key);

            transaction.oncomplete = () => resolve(fullMetadata);
            transaction.onerror = () => reject(transaction.error);
        });
    }

    async _computeHash(data) {
        if (typeof crypto !== 'undefined' && crypto.subtle) {
            const buffer = data instanceof ArrayBuffer ? data : data.buffer;
            const hashBuffer = await crypto.subtle.digest('SHA-256', buffer);
            const hashArray = Array.from(new Uint8Array(hashBuffer));
            const hashHex = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
            return `sha256:${hashHex}`;
        }
        return null;
    }

    async delete(key) {
        await this.init();

        return new Promise((resolve, reject) => {
            const transaction = this.db.transaction(
                [this.storeName, this.metadataStoreName],
                'readwrite'
            );

            transaction.objectStore(this.storeName).delete(key);
            transaction.objectStore(this.metadataStoreName).delete(key);

            transaction.oncomplete = () => resolve(true);
            transaction.onerror = () => reject(transaction.error);
        });
    }

    async has(key) {
        const value = await this.get(key);
        return value !== null;
    }

    async list() {
        await this.init();

        return new Promise((resolve, reject) => {
            const transaction = this.db.transaction([this.storeName], 'readonly');
            const store = transaction.objectStore(this.storeName);
            const request = store.getAllKeys();

            request.onerror = () => reject(request.error);
            request.onsuccess = () => resolve(request.result);
        });
    }

    async getMetadata(key) {
        await this.init();

        return new Promise((resolve, reject) => {
            const transaction = this.db.transaction([this.metadataStoreName], 'readonly');
            const store = transaction.objectStore(this.metadataStoreName);
            const request = store.get(key);

            request.onerror = () => reject(request.error);
            request.onsuccess = () => resolve(request.result || null);
        });
    }

    async _updateAccessTime(key) {
        const metadata = await this.getMetadata(key);
        if (!metadata) return;

        metadata.accessedAt = new Date().toISOString();
        metadata.accessCount = (metadata.accessCount || 0) + 1;

        const transaction = this.db.transaction([this.metadataStoreName], 'readwrite');
        transaction.objectStore(this.metadataStoreName).put(metadata, key);
    }

    async clear() {
        await this.init();

        return new Promise((resolve, reject) => {
            const transaction = this.db.transaction(
                [this.storeName, this.metadataStoreName],
                'readwrite'
            );

            transaction.objectStore(this.storeName).clear();
            transaction.objectStore(this.metadataStoreName).clear();

            transaction.oncomplete = () => resolve();
            transaction.onerror = () => reject(transaction.error);
        });
    }

    async getSize() {
        await this.init();

        return new Promise((resolve, reject) => {
            const transaction = this.db.transaction([this.metadataStoreName], 'readonly');
            const store = transaction.objectStore(this.metadataStoreName);
            const request = store.getAll();

            request.onerror = () => reject(request.error);
            request.onsuccess = () => {
                const totalSize = request.result.reduce((sum, meta) => sum + (meta.size || 0), 0);
                resolve(totalSize);
            };
        });
    }
}

// ============================================
// MODEL LOADER
// ============================================

/**
 * ModelLoader - Smart model loading with caching
 */
export class ModelLoader extends EventEmitter {
    /**
     * Create a new ModelLoader
     * @param {object} options - Configuration options
     */
    constructor(options = {}) {
        super();

        this.id = `loader-${randomBytes(6).toString('hex')}`;

        // Create registry if not provided
        this.registry = options.registry || new ModelRegistry({
            registryPath: options.registryPath,
        });

        // Create distribution manager if not provided
        this.distribution = options.distribution || new DistributionManager({
            gcsBucket: options.gcsBucket,
            gcsProjectId: options.gcsProjectId,
            cdnBaseUrl: options.cdnBaseUrl,
            ipfsGateway: options.ipfsGateway,
        });

        // Cache configuration
        this.cacheDir = options.cacheDir || DEFAULT_CACHE_DIR;
        this.maxCacheSize = options.maxCacheSize || MAX_CACHE_SIZE_BYTES;

        // Initialize cache storage based on environment
        this.cache = this._createCacheStorage(options);

        // Loaded models (in-memory)
        this.loadedModels = new Map();

        // Loading promises (prevent duplicate loads)
        this.loadingPromises = new Map();

        // Lazy load queue
        this.lazyLoadQueue = [];
        this.lazyLoadActive = false;

        // Stats
        this.stats = {
            cacheHits: 0,
            cacheMisses: 0,
            downloads: 0,
            validationErrors: 0,
            lazyLoads: 0,
        };
    }

    /**
     * Create appropriate cache storage for environment
     * @private
     */
    _createCacheStorage(options) {
        // Browser environment
        if (typeof window !== 'undefined' && typeof indexedDB !== 'undefined') {
            return new IndexedDBCache(options.dbName || 'ruvector-models');
        }

        // Node.js environment
        return new FileSystemCache(this.cacheDir);
    }

    /**
     * Initialize the loader
     */
    async initialize() {
        // Initialize cache
        if (this.cache.init) {
            await this.cache.init();
        }

        // Load registry if path provided
        if (this.registry.registryPath) {
            try {
                await this.registry.load();
            } catch (error) {
                this.emit('warning', { message: 'Failed to load registry', error });
            }
        }

        this.emit('initialized', { loaderId: this.id });

        return this;
    }

    /**
     * Get cache key for a model
     * @private
     */
    _getCacheKey(name, version) {
        return `${name}@${version}`;
    }

    /**
     * Load a model
     * @param {string} name - Model name
     * @param {string} version - Version (default: latest)
     * @param {object} options - Load options
     * @returns {Promise<Buffer|Uint8Array>}
     */
    async load(name, version = 'latest', options = {}) {
        const key = this._getCacheKey(name, version);

        // Return cached in-memory model
        if (this.loadedModels.has(key) && !options.forceReload) {
            this.stats.cacheHits++;
            return this.loadedModels.get(key);
        }

        // Return existing loading promise
        if (this.loadingPromises.has(key)) {
            return this.loadingPromises.get(key);
        }

        // Start loading
        const loadPromise = this._loadModel(name, version, options);
        this.loadingPromises.set(key, loadPromise);

        try {
            const model = await loadPromise;
            this.loadedModels.set(key, model);
            return model;
        } finally {
            this.loadingPromises.delete(key);
        }
    }

    /**
     * Internal model loading logic
     * @private
     */
    async _loadModel(name, version, options = {}) {
        const { onProgress, skipCache = false, skipValidation = false } = options;

        // Get metadata from registry
        let metadata = this.registry.get(name, version);

        if (!metadata) {
            // Try to fetch from remote registry
            this.emit('warning', { message: `Model ${name}@${version} not in local registry` });
            throw new Error(`Model not found: ${name}@${version}`);
        }

        const resolvedVersion = metadata.version;
        const key = this._getCacheKey(name, resolvedVersion);

        // Check cache first (unless skipped)
        if (!skipCache) {
            const cached = await this.cache.get(key);
            if (cached) {
                // Validate cached data
                if (!skipValidation && metadata.hash) {
                    const isValid = this.distribution.verifyIntegrity(cached, metadata.hash);
                    if (isValid) {
                        this.stats.cacheHits++;
                        this.emit('cache_hit', { name, version: resolvedVersion });
                        return cached;
                    } else {
                        this.stats.validationErrors++;
                        this.emit('cache_invalid', { name, version: resolvedVersion });
                        await this.cache.delete(key);
                    }
                } else {
                    this.stats.cacheHits++;
                    return cached;
                }
            }
        }

        this.stats.cacheMisses++;

        // Download model
        this.emit('download_start', { name, version: resolvedVersion });

        const data = await this.distribution.download(metadata, {
            onProgress: (progress) => {
                this.emit('progress', { name, version: resolvedVersion, ...progress });
                if (onProgress) onProgress(progress);
            },
        });

        // Validate downloaded data
        if (!skipValidation) {
            const validation = this.distribution.verifyModel(data, metadata);
            if (!validation.valid) {
                this.stats.validationErrors++;
                throw new Error(`Model validation failed: ${JSON.stringify(validation.checks)}`);
            }
        }

        // Store in cache
        await this.cache.set(key, data, {
            modelName: name,
            version: resolvedVersion,
            format: metadata.format,
        });

        this.stats.downloads++;
        this.emit('loaded', { name, version: resolvedVersion, size: data.length });

        // Cleanup cache if needed
        await this._cleanupCacheIfNeeded();

        return data;
    }

    /**
     * Lazy load a model (load in background)
     * @param {string} name - Model name
     * @param {string} version - Version
     * @param {object} options - Load options
     */
    async lazyLoad(name, version = 'latest', options = {}) {
        const key = this._getCacheKey(name, version);

        // Already loaded or loading
        if (this.loadedModels.has(key) || this.loadingPromises.has(key)) {
            return;
        }

        // Check cache
        const cached = await this.cache.has(key);
        if (cached) {
            return; // Already in cache
        }

        // Add to queue
        this.lazyLoadQueue.push({ name, version, options });
        this.stats.lazyLoads++;

        this.emit('lazy_queued', { name, version, queueLength: this.lazyLoadQueue.length });

        // Start processing if not active
        if (!this.lazyLoadActive) {
            this._processLazyLoadQueue();
        }
    }

    /**
     * Process lazy load queue
     * @private
     */
    async _processLazyLoadQueue() {
        if (this.lazyLoadActive || this.lazyLoadQueue.length === 0) return;

        this.lazyLoadActive = true;

        while (this.lazyLoadQueue.length > 0) {
            const { name, version, options } = this.lazyLoadQueue.shift();

            try {
                await this.load(name, version, {
                    ...options,
                    lazy: true,
                });
            } catch (error) {
                this.emit('lazy_error', { name, version, error: error.message });
            }

            // Small delay between lazy loads
            await new Promise(resolve => setTimeout(resolve, 100));
        }

        this.lazyLoadActive = false;
    }

    /**
     * Preload multiple models
     * @param {Array<{name: string, version?: string}>} models - Models to preload
     */
    async preload(models) {
        const results = await Promise.allSettled(
            models.map(({ name, version }) => this.load(name, version || 'latest'))
        );

        return {
            total: models.length,
            loaded: results.filter(r => r.status === 'fulfilled').length,
            failed: results.filter(r => r.status === 'rejected').length,
            results,
        };
    }

    /**
     * Check if a model is loaded in memory
     * @param {string} name - Model name
     * @param {string} version - Version
     * @returns {boolean}
     */
    isLoaded(name, version = 'latest') {
        const metadata = this.registry.get(name, version);
        if (!metadata) return false;

        const key = this._getCacheKey(name, metadata.version);
        return this.loadedModels.has(key);
    }

    /**
     * Check if a model is cached on disk
     * @param {string} name - Model name
     * @param {string} version - Version
     * @returns {Promise<boolean>}
     */
    async isCached(name, version = 'latest') {
        const metadata = this.registry.get(name, version);
        if (!metadata) return false;

        const key = this._getCacheKey(name, metadata.version);
        return this.cache.has(key);
    }

    /**
     * Unload a model from memory
     * @param {string} name - Model name
     * @param {string} version - Version
     */
    unload(name, version = 'latest') {
        const metadata = this.registry.get(name, version);
        if (!metadata) return false;

        const key = this._getCacheKey(name, metadata.version);
        return this.loadedModels.delete(key);
    }

    /**
     * Unload all models from memory
     */
    unloadAll() {
        const count = this.loadedModels.size;
        this.loadedModels.clear();
        return count;
    }

    /**
     * Remove a model from cache
     * @param {string} name - Model name
     * @param {string} version - Version
     */
    async removeFromCache(name, version = 'latest') {
        const metadata = this.registry.get(name, version);
        if (!metadata) return false;

        const key = this._getCacheKey(name, metadata.version);
        return this.cache.delete(key);
    }

    /**
     * Clear all cached models
     */
    async clearCache() {
        await this.cache.clear();
        this.emit('cache_cleared');
    }

    /**
     * Cleanup cache if over size limit
     * @private
     */
    async _cleanupCacheIfNeeded() {
        const currentSize = await this.cache.getSize();
        const threshold = this.maxCacheSize * CACHE_CLEANUP_THRESHOLD;

        if (currentSize < threshold) return;

        this.emit('cache_cleanup_start', { currentSize, maxSize: this.maxCacheSize });

        // Get entries sorted by last access time
        let entries;
        if (this.cache.getEntriesWithMetadata) {
            entries = await this.cache.getEntriesWithMetadata();
        } else {
            const keys = await this.cache.list();
            entries = [];
            for (const key of keys) {
                const meta = await this.cache.getMetadata(key);
                if (meta) entries.push(meta);
            }
        }

        // Sort by access time (oldest first)
        entries.sort((a, b) =>
            new Date(a.accessedAt).getTime() - new Date(b.accessedAt).getTime()
        );

        // Remove oldest entries until under 80% capacity
        const targetSize = this.maxCacheSize * 0.8;
        let removedSize = 0;
        let removedCount = 0;

        for (const entry of entries) {
            if (currentSize - removedSize <= targetSize) break;

            await this.cache.delete(entry.key);
            removedSize += entry.size;
            removedCount++;
        }

        this.emit('cache_cleanup_complete', {
            removedCount,
            removedSize,
            newSize: currentSize - removedSize,
        });
    }

    /**
     * Get cache statistics
     * @returns {Promise<object>}
     */
    async getCacheStats() {
        const size = await this.cache.getSize();
        const keys = await this.cache.list();

        return {
            entries: keys.length,
            sizeBytes: size,
            sizeMB: Math.round(size / (1024 * 1024) * 100) / 100,
            maxSizeBytes: this.maxCacheSize,
            usagePercent: Math.round((size / this.maxCacheSize) * 100),
        };
    }

    /**
     * Get loader statistics
     * @returns {object}
     */
    getStats() {
        return {
            ...this.stats,
            loadedModels: this.loadedModels.size,
            pendingLoads: this.loadingPromises.size,
            lazyQueueLength: this.lazyLoadQueue.length,
            hitRate: this.stats.cacheHits + this.stats.cacheMisses > 0
                ? Math.round(
                    (this.stats.cacheHits / (this.stats.cacheHits + this.stats.cacheMisses)) * 100
                )
                : 0,
        };
    }

    /**
     * Get model from registry (without loading)
     * @param {string} name - Model name
     * @param {string} version - Version
     * @returns {object|null}
     */
    getModelInfo(name, version = 'latest') {
        return this.registry.get(name, version);
    }

    /**
     * Search for models
     * @param {object} criteria - Search criteria
     * @returns {Array}
     */
    searchModels(criteria) {
        return this.registry.search(criteria);
    }

    /**
     * List all available models
     * @returns {string[]}
     */
    listModels() {
        return this.registry.listModels();
    }
}

// ============================================
// EXPORTS
// ============================================

export { FileSystemCache, IndexedDBCache, CacheStorage };

export default ModelLoader;
