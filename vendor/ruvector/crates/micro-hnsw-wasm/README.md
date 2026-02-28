# Micro HNSW v2.2 - Neuromorphic Vector Search Engine

A **7.2KB** neuromorphic computing core that fuses graph-based vector search (HNSW) with biologically-inspired spiking neural networks. Designed for 256-core ASIC deployment, edge AI, and real-time similarity-driven neural processing.

> **Vector search meets brain-inspired computing** — query vectors trigger neural spikes, enabling attention mechanisms, winner-take-all selection, and online learning through spike-timing dependent plasticity (STDP).

## Why Micro HNSW + SNN?

Traditional vector databases return ranked results. Micro HNSW v2.2 goes further: similarity scores become neural currents that drive a spiking network. This enables:

- **Spiking Attention**: Similar vectors compete via lateral inhibition — only the strongest survive
- **Temporal Coding**: Spike timing encodes confidence (first spike = best match)
- **Online Learning**: STDP automatically strengthens connections between co-activated vectors
- **Event-Driven Efficiency**: Neurons only compute when they spike — 1000x more efficient than dense networks
- **Neuromorphic Hardware Ready**: Direct mapping to Intel Loihi, IBM TrueNorth, or custom ASIC

## Features

### Vector Search (HNSW Core)
- **Multi-core sharding**: 256 cores × 32 vectors = 8,192 total vectors
- **Distance metrics**: L2 (Euclidean), Cosine similarity, Dot product
- **Beam search**: Width-3 beam for improved recall
- **Cross-core merging**: Unified results from distributed search

### Graph Neural Network Extensions
- **Typed nodes**: 16 Cypher-style types for heterogeneous graphs
- **Weighted edges**: Per-node weights for message passing
- **Neighbor aggregation**: GNN-style feature propagation
- **In-place updates**: Online learning and embedding refinement

### Spiking Neural Network Layer
- **LIF neurons**: Leaky Integrate-and-Fire with membrane dynamics
- **Refractory periods**: Biologically-realistic spike timing
- **STDP plasticity**: Hebbian learning from spike correlations
- **Spike propagation**: Graph-routed neural activation
- **HNSW→SNN bridge**: Vector similarity drives neural currents

### Deployment
- **7.2KB WASM**: Runs anywhere WebAssembly runs
- **No allocator**: Pure static memory, `no_std` Rust
- **ASIC-ready**: Synthesizable for custom silicon
- **Edge-native**: Microcontrollers to data centers

 "Real-World Applications" Section

  | Application                       | Description                                                                    |
  |-----------------------------------|--------------------------------------------------------------------------------|
  | 1. Embedded Vector Database       | Semantic search on microcontrollers/IoT with 256-core sharding                 |
  | 2. Knowledge Graphs               | Cypher-style typed entities (GENE, PROTEIN, DISEASE) with spreading activation |
  | 3. Self-Learning Systems          | Anomaly detection that learns via STDP without retraining                      |
  | 4. DNA/Protein Analysis           | k-mer embeddings for genomic similarity with winner-take-all alignment         |
  | 5. Algorithmic Trading            | Microsecond pattern matching with neural winner-take-all signals               |
  | 6. Industrial Control (PLC/SCADA) | Predictive maintenance via vibration analysis at the edge                      |
  | 7. Robotics & Sensor Fusion       | Multi-modal LIDAR/camera/IMU fusion with spike-based binding                   |

## Specifications

| Parameter | Value | Notes |
|-----------|-------|-------|
| Vectors/Core | 32 | Static allocation |
| Total Vectors | 8,192 | 256 cores × 32 vectors |
| Max Dimensions | 16 | Per vector |
| Neighbors (M) | 6 | Graph connectivity |
| Beam Width | 3 | Search beam size |
| Node Types | 16 | 4-bit packed |
| SNN Neurons | 32 | One per vector |
| **WASM Size** | **~7.2KB** | After wasm-opt -Oz |
| Gate Count | ~45K | Estimated for ASIC |

## Building

```bash
# Add wasm32 target
rustup target add wasm32-unknown-unknown

# Build with size optimizations
cargo build --release --target wasm32-unknown-unknown

# Optimize with wasm-opt (required for SNN features)
wasm-opt -Oz --enable-nontrapping-float-to-int -o micro_hnsw.wasm \
    target/wasm32-unknown-unknown/release/micro_hnsw_wasm.wasm

# Check size
ls -la micro_hnsw.wasm
```

## JavaScript Usage

### Basic Usage

```javascript
const response = await fetch('micro_hnsw.wasm');
const bytes = await response.arrayBuffer();
const { instance } = await WebAssembly.instantiate(bytes);
const wasm = instance.exports;

// Initialize: init(dims, metric, core_id)
// metric: 0=L2, 1=Cosine, 2=Dot
wasm.init(8, 1, 0);  // 8 dims, cosine similarity, core 0

// Insert vectors
const insertBuf = new Float32Array(wasm.memory.buffer, wasm.get_insert_ptr(), 16);
insertBuf.set([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
const idx = wasm.insert();  // Returns 0, or 255 if full

// Set node type (for Cypher-style queries)
wasm.set_node_type(idx, 3);  // Type 3 = e.g., "Person"

// Search
const queryBuf = new Float32Array(wasm.memory.buffer, wasm.get_query_ptr(), 16);
queryBuf.set([0.95, 0.05, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
const resultCount = wasm.search(5);  // k=5

// Read results
const resultPtr = wasm.get_result_ptr();
const resultView = new DataView(wasm.memory.buffer, resultPtr);
for (let i = 0; i < resultCount; i++) {
    const idx = resultView.getUint8(i * 8);
    const coreId = resultView.getUint8(i * 8 + 1);
    const dist = resultView.getFloat32(i * 8 + 4, true);

    // Filter by type if needed
    if (wasm.type_matches(idx, 0b1000)) {  // Only type 3
        console.log(`Result: idx=${idx}, distance=${dist}`);
    }
}
```

### Spiking Neural Network (NEW)

```javascript
// Reset SNN state
wasm.snn_reset();

// Inject current into neurons (simulates input)
wasm.snn_inject(0, 1.5);  // Strong input to neuron 0
wasm.snn_inject(1, 0.8);  // Weaker input to neuron 1

// Run simulation step (dt in ms)
const spikeCount = wasm.snn_step(1.0);  // 1ms timestep
console.log(`${spikeCount} neurons spiked`);

// Propagate spikes to neighbors
wasm.snn_propagate(0.5);  // gain=0.5

// Apply STDP learning
wasm.snn_stdp();

// Or use combined tick (step + propagate + optional STDP)
const spikes = wasm.snn_tick(1.0, 0.5, 1);  // dt=1ms, gain=0.5, learn=true

// Get spike bitset (which neurons fired)
const spikeBits = wasm.snn_get_spikes();
for (let i = 0; i < 32; i++) {
    if (spikeBits & (1 << i)) {
        console.log(`Neuron ${i} spiked!`);
    }
}

// Check individual neuron
if (wasm.snn_spiked(0)) {
    console.log('Neuron 0 fired');
}

// Get/set membrane potential
const v = wasm.snn_get_membrane(0);
wasm.snn_set_membrane(0, 0.5);

// Get simulation time
console.log(`Time: ${wasm.snn_get_time()} ms`);
```

### HNSW-SNN Integration

```javascript
// Vector search activates matching neurons
// Search converts similarity to neural current
const queryBuf = new Float32Array(wasm.memory.buffer, wasm.get_query_ptr(), 16);
queryBuf.set([0.9, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);

// hnsw_to_snn: search + inject currents based on distance
const found = wasm.hnsw_to_snn(5, 2.0);  // k=5, gain=2.0

// Now run SNN to see which neurons fire from similarity
wasm.snn_tick(1.0, 0.5, 1);
const spikes = wasm.snn_get_spikes();
console.log(`Similar vectors that spiked: 0b${spikes.toString(2)}`);
```

### GNN Message Passing

```javascript
// Set edge weights for nodes (0-255, higher = more important)
wasm.set_edge_weight(0, 255);  // Node 0: full weight
wasm.set_edge_weight(1, 128);  // Node 1: half weight

// Aggregate neighbors (GNN-style)
wasm.aggregate_neighbors(0);  // Aggregates neighbors of node 0

// Read aggregated embedding from DELTA buffer
const deltaBuf = new Float32Array(wasm.memory.buffer, wasm.get_delta_ptr(), 16);
console.log('Aggregated:', Array.from(deltaBuf));

// Update vector: v = v + alpha * delta
wasm.update_vector(0, 0.1);  // 10% update toward neighbors
```

### Multi-Core (256 Cores)

```javascript
const cores = [];
for (let i = 0; i < 256; i++) {
    const { instance } = await WebAssembly.instantiate(wasmBytes);
    instance.exports.init(8, 1, i);
    cores.push(instance.exports);
}

// Parallel search with merging
async function searchAll(query, k) {
    for (const core of cores) {
        new Float32Array(core.memory.buffer, core.get_query_ptr(), 16).set(query);
    }

    const results = await Promise.all(cores.map(c => c.search(k)));

    cores[0].clear_global();
    for (let i = 0; i < cores.length; i++) {
        cores[0].merge(cores[i].get_result_ptr(), results[i]);
    }

    return cores[0].get_global_ptr();
}
```

## C API

```c
// Core API
void init(uint8_t dims, uint8_t metric, uint8_t core_id);
float* get_insert_ptr(void);
float* get_query_ptr(void);
SearchResult* get_result_ptr(void);
SearchResult* get_global_ptr(void);
uint8_t insert(void);
uint8_t search(uint8_t k);
uint8_t merge(SearchResult* results, uint8_t count);
void clear_global(void);

// Info
uint8_t count(void);
uint8_t get_core_id(void);
uint8_t get_metric(void);
uint8_t get_dims(void);
uint8_t get_capacity(void);

// Cypher Node Types
void set_node_type(uint8_t idx, uint8_t type);  // type: 0-15
uint8_t get_node_type(uint8_t idx);
uint8_t type_matches(uint8_t idx, uint16_t type_mask);

// GNN Edge Weights
void set_edge_weight(uint8_t node, uint8_t weight);  // weight: 0-255
uint8_t get_edge_weight(uint8_t node);
void aggregate_neighbors(uint8_t idx);  // Results in DELTA buffer

// Vector Updates
float* get_delta_ptr(void);
float* set_delta_ptr(void);  // Mutable access
void update_vector(uint8_t idx, float alpha);  // v += alpha * delta

// Spiking Neural Network (NEW in v2.2)
void snn_reset(void);                           // Reset all SNN state
void snn_set_membrane(uint8_t idx, float v);    // Set membrane potential
float snn_get_membrane(uint8_t idx);            // Get membrane potential
void snn_set_threshold(uint8_t idx, float t);   // Set firing threshold
void snn_inject(uint8_t idx, float current);    // Inject current
uint8_t snn_spiked(uint8_t idx);                // Did neuron spike?
uint32_t snn_get_spikes(void);                  // Spike bitset (32 neurons)
uint8_t snn_step(float dt);                     // LIF step, returns spike count
void snn_propagate(float gain);                 // Propagate spikes to neighbors
void snn_stdp(void);                            // STDP weight update
uint8_t snn_tick(float dt, float gain, uint8_t learn);  // Combined step
float snn_get_time(void);                       // Get simulation time
uint8_t hnsw_to_snn(uint8_t k, float gain);     // Search → neural activation

// SearchResult structure (8 bytes)
typedef struct {
    uint8_t idx;
    uint8_t core_id;
    uint8_t _pad[2];
    float distance;
} SearchResult;
```

## Real-World Applications

### 1. Embedded Vector Database

Run semantic search on microcontrollers, IoT devices, or edge servers without external dependencies.

```javascript
// Semantic search on edge device
// Each core handles a shard of your embedding space
const cores = await initializeCores(256);

// Insert document embeddings (from TinyBERT, MiniLM, etc.)
for (const doc of documents) {
    const embedding = await encoder.encode(doc.text);
    const coreId = hashToCoreId(doc.id);
    cores[coreId].insertVector(embedding, doc.type);
}

// Query: "machine learning tutorials"
const queryVec = await encoder.encode(query);
const results = await searchAllCores(queryVec, k=10);

// Results ranked by cosine similarity across 8K vectors
// Total memory: 7.2KB × 256 = 1.8MB for 8K vectors
```

**Why SNN helps**: After search, run `snn_tick()` with inhibition — only the most relevant results survive the neural competition. Better than simple top-k.

---

### 2. Knowledge Graphs (Cypher-Style)

Build typed property graphs with vector-enhanced traversal.

```javascript
// Define entity types for a biomedical knowledge graph
const GENE = 0, PROTEIN = 1, DISEASE = 2, DRUG = 3, PATHWAY = 4;

// Insert entities with embeddings
insertVector(geneEmbedding, GENE);      // "BRCA1" → type 0
insertVector(proteinEmbedding, PROTEIN); // "p53" → type 1
insertVector(diseaseEmbedding, DISEASE); // "breast cancer" → type 2

// Cypher-like query: Find proteins similar to query, connected to diseases
const proteinMask = 1 << PROTEIN;
const results = wasm.search(20);

for (const r of results) {
    if (wasm.type_matches(r.idx, proteinMask)) {
        // Found similar protein - now traverse edges
        wasm.aggregate_neighbors(r.idx);
        // Check if neighbors include diseases
    }
}
```

**Why SNN helps**: Model spreading activation through the knowledge graph. A query about "cancer treatment" activates DISEASE nodes, which propagate to connected DRUG and GENE nodes via `snn_propagate()`.

---

### 3. Self-Learning Systems (Online STDP)

Systems that learn patterns from experience without retraining.

```javascript
// Anomaly detection that learns normal patterns
class SelfLearningAnomalyDetector {
    async processEvent(sensorVector) {
        // Find similar past events
        wasm.hnsw_to_snn(5, 2.0);  // Top-5 similar → neural current

        // Run SNN with STDP learning enabled
        const spikes = wasm.snn_tick(1.0, 0.5, 1);  // learn=1

        if (spikes === 0) {
            // Nothing spiked = no similar patterns = ANOMALY
            return { anomaly: true, confidence: 0.95 };
        }

        // Normal: similar patterns recognized and reinforced
        // STDP strengthened the connection for next time
        return { anomaly: false };
    }
}

// Over time, the system learns what "normal" looks like
// New attack patterns won't match → no spikes → alert
```

**How it works**: STDP increases edge weights between vectors that co-activate. Repeated normal patterns build strong connections; novel anomalies find no matching pathways.

---

### 4. DNA/Protein Sequence Analysis

k-mer embeddings enable similarity search across genomic data.

```javascript
// DNA sequence similarity with neuromorphic processing
const KMER_SIZE = 6;  // 6-mer embeddings

// Embed reference genome k-mers
for (let i = 0; i < genome.length - KMER_SIZE; i++) {
    const kmer = genome.slice(i, i + KMER_SIZE);
    const embedding = kmerToVector(kmer);  // One-hot or learned embedding
    wasm.insert();
    wasm.set_node_type(i % 32, positionToType(i));  // Encode genomic region
}

// Query: Find similar sequences to a mutation site
const mutationKmer = "ATCGTA";
const queryVec = kmerToVector(mutationKmer);
wasm.hnsw_to_snn(10, 3.0);

// SNN competition finds the MOST similar reference positions
wasm.snn_tick(1.0, -0.2, 0);  // Lateral inhibition
const matches = wasm.snn_get_spikes();

// Surviving spikes = strongest matches
// Spike timing = match confidence (earlier = better)
```

**Why SNN helps**:
- **Winner-take-all**: Only the best alignments survive
- **Temporal coding**: First spike indicates highest similarity
- **Distributed processing**: 256 cores = parallel genome scanning

---

### 5. Algorithmic Trading

Microsecond pattern matching for market microstructure.

```javascript
// Real-time order flow pattern recognition
class TradingPatternMatcher {
    constructor() {
        // Pre-load known patterns: momentum, mean-reversion, spoofing, etc.
        this.patterns = [
            { name: 'momentum_breakout', vector: [...], type: 0 },
            { name: 'mean_reversion', vector: [...], type: 1 },
            { name: 'spoofing_signature', vector: [...], type: 2 },
            { name: 'iceberg_order', vector: [...], type: 3 },
        ];

        for (const p of this.patterns) {
            insertVector(p.vector, p.type);
        }
    }

    // Called every tick (microseconds)
    onMarketData(orderBookSnapshot) {
        const features = extractFeatures(orderBookSnapshot);
        // [bid_depth, ask_depth, spread, imbalance, volatility, ...]

        // Find matching patterns
        setQuery(features);
        wasm.hnsw_to_snn(5, 2.0);

        // SNN decides which pattern "wins"
        wasm.snn_tick(0.1, -0.5, 0);  // Fast tick, strong inhibition

        const winner = wasm.snn_get_spikes();
        if (winner & (1 << 0)) return 'GO_LONG';   // Momentum
        if (winner & (1 << 1)) return 'GO_SHORT';  // Mean reversion
        if (winner & (1 << 2)) return 'CANCEL';    // Spoofing detected

        return 'HOLD';
    }
}
```

**Why SNN helps**:
- **Sub-millisecond latency**: 7.2KB WASM runs in L1 cache
- **Winner-take-all**: Only one signal fires, no conflicting trades
- **Adaptive thresholds**: Market regime changes adjust neuron sensitivity

---

### 6. Industrial Control Systems (PLC/SCADA)

Predictive maintenance and anomaly detection at the edge.

```javascript
// Vibration analysis for rotating machinery
class PredictiveMaintenance {
    constructor() {
        // Reference signatures: healthy, bearing_wear, misalignment, imbalance
        this.signatures = loadVibrationSignatures();
        for (const sig of this.signatures) {
            insertVector(sig.fftFeatures, sig.condition);
        }
    }

    // Called every 100ms from accelerometer
    analyzeVibration(fftSpectrum) {
        setQuery(fftSpectrum);

        // Match against known conditions
        wasm.hnsw_to_snn(this.signatures.length, 1.5);
        wasm.snn_tick(1.0, 0.3, 1);  // Learn new patterns over time

        const spikes = wasm.snn_get_spikes();

        // Check which condition matched
        if (spikes & (1 << HEALTHY)) {
            return { status: 'OK', confidence: wasm.snn_get_membrane(HEALTHY) };
        }
        if (spikes & (1 << BEARING_WEAR)) {
            return {
                status: 'WARNING',
                condition: 'bearing_wear',
                action: 'Schedule maintenance in 72 hours'
            };
        }
        if (spikes & (1 << CRITICAL)) {
            return { status: 'ALARM', action: 'Immediate shutdown' };
        }

        // No match = unknown condition = anomaly
        return { status: 'UNKNOWN', action: 'Flag for analysis' };
    }
}
```

**Why SNN helps**:
- **Edge deployment**: Runs on PLC without cloud connectivity
- **Continuous learning**: STDP adapts to machine aging
- **Deterministic timing**: No garbage collection pauses

---

### 7. Robotics & Sensor Fusion

Combine LIDAR, camera, and IMU embeddings for navigation.

```javascript
// Multi-modal sensor fusion for autonomous navigation
class SensorFusion {
    // Each sensor type gets dedicated neurons
    LIDAR_NEURONS = [0, 1, 2, 3, 4, 5, 6, 7];      // 8 neurons
    CAMERA_NEURONS = [8, 9, 10, 11, 12, 13, 14, 15]; // 8 neurons
    IMU_NEURONS = [16, 17, 18, 19, 20, 21, 22, 23];  // 8 neurons

    fuseAndDecide(lidarEmbed, cameraEmbed, imuEmbed) {
        wasm.snn_reset();

        // Inject sensor readings as currents
        for (let i = 0; i < 8; i++) {
            wasm.snn_inject(this.LIDAR_NEURONS[i], lidarEmbed[i] * 2.0);
            wasm.snn_inject(this.CAMERA_NEURONS[i], cameraEmbed[i] * 1.5);
            wasm.snn_inject(this.IMU_NEURONS[i], imuEmbed[i] * 1.0);
        }

        // Run competition — strongest signals propagate
        for (let t = 0; t < 5; t++) {
            wasm.snn_tick(1.0, 0.4, 0);
        }

        // Surviving spikes = fused representation
        const fusedSpikes = wasm.snn_get_spikes();

        // Decision: which direction is clear?
        // Spike pattern encodes navigable directions
        return decodeSpikePattern(fusedSpikes);
    }
}
```

**Why SNN helps**:
- **Natural sensor fusion**: Different modalities compete and cooperate
- **Graceful degradation**: If camera fails, LIDAR/IMU still produce spikes
- **Temporal binding**: Synchronous spikes indicate consistent information

---

## Architecture: How It All Connects

```
┌─────────────────────────────────────────────────────────────────────┐
│                         APPLICATION LAYER                            │
├─────────────────────────────────────────────────────────────────────┤
│  Trading   │  Genomics  │  Robotics  │  Industrial  │  Knowledge   │
│  Signals   │  k-mers    │  Sensors   │  Vibration   │  Graphs      │
└─────┬──────┴─────┬──────┴─────┬──────┴──────┬───────┴──────┬───────┘
      │            │            │             │              │
      ▼            ▼            ▼             ▼              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        EMBEDDING LAYER                               │
│  Convert domain data → 16-dimensional vectors                        │
│  (TinyBERT, k-mer encoding, FFT features, one-hot, learned, etc.)   │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    MICRO HNSW v2.2 CORE (7.2KB)                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐            │
│   │   HNSW      │───▶│    GNN      │───▶│    SNN      │            │
│   │  (Search)   │    │  (Propagate)│    │   (Decide)  │            │
│   └─────────────┘    └─────────────┘    └─────────────┘            │
│         │                   │                  │                    │
│         ▼                   ▼                  ▼                    │
│   ┌──────────┐       ┌──────────┐       ┌──────────┐               │
│   │ Cosine   │       │ Neighbor │       │ LIF      │               │
│   │ L2, Dot  │       │ Aggregate│       │ Dynamics │               │
│   └──────────┘       └──────────┘       └──────────┘               │
│                                                │                    │
│                                                ▼                    │
│                                         ┌──────────┐               │
│                                         │  STDP    │               │
│                                         │ Learning │               │
│                                         └──────────┘               │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        OUTPUT: SPIKE PATTERN                         │
│  • Which neurons fired → Classification/Decision                     │
│  • Spike timing → Confidence ranking                                 │
│  • Membrane levels → Continuous scores                               │
│  • Updated weights → Learned associations                            │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Quick Reference: API by Use Case

| Use Case | Key Functions | Pattern |
|----------|---------------|---------|
| **Vector DB** | `insert()`, `search()`, `merge()` | Insert → Search → Rank |
| **Knowledge Graph** | `set_node_type()`, `type_matches()`, `aggregate_neighbors()` | Type → Filter → Traverse |
| **Self-Learning** | `snn_tick(..., learn=1)`, `snn_stdp()` | Process → Learn → Adapt |
| **Anomaly Detection** | `hnsw_to_snn()`, `snn_get_spikes()` | Match → Spike/NoSpike → Alert |
| **Trading** | `snn_tick()` with inhibition, `snn_get_spikes()` | Compete → Winner → Signal |
| **Industrial** | `snn_inject()`, `snn_tick()`, `snn_get_membrane()` | Sense → Fuse → Classify |
| **Sensor Fusion** | Multiple `snn_inject()`, `snn_propagate()` | Inject → Propagate → Bind |

---

## Code Examples

### Cypher-Style Typed Queries

```javascript
// Define node types
const PERSON = 0, COMPANY = 1, PRODUCT = 2;

// Insert typed nodes
insertVector([...], PERSON);
insertVector([...], COMPANY);

// Search only for PERSON nodes
const personMask = 1 << PERSON;  // 0b001
for (let i = 0; i < resultCount; i++) {
    if (wasm.type_matches(results[i].idx, personMask)) {
        // This is a Person node
    }
}
```

### GNN Layer Implementation

```javascript
// One GNN propagation step across all nodes
function gnnStep(alpha = 0.1) {
    for (let i = 0; i < wasm.count(); i++) {
        wasm.aggregate_neighbors(i);  // Mean of neighbors
        wasm.update_vector(i, alpha); // Blend with self
    }
}

// Run 3 GNN layers
for (let layer = 0; layer < 3; layer++) {
    gnnStep(0.5);
}
```

### Spiking Attention Layer

```javascript
// Use SNN for attention: similar vectors compete via lateral inhibition
function spikingAttention(queryVec, steps = 10) {
    wasm.snn_reset();

    const queryBuf = new Float32Array(wasm.memory.buffer, wasm.get_query_ptr(), 16);
    queryBuf.set(queryVec);
    wasm.hnsw_to_snn(wasm.count(), 3.0);  // Strong activation from similarity

    // Run SNN dynamics - winner-take-all emerges
    for (let t = 0; t < steps; t++) {
        wasm.snn_tick(1.0, -0.3, 0);  // Negative gain = inhibition
    }

    // Surviving spikes = attention winners
    return wasm.snn_get_spikes();
}
```

### Online Learning with STDP

```javascript
// Present pattern sequence, learn associations
function learnSequence(patterns, dt = 10.0) {
    wasm.snn_reset();

    for (const pattern of patterns) {
        // Inject current for active neurons
        for (const neuron of pattern) {
            wasm.snn_inject(neuron, 2.0);
        }

        // Run with STDP learning enabled
        wasm.snn_tick(dt, 0.5, 1);
    }

    // Edge weights now encode sequence associations
}
```

## ASIC / Verilog

The `verilog/` directory contains synthesizable RTL for direct ASIC implementation.

### Multi-Core Architecture with SNN

```
┌─────────────────────────────────────────────────────────────┐
│                    256-Core ASIC Layout                      │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────┐    │
│  │                   SNN Controller                     │    │
│  │  (Membrane, Threshold, Spike Router, STDP Engine)   │    │
│  └─────────────────────────────────────────────────────┘    │
│                         ↕                                    │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐     ┌─────┐ ┌─────┐       │
│  │Core │ │Core │ │Core │ │Core │ ... │Core │ │Core │       │
│  │  0  │ │  1  │ │  2  │ │  3  │     │ 254 │ │ 255 │       │
│  │ 32  │ │ 32  │ │ 32  │ │ 32  │     │ 32  │ │ 32  │       │
│  │ vec │ │ vec │ │ vec │ │ vec │     │ vec │ │ vec │       │
│  │ LIF │ │ LIF │ │ LIF │ │ LIF │     │ LIF │ │ LIF │       │
│  └──┬──┘ └──┬──┘ └──┬──┘ └──┬──┘     └──┬──┘ └──┬──┘       │
│     │       │       │       │           │       │           │
│     └───────┴───────┴───────┴───────────┴───────┘           │
│                         ▼                                    │
│              ┌─────────────────────┐                        │
│              │   Result Merger     │                        │
│              │  (Priority Queue)   │                        │
│              └─────────────────────┘                        │
│                         ▼                                    │
│              ┌─────────────────────┐                        │
│              │    AXI-Lite I/F     │                        │
│              └─────────────────────┘                        │
└─────────────────────────────────────────────────────────────┘
```

## Version History

| Version | Size | Features |
|---------|------|----------|
| v1 | 4.6KB | L2 only, single core, greedy search |
| v2 | 7.3KB | +3 metrics, +multi-core, +beam search |
| v2.1 | 5.5KB | +node types, +edge weights, +GNN updates, wasm-opt |
| **v2.2** | **7.2KB** | +LIF neurons, +STDP learning, +spike propagation, +HNSW-SNN bridge |

## Performance

| Operation | Complexity | Notes |
|-----------|------------|-------|
| Insert | O(n × dims) | Per core |
| Search | O(beam × M × dims) | Beam search |
| Merge | O(k × cores) | Result combining |
| Aggregate | O(M × dims) | GNN message passing |
| Update | O(dims) | Vector modification |
| SNN Step | O(n) | Per neuron LIF |
| Propagate | O(n × M) | Spike routing |
| STDP | O(spikes × M) | Only for spiking neurons |

## SNN Parameters (Compile-time)

| Parameter | Value | Description |
|-----------|-------|-------------|
| TAU_MEMBRANE | 20.0 | Membrane time constant (ms) |
| TAU_REFRAC | 2.0 | Refractory period (ms) |
| V_RESET | 0.0 | Reset potential after spike |
| V_REST | 0.0 | Resting potential |
| STDP_A_PLUS | 0.01 | LTP magnitude |
| STDP_A_MINUS | 0.012 | LTD magnitude |
| TAU_STDP | 20.0 | STDP time constant (ms) |

## License

MIT OR Apache-2.0
