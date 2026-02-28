/**
 * Mock VectorDB implementation for testing
 * This simulates the interface that @ruvector/core and @ruvector/wasm will provide
 */

class VectorDB {
  constructor(options) {
    this.options = options;
    this.dimension = options.dimension;
    this.metric = options.metric || 'cosine';
    this.vectors = new Map();
  }

  insert(entry) {
    if (!entry.id || !entry.vector) {
      throw new Error('Entry must have id and vector');
    }
    if (entry.vector.length !== this.dimension) {
      throw new Error(`Vector dimension must be ${this.dimension}`);
    }
    this.vectors.set(entry.id, {
      id: entry.id,
      vector: entry.vector,
      metadata: entry.metadata || {}
    });
  }

  insertBatch(entries) {
    for (const entry of entries) {
      this.insert(entry);
    }
  }

  search(query) {
    const results = [];
    const k = query.k || 10;
    const threshold = query.threshold || 0.0;

    for (const [id, entry] of this.vectors.entries()) {
      const score = this._computeSimilarity(query.vector, entry.vector);
      if (score >= threshold) {
        results.push({
          id: entry.id,
          score,
          vector: entry.vector,
          metadata: entry.metadata
        });
      }
    }

    // Sort by score descending
    results.sort((a, b) => b.score - a.score);

    return results.slice(0, k);
  }

  get(id) {
    return this.vectors.get(id) || null;
  }

  delete(id) {
    return this.vectors.delete(id);
  }

  updateMetadata(id, metadata) {
    const entry = this.vectors.get(id);
    if (entry) {
      entry.metadata = { ...entry.metadata, ...metadata };
    }
  }

  stats() {
    return {
      count: this.vectors.size,
      dimension: this.dimension,
      metric: this.metric,
      memoryUsage: this.vectors.size * this.dimension * 8, // rough estimate
      indexType: 'flat'
    };
  }

  save(path) {
    // Mock save
    const data = {
      dimension: this.dimension,
      metric: this.metric,
      vectors: Array.from(this.vectors.values())
    };
    return JSON.stringify(data);
  }

  load(path) {
    // Mock load - would read from file
    this.vectors.clear();
  }

  clear() {
    this.vectors.clear();
  }

  buildIndex() {
    // Mock index building
  }

  optimize() {
    // Mock optimization
  }

  _computeSimilarity(a, b) {
    if (this.metric === 'cosine') {
      return this._cosineSimilarity(a, b);
    } else if (this.metric === 'euclidean') {
      return 1 / (1 + this._euclideanDistance(a, b));
    } else {
      return this._dotProduct(a, b);
    }
  }

  _cosineSimilarity(a, b) {
    let dot = 0;
    let magA = 0;
    let magB = 0;

    for (let i = 0; i < a.length; i++) {
      dot += a[i] * b[i];
      magA += a[i] * a[i];
      magB += b[i] * b[i];
    }

    return dot / (Math.sqrt(magA) * Math.sqrt(magB));
  }

  _euclideanDistance(a, b) {
    let sum = 0;
    for (let i = 0; i < a.length; i++) {
      const diff = a[i] - b[i];
      sum += diff * diff;
    }
    return Math.sqrt(sum);
  }

  _dotProduct(a, b) {
    let sum = 0;
    for (let i = 0; i < a.length; i++) {
      sum += a[i] * b[i];
    }
    return sum;
  }
}

module.exports = { VectorDB };
