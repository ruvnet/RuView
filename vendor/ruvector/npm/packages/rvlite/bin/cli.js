#!/usr/bin/env node

import { Command } from 'commander';
import chalk from 'chalk';
import ora from 'ora';
import fs from 'fs';
import path from 'path';
import readline from 'readline';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const VERSION = '0.3.0';

const program = new Command();

program
  .name('rvlite')
  .description('Lightweight vector database with SQL, SPARQL, and Cypher')
  .version(VERSION);

// Database state (in-memory for now, persisted to JSON)
let state = {
  vectors: {},   // id -> { vector, metadata, norm }
  graph: { nodes: {}, edges: {} },
  triples: [],
  nextId: 1
};

// Precomputed norms cache for faster search
let normCache = new Map();

const DEFAULT_DB_PATH = '.rvlite/db.json';

// Load state from file
function loadState(dbPath) {
  if (fs.existsSync(dbPath)) {
    try {
      state = JSON.parse(fs.readFileSync(dbPath, 'utf-8'));
      // Rebuild norm cache
      rebuildNormCache();
    } catch (e) {
      // Start fresh if file is corrupt
    }
  }
}

// Save state to file (optimized: no pretty print for large DBs)
function saveState(dbPath, pretty = true) {
  const dir = path.dirname(dbPath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  const vectorCount = Object.keys(state.vectors).length;
  // Skip pretty print for large databases (>1000 vectors)
  const json = vectorCount > 1000
    ? JSON.stringify(state)
    : JSON.stringify(state, null, 2);
  fs.writeFileSync(dbPath, json);
}

// Rebuild norm cache from state
function rebuildNormCache() {
  normCache.clear();
  for (const [id, entry] of Object.entries(state.vectors)) {
    if (entry.norm === undefined) {
      entry.norm = computeNorm(entry.vector);
    }
    normCache.set(id, entry.norm);
  }
}

// Generate unique ID
function generateId() {
  return `vec_${state.nextId++}`;
}

// Compute vector norm (magnitude)
function computeNorm(vec) {
  let sum = 0;
  for (let i = 0; i < vec.length; i++) {
    sum += vec[i] * vec[i];
  }
  return Math.sqrt(sum);
}

// Optimized cosine similarity (using precomputed norms)
function cosineSimilarity(a, b, normA, normB) {
  if (normA === 0 || normB === 0) return 0;
  let dot = 0;
  // Unrolled loop for better performance
  const len = a.length;
  let i = 0;
  for (; i + 3 < len; i += 4) {
    dot += a[i] * b[i] + a[i+1] * b[i+1] + a[i+2] * b[i+2] + a[i+3] * b[i+3];
  }
  for (; i < len; i++) {
    dot += a[i] * b[i];
  }
  return dot / (normA * normB);
}

// Euclidean distance (squared, for comparison only)
function euclideanDistanceSquared(a, b) {
  let sum = 0;
  const len = a.length;
  for (let i = 0; i < len; i++) {
    const diff = a[i] - b[i];
    sum += diff * diff;
  }
  return sum;
}

// Dot product similarity
function dotProduct(a, b) {
  let dot = 0;
  const len = a.length;
  for (let i = 0; i < len; i++) {
    dot += a[i] * b[i];
  }
  return dot;
}

// ============ HYPERBOLIC GEOMETRY ============

// Project vector to Poincare ball (ensure ||x|| < 1)
function projectToPoincareBall(vec, eps = 1e-5) {
  const norm = computeNorm(vec);
  const maxNorm = 1 - eps;
  if (norm >= maxNorm) {
    const scale = maxNorm / norm;
    return vec.map(v => v * scale);
  }
  return vec;
}

// Poincare distance (hyperbolic distance in Poincare ball model)
function poincareDistance(a, b) {
  // Project to ball if needed
  const u = projectToPoincareBall(a);
  const v = projectToPoincareBall(b);

  // ||u - v||^2
  let diffSq = 0;
  for (let i = 0; i < u.length; i++) {
    const d = u[i] - v[i];
    diffSq += d * d;
  }

  // ||u||^2 and ||v||^2
  let normUSq = 0, normVSq = 0;
  for (let i = 0; i < u.length; i++) {
    normUSq += u[i] * u[i];
    normVSq += v[i] * v[i];
  }

  // d(u,v) = arcosh(1 + 2 * ||u-v||^2 / ((1-||u||^2)(1-||v||^2)))
  const denom = (1 - normUSq) * (1 - normVSq);
  if (denom <= 0) return Infinity;

  const arg = 1 + 2 * diffSq / denom;
  // arcosh(x) = ln(x + sqrt(x^2 - 1))
  return Math.log(arg + Math.sqrt(arg * arg - 1));
}

// Mobius addition (gyrovector addition in Poincare ball)
function mobiusAdd(u, v, c = 1.0) {
  const uDotV = dotProduct(u, v);
  const normUSq = u.reduce((s, x) => s + x*x, 0);
  const normVSq = v.reduce((s, x) => s + x*x, 0);

  const denom = 1 + 2 * c * uDotV + c * c * normUSq * normVSq;
  const result = [];
  for (let i = 0; i < u.length; i++) {
    const num = (1 + 2 * c * uDotV + c * normVSq) * u[i] + (1 - c * normUSq) * v[i];
    result.push(num / denom);
  }
  return projectToPoincareBall(result);
}

// Lorentz inner product <x,y>_L = -x0*y0 + x1*y1 + ... + xn*yn
function lorentzInner(a, b) {
  let result = -a[0] * b[0]; // Time component is negative
  for (let i = 1; i < a.length; i++) {
    result += a[i] * b[i];
  }
  return result;
}

// Project to Lorentz hyperboloid (ensure x0 = sqrt(1 + ||x_space||^2))
function projectToHyperboloid(vec) {
  // Spatial components
  let spaceSq = 0;
  for (let i = 1; i < vec.length; i++) {
    spaceSq += vec[i] * vec[i];
  }
  // Time component (x0) must satisfy -x0^2 + ||x_space||^2 = -1
  const x0 = Math.sqrt(1 + spaceSq);
  return [x0, ...vec.slice(1)];
}

// Convert Euclidean vector to Lorentz hyperboloid point
function euclideanToLorentz(vec) {
  // Scale down to fit on hyperboloid
  const norm = computeNorm(vec);
  const scale = Math.min(norm, 0.99) / (norm || 1);
  const scaled = vec.map(v => v * scale * 0.5);

  // Add time dimension
  let spaceSq = 0;
  for (const v of scaled) {
    spaceSq += v * v;
  }
  const x0 = Math.sqrt(1 + spaceSq);
  return [x0, ...scaled];
}

// Lorentz distance (hyperbolic distance on hyperboloid)
function lorentzDistance(a, b) {
  // Convert to hyperboloid representation
  const u = euclideanToLorentz(a);
  const v = euclideanToLorentz(b);

  // d(u,v) = arcosh(-<u,v>_L)
  const inner = -lorentzInner(u, v);
  if (inner < 1) return 0;
  return Math.log(inner + Math.sqrt(inner * inner - 1));
}

// ============ TEXT EMBEDDING ============

// Hash-based embedding (fast but not semantic - for testing/prototyping)
function hashEmbed(text, dimensions = 384) {
  const embedding = new Array(dimensions).fill(0);
  const bytes = Buffer.from(text.toLowerCase());

  // Character n-gram hashing with position encoding
  for (let i = 0; i < bytes.length; i++) {
    // Unigram
    embedding[bytes[i] % dimensions] += 1.0;

    // Bigram
    if (i > 0) {
      const bigram = (bytes[i-1] << 8) | bytes[i];
      embedding[bigram % dimensions] += 0.5;
    }

    // Trigram
    if (i > 1) {
      const trigram = (bytes[i-2] << 16) | (bytes[i-1] << 8) | bytes[i];
      embedding[trigram % dimensions] += 0.25;
    }

    // Position encoding (sine/cosine like transformers)
    const pos = i / bytes.length;
    for (let d = 0; d < Math.min(64, dimensions); d++) {
      const freq = 1.0 / Math.pow(10000, d / 64);
      if (d % 2 === 0) {
        embedding[d] += Math.sin(pos * freq) * 0.1;
      } else {
        embedding[d] += Math.cos(pos * freq) * 0.1;
      }
    }
  }

  // L2 normalize
  let norm = 0;
  for (const v of embedding) {
    norm += v * v;
  }
  norm = Math.sqrt(norm);
  if (norm > 0) {
    for (let i = 0; i < dimensions; i++) {
      embedding[i] /= norm;
    }
  }

  return embedding;
}

// Optimized search with precomputed norms and early termination
function searchVectors(query, k, metric = 'cosine') {
  const entries = Object.entries(state.vectors);
  if (entries.length === 0) return [];

  const queryNorm = computeNorm(query);
  const results = [];

  if (metric === 'cosine') {
    for (const [id, entry] of entries) {
      const vecNorm = normCache.get(id) || computeNorm(entry.vector);
      const score = cosineSimilarity(query, entry.vector, queryNorm, vecNorm);
      results.push({ id, score, metadata: entry.metadata });
    }
    results.sort((a, b) => b.score - a.score);
  } else if (metric === 'euclidean') {
    for (const [id, entry] of entries) {
      const dist = euclideanDistanceSquared(query, entry.vector);
      results.push({ id, score: -dist, distance: Math.sqrt(dist), metadata: entry.metadata });
    }
    results.sort((a, b) => b.score - a.score);
  } else if (metric === 'dotproduct') {
    for (const [id, entry] of entries) {
      const score = dotProduct(query, entry.vector);
      results.push({ id, score, metadata: entry.metadata });
    }
    results.sort((a, b) => b.score - a.score);
  } else if (metric === 'poincare') {
    // Hyperbolic distance in Poincare ball model
    for (const [id, entry] of entries) {
      const dist = poincareDistance(query, entry.vector);
      results.push({ id, score: -dist, distance: dist, metadata: entry.metadata });
    }
    results.sort((a, b) => b.score - a.score); // Smaller distance = higher score
  } else if (metric === 'lorentz') {
    // Hyperbolic distance on Lorentz hyperboloid
    for (const [id, entry] of entries) {
      const dist = lorentzDistance(query, entry.vector);
      results.push({ id, score: -dist, distance: dist, metadata: entry.metadata });
    }
    results.sort((a, b) => b.score - a.score);
  }

  return results.slice(0, k);
}

// Insert vector with precomputed norm
function insertVector(vector, metadata, id) {
  const vecId = id || generateId();
  const norm = computeNorm(vector);
  state.vectors[vecId] = { vector, metadata, norm };
  normCache.set(vecId, norm);
  return vecId;
}

// Batch insert for bulk loading
function insertBatch(vectors) {
  const ids = [];
  for (const { vector, metadata, id } of vectors) {
    ids.push(insertVector(vector, metadata, id));
  }
  return ids;
}

// Delete vector
function deleteVector(id) {
  if (state.vectors[id]) {
    delete state.vectors[id];
    normCache.delete(id);
    return true;
  }
  return false;
}

// ============ COMMANDS ============

program
  .command('init')
  .description('Initialize a new RvLite database')
  .option('-d, --dimensions <n>', 'Vector dimensions', '384')
  .option('-m, --metric <type>', 'Distance metric (cosine, euclidean, dotproduct)', 'cosine')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .action(async (options) => {
    const spinner = ora('Initializing RvLite database...').start();
    try {
      state = {
        vectors: {},
        graph: { nodes: {}, edges: {} },
        triples: [],
        nextId: 1,
        config: {
          dimensions: parseInt(options.dimensions),
          metric: options.metric
        }
      };
      normCache.clear();
      saveState(options.path);
      spinner.succeed(chalk.green(`Database initialized at ${options.path}`));
      console.log(chalk.dim(`  Dimensions: ${options.dimensions}`));
      console.log(chalk.dim(`  Metric: ${options.metric}`));
    } catch (err) {
      spinner.fail(chalk.red(`Failed: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('insert')
  .description('Insert a vector with metadata')
  .argument('<vector>', 'Vector as JSON array, e.g., "[0.1, 0.2, ...]"')
  .option('-m, --metadata <json>', 'Metadata as JSON object')
  .option('-i, --id <id>', 'Custom ID')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .action(async (vectorStr, options) => {
    try {
      loadState(options.path);
      const vector = JSON.parse(vectorStr);
      const metadata = options.metadata ? JSON.parse(options.metadata) : null;
      const id = insertVector(vector, metadata, options.id);
      saveState(options.path);
      console.log(chalk.green(`Inserted vector: ${id}`));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('embed')
  .description('Generate embedding from text (hash-based, fast but not semantic)')
  .argument('<text>', 'Text to embed')
  .option('-d, --dimensions <n>', 'Embedding dimensions', '384')
  .option('--insert', 'Also insert into database')
  .option('-m, --metadata <json>', 'Metadata (when using --insert)')
  .option('-p, --path <path>', 'Database path (when using --insert)', DEFAULT_DB_PATH)
  .action(async (text, options) => {
    try {
      const dims = parseInt(options.dimensions);
      const embedding = hashEmbed(text, dims);

      if (options.insert) {
        loadState(options.path);
        const metadata = options.metadata ? JSON.parse(options.metadata) : { text };
        const id = insertVector(embedding, metadata);
        saveState(options.path);
        console.log(chalk.green(`Embedded and inserted: ${id}`));
        console.log(chalk.dim(`  Text: "${text.substring(0, 50)}${text.length > 50 ? '...' : ''}"`));
        console.log(chalk.dim(`  Dimensions: ${dims}`));
      } else {
        console.log(JSON.stringify(embedding));
      }
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('embed-search')
  .description('Search using text query (generates embedding automatically)')
  .argument('<text>', 'Text query')
  .option('-k, --top <n>', 'Number of results', '5')
  .option('-d, --dimensions <n>', 'Embedding dimensions', '384')
  .option('--metric <type>', 'Distance metric (cosine, euclidean, poincare, lorentz)', 'cosine')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .action(async (text, options) => {
    try {
      loadState(options.path);
      const dims = parseInt(options.dimensions);
      const queryVec = hashEmbed(text, dims);
      const start = performance.now();
      const results = searchVectors(queryVec, parseInt(options.top), options.metric);
      const elapsed = performance.now() - start;

      console.log(chalk.cyan(`Query: "${text}"`));
      console.log(chalk.cyan('Search Results:'));
      console.log(JSON.stringify(results, null, 2));
      console.log(chalk.dim(`  Search time: ${elapsed.toFixed(3)}ms`));
      console.log(chalk.dim(`  Metric: ${options.metric}`));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('batch-insert')
  .description('Insert multiple vectors from JSON file')
  .argument('<file>', 'JSON file with array of {vector, metadata?, id?}')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .action(async (file, options) => {
    const spinner = ora('Batch inserting vectors...').start();
    try {
      loadState(options.path);
      const vectors = JSON.parse(fs.readFileSync(file, 'utf-8'));
      const start = performance.now();
      const ids = insertBatch(vectors);
      const elapsed = performance.now() - start;
      saveState(options.path, false);
      spinner.succeed(chalk.green(`Inserted ${ids.length} vectors in ${elapsed.toFixed(2)}ms`));
      console.log(chalk.dim(`  Rate: ${(ids.length / (elapsed / 1000)).toFixed(0)} vectors/sec`));
    } catch (err) {
      spinner.fail(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('search')
  .description('Search for similar vectors')
  .argument('<vector>', 'Query vector as JSON array')
  .option('-k, --top <n>', 'Number of results', '5')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .option('--metric <type>', 'Override distance metric')
  .action(async (vectorStr, options) => {
    try {
      loadState(options.path);
      const vector = JSON.parse(vectorStr);
      const metric = options.metric || state.config?.metric || 'cosine';
      const start = performance.now();
      const results = searchVectors(vector, parseInt(options.top), metric);
      const elapsed = performance.now() - start;
      console.log(chalk.cyan('Search Results:'));
      console.log(JSON.stringify(results, null, 2));
      console.log(chalk.dim(`  Search time: ${elapsed.toFixed(3)}ms`));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('get')
  .description('Get a vector by ID')
  .argument('<id>', 'Vector ID')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .action(async (id, options) => {
    try {
      loadState(options.path);
      const entry = state.vectors[id];
      if (entry) {
        console.log(JSON.stringify({ vector: entry.vector, metadata: entry.metadata }, null, 2));
      } else {
        console.error(chalk.red(`Vector not found: ${id}`));
        process.exit(1);
      }
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('delete')
  .description('Delete a vector by ID')
  .argument('<id>', 'Vector ID')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .action(async (id, options) => {
    try {
      loadState(options.path);
      if (deleteVector(id)) {
        saveState(options.path);
        console.log(chalk.green(`Deleted: ${id}`));
      } else {
        console.error(chalk.red(`Vector not found: ${id}`));
        process.exit(1);
      }
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('triple')
  .description('Add an RDF triple')
  .argument('<subject>', 'Subject IRI')
  .argument('<predicate>', 'Predicate IRI')
  .argument('<object>', 'Object (IRI or literal)')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .action(async (subject, predicate, object, options) => {
    try {
      loadState(options.path);
      state.triples.push({ subject, predicate, object });
      saveState(options.path);
      console.log(chalk.green('Triple added'));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('triples')
  .description('List all triples')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .option('--limit <n>', 'Max results', '100')
  .action(async (options) => {
    try {
      loadState(options.path);
      const limit = parseInt(options.limit);
      const results = state.triples.slice(0, limit);
      console.log(JSON.stringify(results, null, 2));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('stats')
  .description('Show database statistics')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .action(async (options) => {
    try {
      loadState(options.path);
      const vectorCount = Object.keys(state.vectors).length;
      const tripleCount = state.triples.length;
      const nodeCount = Object.keys(state.graph?.nodes || {}).length;
      const edgeCount = Object.keys(state.graph?.edges || {}).length;

      // Calculate memory usage estimate
      let memoryBytes = 0;
      for (const entry of Object.values(state.vectors)) {
        memoryBytes += entry.vector.length * 8; // 64-bit floats
        memoryBytes += 8; // norm
        memoryBytes += JSON.stringify(entry.metadata || {}).length;
      }

      console.log(chalk.cyan('\nRvLite Database Statistics'));
      console.log(chalk.dim('─'.repeat(40)));
      console.log(`  Vectors:      ${chalk.yellow(vectorCount)}`);
      console.log(`  Graph Nodes:  ${chalk.yellow(nodeCount)}`);
      console.log(`  Graph Edges:  ${chalk.yellow(edgeCount)}`);
      console.log(`  RDF Triples:  ${chalk.yellow(tripleCount)}`);
      if (state.config) {
        console.log(chalk.dim('─'.repeat(40)));
        console.log(`  Dimensions:   ${chalk.dim(state.config.dimensions || 'N/A')}`);
        console.log(`  Metric:       ${chalk.dim(state.config.metric || 'cosine')}`);
      }
      console.log(chalk.dim('─'.repeat(40)));
      console.log(`  Memory Est:   ${chalk.dim(formatBytes(memoryBytes))}`);
      console.log(`  DB File:      ${chalk.dim(formatBytes(fs.existsSync(options.path) ? fs.statSync(options.path).size : 0))}`);
      console.log(chalk.dim('─'.repeat(40)));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('benchmark')
  .description('Run performance benchmarks')
  .option('-n, --count <n>', 'Number of vectors to test', '1000')
  .option('-d, --dimensions <n>', 'Vector dimensions', '384')
  .option('-k, --top <n>', 'Search results count', '10')
  .option('-p, --path <path>', 'Database path', '/tmp/rvlite-bench.db')
  .action(async (options) => {
    const count = parseInt(options.count);
    const dims = parseInt(options.dimensions);
    const k = parseInt(options.top);

    console.log(chalk.cyan(`\nRvLite Benchmark Suite v${VERSION}`));
    console.log(chalk.dim('═'.repeat(50)));
    console.log(chalk.dim(`  Vectors: ${count}, Dimensions: ${dims}, Top-K: ${k}`));
    console.log(chalk.dim('═'.repeat(50)));

    // Generate random vectors
    const generateVector = () => Array.from({ length: dims }, () => Math.random() * 2 - 1);

    // Initialize fresh state
    state = { vectors: {}, graph: { nodes: {}, edges: {} }, triples: [], nextId: 1, config: { dimensions: dims, metric: 'cosine' } };
    normCache.clear();

    // Benchmark: Insert
    console.log(chalk.yellow('\n1. Insert Benchmark'));
    const vectors = [];
    for (let i = 0; i < count; i++) {
      vectors.push({ vector: generateVector(), metadata: { index: i } });
    }

    const insertStart = performance.now();
    insertBatch(vectors);
    const insertTime = performance.now() - insertStart;
    console.log(`   ${chalk.green('✓')} Inserted ${count} vectors in ${insertTime.toFixed(2)}ms`);
    console.log(`   ${chalk.dim(`  Rate: ${(count / (insertTime / 1000)).toFixed(0)} inserts/sec`)}`);

    // Benchmark: Search (cold)
    console.log(chalk.yellow('\n2. Search Benchmark (Cold)'));
    const queryVec = generateVector();
    const searchStart1 = performance.now();
    searchVectors(queryVec, k, 'cosine');
    const searchTime1 = performance.now() - searchStart1;
    console.log(`   ${chalk.green('✓')} Search ${count} vectors in ${searchTime1.toFixed(3)}ms`);
    console.log(`   ${chalk.dim(`  Throughput: ${(count / (searchTime1 / 1000)).toFixed(0)} comparisons/sec`)}`);

    // Benchmark: Search (warm - multiple queries)
    console.log(chalk.yellow('\n3. Search Benchmark (Warm - 100 queries)'));
    const queries = Array.from({ length: 100 }, generateVector);
    const searchStart2 = performance.now();
    for (const q of queries) {
      searchVectors(q, k, 'cosine');
    }
    const searchTime2 = performance.now() - searchStart2;
    const avgSearchTime = searchTime2 / 100;
    console.log(`   ${chalk.green('✓')} 100 searches in ${searchTime2.toFixed(2)}ms`);
    console.log(`   ${chalk.dim(`  Avg per query: ${avgSearchTime.toFixed(3)}ms`)}`);
    console.log(`   ${chalk.dim(`  Queries/sec: ${(100 / (searchTime2 / 1000)).toFixed(0)}`)}`);

    // Benchmark: Persistence (Save)
    console.log(chalk.yellow('\n4. Persistence Benchmark'));
    const saveStart = performance.now();
    saveState(options.path, false);
    const saveTime = performance.now() - saveStart;
    const fileSize = fs.statSync(options.path).size;
    console.log(`   ${chalk.green('✓')} Saved to disk in ${saveTime.toFixed(2)}ms`);
    console.log(`   ${chalk.dim(`  File size: ${formatBytes(fileSize)}`)}`);
    console.log(`   ${chalk.dim(`  Write speed: ${formatBytes(fileSize / (saveTime / 1000))}/sec`)}`);

    // Benchmark: Persistence (Load)
    state = { vectors: {}, graph: { nodes: {}, edges: {} }, triples: [], nextId: 1 };
    normCache.clear();
    const loadStart = performance.now();
    loadState(options.path);
    const loadTime = performance.now() - loadStart;
    console.log(`   ${chalk.green('✓')} Loaded from disk in ${loadTime.toFixed(2)}ms`);
    console.log(`   ${chalk.dim(`  Read speed: ${formatBytes(fileSize / (loadTime / 1000))}/sec`)}`);

    // Benchmark: Distance metrics comparison
    console.log(chalk.yellow('\n5. Distance Metrics Comparison'));
    const testQuery = generateVector();

    const cosStart = performance.now();
    for (let i = 0; i < 10; i++) searchVectors(testQuery, k, 'cosine');
    const cosTime = (performance.now() - cosStart) / 10;

    const eucStart = performance.now();
    for (let i = 0; i < 10; i++) searchVectors(testQuery, k, 'euclidean');
    const eucTime = (performance.now() - eucStart) / 10;

    const dotStart = performance.now();
    for (let i = 0; i < 10; i++) searchVectors(testQuery, k, 'dotproduct');
    const dotTime = (performance.now() - dotStart) / 10;

    const poincareStart = performance.now();
    for (let i = 0; i < 10; i++) searchVectors(testQuery, k, 'poincare');
    const poincareTime = (performance.now() - poincareStart) / 10;

    const lorentzStart = performance.now();
    for (let i = 0; i < 10; i++) searchVectors(testQuery, k, 'lorentz');
    const lorentzTime = (performance.now() - lorentzStart) / 10;

    console.log(chalk.dim('   Euclidean Metrics:'));
    console.log(`     Cosine:      ${cosTime.toFixed(3)}ms`);
    console.log(`     Euclidean:   ${eucTime.toFixed(3)}ms`);
    console.log(`     Dot Product: ${dotTime.toFixed(3)}ms`);
    console.log(chalk.dim('   Hyperbolic Metrics:'));
    console.log(`     Poincare:    ${poincareTime.toFixed(3)}ms`);
    console.log(`     Lorentz:     ${lorentzTime.toFixed(3)}ms`);

    // Benchmark: Text Embedding
    console.log(chalk.yellow('\n6. Text Embedding Benchmark'));
    const testTexts = [
      "The quick brown fox jumps over the lazy dog",
      "Machine learning models process natural language",
      "Vector databases enable semantic search",
    ];
    const embedStart = performance.now();
    for (let i = 0; i < 100; i++) {
      for (const text of testTexts) {
        hashEmbed(text, dims);
      }
    }
    const embedTime = (performance.now() - embedStart) / 300;
    console.log(`   ${chalk.green('✓')} Hash embedding: ${embedTime.toFixed(3)}ms per text`);
    console.log(`   ${chalk.dim(`  Throughput: ${(1000 / embedTime).toFixed(0)} embeddings/sec`)}`);
    console.log(chalk.dim('   Note: Hash embedding is fast but not semantic'));

    // Summary
    console.log(chalk.cyan('\n═══════════════════════════════════════════════════'));
    console.log(chalk.cyan('                    SUMMARY'));
    console.log(chalk.cyan('═══════════════════════════════════════════════════'));
    console.log(`  Insert Rate:     ${chalk.green((count / (insertTime / 1000)).toFixed(0))} vectors/sec`);
    console.log(`  Search Latency:  ${chalk.green(avgSearchTime.toFixed(3))}ms (avg)`);
    console.log(`  Search QPS:      ${chalk.green((100 / (searchTime2 / 1000)).toFixed(0))} queries/sec`);
    console.log(`  Save Latency:    ${chalk.green(saveTime.toFixed(2))}ms`);
    console.log(`  Load Latency:    ${chalk.green(loadTime.toFixed(2))}ms`);
    console.log(`  Storage:         ${chalk.green(formatBytes(fileSize))} for ${count} vectors`);
    console.log(chalk.cyan('═══════════════════════════════════════════════════\n'));

    // Cleanup
    fs.unlinkSync(options.path);
  });

program
  .command('export')
  .description('Export database to JSON file')
  .argument('<output>', 'Output file path')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .action(async (output, options) => {
    try {
      loadState(options.path);
      fs.writeFileSync(output, JSON.stringify(state, null, 2));
      console.log(chalk.green(`Exported to ${output}`));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('import')
  .description('Import database from JSON file')
  .argument('<input>', 'Input file path')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .action(async (input, options) => {
    try {
      state = JSON.parse(fs.readFileSync(input, 'utf-8'));
      rebuildNormCache();
      saveState(options.path);
      console.log(chalk.green(`Imported from ${input}`));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('repl')
  .description('Start interactive REPL')
  .option('-p, --path <path>', 'Database path', DEFAULT_DB_PATH)
  .action(async (options) => {
    loadState(options.path);

    console.log(chalk.cyan(`
╔═══════════════════════════════════════════════════╗
║  RvLite Interactive REPL v${VERSION}                 ║
║  Type .help for commands, .exit to quit           ║
╚═══════════════════════════════════════════════════╝
`));

    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout,
      prompt: chalk.green('rvlite> ')
    });

    rl.prompt();

    rl.on('line', async (line) => {
      const input = line.trim();

      if (input.startsWith('.')) {
        const [cmd, ...args] = input.slice(1).split(' ');
        switch (cmd) {
          case 'help':
            console.log(`
  ${chalk.yellow('.insert')} <vector> [metadata] - Insert a vector
  ${chalk.yellow('.search')} <vector> [k]        - Search for similar vectors
  ${chalk.yellow('.get')} <id>                   - Get vector by ID
  ${chalk.yellow('.delete')} <id>                - Delete vector
  ${chalk.yellow('.triple')} <s> <p> <o>         - Add RDF triple
  ${chalk.yellow('.triples')}                    - List all triples
  ${chalk.yellow('.stats')}                      - Show statistics
  ${chalk.yellow('.bench')} [n]                  - Quick benchmark (n vectors)
  ${chalk.yellow('.save')}                       - Save database
  ${chalk.yellow('.clear')}                      - Clear screen
  ${chalk.yellow('.exit')}                       - Exit REPL
`);
            break;
          case 'insert':
            if (args.length > 0) {
              try {
                const vector = JSON.parse(args[0]);
                const metadata = args[1] ? JSON.parse(args[1]) : null;
                const id = insertVector(vector, metadata);
                console.log(chalk.green(`Inserted: ${id}`));
              } catch (e) {
                console.error(chalk.red(`Error: ${e.message}`));
              }
            } else {
              console.log(chalk.dim('Usage: .insert [0.1,0.2,...] {"key":"value"}'));
            }
            break;
          case 'search':
            if (args.length > 0) {
              try {
                const vector = JSON.parse(args[0]);
                const k = args[1] ? parseInt(args[1]) : 5;
                const start = performance.now();
                const results = searchVectors(vector, k, state.config?.metric || 'cosine');
                const elapsed = performance.now() - start;
                console.log(JSON.stringify(results, null, 2));
                console.log(chalk.dim(`Search time: ${elapsed.toFixed(3)}ms`));
              } catch (e) {
                console.error(chalk.red(`Error: ${e.message}`));
              }
            } else {
              console.log(chalk.dim('Usage: .search [0.1,0.2,...] 5'));
            }
            break;
          case 'get':
            if (args[0] && state.vectors[args[0]]) {
              const entry = state.vectors[args[0]];
              console.log(JSON.stringify({ vector: entry.vector, metadata: entry.metadata }, null, 2));
            } else {
              console.error(chalk.red('Vector not found'));
            }
            break;
          case 'delete':
            if (args[0] && deleteVector(args[0])) {
              console.log(chalk.green('Deleted'));
            } else {
              console.error(chalk.red('Vector not found'));
            }
            break;
          case 'triple':
            if (args.length >= 3) {
              state.triples.push({ subject: args[0], predicate: args[1], object: args[2] });
              console.log(chalk.green('Triple added'));
            } else {
              console.log(chalk.dim('Usage: .triple <subject> <predicate> <object>'));
            }
            break;
          case 'triples':
            console.log(JSON.stringify(state.triples.slice(0, 20), null, 2));
            break;
          case 'stats':
            console.log(`Vectors: ${Object.keys(state.vectors).length}, Triples: ${state.triples.length}`);
            break;
          case 'bench':
            const n = args[0] ? parseInt(args[0]) : 100;
            const dims = state.config?.dimensions || 384;
            console.log(chalk.dim(`Quick benchmark: ${n} vectors, ${dims} dimensions`));
            const benchVecs = Array.from({ length: n }, () => ({
              vector: Array.from({ length: dims }, () => Math.random() * 2 - 1),
              metadata: null
            }));
            const t1 = performance.now();
            insertBatch(benchVecs);
            console.log(chalk.dim(`Insert: ${(performance.now() - t1).toFixed(2)}ms`));
            const q = Array.from({ length: dims }, () => Math.random() * 2 - 1);
            const t2 = performance.now();
            searchVectors(q, 10, state.config?.metric || 'cosine');
            console.log(chalk.dim(`Search: ${(performance.now() - t2).toFixed(3)}ms`));
            break;
          case 'save':
            saveState(options.path);
            console.log(chalk.green('Saved'));
            break;
          case 'clear':
            console.clear();
            break;
          case 'exit':
          case 'quit':
            saveState(options.path);
            console.log(chalk.dim('Goodbye!'));
            process.exit(0);
          default:
            console.log(chalk.red(`Unknown command: .${cmd}`));
        }
      } else if (input) {
        console.log(chalk.dim('Type .help for available commands'));
      }

      rl.prompt();
    });

    rl.on('close', async () => {
      saveState(options.path);
      console.log(chalk.dim('\nGoodbye!'));
      process.exit(0);
    });
  });

program
  .command('wasm')
  .description('Show info about WASM integration')
  .action(() => {
    console.log(chalk.cyan(`
RvLite WASM Integration
${'─'.repeat(40)}
For full WASM-powered features (SQL, Cypher, SPARQL), use the SDK:

  import { RvLite } from 'rvlite';

  const db = await RvLite.create({ dimensions: 384 });

  // Full SQL support
  await db.sql("SELECT * FROM vectors WHERE ...");

  // Cypher property graph
  await db.cypher("CREATE (n:Person {name: 'Alice'})");

  // SPARQL RDF queries
  await db.sparql("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");

The CLI provides optimized vector operations.
Full query language support requires the SDK.
`));
  });

// Utility function to format bytes
function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

// ============================================================================
// WASM MODULE LOADERS
// ============================================================================

let sonaModule = null;
let sonaEngine = null;
let attentionModule = null;

async function loadSonaWasm() {
  if (sonaModule) return sonaModule;
  try {
    const wasmPath = path.join(__dirname, '../dist/wasm/sona/ruvector_sona.js');
    if (!fs.existsSync(wasmPath)) {
      throw new Error('SONA WASM not found. Run: npm run build:wasm');
    }
    sonaModule = await import(wasmPath);
    await sonaModule.default();
    return sonaModule;
  } catch (e) {
    throw new Error(`Failed to load SONA WASM: ${e.message}`);
  }
}

async function loadAttentionWasm() {
  if (attentionModule) return attentionModule;
  try {
    const wasmPath = path.join(__dirname, '../dist/wasm/attention/ruvector_attention_wasm.js');
    if (!fs.existsSync(wasmPath)) {
      throw new Error('Attention WASM not found. Run: npm run build:wasm');
    }
    attentionModule = await import(wasmPath);
    await attentionModule.default();
    attentionModule.init();
    return attentionModule;
  } catch (e) {
    throw new Error(`Failed to load Attention WASM: ${e.message}`);
  }
}

async function getSonaEngine(hiddenDim = 384) {
  if (sonaEngine) return sonaEngine;
  const sona = await loadSonaWasm();
  sonaEngine = new sona.WasmSonaEngine(hiddenDim);
  return sonaEngine;
}

// ============================================================================
// SONA SELF-LEARNING COMMANDS
// ============================================================================

program
  .command('sona-init')
  .description('Initialize SONA self-learning engine')
  .option('-d, --dimensions <n>', 'Hidden dimensions', '384')
  .option('--config <json>', 'Custom SONA config as JSON')
  .action(async (options) => {
    const spinner = ora('Initializing SONA engine...').start();
    try {
      const sona = await loadSonaWasm();
      const dims = parseInt(options.dimensions);

      if (options.config) {
        const config = JSON.parse(options.config);
        sonaEngine = sona.WasmSonaEngine.withConfig(config);
      } else {
        sonaEngine = new sona.WasmSonaEngine(dims);
      }

      spinner.succeed('SONA engine initialized');
      console.log(chalk.cyan('  Hidden dimensions:'), dims);
      console.log(chalk.cyan('  Features:'), 'LoRA, EWC++, ReasoningBank');
      console.log(chalk.dim('  Use "rvlite sona-learn" to record trajectories'));
    } catch (err) {
      spinner.fail(`Error: ${err.message}`);
      process.exit(1);
    }
  });

program
  .command('sona-learn')
  .description('Record learning trajectory and apply feedback')
  .argument('<embedding>', 'Embedding vector as JSON array')
  .option('-q, --quality <n>', 'Quality score 0.0-1.0', '0.8')
  .option('-l, --latency <ms>', 'Latency in milliseconds', '50')
  .option('--success', 'Mark as successful', true)
  .action(async (embedding, options) => {
    try {
      const engine = await getSonaEngine();
      const vec = new Float32Array(JSON.parse(embedding));
      const quality = parseFloat(options.quality);
      const latency = parseFloat(options.latency);

      // Record trajectory
      const trajectoryId = engine.startTrajectory(vec);
      engine.recordStep(trajectoryId, 0, quality, BigInt(Math.floor(latency * 1000)));
      engine.endTrajectory(trajectoryId, quality);

      // Apply feedback
      engine.learnFromFeedback(options.success, latency, quality);

      console.log(chalk.green('Learning recorded'));
      console.log(chalk.dim(`  Trajectory ID: ${trajectoryId}`));
      console.log(chalk.dim(`  Quality: ${quality}`));
      console.log(chalk.dim(`  Latency: ${latency}ms`));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('sona-apply')
  .description('Apply LoRA transformation to vector')
  .argument('<embedding>', 'Input embedding as JSON array')
  .action(async (embedding, options) => {
    try {
      const engine = await getSonaEngine();
      const input = new Float32Array(JSON.parse(embedding));
      const output = engine.applyLora(input);
      console.log(JSON.stringify(Array.from(output)));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('sona-patterns')
  .description('Find similar patterns in ReasoningBank')
  .argument('<embedding>', 'Query embedding as JSON array')
  .option('-k, --top <n>', 'Number of patterns', '5')
  .action(async (embedding, options) => {
    try {
      const engine = await getSonaEngine();
      const query = new Float32Array(JSON.parse(embedding));
      const patterns = engine.findPatterns(query, parseInt(options.top));
      console.log(chalk.cyan('Similar Patterns:'));
      console.log(JSON.stringify(patterns, null, 2));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('sona-stats')
  .description('Show SONA engine statistics')
  .action(async () => {
    try {
      const engine = await getSonaEngine();
      const stats = engine.getStats();
      console.log(chalk.cyan('SONA Engine Statistics'));
      console.log(chalk.cyan('───────────────────────────────────────'));
      console.log(JSON.stringify(stats, null, 2));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('sona-force-learn')
  .description('Force a background learning cycle')
  .action(async () => {
    const spinner = ora('Running learning cycle...').start();
    try {
      const engine = await getSonaEngine();
      const result = engine.forceLearn();
      spinner.succeed('Learning cycle complete');
      console.log(chalk.dim(result));
    } catch (err) {
      spinner.fail(`Error: ${err.message}`);
      process.exit(1);
    }
  });

// ============================================================================
// FEDERATED LEARNING COMMANDS
// ============================================================================

let federatedCoordinator = null;
let ephemeralAgents = new Map();

program
  .command('federated-init')
  .description('Initialize federated learning coordinator')
  .option('-d, --dimensions <n>', 'Hidden dimensions', '384')
  .option('--threshold <n>', 'Quality threshold', '0.4')
  .action(async (options) => {
    const spinner = ora('Initializing federated coordinator...').start();
    try {
      const sona = await loadSonaWasm();
      federatedCoordinator = new sona.WasmFederatedCoordinator('coordinator-main');
      federatedCoordinator.setQualityThreshold(parseFloat(options.threshold));

      spinner.succeed('Federated coordinator initialized');
      console.log(chalk.cyan('  Quality threshold:'), options.threshold);
      console.log(chalk.dim('  Use "rvlite agent-spawn" to create agents'));
    } catch (err) {
      spinner.fail(`Error: ${err.message}`);
      process.exit(1);
    }
  });

program
  .command('agent-spawn')
  .description('Spawn ephemeral learning agent')
  .argument('<agent-id>', 'Unique agent identifier')
  .option('-d, --dimensions <n>', 'Hidden dimensions', '384')
  .action(async (agentId, options) => {
    try {
      const sona = await loadSonaWasm();
      const config = {
        hidden_dim: parseInt(options.dimensions),
        trajectory_capacity: 500,
        pattern_clusters: 25
      };
      const agent = sona.WasmEphemeralAgent.withConfig(agentId, config);
      ephemeralAgents.set(agentId, agent);

      console.log(chalk.green(`Agent spawned: ${agentId}`));
      console.log(chalk.dim(`  Hidden dim: ${options.dimensions}`));
      console.log(chalk.dim(`  Capacity: 500 trajectories`));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('agent-task')
  .description('Process task with agent')
  .argument('<agent-id>', 'Agent identifier')
  .argument('<embedding>', 'Task embedding as JSON array')
  .option('-q, --quality <n>', 'Quality score', '0.8')
  .option('-r, --route <model>', 'Model route (e.g., gpt-4, claude-3)')
  .action(async (agentId, embedding, options) => {
    try {
      const agent = ephemeralAgents.get(agentId);
      if (!agent) {
        throw new Error(`Agent not found: ${agentId}. Use "rvlite agent-spawn ${agentId}" first.`);
      }

      const vec = new Float32Array(JSON.parse(embedding));
      const quality = parseFloat(options.quality);

      if (options.route) {
        agent.processTaskWithRoute(vec, quality, options.route);
      } else {
        agent.processTask(vec, quality);
      }

      console.log(chalk.green(`Task processed by ${agentId}`));
      console.log(chalk.dim(`  Quality: ${quality}`));
      console.log(chalk.dim(`  Trajectories: ${agent.trajectoryCount()}`));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('agent-export')
  .description('Export agent state for aggregation')
  .argument('<agent-id>', 'Agent identifier')
  .action(async (agentId) => {
    try {
      const agent = ephemeralAgents.get(agentId);
      if (!agent) {
        throw new Error(`Agent not found: ${agentId}`);
      }

      const state = agent.exportState();
      console.log(JSON.stringify(state, null, 2));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('federated-aggregate')
  .description('Aggregate agent exports into coordinator')
  .argument('<agent-id>', 'Agent identifier to aggregate')
  .action(async (agentId) => {
    try {
      if (!federatedCoordinator) {
        throw new Error('Coordinator not initialized. Use "rvlite federated-init" first.');
      }

      const agent = ephemeralAgents.get(agentId);
      if (!agent) {
        throw new Error(`Agent not found: ${agentId}`);
      }

      const agentState = agent.exportState();
      const result = federatedCoordinator.aggregate(agentState);

      console.log(chalk.green(`Aggregated agent: ${agentId}`));
      console.log(chalk.dim(`  Accepted: ${result.accepted || 0}`));
      console.log(chalk.dim(`  Rejected: ${result.rejected || 0}`));
      console.log(chalk.dim(`  Total agents: ${federatedCoordinator.agentCount()}`));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('federated-consolidate')
  .description('Consolidate learning from all agents')
  .action(async () => {
    const spinner = ora('Consolidating federated learning...').start();
    try {
      if (!federatedCoordinator) {
        throw new Error('Coordinator not initialized');
      }

      const result = federatedCoordinator.consolidate();
      spinner.succeed('Consolidation complete');
      console.log(chalk.dim(result));
      console.log(chalk.cyan(`  Total trajectories: ${federatedCoordinator.totalTrajectories()}`));
    } catch (err) {
      spinner.fail(`Error: ${err.message}`);
      process.exit(1);
    }
  });

program
  .command('federated-stats')
  .description('Show federated learning statistics')
  .action(async () => {
    try {
      if (!federatedCoordinator) {
        throw new Error('Coordinator not initialized');
      }

      const stats = federatedCoordinator.getStats();
      console.log(chalk.cyan('Federated Learning Statistics'));
      console.log(chalk.cyan('───────────────────────────────────────'));
      console.log(JSON.stringify(stats, null, 2));
      console.log(chalk.dim(`\nActive agents: ${ephemeralAgents.size}`));
      for (const [id, agent] of ephemeralAgents) {
        console.log(chalk.dim(`  ${id}: ${agent.trajectoryCount()} trajectories`));
      }
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

// ============================================================================
// ATTENTION MECHANISM COMMANDS
// ============================================================================

program
  .command('attention')
  .description('Compute attention between query and key-value pairs')
  .argument('<query>', 'Query vector as JSON array')
  .argument('<keys>', 'Key vectors as JSON array of arrays')
  .argument('<values>', 'Value vectors as JSON array of arrays')
  .option('-t, --type <type>', 'Attention type: scaled, multi-head, hyperbolic, flash, linear, moe', 'scaled')
  .option('-h, --heads <n>', 'Number of heads (multi-head)', '8')
  .option('-c, --curvature <n>', 'Curvature (hyperbolic)', '1.0')
  .option('-b, --block-size <n>', 'Block size (flash)', '64')
  .option('-e, --experts <n>', 'Number of experts (moe)', '4')
  .option('-k, --top-k <n>', 'Top-k experts (moe)', '2')
  .action(async (query, keys, values, options) => {
    try {
      const attn = await loadAttentionWasm();
      const q = new Float32Array(JSON.parse(query));
      const k = JSON.parse(keys);
      const v = JSON.parse(values);

      let result;
      const dim = q.length;

      switch (options.type) {
        case 'scaled':
          result = attn.scaled_dot_attention(q, k, v);
          break;
        case 'multi-head': {
          const mha = new attn.WasmMultiHeadAttention(dim, parseInt(options.heads));
          result = mha.compute(q, k, v);
          mha.free();
          break;
        }
        case 'hyperbolic': {
          const hyp = new attn.WasmHyperbolicAttention(dim, parseFloat(options.curvature));
          result = hyp.compute(q, k, v);
          hyp.free();
          break;
        }
        case 'flash': {
          const flash = new attn.WasmFlashAttention(dim, parseInt(options.blockSize));
          result = flash.compute(q, k, v);
          flash.free();
          break;
        }
        case 'linear': {
          const linear = new attn.WasmLinearAttention(dim, dim);
          result = linear.compute(q, k, v);
          linear.free();
          break;
        }
        case 'moe': {
          const moe = new attn.WasmMoEAttention(dim, parseInt(options.experts), parseInt(options.topK));
          result = moe.compute(q, k, v);
          moe.free();
          break;
        }
        default:
          throw new Error(`Unknown attention type: ${options.type}`);
      }

      console.log(chalk.cyan(`Attention (${options.type}):`));
      console.log(JSON.stringify(Array.from(result)));
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('attention-train')
  .description('Train with InfoNCE contrastive loss')
  .argument('<anchor>', 'Anchor embedding')
  .argument('<positive>', 'Positive example')
  .argument('<negatives>', 'Negative examples as JSON array of arrays')
  .option('-t, --temperature <n>', 'Temperature', '0.07')
  .option('-l, --lr <n>', 'Learning rate', '0.001')
  .action(async (anchor, positive, negatives, options) => {
    try {
      const attn = await loadAttentionWasm();
      const a = new Float32Array(JSON.parse(anchor));
      const p = new Float32Array(JSON.parse(positive));
      const n = JSON.parse(negatives);

      const loss = new attn.WasmInfoNCELoss(parseFloat(options.temperature));
      const lossValue = loss.compute(a, p, n);

      console.log(chalk.cyan('InfoNCE Contrastive Loss'));
      console.log(chalk.cyan('───────────────────────────────────────'));
      console.log(`  Loss: ${lossValue.toFixed(6)}`);
      console.log(chalk.dim(`  Temperature: ${options.temperature}`));
      console.log(chalk.dim(`  Negatives: ${n.length}`));

      loss.free();
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

program
  .command('optimizer')
  .description('Create and test optimizer')
  .option('-t, --type <type>', 'Optimizer: adam, adamw, sgd', 'adam')
  .option('-l, --lr <n>', 'Learning rate', '0.001')
  .option('-w, --weight-decay <n>', 'Weight decay (adamw)', '0.01')
  .option('-m, --momentum <n>', 'Momentum (sgd)', '0.9')
  .option('-p, --params <n>', 'Number of parameters', '384')
  .action(async (options) => {
    try {
      const attn = await loadAttentionWasm();
      const paramCount = parseInt(options.params);
      const lr = parseFloat(options.lr);

      let optimizer;
      switch (options.type) {
        case 'adam':
          optimizer = new attn.WasmAdam(paramCount, lr);
          break;
        case 'adamw':
          optimizer = new attn.WasmAdamW(paramCount, lr, parseFloat(options.weightDecay));
          break;
        case 'sgd':
          optimizer = new attn.WasmSGD(paramCount, lr, parseFloat(options.momentum));
          break;
        default:
          throw new Error(`Unknown optimizer: ${options.type}`);
      }

      // Demo optimization step
      const params = new Float32Array(paramCount).fill(1.0);
      const grads = new Float32Array(paramCount).fill(0.1);

      console.log(chalk.cyan(`Optimizer: ${options.type.toUpperCase()}`));
      console.log(chalk.dim(`  Parameters: ${paramCount}`));
      console.log(chalk.dim(`  Learning rate: ${lr}`));

      const before = params[0];
      optimizer.step(params, grads);
      const after = params[0];

      console.log(chalk.green('  Step applied'));
      console.log(chalk.dim(`  param[0]: ${before.toFixed(6)} → ${after.toFixed(6)}`));

      optimizer.free();
    } catch (err) {
      console.error(chalk.red(`Error: ${err.message}`));
      process.exit(1);
    }
  });

// ============================================================================
// WASM UTILITIES
// ============================================================================

program
  .command('wasm-info')
  .description('Show available WASM modules and their capabilities')
  .action(async () => {
    console.log(chalk.cyan('\nRvLite WASM Modules'));
    console.log(chalk.cyan('═══════════════════════════════════════════════════════════'));

    // Check rvlite WASM
    const rvlitePath = path.join(__dirname, '../dist/wasm/rvlite_bg.wasm');
    if (fs.existsSync(rvlitePath)) {
      const size = fs.statSync(rvlitePath).size;
      console.log(chalk.green('\n✓ rvlite') + chalk.dim(` (${formatBytes(size)})`));
      console.log('  Vector DB, SQL, SPARQL, Cypher, IndexedDB persistence');
    } else {
      console.log(chalk.red('\n✗ rvlite') + chalk.dim(' (not found)'));
    }

    // Check SONA WASM
    const sonaPath = path.join(__dirname, '../dist/wasm/sona/ruvector_sona_bg.wasm');
    if (fs.existsSync(sonaPath)) {
      const size = fs.statSync(sonaPath).size;
      console.log(chalk.green('\n✓ SONA') + chalk.dim(` (${formatBytes(size)})`));
      console.log('  Self-learning: LoRA, EWC++, ReasoningBank');
      console.log('  Federated: EphemeralAgent, FederatedCoordinator');
      console.log('  Commands: sona-init, sona-learn, sona-apply, sona-patterns');
    } else {
      console.log(chalk.red('\n✗ SONA') + chalk.dim(' (not found)'));
    }

    // Check Attention WASM
    const attnPath = path.join(__dirname, '../dist/wasm/attention/ruvector_attention_wasm_bg.wasm');
    if (fs.existsSync(attnPath)) {
      const size = fs.statSync(attnPath).size;
      console.log(chalk.green('\n✓ Attention') + chalk.dim(` (${formatBytes(size)})`));
      console.log('  Mechanisms: MultiHead, Hyperbolic, Flash, Linear, MoE');
      console.log('  Training: Adam, AdamW, SGD, LR Scheduler, InfoNCE Loss');
      console.log('  Commands: attention, attention-train, optimizer');
    } else {
      console.log(chalk.red('\n✗ Attention') + chalk.dim(' (not found)'));
    }

    console.log(chalk.cyan('\n═══════════════════════════════════════════════════════════'));

    // Try to get versions
    try {
      const attn = await loadAttentionWasm();
      console.log(chalk.dim(`Attention version: ${attn.version()}`));
    } catch (e) {}
  });

program
  .command('benchmark-wasm')
  .description('Benchmark WASM modules')
  .option('-n, --count <n>', 'Number of iterations', '100')
  .option('-d, --dimensions <n>', 'Vector dimensions', '384')
  .action(async (options) => {
    const count = parseInt(options.count);
    const dims = parseInt(options.dimensions);

    console.log(chalk.cyan('\nRvLite WASM Benchmark'));
    console.log(chalk.cyan('═══════════════════════════════════════════════════════════'));
    console.log(`  Iterations: ${count}, Dimensions: ${dims}`);

    // Benchmark SONA
    try {
      const sona = await loadSonaWasm();
      console.log(chalk.yellow('\n1. SONA Self-Learning'));

      const engine = new sona.WasmSonaEngine(dims);
      const vec = new Float32Array(dims).fill(0.1);

      // Benchmark LoRA apply
      const loraStart = performance.now();
      for (let i = 0; i < count; i++) {
        engine.applyLora(vec);
      }
      const loraTime = (performance.now() - loraStart) / count;
      console.log(`   LoRA apply: ${loraTime.toFixed(3)}ms (${(1000/loraTime).toFixed(0)}/sec)`);

      // Benchmark trajectory recording
      const trajStart = performance.now();
      for (let i = 0; i < count; i++) {
        const tid = engine.startTrajectory(vec);
        engine.recordStep(tid, 0, 0.8, BigInt(1000));
        engine.endTrajectory(tid, 0.8);
      }
      const trajTime = (performance.now() - trajStart) / count;
      console.log(`   Trajectory: ${trajTime.toFixed(3)}ms (${(1000/trajTime).toFixed(0)}/sec)`);

      // Force learn
      const learnStart = performance.now();
      engine.forceLearn();
      const learnTime = performance.now() - learnStart;
      console.log(`   Force learn: ${learnTime.toFixed(2)}ms`);

    } catch (e) {
      console.log(chalk.red(`   SONA: ${e.message}`));
    }

    // Benchmark Attention
    try {
      const attn = await loadAttentionWasm();
      console.log(chalk.yellow('\n2. Attention Mechanisms'));

      const q = new Float32Array(dims).fill(0.1);
      const k = [Array(dims).fill(0.2), Array(dims).fill(0.3), Array(dims).fill(0.4)];
      const v = [Array(dims).fill(1.0), Array(dims).fill(2.0), Array(dims).fill(3.0)];

      // Benchmark scaled dot attention
      const sdaStart = performance.now();
      for (let i = 0; i < count; i++) {
        attn.scaled_dot_attention(q, k, v);
      }
      const sdaTime = (performance.now() - sdaStart) / count;
      console.log(`   Scaled dot: ${sdaTime.toFixed(3)}ms (${(1000/sdaTime).toFixed(0)}/sec)`);

      // Multi-head attention
      const mha = new attn.WasmMultiHeadAttention(dims, 8);
      const mhaStart = performance.now();
      for (let i = 0; i < count; i++) {
        mha.compute(q, k, v);
      }
      const mhaTime = (performance.now() - mhaStart) / count;
      console.log(`   Multi-head: ${mhaTime.toFixed(3)}ms (${(1000/mhaTime).toFixed(0)}/sec)`);
      mha.free();

      // Hyperbolic attention
      const hyp = new attn.WasmHyperbolicAttention(dims, 1.0);
      const hypStart = performance.now();
      for (let i = 0; i < count; i++) {
        hyp.compute(q, k, v);
      }
      const hypTime = (performance.now() - hypStart) / count;
      console.log(`   Hyperbolic: ${hypTime.toFixed(3)}ms (${(1000/hypTime).toFixed(0)}/sec)`);
      hyp.free();

      // MoE attention
      const moe = new attn.WasmMoEAttention(dims, 4, 2);
      const moeStart = performance.now();
      for (let i = 0; i < count; i++) {
        moe.compute(q, k, v);
      }
      const moeTime = (performance.now() - moeStart) / count;
      console.log(`   MoE (4 experts): ${moeTime.toFixed(3)}ms (${(1000/moeTime).toFixed(0)}/sec)`);
      moe.free();

    } catch (e) {
      console.log(chalk.red(`   Attention: ${e.message}`));
    }

    // Benchmark optimizers
    try {
      const attn = await loadAttentionWasm();
      console.log(chalk.yellow('\n3. Optimizers'));

      const params = new Float32Array(dims).fill(1.0);
      const grads = new Float32Array(dims).fill(0.01);

      const adam = new attn.WasmAdam(dims, 0.001);
      const adamStart = performance.now();
      for (let i = 0; i < count; i++) {
        adam.step(params, grads);
      }
      const adamTime = (performance.now() - adamStart) / count;
      console.log(`   Adam step: ${adamTime.toFixed(3)}ms (${(1000/adamTime).toFixed(0)}/sec)`);
      adam.free();

      const sgd = new attn.WasmSGD(dims, 0.01, 0.9);
      params.fill(1.0);
      const sgdStart = performance.now();
      for (let i = 0; i < count; i++) {
        sgd.step(params, grads);
      }
      const sgdTime = (performance.now() - sgdStart) / count;
      console.log(`   SGD step: ${sgdTime.toFixed(3)}ms (${(1000/sgdTime).toFixed(0)}/sec)`);
      sgd.free();

    } catch (e) {
      console.log(chalk.red(`   Optimizers: ${e.message}`));
    }

    console.log(chalk.cyan('\n═══════════════════════════════════════════════════════════'));
  });

program.parse();
