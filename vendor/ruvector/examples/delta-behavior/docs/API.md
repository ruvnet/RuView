# Delta-Behavior API Reference

Comprehensive API documentation for the Delta-behavior library.

## Table of Contents

- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
  - [Coherence](#coherence)
  - [Attractors](#attractors)
  - [Transitions](#transitions)
- [API Reference](#api-reference)
  - [Core Types](#core-types)
  - [Configuration](#configuration)
  - [Enforcement](#enforcement)
- [Applications](#applications)
- [Integration Examples](#integration-examples)

---

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
delta-behavior = "0.1"

# Or with specific applications
delta-behavior = { version = "0.1", features = ["containment", "swarm-intelligence"] }
```

### Minimal Example

```rust
use delta_behavior::{DeltaSystem, Coherence, DeltaConfig};

// Implement DeltaSystem for your type
struct MySystem {
    state: Vec<f64>,
    coherence: Coherence,
}

impl DeltaSystem for MySystem {
    type State = Vec<f64>;
    type Transition = Vec<f64>;
    type Error = String;

    fn coherence(&self) -> Coherence {
        self.coherence
    }

    fn step(&mut self, delta: &Self::Transition) -> Result<(), Self::Error> {
        // Validate coherence before applying
        let predicted = self.predict_coherence(delta);
        if predicted.value() < 0.3 {
            return Err("Would violate coherence bound".into());
        }

        // Apply the transition
        for (s, d) in self.state.iter_mut().zip(delta) {
            *s += d;
        }
        self.coherence = predicted;
        Ok(())
    }

    fn predict_coherence(&self, delta: &Self::Transition) -> Coherence {
        let magnitude: f64 = delta.iter().map(|x| x.abs()).sum();
        let impact = magnitude * 0.01;
        Coherence::clamped(self.coherence.value() - impact)
    }

    fn state(&self) -> &Self::State {
        &self.state
    }

    fn in_attractor(&self) -> bool {
        // Check if state is near a stable configuration
        self.state.iter().all(|x| x.abs() < 1.0)
    }
}
```

### With Enforcement

```rust
use delta_behavior::{DeltaConfig, enforcement::DeltaEnforcer};

fn main() {
    let config = DeltaConfig::default();
    let mut enforcer = DeltaEnforcer::new(config);

    let current = Coherence::new(0.8).unwrap();
    let predicted = Coherence::new(0.75).unwrap();

    match enforcer.check(current, predicted) {
        EnforcementResult::Allowed => {
            // Apply the transition
        }
        EnforcementResult::Throttled(duration) => {
            // Wait before retrying
            std::thread::sleep(duration);
        }
        EnforcementResult::Blocked(reason) => {
            // Transition rejected
            eprintln!("Blocked: {}", reason);
        }
    }
}
```

---

## Core Concepts

### Coherence

Coherence is the central metric in Delta-behavior systems. It measures how "organized" or "stable" a system currently is.

#### Properties

| Value | Meaning |
|-------|---------|
| 1.0 | Maximum coherence - perfectly organized |
| 0.8+ | High coherence - system is stable |
| 0.5-0.8 | Moderate coherence - may be throttled |
| 0.3-0.5 | Low coherence - transitions restricted |
| <0.3 | Critical - writes blocked |
| 0.0 | Collapsed - system failure |

#### Computing Coherence

Coherence computation depends on your domain:

##### For Vector Spaces (HNSW neighborhoods)

```rust
pub fn vector_coherence(center: &[f64], neighbors: &[&[f64]]) -> f64 {
    let distances: Vec<f64> = neighbors
        .iter()
        .map(|n| cosine_distance(center, n))
        .collect();

    let mean = distances.iter().sum::<f64>() / distances.len() as f64;
    let variance = distances.iter()
        .map(|d| (d - mean).powi(2))
        .sum::<f64>() / distances.len() as f64;

    // Low variance = high coherence (tight neighborhood)
    1.0 / (1.0 + variance)
}
```

##### For Graphs

```rust
pub fn graph_coherence(graph: &Graph) -> f64 {
    let clustering = compute_clustering_coefficient(graph);
    let modularity = compute_modularity(graph);
    let connectivity = compute_algebraic_connectivity(graph);

    0.4 * clustering + 0.3 * modularity + 0.3 * connectivity.min(1.0)
}
```

##### For Agent State

```rust
pub fn agent_coherence(state: &AgentState) -> f64 {
    let attention_entropy = compute_attention_entropy(&state.attention);
    let memory_consistency = compute_memory_consistency(&state.memory);
    let goal_alignment = compute_goal_alignment(&state.goals, &state.actions);

    // Low entropy + high consistency + high alignment = coherent
    ((1.0 - attention_entropy) * memory_consistency * goal_alignment).clamp(0.0, 1.0)
}
```

### Attractors

Attractors are stable states the system naturally evolves toward.

#### Types of Attractors

| Type | Description | Example |
|------|-------------|---------|
| Fixed Point | Single stable state | Thermostat at target temperature |
| Limit Cycle | Repeating sequence | Day/night cycle |
| Strange Attractor | Bounded chaos | Weather patterns |

#### Using Attractors

```rust
use delta_behavior::attractor::{Attractor, GuidanceForce};

// Define an attractor
let stable_state = Attractor {
    state: vec![0.0, 0.0, 0.0],  // Origin is stable
    strength: 0.8,
    radius: 1.0,
};

// Compute guidance force
let current_position = vec![0.5, 0.3, 0.2];
let force = GuidanceForce::toward(
    &current_position,
    &stable_state.state,
    stable_state.strength,
);

// Apply to transition
let biased_delta: Vec<f64> = original_delta
    .iter()
    .zip(&force.direction)
    .map(|(d, f)| d + f * force.magnitude * 0.1)
    .collect();
```

### Transitions

Transitions are state changes that must preserve coherence.

#### The Delta-Behavior Invariant

Every transition must satisfy:

```
coherence(S') >= coherence(S) - epsilon_max
coherence(S') >= coherence_min
```

#### Transition Results

| Result | Meaning | Action |
|--------|---------|--------|
| `Applied` | Transition succeeded | Continue |
| `Blocked` | Transition rejected | Find alternative |
| `Throttled` | Transition delayed | Wait and retry |
| `Modified` | Transition adjusted | Use modified version |

---

## API Reference

### Core Types

#### `Coherence`

```rust
pub struct Coherence(f64);

impl Coherence {
    /// Create new coherence value (must be 0.0-1.0)
    pub fn new(value: f64) -> Result<Self, &'static str>;

    /// Create coherence, clamping to valid range
    pub fn clamped(value: f64) -> Self;

    /// Maximum coherence (1.0)
    pub fn maximum() -> Self;

    /// Minimum coherence (0.0)
    pub fn minimum() -> Self;

    /// Get the underlying value
    pub fn value(&self) -> f64;

    /// Check if above threshold
    pub fn is_above(&self, threshold: f64) -> bool;

    /// Check if below threshold
    pub fn is_below(&self, threshold: f64) -> bool;

    /// Calculate drop from another value
    pub fn drop_from(&self, other: &Coherence) -> f64;
}
```

#### `CoherenceBounds`

```rust
pub struct CoherenceBounds {
    /// Minimum acceptable coherence (writes blocked below this)
    pub min_coherence: Coherence,

    /// Throttle threshold (rate limited below this)
    pub throttle_threshold: Coherence,

    /// Target coherence for recovery
    pub target_coherence: Coherence,

    /// Maximum drop allowed per transition
    pub max_delta_drop: f64,
}

impl Default for CoherenceBounds {
    fn default() -> Self {
        Self {
            min_coherence: Coherence(0.3),
            throttle_threshold: Coherence(0.5),
            target_coherence: Coherence(0.8),
            max_delta_drop: 0.1,
        }
    }
}
```

#### `DeltaSystem` Trait

```rust
pub trait DeltaSystem {
    /// The state type
    type State: Clone;

    /// The transition type
    type Transition;

    /// Error type for failed transitions
    type Error;

    /// Measure current coherence
    fn coherence(&self) -> Coherence;

    /// Apply a transition
    fn step(&mut self, transition: &Self::Transition) -> Result<(), Self::Error>;

    /// Predict coherence after transition (without applying)
    fn predict_coherence(&self, transition: &Self::Transition) -> Coherence;

    /// Get current state
    fn state(&self) -> &Self::State;

    /// Check if in an attractor basin
    fn in_attractor(&self) -> bool;
}
```

### Configuration

#### `DeltaConfig`

```rust
pub struct DeltaConfig {
    pub bounds: CoherenceBounds,
    pub energy: EnergyConfig,
    pub scheduling: SchedulingConfig,
    pub gating: GatingConfig,
    pub guidance_strength: f64,  // 0.0-1.0
}

impl DeltaConfig {
    /// Default configuration
    pub fn default() -> Self;

    /// Strict configuration for safety-critical systems
    pub fn strict() -> Self;

    /// Relaxed configuration for exploratory systems
    pub fn relaxed() -> Self;
}
```

#### `EnergyConfig`

Controls the soft enforcement layer where unstable transitions become expensive.

```rust
pub struct EnergyConfig {
    /// Base cost for any transition
    pub base_cost: f64,           // default: 1.0

    /// Exponent for instability scaling
    pub instability_exponent: f64, // default: 2.0

    /// Maximum cost cap
    pub max_cost: f64,             // default: 100.0

    /// Budget regeneration per tick
    pub budget_per_tick: f64,      // default: 10.0
}
```

Energy cost formula:
```
cost = base_cost * (1 + instability)^instability_exponent
```

#### `SchedulingConfig`

Controls the medium enforcement layer for prioritization.

```rust
pub struct SchedulingConfig {
    /// Coherence thresholds for 5 priority levels
    pub priority_thresholds: [f64; 5],  // default: [0.0, 0.3, 0.5, 0.7, 0.9]

    /// Rate limits per priority level
    pub rate_limits: [usize; 5],        // default: [100, 50, 20, 10, 5]
}
```

#### `GatingConfig`

Controls the hard enforcement layer that blocks writes.

```rust
pub struct GatingConfig {
    /// Minimum coherence to allow any writes
    pub min_write_coherence: f64,       // default: 0.3

    /// Minimum coherence after write
    pub min_post_write_coherence: f64,  // default: 0.25

    /// Recovery margin before writes resume
    pub recovery_margin: f64,           // default: 0.2
}
```

### Enforcement

#### `DeltaEnforcer`

```rust
pub struct DeltaEnforcer {
    config: DeltaConfig,
    energy_budget: f64,
    in_recovery: bool,
}

impl DeltaEnforcer {
    /// Create new enforcer
    pub fn new(config: DeltaConfig) -> Self;

    /// Check if transition should be allowed
    pub fn check(
        &mut self,
        current: Coherence,
        predicted: Coherence,
    ) -> EnforcementResult;

    /// Regenerate energy budget (call once per tick)
    pub fn tick(&mut self);
}
```

#### `EnforcementResult`

```rust
pub enum EnforcementResult {
    /// Transition allowed
    Allowed,

    /// Transition blocked with reason
    Blocked(String),

    /// Transition throttled (delayed)
    Throttled(Duration),
}

impl EnforcementResult {
    /// Check if allowed
    pub fn is_allowed(&self) -> bool;
}
```

---

## Applications

Enable via feature flags:

| Feature | Application | Description |
|---------|-------------|-------------|
| `self-limiting` | Self-Limiting Reasoning | AI that does less when uncertain |
| `event-horizon` | Computational Event Horizons | Bounded recursion without hard limits |
| `homeostasis` | Artificial Homeostasis | Synthetic life with coherence-based survival |
| `world-model` | Self-Stabilizing World Models | Models that refuse to hallucinate |
| `creativity` | Coherence-Bounded Creativity | Novelty without chaos |
| `financial` | Anti-Cascade Financial | Markets that cannot collapse |
| `aging` | Graceful Aging | Systems that simplify over time |
| `swarm` | Swarm Intelligence | Collective behavior without pathology |
| `shutdown` | Graceful Shutdown | Systems that seek safe termination |
| `containment` | Pre-AGI Containment | Bounded intelligence growth |
| `all-applications` | All of the above | Full feature set |

### Application 1: Self-Limiting Reasoning

AI systems that automatically reduce activity when uncertain.

```rust
use delta_behavior::applications::self_limiting::{SelfLimitingReasoner, ReasoningStep};

let mut reasoner = SelfLimitingReasoner::new(0.6); // Min coherence

// Reasoning naturally slows as confidence drops
let result = reasoner.reason(query, context);

match result {
    ReasoningResult::Complete(answer) => println!("Answer: {}", answer),
    ReasoningResult::Halted { reason, partial } => {
        println!("Stopped: {} (partial: {:?})", reason, partial);
    }
    ReasoningResult::Shallow { depth_reached } => {
        println!("Limited to depth {}", depth_reached);
    }
}
```

### Application 5: Coherence-Bounded Creativity

Generate novel outputs while maintaining coherence.

```rust
use delta_behavior::applications::creativity::{CreativeEngine, NoveltyMetrics};

let mut engine = CreativeEngine::new(0.5, 0.8); // coherence, novelty bounds

// Generate creative output that stays coherent
let output = engine.generate(seed, context);

println!("Novelty: {:.2}", output.novelty_score);
println!("Coherence: {:.2}", output.coherence);
println!("Result: {}", output.content);
```

### Application 8: Swarm Intelligence

Collective behavior with coherence-enforced coordination.

```rust
use delta_behavior::applications::swarm::{CoherentSwarm, SwarmAction};

let mut swarm = CoherentSwarm::new(0.6); // Min coherence

// Add agents
for i in 0..10 {
    swarm.add_agent(&format!("agent_{}", i), (i as f64, 0.0));
}

// Agent actions are validated against swarm coherence
let result = swarm.execute_action("agent_5", SwarmAction::Move { dx: 10.0, dy: 5.0 });

match result {
    ActionResult::Executed => println!("Action completed"),
    ActionResult::Modified { original, modified, reason } => {
        println!("Modified: {} -> {} ({})", original, modified, reason);
    }
    ActionResult::Rejected { reason } => {
        println!("Rejected: {}", reason);
    }
}
```

### Application 10: Pre-AGI Containment

Intelligence growth bounded by coherence.

```rust
use delta_behavior::applications::containment::{
    ContainmentSubstrate, CapabilityDomain, GrowthResult
};

let mut substrate = ContainmentSubstrate::new();

// Attempt capability growth
let result = substrate.attempt_growth(CapabilityDomain::Reasoning, 0.5);

match result {
    GrowthResult::Approved { increase, new_level, coherence_cost, .. } => {
        println!("Grew by {:.2} to {:.2} (cost: {:.3})", increase, new_level, coherence_cost);
    }
    GrowthResult::Dampened { requested, actual, reason, .. } => {
        println!("Dampened: {:.2} -> {:.2} ({})", requested, actual, reason);
    }
    GrowthResult::Blocked { reason, .. } => {
        println!("Blocked: {}", reason);
    }
    GrowthResult::Lockdown { reason } => {
        println!("LOCKDOWN: {}", reason);
    }
}
```

---

## Integration Examples

### With Async Runtimes

```rust
use delta_behavior::{DeltaConfig, enforcement::DeltaEnforcer, Coherence};
use tokio::sync::Mutex;
use std::sync::Arc;

struct AsyncDeltaSystem {
    enforcer: Arc<Mutex<DeltaEnforcer>>,
    state: Arc<Mutex<SystemState>>,
}

impl AsyncDeltaSystem {
    pub async fn transition(&self, delta: Delta) -> Result<(), Error> {
        let mut enforcer = self.enforcer.lock().await;
        let state = self.state.lock().await;

        let current = state.coherence();
        let predicted = state.predict_coherence(&delta);

        match enforcer.check(current, predicted) {
            EnforcementResult::Allowed => {
                drop(state);  // Release read lock
                let mut state = self.state.lock().await;
                state.apply(delta);
                Ok(())
            }
            EnforcementResult::Throttled(duration) => {
                drop(enforcer);
                drop(state);
                tokio::time::sleep(duration).await;
                self.transition(delta).await  // Retry
            }
            EnforcementResult::Blocked(reason) => {
                Err(Error::Blocked(reason))
            }
        }
    }
}
```

### With WASM

```rust
use wasm_bindgen::prelude::*;
use delta_behavior::{Coherence, CoherenceBounds};

#[wasm_bindgen]
pub struct WasmCoherenceMeter {
    current: f64,
    bounds: CoherenceBounds,
}

#[wasm_bindgen]
impl WasmCoherenceMeter {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            current: 1.0,
            bounds: CoherenceBounds::default(),
        }
    }

    #[wasm_bindgen]
    pub fn check(&self, predicted: f64) -> bool {
        predicted >= self.bounds.min_coherence.value()
    }

    #[wasm_bindgen]
    pub fn update(&mut self, new_coherence: f64) {
        self.current = new_coherence.clamp(0.0, 1.0);
    }

    #[wasm_bindgen]
    pub fn current(&self) -> f64 {
        self.current
    }
}
```

### With Machine Learning Frameworks

```rust
use delta_behavior::{DeltaSystem, Coherence, DeltaConfig};

struct CoherentNeuralNetwork {
    weights: Vec<Vec<f64>>,
    coherence: Coherence,
    config: DeltaConfig,
}

impl CoherentNeuralNetwork {
    /// Training step with coherence constraints
    pub fn train_step(&mut self, gradients: &[Vec<f64>], learning_rate: f64) -> Result<(), String> {
        // Compute coherence impact of update
        let update_magnitude: f64 = gradients.iter()
            .flat_map(|g| g.iter())
            .map(|x| (x * learning_rate).abs())
            .sum();

        let predicted_coherence = Coherence::clamped(
            self.coherence.value() - update_magnitude * 0.01
        );

        // Check bounds
        if predicted_coherence.value() < self.config.bounds.min_coherence.value() {
            // Reduce learning rate to maintain coherence
            let safe_lr = learning_rate * 0.5;
            return self.train_step(gradients, safe_lr);
        }

        // Apply update
        for (w, g) in self.weights.iter_mut().zip(gradients) {
            for (wi, gi) in w.iter_mut().zip(g) {
                *wi -= gi * learning_rate;
            }
        }

        self.coherence = predicted_coherence;
        Ok(())
    }
}
```

---

## Best Practices

### 1. Choose Appropriate Coherence Metrics

Match your coherence computation to your domain:
- **Vector spaces**: Distance variance, neighborhood consistency
- **Graphs**: Clustering coefficient, modularity, connectivity
- **Agent systems**: Entropy, goal alignment, memory consistency

### 2. Start Conservative, Relax Gradually

Begin with `DeltaConfig::strict()` and relax constraints as you understand your system's behavior.

### 3. Implement Graceful Degradation

Always handle `Throttled` and `Blocked` results:

```rust
fn robust_transition(system: &mut MySystem, delta: Delta) -> Result<(), Error> {
    for attempt in 0..3 {
        match system.try_transition(&delta) {
            Ok(()) => return Ok(()),
            Err(TransitionError::Throttled(delay)) => {
                std::thread::sleep(delay);
            }
            Err(TransitionError::Blocked(_)) if attempt < 2 => {
                delta = delta.dampen(0.5);  // Try smaller delta
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(Error::MaxRetriesExceeded)
}
```

### 4. Monitor Coherence Trends

Track coherence over time to detect gradual degradation:

```rust
let mut state = CoherenceState::new(Coherence::maximum());

// In your main loop
state.update(system.coherence());

if state.is_declining() && state.current.value() < 0.6 {
    // Trigger recovery actions
    system.enter_recovery_mode();
}
```

### 5. Use Attractors for Stability

Pre-compute and register stable states:

```rust
let attractors = discover_attractors(&system, 1000);

for attractor in attractors {
    system.register_attractor(attractor);
}

// Now transitions will be biased toward these stable states
```

---

## Troubleshooting

### High Rejection Rate

If too many transitions are being blocked:

1. Check if `max_delta_drop` is too restrictive
2. Consider using `DeltaConfig::relaxed()`
3. Ensure coherence computation is correctly calibrated

### Energy Exhaustion

If running out of energy budget:

1. Increase `budget_per_tick`
2. Lower `instability_exponent` for gentler cost curves
3. Call `enforcer.tick()` more frequently

### Stuck in Recovery Mode

If the system stays in recovery mode:

1. Reduce `recovery_margin`
2. Implement active coherence restoration
3. Lower `min_write_coherence` temporarily

---

## Version History

See [CHANGELOG.md](../CHANGELOG.md) for version history.

## License

MIT OR Apache-2.0
