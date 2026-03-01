/**
 * Regional Agent - Per-region agent implementation for distributed processing
 *
 * Handles:
 * - Region-specific initialization
 * - Local query processing
 * - Cross-region communication
 * - State synchronization
 * - Metrics reporting
 */

import { EventEmitter } from 'events';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

export interface RegionalAgentConfig {
  agentId: string;
  region: string;
  coordinatorEndpoint: string;
  localStoragePath: string;
  maxConcurrentStreams: number;
  metricsReportInterval: number;
  syncInterval: number;
  enableClaudeFlowHooks: boolean;
  vectorDimensions: number;
  capabilities: string[];
}

export interface QueryRequest {
  id: string;
  vector: number[];
  topK: number;
  filters?: Record<string, any>;
  timeout: number;
}

export interface QueryResult {
  id: string;
  matches: Array<{
    id: string;
    score: number;
    metadata: Record<string, any>;
  }>;
  latency: number;
  region: string;
}

export interface SyncPayload {
  type: 'index' | 'update' | 'delete';
  data: any;
  timestamp: number;
  sourceRegion: string;
}

export class RegionalAgent extends EventEmitter {
  private activeStreams = 0;
  private totalQueries = 0;
  private totalLatency = 0;
  private metricsTimer?: NodeJS.Timeout;
  private syncTimer?: NodeJS.Timeout;
  private localIndex: Map<string, any> = new Map();
  private syncQueue: SyncPayload[] = [];
  private rateLimiter: RateLimiter;

  constructor(private config: RegionalAgentConfig) {
    super();
    this.rateLimiter = new RateLimiter({
      maxRequests: config.maxConcurrentStreams,
      windowMs: 1000,
    });
    this.initialize();
  }

  /**
   * Initialize regional agent
   */
  private async initialize(): Promise<void> {
    console.log(`[RegionalAgent:${this.config.region}] Initializing agent ${this.config.agentId}...`);

    if (this.config.enableClaudeFlowHooks) {
      try {
        // Pre-task hook for agent initialization
        await execAsync(
          `npx claude-flow@alpha hooks pre-task --description "Initialize regional agent ${this.config.agentId} in ${this.config.region}"`
        );

        // Restore session if available
        await execAsync(
          `npx claude-flow@alpha hooks session-restore --session-id "agent-${this.config.agentId}"`
        );

        console.log(`[RegionalAgent:${this.config.region}] Claude-flow hooks initialized`);
      } catch (error) {
        console.warn(`[RegionalAgent:${this.config.region}] Claude-flow hooks not available:`, error);
      }
    }

    // Load local index from storage
    await this.loadLocalIndex();

    // Start metrics reporting
    this.startMetricsReporting();

    // Start sync process
    this.startSyncProcess();

    // Register with coordinator
    await this.registerWithCoordinator();

    this.emit('agent:initialized', {
      agentId: this.config.agentId,
      region: this.config.region,
    });

    console.log(`[RegionalAgent:${this.config.region}] Agent ${this.config.agentId} initialized successfully`);
  }

  /**
   * Load local index from persistent storage
   */
  private async loadLocalIndex(): Promise<void> {
    try {
      // Placeholder for actual storage loading
      // In production, this would load from disk/database
      console.log(`[RegionalAgent:${this.config.region}] Loading local index from ${this.config.localStoragePath}`);

      // Simulate loading
      this.localIndex.clear();

      console.log(`[RegionalAgent:${this.config.region}] Local index loaded: ${this.localIndex.size} vectors`);
    } catch (error) {
      console.error(`[RegionalAgent:${this.config.region}] Error loading local index:`, error);
      throw error;
    }
  }

  /**
   * Register with coordinator
   */
  private async registerWithCoordinator(): Promise<void> {
    try {
      console.log(`[RegionalAgent:${this.config.region}] Registering with coordinator at ${this.config.coordinatorEndpoint}`);

      // In production, this would be an HTTP/gRPC call
      // For now, emit event
      this.emit('coordinator:register', {
        agentId: this.config.agentId,
        region: this.config.region,
        endpoint: `https://${this.config.region}.ruvector.io/agent/${this.config.agentId}`,
        capabilities: this.config.capabilities,
        capacity: this.config.maxConcurrentStreams,
        registeredAt: Date.now(),
      });

      console.log(`[RegionalAgent:${this.config.region}] Successfully registered with coordinator`);
    } catch (error) {
      console.error(`[RegionalAgent:${this.config.region}] Failed to register with coordinator:`, error);
      throw error;
    }
  }

  /**
   * Process query request locally
   */
  async processQuery(request: QueryRequest): Promise<QueryResult> {
    const startTime = Date.now();

    // Check rate limit
    if (!this.rateLimiter.tryAcquire()) {
      throw new Error('Rate limit exceeded');
    }

    this.activeStreams++;
    this.totalQueries++;

    try {
      console.log(`[RegionalAgent:${this.config.region}] Processing query ${request.id}`);

      // Validate query
      this.validateQuery(request);

      // Execute vector search
      const matches = await this.searchVectors(request);

      const latency = Date.now() - startTime;
      this.totalLatency += latency;

      const result: QueryResult = {
        id: request.id,
        matches,
        latency,
        region: this.config.region,
      };

      this.emit('query:completed', {
        queryId: request.id,
        latency,
        matchCount: matches.length,
      });

      if (this.config.enableClaudeFlowHooks) {
        try {
          // Notify about query completion
          await execAsync(
            `npx claude-flow@alpha hooks notify --message "Query ${request.id} completed in ${latency}ms with ${matches.length} matches"`
          );
        } catch (error) {
          // Non-critical error
        }
      }

      return result;

    } catch (error) {
      console.error(`[RegionalAgent:${this.config.region}] Error processing query ${request.id}:`, error);

      this.emit('query:failed', {
        queryId: request.id,
        error: error instanceof Error ? error.message : 'Unknown error',
      });

      throw error;

    } finally {
      this.activeStreams--;
      this.rateLimiter.release();
    }
  }

  /**
   * Validate query request
   */
  private validateQuery(request: QueryRequest): void {
    if (!request.vector || request.vector.length !== this.config.vectorDimensions) {
      throw new Error(
        `Invalid vector dimensions: expected ${this.config.vectorDimensions}, got ${request.vector?.length || 0}`
      );
    }

    if (request.topK <= 0 || request.topK > 1000) {
      throw new Error(`Invalid topK value: ${request.topK} (must be between 1 and 1000)`);
    }
  }

  /**
   * Search vectors in local index
   */
  private async searchVectors(request: QueryRequest): Promise<QueryResult['matches']> {
    // Placeholder for actual vector search
    // In production, this would use FAISS, Annoy, or similar library

    const matches: QueryResult['matches'] = [];

    // Simulate vector search
    for (const [id, vector] of this.localIndex.entries()) {
      const score = this.calculateSimilarity(request.vector, vector);

      // Apply filters if present
      if (request.filters && !this.matchesFilters(vector.metadata, request.filters)) {
        continue;
      }

      matches.push({
        id,
        score,
        metadata: vector.metadata || {},
      });
    }

    // Sort by score and return top-k
    matches.sort((a, b) => b.score - a.score);
    return matches.slice(0, request.topK);
  }

  /**
   * Calculate cosine similarity between vectors
   */
  private calculateSimilarity(v1: number[], v2: number[]): number {
    let dotProduct = 0;
    let norm1 = 0;
    let norm2 = 0;

    for (let i = 0; i < v1.length; i++) {
      dotProduct += v1[i] * v2[i];
      norm1 += v1[i] * v1[i];
      norm2 += v2[i] * v2[i];
    }

    return dotProduct / (Math.sqrt(norm1) * Math.sqrt(norm2));
  }

  /**
   * Check if metadata matches filters
   */
  private matchesFilters(metadata: Record<string, any>, filters: Record<string, any>): boolean {
    for (const [key, value] of Object.entries(filters)) {
      if (metadata[key] !== value) {
        return false;
      }
    }
    return true;
  }

  /**
   * Add/update vectors in local index
   */
  async indexVectors(vectors: Array<{ id: string; vector: number[]; metadata?: Record<string, any> }>): Promise<void> {
    console.log(`[RegionalAgent:${this.config.region}] Indexing ${vectors.length} vectors`);

    for (const { id, vector, metadata } of vectors) {
      this.localIndex.set(id, { vector, metadata });
    }

    // Queue for cross-region sync
    this.syncQueue.push({
      type: 'index',
      data: vectors,
      timestamp: Date.now(),
      sourceRegion: this.config.region,
    });

    this.emit('vectors:indexed', { count: vectors.length });

    if (this.config.enableClaudeFlowHooks) {
      try {
        await execAsync(
          `npx claude-flow@alpha hooks post-edit --file "local-index" --memory-key "swarm/${this.config.agentId}/index-update"`
        );
      } catch (error) {
        // Non-critical
      }
    }
  }

  /**
   * Delete vectors from local index
   */
  async deleteVectors(ids: string[]): Promise<void> {
    console.log(`[RegionalAgent:${this.config.region}] Deleting ${ids.length} vectors`);

    for (const id of ids) {
      this.localIndex.delete(id);
    }

    // Queue for cross-region sync
    this.syncQueue.push({
      type: 'delete',
      data: ids,
      timestamp: Date.now(),
      sourceRegion: this.config.region,
    });

    this.emit('vectors:deleted', { count: ids.length });
  }

  /**
   * Handle sync payload from other regions
   */
  async handleSyncPayload(payload: SyncPayload): Promise<void> {
    // Don't process our own sync messages
    if (payload.sourceRegion === this.config.region) {
      return;
    }

    console.log(
      `[RegionalAgent:${this.config.region}] Received sync payload from ${payload.sourceRegion}: ${payload.type}`
    );

    try {
      switch (payload.type) {
        case 'index':
          await this.indexVectors(payload.data);
          break;
        case 'update':
          await this.indexVectors(payload.data);
          break;
        case 'delete':
          await this.deleteVectors(payload.data);
          break;
      }

      this.emit('sync:applied', {
        type: payload.type,
        sourceRegion: payload.sourceRegion,
      });

    } catch (error) {
      console.error(`[RegionalAgent:${this.config.region}] Error applying sync payload:`, error);

      this.emit('sync:failed', {
        type: payload.type,
        sourceRegion: payload.sourceRegion,
        error: error instanceof Error ? error.message : 'Unknown error',
      });
    }
  }

  /**
   * Start metrics reporting loop
   */
  private startMetricsReporting(): void {
    this.metricsTimer = setInterval(() => {
      this.reportMetrics();
    }, this.config.metricsReportInterval);
  }

  /**
   * Report metrics to coordinator
   */
  private reportMetrics(): void {
    const metrics = {
      agentId: this.config.agentId,
      region: this.config.region,
      cpuUsage: this.getCpuUsage(),
      memoryUsage: this.getMemoryUsage(),
      activeStreams: this.activeStreams,
      queryLatency: this.totalQueries > 0 ? this.totalLatency / this.totalQueries : 0,
      timestamp: Date.now(),
      healthy: this.isHealthy(),
    };

    this.emit('metrics:report', metrics);

    // Reset counters (sliding window)
    if (this.totalQueries > 1000) {
      this.totalQueries = 0;
      this.totalLatency = 0;
    }
  }

  /**
   * Get CPU usage (placeholder)
   */
  private getCpuUsage(): number {
    // In production, this would read from /proc/stat or similar
    return Math.random() * 100;
  }

  /**
   * Get memory usage (placeholder)
   */
  private getMemoryUsage(): number {
    // In production, this would read from process.memoryUsage()
    const usage = process.memoryUsage();
    return (usage.heapUsed / usage.heapTotal) * 100;
  }

  /**
   * Check if agent is healthy
   */
  private isHealthy(): boolean {
    return (
      this.activeStreams < this.config.maxConcurrentStreams &&
      this.getMemoryUsage() < 90 &&
      this.getCpuUsage() < 90
    );
  }

  /**
   * Start sync process loop
   */
  private startSyncProcess(): void {
    this.syncTimer = setInterval(() => {
      this.processSyncQueue();
    }, this.config.syncInterval);
  }

  /**
   * Process sync queue (send to other regions)
   */
  private async processSyncQueue(): Promise<void> {
    if (this.syncQueue.length === 0) return;

    const batch = this.syncQueue.splice(0, 100); // Process in batches

    console.log(`[RegionalAgent:${this.config.region}] Processing sync batch: ${batch.length} items`);

    for (const payload of batch) {
      this.emit('sync:broadcast', payload);
    }
  }

  /**
   * Get agent status
   */
  getStatus(): {
    agentId: string;
    region: string;
    healthy: boolean;
    activeStreams: number;
    indexSize: number;
    syncQueueSize: number;
    avgQueryLatency: number;
  } {
    return {
      agentId: this.config.agentId,
      region: this.config.region,
      healthy: this.isHealthy(),
      activeStreams: this.activeStreams,
      indexSize: this.localIndex.size,
      syncQueueSize: this.syncQueue.length,
      avgQueryLatency: this.totalQueries > 0 ? this.totalLatency / this.totalQueries : 0,
    };
  }

  /**
   * Shutdown agent gracefully
   */
  async shutdown(): Promise<void> {
    console.log(`[RegionalAgent:${this.config.region}] Shutting down agent ${this.config.agentId}...`);

    // Stop timers
    if (this.metricsTimer) {
      clearInterval(this.metricsTimer);
    }
    if (this.syncTimer) {
      clearInterval(this.syncTimer);
    }

    // Process remaining sync queue
    await this.processSyncQueue();

    // Save local index
    await this.saveLocalIndex();

    if (this.config.enableClaudeFlowHooks) {
      try {
        await execAsync(
          `npx claude-flow@alpha hooks post-task --task-id "agent-${this.config.agentId}-shutdown"`
        );
        await execAsync(
          `npx claude-flow@alpha hooks session-end --export-metrics true`
        );
      } catch (error) {
        console.warn(`[RegionalAgent:${this.config.region}] Error executing shutdown hooks:`, error);
      }
    }

    this.emit('agent:shutdown', {
      agentId: this.config.agentId,
      region: this.config.region,
    });
  }

  /**
   * Save local index to persistent storage
   */
  private async saveLocalIndex(): Promise<void> {
    try {
      console.log(`[RegionalAgent:${this.config.region}] Saving local index to ${this.config.localStoragePath}`);

      // Placeholder for actual storage saving
      // In production, this would write to disk/database

      console.log(`[RegionalAgent:${this.config.region}] Local index saved: ${this.localIndex.size} vectors`);
    } catch (error) {
      console.error(`[RegionalAgent:${this.config.region}] Error saving local index:`, error);
      throw error;
    }
  }
}

/**
 * Rate limiter for query processing
 */
class RateLimiter {
  private requests = 0;
  private windowStart = Date.now();

  constructor(
    private config: {
      maxRequests: number;
      windowMs: number;
    }
  ) {}

  tryAcquire(): boolean {
    const now = Date.now();

    // Reset window if expired
    if (now - this.windowStart >= this.config.windowMs) {
      this.requests = 0;
      this.windowStart = now;
    }

    if (this.requests < this.config.maxRequests) {
      this.requests++;
      return true;
    }

    return false;
  }

  release(): void {
    if (this.requests > 0) {
      this.requests--;
    }
  }
}
