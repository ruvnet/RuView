/**
 * Distance metric for vector similarity
 */
export enum DistanceMetric {
  /** Euclidean (L2) distance */
  Euclidean = 0,
  /** Cosine similarity */
  Cosine = 1,
  /** Dot product similarity */
  DotProduct = 2,
  /** Manhattan (L1) distance */
  Manhattan = 3
}

/**
 * Options for creating a VectorDb instance
 */
export interface DbOptions {
  /** Vector dimension size (required) */
  dimensions: number;
  /** Maximum number of elements (optional) */
  maxElements?: number;
  /** Distance metric for similarity (optional, default: Cosine) */
  distanceMetric?: DistanceMetric;
  /** HNSW M parameter (optional, default: 16) */
  hnswM?: number;
  /** HNSW ef_construction parameter (optional, default: 200) */
  hnswEfConstruction?: number;
  /** HNSW ef_search parameter (optional, default: 100) */
  hnswEfSearch?: number;
  /** Storage path for persistence (optional) */
  storagePath?: string;
}

/**
 * Search result from a vector query
 */
export interface SearchResult {
  /** Vector ID */
  id: string;
  /** Similarity score */
  score: number;
}

/**
 * High-performance vector database for semantic search
 *
 * @example
 * ```typescript
 * import { VectorDb, DistanceMetric } from '@ruvector/router';
 *
 * // Create a vector database
 * const db = new VectorDb({
 *   dimensions: 384,
 *   distanceMetric: DistanceMetric.Cosine
 * });
 *
 * // Insert vectors
 * const embedding = new Float32Array(384).fill(0.5);
 * db.insert('doc-1', embedding);
 *
 * // Search for similar vectors
 * const results = db.search(embedding, 5);
 * console.log(results[0].id);    // 'doc-1'
 * console.log(results[0].score); // ~1.0
 * ```
 */
export class VectorDb {
  /**
   * Create a new vector database
   * @param options Database options
   */
  constructor(options: DbOptions);

  /**
   * Insert a vector into the database
   * @param id Unique identifier
   * @param vector Vector data (Float32Array)
   * @returns The inserted ID
   */
  insert(id: string, vector: Float32Array): string;

  /**
   * Insert a vector asynchronously
   * @param id Unique identifier
   * @param vector Vector data (Float32Array)
   * @returns Promise resolving to the inserted ID
   */
  insertAsync(id: string, vector: Float32Array): Promise<string>;

  /**
   * Search for similar vectors
   * @param queryVector Query embedding
   * @param k Number of results to return
   * @returns Array of search results
   */
  search(queryVector: Float32Array, k: number): SearchResult[];

  /**
   * Search for similar vectors asynchronously
   * @param queryVector Query embedding
   * @param k Number of results to return
   * @returns Promise resolving to search results
   */
  searchAsync(queryVector: Float32Array, k: number): Promise<SearchResult[]>;

  /**
   * Delete a vector by ID
   * @param id Vector ID to delete
   * @returns true if deleted, false if not found
   */
  delete(id: string): boolean;

  /**
   * Get the total count of vectors
   * @returns Number of vectors in the database
   */
  count(): number;

  /**
   * Get all vector IDs
   * @returns Array of all IDs
   */
  getAllIds(): string[];
}

/**
 * Configuration for SemanticRouter
 */
export interface RouterConfig {
  /** Embedding dimension size (required) */
  dimension: number;
  /** Distance metric: 'cosine', 'euclidean', 'dot', 'manhattan' (default: 'cosine') */
  metric?: 'cosine' | 'euclidean' | 'dot' | 'manhattan';
  /** HNSW M parameter (default: 16) */
  m?: number;
  /** HNSW ef_construction (default: 200) */
  efConstruction?: number;
  /** HNSW ef_search (default: 100) */
  efSearch?: number;
  /** Enable quantization (default: false) */
  quantization?: boolean;
  /** Minimum similarity threshold for matches (default: 0.7) */
  threshold?: number;
}

/**
 * Intent definition for the router
 */
export interface Intent {
  /** Unique intent identifier */
  name: string;
  /** Example utterances for this intent */
  utterances: string[];
  /** Pre-computed embedding (centroid) */
  embedding?: Float32Array | number[];
  /** Custom metadata */
  metadata?: Record<string, unknown>;
}

/**
 * Result from routing a query
 */
export interface RouteResult {
  /** Matched intent name */
  intent: string;
  /** Similarity score (0-1) */
  score: number;
  /** Intent metadata */
  metadata?: Record<string, unknown>;
}

/**
 * Embedder function type
 */
export type EmbedderFunction = (text: string) => Promise<Float32Array>;

/**
 * Semantic router for AI agents - vector-based intent matching
 *
 * @example
 * ```typescript
 * import { SemanticRouter } from '@ruvector/router';
 *
 * // Create router
 * const router = new SemanticRouter({ dimension: 384 });
 *
 * // Add intents with pre-computed embeddings
 * router.addIntent({
 *   name: 'weather',
 *   utterances: ['What is the weather?', 'Will it rain?'],
 *   embedding: weatherEmbedding,
 *   metadata: { handler: 'weather_agent' }
 * });
 *
 * // Route with embedding
 * const results = router.routeWithEmbedding(queryEmbedding, 3);
 * console.log(results[0].intent); // 'weather'
 * ```
 */
export class SemanticRouter {
  /**
   * Create a new SemanticRouter
   * @param config Router configuration
   */
  constructor(config: RouterConfig);

  /**
   * Set the embedder function for converting text to vectors
   * @param embedder Async function (text: string) => Float32Array
   */
  setEmbedder(embedder: EmbedderFunction): void;

  /**
   * Add an intent to the router (synchronous, requires pre-computed embedding)
   * @param intent Intent configuration
   */
  addIntent(intent: Intent): void;

  /**
   * Add an intent with automatic embedding computation
   * @param intent Intent configuration
   */
  addIntentAsync(intent: Intent): Promise<void>;

  /**
   * Route a query to matching intents
   * @param query Query text or embedding
   * @param k Number of results to return (default: 1)
   * @returns Promise resolving to route results
   */
  route(query: string | Float32Array, k?: number): Promise<RouteResult[]>;

  /**
   * Route with a pre-computed embedding (synchronous)
   * @param embedding Query embedding
   * @param k Number of results to return (default: 1)
   * @returns Route results
   */
  routeWithEmbedding(embedding: Float32Array | number[], k?: number): RouteResult[];

  /**
   * Remove an intent from the router
   * @param name Intent name to remove
   * @returns true if removed, false if not found
   */
  removeIntent(name: string): boolean;

  /**
   * Get all registered intent names
   * @returns Array of intent names
   */
  getIntents(): string[];

  /**
   * Get intent details
   * @param name Intent name
   * @returns Intent info or null if not found
   */
  getIntent(name: string): { name: string; utterances: string[]; metadata: Record<string, unknown> } | null;

  /**
   * Clear all intents
   */
  clear(): void;

  /**
   * Get the number of intents
   * @returns Number of registered intents
   */
  count(): number;

  /**
   * Save router state to disk
   * @param filePath Path to save to
   */
  save(filePath: string): Promise<void>;

  /**
   * Load router state from disk
   * @param filePath Path to load from
   */
  load(filePath: string): Promise<void>;
}
