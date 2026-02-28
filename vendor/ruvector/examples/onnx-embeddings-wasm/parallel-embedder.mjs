/**
 * Parallel ONNX Embedder using Worker Threads
 *
 * Distributes embedding work across multiple CPU cores for true parallelism.
 */
import { Worker } from 'worker_threads';
import { cpus } from 'os';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { ModelLoader, DEFAULT_MODEL } from './loader.js';

const __dirname = dirname(fileURLToPath(import.meta.url));

export class ParallelEmbedder {
    constructor(options = {}) {
        this.numWorkers = options.numWorkers || Math.max(1, cpus().length - 1);
        this.workers = [];
        this.readyWorkers = [];
        this.pendingRequests = new Map();
        this.requestId = 0;
        this.initialized = false;
        this.modelData = null;
    }

    /**
     * Initialize the parallel embedder with a model
     */
    async init(modelName = DEFAULT_MODEL) {
        if (this.initialized) return;

        console.log(`🚀 Initializing ${this.numWorkers} worker threads...`);

        // Load model once in main thread
        const loader = new ModelLoader({ cache: false });
        const { modelBytes, tokenizerJson, config } = await loader.loadModel(modelName);

        // Store as transferable data
        this.modelData = {
            modelBytes: Array.from(modelBytes), // Convert to regular array for transfer
            tokenizerJson,
            config
        };
        this.dimension = config.dimension;

        // Spawn workers
        const workerPromises = [];
        for (let i = 0; i < this.numWorkers; i++) {
            workerPromises.push(this._spawnWorker(i));
        }
        await Promise.all(workerPromises);

        this.initialized = true;
        console.log(`✅ ${this.numWorkers} workers ready`);
    }

    async _spawnWorker(index) {
        return new Promise((resolve, reject) => {
            const worker = new Worker(join(__dirname, 'parallel-worker.mjs'), {
                workerData: this.modelData
            });

            worker.on('message', (msg) => {
                if (msg.type === 'ready') {
                    this.readyWorkers.push(worker);
                    resolve();
                } else if (msg.type === 'result') {
                    const { id, embeddings } = msg;
                    const pending = this.pendingRequests.get(id);
                    if (pending) {
                        pending.resolve(embeddings);
                        this.pendingRequests.delete(id);
                        this.readyWorkers.push(worker);
                        this._processQueue();
                    }
                } else if (msg.type === 'error') {
                    const { id, error } = msg;
                    const pending = this.pendingRequests.get(id);
                    if (pending) {
                        pending.reject(new Error(error));
                        this.pendingRequests.delete(id);
                        this.readyWorkers.push(worker);
                        this._processQueue();
                    }
                }
            });

            worker.on('error', reject);
            this.workers.push(worker);
        });
    }

    _processQueue() {
        // Process any queued requests when workers become available
    }

    /**
     * Embed texts in parallel across worker threads
     */
    async embedBatch(texts) {
        if (!this.initialized) {
            throw new Error('ParallelEmbedder not initialized. Call init() first.');
        }

        // Split texts into chunks for each worker
        const chunkSize = Math.ceil(texts.length / this.numWorkers);
        const chunks = [];
        for (let i = 0; i < texts.length; i += chunkSize) {
            chunks.push(texts.slice(i, i + chunkSize));
        }

        // Send to workers in parallel
        const promises = chunks.map((chunk, i) => {
            return new Promise((resolve, reject) => {
                const id = this.requestId++;
                const worker = this.readyWorkers.shift() || this.workers[i % this.workers.length];

                this.pendingRequests.set(id, { resolve, reject });
                worker.postMessage({ type: 'embed', id, texts: chunk });
            });
        });

        // Wait for all results
        const results = await Promise.all(promises);

        // Flatten results
        return results.flat();
    }

    /**
     * Embed a single text (uses one worker)
     */
    async embedOne(text) {
        const results = await this.embedBatch([text]);
        return new Float32Array(results[0]);
    }

    /**
     * Compute similarity between two texts
     */
    async similarity(text1, text2) {
        const [emb1, emb2] = await this.embedBatch([text1, text2]);
        return this._cosineSimilarity(emb1, emb2);
    }

    _cosineSimilarity(a, b) {
        let dot = 0, normA = 0, normB = 0;
        for (let i = 0; i < a.length; i++) {
            dot += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        return dot / (Math.sqrt(normA) * Math.sqrt(normB));
    }

    /**
     * Shutdown all workers
     */
    async shutdown() {
        for (const worker of this.workers) {
            worker.postMessage({ type: 'shutdown' });
        }
        this.workers = [];
        this.readyWorkers = [];
        this.initialized = false;
    }
}

export default ParallelEmbedder;
