# Delta-Behavior

**The mathematics of systems that refuse to collapse.**

[![Crates.io](https://img.shields.io/crates/v/delta-behavior.svg)](https://crates.io/crates/delta-behavior)
[![Documentation](https://docs.rs/delta-behavior/badge.svg)](https://docs.rs/delta-behavior)
[![License](https://img.shields.io/crates/l/delta-behavior.svg)](LICENSE-MIT)

Delta-behavior is a design principle for building systems where **change is permitted but collapse is not**. It provides a framework for constraining state transitions to preserve global coherence.

## Key Features

- **Coherence-First Design**: Optimize for stability, not just performance
- **Three-Layer Enforcement**: Energy cost, scheduling, and memory gating
- **Attractor Dynamics**: Systems naturally gravitate toward stable states
- **11 Exotic Applications**: From AI safety to extropic intelligence substrates
- **WASM + TypeScript SDK**: Full browser/Node.js support
- **Performance Optimized**: O(n) algorithms with SIMD acceleration

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
delta-behavior = "0.1"
```

Basic usage:

```rust
use delta_behavior::{DeltaSystem, Coherence, DeltaConfig};
use delta_behavior::enforcement::{DeltaEnforcer, EnforcementResult};

// Create an enforcer with default configuration
let config = DeltaConfig::default();
let mut enforcer = DeltaEnforcer::new(config);

// Check if a transition should be allowed
let current = Coherence::clamped(0.8);
let predicted = Coherence::clamped(0.75);

match enforcer.check(current, predicted) {
    EnforcementResult::Allowed => println!("Transition allowed"),
    EnforcementResult::Throttled(delay) => println!("Wait {:?}", delay),
    EnforcementResult::Blocked(reason) => println!("Blocked: {}", reason),
}
```

## The Four Properties

A system exhibits Delta-behavior when:

1. **Local Change**: State updates happen in bounded steps
2. **Global Preservation**: Local changes don't break overall structure
3. **Violation Resistance**: Destabilizing transitions are damped/blocked
4. **Closure Preference**: System naturally settles into stable attractors

## Configuration

Three preset configurations are available:

```rust
// Default: Balanced stability and flexibility
let config = DeltaConfig::default();

// Strict: For safety-critical applications
let config = DeltaConfig::strict();

// Relaxed: For exploratory applications
let config = DeltaConfig::relaxed();
```

Custom configuration:

```rust
use delta_behavior::{DeltaConfig, CoherenceBounds, Coherence};

let config = DeltaConfig {
    bounds: CoherenceBounds {
        min_coherence: Coherence::clamped(0.4),
        throttle_threshold: Coherence::clamped(0.6),
        target_coherence: Coherence::clamped(0.85),
        max_delta_drop: 0.08,
    },
    guidance_strength: 0.7,
    ..DeltaConfig::default()
};
```

## 11 Exotic Applications

| # | Application | Description | Key Innovation |
|---|-------------|-------------|----------------|
| 01 | **Self-Limiting Reasoning** | AI that does less when uncertain | Depth/scope scales with coherence |
| 02 | **Computational Event Horizon** | Bounded computation without hard limits | Asymptotic approach, never arrival |
| 03 | **Artificial Homeostasis** | Synthetic life with coherence-based survival | Death = coherence collapse |
| 04 | **Self-Stabilizing World Model** | Models that refuse to hallucinate | Observations that would destabilize are dampened |
| 05 | **Coherence-Bounded Creativity** | Novelty without chaos | Perturbations rejected if incoherent |
| 06 | **Anti-Cascade Financial System** | Markets that cannot collapse | Leverage tied to systemic coherence |
| 07 | **Graceful Aging** | Systems that simplify over time | Capability reduction as coherence maintenance cost |
| 08 | **Swarm Intelligence** | Collective behavior without pathology | Actions modified to preserve swarm coherence |
| 09 | **Graceful Shutdown** | Systems that seek safe termination | Cleanup as coherence-preserving operation |
| 10 | **Pre-AGI Containment** | Bounded intelligence growth | Intelligence ↔ coherence bidirectional constraint |
| 11 | **Extropic Substrate** | Complete intelligence substrate | Goal mutation, agent lifecycles, spike semantics |

### Application 11: Extropic Intelligence Substrate

The crown jewel - implements three missing pieces for explicit extropic intelligence:

```rust
use delta_behavior::applications::extropic::{
    MutableGoal, MemoryAgent, SpikeBus, ExtropicSubstrate
};

// 1. Goals that mutate autonomously under coherence constraints
let mut goal = MutableGoal::new(vec![1.0, 0.0, 0.0]);
goal.attempt_mutation(vec![0.1, 0.05, 0.0], 0.95); // Coherence-gated

// 2. Agents with native lifecycles in memory
let mut substrate = ExtropicSubstrate::new(SubstrateConfig::default());
let agent_id = substrate.spawn_agent(vec![0.0, 0.0], AgentGenome::default());
// Agent progresses: Embryonic → Growing → Mature → Senescent → Dying → Dead

// 3. Hardware-enforced spike/silence semantics
substrate.tick(); // Processes spike bus, enforces refractory periods
```

## Three-Layer Enforcement

Delta-behavior uses defense-in-depth:

```
  Transition
      |
      v
+-------------+     Soft constraint:
| Energy Cost |---> Unstable = expensive
+-------------+
      |
      v
+-------------+     Medium constraint:
|  Scheduling |---> Unstable = delayed
+-------------+
      |
      v
+-------------+     Hard constraint:
| Memory Gate |---> Incoherent = blocked
+-------------+
      |
      v
   Applied
```

## Performance Optimizations

The implementation includes several critical optimizations:

| Component | Optimization | Improvement |
|-----------|-------------|-------------|
| Swarm neighbors | SpatialGrid partitioning | O(n²) → O(n·k) |
| Coherence calculation | Incremental cache | O(n) → O(1) |
| Financial history | VecDeque | O(n) → O(1) removal |
| Distance calculations | Squared comparisons | Avoids sqrt() |
| Batch operations | SIMD with 8x unrolling | ~4x throughput |

### SIMD Utilities

```rust
use delta_behavior::simd_utils::{
    batch_squared_distances,
    batch_in_range,
    vector_coherence,
    normalize_vectors,
};

// Process vectors in batches with SIMD acceleration
let distances = batch_squared_distances(&positions, &center);
let neighbors = batch_in_range(&positions, &center, radius);
```

## WASM Support

Build for WebAssembly:

```bash
wasm-pack build --target web
```

### TypeScript SDK

```typescript
import init, {
  WasmCoherence,
  WasmSelfLimitingReasoner,
  WasmCoherentSwarm,
  WasmContainmentSubstrate,
} from 'delta-behavior';

await init();

// Self-limiting reasoning
const reasoner = new WasmSelfLimitingReasoner(10, 5);
console.log(`Allowed depth: ${reasoner.allowed_depth()}`);

// Coherent swarm
const swarm = new WasmCoherentSwarm();
swarm.add_agent("agent-1", "[0, 0]", "[1, 0]", "[10, 10]");
const result = swarm.propose_action("agent-1", "move", "[0.5, 0.5]");

// Pre-AGI containment
const substrate = new WasmContainmentSubstrate(1.0, 10.0);
const growth = substrate.attempt_growth("Reasoning", 0.5);
```

## Architecture

```
delta-behavior/
├── src/
│   ├── lib.rs          # Core traits, Coherence, DeltaConfig
│   ├── wasm.rs         # WASM bindings for all 11 applications
│   └── simd_utils.rs   # SIMD-accelerated batch operations
├── applications/       # 11 exotic application implementations
│   ├── 01-self-limiting-reasoning.rs
│   ├── 02-computational-event-horizon.rs
│   ├── ...
│   └── 11-extropic-substrate.rs
├── adr/               # Architecture Decision Records
│   ├── ADR-000-DELTA-BEHAVIOR-DEFINITION.md
│   ├── ADR-001-COHERENCE-BOUNDS.md
│   ├── ADR-002-ENERGY-COST-LAYER.md
│   └── ...
├── wasm/              # TypeScript SDK
│   └── src/index.ts
└── pkg/               # Built WASM package
```

## Test Coverage

```
✅ 32 lib tests
✅ 14 WASM binding tests
✅ 13 doc tests
───────────────────────
   59 tests passing
```

## Documentation

- [API Reference](https://docs.rs/delta-behavior)
- [Whitepaper](./WHITEPAPER.md) - Full theoretical foundations
- [API Guide](./docs/API.md) - Comprehensive API documentation
- [ADR Series](./adr/) - Architecture Decision Records

### ADR Overview

| ADR | Title |
|-----|-------|
| 000 | Delta-Behavior Definition |
| 001 | Coherence Bounds |
| 002 | Energy Cost Layer |
| 003 | Scheduling Layer |
| 004 | Memory Gating Layer |
| 005 | Attractor Dynamics |
| 006 | Application Framework |
| 007 | WASM Architecture |
| 008 | TypeScript SDK |
| 009 | Performance Targets |
| 010 | Security Model |

## Minimum Supported Rust Version

Rust 1.75.0 or later.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions are welcome! Please read the contributing guidelines before submitting PRs.

## Citation

If you use Delta-behavior in academic work, please cite:

```bibtex
@software{delta_behavior,
  title = {Delta-Behavior: Constrained State Transitions for Coherent Systems},
  author = {RuVector Team},
  year = {2026},
  url = {https://github.com/ruvnet/ruvector}
}
```
