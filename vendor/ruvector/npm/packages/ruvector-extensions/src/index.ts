/**
 * @fileoverview ruvector-extensions - Advanced features for ruvector
 *
 * Provides embeddings integration, UI components, export utilities,
 * temporal tracking, and persistence layers for ruvector vector database.
 *
 * @module ruvector-extensions
 * @author ruv.io Team <info@ruv.io>
 * @license MIT
 */

// Export embeddings module
export {
  // Base class
  EmbeddingProvider,

  // Provider implementations
  OpenAIEmbeddings,
  CohereEmbeddings,
  AnthropicEmbeddings,
  HuggingFaceEmbeddings,

  // Helper functions
  embedAndInsert,
  embedAndSearch,

  // Types and interfaces
  type RetryConfig,
  type EmbeddingResult,
  type BatchEmbeddingResult,
  type EmbeddingError,
  type DocumentToEmbed,
  type OpenAIEmbeddingsConfig,
  type CohereEmbeddingsConfig,
  type AnthropicEmbeddingsConfig,
  type HuggingFaceEmbeddingsConfig,
} from './embeddings.js';

// Re-export default for convenience
export { default as embeddings } from './embeddings.js';

// Export graph exporters module
export {
  // Graph builders
  buildGraphFromEntries,
  buildGraphFromVectorDB,

  // Format exporters
  exportToGraphML,
  exportToGEXF,
  exportToNeo4j,
  exportToNeo4jJSON,
  exportToD3,
  exportToD3Hierarchy,
  exportToNetworkX,
  exportToNetworkXEdgeList,
  exportToNetworkXAdjacencyList,

  // Unified export
  exportGraph,

  // Streaming exporters
  GraphMLStreamExporter,
  D3StreamExporter,
  streamToGraphML,

  // Utilities
  validateGraph,

  // Types
  type Graph,
  type GraphNode,
  type GraphEdge,
  type ExportOptions,
  type ExportFormat,
  type ExportResult
} from './exporters.js';

// Export temporal tracking module
export {
  // Main class
  TemporalTracker,

  // Singleton instance
  temporalTracker,

  // Enums
  ChangeType,

  // Type guards
  isChange,
  isVersion,

  // Types
  type Change,
  type Version,
  type VersionDiff,
  type AuditLogEntry,
  type CreateVersionOptions,
  type QueryOptions,
  type VisualizationData,
  type TemporalTrackerEvents,
} from './temporal.js';

// Export UI server module
export {
  // Main class
  UIServer,

  // Helper function
  startUIServer,

  // Types
  type GraphNode as UIGraphNode,
  type GraphLink,
  type GraphData,
} from "./ui-server.js";

