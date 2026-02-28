# Exotic Examples: Coherence-Sensing Substrates

These examples explore systems that respond to internal tension rather than external commands—where intelligence emerges as homeostasis.

## Philosophy

Traditional AI systems are goal-directed: they receive objectives and optimize toward them. These examples flip that model:

> **Intelligence as maintaining coherence under perturbation.**

A system doesn't need goals if it can feel when it's "out of tune" and naturally moves toward equilibrium.

## The Examples

### 1. synthetic_reflex_organism.rs
**Intelligence as Homeostasis**

No goals, only stress minimization. The organism responds to tension by adjusting its internal state, learning only when instability crosses thresholds.

```rust
pub enum OrganismResponse {
    Rest,       // Low tension: do nothing
    Contract,   // Rising tension: consolidate
    Expand,     // Stable low tension: explore
    Partition,  // High tension: segment
    Rebalance,  // Oscillating: redistribute
}
```

### 2. timing_synchronization.rs
**Machines That Feel Timing**

Phase-locked loops using DAG coherence. The system "feels" when its internal rhythms drift from external signals and self-corrects.

```rust
// Timing is not measured, it's felt
let phase_error = self.measure_phase_deviation();
let tension = self.dag.compute_tension_from_timing(phase_error);
self.adjust_internal_clock(tension);
```

### 3. coherence_safety.rs
**Structural Safety**

Safety isn't a monitor checking outputs—it's a structural property. When coherence drops below threshold, the system naturally enters a safe state.

```rust
// No safety rules, just coherence
if coherence < 0.3 {
    // System structurally cannot produce dangerous output
    // because the pathways become disconnected
}
```

### 4. artificial_instincts.rs
**Hardwired Biases**

Instincts encoded via MinCut boundaries and attention patterns. These aren't learned—they're structural constraints that shape behavior.

```rust
// Fear isn't learned, it's architectural
let fear_boundary = mincut.compute(threat_region, action_region);
if fear_boundary.cut_value < threshold {
    // Action pathway is structurally blocked
}
```

### 5. living_simulation.rs
**Fragility-Aware Modeling**

Simulations that model not just outcomes, but structural health. The simulation knows when it's "sick" and can heal itself.

```rust
// Simulation health = structural coherence
let health = simulation.dag.coherence();
if health < 0.5 {
    simulation.trigger_healing();
}
```

### 6. thought_integrity.rs
**Reasoning Monitored Like Voltage**

Logical inference as a DAG where coherence indicates correctness. Errors show up as tension in the reasoning graph.

```rust
// Contradiction creates structural tension
let reasoning = build_inference_dag(premises, conclusion);
let integrity = reasoning.coherence();
// Low integrity = likely logical error
```

### 7. federated_coherence.rs
**Consensus Through Coherence**

Distributed systems that agree not by voting, but by structural alignment. Nodes synchronize patterns when their coherence matrices align.

```rust
pub enum FederationMessage {
    Heartbeat { coherence: f32 },
    ProposePattern { pattern: DagPattern },
    ValidatePattern { id: String, local_coherence: f32 },
    RejectPattern { id: String, tension_source: String },
    TensionAlert { severity: f32, region: Vec<usize> },
    SyncRequest { since_round: u64 },
    SyncResponse { patterns: Vec<DagPattern> },
}
```

## Core Insight

These systems demonstrate that:

1. **Intelligence doesn't require goals** — maintaining structure is sufficient
2. **Safety can be architectural** — not a bolt-on monitor
3. **Learning should be gated** — only update when stable
4. **Consensus can emerge** — from structural agreement, not voting

## Running

```bash
# Run all exotic examples
for ex in synthetic_reflex_organism timing_synchronization \
          coherence_safety artificial_instincts living_simulation \
          thought_integrity federated_coherence; do
    cargo run -p ruvector-dag --example $ex
done
```

## Key Metrics

| Metric | Meaning | Healthy Range |
|--------|---------|---------------|
| Tension | Deviation from equilibrium | < 0.3 |
| Coherence | Structural consistency | > 0.8 |
| Cut Value | Flow capacity stress | < 100 |
| Criticality | Node importance | 0.0-1.0 |

## Further Reading

These concepts draw from:
- Homeostatic regulation in biological systems
- Free energy principle (Friston)
- Autopoiesis (Maturana & Varela)
- Active inference
- Predictive processing

The key shift: from "what should I do?" to "how do I stay coherent?"
