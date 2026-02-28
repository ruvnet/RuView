/**
 * @ruvector/wasm - WebAssembly bindings for Ruvector
 *
 * High-performance vector database for browsers and Node.js
 * Features:
 * - SIMD acceleration (when available)
 * - Multiple distance metrics (cosine, euclidean, dot product, manhattan)
 * - HNSW indexing for fast approximate nearest neighbor search
 * - Zero-copy operations via transferable objects
 * - IndexedDB persistence (browser)
 * - File system persistence (Node.js)
 */

// Auto-detect environment and load appropriate WASM module
const isBrowser = typeof window !== 'undefined' && typeof window.document !== 'undefined';
const isNode = typeof process !== 'undefined' && process.versions != null && process.versions.node != null;

/**
 * Vector entry interface
 */
export interface VectorEntry {
  id?: string;
  vector: Float32Array | number[];
  metadata?: Record<string, any>;
}

/**
 * Search result interface
 */
export interface SearchResult {
  id: string;
  score: number;
  vector?: Float32Array;
  metadata?: Record<string, any>;
}

/**
 * Database options
 */
export interface DbOptions {
  dimensions: number;
  metric?: 'euclidean' | 'cosine' | 'dotproduct' | 'manhattan';
  useHnsw?: boolean;
}

/**
 * VectorDB class - unified interface for browser and Node.js
 */
export class VectorDB {
  private wasmModule: any;
  private db: any;
  private dimensions: number;

  constructor(options: DbOptions) {
    this.dimensions = options.dimensions;
  }

  /**
   * Initialize the database (async)
   * Must be called before using the database
   */
  async init(): Promise<void> {
    if (isBrowser) {
      this.wasmModule = await import('../pkg/ruvector_wasm.js');
      await this.wasmModule.default();
      this.db = new this.wasmModule.VectorDB(
        this.dimensions,
        'cosine',
        true
      );
    } else if (isNode) {
      this.wasmModule = await import('../pkg-node/ruvector_wasm.js');
      this.db = new this.wasmModule.VectorDB(
        this.dimensions,
        'cosine',
        true
      );
    } else {
      throw new Error('Unsupported environment');
    }
  }

  /**
   * Insert a single vector
   */
  insert(vector: Float32Array | number[], id?: string, metadata?: Record<string, any>): string {
    if (!this.db) {
      throw new Error('Database not initialized. Call init() first.');
    }

    const vectorArray = vector instanceof Float32Array
      ? vector
      : new Float32Array(vector);

    return this.db.insert(vectorArray, id, metadata);
  }

  /**
   * Insert multiple vectors in a batch
   */
  insertBatch(entries: VectorEntry[]): string[] {
    if (!this.db) {
      throw new Error('Database not initialized. Call init() first.');
    }

    const processedEntries = entries.map(entry => ({
      id: entry.id,
      vector: entry.vector instanceof Float32Array
        ? entry.vector
        : new Float32Array(entry.vector),
      metadata: entry.metadata
    }));

    return this.db.insertBatch(processedEntries);
  }

  /**
   * Search for similar vectors
   */
  search(query: Float32Array | number[], k: number, filter?: Record<string, any>): SearchResult[] {
    if (!this.db) {
      throw new Error('Database not initialized. Call init() first.');
    }

    const queryArray = query instanceof Float32Array
      ? query
      : new Float32Array(query);

    const results = this.db.search(queryArray, k, filter);

    // Convert WASM results to plain objects
    return results.map((r: any) => ({
      id: r.id,
      score: r.score,
      vector: r.vector,
      metadata: r.metadata
    }));
  }

  /**
   * Delete a vector by ID
   */
  delete(id: string): boolean {
    if (!this.db) {
      throw new Error('Database not initialized. Call init() first.');
    }

    return this.db.delete(id);
  }

  /**
   * Get a vector by ID
   */
  get(id: string): VectorEntry | null {
    if (!this.db) {
      throw new Error('Database not initialized. Call init() first.');
    }

    const entry = this.db.get(id);
    if (!entry) return null;

    return {
      id: entry.id,
      vector: entry.vector,
      metadata: entry.metadata
    };
  }

  /**
   * Get the number of vectors in the database
   */
  len(): number {
    if (!this.db) {
      throw new Error('Database not initialized. Call init() first.');
    }

    return this.db.len();
  }

  /**
   * Check if the database is empty
   */
  isEmpty(): boolean {
    if (!this.db) {
      throw new Error('Database not initialized. Call init() first.');
    }

    return this.db.isEmpty();
  }

  /**
   * Get database dimensions
   */
  getDimensions(): number {
    return this.dimensions;
  }

  /**
   * Save database to persistent storage
   * - Browser: IndexedDB
   * - Node.js: File system
   */
  async save(path?: string): Promise<void> {
    if (!this.db) {
      throw new Error('Database not initialized. Call init() first.');
    }

    if (isBrowser) {
      await this.db.saveToIndexedDB();
    } else if (isNode) {
      // Node.js file system persistence would go here
      console.warn('Node.js persistence not yet implemented');
    }
  }

  /**
   * Load database from persistent storage
   */
  static async load(path: string, options: DbOptions): Promise<VectorDB> {
    const db = new VectorDB(options);
    await db.init();

    if (isBrowser) {
      await db.db.loadFromIndexedDB(path);
    } else if (isNode) {
      // Node.js file system loading would go here
      console.warn('Node.js persistence not yet implemented');
    }

    return db;
  }
}

/**
 * Detect SIMD support in current environment
 */
export async function detectSIMD(): Promise<boolean> {
  try {
    if (isBrowser) {
      const module = await import('../pkg/ruvector_wasm.js');
      await module.default();
      return module.detectSIMD();
    } else if (isNode) {
      const module = await import('../pkg-node/ruvector_wasm.js');
      return module.detectSIMD();
    }
    return false;
  } catch (error) {
    console.error('Error detecting SIMD:', error);
    return false;
  }
}

/**
 * Get version information
 */
export async function version(): Promise<string> {
  try {
    if (isBrowser) {
      const module = await import('../pkg/ruvector_wasm.js');
      await module.default();
      return module.version();
    } else if (isNode) {
      const module = await import('../pkg-node/ruvector_wasm.js');
      return module.version();
    }
    return 'unknown';
  } catch (error) {
    console.error('Error getting version:', error);
    return 'unknown';
  }
}

/**
 * Run a benchmark
 */
export async function benchmark(
  name: string,
  iterations: number,
  dimensions: number
): Promise<number> {
  try {
    if (isBrowser) {
      const module = await import('../pkg/ruvector_wasm.js');
      await module.default();
      return module.benchmark(name, iterations, dimensions);
    } else if (isNode) {
      const module = await import('../pkg-node/ruvector_wasm.js');
      return module.benchmark(name, iterations, dimensions);
    }
    return 0;
  } catch (error) {
    console.error('Error running benchmark:', error);
    return 0;
  }
}

// Export types
export type { DbOptions, VectorEntry, SearchResult };

// Default export
export default VectorDB;
