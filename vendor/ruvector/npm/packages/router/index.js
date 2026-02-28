const { platform, arch } = process;
const path = require('path');

// Platform mapping for @ruvector/router
const platformMap = {
  'linux': {
    'x64': { package: '@ruvector/router-linux-x64-gnu', file: 'ruvector-router.linux-x64-gnu.node' },
    'arm64': { package: '@ruvector/router-linux-arm64-gnu', file: 'ruvector-router.linux-arm64-gnu.node' }
  },
  'darwin': {
    'x64': { package: '@ruvector/router-darwin-x64', file: 'ruvector-router.darwin-x64.node' },
    'arm64': { package: '@ruvector/router-darwin-arm64', file: 'ruvector-router.darwin-arm64.node' }
  },
  'win32': {
    'x64': { package: '@ruvector/router-win32-x64-msvc', file: 'ruvector-router.win32-x64-msvc.node' }
  }
};

function loadNativeModule() {
  const platformInfo = platformMap[platform]?.[arch];

  if (!platformInfo) {
    throw new Error(
      `Unsupported platform: ${platform}-${arch}\n` +
      `@ruvector/router native module is available for:\n` +
      `- Linux (x64, ARM64)\n` +
      `- macOS (x64, ARM64)\n` +
      `- Windows (x64)\n\n` +
      `Install the package for your platform:\n` +
      `  npm install @ruvector/router`
    );
  }

  // Try local .node file first (for development and bundled packages)
  try {
    const localPath = path.join(__dirname, platformInfo.file);
    return require(localPath);
  } catch (localError) {
    // Fall back to platform-specific package
    try {
      return require(platformInfo.package);
    } catch (error) {
      if (error.code === 'MODULE_NOT_FOUND') {
        throw new Error(
          `Native module not found for ${platform}-${arch}\n` +
          `Please install: npm install ${platformInfo.package}\n` +
          `Or reinstall @ruvector/router to get optional dependencies`
        );
      }
      throw error;
    }
  }
}

// Load native module
const native = loadNativeModule();

/**
 * SemanticRouter - High-level semantic routing for AI agents
 *
 * Wraps the native VectorDB to provide intent-based routing.
 */
class SemanticRouter {
  /**
   * Create a new SemanticRouter
   * @param {Object} config - Router configuration
   * @param {number} config.dimension - Embedding dimension size (required)
   * @param {string} [config.metric='cosine'] - Distance metric: 'cosine', 'euclidean', 'dot', 'manhattan'
   * @param {number} [config.m=16] - HNSW M parameter
   * @param {number} [config.efConstruction=200] - HNSW ef_construction
   * @param {number} [config.efSearch=100] - HNSW ef_search
   * @param {boolean} [config.quantization=false] - Enable quantization (not yet implemented)
   * @param {number} [config.threshold=0.7] - Minimum similarity threshold for matches
   */
  constructor(config) {
    if (!config || typeof config.dimension !== 'number') {
      throw new Error('SemanticRouter requires config.dimension (number)');
    }

    const metricMap = {
      'cosine': native.DistanceMetric.Cosine,
      'euclidean': native.DistanceMetric.Euclidean,
      'dot': native.DistanceMetric.DotProduct,
      'manhattan': native.DistanceMetric.Manhattan
    };

    this._db = new native.VectorDb({
      dimensions: config.dimension,
      distanceMetric: metricMap[config.metric] || native.DistanceMetric.Cosine,
      hnswM: config.m || 16,
      hnswEfConstruction: config.efConstruction || 200,
      hnswEfSearch: config.efSearch || 100
    });

    this._intents = new Map(); // name -> { utterances, metadata, embeddings }
    this._threshold = config.threshold || 0.7;
    this._dimension = config.dimension;
    this._embedder = null; // External embedder function
  }

  /**
   * Set the embedder function for converting text to vectors
   * @param {Function} embedder - Async function (text: string) => Float32Array
   */
  setEmbedder(embedder) {
    if (typeof embedder !== 'function') {
      throw new Error('Embedder must be a function');
    }
    this._embedder = embedder;
  }

  /**
   * Add an intent to the router
   * @param {Object} intent - Intent configuration
   * @param {string} intent.name - Unique intent identifier
   * @param {string[]} intent.utterances - Example utterances for this intent
   * @param {Float32Array|number[]} [intent.embedding] - Pre-computed embedding (centroid)
   * @param {Object} [intent.metadata] - Custom metadata
   */
  addIntent(intent) {
    if (!intent || typeof intent.name !== 'string') {
      throw new Error('Intent requires a name (string)');
    }
    if (!Array.isArray(intent.utterances) || intent.utterances.length === 0) {
      throw new Error('Intent requires utterances (non-empty array)');
    }

    // Store intent info
    this._intents.set(intent.name, {
      utterances: intent.utterances,
      metadata: intent.metadata || {},
      embedding: intent.embedding || null
    });

    // If pre-computed embedding provided, insert directly
    if (intent.embedding) {
      const vector = intent.embedding instanceof Float32Array
        ? intent.embedding
        : new Float32Array(intent.embedding);
      this._db.insert(intent.name, vector);
    }
  }

  /**
   * Add intent with embedding (async version that computes embeddings)
   * @param {Object} intent - Intent configuration
   */
  async addIntentAsync(intent) {
    if (!intent || typeof intent.name !== 'string') {
      throw new Error('Intent requires a name (string)');
    }
    if (!Array.isArray(intent.utterances) || intent.utterances.length === 0) {
      throw new Error('Intent requires utterances (non-empty array)');
    }

    // Store intent info
    this._intents.set(intent.name, {
      utterances: intent.utterances,
      metadata: intent.metadata || {},
      embedding: null
    });

    // Compute embedding if we have an embedder
    if (this._embedder && !intent.embedding) {
      // Compute centroid from all utterances
      const embeddings = await Promise.all(
        intent.utterances.map(u => this._embedder(u))
      );

      // Average the embeddings
      const centroid = new Float32Array(this._dimension);
      for (const emb of embeddings) {
        for (let i = 0; i < this._dimension; i++) {
          centroid[i] += emb[i] / embeddings.length;
        }
      }

      this._intents.get(intent.name).embedding = centroid;
      this._db.insert(intent.name, centroid);
    } else if (intent.embedding) {
      const vector = intent.embedding instanceof Float32Array
        ? intent.embedding
        : new Float32Array(intent.embedding);
      this._intents.get(intent.name).embedding = vector;
      this._db.insert(intent.name, vector);
    }
  }

  /**
   * Route a query to matching intents
   * @param {string|Float32Array} query - Query text or embedding
   * @param {number} [k=1] - Number of results to return
   * @returns {Promise<Array<{intent: string, score: number, metadata: Object}>>}
   */
  async route(query, k = 1) {
    let embedding;

    if (query instanceof Float32Array) {
      embedding = query;
    } else if (typeof query === 'string') {
      if (!this._embedder) {
        throw new Error('No embedder set. Call setEmbedder() first or pass a Float32Array.');
      }
      embedding = await this._embedder(query);
    } else {
      throw new Error('Query must be a string or Float32Array');
    }

    return this.routeWithEmbedding(embedding, k);
  }

  /**
   * Route with a pre-computed embedding (synchronous)
   * @param {Float32Array} embedding - Query embedding
   * @param {number} [k=1] - Number of results to return
   * @returns {Array<{intent: string, score: number, metadata: Object}>}
   */
  routeWithEmbedding(embedding, k = 1) {
    if (!(embedding instanceof Float32Array)) {
      embedding = new Float32Array(embedding);
    }

    const results = this._db.search(embedding, k);

    return results
      .filter(r => r.score >= this._threshold)
      .map(r => {
        const intentInfo = this._intents.get(r.id);
        return {
          intent: r.id,
          score: r.score,
          metadata: intentInfo ? intentInfo.metadata : {}
        };
      });
  }

  /**
   * Remove an intent from the router
   * @param {string} name - Intent name to remove
   * @returns {boolean} - True if removed, false if not found
   */
  removeIntent(name) {
    if (!this._intents.has(name)) {
      return false;
    }
    this._intents.delete(name);
    return this._db.delete(name);
  }

  /**
   * Get all registered intent names
   * @returns {string[]}
   */
  getIntents() {
    return Array.from(this._intents.keys());
  }

  /**
   * Get intent details
   * @param {string} name - Intent name
   * @returns {Object|null} - Intent info or null if not found
   */
  getIntent(name) {
    const info = this._intents.get(name);
    if (!info) return null;
    return {
      name,
      utterances: info.utterances,
      metadata: info.metadata
    };
  }

  /**
   * Clear all intents
   */
  clear() {
    for (const name of this._intents.keys()) {
      this._db.delete(name);
    }
    this._intents.clear();
  }

  /**
   * Get the number of intents
   * @returns {number}
   */
  count() {
    return this._intents.size;
  }

  /**
   * Save router state to disk (intents only, not the index)
   * @param {string} filePath - Path to save to
   */
  async save(filePath) {
    const fs = require('fs').promises;
    const data = {
      dimension: this._dimension,
      threshold: this._threshold,
      intents: []
    };

    for (const [name, info] of this._intents) {
      data.intents.push({
        name,
        utterances: info.utterances,
        metadata: info.metadata,
        embedding: info.embedding ? Array.from(info.embedding) : null
      });
    }

    await fs.writeFile(filePath, JSON.stringify(data, null, 2));
  }

  /**
   * Load router state from disk
   * @param {string} filePath - Path to load from
   */
  async load(filePath) {
    const fs = require('fs').promises;
    const content = await fs.readFile(filePath, 'utf8');
    const data = JSON.parse(content);

    this.clear();
    this._threshold = data.threshold || 0.7;

    for (const intent of data.intents) {
      this.addIntent({
        name: intent.name,
        utterances: intent.utterances,
        metadata: intent.metadata,
        embedding: intent.embedding ? new Float32Array(intent.embedding) : null
      });
    }
  }
}

// Export native module plus SemanticRouter
module.exports = {
  ...native,
  VectorDb: native.VectorDb,
  DistanceMetric: native.DistanceMetric,
  SemanticRouter
};
