/**
 * Cloud Run Streaming Service - Main Entry Point
 *
 * High-performance HTTP/2 + WebSocket server for massive concurrent connections.
 * Optimized for 500M concurrent learning streams with adaptive scaling.
 */

import Fastify, { FastifyInstance, FastifyRequest, FastifyReply } from 'fastify';
import fastifyWebsocket from '@fastify/websocket';
import fastifyCompress from '@fastify/compress';
import fastifyHelmet from '@fastify/helmet';
import fastifyRateLimit from '@fastify/rate-limit';
import { WebSocket } from 'ws';
import { VectorClient } from './vector-client';
import { LoadBalancer } from './load-balancer';
import { trace, context, SpanStatusCode } from '@opentelemetry/api';
import { register as metricsRegister, Counter, Histogram, Gauge } from 'prom-client';

// Environment configuration
const CONFIG = {
  port: parseInt(process.env.PORT || '8080', 10),
  host: process.env.HOST || '0.0.0.0',
  nodeEnv: process.env.NODE_ENV || 'production',
  maxConnections: parseInt(process.env.MAX_CONNECTIONS || '100000', 10),
  requestTimeout: parseInt(process.env.REQUEST_TIMEOUT || '30000', 10),
  keepAliveTimeout: parseInt(process.env.KEEP_ALIVE_TIMEOUT || '65000', 10),
  headersTimeout: parseInt(process.env.HEADERS_TIMEOUT || '66000', 10),
  maxRequestsPerSocket: parseInt(process.env.MAX_REQUESTS_PER_SOCKET || '1000', 10),
  ruvectorHost: process.env.RUVECTOR_HOST || 'localhost:50051',
  enableTracing: process.env.ENABLE_TRACING === 'true',
  enableMetrics: process.env.ENABLE_METRICS !== 'false',
  gracefulShutdownTimeout: parseInt(process.env.GRACEFUL_SHUTDOWN_TIMEOUT || '10000', 10),
};

// Prometheus metrics
const metrics = {
  httpRequests: new Counter({
    name: 'http_requests_total',
    help: 'Total number of HTTP requests',
    labelNames: ['method', 'path', 'status_code'],
  }),
  httpDuration: new Histogram({
    name: 'http_request_duration_seconds',
    help: 'HTTP request duration in seconds',
    labelNames: ['method', 'path', 'status_code'],
    buckets: [0.01, 0.05, 0.1, 0.5, 1, 2.5, 5, 10],
  }),
  activeConnections: new Gauge({
    name: 'active_connections',
    help: 'Number of active connections',
    labelNames: ['type'],
  }),
  streamingQueries: new Counter({
    name: 'streaming_queries_total',
    help: 'Total number of streaming queries',
    labelNames: ['protocol', 'status'],
  }),
  vectorOperations: new Histogram({
    name: 'vector_operations_duration_seconds',
    help: 'Vector operation duration in seconds',
    labelNames: ['operation', 'status'],
    buckets: [0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1],
  }),
  batchSize: new Histogram({
    name: 'batch_size',
    help: 'Size of batched requests',
    buckets: [1, 5, 10, 25, 50, 100, 250, 500],
  }),
};

// Tracer
const tracer = trace.getTracer('streaming-service', '1.0.0');

// Connection manager
class ConnectionManager {
  private httpConnections = new Set<FastifyReply>();
  private wsConnections = new Set<WebSocket>();
  private batchQueue: Map<string, Array<{ query: any; callback: Function }>> = new Map();
  private batchTimer: NodeJS.Timeout | null = null;
  private readonly BATCH_INTERVAL = 10; // 10ms batching window
  private readonly MAX_BATCH_SIZE = 100;

  constructor(
    private vectorClient: VectorClient,
    private loadBalancer: LoadBalancer
  ) {}

  // HTTP connection tracking
  registerHttpConnection(reply: FastifyReply): void {
    this.httpConnections.add(reply);
    metrics.activeConnections.inc({ type: 'http' });
  }

  unregisterHttpConnection(reply: FastifyReply): void {
    this.httpConnections.delete(reply);
    metrics.activeConnections.dec({ type: 'http' });
  }

  // WebSocket connection tracking
  registerWsConnection(ws: WebSocket): void {
    this.wsConnections.add(ws);
    metrics.activeConnections.inc({ type: 'websocket' });

    ws.on('close', () => {
      this.unregisterWsConnection(ws);
    });
  }

  unregisterWsConnection(ws: WebSocket): void {
    this.wsConnections.delete(ws);
    metrics.activeConnections.dec({ type: 'websocket' });
  }

  // Request batching for efficiency
  async batchQuery(query: any): Promise<any> {
    return new Promise((resolve, reject) => {
      const batchKey = this.getBatchKey(query);

      if (!this.batchQueue.has(batchKey)) {
        this.batchQueue.set(batchKey, []);
      }

      const batch = this.batchQueue.get(batchKey)!;
      batch.push({ query, callback: (err: Error | null, result: any) => {
        if (err) reject(err);
        else resolve(result);
      }});

      metrics.batchSize.observe(batch.length);

      // Process batch when full or after timeout
      if (batch.length >= this.MAX_BATCH_SIZE) {
        this.processBatch(batchKey);
      } else if (!this.batchTimer) {
        this.batchTimer = setTimeout(() => {
          this.processAllBatches();
        }, this.BATCH_INTERVAL);
      }
    });
  }

  private getBatchKey(query: any): string {
    // Group similar queries for batching
    return `${query.collection || 'default'}_${query.operation || 'search'}`;
  }

  private async processBatch(batchKey: string): Promise<void> {
    const batch = this.batchQueue.get(batchKey);
    if (!batch || batch.length === 0) return;

    this.batchQueue.delete(batchKey);

    const span = tracer.startSpan('process-batch', {
      attributes: { batchKey, batchSize: batch.length },
    });

    try {
      const queries = batch.map(item => item.query);
      const results = await this.vectorClient.batchQuery(queries);

      results.forEach((result, index) => {
        batch[index].callback(null, result);
      });

      span.setStatus({ code: SpanStatusCode.OK });
    } catch (error) {
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error as Error).message });
      batch.forEach(item => item.callback(error, null));
    } finally {
      span.end();
    }
  }

  private async processAllBatches(): Promise<void> {
    this.batchTimer = null;
    const batchKeys = Array.from(this.batchQueue.keys());
    await Promise.all(batchKeys.map(key => this.processBatch(key)));
  }

  // Graceful shutdown
  async shutdown(): Promise<void> {
    console.log('Starting graceful shutdown...');

    // Stop accepting new connections
    this.httpConnections.forEach(reply => {
      if (!reply.sent) {
        reply.code(503).send({ error: 'Service shutting down' });
      }
    });

    // Close WebSocket connections gracefully
    this.wsConnections.forEach(ws => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'shutdown', message: 'Service shutting down' }));
        ws.close(1001, 'Service shutting down');
      }
    });

    // Process remaining batches
    await this.processAllBatches();

    console.log(`Closed ${this.httpConnections.size} HTTP and ${this.wsConnections.size} WebSocket connections`);
  }

  getStats() {
    return {
      httpConnections: this.httpConnections.size,
      wsConnections: this.wsConnections.size,
      pendingBatches: this.batchQueue.size,
    };
  }
}

// Main application setup
export class StreamingService {
  private app: FastifyInstance;
  private vectorClient: VectorClient;
  private loadBalancer: LoadBalancer;
  private connectionManager: ConnectionManager;
  private isShuttingDown = false;

  constructor() {
    this.app = Fastify({
      logger: {
        level: CONFIG.nodeEnv === 'production' ? 'info' : 'debug',
        serializers: {
          req(request) {
            return {
              method: request.method,
              url: request.url,
              headers: request.headers,
              remoteAddress: request.ip,
            };
          },
        },
      },
      trustProxy: true,
      http2: true,
      connectionTimeout: CONFIG.requestTimeout,
      keepAliveTimeout: CONFIG.keepAliveTimeout,
      requestIdHeader: 'x-request-id',
      requestIdLogLabel: 'requestId',
    });

    this.vectorClient = new VectorClient({
      host: CONFIG.ruvectorHost,
      maxConnections: 100,
      enableMetrics: CONFIG.enableMetrics,
    });

    this.loadBalancer = new LoadBalancer({
      maxRequestsPerSecond: 10000,
      circuitBreakerThreshold: 0.5,
      circuitBreakerTimeout: 30000,
    });

    this.connectionManager = new ConnectionManager(this.vectorClient, this.loadBalancer);

    this.setupMiddleware();
    this.setupRoutes();
    this.setupShutdownHandlers();
  }

  private setupMiddleware(): void {
    // Security headers
    this.app.register(fastifyHelmet, {
      contentSecurityPolicy: false,
    });

    // Compression
    this.app.register(fastifyCompress, {
      global: true,
      encodings: ['gzip', 'deflate', 'br'],
    });

    // Rate limiting
    this.app.register(fastifyRateLimit, {
      max: 1000,
      timeWindow: '1 minute',
      cache: 10000,
      allowList: ['127.0.0.1'],
      redis: process.env.REDIS_URL ? { url: process.env.REDIS_URL } : undefined,
    });

    // WebSocket support
    this.app.register(fastifyWebsocket, {
      options: {
        maxPayload: 1024 * 1024, // 1MB
        perMessageDeflate: true,
      },
    });

    // Request tracking
    this.app.addHook('onRequest', async (request, reply) => {
      const startTime = Date.now();
      reply.raw.on('finish', () => {
        const duration = (Date.now() - startTime) / 1000;
        const labels = {
          method: request.method,
          path: request.routerPath || request.url,
          status_code: reply.statusCode.toString(),
        };
        metrics.httpRequests.inc(labels);
        metrics.httpDuration.observe(labels, duration);
      });
    });

    // Shutdown check
    this.app.addHook('onRequest', async (request, reply) => {
      if (this.isShuttingDown) {
        reply.code(503).send({ error: 'Service shutting down' });
      }
    });
  }

  private setupRoutes(): void {
    // Health check endpoint
    this.app.get('/health', async (request, reply) => {
      const isHealthy = await this.vectorClient.healthCheck();
      const stats = this.connectionManager.getStats();

      if (isHealthy) {
        return {
          status: 'healthy',
          timestamp: new Date().toISOString(),
          connections: stats,
          version: process.env.SERVICE_VERSION || '1.0.0',
        };
      } else {
        reply.code(503);
        return {
          status: 'unhealthy',
          timestamp: new Date().toISOString(),
          error: 'Vector client unhealthy',
        };
      }
    });

    // Readiness check
    this.app.get('/ready', async (request, reply) => {
      if (this.isShuttingDown) {
        reply.code(503);
        return { status: 'not ready', reason: 'shutting down' };
      }

      const stats = this.connectionManager.getStats();
      if (stats.httpConnections + stats.wsConnections >= CONFIG.maxConnections) {
        reply.code(503);
        return { status: 'not ready', reason: 'max connections reached' };
      }

      return { status: 'ready', connections: stats };
    });

    // Metrics endpoint
    this.app.get('/metrics', async (request, reply) => {
      reply.type('text/plain');
      return metricsRegister.metrics();
    });

    // SSE streaming endpoint
    this.app.get('/stream/sse/:collection', async (request, reply) => {
      const { collection } = request.params as { collection: string };
      const query = request.query as any;

      reply.raw.writeHead(200, {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive',
        'X-Accel-Buffering': 'no', // Disable nginx buffering
      });

      this.connectionManager.registerHttpConnection(reply);

      const span = tracer.startSpan('sse-stream', {
        attributes: { collection, queryType: query.type || 'search' },
      });

      try {
        // Heartbeat to keep connection alive
        const heartbeat = setInterval(() => {
          if (!reply.raw.destroyed) {
            reply.raw.write(': heartbeat\n\n');
          } else {
            clearInterval(heartbeat);
          }
        }, 30000);

        // Stream results
        await this.vectorClient.streamQuery(collection, query, (chunk) => {
          if (!reply.raw.destroyed) {
            const data = JSON.stringify(chunk);
            reply.raw.write(`data: ${data}\n\n`);
          }
        });

        clearInterval(heartbeat);
        reply.raw.write('event: done\ndata: {}\n\n');
        reply.raw.end();

        metrics.streamingQueries.inc({ protocol: 'sse', status: 'success' });
        span.setStatus({ code: SpanStatusCode.OK });
      } catch (error) {
        this.app.log.error({ error, collection }, 'SSE stream error');
        metrics.streamingQueries.inc({ protocol: 'sse', status: 'error' });
        span.setStatus({ code: SpanStatusCode.ERROR, message: (error as Error).message });
        reply.raw.end();
      } finally {
        this.connectionManager.unregisterHttpConnection(reply);
        span.end();
      }
    });

    // WebSocket streaming endpoint
    this.app.get('/stream/ws/:collection', { websocket: true }, (connection, request) => {
      const { collection } = request.params as { collection: string };
      const ws = connection.socket;

      this.connectionManager.registerWsConnection(ws);

      const span = tracer.startSpan('websocket-stream', {
        attributes: { collection },
      });

      ws.on('message', async (message) => {
        try {
          const query = JSON.parse(message.toString());

          if (query.type === 'ping') {
            ws.send(JSON.stringify({ type: 'pong', timestamp: Date.now() }));
            return;
          }

          // Route through load balancer
          const routed = await this.loadBalancer.route(collection, query);
          if (!routed) {
            ws.send(JSON.stringify({ type: 'error', error: 'Load balancer rejected request' }));
            return;
          }

          // Stream results
          await this.vectorClient.streamQuery(collection, query, (chunk) => {
            if (ws.readyState === WebSocket.OPEN) {
              ws.send(JSON.stringify({ type: 'data', data: chunk }));
            }
          });

          ws.send(JSON.stringify({ type: 'done' }));
          metrics.streamingQueries.inc({ protocol: 'websocket', status: 'success' });
        } catch (error) {
          this.app.log.error({ error, collection }, 'WebSocket message error');
          ws.send(JSON.stringify({ type: 'error', error: (error as Error).message }));
          metrics.streamingQueries.inc({ protocol: 'websocket', status: 'error' });
        }
      });

      ws.on('error', (error) => {
        this.app.log.error({ error }, 'WebSocket error');
        span.setStatus({ code: SpanStatusCode.ERROR, message: error.message });
      });

      ws.on('close', () => {
        span.setStatus({ code: SpanStatusCode.OK });
        span.end();
      });
    });

    // Batch query endpoint
    this.app.post('/query/batch', async (request, reply) => {
      const { queries } = request.body as { queries: any[] };

      if (!Array.isArray(queries) || queries.length === 0) {
        reply.code(400);
        return { error: 'queries must be a non-empty array' };
      }

      const span = tracer.startSpan('batch-query', {
        attributes: { queryCount: queries.length },
      });

      try {
        const results = await Promise.all(
          queries.map(query => this.connectionManager.batchQuery(query))
        );

        span.setStatus({ code: SpanStatusCode.OK });
        return { results };
      } catch (error) {
        this.app.log.error({ error }, 'Batch query error');
        span.setStatus({ code: SpanStatusCode.ERROR, message: (error as Error).message });
        reply.code(500);
        return { error: (error as Error).message };
      } finally {
        span.end();
      }
    });

    // Single query endpoint
    this.app.post('/query/:collection', async (request, reply) => {
      const { collection } = request.params as { collection: string };
      const query = request.body as any;

      const span = tracer.startSpan('single-query', {
        attributes: { collection, queryType: query.type || 'search' },
      });

      try {
        const result = await this.connectionManager.batchQuery({ collection, ...query });
        span.setStatus({ code: SpanStatusCode.OK });
        return result;
      } catch (error) {
        this.app.log.error({ error, collection }, 'Query error');
        span.setStatus({ code: SpanStatusCode.ERROR, message: (error as Error).message });
        reply.code(500);
        return { error: (error as Error).message };
      } finally {
        span.end();
      }
    });
  }

  private setupShutdownHandlers(): void {
    const shutdown = async (signal: string) => {
      console.log(`Received ${signal}, starting graceful shutdown...`);
      this.isShuttingDown = true;

      const timeout = setTimeout(() => {
        console.error('Graceful shutdown timeout, forcing exit');
        process.exit(1);
      }, CONFIG.gracefulShutdownTimeout);

      try {
        await this.connectionManager.shutdown();
        await this.vectorClient.close();
        await this.app.close();
        clearTimeout(timeout);
        console.log('Graceful shutdown completed');
        process.exit(0);
      } catch (error) {
        console.error('Error during shutdown:', error);
        clearTimeout(timeout);
        process.exit(1);
      }
    };

    process.on('SIGTERM', () => shutdown('SIGTERM'));
    process.on('SIGINT', () => shutdown('SIGINT'));
  }

  async start(): Promise<void> {
    try {
      await this.vectorClient.initialize();
      await this.app.listen({ port: CONFIG.port, host: CONFIG.host });
      console.log(`Streaming service running on ${CONFIG.host}:${CONFIG.port}`);
      console.log(`Environment: ${CONFIG.nodeEnv}`);
      console.log(`Max connections: ${CONFIG.maxConnections}`);
    } catch (error) {
      this.app.log.error(error);
      process.exit(1);
    }
  }
}

// Start service if run directly
if (require.main === module) {
  const service = new StreamingService();
  service.start();
}
