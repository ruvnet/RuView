# Ruvector Tiny Dancer Node

[![npm](https://img.shields.io/npm/v/@ruvector/tiny-dancer-node.svg)](https://www.npmjs.com/package/@ruvector/tiny-dancer-node)
[![Crates.io](https://img.shields.io/crates/v/ruvector-tiny-dancer-node.svg)](https://crates.io/crates/ruvector-tiny-dancer-node)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

**Node.js bindings for Tiny Dancer neural routing via NAPI-RS.**

`ruvector-tiny-dancer-node` provides native Node.js bindings for production-grade AI agent routing. Run FastGRNN neural inference at native speed for intelligent request routing in server-side applications. Part of the [Ruvector](https://github.com/ruvnet/ruvector) ecosystem.

## Why Tiny Dancer Node?

- **Native Performance**: Rust speed in Node.js
- **Production Ready**: Battle-tested in high-throughput systems
- **Async/Await**: Non-blocking inference operations
- **TypeScript**: Complete type definitions included
- **Multi-Threaded**: Leverage all CPU cores

## Features

### Core Capabilities

- **Neural Inference**: FastGRNN model execution
- **Model Training**: Train custom routing models
- **Feature Engineering**: Request feature extraction
- **Persistent Storage**: SQLite-backed model storage
- **Batch Processing**: Efficient batch inference

### Advanced Features

- **Model Versioning**: Manage multiple model versions
- **A/B Testing**: Route comparison and testing
- **Metrics**: Performance and accuracy tracking
- **Hot Reload**: Update models without restart
- **Distributed**: Coordinate across instances

## Installation

```bash
npm install @ruvector/tiny-dancer-node
# or
yarn add @ruvector/tiny-dancer-node
# or
pnpm add @ruvector/tiny-dancer-node
```

## Quick Start

### Basic Routing

```typescript
import { TinyDancer, RouteRequest } from '@ruvector/tiny-dancer-node';

// Create router instance
const router = new TinyDancer({
  modelPath: './models/router.db',
});

// Initialize
await router.init();

// Route request
const result = await router.route({
  query: "What is the weather like today?",
  context: {
    userId: "user-123",
    sessionLength: 5,
  },
  agents: ["weather", "general", "calendar"],
});

console.log(`Route to: ${result.agent} (confidence: ${result.confidence})`);
```

### Model Training

```typescript
import { TinyDancer, TrainingData } from '@ruvector/tiny-dancer-node';

const router = new TinyDancer();
await router.init();

// Prepare training data
const trainingData: TrainingData[] = [
  {
    query: "What's the weather?",
    correctAgent: "weather",
    context: { category: "weather" },
  },
  {
    query: "Schedule a meeting",
    correctAgent: "calendar",
    context: { category: "scheduling" },
  },
  // ... more examples
];

// Train model
const result = await router.train({
  data: trainingData,
  epochs: 100,
  learningRate: 0.001,
  validationSplit: 0.2,
});

console.log(`Training accuracy: ${result.accuracy}`);
console.log(`Validation accuracy: ${result.validationAccuracy}`);

// Save model
await router.saveModel('./models/custom-router.bin');
```

### Performance Monitoring

```typescript
import { TinyDancer } from '@ruvector/tiny-dancer-node';

const router = new TinyDancer({ enableMetrics: true });
await router.init();

// Route with metrics
const result = await router.route(request);

// Get performance metrics
const metrics = router.getMetrics();
console.log(`Average latency: ${metrics.avgLatencyMs}ms`);
console.log(`P99 latency: ${metrics.p99LatencyMs}ms`);
console.log(`Requests/sec: ${metrics.requestsPerSecond}`);
console.log(`Cache hit rate: ${metrics.cacheHitRate}`);
```

## API Reference

### TinyDancer Class

```typescript
class TinyDancer {
  constructor(config?: TinyDancerConfig);

  // Lifecycle
  init(): Promise<void>;
  close(): Promise<void>;

  // Routing
  route(request: RouteRequest): Promise<RouteResult>;
  routeBatch(requests: RouteRequest[]): Promise<RouteResult[]>;

  // Training
  train(options: TrainOptions): Promise<TrainResult>;
  loadModel(path: string): Promise<void>;
  saveModel(path: string): Promise<void>;

  // Scoring
  scoreAgents(request: RouteRequest): Promise<AgentScore[]>;

  // Metrics
  getMetrics(): RouterMetrics;
  resetMetrics(): void;
}
```

### Types

```typescript
interface TinyDancerConfig {
  modelPath?: string;
  enableMetrics?: boolean;
  cacheSize?: number;
  numThreads?: number;
}

interface RouteRequest {
  query: string;
  context?: Record<string, any>;
  agents: string[];
  constraints?: RouteConstraints;
}

interface RouteResult {
  agent: string;
  confidence: number;
  scores: Record<string, number>;
  latencyMs: number;
}

interface TrainOptions {
  data: TrainingData[];
  epochs: number;
  learningRate: number;
  validationSplit?: number;
  batchSize?: number;
}

interface TrainResult {
  accuracy: number;
  validationAccuracy: number;
  loss: number;
  epochs: number;
  trainingTimeMs: number;
}

interface RouterMetrics {
  totalRequests: number;
  avgLatencyMs: number;
  p50LatencyMs: number;
  p99LatencyMs: number;
  requestsPerSecond: number;
  cacheHitRate: number;
}
```

## Express Integration

```typescript
import express from 'express';
import { TinyDancer } from '@ruvector/tiny-dancer-node';

const app = express();
const router = new TinyDancer();

app.use(express.json());

app.post('/route', async (req, res) => {
  const result = await router.route({
    query: req.body.query,
    context: req.body.context,
    agents: ['agent-a', 'agent-b', 'agent-c'],
  });

  res.json(result);
});

app.listen(3000);
```

## Platform Support

| Platform | Architecture | Status |
|----------|-------------|--------|
| Linux | x64 | ✅ |
| Linux | arm64 | ✅ |
| macOS | x64 | ✅ |
| macOS | arm64 (M1/M2) | ✅ |
| Windows | x64 | ✅ |

## Building from Source

```bash
# Clone repository
git clone https://github.com/ruvnet/ruvector.git
cd ruvector/crates/ruvector-tiny-dancer-node

# Install dependencies
npm install

# Build native module
npm run build

# Run tests
npm test
```

## Related Packages

- **[ruvector-tiny-dancer-core](../ruvector-tiny-dancer-core/)** - Core Rust implementation
- **[ruvector-tiny-dancer-wasm](../ruvector-tiny-dancer-wasm/)** - WebAssembly bindings
- **[@ruvector/core](https://www.npmjs.com/package/@ruvector/core)** - Core vector bindings

## Documentation

- **[Main README](../../README.md)** - Complete project overview
- **[API Documentation](https://docs.rs/ruvector-tiny-dancer-node)** - Full API reference
- **[GitHub Repository](https://github.com/ruvnet/ruvector)** - Source code

## License

**MIT License** - see [LICENSE](../../LICENSE) for details.

---

<div align="center">

**Part of [Ruvector](https://github.com/ruvnet/ruvector) - Built by [rUv](https://ruv.io)**

[![Star on GitHub](https://img.shields.io/github/stars/ruvnet/ruvector?style=social)](https://github.com/ruvnet/ruvector)

[Documentation](https://docs.rs/ruvector-tiny-dancer-node) | [npm](https://www.npmjs.com/package/@ruvector/tiny-dancer-node) | [GitHub](https://github.com/ruvnet/ruvector)

</div>
