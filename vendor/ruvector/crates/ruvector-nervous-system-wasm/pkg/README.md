# @ruvector/nervous-system-wasm - Bio-Inspired AI for WebAssembly

[![npm version](https://img.shields.io/npm/v/ruvector-nervous-system-wasm.svg)](https://www.npmjs.com/package/ruvector-nervous-system-wasm)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/ruvnet/ruvector)
[![Bundle Size](https://img.shields.io/badge/bundle%20size-174KB%20gzip-green.svg)](https://www.npmjs.com/package/ruvector-nervous-system-wasm)
[![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?logo=webassembly&logoColor=white)](https://webassembly.org/)

**Bio-inspired neural system components** for browser execution. Implements neuromorphic computing primitives including Hyperdimensional Computing (HDC), Behavioral Timescale Synaptic Plasticity (BTSP), Winner-Take-All networks, and Global Workspace attention.

## Key Features

- **Hyperdimensional Computing (HDC)**: 10,000-bit binary hypervectors for similarity-preserving encoding
- **BTSP (Behavioral Timescale Synaptic Plasticity)**: One-shot learning without iteration
- **Winner-Take-All (WTA)**: Sub-microsecond instant decisions through lateral inhibition
- **K-WTA (K-Winner-Take-All)**: Sparse distributed coding for neural representations
- **Global Workspace**: 4-7 item attention bottleneck inspired by conscious access
- **WASM-Optimized**: Designed for browser ML and edge inference

## Installation

```bash
npm install ruvector-nervous-system-wasm
# or
yarn add ruvector-nervous-system-wasm
# or
pnpm add ruvector-nervous-system-wasm
```

## Quick Start

```typescript
import init, {
  BTSPLayer,
  Hypervector,
  HdcMemory,
  WTALayer,
  KWTALayer,
  GlobalWorkspace,
  WorkspaceItem
} from 'ruvector-nervous-system-wasm';

await init();

// One-shot learning with BTSP
const btsp = new BTSPLayer(100, 2000.0);
const pattern = new Float32Array(100).fill(0.1);
btsp.one_shot_associate(pattern, 1.0);

// Hyperdimensional computing
const apple = Hypervector.random();
const orange = Hypervector.random();
const similarity = apple.similarity(orange);

// Winner-take-all decisions
const wta = new WTALayer(1000, 0.5, 0.8);
const activations = new Float32Array(1000);
const winner = wta.compete(activations);
```

## Hyperdimensional Computing (HDC)

HDC represents information using high-dimensional binary vectors (~10,000 bits). Similar concepts have similar vectors, enabling robust pattern matching.

### Key Properties

- **High Dimensionality**: 10,000 bits provides exponential capacity
- **Holographic**: Information distributed across entire vector
- **Noise Tolerant**: Robust to bit flips and partial corruption
- **Single-Operation Learning**: No iterative training needed

```typescript
import { Hypervector, HdcMemory } from 'ruvector-nervous-system-wasm';

// Create random hypervectors for concepts
const apple = Hypervector.random();
const red = Hypervector.random();
const fruit = Hypervector.random();

// Bind: Associate concepts (XOR operation)
// Binding is self-inverse: a.bind(b).bind(b) == a
const redApple = apple.bind(red);

// Bundle: Combine multiple concepts (majority voting)
const fruitConcept = Hypervector.bundle_3(apple, orange, banana);

// Measure similarity (-1.0 to 1.0)
const sim = apple.similarity(redApple);
console.log(`Apple-RedApple similarity: ${sim.toFixed(3)}`);

// Hamming distance (number of differing bits)
const distance = apple.hamming_distance(orange);
console.log(`Hamming distance: ${distance}`);

// Reproducible vectors from seed
const seededVector = Hypervector.from_seed(42n);

// Serialize/deserialize
const bytes = apple.to_bytes();
const restored = Hypervector.from_bytes(bytes);
```

### HDC Memory Store

```typescript
import { HdcMemory, Hypervector } from 'ruvector-nervous-system-wasm';

const memory = new HdcMemory();

// Store concept vectors
memory.store("apple", Hypervector.random());
memory.store("banana", Hypervector.random());
memory.store("car", Hypervector.random());

// Retrieve similar concepts
const query = memory.get("apple")!;
const results = memory.retrieve(query, 0.8);  // threshold
console.log(`Found ${results.length} similar concepts`);

// Get top-k most similar
const topK = memory.top_k(query, 3);
for (const [label, similarity] of topK) {
  console.log(`${label}: ${similarity.toFixed(3)}`);
}

// Check existence
if (memory.has("apple")) {
  const vec = memory.get("apple");
}
```

## BTSP (Behavioral Timescale Synaptic Plasticity)

BTSP enables **one-shot learning** - learning patterns in a single exposure, inspired by hippocampal place field formation (Bittner et al., 2017).

```typescript
import { BTSPLayer, BTSPSynapse, BTSPAssociativeMemory } from 'ruvector-nervous-system-wasm';

// Create BTSP layer
const btsp = new BTSPLayer(256, 2000.0);  // 256 synapses, 2s time constant

// One-shot association: learn pattern -> target immediately
const pattern = new Float32Array(256);
pattern.fill(0.1);
pattern[0] = 0.9; pattern[42] = 0.8;

btsp.one_shot_associate(pattern, 1.0);  // Target value = 1.0

// Forward pass - retrieves learned pattern
const output = btsp.forward(pattern);
console.log(`Retrieved value: ${output.toFixed(3)}`);

// Get learned weights
const weights = btsp.get_weights();
```

### Individual Synapse Control

```typescript
import { BTSPSynapse } from 'ruvector-nervous-system-wasm';

// Create synapse with initial weight
const synapse = new BTSPSynapse(0.5, 2000.0);

// Update based on neural activity
synapse.update(
  true,   // presynaptic active
  true,   // plateau signal detected
  10.0    // dt in milliseconds
);

console.log(`Weight: ${synapse.weight.toFixed(3)}`);
console.log(`Eligibility: ${synapse.eligibility_trace.toFixed(3)}`);
```

### Associative Memory

```typescript
import { BTSPAssociativeMemory } from 'ruvector-nervous-system-wasm';

// Create key-value associative memory
const assocMem = new BTSPAssociativeMemory(64, 128);  // 64-dim keys -> 128-dim values

// Store associations in one shot
const key = new Float32Array(64).fill(0.1);
const value = new Float32Array(128).fill(0.5);
assocMem.store_one_shot(key, value);

// Retrieve from partial/noisy key
const query = new Float32Array(64).fill(0.1);
const retrieved = assocMem.retrieve(query);
```

## Winner-Take-All (WTA)

WTA implements competitive neural dynamics where only the strongest activation survives - enabling ultra-fast decision making.

```typescript
import { WTALayer } from 'ruvector-nervous-system-wasm';

// Create WTA layer: 1000 neurons, 0.5 threshold, 0.8 inhibition
const wta = new WTALayer(1000, 0.5, 0.8);

// Compete for winner
const activations = new Float32Array(1000);
activations[42] = 0.9;
activations[100] = 0.7;

const winner = wta.compete(activations);
console.log(`Winner index: ${winner}`);  // 42, or -1 if none exceed threshold

// Soft competition (softmax-like)
const softActivations = wta.compete_soft(activations);

// Get membrane potentials
const membranes = wta.get_membranes();

// Configure refractory period
wta.set_refractory_period(5.0);

// Reset layer state
wta.reset();
```

## K-Winner-Take-All (K-WTA)

K-WTA selects the top-k neurons, enabling sparse distributed coding.

```typescript
import { KWTALayer } from 'ruvector-nervous-system-wasm';

// Create K-WTA: 1000 neurons, select top 50
const kwta = new KWTALayer(1000, 50);

const activations = new Float32Array(1000);
// Fill with random values
for (let i = 0; i < 1000; i++) {
  activations[i] = Math.random();
}

// Get indices of top-k winners (sorted descending by value)
const winnerIndices = kwta.select(activations);
console.log(`Top 50 winners: ${winnerIndices}`);

// Get winners with their values
const winnersWithValues = kwta.select_with_values(activations);
for (const [index, value] of winnersWithValues) {
  console.log(`Neuron ${index}: ${value.toFixed(3)}`);
}

// Create sparse activation vector
const sparse = kwta.sparse_activations(activations);
// Only top-k values preserved, rest are 0
```

## Global Workspace

Implements the Global Workspace Theory of consciousness - a limited-capacity "workspace" where only the most salient information gains access.

```typescript
import { GlobalWorkspace, WorkspaceItem } from 'ruvector-nervous-system-wasm';

// Create workspace with capacity 7 (Miller's Law: 7 +/- 2)
const workspace = new GlobalWorkspace(7);

// Create workspace items
const content = new Float32Array([1.0, 2.0, 3.0, 4.0]);
const item1 = new WorkspaceItem(
  content,
  0.9,           // salience
  1,             // source module ID
  BigInt(Date.now())
);

const item2 = WorkspaceItem.with_decay(
  content,
  0.7,           // salience
  2,             // source module
  BigInt(Date.now()),
  0.1,           // decay rate
  5000n          // lifetime ms
);

// Broadcast to workspace (returns true if accepted)
if (workspace.broadcast(item1)) {
  console.log("Item accepted into workspace");
}

// Run competitive dynamics
workspace.compete();  // Lower salience items decay/get pruned

// Retrieve most salient
const mostSalient = workspace.most_salient();
if (mostSalient) {
  console.log(`Most salient: ${mostSalient.salience}`);
}

// Get all current items
const allItems = workspace.retrieve();

// Get top-k items
const topItems = workspace.retrieve_top_k(3);

// Check workspace state
console.log(`Items: ${workspace.len} / ${workspace.capacity}`);
console.log(`Load: ${(workspace.current_load() * 100).toFixed(1)}%`);
console.log(`Average salience: ${workspace.average_salience().toFixed(2)}`);

// Configure decay
workspace.set_decay_rate(0.05);
```

## Performance Benchmarks

| Component | Operation | Target Latency |
|-----------|-----------|----------------|
| BTSP | one_shot_associate | Immediate (no iteration) |
| HDC | bind (XOR) | < 50ns |
| HDC | similarity | < 100ns |
| WTA | compete | < 1us |
| K-WTA | select (k=50, n=1000) | < 10us |
| Workspace | broadcast | < 10us |

## Biological References

| Component | Biological Inspiration | Reference |
|-----------|----------------------|-----------|
| BTSP | Hippocampal place fields | Bittner et al., 2017 |
| HDC | Cortical sparse coding | Kanerva, 1988; Plate, 2003 |
| WTA | Lateral inhibition | Cortical microcircuits |
| Global Workspace | Conscious access | Baars, 1988; Dehaene, 2014 |

## API Reference

### Hypervector

| Method | Description |
|--------|-------------|
| `random()` | Create random hypervector (static) |
| `from_seed(seed)` | Reproducible from seed (static) |
| `bind(other)` | XOR binding (associative, self-inverse) |
| `bundle_3(a, b, c)` | Majority voting bundle (static) |
| `similarity(other)` | Cosine-like similarity (-1 to 1) |
| `hamming_distance(other)` | Number of differing bits |
| `to_bytes()` / `from_bytes()` | Serialization |

### BTSPLayer

| Method | Description |
|--------|-------------|
| `new(size, tau)` | Create layer |
| `one_shot_associate(pattern, target)` | Single-step learning |
| `forward(input)` | Compute output |
| `get_weights()` | Get learned weights |
| `reset()` | Reset to initial state |

### WTALayer / KWTALayer

| Method | Description |
|--------|-------------|
| `new(size, threshold, inhibition)` | Create WTA |
| `new(size, k)` | Create K-WTA |
| `compete(inputs)` | Get winner index |
| `select(inputs)` | Get top-k indices |
| `sparse_activations(inputs)` | Sparse output |

### GlobalWorkspace

| Method | Description |
|--------|-------------|
| `new(capacity)` | Create workspace (4-7 typical) |
| `broadcast(item)` | Add item to workspace |
| `compete()` | Run competitive dynamics |
| `most_salient()` | Get top item |
| `retrieve_top_k(k)` | Get top k items |

## Use Cases

- **Neuromorphic Computing**: Brain-inspired computing architectures
- **One-Shot Learning**: Learn from single examples
- **Attention Mechanisms**: Biologically-plausible attention
- **Sparse Coding**: Efficient neural representations
- **Symbol Binding**: Compositional representations with HDC
- **Fast Decision Making**: Ultra-low-latency neural decisions
- **Memory Systems**: Associative and content-addressable memory

## Bundle Size

- **WASM binary**: ~174KB (uncompressed)
- **Gzip compressed**: ~65KB
- **JavaScript glue**: ~8KB

## Related Packages

- [ruvector-attention-unified-wasm](https://www.npmjs.com/package/ruvector-attention-unified-wasm) - 18+ attention mechanisms
- [ruvector-learning-wasm](https://www.npmjs.com/package/ruvector-learning-wasm) - MicroLoRA adaptation
- [ruvector-exotic-wasm](https://www.npmjs.com/package/ruvector-exotic-wasm) - NAO governance, exotic AI

## License

MIT

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [Full Documentation](https://ruv.io)
- [Bug Reports](https://github.com/ruvnet/ruvector/issues)

---

**Keywords**: hyperdimensional computing, HDC, BTSP, behavioral timescale synaptic plasticity, neuromorphic, winner-take-all, WTA, K-WTA, sparse coding, neural networks, one-shot learning, WebAssembly, WASM, bio-inspired, brain-inspired, neural competition, lateral inhibition, global workspace, attention, consciousness, associative memory
