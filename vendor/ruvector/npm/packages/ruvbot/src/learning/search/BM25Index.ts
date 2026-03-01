/**
 * BM25Index - Full-Text Search with BM25 Scoring
 *
 * Implements the Okapi BM25 ranking algorithm for keyword-based search.
 * Used in hybrid search to complement vector similarity search.
 */

// ============================================================================
// Types
// ============================================================================

export interface BM25Config {
  k1: number;  // Term frequency saturation (default: 1.2)
  b: number;   // Document length normalization (default: 0.75)
}

export interface Document {
  id: string;
  content: string;
  tokens?: string[];
}

export interface BM25Result {
  id: string;
  score: number;
  matchedTerms: string[];
}

// ============================================================================
// BM25Index Implementation
// ============================================================================

export class BM25Index {
  private readonly k1: number;
  private readonly b: number;

  // Document storage
  private documents: Map<string, Document> = new Map();

  // Inverted index: term -> Set of document IDs
  private invertedIndex: Map<string, Set<string>> = new Map();

  // Document frequency: term -> number of documents containing term
  private docFrequency: Map<string, number> = new Map();

  // Document lengths (number of tokens)
  private docLengths: Map<string, number> = new Map();

  // Average document length
  private avgDocLength: number = 0;

  // Stopwords to filter
  private readonly stopwords = new Set([
    'a', 'an', 'the', 'and', 'or', 'but', 'is', 'are', 'was', 'were',
    'be', 'been', 'being', 'have', 'has', 'had', 'do', 'does', 'did',
    'will', 'would', 'could', 'should', 'may', 'might', 'must', 'shall',
    'can', 'need', 'dare', 'ought', 'used', 'to', 'of', 'in', 'for',
    'on', 'with', 'at', 'by', 'from', 'as', 'into', 'through', 'during',
    'before', 'after', 'above', 'below', 'between', 'under', 'again',
    'further', 'then', 'once', 'here', 'there', 'when', 'where', 'why',
    'how', 'all', 'each', 'few', 'more', 'most', 'other', 'some', 'such',
    'no', 'nor', 'not', 'only', 'own', 'same', 'so', 'than', 'too',
    'very', 's', 't', 'just', 'don', 'now', 'i', 'me', 'my', 'myself',
    'we', 'our', 'ours', 'ourselves', 'you', 'your', 'yours', 'yourself',
    'he', 'him', 'his', 'himself', 'she', 'her', 'hers', 'herself',
    'it', 'its', 'itself', 'they', 'them', 'their', 'theirs', 'themselves',
    'what', 'which', 'who', 'whom', 'this', 'that', 'these', 'those',
  ]);

  constructor(config: Partial<BM25Config> = {}) {
    this.k1 = config.k1 ?? 1.2;
    this.b = config.b ?? 0.75;
  }

  /**
   * Add a document to the index
   */
  add(id: string, content: string): void {
    // Tokenize content
    const tokens = this.tokenize(content);

    // Store document
    const doc: Document = { id, content, tokens };
    this.documents.set(id, doc);
    this.docLengths.set(id, tokens.length);

    // Update inverted index
    const uniqueTerms = new Set(tokens);
    for (const term of uniqueTerms) {
      if (!this.invertedIndex.has(term)) {
        this.invertedIndex.set(term, new Set());
      }
      this.invertedIndex.get(term)!.add(id);

      // Update document frequency
      this.docFrequency.set(term, (this.docFrequency.get(term) ?? 0) + 1);
    }

    // Update average document length
    this.updateAvgDocLength();
  }

  /**
   * Remove a document from the index
   */
  delete(id: string): boolean {
    const doc = this.documents.get(id);
    if (!doc) return false;

    // Remove from inverted index
    const uniqueTerms = new Set(doc.tokens ?? this.tokenize(doc.content));
    for (const term of uniqueTerms) {
      const termDocs = this.invertedIndex.get(term);
      if (termDocs) {
        termDocs.delete(id);
        if (termDocs.size === 0) {
          this.invertedIndex.delete(term);
          this.docFrequency.delete(term);
        } else {
          this.docFrequency.set(term, (this.docFrequency.get(term) ?? 1) - 1);
        }
      }
    }

    // Remove document
    this.documents.delete(id);
    this.docLengths.delete(id);

    // Update average document length
    this.updateAvgDocLength();

    return true;
  }

  /**
   * Search the index with BM25 scoring
   */
  search(query: string, topK: number = 10): BM25Result[] {
    const queryTerms = this.tokenize(query);
    if (queryTerms.length === 0) return [];

    const scores = new Map<string, { score: number; matchedTerms: string[] }>();
    const N = this.documents.size;

    for (const term of queryTerms) {
      const docs = this.invertedIndex.get(term);
      if (!docs) continue;

      // Document frequency for IDF
      const df = this.docFrequency.get(term) ?? 0;
      // IDF with smoothing
      const idf = Math.log((N - df + 0.5) / (df + 0.5) + 1);

      for (const docId of docs) {
        const docLength = this.docLengths.get(docId) ?? 0;
        const doc = this.documents.get(docId);
        if (!doc) continue;

        // Term frequency in document
        const tf = this.termFrequency(term, doc.tokens ?? []);

        // BM25 score for this term
        const numerator = tf * (this.k1 + 1);
        const denominator = tf + this.k1 * (1 - this.b + this.b * (docLength / this.avgDocLength));
        const termScore = idf * (numerator / denominator);

        // Accumulate score
        if (!scores.has(docId)) {
          scores.set(docId, { score: 0, matchedTerms: [] });
        }
        const existing = scores.get(docId)!;
        existing.score += termScore;
        if (!existing.matchedTerms.includes(term)) {
          existing.matchedTerms.push(term);
        }
      }
    }

    // Sort by score and return top K
    return Array.from(scores.entries())
      .map(([id, { score, matchedTerms }]) => ({ id, score, matchedTerms }))
      .sort((a, b) => b.score - a.score)
      .slice(0, topK);
  }

  /**
   * Get document by ID
   */
  get(id: string): Document | undefined {
    return this.documents.get(id);
  }

  /**
   * Check if document exists
   */
  has(id: string): boolean {
    return this.documents.has(id);
  }

  /**
   * Get index size
   */
  size(): number {
    return this.documents.size;
  }

  /**
   * Clear all documents
   */
  clear(): void {
    this.documents.clear();
    this.invertedIndex.clear();
    this.docFrequency.clear();
    this.docLengths.clear();
    this.avgDocLength = 0;
  }

  /**
   * Get index statistics
   */
  getStats(): {
    documentCount: number;
    uniqueTerms: number;
    avgDocLength: number;
    k1: number;
    b: number;
  } {
    return {
      documentCount: this.documents.size,
      uniqueTerms: this.invertedIndex.size,
      avgDocLength: this.avgDocLength,
      k1: this.k1,
      b: this.b,
    };
  }

  // ==========================================================================
  // Private Methods
  // ==========================================================================

  /**
   * Tokenize text into normalized terms
   */
  private tokenize(text: string): string[] {
    return text
      .toLowerCase()
      // Split on non-alphanumeric characters
      .split(/[^a-z0-9]+/)
      // Filter empty strings and stopwords
      .filter(token => token.length > 1 && !this.stopwords.has(token))
      // Stem basic suffixes (simple Porter-like stemming)
      .map(token => this.stem(token));
  }

  /**
   * Simple stemming (basic suffix removal)
   */
  private stem(word: string): string {
    // Very basic stemming - just remove common suffixes
    if (word.length > 5) {
      if (word.endsWith('ing')) return word.slice(0, -3);
      if (word.endsWith('ed')) return word.slice(0, -2);
      if (word.endsWith('es')) return word.slice(0, -2);
      if (word.endsWith('s') && !word.endsWith('ss')) return word.slice(0, -1);
      if (word.endsWith('ly')) return word.slice(0, -2);
      if (word.endsWith('tion')) return word.slice(0, -4) + 't';
    }
    return word;
  }

  /**
   * Count term frequency in tokens
   */
  private termFrequency(term: string, tokens: string[]): number {
    return tokens.filter(t => t === term).length;
  }

  /**
   * Update average document length
   */
  private updateAvgDocLength(): void {
    if (this.docLengths.size === 0) {
      this.avgDocLength = 0;
      return;
    }
    let total = 0;
    for (const length of this.docLengths.values()) {
      total += length;
    }
    this.avgDocLength = total / this.docLengths.size;
  }
}

// ============================================================================
// Factory Function
// ============================================================================

export function createBM25Index(config?: Partial<BM25Config>): BM25Index {
  return new BM25Index(config);
}

export default BM25Index;
