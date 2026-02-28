/**
 * Browser-specific exports for @ruvector/wasm
 */

import type { VectorEntry, SearchResult, DbOptions } from './index';

let wasmModule: any = null;

/**
 * Initialize WASM module for browser
 */
async function initWasm() {
  if (!wasmModule) {
    wasmModule = await import('../pkg/ruvector_wasm.js');
    await wasmModule.default();
  }
  return wasmModule;
}

/**
 * VectorDB class for browser
 */
export class VectorDB {
  private db: any;
  private dimensions: number;

  constructor(options: DbOptions) {
    this.dimensions = options.dimensions;
  }

  async init(): Promise<void> {
    const module = await initWasm();
    this.db = new module.VectorDB(
      this.dimensions,
      'cosine',
      true
    );
  }

  insert(vector: Float32Array | number[], id?: string, metadata?: Record<string, any>): string {
    if (!this.db) throw new Error('Database not initialized. Call init() first.');
    const vectorArray = vector instanceof Float32Array ? vector : new Float32Array(vector);
    return this.db.insert(vectorArray, id, metadata);
  }

  insertBatch(entries: VectorEntry[]): string[] {
    if (!this.db) throw new Error('Database not initialized. Call init() first.');
    const processedEntries = entries.map(entry => ({
      id: entry.id,
      vector: entry.vector instanceof Float32Array ? entry.vector : new Float32Array(entry.vector),
      metadata: entry.metadata
    }));
    return this.db.insertBatch(processedEntries);
  }

  search(query: Float32Array | number[], k: number, filter?: Record<string, any>): SearchResult[] {
    if (!this.db) throw new Error('Database not initialized. Call init() first.');
    const queryArray = query instanceof Float32Array ? query : new Float32Array(query);
    const results = this.db.search(queryArray, k, filter);
    return results.map((r: any) => ({
      id: r.id,
      score: r.score,
      vector: r.vector,
      metadata: r.metadata
    }));
  }

  delete(id: string): boolean {
    if (!this.db) throw new Error('Database not initialized. Call init() first.');
    return this.db.delete(id);
  }

  get(id: string): VectorEntry | null {
    if (!this.db) throw new Error('Database not initialized. Call init() first.');
    const entry = this.db.get(id);
    if (!entry) return null;
    return { id: entry.id, vector: entry.vector, metadata: entry.metadata };
  }

  len(): number {
    if (!this.db) throw new Error('Database not initialized. Call init() first.');
    return this.db.len();
  }

  isEmpty(): boolean {
    if (!this.db) throw new Error('Database not initialized. Call init() first.');
    return this.db.isEmpty();
  }

  getDimensions(): number {
    return this.dimensions;
  }

  async saveToIndexedDB(): Promise<void> {
    if (!this.db) throw new Error('Database not initialized. Call init() first.');
    await this.db.saveToIndexedDB();
  }

  static async loadFromIndexedDB(dbName: string, options: DbOptions): Promise<VectorDB> {
    const db = new VectorDB(options);
    await db.init();
    await db.db.loadFromIndexedDB(dbName);
    return db;
  }
}

export async function detectSIMD(): Promise<boolean> {
  const module = await initWasm();
  return module.detectSIMD();
}

export async function version(): Promise<string> {
  const module = await initWasm();
  return module.version();
}

export async function benchmark(name: string, iterations: number, dimensions: number): Promise<number> {
  const module = await initWasm();
  return module.benchmark(name, iterations, dimensions);
}

export type { VectorEntry, SearchResult, DbOptions };
export default VectorDB;
