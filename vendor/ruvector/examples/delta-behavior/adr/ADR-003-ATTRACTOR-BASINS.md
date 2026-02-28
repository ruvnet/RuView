# ADR-003: Attractor Basins and Closure Preference

## Status
PROPOSED

## Context

Δ-behavior systems **prefer closure** - they naturally settle into stable, repeatable patterns called **attractors**.

## What Are Attractors?

An **attractor** is a state (or set of states) toward which the system naturally evolves:

```
trajectory(s₀, t) → A as t → ∞
```

Types of attractors:
- **Fixed point**: Single stable state
- **Limit cycle**: Repeating sequence of states
- **Strange attractor**: Complex but bounded pattern (chaos with structure)

## Attractor Basins

The **basin of attraction** is the set of all initial states that evolve toward a given attractor:

```
Basin(A) = { s₀ : trajectory(s₀, t) → A }
```

## Implementation

### Attractor Discovery

```rust
/// Discovered attractor in the system
pub struct Attractor {
    /// Unique identifier
    pub id: AttractorId,

    /// Type of attractor
    pub kind: AttractorKind,

    /// Representative state(s)
    pub states: Vec<SystemState>,

    /// Stability measure (higher = more stable)
    pub stability: f64,

    /// Coherence when in this attractor
    pub coherence: f64,

    /// Energy cost to reach this attractor
    pub energy_cost: f64,
}

pub enum AttractorKind {
    /// Single stable state
    FixedPoint,

    /// Repeating cycle of states
    LimitCycle { period: usize },

    /// Bounded but complex pattern
    StrangeAttractor { lyapunov_exponent: f64 },
}

/// Attractor discovery through simulation
pub struct AttractorDiscoverer {
    /// Number of random initial states to try
    sample_count: usize,

    /// Maximum simulation steps
    max_steps: usize,

    /// Convergence threshold
    convergence_epsilon: f64,
}

impl AttractorDiscoverer {
    pub fn discover(&self, system: &impl DeltaSystem) -> Vec<Attractor> {
        let mut attractors: HashMap<AttractorId, Attractor> = HashMap::new();

        for _ in 0..self.sample_count {
            let initial = system.random_state();
            let trajectory = self.simulate(system, initial);

            if let Some(attractor) = self.identify_attractor(&trajectory) {
                attractors
                    .entry(attractor.id.clone())
                    .or_insert(attractor)
                    .stability += 1.0;  // More samples → more stable
            }
        }

        // Normalize stability
        let max_stability = attractors.values().map(|a| a.stability).max_by(f64::total_cmp);
        for attractor in attractors.values_mut() {
            attractor.stability /= max_stability.unwrap_or(1.0);
        }

        attractors.into_values().collect()
    }

    fn simulate(&self, system: &impl DeltaSystem, initial: SystemState) -> Vec<SystemState> {
        let mut trajectory = vec![initial.clone()];
        let mut current = initial;

        for _ in 0..self.max_steps {
            let next = system.step(&current);

            // Check convergence
            if current.distance(&next) < self.convergence_epsilon {
                break;
            }

            trajectory.push(next.clone());
            current = next;
        }

        trajectory
    }

    fn identify_attractor(&self, trajectory: &[SystemState]) -> Option<Attractor> {
        let n = trajectory.len();
        if n < 10 {
            return None;
        }

        // Check for fixed point (last states are identical)
        let final_states = &trajectory[n-5..];
        if final_states.windows(2).all(|w| w[0].distance(&w[1]) < self.convergence_epsilon) {
            return Some(Attractor {
                id: AttractorId::from_state(&trajectory[n-1]),
                kind: AttractorKind::FixedPoint,
                states: vec![trajectory[n-1].clone()],
                stability: 1.0,
                coherence: trajectory[n-1].coherence(),
                energy_cost: 0.0,
            });
        }

        // Check for limit cycle
        for period in 2..20 {
            if n > period * 2 {
                let recent = &trajectory[n-period..];
                let previous = &trajectory[n-2*period..n-period];

                if recent.iter().zip(previous).all(|(a, b)| a.distance(b) < self.convergence_epsilon) {
                    return Some(Attractor {
                        id: AttractorId::from_cycle(recent),
                        kind: AttractorKind::LimitCycle { period },
                        states: recent.to_vec(),
                        stability: 1.0,
                        coherence: recent.iter().map(|s| s.coherence()).sum::<f64>() / period as f64,
                        energy_cost: 0.0,
                    });
                }
            }
        }

        None
    }
}
```

### Attractor-Aware Transitions

```rust
/// System that prefers transitions toward attractors
pub struct AttractorGuidedSystem {
    /// Known attractors
    attractors: Vec<Attractor>,

    /// Current state
    current: SystemState,

    /// Guidance strength (0 = no guidance, 1 = strong guidance)
    guidance_strength: f64,
}

impl AttractorGuidedSystem {
    /// Find nearest attractor to current state
    pub fn nearest_attractor(&self) -> Option<&Attractor> {
        self.attractors
            .iter()
            .min_by(|a, b| {
                let dist_a = self.distance_to_attractor(a);
                let dist_b = self.distance_to_attractor(b);
                dist_a.partial_cmp(&dist_b).unwrap()
            })
    }

    fn distance_to_attractor(&self, attractor: &Attractor) -> f64 {
        attractor
            .states
            .iter()
            .map(|s| self.current.distance(s))
            .min_by(f64::total_cmp)
            .unwrap_or(f64::INFINITY)
    }

    /// Bias transition toward attractor
    pub fn guided_transition(&self, proposed: Transition) -> Transition {
        if let Some(attractor) = self.nearest_attractor() {
            let current_dist = self.distance_to_attractor(attractor);
            let proposed_state = proposed.apply_to(&self.current);
            let proposed_dist = attractor
                .states
                .iter()
                .map(|s| proposed_state.distance(s))
                .min_by(f64::total_cmp)
                .unwrap_or(f64::INFINITY);

            // If proposed moves away from attractor, dampen it
            if proposed_dist > current_dist {
                let damping = (proposed_dist - current_dist) / current_dist;
                let damping_factor = (1.0 - self.guidance_strength * damping).max(0.1);
                proposed.scale(damping_factor)
            } else {
                // Moving toward attractor - allow or amplify
                let boost = (current_dist - proposed_dist) / current_dist;
                let boost_factor = 1.0 + self.guidance_strength * boost * 0.5;
                proposed.scale(boost_factor)
            }
        } else {
            proposed
        }
    }
}
```

### Closure Pressure

```rust
/// Pressure that pushes system toward closure
pub struct ClosurePressure {
    /// Attractors to prefer
    attractors: Vec<Attractor>,

    /// Pressure strength
    strength: f64,

    /// History of recent states
    recent_states: RingBuffer<SystemState>,

    /// Divergence detection
    divergence_threshold: f64,
}

impl ClosurePressure {
    /// Compute closure pressure for a transition
    pub fn pressure(&self, from: &SystemState, transition: &Transition) -> f64 {
        let to = transition.apply_to(from);

        // Distance to nearest attractor (normalized)
        let attractor_dist = self.attractors
            .iter()
            .map(|a| self.normalized_distance(&to, a))
            .min_by(f64::total_cmp)
            .unwrap_or(1.0);

        // Divergence from recent trajectory
        let divergence = self.compute_divergence(&to);

        // Combined pressure: high when far from attractors and diverging
        self.strength * (attractor_dist + divergence) / 2.0
    }

    fn normalized_distance(&self, state: &SystemState, attractor: &Attractor) -> f64 {
        let min_dist = attractor
            .states
            .iter()
            .map(|s| state.distance(s))
            .min_by(f64::total_cmp)
            .unwrap_or(f64::INFINITY);

        // Normalize by attractor's typical basin size (heuristic)
        (min_dist / attractor.stability.max(0.1)).min(1.0)
    }

    fn compute_divergence(&self, state: &SystemState) -> f64 {
        if self.recent_states.len() < 3 {
            return 0.0;
        }

        // Check if state is diverging from recent trajectory
        let recent_mean = self.recent_states.mean();
        let recent_variance = self.recent_states.variance();

        let deviation = state.distance(&recent_mean);
        let normalized_deviation = deviation / recent_variance.sqrt().max(0.001);

        (normalized_deviation / self.divergence_threshold).min(1.0)
    }

    /// Check if system is approaching an attractor
    pub fn is_converging(&self) -> bool {
        if self.recent_states.len() < 10 {
            return false;
        }

        let distances: Vec<f64> = self.recent_states
            .iter()
            .map(|s| {
                self.attractors
                    .iter()
                    .map(|a| a.states.iter().map(|as_| s.distance(as_)).min_by(f64::total_cmp).unwrap())
                    .min_by(f64::total_cmp)
                    .unwrap_or(f64::INFINITY)
            })
            .collect();

        // Check if distances are decreasing
        distances.windows(2).filter(|w| w[0] > w[1]).count() > distances.len() / 2
    }
}
```

### WASM Attractor Support

```rust
// ruvector-delta-wasm/src/attractor.rs

#[wasm_bindgen]
pub struct WasmAttractorField {
    attractors: Vec<WasmAttractor>,
    current_position: Vec<f32>,
}

#[wasm_bindgen]
pub struct WasmAttractor {
    center: Vec<f32>,
    strength: f32,
    radius: f32,
}

#[wasm_bindgen]
impl WasmAttractorField {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            attractors: Vec::new(),
            current_position: Vec::new(),
        }
    }

    #[wasm_bindgen]
    pub fn add_attractor(&mut self, center: &[f32], strength: f32, radius: f32) {
        self.attractors.push(WasmAttractor {
            center: center.to_vec(),
            strength,
            radius,
        });
    }

    #[wasm_bindgen]
    pub fn closure_force(&self, position: &[f32]) -> Vec<f32> {
        let mut force = vec![0.0f32; position.len()];

        for attractor in &self.attractors {
            let dist = euclidean_distance(position, &attractor.center);
            if dist < attractor.radius && dist > 0.001 {
                let magnitude = attractor.strength * (1.0 - dist / attractor.radius);
                for (i, f) in force.iter_mut().enumerate() {
                    *f += magnitude * (attractor.center[i] - position[i]) / dist;
                }
            }
        }

        force
    }

    #[wasm_bindgen]
    pub fn nearest_attractor_distance(&self, position: &[f32]) -> f32 {
        self.attractors
            .iter()
            .map(|a| euclidean_distance(position, &a.center))
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(f32::INFINITY)
    }
}
```

## Consequences

### Positive
- System naturally stabilizes
- Predictable long-term behavior
- Reduced computational exploration

### Negative
- May get stuck in suboptimal attractors
- Exploration is discouraged
- Novel states are harder to reach

### Neutral
- Trade-off between stability and adaptability
- Requires periodic attractor re-discovery
