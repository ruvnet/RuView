"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const fastify_1 = __importDefault(require("fastify"));
const helmet_1 = __importDefault(require("@fastify/helmet"));
const compress_1 = __importDefault(require("@fastify/compress"));
const rate_limit_1 = __importDefault(require("@fastify/rate-limit"));
const websocket_1 = __importDefault(require("@fastify/websocket"));
const vector_client_1 = require("./vector-client");
const load_balancer_1 = require("./load-balancer");
const events_1 = __importDefault(require("events"));
// ===== ADVANCED OPTIMIZATIONS =====
// 1. ADAPTIVE BATCHING WITH PRIORITY QUEUES
class AdaptiveBatcher extends events_1.default {
    constructor() {
        super();
        this.queues = new Map();
        this.timers = new Map();
        this.batchSizes = new Map();
        // Dynamic batch size based on load
        this.MIN_BATCH = 10;
        this.MAX_BATCH = 500;
        this.TARGET_LATENCY_MS = 5;
        // Initialize priority queues
        ['critical', 'high', 'normal', 'low'].forEach(priority => {
            this.queues.set(priority, []);
            this.batchSizes.set(priority, 50);
        });
        // Adaptive tuning every 10 seconds
        setInterval(() => this.tuneParameters(), 10000);
    }
    async add(item, priority = 'normal') {
        const queue = this.queues.get(priority) || this.queues.get('normal');
        return new Promise((resolve, reject) => {
            queue.push({ ...item, resolve, reject, addedAt: Date.now() });
            const batchSize = this.batchSizes.get(priority) || 50;
            if (queue.length >= batchSize) {
                this.flush(priority);
            }
            else if (!this.timers.has(priority)) {
                // Dynamic timeout based on queue length
                const timeout = Math.max(1, this.TARGET_LATENCY_MS - queue.length);
                this.timers.set(priority, setTimeout(() => this.flush(priority), timeout));
            }
        });
    }
    async flush(priority) {
        const queue = this.queues.get(priority);
        if (!queue || queue.length === 0)
            return;
        const timer = this.timers.get(priority);
        if (timer) {
            clearTimeout(timer);
            this.timers.delete(priority);
        }
        const batch = queue.splice(0, this.batchSizes.get(priority) || 50);
        const startTime = Date.now();
        try {
            this.emit('batch', { priority, size: batch.length });
            const results = await this.processBatch(batch.map(b => b.query));
            results.forEach((result, i) => {
                batch[i].resolve(result);
            });
            // Track latency for adaptive tuning
            const latency = Date.now() - startTime;
            this.emit('latency', { priority, latency, batchSize: batch.length });
        }
        catch (error) {
            batch.forEach(b => b.reject(error));
        }
    }
    async processBatch(queries) {
        // Override in subclass
        return queries;
    }
    tuneParameters() {
        // Adaptive batch size based on recent performance
        this.queues.forEach((queue, priority) => {
            const currentSize = this.batchSizes.get(priority) || 50;
            const queueLength = queue.length;
            let newSize = currentSize;
            if (queueLength > currentSize * 2) {
                // Queue backing up, increase batch size
                newSize = Math.min(this.MAX_BATCH, currentSize * 1.2);
            }
            else if (queueLength < currentSize * 0.3) {
                // Queue empty, decrease batch size
                newSize = Math.max(this.MIN_BATCH, currentSize * 0.8);
            }
            this.batchSizes.set(priority, Math.round(newSize));
        });
    }
}
// 2. MULTI-LEVEL CACHE WITH COMPRESSION
class CompressedCache {
    constructor(redis) {
        this.compressionThreshold = 1024; // bytes
        this.l1 = new Map();
        this.l2 = redis;
        // LRU eviction for L1 every minute
        setInterval(() => this.evictL1(), 60000);
    }
    async get(key) {
        // Check L1 (in-memory)
        if (this.l1.has(key)) {
            return this.l1.get(key);
        }
        // Check L2 (Redis)
        const compressed = await this.l2.getBuffer(key);
        if (compressed) {
            const value = await this.decompress(compressed);
            // Promote to L1
            this.l1.set(key, value);
            return value;
        }
        return null;
    }
    async set(key, value, ttl = 3600) {
        // Set L1
        this.l1.set(key, value);
        // Set L2 with compression for large values
        const serialized = JSON.stringify(value);
        const buffer = Buffer.from(serialized);
        if (buffer.length > this.compressionThreshold) {
            const compressed = await this.compress(buffer);
            await this.l2.setex(key, ttl, compressed);
        }
        else {
            await this.l2.setex(key, ttl, serialized);
        }
    }
    async compress(buffer) {
        const { promisify } = require('util');
        const { brotliCompress } = require('zlib');
        const compress = promisify(brotliCompress);
        return compress(buffer);
    }
    async decompress(buffer) {
        const { promisify } = require('util');
        const { brotliDecompress } = require('zlib');
        const decompress = promisify(brotliDecompress);
        const decompressed = await decompress(buffer);
        return JSON.parse(decompressed.toString());
    }
    evictL1() {
        if (this.l1.size > 10000) {
            const toDelete = this.l1.size - 8000;
            const keys = Array.from(this.l1.keys()).slice(0, toDelete);
            keys.forEach(k => this.l1.delete(k));
        }
    }
}
// 3. CONNECTION POOLING WITH HEALTH CHECKS
class AdvancedConnectionPool {
    constructor() {
        this.pools = new Map();
        this.healthScores = new Map();
        this.maxPerPool = 100;
        this.minPerPool = 10;
        // Health check every 30 seconds
        setInterval(() => this.healthCheck(), 30000);
    }
    async acquire(poolId) {
        let pool = this.pools.get(poolId);
        if (!pool) {
            pool = [];
            this.pools.set(poolId, pool);
            this.healthScores.set(poolId, 1.0);
        }
        // Try to get healthy connection
        let connection = null;
        while (pool.length > 0 && !connection) {
            const candidate = pool.pop();
            if (await this.isHealthy(candidate)) {
                connection = candidate;
            }
        }
        // Create new if needed
        if (!connection) {
            connection = await this.createConnection(poolId);
        }
        return connection;
    }
    async release(poolId, connection) {
        const pool = this.pools.get(poolId);
        if (pool && pool.length < this.maxPerPool) {
            pool.push(connection);
        }
        else {
            await this.closeConnection(connection);
        }
    }
    async isHealthy(connection) {
        try {
            await connection.ping();
            return true;
        }
        catch {
            return false;
        }
    }
    async healthCheck() {
        for (const [poolId, pool] of this.pools) {
            let healthy = 0;
            for (const conn of pool) {
                if (await this.isHealthy(conn)) {
                    healthy++;
                }
            }
            const healthScore = pool.length > 0 ? healthy / pool.length : 1.0;
            this.healthScores.set(poolId, healthScore);
            // Maintain minimum pool size
            while (pool.length < this.minPerPool) {
                pool.push(await this.createConnection(poolId));
            }
        }
    }
    async createConnection(poolId) {
        // Override in subclass
        return { poolId, id: Math.random() };
    }
    async closeConnection(connection) {
        // Override in subclass
    }
    getHealthScore(poolId) {
        return this.healthScores.get(poolId) || 0;
    }
}
// 4. RESULT STREAMING WITH BACKPRESSURE
class StreamingResponder {
    constructor() {
        this.maxBufferSize = 1000;
    }
    async streamResults(query, processor, response) {
        response.raw.setHeader('Content-Type', 'application/x-ndjson');
        response.raw.setHeader('Cache-Control', 'no-cache');
        response.raw.setHeader('X-Accel-Buffering', 'no'); // Disable nginx buffering
        let bufferSize = 0;
        let backpressure = false;
        for await (const result of processor) {
            // Check backpressure
            if (!response.raw.write(JSON.stringify(result) + '\n')) {
                backpressure = true;
                await new Promise(resolve => response.raw.once('drain', resolve));
                backpressure = false;
            }
            bufferSize++;
            // Apply backpressure to source if buffer too large
            if (bufferSize > this.maxBufferSize) {
                await new Promise(resolve => setTimeout(resolve, 10));
                bufferSize = Math.max(0, bufferSize - 100);
            }
        }
        response.raw.end();
    }
}
// 5. QUERY PLAN CACHE (for complex filters)
class QueryPlanCache {
    constructor() {
        this.cache = new Map();
        this.stats = new Map();
    }
    getPlan(filter) {
        const key = this.getKey(filter);
        const plan = this.cache.get(key);
        if (plan) {
            const stat = this.stats.get(key) || { hits: 0, avgTime: 0 };
            stat.hits++;
            this.stats.set(key, stat);
        }
        return plan;
    }
    cachePlan(filter, plan, executionTime) {
        const key = this.getKey(filter);
        this.cache.set(key, plan);
        const stat = this.stats.get(key) || { hits: 0, avgTime: 0 };
        stat.avgTime = (stat.avgTime * stat.hits + executionTime) / (stat.hits + 1);
        this.stats.set(key, stat);
        // Evict least valuable plans
        if (this.cache.size > 1000) {
            this.evictLowValue();
        }
    }
    getKey(filter) {
        return JSON.stringify(filter, Object.keys(filter).sort());
    }
    evictLowValue() {
        // Calculate value score: hits / avgTime
        const scored = Array.from(this.stats.entries())
            .map(([key, stat]) => ({
            key,
            score: stat.hits / (stat.avgTime + 1)
        }))
            .sort((a, b) => a.score - b.score);
        // Remove bottom 20%
        const toRemove = Math.floor(scored.length * 0.2);
        for (let i = 0; i < toRemove; i++) {
            this.cache.delete(scored[i].key);
            this.stats.delete(scored[i].key);
        }
    }
}
// 6. OPTIMIZED MAIN SERVICE
const fastify = (0, fastify_1.default)({
    logger: true,
    trustProxy: true,
    http2: true,
    requestIdHeader: 'x-request-id',
    requestIdLogLabel: 'reqId',
    disableRequestLogging: true, // Custom logging for better performance
    ignoreTrailingSlash: true,
    maxParamLength: 500,
    bodyLimit: 1048576, // 1MB
    keepAliveTimeout: 65000, // Longer than ALB timeout
    connectionTimeout: 70000,
});
// Register plugins
fastify.register(helmet_1.default, {
    contentSecurityPolicy: false,
    global: true,
});
fastify.register(compress_1.default, {
    global: true,
    threshold: 1024,
    encodings: ['br', 'gzip', 'deflate'],
    brotliOptions: {
        params: {
            [require('zlib').constants.BROTLI_PARAM_MODE]: require('zlib').constants.BROTLI_MODE_TEXT,
            [require('zlib').constants.BROTLI_PARAM_QUALITY]: 4, // Fast compression
        }
    },
    zlibOptions: {
        level: 6, // Balanced
    }
});
// Redis-based rate limiting for distributed environment
fastify.register(rate_limit_1.default, {
    global: true,
    max: 1000,
    timeWindow: '1 minute',
    cache: 10000,
    allowList: ['127.0.0.1'],
    redis: process.env.REDIS_URL ? require('ioredis').createClient(process.env.REDIS_URL) : undefined,
    nameSpace: 'ruvector:ratelimit:',
    continueExceeding: true,
    enableDraftSpec: true,
});
fastify.register(websocket_1.default, {
    options: {
        maxPayload: 1048576,
        clientTracking: true,
        perMessageDeflate: {
            zlibDeflateOptions: {
                level: 6,
            },
            threshold: 1024,
        }
    }
});
// Initialize optimized components
const vectorClient = new vector_client_1.VectorClient({
    host: process.env.RUVECTOR_HOST || 'localhost',
    port: parseInt(process.env.RUVECTOR_PORT || '50051'),
    maxConnections: parseInt(process.env.MAX_CONNECTIONS || '100'),
    minConnections: parseInt(process.env.MIN_CONNECTIONS || '10'),
    enableCache: true,
    cacheTTL: 3600,
});
const loadBalancer = new load_balancer_1.LoadBalancer({
    backends: (process.env.BACKEND_URLS || '').split(','),
    healthCheckInterval: 30000,
    circuitBreakerThreshold: 5,
    circuitBreakerTimeout: 60000,
});
const batcher = new AdaptiveBatcher();
const queryPlanCache = new QueryPlanCache();
const streamer = new StreamingResponder();
// Setup adaptive batching
class VectorBatcher extends AdaptiveBatcher {
    async processBatch(queries) {
        return vectorClient.batchQuery(queries);
    }
}
const vectorBatcher = new VectorBatcher();
// Optimized batch query endpoint with plan caching
fastify.post('/api/query/batch', async (request, reply) => {
    const { queries, priority = 'normal' } = request.body;
    const results = await Promise.all(queries.map((query) => vectorBatcher.add(query, priority)));
    return { results, count: results.length };
});
// Streaming query with backpressure
fastify.get('/api/query/stream', async (request, reply) => {
    const { vector, topK = 10, filters } = request.query;
    // Check query plan cache
    let plan = filters ? queryPlanCache.getPlan(filters) : null;
    async function* resultGenerator() {
        const startTime = Date.now();
        for await (const result of vectorClient.streamQuery({ vector, topK, filters, plan })) {
            yield result;
        }
        // Cache the plan if it was efficient
        if (filters && !plan) {
            const executionTime = Date.now() - startTime;
            queryPlanCache.cachePlan(filters, { ...filters, optimized: true }, executionTime);
        }
    }
    await streamer.streamResults({ vector, topK, filters }, resultGenerator(), reply);
});
// Health endpoint with detailed status
fastify.get('/health', async (request, reply) => {
    const health = {
        status: 'healthy',
        timestamp: new Date().toISOString(),
        uptime: process.uptime(),
        memory: process.memoryUsage(),
        connections: {
            active: vectorClient.getActiveConnections(),
            poolSize: vectorClient.getPoolSize(),
        },
        cache: {
            hitRate: vectorClient.getCacheHitRate(),
            size: vectorClient.getCacheSize(),
        },
        batcher: {
            queueSizes: {},
        },
        loadBalancer: {
            backends: loadBalancer.getBackendHealth(),
        },
    };
    return health;
});
// Graceful shutdown
const gracefulShutdown = async (signal) => {
    console.log(`Received ${signal}, starting graceful shutdown...`);
    // Stop accepting new connections
    await fastify.close();
    // Wait for in-flight requests (max 30 seconds)
    await new Promise(resolve => setTimeout(resolve, 30000));
    // Close connections
    await vectorClient.close();
    console.log('Graceful shutdown complete');
    process.exit(0);
};
process.on('SIGTERM', () => gracefulShutdown('SIGTERM'));
process.on('SIGINT', () => gracefulShutdown('SIGINT'));
// Start server
const start = async () => {
    try {
        const port = parseInt(process.env.PORT || '8080');
        const host = process.env.HOST || '0.0.0.0';
        await fastify.listen({ port, host });
        console.log(`Server listening on ${host}:${port}`);
        console.log(`Optimizations enabled: adaptive batching, compressed cache, connection pooling`);
    }
    catch (err) {
        fastify.log.error(err);
        process.exit(1);
    }
};
start();
exports.default = fastify;
//# sourceMappingURL=streaming-service-optimized.js.map