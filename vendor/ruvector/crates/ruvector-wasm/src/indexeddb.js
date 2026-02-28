/**
 * IndexedDB Persistence Layer for Ruvector
 *
 * Provides:
 * - Save/load database state to IndexedDB
 * - Batch operations for performance
 * - Progressive loading with pagination
 * - LRU cache for hot vectors
 */

const DB_NAME = 'ruvector_storage';
const DB_VERSION = 1;
const VECTOR_STORE = 'vectors';
const META_STORE = 'metadata';

/**
 * LRU Cache for hot vectors
 */
class LRUCache {
  constructor(capacity = 1000) {
    this.capacity = capacity;
    this.cache = new Map();
  }

  get(key) {
    if (!this.cache.has(key)) return null;

    // Move to end (most recently used)
    const value = this.cache.get(key);
    this.cache.delete(key);
    this.cache.set(key, value);

    return value;
  }

  set(key, value) {
    // Remove if exists
    if (this.cache.has(key)) {
      this.cache.delete(key);
    }

    // Add to end
    this.cache.set(key, value);

    // Evict oldest if over capacity
    if (this.cache.size > this.capacity) {
      const firstKey = this.cache.keys().next().value;
      this.cache.delete(firstKey);
    }
  }

  has(key) {
    return this.cache.has(key);
  }

  clear() {
    this.cache.clear();
  }

  get size() {
    return this.cache.size;
  }
}

/**
 * IndexedDB Persistence Manager
 */
export class IndexedDBPersistence {
  constructor(dbName = null) {
    this.dbName = dbName || DB_NAME;
    this.db = null;
    this.cache = new LRUCache(1000);
  }

  /**
   * Open IndexedDB connection
   */
  async open() {
    return new Promise((resolve, reject) => {
      const request = indexedDB.open(this.dbName, DB_VERSION);

      request.onerror = () => reject(request.error);
      request.onsuccess = () => {
        this.db = request.result;
        resolve(this.db);
      };

      request.onupgradeneeded = (event) => {
        const db = event.target.result;

        // Create object stores if they don't exist
        if (!db.objectStoreNames.contains(VECTOR_STORE)) {
          const vectorStore = db.createObjectStore(VECTOR_STORE, { keyPath: 'id' });
          vectorStore.createIndex('timestamp', 'timestamp', { unique: false });
        }

        if (!db.objectStoreNames.contains(META_STORE)) {
          db.createObjectStore(META_STORE, { keyPath: 'key' });
        }
      };
    });
  }

  /**
   * Save a single vector
   */
  async saveVector(id, vector, metadata = null) {
    if (!this.db) await this.open();

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction([VECTOR_STORE], 'readwrite');
      const store = transaction.objectStore(VECTOR_STORE);

      const data = {
        id,
        vector: Array.from(vector), // Convert Float32Array to regular array
        metadata,
        timestamp: Date.now()
      };

      const request = store.put(data);

      request.onsuccess = () => {
        this.cache.set(id, data);
        resolve(id);
      };
      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Save vectors in batch (more efficient)
   */
  async saveBatch(entries, batchSize = 100) {
    if (!this.db) await this.open();

    const chunks = [];
    for (let i = 0; i < entries.length; i += batchSize) {
      chunks.push(entries.slice(i, i + batchSize));
    }

    for (const chunk of chunks) {
      await new Promise((resolve, reject) => {
        const transaction = this.db.transaction([VECTOR_STORE], 'readwrite');
        const store = transaction.objectStore(VECTOR_STORE);

        for (const entry of chunk) {
          const data = {
            id: entry.id,
            vector: Array.from(entry.vector),
            metadata: entry.metadata,
            timestamp: Date.now()
          };

          store.put(data);
          this.cache.set(entry.id, data);
        }

        transaction.oncomplete = () => resolve();
        transaction.onerror = () => reject(transaction.error);
      });
    }

    return entries.length;
  }

  /**
   * Load a single vector by ID
   */
  async loadVector(id) {
    // Check cache first
    if (this.cache.has(id)) {
      return this.cache.get(id);
    }

    if (!this.db) await this.open();

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction([VECTOR_STORE], 'readonly');
      const store = transaction.objectStore(VECTOR_STORE);
      const request = store.get(id);

      request.onsuccess = () => {
        const data = request.result;
        if (data) {
          // Convert array back to Float32Array
          data.vector = new Float32Array(data.vector);
          this.cache.set(id, data);
        }
        resolve(data);
      };
      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Load all vectors (with progressive loading)
   */
  async loadAll(onProgress = null, batchSize = 100) {
    if (!this.db) await this.open();

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction([VECTOR_STORE], 'readonly');
      const store = transaction.objectStore(VECTOR_STORE);
      const request = store.openCursor();

      const vectors = [];
      let count = 0;

      request.onsuccess = (event) => {
        const cursor = event.target.result;

        if (cursor) {
          const data = cursor.value;
          data.vector = new Float32Array(data.vector);
          vectors.push(data);
          count++;

          // Cache hot vectors (first 1000)
          if (count <= 1000) {
            this.cache.set(data.id, data);
          }

          // Report progress every batch
          if (onProgress && count % batchSize === 0) {
            onProgress({
              loaded: count,
              vectors: [...vectors]
            });
            vectors.length = 0; // Clear batch
          }

          cursor.continue();
        } else {
          // Done
          if (onProgress && vectors.length > 0) {
            onProgress({
              loaded: count,
              vectors: vectors,
              complete: true
            });
          }
          resolve({ count, complete: true });
        }
      };

      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Delete a vector by ID
   */
  async deleteVector(id) {
    if (!this.db) await this.open();

    this.cache.delete(id);

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction([VECTOR_STORE], 'readwrite');
      const store = transaction.objectStore(VECTOR_STORE);
      const request = store.delete(id);

      request.onsuccess = () => resolve(true);
      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Clear all vectors
   */
  async clear() {
    if (!this.db) await this.open();

    this.cache.clear();

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction([VECTOR_STORE], 'readwrite');
      const store = transaction.objectStore(VECTOR_STORE);
      const request = store.clear();

      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Get database statistics
   */
  async getStats() {
    if (!this.db) await this.open();

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction([VECTOR_STORE], 'readonly');
      const store = transaction.objectStore(VECTOR_STORE);
      const request = store.count();

      request.onsuccess = () => {
        resolve({
          totalVectors: request.result,
          cacheSize: this.cache.size,
          cacheHitRate: this.cache.size / request.result
        });
      };
      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Save metadata
   */
  async saveMeta(key, value) {
    if (!this.db) await this.open();

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction([META_STORE], 'readwrite');
      const store = transaction.objectStore(META_STORE);
      const request = store.put({ key, value });

      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Load metadata
   */
  async loadMeta(key) {
    if (!this.db) await this.open();

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction([META_STORE], 'readonly');
      const store = transaction.objectStore(META_STORE);
      const request = store.get(key);

      request.onsuccess = () => {
        const data = request.result;
        resolve(data ? data.value : null);
      };
      request.onerror = () => reject(request.error);
    });
  }

  /**
   * Close the database connection
   */
  close() {
    if (this.db) {
      this.db.close();
      this.db = null;
    }
  }
}

export default IndexedDBPersistence;
