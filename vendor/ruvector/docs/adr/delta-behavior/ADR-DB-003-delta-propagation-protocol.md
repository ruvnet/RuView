# ADR-DB-003: Delta Propagation Protocol

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

### The Propagation Challenge

Delta-first architecture requires efficient distribution of deltas across the system:

1. **Storage Layer**: Persist to durable storage
2. **Index Layer**: Update search indexes
3. **Cache Layer**: Invalidate/update caches
4. **Replication Layer**: Sync to replicas
5. **Client Layer**: Notify subscribers

The propagation protocol must balance:
- **Latency**: Fast delivery to all consumers
- **Ordering**: Preserve causal relationships
- **Reliability**: No delta loss
- **Backpressure**: Handle slow consumers

### Propagation Patterns

| Pattern | Use Case | Challenge |
|---------|----------|-----------|
| Single writer | Local updates | Simple, no conflicts |
| Multi-writer | Distributed updates | Ordering, conflicts |
| High throughput | Batch updates | Backpressure, batching |
| Low latency | Real-time search | Immediate propagation |
| Geo-distributed | Multi-region | Network partitions |

---

## Decision

### Adopt Reactive Push with Backpressure

We implement a reactive push protocol with causal ordering and adaptive backpressure.

### Architecture Overview

```
                    ┌─────────────────────────────────────────────────────────────┐
                    │                     DELTA SOURCES                            │
                    │   Local Writer │ Remote Replica │ Import │ Transform        │
                    └─────────────────────────────┬───────────────────────────────┘
                                                  │
                                                  v
                    ┌─────────────────────────────────────────────────────────────┐
                    │                   DELTA INGEST QUEUE                         │
                    │         (bounded, backpressure-aware, deduplication)         │
                    └─────────────────────────────┬───────────────────────────────┘
                                                  │
                                                  v
                    ┌─────────────────────────────────────────────────────────────┐
                    │                   CAUSAL ORDERING                            │
                    │      (vector clocks, dependency resolution, buffering)       │
                    └─────────────────────────────┬───────────────────────────────┘
                                                  │
                                                  v
                    ┌─────────────────────────────────────────────────────────────┐
                    │                   PROPAGATION ROUTER                         │
                    │        (topic-based routing, priority queues, filtering)     │
                    └────┬────────────┬────────────┬────────────┬─────────────────┘
                         │            │            │            │
                         v            v            v            v
                    ┌────────┐  ┌────────┐  ┌────────┐  ┌────────────┐
                    │Storage │  │ Index  │  │ Cache  │  │Replication │
                    │Sinks   │  │ Sinks  │  │ Sinks  │  │  Sinks     │
                    └────────┘  └────────┘  └────────┘  └────────────┘
```

### Core Components

#### 1. Delta Ingest Queue

```rust
/// Bounded, backpressure-aware delta ingest queue
pub struct DeltaIngestQueue {
    /// Bounded queue with configurable capacity
    queue: ArrayQueue<IngestDelta>,
    /// Capacity for backpressure signaling
    capacity: usize,
    /// High water mark for warning
    high_water_mark: usize,
    /// Deduplication bloom filter
    dedup_filter: BloomFilter<DeltaId>,
    /// Metrics
    metrics: IngestMetrics,
}

pub struct IngestDelta {
    pub delta: VectorDelta,
    pub source: DeltaSource,
    pub received_at: Instant,
    pub priority: Priority,
}

#[derive(Debug, Clone, Copy)]
pub enum Priority {
    Critical = 0,  // User-facing writes
    High = 1,      // Replication
    Normal = 2,    // Batch imports
    Low = 3,       // Background tasks
}

impl DeltaIngestQueue {
    /// Attempt to enqueue delta with backpressure
    pub fn try_enqueue(&self, delta: IngestDelta) -> Result<(), BackpressureError> {
        // Check deduplication
        if self.dedup_filter.contains(&delta.delta.delta_id) {
            return Err(BackpressureError::Duplicate);
        }

        // Check capacity
        let current = self.queue.len();
        if current >= self.capacity {
            self.metrics.record_rejection();
            return Err(BackpressureError::QueueFull {
                current,
                capacity: self.capacity,
            });
        }

        // Enqueue with priority sorting
        self.queue.push(delta).map_err(|_| BackpressureError::QueueFull {
            current,
            capacity: self.capacity,
        })?;

        // Track for deduplication
        self.dedup_filter.insert(&delta.delta.delta_id);

        // Emit high water mark warning
        if current > self.high_water_mark {
            self.metrics.record_high_water_mark(current);
        }

        Ok(())
    }

    /// Blocking enqueue with timeout
    pub async fn enqueue_timeout(
        &self,
        delta: IngestDelta,
        timeout: Duration,
    ) -> Result<(), BackpressureError> {
        let deadline = Instant::now() + timeout;

        loop {
            match self.try_enqueue(delta.clone()) {
                Ok(()) => return Ok(()),
                Err(BackpressureError::QueueFull { .. }) => {
                    if Instant::now() >= deadline {
                        return Err(BackpressureError::Timeout);
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

#### 2. Causal Ordering

```rust
/// Causal ordering component using vector clocks
pub struct CausalOrderer {
    /// Per-vector clock tracking
    vector_clocks: DashMap<VectorId, VectorClock>,
    /// Pending deltas waiting for dependencies
    pending: DashMap<DeltaId, PendingDelta>,
    /// Ready queue (topologically sorted)
    ready: ArrayQueue<VectorDelta>,
    /// Maximum buffer size
    max_pending: usize,
}

struct PendingDelta {
    delta: VectorDelta,
    missing_deps: HashSet<DeltaId>,
    buffered_at: Instant,
}

impl CausalOrderer {
    /// Process incoming delta, enforcing causal ordering
    pub fn process(&self, delta: VectorDelta) -> Vec<VectorDelta> {
        let mut ready_deltas = Vec::new();

        // Check if parent delta is satisfied
        if let Some(parent) = &delta.parent_delta {
            if !self.is_delivered(parent) {
                // Buffer until parent arrives
                self.buffer_pending(delta, parent);
                return ready_deltas;
            }
        }

        // Delta is ready
        self.mark_delivered(&delta);
        ready_deltas.push(delta.clone());

        // Release any deltas waiting on this one
        self.release_dependents(&delta.delta_id, &mut ready_deltas);

        ready_deltas
    }

    fn buffer_pending(&self, delta: VectorDelta, missing: &DeltaId) {
        let mut missing_deps = HashSet::new();
        missing_deps.insert(missing.clone());

        self.pending.insert(delta.delta_id.clone(), PendingDelta {
            delta,
            missing_deps,
            buffered_at: Instant::now(),
        });
    }

    fn release_dependents(&self, delta_id: &DeltaId, ready: &mut Vec<VectorDelta>) {
        let dependents: Vec<_> = self.pending
            .iter()
            .filter(|p| p.missing_deps.contains(delta_id))
            .map(|p| p.key().clone())
            .collect();

        for dep_id in dependents {
            if let Some((_, mut pending)) = self.pending.remove(&dep_id) {
                pending.missing_deps.remove(delta_id);
                if pending.missing_deps.is_empty() {
                    self.mark_delivered(&pending.delta);
                    ready.push(pending.delta.clone());
                    self.release_dependents(&dep_id, ready);
                } else {
                    self.pending.insert(dep_id, pending);
                }
            }
        }
    }
}
```

#### 3. Propagation Router

```rust
/// Topic-based delta router with priority queues
pub struct PropagationRouter {
    /// Registered sinks by topic
    sinks: DashMap<Topic, Vec<Arc<dyn DeltaSink>>>,
    /// Per-sink priority queues
    sink_queues: DashMap<SinkId, PriorityQueue<VectorDelta>>,
    /// Sink health tracking
    sink_health: DashMap<SinkId, SinkHealth>,
    /// Router configuration
    config: RouterConfig,
}

#[async_trait]
pub trait DeltaSink: Send + Sync {
    /// Unique sink identifier
    fn id(&self) -> SinkId;

    /// Topics this sink subscribes to
    fn topics(&self) -> Vec<Topic>;

    /// Process a delta
    async fn process(&self, delta: &VectorDelta) -> Result<()>;

    /// Batch process multiple deltas
    async fn process_batch(&self, deltas: &[VectorDelta]) -> Result<()> {
        for delta in deltas {
            self.process(delta).await?;
        }
        Ok(())
    }

    /// Sink capacity for backpressure
    fn capacity(&self) -> usize;

    /// Current queue depth
    fn queue_depth(&self) -> usize;
}

#[derive(Debug, Clone)]
pub enum Topic {
    AllDeltas,
    VectorId(VectorId),
    Namespace(String),
    DeltaType(DeltaType),
    Custom(String),
}

impl PropagationRouter {
    /// Route delta to all matching sinks
    pub async fn route(&self, delta: VectorDelta) -> Result<PropagationResult> {
        let topics = self.extract_topics(&delta);
        let mut results = Vec::new();

        for topic in topics {
            if let Some(sinks) = self.sinks.get(&topic) {
                for sink in sinks.iter() {
                    // Check sink health
                    let health = self.sink_health.get(&sink.id())
                        .map(|h| h.clone())
                        .unwrap_or_default();

                    if health.is_unhealthy() {
                        results.push(SinkResult::Skipped {
                            sink_id: sink.id(),
                            reason: "Unhealthy sink".into(),
                        });
                        continue;
                    }

                    // Apply backpressure if needed
                    if sink.queue_depth() >= sink.capacity() {
                        results.push(SinkResult::Backpressure {
                            sink_id: sink.id(),
                        });
                        self.apply_backpressure(&sink.id()).await;
                        continue;
                    }

                    // Route to sink
                    match sink.process(&delta).await {
                        Ok(()) => {
                            results.push(SinkResult::Success { sink_id: sink.id() });
                            self.record_success(&sink.id());
                        }
                        Err(e) => {
                            results.push(SinkResult::Error {
                                sink_id: sink.id(),
                                error: e.to_string(),
                            });
                            self.record_failure(&sink.id());
                        }
                    }
                }
            }
        }

        Ok(PropagationResult { delta_id: delta.delta_id, sink_results: results })
    }
}
```

### Backpressure Mechanism

```
                    ┌──────────────────────────────────────────────────────────┐
                    │                BACKPRESSURE FLOW                          │
                    └──────────────────────────────────────────────────────────┘

    Producer                    Router                         Slow Sink
       │                          │                                │
       │  ──── Delta 1 ────────>  │                                │
       │                          │  ──── Delta 1 ──────────────>  │
       │  ──── Delta 2 ────────>  │                                │ Processing
       │                          │  (Queue Delta 2)               │
       │  ──── Delta 3 ────────>  │                                │
       │                          │  (Queue Full!)                 │
       │  <── Backpressure ────   │                                │
       │                          │                                │
       │  (Slow down...)          │                      ACK       │
       │                          │  <─────────────────────────    │
       │                          │  ──── Delta 2 ──────────────>  │
       │  ──── Delta 4 ────────>  │                                │
       │                          │  (Queue has space)             │
       │                          │  ──── Delta 3 ──────────────>  │
```

### Adaptive Backpressure Algorithm

```rust
pub struct AdaptiveBackpressure {
    /// Current rate limit (deltas per second)
    rate_limit: AtomicF64,
    /// Minimum rate limit
    min_rate: f64,
    /// Maximum rate limit
    max_rate: f64,
    /// Window for measuring throughput
    window: Duration,
    /// Adjustment factor
    alpha: f64,
}

impl AdaptiveBackpressure {
    /// Adjust rate based on sink feedback
    pub fn adjust(&self, sink_stats: &SinkStats) {
        let current = self.rate_limit.load(Ordering::Relaxed);

        // Calculate optimal rate based on sink capacity
        let utilization = sink_stats.queue_depth as f64 / sink_stats.capacity as f64;

        let new_rate = if utilization > 0.9 {
            // Sink overwhelmed - reduce aggressively
            (current * 0.5).max(self.min_rate)
        } else if utilization > 0.7 {
            // Approaching capacity - reduce slowly
            (current * 0.9).max(self.min_rate)
        } else if utilization < 0.3 {
            // Underutilized - increase slowly
            (current * 1.1).min(self.max_rate)
        } else {
            // Optimal range - maintain
            current
        };

        // Exponential smoothing
        let adjusted = self.alpha * new_rate + (1.0 - self.alpha) * current;
        self.rate_limit.store(adjusted, Ordering::Relaxed);
    }
}
```

---

## Latency and Throughput Analysis

### Latency Breakdown

| Stage | p50 | p95 | p99 |
|-------|-----|-----|-----|
| Ingest queue | 5us | 15us | 50us |
| Causal ordering | 10us | 30us | 100us |
| Router dispatch | 8us | 25us | 80us |
| Storage sink | 100us | 500us | 2ms |
| Index sink | 50us | 200us | 1ms |
| Cache sink | 2us | 10us | 30us |
| **Total (fast path)** | **175us** | **780us** | **3.3ms** |

### Throughput Characteristics

| Configuration | Throughput | Notes |
|---------------|------------|-------|
| Single sink | 500K delta/s | Memory-limited |
| Storage + Index | 100K delta/s | I/O bound |
| Full pipeline | 50K delta/s | With replication |
| Geo-distributed | 10K delta/s | Network bound |

### Batching Impact

| Batch Size | Latency | Throughput | Memory |
|------------|---------|------------|--------|
| 1 | 175us | 50K/s | 1KB |
| 10 | 200us | 200K/s | 10KB |
| 100 | 500us | 500K/s | 100KB |
| 1000 | 2ms | 800K/s | 1MB |

---

## Considered Options

### Option 1: Pull-Based (Polling)

**Description**: Consumers poll for new deltas.

**Pros**:
- Consumer controls rate
- Simple producer
- No backpressure needed

**Cons**:
- High latency (polling interval)
- Wasted requests when idle
- Ordering complexity at consumer

**Verdict**: Rejected - latency unacceptable for real-time search.

### Option 2: Pure Push (Fire-and-Forget)

**Description**: Producer pushes deltas without acknowledgment.

**Pros**:
- Lowest latency
- Simplest protocol
- Maximum throughput

**Cons**:
- No delivery guarantee
- No backpressure
- Slow consumers drop deltas

**Verdict**: Rejected - reliability requirements not met.

### Option 3: Reactive Streams (Rx-style)

**Description**: Full reactive streams with backpressure.

**Pros**:
- Proper backpressure
- Composable operators
- Industry standard

**Cons**:
- Complex implementation
- Learning curve
- Overhead for simple cases

**Verdict**: Partially adopted - backpressure concepts without full Rx.

### Option 4: Reactive Push with Backpressure (Selected)

**Description**: Push-based with explicit backpressure signaling.

**Pros**:
- Low latency push
- Backpressure handling
- Causal ordering
- Reliability guarantees

**Cons**:
- More complex than pure push
- Requires sink cooperation

**Verdict**: Adopted - optimal balance for delta propagation.

---

## Technical Specification

### Wire Protocol

```
Delta Propagation Message:
+--------+--------+--------+--------+--------+--------+--------+--------+
| Magic  | Version| MsgType| Flags  |    Sequence Number (64-bit)       |
| 0xD3   | 0x01   | 0-7    | 8 bits |                                   |
+--------+--------+--------+--------+--------+--------+--------+--------+
|     Payload Length (32-bit)       |         Delta Payload             |
|                                   |         (variable)                |
+--------+--------+--------+--------+-----------------------------------|

Message Types:
  0x00: Delta
  0x01: Batch
  0x02: Ack
  0x03: Nack
  0x04: Backpressure
  0x05: Heartbeat
  0x06: Subscribe
  0x07: Unsubscribe

Flags:
  bit 0: Requires acknowledgment
  bit 1: Priority (0=normal, 1=high)
  bit 2: Compressed
  bit 3: Batched
  bits 4-7: Reserved
```

### Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropagationConfig {
    /// Ingest queue capacity
    pub ingest_queue_capacity: usize,
    /// High water mark percentage (0.0-1.0)
    pub high_water_mark: f32,
    /// Maximum pending deltas in causal orderer
    pub max_pending_deltas: usize,
    /// Pending delta timeout
    pub pending_timeout: Duration,
    /// Batch size for sink delivery
    pub batch_size: usize,
    /// Batch timeout (flush even if batch not full)
    pub batch_timeout: Duration,
    /// Backpressure adjustment interval
    pub backpressure_interval: Duration,
    /// Retry configuration
    pub retry_config: RetryConfig,
}

impl Default for PropagationConfig {
    fn default() -> Self {
        Self {
            ingest_queue_capacity: 100_000,
            high_water_mark: 0.8,
            max_pending_deltas: 10_000,
            pending_timeout: Duration::from_secs(30),
            batch_size: 100,
            batch_timeout: Duration::from_millis(10),
            backpressure_interval: Duration::from_millis(100),
            retry_config: RetryConfig::default(),
        }
    }
}
```

---

## Consequences

### Benefits

1. **Low Latency**: Sub-millisecond propagation on fast path
2. **Reliability**: Delivery guarantees with acknowledgments
3. **Scalability**: Backpressure prevents overload
4. **Ordering**: Causal consistency preserved
5. **Flexibility**: Topic-based routing for selective propagation

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Message loss | Low | High | WAL + acknowledgments |
| Ordering violations | Low | High | Vector clocks, buffering |
| Backpressure storms | Medium | Medium | Adaptive rate limiting |
| Sink failure cascade | Medium | High | Circuit breakers, health checks |

---

## References

1. Chandy, K.M., & Lamport, L. "Distributed Snapshots: Determining Global States of Distributed Systems."
2. Reactive Streams Specification. https://www.reactive-streams.org/
3. ADR-DB-001: Delta Behavior Core Architecture
4. Ruvector gossip.rs: SWIM membership protocol

---

## Related Decisions

- **ADR-DB-001**: Delta Behavior Core Architecture
- **ADR-DB-004**: Delta Conflict Resolution
- **ADR-DB-007**: Delta Temporal Windows
