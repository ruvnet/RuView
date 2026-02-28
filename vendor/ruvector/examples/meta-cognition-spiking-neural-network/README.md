# Meta-Cognition Spiking Neural Network

Advanced hybrid AI architecture combining **Spiking Neural Networks (SNN)**, **SIMD-optimized vector operations**, and **5 attention mechanisms** with meta-cognitive self-discovery capabilities.

## Features

| Capability | Performance | Description |
|------------|-------------|-------------|
| **Spiking Neural Networks** | 10-50x faster | LIF neurons + STDP learning with N-API SIMD |
| **SIMD Vector Operations** | 5-54x faster | Loop-unrolled distance/dot product calculations |
| **5 Attention Mechanisms** | Sub-millisecond | Multi-Head, Flash, Linear, Hyperbolic, MoE |
| **Vector Search** | 150x faster | RuVector-powered semantic search |
| **Meta-Cognition** | Autonomous | Self-discovering emergent capabilities |

## Quick Start

```bash
# Install dependencies
npm install

# Run all demos
node demos/run-all.js

# Or run specific demos
node demos/snn/examples/pattern-recognition.js
node demos/attention/all-mechanisms.js
node demos/optimization/simd-optimized-ops.js
```

## Project Structure

```
meta-cognition-spiking-neural-network/
├── demos/                          # Runnable examples
│   ├── attention/                  # Attention mechanism demos
│   │   ├── all-mechanisms.js       # All 5 attention types compared
│   │   └── hyperbolic-deep-dive.js # Poincaré ball model exploration
│   ├── exploration/                # Autonomous discovery
│   │   ├── cognitive-explorer.js   # Full hybrid architecture
│   │   └── discoveries.js          # Emergent capability finder
│   ├── optimization/               # Performance optimization
│   │   ├── adaptive-cognitive-system.js  # Self-optimizing attention selection
│   │   ├── performance-benchmark.js      # Comprehensive benchmarks
│   │   └── simd-optimized-ops.js         # SIMD vector operations
│   ├── self-discovery/             # Meta-cognitive systems
│   │   ├── cognitive-explorer.js   # Self-awareness demos
│   │   └── enhanced-cognitive-system.js  # Multi-attention integration
│   ├── snn/                        # Spiking Neural Network
│   │   ├── examples/               # SNN demos
│   │   ├── lib/                    # JavaScript wrapper
│   │   └── native/                 # C++ SIMD implementation
│   ├── vector-search/              # Semantic search demos
│   └── run-all.js                  # Master demo runner
├── docs/                           # Documentation
│   ├── AGENTDB-EXPLORATION.md      # AgentDB capabilities guide
│   ├── DISCOVERIES.md              # 6 emergent discoveries
│   ├── HYPERBOLIC-ATTENTION-GUIDE.md # Poincaré ball attention
│   ├── OPTIMIZATION-GUIDE.md       # Performance tuning guide
│   ├── SIMD-OPTIMIZATION-GUIDE.md  # SIMD techniques
│   └── SNN-GUIDE.md                # Spiking Neural Network guide
├── verification/                   # Testing & verification
│   ├── VERIFICATION-REPORT.md      # Package verification results
│   ├── functional-test.js          # API functional tests
│   └── verify-agentdb.js           # AgentDB verification script
└── package.json
```

## Core Components

### 1. Spiking Neural Networks (SNN)

Biologically-inspired neural networks with **SIMD-optimized N-API** native addon.

```javascript
const { createFeedforwardSNN, rateEncoding } = require('./demos/snn/lib/SpikingNeuralNetwork');

const snn = createFeedforwardSNN([100, 50, 10], {
  dt: 1.0,
  tau: 20.0,
  a_plus: 0.005,
  lateral_inhibition: true
});

// Train with STDP
const input = rateEncoding(pattern, snn.dt, 100);
snn.step(input);
```

**Performance**:
- LIF Updates: **16.7x** speedup
- Synaptic Forward: **14.9x** speedup
- STDP Learning: **26.3x** speedup
- Full Simulation: **18.4x** speedup

### 2. SIMD Vector Operations

Loop-unrolled operations enabling CPU auto-vectorization.

```javascript
const { distanceSIMD, dotProductSIMD, cosineSimilaritySIMD } = require('./demos/optimization/simd-optimized-ops');

const dist = distanceSIMD(vectorA, vectorB);  // 5-54x faster
const dot = dotProductSIMD(query, key);        // 1.5x faster
const cos = cosineSimilaritySIMD(a, b);        // 2.7x faster
```

**Peak Performance**:
- Distance (128d): **54x** speedup
- Cosine (64d): **2.73x** speedup
- Batch (100+ pairs): **2.46x** speedup

### 3. Attention Mechanisms

Five specialized attention types for different data structures.

| Mechanism | Best For | Latency |
|-----------|----------|---------|
| **Flash** | Long sequences | 0.023ms |
| **MoE** | Specialized domains | 0.021ms |
| **Multi-Head** | Complex patterns | 0.047ms |
| **Linear** | Real-time processing | 0.075ms |
| **Hyperbolic** | Hierarchical data | 0.222ms |

```javascript
// Run all mechanisms demo
node demos/attention/all-mechanisms.js

// Deep dive into hyperbolic attention
node demos/attention/hyperbolic-deep-dive.js
```

### 4. Meta-Cognitive Discovery

Autonomous system that discovers emergent capabilities.

```javascript
// Run discovery system
node demos/exploration/discoveries.js
```

**6 Discovered Emergent Behaviors**:
1. Multi-Scale Attention Hierarchy (Novelty: 5/5)
2. Spike Synchronization Patterns
3. Attention-Gated Spike Propagation
4. Temporal Coherence Emergence
5. Emergent Sparsity (80% fewer active neurons)
6. Meta-Plasticity (faster learning on later tasks)

### 5. Vector Search

High-performance semantic search powered by RuVector.

```javascript
node demos/vector-search/semantic-search.js
```

**Performance**: 0.409ms latency, 2,445 QPS, 150x faster than SQLite

## Demos

### Run All Demos
```bash
node demos/run-all.js
```

### Individual Demos

| Demo | Command | Description |
|------|---------|-------------|
| SNN Pattern Recognition | `node demos/snn/examples/pattern-recognition.js` | 5x5 pattern classification with STDP |
| SNN Benchmark | `node demos/snn/examples/benchmark.js` | Performance analysis |
| All Attention | `node demos/attention/all-mechanisms.js` | Compare 5 mechanisms |
| Hyperbolic Deep Dive | `node demos/attention/hyperbolic-deep-dive.js` | Poincaré ball exploration |
| SIMD Operations | `node demos/optimization/simd-optimized-ops.js` | Vector operation benchmarks |
| Adaptive System | `node demos/optimization/adaptive-cognitive-system.js` | Self-optimizing attention |
| Performance Benchmark | `node demos/optimization/performance-benchmark.js` | Comprehensive benchmarks |
| Semantic Search | `node demos/vector-search/semantic-search.js` | Vector search demo |
| Cognitive Explorer | `node demos/self-discovery/cognitive-explorer.js` | Self-awareness demo |
| Enhanced Cognitive | `node demos/self-discovery/enhanced-cognitive-system.js` | Multi-attention integration |
| Discoveries | `node demos/exploration/discoveries.js` | Emergent capability discovery |
| Full Explorer | `node demos/exploration/cognitive-explorer.js` | Complete hybrid architecture |

## Documentation

Detailed guides in the `docs/` folder:

- **[SNN-GUIDE.md](docs/SNN-GUIDE.md)** - Spiking Neural Network architecture and API
- **[SIMD-OPTIMIZATION-GUIDE.md](docs/SIMD-OPTIMIZATION-GUIDE.md)** - SIMD techniques and benchmarks
- **[HYPERBOLIC-ATTENTION-GUIDE.md](docs/HYPERBOLIC-ATTENTION-GUIDE.md)** - Poincaré ball model for hierarchies
- **[OPTIMIZATION-GUIDE.md](docs/OPTIMIZATION-GUIDE.md)** - Performance tuning strategies
- **[DISCOVERIES.md](docs/DISCOVERIES.md)** - 6 emergent capability discoveries
- **[AGENTDB-EXPLORATION.md](docs/AGENTDB-EXPLORATION.md)** - AgentDB capabilities

## Building Native SNN Addon

For maximum SNN performance, build the native SIMD addon:

```bash
cd demos/snn
npm install
npm run build

# Verify native addon
node examples/benchmark.js
```

**Requirements**:
- Node.js >= 16.0.0
- C++ compiler (g++, clang, or MSVC)
- SSE/AVX CPU support

## Key Insights

1. **Hybrid Architectures Win**: SNN + Attention creates emergent capabilities
2. **SIMD is Essential**: 5-54x speedup for vector operations
3. **Attention Selection Matters**: Different mechanisms for different problems
4. **Meta-Cognition Works**: Systems can discover their own capabilities
5. **Sparsity is Efficient**: 80% reduction in active neurons via lateral inhibition

## Performance Summary

```
Operation               | Speedup | Notes
------------------------|---------|---------------------------
STDP Learning           | 26.3x   | SIMD + N-API
Distance (128d)         | 54.0x   | Loop unrolling champion
Full SNN Simulation     | 18.4x   | LIF + Synaptic + STDP
Cosine Similarity (64d) | 2.73x   | Triple accumulation
Vector Search           | 150x    | vs SQLite baseline
Attention (Flash)       | 0.023ms | Sub-millisecond
```

## License

MIT License - See [LICENSE](LICENSE)

## Related Packages

- **[agentdb@alpha](https://www.npmjs.com/package/agentdb)** - Full AgentDB with 5 attention mechanisms
- **[micro-hnsw-wasm](../micro-hnsw-wasm/)** - WASM-optimized HNSW vector search
