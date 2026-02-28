# ADR-002: Transition Constraints and Enforcement

## Status
PROPOSED

## Context

Δ-behavior requires that **unstable transitions are resisted**. This ADR defines the constraint mechanisms.

## The Three Enforcement Layers

```
┌─────────────────────────────────────────────────────────────┐
│                     Transition Request                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  LAYER 1: ENERGY COST (Soft Constraint)                     │
│  - Expensive transitions naturally deprioritized            │
│  - Self-regulating through resource limits                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  LAYER 2: SCHEDULING (Medium Constraint)                    │
│  - Unstable transitions delayed/throttled                   │
│  - Backpressure on high-instability operations              │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  LAYER 3: MEMORY GATE (Hard Constraint)                     │
│  - Incoherent writes blocked                                │
│  - State corruption prevented                               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Transition Applied                       │
│                  (or Rejected/Rerouted)                      │
└─────────────────────────────────────────────────────────────┘
```

## Layer 1: Energy Cost

### Concept
Make unstable transitions **expensive** so they naturally lose competition for resources.

### Implementation

```rust
/// Energy cost model for transitions
pub struct EnergyCostModel {
    /// Base cost for any transition
    base_cost: f64,

    /// Instability multiplier exponent
    instability_exponent: f64,

    /// Maximum cost cap (prevents infinite costs)
    max_cost: f64,
}

impl EnergyCostModel {
    pub fn compute_cost(&self, transition: &Transition) -> f64 {
        let instability = self.measure_instability(transition);
        let cost = self.base_cost * (1.0 + instability).powf(self.instability_exponent);
        cost.min(self.max_cost)
    }

    fn measure_instability(&self, transition: &Transition) -> f64 {
        let coherence_impact = transition.predicted_coherence_drop();
        let locality_violation = transition.non_local_effects();
        let attractor_distance = transition.distance_from_attractors();

        // Weighted instability score
        0.4 * coherence_impact + 0.3 * locality_violation + 0.3 * attractor_distance
    }
}

/// Resource-aware transition executor
pub struct EnergyAwareExecutor {
    cost_model: EnergyCostModel,
    budget: AtomicF64,
    budget_per_tick: f64,
}

impl EnergyAwareExecutor {
    pub fn execute(&self, transition: Transition) -> Result<(), EnergyExhausted> {
        let cost = self.cost_model.compute_cost(&transition);

        // Try to spend energy
        let mut budget = self.budget.load(Ordering::Acquire);
        loop {
            if budget < cost {
                return Err(EnergyExhausted { required: cost, available: budget });
            }

            match self.budget.compare_exchange_weak(
                budget,
                budget - cost,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(current) => budget = current,
            }
        }

        // Execute transition
        transition.apply()
    }

    pub fn replenish(&self) {
        // Called periodically to refill budget
        self.budget.fetch_add(self.budget_per_tick, Ordering::Release);
    }
}
```

### WASM Binding

```rust
#[wasm_bindgen]
pub struct WasmEnergyCost {
    model: EnergyCostModel,
}

#[wasm_bindgen]
impl WasmEnergyCost {
    #[wasm_bindgen(constructor)]
    pub fn new(base_cost: f64, exponent: f64, max_cost: f64) -> Self {
        Self {
            model: EnergyCostModel {
                base_cost,
                instability_exponent: exponent,
                max_cost,
            },
        }
    }

    #[wasm_bindgen]
    pub fn cost(&self, coherence_drop: f64, locality_violation: f64, attractor_dist: f64) -> f64 {
        let instability = 0.4 * coherence_drop + 0.3 * locality_violation + 0.3 * attractor_dist;
        (self.model.base_cost * (1.0 + instability).powf(self.model.instability_exponent))
            .min(self.model.max_cost)
    }
}
```

## Layer 2: Scheduling

### Concept
Delay or deprioritize transitions based on their stability impact.

### Implementation

```rust
/// Priority levels for transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransitionPriority {
    /// Execute immediately
    Immediate = 0,
    /// Execute soon
    High = 1,
    /// Execute when convenient
    Normal = 2,
    /// Execute when system is stable
    Low = 3,
    /// Defer until explicitly requested
    Deferred = 4,
}

/// Scheduler for Δ-constrained transitions
pub struct DeltaScheduler {
    /// Priority queues for each level
    queues: [VecDeque<Transition>; 5],

    /// Current system coherence
    coherence: AtomicF64,

    /// Scheduling policy
    policy: SchedulingPolicy,
}

pub struct SchedulingPolicy {
    /// Coherence threshold for each priority level
    coherence_thresholds: [f64; 5],

    /// Maximum transitions per tick at each priority
    rate_limits: [usize; 5],

    /// Backoff multiplier when coherence is low
    backoff_multiplier: f64,
}

impl DeltaScheduler {
    pub fn schedule(&mut self, transition: Transition) {
        let priority = self.compute_priority(&transition);
        self.queues[priority as usize].push_back(transition);
    }

    fn compute_priority(&self, transition: &Transition) -> TransitionPriority {
        let coherence_impact = transition.predicted_coherence_drop();
        let current_coherence = self.coherence.load(Ordering::Acquire);

        // Lower coherence = more conservative scheduling
        let adjusted_impact = coherence_impact / current_coherence.max(0.1);

        match adjusted_impact {
            x if x < 0.05 => TransitionPriority::Immediate,
            x if x < 0.10 => TransitionPriority::High,
            x if x < 0.20 => TransitionPriority::Normal,
            x if x < 0.40 => TransitionPriority::Low,
            _ => TransitionPriority::Deferred,
        }
    }

    pub fn tick(&mut self) -> Vec<Transition> {
        let current_coherence = self.coherence.load(Ordering::Acquire);
        let mut executed = Vec::new();

        for (priority, queue) in self.queues.iter_mut().enumerate() {
            // Check if coherence allows this priority level
            if current_coherence < self.policy.coherence_thresholds[priority] {
                continue;
            }

            // Execute up to rate limit
            let limit = self.policy.rate_limits[priority];
            for _ in 0..limit {
                if let Some(transition) = queue.pop_front() {
                    executed.push(transition);
                }
            }
        }

        executed
    }
}
```

### Backpressure Mechanism

```rust
/// Backpressure controller for high-instability periods
pub struct BackpressureController {
    /// Current backpressure level (0.0 = none, 1.0 = maximum)
    level: AtomicF64,

    /// Coherence history for trend detection
    history: RwLock<RingBuffer<f64>>,

    /// Configuration
    config: BackpressureConfig,
}

impl BackpressureController {
    pub fn update(&self, current_coherence: f64) {
        let mut history = self.history.write().unwrap();
        history.push(current_coherence);

        let trend = history.trend();  // Negative = declining
        let volatility = history.volatility();

        // Compute new backpressure level
        let base_pressure = if current_coherence < self.config.low_coherence_threshold {
            (self.config.low_coherence_threshold - current_coherence)
                / self.config.low_coherence_threshold
        } else {
            0.0
        };

        let trend_pressure = (-trend).max(0.0) * self.config.trend_sensitivity;
        let volatility_pressure = volatility * self.config.volatility_sensitivity;

        let total_pressure = (base_pressure + trend_pressure + volatility_pressure).clamp(0.0, 1.0);
        self.level.store(total_pressure, Ordering::Release);
    }

    pub fn apply_backpressure(&self, base_delay: Duration) -> Duration {
        let level = self.level.load(Ordering::Acquire);
        let multiplier = 1.0 + (level * self.config.max_delay_multiplier);
        Duration::from_secs_f64(base_delay.as_secs_f64() * multiplier)
    }
}
```

## Layer 3: Memory Gate

### Concept
The final line of defense: **block** writes that would corrupt coherence.

### Implementation

```rust
/// Memory gate that blocks incoherent writes
pub struct CoherenceGate {
    /// Current system coherence
    coherence: AtomicF64,

    /// Minimum coherence to allow writes
    min_write_coherence: f64,

    /// Minimum coherence after write
    min_post_write_coherence: f64,

    /// Gate state
    state: AtomicU8,  // 0=open, 1=throttled, 2=closed
}

#[derive(Debug)]
pub enum GateDecision {
    Open,
    Throttled { wait: Duration },
    Closed { reason: GateClosedReason },
}

#[derive(Debug)]
pub enum GateClosedReason {
    CoherenceTooLow,
    WriteTooDestabilizing,
    SystemInRecovery,
    EmergencyHalt,
}

impl CoherenceGate {
    pub fn check(&self, predicted_post_write_coherence: f64) -> GateDecision {
        let current = self.coherence.load(Ordering::Acquire);
        let state = self.state.load(Ordering::Acquire);

        // Emergency halt state
        if state == 2 {
            return GateDecision::Closed {
                reason: GateClosedReason::EmergencyHalt
            };
        }

        // Current coherence check
        if current < self.min_write_coherence {
            return GateDecision::Closed {
                reason: GateClosedReason::CoherenceTooLow
            };
        }

        // Post-write coherence check
        if predicted_post_write_coherence < self.min_post_write_coherence {
            return GateDecision::Closed {
                reason: GateClosedReason::WriteTooDestabilizing
            };
        }

        // Throttled state
        if state == 1 {
            let wait = Duration::from_millis(
                ((self.min_write_coherence - current) * 1000.0) as u64
            );
            return GateDecision::Throttled { wait };
        }

        GateDecision::Open
    }

    pub fn emergency_halt(&self) {
        self.state.store(2, Ordering::Release);
    }

    pub fn recover(&self) {
        let current = self.coherence.load(Ordering::Acquire);
        if current >= self.min_write_coherence * 1.2 {
            // 20% above minimum before reopening
            self.state.store(0, Ordering::Release);
        } else if current >= self.min_write_coherence {
            self.state.store(1, Ordering::Release);  // Throttled
        }
    }
}
```

### Gated Memory Write

```rust
/// Memory with coherence-gated writes
pub struct GatedMemory<T> {
    storage: RwLock<T>,
    gate: CoherenceGate,
    coherence_computer: Box<dyn Fn(&T) -> f64>,
}

impl<T: Clone> GatedMemory<T> {
    pub fn write(&self, mutator: impl FnOnce(&mut T)) -> Result<(), GateDecision> {
        // Simulate the write
        let mut simulation = self.storage.read().unwrap().clone();
        mutator(&mut simulation);

        // Compute post-write coherence
        let predicted_coherence = (self.coherence_computer)(&simulation);

        // Check gate
        match self.gate.check(predicted_coherence) {
            GateDecision::Open => {
                let mut storage = self.storage.write().unwrap();
                mutator(&mut storage);
                self.gate.coherence.store(predicted_coherence, Ordering::Release);
                Ok(())
            }
            decision => Err(decision),
        }
    }
}
```

## Combined Enforcement

```rust
/// Complete Δ-behavior enforcement system
pub struct DeltaEnforcer {
    energy: EnergyAwareExecutor,
    scheduler: DeltaScheduler,
    gate: CoherenceGate,
}

impl DeltaEnforcer {
    pub fn submit(&mut self, transition: Transition) -> EnforcementResult {
        // Layer 1: Energy check
        let cost = self.energy.cost_model.compute_cost(&transition);
        if self.energy.budget.load(Ordering::Acquire) < cost {
            return EnforcementResult::RejectedByEnergy { cost };
        }

        // Layer 2: Schedule
        self.scheduler.schedule(transition);

        EnforcementResult::Scheduled
    }

    pub fn execute_tick(&mut self) -> Vec<ExecutionResult> {
        let transitions = self.scheduler.tick();
        let mut results = Vec::new();

        for transition in transitions {
            // Layer 1: Spend energy
            if let Err(e) = self.energy.execute(transition.clone()) {
                results.push(ExecutionResult::EnergyExhausted(e));
                self.scheduler.schedule(transition);  // Re-queue
                continue;
            }

            // Layer 3: Gate check
            let predicted = transition.predict_coherence();
            match self.gate.check(predicted) {
                GateDecision::Open => {
                    transition.apply();
                    self.gate.coherence.store(predicted, Ordering::Release);
                    results.push(ExecutionResult::Applied);
                }
                GateDecision::Throttled { wait } => {
                    results.push(ExecutionResult::Throttled(wait));
                    self.scheduler.schedule(transition);  // Re-queue
                }
                GateDecision::Closed { reason } => {
                    results.push(ExecutionResult::Rejected(reason));
                }
            }
        }

        results
    }
}
```

## Consequences

### Positive
- Three independent safety layers
- Graceful degradation under stress
- Self-regulating resource usage

### Negative
- Added complexity
- Potential for false rejections
- Requires careful tuning

### Neutral
- Clear separation of concerns
- Debuggable enforcement chain
