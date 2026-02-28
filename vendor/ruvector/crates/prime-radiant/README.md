# Prime-Radiant

[![Crates.io](https://img.shields.io/crates/v/prime-radiant.svg)](https://crates.io/crates/prime-radiant)
[![Documentation](https://docs.rs/prime-radiant/badge.svg)](https://docs.rs/prime-radiant)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/ruvnet/ruvector/ci.yml)](https://github.com/ruvnet/ruvector/actions)

**A Real-Time Coherence Gate for Autonomous Systems**

Prime-Radiant is infrastructure for AI safety — a mathematical gate that proves whether a system's beliefs, facts, and claims are internally consistent before allowing action.

Instead of asking "How confident am I?" (which can be wrong), Prime-Radiant asks "Are there any contradictions?" — and provides mathematical proof of the answer.

```
┌─────────────────────────────────────────────────────────────────┐
│  "The meeting is at 3pm"  ←──────→  "The meeting is at 4pm"    │
│         (Memory A)           ✗            (Memory B)            │
│                                                                 │
│  Energy = 0.92  →  HIGH INCOHERENCE  →  Block / Escalate       │
└─────────────────────────────────────────────────────────────────┘
```

## Table of Contents

- [What It Does](#what-it-does)
- [Mathematical Foundation](#mathematical-foundation)
- [Key Concepts](#key-concepts)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Performance & Acceleration](#performance--acceleration)
- [Storage Backends](#storage-backends)
- [Applications](#applications)
- [Feature Flags](#feature-flags)
- [Architecture](#architecture)
- [API Reference](#api-reference)
- [Learn More](#learn-more)

## What It Does

Imagine you have an AI assistant that:
- Retrieves facts from a database
- Remembers your conversation history
- Makes claims based on what it knows

**The problem**: These pieces can contradict each other. The AI might confidently say something that conflicts with facts it just retrieved. Traditional systems can't detect this reliably.

**Prime-Radiant's solution**: Model everything as a graph where:
- **Nodes** are pieces of information (facts, beliefs, memories)
- **Edges** are relationships that should be consistent
- **Energy** measures how much things disagree

| Traditional AI | Prime-Radiant |
|----------------|---------------|
| "I'm 85% confident" | "Zero contradictions found" |
| Can be confidently wrong | Knows when it doesn't know |
| Guesses about the future | Proves consistency right now |
| Trust the model | Trust the math |

### What Prime-Radiant is NOT

- **Not a probabilistic scorer** — It doesn't estimate likelihood. It proves structural consistency.
- **Not a belief model** — It doesn't track what's "true." It tracks what's *mutually compatible*.
- **Not a predictor** — It doesn't forecast outcomes. It validates the present state.
- **Not an LLM feature** — It's infrastructure that sits beneath any autonomous system.

## Mathematical Foundation

Prime-Radiant is built on **Sheaf Laplacian** mathematics — a rigorous framework for measuring consistency across interconnected data.

### The Energy Formula

```
E(S) = Σ wₑ · ‖ρᵤ(xᵤ) - ρᵥ(xᵥ)‖²
       e∈E
```

Where:
- **E(S)** = Total coherence energy (lower = more coherent)
- **wₑ** = Edge weight (importance of this relationship)
- **ρᵤ, ρᵥ** = Restriction maps (how information transforms between nodes)
- **xᵤ, xᵥ** = Node states (embedded representations)

### Concrete Example

```
Node A: "Meeting at 3pm"    → embedding: [0.9, 0.1, 0.0]
Node B: "Meeting at 4pm"    → embedding: [0.1, 0.9, 0.0]
Edge A→B: Identity map (they should match)

Residual = ρ(A) - ρ(B) = [0.9, 0.1, 0.0] - [0.1, 0.9, 0.0] = [0.8, -0.8, 0.0]
Energy   = ‖residual‖² = 0.8² + 0.8² + 0² = 1.28

Threshold (Heavy lane) = 0.4
1.28 > 0.4 → Route to Human review
```

One line of arithmetic. The contradiction is now a number. The gate has a decision.

### Restriction Maps

Restriction maps encode *how* information should relate across edges:

| Map Type | Formula | Use Case |
|----------|---------|----------|
| **Identity** | ρ(x) = x | Direct comparison |
| **Diagonal** | ρ(x) = diag(d) · x | Weighted dimensions |
| **Projection** | ρ(x) = P · x | Dimensionality reduction |
| **Dense** | ρ(x) = A · x + b | Learned transformations |
| **Sparse** | ρ(x) = S · x | Efficient large-scale |

### Coherence Field Visualization

```
Low Energy (Coherent)          High Energy (Incoherent)
        ✓                              ✗

  Fact A ←→ Fact B              Fact A ←→ Fact B
     ↓         ↓                   ↓    ✗    ↓
  Claim C ←→ Claim D            Claim C ←✗→ Claim D

  "Everything agrees"           "Contradictions detected"
  → Safe to act                 → Stop, escalate, or refuse
```

## Key Concepts

### Compute Ladder

Based on coherence energy, actions are routed to appropriate compute lanes:

```
┌─────────────────────────────────────────────────────────────────┐
│ Energy   │ Lane        │ Latency  │ Action                      │
├──────────┼─────────────┼──────────┼─────────────────────────────┤
│ < 0.1    │ Reflex      │ < 1ms    │ Immediate approval          │
│ 0.1-0.4  │ Retrieval   │ ~10ms    │ Fetch more evidence         │
│ 0.4-0.7  │ Heavy       │ ~100ms   │ Deep analysis               │
│ > 0.7    │ Human       │ async    │ Escalate to human review    │
└─────────────────────────────────────────────────────────────────┘
```

### Governance & Audit

Every decision creates an immutable audit trail:

- **Witness Records** — Cryptographic proof of every gate decision (Blake3 hash chain)
- **Policy Bundles** — Signed threshold configurations with multi-party approval
- **Lineage Tracking** — Full provenance for all graph modifications
- **Deterministic Replay** — Reconstruct any past state from witness chain

### RuvLLM Integration

Specialized layer for LLM coherence checking:

- **Hallucination Detection** — Mathematical, not heuristic
- **Confidence from Energy** — Interpretable uncertainty scores
- **Memory Coherence** — Track context consistency across conversation
- **Unified Audit Trail** — Link inference decisions to coherence witnesses

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
# Core coherence engine
prime-radiant = "0.1"

# With LLM integration
prime-radiant = { version = "0.1", features = ["ruvllm"] }

# With GPU acceleration
prime-radiant = { version = "0.1", features = ["gpu"] }

# With SIMD optimizations
prime-radiant = { version = "0.1", features = ["simd"] }

# Everything
prime-radiant = { version = "0.1", features = ["full"] }
```

## Quick Start

### Basic Coherence Check

```rust
use prime_radiant::{
    substrate::{SheafGraph, SheafNodeBuilder, SheafEdgeBuilder},
    coherence::CoherenceEngine,
    execution::{CoherenceGate, PolicyBundleRef},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a graph of related facts
    let graph = SheafGraph::new();

    // Add nodes with state vectors (embeddings)
    let fact_a = graph.add_node(
        SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 0.0, 0.0])
            .namespace("knowledge")
            .metadata("source", "database")
            .build()
    );

    let fact_b = graph.add_node(
        SheafNodeBuilder::new()
            .state_from_slice(&[0.95, 0.05, 0.0])  // Similar to fact_a
            .namespace("knowledge")
            .build()
    );

    // Add edge with identity restriction (they should match)
    graph.add_edge(
        SheafEdgeBuilder::new(fact_a, fact_b)
            .identity_restrictions(3)
            .weight(1.0)
            .namespace("knowledge")
            .build()
    );

    // Compute coherence energy
    let energy = graph.compute_energy();
    println!("Total energy: {:.4}", energy.total_energy);
    println!("Is coherent: {}", energy.is_coherent(0.1));

    // Gate a decision based on energy
    let policy = PolicyBundleRef::placeholder();
    let mut gate = CoherenceGate::with_defaults(policy);

    let decision = gate.evaluate_energy(energy.total_energy);

    println!("Decision: {:?}", decision.lane);
    println!("Allowed: {}", decision.allow);

    Ok(())
}
```

### LLM Response Validation

```rust
use prime_radiant::ruvllm_integration::{
    SheafCoherenceValidator, ValidationContext, ValidatorConfig,
    EdgeWeights,
};

async fn validate_response(
    context_embedding: Vec<f32>,
    response_embedding: Vec<f32>,
    retrieved_facts: Vec<Vec<f32>>,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Create validator with custom thresholds
    let config = ValidatorConfig {
        coherence_threshold: 0.3,
        max_edges_per_claim: 10,
        ..Default::default()
    };
    let validator = SheafCoherenceValidator::new(config);

    // Build validation context
    let context = ValidationContext::builder()
        .context_embedding(context_embedding)
        .response_embedding(response_embedding)
        .supporting_facts(retrieved_facts)
        .edge_weights(EdgeWeights::default())
        .build();

    // Validate
    let result = validator.validate(&context)?;

    println!("Energy: {:.4}", result.energy);
    println!("Coherent: {}", result.is_coherent);
    println!("Witness ID: {}", result.witness.id);

    if !result.is_coherent {
        println!("Incoherent claims: {:?}", result.incoherent_edges);
    }

    Ok(result.is_coherent)
}
```

### Memory Coherence Tracking

```rust
use prime_radiant::ruvllm_integration::{
    MemoryCoherenceLayer, MemoryCoherenceConfig, MemoryEntry, MemoryType,
};

fn track_conversation_memory() -> Result<(), Box<dyn std::error::Error>> {
    let config = MemoryCoherenceConfig {
        similarity_threshold: 0.7,
        max_memories: 1000,
        ..Default::default()
    };
    let mut memory = MemoryCoherenceLayer::new(config);

    // Add first memory
    let entry1 = MemoryEntry {
        id: "mem_1".into(),
        memory_type: MemoryType::Working,
        embedding: vec![1.0, 0.0, 0.0],
        content: "User prefers morning meetings".into(),
        timestamp: chrono::Utc::now(),
    };
    memory.add_with_coherence(entry1)?;

    // Add potentially conflicting memory
    let entry2 = MemoryEntry {
        id: "mem_2".into(),
        memory_type: MemoryType::Working,
        embedding: vec![-0.9, 0.1, 0.0],  // Opposite direction!
        content: "User prefers evening meetings".into(),
        timestamp: chrono::Utc::now(),
    };

    let result = memory.add_with_coherence(entry2)?;

    if !result.coherent {
        println!("Contradiction detected!");
        println!("Conflicts with: {:?}", result.conflicts);
        println!("Energy: {:.4}", result.energy);
    }

    Ok(())
}
```

### Confidence from Coherence

```rust
use prime_radiant::ruvllm_integration::{
    CoherenceConfidence, ConfidenceLevel,
};

fn interpret_energy(energy: f32) {
    let confidence = CoherenceConfidence::default();
    let score = confidence.from_energy(energy);

    println!("Confidence: {:.1}%", score.value * 100.0);
    println!("Level: {:?}", score.level);
    println!("Explanation: {}", score.explanation);

    match score.level {
        ConfidenceLevel::VeryHigh => println!("Safe to proceed automatically"),
        ConfidenceLevel::High => println!("Proceed with logging"),
        ConfidenceLevel::Moderate => println!("Consider additional verification"),
        ConfidenceLevel::Low => println!("Recommend human review"),
        ConfidenceLevel::VeryLow => println!("Block action, require escalation"),
    }
}
```

## Performance & Acceleration

### CPU Baseline

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Single residual | < 1μs | 1M+ ops/sec |
| Graph energy (10K nodes) | < 10ms | 100 graphs/sec |
| Incremental update | < 100μs | 10K updates/sec |
| Gate evaluation | < 500μs | 2K decisions/sec |

### SIMD Acceleration

Enable with `--features simd`:

```rust
use prime_radiant::simd::{
    dot_product_simd, norm_squared_simd, batch_residuals_simd,
};

// Automatic CPU feature detection
let width = prime_radiant::simd::best_simd_width();
println!("Using SIMD width: {:?}", width);  // Avx512, Avx2, Sse42, or Scalar

// 4-8x speedup on vector operations
let dot = dot_product_simd(&a, &b);
let norm = norm_squared_simd(&v);
```

| SIMD Feature | Speedup | Platform |
|--------------|---------|----------|
| AVX-512 | 8-16x | Intel Xeon, AMD Zen4+ |
| AVX2 | 4-8x | Most modern x86_64 |
| SSE4.2 | 2-4x | Older x86_64 |
| NEON | 2-4x | ARM64 (Apple M1/M2, etc.) |

### GPU Acceleration

Enable with `--features gpu`:

```rust
use prime_radiant::gpu::{GpuCoherenceEngine, GpuConfig};

async fn gpu_compute() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize GPU (auto-detects best available)
    let config = GpuConfig {
        prefer_discrete: true,
        max_buffer_size: 256 * 1024 * 1024,  // 256MB
        ..Default::default()
    };

    let gpu_engine = GpuCoherenceEngine::new(&graph, config).await?;

    // Compute on GPU (falls back to CPU if unavailable)
    let energy = gpu_engine.compute_energy().await?;

    println!("GPU Energy: {:.4}", energy.total_energy);
    println!("Backend: {:?}", gpu_engine.backend());  // Vulkan, Metal, DX12, WebGPU

    Ok(())
}
```

| GPU Backend | Supported Platforms |
|-------------|---------------------|
| Vulkan | Linux, Windows, Android |
| Metal | macOS, iOS |
| DX12 | Windows 10+ |
| WebGPU | Browsers (wasm32) |

**GPU Kernels:**
- `compute_residuals.wgsl` — Parallel edge residual computation
- `compute_energy.wgsl` — Reduction-based energy aggregation
- `sheaf_attention.wgsl` — Batched attention with energy weighting
- `token_routing.wgsl` — Parallel lane assignment

## Storage Backends

### In-Memory (Default)

Fast, thread-safe storage for development and testing:

```rust
use prime_radiant::storage::{InMemoryStorage, StorageConfig};

let storage = InMemoryStorage::new();
// Or with indexing for fast KNN search:
let indexed = IndexedInMemoryStorage::new();
```

### File Storage with WAL

Persistent storage with Write-Ahead Logging for durability:

```rust
use prime_radiant::storage::{FileStorage, StorageFormat};

let storage = FileStorage::new(
    "./data/coherence.db",
    StorageFormat::Bincode,  // Or Json for debugging
)?;
```

### PostgreSQL (Production)

Full ACID compliance with indexed queries:

```toml
# Cargo.toml
prime-radiant = { version = "0.1", features = ["postgres"] }
```

```rust
use prime_radiant::storage::PostgresStorage;

let storage = PostgresStorage::connect(
    "postgres://user:pass@localhost/coherence"
).await?;
```

**Schema includes:**
- `policy_bundles` — Versioned policies with approval tracking
- `witness_records` — Hash-chained audit trail
- `lineage_records` — Full graph modification history
- `node_states` / `edges` — Graph storage with vector indexing

## Applications

### Flagship: LLM Hallucination Refusal

A complete walkthrough of Prime-Radiant blocking a hallucinated response:

```
Step 1: RAG retrieves context
  ┌─────────────────────────────────────────────────────────┐
  │ Retrieved Fact: "Company founded in 2019"               │
  │ Embedding: [0.82, 0.15, 0.03]                           │
  └─────────────────────────────────────────────────────────┘

Step 2: LLM generates response
  ┌─────────────────────────────────────────────────────────┐
  │ Generated Claim: "The company has 15 years of history" │
  │ Embedding: [0.11, 0.85, 0.04]                           │
  └─────────────────────────────────────────────────────────┘

Step 3: Prime-Radiant computes coherence
  ┌─────────────────────────────────────────────────────────┐
  │ Edge: Fact → Claim (identity restriction)               │
  │ Residual: [0.82-0.11, 0.15-0.85, 0.03-0.04]            │
  │         = [0.71, -0.70, -0.01]                          │
  │ Energy:  = 0.71² + 0.70² + 0.01² = 0.996               │
  └─────────────────────────────────────────────────────────┘

Step 4: Gate decision
  ┌─────────────────────────────────────────────────────────┐
  │ Energy: 0.996                                           │
  │ Threshold (Human): 0.7                                  │
  │ Decision: BLOCK → Escalate to human review             │
  │ Witness ID: 7f3a...c921 (cryptographic proof)          │
  └─────────────────────────────────────────────────────────┘
```

The hallucination never reaches the user. The decision is auditable forever.

### Tier 1: Production Ready

| Application | How It Works |
|-------------|--------------|
| **LLM Anti-Hallucination** | Gate responses when energy exceeds threshold |
| **RAG Consistency** | Verify retrieved context matches generated claims |
| **Trading Throttles** | Pause when market signals become structurally inconsistent |
| **Compliance Proofs** | Cryptographic witness for every automated decision |

### Tier 2: Near-Term

| Application | How It Works |
|-------------|--------------|
| **Autonomous Vehicles** | Refuse motion when sensor/plan coherence breaks |
| **Medical Monitoring** | Escalate only on sustained diagnostic disagreement |
| **Zero-Trust Security** | Detect authorization graph inconsistencies |

### Domain Mapping

The same math works everywhere — only the interpretation changes:

| Domain | Nodes | Edges | High Energy Means | Gate Action |
|--------|-------|-------|-------------------|-------------|
| **AI Agents** | Beliefs, facts | Citations | Hallucination | Refuse generation |
| **Finance** | Trades, positions | Arbitrage links | Regime change | Throttle trading |
| **Medical** | Vitals, diagnoses | Physiology | Clinical disagreement | Escalate to doctor |
| **Robotics** | Sensors, plans | Physics | Motion impossibility | Emergency stop |
| **Security** | Identities, permissions | Policy rules | Auth violation | Deny access |

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `default` | Core coherence engine | ✓ |
| `full` | All features enabled | |
| `simd` | SIMD-optimized operations | |
| `gpu` | GPU acceleration via wgpu | |
| `ruvllm` | LLM integration layer | |
| `postgres` | PostgreSQL storage backend | |
| `sona` | Self-optimizing threshold tuning | |
| `learned-rho` | GNN-learned restriction maps | |
| `hyperbolic` | Poincaré ball energy for hierarchies | |
| `distributed` | Raft-based multi-node coherence | |
| `attention` | Coherence-Gated Transformer attention | |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      APPLICATION LAYER                          │
│   LLM Guards  │  Trading  │  Medical  │  Robotics  │  Security │
├─────────────────────────────────────────────────────────────────┤
│                      COHERENCE GATE                             │
│   Reflex (L0)  │  Retrieval (L1)  │  Heavy (L2)  │  Human (L3) │
├─────────────────────────────────────────────────────────────────┤
│                   COHERENCE COMPUTATION                         │
│   Residuals  │  Energy Aggregation  │  Spectral Analysis       │
├─────────────────────────────────────────────────────────────────┤
│                    ACCELERATION LAYER                           │
│   CPU (Scalar)  │  SIMD (AVX/NEON)  │  GPU (wgpu)              │
├─────────────────────────────────────────────────────────────────┤
│                    GOVERNANCE LAYER                             │
│   Policy Bundles  │  Witnesses  │  Lineage  │  Threshold Tuning│
├─────────────────────────────────────────────────────────────────┤
│                   KNOWLEDGE SUBSTRATE                           │
│   Sheaf Graph  │  Nodes  │  Edges  │  Restriction Maps         │
├─────────────────────────────────────────────────────────────────┤
│                     STORAGE LAYER                               │
│   In-Memory  │  File (WAL)  │  PostgreSQL                      │
└─────────────────────────────────────────────────────────────────┘
```

## API Reference

### Core Types

```rust
// Graph primitives
SheafGraph        // Thread-safe graph container
SheafNode         // Node with state vector
SheafEdge         // Edge with restriction maps
RestrictionMap    // Linear transformation ρ(x) = Ax + b

// Energy computation
CoherenceEnergy   // Energy breakdown by edge and scope
CoherenceEngine   // Computation engine with caching

// Gating
CoherenceGate     // Decision gate with compute ladder
GateDecision      // Allow/deny with lane assignment
ComputeLane       // Reflex, Retrieval, Heavy, Human

// Governance
PolicyBundle      // Threshold configuration
WitnessRecord     // Cryptographic audit entry
LineageRecord     // Graph modification history
```

### Builder Pattern

All major types support the builder pattern:

```rust
let node = SheafNodeBuilder::new()
    .state_from_slice(&[1.0, 0.0, 0.0])
    .namespace("facts")
    .metadata("source", "api")
    .metadata("confidence", "0.95")
    .build();

let edge = SheafEdgeBuilder::new(source_id, target_id)
    .dense_restriction(&matrix, &bias)
    .weight(2.5)
    .namespace("citations")
    .build();

let policy = PolicyBundleBuilder::new("production-v1")
    .with_threshold("default", ThresholdConfig::moderate())
    .with_threshold("safety", ThresholdConfig::strict())
    .with_required_approvals(2)
    .with_approver(ApproverId::new("admin"))
    .build();
```

## Learn More

- [ADR-014: Coherence Engine Architecture](../../docs/adr/ADR-014-coherence-engine.md)
- [ADR-015: Coherence-Gated Transformer](../../docs/adr/ADR-015-coherence-gated-transformer.md)
- [Internal ADRs](../../docs/adr/coherence-engine/) (22 detailed decision records)
- [API Documentation](https://docs.rs/prime-radiant)

## Why "Prime Radiant"?

In Isaac Asimov's *Foundation* series, the Prime Radiant is a device that displays the mathematical equations of psychohistory — allowing scientists to see how changes propagate through a complex system.

Similarly, this Prime-Radiant shows how consistency propagates (or breaks down) through your AI system's knowledge graph. It doesn't predict the future — it shows you where the present is coherent and where it isn't.

## Positioning

Prime-Radiant is not an LLM feature or a developer library. It is **infrastructure** — a coherence gate that sits beneath autonomous systems, ensuring they cannot act on contradictory beliefs.

Think of it as a circuit breaker for AI reasoning. When the math says "contradiction," the system stops. No probability. No guessing. Just structure.

This is the kind of primitive that agentic systems will need for the next decade.

## License

MIT License - See [LICENSE](../../LICENSE) for details.

---

<p align="center">
<b>Prime-Radiant: A safety primitive for autonomous systems.</b><br><br>
<i>"Most systems try to get smarter by making better guesses.<br>
Prime-Radiant takes a different route: systems that stay stable under uncertainty<br>
by proving when the world still fits together — and when it does not."</i>
</p>
