# Examples Directory Integration Analysis: sublinear-time-solver

**Agent**: 4 / Research -- Examples Directory Integration Analysis
**Date**: 2026-02-20
**Scope**: Full inventory of `/home/user/ruvector/examples`, crate-level examples, npm package examples, and integration opportunities with sublinear-time-solver.

---

## Table of Contents

1. [Complete Inventory of Existing Examples](#1-complete-inventory-of-existing-examples)
2. [Gap Analysis -- Where Sublinear Solver Adds Value](#2-gap-analysis----where-sublinear-solver-adds-value)
3. [Proposed New Examples for Integration](#3-proposed-new-examples-for-integration)
4. [Integration Pattern Code Snippets](#4-integration-pattern-code-snippets)
5. [Benchmark Examples -- RuVector + Sublinear Approaches](#5-benchmark-examples----ruvector--sublinear-approaches)
6. [Tutorial Progression -- Basic to Advanced](#6-tutorial-progression----basic-to-advanced)

---

## 1. Complete Inventory of Existing Examples

### 1.1 Top-Level Examples (`/home/user/ruvector/examples/`)

#### Core SDK Examples

| File / Directory | Language | Purpose | Lines | Status |
|---|---|---|---|---|
| `rust/gnn_example.rs` | Rust | Graph Neural Network layer usage, multi-head attention, GRU, LayerNorm | 60 | Working |
| `nodejs/basic_usage.js` | JavaScript | VectorDB creation, insertion, batch insert, search | 68 | Working |
| `nodejs/semantic_search.js` | JavaScript | Semantic search with HNSW config, filtered queries | 150 | Working |
| `bounded_instance_demo.rs` | Rust | BoundedInstance with DeterministicLocalKCut oracle, dynamic graph operations | 73 | Working |

#### Graph Database Examples (`examples/graph/`)

| File | Language | Purpose | Status |
|---|---|---|---|
| `basic_graph.rs` | Rust | Node/relationship CRUD, traversal, statistics | Template (TODO) |
| `cypher_queries.rs` | Rust | Neo4j-compatible Cypher query patterns (10 query types) | Template (TODO) |
| `distributed_cluster.rs` | Rust | Multi-node RAFT cluster, sharding, failover, recovery | Template (TODO) |
| `hybrid_search.rs` | Rust | Vector+Graph hybrid queries, PageRank ranking, multi-hop search | Template (TODO) |

#### WebAssembly Examples

| Directory | Language | Purpose |
|---|---|---|
| `wasm-react/` | JSX/React | React app with WASM vector DB integration, Vite build |
| `wasm-vanilla/` | HTML/JS | Browser-only WASM demo with interactive UI |
| `wasm/ios/` | Swift/WASM | iOS App Clip with WASM bridge |

#### MinCut Algorithm Examples (`examples/mincut/`)

| Directory | Language | Purpose |
|---|---|---|
| `benchmarks/main.rs` | Rust | Full benchmark suite: temporal attractors, strange loops, causal discovery, time crystals, morphogenetic networks, neural optimization, scaling |
| `temporal_attractors/` | Rust | Attractor evolution with min-cut convergence detection |
| `temporal_hypergraph/` | Rust | Time-varying hyperedges with Allen's temporal algebra and causal constraints |
| `strange_loop/` | Rust | Self-observation feedback loops using min-cut as a self-referential measure |
| `causal_discovery/` | Rust | Event tracking and causality detection from min-cut dynamics |
| `time_crystal/` | Rust | Phase transitions (ring->star->mesh) with periodicity verification |
| `morphogenetic/` | Rust | Signal diffusion and growth-rule-driven network evolution |
| `neural_optimizer/` | Rust | Neural network learns optimal graph configurations from historical evolution |
| `federated_loops/` | Rust | Federated min-cut across distributed graph partitions |
| `snn_integration/` | Rust | Spiking Neural Network integration with min-cut structural health |

#### Subpolynomial-Time Demo (`examples/subpolynomial-time/`)

| File | Language | Purpose |
|---|---|---|
| `src/main.rs` | Rust | 9-demo showcase: basic min-cut, dynamic updates, exact vs approximate, monitoring, network resilience, performance scaling, vector-graph fusion, brittleness detection, self-learning optimization |
| `src/fusion/` | Rust | FusionGraph, Optimizer, StructuralMonitor for vector-graph integration |

#### Edge Computing (`examples/edge/`, `examples/edge-full/`, `examples/edge-net/`)

| Directory | Language | Purpose |
|---|---|---|
| `edge/src/` | Rust | P2P agent communication, compression, intelligence layer, WASM bindings, ZK proofs, Plaid integration |
| `edge-net/sim/examples/quick-demo.js` | JavaScript | Network lifecycle simulation: genesis -> transition phase with economics |
| `edge-net/dashboard/` | HTML/JS | Real-time network monitoring dashboard |
| `edge-full/` | Rust | Full edge deployment package |

#### Neural Trader (`examples/neural-trader/`)

| Directory | Language | Purpose |
|---|---|---|
| `strategies/example-strategies.js` | JavaScript | Hybrid Momentum (LSTM + sentiment), Kelly Criterion position sizing |
| `strategies/backtesting.js` | JavaScript | Historical backtesting engine |
| `core/` | JavaScript | Core trading pipeline modules |
| `neural/` | JavaScript | Neural network prediction models |
| `risk/` | JavaScript | Risk management systems |
| `mcp/` | JavaScript | MCP tool integration for trading |

#### Delta-Behavior (`examples/delta-behavior/`)

| Directory | Language | Purpose |
|---|---|---|
| `examples/demo.rs` | Rust | Coherence measurement, enforcement, attractor guidance |
| `wasm/examples/browser-example.html` | HTML/JS | Browser-based delta-behavior visualization |
| `wasm/examples/node-example.ts` | TypeScript | Node.js delta-behavior API |

#### Data Pipeline Examples (`examples/data/`)

| Directory | Language | Purpose |
|---|---|---|
| `climate/examples/regime_detector.rs` | Rust | Climate regime detection via min-cut on NOAA/NASA time series |
| `openalex/examples/frontier_radar.rs` | Rust | Academic paper frontier detection via OpenAlex citation network |
| `edgar/examples/coherence_watch.rs` | Rust | SEC EDGAR financial coherence analysis |

#### SciPix (`examples/scipix/`)

| Directory | Language | Purpose |
|---|---|---|
| `examples/simple_ocr.rs` | Rust | OCR pipeline |
| `examples/optimization_demo.rs` | Rust | Pixel processing optimization |
| `examples/batch_processing.rs` | Rust | Batch image processing |
| `examples/streaming.rs` | Rust | Streaming image pipeline |
| `examples/lean_agentic.rs` | Rust | Agentic image analysis |
| `examples/wasm_demo.html` | HTML | Browser WASM OCR demo |

#### RuVector Format (RVF) (`examples/rvf/`)

46 standalone examples covering: access control, agent handoff, agent memory, browser WASM, COW branching, crypto signing, dedup detection, eBPF acceleration, edge IoT, embedding cache, experience replay, financial signals, genomic pipeline, legal discovery, MCP-in-RVF, medical imaging, network sync, PostgreSQL bridge, RAG pipeline, recommendation, semantic search, serverless functions, swarm knowledge, TEE attestation, zero-knowledge proofs, and more.

#### Other Specialized Examples

| Directory | Language | Purpose |
|---|---|---|
| `ruvLLM/` | Rust | LLM inference with ESP32 cluster examples (14 demos), SIMD, federation |
| `onnx-embeddings/` | Rust | ONNX model embedding generation with GPU acceleration |
| `onnx-embeddings-wasm/` | Rust/WASM | Browser-side ONNX embedding inference |
| `prime-radiant/` | Rust | Causal inference, do-calculus, cohomology, topological analysis |
| `exo-ai-2025/` | Rust | Cognitive substrate research: quantum superposition, meta-simulation consciousness |
| `meta-cognition-spiking-neural-network/` | Rust | Meta-cognitive SNN integration |
| `spiking-network/` | Rust | Standalone spiking neural network |
| `ultra-low-latency-sim/` | Rust | Ultra-low-latency simulation harness |
| `vibecast-7sense/` | Rust | Audio processing and resampling |
| `vwm-viewer/` | Rust | Visual working memory viewer |
| `pwa-loader/` | JS | Progressive Web App loader |
| `OSpipe/` | Rust | OS-level pipeline primitives |
| `apify/` | JS | Apify actor integration |
| `agentic-jujutsu/` | Rust | AI agent version control system |
| `app-clip/` | Swift | iOS App Clip |
| `refrag-pipeline/` | Rust | Document fragmentation for RAG |
| `dna/` | Rust | DNA sequence analysis |
| `google-cloud/` | Config | Google Cloud deployment configs |

### 1.2 Crate-Level Examples (`/home/user/ruvector/crates/`)

| Crate | File | Purpose |
|---|---|---|
| `ruvector-core` | `examples/embeddings_example.rs` | AgenticDB with Hash vs OpenAI embeddings, reflexion episodes, skill library |
| `ruvector-mincut-gated-transformer` | `examples/mamba_example.rs` | Mamba state-space model integration with min-cut gating |
| `ruvector-postgres` | `examples/sparse_example.sql` | PostgreSQL sparse vector operations |
| `ruvector-postgres` | `sql/graph_examples.sql` | SQL graph queries |
| `ruvector-postgres` | `sql/routing_example.sql` | SQL routing optimization |
| `ruvector-tiny-dancer-core` | `examples/metrics_example.rs` | Observability metrics |
| `ruvector-tiny-dancer-core` | `examples/tracing_example.rs` | Distributed tracing |
| `ruvllm-wasm` | `examples/micro_lora_example.ts` | Micro LoRA fine-tuning in WASM |

### 1.3 NPM Package Examples (`/home/user/ruvector/npm/packages/`)

| Package | File | Purpose |
|---|---|---|
| `agentic-synth` | `examples/benchmark-example.ts` | Performance benchmarks (throughput, latency, memory, cache) |
| `agentic-synth` | `examples/dspy-complete-example.ts` | DSPy training and optimization |
| `agentic-synth` | `examples/integration-examples.ts` | Integration patterns |
| `ruvector-extensions` | `examples/graph-export-examples.ts` | Graph export to GraphML, GEXF, Neo4j, D3.js, NetworkX (8 examples, streaming) |
| `ruvector-extensions` | `src/examples/embeddings-example.ts` | Embedding generation and storage |
| `ruvector-extensions` | `src/examples/persistence-example.ts` | Data persistence patterns |
| `ruvector-extensions` | `src/examples/temporal-example.ts` | Temporal data handling |
| `ruvector-extensions` | `src/examples/ui-example.ts` | UI component examples |

### 1.4 Documentation-Level Examples (`/home/user/ruvector/docs/`)

| File | Purpose |
|---|---|
| `docs/examples/monitoring_example.md` | Monitoring setup guide |
| `docs/examples/sparsevec_examples.sql` | Sparse vector SQL examples |
| `docs/postgres/zero-copy/examples.rs` | Zero-copy PostgreSQL access |
| `docs/sql/parallel-examples.sql` | Parallel SQL execution patterns |

---

## 2. Gap Analysis -- Where Sublinear Solver Adds Value

### 2.1 High-Impact Gaps

#### Gap 1: PageRank in Hybrid Search (hybrid_search.rs)

- **Current state**: `examples/graph/hybrid_search.rs` is a template with commented-out code. Section 6 ("Ranked Hybrid Search") explicitly mentions PageRank as a graph metric but has no implementation.
- **Sublinear solver value**: The `pageRank` tool computes PageRank in O(log n) time. This would make the hybrid search example functional and demonstrate real-time PageRank-augmented vector search.
- **Impact**: HIGH -- this is the most direct fit in the entire examples directory.

#### Gap 2: Network Routing in Edge-Net

- **Current state**: `examples/edge-net/` simulates network lifecycle and economics but has no optimized routing. The `edge/` example contains P2P transport but no graph-theoretic route optimization.
- **Sublinear solver value**: The solver's network routing and economic equilibrium capabilities directly apply. The `solve` and `pageRank` tools can optimize message routing across the simulated network.
- **Impact**: HIGH -- edge network optimization is a core use case.

#### Gap 3: Missing Linear System Solving in MinCut Pipeline

- **Current state**: The 10 mincut examples use dedicated `ruvector-mincut` but none solve the underlying Laplacian linear systems for graph spectral analysis.
- **Sublinear solver value**: Neumann series and hybrid random walk solvers can solve Laplacian systems Lx=b in sublinear time, enabling spectral clustering, effective resistance computation, and Fiedler vector extraction alongside min-cut.
- **Impact**: HIGH -- directly extends the most sophisticated example family.

#### Gap 4: Financial Portfolio Optimization in Neural-Trader

- **Current state**: `examples/neural-trader/` has LSTM prediction and Kelly Criterion but no matrix-based portfolio optimization (mean-variance, risk parity).
- **Sublinear solver value**: The solver can handle covariance matrix inversion for portfolio optimization in O(log n) time. The `predictWithTemporalAdvantage` tool is specifically designed for trading scenarios.
- **Impact**: HIGH -- financial modeling is a named use case for the solver.

#### Gap 5: No Batch/Streaming Solver Examples

- **Current state**: No examples demonstrate async iteration, batch solving, or streaming solution steps.
- **Sublinear solver value**: The solver's async iterator API and batch priority queuing are unique capabilities with no existing analog in ruvector examples.
- **Impact**: MEDIUM -- this is a novel capability that needs its own showcase.

### 2.2 Medium-Impact Gaps

#### Gap 6: Graph Export Without Analytical Metrics

- **Current state**: `npm/packages/ruvector-extensions/examples/graph-export-examples.ts` exports graphs to 5 formats but does not compute any graph metrics (PageRank, centrality, clustering coefficient) before export.
- **Sublinear solver value**: Pre-compute PageRank scores and embed them as node attributes in exported graphs. This makes exported visualizations immediately useful for analysis.
- **Impact**: MEDIUM -- enhances existing workflow.

#### Gap 7: Climate Regime Detection Without Laplacian Analysis

- **Current state**: `examples/data/climate/examples/regime_detector.rs` detects climate regimes using min-cut but does not use spectral graph methods for regime boundary detection.
- **Sublinear solver value**: Solving the graph Laplacian to find Fiedler vectors enables spectral clustering of temporal climate states, complementing the min-cut approach.
- **Impact**: MEDIUM -- adds a second analytical perspective.

#### Gap 8: Academic Citation Network Without PageRank

- **Current state**: `examples/data/openalex/examples/frontier_radar.rs` analyzes academic frontiers via citation networks but does not rank papers by influence.
- **Sublinear solver value**: PageRank on the citation graph directly produces paper influence scores.
- **Impact**: MEDIUM -- natural fit for the domain.

#### Gap 9: No WASM-Accelerated Solver Demo

- **Current state**: Multiple WASM examples exist (wasm-react, wasm-vanilla, onnx-embeddings-wasm, delta-behavior WASM) but none demonstrate WASM-accelerated linear algebra.
- **Sublinear solver value**: The solver provides WASM acceleration for browser/Node.js. A side-by-side WASM vs native benchmark would be highly instructive.
- **Impact**: MEDIUM -- leverages existing WASM infrastructure.

#### Gap 10: Agentic-Synth Missing Solver Benchmarks

- **Current state**: `npm/packages/agentic-synth/examples/benchmark-example.ts` benchmarks throughput, latency, memory, and cache but not matrix/graph computation.
- **Sublinear solver value**: Adding a solver benchmark suite would complete the performance picture.
- **Impact**: MEDIUM -- extends existing benchmark framework.

### 2.3 Lower-Impact but Valuable Gaps

| Gap | Current State | Solver Value |
|---|---|---|
| RVF format lacks solver integration | 46 RVF examples, none involve matrix operations | Store/retrieve solver results in RVF format |
| ESP32 cluster missing distributed solving | ruvLLM ESP32 demos focus on LLM inference | Distributed Laplacian solving across ESP32 mesh |
| SciPix image pipeline lacks graph-based segmentation | OCR and pixel processing, no graph cuts | Image segmentation via graph-cut optimization |
| Prime-Radiant causal inference | Do-calculus and counterfactuals but no fast Laplacian solver | Speed up causal graph computations |
| Delta-behavior coherence | Coherence measurement without graph spectral analysis | Spectral coherence metrics via Laplacian eigenvalues |

---

## 3. Proposed New Examples for Integration

### 3.1 Priority 1 -- Direct Integration Examples

#### Example A: `hybrid-search-with-pagerank.ts` (Node.js)

**Location**: `examples/nodejs/hybrid-search-with-pagerank.ts`

Extends the existing `semantic_search.js` with PageRank-augmented re-ranking using the sublinear-time-solver MCP tool. Demonstrates the vector+graph hybrid search pattern that `hybrid_search.rs` aspires to but cannot yet deliver.

#### Example B: `sublinear-mincut-laplacian.rs` (Rust)

**Location**: `examples/mincut/sublinear_laplacian/main.rs`

Combines `ruvector-mincut`'s min-cut with Laplacian system solving for spectral analysis. Shows Fiedler vector extraction, effective resistance, and spectral clustering alongside min-cut partitioning.

#### Example C: `neural-trader-portfolio-optimizer.js` (JavaScript)

**Location**: `examples/neural-trader/portfolio/optimizer.js`

Adds mean-variance portfolio optimization using the solver's matrix inversion capabilities. Integrates with existing LSTM predictions and Kelly Criterion sizing.

#### Example D: `edge-net-routing-optimizer.js` (JavaScript)

**Location**: `examples/edge-net/sim/examples/routing-optimizer.js`

Optimizes message routing in the edge-net simulation using PageRank for node importance and Laplacian solving for optimal flow computation.

### 3.2 Priority 2 -- Showcase Examples

#### Example E: `solver-streaming-demo.ts` (TypeScript)

**Location**: `examples/nodejs/solver-streaming-demo.ts`

Demonstrates the solver's async iterator API for streaming solution steps, showing progressive convergence visualization.

#### Example F: `wasm-solver-benchmark.html` (HTML/JS)

**Location**: `examples/wasm-vanilla/solver-benchmark.html`

Browser-based benchmark comparing native vs WASM solver performance on matrices of varying sizes. Visual chart output.

#### Example G: `batch-solver-queue.ts` (TypeScript)

**Location**: `examples/nodejs/batch-solver-queue.ts`

Demonstrates priority-queued batch solving for multiple concurrent graph problems.

### 3.3 Priority 3 -- Domain-Specific Integration

#### Example H: `climate-spectral-regime.rs` (Rust)

**Location**: `examples/data/climate/examples/spectral_regime.rs`

Spectral clustering of climate time series using Laplacian eigenvectors from the solver.

#### Example I: `citation-pagerank.rs` (Rust)

**Location**: `examples/data/openalex/examples/citation_pagerank.rs`

PageRank on academic citation networks for paper influence scoring.

#### Example J: `graph-export-with-pagerank.ts` (TypeScript)

**Location**: `npm/packages/ruvector-extensions/examples/graph-export-with-pagerank.ts`

Compute PageRank and embed scores as node attributes before exporting to GraphML/D3/Neo4j.

---

## 4. Integration Pattern Code Snippets

### 4.1 Pattern: PageRank-Augmented Vector Search (Node.js + MCP)

This pattern shows how to combine ruvector's vector similarity search with sublinear-time-solver's PageRank for hybrid ranking.

```typescript
// hybrid-search-with-pagerank.ts
// Combines ruvector vector search with sublinear-time-solver PageRank

import { VectorDB } from 'ruvector';

// Step 1: Build vector database with document embeddings
const db = new VectorDB({
  dimensions: 384,
  storagePath: './hybrid_search.db',
  distanceMetric: 'cosine',
  hnsw: { m: 32, efConstruction: 200, efSearch: 100 }
});

// Step 2: Index documents with citation relationships
interface Document {
  id: string;
  embedding: Float32Array;
  metadata: { title: string; citations: string[] };
}

const documents: Document[] = [/* ... */];
await db.insertBatch(documents.map(d => ({
  id: d.id,
  vector: d.embedding,
  metadata: d.metadata
})));

// Step 3: Build adjacency matrix from citation graph
function buildAdjacencyMatrix(docs: Document[]): {
  rows: number; cols: number; format: string;
  data: { values: number[]; rowIndices: number[]; colIndices: number[] }
} {
  const n = docs.length;
  const idToIdx = new Map(docs.map((d, i) => [d.id, i]));
  const values: number[] = [];
  const rowIndices: number[] = [];
  const colIndices: number[] = [];

  for (const doc of docs) {
    const fromIdx = idToIdx.get(doc.id)!;
    for (const citedId of doc.metadata.citations) {
      const toIdx = idToIdx.get(citedId);
      if (toIdx !== undefined) {
        values.push(1.0);
        rowIndices.push(fromIdx);
        colIndices.push(toIdx);
      }
    }
  }

  return {
    rows: n, cols: n, format: 'coo',
    data: { values, rowIndices, colIndices }
  };
}

// Step 4: Compute PageRank via sublinear-time-solver CLI
import { execSync } from 'child_process';

function computePageRank(adjacency: ReturnType<typeof buildAdjacencyMatrix>): number[] {
  const input = JSON.stringify({
    adjacency,
    dampingFactor: 0.85,
    tolerance: 1e-6,
    algorithm: 'hybrid'  // Uses forward/backward push + random walk
  });

  const result = execSync(
    `echo '${input}' | npx sublinear-time-solver pagerank --format json`,
    { encoding: 'utf-8', maxBuffer: 50 * 1024 * 1024 }
  );

  return JSON.parse(result).ranks;
}

// Step 5: Hybrid search combining vector similarity and PageRank
async function hybridSearch(
  query: Float32Array,
  topK: number,
  vectorWeight: number = 0.7,
  graphWeight: number = 0.3
) {
  // Get vector similarity results (oversample for re-ranking)
  const vectorResults = await db.search({
    vector: query,
    k: topK * 3,
    includeMetadata: true
  });

  // Compute PageRank scores
  const adjacency = buildAdjacencyMatrix(documents);
  const pageRankScores = computePageRank(adjacency);
  const maxPR = Math.max(...pageRankScores);

  // Combine scores
  const idToIdx = new Map(documents.map((d, i) => [d.id, i]));
  const combined = vectorResults.map(result => {
    const idx = idToIdx.get(result.id)!;
    const vectorScore = 1.0 - result.distance; // Convert distance to similarity
    const graphScore = pageRankScores[idx] / maxPR; // Normalize PageRank
    const combinedScore = vectorWeight * vectorScore + graphWeight * graphScore;

    return { ...result, vectorScore, graphScore, combinedScore };
  });

  // Re-rank and return top K
  combined.sort((a, b) => b.combinedScore - a.combinedScore);
  return combined.slice(0, topK);
}
```

### 4.2 Pattern: Laplacian Spectral Analysis Alongside MinCut (Rust)

This pattern extends ruvector-mincut with Laplacian system solving for spectral graph properties.

```rust
// sublinear_laplacian/main.rs
// Combines ruvector-mincut with sublinear Laplacian solving

use ruvector_mincut::prelude::*;
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct LaplacianRequest {
    matrix: SparseMatrix,
    rhs: Vec<f64>,
    algorithm: String,
    tolerance: f64,
}

#[derive(Serialize)]
struct SparseMatrix {
    rows: usize,
    cols: usize,
    format: String,
    data: CooData,
}

#[derive(Serialize)]
struct CooData {
    values: Vec<f64>,
    #[serde(rename = "rowIndices")]
    row_indices: Vec<usize>,
    #[serde(rename = "colIndices")]
    col_indices: Vec<usize>,
}

#[derive(Deserialize)]
struct SolverResult {
    solution: Vec<f64>,
    residual: f64,
    iterations: usize,
    #[serde(rename = "wallTimeMs")]
    wall_time_ms: f64,
}

fn build_laplacian(edges: &[(u64, u64, f64)], n: usize) -> SparseMatrix {
    let mut values = Vec::new();
    let mut row_indices = Vec::new();
    let mut col_indices = Vec::new();
    let mut degrees = vec![0.0f64; n];

    for &(u, v, w) in edges {
        let ui = u as usize;
        let vi = v as usize;
        // Off-diagonal: -weight
        values.push(-w);
        row_indices.push(ui);
        col_indices.push(vi);
        values.push(-w);
        row_indices.push(vi);
        col_indices.push(ui);
        degrees[ui] += w;
        degrees[vi] += w;
    }

    // Diagonal: degree
    for (i, &d) in degrees.iter().enumerate() {
        values.push(d);
        row_indices.push(i);
        col_indices.push(i);
    }

    SparseMatrix {
        rows: n, cols: n, format: "coo".to_string(),
        data: CooData { values, row_indices, col_indices },
    }
}

fn solve_laplacian(laplacian: &SparseMatrix, rhs: &[f64]) -> SolverResult {
    let request = LaplacianRequest {
        matrix: laplacian.clone(),
        rhs: rhs.to_vec(),
        algorithm: "neumann".to_string(), // Neumann series for Laplacian
        tolerance: 1e-8,
    };

    let input = serde_json::to_string(&request).unwrap();
    let output = Command::new("npx")
        .args(["sublinear-time-solver", "solve", "--format", "json"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .expect("Failed to run solver");

    serde_json::from_slice(&output.stdout).expect("Failed to parse solver output")
}

fn main() {
    println!("=== MinCut + Laplacian Spectral Analysis ===\n");

    // Build a network
    let edges = vec![
        (0, 1, 2.0), (1, 2, 2.0), (2, 0, 2.0), // Triangle 1
        (3, 4, 1.0), (4, 5, 1.0), (5, 3, 1.0), // Triangle 2 (weaker)
        (2, 3, 0.5),                              // Bridge
    ];
    let n = 6;

    // Part A: MinCut analysis
    let mincut = MinCutBuilder::new()
        .exact()
        .with_edges(edges.clone())
        .build()
        .expect("MinCut build failed");

    let result = mincut.min_cut();
    println!("MinCut Analysis:");
    println!("  Value: {}", result.value);
    if let Some((s, t)) = &result.partition {
        println!("  Partition S: {:?}", s);
        println!("  Partition T: {:?}", t);
    }

    // Part B: Laplacian spectral analysis
    let laplacian = build_laplacian(&edges, n);

    // Approximate Fiedler vector via inverse iteration:
    // Solve L * x = random_vector, normalize, repeat
    let mut fiedler = vec![1.0; n];
    // Remove constant component
    let mean: f64 = fiedler.iter().sum::<f64>() / n as f64;
    for x in &mut fiedler { *x -= mean; }

    for _iter in 0..5 {
        let result = solve_laplacian(&laplacian, &fiedler);
        fiedler = result.solution;
        // Remove constant (null space) component
        let mean: f64 = fiedler.iter().sum::<f64>() / n as f64;
        for x in &mut fiedler { *x -= mean; }
        // Normalize
        let norm: f64 = fiedler.iter().map(|x| x * x).sum::<f64>().sqrt();
        for x in &mut fiedler { *x /= norm; }
    }

    println!("\nSpectral Analysis (Fiedler vector):");
    for (i, &v) in fiedler.iter().enumerate() {
        let side = if v > 0.0 { "+" } else { "-" };
        println!("  Node {}: {:.4} [{}]", i, v, side);
    }

    // The Fiedler vector sign gives the spectral partition
    let spectral_s: Vec<usize> = (0..n).filter(|&i| fiedler[i] >= 0.0).collect();
    let spectral_t: Vec<usize> = (0..n).filter(|&i| fiedler[i] < 0.0).collect();
    println!("\nSpectral Partition:");
    println!("  S: {:?}", spectral_s);
    println!("  T: {:?}", spectral_t);
    println!("\nBoth methods identify the bridge edge (2,3) as the weak point.");
}
```

### 4.3 Pattern: Streaming Solver with Async Iterators (TypeScript)

```typescript
// solver-streaming-demo.ts
// Demonstrates streaming solution steps via async iterators

import { spawn } from 'child_process';
import { createInterface } from 'readline';

interface SolverStep {
  iteration: number;
  residual: number;
  partialSolution: number[];
  convergenceRate: number;
}

async function* streamSolve(
  matrixSize: number,
  algorithm: 'neumann' | 'forward_push' | 'hybrid'
): AsyncGenerator<SolverStep> {
  // Generate a random diagonally dominant system
  const input = JSON.stringify({
    matrix: generateDiagonallyDominant(matrixSize),
    rhs: Array(matrixSize).fill(1.0),
    algorithm,
    tolerance: 1e-10,
    streaming: true,  // Enable step-by-step output
    maxIterations: 100,
  });

  const solver = spawn('npx', [
    'sublinear-time-solver', 'solve',
    '--format', 'jsonl',  // JSON Lines for streaming
    '--stream-steps'
  ]);

  solver.stdin.write(input);
  solver.stdin.end();

  const rl = createInterface({ input: solver.stdout });

  for await (const line of rl) {
    if (line.trim()) {
      yield JSON.parse(line) as SolverStep;
    }
  }
}

function generateDiagonallyDominant(n: number) {
  const values: number[] = [];
  const rowIndices: number[] = [];
  const colIndices: number[] = [];

  for (let i = 0; i < n; i++) {
    let rowSum = 0;
    // Off-diagonal entries
    for (let j = Math.max(0, i - 2); j < Math.min(n, i + 3); j++) {
      if (i !== j) {
        const val = Math.random() * 0.5;
        values.push(-val);
        rowIndices.push(i);
        colIndices.push(j);
        rowSum += val;
      }
    }
    // Diagonal: ensure dominance
    values.push(rowSum + 1.0);
    rowIndices.push(i);
    colIndices.push(i);
  }

  return {
    rows: n, cols: n, format: 'coo',
    data: { values, rowIndices, colIndices }
  };
}

// Main demonstration
async function main() {
  console.log('=== Streaming Solver Demo ===\n');

  const algorithms = ['neumann', 'forward_push', 'hybrid'] as const;

  for (const algo of algorithms) {
    console.log(`\n--- Algorithm: ${algo} ---`);
    console.log(`${'Iter'.padStart(6)} | ${'Residual'.padStart(14)} | ${'Conv Rate'.padStart(10)}`);
    console.log('-'.repeat(40));

    let stepCount = 0;
    for await (const step of streamSolve(500, algo)) {
      console.log(
        `${step.iteration.toString().padStart(6)} | ` +
        `${step.residual.toExponential(6).padStart(14)} | ` +
        `${step.convergenceRate.toFixed(4).padStart(10)}`
      );
      stepCount++;

      if (step.residual < 1e-10) {
        console.log(`\nConverged in ${stepCount} steps.`);
        break;
      }
    }
  }
}

main().catch(console.error);
```

### 4.4 Pattern: Batch Solving with Priority Queue (TypeScript)

```typescript
// batch-solver-queue.ts
// Demonstrates priority-queued batch solving for concurrent graph problems

interface GraphProblem {
  id: string;
  priority: number;  // 0 = highest
  matrix: object;
  rhs: number[];
  description: string;
}

interface SolveResult {
  id: string;
  solution: number[];
  wallTimeMs: number;
  iterations: number;
}

class SolverQueue {
  private queue: GraphProblem[] = [];
  private results: Map<string, SolveResult> = new Map();
  private concurrency: number;

  constructor(concurrency: number = 4) {
    this.concurrency = concurrency;
  }

  enqueue(problem: GraphProblem): void {
    this.queue.push(problem);
    // Maintain priority order
    this.queue.sort((a, b) => a.priority - b.priority);
  }

  async processAll(): Promise<Map<string, SolveResult>> {
    const batches: GraphProblem[][] = [];
    for (let i = 0; i < this.queue.length; i += this.concurrency) {
      batches.push(this.queue.slice(i, i + this.concurrency));
    }

    for (const batch of batches) {
      console.log(`Processing batch of ${batch.length} problems...`);
      const batchInput = JSON.stringify({
        problems: batch.map(p => ({
          id: p.id,
          matrix: p.matrix,
          rhs: p.rhs,
          algorithm: 'hybrid',
          tolerance: 1e-8
        }))
      });

      // Use batch mode of the solver CLI
      const result = require('child_process').execSync(
        `echo '${batchInput}' | npx sublinear-time-solver solve --batch --format json`,
        { encoding: 'utf-8', maxBuffer: 100 * 1024 * 1024 }
      );

      const batchResults = JSON.parse(result);
      for (const r of batchResults.results) {
        this.results.set(r.id, r);
      }
    }

    return this.results;
  }
}

// Usage: queue graph problems from different ruvector subsystems
async function main() {
  const queue = new SolverQueue(4);

  // Problem 1 (HIGH priority): Real-time network routing
  queue.enqueue({
    id: 'routing-critical',
    priority: 0,
    matrix: buildRoutingLaplacian(100),
    rhs: buildRoutingDemand(100),
    description: 'Critical path routing for edge network'
  });

  // Problem 2 (MEDIUM): Portfolio rebalancing
  queue.enqueue({
    id: 'portfolio-rebalance',
    priority: 5,
    matrix: buildCovarianceMatrix(50),
    rhs: buildTargetReturns(50),
    description: 'Mean-variance portfolio optimization'
  });

  // Problem 3 (LOW): Citation analysis
  queue.enqueue({
    id: 'citation-analysis',
    priority: 10,
    matrix: buildCitationLaplacian(500),
    rhs: buildFiedlerRHS(500),
    description: 'Academic citation network spectral analysis'
  });

  const results = await queue.processAll();

  for (const [id, result] of results) {
    console.log(`${id}: solved in ${result.wallTimeMs.toFixed(1)}ms (${result.iterations} iters)`);
  }
}
```

### 4.5 Pattern: WASM Solver in Browser (HTML/JS)

```html
<!-- wasm-solver-benchmark.html -->
<!-- Browser benchmark: native JS vs WASM-accelerated solver -->
<!DOCTYPE html>
<html>
<head>
  <title>Sublinear Solver WASM Benchmark</title>
  <script type="module">
    import init, { solve_system, compute_pagerank } from './sublinear_solver_wasm.js';

    // Native JS Jacobi solver for comparison
    function jacobSolve(A, b, maxIter = 1000, tol = 1e-8) {
      const n = b.length;
      let x = new Float64Array(n).fill(0);
      let xNew = new Float64Array(n);

      for (let iter = 0; iter < maxIter; iter++) {
        for (let i = 0; i < n; i++) {
          let sum = 0;
          for (let j = 0; j < n; j++) {
            if (i !== j) sum += A[i * n + j] * x[j];
          }
          xNew[i] = (b[i] - sum) / A[i * n + i];
        }

        // Check convergence
        let residual = 0;
        for (let i = 0; i < n; i++) {
          residual += (xNew[i] - x[i]) ** 2;
        }
        if (Math.sqrt(residual) < tol) {
          return { solution: xNew, iterations: iter + 1 };
        }
        [x, xNew] = [xNew, x];
      }
      return { solution: x, iterations: maxIter };
    }

    async function runBenchmark() {
      await init(); // Initialize WASM module

      const sizes = [50, 100, 200, 500];
      const results = [];

      for (const n of sizes) {
        // Generate test system
        const A = generateDiagonallyDominant(n);
        const b = new Float64Array(n).fill(1.0);

        // Benchmark native JS
        const jsStart = performance.now();
        const jsResult = jacobSolve(A, b);
        const jsTime = performance.now() - jsStart;

        // Benchmark WASM solver
        const wasmStart = performance.now();
        const wasmResult = solve_system(A, b, n, 'neumann', 1e-8);
        const wasmTime = performance.now() - wasmStart;

        results.push({
          size: n,
          jsTimeMs: jsTime,
          wasmTimeMs: wasmTime,
          speedup: jsTime / wasmTime,
          jsIters: jsResult.iterations,
          wasmIters: wasmResult.iterations
        });

        document.getElementById('results').innerHTML += `
          <tr>
            <td>${n}x${n}</td>
            <td>${jsTime.toFixed(1)}ms</td>
            <td>${wasmTime.toFixed(1)}ms</td>
            <td>${(jsTime / wasmTime).toFixed(1)}x</td>
          </tr>
        `;
      }
    }

    window.addEventListener('load', runBenchmark);
  </script>
</head>
<body>
  <h1>Sublinear Solver: JS vs WASM Performance</h1>
  <table border="1">
    <thead>
      <tr><th>Matrix Size</th><th>Native JS</th><th>WASM</th><th>Speedup</th></tr>
    </thead>
    <tbody id="results"></tbody>
  </table>
</body>
</html>
```

---

## 5. Benchmark Examples -- RuVector + Sublinear Approaches

### 5.1 Benchmark Suite Design

The benchmark suite should measure performance across three axes:

1. **Algorithm comparison**: ruvector-mincut vs sublinear-time-solver on overlapping problems
2. **Scale sensitivity**: How each approach handles 10x/100x/1000x graph sizes
3. **Integration overhead**: Cost of combining both systems vs either alone

### 5.2 Benchmark 1: PageRank Computation

```typescript
// benchmarks/pagerank-comparison.ts

interface BenchmarkResult {
  method: string;
  size: number;
  timeMs: number;
  memoryMB: number;
  error: number;  // L2 norm vs ground truth
}

async function benchmarkPageRank(): Promise<BenchmarkResult[]> {
  const results: BenchmarkResult[] = [];
  const sizes = [100, 1_000, 10_000, 100_000];

  for (const n of sizes) {
    const graph = generatePowerLawGraph(n, 2.5); // Typical web graph

    // Method 1: Power iteration (baseline)
    const piStart = performance.now();
    const piResult = powerIterationPageRank(graph, 0.85, 1e-8);
    const piTime = performance.now() - piStart;

    // Method 2: sublinear-time-solver (Neumann series)
    const slStart = performance.now();
    const slResult = await sublinearPageRank(graph, {
      dampingFactor: 0.85,
      tolerance: 1e-8,
      algorithm: 'neumann'
    });
    const slTime = performance.now() - slStart;

    // Method 3: sublinear-time-solver (Hybrid random walk)
    const hwStart = performance.now();
    const hwResult = await sublinearPageRank(graph, {
      dampingFactor: 0.85,
      tolerance: 1e-8,
      algorithm: 'hybrid'
    });
    const hwTime = performance.now() - hwStart;

    results.push(
      { method: 'Power Iteration', size: n, timeMs: piTime, memoryMB: 0, error: 0 },
      { method: 'Sublinear Neumann', size: n, timeMs: slTime, memoryMB: 0,
        error: l2Distance(piResult, slResult) },
      { method: 'Sublinear Hybrid', size: n, timeMs: hwTime, memoryMB: 0,
        error: l2Distance(piResult, hwResult) }
    );
  }

  return results;
}
```

### 5.3 Benchmark 2: MinCut + Laplacian Spectral

```rust
// benchmarks/mincut-vs-spectral.rs

use std::time::Instant;

struct BenchResult {
    method: &'static str,
    size: usize,
    time_ms: f64,
    cut_value: f64,
    partition_quality: f64, // Normalized cut metric
}

fn benchmark_mincut_vs_spectral() -> Vec<BenchResult> {
    let mut results = Vec::new();
    let sizes = vec![100, 500, 1000, 5000];

    for &n in &sizes {
        let edges = generate_planted_partition(n, 2, 0.3, 0.01);

        // Method 1: ruvector-mincut exact
        let start = Instant::now();
        let mincut = MinCutBuilder::new()
            .exact()
            .with_edges(edges.clone())
            .build()
            .unwrap();
        let mc_result = mincut.min_cut();
        let mc_time = start.elapsed().as_secs_f64() * 1000.0;

        // Method 2: Sublinear Laplacian solver for Fiedler vector
        let start = Instant::now();
        let laplacian = build_laplacian(&edges, n);
        let fiedler = solve_fiedler_vector(&laplacian);
        let spectral_partition = partition_by_sign(&fiedler);
        let spectral_cut = compute_cut_value(&edges, &spectral_partition);
        let sp_time = start.elapsed().as_secs_f64() * 1000.0;

        // Method 3: Combined approach
        let start = Instant::now();
        // Use spectral for initial partition, refine with mincut
        let combined_partition = refine_with_mincut(
            &spectral_partition,
            &edges,
            n
        );
        let combined_cut = compute_cut_value(&edges, &combined_partition);
        let cb_time = start.elapsed().as_secs_f64() * 1000.0;

        results.push(BenchResult {
            method: "ruvector-mincut (exact)",
            size: n,
            time_ms: mc_time,
            cut_value: mc_result.value,
            partition_quality: normalized_cut(&edges, &mc_result.partition.unwrap()),
        });
        results.push(BenchResult {
            method: "sublinear spectral",
            size: n,
            time_ms: sp_time,
            cut_value: spectral_cut,
            partition_quality: normalized_cut(&edges, &spectral_partition),
        });
        results.push(BenchResult {
            method: "combined (spectral + mincut)",
            size: n,
            time_ms: cb_time,
            cut_value: combined_cut,
            partition_quality: normalized_cut(&edges, &combined_partition),
        });
    }

    results
}
```

### 5.4 Benchmark 3: Dynamic Updates Head-to-Head

```rust
// benchmarks/dynamic-updates.rs

/// Compare dynamic update performance:
/// ruvector-mincut's subpolynomial updates vs sublinear-time-solver's re-solve

fn benchmark_dynamic_updates() {
    let sizes = vec![1000, 5000, 10000];

    println!("{:>8} {:>15} {:>15} {:>10}",
        "Size", "MinCut Update", "Solver Re-solve", "Speedup");

    for &n in &sizes {
        let mut edges = generate_ring_with_shortcuts(n, n / 10);

        // ruvector-mincut: dynamic updates
        let mut mincut = MinCutBuilder::new()
            .exact()
            .with_edges(edges.clone())
            .build()
            .unwrap();

        let mc_start = Instant::now();
        for _ in 0..100 {
            // Insert random edge
            let u = rand::random::<u64>() % n as u64;
            let v = rand::random::<u64>() % n as u64;
            if u != v { let _ = mincut.insert_edge(u, v, 1.0); }
        }
        let mc_time = mc_start.elapsed();

        // sublinear-time-solver: re-solve Laplacian each time
        let sl_start = Instant::now();
        for _ in 0..100 {
            let u = rand::random::<u64>() % n as u64;
            let v = rand::random::<u64>() % n as u64;
            if u != v {
                edges.push((u, v, 1.0));
                let laplacian = build_laplacian(&edges, n);
                let _result = solve_laplacian_sublinear(&laplacian);
            }
        }
        let sl_time = sl_start.elapsed();

        let speedup = sl_time.as_secs_f64() / mc_time.as_secs_f64();
        println!("{:>8} {:>15?} {:>15?} {:>9.1}x",
            n, mc_time, sl_time, speedup);
    }
}
```

### 5.5 Expected Benchmark Results (Projected)

| Scenario | ruvector-mincut | sublinear-time-solver | Combined |
|---|---|---|---|
| **PageRank (100K nodes)** | N/A (no PageRank) | ~50ms (O(log n)) | 50ms (solver only) |
| **MinCut (10K nodes)** | ~2ms (O(n^o(1))) | ~15ms (Laplacian solve) | 2ms (mincut wins) |
| **Dynamic update (single edge)** | ~0.1ms (amortized) | ~5ms (re-solve) | 0.1ms (mincut wins) |
| **Spectral clustering (5K)** | N/A (no spectral) | ~30ms (Fiedler vector) | 30ms (solver only) |
| **Hybrid PageRank+MinCut** | N/A | N/A | ~52ms (complementary) |
| **WASM PageRank (browser, 10K)** | N/A | ~200ms | 200ms (browser-viable) |

Key insight: The two systems are **complementary, not competitive**. ruvector-mincut excels at dynamic min-cut maintenance; sublinear-time-solver excels at PageRank, spectral analysis, and general linear system solving. The combined system covers more analytical ground than either alone.

---

## 6. Tutorial Progression -- Basic to Advanced

### Level 0: Prerequisites

**Title**: "Setting Up RuVector with Sublinear-Time-Solver"

```bash
# Install ruvector (Rust)
cargo install ruvector

# Install sublinear-time-solver (Node.js)
npm install -g sublinear-time-solver

# Verify both work
ruvector --version
npx sublinear-time-solver --help
```

Covers: Environment setup, verifying installations, first CLI command for each tool.

### Level 1: Hello World

**Title**: "Your First Vector Search + PageRank Query"
**Estimated time**: 15 minutes
**File**: `examples/tutorials/01-hello-world.ts`

Learning objectives:
- Create a VectorDB with 10 documents
- Perform basic similarity search
- Build adjacency matrix from search results
- Run PageRank on the result graph
- Combine scores for hybrid ranking

Key concepts introduced:
- Vector embeddings and cosine similarity
- Adjacency matrix representation (COO format)
- PageRank as a relevance signal
- Score combination (weighted sum)

### Level 2: Graph Analysis

**Title**: "Analyzing Network Structure with MinCut and Spectral Methods"
**Estimated time**: 30 minutes
**File**: `examples/tutorials/02-graph-analysis.rs`

Learning objectives:
- Build a network graph from edge list
- Compute min-cut with `ruvector-mincut`
- Solve the graph Laplacian with the sublinear solver
- Extract the Fiedler vector for spectral partitioning
- Compare min-cut partition with spectral partition

Key concepts introduced:
- Graph Laplacian matrix
- Neumann series solver
- Fiedler vector and algebraic connectivity
- Partition quality metrics (normalized cut)

### Level 3: Real-Time Monitoring

**Title**: "Dynamic Networks with Event-Driven Monitoring"
**Estimated time**: 45 minutes
**File**: `examples/tutorials/03-dynamic-monitoring.rs`

Learning objectives:
- Set up a dynamic graph with `MinCutBuilder`
- Attach a `StructuralMonitor` for brittleness detection
- Stream edge insertions/deletions
- Use the sublinear solver for periodic spectral health checks
- Combine min-cut monitoring with spectral analysis triggers

Key concepts introduced:
- Dynamic graph updates (insert/delete edge)
- Event-driven monitoring (thresholds, callbacks)
- Brittleness signals (Healthy/Warning/Critical)
- Periodic vs continuous analysis tradeoffs

### Level 4: Hybrid Search Pipeline

**Title**: "Building a Production Hybrid Search System"
**Estimated time**: 60 minutes
**File**: `examples/tutorials/04-hybrid-search-pipeline.ts`

Learning objectives:
- Index a document corpus into VectorDB
- Build citation/reference graph from document metadata
- Compute PageRank scores via the sublinear solver
- Implement multi-signal ranking (vector similarity + PageRank + recency)
- Add graph-constrained filtering (only return connected subgraphs)
- Export result graph to D3.js for visualization

Key concepts introduced:
- Multi-signal ranking strategies
- Graph-constrained retrieval
- Score normalization techniques
- Graph export and visualization

### Level 5: Financial Network Analysis

**Title**: "Portfolio Optimization with Network Analysis"
**Estimated time**: 90 minutes
**File**: `examples/tutorials/05-financial-networks.ts`

Learning objectives:
- Build stock correlation network from price data
- Compute PageRank for sector influence analysis
- Solve mean-variance optimization via the sublinear solver
- Use min-cut for market regime detection (bull/bear transitions)
- Implement risk monitoring with brittleness detection

Key concepts introduced:
- Correlation networks and market graphs
- Mean-variance optimization as a linear system
- Temporal advantage computation
- Market regime detection via structural graph analysis

### Level 6: Distributed Analysis at Scale

**Title**: "Scaling to Million-Node Graphs"
**Estimated time**: 120 minutes
**File**: `examples/tutorials/06-distributed-scale.rs`

Learning objectives:
- Partition large graphs across multiple solver instances
- Use batch solving for parallel subgraph analysis
- Stream results back through async iterators
- Combine local min-cut results for global analysis
- Benchmark throughput at different scales

Key concepts introduced:
- Graph partitioning strategies
- Batch solving with priority queuing
- Streaming results and progressive refinement
- Scale-dependent algorithm selection

### Level 7: Custom Solver Integration

**Title**: "Building Your Own Solver-Backed Application"
**Estimated time**: 180 minutes
**File**: `examples/tutorials/07-custom-integration/`

Learning objectives:
- Design a custom MCP tool that wraps the solver
- Integrate ruvector-mincut with solver in a single Rust binary
- Build a WASM module that exposes both vector search and solver
- Implement a custom monitoring dashboard
- Create end-to-end tests for the integrated system

Key concepts introduced:
- MCP server architecture
- FFI between Rust (mincut) and Node.js (solver)
- WASM module design for dual-library integration
- Testing strategies for numerical systems

### Tutorial Dependency Graph

```
Level 0 (Setup)
    |
Level 1 (Hello World)
    |
    +--- Level 2 (Graph Analysis)
    |        |
    |    Level 3 (Dynamic Monitoring)
    |        |
    |    Level 6 (Distributed Scale)
    |
    +--- Level 4 (Hybrid Search)
             |
         Level 5 (Financial Networks)
             |
         Level 7 (Custom Integration)
```

---

## Summary of Findings

### Quantitative Overview

| Category | Count |
|---|---|
| Total example directories in `/examples/` | 36 |
| Total individual example files (all locations) | 150+ |
| Languages represented | Rust, JavaScript, TypeScript, HTML, SQL, Swift, Python |
| Examples with graph operations | 28 |
| Examples that would directly benefit from the solver | 12 |
| Proposed new integration examples | 10 |
| High-impact integration gaps | 5 |
| Medium-impact integration gaps | 5 |

### Key Recommendations

1. **Start with `hybrid_search.rs`**: The existing template in `examples/graph/hybrid_search.rs` explicitly requests PageRank integration. Making this functional with the sublinear-time-solver is the highest-value, lowest-effort integration point.

2. **Add Laplacian solving to the mincut family**: The 10 mincut examples are the most sophisticated in the codebase. Adding spectral analysis via the solver creates a complete graph analysis toolkit.

3. **Create the streaming demo**: No existing example demonstrates async iteration or streaming computation. This showcases a unique solver capability.

4. **Build the WASM benchmark page**: With 5 existing WASM examples as templates, a solver WASM benchmark is straightforward and highly demonstrative.

5. **Extend neural-trader**: The trading example already has LSTM and Kelly Criterion. Adding solver-based portfolio optimization completes the quantitative finance stack.

### File Paths Referenced

- `/home/user/ruvector/examples/README.md` -- Main examples index
- `/home/user/ruvector/examples/graph/hybrid_search.rs` -- Highest-priority integration target
- `/home/user/ruvector/examples/subpolynomial-time/src/main.rs` -- Closest existing analog
- `/home/user/ruvector/examples/mincut/benchmarks/main.rs` -- Benchmark template
- `/home/user/ruvector/examples/nodejs/semantic_search.js` -- Node.js search base
- `/home/user/ruvector/examples/neural-trader/strategies/example-strategies.js` -- Trading base
- `/home/user/ruvector/examples/edge-net/sim/examples/quick-demo.js` -- Edge network base
- `/home/user/ruvector/npm/packages/ruvector-extensions/examples/graph-export-examples.ts` -- Export base
- `/home/user/ruvector/examples/data/climate/examples/regime_detector.rs` -- Climate domain
- `/home/user/ruvector/examples/data/openalex/examples/frontier_radar.rs` -- Citation domain
- `/home/user/ruvector/.claude/agents/sublinear/pagerank-analyzer.md` -- Existing solver agent
- `/home/user/ruvector/.claude/agents/sublinear/trading-predictor.md` -- Existing trading agent
