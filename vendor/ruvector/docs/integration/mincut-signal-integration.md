# MinCut Coherence Signal Integration Research

**Author:** Research Agent
**Date:** 2026-01-01
**Status:** Research Complete

---

## Executive Summary

MinCut tension serves as the central coherence signal for RuVector's self-healing infrastructure. This document analyzes the current signal flow architecture and recommends an event bus design to coordinate all subsystems through unified coherence signals.

**Key Finding:** The 0.7 tension threshold (lambda_min=30 with drop_ratio ~37.5%) triggers intervention across transformer attention, learning rate boosts, and self-healing mechanisms.

---

## 1. Current Signal Flow Architecture

### 1.1 GatePacket: The Core Coherence Signal

The `GatePacket` structure (defined in `/workspaces/ruvector/crates/ruvector-mincut-gated-transformer/src/packets.rs`) is the primary coherence carrier:

```rust
#[repr(C)]
pub struct GatePacket {
    /// Current lambda (minimum cut value / coherence metric)
    pub lambda: u32,

    /// Previous lambda for trend detection
    pub lambda_prev: u32,

    /// Number of edges crossing partition boundaries
    pub boundary_edges: u16,

    /// Boundary edge concentration (Q15: 0-32767)
    pub boundary_concentration_q15: u16,

    /// Number of partitions in current graph state
    pub partition_count: u16,

    /// Policy flags (force safe mode, etc.)
    pub flags: u16,
}
```

**Critical Methods:**
- `drop_ratio_q15()` - Computes normalized drop rate: `((lambda_prev - lambda) * 32768) / lambda_prev`
- `lambda_delta()` - Signed delta for trend analysis
- Flag constants: `FLAG_FORCE_SAFE`, `FLAG_SKIP`, `FLAG_BOUNDARY_IDS_AVAILABLE`

### 1.2 Current Signal Propagation Path

```
                    MinCut Engine (ruvector-mincut)
                              |
                              v
                    +-------------------+
                    |   GatePacket      |
                    | lambda, boundary, |
                    | partition_count   |
                    +-------------------+
                              |
              +---------------+---------------+
              |               |               |
              v               v               v
     +-------------+  +-------------+  +-------------+
     |  GateController  |  Transformer  |  Early Exit |
     |  (gate.rs)   |  |   Model     |  | (speculative)|
     +-------------+  +-------------+  +-------------+
              |               |               |
              v               v               v
     +-------------+  +-------------+  +-------------+
     | TierDecision|  | Attention   |  | Layer Skip  |
     | (0-3 tiers) |  | Window Size |  | Decision    |
     +-------------+  +-------------+  +-------------+
```

### 1.3 GatePolicy Thresholds (Critical Values)

From `/workspaces/ruvector/crates/ruvector-mincut-gated-transformer/src/config.rs`:

| Parameter | Default | Conservative | Permissive | Meaning |
|-----------|---------|--------------|------------|---------|
| `lambda_min` | 30 | 50 | 20 | Minimum coherence before quarantine |
| `drop_ratio_q15_max` | 12288 (~37.5%) | 8192 (~25%) | 16384 (~50%) | Max drop before FlushKV |
| `boundary_edges_max` | 20 | 10 | 50 | Max boundary crossings |
| `boundary_concentration_q15_max` | 20480 (~62.5%) | 16384 (~50%) | 24576 (~75%) | Concentration limit |
| `partitions_max` | 10 | 5 | 20 | Max partition fragmentation |

**The 0.7 Threshold:** When lambda drops below 30 (default `lambda_min`), or when `drop_ratio_q15 > 12288` (about 37.5% drop, roughly equivalent to crossing 0.7 of previous stability), interventions trigger.

### 1.4 Gate Decisions and Their Effects

```rust
pub enum GateDecision {
    Allow = 0,            // Normal operation
    ReduceScope = 1,      // Reduce seq_len and window
    FlushKv = 2,          // Flush KV cache before proceeding
    FreezeWrites = 3,     // Read-only mode (no KV updates)
    QuarantineUpdates = 4, // Discard all state changes
}
```

**Tier Mapping:**
- Tier 0: Normal operation (4 layers, 64 seq_len, 16 window)
- Tier 1: Degraded mode (2 layers, 32 seq_len, 8 window)
- Tier 2: Safe mode (1 layer, 8 seq_len, 4 window)
- Tier 3: Skip (no computation)

---

## 2. Recommended Event Bus Design

### 2.1 Unified Coherence Event Bus

```rust
/// Central event bus for coherence signal distribution
pub struct CoherenceEventBus {
    /// Current coherence state
    current_state: CoherenceState,

    /// Registered listeners by subsystem
    listeners: Vec<Box<dyn CoherenceListener>>,

    /// Event history for replay/debugging
    history: RingBuffer<CoherenceEvent>,

    /// Metrics collector
    metrics: CoherenceMetrics,
}

/// Coherence state derived from MinCut signals
#[derive(Clone, Debug)]
pub struct CoherenceState {
    /// Current lambda (min-cut value)
    pub lambda: u32,

    /// Trend direction (-1, 0, +1)
    pub trend: i8,

    /// Stability score (0.0 - 1.0)
    pub stability: f32,

    /// Computed tension level (0.0 - 1.0)
    pub tension: f32,

    /// Recommended intervention tier
    pub recommended_tier: u8,

    /// Timestamp
    pub timestamp_ms: u64,
}

/// Events emitted by the coherence bus
#[derive(Clone, Debug)]
pub enum CoherenceEvent {
    /// Lambda changed
    LambdaUpdate {
        old: u32,
        new: u32,
        delta_ratio: f32,
    },

    /// Tension threshold crossed
    TensionThreshold {
        threshold: f32,
        direction: ThresholdDirection,
    },

    /// Intervention triggered
    InterventionTriggered {
        decision: GateDecision,
        reason: GateReason,
    },

    /// Recovery detected
    RecoveryDetected {
        from_tier: u8,
        to_tier: u8,
    },

    /// Partition structure changed
    PartitionChanged {
        old_count: u16,
        new_count: u16,
        boundary_edges: u16,
    },
}

/// Trait for subsystems that respond to coherence signals
pub trait CoherenceListener: Send + Sync {
    /// Called when coherence state changes
    fn on_coherence_update(&mut self, state: &CoherenceState, event: &CoherenceEvent);

    /// Called to query current subsystem health
    fn health(&self) -> SubsystemHealth;

    /// Subsystem identifier
    fn id(&self) -> &'static str;
}
```

### 2.2 Event Bus Implementation

```rust
impl CoherenceEventBus {
    pub fn new(capacity: usize) -> Self {
        Self {
            current_state: CoherenceState::default(),
            listeners: Vec::new(),
            history: RingBuffer::new(capacity),
            metrics: CoherenceMetrics::new(),
        }
    }

    /// Process incoming GatePacket and emit events
    pub fn process_gate_packet(&mut self, packet: &GatePacket) {
        let old_state = self.current_state.clone();

        // Compute new state
        let tension = self.compute_tension(packet);
        let trend = if packet.lambda > packet.lambda_prev {
            1
        } else if packet.lambda < packet.lambda_prev {
            -1
        } else {
            0
        };

        self.current_state = CoherenceState {
            lambda: packet.lambda,
            trend,
            stability: 1.0 - tension,
            tension,
            recommended_tier: self.recommend_tier(tension, packet),
            timestamp_ms: Self::now_ms(),
        };

        // Emit events
        self.emit_events(&old_state, packet);
    }

    fn compute_tension(&self, packet: &GatePacket) -> f32 {
        // Tension = weighted combination of signals
        let lambda_factor = if packet.lambda < 30 {
            1.0
        } else {
            1.0 - (packet.lambda as f32 / 100.0).min(1.0)
        };

        let drop_factor = (packet.drop_ratio_q15() as f32) / 32767.0;
        let boundary_factor = (packet.boundary_concentration_q15 as f32) / 32767.0;
        let partition_factor = (packet.partition_count as f32 / 10.0).min(1.0);

        // Weighted tension (drop is most critical)
        0.4 * drop_factor + 0.3 * lambda_factor + 0.2 * boundary_factor + 0.1 * partition_factor
    }

    fn emit_events(&mut self, old: &CoherenceState, packet: &GatePacket) {
        // Lambda update event
        if old.lambda != self.current_state.lambda {
            let event = CoherenceEvent::LambdaUpdate {
                old: old.lambda,
                new: self.current_state.lambda,
                delta_ratio: (self.current_state.lambda as f32 - old.lambda as f32) / old.lambda.max(1) as f32,
            };
            self.dispatch_event(event);
        }

        // Tension threshold events
        let thresholds = [0.3, 0.5, 0.7, 0.9];
        for &threshold in &thresholds {
            if (old.tension < threshold) != (self.current_state.tension < threshold) {
                let direction = if self.current_state.tension >= threshold {
                    ThresholdDirection::Crossed
                } else {
                    ThresholdDirection::Recovered
                };
                self.dispatch_event(CoherenceEvent::TensionThreshold { threshold, direction });
            }
        }

        // Tier change events
        if old.recommended_tier != self.current_state.recommended_tier {
            if self.current_state.recommended_tier < old.recommended_tier {
                self.dispatch_event(CoherenceEvent::RecoveryDetected {
                    from_tier: old.recommended_tier,
                    to_tier: self.current_state.recommended_tier,
                });
            }
        }
    }

    fn dispatch_event(&mut self, event: CoherenceEvent) {
        self.history.push(event.clone());
        self.metrics.record_event(&event);

        for listener in &mut self.listeners {
            listener.on_coherence_update(&self.current_state, &event);
        }
    }

    pub fn register(&mut self, listener: Box<dyn CoherenceListener>) {
        self.listeners.push(listener);
    }
}
```

---

## 3. Integration Points for Each Subsystem

### 3.1 SONA (Self-Optimizing Neural Architecture) Integration

SONA's learning loops should respond to coherence signals:

```rust
/// SONA coherence listener
pub struct SonaCoherenceListener {
    coordinator: Arc<LoopCoordinator>,
    base_learning_rate: f32,
}

impl CoherenceListener for SonaCoherenceListener {
    fn on_coherence_update(&mut self, state: &CoherenceState, event: &CoherenceEvent) {
        match event {
            // Boost learning rate when recovering from instability
            CoherenceEvent::RecoveryDetected { from_tier, to_tier } => {
                if *from_tier > 1 && *to_tier <= 1 {
                    // Boost learning rate during recovery
                    let boost_factor = 1.0 + (1.0 - state.tension) * 0.5;
                    self.coordinator.set_learning_rate(self.base_learning_rate * boost_factor);
                }
            }

            // Pause background learning during high tension
            CoherenceEvent::TensionThreshold { threshold, direction } => {
                if *threshold >= 0.7 && matches!(direction, ThresholdDirection::Crossed) {
                    self.coordinator.set_background_enabled(false);
                } else if *threshold >= 0.7 && matches!(direction, ThresholdDirection::Recovered) {
                    self.coordinator.set_background_enabled(true);
                }
            }

            _ => {}
        }
    }

    fn health(&self) -> SubsystemHealth {
        let stats = self.coordinator.stats();
        SubsystemHealth {
            name: "sona",
            status: if stats.background_enabled { "active" } else { "paused" },
            metrics: vec![
                ("trajectories_buffered", stats.trajectories_buffered as f64),
                ("patterns_stored", stats.patterns_stored as f64),
            ],
        }
    }

    fn id(&self) -> &'static str { "sona" }
}
```

### 3.2 Attention Selection Integration

The transformer attention mechanism already responds via GateController, but we can enhance with event-driven updates:

```rust
/// Attention coherence listener for adaptive window sizing
pub struct AttentionCoherenceListener {
    gate_controller: Arc<RwLock<GateController>>,
    window_history: VecDeque<u16>,
}

impl CoherenceListener for AttentionCoherenceListener {
    fn on_coherence_update(&mut self, state: &CoherenceState, event: &CoherenceEvent) {
        match event {
            CoherenceEvent::LambdaUpdate { old, new, delta_ratio } => {
                // Predictive window adjustment
                if *delta_ratio < -0.1 {
                    // Rapid drop - preemptively reduce window
                    let predicted_next = (state.tension * 1.2).min(1.0);
                    if predicted_next > 0.5 {
                        self.preemptively_reduce_window();
                    }
                }
            }

            CoherenceEvent::PartitionChanged { boundary_edges, .. } => {
                // Boundary edge spike may indicate attention should focus
                if *boundary_edges > 15 {
                    self.enable_sparse_attention_mode();
                }
            }

            _ => {}
        }
    }

    fn id(&self) -> &'static str { "attention" }
}
```

### 3.3 Self-Healing (RAC Adversarial Coherence) Integration

The RAC layer in edge-net can use coherence signals for conflict escalation:

```rust
/// RAC coherence listener for adversarial coherence coordination
pub struct RacCoherenceListener {
    engine: Arc<RwLock<CoherenceEngine>>,
    escalation_multiplier: f32,
}

impl CoherenceListener for RacCoherenceListener {
    fn on_coherence_update(&mut self, state: &CoherenceState, event: &CoherenceEvent) {
        match event {
            // High structural tension increases semantic scrutiny
            CoherenceEvent::TensionThreshold { threshold, direction } => {
                if *threshold >= 0.7 && matches!(direction, ThresholdDirection::Crossed) {
                    // Increase escalation sensitivity during instability
                    self.escalation_multiplier = 1.5;

                    // Tighten quarantine thresholds
                    let mut engine = self.engine.write().unwrap();
                    engine.set_witness_requirement(5); // Require more witnesses
                }
            }

            // During recovery, relax constraints gradually
            CoherenceEvent::RecoveryDetected { from_tier, to_tier } => {
                if *to_tier <= 1 {
                    self.escalation_multiplier = 1.0;
                }
            }

            _ => {}
        }
    }

    fn id(&self) -> &'static str { "rac" }
}
```

### 3.4 Edge-Net Learning Intelligence Integration

The NetworkLearning module can adapt based on coherence:

```rust
/// Edge-net learning coherence listener
pub struct EdgeNetLearningListener {
    learning: Arc<RwLock<NetworkLearning>>,
    spike_threshold_base: u16,
}

impl CoherenceListener for EdgeNetLearningListener {
    fn on_coherence_update(&mut self, state: &CoherenceState, event: &CoherenceEvent) {
        match event {
            // Adjust spike threshold based on tension (energy efficiency)
            CoherenceEvent::LambdaUpdate { .. } => {
                // Higher tension = more aggressive spike filtering (save energy)
                let adjusted_threshold = self.spike_threshold_base +
                    (state.tension * 8192.0) as u16;

                // This affects energy efficiency of spike-driven attention
                let mut learning = self.learning.write().unwrap();
                learning.set_spike_threshold(adjusted_threshold);
            }

            // Pattern pruning during sustained tension
            CoherenceEvent::TensionThreshold { threshold, direction } => {
                if *threshold >= 0.7 && matches!(direction, ThresholdDirection::Crossed) {
                    let learning = self.learning.read().unwrap();
                    // Prune low-confidence patterns to reduce memory pressure
                    learning.prune(2, 0.5);
                }
            }

            _ => {}
        }
    }

    fn id(&self) -> &'static str { "edge-learning" }
}
```

---

## 4. Performance Considerations

### 4.1 Event Bus Latency

The event bus should be non-blocking:

```rust
/// Lock-free event bus for hot path
pub struct LockFreeCoherenceBus {
    /// Current state (atomic)
    state: AtomicCoherenceState,

    /// Event channel (bounded, non-blocking)
    tx: crossbeam::channel::Sender<CoherenceEvent>,
    rx: crossbeam::channel::Receiver<CoherenceEvent>,

    /// Listener threads
    listener_handles: Vec<JoinHandle<()>>,
}

impl LockFreeCoherenceBus {
    /// Process gate packet without blocking
    #[inline]
    pub fn process_gate_packet_nonblocking(&self, packet: &GatePacket) -> bool {
        // Atomic state update
        let new_state = self.compute_state(packet);
        self.state.store(new_state);

        // Non-blocking event emit
        self.tx.try_send(CoherenceEvent::LambdaUpdate {
            old: 0, // simplified
            new: packet.lambda,
            delta_ratio: packet.drop_ratio_q15() as f32 / 32767.0,
        }).is_ok()
    }
}
```

### 4.2 Batching for High-Frequency Updates

```rust
/// Batched coherence updates for high-frequency MinCut recalculation
pub struct BatchedCoherenceBus {
    bus: CoherenceEventBus,
    batch: Vec<GatePacket>,
    batch_size: usize,
    last_emit: Instant,
    emit_interval: Duration,
}

impl BatchedCoherenceBus {
    pub fn enqueue(&mut self, packet: GatePacket) {
        self.batch.push(packet);

        if self.batch.len() >= self.batch_size ||
           self.last_emit.elapsed() >= self.emit_interval {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.batch.is_empty() { return; }

        // Use latest packet for state, but aggregate metrics
        let latest = self.batch.last().unwrap().clone();

        // Compute aggregate tension metrics
        let avg_lambda: u32 = self.batch.iter().map(|p| p.lambda).sum::<u32>() /
                              self.batch.len() as u32;
        let max_drop: u16 = self.batch.iter()
            .map(|p| p.drop_ratio_q15())
            .max()
            .unwrap_or(0);

        self.bus.process_gate_packet(&latest);
        self.batch.clear();
        self.last_emit = Instant::now();
    }
}
```

### 4.3 WASM Considerations

For browser deployment, use postMessage for cross-worker coordination:

```rust
#[cfg(target_arch = "wasm32")]
pub struct WasmCoherenceBridge {
    /// Web Worker port for event dispatch
    port: web_sys::MessagePort,
}

#[cfg(target_arch = "wasm32")]
impl WasmCoherenceBridge {
    pub fn emit_event(&self, event: &CoherenceEvent) -> Result<(), JsValue> {
        let json = serde_json::to_string(event).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.port.post_message(&JsValue::from_str(&json))
    }
}
```

---

## 5. Integration Code Snippets

### 5.1 Complete Bus Setup

```rust
/// Create and configure the coherence event bus
pub fn setup_coherence_bus(
    sona_coordinator: Arc<LoopCoordinator>,
    rac_engine: Arc<RwLock<CoherenceEngine>>,
    learning: Arc<RwLock<NetworkLearning>>,
) -> CoherenceEventBus {
    let mut bus = CoherenceEventBus::new(1000);

    // Register SONA listener
    bus.register(Box::new(SonaCoherenceListener {
        coordinator: sona_coordinator,
        base_learning_rate: 0.01,
    }));

    // Register RAC listener
    bus.register(Box::new(RacCoherenceListener {
        engine: rac_engine,
        escalation_multiplier: 1.0,
    }));

    // Register Edge-Net learning listener
    bus.register(Box::new(EdgeNetLearningListener {
        learning,
        spike_threshold_base: 16384,
    }));

    bus
}
```

### 5.2 MinCut Engine Integration

```rust
/// Integrate event bus with MinCut engine updates
impl DynamicMinCut {
    pub fn insert_edge_with_events(
        &mut self,
        u: u64,
        v: u64,
        weight: f64,
        bus: &mut CoherenceEventBus,
    ) -> Result<f64, MinCutError> {
        let old_cut = self.min_cut_value();

        let new_cut = self.insert_edge(u, v, weight)?;

        // Emit coherence event
        let packet = GatePacket {
            lambda: new_cut as u32,
            lambda_prev: old_cut as u32,
            boundary_edges: self.boundary_edge_count() as u16,
            boundary_concentration_q15: self.boundary_concentration_q15(),
            partition_count: self.partition_count() as u16,
            flags: 0,
        };

        bus.process_gate_packet(&packet);

        Ok(new_cut)
    }
}
```

### 5.3 Transformer Model Integration

```rust
/// Enhanced transformer with event bus integration
impl GatedTransformer {
    pub fn infer_with_events(
        &mut self,
        input: &InferInput,
        bus: &mut CoherenceEventBus,
    ) -> InferOutput {
        // Process gate packet through event bus first
        bus.process_gate_packet(&input.gate);

        // Get coherence state for additional context
        let state = bus.current_state();

        // Use state.recommended_tier to override if needed
        let effective_gate = if state.tension > 0.8 {
            // Critical tension - force safe mode
            GatePacket {
                flags: GatePacket::FLAG_FORCE_SAFE,
                ..input.gate.clone()
            }
        } else {
            input.gate.clone()
        };

        // Proceed with inference
        self.infer(&InferInput {
            gate: effective_gate,
            ..input.clone()
        })
    }
}
```

---

## 6. Conclusion and Recommendations

### 6.1 Key Insights

1. **GatePacket is the atomic coherence unit** - All subsystems should consume this structure
2. **0.7 tension threshold is critical** - Maps to lambda_min=30 and drop_ratio_q15_max=12288
3. **Tier system provides graceful degradation** - 0=normal, 1=degraded, 2=safe, 3=skip
4. **RAC adds semantic coherence** - Structural coherence (MinCut) + semantic coherence (RAC) = robust system

### 6.2 Implementation Priority

1. **Phase 1:** Implement `CoherenceEventBus` core with `GatePacket` processing
2. **Phase 2:** Add SONA listener for learning rate boost during recovery
3. **Phase 3:** Add RAC listener for escalation coordination
4. **Phase 4:** Add Edge-Net learning listener for energy optimization
5. **Phase 5:** Add performance optimizations (lock-free, batching)

### 6.3 Files to Create/Modify

| File | Purpose |
|------|---------|
| `crates/ruvector-coherence-bus/src/lib.rs` | New crate for event bus |
| `crates/ruvector-coherence-bus/src/listeners/mod.rs` | Listener trait and implementations |
| `crates/sona/src/coherence_listener.rs` | SONA integration |
| `crates/ruvector-mincut-gated-transformer/src/bus_integration.rs` | Transformer integration |
| `examples/edge-net/src/coherence/mod.rs` | Edge-net integration |

---

## Appendix A: Complete GatePacket Reference

```rust
// From /workspaces/ruvector/crates/ruvector-mincut-gated-transformer/src/packets.rs

impl GatePacket {
    pub const FLAG_FORCE_SAFE: u16 = 1 << 0;
    pub const FLAG_SKIP: u16 = 1 << 1;
    pub const FLAG_BOUNDARY_IDS_AVAILABLE: u16 = 1 << 2;

    pub fn force_safe(&self) -> bool;
    pub fn skip_requested(&self) -> bool;
    pub fn lambda_delta(&self) -> i32;
    pub fn drop_ratio_q15(&self) -> u16;
}
```

## Appendix B: Tension Calculation Reference

```rust
/// Normalized tension (0.0 = stable, 1.0 = critical)
pub fn compute_tension(packet: &GatePacket, policy: &GatePolicy) -> f32 {
    let lambda_factor = if packet.lambda < policy.lambda_min {
        1.0
    } else {
        1.0 - (packet.lambda as f32 / (policy.lambda_min * 3) as f32).min(1.0)
    };

    let drop_factor = (packet.drop_ratio_q15() as f32) / (policy.drop_ratio_q15_max as f32);
    let boundary_factor = (packet.boundary_concentration_q15 as f32) /
                          (policy.boundary_concentration_q15_max as f32);
    let partition_factor = (packet.partition_count as f32) / (policy.partitions_max as f32);

    // Weighted sum (drop is most critical signal)
    (0.4 * drop_factor.min(1.0) +
     0.3 * lambda_factor.min(1.0) +
     0.2 * boundary_factor.min(1.0) +
     0.1 * partition_factor.min(1.0))
    .clamp(0.0, 1.0)
}
```

---

*End of Research Document*
