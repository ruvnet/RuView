# ruvector-nervous-system-wasm

Bio-inspired neural system components for browser execution via WebAssembly.

## Installation

```bash
npm install ruvector-nervous-system-wasm
```

## Quick Start

```javascript
import init, {
  BTSPLayer,
  BTSPAssociativeMemory,
  Hypervector,
  HdcMemory,
  WTALayer,
  KWTALayer,
  GlobalWorkspace,
  WorkspaceItem,
  version,
  available_mechanisms,
  performance_targets,
} from 'ruvector-nervous-system-wasm';

// Initialize WASM module (required before using any components)
await init();

console.log('Version:', version());
console.log('Available mechanisms:', available_mechanisms());
```

## Components

### 1. BTSP (Behavioral Timescale Synaptic Plasticity)

One-shot learning based on Bittner et al. 2017 hippocampal place field formation.

#### BTSPLayer

```javascript
// Create a BTSP layer with 100 synapses and 2000ms time constant
const btsp = new BTSPLayer(100, 2000.0);

// One-shot learning: associate pattern with target value immediately
const pattern = new Float32Array(100).fill(0.1);
btsp.one_shot_associate(pattern, 1.0);

// Forward pass: compute output for input pattern
const output = btsp.forward(pattern);
console.log('Output:', output);

// Get layer properties
console.log('Size:', btsp.size);
console.log('Weights:', btsp.get_weights());

// Reset layer to initial random state
btsp.reset();
```

#### BTSPSynapse

```javascript
// Create individual synapse with initial weight and time constant
const synapse = new BTSPSynapse(0.5, 2000.0);

// Update synapse based on neural activity
synapse.update(
  true,   // presynaptic_active: presynaptic neuron is firing
  true,   // plateau_signal: dendritic plateau detected
  10.0    // dt: time step in milliseconds
);

// Get synapse state
console.log('Weight:', synapse.weight);
console.log('Eligibility trace:', synapse.eligibility_trace);

// Compute synaptic output
const output = synapse.forward(0.8);
```

#### BTSPAssociativeMemory

```javascript
// Create key-value associative memory (input_size, output_size)
const memory = new BTSPAssociativeMemory(128, 64);

// Store key-value pair in one shot (no iteration needed)
const key = new Float32Array(128).fill(0.1);
const value = new Float32Array(64).fill(0.5);
memory.store_one_shot(key, value);

// Retrieve value from key
const retrieved = memory.retrieve(key);
console.log('Retrieved value:', retrieved);

// Get memory dimensions
console.log('Dimensions:', memory.dimensions());
```

### 2. HDC (Hyperdimensional Computing)

10,000-bit binary hypervectors with ultra-fast operations.

#### Hypervector

```javascript
// Create hypervectors
const hv1 = new Hypervector();           // Zero vector
const hv2 = Hypervector.random();        // Random (~50% bits set)
const hv3 = Hypervector.from_seed(42);   // Reproducible from seed

// Binding (XOR) - associative, commutative, self-inverse
const bound = hv2.bind(hv3);
console.log('Binding is self-inverse:', hv2.similarity(bound.bind(hv3)) > 0.99);

// Similarity: 1.0 = identical, 0.0 = orthogonal, -1.0 = opposite
const sim = hv2.similarity(hv3);
console.log('Similarity:', sim);

// Hamming distance (differing bits)
const distance = hv2.hamming_distance(hv3);
console.log('Hamming distance:', distance);

// Population count (set bits)
console.log('Popcount:', hv2.popcount());

// Bundle 3 vectors by majority voting
const bundled = Hypervector.bundle_3(hv1, hv2, hv3);

// Serialization
const bytes = hv2.to_bytes();
const restored = Hypervector.from_bytes(bytes);
console.log('Restored correctly:', hv2.similarity(restored) === 1.0);

// Properties
console.log('Dimension:', hv2.dimension);  // 10000
```

#### HdcMemory

```javascript
// Create memory store
const hdcMem = new HdcMemory();

// Store labeled hypervectors
const apple = Hypervector.random();
const orange = Hypervector.random();
const banana = Hypervector.random();

hdcMem.store("apple", apple);
hdcMem.store("orange", orange);
hdcMem.store("banana", banana);

// Retrieve similar vectors above threshold
const results = hdcMem.retrieve(apple, 0.5);
console.log('Similar to apple:', results);
// Returns: [["apple", 1.0], ...]

// Find top-k most similar
const topK = hdcMem.top_k(apple, 2);
console.log('Top 2:', topK);

// Query memory
console.log('Size:', hdcMem.size);
console.log('Has apple:', hdcMem.has("apple"));

// Get specific vector
const appleVec = hdcMem.get("apple");

// Clear memory
hdcMem.clear();
```

### 3. WTA (Winner-Take-All)

Instant decisions via neural competition.

#### WTALayer

```javascript
// Create WTA layer with 1000 neurons, threshold 0.5, inhibition strength 0.8
const wta = new WTALayer(1000, 0.5, 0.8);

// Competition: returns winning neuron index (or -1 if none exceeds threshold)
const activations = new Float32Array(1000);
activations[42] = 0.9;  // Make neuron 42 the winner
activations[100] = 0.7;

const winner = wta.compete(activations);
console.log('Winner:', winner);  // 42

// Soft competition: normalized activations (softmax-like)
const softActivations = wta.compete_soft(activations);
console.log('Soft activations:', softActivations);

// Get membrane potentials
const membranes = wta.get_membranes();

// Reset layer state
wta.reset();

// Configure refractory period (prevents winner from winning again immediately)
wta.set_refractory_period(20);

// Properties
console.log('Size:', wta.size);
```

#### KWTALayer

```javascript
// Create K-WTA layer: 1000 neurons, select top 50
const kwta = new KWTALayer(1000, 50);

// Optional: set activation threshold
kwta.with_threshold(0.1);

const activations = new Float32Array(1000);
for (let i = 0; i < 1000; i++) {
  activations[i] = Math.random();
}

// Select top-k neuron indices (sorted by activation, descending)
const winners = kwta.select(activations);
console.log('Winner indices:', winners);  // Uint32Array of 50 indices

// Select with values: array of [index, value] pairs
const winnersWithValues = kwta.select_with_values(activations);
console.log('Winners with values:', winnersWithValues);

// Get sparse activation vector (only top-k preserved, rest zeroed)
const sparse = kwta.sparse_activations(activations);
console.log('Sparse vector:', sparse);

// Properties
console.log('k:', kwta.k);       // 50
console.log('Size:', kwta.size); // 1000
```

### 4. Global Workspace

Attention bottleneck based on Global Workspace Theory (Baars, Dehaene).

#### WorkspaceItem

```javascript
// Create a workspace item
const content = new Float32Array([1.0, 2.0, 3.0, 4.0]);
const item = new WorkspaceItem(
  content,           // content vector
  0.9,               // salience (importance)
  1,                 // source_module ID
  Date.now()         // timestamp
);

// Create with custom decay and lifetime
const itemWithDecay = WorkspaceItem.with_decay(
  content,
  0.9,               // salience
  1,                 // source_module
  Date.now(),        // timestamp
  0.95,              // decay_rate per timestep
  5000               // lifetime in ms
);

// Access item properties
console.log('Salience:', item.salience);
console.log('Source module:', item.source_module);
console.log('Timestamp:', item.timestamp);
console.log('ID:', item.id);
console.log('Content:', item.get_content());
console.log('Magnitude:', item.magnitude());

// Update salience
item.update_salience(0.8);

// Apply temporal decay
item.apply_decay(1.0);  // dt = 1.0

// Check expiration
console.log('Expired:', item.is_expired(Date.now() + 10000));
```

#### GlobalWorkspace

```javascript
// Create workspace with capacity 7 (Miller's Law: 7 +/- 2)
const workspace = new GlobalWorkspace(7);

// Or with custom salience threshold
const workspace2 = GlobalWorkspace.with_threshold(7, 0.2);

// Configure decay rate
workspace.set_decay_rate(0.95);

// Broadcast items to workspace (returns true if accepted)
const item1 = new WorkspaceItem(new Float32Array([1, 2, 3]), 0.9, 1, Date.now());
const item2 = new WorkspaceItem(new Float32Array([4, 5, 6]), 0.7, 2, Date.now());

const accepted1 = workspace.broadcast(item1);
const accepted2 = workspace.broadcast(item2);
console.log('Item 1 accepted:', accepted1);
console.log('Item 2 accepted:', accepted2);

// Run competitive dynamics (decay + pruning)
workspace.compete();

// Retrieve all current representations
const allItems = workspace.retrieve();
console.log('All items:', allItems);
// Returns: [{ content: [...], salience: ..., source_module: ..., timestamp: ..., id: ... }, ...]

// Retrieve top-k most salient
const topItems = workspace.retrieve_top_k(3);
console.log('Top 3:', topItems);

// Get most salient item
const mostSalient = workspace.most_salient();
if (mostSalient) {
  console.log('Most salient:', mostSalient.salience);
}

// Query workspace state
console.log('Length:', workspace.len);
console.log('Capacity:', workspace.capacity);
console.log('Is full:', workspace.is_full());
console.log('Is empty:', workspace.is_empty());
console.log('Available slots:', workspace.available_slots());
console.log('Current load:', workspace.current_load());  // 0.0 to 1.0
console.log('Average salience:', workspace.average_salience());

// Clear workspace
workspace.clear();
```

## Performance Targets

| Component | Target | Method |
|-----------|--------|--------|
| BTSP one_shot_associate | Immediate | Gradient normalization |
| HDC bind | <50ns | XOR operation |
| HDC similarity | <100ns | Hamming distance + unrolled popcount |
| WTA compete | <1us | Single-pass argmax |
| K-WTA select | <10us | Partial sort (O(n + k log k)) |
| Workspace broadcast | <10us | Competition |

## Bundle Size

- WASM binary: ~178 KB
- JavaScript glue: ~54 KB
- TypeScript definitions: ~17 KB

## Biological References

| Mechanism | Reference |
|-----------|-----------|
| BTSP | Bittner et al. 2017 - Hippocampal place fields |
| HDC | Kanerva 1988, Plate 2003 - Hyperdimensional computing |
| WTA | Cortical microcircuits - Lateral inhibition |
| Global Workspace | Baars 1988, Dehaene 2014 - Consciousness and attention |

## Utility Functions

```javascript
// Get crate version
console.log(version());  // "0.1.0"

// List available mechanisms with descriptions
console.log(available_mechanisms());
// [["btsp", "Behavioral Timescale Synaptic Plasticity - One-shot learning"], ...]

// Get performance targets
console.log(performance_targets());
// [["btsp_one_shot", "Immediate (no iteration)"], ...]

// Get biological references
console.log(biological_references());
// [["BTSP", "Bittner et al. 2017 - Hippocampal place fields"], ...]
```

## TypeScript Support

Full TypeScript definitions are included. All classes and functions are fully typed:

```typescript
import init, {
  BTSPLayer,
  BTSPSynapse,
  BTSPAssociativeMemory,
  Hypervector,
  HdcMemory,
  WTALayer,
  KWTALayer,
  GlobalWorkspace,
  WorkspaceItem,
} from 'ruvector-nervous-system-wasm';

await init();

const layer: BTSPLayer = new BTSPLayer(100, 2000.0);
const hv: Hypervector = Hypervector.random();
const wta: WTALayer = new WTALayer(1000, 0.5, 0.8);
const ws: GlobalWorkspace = new GlobalWorkspace(7);
```

## License

MIT
