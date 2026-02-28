/**
 * Web Worker for parallel vector search operations
 *
 * This worker handles:
 * - Vector search operations in parallel
 * - Batch insert operations
 * - Zero-copy transfers via transferable objects
 */

// Import the WASM module
let wasmModule = null;
let vectorDB = null;

/**
 * Initialize the worker with WASM module
 */
self.onmessage = async function(e) {
  const { type, data } = e.data;

  try {
    switch (type) {
      case 'init':
        await initWorker(data);
        self.postMessage({ type: 'init', success: true });
        break;

      case 'insert':
        await handleInsert(data);
        break;

      case 'insertBatch':
        await handleInsertBatch(data);
        break;

      case 'search':
        await handleSearch(data);
        break;

      case 'delete':
        await handleDelete(data);
        break;

      case 'get':
        await handleGet(data);
        break;

      case 'len':
        const length = vectorDB.len();
        self.postMessage({ type: 'len', data: length });
        break;

      default:
        throw new Error(`Unknown message type: ${type}`);
    }
  } catch (error) {
    self.postMessage({
      type: 'error',
      error: {
        message: error.message,
        stack: error.stack
      }
    });
  }
};

/**
 * Initialize WASM module and VectorDB
 */
async function initWorker(config) {
  const { wasmUrl, dimensions, metric, useHnsw } = config;

  // Import WASM module
  wasmModule = await import(wasmUrl);

  // Initialize WASM
  await wasmModule.default();

  // Create VectorDB instance
  vectorDB = new wasmModule.VectorDB(dimensions, metric, useHnsw);

  console.log(`Worker initialized with dimensions=${dimensions}, metric=${metric}, SIMD=${wasmModule.detectSIMD()}`);
}

/**
 * Handle single vector insert
 */
async function handleInsert(data) {
  const { vector, id, metadata, requestId } = data;

  // Convert array to Float32Array if needed
  const vectorArray = new Float32Array(vector);

  const resultId = vectorDB.insert(vectorArray, id, metadata);

  self.postMessage({
    type: 'insert',
    requestId,
    data: resultId
  });
}

/**
 * Handle batch insert
 */
async function handleInsertBatch(data) {
  const { entries, requestId } = data;

  // Convert vectors to Float32Array
  const processedEntries = entries.map(entry => ({
    vector: new Float32Array(entry.vector),
    id: entry.id,
    metadata: entry.metadata
  }));

  const ids = vectorDB.insertBatch(processedEntries);

  self.postMessage({
    type: 'insertBatch',
    requestId,
    data: ids
  });
}

/**
 * Handle vector search
 */
async function handleSearch(data) {
  const { query, k, filter, requestId } = data;

  // Convert query to Float32Array
  const queryArray = new Float32Array(query);

  const results = vectorDB.search(queryArray, k, filter);

  // Convert results to plain objects
  const plainResults = results.map(result => ({
    id: result.id,
    score: result.score,
    vector: result.vector ? Array.from(result.vector) : null,
    metadata: result.metadata
  }));

  self.postMessage({
    type: 'search',
    requestId,
    data: plainResults
  });
}

/**
 * Handle delete operation
 */
async function handleDelete(data) {
  const { id, requestId } = data;

  const deleted = vectorDB.delete(id);

  self.postMessage({
    type: 'delete',
    requestId,
    data: deleted
  });
}

/**
 * Handle get operation
 */
async function handleGet(data) {
  const { id, requestId } = data;

  const entry = vectorDB.get(id);

  const plainEntry = entry ? {
    id: entry.id,
    vector: Array.from(entry.vector),
    metadata: entry.metadata
  } : null;

  self.postMessage({
    type: 'get',
    requestId,
    data: plainEntry
  });
}
