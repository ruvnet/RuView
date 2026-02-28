# @cognitum/gate

[![npm version](https://img.shields.io/npm/v/@cognitum/gate.svg)](https://www.npmjs.com/package/@cognitum/gate)
[![bundle size](https://img.shields.io/bundlephobia/minzip/@cognitum/gate)](https://bundlephobia.com/package/@cognitum/gate)
[![license](https://img.shields.io/npm/l/@cognitum/gate.svg)](https://github.com/ruvnet/ruvector/blob/main/LICENSE)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.0+-blue.svg)](https://www.typescriptlang.org/)
[![WASM](https://img.shields.io/badge/WebAssembly-1.0-654FF0.svg)](https://webassembly.org/)

**Browser and Node.js coherence gate for AI agent safety**

---

## Introduction

The Cognitum Gate is a high-performance WASM-based coherence verification system designed to bring real-time safety guarantees to AI agent operations. Whether you're building autonomous agents in the browser or orchestrating complex workflows on Node.js, this package provides cryptographically-verifiable permit/defer/deny decisions in microseconds. Every action your agent considers passes through the gate, receiving an immediate verdict backed by witness receipts that create an immutable audit trail.

Unlike traditional attention mechanisms that weight tokens by relevance, the coherence gate transforms attention into a permission system. Actions are not merely ranked by probability or popularity---they are explicitly permitted or denied based on configurable safety thresholds, context windows, and agent-specific policies. This paradigm shift means your agents operate within well-defined boundaries, preventing runaway behaviors while maintaining the responsiveness users expect from modern AI applications.

**Attention becomes a permission system, not a popularity contest.**

The gate achieves sub-millisecond latency through a 256-tile WASM fabric that distributes verification across Web Workers (browser) or worker threads (Node.js). Each tile maintains its own coherence state, enabling horizontal scaling without sacrificing consistency. The result is a system that can handle thousands of permission checks per second while generating cryptographic receipts suitable for compliance, debugging, and post-hoc analysis.

**Created by [ruv.io](https://ruv.io) and [RuVector](https://github.com/ruvnet/ruvector)**

---

## Quick Start

```bash
npm install @cognitum/gate
```

```typescript
import { CognitumGate } from '@cognitum/gate';

// Initialize the gate with default configuration
const gate = await CognitumGate.init({
  tileCount: 16,
  coherenceThreshold: 0.85,
  maxContextTokens: 8192,
});

// Request permission for an agent action
const result = await gate.permitAction({
  agentId: 'agent-001',
  action: 'file_write',
  target: '/app/config.json',
  context: { reason: 'Update user preferences' },
});

if (result.verdict === 'permit') {
  console.log('Action permitted:', result.token);
  // Proceed with the action...

  // Get the witness receipt for audit trail
  const receipt = await gate.getReceipt(result.token);
  console.log('Receipt hash:', receipt.witnessHash);
} else if (result.verdict === 'defer') {
  console.log('Action deferred, retry after:', result.deferMs, 'ms');
} else {
  console.log('Action denied:', result.reason);
}
```

---

<details>
<summary><h2>Architecture</h2></summary>

### How WASM Tiles Work in Browser/Node

The coherence gate operates through a distributed tile architecture where each tile is an independent WASM module responsible for a subset of coherence verification. This design enables:

- **Parallel Processing**: Multiple tiles process requests concurrently
- **Fault Isolation**: A failing tile doesn't crash the entire system
- **Horizontal Scaling**: Add more tiles as load increases

```
+---------------------------------------------------------------+
|                      CognitumGate API                         |
+---------------------------------------------------------------+
|                     Tile Coordinator                          |
+-------+-------+-------+-------+-------+-------+-------+-------+
|Tile 0 |Tile 1 |Tile 2 |Tile 3 |  ...  |Tile N |Arbiter|Witness|
| WASM  | WASM  | WASM  | WASM  |       | WASM  | Tile  | Store |
+-------+-------+-------+-------+-------+-------+-------+-------+
    |       |       |       |               |       |
    +-------+-------+-------+---------------+-------+
              SharedArrayBuffer / MessageChannel
```

### Web Worker Distribution (Browser)

In browser environments, each tile runs in its own Web Worker for true parallelism:

```typescript
// The gate automatically spawns workers
const gate = await CognitumGate.init({
  tileCount: navigator.hardwareConcurrency || 4,
  workerUrl: '/cognitum-worker.js', // Optional custom worker
});

// Check active workers
console.log('Active tiles:', gate.getStats().activeTiles);
```

Workers communicate through `SharedArrayBuffer` when available (requires cross-origin isolation) or fall back to `MessageChannel` for broader compatibility.

### Worker Threads (Node.js)

On Node.js, the gate uses `worker_threads` for true parallelism:

```typescript
import { CognitumGate } from '@cognitum/gate/node';
import os from 'os';

const gate = await CognitumGate.init({
  tileCount: os.cpus().length,
  threadPoolSize: 4,
});
```

### SharedArrayBuffer for Tile Communication

When cross-origin isolation is enabled, tiles share memory through `SharedArrayBuffer`:

```typescript
// Check if SharedArrayBuffer is available
if (gate.supportsSharedMemory) {
  console.log('Using SharedArrayBuffer for zero-copy communication');
} else {
  console.log('Falling back to structured clone');
}
```

Required headers for cross-origin isolation:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

### Memory Layout Per Tile

Each tile maintains approximately 41KB of state:

```typescript
interface TileState {
  graphShard: Uint8Array;      // ~32KB - compact neighborhood graph
  featureWindow: Float32Array; // ~8KB - rolling normality scores
  coherence: number;           // f32 - local coherence score
  boundaryEdges: Uint32Array;  // 8 edges - local boundary candidates
  eAccumulator: number;        // f64 - local E-value accumulator
  tick: bigint;                // u64 - tick counter
}
```

</details>

---

<details>
<summary><h2>Technical Deep Dive</h2></summary>

### WASM Module Loading

The gate loads WASM modules asynchronously with streaming compilation when supported:

```typescript
import { loadWasmModule } from '@cognitum/gate/wasm';

// Manual WASM loading (usually handled automatically)
const wasmModule = await loadWasmModule({
  url: '/cognitum-gate.wasm',
  streaming: true, // Use WebAssembly.instantiateStreaming
  cache: 'persistent', // Cache in IndexedDB
});
```

The WASM binary is approximately 180KB gzipped and includes:
- Coherence scoring algorithms
- Cryptographic witness generation (BLAKE3/SHA-256)
- Tile state management
- Receipt serialization

### Memory Management (LinearMemory)

Each WASM tile operates with its own `WebAssembly.Memory` instance:

```typescript
interface TileMemoryConfig {
  initial: number;  // Initial pages (64KB each)
  maximum: number;  // Maximum pages
  shared: boolean;  // Use SharedArrayBuffer
}

const gate = await CognitumGate.init({
  tileMemory: {
    initial: 16,    // 1MB initial
    maximum: 256,   // 16MB maximum
    shared: true,   // Enable shared memory
  },
});

// Monitor memory usage
const stats = gate.getStats();
console.log('Memory per tile:', stats.memoryPerTile);
```

Memory lifecycle:
1. **Allocation**: Memory allocated on tile creation
2. **Growth**: Automatic growth up to `maximum` pages
3. **Compaction**: Periodic compaction during idle periods
4. **Release**: Memory freed when gate is destroyed

### TypeScript Type Definitions

Full type coverage for all APIs:

```typescript
// Core types
type Verdict = 'permit' | 'defer' | 'deny';

interface PermitRequest {
  agentId: string;
  action: string;
  target?: string;
  context?: Record<string, unknown>;
  priority?: 'low' | 'normal' | 'high' | 'critical';
  timeoutMs?: number;
}

interface PermitResult {
  verdict: Verdict;
  token: string;           // Unique permit token
  coherenceScore: number;  // 0.0 - 1.0
  tileId: number;          // Processing tile
  latencyUs: number;       // Processing time in microseconds
  reason?: string;         // Human-readable reason for defer/deny
  deferMs?: number;        // Suggested retry delay
}

interface WitnessReceipt {
  token: string;
  witnessHash: string;     // BLAKE3/SHA-256 hash
  timestamp: number;       // Unix timestamp (ms)
  agentId: string;
  action: string;
  verdict: Verdict;
  coherenceScore: number;
  parentHash?: string;     // Chain to previous receipt
  signature?: Uint8Array;  // Optional Ed25519 signature
  outcome?: ActionOutcome; // Recorded outcome
}
```

### Performance Characteristics

| Operation | Latency (p50) | Latency (p99) | Throughput |
|-----------|---------------|---------------|------------|
| `permitAction` | 45 us | 120 us | 22,000 req/s |
| `getReceipt` | 12 us | 35 us | 80,000 req/s |
| `batchPermit` (100) | 2.1 ms | 4.5 ms | 47,000 req/s |
| Tile cold start | 8 ms | 15 ms | N/A |

Benchmarked on:
- Browser: Chrome 120, M2 MacBook Pro
- Node.js: v20.10, 8-core AMD EPYC

</details>

---

<details>
<summary><h2>Tutorials and Examples</h2></summary>

### Example 1: React Integration

```tsx
// hooks/useCognitumGate.ts
import { useState, useEffect, useCallback } from 'react';
import { CognitumGate, GateConfig, PermitRequest, PermitResult } from '@cognitum/gate';

export function useCognitumGate(config?: Partial<GateConfig>) {
  const [gate, setGate] = useState<CognitumGate | null>(null);
  const [isReady, setIsReady] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    let mounted = true;
    let gateInstance: CognitumGate | null = null;

    CognitumGate.init(config).then((g) => {
      if (mounted) {
        gateInstance = g;
        setGate(g);
        setIsReady(true);
      }
    }).catch((e) => {
      if (mounted) setError(e);
    });

    return () => {
      mounted = false;
      gateInstance?.destroy();
    };
  }, []);

  const permit = useCallback(
    async (action: string, target?: string): Promise<PermitResult | null> => {
      if (!gate) return null;
      return gate.permitAction({ agentId: 'react-app', action, target });
    },
    [gate]
  );

  return { gate, isReady, error, permit };
}

// components/AgentAction.tsx
import { useCognitumGate } from '../hooks/useCognitumGate';

function AgentAction() {
  const { permit, isReady } = useCognitumGate();
  const [status, setStatus] = useState<string>('');

  const handleAction = async () => {
    const result = await permit('send_message', 'user-chat');

    if (result?.verdict === 'permit') {
      setStatus(`Permitted: ${result.token.slice(0, 16)}...`);
      // Execute the action
    } else if (result?.verdict === 'defer') {
      setStatus(`Deferred: retry in ${result.deferMs}ms`);
    } else {
      setStatus(`Denied: ${result?.reason || 'Unknown'}`);
    }
  };

  return (
    <div>
      <button onClick={handleAction} disabled={!isReady}>
        {isReady ? 'Send Message' : 'Loading Gate...'}
      </button>
      {status && <p>{status}</p>}
    </div>
  );
}
```

### Example 2: Express Middleware

```typescript
// middleware/cognitum.ts
import { Request, Response, NextFunction } from 'express';
import { CognitumGate, PermitResult } from '@cognitum/gate/node';

declare global {
  namespace Express {
    interface Request {
      permitToken?: string;
      permitResult?: PermitResult;
    }
  }
}

let gate: CognitumGate;

export async function initGateMiddleware() {
  gate = await CognitumGate.init({
    tileCount: 4,
    coherenceThreshold: 0.9,
  });
  console.log('Cognitum Gate initialized');
}

export function requirePermit(action: string) {
  return async (req: Request, res: Response, next: NextFunction) => {
    const result = await gate.permitAction({
      agentId: req.headers['x-agent-id'] as string || 'anonymous',
      action,
      target: req.path,
      context: {
        method: req.method,
        ip: req.ip,
        userAgent: req.headers['user-agent'],
      },
    });

    req.permitResult = result;

    if (result.verdict === 'permit') {
      req.permitToken = result.token;
      res.setHeader('X-Permit-Token', result.token);
      res.setHeader('X-Coherence-Score', result.coherenceScore.toFixed(4));
      next();
    } else if (result.verdict === 'defer') {
      res.status(429).json({
        error: 'Action deferred',
        reason: result.reason,
        retryAfter: result.deferMs,
      });
    } else {
      res.status(403).json({
        error: 'Action denied',
        reason: result.reason,
      });
    }
  };
}

// app.ts
import express from 'express';
import { initGateMiddleware, requirePermit } from './middleware/cognitum';

const app = express();
app.use(express.json());

async function main() {
  await initGateMiddleware();

  app.post('/api/files', requirePermit('file_create'), (req, res) => {
    // Handler only runs if permit granted
    res.json({ success: true, token: req.permitToken });
  });

  app.delete('/api/files/:id', requirePermit('file_delete'), (req, res) => {
    res.json({ deleted: req.params.id });
  });

  app.listen(3000, () => console.log('Server running on :3000'));
}

main();
```

### Example 3: Deno/Bun Usage

**Deno:**

```typescript
import { CognitumGate } from 'npm:@cognitum/gate';

const gate = await CognitumGate.init({
  tileCount: 4,
  runtime: 'deno',
});

Deno.serve({ port: 8000 }, async (req) => {
  const url = new URL(req.url);

  const result = await gate.permitAction({
    agentId: 'deno-server',
    action: 'handle_request',
    target: url.pathname,
  });

  if (result.verdict !== 'permit') {
    return new Response(JSON.stringify({
      error: 'Forbidden',
      reason: result.reason
    }), {
      status: 403,
      headers: { 'Content-Type': 'application/json' }
    });
  }

  return new Response('Hello from Deno!', {
    headers: {
      'X-Permit-Token': result.token,
      'X-Coherence-Score': result.coherenceScore.toString()
    }
  });
});
```

**Bun:**

```typescript
import { CognitumGate } from '@cognitum/gate';

const gate = await CognitumGate.init({
  tileCount: Bun.cpuCount || 4,
  runtime: 'bun',
});

Bun.serve({
  port: 3000,
  async fetch(req) {
    const result = await gate.permitAction({
      agentId: 'bun-server',
      action: 'handle_request',
      target: new URL(req.url).pathname,
    });

    if (result.verdict === 'permit') {
      return new Response('Hello from Bun!', {
        headers: { 'X-Permit-Token': result.token }
      });
    }

    return new Response('Forbidden', { status: 403 });
  },
});

console.log('Bun server running on :3000');
```

### Example 4: Claude-Flow Agent Integration

```typescript
import { CognitumGate, AgentPolicy } from '@cognitum/gate';

// Define agent-specific policies
const policies: AgentPolicy[] = [
  {
    agentId: 'coder',
    permissions: {
      'file_read': { threshold: 0.7 },
      'file_write': { threshold: 0.9, targets: ['src/**', 'tests/**'] },
      'file_delete': { threshold: 0.99 },
      'bash_execute': { threshold: 0.95, denyPatterns: ['rm -rf', 'sudo'] },
    },
  },
  {
    agentId: 'researcher',
    permissions: {
      'file_read': { threshold: 0.5 },
      'web_fetch': { threshold: 0.6 },
      'file_write': { verdict: 'deny' }, // Never permit writes
    },
  },
];

const gate = await CognitumGate.init({
  coherenceThreshold: 0.95, // Higher threshold for AI agents
  maxContextTokens: 16384,
  policies,
});

// Hook into Claude-Flow agent lifecycle
async function wrapToolUse(
  agentId: string,
  tool: { name: string },
  args: Record<string, unknown>
): Promise<{ permitToken: string }> {
  const result = await gate.permitAction({
    agentId,
    action: `tool:${tool.name}`,
    target: (args.path || args.target) as string,
    context: { args },
  });

  if (result.verdict === 'deny') {
    throw new Error(`Action denied: ${result.reason}`);
  }

  if (result.verdict === 'defer') {
    // Wait and retry
    await new Promise(r => setTimeout(r, result.deferMs || 1000));
    return wrapToolUse(agentId, tool, args); // Retry
  }

  return { permitToken: result.token };
}

// After tool execution, record outcome
async function recordToolOutcome(
  permitToken: string,
  success: boolean,
  error?: string
): Promise<void> {
  await gate.recordOutcome(permitToken, {
    success,
    error,
    durationMs: Date.now() - performance.now(),
  });
}
```

</details>

---

<details>
<summary><h2>Super Advanced Usage</h2></summary>

### Custom Tile Topology

Create specialized tile arrangements for specific workloads:

```typescript
import { CognitumGate, TileTopology } from '@cognitum/gate';

// Ring topology: tiles pass state to neighbors
const ringTopology: TileTopology = {
  type: 'ring',
  tiles: 8,
  connections: (tileId, total) => [(tileId + 1) % total],
};

// Hierarchical: fast local decisions, escalation for complex cases
const hierarchicalTopology: TileTopology = {
  type: 'hierarchical',
  levels: [
    { tiles: 16, threshold: 0.7 },  // Fast layer
    { tiles: 4, threshold: 0.85 },  // Review layer
    { tiles: 1, threshold: 0.95 },  // Final arbiter
  ],
};

// Mesh: full connectivity for consensus
const meshTopology: TileTopology = {
  type: 'mesh',
  tiles: 4,
  quorum: 3, // 3 of 4 must agree
};

const gate = await CognitumGate.init({
  topology: hierarchicalTopology,
});
```

### Streaming Decisions with AsyncIterator

Process high-volume action streams efficiently:

```typescript
import { CognitumGate, PermitRequest, PermitResult } from '@cognitum/gate';

const gate = await CognitumGate.init({ tileCount: 16 });

// Create an action stream
async function* actionStream(): AsyncGenerator<PermitRequest> {
  const eventSource = new EventSource('/api/actions');

  for await (const event of eventSource) {
    const data = JSON.parse(event.data);
    yield {
      agentId: data.agentId,
      action: data.type,
      target: data.target,
    };
  }
}

// Process with backpressure handling
const results = gate.permitStream(actionStream(), {
  concurrency: 100,
  bufferSize: 1000,
  onBackpressure: (pending) => {
    console.warn(`Backpressure: ${pending} pending requests`);
  },
});

for await (const result of results) {
  if (result.verdict === 'permit') {
    await executeAction(result);
  } else {
    console.log(`${result.verdict}: ${result.reason}`);
  }
}
```

### Offline-First with IndexedDB Receipt Storage

Store receipts locally for offline operation and later sync:

```typescript
import { CognitumGate, IndexedDBReceiptStore } from '@cognitum/gate';

const receiptStore = new IndexedDBReceiptStore({
  dbName: 'cognitum-receipts',
  maxReceipts: 100000,
  compactionThreshold: 0.8,
});

const gate = await CognitumGate.init({
  receiptStore,
  offlineMode: {
    enabled: true,
    maxOfflineActions: 1000,
    syncInterval: 30000, // Sync every 30s when online
  },
});

// Check offline status
gate.on('offline', () => {
  console.log('Operating in offline mode');
});

gate.on('online', () => {
  console.log('Back online, syncing receipts...');
});

gate.on('sync', (result) => {
  console.log(`Synced ${result.receiptsUploaded} receipts`);
});

// Query local receipts
const recentDenials = await receiptStore.query({
  agentId: 'my-agent',
  since: Date.now() - 86400000, // Last 24 hours
  verdict: 'deny',
  limit: 100,
});

console.log(`Found ${recentDenials.length} denied actions in last 24h`);
```

### Service Worker Integration

Run the gate in a Service Worker for cross-tab coherence:

```typescript
// sw.js - Service Worker
import { CognitumGate, PermitRequest } from '@cognitum/gate/sw';

let gate: CognitumGate;

self.addEventListener('install', (event) => {
  event.waitUntil(
    CognitumGate.init({ tileCount: 4 }).then((g) => {
      gate = g;
      console.log('Gate initialized in Service Worker');
    })
  );
});

self.addEventListener('message', async (event) => {
  if (event.data.type === 'permit') {
    const result = await gate.permitAction(event.data.request as PermitRequest);
    event.ports[0].postMessage(result);
  }

  if (event.data.type === 'get-stats') {
    event.ports[0].postMessage(gate.getStats());
  }
});

// client.js - Main thread
class ServiceWorkerGate {
  private registration: ServiceWorkerRegistration;

  constructor(registration: ServiceWorkerRegistration) {
    this.registration = registration;
  }

  async permitAction(request: PermitRequest): Promise<PermitResult> {
    const channel = new MessageChannel();

    return new Promise((resolve) => {
      channel.port1.onmessage = (e) => resolve(e.data);
      this.registration.active?.postMessage(
        { type: 'permit', request },
        [channel.port2]
      );
    });
  }

  async getStats(): Promise<GateStats> {
    const channel = new MessageChannel();

    return new Promise((resolve) => {
      channel.port1.onmessage = (e) => resolve(e.data);
      this.registration.active?.postMessage(
        { type: 'get-stats' },
        [channel.port2]
      );
    });
  }
}

// Usage
const reg = await navigator.serviceWorker.register('/sw.js');
const gate = new ServiceWorkerGate(reg);
const result = await gate.permitAction({ agentId: 'tab-1', action: 'fetch' });
```

### WebGPU Acceleration (Experimental)

Leverage GPU compute for high-throughput scenarios:

```typescript
import { CognitumGate, WebGPUAccelerator } from '@cognitum/gate/experimental';

// Check WebGPU support
if (!navigator.gpu) {
  throw new Error('WebGPU not supported');
}

const adapter = await navigator.gpu.requestAdapter();
const device = await adapter?.requestDevice();

if (!device) {
  throw new Error('Failed to get WebGPU device');
}

const accelerator = new WebGPUAccelerator({
  device,
  workgroupSize: 256,
  maxBatchSize: 4096,
});

const gate = await CognitumGate.init({
  accelerator,
  batchingStrategy: 'gpu-optimized',
});

// Batch operations are automatically routed to GPU
const requests = Array.from({ length: 1000 }, (_, i) => ({
  agentId: `agent-${i}`,
  action: 'compute',
  priority: 'normal' as const,
}));

const results = await gate.batchPermit(requests);

const stats = {
  permitted: results.filter(r => r.verdict === 'permit').length,
  deferred: results.filter(r => r.verdict === 'defer').length,
  denied: results.filter(r => r.verdict === 'deny').length,
};

console.log(`Processed ${results.length} requests on GPU:`, stats);
```

### Custom Coherence Scoring

Implement domain-specific coherence algorithms:

```typescript
import { CognitumGate, CoherenceScorer, PermitRequest, ScoringContext } from '@cognitum/gate';

class CustomCoherenceScorer implements CoherenceScorer {
  private actionHistory: Map<string, number[]> = new Map();

  async score(request: PermitRequest, context: ScoringContext): Promise<number> {
    let score = 1.0;

    // Penalize rapid repeated actions
    const key = `${request.agentId}:${request.action}`;
    const history = this.actionHistory.get(key) || [];
    const recentCount = history.filter(t => Date.now() - t < 60000).length;
    score -= recentCount * 0.1;

    // Update history
    history.push(Date.now());
    if (history.length > 100) history.shift();
    this.actionHistory.set(key, history);

    // Boost for high-priority requests
    if (request.priority === 'critical') {
      score += 0.2;
    } else if (request.priority === 'high') {
      score += 0.1;
    }

    // Apply time-of-day adjustments
    const hour = new Date().getHours();
    if (hour < 6 || hour > 22) {
      score -= 0.15; // Stricter during off-hours
    }

    // Consider tile load
    if (context.tileLoad > 0.8) {
      score -= 0.1; // Stricter under high load
    }

    return Math.max(0, Math.min(1, score));
  }
}

const gate = await CognitumGate.init({
  coherenceScorer: new CustomCoherenceScorer(),
});
```

</details>

---

## API Reference

### CognitumGate Class

```typescript
class CognitumGate {
  /**
   * Initialize a new CognitumGate instance
   */
  static init(config?: GateConfig): Promise<CognitumGate>;

  /**
   * Request permission for an action
   */
  permitAction(request: PermitRequest): Promise<PermitResult>;

  /**
   * Batch permission requests for efficiency
   */
  batchPermit(requests: PermitRequest[]): Promise<PermitResult[]>;

  /**
   * Stream permission decisions with backpressure handling
   */
  permitStream(
    requests: AsyncIterable<PermitRequest>,
    options?: StreamOptions
  ): AsyncIterable<PermitResult>;

  /**
   * Retrieve a witness receipt by token
   */
  getReceipt(token: string): Promise<WitnessReceipt>;

  /**
   * Record the outcome of a permitted action
   */
  recordOutcome(token: string, outcome: ActionOutcome): Promise<void>;

  /**
   * Get current gate statistics
   */
  getStats(): GateStats;

  /**
   * Check if SharedArrayBuffer is available
   */
  readonly supportsSharedMemory: boolean;

  /**
   * Subscribe to gate events
   */
  on(event: GateEvent, handler: EventHandler): void;

  /**
   * Unsubscribe from gate events
   */
  off(event: GateEvent, handler: EventHandler): void;

  /**
   * Destroy the gate and release resources
   */
  destroy(): Promise<void>;
}
```

### Type Definitions

```typescript
interface GateConfig {
  /** Number of WASM tiles (default: navigator.hardwareConcurrency || 4) */
  tileCount?: number;

  /** Minimum coherence score to permit (default: 0.85) */
  coherenceThreshold?: number;

  /** Maximum context tokens to consider (default: 8192) */
  maxContextTokens?: number;

  /** Custom tile topology */
  topology?: TileTopology;

  /** Custom receipt storage backend */
  receiptStore?: ReceiptStore;

  /** Tile memory configuration */
  tileMemory?: TileMemoryConfig;

  /** Custom coherence scoring implementation */
  coherenceScorer?: CoherenceScorer;

  /** Agent permission policies */
  policies?: AgentPolicy[];

  /** Default policy for unspecified agents */
  defaultPolicy?: DefaultPolicy;

  /** Offline mode configuration */
  offlineMode?: OfflineModeConfig;

  /** Runtime hint ('browser' | 'node' | 'deno' | 'bun') */
  runtime?: RuntimeHint;
}

interface PermitRequest {
  /** Unique identifier for the requesting agent */
  agentId: string;

  /** Action being requested */
  action: string;

  /** Target resource (optional) */
  target?: string;

  /** Additional context for coherence scoring */
  context?: Record<string, unknown>;

  /** Request priority (default: 'normal') */
  priority?: 'low' | 'normal' | 'high' | 'critical';

  /** Timeout in milliseconds (default: 5000) */
  timeoutMs?: number;
}

interface PermitResult {
  /** Decision: permit, defer, or deny */
  verdict: 'permit' | 'defer' | 'deny';

  /** Unique permit token (for receipts) */
  token: string;

  /** Coherence score (0.0 - 1.0) */
  coherenceScore: number;

  /** ID of the tile that processed the request */
  tileId: number;

  /** Processing latency in microseconds */
  latencyUs: number;

  /** Human-readable reason for defer/deny */
  reason?: string;

  /** Suggested delay for deferred requests (ms) */
  deferMs?: number;
}

interface WitnessReceipt {
  /** Permit token */
  token: string;

  /** BLAKE3/SHA-256 witness hash */
  witnessHash: string;

  /** Unix timestamp (milliseconds) */
  timestamp: number;

  /** Agent that made the request */
  agentId: string;

  /** Requested action */
  action: string;

  /** Final verdict */
  verdict: 'permit' | 'defer' | 'deny';

  /** Coherence score at decision time */
  coherenceScore: number;

  /** Hash of the previous receipt (chain) */
  parentHash?: string;

  /** Optional Ed25519 signature */
  signature?: Uint8Array;

  /** Action outcome (if recorded) */
  outcome?: ActionOutcome;
}

interface ActionOutcome {
  /** Whether the action succeeded */
  success: boolean;

  /** Error message if failed */
  error?: string;

  /** Execution duration in milliseconds */
  durationMs?: number;

  /** Additional outcome metadata */
  metadata?: Record<string, unknown>;
}

interface GateStats {
  /** Total requests processed */
  totalRequests: number;

  /** Requests by verdict */
  verdicts: {
    permit: number;
    defer: number;
    deny: number;
  };

  /** Average latency in microseconds */
  avgLatencyUs: number;

  /** P99 latency in microseconds */
  p99LatencyUs: number;

  /** Active tiles */
  activeTiles: number;

  /** Memory usage per tile (bytes) */
  memoryPerTile: number[];

  /** Uptime in milliseconds */
  uptimeMs: number;
}

type GateEvent =
  | 'permit'
  | 'defer'
  | 'deny'
  | 'error'
  | 'offline'
  | 'online'
  | 'sync'
  | 'tile-error'
  | 'tile-restart';
```

---

## Claude-Flow Integration

### MCP Server Setup

Add the Cognitum Gate MCP server to your Claude Code configuration:

```bash
claude mcp add cognitum-gate npx @cognitum/gate mcp start
```

Or configure in `.claude/settings.json`:

```json
{
  "mcpServers": {
    "cognitum-gate": {
      "command": "npx",
      "args": ["@cognitum/gate", "mcp", "start"],
      "env": {
        "COGNITUM_THRESHOLD": "0.9",
        "COGNITUM_TILES": "8"
      }
    }
  }
}
```

### Using with Claude Code

Once configured, the gate automatically integrates with Claude Code's hook system:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write|Bash",
        "hooks": [
          {
            "type": "command",
            "command": "npx @cognitum/gate permit --action $TOOL_NAME --target \"$TOOL_INPUT_file_path\""
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write|Bash",
        "hooks": [
          {
            "type": "command",
            "command": "npx @cognitum/gate record-outcome --token \"$PERMIT_TOKEN\" --success $TOOL_SUCCESS"
          }
        ]
      }
    ]
  }
}
```

### Agent Permission Patterns

Define granular permissions for different agent types:

```typescript
import { CognitumGate, AgentPolicy } from '@cognitum/gate';

const policies: AgentPolicy[] = [
  {
    agentId: 'coder',
    permissions: {
      'file_read': { threshold: 0.7 },
      'file_write': { threshold: 0.9, targets: ['src/**', 'tests/**'] },
      'file_delete': { threshold: 0.99 },
      'bash_execute': { threshold: 0.95, denyPatterns: ['rm -rf', 'sudo', 'chmod 777'] },
    },
  },
  {
    agentId: 'researcher',
    permissions: {
      'file_read': { threshold: 0.5 },
      'web_fetch': { threshold: 0.6 },
      'file_write': { verdict: 'deny' }, // Never permit writes
    },
  },
  {
    agentId: 'reviewer',
    permissions: {
      'file_read': { threshold: 0.5 },
      'git_command': { threshold: 0.8 },
      'file_write': { verdict: 'deny' },
    },
  },
];

const gate = await CognitumGate.init({
  policies,
  defaultPolicy: {
    threshold: 0.95, // Strict default for unknown agents
  },
});
```

---

## Browser Support

| Browser | Version | SharedArrayBuffer | WebGPU | Notes |
|---------|---------|-------------------|--------|-------|
| Chrome | 89+ | Yes | Yes | Full support |
| Firefox | 79+ | Yes | Partial | WebGPU behind flag |
| Safari | 15.2+ | Yes | Yes | Requires COOP/COEP |
| Edge | 89+ | Yes | Yes | Full support |
| Node.js | 16+ | Yes | N/A | Full support |
| Deno | 1.25+ | Yes | Partial | Full support |
| Bun | 0.6+ | Yes | N/A | Full support |

**Note**: SharedArrayBuffer requires cross-origin isolation headers:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

### CSP Requirements

If using Content Security Policy, ensure WASM is allowed:

```
Content-Security-Policy: script-src 'self' 'wasm-unsafe-eval';
```

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

---

**Created by [ruv.io](https://ruv.io) and [RuVector](https://github.com/ruvnet/ruvector)**

*Attention becomes a permission system, not a popularity contest.*
