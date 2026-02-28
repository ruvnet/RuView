"use strict";
/**
 * IndexedDB-based persistence layer for DAG storage
 * Provides browser-compatible persistent storage for DAGs
 *
 * @performance Single-transaction pattern for atomic operations
 * @security ID validation to prevent injection
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.MemoryStorage = exports.DagStorage = void 0;
exports.isIndexedDBAvailable = isIndexedDBAvailable;
exports.createStorage = createStorage;
const DB_NAME = 'rudag-storage';
const DB_VERSION = 1;
const STORE_NAME = 'dags';
/**
 * Validate storage ID
 * @security Prevents injection attacks via ID
 */
function isValidStorageId(id) {
    if (typeof id !== 'string' || id.length === 0 || id.length > 256)
        return false;
    return /^[a-zA-Z0-9_-]+$/.test(id);
}
/**
 * Check if IndexedDB is available (browser environment)
 */
function isIndexedDBAvailable() {
    return typeof indexedDB !== 'undefined';
}
/**
 * IndexedDB storage class for DAG persistence
 *
 * @performance Uses single-transaction pattern for save operations
 */
class DagStorage {
    constructor(options = {}) {
        this.db = null;
        this.initialized = false;
        this.dbName = options.dbName || DB_NAME;
        this.version = options.version || DB_VERSION;
    }
    /**
     * Initialize the database connection
     * @throws {Error} If IndexedDB is not available
     * @throws {Error} If database is blocked by another tab
     */
    async init() {
        if (this.initialized && this.db)
            return;
        if (!isIndexedDBAvailable()) {
            throw new Error('IndexedDB is not available in this environment');
        }
        return new Promise((resolve, reject) => {
            const request = indexedDB.open(this.dbName, this.version);
            request.onerror = () => {
                reject(new Error(`Failed to open database: ${request.error?.message || 'Unknown error'}`));
            };
            request.onblocked = () => {
                reject(new Error('Database blocked - please close other tabs using this application'));
            };
            request.onsuccess = () => {
                this.db = request.result;
                this.initialized = true;
                // Handle connection errors after open
                this.db.onerror = (event) => {
                    console.error('[DagStorage] Database error:', event);
                };
                // Handle version change (another tab upgraded)
                this.db.onversionchange = () => {
                    this.db?.close();
                    this.db = null;
                    this.initialized = false;
                    console.warn('[DagStorage] Database version changed - connection closed');
                };
                resolve();
            };
            request.onupgradeneeded = (event) => {
                const db = event.target.result;
                if (!db.objectStoreNames.contains(STORE_NAME)) {
                    const store = db.createObjectStore(STORE_NAME, { keyPath: 'id' });
                    store.createIndex('name', 'name', { unique: false });
                    store.createIndex('createdAt', 'createdAt', { unique: false });
                    store.createIndex('updatedAt', 'updatedAt', { unique: false });
                }
            };
        });
    }
    /**
     * Ensure database is initialized
     * @throws {Error} If database not initialized
     */
    ensureInit() {
        if (!this.db) {
            throw new Error('Database not initialized. Call init() first.');
        }
        return this.db;
    }
    /**
     * Save a DAG to storage (single-transaction pattern)
     * @performance Uses single transaction for atomic read-modify-write
     */
    async save(id, data, options = {}) {
        if (!isValidStorageId(id)) {
            throw new Error(`Invalid storage ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
        }
        const db = this.ensureInit();
        const now = Date.now();
        return new Promise((resolve, reject) => {
            const transaction = db.transaction([STORE_NAME], 'readwrite');
            const store = transaction.objectStore(STORE_NAME);
            // First, get existing record in same transaction
            const getRequest = store.get(id);
            getRequest.onsuccess = () => {
                const existing = getRequest.result;
                const record = {
                    id,
                    name: options.name,
                    data,
                    createdAt: existing?.createdAt || now,
                    updatedAt: now,
                    metadata: options.metadata,
                };
                // Put in same transaction
                const putRequest = store.put(record);
                putRequest.onsuccess = () => resolve(record);
                putRequest.onerror = () => reject(new Error(`Failed to save DAG: ${putRequest.error?.message}`));
            };
            getRequest.onerror = () => {
                reject(new Error(`Failed to check existing DAG: ${getRequest.error?.message}`));
            };
            transaction.onerror = () => {
                reject(new Error(`Transaction failed: ${transaction.error?.message}`));
            };
        });
    }
    /**
     * Save multiple DAGs in a single transaction (batch operation)
     * @performance Much faster than individual saves for bulk operations
     */
    async saveBatch(dags) {
        for (const dag of dags) {
            if (!isValidStorageId(dag.id)) {
                throw new Error(`Invalid storage ID: "${dag.id}". Must be alphanumeric with dashes/underscores only.`);
            }
        }
        const db = this.ensureInit();
        const now = Date.now();
        return new Promise((resolve, reject) => {
            const transaction = db.transaction([STORE_NAME], 'readwrite');
            const store = transaction.objectStore(STORE_NAME);
            const results = [];
            let completed = 0;
            for (const dag of dags) {
                const getRequest = store.get(dag.id);
                getRequest.onsuccess = () => {
                    const existing = getRequest.result;
                    const record = {
                        id: dag.id,
                        name: dag.name,
                        data: dag.data,
                        createdAt: existing?.createdAt || now,
                        updatedAt: now,
                        metadata: dag.metadata,
                    };
                    store.put(record);
                    results.push(record);
                    completed++;
                };
            }
            transaction.oncomplete = () => resolve(results);
            transaction.onerror = () => reject(new Error(`Batch save failed: ${transaction.error?.message}`));
        });
    }
    /**
     * Get a DAG from storage
     */
    async get(id) {
        if (!isValidStorageId(id)) {
            throw new Error(`Invalid storage ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
        }
        const db = this.ensureInit();
        return new Promise((resolve, reject) => {
            const transaction = db.transaction([STORE_NAME], 'readonly');
            const store = transaction.objectStore(STORE_NAME);
            const request = store.get(id);
            request.onsuccess = () => resolve(request.result || null);
            request.onerror = () => reject(new Error(`Failed to get DAG: ${request.error?.message}`));
        });
    }
    /**
     * Delete a DAG from storage
     */
    async delete(id) {
        if (!isValidStorageId(id)) {
            throw new Error(`Invalid storage ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
        }
        const db = this.ensureInit();
        return new Promise((resolve, reject) => {
            const transaction = db.transaction([STORE_NAME], 'readwrite');
            const store = transaction.objectStore(STORE_NAME);
            const request = store.delete(id);
            request.onsuccess = () => resolve(true);
            request.onerror = () => reject(new Error(`Failed to delete DAG: ${request.error?.message}`));
        });
    }
    /**
     * List all DAGs in storage
     */
    async list() {
        const db = this.ensureInit();
        return new Promise((resolve, reject) => {
            const transaction = db.transaction([STORE_NAME], 'readonly');
            const store = transaction.objectStore(STORE_NAME);
            const request = store.getAll();
            request.onsuccess = () => resolve(request.result);
            request.onerror = () => reject(new Error(`Failed to list DAGs: ${request.error?.message}`));
        });
    }
    /**
     * Search DAGs by name
     */
    async findByName(name) {
        const db = this.ensureInit();
        return new Promise((resolve, reject) => {
            const transaction = db.transaction([STORE_NAME], 'readonly');
            const store = transaction.objectStore(STORE_NAME);
            const index = store.index('name');
            const request = index.getAll(name);
            request.onsuccess = () => resolve(request.result);
            request.onerror = () => reject(new Error(`Failed to find DAGs by name: ${request.error?.message}`));
        });
    }
    /**
     * Clear all DAGs from storage
     */
    async clear() {
        const db = this.ensureInit();
        return new Promise((resolve, reject) => {
            const transaction = db.transaction([STORE_NAME], 'readwrite');
            const store = transaction.objectStore(STORE_NAME);
            const request = store.clear();
            request.onsuccess = () => resolve();
            request.onerror = () => reject(new Error(`Failed to clear storage: ${request.error?.message}`));
        });
    }
    /**
     * Get storage statistics
     */
    async stats() {
        const dags = await this.list();
        const totalSize = dags.reduce((sum, dag) => sum + dag.data.byteLength, 0);
        return { count: dags.length, totalSize };
    }
    /**
     * Close the database connection
     */
    close() {
        if (this.db) {
            this.db.close();
            this.db = null;
        }
        this.initialized = false;
    }
}
exports.DagStorage = DagStorage;
/**
 * In-memory storage fallback for Node.js or environments without IndexedDB
 */
class MemoryStorage {
    constructor() {
        this.store = new Map();
        this.initialized = false;
    }
    async init() {
        this.initialized = true;
    }
    async save(id, data, options = {}) {
        if (!isValidStorageId(id)) {
            throw new Error(`Invalid storage ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
        }
        const now = Date.now();
        const existing = this.store.get(id);
        const record = {
            id,
            name: options.name,
            data,
            createdAt: existing?.createdAt || now,
            updatedAt: now,
            metadata: options.metadata,
        };
        this.store.set(id, record);
        return record;
    }
    async saveBatch(dags) {
        return Promise.all(dags.map(dag => this.save(dag.id, dag.data, { name: dag.name, metadata: dag.metadata })));
    }
    async get(id) {
        if (!isValidStorageId(id)) {
            throw new Error(`Invalid storage ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
        }
        return this.store.get(id) || null;
    }
    async delete(id) {
        if (!isValidStorageId(id)) {
            throw new Error(`Invalid storage ID: "${id}". Must be alphanumeric with dashes/underscores only.`);
        }
        return this.store.delete(id);
    }
    async list() {
        return Array.from(this.store.values());
    }
    async findByName(name) {
        return Array.from(this.store.values()).filter(dag => dag.name === name);
    }
    async clear() {
        this.store.clear();
    }
    async stats() {
        const dags = Array.from(this.store.values());
        const totalSize = dags.reduce((sum, dag) => sum + dag.data.byteLength, 0);
        return { count: dags.length, totalSize };
    }
    close() {
        this.initialized = false;
    }
}
exports.MemoryStorage = MemoryStorage;
/**
 * Create appropriate storage based on environment
 */
function createStorage(options = {}) {
    if (isIndexedDBAvailable()) {
        return new DagStorage(options);
    }
    return new MemoryStorage();
}
//# sourceMappingURL=storage.js.map