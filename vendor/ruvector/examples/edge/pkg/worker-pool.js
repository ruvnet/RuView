/**
 * Web Worker Pool Manager
 *
 * Manages a pool of workers for parallel vector operations.
 * Supports:
 * - Round-robin task distribution
 * - Load balancing
 * - Automatic worker initialization
 * - Promise-based API
 */

export class WorkerPool {
  constructor(workerUrl, wasmUrl, options = {}) {
    this.workerUrl = workerUrl;
    this.wasmUrl = wasmUrl;
    this.poolSize = options.poolSize || navigator.hardwareConcurrency || 4;
    this.workers = [];
    this.nextWorker = 0;
    this.pendingRequests = new Map();
    this.requestId = 0;
    this.initialized = false;
    this.options = options;
  }

  /**
   * Initialize the worker pool
   */
  async init() {
    if (this.initialized) return;

    console.log(`Initializing worker pool with ${this.poolSize} workers...`);

    const initPromises = [];

    for (let i = 0; i < this.poolSize; i++) {
      const worker = new Worker(this.workerUrl, { type: 'module' });

      worker.onmessage = (e) => this.handleMessage(i, e);
      worker.onerror = (error) => this.handleError(i, error);

      this.workers.push({
        worker,
        busy: false,
        id: i
      });

      // Initialize worker with WASM
      const initPromise = this.sendToWorker(i, 'init', {
        wasmUrl: this.wasmUrl,
        dimensions: this.options.dimensions,
        metric: this.options.metric,
        useHnsw: this.options.useHnsw
      });

      initPromises.push(initPromise);
    }

    await Promise.all(initPromises);
    this.initialized = true;

    console.log(`Worker pool initialized successfully`);
  }

  /**
   * Handle message from worker
   */
  handleMessage(workerId, event) {
    const { type, requestId, data, error } = event.data;

    if (type === 'error') {
      const request = this.pendingRequests.get(requestId);
      if (request) {
        request.reject(new Error(error.message));
        this.pendingRequests.delete(requestId);
      }
      return;
    }

    const request = this.pendingRequests.get(requestId);
    if (request) {
      this.workers[workerId].busy = false;
      request.resolve(data);
      this.pendingRequests.delete(requestId);
    }
  }

  /**
   * Handle worker error
   */
  handleError(workerId, error) {
    console.error(`Worker ${workerId} error:`, error);

    // Reject all pending requests for this worker
    for (const [requestId, request] of this.pendingRequests) {
      if (request.workerId === workerId) {
        request.reject(error);
        this.pendingRequests.delete(requestId);
      }
    }
  }

  /**
   * Get next available worker (round-robin)
   */
  getNextWorker() {
    // Try to find an idle worker
    for (let i = 0; i < this.workers.length; i++) {
      const idx = (this.nextWorker + i) % this.workers.length;
      if (!this.workers[idx].busy) {
        this.nextWorker = (idx + 1) % this.workers.length;
        return idx;
      }
    }

    // All busy, use round-robin
    const idx = this.nextWorker;
    this.nextWorker = (this.nextWorker + 1) % this.workers.length;
    return idx;
  }

  /**
   * Send message to specific worker
   */
  sendToWorker(workerId, type, data) {
    return new Promise((resolve, reject) => {
      const requestId = this.requestId++;

      this.pendingRequests.set(requestId, {
        resolve,
        reject,
        workerId,
        timestamp: Date.now()
      });

      this.workers[workerId].busy = true;
      this.workers[workerId].worker.postMessage({
        type,
        data: { ...data, requestId }
      });

      // Timeout after 30 seconds
      setTimeout(() => {
        if (this.pendingRequests.has(requestId)) {
          this.pendingRequests.delete(requestId);
          reject(new Error('Request timeout'));
        }
      }, 30000);
    });
  }

  /**
   * Execute operation on next available worker
   */
  async execute(type, data) {
    if (!this.initialized) {
      await this.init();
    }

    const workerId = this.getNextWorker();
    return this.sendToWorker(workerId, type, data);
  }

  /**
   * Insert vector
   */
  async insert(vector, id = null, metadata = null) {
    return this.execute('insert', { vector, id, metadata });
  }

  /**
   * Insert batch of vectors
   */
  async insertBatch(entries) {
    // Distribute batch across workers
    const chunkSize = Math.ceil(entries.length / this.poolSize);
    const chunks = [];

    for (let i = 0; i < entries.length; i += chunkSize) {
      chunks.push(entries.slice(i, i + chunkSize));
    }

    const promises = chunks.map((chunk, i) =>
      this.sendToWorker(i % this.poolSize, 'insertBatch', { entries: chunk })
    );

    const results = await Promise.all(promises);
    return results.flat();
  }

  /**
   * Search for similar vectors
   */
  async search(query, k = 10, filter = null) {
    return this.execute('search', { query, k, filter });
  }

  /**
   * Parallel search across multiple queries
   */
  async searchBatch(queries, k = 10, filter = null) {
    const promises = queries.map((query, i) =>
      this.sendToWorker(i % this.poolSize, 'search', { query, k, filter })
    );

    return Promise.all(promises);
  }

  /**
   * Delete vector
   */
  async delete(id) {
    return this.execute('delete', { id });
  }

  /**
   * Get vector by ID
   */
  async get(id) {
    return this.execute('get', { id });
  }

  /**
   * Get database length (from first worker)
   */
  async len() {
    return this.sendToWorker(0, 'len', {});
  }

  /**
   * Terminate all workers
   */
  terminate() {
    for (const { worker } of this.workers) {
      worker.terminate();
    }
    this.workers = [];
    this.initialized = false;
    console.log('Worker pool terminated');
  }

  /**
   * Get pool statistics
   */
  getStats() {
    return {
      poolSize: this.poolSize,
      busyWorkers: this.workers.filter(w => w.busy).length,
      idleWorkers: this.workers.filter(w => !w.busy).length,
      pendingRequests: this.pendingRequests.size
    };
  }
}

export default WorkerPool;
