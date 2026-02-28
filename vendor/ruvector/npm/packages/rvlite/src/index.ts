/**
 * RvLite - Lightweight Vector Database SDK
 *
 * A unified database combining:
 * - Vector similarity search
 * - SQL queries with vector distance operations
 * - Cypher property graph queries
 * - SPARQL RDF triple queries
 *
 * @example
 * ```typescript
 * import { RvLite } from 'rvlite';
 *
 * const db = new RvLite({ dimensions: 384 });
 *
 * // Insert vectors
 * db.insert([0.1, 0.2, ...], { text: "Hello world" });
 *
 * // Search similar
 * const results = db.search([0.1, 0.2, ...], 5);
 *
 * // SQL with vector distance
 * db.sql("SELECT * FROM vectors WHERE distance(embedding, ?) < 0.5");
 *
 * // Cypher graph queries
 * db.cypher("CREATE (p:Person {name: 'Alice'})");
 *
 * // SPARQL RDF queries
 * db.sparql("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");
 * ```
 */

// Re-export WASM module for advanced usage
export * from '../dist/wasm/rvlite.js';

// ── RVF Backend Detection ─────────────────────────────────────────────────

let rvfWasmAvailable: boolean | null = null;

/**
 * Check if @ruvector/rvf-wasm is installed for persistent RVF storage.
 */
export function isRvfAvailable(): boolean {
  if (rvfWasmAvailable !== null) return rvfWasmAvailable;
  try {
    require.resolve('@ruvector/rvf-wasm');
    rvfWasmAvailable = true;
  } catch {
    rvfWasmAvailable = false;
  }
  return rvfWasmAvailable;
}

/**
 * Get the active storage backend.
 */
export function getStorageBackend(): 'rvf' | 'indexeddb' | 'memory' {
  if (isRvfAvailable()) return 'rvf';
  if (typeof indexedDB !== 'undefined') return 'indexeddb';
  return 'memory';
}

export interface RvLiteConfig {
  dimensions?: number;
  distanceMetric?: 'cosine' | 'euclidean' | 'dotproduct';
  /** Force a specific storage backend. Auto-detected if omitted. */
  backend?: 'rvf' | 'indexeddb' | 'memory' | 'auto';
  /** Path to RVF file for persistent storage. */
  rvfPath?: string;
}

export interface SearchResult {
  id: string;
  score: number;
  metadata?: Record<string, unknown>;
}

export interface QueryResult {
  columns?: string[];
  rows?: unknown[][];
  [key: string]: unknown;
}

/**
 * Main RvLite class - wraps the WASM module with a friendly API
 */
export class RvLite {
  private wasm: any;
  private config: RvLiteConfig;
  private initialized: boolean = false;

  constructor(config: RvLiteConfig = {}) {
    this.config = {
      dimensions: config.dimensions || 384,
      distanceMetric: config.distanceMetric || 'cosine',
    };
  }

  /**
   * Initialize the WASM module (called automatically on first use)
   */
  async init(): Promise<void> {
    if (this.initialized) return;

    // Dynamic import to support both Node.js and browser
    // Use 'as any' for WASM interop: generated types conflict with SDK types
    const wasmModule = await import('../dist/wasm/rvlite.js') as any;
    await wasmModule.default();

    this.wasm = new wasmModule.RvLite({
      dimensions: this.config.dimensions,
      distance_metric: this.config.distanceMetric,
    });

    this.initialized = true;
  }

  private async ensureInit(): Promise<void> {
    if (!this.initialized) {
      await this.init();
    }
  }

  // ============ Vector Operations ============

  /**
   * Insert a vector with optional metadata
   */
  async insert(
    vector: number[],
    metadata?: Record<string, unknown>
  ): Promise<string> {
    await this.ensureInit();
    return this.wasm.insert(vector, metadata || null);
  }

  /**
   * Insert a vector with a specific ID
   */
  async insertWithId(
    id: string,
    vector: number[],
    metadata?: Record<string, unknown>
  ): Promise<void> {
    await this.ensureInit();
    this.wasm.insert_with_id(id, vector, metadata || null);
  }

  /**
   * Search for similar vectors
   */
  async search(query: number[], k: number = 5): Promise<SearchResult[]> {
    await this.ensureInit();
    return this.wasm.search(query, k);
  }

  /**
   * Get a vector by ID
   */
  async get(id: string): Promise<{ vector: number[]; metadata?: Record<string, unknown> } | null> {
    await this.ensureInit();
    return this.wasm.get(id);
  }

  /**
   * Delete a vector by ID
   */
  async delete(id: string): Promise<boolean> {
    await this.ensureInit();
    return this.wasm.delete(id);
  }

  /**
   * Get the number of vectors
   */
  async len(): Promise<number> {
    await this.ensureInit();
    return this.wasm.len();
  }

  // ============ SQL Operations ============

  /**
   * Execute a SQL query
   *
   * Supports vector distance operations:
   * - distance(col, vector) - Calculate distance
   * - vec_search(col, vector, k) - Find k nearest
   */
  async sql(query: string): Promise<QueryResult> {
    await this.ensureInit();
    return this.wasm.sql(query);
  }

  // ============ Cypher Operations ============

  /**
   * Execute a Cypher graph query
   *
   * Supports:
   * - CREATE (n:Label {props})
   * - MATCH (n:Label) WHERE ... RETURN n
   * - CREATE (a)-[:REL]->(b)
   */
  async cypher(query: string): Promise<QueryResult> {
    await this.ensureInit();
    return this.wasm.cypher(query);
  }

  /**
   * Get Cypher graph statistics
   */
  async cypherStats(): Promise<{ node_count: number; edge_count: number }> {
    await this.ensureInit();
    return this.wasm.cypher_stats();
  }

  // ============ SPARQL Operations ============

  /**
   * Execute a SPARQL query
   *
   * Supports SELECT, ASK queries over RDF triples
   */
  async sparql(query: string): Promise<QueryResult> {
    await this.ensureInit();
    return this.wasm.sparql(query);
  }

  /**
   * Add an RDF triple
   */
  async addTriple(
    subject: string,
    predicate: string,
    object: string,
    graph?: string
  ): Promise<void> {
    await this.ensureInit();
    this.wasm.add_triple(subject, predicate, object, graph || null);
  }

  /**
   * Get the number of triples
   */
  async tripleCount(): Promise<number> {
    await this.ensureInit();
    return this.wasm.triple_count();
  }

  // ============ Persistence ============

  /**
   * Export database state to JSON
   */
  async exportJson(): Promise<unknown> {
    await this.ensureInit();
    return this.wasm.export_json();
  }

  /**
   * Import database state from JSON
   */
  async importJson(data: unknown): Promise<void> {
    await this.ensureInit();
    this.wasm.import_json(data);
  }

  /**
   * Save to IndexedDB (browser only)
   */
  async save(): Promise<void> {
    await this.ensureInit();
    return this.wasm.save();
  }

  /**
   * Load from IndexedDB (browser only)
   */
  static async load(config: RvLiteConfig = {}): Promise<RvLite> {
    const instance = new RvLite(config);
    await instance.init();

    // Dynamic import for WASM (cast to any: generated types conflict with SDK types)
    const wasmModule = await import('../dist/wasm/rvlite.js') as any;
    instance.wasm = await wasmModule.RvLite.load(config);

    return instance;
  }

  /**
   * Clear IndexedDB storage (browser only)
   */
  static async clearStorage(): Promise<void> {
    const wasmModule = await import('../dist/wasm/rvlite.js') as any;
    return wasmModule.RvLite.clear_storage();
  }

  // ============ RVF Persistence ============

  /**
   * Factory method: create an RvLite instance backed by an RVF file.
   *
   * Opens or creates an RVF file at the given path, initialises the WASM
   * module, and (when available) uses `@ruvector/rvf-wasm` for vector storage.
   * Falls back to standard WASM + JSON-based RVF if the optional package is
   * not installed.
   *
   * @param config - Standard RvLiteConfig plus a required `rvfPath`.
   * @returns A fully-initialised RvLite instance with data loaded from the
   *          RVF file (if it already exists).
   */
  static async createWithRvf(
    config: RvLiteConfig & { rvfPath: string }
  ): Promise<RvLite> {
    const instance = new RvLite(config);
    instance.rvfPath = config.rvfPath;

    // Attempt to use @ruvector/rvf-wasm for native RVF I/O
    try {
      const rvfWasm = await import('@ruvector/rvf-wasm' as string);
      instance.rvfModule = rvfWasm;
    } catch {
      // Optional dependency not available — fall back to JSON-based RVF.
    }

    await instance.init();

    // If the file exists on disk, load its content.
    if (typeof globalThis.process !== 'undefined') {
      try {
        const fs = await import('fs' as string);
        if (fs.existsSync(config.rvfPath)) {
          await instance.loadFromRvf(config.rvfPath);
        }
      } catch {
        // Browser or other environment — skip file check.
      }
    }

    return instance;
  }

  /**
   * Export the current vector state to an RVF file.
   *
   * When `@ruvector/rvf-wasm` is available the export uses the native RVF
   * binary writer.  Otherwise the method falls back to a JSON payload
   * wrapped with RVF header metadata so the file can be identified as RVF.
   *
   * @param filePath - Destination path for the RVF file.
   */
  async saveToRvf(filePath: string): Promise<void> {
    await this.ensureInit();

    const jsonState = await this.exportJson();

    // Prefer native RVF writer when available.
    if (this.rvfModule && typeof this.rvfModule.writeRvf === 'function') {
      await this.rvfModule.writeRvf(filePath, jsonState);
      return;
    }

    // Fallback: JSON with RVF envelope
    const rvfEnvelope: RvfFileEnvelope = {
      rvf_version: 1,
      magic: 'RVF1',
      created_at: new Date().toISOString(),
      dimensions: this.config.dimensions ?? 384,
      distance_metric: this.config.distanceMetric ?? 'cosine',
      payload: jsonState,
    };

    if (typeof globalThis.process !== 'undefined') {
      const fs = await import('fs' as string);
      const path = await import('path' as string);
      const dir = path.dirname(filePath);
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }
      fs.writeFileSync(filePath, JSON.stringify(rvfEnvelope, null, 2), 'utf-8');
    } else {
      throw new Error(
        'saveToRvf is only supported in Node.js environments. ' +
        'Use exportJson() for browser-side persistence.'
      );
    }
  }

  /**
   * Import vector data from an RVF file.
   *
   * Parses the RVF format (either native binary via `@ruvector/rvf-wasm` or
   * the JSON-based fallback envelope) and loads vectors + metadata into the
   * current instance.
   *
   * @param filePath - Source path of the RVF file to import.
   */
  async loadFromRvf(filePath: string): Promise<void> {
    await this.ensureInit();

    // Prefer native RVF reader.
    if (this.rvfModule && typeof this.rvfModule.readRvf === 'function') {
      const data = await this.rvfModule.readRvf(filePath);
      await this.importJson(data);
      return;
    }

    // Fallback: read JSON envelope.
    if (typeof globalThis.process !== 'undefined') {
      const fs = await import('fs' as string);
      if (!fs.existsSync(filePath)) {
        throw new Error(`RVF file not found: ${filePath}`);
      }
      const raw = fs.readFileSync(filePath, 'utf-8');
      const envelope = JSON.parse(raw) as RvfFileEnvelope;

      if (envelope.magic !== 'RVF1') {
        throw new Error(
          `Invalid RVF file: expected magic "RVF1", got "${envelope.magic}"`
        );
      }

      await this.importJson(envelope.payload);
    } else {
      throw new Error(
        'loadFromRvf is only supported in Node.js environments. ' +
        'Use importJson() for browser-side persistence.'
      );
    }
  }

  /** @internal handle to optional @ruvector/rvf-wasm module */
  private rvfModule: any = null;
  /** @internal path to the RVF backing file (set by createWithRvf) */
  private rvfPath: string | null = null;
}

// ============ Convenience Functions ============

/**
 * Create a new RvLite instance (async factory).
 *
 * When `@ruvector/rvf-wasm` is installed, persistence uses RVF format.
 * Override with `config.backend` to force a specific backend.
 */
export async function createRvLite(config: RvLiteConfig = {}): Promise<RvLite> {
  const requestedBackend = config.backend || 'auto';
  const actualBackend = requestedBackend === 'auto' ? getStorageBackend() : requestedBackend;

  // Log backend selection (useful for debugging)
  if (typeof process !== 'undefined' && process.env && process.env.RVLITE_DEBUG) {
    console.log(`[rvlite] storage backend: ${actualBackend} (requested: ${requestedBackend}, rvf available: ${isRvfAvailable()})`);
  }

  const db = new RvLite(config);
  await db.init();
  return db;
}

/**
 * Generate embeddings using various providers
 */
export interface EmbeddingProvider {
  embed(text: string): Promise<number[]>;
  embedBatch(texts: string[]): Promise<number[][]>;
}

/**
 * Create an embedding provider using Anthropic Claude
 */
export function createAnthropicEmbeddings(apiKey?: string): EmbeddingProvider {
  // Note: Claude doesn't have native embeddings, this is a placeholder
  // Users should use their own embedding provider
  throw new Error(
    'Anthropic does not provide embeddings. Use createOpenAIEmbeddings or a custom provider.'
  );
}

/**
 * Sanitize a string for safe use in Cypher queries.
 */
function sanitizeCypher(value: string): string {
  return value
    .replace(/\\/g, '\\\\')
    .replace(/"/g, '\\"')
    .replace(/'/g, "\\'")
    .replace(/[\x00-\x1f\x7f]/g, '');
}

/**
 * Validate a Cypher relationship type (alphanumeric + underscores only).
 */
function validateRelationType(rel: string): string {
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(rel)) {
    throw new Error(`Invalid relation type: ${rel}`);
  }
  return rel;
}

/**
 * Semantic Memory - Higher-level API for AI memory applications
 *
 * Combines vector search with knowledge graph storage
 */
export class SemanticMemory {
  private db: RvLite;
  private embedder?: EmbeddingProvider;

  constructor(db: RvLite, embedder?: EmbeddingProvider) {
    this.db = db;
    this.embedder = embedder;
  }

  /**
   * Store a memory with semantic embedding
   */
  async store(
    key: string,
    content: string,
    embedding?: number[],
    metadata?: Record<string, unknown>
  ): Promise<void> {
    let vector = embedding;
    if (!vector && this.embedder) {
      vector = await this.embedder.embed(content);
    }

    if (vector) {
      await this.db.insertWithId(key, vector, { content, ...metadata });
    }

    // Also store as graph node
    const safeKey = sanitizeCypher(key);
    const safeContent = sanitizeCypher(content);
    await this.db.cypher(
      `CREATE (m:Memory {key: "${safeKey}", content: "${safeContent}", timestamp: ${Date.now()}})`
    );
  }

  /**
   * Query memories by semantic similarity
   */
  async query(
    queryText: string,
    embedding?: number[],
    k: number = 5
  ): Promise<SearchResult[]> {
    let vector = embedding;
    if (!vector && this.embedder) {
      vector = await this.embedder.embed(queryText);
    }

    if (!vector) {
      throw new Error('No embedding provided and no embedder configured');
    }

    return this.db.search(vector, k);
  }

  /**
   * Add a relationship between memories
   */
  async addRelation(
    fromKey: string,
    relation: string,
    toKey: string
  ): Promise<void> {
    const safeFrom = sanitizeCypher(fromKey);
    const safeTo = sanitizeCypher(toKey);
    const safeRel = validateRelationType(relation);
    await this.db.cypher(
      `MATCH (a:Memory {key: "${safeFrom}"}), (b:Memory {key: "${safeTo}"}) CREATE (a)-[:${safeRel}]->(b)`
    );
  }

  /**
   * Find related memories through graph traversal
   */
  async findRelated(key: string, depth: number = 2): Promise<QueryResult> {
    const safeKey = sanitizeCypher(key);
    const safeDepth = Math.max(1, Math.min(10, Math.floor(depth)));
    return this.db.cypher(
      `MATCH (m:Memory {key: "${safeKey}"})-[*1..${safeDepth}]-(related:Memory) RETURN DISTINCT related`
    );
  }
}

// ── RVF File Envelope ────────────────────────────────────────────────────

/**
 * JSON-based RVF file structure used when `@ruvector/rvf-wasm` is not
 * available.  The envelope wraps the standard export_json() payload with
 * header metadata so the file is self-describing.
 */
export interface RvfFileEnvelope {
  /** RVF format version (currently 1). */
  rvf_version: number;
  /** Magic identifier — always "RVF1". */
  magic: 'RVF1';
  /** ISO-8601 timestamp of when the file was created. */
  created_at: string;
  /** Vector dimensions stored in this file. */
  dimensions: number;
  /** Distance metric used. */
  distance_metric: string;
  /** The full database state (as returned by `exportJson()`). */
  payload: unknown;
}

// ── Browser Writer Lease ─────────────────────────────────────────────────

/**
 * Browser-side writer lease that uses IndexedDB for lock coordination.
 *
 * Only one writer may hold the lease for a given `storeId` at a time.
 * The holder sends heartbeats (timestamp updates) every 10 seconds so
 * that other tabs / windows can detect stale leases.
 *
 * Auto-releases on `beforeunload` to avoid dangling locks.
 */
export class BrowserWriterLease {
  private heartbeatInterval: number | null = null;
  private storeId: string | null = null;
  private static readonly DB_NAME = '_rvlite_locks';
  private static readonly STORE_NAME = 'locks';
  private static readonly HEARTBEAT_MS = 10_000;
  private static readonly DEFAULT_STALE_MS = 30_000;

  // ---- helpers ----

  private static openDb(): Promise<IDBDatabase> {
    return new Promise((resolve, reject) => {
      const req = indexedDB.open(BrowserWriterLease.DB_NAME, 1);
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains(BrowserWriterLease.STORE_NAME)) {
          db.createObjectStore(BrowserWriterLease.STORE_NAME, { keyPath: 'id' });
        }
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }

  private static idbPut(db: IDBDatabase, record: unknown): Promise<void> {
    return new Promise((resolve, reject) => {
      const tx = db.transaction(BrowserWriterLease.STORE_NAME, 'readwrite');
      const store = tx.objectStore(BrowserWriterLease.STORE_NAME);
      const req = store.put(record);
      req.onsuccess = () => resolve();
      req.onerror = () => reject(req.error);
    });
  }

  private static idbGet(db: IDBDatabase, key: string): Promise<any> {
    return new Promise((resolve, reject) => {
      const tx = db.transaction(BrowserWriterLease.STORE_NAME, 'readonly');
      const store = tx.objectStore(BrowserWriterLease.STORE_NAME);
      const req = store.get(key);
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }

  private static idbDelete(db: IDBDatabase, key: string): Promise<void> {
    return new Promise((resolve, reject) => {
      const tx = db.transaction(BrowserWriterLease.STORE_NAME, 'readwrite');
      const store = tx.objectStore(BrowserWriterLease.STORE_NAME);
      const req = store.delete(key);
      req.onsuccess = () => resolve();
      req.onerror = () => reject(req.error);
    });
  }

  // ---- public API ----

  /**
   * Try to acquire the writer lease for the given store.
   *
   * @param storeId  - Unique identifier for the rvlite store being locked.
   * @param timeout  - Maximum time in ms to wait for the lease (default 5000).
   * @returns `true` if the lease was acquired, `false` on timeout.
   */
  async acquire(storeId: string, timeout: number = 5000): Promise<boolean> {
    if (typeof indexedDB === 'undefined') {
      throw new Error('BrowserWriterLease requires IndexedDB');
    }

    const deadline = Date.now() + timeout;
    const db = await BrowserWriterLease.openDb();

    while (Date.now() < deadline) {
      const existing = await BrowserWriterLease.idbGet(db, storeId);

      if (!existing || await BrowserWriterLease.isStale(storeId)) {
        // Write our lock record.
        await BrowserWriterLease.idbPut(db, {
          id: storeId,
          holder: this.holderId(),
          ts: Date.now(),
        });

        // Re-read to confirm we won (poor-man's CAS).
        const confirm = await BrowserWriterLease.idbGet(db, storeId);
        if (confirm && confirm.holder === this.holderId()) {
          this.storeId = storeId;
          this.startHeartbeat(db);
          this.registerUnloadHandler();
          db.close();
          return true;
        }
      }

      // Back off before retrying.
      await new Promise(r => setTimeout(r, 200));
    }

    db.close();
    return false;
  }

  /**
   * Release the currently held lease.
   */
  async release(): Promise<void> {
    this.stopHeartbeat();

    if (this.storeId === null) return;

    try {
      const db = await BrowserWriterLease.openDb();
      await BrowserWriterLease.idbDelete(db, this.storeId);
      db.close();
    } catch {
      // Best-effort release.
    }

    this.storeId = null;
  }

  /**
   * Check whether the lease for `storeId` is stale (the holder has stopped
   * sending heartbeats).
   *
   * @param storeId     - Store identifier.
   * @param thresholdMs - Staleness threshold (default 30 000 ms).
   */
  static async isStale(
    storeId: string,
    thresholdMs: number = BrowserWriterLease.DEFAULT_STALE_MS
  ): Promise<boolean> {
    if (typeof indexedDB === 'undefined') return true;

    const db = await BrowserWriterLease.openDb();
    const record = await BrowserWriterLease.idbGet(db, storeId);
    db.close();

    if (!record) return true;
    return Date.now() - record.ts > thresholdMs;
  }

  // ---- private helpers ----

  private _holderId: string | null = null;

  private holderId(): string {
    if (!this._holderId) {
      this._holderId = `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
    }
    return this._holderId;
  }

  private startHeartbeat(db: IDBDatabase): void {
    this.stopHeartbeat();
    const storeId = this.storeId!;
    const holder = this.holderId();

    const beat = async () => {
      try {
        const freshDb = await BrowserWriterLease.openDb();
        await BrowserWriterLease.idbPut(freshDb, {
          id: storeId,
          holder,
          ts: Date.now(),
        });
        freshDb.close();
      } catch {
        // Heartbeat failures are non-fatal.
      }
    };

    this.heartbeatInterval = setInterval(
      beat,
      BrowserWriterLease.HEARTBEAT_MS
    ) as unknown as number;
  }

  private stopHeartbeat(): void {
    if (this.heartbeatInterval !== null) {
      clearInterval(this.heartbeatInterval);
      this.heartbeatInterval = null;
    }
  }

  private registerUnloadHandler(): void {
    if (typeof globalThis.addEventListener === 'function') {
      const handler = () => {
        this.stopHeartbeat();
        // Synchronous best-effort release — IndexedDB is unavailable during
        // unload in some browsers so we just stop the heartbeat, letting the
        // lease expire via staleness detection.
      };
      globalThis.addEventListener('beforeunload', handler, { once: true });
    }
  }
}

// ── Epoch Sync ───────────────────────────────────────────────────────────

/**
 * Describes the synchronisation state between the RVF vector store epoch
 * and the metadata (SQL / Cypher / SPARQL) epoch.
 */
export interface EpochState {
  /** Monotonic epoch counter for the RVF vector store. */
  rvfEpoch: number;
  /** Monotonic epoch counter for metadata stores. */
  metadataEpoch: number;
  /** Human-readable sync status. */
  status: 'synchronized' | 'rvf_ahead' | 'metadata_ahead';
}

/**
 * Inspect the current epoch state of an RvLite instance.
 *
 * The epochs are stored as metadata keys inside the database itself
 * (`_rvlite_rvf_epoch` and `_rvlite_metadata_epoch`).
 *
 * @param db - An initialised RvLite instance.
 * @returns The current epoch state.
 */
export async function checkEpochSync(db: RvLite): Promise<EpochState> {
  const rvfEntry = await db.get('_rvlite_rvf_epoch');
  const metaEntry = await db.get('_rvlite_metadata_epoch');

  const rvfEpoch = rvfEntry?.metadata?.epoch as number ?? 0;
  const metadataEpoch = metaEntry?.metadata?.epoch as number ?? 0;

  let status: EpochState['status'];
  if (rvfEpoch === metadataEpoch) {
    status = 'synchronized';
  } else if (rvfEpoch > metadataEpoch) {
    status = 'rvf_ahead';
  } else {
    status = 'metadata_ahead';
  }

  return { rvfEpoch, metadataEpoch, status };
}

/**
 * Reconcile mismatched epochs by advancing the lagging store to match
 * the leading one.
 *
 * - **rvf_ahead**: bumps the metadata epoch to match the RVF epoch.
 * - **metadata_ahead**: bumps the RVF epoch to match the metadata epoch.
 * - **synchronized**: no-op.
 *
 * @param db    - An initialised RvLite instance.
 * @param state - The epoch state (as returned by `checkEpochSync`).
 */
export async function reconcileEpochs(
  db: RvLite,
  state: EpochState
): Promise<void> {
  if (state.status === 'synchronized') return;

  const targetEpoch = Math.max(state.rvfEpoch, state.metadataEpoch);
  const dummyVector = [0]; // minimal placeholder vector

  // Upsert both epoch sentinel records to the target epoch.
  // We use insertWithId so the key is deterministic.
  try { await db.delete('_rvlite_rvf_epoch'); } catch { /* may not exist */ }
  try { await db.delete('_rvlite_metadata_epoch'); } catch { /* may not exist */ }

  await db.insertWithId('_rvlite_rvf_epoch', dummyVector, { epoch: targetEpoch });
  await db.insertWithId('_rvlite_metadata_epoch', dummyVector, { epoch: targetEpoch });
}

/**
 * Convenience helper: increment the RVF epoch by 1.
 * Call this after every successful vector-store mutation.
 */
export async function bumpRvfEpoch(db: RvLite): Promise<number> {
  const current = await checkEpochSync(db);
  const next = current.rvfEpoch + 1;
  const dummyVector = [0];
  try { await db.delete('_rvlite_rvf_epoch'); } catch { /* ignore */ }
  await db.insertWithId('_rvlite_rvf_epoch', dummyVector, { epoch: next });
  return next;
}

/**
 * Convenience helper: increment the metadata epoch by 1.
 * Call this after every successful metadata mutation (SQL / Cypher / SPARQL).
 */
export async function bumpMetadataEpoch(db: RvLite): Promise<number> {
  const current = await checkEpochSync(db);
  const next = current.metadataEpoch + 1;
  const dummyVector = [0];
  try { await db.delete('_rvlite_metadata_epoch'); } catch { /* ignore */ }
  await db.insertWithId('_rvlite_metadata_epoch', dummyVector, { epoch: next });
  return next;
}

export default RvLite;
