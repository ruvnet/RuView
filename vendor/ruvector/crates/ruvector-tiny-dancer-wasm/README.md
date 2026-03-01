# Ruvector Tiny Dancer WASM

[![npm](https://img.shields.io/npm/v/@ruvector/tiny-dancer.svg)](https://www.npmjs.com/package/@ruvector/tiny-dancer)
[![Crates.io](https://img.shields.io/crates/v/ruvector-tiny-dancer-wasm.svg)](https://crates.io/crates/ruvector-tiny-dancer-wasm)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

**WebAssembly bindings for Tiny Dancer neural routing.**

`ruvector-tiny-dancer-wasm` brings production-grade AI agent routing to the browser with WebAssembly. Run FastGRNN neural inference for intelligent request routing directly in client-side applications. Part of the [Ruvector](https://github.com/ruvnet/ruvector) ecosystem.

## Why Tiny Dancer WASM?

- **Browser Native**: Run neural routing in any browser
- **Low Latency**: Sub-millisecond inference times
- **Small Bundle**: Optimized WASM binary (~100KB gzipped)
- **Offline Capable**: No server required for inference
- **Privacy First**: Route decisions stay client-side

## Features

### Core Capabilities

- **Neural Inference**: FastGRNN model execution
- **Feature Engineering**: Request feature extraction
- **Multi-Agent Routing**: Score and rank agent candidates
- **Model Loading**: Load pre-trained models
- **Batch Inference**: Process multiple requests

### Advanced Features

- **Web Workers**: Background inference threads
- **Streaming**: Process streaming requests
- **Model Caching**: IndexedDB model persistence
- **Quantization**: INT8 models for smaller size
- **SIMD**: Hardware acceleration when available

## Installation

```bash
npm install @ruvector/tiny-dancer-wasm
# or
yarn add @ruvector/tiny-dancer-wasm
```

## Quick Start

### Basic Usage

```typescript
import init, { TinyDancer, RouteRequest } from '@ruvector/tiny-dancer-wasm';

// Initialize WASM module
await init();

// Create router instance
const router = new TinyDancer();

// Load pre-trained model
await router.loadModel('/models/router-v1.bin');

// Create routing request
const request: RouteRequest = {
  query: "What is the weather like today?",
  context: {
    userId: "user-123",
    sessionLength: 5,
    previousAgent: "general",
  },
  agents: ["weather", "general", "calendar", "search"],
};

// Get routing decision
const result = await router.route(request);
console.log(`Route to: ${result.agent} (confidence: ${result.confidence})`);
```

### With Web Workers

```typescript
import { TinyDancerWorker } from '@ruvector/tiny-dancer-wasm/worker';

// Create worker-based router (non-blocking)
const router = new TinyDancerWorker();

// Initialize in background
await router.init();
await router.loadModel('/models/router-v1.bin');

// Route without blocking main thread
const result = await router.route(request);
```

### Feature Engineering

```typescript
import { FeatureExtractor } from '@ruvector/tiny-dancer-wasm';

const extractor = new FeatureExtractor();

// Extract features from request
const features = extractor.extract({
  query: "Book a flight to Paris",
  tokens: 6,
  language: "en",
  sentiment: 0.7,
  entities: ["Paris"],
});

console.log(`Feature vector: ${features.length} dimensions`);
```

## API Reference

### TinyDancer Class

```typescript
class TinyDancer {
  constructor();

  // Model management
  loadModel(url: string): Promise<void>;
  loadModelFromBuffer(buffer: Uint8Array): void;

  // Routing
  route(request: RouteRequest): Promise<RouteResult>;
  routeBatch(requests: RouteRequest[]): Promise<RouteResult[]>;

  // Scoring
  scoreAgents(request: RouteRequest): Promise<AgentScore[]>;

  // Info
  getModelInfo(): ModelInfo;
  isReady(): boolean;
}
```

### Types

```typescript
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

interface AgentScore {
  agent: string;
  score: number;
  features: number[];
}

interface RouteConstraints {
  excludeAgents?: string[];
  minConfidence?: number;
  timeout?: number;
}
```

## Bundle Optimization

### Tree Shaking

```typescript
// Import only what you need
import { TinyDancer } from '@ruvector/tiny-dancer-wasm/core';
import { FeatureExtractor } from '@ruvector/tiny-dancer-wasm/features';
```

### CDN Usage

```html
<script type="module">
  import init, { TinyDancer } from 'https://unpkg.com/@ruvector/tiny-dancer-wasm';

  await init();
  const router = new TinyDancer();
</script>
```

## Performance

### Benchmarks (Chrome 120, M1 Mac)

```
Operation           Latency (p50)
────────────────────────────────
Model load          ~50ms
Single inference    ~0.5ms
Batch (10)          ~2ms
Feature extraction  ~0.1ms
```

### Bundle Size

```
Format              Size
────────────────────────
WASM binary         ~100KB gzipped
JS glue             ~5KB gzipped
Total               ~105KB gzipped
```

## Browser Support

| Browser | Version | SIMD |
|---------|---------|------|
| Chrome | 89+ | ✅ |
| Firefox | 89+ | ✅ |
| Safari | 15+ | ✅ |
| Edge | 89+ | ✅ |

## Related Packages

- **[ruvector-tiny-dancer-core](../ruvector-tiny-dancer-core/)** - Core Rust implementation
- **[ruvector-tiny-dancer-node](../ruvector-tiny-dancer-node/)** - Node.js bindings
- **[ruvector-core](../ruvector-core/)** - Core vector database

## Documentation

- **[Main README](../../README.md)** - Complete project overview
- **[API Documentation](https://docs.rs/ruvector-tiny-dancer-wasm)** - Full API reference
- **[GitHub Repository](https://github.com/ruvnet/ruvector)** - Source code

## License

**MIT License** - see [LICENSE](../../LICENSE) for details.

---

<div align="center">

**Part of [Ruvector](https://github.com/ruvnet/ruvector) - Built by [rUv](https://ruv.io)**

[![Star on GitHub](https://img.shields.io/github/stars/ruvnet/ruvector?style=social)](https://github.com/ruvnet/ruvector)

[Documentation](https://docs.rs/ruvector-tiny-dancer-wasm) | [npm](https://www.npmjs.com/package/@ruvector/tiny-dancer) | [GitHub](https://github.com/ruvnet/ruvector)

</div>
