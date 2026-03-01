/**
 * Search module exports
 *
 * Provides hybrid search combining vector similarity and BM25 keyword search.
 */

export { BM25Index, createBM25Index } from './BM25Index.js';
export type { BM25Config, Document, BM25Result } from './BM25Index.js';

export {
  HybridSearch,
  createHybridSearch,
  DEFAULT_HYBRID_CONFIG,
} from './HybridSearch.js';
export type {
  HybridSearchConfig,
  HybridSearchResult,
  HybridSearchOptions,
} from './HybridSearch.js';
