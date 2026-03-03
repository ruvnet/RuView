# Flow-Nexus HTTP Streaming & Swarm Integration Plan

## Executive Summary

This plan outlines the integration of the sublinear-time solver with Flow-Nexus platform for real-time HTTP streaming, swarm-based cost propagation, and distributed verification. The architecture enables dynamic solver optimization through continuous cost updates and verification loops while maintaining sub-millisecond latency targets.

## 1. HTTP Streaming Architecture

### 1.1 Core Endpoints

```
/api/v1/solve-stream    - Primary streaming solver endpoint
/api/v1/verify         - Verification probe endpoint
/api/v1/status         - Session status and metrics
/api/v1/swarm/join     - Swarm participation endpoint
/api/v1/swarm/costs    - Cost propagation endpoint
/api/v1/health         - Health check for load balancers
```

### 1.2 Streaming Protocol Design

**Newline-Delimited JSON (NDJSON)**
- Each line is a complete JSON object
- Enables incremental parsing
- Supports backpressure handling
- Compatible with standard HTTP/1.1 and HTTP/2

**Connection Patterns:**
```javascript
// Server-Sent Events for web clients
Accept: text/event-stream

// Raw NDJSON for programmatic access
Accept: application/x-ndjson

// WebSocket upgrade for bidirectional
Upgrade: websocket
```

### 1.3 WebSocket vs HTTP Streaming Comparison

| Feature | HTTP Streaming | WebSocket |
|---------|----------------|-----------|
| Latency | ~2-5ms | ~1-2ms |
| Browser Support | Universal | IE10+ |
| Proxy Compatibility | Excellent | Good |
| Backpressure | Built-in | Manual |
| Reconnection | Automatic | Manual |
| **Recommendation** | Primary | Fallback |

### 1.4 Connection Persistence Strategy

```javascript
// Connection pool configuration
const connectionConfig = {
  maxConcurrent: 1000,        // Maximum concurrent streams
  keepAliveTimeout: 60000,    // 60 seconds
  heartbeatInterval: 15000,   // 15 second ping
  maxSessionDuration: 3600000, // 1 hour max
  gracefulShutdown: 30000     // 30 second drain
};
```

### 1.5 Multi-Session Management

```javascript
// Session state structure
class SolverSession {
  sessionId: string;          // UUID
  matrix: SparseMatrix;       // Problem definition
  currentSolution: Vector;    // Latest x vector
  swarmNodes: Set<string>;    // Connected nodes
  verificationState: VerificationLoop;
  metrics: PerformanceMetrics;
  createdAt: timestamp;
  lastActivity: timestamp;
}
```

## 2. Request/Response Schemas

### 2.1 Initial Solve Request

```json
{
  "type": "solve_request",
  "session_id": "uuid-v4",
  "matrix": {
    "rows": 10000,
    "cols": 10000,
    "nnz": 50000,
    "data": {
      "values": [1.5, 2.3, ...],
      "row_indices": [0, 1, 2, ...],
      "col_pointers": [0, 3, 7, ...]
    },
    "format": "csr"
  },
  "vector": [1.0, 2.0, 3.0, ...],
  "method": "hybrid",
  "options": {
    "tolerance": 1e-8,
    "max_iterations": 10000,
    "preconditioner": "jacobi",
    "swarm_enabled": true,
    "verification_interval": 100
  },
  "flow_nexus": {
    "swarm_topology": "mesh",
    "cost_aggregation": "weighted_average",
    "verification_strategy": "random_probe"
  }
}
```

### 2.2 Cost Update Stream (Input)

```json
{
  "type": "cost_update",
  "session_id": "uuid-v4",
  "timestamp": "2025-09-19T19:26:51.605Z",
  "delta_costs": {
    "indices": [100, 205, 1337],
    "values": [0.01, -0.003, 0.007]
  },
  "matrix_updates": [
    {
      "row": 100,
      "col": 205,
      "old_value": 1.5,
      "new_value": 1.51
    }
  ],
  "source_node": "node-abc123",
  "propagation_depth": 3
}
```

### 2.3 Solution Response Stream (Output)

```json
{
  "type": "iteration_update",
  "session_id": "uuid-v4",
  "iteration": 42,
  "timestamp": "2025-09-19T19:26:51.605Z",
  "x_partial": {
    "indices": [0, 1, 2],
    "values": [1.001, 2.003, 3.007]
  },
  "residual_norm": 0.001,
  "convergence_rate": 0.95,
  "verified": true,
  "verification_score": 0.999,
  "swarm_consensus": {
    "participating_nodes": 5,
    "agreement_level": 0.98
  },
  "performance": {
    "iteration_time_ms": 0.8,
    "memory_usage_mb": 150.2,
    "flops": 1500000
  }
}
```

### 2.4 Verification Response

```json
{
  "type": "verification_result",
  "session_id": "uuid-v4",
  "probe_id": "probe-xyz789",
  "timestamp": "2025-09-19T19:26:51.605Z",
  "verified": true,
  "residual_components": [0.001, 0.0008, 0.0012],
  "drift_detected": false,
  "correction_applied": false,
  "verification_time_ms": 0.3
}
```

## 3. Swarm Cost Propagation

### 3.1 Agent Cost Reporting Protocol

```javascript
class SwarmCostReporter {
  async reportCosts(sessionId, localCosts) {
    const report = {
      type: 'cost_report',
      session_id: sessionId,
      node_id: this.nodeId,
      timestamp: Date.now(),
      local_costs: localCosts,
      computation_confidence: this.getConfidence(),
      network_latency: this.measureLatency()
    };

    await this.broadcast(report);
  }
}
```

### 3.2 Incremental Update Processing

```javascript
class IncrementalProcessor {
  processUpdate(costUpdate) {
    // Apply delta to local cost matrix
    this.applyCostDelta(costUpdate.delta_costs);

    // Update solver state incrementally
    this.solver.updateCosts(costUpdate.indices, costUpdate.values);

    // Trigger recomputation if significant change
    if (this.detectSignificantChange(costUpdate)) {
      this.triggerRecomputation();
    }

    // Propagate to swarm neighbors
    this.propagateToNeighbors(costUpdate);
  }
}
```

### 3.3 Convergence Signaling

```javascript
// Convergence detection algorithm
const convergenceSignal = {
  type: 'convergence_signal',
  criteria: {
    residual_threshold: 1e-8,
    consecutive_iterations: 10,
    swarm_agreement: 0.95
  },
  current_state: {
    residual_norm: 5.2e-9,
    iterations_below_threshold: 12,
    swarm_consensus: 0.97
  },
  converged: true
};
```

### 3.4 Distributed Verification

```javascript
class DistributedVerifier {
  async verifyWithSwarm(solution, sessionId) {
    const verificationTasks = this.generateRandomProbes(solution);

    const promises = this.swarmNodes.map(node =>
      node.verify(verificationTasks[node.id])
    );

    const results = await Promise.allSettled(promises);
    return this.aggregateVerificationResults(results);
  }
}
```

### 3.5 Consensus Mechanisms

```javascript
// Byzantine fault tolerant consensus
class ByzantineConsensus {
  async reachConsensus(proposals) {
    const votes = await this.collectVotes(proposals);
    const validated = this.validateVotes(votes);

    // Require 2/3 + 1 agreement for Byzantine tolerance
    const threshold = Math.floor(this.swarmSize * 2/3) + 1;

    return this.selectConsensusValue(validated, threshold);
  }
}
```

## 4. Node.js Server Implementation

### 4.1 Express/Fastify Setup

```javascript
const fastify = require('fastify')({
  logger: true,
  keepAliveTimeout: 60000,
  bodyLimit: 50 * 1024 * 1024 // 50MB for large matrices
});

// Register streaming plugin
await fastify.register(require('./plugins/streaming'));
await fastify.register(require('./plugins/swarm'));
await fastify.register(require('./plugins/verification'));

// CORS for web clients
await fastify.register(require('@fastify/cors'), {
  origin: true,
  credentials: true
});

// Rate limiting
await fastify.register(require('@fastify/rate-limit'), {
  max: 100,
  timeWindow: '1 minute'
});
```

### 4.2 Stream Handling with Backpressure

```javascript
class StreamHandler {
  constructor(request, reply) {
    this.request = request;
    this.reply = reply;
    this.buffer = [];
    this.draining = false;
  }

  async writeUpdate(update) {
    const data = JSON.stringify(update) + '\n';

    if (this.reply.writable) {
      const canContinue = this.reply.raw.write(data);

      if (!canContinue && !this.draining) {
        this.draining = true;
        await new Promise(resolve => {
          this.reply.raw.once('drain', () => {
            this.draining = false;
            resolve();
          });
        });
      }
    }
  }
}
```

### 4.3 WASM Solver Integration

```javascript
const WasmSolver = require('./wasm/solver.js');

class SolverManager {
  constructor() {
    this.wasmModule = null;
    this.solverInstances = new Map();
  }

  async initialize() {
    this.wasmModule = await WasmSolver();
  }

  createSolver(sessionId, matrix, vector, options) {
    const solver = new this.wasmModule.SublinearSolver(
      matrix.data, matrix.rows, matrix.cols,
      vector, options
    );

    this.solverInstances.set(sessionId, solver);
    return solver;
  }
}
```

### 4.4 Session State Management

```javascript
class SessionManager {
  constructor() {
    this.sessions = new Map();
    this.redis = new Redis(process.env.REDIS_URL);
  }

  async createSession(sessionId, config) {
    const session = new SolverSession(sessionId, config);
    this.sessions.set(sessionId, session);

    // Persist to Redis for clustering
    await this.redis.setex(
      `session:${sessionId}`,
      3600,
      JSON.stringify(session.serialize())
    );

    return session;
  }

  async restoreSession(sessionId) {
    const data = await this.redis.get(`session:${sessionId}`);
    if (data) {
      const session = SolverSession.deserialize(JSON.parse(data));
      this.sessions.set(sessionId, session);
      return session;
    }
    return null;
  }
}
```

### 4.5 Concurrent Request Handling

```javascript
// Worker pool for CPU-intensive tasks
const { Worker, isMainThread, parentPort } = require('worker_threads');

class WorkerPool {
  constructor(size = require('os').cpus().length) {
    this.workers = [];
    this.queue = [];
    this.activeJobs = new Map();

    for (let i = 0; i < size; i++) {
      this.createWorker();
    }
  }

  async execute(task) {
    return new Promise((resolve, reject) => {
      const job = { task, resolve, reject, id: generateId() };

      const worker = this.getAvailableWorker();
      if (worker) {
        this.assignJob(worker, job);
      } else {
        this.queue.push(job);
      }
    });
  }
}
```

## 5. Verification Loop Design

### 5.1 Random Probe Verification

```javascript
class RandomProbeVerifier {
  generateProbes(solution, count = 10) {
    const probes = [];
    const n = solution.length;

    for (let i = 0; i < count; i++) {
      const indices = this.selectRandomIndices(n, Math.min(100, n));
      probes.push({
        id: generateId(),
        indices: indices,
        expected_values: indices.map(idx => solution[idx])
      });
    }

    return probes;
  }

  async verifyProbe(probe, matrix, vector) {
    // Compute Ax for probe indices
    const computed = this.computeMatrixVectorProduct(
      matrix, probe.indices, this.currentSolution
    );

    // Compare with expected vector values
    const errors = probe.indices.map((idx, i) =>
      Math.abs(computed[i] - vector[idx])
    );

    return {
      probe_id: probe.id,
      max_error: Math.max(...errors),
      mean_error: errors.reduce((a, b) => a + b) / errors.length,
      verified: Math.max(...errors) < this.tolerance
    };
  }
}
```

### 5.2 Residual Norm Tracking

```javascript
class ResidualTracker {
  constructor(historySize = 1000) {
    this.history = [];
    this.historySize = historySize;
    this.trendAnalyzer = new TrendAnalyzer();
  }

  addResidual(norm, iteration) {
    this.history.push({ norm, iteration, timestamp: Date.now() });

    if (this.history.length > this.historySize) {
      this.history.shift();
    }

    // Analyze convergence trend
    return this.trendAnalyzer.analyze(this.history);
  }

  detectStagnation(windowSize = 50) {
    if (this.history.length < windowSize) return false;

    const recent = this.history.slice(-windowSize);
    const variance = this.calculateVariance(recent.map(h => h.norm));

    // Stagnation if variance is very low
    return variance < 1e-12;
  }
}
```

### 5.3 Drift Detection Algorithms

```javascript
class DriftDetector {
  constructor() {
    this.baseline = null;
    this.alertThreshold = 0.1; // 10% drift threshold
    this.ewmaAlpha = 0.1;      // Exponential smoothing
    this.ewmaValue = 0;
  }

  detectDrift(currentMetrics) {
    if (!this.baseline) {
      this.baseline = currentMetrics;
      return { driftDetected: false, severity: 0 };
    }

    // Update EWMA
    this.ewmaValue = this.ewmaAlpha * currentMetrics.residual_norm +
                     (1 - this.ewmaAlpha) * this.ewmaValue;

    // Calculate drift magnitude
    const drift = Math.abs(this.ewmaValue - this.baseline.residual_norm) /
                  this.baseline.residual_norm;

    return {
      driftDetected: drift > this.alertThreshold,
      severity: drift,
      recommendation: drift > 0.5 ? 'restart' : 'adjust'
    };
  }
}
```

### 5.4 Auto-Correction Triggers

```javascript
class AutoCorrector {
  constructor(solver) {
    this.solver = solver;
    this.correctionStrategies = {
      'drift': this.handleDrift.bind(this),
      'stagnation': this.handleStagnation.bind(this),
      'divergence': this.handleDivergence.bind(this)
    };
  }

  async handleDrift(severity) {
    if (severity < 0.2) {
      // Minor drift - adjust step size
      await this.solver.adjustStepSize(0.8);
    } else if (severity < 0.5) {
      // Moderate drift - restart with better preconditioner
      await this.solver.restart({ preconditioner: 'ilu' });
    } else {
      // Major drift - full restart
      await this.solver.fullRestart();
    }
  }
}
```

### 5.5 Verification Scheduling

```javascript
class VerificationScheduler {
  constructor() {
    this.schedule = new Map();
    this.adaptiveInterval = true;
  }

  scheduleVerification(sessionId, initialInterval = 100) {
    const schedule = {
      interval: initialInterval,
      lastVerified: 0,
      adaptiveMultiplier: 1.0
    };

    this.schedule.set(sessionId, schedule);
  }

  shouldVerify(sessionId, currentIteration) {
    const schedule = this.schedule.get(sessionId);
    if (!schedule) return false;

    const nextVerification = schedule.lastVerified +
                           (schedule.interval * schedule.adaptiveMultiplier);

    return currentIteration >= nextVerification;
  }

  adaptInterval(sessionId, verificationResult) {
    const schedule = this.schedule.get(sessionId);

    if (verificationResult.verified) {
      // Increase interval if consistently verified
      schedule.adaptiveMultiplier = Math.min(2.0, schedule.adaptiveMultiplier * 1.1);
    } else {
      // Decrease interval if verification fails
      schedule.adaptiveMultiplier = Math.max(0.5, schedule.adaptiveMultiplier * 0.8);
    }
  }
}
```

## 6. Flow-Nexus MCP Integration

### 6.1 Tool Registration Format

```json
{
  "tools": [
    {
      "name": "sublinear_solver_stream",
      "description": "Stream-based sublinear time matrix solver",
      "inputSchema": {
        "type": "object",
        "properties": {
          "matrix": { "$ref": "#/definitions/SparseMatrix" },
          "vector": { "type": "array", "items": { "type": "number" } },
          "method": { "enum": ["jacobi", "gauss_seidel", "cg", "hybrid"] },
          "stream_options": {
            "type": "object",
            "properties": {
              "real_time": { "type": "boolean", "default": true },
              "verification_enabled": { "type": "boolean", "default": true },
              "swarm_coordination": { "type": "boolean", "default": false }
            }
          }
        }
      }
    },
    {
      "name": "solver_verification",
      "description": "Verify solution accuracy with random probes",
      "inputSchema": {
        "type": "object",
        "properties": {
          "session_id": { "type": "string" },
          "probe_count": { "type": "integer", "minimum": 1, "maximum": 100 }
        }
      }
    }
  ]
}
```

### 6.2 Authentication Handling

```javascript
class FlowNexusAuth {
  constructor() {
    this.tokenCache = new Map();
    this.refreshThreshold = 300000; // 5 minutes
  }

  async authenticate(request) {
    const token = this.extractToken(request);
    if (!token) {
      throw new Error('No authentication token provided');
    }

    const cached = this.tokenCache.get(token);
    if (cached && (Date.now() - cached.timestamp) < this.refreshThreshold) {
      return cached.user;
    }

    // Validate with Flow-Nexus
    const user = await this.validateWithFlowNexus(token);
    this.tokenCache.set(token, { user, timestamp: Date.now() });

    return user;
  }
}
```

### 6.3 Rate Limiting Strategies

```javascript
const rateLimitConfig = {
  // Per-user limits
  user: {
    requests: 1000,     // requests per window
    window: 3600000,    // 1 hour
    concurrent: 10      // concurrent streams
  },

  // Per-IP limits (DDoS protection)
  ip: {
    requests: 100,
    window: 300000,     // 5 minutes
    concurrent: 5
  },

  // Global system limits
  global: {
    concurrent_sessions: 10000,
    memory_limit_gb: 100,
    cpu_utilization_max: 0.8
  }
};
```

### 6.4 Monitoring Hooks

```javascript
class MonitoringHooks {
  constructor() {
    this.metrics = new Map();
    this.alertManager = new AlertManager();
  }

  async onSessionStart(sessionId, config) {
    await this.recordEvent('session_start', {
      session_id: sessionId,
      matrix_size: config.matrix.rows,
      method: config.method
    });
  }

  async onIterationComplete(sessionId, iteration, metrics) {
    await this.recordMetric('iteration_time', metrics.duration, {
      session_id: sessionId,
      iteration: iteration
    });

    // Check for performance alerts
    if (metrics.duration > 10) { // 10ms threshold
      await this.alertManager.sendAlert('slow_iteration', metrics);
    }
  }
}
```

### 6.5 Error Reporting

```javascript
class ErrorReporter {
  constructor() {
    this.errorBuffer = [];
    this.reportingInterval = 60000; // 1 minute
    this.setupPeriodicReporting();
  }

  reportError(error, context) {
    const errorReport = {
      timestamp: Date.now(),
      error: {
        message: error.message,
        stack: error.stack,
        type: error.constructor.name
      },
      context: {
        session_id: context.sessionId,
        operation: context.operation,
        user_id: context.userId
      },
      severity: this.assessSeverity(error)
    };

    this.errorBuffer.push(errorReport);

    // Immediate reporting for critical errors
    if (errorReport.severity === 'critical') {
      this.sendImmediateReport(errorReport);
    }
  }
}
```

## 7. Performance & Reliability

### 7.1 Latency Targets

| Operation | Target | Maximum | SLA |
|-----------|--------|---------|-----|
| Initial connection | <100ms | 500ms | 99.9% |
| Iteration update | <1ms | 5ms | 99.5% |
| Cost propagation | <2ms | 10ms | 99.0% |
| Verification probe | <5ms | 20ms | 98.0% |
| Swarm consensus | <10ms | 50ms | 95.0% |

### 7.2 Throughput Benchmarks

```javascript
const performanceTargets = {
  concurrent_sessions: 1000,
  iterations_per_second: 10000,
  matrix_updates_per_second: 50000,
  verification_probes_per_second: 1000,
  network_bandwidth_mbps: 1000
};

// Load testing configuration
const loadTest = {
  ramp_up_duration: 60,      // 1 minute
  steady_state_duration: 300, // 5 minutes
  max_virtual_users: 1000,
  scenarios: [
    {
      name: 'heavy_computation',
      matrix_size: 10000,
      users: 100
    },
    {
      name: 'light_streaming',
      matrix_size: 1000,
      users: 500
    }
  ]
};
```

### 7.3 Failover Strategies

```javascript
class FailoverManager {
  constructor() {
    this.primaryNodes = new Set();
    this.backupNodes = new Set();
    this.sessionMigration = new SessionMigration();
  }

  async handleNodeFailure(failedNode) {
    // Identify affected sessions
    const affectedSessions = await this.getSessionsOnNode(failedNode);

    // Migrate sessions to healthy nodes
    const migrations = affectedSessions.map(session =>
      this.sessionMigration.migrate(session, this.selectHealthyNode())
    );

    await Promise.all(migrations);

    // Update load balancer
    await this.updateLoadBalancerConfig();

    // Notify monitoring
    await this.notifyFailover(failedNode, affectedSessions.length);
  }
}
```

### 7.4 Memory Leak Prevention

```javascript
class MemoryManager {
  constructor() {
    this.sessionCleanup = new Map();
    this.gcInterval = 300000; // 5 minutes
    this.setupPeriodicCleanup();
  }

  trackSession(sessionId) {
    this.sessionCleanup.set(sessionId, {
      lastActivity: Date.now(),
      memorySnapshot: process.memoryUsage()
    });
  }

  async cleanupStaleData() {
    const now = Date.now();
    const staleThreshold = 3600000; // 1 hour

    for (const [sessionId, data] of this.sessionCleanup) {
      if (now - data.lastActivity > staleThreshold) {
        await this.cleanupSession(sessionId);
        this.sessionCleanup.delete(sessionId);
      }
    }

    // Force garbage collection if memory usage is high
    const memUsage = process.memoryUsage();
    if (memUsage.heapUsed > 1024 * 1024 * 1024) { // 1GB
      global.gc && global.gc();
    }
  }
}
```

### 7.5 Connection Pooling

```javascript
class ConnectionPool {
  constructor(config) {
    this.maxConnections = config.maxConnections || 1000;
    this.minConnections = config.minConnections || 10;
    this.activeConnections = new Set();
    this.idleConnections = [];
    this.waitingQueue = [];
  }

  async acquireConnection() {
    if (this.idleConnections.length > 0) {
      const conn = this.idleConnections.pop();
      this.activeConnections.add(conn);
      return conn;
    }

    if (this.activeConnections.size < this.maxConnections) {
      const conn = await this.createConnection();
      this.activeConnections.add(conn);
      return conn;
    }

    // Wait for available connection
    return new Promise((resolve) => {
      this.waitingQueue.push(resolve);
    });
  }
}
```

## 8. Example Flow-Nexus Workflow YAML

```yaml
apiVersion: flow-nexus.ruv.io/v1
kind: SolverWorkflow
metadata:
  name: distributed-sublinear-solver
  description: "Distributed matrix solving with real-time streaming"
spec:
  topology: mesh
  agents:
    - name: primary-solver
      type: solver
      replicas: 3
      resources:
        cpu: "2000m"
        memory: "4Gi"
        wasm: true

    - name: verifier
      type: verifier
      replicas: 2
      resources:
        cpu: "500m"
        memory: "1Gi"

    - name: cost-aggregator
      type: coordinator
      replicas: 1
      resources:
        cpu: "1000m"
        memory: "2Gi"

  streaming:
    protocol: ndjson
    compression: gzip
    heartbeat_interval: 15s
    max_session_duration: 1h

  verification:
    enabled: true
    interval: 100
    probe_count: 10
    tolerance: 1e-8
    auto_correction: true

  swarm:
    cost_propagation: true
    consensus_mechanism: byzantine
    fault_tolerance: 0.33
    network_partition_handling: true

  monitoring:
    metrics_endpoint: "/metrics"
    health_check: "/health"
    log_level: "info"
    tracing: true

  authentication:
    type: bearer_token
    flow_nexus_integration: true
    rate_limiting:
      requests_per_hour: 10000
      concurrent_sessions: 100
```

## 9. Curl Test Commands

### 9.1 Basic Solver Stream Test

```bash
# Start a streaming solve session
curl -X POST http://localhost:3000/api/v1/solve-stream \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $FLOW_NEXUS_TOKEN" \
  -d '{
    "matrix": {
      "rows": 1000,
      "cols": 1000,
      "data": {
        "values": [2, -1, -1, 2, -1, -1, 2],
        "row_indices": [0, 0, 1, 1, 1, 2, 2],
        "col_pointers": [0, 2, 5, 7]
      },
      "format": "csr"
    },
    "vector": [1, 2, 3],
    "method": "hybrid",
    "options": {
      "tolerance": 1e-8,
      "swarm_enabled": true
    }
  }' \
  --no-buffer | while read line; do
    echo "$(date): $line"
  done
```

### 9.2 Cost Update Stream

```bash
# Send cost updates to running session
curl -X POST http://localhost:3000/api/v1/swarm/costs \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $FLOW_NEXUS_TOKEN" \
  -d '{
    "session_id": "uuid-session-123",
    "delta_costs": {
      "indices": [100, 205],
      "values": [0.01, -0.003]
    },
    "source_node": "node-abc123"
  }'
```

### 9.3 Verification Test

```bash
# Trigger manual verification
curl -X POST http://localhost:3000/api/v1/verify \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $FLOW_NEXUS_TOKEN" \
  -d '{
    "session_id": "uuid-session-123",
    "probe_count": 20
  }'
```

### 9.4 Session Status Check

```bash
# Get session status and metrics
curl -X GET http://localhost:3000/api/v1/status/uuid-session-123 \
  -H "Authorization: Bearer $FLOW_NEXUS_TOKEN" | jq '.'
```

### 9.5 WebSocket Test

```bash
# WebSocket connection test using websocat
echo '{
  "type": "subscribe",
  "session_id": "uuid-session-123"
}' | websocat ws://localhost:3000/ws/solver
```

### 9.6 Load Testing with Artillery

```yaml
# artillery-config.yml
config:
  target: 'http://localhost:3000'
  phases:
    - duration: 60
      arrivalRate: 10
      name: "Warm up"
    - duration: 300
      arrivalRate: 50
      name: "Load test"
  processor: "./custom-functions.js"

scenarios:
  - name: "Streaming solver test"
    weight: 80
    flow:
      - post:
          url: "/api/v1/solve-stream"
          headers:
            Authorization: "Bearer {{ auth_token }}"
          json:
            matrix: "{{ matrix_1000x1000 }}"
            vector: "{{ vector_1000 }}"
            method: "hybrid"

  - name: "Cost updates"
    weight: 20
    flow:
      - post:
          url: "/api/v1/swarm/costs"
          json:
            session_id: "{{ session_id }}"
            delta_costs:
              indices: [1, 2, 3]
              values: [0.001, 0.002, 0.003]

# Run load test
artillery run artillery-config.yml
```

## 10. Implementation Timeline

### Phase 1: Core HTTP Streaming (Weeks 1-2)
- [ ] Basic Express/Fastify server setup
- [ ] NDJSON streaming implementation
- [ ] Session management
- [ ] WASM solver integration
- [ ] Basic authentication

### Phase 2: Verification System (Weeks 3-4)
- [ ] Random probe verification
- [ ] Residual tracking
- [ ] Drift detection
- [ ] Auto-correction mechanisms
- [ ] Verification scheduling

### Phase 3: Swarm Integration (Weeks 5-6)
- [ ] Cost propagation protocols
- [ ] Consensus mechanisms
- [ ] Distributed verification
- [ ] Node failure handling
- [ ] Load balancing

### Phase 4: Flow-Nexus MCP (Weeks 7-8)
- [ ] MCP tool registration
- [ ] Authentication integration
- [ ] Rate limiting
- [ ] Monitoring hooks
- [ ] Error reporting

### Phase 5: Performance Optimization (Weeks 9-10)
- [ ] Connection pooling
- [ ] Memory management
- [ ] Caching strategies
- [ ] Performance benchmarking
- [ ] Load testing

### Phase 6: Production Deployment (Weeks 11-12)
- [ ] Docker containerization
- [ ] Kubernetes deployment
- [ ] Monitoring setup
- [ ] Documentation
- [ ] Integration testing

## Conclusion

This comprehensive integration plan provides a robust foundation for implementing HTTP streaming and swarm coordination in the sublinear-time solver. The architecture prioritizes performance, reliability, and scalability while maintaining compatibility with Flow-Nexus platform requirements.

Key success metrics:
- Sub-millisecond iteration updates
- 99.9% uptime reliability
- Seamless swarm coordination
- Comprehensive verification
- Production-ready deployment

The modular design allows for incremental implementation and testing, ensuring each component can be thoroughly validated before moving to the next phase.