# Prime-Radiant: Universal Coherence Engine

**Advanced Mathematical Framework for AI Safety, Hallucination Detection, and Structural Consistency Verification**

Prime-Radiant implements a universal coherence engine using sheaf Laplacian mathematics to provide structural consistency guarantees across domains. Rather than trying to make better predictions, Prime-Radiant proves when the world still fits together and when it does not.

---

## Table of Contents

1. [Overview](#overview)
2. [Six Mathematical Directions](#six-mathematical-directions)
3. [Installation](#installation)
4. [Quick Start](#quick-start)
5. [API Reference](#api-reference)
6. [Performance Characteristics](#performance-characteristics)
7. [Use Cases](#use-cases)
8. [Architecture](#architecture)

---

## Overview

Prime-Radiant provides a **single underlying coherence object** that can be interpreted across multiple domains:

| Domain | Nodes Are | Edges Are | Residual Becomes | Gate Becomes |
|--------|-----------|-----------|------------------|--------------|
| **AI Agents** | Facts, hypotheses, beliefs | Citations, logical implication | Contradiction energy | Hallucination refusal |
| **Finance** | Trades, positions, signals | Market dependencies, arbitrage | Regime mismatch | Trading throttle |
| **Medical** | Vitals, diagnoses, treatments | Physiological causality | Clinical disagreement | Escalation trigger |
| **Robotics** | Sensor readings, goals, plans | Physics, kinematics | Motion impossibility | Safety stop |
| **Security** | Identities, permissions, actions | Policy rules, trust chains | Authorization violation | Access denial |
| **Science** | Hypotheses, observations, models | Experimental evidence | Theory inconsistency | Pruning signal |

### Core Mathematical Formula

The coherence energy is computed as:

```
E(S) = sum(w_e * ||r_e||^2)

where r_e = rho_u(x_u) - rho_v(x_v)
```

- **rho**: Restriction map (linear transform defining how states constrain each other)
- **r_e**: Residual at edge (measures local inconsistency)
- **w_e**: Edge weight
- **E(S)**: Global incoherence measure

---

## Six Mathematical Directions

Prime-Radiant implements six advanced mathematical frameworks for coherence analysis:

### 1. Sheaf Cohomology for AI Coherence

Sheaf theory provides the mathematical foundation for understanding local-to-global consistency:

- **Stalks**: Fixed-dimensional state vectors at each node
- **Restriction Maps**: Constraints defining how states relate
- **Global Sections**: Coherent assignments across the entire graph
- **Cohomology Groups**: Obstruction measures for global consistency

[ADR-001: Sheaf Cohomology](docs/adr/ADR-001-sheaf-cohomology.md)

### 2. Category Theory and Topos-Theoretic Belief Models

Functorial retrieval and higher category structures enable:

- **Functorial Retrieval**: Structure-preserving knowledge access
- **Topos Models**: Intuitionistic logic for belief systems
- **Higher Categories**: Multi-level coherence laws
- **Natural Transformations**: Systematic relationship mapping

[ADR-002: Category and Topos Theory](docs/adr/ADR-002-category-topos.md)

### 3. Homotopy Type Theory for Verified Reasoning

HoTT provides verified reasoning with proof transport:

- **Univalence Axiom**: Equivalent structures are identical
- **Path Induction**: Proofs follow identity paths
- **Higher Inductive Types**: Complex data structures with equalities
- **Proof Transport**: Transfer proofs across equivalent structures

[ADR-003: Homotopy Type Theory](docs/adr/ADR-003-homotopy-type-theory.md)

### 4. Spectral Invariants for Cut Prediction

Spectral analysis of the sheaf Laplacian enables:

- **Cheeger Bounds**: Relationship between spectral gap and graph cuts
- **Algebraic Connectivity**: Second eigenvalue measures graph cohesion
- **Early Warning Systems**: Detect structural weakening before failure
- **Drift Detection**: Identify fundamental structural shifts

[ADR-004: Spectral Invariants](docs/adr/ADR-004-spectral-invariants.md)

### 5. Causal Abstraction for Consistency

Causal reasoning distinguishes correlation from causation:

- **Do-Calculus**: Intervention-based causal reasoning
- **Structural Causal Models**: Explicit causal relationships
- **Abstraction Verification**: Ensure high-level models match low-level
- **Counterfactual Analysis**: "What if" reasoning support

[ADR-005: Causal Abstraction](docs/adr/ADR-005-causal-abstraction.md)

### 6. Quantum Topology for Coherence Invariants

Topological methods provide robust coherence measures:

- **Persistent Homology**: Multi-scale topological features
- **Betti Numbers**: Counts of topological holes
- **Quantum-Inspired Encodings**: Superposition-based representations
- **Stability Theorems**: Robustness guarantees for features

[ADR-006: Quantum Topology](docs/adr/ADR-006-quantum-topology.md)

---

## Installation

### Rust (Native)

Add to your `Cargo.toml`:

```toml
[dependencies]
prime-radiant = "0.1.0"

# Full feature set
prime-radiant = { version = "0.1.0", features = ["full"] }
```

### Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `tiles` | No | cognitum-gate-kernel 256-tile WASM fabric |
| `sona` | No | Self-optimizing threshold tuning (SONA) |
| `learned-rho` | No | GNN-learned restriction maps |
| `hyperbolic` | No | Hierarchy-aware Poincare energy |
| `mincut` | No | Subpolynomial n^o(1) graph partitioning |
| `neural-gate` | No | Biologically-inspired gating |
| `attention` | No | Topology-gated attention, MoE, PDE diffusion |
| `distributed` | No | Raft-based multi-node coherence |
| `spectral` | No | nalgebra-based eigenvalue computation |
| `simd` | No | SIMD-optimized residual calculation |
| `gpu` | No | wgpu-based parallel computation |
| `ruvllm` | No | LLM serving integration |
| `full` | No | All features enabled |

### WASM

```bash
# Install wasm-pack
cargo install wasm-pack

# Build for web
wasm-pack build --target web

# Build for Node.js
wasm-pack build --target nodejs
```

---

## Quick Start

### Basic Coherence Computation

```rust
use prime_radiant::prelude::*;

fn main() -> Result<(), CoherenceError> {
    // Create a sheaf graph
    let mut graph = SheafGraph::new();

    // Add nodes with state vectors
    let fact1 = SheafNode::new(vec![1.0, 0.0, 0.0, 0.5]);
    let fact2 = SheafNode::new(vec![0.9, 0.1, 0.0, 0.4]);

    let id1 = graph.add_node(fact1);
    let id2 = graph.add_node(fact2);

    // Add edge with restriction map
    let rho = RestrictionMap::identity(4);
    graph.add_edge(SheafEdge::new(id1, id2, rho.clone(), rho, 1.0))?;

    // Compute coherence energy
    let energy = graph.compute_energy();
    println!("Total coherence energy: {}", energy.total);

    Ok(())
}
```

### Coherence Gate with Compute Ladder

```rust
use prime_radiant::{CoherenceGate, ComputeLane, EnergySnapshot};

fn main() {
    let policy = PolicyBundleRef::placeholder();
    let mut gate = CoherenceGate::with_defaults(policy);

    let energy = EnergySnapshot::new(0.15, 0.12, ScopeId::new("test"));
    let (decision, witness) = gate.evaluate_with_witness(&action, &energy);

    match decision.lane {
        ComputeLane::Reflex => println!("Approved (<1ms)"),
        ComputeLane::Retrieval => println!("Evidence needed (~10ms)"),
        ComputeLane::Heavy => println!("Heavy processing (~100ms)"),
        ComputeLane::Human => println!("Human review required"),
    }
}
```

### Spectral Drift Detection

```rust
use prime_radiant::coherence::{SpectralAnalyzer, SpectralConfig};

let mut analyzer = SpectralAnalyzer::new(SpectralConfig::default());

analyzer.record_eigenvalues(vec![0.0, 0.5, 1.2, 2.1]);
analyzer.record_eigenvalues(vec![0.0, 0.3, 0.9, 1.8]); // Drift!

if let Some(drift) = analyzer.detect_drift() {
    println!("Drift: {:?}, severity: {:?}", drift.description, drift.severity);
}
```

---

## API Reference

### Core Types

| Type | Description |
|------|-------------|
| `SheafGraph` | Graph with nodes, edges, and restriction maps |
| `SheafNode` | Vertex with state vector (stalk) |
| `SheafEdge` | Edge with restriction maps and weight |
| `RestrictionMap` | Linear transform for state constraints |
| `CoherenceEnergy` | Global incoherence measure |
| `CoherenceGate` | Threshold-based action gating |
| `GateDecision` | Allow/deny with compute lane |
| `WitnessRecord` | Immutable audit record |

### Compute Ladder

| Lane | Latency | Use Case |
|------|---------|----------|
| `Reflex` | <1ms | Low-energy automatic approval |
| `Retrieval` | ~10ms | Evidence fetching |
| `Heavy` | ~100ms | Multi-step planning |
| `Human` | Unbounded | Sustained incoherence review |

---

## Performance Characteristics

| Operation | Target |
|-----------|--------|
| Single residual | < 1us |
| Full energy (10K nodes) | < 10ms |
| Incremental update | < 100us |
| Gate evaluation | < 500us |
| SONA adaptation | < 0.05ms |
| MinCut update | n^o(1) subpolynomial |
| Hyperbolic distance | < 500ns |

---

## Use Cases

- **AI Safety**: Detect hallucinations via structural inconsistency
- **Finance**: Regime change detection and arbitrage validation
- **Medical**: Clinical decision consistency verification
- **Robotics**: Kinematic constraint enforcement
- **Security**: Policy rule coherence checking

---

## Architecture

```
+-----------------------------------------------------------------------------+
|                           APPLICATION LAYER                                  |
|  LLM Guards | Fraud Detection | Compliance Proofs | Robotics Safety         |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                           COHERENCE GATE                                     |
|  Lane 0 (Reflex) | Lane 1 (Retrieval) | Lane 2 (Heavy) | Lane 3 (Human)     |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                           COHERENCE COMPUTATION                              |
|  Residual Calculator | Energy Aggregator | Spectral Analyzer                |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                           KNOWLEDGE SUBSTRATE                                |
|  Sheaf Graph | Node States | Edge Constraints | Restriction Maps           |
+-----------------------------------------------------------------------------+
```

---

## Documentation

- [ADR-001: Sheaf Cohomology](docs/adr/ADR-001-sheaf-cohomology.md)
- [ADR-002: Category and Topos Theory](docs/adr/ADR-002-category-topos.md)
- [ADR-003: Homotopy Type Theory](docs/adr/ADR-003-homotopy-type-theory.md)
- [ADR-004: Spectral Invariants](docs/adr/ADR-004-spectral-invariants.md)
- [ADR-005: Causal Abstraction](docs/adr/ADR-005-causal-abstraction.md)
- [ADR-006: Quantum Topology](docs/adr/ADR-006-quantum-topology.md)
- [Domain Model](docs/ddd/domain-model.md)

---

## References

1. Hansen, J., & Ghrist, R. (2019). "Toward a spectral theory of cellular sheaves."
2. Robinson, M. (2014). "Topological Signal Processing."
3. Curry, J. (2014). "Sheaves, Cosheaves and Applications."
4. Univalent Foundations Program. "Homotopy Type Theory."

---

## License

MIT OR Apache-2.0

---

*Prime-Radiant: Where mathematics meets machine safety.*
