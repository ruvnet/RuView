/**
 * @ruvector/node - Unified Ruvector Package
 *
 * High-performance Rust vector database with GNN capabilities for Node.js.
 * This package re-exports both @ruvector/core and @ruvector/gnn for convenience.
 *
 * @example
 * ```typescript
 * import {
 *   // Core vector database
 *   VectorDB,
 *   CollectionManager,
 *   DistanceMetric,
 *   // GNN capabilities
 *   RuvectorLayer,
 *   TensorCompress,
 *   differentiableSearch
 * } from '@ruvector/node';
 *
 * // Create vector database
 * const db = new VectorDB({ dimensions: 384 });
 *
 * // Create GNN layer
 * const layer = new RuvectorLayer(384, 256, 4, 0.1);
 * ```
 */

// Re-export everything from @ruvector/core
export {
  VectorDB,
  CollectionManager,
  version,
  hello,
  getMetrics,
  getHealth,
  DistanceMetric,
  type DbOptions,
  type HnswConfig,
  type QuantizationConfig,
  type VectorEntry,
  type SearchQuery,
  type SearchResult as CoreSearchResult,
  type CollectionConfig,
  type CollectionStats,
  type Alias,
  type HealthResponse,
  type Filter
} from '@ruvector/core';

// Re-export everything from @ruvector/gnn
export {
  RuvectorLayer,
  TensorCompress,
  differentiableSearch,
  hierarchicalForward,
  getCompressionLevel,
  init as initGnn,
  type CompressionLevelConfig,
  type SearchResult as GnnSearchResult
} from '@ruvector/gnn';

// Convenience default export
import * as core from '@ruvector/core';
import * as gnn from '@ruvector/gnn';

export default {
  // Core exports
  VectorDB: core.VectorDB,
  CollectionManager: core.CollectionManager,
  version: core.version,
  hello: core.hello,
  getMetrics: core.getMetrics,
  getHealth: core.getHealth,
  DistanceMetric: core.DistanceMetric,

  // GNN exports
  RuvectorLayer: gnn.RuvectorLayer,
  TensorCompress: gnn.TensorCompress,
  differentiableSearch: gnn.differentiableSearch,
  hierarchicalForward: gnn.hierarchicalForward,
  getCompressionLevel: gnn.getCompressionLevel,
  initGnn: gnn.init
};
