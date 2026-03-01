# Δ-Behavior Domain Model

## Bounded Contexts

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Δ-BEHAVIOR SYSTEM                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐      │
│  │   COHERENCE      │    │   TRANSITION     │    │   ATTRACTOR      │      │
│  │   CONTEXT        │◄───│   CONTEXT        │───►│   CONTEXT        │      │
│  │                  │    │                  │    │                  │      │
│  │  • Measurement   │    │  • Validation    │    │  • Discovery     │      │
│  │  • Bounds        │    │  • Enforcement   │    │  • Basin mapping │      │
│  │  • History       │    │  • Scheduling    │    │  • Guidance      │      │
│  └──────────────────┘    └──────────────────┘    └──────────────────┘      │
│           │                       │                       │                 │
│           └───────────────────────┼───────────────────────┘                 │
│                                   │                                         │
│                                   ▼                                         │
│                    ┌──────────────────────────┐                             │
│                    │      ENFORCEMENT         │                             │
│                    │      CONTEXT             │                             │
│                    │                          │                             │
│                    │  • Energy costing        │                             │
│                    │  • Scheduling            │                             │
│                    │  • Memory gating         │                             │
│                    └──────────────────────────┘                             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Aggregate Roots

### 1. CoherenceState (Coherence Context)

```rust
/// Root aggregate for coherence tracking
pub struct CoherenceState {
    // Identity
    id: CoherenceStateId,

    // Value objects
    current_coherence: Coherence,
    bounds: CoherenceBounds,
    history: CoherenceHistory,

    // Computed
    trend: CoherenceTrend,
    stability_score: f64,

    // Domain events to publish
    events: Vec<CoherenceEvent>,
}

impl CoherenceState {
    /// Factory method - ensures valid initial state
    pub fn new(bounds: CoherenceBounds) -> Self {
        let mut state = Self {
            id: CoherenceStateId::generate(),
            current_coherence: Coherence::maximum(),
            bounds,
            history: CoherenceHistory::new(1000),
            trend: CoherenceTrend::Stable,
            stability_score: 1.0,
            events: Vec::new(),
        };
        state.events.push(CoherenceEvent::StateCreated { id: state.id });
        state
    }

    /// Update coherence - enforces invariants
    pub fn update(&mut self, new_coherence: Coherence) -> Result<(), CoherenceViolation> {
        // Invariant: cannot exceed bounds
        if new_coherence.value() > 1.0 || new_coherence.value() < 0.0 {
            return Err(CoherenceViolation::OutOfRange);
        }

        let old = self.current_coherence;
        self.history.record(old);
        self.current_coherence = new_coherence;
        self.trend = self.history.compute_trend();
        self.stability_score = self.compute_stability();

        // Emit events based on state change
        if new_coherence < self.bounds.min_coherence {
            self.events.push(CoherenceEvent::BelowMinimum {
                coherence: new_coherence,
                minimum: self.bounds.min_coherence,
            });
        }

        if self.trend == CoherenceTrend::Declining && self.stability_score < 0.5 {
            self.events.push(CoherenceEvent::StabilityWarning {
                score: self.stability_score,
            });
        }

        Ok(())
    }

    /// Check if transition would violate coherence bounds
    pub fn validate_transition(&self, predicted_post_coherence: Coherence) -> TransitionValidity {
        let drop = self.current_coherence.value() - predicted_post_coherence.value();

        if predicted_post_coherence < self.bounds.min_coherence {
            return TransitionValidity::Rejected(RejectionReason::BelowMinimum);
        }

        if drop > self.bounds.max_delta_drop {
            return TransitionValidity::Rejected(RejectionReason::ExcessiveDrop);
        }

        if predicted_post_coherence < self.bounds.throttle_threshold {
            return TransitionValidity::Throttled(self.compute_throttle_duration(drop));
        }

        TransitionValidity::Allowed
    }
}
```

### 2. TransitionRequest (Transition Context)

```rust
/// Root aggregate for transition lifecycle
pub struct TransitionRequest {
    // Identity
    id: TransitionId,

    // Specification
    spec: TransitionSpec,

    // State
    status: TransitionStatus,

    // Validation results
    coherence_validation: Option<TransitionValidity>,
    energy_cost: Option<EnergyCost>,
    priority: Option<TransitionPriority>,

    // Timestamps
    requested_at: Instant,
    validated_at: Option<Instant>,
    executed_at: Option<Instant>,

    // Events
    events: Vec<TransitionEvent>,
}

#[derive(Debug, Clone)]
pub enum TransitionStatus {
    Pending,
    Validated,
    Scheduled { priority: TransitionPriority },
    Throttled { until: Instant },
    Executing,
    Completed,
    Rejected { reason: RejectionReason },
}

impl TransitionRequest {
    /// Create new transition request
    pub fn new(spec: TransitionSpec) -> Self {
        let id = TransitionId::generate();
        let now = Instant::now();

        Self {
            id,
            spec,
            status: TransitionStatus::Pending,
            coherence_validation: None,
            energy_cost: None,
            priority: None,
            requested_at: now,
            validated_at: None,
            executed_at: None,
            events: vec![TransitionEvent::Requested { id, spec: spec.clone(), at: now }],
        }
    }

    /// Validate against coherence bounds
    pub fn validate(&mut self, coherence_state: &CoherenceState) -> &TransitionValidity {
        let predicted = self.spec.predict_coherence();
        let validity = coherence_state.validate_transition(predicted);

        self.coherence_validation = Some(validity.clone());
        self.validated_at = Some(Instant::now());

        match &validity {
            TransitionValidity::Allowed => {
                self.status = TransitionStatus::Validated;
                self.events.push(TransitionEvent::Validated { id: self.id });
            }
            TransitionValidity::Throttled(duration) => {
                self.status = TransitionStatus::Throttled { until: Instant::now() + *duration };
                self.events.push(TransitionEvent::Throttled { id: self.id, duration: *duration });
            }
            TransitionValidity::Rejected(reason) => {
                self.status = TransitionStatus::Rejected { reason: reason.clone() };
                self.events.push(TransitionEvent::Rejected { id: self.id, reason: reason.clone() });
            }
        }

        self.coherence_validation.as_ref().unwrap()
    }

    /// Assign energy cost
    pub fn assign_cost(&mut self, cost: EnergyCost) {
        self.energy_cost = Some(cost);
    }

    /// Schedule for execution
    pub fn schedule(&mut self, priority: TransitionPriority) {
        self.priority = Some(priority);
        self.status = TransitionStatus::Scheduled { priority };
        self.events.push(TransitionEvent::Scheduled { id: self.id, priority });
    }

    /// Execute the transition
    pub fn execute(&mut self) -> Result<TransitionResult, TransitionError> {
        match &self.status {
            TransitionStatus::Scheduled { .. } | TransitionStatus::Validated => {
                self.status = TransitionStatus::Executing;
                let result = self.spec.execute()?;
                self.status = TransitionStatus::Completed;
                self.executed_at = Some(Instant::now());
                self.events.push(TransitionEvent::Executed {
                    id: self.id,
                    at: self.executed_at.unwrap(),
                });
                Ok(result)
            }
            _ => Err(TransitionError::InvalidStatus(self.status.clone())),
        }
    }
}
```

### 3. AttractorBasin (Attractor Context)

```rust
/// Root aggregate for attractor management
pub struct AttractorBasin {
    // Identity
    id: AttractorBasinId,

    // The attractor itself
    attractor: Attractor,

    // Basin membership
    member_states: HashSet<StateFingerprint>,

    // Statistics
    entry_count: u64,
    exit_count: u64,
    average_residence_time: Duration,

    // Events
    events: Vec<AttractorEvent>,
}

impl AttractorBasin {
    /// Discover new attractor from trajectory
    pub fn from_trajectory(trajectory: &[SystemState]) -> Option<Self> {
        let attractor = Attractor::identify(trajectory)?;

        Some(Self {
            id: AttractorBasinId::from(&attractor),
            attractor,
            member_states: HashSet::new(),
            entry_count: 0,
            exit_count: 0,
            average_residence_time: Duration::ZERO,
            events: vec![AttractorEvent::Discovered { attractor: attractor.clone() }],
        })
    }

    /// Check if state is in this basin
    pub fn contains(&self, state: &SystemState) -> bool {
        let distance = self.attractor.distance_to(state);
        distance < self.attractor.basin_radius()
    }

    /// Record state entering basin
    pub fn record_entry(&mut self, state: &SystemState) {
        let fingerprint = state.fingerprint();
        if self.member_states.insert(fingerprint) {
            self.entry_count += 1;
            self.events.push(AttractorEvent::StateEntered {
                basin_id: self.id,
                state_fingerprint: fingerprint,
            });
        }
    }

    /// Record state leaving basin
    pub fn record_exit(&mut self, state: &SystemState, residence_time: Duration) {
        let fingerprint = state.fingerprint();
        if self.member_states.remove(&fingerprint) {
            self.exit_count += 1;
            self.update_average_residence_time(residence_time);
            self.events.push(AttractorEvent::StateExited {
                basin_id: self.id,
                state_fingerprint: fingerprint,
                residence_time,
            });
        }
    }

    /// Compute guidance force toward attractor
    pub fn guidance_force(&self, from: &SystemState) -> GuidanceForce {
        let direction = self.attractor.center().direction_from(from);
        let distance = self.attractor.distance_to(from);

        // Force decreases with distance (inverse square for smooth approach)
        let magnitude = self.attractor.stability / (1.0 + distance.powi(2));

        GuidanceForce { direction, magnitude }
    }
}
```

## Value Objects

```rust
/// Coherence value (0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Coherence(f64);

impl Coherence {
    pub fn new(value: f64) -> Result<Self, CoherenceError> {
        if value < 0.0 || value > 1.0 {
            Err(CoherenceError::OutOfRange(value))
        } else {
            Ok(Self(value))
        }
    }

    pub fn maximum() -> Self { Self(1.0) }
    pub fn minimum() -> Self { Self(0.0) }
    pub fn value(&self) -> f64 { self.0 }
}

/// Energy cost for a transition
#[derive(Debug, Clone, Copy)]
pub struct EnergyCost {
    base: f64,
    instability_factor: f64,
    total: f64,
}

/// Transition specification (immutable)
#[derive(Debug, Clone)]
pub struct TransitionSpec {
    pub operation: Operation,
    pub target: TransitionTarget,
    pub predicted_coherence_impact: f64,
    pub locality_score: f64,
}

/// Guidance force from attractor
#[derive(Debug, Clone)]
pub struct GuidanceForce {
    pub direction: Vec<f64>,
    pub magnitude: f64,
}
```

## Domain Events

```rust
// Coherence Events
pub enum CoherenceEvent {
    StateCreated { id: CoherenceStateId },
    Updated { from: Coherence, to: Coherence },
    BelowMinimum { coherence: Coherence, minimum: Coherence },
    StabilityWarning { score: f64 },
    EmergencyHalt,
}

// Transition Events
pub enum TransitionEvent {
    Requested { id: TransitionId, spec: TransitionSpec, at: Instant },
    Validated { id: TransitionId },
    Throttled { id: TransitionId, duration: Duration },
    Scheduled { id: TransitionId, priority: TransitionPriority },
    Executed { id: TransitionId, at: Instant },
    Rejected { id: TransitionId, reason: RejectionReason },
}

// Attractor Events
pub enum AttractorEvent {
    Discovered { attractor: Attractor },
    StateEntered { basin_id: AttractorBasinId, state_fingerprint: StateFingerprint },
    StateExited { basin_id: AttractorBasinId, state_fingerprint: StateFingerprint, residence_time: Duration },
    BasinExpanded { basin_id: AttractorBasinId, new_radius: f64 },
    AttractorMerged { absorbed: AttractorBasinId, into: AttractorBasinId },
}
```

## Domain Services

```rust
/// Service for coherence measurement
pub struct CoherenceMeasurementService {
    vector_measurer: VectorCoherenceMeasurer,
    graph_measurer: GraphCoherenceMeasurer,
    agent_measurer: AgentCoherenceMeasurer,
}

impl CoherenceMeasurementService {
    pub fn measure(&self, system: &SystemState) -> Coherence {
        match system.kind() {
            SystemKind::Vector => self.vector_measurer.measure(system),
            SystemKind::Graph => self.graph_measurer.measure(system),
            SystemKind::Agent => self.agent_measurer.measure(system),
        }
    }
}

/// Service for transition enforcement
pub struct TransitionEnforcementService {
    energy_layer: EnergyEnforcementLayer,
    scheduling_layer: SchedulingEnforcementLayer,
    gating_layer: GatingEnforcementLayer,
}

impl TransitionEnforcementService {
    pub fn enforce(&self, request: &mut TransitionRequest, coherence: &CoherenceState) -> EnforcementResult {
        // Layer 1: Energy
        let cost = self.energy_layer.compute_cost(&request.spec);
        request.assign_cost(cost);

        if !self.energy_layer.can_afford(cost) {
            return EnforcementResult::EnergyExhausted;
        }

        // Layer 2: Scheduling
        request.validate(coherence);
        match request.coherence_validation.as_ref().unwrap() {
            TransitionValidity::Rejected(reason) => {
                return EnforcementResult::Rejected(reason.clone());
            }
            TransitionValidity::Throttled(duration) => {
                return EnforcementResult::Throttled(*duration);
            }
            TransitionValidity::Allowed => {}
        }

        let priority = self.scheduling_layer.assign_priority(request, coherence);
        request.schedule(priority);

        // Layer 3: Gating (final check before execution)
        if !self.gating_layer.is_open() {
            return EnforcementResult::GateClosed;
        }

        EnforcementResult::Scheduled(priority)
    }
}

/// Service for attractor discovery
pub struct AttractorDiscoveryService {
    discoverer: AttractorDiscoverer,
    basins: HashMap<AttractorBasinId, AttractorBasin>,
}

impl AttractorDiscoveryService {
    pub fn discover_from_trajectory(&mut self, trajectory: &[SystemState]) {
        if let Some(basin) = AttractorBasin::from_trajectory(trajectory) {
            self.basins.insert(basin.id, basin);
        }
    }

    pub fn find_nearest_basin(&self, state: &SystemState) -> Option<&AttractorBasin> {
        self.basins.values()
            .filter(|b| b.contains(state))
            .min_by(|a, b| {
                a.attractor.distance_to(state)
                    .partial_cmp(&b.attractor.distance_to(state))
                    .unwrap()
            })
    }
}
```

## Repositories

```rust
/// Repository for coherence state persistence
#[async_trait]
pub trait CoherenceRepository {
    async fn save(&self, state: &CoherenceState) -> Result<(), RepositoryError>;
    async fn load(&self, id: CoherenceStateId) -> Result<CoherenceState, RepositoryError>;
    async fn history(&self, id: CoherenceStateId, limit: usize) -> Result<Vec<Coherence>, RepositoryError>;
}

/// Repository for attractor basins
#[async_trait]
pub trait AttractorRepository {
    async fn save_basin(&self, basin: &AttractorBasin) -> Result<(), RepositoryError>;
    async fn load_basin(&self, id: AttractorBasinId) -> Result<AttractorBasin, RepositoryError>;
    async fn all_basins(&self) -> Result<Vec<AttractorBasin>, RepositoryError>;
    async fn find_containing(&self, state: &SystemState) -> Result<Option<AttractorBasin>, RepositoryError>;
}

/// Repository for transition audit log
#[async_trait]
pub trait TransitionAuditRepository {
    async fn record(&self, request: &TransitionRequest) -> Result<(), RepositoryError>;
    async fn query(&self, filter: TransitionFilter) -> Result<Vec<TransitionRequest>, RepositoryError>;
}
```
