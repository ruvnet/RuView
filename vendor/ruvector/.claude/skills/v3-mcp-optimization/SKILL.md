---
name: "V3 MCP Optimization"
description: "MCP server optimization and transport layer enhancement for claude-flow v3. Implements connection pooling, load balancing, tool registry optimization, and performance monitoring for sub-100ms response times."
---

# V3 MCP Optimization

## What This Skill Does

Optimizes claude-flow v3 MCP (Model Context Protocol) server implementation with advanced transport layer optimizations, connection pooling, load balancing, and comprehensive performance monitoring to achieve sub-100ms response times.

## Quick Start

```bash
# Initialize MCP optimization analysis
Task("MCP architecture", "Analyze current MCP server performance and bottlenecks", "mcp-specialist")

# Optimization implementation (parallel)
Task("Connection pooling", "Implement MCP connection pooling and reuse", "mcp-specialist")
Task("Load balancing", "Add dynamic load balancing for MCP tools", "mcp-specialist")
Task("Transport optimization", "Optimize transport layer performance", "mcp-specialist")
```

## MCP Performance Architecture

### Current State Analysis
```
Current MCP Issues:
├── Cold Start Latency: ~1.8s MCP server init
├── Connection Overhead: New connection per request
├── Tool Registry: Linear search O(n) for 213+ tools
├── Transport Layer: No connection reuse
└── Memory Usage: No cleanup of idle connections

Target Performance:
├── Startup Time: <400ms (4.5x improvement)
├── Tool Lookup: <5ms (O(1) hash table)
├── Connection Reuse: 90%+ connection pool hits
├── Response Time: <100ms p95
└── Memory Efficiency: 50% reduction
```

### MCP Server Architecture
```typescript
// src/core/mcp/mcp-server.ts
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';

interface OptimizedMCPConfig {
  // Connection pooling
  maxConnections: number;
  idleTimeoutMs: number;
  connectionReuseEnabled: boolean;

  // Tool registry
  toolCacheEnabled: boolean;
  toolIndexType: 'hash' | 'trie';

  // Performance
  requestTimeoutMs: number;
  batchingEnabled: boolean;
  compressionEnabled: boolean;

  // Monitoring
  metricsEnabled: boolean;
  healthCheckIntervalMs: number;
}

export class OptimizedMCPServer {
  private server: Server;
  private connectionPool: ConnectionPool;
  private toolRegistry: FastToolRegistry;
  private loadBalancer: MCPLoadBalancer;
  private metrics: MCPMetrics;

  constructor(config: OptimizedMCPConfig) {
    this.server = new Server({
      name: 'claude-flow-v3',
      version: '3.0.0'
    }, {
      capabilities: {
        tools: { listChanged: true },
        resources: { subscribe: true, listChanged: true },
        prompts: { listChanged: true }
      }
    });

    this.connectionPool = new ConnectionPool(config);
    this.toolRegistry = new FastToolRegistry(config.toolIndexType);
    this.loadBalancer = new MCPLoadBalancer();
    this.metrics = new MCPMetrics(config.metricsEnabled);
  }

  async start(): Promise<void> {
    // Pre-warm connection pool
    await this.connectionPool.preWarm();

    // Pre-build tool index
    await this.toolRegistry.buildIndex();

    // Setup request handlers with optimizations
    this.setupOptimizedHandlers();

    // Start health monitoring
    this.startHealthMonitoring();

    // Start server
    const transport = new StdioServerTransport();
    await this.server.connect(transport);

    this.metrics.recordStartup();
  }
}
```

## Connection Pool Implementation

### Advanced Connection Pooling
```typescript
// src/core/mcp/connection-pool.ts
interface PooledConnection {
  id: string;
  connection: MCPConnection;
  lastUsed: number;
  usageCount: number;
  isHealthy: boolean;
}

export class ConnectionPool {
  private pool: Map<string, PooledConnection> = new Map();
  private readonly config: ConnectionPoolConfig;
  private healthChecker: HealthChecker;

  constructor(config: ConnectionPoolConfig) {
    this.config = {
      maxConnections: 50,
      minConnections: 5,
      idleTimeoutMs: 300000, // 5 minutes
      maxUsageCount: 1000,
      healthCheckIntervalMs: 30000,
      ...config
    };

    this.healthChecker = new HealthChecker(this.config.healthCheckIntervalMs);
  }

  async getConnection(endpoint: string): Promise<MCPConnection> {
    const start = performance.now();

    // Try to get from pool first
    const pooled = this.findAvailableConnection(endpoint);
    if (pooled) {
      pooled.lastUsed = Date.now();
      pooled.usageCount++;

      this.recordMetric('pool_hit', performance.now() - start);
      return pooled.connection;
    }

    // Check pool capacity
    if (this.pool.size >= this.config.maxConnections) {
      await this.evictLeastUsedConnection();
    }

    // Create new connection
    const connection = await this.createConnection(endpoint);
    const pooledConn: PooledConnection = {
      id: this.generateConnectionId(),
      connection,
      lastUsed: Date.now(),
      usageCount: 1,
      isHealthy: true
    };

    this.pool.set(pooledConn.id, pooledConn);
    this.recordMetric('pool_miss', performance.now() - start);

    return connection;
  }

  async releaseConnection(connection: MCPConnection): Promise<void> {
    // Mark connection as available for reuse
    const pooled = this.findConnectionById(connection.id);
    if (pooled) {
      // Check if connection should be retired
      if (pooled.usageCount >= this.config.maxUsageCount) {
        await this.removeConnection(pooled.id);
      }
    }
  }

  async preWarm(): Promise<void> {
    const connections: Promise<MCPConnection>[] = [];

    for (let i = 0; i < this.config.minConnections; i++) {
      connections.push(this.createConnection('default'));
    }

    await Promise.all(connections);
  }

  private async evictLeastUsedConnection(): Promise<void> {
    let oldestConn: PooledConnection | null = null;
    let oldestTime = Date.now();

    for (const conn of this.pool.values()) {
      if (conn.lastUsed < oldestTime) {
        oldestTime = conn.lastUsed;
        oldestConn = conn;
      }
    }

    if (oldestConn) {
      await this.removeConnection(oldestConn.id);
    }
  }

  private findAvailableConnection(endpoint: string): PooledConnection | null {
    for (const conn of this.pool.values()) {
      if (conn.isHealthy &&
          conn.connection.endpoint === endpoint &&
          Date.now() - conn.lastUsed < this.config.idleTimeoutMs) {
        return conn;
      }
    }
    return null;
  }
}
```

## Fast Tool Registry

### O(1) Tool Lookup Implementation
```typescript
// src/core/mcp/fast-tool-registry.ts
interface ToolIndexEntry {
  name: string;
  handler: ToolHandler;
  metadata: ToolMetadata;
  usageCount: number;
  avgLatencyMs: number;
}

export class FastToolRegistry {
  private toolIndex: Map<string, ToolIndexEntry> = new Map();
  private categoryIndex: Map<string, string[]> = new Map();
  private fuzzyMatcher: FuzzyMatcher;
  private cache: LRUCache<string, ToolIndexEntry>;

  constructor(indexType: 'hash' | 'trie' = 'hash') {
    this.fuzzyMatcher = new FuzzyMatcher();
    this.cache = new LRUCache<string, ToolIndexEntry>(1000); // Cache 1000 most used tools
  }

  async buildIndex(): Promise<void> {
    const start = performance.now();

    // Load all available tools
    const tools = await this.loadAllTools();

    // Build hash index for O(1) lookup
    for (const tool of tools) {
      const entry: ToolIndexEntry = {
        name: tool.name,
        handler: tool.handler,
        metadata: tool.metadata,
        usageCount: 0,
        avgLatencyMs: 0
      };

      this.toolIndex.set(tool.name, entry);

      // Build category index
      const category = tool.metadata.category || 'general';
      if (!this.categoryIndex.has(category)) {
        this.categoryIndex.set(category, []);
      }
      this.categoryIndex.get(category)!.push(tool.name);
    }

    // Build fuzzy search index
    await this.fuzzyMatcher.buildIndex(tools.map(t => t.name));

    console.log(`Tool index built in ${(performance.now() - start).toFixed(2)}ms for ${tools.length} tools`);
  }

  findTool(name: string): ToolIndexEntry | null {
    // Try cache first
    const cached = this.cache.get(name);
    if (cached) return cached;

    // Try exact match
    const exact = this.toolIndex.get(name);
    if (exact) {
      this.cache.set(name, exact);
      return exact;
    }

    // Try fuzzy match
    const fuzzyMatches = this.fuzzyMatcher.search(name, 1);
    if (fuzzyMatches.length > 0) {
      const match = this.toolIndex.get(fuzzyMatches[0]);
      if (match) {
        this.cache.set(name, match);
        return match;
      }
    }

    return null;
  }

  findToolsByCategory(category: string): ToolIndexEntry[] {
    const toolNames = this.categoryIndex.get(category) || [];
    return toolNames
      .map(name => this.toolIndex.get(name))
      .filter(entry => entry !== undefined) as ToolIndexEntry[];
  }

  getMostUsedTools(limit: number = 10): ToolIndexEntry[] {
    return Array.from(this.toolIndex.values())
      .sort((a, b) => b.usageCount - a.usageCount)
      .slice(0, limit);
  }

  recordToolUsage(toolName: string, latencyMs: number): void {
    const entry = this.toolIndex.get(toolName);
    if (entry) {
      entry.usageCount++;
      // Moving average for latency
      entry.avgLatencyMs = (entry.avgLatencyMs + latencyMs) / 2;
    }
  }
}
```

## Load Balancing & Request Distribution

### Intelligent Load Balancer
```typescript
// src/core/mcp/load-balancer.ts
interface ServerInstance {
  id: string;
  endpoint: string;
  load: number;
  responseTime: number;
  isHealthy: boolean;
  maxConnections: number;
  currentConnections: number;
}

export class MCPLoadBalancer {
  private servers: Map<string, ServerInstance> = new Map();
  private routingStrategy: RoutingStrategy = 'least-connections';

  addServer(server: ServerInstance): void {
    this.servers.set(server.id, server);
  }

  selectServer(toolCategory?: string): ServerInstance | null {
    const healthyServers = Array.from(this.servers.values())
      .filter(server => server.isHealthy);

    if (healthyServers.length === 0) return null;

    switch (this.routingStrategy) {
      case 'round-robin':
        return this.roundRobinSelection(healthyServers);

      case 'least-connections':
        return this.leastConnectionsSelection(healthyServers);

      case 'response-time':
        return this.responseTimeSelection(healthyServers);

      case 'weighted':
        return this.weightedSelection(healthyServers, toolCategory);

      default:
        return healthyServers[0];
    }
  }

  private leastConnectionsSelection(servers: ServerInstance[]): ServerInstance {
    return servers.reduce((least, current) =>
      current.currentConnections < least.currentConnections ? current : least
    );
  }

  private responseTimeSelection(servers: ServerInstance[]): ServerInstance {
    return servers.reduce((fastest, current) =>
      current.responseTime < fastest.responseTime ? current : fastest
    );
  }

  private weightedSelection(servers: ServerInstance[], category?: string): ServerInstance {
    // Prefer servers with lower load and better response time
    const scored = servers.map(server => ({
      server,
      score: this.calculateServerScore(server, category)
    }));

    scored.sort((a, b) => b.score - a.score);
    return scored[0].server;
  }

  private calculateServerScore(server: ServerInstance, category?: string): number {
    const loadFactor = 1 - (server.currentConnections / server.maxConnections);
    const responseFactor = 1 / (server.responseTime + 1);
    const categoryBonus = this.getCategoryBonus(server, category);

    return loadFactor * 0.4 + responseFactor * 0.4 + categoryBonus * 0.2;
  }

  updateServerMetrics(serverId: string, metrics: Partial<ServerInstance>): void {
    const server = this.servers.get(serverId);
    if (server) {
      Object.assign(server, metrics);
    }
  }
}
```

## Transport Layer Optimization

### High-Performance Transport
```typescript
// src/core/mcp/optimized-transport.ts
export class OptimizedTransport {
  private compression: boolean = true;
  private batching: boolean = true;
  private batchBuffer: MCPMessage[] = [];
  private batchTimeout: NodeJS.Timeout | null = null;

  constructor(private config: TransportConfig) {}

  async send(message: MCPMessage): Promise<void> {
    if (this.batching && this.canBatch(message)) {
      this.addToBatch(message);
      return;
    }

    await this.sendImmediate(message);
  }

  private async sendImmediate(message: MCPMessage): Promise<void> {
    const start = performance.now();

    // Compress if enabled
    const payload = this.compression
      ? await this.compress(message)
      : message;

    // Send through transport
    await this.transport.send(payload);

    // Record metrics
    this.recordLatency(performance.now() - start);
  }

  private addToBatch(message: MCPMessage): void {
    this.batchBuffer.push(message);

    // Start batch timeout if not already running
    if (!this.batchTimeout) {
      this.batchTimeout = setTimeout(
        () => this.flushBatch(),
        this.config.batchTimeoutMs || 10
      );
    }

    // Flush if batch is full
    if (this.batchBuffer.length >= this.config.maxBatchSize) {
      this.flushBatch();
    }
  }

  private async flushBatch(): Promise<void> {
    if (this.batchBuffer.length === 0) return;

    const batch = this.batchBuffer.splice(0);
    this.batchTimeout = null;

    // Send as single batched message
    await this.sendImmediate({
      type: 'batch',
      messages: batch
    });
  }

  private canBatch(message: MCPMessage): boolean {
    // Don't batch urgent messages or responses
    return message.type !== 'response' &&
           message.priority !== 'high' &&
           message.type !== 'error';
  }

  private async compress(data: any): Promise<Buffer> {
    // Use fast compression for smaller messages
    return gzipSync(JSON.stringify(data));
  }
}
```

## Performance Monitoring

### Real-time MCP Metrics
```typescript
// src/core/mcp/metrics.ts
interface MCPMetrics {
  requestCount: number;
  errorCount: number;
  avgResponseTime: number;
  p95ResponseTime: number;
  connectionPoolHits: number;
  connectionPoolMisses: number;
  toolLookupTime: number;
  startupTime: number;
}

export class MCPMetricsCollector {
  private metrics: MCPMetrics;
  private responseTimeBuffer: number[] = [];
  private readonly bufferSize = 1000;

  constructor() {
    this.metrics = this.createInitialMetrics();
  }

  recordRequest(latencyMs: number): void {
    this.metrics.requestCount++;
    this.updateResponseTimes(latencyMs);
  }

  recordError(): void {
    this.metrics.errorCount++;
  }

  recordConnectionPoolHit(): void {
    this.metrics.connectionPoolHits++;
  }

  recordConnectionPoolMiss(): void {
    this.metrics.connectionPoolMisses++;
  }

  recordToolLookup(latencyMs: number): void {
    this.metrics.toolLookupTime = this.updateMovingAverage(
      this.metrics.toolLookupTime,
      latencyMs
    );
  }

  recordStartup(latencyMs: number): void {
    this.metrics.startupTime = latencyMs;
  }

  getMetrics(): MCPMetrics {
    return { ...this.metrics };
  }

  getHealthStatus(): HealthStatus {
    const errorRate = this.metrics.errorCount / this.metrics.requestCount;
    const poolHitRate = this.metrics.connectionPoolHits /
      (this.metrics.connectionPoolHits + this.metrics.connectionPoolMisses);

    return {
      status: this.determineHealthStatus(errorRate, poolHitRate),
      errorRate,
      poolHitRate,
      avgResponseTime: this.metrics.avgResponseTime,
      p95ResponseTime: this.metrics.p95ResponseTime
    };
  }

  private updateResponseTimes(latency: number): void {
    this.responseTimeBuffer.push(latency);

    if (this.responseTimeBuffer.length > this.bufferSize) {
      this.responseTimeBuffer.shift();
    }

    this.metrics.avgResponseTime = this.calculateAverage(this.responseTimeBuffer);
    this.metrics.p95ResponseTime = this.calculatePercentile(this.responseTimeBuffer, 95);
  }

  private calculatePercentile(arr: number[], percentile: number): number {
    const sorted = arr.slice().sort((a, b) => a - b);
    const index = Math.ceil((percentile / 100) * sorted.length) - 1;
    return sorted[index] || 0;
  }

  private determineHealthStatus(errorRate: number, poolHitRate: number): 'healthy' | 'warning' | 'critical' {
    if (errorRate > 0.1 || poolHitRate < 0.5) return 'critical';
    if (errorRate > 0.05 || poolHitRate < 0.7) return 'warning';
    return 'healthy';
  }
}
```

## Tool Registry Optimization

### Pre-compiled Tool Index
```typescript
// src/core/mcp/tool-precompiler.ts
export class ToolPrecompiler {
  async precompileTools(): Promise<CompiledToolRegistry> {
    const tools = await this.loadAllTools();

    // Create optimized lookup structures
    const nameIndex = new Map<string, Tool>();
    const categoryIndex = new Map<string, Tool[]>();
    const fuzzyIndex = new Map<string, string[]>();

    for (const tool of tools) {
      // Exact name index
      nameIndex.set(tool.name, tool);

      // Category index
      const category = tool.metadata.category || 'general';
      if (!categoryIndex.has(category)) {
        categoryIndex.set(category, []);
      }
      categoryIndex.get(category)!.push(tool);

      // Pre-compute fuzzy variations
      const variations = this.generateFuzzyVariations(tool.name);
      for (const variation of variations) {
        if (!fuzzyIndex.has(variation)) {
          fuzzyIndex.set(variation, []);
        }
        fuzzyIndex.get(variation)!.push(tool.name);
      }
    }

    return {
      nameIndex,
      categoryIndex,
      fuzzyIndex,
      totalTools: tools.length,
      compiledAt: new Date()
    };
  }

  private generateFuzzyVariations(name: string): string[] {
    const variations: string[] = [];

    // Common typos and abbreviations
    variations.push(name.toLowerCase());
    variations.push(name.replace(/[-_]/g, ''));
    variations.push(name.replace(/[aeiou]/gi, '')); // Consonants only

    // Add more fuzzy matching logic as needed

    return variations;
  }
}
```

## Advanced Caching Strategy

### Multi-Level Caching
```typescript
// src/core/mcp/multi-level-cache.ts
export class MultiLevelCache {
  private l1Cache: Map<string, any> = new Map(); // In-memory, fastest
  private l2Cache: LRUCache<string, any>; // LRU cache, larger capacity
  private l3Cache: DiskCache; // Persistent disk cache

  constructor(config: CacheConfig) {
    this.l2Cache = new LRUCache<string, any>({
      max: config.l2MaxEntries || 10000,
      ttl: config.l2TTL || 300000 // 5 minutes
    });

    this.l3Cache = new DiskCache(config.l3Path || './.cache/mcp');
  }

  async get(key: string): Promise<any | null> {
    // Try L1 cache first (fastest)
    if (this.l1Cache.has(key)) {
      return this.l1Cache.get(key);
    }

    // Try L2 cache
    const l2Value = this.l2Cache.get(key);
    if (l2Value) {
      // Promote to L1
      this.l1Cache.set(key, l2Value);
      return l2Value;
    }

    // Try L3 cache (disk)
    const l3Value = await this.l3Cache.get(key);
    if (l3Value) {
      // Promote to L2 and L1
      this.l2Cache.set(key, l3Value);
      this.l1Cache.set(key, l3Value);
      return l3Value;
    }

    return null;
  }

  async set(key: string, value: any, options?: CacheOptions): Promise<void> {
    // Set in all levels
    this.l1Cache.set(key, value);
    this.l2Cache.set(key, value);

    if (options?.persistent) {
      await this.l3Cache.set(key, value);
    }

    // Manage L1 cache size
    if (this.l1Cache.size > 1000) {
      const firstKey = this.l1Cache.keys().next().value;
      this.l1Cache.delete(firstKey);
    }
  }
}
```

## Success Metrics

### Performance Targets
- [ ] **Startup Time**: <400ms MCP server initialization (4.5x improvement)
- [ ] **Response Time**: <100ms p95 for tool execution
- [ ] **Tool Lookup**: <5ms average lookup time
- [ ] **Connection Pool**: >90% hit rate
- [ ] **Memory Usage**: 50% reduction in idle memory
- [ ] **Error Rate**: <1% failed requests
- [ ] **Throughput**: >1000 requests/second

### Monitoring Dashboards
```typescript
const mcpDashboard = {
  metrics: [
    'Request latency (p50, p95, p99)',
    'Error rate by tool category',
    'Connection pool utilization',
    'Tool lookup performance',
    'Memory usage trends',
    'Cache hit rates (L1, L2, L3)'
  ],

  alerts: [
    'Response time >200ms for 5 minutes',
    'Error rate >5% for 1 minute',
    'Pool hit rate <70% for 10 minutes',
    'Memory usage >500MB for 5 minutes'
  ]
};
```

## Related V3 Skills

- `v3-core-implementation` - Core domain integration with MCP
- `v3-performance-optimization` - Overall performance optimization
- `v3-swarm-coordination` - MCP integration with swarm coordination
- `v3-memory-unification` - Memory sharing via MCP tools

## Usage Examples

### Complete MCP Optimization
```bash
# Full MCP server optimization
Task("MCP optimization implementation",
     "Implement all MCP performance optimizations with monitoring",
     "mcp-specialist")
```

### Specific Optimization
```bash
# Connection pool optimization
Task("MCP connection pooling",
     "Implement advanced connection pooling with health monitoring",
     "mcp-specialist")
```