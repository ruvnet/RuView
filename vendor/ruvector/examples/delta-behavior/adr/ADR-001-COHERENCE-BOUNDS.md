# ADR-001: Coherence Bounds and Measurement

## Status
PROPOSED

## Context

For Δ-behavior to be enforced, we must be able to **measure coherence** and define **bounds** that constrain transitions.

## Decision Drivers

1. **Measurability**: Coherence must be computable in O(1) or O(log n)
2. **Monotonicity**: Coherence should degrade predictably
3. **Composability**: Local coherence should aggregate to global coherence
4. **Hardware-friendliness**: Must be SIMD/WASM accelerable

## Coherence Definition

**Coherence** is a scalar measure of system organization:

```
C(S) ∈ [0, 1]  where 1 = maximally coherent, 0 = maximally disordered
```

### For Vector Spaces (HNSW)

```rust
/// Coherence of a vector neighborhood
pub fn vector_coherence(center: &Vector, neighbors: &[Vector]) -> f64 {
    let distances: Vec<f64> = neighbors
        .iter()
        .map(|n| cosine_distance(center, n))
        .collect();

    let mean_dist = distances.iter().sum::<f64>() / distances.len() as f64;
    let variance = distances
        .iter()
        .map(|d| (d - mean_dist).powi(2))
        .sum::<f64>() / distances.len() as f64;

    // Low variance = high coherence (tight neighborhood)
    1.0 / (1.0 + variance)
}
```

### For Graphs

```rust
/// Coherence of graph structure
pub fn graph_coherence(graph: &Graph) -> f64 {
    let clustering_coeff = compute_clustering_coefficient(graph);
    let modularity = compute_modularity(graph);
    let connectivity = compute_algebraic_connectivity(graph);

    // Weighted combination
    0.4 * clustering_coeff + 0.3 * modularity + 0.3 * connectivity.min(1.0)
}
```

### For Agent State

```rust
/// Coherence of agent memory/attention
pub fn agent_coherence(state: &AgentState) -> f64 {
    let attention_entropy = compute_attention_entropy(&state.attention);
    let memory_consistency = compute_memory_consistency(&state.memory);
    let goal_alignment = compute_goal_alignment(&state.goals, &state.actions);

    // Low entropy + high consistency + high alignment = coherent
    let coherence = (1.0 - attention_entropy) * memory_consistency * goal_alignment;
    coherence.clamp(0.0, 1.0)
}
```

## Coherence Bounds

### Static Bounds (Structural)

```rust
pub struct CoherenceBounds {
    /// Minimum coherence to allow any transition
    pub min_coherence: f64,  // e.g., 0.3

    /// Coherence below which transitions are throttled
    pub throttle_threshold: f64,  // e.g., 0.5

    /// Target coherence the system seeks
    pub target_coherence: f64,  // e.g., 0.8

    /// Maximum coherence drop per transition
    pub max_delta_drop: f64,  // e.g., 0.1
}

impl Default for CoherenceBounds {
    fn default() -> Self {
        Self {
            min_coherence: 0.3,
            throttle_threshold: 0.5,
            target_coherence: 0.8,
            max_delta_drop: 0.1,
        }
    }
}
```

### Dynamic Bounds (Learned)

```rust
pub struct AdaptiveCoherenceBounds {
    base: CoherenceBounds,

    /// Historical coherence trajectory
    history: RingBuffer<f64>,

    /// Learned adjustment factors
    adjustment: CoherenceAdjustment,
}

impl AdaptiveCoherenceBounds {
    pub fn effective_min_coherence(&self) -> f64 {
        let trend = self.history.trend();
        let adjustment = if trend < 0.0 {
            // Coherence declining: tighten bounds
            self.adjustment.tightening_factor
        } else {
            // Coherence stable/rising: relax bounds
            self.adjustment.relaxation_factor
        };

        (self.base.min_coherence * adjustment).clamp(0.1, 0.9)
    }
}
```

## Transition Validation

```rust
pub enum TransitionDecision {
    Allow,
    Throttle { delay_ms: u64 },
    Reroute { alternative: Transition },
    Reject { reason: RejectionReason },
}

pub fn validate_transition(
    current_coherence: f64,
    predicted_coherence: f64,
    bounds: &CoherenceBounds,
) -> TransitionDecision {
    let coherence_drop = current_coherence - predicted_coherence;

    // Hard rejection: would drop below minimum
    if predicted_coherence < bounds.min_coherence {
        return TransitionDecision::Reject {
            reason: RejectionReason::BelowMinimumCoherence,
        };
    }

    // Hard rejection: drop too large
    if coherence_drop > bounds.max_delta_drop {
        return TransitionDecision::Reject {
            reason: RejectionReason::ExcessiveCoherenceDrop,
        };
    }

    // Throttling: below target
    if predicted_coherence < bounds.throttle_threshold {
        let severity = (bounds.throttle_threshold - predicted_coherence)
            / bounds.throttle_threshold;
        let delay = (severity * 1000.0) as u64;  // Up to 1 second
        return TransitionDecision::Throttle { delay_ms: delay };
    }

    TransitionDecision::Allow
}
```

## WASM Implementation

```rust
// ruvector-delta-wasm/src/coherence.rs

#[wasm_bindgen]
pub struct CoherenceMeter {
    bounds: CoherenceBounds,
    current: f64,
    history: Vec<f64>,
}

#[wasm_bindgen]
impl CoherenceMeter {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            bounds: CoherenceBounds::default(),
            current: 1.0,
            history: Vec::with_capacity(1000),
        }
    }

    #[wasm_bindgen]
    pub fn measure_vector_coherence(&self, center: &[f32], neighbors: &[f32], dim: usize) -> f64 {
        // SIMD-accelerated coherence measurement
        #[cfg(target_feature = "simd128")]
        {
            simd_vector_coherence(center, neighbors, dim)
        }
        #[cfg(not(target_feature = "simd128"))]
        {
            scalar_vector_coherence(center, neighbors, dim)
        }
    }

    #[wasm_bindgen]
    pub fn validate(&self, predicted_coherence: f64) -> JsValue {
        let decision = validate_transition(self.current, predicted_coherence, &self.bounds);
        serde_wasm_bindgen::to_value(&decision).unwrap()
    }

    #[wasm_bindgen]
    pub fn update(&mut self, new_coherence: f64) {
        self.history.push(self.current);
        if self.history.len() > 1000 {
            self.history.remove(0);
        }
        self.current = new_coherence;
    }
}
```

## Consequences

### Positive
- Coherence is measurable and bounded
- Transitions are predictably constrained
- System has quantifiable stability guarantees

### Negative
- Adds overhead to every transition
- Requires calibration per domain
- May reject valid but "unusual" operations

### Neutral
- Shifts optimization target from raw speed to stable speed

## References

- Newman, M. E. J. (2003). "The Structure and Function of Complex Networks"
- Fiedler, M. (1973). "Algebraic Connectivity of Graphs"
- Shannon, C. E. (1948). "A Mathematical Theory of Communication" - entropy
