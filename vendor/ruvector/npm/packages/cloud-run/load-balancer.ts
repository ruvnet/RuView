/**
 * Load Balancer - Intelligent request routing and traffic management
 *
 * Features:
 * - Circuit breaker pattern
 * - Rate limiting per client
 * - Regional routing
 * - Request prioritization
 * - Health-based routing
 */

import { EventEmitter } from 'events';
import { trace, SpanStatusCode } from '@opentelemetry/api';
import { Counter, Gauge, Histogram } from 'prom-client';

// Metrics
const metrics = {
  routedRequests: new Counter({
    name: 'load_balancer_routed_requests_total',
    help: 'Total number of routed requests',
    labelNames: ['backend', 'status'],
  }),
  rejectedRequests: new Counter({
    name: 'load_balancer_rejected_requests_total',
    help: 'Total number of rejected requests',
    labelNames: ['reason'],
  }),
  circuitBreakerState: new Gauge({
    name: 'circuit_breaker_state',
    help: 'Circuit breaker state (0=closed, 1=open, 2=half-open)',
    labelNames: ['backend'],
  }),
  rateLimitActive: new Gauge({
    name: 'rate_limit_active_clients',
    help: 'Number of clients currently rate limited',
  }),
  requestLatency: new Histogram({
    name: 'load_balancer_request_latency_seconds',
    help: 'Request latency in seconds',
    labelNames: ['backend'],
    buckets: [0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1],
  }),
};

const tracer = trace.getTracer('load-balancer', '1.0.0');

// Configuration
export interface LoadBalancerConfig {
  maxRequestsPerSecond?: number;
  circuitBreakerThreshold?: number;
  circuitBreakerTimeout?: number;
  halfOpenMaxRequests?: number;
  backends?: BackendConfig[];
  enableRegionalRouting?: boolean;
  priorityQueueSize?: number;
}

export interface BackendConfig {
  id: string;
  host: string;
  region?: string;
  weight?: number;
  maxConcurrency?: number;
}

// Circuit breaker states
enum CircuitState {
  CLOSED = 0,
  OPEN = 1,
  HALF_OPEN = 2,
}

// Request priority
enum RequestPriority {
  LOW = 0,
  NORMAL = 1,
  HIGH = 2,
  CRITICAL = 3,
}

/**
 * Token Bucket Rate Limiter
 */
class RateLimiter {
  private buckets = new Map<string, { tokens: number; lastRefill: number }>();
  private readonly capacity: number;
  private readonly refillRate: number;

  constructor(requestsPerSecond: number) {
    this.capacity = requestsPerSecond;
    this.refillRate = requestsPerSecond;
  }

  tryAcquire(clientId: string, tokens = 1): boolean {
    const now = Date.now();
    let bucket = this.buckets.get(clientId);

    if (!bucket) {
      bucket = { tokens: this.capacity, lastRefill: now };
      this.buckets.set(clientId, bucket);
    }

    // Refill tokens based on time passed
    const timePassed = (now - bucket.lastRefill) / 1000;
    const tokensToAdd = timePassed * this.refillRate;
    bucket.tokens = Math.min(this.capacity, bucket.tokens + tokensToAdd);
    bucket.lastRefill = now;

    // Try to consume tokens
    if (bucket.tokens >= tokens) {
      bucket.tokens -= tokens;
      return true;
    }

    return false;
  }

  reset(clientId: string): void {
    this.buckets.delete(clientId);
  }

  getStats(): { totalClients: number; limitedClients: number } {
    let limitedClients = 0;
    for (const [_, bucket] of this.buckets) {
      if (bucket.tokens < 1) {
        limitedClients++;
      }
    }
    return {
      totalClients: this.buckets.size,
      limitedClients,
    };
  }
}

/**
 * Circuit Breaker
 */
class CircuitBreaker {
  private state = CircuitState.CLOSED;
  private failures = 0;
  private successes = 0;
  private lastFailureTime = 0;
  private halfOpenRequests = 0;

  constructor(
    private backendId: string,
    private threshold: number,
    private timeout: number,
    private halfOpenMaxRequests: number
  ) {
    this.updateMetrics();
  }

  async execute<T>(fn: () => Promise<T>): Promise<T> {
    if (this.state === CircuitState.OPEN) {
      // Check if timeout has passed
      if (Date.now() - this.lastFailureTime >= this.timeout) {
        this.state = CircuitState.HALF_OPEN;
        this.halfOpenRequests = 0;
        this.updateMetrics();
      } else {
        throw new Error(`Circuit breaker open for backend ${this.backendId}`);
      }
    }

    if (this.state === CircuitState.HALF_OPEN) {
      if (this.halfOpenRequests >= this.halfOpenMaxRequests) {
        throw new Error(`Circuit breaker half-open limit reached for backend ${this.backendId}`);
      }
      this.halfOpenRequests++;
    }

    const startTime = Date.now();

    try {
      const result = await fn();
      this.onSuccess();

      const duration = (Date.now() - startTime) / 1000;
      metrics.requestLatency.observe({ backend: this.backendId }, duration);
      metrics.routedRequests.inc({ backend: this.backendId, status: 'success' });

      return result;
    } catch (error) {
      this.onFailure();
      metrics.routedRequests.inc({ backend: this.backendId, status: 'failure' });
      throw error;
    }
  }

  private onSuccess(): void {
    this.failures = 0;
    this.successes++;

    if (this.state === CircuitState.HALF_OPEN) {
      if (this.successes >= this.halfOpenMaxRequests) {
        this.state = CircuitState.CLOSED;
        this.successes = 0;
        this.updateMetrics();
      }
    }
  }

  private onFailure(): void {
    this.failures++;
    this.lastFailureTime = Date.now();

    const failureRate = this.failures / (this.failures + this.successes);

    if (failureRate >= this.threshold) {
      this.state = CircuitState.OPEN;
      this.updateMetrics();
    }
  }

  private updateMetrics(): void {
    metrics.circuitBreakerState.set({ backend: this.backendId }, this.state);
  }

  getState(): CircuitState {
    return this.state;
  }

  reset(): void {
    this.state = CircuitState.CLOSED;
    this.failures = 0;
    this.successes = 0;
    this.lastFailureTime = 0;
    this.halfOpenRequests = 0;
    this.updateMetrics();
  }
}

/**
 * Backend Manager
 */
class BackendManager {
  private backends: Map<string, {
    config: BackendConfig;
    circuitBreaker: CircuitBreaker;
    activeRequests: number;
    healthScore: number;
  }> = new Map();

  constructor(
    backends: BackendConfig[],
    circuitBreakerThreshold: number,
    circuitBreakerTimeout: number,
    halfOpenMaxRequests: number
  ) {
    for (const backend of backends) {
      this.backends.set(backend.id, {
        config: backend,
        circuitBreaker: new CircuitBreaker(
          backend.id,
          circuitBreakerThreshold,
          circuitBreakerTimeout,
          halfOpenMaxRequests
        ),
        activeRequests: 0,
        healthScore: 1.0,
      });
    }
  }

  selectBackend(region?: string): string | null {
    const available = Array.from(this.backends.entries())
      .filter(([_, backend]) => {
        // Filter by region if specified
        if (region && backend.config.region !== region) {
          return false;
        }

        // Filter by circuit breaker state
        if (backend.circuitBreaker.getState() === CircuitState.OPEN) {
          return false;
        }

        // Filter by concurrency limit
        if (backend.config.maxConcurrency &&
            backend.activeRequests >= backend.config.maxConcurrency) {
          return false;
        }

        return true;
      })
      .map(([id, backend]) => ({
        id,
        score: this.calculateScore(backend),
      }))
      .sort((a, b) => b.score - a.score);

    return available.length > 0 ? available[0].id : null;
  }

  private calculateScore(backend: {
    config: BackendConfig;
    activeRequests: number;
    healthScore: number;
  }): number {
    const weight = backend.config.weight || 1;
    const loadFactor = backend.config.maxConcurrency
      ? 1 - (backend.activeRequests / backend.config.maxConcurrency)
      : 1;

    return weight * loadFactor * backend.healthScore;
  }

  async executeOnBackend<T>(backendId: string, fn: () => Promise<T>): Promise<T> {
    const backend = this.backends.get(backendId);
    if (!backend) {
      throw new Error(`Backend ${backendId} not found`);
    }

    backend.activeRequests++;

    try {
      return await backend.circuitBreaker.execute(fn);
    } finally {
      backend.activeRequests--;
    }
  }

  updateHealth(backendId: string, healthScore: number): void {
    const backend = this.backends.get(backendId);
    if (backend) {
      backend.healthScore = Math.max(0, Math.min(1, healthScore));
    }
  }

  getStats() {
    const stats: Record<string, any> = {};
    for (const [id, backend] of this.backends) {
      stats[id] = {
        activeRequests: backend.activeRequests,
        healthScore: backend.healthScore,
        circuitState: backend.circuitBreaker.getState(),
        region: backend.config.region,
      };
    }
    return stats;
  }
}

/**
 * Priority Queue for request scheduling
 */
class PriorityQueue<T> {
  private queues: Map<RequestPriority, T[]> = new Map([
    [RequestPriority.CRITICAL, []],
    [RequestPriority.HIGH, []],
    [RequestPriority.NORMAL, []],
    [RequestPriority.LOW, []],
  ]);

  enqueue(item: T, priority: RequestPriority): void {
    const queue = this.queues.get(priority)!;
    queue.push(item);
  }

  dequeue(): T | undefined {
    // Process by priority
    for (const priority of [
      RequestPriority.CRITICAL,
      RequestPriority.HIGH,
      RequestPriority.NORMAL,
      RequestPriority.LOW,
    ]) {
      const queue = this.queues.get(priority)!;
      if (queue.length > 0) {
        return queue.shift();
      }
    }
    return undefined;
  }

  size(): number {
    return Array.from(this.queues.values()).reduce((sum, q) => sum + q.length, 0);
  }

  clear(): void {
    for (const queue of this.queues.values()) {
      queue.length = 0;
    }
  }
}

/**
 * Load Balancer
 */
export class LoadBalancer extends EventEmitter {
  private rateLimiter: RateLimiter;
  private backendManager: BackendManager;
  private requestQueue: PriorityQueue<() => Promise<any>>;
  private config: Required<LoadBalancerConfig>;

  constructor(config: LoadBalancerConfig) {
    super();

    this.config = {
      maxRequestsPerSecond: config.maxRequestsPerSecond || 10000,
      circuitBreakerThreshold: config.circuitBreakerThreshold || 0.5,
      circuitBreakerTimeout: config.circuitBreakerTimeout || 30000,
      halfOpenMaxRequests: config.halfOpenMaxRequests || 5,
      backends: config.backends || [{ id: 'default', host: 'localhost' }],
      enableRegionalRouting: config.enableRegionalRouting !== false,
      priorityQueueSize: config.priorityQueueSize || 1000,
    };

    this.rateLimiter = new RateLimiter(this.config.maxRequestsPerSecond);
    this.backendManager = new BackendManager(
      this.config.backends,
      this.config.circuitBreakerThreshold,
      this.config.circuitBreakerTimeout,
      this.config.halfOpenMaxRequests
    );
    this.requestQueue = new PriorityQueue();

    this.updateMetrics();
  }

  async route(
    collection: string,
    query: any,
    clientId: string = 'default',
    priority: RequestPriority = RequestPriority.NORMAL
  ): Promise<boolean> {
    const span = tracer.startSpan('load-balancer-route', {
      attributes: { collection, clientId, priority },
    });

    try {
      // Rate limiting check
      if (!this.rateLimiter.tryAcquire(clientId)) {
        metrics.rejectedRequests.inc({ reason: 'rate_limit' });
        span.setStatus({ code: SpanStatusCode.ERROR, message: 'Rate limit exceeded' });
        return false;
      }

      // Queue size check
      if (this.requestQueue.size() >= this.config.priorityQueueSize) {
        metrics.rejectedRequests.inc({ reason: 'queue_full' });
        span.setStatus({ code: SpanStatusCode.ERROR, message: 'Queue full' });
        return false;
      }

      // Select backend
      const region = query.region;
      const backendId = this.backendManager.selectBackend(
        this.config.enableRegionalRouting ? region : undefined
      );

      if (!backendId) {
        metrics.rejectedRequests.inc({ reason: 'no_backend' });
        span.setStatus({ code: SpanStatusCode.ERROR, message: 'No backend available' });
        return false;
      }

      span.setStatus({ code: SpanStatusCode.OK });
      return true;
    } catch (error) {
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error as Error).message });
      return false;
    } finally {
      span.end();
    }
  }

  async executeWithLoadBalancing<T>(
    fn: () => Promise<T>,
    region?: string,
    priority: RequestPriority = RequestPriority.NORMAL
  ): Promise<T> {
    const backendId = this.backendManager.selectBackend(
      this.config.enableRegionalRouting ? region : undefined
    );

    if (!backendId) {
      throw new Error('No backend available');
    }

    return this.backendManager.executeOnBackend(backendId, fn);
  }

  updateBackendHealth(backendId: string, healthScore: number): void {
    this.backendManager.updateHealth(backendId, healthScore);
  }

  private updateMetrics(): void {
    setInterval(() => {
      const rateLimitStats = this.rateLimiter.getStats();
      metrics.rateLimitActive.set(rateLimitStats.limitedClients);
    }, 5000);
  }

  getStats() {
    return {
      rateLimit: this.rateLimiter.getStats(),
      backends: this.backendManager.getStats(),
      queueSize: this.requestQueue.size(),
    };
  }

  reset(): void {
    this.requestQueue.clear();
  }
}
