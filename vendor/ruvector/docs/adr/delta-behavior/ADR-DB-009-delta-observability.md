# ADR-DB-009: Delta Observability

**Status**: Proposed
**Date**: 2026-01-28
**Authors**: RuVector Architecture Team
**Deciders**: Architecture Review Board
**Parent**: ADR-DB-001 Delta Behavior Core Architecture

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-28 | Architecture Team | Initial proposal |

---

## Context and Problem Statement

### The Observability Challenge

Delta-first architecture introduces new debugging and monitoring needs:

1. **Delta Lineage**: Understanding where a vector's current state came from
2. **Performance Tracing**: Identifying bottlenecks in delta pipelines
3. **Anomaly Detection**: Spotting unusual delta patterns
4. **Debugging**: Reconstructing state at any point in time
5. **Auditing**: Compliance requirements for tracking changes

### Observability Pillars

| Pillar | Delta-Specific Need |
|--------|---------------------|
| Metrics | Delta rates, composition times, compression ratios |
| Tracing | Delta propagation paths, end-to-end latency |
| Logging | Delta events, conflicts, compactions |
| Lineage | Delta chains, causal dependencies |

---

## Decision

### Adopt Delta Lineage Tracking with OpenTelemetry Integration

We implement comprehensive delta observability with lineage tracking as a first-class feature.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         OBSERVABILITY LAYER                                  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
        ┌───────────────────────────┼───────────────────────────────┐
        │                           │                               │
        v                           v                               v
┌───────────────┐           ┌───────────────┐               ┌───────────────┐
│    METRICS    │           │    TRACING    │               │   LINEAGE     │
│               │           │               │               │               │
│ - Delta rates │           │ - Propagation │               │ - Delta chains│
│ - Latencies   │           │ - Conflicts   │               │ - Causal DAG  │
│ - Compression │           │ - Compaction  │               │ - Snapshots   │
│ - Queue depths│           │ - Searches    │               │ - Provenance  │
└───────────────┘           └───────────────┘               └───────────────┘
        │                           │                               │
        v                           v                               v
┌─────────────────────────────────────────────────────────────────────────────┐
│                          OPENTELEMETRY EXPORTER                              │
│    Prometheus │ Jaeger │ OTLP │ Custom Lineage Store                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Core Components

#### 1. Delta Lineage Tracker

```rust
/// Tracks delta lineage and causal relationships
pub struct DeltaLineageTracker {
    /// Delta dependency graph
    dag: DeltaDAG,
    /// Vector state snapshots
    snapshots: SnapshotStore,
    /// Lineage query interface
    query: LineageQuery,
    /// Configuration
    config: LineageConfig,
}

/// Directed Acyclic Graph of delta dependencies
pub struct DeltaDAG {
    /// Nodes: delta IDs
    nodes: DashMap<DeltaId, DeltaNode>,
    /// Edges: causal dependencies
    edges: DashMap<(DeltaId, DeltaId), EdgeMetadata>,
    /// Index by vector ID
    by_vector: DashMap<VectorId, Vec<DeltaId>>,
    /// Index by timestamp
    by_time: BTreeMap<DateTime<Utc>, Vec<DeltaId>>,
}

#[derive(Debug, Clone)]
pub struct DeltaNode {
    /// Delta identifier
    pub delta_id: DeltaId,
    /// Target vector
    pub vector_id: VectorId,
    /// Operation type
    pub operation_type: OperationType,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Source replica
    pub origin: ReplicaId,
    /// Parent delta (if any)
    pub parent: Option<DeltaId>,
    /// Trace context
    pub trace_context: Option<TraceContext>,
    /// Additional metadata
    pub metadata: HashMap<String, Value>,
}

impl DeltaLineageTracker {
    /// Record a new delta in the lineage
    pub fn record_delta(&self, delta: &VectorDelta, context: &DeltaContext) {
        let node = DeltaNode {
            delta_id: delta.delta_id.clone(),
            vector_id: delta.vector_id.clone(),
            operation_type: delta.operation.operation_type(),
            created_at: delta.timestamp,
            origin: delta.origin_replica.clone(),
            parent: delta.parent_delta.clone(),
            trace_context: context.trace_context.clone(),
            metadata: context.metadata.clone(),
        };

        // Insert node
        self.dag.nodes.insert(delta.delta_id.clone(), node);

        // Add edge to parent
        if let Some(parent) = &delta.parent_delta {
            self.dag.edges.insert(
                (parent.clone(), delta.delta_id.clone()),
                EdgeMetadata {
                    edge_type: EdgeType::CausalDependency,
                    created_at: Utc::now(),
                },
            );
        }

        // Update indexes
        self.dag.by_vector
            .entry(delta.vector_id.clone())
            .or_default()
            .push(delta.delta_id.clone());

        self.dag.by_time
            .entry(delta.timestamp)
            .or_default()
            .push(delta.delta_id.clone());
    }

    /// Get lineage for a vector
    pub fn get_lineage(&self, vector_id: &VectorId) -> DeltaLineage {
        let delta_ids = self.dag.by_vector.get(vector_id)
            .map(|v| v.clone())
            .unwrap_or_default();

        let nodes: Vec<_> = delta_ids.iter()
            .filter_map(|id| self.dag.nodes.get(id).map(|n| n.clone()))
            .collect();

        DeltaLineage {
            vector_id: vector_id.clone(),
            deltas: nodes,
            chain_length: delta_ids.len(),
        }
    }

    /// Get causal ancestors of a delta
    pub fn get_ancestors(&self, delta_id: &DeltaId) -> Vec<DeltaId> {
        let mut ancestors = Vec::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        queue.push_back(delta_id.clone());

        while let Some(current) = queue.pop_front() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            if let Some(node) = self.dag.nodes.get(&current) {
                if let Some(parent) = &node.parent {
                    ancestors.push(parent.clone());
                    queue.push_back(parent.clone());
                }
            }
        }

        ancestors
    }

    /// Find common ancestor of two deltas
    pub fn find_common_ancestor(&self, a: &DeltaId, b: &DeltaId) -> Option<DeltaId> {
        let ancestors_a: HashSet<_> = self.get_ancestors(a).into_iter().collect();

        for ancestor in self.get_ancestors(b) {
            if ancestors_a.contains(&ancestor) {
                return Some(ancestor);
            }
        }

        None
    }
}
```

#### 2. Metrics Collector

```rust
use opentelemetry::metrics::{Counter, Histogram, Meter};

/// Delta-specific metrics
pub struct DeltaMetrics {
    /// Delta application counter
    deltas_applied: Counter<u64>,
    /// Delta application latency
    apply_latency: Histogram<f64>,
    /// Composition latency
    compose_latency: Histogram<f64>,
    /// Compression ratio
    compression_ratio: Histogram<f64>,
    /// Delta chain length
    chain_length: Histogram<f64>,
    /// Conflict counter
    conflicts: Counter<u64>,
    /// Queue depth gauge
    queue_depth: ObservableGauge<u64>,
    /// Checkpoint counter
    checkpoints: Counter<u64>,
    /// Compaction counter
    compactions: Counter<u64>,
}

impl DeltaMetrics {
    pub fn new(meter: &Meter) -> Self {
        Self {
            deltas_applied: meter
                .u64_counter("ruvector.delta.applied")
                .with_description("Number of deltas applied")
                .init(),

            apply_latency: meter
                .f64_histogram("ruvector.delta.apply_latency")
                .with_description("Delta application latency in milliseconds")
                .with_unit(Unit::new("ms"))
                .init(),

            compose_latency: meter
                .f64_histogram("ruvector.delta.compose_latency")
                .with_description("Vector composition latency")
                .with_unit(Unit::new("ms"))
                .init(),

            compression_ratio: meter
                .f64_histogram("ruvector.delta.compression_ratio")
                .with_description("Compression ratio achieved")
                .init(),

            chain_length: meter
                .f64_histogram("ruvector.delta.chain_length")
                .with_description("Delta chain length at composition")
                .init(),

            conflicts: meter
                .u64_counter("ruvector.delta.conflicts")
                .with_description("Number of delta conflicts detected")
                .init(),

            queue_depth: meter
                .u64_observable_gauge("ruvector.delta.queue_depth")
                .with_description("Current depth of delta queue")
                .init(),

            checkpoints: meter
                .u64_counter("ruvector.delta.checkpoints")
                .with_description("Number of checkpoints created")
                .init(),

            compactions: meter
                .u64_counter("ruvector.delta.compactions")
                .with_description("Number of compactions performed")
                .init(),
        }
    }

    /// Record delta application
    pub fn record_delta_applied(
        &self,
        operation_type: &str,
        vector_id: &str,
        latency_ms: f64,
    ) {
        let attributes = [
            KeyValue::new("operation_type", operation_type.to_string()),
        ];

        self.deltas_applied.add(1, &attributes);
        self.apply_latency.record(latency_ms, &attributes);
    }

    /// Record vector composition
    pub fn record_composition(
        &self,
        chain_length: usize,
        latency_ms: f64,
    ) {
        self.chain_length.record(chain_length as f64, &[]);
        self.compose_latency.record(latency_ms, &[]);
    }

    /// Record conflict
    pub fn record_conflict(&self, resolution_strategy: &str) {
        self.conflicts.add(1, &[
            KeyValue::new("strategy", resolution_strategy.to_string()),
        ]);
    }
}
```

#### 3. Distributed Tracing

```rust
use opentelemetry::trace::{Tracer, Span, SpanKind};

/// Delta operation tracing
pub struct DeltaTracer {
    tracer: Arc<dyn Tracer + Send + Sync>,
}

impl DeltaTracer {
    /// Start a trace span for delta application
    pub fn trace_apply_delta(&self, delta: &VectorDelta) -> impl Span {
        let span = self.tracer.span_builder("delta.apply")
            .with_kind(SpanKind::Internal)
            .with_attributes(vec![
                KeyValue::new("delta.id", delta.delta_id.to_string()),
                KeyValue::new("delta.vector_id", delta.vector_id.to_string()),
                KeyValue::new("delta.operation", delta.operation.type_name()),
            ])
            .start(&self.tracer);

        span
    }

    /// Trace delta propagation
    pub fn trace_propagation(&self, delta: &VectorDelta, target: &str) -> impl Span {
        self.tracer.span_builder("delta.propagate")
            .with_kind(SpanKind::Producer)
            .with_attributes(vec![
                KeyValue::new("delta.id", delta.delta_id.to_string()),
                KeyValue::new("target", target.to_string()),
            ])
            .start(&self.tracer)
    }

    /// Trace conflict resolution
    pub fn trace_conflict_resolution(
        &self,
        delta_a: &DeltaId,
        delta_b: &DeltaId,
        strategy: &str,
    ) -> impl Span {
        self.tracer.span_builder("delta.conflict.resolve")
            .with_kind(SpanKind::Internal)
            .with_attributes(vec![
                KeyValue::new("delta.a", delta_a.to_string()),
                KeyValue::new("delta.b", delta_b.to_string()),
                KeyValue::new("strategy", strategy.to_string()),
            ])
            .start(&self.tracer)
    }

    /// Trace vector composition
    pub fn trace_composition(
        &self,
        vector_id: &VectorId,
        chain_length: usize,
    ) -> impl Span {
        self.tracer.span_builder("delta.compose")
            .with_kind(SpanKind::Internal)
            .with_attributes(vec![
                KeyValue::new("vector.id", vector_id.to_string()),
                KeyValue::new("chain.length", chain_length as i64),
            ])
            .start(&self.tracer)
    }
}

/// Trace context for cross-process propagation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub trace_flags: u8,
    pub trace_state: Option<String>,
}

impl TraceContext {
    /// Extract from W3C Trace Context header
    pub fn from_traceparent(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() != 4 {
            return None;
        }

        Some(Self {
            trace_id: parts[1].to_string(),
            span_id: parts[2].to_string(),
            trace_flags: u8::from_str_radix(parts[3], 16).ok()?,
            trace_state: None,
        })
    }

    /// Convert to W3C Trace Context header
    pub fn to_traceparent(&self) -> String {
        format!(
            "00-{}-{}-{:02x}",
            self.trace_id, self.span_id, self.trace_flags
        )
    }
}
```

#### 4. Event Logging

```rust
use tracing::{info, warn, error, debug, instrument};

/// Delta event logger with structured logging
pub struct DeltaEventLogger {
    /// Log level configuration
    config: LogConfig,
}

impl DeltaEventLogger {
    /// Log delta application
    #[instrument(
        name = "delta_applied",
        skip(self, delta),
        fields(
            delta.id = %delta.delta_id,
            delta.vector_id = %delta.vector_id,
            delta.operation = %delta.operation.type_name(),
        )
    )]
    pub fn log_delta_applied(&self, delta: &VectorDelta, latency: Duration) {
        info!(
            latency_us = latency.as_micros() as u64,
            "Delta applied successfully"
        );
    }

    /// Log conflict detection
    #[instrument(
        name = "delta_conflict",
        skip(self),
        fields(
            delta.a = %delta_a,
            delta.b = %delta_b,
        )
    )]
    pub fn log_conflict(
        &self,
        delta_a: &DeltaId,
        delta_b: &DeltaId,
        resolution: &str,
    ) {
        warn!(
            resolution = resolution,
            "Delta conflict detected and resolved"
        );
    }

    /// Log compaction event
    #[instrument(
        name = "delta_compaction",
        skip(self),
        fields(
            vector.id = %vector_id,
        )
    )]
    pub fn log_compaction(
        &self,
        vector_id: &VectorId,
        deltas_merged: usize,
        space_saved: usize,
    ) {
        info!(
            deltas_merged = deltas_merged,
            space_saved_bytes = space_saved,
            "Delta chain compacted"
        );
    }

    /// Log checkpoint creation
    #[instrument(
        name = "delta_checkpoint",
        skip(self),
        fields(
            vector.id = %vector_id,
        )
    )]
    pub fn log_checkpoint(
        &self,
        vector_id: &VectorId,
        at_delta: &DeltaId,
    ) {
        debug!(
            at_delta = %at_delta,
            "Checkpoint created"
        );
    }

    /// Log propagation event
    #[instrument(
        name = "delta_propagation",
        skip(self),
        fields(
            delta.id = %delta_id,
            target = %target,
        )
    )]
    pub fn log_propagation(&self, delta_id: &DeltaId, target: &str, success: bool) {
        if success {
            debug!("Delta propagated successfully");
        } else {
            error!("Delta propagation failed");
        }
    }
}
```

### Lineage Query API

```rust
/// Query interface for delta lineage
pub struct LineageQuery {
    tracker: Arc<DeltaLineageTracker>,
}

impl LineageQuery {
    /// Reconstruct vector at specific time
    pub fn vector_at_time(
        &self,
        vector_id: &VectorId,
        timestamp: DateTime<Utc>,
    ) -> Result<Vec<f32>> {
        let lineage = self.tracker.get_lineage(vector_id);

        // Filter deltas before timestamp
        let relevant_deltas: Vec<_> = lineage.deltas
            .into_iter()
            .filter(|d| d.created_at <= timestamp)
            .collect();

        // Compose from filtered deltas
        self.compose_from_deltas(&relevant_deltas)
    }

    /// Get all changes to a vector in time range
    pub fn changes_in_range(
        &self,
        vector_id: &VectorId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<DeltaNode> {
        let lineage = self.tracker.get_lineage(vector_id);

        lineage.deltas
            .into_iter()
            .filter(|d| d.created_at >= start && d.created_at <= end)
            .collect()
    }

    /// Diff between two points in time
    pub fn diff(
        &self,
        vector_id: &VectorId,
        time_a: DateTime<Utc>,
        time_b: DateTime<Utc>,
    ) -> Result<VectorDiff> {
        let vector_a = self.vector_at_time(vector_id, time_a)?;
        let vector_b = self.vector_at_time(vector_id, time_b)?;

        let changes: Vec<_> = vector_a.iter()
            .zip(vector_b.iter())
            .enumerate()
            .filter(|(_, (a, b))| (a - b).abs() > 1e-7)
            .map(|(i, (a, b))| DimensionChange {
                index: i,
                from: *a,
                to: *b,
            })
            .collect();

        Ok(VectorDiff {
            vector_id: vector_id.clone(),
            from_time: time_a,
            to_time: time_b,
            changes,
            l2_distance: euclidean_distance(&vector_a, &vector_b),
        })
    }

    /// Find which delta caused a dimension change
    pub fn blame(
        &self,
        vector_id: &VectorId,
        dimension: usize,
    ) -> Option<DeltaNode> {
        let lineage = self.tracker.get_lineage(vector_id);

        // Find last delta that modified this dimension
        lineage.deltas
            .into_iter()
            .rev()
            .find(|d| self.delta_affects_dimension(d, dimension))
    }
}
```

---

## Tracing and Metrics Reference

### Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `ruvector.delta.applied` | Counter | Total deltas applied |
| `ruvector.delta.apply_latency` | Histogram | Apply latency (ms) |
| `ruvector.delta.compose_latency` | Histogram | Composition latency (ms) |
| `ruvector.delta.compression_ratio` | Histogram | Compression ratio |
| `ruvector.delta.chain_length` | Histogram | Chain length at composition |
| `ruvector.delta.conflicts` | Counter | Conflicts detected |
| `ruvector.delta.queue_depth` | Gauge | Queue depth |
| `ruvector.delta.checkpoints` | Counter | Checkpoints created |
| `ruvector.delta.compactions` | Counter | Compactions performed |

### Span Names

| Span | Kind | Description |
|------|------|-------------|
| `delta.apply` | Internal | Delta application |
| `delta.propagate` | Producer | Delta propagation |
| `delta.conflict.resolve` | Internal | Conflict resolution |
| `delta.compose` | Internal | Vector composition |
| `delta.checkpoint` | Internal | Checkpoint creation |
| `delta.compact` | Internal | Chain compaction |
| `delta.search` | Internal | Search with delta awareness |

---

## Considered Options

### Option 1: Minimal Logging

**Description**: Basic log statements only.

**Pros**:
- Simple
- Low overhead

**Cons**:
- Poor debugging
- No lineage
- No distributed tracing

**Verdict**: Rejected - insufficient for production.

### Option 2: Custom Observability Stack

**Description**: Build custom metrics and tracing.

**Pros**:
- Full control
- Optimized for deltas

**Cons**:
- Maintenance burden
- No ecosystem integration
- Reinventing wheel

**Verdict**: Rejected - OpenTelemetry provides better value.

### Option 3: OpenTelemetry Integration (Selected)

**Description**: Full OpenTelemetry integration with delta-specific lineage.

**Pros**:
- Industry standard
- Ecosystem integration
- Flexible exporters
- Future-proof

**Cons**:
- Some overhead
- Learning curve

**Verdict**: Adopted - standard with delta-specific extensions.

---

## Technical Specification

### Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Enable metrics collection
    pub metrics_enabled: bool,
    /// Enable distributed tracing
    pub tracing_enabled: bool,
    /// Enable lineage tracking
    pub lineage_enabled: bool,
    /// Lineage retention period
    pub lineage_retention: Duration,
    /// Sampling rate for tracing (0.0 to 1.0)
    pub trace_sampling_rate: f32,
    /// OTLP endpoint for export
    pub otlp_endpoint: Option<String>,
    /// Prometheus endpoint
    pub prometheus_port: Option<u16>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            tracing_enabled: true,
            lineage_enabled: true,
            lineage_retention: Duration::from_secs(86400 * 7), // 7 days
            trace_sampling_rate: 0.1, // 10%
            otlp_endpoint: None,
            prometheus_port: Some(9090),
        }
    }
}
```

---

## Consequences

### Benefits

1. **Debugging**: Full delta history and lineage
2. **Performance Analysis**: Detailed latency metrics
3. **Compliance**: Audit trail for all changes
4. **Integration**: Works with existing observability tools
5. **Temporal Queries**: Reconstruct state at any time

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Performance overhead | Medium | Medium | Sampling, async export |
| Storage growth | Medium | Medium | Retention policies |
| Complexity | Medium | Low | Configuration presets |

---

## References

1. OpenTelemetry Specification. https://opentelemetry.io/docs/specs/
2. W3C Trace Context. https://www.w3.org/TR/trace-context/
3. ADR-DB-001: Delta Behavior Core Architecture

---

## Related Decisions

- **ADR-DB-001**: Delta Behavior Core Architecture
- **ADR-DB-003**: Delta Propagation Protocol
- **ADR-DB-010**: Delta Security Model
