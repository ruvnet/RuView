# RuVector DAG Examples

Comprehensive examples demonstrating the Neural Self-Learning DAG system.

## Quick Start

```bash
# Run any example
cargo run -p ruvector-dag --example <name>

# Run with release optimizations
cargo run -p ruvector-dag --example <name> --release

# Run tests for an example
cargo test -p ruvector-dag --example <name>
```

## Core Examples

### basic_usage
Fundamental DAG operations: creating nodes, adding edges, topological sort.

```bash
cargo run -p ruvector-dag --example basic_usage
```

**Demonstrates:**
- `QueryDag::new()`, `add_node()`, `add_edge()`
- `OperatorNode` types: SeqScan, Filter, Sort, Aggregate
- Topological iteration and depth computation

### attention_demo
All 7 attention mechanisms with visual output.

```bash
cargo run -p ruvector-dag --example attention_demo
```

**Demonstrates:**
- `TopologicalAttention` - DAG layer-based scoring
- `CriticalPathAttention` - Longest path weighting
- `CausalConeAttention` - Ancestor/descendant influence
- `MinCutGatedAttention` - Bottleneck-aware attention
- `HierarchicalLorentzAttention` - Hyperbolic embeddings
- `ParallelBranchAttention` - Branch parallelism scoring
- `TemporalBTSPAttention` - Time-aware plasticity

### attention_selection
UCB bandit algorithm for dynamic mechanism selection.

```bash
cargo run -p ruvector-dag --example attention_selection
```

**Demonstrates:**
- `AttentionSelector` with UCB1 exploration/exploitation
- Automatic mechanism performance tracking
- Adaptive selection based on observed rewards

### learning_workflow
Complete SONA learning pipeline with trajectory recording.

```bash
cargo run -p ruvector-dag --example learning_workflow
```

**Demonstrates:**
- `DagSonaEngine` initialization and training
- `DagTrajectoryBuffer` for lock-free trajectory collection
- `DagReasoningBank` for pattern storage
- MicroLoRA fast adaptation
- EWC++ continual learning

### self_healing
Autonomous anomaly detection and repair system.

```bash
cargo run -p ruvector-dag --example self_healing
```

**Demonstrates:**
- `HealingOrchestrator` configuration
- `AnomalyDetector` with statistical thresholds
- `LearningDriftDetector` for performance degradation
- Custom `RepairStrategy` implementations
- Health score computation

## Exotic Examples

These examples explore unconventional applications of coherence-sensing substrates—systems that respond to internal tension rather than external commands.

### synthetic_haptic ⭐ NEW
Complete nervous system for machines: sensor → reflex → actuator with memory and learning.

```bash
cargo run -p ruvector-dag --example synthetic_haptic
```

**Architecture:**
| Layer | Component | Purpose |
|-------|-----------|---------|
| 1 | Event Sensing | Microsecond timestamps, 6-channel input |
| 2 | Reflex Arc | DAG tension + MinCut → ReflexMode |
| 3 | HDC Memory | 256-dim hypervector associative memory |
| 4 | SONA Learning | Coherence-gated adaptation |
| 5 | Actuation | Energy-budgeted force + vibro output |

**Key Concepts:**
- Intelligence as homeostasis, not goal-seeking
- Tension drives immediate response
- Coherence gates learning (only when stable)
- ReflexModes: Calm → Active → Spike → Protect

**Performance:** 192 μs avg loop @ 1000 Hz

### synthetic_reflex_organism
Intelligence as homeostasis—organisms that minimize stress without explicit goals.

```bash
cargo run -p ruvector-dag --example synthetic_reflex_organism
```

**Demonstrates:**
- `ReflexOrganism` with metabolic rate and tension tracking
- `OrganismResponse`: Rest, Contract, Expand, Partition, Rebalance
- Learning only when instability crosses thresholds
- No objectives, only stress minimization

### timing_synchronization
Machines that "feel" timing through phase alignment.

```bash
cargo run -p ruvector-dag --example timing_synchronization
```

**Demonstrates:**
- Phase-locked loops using DAG coherence
- Biological rhythm synchronization
- Timing deviation as tension signal
- Self-correcting temporal alignment

### coherence_safety
Safety as structural property—systems that shut down when coherence drops.

```bash
cargo run -p ruvector-dag --example coherence_safety
```

**Demonstrates:**
- `SafetyEnvelope` with coherence thresholds
- Automatic graceful degradation
- No external safety monitors needed
- Structural shutdown mechanisms

### artificial_instincts
Hardwired biases via MinCut boundaries and attention patterns.

```bash
cargo run -p ruvector-dag --example artificial_instincts
```

**Demonstrates:**
- Instinct encoding via graph structure
- MinCut-enforced behavioral boundaries
- Attention-weighted decision biases
- Healing as instinct restoration

### living_simulation
Simulations that model fragility, not just outcomes.

```bash
cargo run -p ruvector-dag --example living_simulation
```

**Demonstrates:**
- Coherence as simulation health metric
- Fragility-aware state evolution
- Self-healing simulation repair
- Tension-driven adaptation

### thought_integrity
Reasoning monitored like electrical voltage—coherence as correctness signal.

```bash
cargo run -p ruvector-dag --example thought_integrity
```

**Demonstrates:**
- Reasoning chain as DAG structure
- Coherence drops indicate logical errors
- Self-correcting inference
- Integrity verification without external validation

### federated_coherence
Distributed consensus through coherence, not voting.

```bash
cargo run -p ruvector-dag --example federated_coherence
```

**Demonstrates:**
- `FederatedNode` with peer coherence tracking
- 7 message types for distributed coordination
- Pattern propagation via coherence alignment
- Consensus emerges from structural agreement

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    QueryDag                             │
│  ┌─────┐   ┌─────┐   ┌─────┐   ┌─────┐   ┌─────┐      │
│  │Scan │──▶│Filter│──▶│Agg  │──▶│Sort │──▶│Result│     │
│  └─────┘   └─────┘   └─────┘   └─────┘   └─────┘      │
└─────────────────────────────────────────────────────────┘
        │                   │                   │
        ▼                   ▼                   ▼
┌───────────────┐  ┌───────────────┐  ┌───────────────┐
│   Attention   │  │    MinCut     │  │     SONA      │
│  Mechanisms   │  │    Engine     │  │   Learning    │
│  (7 types)    │  │  (tension)    │  │  (coherence)  │
└───────────────┘  └───────────────┘  └───────────────┘
        │                   │                   │
        └───────────────────┴───────────────────┘
                            │
                            ▼
                   ┌───────────────┐
                   │   Healing     │
                   │ Orchestrator  │
                   └───────────────┘
```

## Key Concepts

### Tension
How far the current state is from homeostasis. Computed from:
- MinCut flow capacity stress
- Node criticality deviation
- Sensor/input anomalies

**Usage:** Drives immediate reflex-level responses.

### Coherence
How consistent the internal state is over time. Drops when:
- Tension changes rapidly
- Partitioning becomes unstable
- Learning causes drift

**Usage:** Gates learning and safety decisions.

### Reflex Modes
| Mode | Tension | Behavior |
|------|---------|----------|
| Calm | < 0.20 | Minimal response, learning allowed |
| Active | 0.20-0.55 | Proportional response |
| Spike | 0.55-0.85 | Heightened response, haptic feedback |
| Protect | > 0.85 | Protective shutdown, no output |

## Running All Examples

```bash
# Quick verification
for ex in basic_usage attention_demo attention_selection \
          learning_workflow self_healing synthetic_haptic; do
    echo "=== $ex ===" && cargo run -p ruvector-dag --example $ex 2>/dev/null | head -20
done

# Exotic examples
for ex in synthetic_reflex_organism timing_synchronization coherence_safety \
          artificial_instincts living_simulation thought_integrity federated_coherence; do
    echo "=== $ex ===" && cargo run -p ruvector-dag --example $ex 2>/dev/null | head -20
done
```

## Testing

```bash
# Run all example tests
cargo test -p ruvector-dag --examples

# Test specific example
cargo test -p ruvector-dag --example synthetic_haptic
```

## Performance Notes

- **Attention**: O(V+E) for topological, O(V²) for causal cone
- **MinCut**: O(n^0.12) amortized with caching
- **SONA Learning**: Background thread, non-blocking
- **Haptic Loop**: Target <1ms, achieved ~200μs average

## License

MIT - See repository root for details.
