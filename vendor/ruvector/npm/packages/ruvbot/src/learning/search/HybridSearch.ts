/**
 * HybridSearch - Combined Vector + Keyword Search
 *
 * Implements Reciprocal Rank Fusion (RRF) to combine vector similarity
 * and BM25 keyword search for improved recall and precision.
 */

import { BM25Index } from './BM25Index.js';
import type { Embedder, VectorIndex } from '../memory/MemoryManager.js';

// ============================================================================
// Types
// ============================================================================

export interface HybridSearchConfig {
  vector: {
    enabled: boolean;
    weight: number;  // 0.0-1.0
  };
  keyword: {
    enabled: boolean;
    weight: number;  // 0.0-1.0
    k1?: number;     // BM25 k1 parameter
    b?: number;      // BM25 b parameter
  };
  fusion: {
    method: 'rrf' | 'linear' | 'weighted';
    k: number;       // RRF constant (default: 60)
    candidateMultiplier: number;  // Fetch more candidates for filtering
  };
}

export interface HybridSearchResult {
  id: string;
  vectorScore: number;
  keywordScore: number;
  fusedScore: number;
  matchedTerms?: string[];
}

export interface HybridSearchOptions {
  topK?: number;
  threshold?: number;
  vectorOnly?: boolean;
  keywordOnly?: boolean;
}

// ============================================================================
// Default Configuration
// ============================================================================

export const DEFAULT_HYBRID_CONFIG: HybridSearchConfig = {
  vector: {
    enabled: true,
    weight: 0.7,
  },
  keyword: {
    enabled: true,
    weight: 0.3,
    k1: 1.2,
    b: 0.75,
  },
  fusion: {
    method: 'rrf',
    k: 60,
    candidateMultiplier: 3,
  },
};

// ============================================================================
// HybridSearch Implementation
// ============================================================================

export class HybridSearch {
  private readonly config: HybridSearchConfig;
  private vectorIndex: VectorIndex | null = null;
  private embedder: Embedder | null = null;
  private bm25Index: BM25Index;
  private initialized: boolean = false;

  constructor(config: Partial<HybridSearchConfig> = {}) {
    this.config = {
      vector: { ...DEFAULT_HYBRID_CONFIG.vector, ...config.vector },
      keyword: { ...DEFAULT_HYBRID_CONFIG.keyword, ...config.keyword },
      fusion: { ...DEFAULT_HYBRID_CONFIG.fusion, ...config.fusion },
    };

    this.bm25Index = new BM25Index({
      k1: this.config.keyword.k1,
      b: this.config.keyword.b,
    });
  }

  /**
   * Initialize with vector index and embedder
   */
  initialize(vectorIndex: VectorIndex, embedder: Embedder): void {
    this.vectorIndex = vectorIndex;
    this.embedder = embedder;
    this.initialized = true;
  }

  /**
   * Check if initialized
   */
  isInitialized(): boolean {
    return this.initialized;
  }

  /**
   * Add document to both indices
   */
  async add(id: string, content: string, embedding?: Float32Array): Promise<void> {
    // Add to BM25 index
    if (this.config.keyword.enabled) {
      this.bm25Index.add(id, content);
    }

    // Add to vector index
    if (this.config.vector.enabled && this.vectorIndex) {
      if (!embedding && this.embedder) {
        embedding = await this.embedder.embed(content);
      }
      if (embedding) {
        await this.vectorIndex.add(id, embedding);
      }
    }
  }

  /**
   * Remove document from both indices
   */
  delete(id: string): boolean {
    let deleted = false;

    if (this.config.keyword.enabled) {
      deleted = this.bm25Index.delete(id) || deleted;
    }

    if (this.config.vector.enabled && this.vectorIndex) {
      deleted = this.vectorIndex.delete(id) || deleted;
    }

    return deleted;
  }

  /**
   * Hybrid search combining vector and keyword
   */
  async search(
    query: string,
    options: HybridSearchOptions = {}
  ): Promise<HybridSearchResult[]> {
    // Return empty results for empty query
    if (!query || query.trim().length === 0) {
      return [];
    }

    const {
      topK = 10,
      threshold = 0,
      vectorOnly = false,
      keywordOnly = false,
    } = options;

    const fetchK = topK * this.config.fusion.candidateMultiplier;

    // Parallel search on both indices
    const [vectorResults, keywordResults] = await Promise.all([
      this.vectorSearch(query, fetchK, vectorOnly || !this.config.keyword.enabled),
      this.keywordSearch(query, fetchK, keywordOnly || !this.config.vector.enabled),
    ]);

    // If only one mode is enabled/requested, return those results
    if (vectorOnly || !this.config.keyword.enabled) {
      return vectorResults
        .filter(r => r.fusedScore >= threshold)
        .slice(0, topK);
    }

    if (keywordOnly || !this.config.vector.enabled) {
      return keywordResults
        .filter(r => r.fusedScore >= threshold)
        .slice(0, topK);
    }

    // Fuse results
    const fused = this.fuseResults(vectorResults, keywordResults);

    return fused
      .filter(r => r.fusedScore >= threshold)
      .slice(0, topK);
  }

  /**
   * Get statistics
   */
  getStats(): {
    config: HybridSearchConfig;
    bm25Stats: ReturnType<BM25Index['getStats']>;
    vectorIndexSize: number;
  } {
    return {
      config: this.config,
      bm25Stats: this.bm25Index.getStats(),
      vectorIndexSize: this.vectorIndex?.size() ?? 0,
    };
  }

  /**
   * Clear both indices
   */
  clear(): void {
    this.bm25Index.clear();
    this.vectorIndex?.clear();
  }

  // ==========================================================================
  // Private Methods
  // ==========================================================================

  private async vectorSearch(
    query: string,
    topK: number,
    returnDirectly: boolean
  ): Promise<HybridSearchResult[]> {
    if (!this.config.vector.enabled || !this.vectorIndex || !this.embedder) {
      return [];
    }

    const queryEmbedding = await this.embedder.embed(query);
    const results = await this.vectorIndex.search(queryEmbedding, topK);

    return results.map((r: { id: string; score: number }) => ({
      id: r.id,
      vectorScore: r.score,
      keywordScore: 0,
      fusedScore: returnDirectly ? r.score : 0,
    }));
  }

  private async keywordSearch(
    query: string,
    topK: number,
    returnDirectly: boolean
  ): Promise<HybridSearchResult[]> {
    if (!this.config.keyword.enabled) {
      return [];
    }

    const results = this.bm25Index.search(query, topK);

    // Normalize BM25 scores to 0-1 range
    const maxScore = results.length > 0 ? results[0].score : 1;

    return results.map(r => ({
      id: r.id,
      vectorScore: 0,
      keywordScore: maxScore > 0 ? r.score / maxScore : 0,
      fusedScore: returnDirectly ? (maxScore > 0 ? r.score / maxScore : 0) : 0,
      matchedTerms: r.matchedTerms,
    }));
  }

  private fuseResults(
    vectorResults: HybridSearchResult[],
    keywordResults: HybridSearchResult[]
  ): HybridSearchResult[] {
    const { method, k } = this.config.fusion;
    const { weight: vectorWeight } = this.config.vector;
    const { weight: keywordWeight } = this.config.keyword;

    // Normalize weights
    const totalWeight = vectorWeight + keywordWeight;
    const normVectorWeight = vectorWeight / totalWeight;
    const normKeywordWeight = keywordWeight / totalWeight;

    // Create maps for quick lookup
    const vectorMap = new Map(vectorResults.map((r, i) => [r.id, { ...r, rank: i + 1 }]));
    const keywordMap = new Map(keywordResults.map((r, i) => [r.id, { ...r, rank: i + 1 }]));

    // Collect all unique IDs
    const allIds = new Set([
      ...vectorResults.map(r => r.id),
      ...keywordResults.map(r => r.id),
    ]);

    // Calculate fused scores
    const fusedResults: HybridSearchResult[] = [];

    for (const id of allIds) {
      const vectorResult = vectorMap.get(id);
      const keywordResult = keywordMap.get(id);

      const vectorScore = vectorResult?.vectorScore ?? 0;
      const keywordScore = keywordResult?.keywordScore ?? 0;

      let fusedScore: number;

      switch (method) {
        case 'rrf': {
          // Reciprocal Rank Fusion
          const vectorRRF = vectorResult ? 1 / (k + vectorResult.rank) : 0;
          const keywordRRF = keywordResult ? 1 / (k + keywordResult.rank) : 0;
          fusedScore = normVectorWeight * vectorRRF + normKeywordWeight * keywordRRF;
          break;
        }

        case 'linear': {
          // Linear combination of scores
          fusedScore = normVectorWeight * vectorScore + normKeywordWeight * keywordScore;
          break;
        }

        case 'weighted':
        default: {
          // Weighted average with presence bonus
          const presence = (vectorResult ? 1 : 0) + (keywordResult ? 1 : 0);
          const presenceBonus = presence === 2 ? 0.1 : 0;
          fusedScore = normVectorWeight * vectorScore + normKeywordWeight * keywordScore + presenceBonus;
          break;
        }
      }

      fusedResults.push({
        id,
        vectorScore,
        keywordScore,
        fusedScore,
        matchedTerms: keywordResult?.matchedTerms,
      });
    }

    // Sort by fused score
    return fusedResults.sort((a, b) => b.fusedScore - a.fusedScore);
  }
}

// ============================================================================
// Factory Function
// ============================================================================

export function createHybridSearch(config?: Partial<HybridSearchConfig>): HybridSearch {
  return new HybridSearch(config);
}

export default HybridSearch;
