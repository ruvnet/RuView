# ruvector-attention-unified-wasm

Unified WebAssembly bindings for 18+ attention mechanisms, combining Neural, DAG, Graph, and Mamba SSM attention types into a single npm package.

## Installation

```bash
npm install ruvector-attention-unified-wasm
# or
yarn add ruvector-attention-unified-wasm
```

## Quick Start

```javascript
import init, {
  // Neural attention
  WasmScaledDotProductAttention,
  WasmMultiHeadAttention,

  // DAG attention
  WasmQueryDag,
  WasmTopologicalAttention,

  // Graph attention
  WasmGraphAttention,
  GraphAttentionType,

  // SSM attention
  MambaSSMAttention,
  MambaConfig,

  // Utilities
  UnifiedAttention,
  availableMechanisms,
  version
} from 'ruvector-attention-unified-wasm';

// Initialize WASM module
await init();

console.log('Version:', version());
console.log('Mechanisms:', availableMechanisms());
```

## Attention Mechanism Categories

### 1. Neural Attention (7 mechanisms)

Standard transformer-style attention mechanisms for sequence processing.

#### Scaled Dot-Product Attention

```javascript
import { WasmScaledDotProductAttention } from 'ruvector-attention-unified-wasm';

// Create attention layer (dimension, dropout_rate)
const attention = new WasmScaledDotProductAttention(64, 0.1);

// Prepare query, key, value vectors (as Float32Array)
const query = new Float32Array(64);  // [dim]
const keys = new Float32Array(320);  // [5, dim] = 5 key vectors
const values = new Float32Array(320); // [5, dim] = 5 value vectors

// Fill with your embeddings...
for (let i = 0; i < 64; i++) query[i] = Math.random();

// Compute attention output
const output = attention.forward(query, keys, values, 5); // numKeys = 5
console.log('Output shape:', output.length); // 64

// Get attention weights for visualization
const weights = attention.getWeights(query, keys, 5);
console.log('Attention weights:', weights); // [5] probabilities
```

#### Multi-Head Attention

```javascript
import { WasmMultiHeadAttention } from 'ruvector-attention-unified-wasm';

// Create with dimensions and number of heads
const mha = new WasmMultiHeadAttention(
  512,  // model dimension
  8,    // number of heads
  0.1   // dropout
);

// Forward pass with batched inputs
const queries = new Float32Array(512 * 10);  // [batch=10, dim=512]
const keys = new Float32Array(512 * 20);     // [seq=20, dim=512]
const values = new Float32Array(512 * 20);

const output = mha.forward(queries, keys, values, 10, 20);
console.log('Output:', output.length); // 512 * 10 = 5120
```

#### Hyperbolic Attention

For hierarchical data like trees and taxonomies.

```javascript
import { WasmHyperbolicAttention } from 'ruvector-attention-unified-wasm';

// Curvature controls the hyperbolic space geometry
const hyperbolic = new WasmHyperbolicAttention(64, -1.0);

const output = hyperbolic.forward(query, keys, values, 5);
```

#### Linear Attention (Performer-style)

O(n) complexity for long sequences.

```javascript
import { WasmLinearAttention } from 'ruvector-attention-unified-wasm';

const linear = new WasmLinearAttention(64);
const output = linear.forward(query, keys, values, numKeys);
```

#### Flash Attention

Memory-efficient blocked attention for large sequences.

```javascript
import { WasmFlashAttention } from 'ruvector-attention-unified-wasm';

// Block size controls memory/compute tradeoff
const flash = new WasmFlashAttention(64, 256); // dim=64, block_size=256
const output = flash.forward(queries, keys, values, seqLen);
```

#### Local-Global Attention

Sparse attention with global tokens (like Longformer).

```javascript
import { WasmLocalGlobalAttention } from 'ruvector-attention-unified-wasm';

const lg = new WasmLocalGlobalAttention(
  64,   // dimension
  128,  // local window size
  4     // number of global tokens
);
const output = lg.forward(queries, keys, values, seqLen);
```

#### Mixture of Experts Attention

Route tokens to specialized expert attention heads.

```javascript
import { WasmMoEAttention } from 'ruvector-attention-unified-wasm';

const moe = new WasmMoEAttention(
  64,  // dimension
  8,   // number of experts
  2    // top-k experts per token
);
const output = moe.forward(input, seqLen);
```

### 2. DAG Attention (7 mechanisms)

Graph-topology-aware attention for directed acyclic graphs.

#### Building a DAG

```javascript
import { WasmQueryDag } from 'ruvector-attention-unified-wasm';

// Create DAG for query plan
const dag = new WasmQueryDag();

// Add nodes (operator_type, cost)
const scan = dag.addNode("scan", 100.0);
const filter = dag.addNode("filter", 20.0);
const join = dag.addNode("join", 50.0);
const aggregate = dag.addNode("aggregate", 30.0);

// Add edges (from, to)
dag.addEdge(scan, filter);
dag.addEdge(filter, join);
dag.addEdge(join, aggregate);

console.log('Nodes:', dag.nodeCount);   // 4
console.log('Edges:', dag.edgeCount);   // 3
console.log('JSON:', dag.toJson());
```

#### Topological Attention

Position-based attention following DAG order.

```javascript
import { WasmTopologicalAttention } from 'ruvector-attention-unified-wasm';

// decay_factor controls position-based decay (0.0-1.0)
const topo = new WasmTopologicalAttention(0.9);
const scores = topo.forward(dag);
console.log('Attention scores:', scores); // [0.35, 0.30, 0.20, 0.15]
```

#### Causal Cone Attention

Lightcone-based attention respecting causal dependencies.

```javascript
import { WasmCausalConeAttention } from 'ruvector-attention-unified-wasm';

// future_discount, ancestor_weight
const causal = new WasmCausalConeAttention(0.8, 0.9);
const scores = causal.forward(dag);
```

#### Critical Path Attention

Weight attention by critical execution path.

```javascript
import { WasmCriticalPathAttention } from 'ruvector-attention-unified-wasm';

// path_weight for critical path nodes, branch_penalty
const critical = new WasmCriticalPathAttention(2.0, 0.5);
const scores = critical.forward(dag);
```

#### MinCut-Gated Attention

Flow-based gating through bottleneck nodes.

```javascript
import { WasmMinCutGatedAttention } from 'ruvector-attention-unified-wasm';

// gate_threshold determines bottleneck detection sensitivity
const mincut = new WasmMinCutGatedAttention(0.5);
const scores = mincut.forward(dag);
```

#### Hierarchical Lorentz Attention

Multi-scale hyperbolic attention for DAG hierarchies.

```javascript
import { WasmHierarchicalLorentzAttention } from 'ruvector-attention-unified-wasm';

// curvature, temperature
const lorentz = new WasmHierarchicalLorentzAttention(-1.0, 0.1);
const scores = lorentz.forward(dag);
```

#### Parallel Branch Attention

Branch-aware attention for parallel DAG structures.

```javascript
import { WasmParallelBranchAttention } from 'ruvector-attention-unified-wasm';

// max_branches, sync_penalty
const parallel = new WasmParallelBranchAttention(8, 0.2);
const scores = parallel.forward(dag);
```

#### Temporal BTSP Attention

Behavioral Time-Series Pattern attention for temporal DAGs.

```javascript
import { WasmTemporalBTSPAttention } from 'ruvector-attention-unified-wasm';

// eligibility_decay, baseline_attention
const btsp = new WasmTemporalBTSPAttention(0.95, 0.5);
const scores = btsp.forward(dag);
```

### 3. Graph Attention (3 mechanisms)

Graph neural network attention for arbitrary graph structures.

#### Graph Attention Networks (GAT)

```javascript
import {
  WasmGraphAttention,
  GraphAttentionType
} from 'ruvector-attention-unified-wasm';

// Create GAT layer
const gat = new WasmGraphAttention(
  GraphAttentionType.GAT,
  64,    // input dimension
  32,    // output dimension
  8      // number of heads
);

// Build adjacency list
const adjacency = [
  [1, 2],      // node 0 connects to 1, 2
  [0, 2, 3],   // node 1 connects to 0, 2, 3
  [0, 1, 3],   // node 2 connects to 0, 1, 3
  [1, 2]       // node 3 connects to 1, 2
];

// Node features [4 nodes x 64 dims]
const features = new Float32Array(4 * 64);
// ... fill with node embeddings

// Forward pass
const output = gat.forward(features, adjacency, 4);
console.log('Output shape:', output.length); // 4 * 32 = 128
```

#### Graph Convolutional Networks (GCN)

```javascript
const gcn = new WasmGraphAttention(
  GraphAttentionType.GCN,
  64,
  32,
  1  // GCN typically uses 1 head
);

const output = gcn.forward(features, adjacency, numNodes);
```

#### GraphSAGE

```javascript
const sage = new WasmGraphAttention(
  GraphAttentionType.GraphSAGE,
  64,
  32,
  1
);

const output = sage.forward(features, adjacency, numNodes);
```

#### Factory Methods

```javascript
import { GraphAttentionFactory } from 'ruvector-attention-unified-wasm';

console.log(GraphAttentionFactory.availableTypes());
// ["gat", "gcn", "graphsage"]

console.log(GraphAttentionFactory.getDescription("gat"));
// "Graph Attention Networks with multi-head attention"

console.log(GraphAttentionFactory.getUseCases("gat"));
// ["Node classification", "Link prediction", ...]
```

### 4. State Space Models (1 mechanism)

#### Mamba SSM Attention

Selective State Space Model for efficient sequence modeling.

```javascript
import {
  MambaSSMAttention,
  MambaConfig,
  HybridMambaAttention
} from 'ruvector-attention-unified-wasm';

// Configure Mamba
const config = new MambaConfig(256)  // model dimension
  .withStateDim(16)
  .withExpandFactor(2)
  .withConvKernelSize(4);

// Create Mamba layer
const mamba = new MambaSSMAttention(config);

// Or use defaults
const mamba2 = MambaSSMAttention.withDefaults(256);

// Forward pass
const input = new Float32Array(256 * 100);  // [seq_len=100, dim=256]
const output = mamba.forward(input, 100);

// Get attention-like scores for visualization
const scores = mamba.getAttentionScores(input, 100);
```

#### Hybrid Mamba-Attention

Combine Mamba efficiency with local attention.

```javascript
import { HybridMambaAttention, MambaConfig } from 'ruvector-attention-unified-wasm';

const config = new MambaConfig(256);
const hybrid = new HybridMambaAttention(config, 64); // local_window=64

const output = hybrid.forward(input, seqLen);
console.log('Local window:', hybrid.localWindow); // 64
```

## Unified Attention Selector

Select the right mechanism dynamically.

```javascript
import { UnifiedAttention } from 'ruvector-attention-unified-wasm';

// Create selector for any mechanism
const selector = new UnifiedAttention("multi_head");

// Query mechanism properties
console.log(selector.mechanism);         // "multi_head"
console.log(selector.category);          // "neural"
console.log(selector.supportsSequences); // true
console.log(selector.supportsGraphs);    // false
console.log(selector.supportsHyperbolic); // false

// DAG mechanism
const dagSelector = new UnifiedAttention("topological");
console.log(dagSelector.category);       // "dag"
console.log(dagSelector.supportsGraphs); // true
```

## Utility Functions

```javascript
import {
  softmax,
  temperatureSoftmax,
  cosineSimilarity,
  availableMechanisms,
  getStats
} from 'ruvector-attention-unified-wasm';

// Softmax normalization
const probs = softmax(new Float32Array([1.0, 2.0, 3.0]));
console.log(probs); // [0.09, 0.24, 0.67]

// Temperature-scaled softmax
const sharpProbs = temperatureSoftmax(
  new Float32Array([1.0, 2.0, 3.0]),
  0.5  // lower temperature = sharper distribution
);

// Cosine similarity
const sim = cosineSimilarity(
  new Float32Array([1, 0, 0]),
  new Float32Array([0.707, 0.707, 0])
);
console.log(sim); // 0.707

// List all mechanisms
const mechs = availableMechanisms();
console.log(mechs.neural);  // ["scaled_dot_product", "multi_head", ...]
console.log(mechs.dag);     // ["topological", "causal_cone", ...]
console.log(mechs.graph);   // ["gat", "gcn", "graphsage"]
console.log(mechs.ssm);     // ["mamba"]

// Library stats
const stats = getStats();
console.log(stats.total_mechanisms);  // 18
console.log(stats.version);           // "0.1.0"
```

## TypeScript Support

Full TypeScript definitions are included. Import types as needed:

```typescript
import type {
  MambaConfig,
  GraphAttentionType,
  WasmQueryDag
} from 'ruvector-attention-unified-wasm';
```

## Performance Tips

1. **Reuse attention instances** - Creating new instances has overhead
2. **Use typed arrays** - Pass `Float32Array` directly, not regular arrays
3. **Batch when possible** - Multi-head attention supports batched inputs
4. **Choose the right mechanism**:
   - Sequences: Scaled Dot-Product, Multi-Head, Linear, Flash
   - Long sequences: Linear, Flash, Mamba
   - Hierarchical data: Hyperbolic, Hierarchical Lorentz
   - Graphs: GAT, GCN, GraphSAGE
   - DAG structures: Topological, Critical Path, MinCut-Gated

## Browser Usage

```html
<script type="module">
  import init, {
    WasmScaledDotProductAttention
  } from './pkg/ruvector_attention_unified_wasm.js';

  async function run() {
    await init();

    const attention = new WasmScaledDotProductAttention(64, 0.1);
    // ... use attention
  }

  run();
</script>
```

## Node.js Usage

```javascript
import { readFile } from 'fs/promises';
import { initSync } from 'ruvector-attention-unified-wasm';

// Load WASM binary
const wasmBuffer = await readFile(
  './node_modules/ruvector-attention-unified-wasm/ruvector_attention_unified_wasm_bg.wasm'
);
initSync(wasmBuffer);

// Now use the library
import { WasmMultiHeadAttention } from 'ruvector-attention-unified-wasm';
```

## Memory Management

WASM objects need explicit cleanup:

```javascript
const attention = new WasmScaledDotProductAttention(64, 0.1);
try {
  const output = attention.forward(query, keys, values, numKeys);
  // ... use output
} finally {
  attention.free();  // Release WASM memory
}

// Or use Symbol.dispose (requires TypeScript 5.2+)
{
  using attention = new WasmScaledDotProductAttention(64, 0.1);
  // Automatically freed at end of block
}
```

## License

MIT OR Apache-2.0

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [Documentation](https://ruvector.dev/docs)
- [NPM Package](https://www.npmjs.com/package/ruvector-attention-unified-wasm)
