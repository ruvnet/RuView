# @ruvector/attention-unified-wasm - 18+ Attention Mechanisms in WASM

[![npm version](https://img.shields.io/npm/v/ruvector-attention-unified-wasm.svg)](https://www.npmjs.com/package/ruvector-attention-unified-wasm)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/ruvnet/ruvector)
[![Bundle Size](https://img.shields.io/badge/bundle%20size-331KB%20gzip-green.svg)](https://www.npmjs.com/package/ruvector-attention-unified-wasm)
[![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?logo=webassembly&logoColor=white)](https://webassembly.org/)

**Unified WebAssembly library** with 18+ attention mechanisms spanning Neural, DAG, Graph, and State Space Model categories. Single import for all your attention needs in browser and edge environments.

## Key Features

- **7 Neural Attention**: Scaled dot-product, multi-head, hyperbolic, linear, flash, local-global, MoE
- **7 DAG Attention**: Topological, causal cone, critical path, MinCut-gated, hierarchical Lorentz, parallel branch, temporal BTSP
- **3 Graph Attention**: GAT, GCN, GraphSAGE
- **1 State Space**: Mamba SSM with hybrid attention
- **Unified API**: Single selector for all mechanisms
- **WASM-Optimized**: Runs in browsers, Node.js, and edge runtimes

## Installation

```bash
npm install ruvector-attention-unified-wasm
# or
yarn add ruvector-attention-unified-wasm
# or
pnpm add ruvector-attention-unified-wasm
```

## Quick Start

```typescript
import init, {
  UnifiedAttention,
  availableMechanisms,
  scaledDotAttention,
  WasmMultiHeadAttention,
  MambaSSMAttention,
  MambaConfig
} from 'ruvector-attention-unified-wasm';

await init();

// List all available mechanisms
const mechanisms = availableMechanisms();
console.log(mechanisms);
// { neural: [...], dag: [...], graph: [...], ssm: [...] }

// Use unified selector
const attention = new UnifiedAttention("multi_head");
console.log(`Category: ${attention.category}`);  // "neural"
console.log(`Supports sequences: ${attention.supportsSequences()}`);

// Direct attention computation
const query = new Float32Array([1.0, 0.5, 0.3, 0.1]);
const keys = [new Float32Array([0.9, 0.4, 0.2, 0.1])];
const values = [new Float32Array([1.0, 1.0, 1.0, 1.0])];
const output = scaledDotAttention(query, keys, values);
```

## Attention Categories

### Neural Attention (7 mechanisms)

Standard transformer-style attention mechanisms for sequence processing.

```typescript
import {
  scaledDotAttention,
  WasmMultiHeadAttention,
  WasmHyperbolicAttention,
  WasmLinearAttention,
  WasmFlashAttention,
  WasmLocalGlobalAttention,
  WasmMoEAttention
} from 'ruvector-attention-unified-wasm';

// Scaled Dot-Product Attention
const output = scaledDotAttention(query, keys, values, scale);

// Multi-Head Attention
const mha = new WasmMultiHeadAttention(256, 8);  // 256 dim, 8 heads
const attended = mha.compute(query, keys, values);
console.log(`Heads: ${mha.numHeads}, Head dim: ${mha.headDim}`);

// Hyperbolic Attention (for hierarchical data)
const hyperbolic = new WasmHyperbolicAttention(64, -1.0);  // curvature = -1
const hypOut = hyperbolic.compute(query, keys, values);

// Linear Attention (O(n) complexity)
const linear = new WasmLinearAttention(64, 32);  // 32 random features
const linOut = linear.compute(query, keys, values);

// Flash Attention (memory-efficient)
const flash = new WasmFlashAttention(64, 32);  // block size 32
const flashOut = flash.compute(query, keys, values);

// Local-Global Attention (sparse)
const localGlobal = new WasmLocalGlobalAttention(64, 128, 4);  // window=128, 4 global
const lgOut = localGlobal.compute(query, keys, values);

// Mixture of Experts Attention
const moe = new WasmMoEAttention(64, 8, 2);  // 8 experts, top-2
const moeOut = moe.compute(query, keys, values);
```

### DAG Attention (7 mechanisms)

Specialized attention for Directed Acyclic Graphs, query plans, and workflow optimization.

```typescript
import {
  WasmQueryDag,
  WasmTopologicalAttention,
  WasmCausalConeAttention,
  WasmCriticalPathAttention,
  WasmMinCutGatedAttention,
  WasmHierarchicalLorentzAttention,
  WasmParallelBranchAttention,
  WasmTemporalBTSPAttention
} from 'ruvector-attention-unified-wasm';

// Create a query DAG
const dag = new WasmQueryDag();
const scan = dag.addNode("scan", 10.0);
const filter = dag.addNode("filter", 5.0);
const join = dag.addNode("join", 20.0);
const aggregate = dag.addNode("aggregate", 15.0);

dag.addEdge(scan, filter);
dag.addEdge(filter, join);
dag.addEdge(scan, join);
dag.addEdge(join, aggregate);

// Topological Attention (position-aware)
const topo = new WasmTopologicalAttention(0.9);  // decay factor
const topoScores = topo.forward(dag);

// Causal Cone Attention (lightcone-based)
const causal = new WasmCausalConeAttention(0.8, 0.6);  // future discount, ancestor weight
const causalScores = causal.forward(dag);

// Critical Path Attention
const critical = new WasmCriticalPathAttention(2.0, 0.5);  // path weight, branch penalty
const criticalScores = critical.forward(dag);

// MinCut-Gated Attention (flow-based)
const mincut = new WasmMinCutGatedAttention(0.5);  // gate threshold
const mincutScores = mincut.forward(dag);

// Hierarchical Lorentz Attention (hyperbolic DAG)
const lorentz = new WasmHierarchicalLorentzAttention(-1.0, 0.1);  // curvature, temperature
const lorentzScores = lorentz.forward(dag);

// Parallel Branch Attention
const parallel = new WasmParallelBranchAttention(4, 0.2);  // max branches, sync penalty
const parallelScores = parallel.forward(dag);

// Temporal BTSP Attention
const btsp = new WasmTemporalBTSPAttention(0.95, 0.1);  // decay, baseline
const btspScores = btsp.forward(dag);
```

### Graph Attention (3 mechanisms)

Attention mechanisms for graph-structured data.

```typescript
import {
  WasmGNNLayer,
  GraphAttentionFactory,
  graphHierarchicalForward,
  graphDifferentiableSearch,
  WasmSearchConfig
} from 'ruvector-attention-unified-wasm';

// Create GNN layer with attention
const gnn = new WasmGNNLayer(
  64,     // input dimension
  128,    // hidden dimension
  4,      // attention heads
  0.1     // dropout
);

// Forward pass for a node
const nodeEmbed = new Float32Array(64);
const neighborEmbeds = [
  new Float32Array(64),
  new Float32Array(64)
];
const edgeWeights = new Float32Array([0.8, 0.6]);

const updated = gnn.forward(nodeEmbed, neighborEmbeds, edgeWeights);
console.log(`Output dim: ${gnn.outputDim}`);

// Get available graph attention types
const types = GraphAttentionFactory.availableTypes();  // ["GAT", "GCN", "GraphSAGE"]

// Differentiable search
const config = new WasmSearchConfig(5, 0.1);  // top-5, temperature
const candidates = [query, ...keys];
const searchResults = graphDifferentiableSearch(query, candidates, config);

// Hierarchical forward through multiple layers
const layers = [gnn, gnn2, gnn3];
const final = graphHierarchicalForward(query, layerEmbeddings, layers);
```

### Mamba SSM (State Space Model)

Selective State Space Model for efficient sequence processing with O(n) complexity.

```typescript
import {
  MambaConfig,
  MambaSSMAttention,
  HybridMambaAttention
} from 'ruvector-attention-unified-wasm';

// Configure Mamba
const config = new MambaConfig(256)  // d_model = 256
  .withStateDim(16)           // state space dimension
  .withExpandFactor(2)        // expansion factor
  .withConvKernelSize(4);     // conv kernel

console.log(`Dim: ${config.dim}, State: ${config.state_dim}`);

// Create Mamba SSM Attention
const mamba = new MambaSSMAttention(config);
console.log(`Inner dim: ${mamba.innerDim}`);

// Or use defaults
const mambaDefault = MambaSSMAttention.withDefaults(128);

// Forward pass (seq_len, dim) flattened to 1D
const seqLen = 32;
const input = new Float32Array(seqLen * 256);
const output = mamba.forward(input, seqLen);

// Get pseudo-attention scores for visualization
const scores = mamba.getAttentionScores(input, seqLen);

// Hybrid Mamba + Local Attention
const hybrid = new HybridMambaAttention(config, 64);  // local window = 64
const hybridOut = hybrid.forward(input, seqLen);
console.log(`Local window: ${hybrid.localWindow}`);
```

## Unified Selector API

```typescript
import { UnifiedAttention } from 'ruvector-attention-unified-wasm';

// Create selector for any mechanism
const attention = new UnifiedAttention("mamba");

// Query capabilities
console.log(`Mechanism: ${attention.mechanism}`);      // "mamba"
console.log(`Category: ${attention.category}`);        // "ssm"
console.log(`Supports sequences: ${attention.supportsSequences()}`);    // true
console.log(`Supports graphs: ${attention.supportsGraphs()}`);          // false
console.log(`Supports hyperbolic: ${attention.supportsHyperbolic()}`);  // false

// Valid mechanisms:
// Neural: scaled_dot_product, multi_head, hyperbolic, linear, flash, local_global, moe
// DAG: topological, causal_cone, critical_path, mincut_gated, hierarchical_lorentz, parallel_branch, temporal_btsp
// Graph: gat, gcn, graphsage
// SSM: mamba
```

## Utility Functions

```typescript
import { softmax, temperatureSoftmax, cosineSimilarity, getStats } from 'ruvector-attention-unified-wasm';

// Softmax normalization
const logits = new Float32Array([1.0, 2.0, 3.0]);
const probs = softmax(logits);

// Temperature-scaled softmax
const sharper = temperatureSoftmax(logits, 0.5);   // More peaked
const flatter = temperatureSoftmax(logits, 2.0);  // More uniform

// Cosine similarity
const a = new Float32Array([1, 0, 0]);
const b = new Float32Array([0.7, 0.7, 0]);
const sim = cosineSimilarity(a, b);

// Library statistics
const stats = getStats();
console.log(`Total mechanisms: ${stats.total_mechanisms}`);  // 18
console.log(`Neural: ${stats.neural_count}`);                // 7
console.log(`DAG: ${stats.dag_count}`);                      // 7
console.log(`Graph: ${stats.graph_count}`);                  // 3
console.log(`SSM: ${stats.ssm_count}`);                      // 1
```

## Tensor Compression

```typescript
import { WasmTensorCompress } from 'ruvector-attention-unified-wasm';

const compressor = new WasmTensorCompress();
const embedding = new Float32Array(256);

// Compress based on access frequency
const compressed = compressor.compress(embedding, 0.5);  // 50% access frequency
const decompressed = compressor.decompress(compressed);

// Or specify compression level directly
const pq8 = compressor.compressWithLevel(embedding, "pq8");  // 8-bit product quantization

// Compression levels: "none", "half", "pq8", "pq4", "binary"
const ratio = compressor.getCompressionRatio(0.5);
```

## Performance Benchmarks

| Mechanism | Complexity | Latency (256-dim) |
|-----------|------------|-------------------|
| Scaled Dot-Product | O(n^2) | ~50us |
| Multi-Head (8 heads) | O(n^2) | ~200us |
| Linear | O(n) | ~30us |
| Flash | O(n^2) | ~100us (memory-efficient) |
| Mamba SSM | O(n) | ~80us |
| Topological DAG | O(V+E) | ~40us |
| GAT | O(E*h) | ~150us |

## API Reference Summary

### Neural Attention

| Class | Description |
|-------|-------------|
| `WasmMultiHeadAttention` | Parallel attention heads |
| `WasmHyperbolicAttention` | Hyperbolic space attention |
| `WasmLinearAttention` | O(n) performer-style |
| `WasmFlashAttention` | Memory-efficient blocked |
| `WasmLocalGlobalAttention` | Sparse with global tokens |
| `WasmMoEAttention` | Mixture of experts |

### DAG Attention

| Class | Description |
|-------|-------------|
| `WasmTopologicalAttention` | Position in topological order |
| `WasmCausalConeAttention` | Lightcone causality |
| `WasmCriticalPathAttention` | Critical path weighting |
| `WasmMinCutGatedAttention` | Flow-based gating |
| `WasmHierarchicalLorentzAttention` | Multi-scale hyperbolic |
| `WasmParallelBranchAttention` | Parallel DAG branches |
| `WasmTemporalBTSPAttention` | Temporal eligibility traces |

### Graph Attention

| Class | Description |
|-------|-------------|
| `WasmGNNLayer` | Multi-head graph attention |
| `GraphAttentionFactory` | Factory for graph attention types |

### State Space

| Class | Description |
|-------|-------------|
| `MambaSSMAttention` | Selective state space model |
| `HybridMambaAttention` | Mamba + local attention |
| `MambaConfig` | Mamba configuration |

## Use Cases

- **Transformers**: Standard and efficient attention variants
- **Query Optimization**: DAG-aware attention for SQL planners
- **Knowledge Graphs**: Graph attention for entity reasoning
- **Long Sequences**: O(n) attention with Mamba SSM
- **Hierarchical Data**: Hyperbolic attention for trees
- **Sparse Attention**: Local-global for long documents

## Bundle Size

- **WASM binary**: ~331KB (uncompressed)
- **Gzip compressed**: ~120KB
- **JavaScript glue**: ~12KB

## Related Packages

- [ruvector-learning-wasm](https://www.npmjs.com/package/ruvector-learning-wasm) - MicroLoRA adaptation
- [ruvector-nervous-system-wasm](https://www.npmjs.com/package/ruvector-nervous-system-wasm) - Bio-inspired neural
- [ruvector-economy-wasm](https://www.npmjs.com/package/ruvector-economy-wasm) - CRDT credit economy

## License

MIT OR Apache-2.0

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [Full Documentation](https://ruv.io)
- [Bug Reports](https://github.com/ruvnet/ruvector/issues)

---

**Keywords**: attention mechanism, transformer, multi-head attention, DAG attention, graph neural network, GAT, GCN, GraphSAGE, Mamba, SSM, state space model, WebAssembly, WASM, hyperbolic attention, linear attention, flash attention, query optimization, neural network, deep learning, browser ML
