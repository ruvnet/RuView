# ADR-QE-010: Observability & Monitoring Integration

**Status**: Proposed
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

---

## Context

ruVector provides comprehensive observability through the `ruvector-metrics` crate,
which aggregates telemetry from all subsystems into a unified monitoring dashboard.
The quantum simulation engine is a new subsystem that must participate in this
observability infrastructure.

Effective monitoring of quantum simulation is essential for:

1. **Performance tuning**: Identifying bottlenecks in gate application, memory
   allocation, and parallelization efficiency.
2. **Resource management**: Tracking memory consumption to prevent OOM conditions
   and to inform auto-scaling decisions.
3. **Debugging**: Tracing the execution of specific circuits to diagnose incorrect
   results or unexpected behavior.
4. **Capacity planning**: Understanding workload patterns (qubit counts, circuit
   depths, simulation frequency) to plan infrastructure.
5. **Compliance**: Auditable logs of simulation executions for regulated
   environments (cryptographic validation, safety-critical applications).

### WASM Constraint

In WebAssembly deployment, there is no direct filesystem access and no native
networking. Observability in WASM must use browser-compatible mechanisms:
`console.log`, `console.warn`, `console.error`, or JavaScript callback functions
registered by the host application.

### Existing Infrastructure

| Component | Role | Integration Point |
|---|---|---|
| `ruvector-metrics` | Metrics aggregation and export | Trait-based sink |
| `ruvector-monitor` | Real-time dashboard UI | WebSocket feed |
| Rust `tracing` crate | Structured logging and spans | Subscriber-based |
| Prometheus / OpenTelemetry | External monitoring | Exporter plugins |
| Ed25519 audit trail | Cryptographic logging | `ruqu-audit` crate |

## Decision

### 1. Metrics Schema

Every simulation execution emits a structured metrics record. The schema is
versioned to allow evolution without breaking consumers.

```rust
/// Metrics emitted after each quantum simulation execution.
/// Schema version: 1.0.0
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationMetrics {
    /// Schema version for forward compatibility.
    pub schema_version: &'static str,

    /// Unique identifier for this simulation run.
    pub simulation_id: Uuid,

    /// Timestamp when simulation started (UTC).
    pub started_at: DateTime<Utc>,

    /// Timestamp when simulation completed (UTC).
    pub completed_at: DateTime<Utc>,

    // -- Circuit characteristics --

    /// Number of qubits in the circuit.
    pub qubit_count: u32,

    /// Total number of gates (before optimization).
    pub gate_count_raw: u64,

    /// Total number of gates (after optimization/fusion).
    pub gate_count_optimized: u64,

    /// Circuit depth (longest path from input to output).
    pub circuit_depth: u32,

    /// Number of two-qubit gates (entangling operations).
    pub two_qubit_gate_count: u64,

    // -- Execution metrics --

    /// Total wall-clock execution time in milliseconds.
    pub execution_time_ms: f64,

    /// Time spent in gate application (excluding allocation, measurement).
    pub gate_application_time_ms: f64,

    /// Time spent in measurement sampling.
    pub measurement_time_ms: f64,

    /// Peak memory consumption in bytes during simulation.
    pub peak_memory_bytes: u64,

    /// Memory allocated for the state vector / tensor network.
    pub state_memory_bytes: u64,

    /// Backend used for this simulation.
    pub backend: BackendType,

    // -- Throughput --

    /// Gates applied per second (optimized gate count / gate application time).
    pub gates_per_second: f64,

    /// Qubits * depth per second (a normalized throughput metric).
    pub quantum_volume_rate: f64,

    // -- Optimization statistics --

    /// Number of gates eliminated by fusion.
    pub gates_fused: u64,

    /// Number of gates eliminated as identity or redundant.
    pub gates_skipped: u64,

    /// Number of gate commutations applied.
    pub gates_commuted: u64,

    // -- Entanglement analysis --

    /// Number of independent qubit subsets (entanglement groups).
    pub entanglement_groups: u32,

    /// Sizes of each entanglement group.
    pub entanglement_group_sizes: Vec<u32>,

    // -- Measurement outcomes (if measured) --

    /// Number of measurement shots executed.
    pub measurement_shots: Option<u64>,

    /// Distribution entropy of measurement outcomes (bits).
    pub outcome_entropy: Option<f64>,

    // -- MPS-specific (tensor network backend) --

    /// Maximum bond dimension reached (MPS mode only).
    pub max_bond_dimension: Option<u32>,

    /// Estimated fidelity after MPS truncation.
    pub mps_fidelity_estimate: Option<f64>,

    // -- Error information --

    /// Whether the simulation completed successfully.
    pub success: bool,

    /// Error message if simulation failed.
    pub error: Option<String>,

    /// Error category for programmatic handling.
    pub error_kind: Option<SimulationErrorKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendType {
    StateVector,
    TensorNetwork,
    Mps,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimulationErrorKind {
    QubitLimitExceeded,
    MemoryAllocationFailed,
    InvalidGateTarget,
    InvalidParameter,
    ContractionFailed,
    MpsFidelityBelowThreshold,
    Timeout,
    InternalError,
}
```

### 2. Metrics Sink Trait

The engine publishes metrics through a trait abstraction, allowing different sinks
for native and WASM environments:

```rust
/// Trait for consuming simulation metrics.
/// Implementations exist for native (ruvector-metrics), WASM (JS callback),
/// and testing (in-memory collector).
pub trait MetricsSink: Send + Sync {
    /// Publish a completed simulation's metrics.
    fn publish(&self, metrics: &SimulationMetrics);

    /// Publish an incremental progress update (for long-running simulations).
    fn progress(&self, simulation_id: Uuid, percent_complete: f32, message: &str);

    /// Publish a health status update.
    fn health(&self, status: EngineHealthStatus);
}

/// Native implementation: forwards to ruvector-metrics.
pub struct NativeMetricsSink {
    registry: Arc<ruvector_metrics::Registry>,
}

impl MetricsSink for NativeMetricsSink {
    fn publish(&self, metrics: &SimulationMetrics) {
        // Emit as histogram/counter/gauge values
        self.registry.histogram("ruqu.execution_time_ms")
            .record(metrics.execution_time_ms);
        self.registry.gauge("ruqu.peak_memory_bytes")
            .set(metrics.peak_memory_bytes as f64);
        self.registry.counter("ruqu.simulations_total")
            .increment(1);
        self.registry.counter("ruqu.gates_applied_total")
            .increment(metrics.gate_count_optimized);
        self.registry.histogram("ruqu.gates_per_second")
            .record(metrics.gates_per_second);

        if !metrics.success {
            self.registry.counter("ruqu.errors_total")
                .increment(1);
        }
    }

    fn progress(&self, _id: Uuid, percent: f32, _msg: &str) {
        self.registry.gauge("ruqu.current_progress")
            .set(percent as f64);
    }

    fn health(&self, status: EngineHealthStatus) {
        self.registry.gauge("ruqu.health_status")
            .set(status.as_numeric());
    }
}
```

### 3. WASM Metrics Sink

In WASM, metrics are delivered via JavaScript callbacks:

```rust
#[cfg(target_arch = "wasm32")]
pub struct WasmMetricsSink {
    /// JS callback function registered by host application.
    callback: js_sys::Function,
}

#[cfg(target_arch = "wasm32")]
impl MetricsSink for WasmMetricsSink {
    fn publish(&self, metrics: &SimulationMetrics) {
        let json = serde_json::to_string(metrics)
            .unwrap_or_else(|_| "{}".to_string());
        let js_value = JsValue::from_str(&json);
        let event_type = JsValue::from_str("simulation_complete");
        let _ = self.callback.call2(&JsValue::NULL, &event_type, &js_value);
    }

    fn progress(&self, id: Uuid, percent: f32, message: &str) {
        let payload = format!(
            r#"{{"simulation_id":"{}","percent":{},"message":"{}"}}"#,
            id, percent, message
        );
        let js_value = JsValue::from_str(&payload);
        let event_type = JsValue::from_str("simulation_progress");
        let _ = self.callback.call2(&JsValue::NULL, &event_type, &js_value);
    }

    fn health(&self, status: EngineHealthStatus) {
        let payload = format!(r#"{{"status":"{}"}}"#, status.as_str());
        let js_value = JsValue::from_str(&payload);
        let event_type = JsValue::from_str("engine_health");
        let _ = self.callback.call2(&JsValue::NULL, &event_type, &js_value);
    }
}
```

JavaScript host registration:

```javascript
// Host application registers the metrics callback
import init, { set_metrics_callback } from 'ruqu-wasm';

await init();

set_metrics_callback((eventType, data) => {
    const metrics = JSON.parse(data);
    switch (eventType) {
        case 'simulation_complete':
            console.log(`Simulation ${metrics.simulation_id} completed in ${metrics.execution_time_ms}ms`);
            dashboard.updateMetrics(metrics);
            break;
        case 'simulation_progress':
            progressBar.update(metrics.percent);
            break;
        case 'engine_health':
            healthIndicator.set(metrics.status);
            break;
    }
});
```

### 4. Tracing Integration

The engine integrates with the Rust `tracing` crate for structured logging and
distributed tracing.

#### Span Hierarchy

```
ruqu::simulation                          (root span for entire simulation)
  |
  +-- ruqu::circuit_validation            (validate circuit structure)
  |
  +-- ruqu::backend_selection             (automatic backend choice)
  |
  +-- ruqu::optimization                  (gate fusion, commutation, etc.)
  |     |
  |     +-- ruqu::optimization::fusion    (individual fusion passes)
  |     +-- ruqu::optimization::cancel    (gate cancellation)
  |
  +-- ruqu::state_init                    (allocate and initialize state)
  |
  +-- ruqu::gate_application              (apply all gates)
  |     |
  |     +-- ruqu::gate                    (individual gate -- DEBUG level only)
  |
  +-- ruqu::measurement                   (perform measurement sampling)
  |
  +-- ruqu::metrics_publish               (emit metrics to sink)
  |
  +-- ruqu::state_cleanup                 (deallocate state vector)
```

#### Instrumentation Code

```rust
use tracing::{info, warn, debug, trace, instrument, Span};

#[instrument(
    name = "ruqu::simulation",
    skip(circuit, config, metrics_sink),
    fields(
        qubit_count = circuit.num_qubits(),
        gate_count = circuit.gate_count(),
        simulation_id = %Uuid::new_v4(),
    )
)]
pub fn execute(
    circuit: &QuantumCircuit,
    shots: usize,
    config: &SimulationConfig,
    metrics_sink: &dyn MetricsSink,
) -> Result<SimulationResult, SimulationError> {
    info!(
        qubits = circuit.num_qubits(),
        gates = circuit.gate_count(),
        depth = circuit.depth(),
        shots = shots,
        "Starting quantum simulation"
    );

    // Validate
    let _validation_span = tracing::info_span!("ruqu::circuit_validation").entered();
    validate_circuit(circuit)?;
    drop(_validation_span);

    // Select backend
    let _backend_span = tracing::info_span!("ruqu::backend_selection").entered();
    let backend = select_backend(circuit, config);
    info!(backend = backend.name(), "Backend selected");
    drop(_backend_span);

    // Optimize
    let _opt_span = tracing::info_span!("ruqu::optimization").entered();
    let optimized = optimize_circuit(circuit, config)?;
    info!(
        original_gates = circuit.gate_count(),
        optimized_gates = optimized.gate_count(),
        gates_fused = circuit.gate_count() - optimized.gate_count(),
        "Circuit optimization complete"
    );
    drop(_opt_span);

    // Execute
    let result = backend.execute(&optimized, shots, config)?;

    // At DEBUG level, log per-gate details
    debug!(
        execution_time_ms = result.execution_time_ms,
        peak_memory = result.peak_memory_bytes,
        "Simulation execution complete"
    );

    // At TRACE level only for small circuits, log amplitude information
    if circuit.num_qubits() <= 10 {
        trace!(
            amplitudes = ?result.state_vector_snapshot(),
            "Final state vector (small circuit trace)"
        );
    }

    Ok(result)
}
```

### 5. Structured Error Reporting

All errors carry structured context for programmatic handling:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SimulationError {
    #[error("Qubit limit exceeded: requested {requested}, maximum {maximum}")]
    QubitLimitExceeded {
        requested: u32,
        maximum: u32,
        estimated_memory_bytes: u64,
        available_memory_bytes: u64,
    },

    #[error("Memory allocation failed for {requested_bytes} bytes")]
    MemoryAllocationFailed {
        requested_bytes: u64,
        qubit_count: u32,
        suggestion: &'static str,
    },

    #[error("Invalid gate target: qubit {qubit} in {qubit_count}-qubit circuit")]
    InvalidGateTarget {
        gate_name: String,
        qubit: u32,
        qubit_count: u32,
        gate_index: usize,
    },

    #[error("Invalid gate parameter: {parameter_name} = {value} ({reason})")]
    InvalidParameter {
        gate_name: String,
        parameter_name: String,
        value: f64,
        reason: &'static str,
    },

    #[error("Tensor contraction failed: {reason}")]
    ContractionFailed {
        reason: String,
        estimated_treewidth: usize,
        suggestion: &'static str,
    },

    #[error("MPS fidelity {fidelity:.6} below threshold {threshold:.6}")]
    MpsFidelityBelowThreshold {
        fidelity: f64,
        threshold: f64,
        max_bond_dimension: usize,
        suggestion: &'static str,
    },

    #[error("Simulation timed out after {elapsed_ms}ms (limit: {timeout_ms}ms)")]
    Timeout {
        elapsed_ms: u64,
        timeout_ms: u64,
        gates_completed: u64,
        gates_remaining: u64,
    },

    #[error("Internal error: {message}")]
    InternalError {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}
```

Each error variant includes a `suggestion` field where applicable, guiding users
toward resolution:

| Error | Suggestion |
|---|---|
| QubitLimitExceeded | "Reduce qubit count or enable tensor-network feature for large circuits" |
| MemoryAllocationFailed | "Try tensor-network backend or reduce qubit count by 1-2 (halves/quarters memory)" |
| ContractionFailed | "Circuit treewidth too high for tensor network; use state vector for <= 30 qubits" |
| MpsFidelityBelowThreshold | "Increase chi_max or switch to exact state vector for high-fidelity results" |

### 6. Health Checks

The engine exposes health status for monitoring systems:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineHealthStatus {
    /// Whether the engine is ready to accept simulations.
    pub ready: bool,

    /// Maximum qubits supportable given current available memory.
    pub max_supported_qubits: u32,

    /// Available memory in bytes.
    pub available_memory_bytes: u64,

    /// Number of CPU cores available for parallel gate application.
    pub available_cores: usize,

    /// Whether the tensor-network backend is compiled in.
    pub tensor_network_available: bool,

    /// Current engine version.
    pub version: &'static str,

    /// Uptime since engine initialization (if applicable).
    pub uptime_seconds: Option<f64>,

    /// Number of simulations executed in current session.
    pub simulations_executed: u64,

    /// Total gates applied across all simulations in current session.
    pub total_gates_applied: u64,
}

/// Check engine health. Callable at any time.
pub fn quantum_engine_ready() -> EngineHealthStatus {
    let available_memory = estimate_available_memory();
    let max_qubits = compute_max_qubits(available_memory);

    EngineHealthStatus {
        ready: max_qubits >= 4,  // Minimum useful simulation
        max_supported_qubits: max_qubits,
        available_memory_bytes: available_memory,
        available_cores: rayon::current_num_threads(),
        tensor_network_available: cfg!(feature = "tensor-network"),
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: None,  // Library mode; no persistent uptime
        simulations_executed: SESSION_COUNTER.load(Ordering::Relaxed),
        total_gates_applied: SESSION_GATES.load(Ordering::Relaxed),
    }
}
```

### 7. Logging Levels

| Level | Content | Audience | Performance Impact |
|---|---|---|---|
| ERROR | Simulation failures, OOM, invalid circuits | Operators, alerting | None |
| WARN | Approaching memory limits (>80%), MPS fidelity degradation, slow contraction | Operators | Negligible |
| INFO | Simulation start/end summaries, backend selection, optimization results | Developers, dashboards | Negligible |
| DEBUG | Per-optimization-pass details, memory allocation sizes, thread utilization | Developers debugging | Low |
| TRACE | Per-gate amplitude changes (small circuits only, n <= 10), SVD singular values | Deep debugging | High (small circuits only) |

TRACE level is gated on circuit size to prevent catastrophic log volume:

```rust
// TRACE-level amplitude logging is only emitted for circuits with <= 10 qubits.
// For larger circuits, TRACE only emits gate-level timing without amplitude data.
if tracing::enabled!(tracing::Level::TRACE) {
    if circuit.num_qubits() <= 10 {
        trace!(amplitudes = ?state.as_slice(), "Post-gate state");
    } else {
        trace!(gate_time_ns = elapsed.as_nanos(), "Gate applied");
    }
}
```

### 8. Dashboard Integration

Metrics from the quantum engine appear in the ruVector monitoring UI as a dedicated
panel alongside vector operations, index health, and system resources.

```
+------------------------------------------------------------------+
|                    ruVector Monitoring Dashboard                   |
+------------------------------------------------------------------+
|                                                                    |
|  Vector Operations          |  Quantum Simulations                |
|  -------------------        |  -----------------------            |
|  Queries/sec: 12,450        |  Simulations/min: 23                |
|  P99 latency: 2.3ms         |  Avg execution: 145ms               |
|  Index size: 2.1M vectors   |  Avg qubits: 18.4                  |
|                              |  Peak memory: 4.2 GiB              |
|                              |  Backend: SV 87% / TN 13%         |
|                              |  Gates/sec: 2.1B                   |
|                              |  Error rate: 0.02%                 |
|                              |                                    |
|  System Resources           |  Recent Simulations                |
|  -------------------        |  -----------------------            |
|  CPU: 34%                   |  #a3f2.. 24q  230ms  OK           |
|  Memory: 61% (49/80 GiB)   |  #b891.. 16q   12ms  OK           |
|  Threads: 64/256 active     |  #c4d0.. 30q 1.2s   OK           |
|                              |  #d122.. 35q  ERR   OOM          |
+------------------------------------------------------------------+
```

Metrics are published via the existing `ruvector-metrics` WebSocket feed:

```json
{
    "source": "ruqu",
    "type": "simulation_complete",
    "timestamp": "2026-02-06T14:23:01.442Z",
    "data": {
        "simulation_id": "a3f2e891-...",
        "qubit_count": 24,
        "execution_time_ms": 230.4,
        "peak_memory_bytes": 268435456,
        "backend": "StateVector",
        "gates_per_second": 2147483648,
        "success": true
    }
}
```

### 9. Prometheus / OpenTelemetry Export

For external monitoring, the native metrics sink exports standard Prometheus
metrics:

```
# HELP ruqu_simulations_total Total quantum simulations executed
# TYPE ruqu_simulations_total counter
ruqu_simulations_total{backend="state_vector",status="success"} 1847
ruqu_simulations_total{backend="state_vector",status="error"} 3
ruqu_simulations_total{backend="tensor_network",status="success"} 241

# HELP ruqu_execution_time_ms Simulation execution time histogram
# TYPE ruqu_execution_time_ms histogram
ruqu_execution_time_ms_bucket{backend="state_vector",le="10"} 423
ruqu_execution_time_ms_bucket{backend="state_vector",le="100"} 1201
ruqu_execution_time_ms_bucket{backend="state_vector",le="1000"} 1834
ruqu_execution_time_ms_bucket{backend="state_vector",le="+Inf"} 1847

# HELP ruqu_peak_memory_bytes Peak memory during simulation
# TYPE ruqu_peak_memory_bytes gauge
ruqu_peak_memory_bytes 4294967296

# HELP ruqu_gates_per_second Gate application throughput
# TYPE ruqu_gates_per_second gauge
ruqu_gates_per_second 2.1e9

# HELP ruqu_max_supported_qubits Maximum qubits based on available memory
# TYPE ruqu_max_supported_qubits gauge
ruqu_max_supported_qubits 33
```

## Consequences

### Positive

1. **Unified observability**: Quantum simulation telemetry integrates seamlessly
   with ruVector's existing monitoring infrastructure.
2. **Cross-platform**: The trait-based sink design supports native, WASM, and
   testing environments without code changes in the engine.
3. **Actionable errors**: Structured errors with suggestions reduce debugging time
   and improve developer experience.
4. **Performance visibility**: Gates-per-second, memory consumption, and backend
   selection metrics enable informed performance tuning.
5. **Compliance ready**: Structured logging with simulation IDs supports audit
   trail requirements.

### Negative

1. **Metric cardinality**: High-frequency simulations could generate significant
   metric volume. Mitigated by aggregation at the sink level.
2. **WASM callback overhead**: JSON serialization for WASM metrics adds ~0.1ms per
   simulation. Acceptable for typical workloads.
3. **Tracing overhead at DEBUG/TRACE**: Enabled tracing at low levels adds
   measurable overhead. Production deployments should use INFO or above.
4. **Schema evolution**: Changes to `SimulationMetrics` require versioned handling
   in consumers.

### Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Metric volume overwhelming storage | Configurable sampling rate; aggregate in sink |
| WASM callback exceptions | Catch JS exceptions in callback wrapper; log to console |
| Schema breaking changes | Version field in metrics; consumer-side version dispatch |
| TRACE logging for large circuits | Qubit-count gate prevents amplitude logging above n=10 |

## References

- `ruvector-metrics` crate: internal metrics infrastructure
- Rust `tracing` crate: https://docs.rs/tracing
- OpenTelemetry Rust SDK: https://docs.rs/opentelemetry
- ADR-QE-005: WASM Compilation Target (WASM constraints)
- ADR-QE-011: Memory Gating & Power Management (resource monitoring)
- Prometheus exposition format: https://prometheus.io/docs/instrumenting/exposition_formats/
