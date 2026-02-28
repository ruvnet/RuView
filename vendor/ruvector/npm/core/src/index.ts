/**
 * @ruvector/core - High-performance Rust vector database for Node.js
 *
 * Automatically detects platform and loads the appropriate native binding.
 */

import { platform, arch } from 'node:os';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

// Platform detection types
type Platform = 'linux' | 'darwin' | 'win32';
type Architecture = 'x64' | 'arm64';

/**
 * Distance metric for similarity calculation
 */
export enum DistanceMetric {
  /** Euclidean (L2) distance */
  Euclidean = 'Euclidean',
  /** Cosine similarity (converted to distance) */
  Cosine = 'Cosine',
  /** Dot product (converted to distance for maximization) */
  DotProduct = 'DotProduct',
  /** Manhattan (L1) distance */
  Manhattan = 'Manhattan'
}

/**
 * Quantization configuration
 */
export interface QuantizationConfig {
  /** Quantization type */
  type: 'none' | 'scalar' | 'product' | 'binary';
  /** Number of subspaces (for product quantization) */
  subspaces?: number;
  /** Codebook size (for product quantization) */
  k?: number;
}

/**
 * HNSW index configuration
 */
export interface HnswConfig {
  /** Number of connections per layer (M) */
  m?: number;
  /** Size of dynamic candidate list during construction */
  efConstruction?: number;
  /** Size of dynamic candidate list during search */
  efSearch?: number;
  /** Maximum number of elements */
  maxElements?: number;
}

/**
 * Database configuration options
 */
export interface DbOptions {
  /** Vector dimensions */
  dimensions: number;
  /** Distance metric */
  distanceMetric?: DistanceMetric;
  /** Storage path */
  storagePath?: string;
  /** HNSW configuration */
  hnswConfig?: HnswConfig;
  /** Quantization configuration */
  quantization?: QuantizationConfig;
}

/**
 * Vector entry
 */
export interface VectorEntry {
  /** Optional ID (auto-generated if not provided) */
  id?: string;
  /** Vector data as Float32Array or array of numbers */
  vector: Float32Array | number[];
}

/**
 * Search query parameters
 */
export interface SearchQuery {
  /** Query vector as Float32Array or array of numbers */
  vector: Float32Array | number[];
  /** Number of results to return (top-k) */
  k: number;
  /** Optional ef_search parameter for HNSW */
  efSearch?: number;
}

/**
 * Search result with similarity score
 */
export interface SearchResult {
  /** Vector ID */
  id: string;
  /** Distance/similarity score (lower is better for distance metrics) */
  score: number;
}

/**
 * High-performance vector database with HNSW indexing
 */
export interface VectorDB {
  /**
   * Insert a vector entry into the database
   * @param entry Vector entry to insert
   * @returns Promise resolving to the ID of the inserted vector
   */
  insert(entry: VectorEntry): Promise<string>;

  /**
   * Insert multiple vectors in a batch
   * @param entries Array of vector entries to insert
   * @returns Promise resolving to an array of IDs for the inserted vectors
   */
  insertBatch(entries: VectorEntry[]): Promise<string[]>;

  /**
   * Search for similar vectors
   * @param query Search query parameters
   * @returns Promise resolving to an array of search results sorted by similarity
   */
  search(query: SearchQuery): Promise<SearchResult[]>;

  /**
   * Delete a vector by ID
   * @param id Vector ID to delete
   * @returns Promise resolving to true if deleted, false if not found
   */
  delete(id: string): Promise<boolean>;

  /**
   * Get a vector by ID
   * @param id Vector ID to retrieve
   * @returns Promise resolving to the vector entry if found, null otherwise
   */
  get(id: string): Promise<VectorEntry | null>;

  /**
   * Get the number of vectors in the database
   * @returns Promise resolving to the number of vectors
   */
  len(): Promise<number>;

  /**
   * Check if the database is empty
   * @returns Promise resolving to true if empty, false otherwise
   */
  isEmpty(): Promise<boolean>;
}

/**
 * VectorDB constructor interface
 */
export interface VectorDBConstructor {
  new(options: DbOptions): VectorDB;
  withDimensions(dimensions: number): VectorDB;
}

/**
 * Filter for metadata-based search
 */
export interface Filter {
  /** Field name to filter on */
  field: string;
  /** Operator: "eq", "ne", "gt", "gte", "lt", "lte", "in", "match" */
  operator: 'eq' | 'ne' | 'gt' | 'gte' | 'lt' | 'lte' | 'in' | 'match';
  /** Value to compare against (JSON string) */
  value: string;
}

/**
 * Collection configuration
 */
export interface CollectionConfig {
  /** Vector dimensions */
  dimensions: number;
  /** Distance metric */
  distanceMetric?: DistanceMetric;
  /** HNSW configuration */
  hnswConfig?: HnswConfig;
  /** Quantization configuration */
  quantization?: QuantizationConfig;
}

/**
 * Collection statistics
 */
export interface CollectionStats {
  /** Number of vectors in the collection */
  vectorsCount: number;
  /** Disk space used in bytes */
  diskSizeBytes: number;
  /** RAM space used in bytes */
  ramSizeBytes: number;
}

/**
 * Collection alias
 */
export interface Alias {
  /** Alias name */
  alias: string;
  /** Collection name */
  collection: string;
}

/**
 * Health response
 */
export interface HealthResponse {
  /** Status: "healthy", "degraded", or "unhealthy" */
  status: 'healthy' | 'degraded' | 'unhealthy';
  /** Version string */
  version: string;
  /** Uptime in seconds */
  uptimeSeconds: number;
}

/**
 * Collection manager for multi-collection support
 */
export interface CollectionManager {
  /**
   * Create a new collection
   * @param name Collection name
   * @param config Collection configuration
   */
  createCollection(name: string, config: CollectionConfig): Promise<void>;

  /**
   * List all collections
   * @returns Array of collection names
   */
  listCollections(): Promise<string[]>;

  /**
   * Delete a collection
   * @param name Collection name to delete
   */
  deleteCollection(name: string): Promise<void>;

  /**
   * Get collection statistics
   * @param name Collection name
   * @returns Collection statistics
   */
  getStats(name: string): Promise<CollectionStats>;

  /**
   * Create an alias for a collection
   * @param alias Alias name
   * @param collection Collection name
   */
  createAlias(alias: string, collection: string): Promise<void>;

  /**
   * Delete an alias
   * @param alias Alias name to delete
   */
  deleteAlias(alias: string): Promise<void>;

  /**
   * List all aliases
   * @returns Array of alias mappings
   */
  listAliases(): Promise<Alias[]>;
}

/**
 * CollectionManager constructor interface
 */
export interface CollectionManagerConstructor {
  new(basePath?: string): CollectionManager;
}

/**
 * Native binding interface
 */
export interface NativeBinding {
  VectorDB: VectorDBConstructor;
  CollectionManager: CollectionManagerConstructor;
  version(): string;
  hello(): string;
  getMetrics(): string;
  getHealth(): HealthResponse;
}

/**
 * Detect the current platform and architecture
 */
function detectPlatform(): { platform: Platform; arch: Architecture; packageName: string } {
  const currentPlatform = platform() as Platform;
  const currentArch = arch() as Architecture;

  // Map platform and architecture to package names
  const platformMap: Record<string, string> = {
    'linux-x64': '@ruvector/core-linux-x64-gnu',
    'linux-arm64': '@ruvector/core-linux-arm64-gnu',
    'darwin-x64': '@ruvector/core-darwin-x64',
    'darwin-arm64': '@ruvector/core-darwin-arm64',
    'win32-x64': '@ruvector/core-win32-x64-msvc'
  };

  const key = `${currentPlatform}-${currentArch}`;
  const packageName = platformMap[key];

  if (!packageName) {
    throw new Error(
      `Unsupported platform: ${currentPlatform}-${currentArch}. ` +
      `Supported platforms: ${Object.keys(platformMap).join(', ')}`
    );
  }

  return { platform: currentPlatform, arch: currentArch, packageName };
}

/**
 * Load the native binding for the current platform
 */
function loadNativeBinding(): NativeBinding {
  const currentPlatform = platform();
  const currentArch = arch();
  const platformKey = `${currentPlatform}-${currentArch}`;

  try {
    // Try to load from native directory first (for direct builds)
    // Use the wrapper index.cjs if it exists, otherwise load the .node file directly
    try {
      const nativeBinding = require(`../native/${platformKey}/index.cjs`) as NativeBinding;
      return nativeBinding;
    } catch {
      const nativeBinding = require(`../native/${platformKey}/ruvector.node`) as NativeBinding;
      return nativeBinding;
    }
  } catch (error) {
    // Fallback to platform-specific packages
    const { packageName } = detectPlatform();

    try {
      const nativeBinding = require(packageName) as NativeBinding;
      return nativeBinding;
    } catch (packageError) {
      // Provide helpful error message
      const err = packageError as NodeJS.ErrnoException;
      if (err.code === 'MODULE_NOT_FOUND') {
        throw new Error(
          `Failed to load native binding for ${platformKey}. ` +
          `Tried: ../native/${platformKey}/ruvector.node and ${packageName}. ` +
          `Please ensure the package is installed by running: npm install ${packageName}`
        );
      }
      throw new Error(`Failed to load native binding: ${err.message}`);
    }
  }
}

// Load the native binding
const nativeBinding = loadNativeBinding();

// Re-export the VectorDB class and utility functions
// Note: NAPI-RS exports as VectorDb (lowercase d), we re-export as VectorDB for consistency
export const VectorDB = (nativeBinding as any).VectorDb || nativeBinding.VectorDB;
export const CollectionManager = nativeBinding.CollectionManager;
export const version = nativeBinding.version;
export const hello = nativeBinding.hello;
export const getMetrics = nativeBinding.getMetrics;
export const getHealth = nativeBinding.getHealth;

// Try to load optional attention module
let attention: any = null;
try {
  attention = require('@ruvector/attention');
} catch {
  // Attention module not installed - this is optional
}

// Export attention if available
export { attention };

// Default export
export default {
  VectorDB,
  CollectionManager,
  version,
  hello,
  getMetrics,
  getHealth,
  DistanceMetric,
  // Include attention if available
  ...(attention ? { attention } : {})
};
