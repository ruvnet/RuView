# Nervous System Examples

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../LICENSE)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()

Bio-inspired nervous system architecture examples demonstrating the transition from **"How do we make machines smarter?"** to **"What kind of organism are we building?"**

## Overview

These examples show how nervous system thinking unlocks new products, markets, and research categories. The architecture enables systems that **age well** instead of breaking under complexity.

All tier examples are organized in the unified `tiers/` folder with prefixed names for easy navigation.

## Application Tiers

### Tier 1: Immediate Practical Applications
*Shippable with current architecture*

| Example | Domain | Key Benefit |
|---------|--------|-------------|
| [t1_anomaly_detection](tiers/t1_anomaly_detection.rs) | Infrastructure, Finance, Security | Detection before failure, microsecond response |
| [t1_edge_autonomy](tiers/t1_edge_autonomy.rs) | Drones, Vehicles, Robotics | Lower power, certified reflex paths |
| [t1_medical_wearable](tiers/t1_medical_wearable.rs) | Monitoring, Assistive Devices | Adapts to the person, always-on, private |

### Tier 2: Near-Term Transformative Applications
*Possible once local learning and coherence routing mature*

| Example | Domain | Key Benefit |
|---------|--------|-------------|
| [t2_self_optimizing](tiers/t2_self_optimizing.rs) | Agents Monitoring Agents | Self-stabilizing software, structural witnesses |
| [t2_swarm_intelligence](tiers/t2_swarm_intelligence.rs) | IoT Fleets, Sensor Meshes | Scale without fragility, emergent intelligence |
| [t2_adaptive_simulation](tiers/t2_adaptive_simulation.rs) | Digital Twins, Logistics | Always-warm simulation, costs scale with relevance |

### Tier 3: Exotic But Real Applications
*Technically grounded, novel research directions*

| Example | Domain | Key Benefit |
|---------|--------|-------------|
| [t3_self_awareness](tiers/t3_self_awareness.rs) | Structural Self-Sensing | Systems say "I am becoming unstable" |
| [t3_synthetic_nervous](tiers/t3_synthetic_nervous.rs) | Buildings, Factories, Cities | Environments respond like organisms |
| [t3_bio_machine](tiers/t3_bio_machine.rs) | Prosthetics, Rehabilitation | Machines stop fighting biology |

### Tier 4: SOTA & Exotic Research Applications
*Cutting-edge research directions pushing neuromorphic boundaries*

| Example | Domain | Key Benefit |
|---------|--------|-------------|
| [t4_neuromorphic_rag](tiers/t4_neuromorphic_rag.rs) | LLM Memory, Retrieval | Coherence-gated retrieval, 100x compute reduction |
| [t4_agentic_self_model](tiers/t4_agentic_self_model.rs) | Agentic AI, Self-Awareness | Agent models own cognition, knows when capable |
| [t4_collective_dreaming](tiers/t4_collective_dreaming.rs) | Swarm Consolidation | Hippocampal replay, cross-agent memory transfer |
| [t4_compositional_hdc](tiers/t4_compositional_hdc.rs) | Zero-Shot Reasoning | HDC binding for analogy and composition |

## Quick Start

```bash
# Run a Tier 1 example
cargo run --example t1_anomaly_detection

# Run a Tier 2 example
cargo run --example t2_swarm_intelligence

# Run a Tier 3 example
cargo run --example t3_self_awareness

# Run a Tier 4 example
cargo run --example t4_neuromorphic_rag
```

## Architecture Principles

Each example demonstrates the same five-layer architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                    COHERENCE LAYER                          │
│  Global Workspace • Oscillatory Routing • Predictive Coding │
└─────────────────────────────────────────────────────────────┘
                              ↑
┌─────────────────────────────────────────────────────────────┐
│                     LEARNING LAYER                          │
│     BTSP One-Shot • E-prop Online • EWC Consolidation      │
└─────────────────────────────────────────────────────────────┘
                              ↑
┌─────────────────────────────────────────────────────────────┐
│                      MEMORY LAYER                           │
│     Hopfield Networks • HDC Vectors • Pattern Separation   │
└─────────────────────────────────────────────────────────────┘
                              ↑
┌─────────────────────────────────────────────────────────────┐
│                      REFLEX LAYER                           │
│      K-WTA Competition • Dendritic Coincidence • Safety    │
└─────────────────────────────────────────────────────────────┘
                              ↑
┌─────────────────────────────────────────────────────────────┐
│                      SENSING LAYER                          │
│      Event Bus • Sparse Spikes • Backpressure Control      │
└─────────────────────────────────────────────────────────────┘
```

## Key Concepts Demonstrated

### Reflex Arcs
Fast, deterministic responses with bounded execution:
- Latency: <100μs
- Certifiable: Maximum iteration counts
- Safety: Witness logging for every decision

### Homeostasis
Self-regulation instead of static thresholds:
- Adaptive learning from normal operation
- Graceful degradation under stress
- Anticipatory maintenance

### Coherence Gating
Synchronize only when needed:
- Kuramoto oscillators for phase coupling
- Communication gain based on phase coherence
- 90-99% bandwidth reduction via prediction

### One-Shot Learning
Learn immediately from single examples:
- BTSP: Seconds-scale eligibility traces
- No batch retraining required
- Personalization through use

## Tutorial: Building a Custom Application

### Step 1: Define Your Sensing Layer

```rust
use ruvector_nervous_system::eventbus::{DVSEvent, EventRingBuffer};

// Create event buffer with backpressure
let buffer = EventRingBuffer::new(1024);

// Process events sparsely
if let Some(event) = buffer.pop() {
    // Only significant changes generate events
}
```

### Step 2: Add Reflex Gates

```rust
use ruvector_nervous_system::compete::WTALayer;

// Winner-take-all for fast decisions
let mut wta = WTALayer::new(100, 0.5, 0.8);

// <1μs for 1000 neurons
if let Some(winner) = wta.compete(&inputs) {
    trigger_immediate_response(winner);
}
```

### Step 3: Implement Memory

```rust
use ruvector_nervous_system::hopfield::ModernHopfield;
use ruvector_nervous_system::hdc::Hypervector;

// Hopfield for associative retrieval
let mut hopfield = ModernHopfield::new(512, 10.0);
hopfield.store(pattern);

// HDC for ultra-fast similarity
let similarity = v1.similarity(&v2); // <100ns
```

### Step 4: Enable Learning

```rust
use ruvector_nervous_system::plasticity::btsp::BTSPSynapse;

// One-shot learning
let mut synapse = BTSPSynapse::new(0.5, 2000.0); // 2s time constant
synapse.update(presynaptic_active, plateau_signal, dt);
```

### Step 5: Add Coherence

```rust
use ruvector_nervous_system::routing::{OscillatoryRouter, GlobalWorkspace};

// Phase-coupled routing
let mut router = OscillatoryRouter::new(10, 40.0); // 40Hz gamma
let gain = router.communication_gain(sender, receiver);

// Global workspace (4-7 items)
let mut workspace = GlobalWorkspace::new(7);
workspace.broadcast(representation);
```

## Performance Targets

| Component | Latency | Throughput |
|-----------|---------|------------|
| Event Bus | <100ns push/pop | 10,000+ events/ms |
| WTA | <1μs | 1M+ decisions/sec |
| HDC Similarity | <100ns | 10M+ comparisons/sec |
| Hopfield Retrieval | <1ms | 1000+ queries/sec |
| BTSP Update | <100ns | 10M+ synapses/sec |

## From Practical to SOTA

The same architecture scales from:

1. **Practical**: Anomaly detection with microsecond response
2. **Transformative**: Self-optimizing software systems
3. **Exotic**: Machines that sense their own coherence
4. **SOTA**: Neuromorphic RAG, self-modeling agents, collective dreaming

The difference is how much reflex, learning, and coherence you turn on.

## Further Reading

- [Architecture Documentation](../../docs/nervous-system/architecture.md)
- [Deployment Guide](../../docs/nervous-system/deployment.md)
- [Test Plan](../../docs/nervous-system/test-plan.md)
- [Main Crate Documentation](../README.md)

## Contributing

Examples welcome! Each should demonstrate:
1. A clear use case
2. The nervous system architecture
3. Performance characteristics
4. Tests and documentation

## License

MIT License - See [LICENSE](../LICENSE)
