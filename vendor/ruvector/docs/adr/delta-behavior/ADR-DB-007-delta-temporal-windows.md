# ADR-DB-007: Delta Temporal Windows

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

### The Windowing Challenge

Delta streams require intelligent batching and aggregation:

1. **Write Amplification**: Processing individual deltas is inefficient
2. **Network Efficiency**: Batching reduces per-message overhead
3. **Memory Pressure**: Unbounded buffering causes OOM
4. **Latency Requirements**: Different use cases have different freshness needs
5. **Compaction**: Old deltas should be merged to save space

### Window Types

| Type | Description | Use Case |
|------|-------------|----------|
| Fixed | Consistent time intervals | Batch processing |
| Sliding | Overlapping windows | Moving averages |
| Session | Activity-based | User sessions |
| Tumbling | Non-overlapping fixed | Checkpointing |
| Adaptive | Dynamic sizing | Variable load |

---

## Decision

### Adopt Adaptive Windows with Compaction

We implement an adaptive windowing system that dynamically adjusts based on load and compacts old deltas.

### Architecture Overview

```
                    ┌─────────────────────────────────────────────────────────────┐
                    │                    DELTA TEMPORAL MANAGER                    │
                    └─────────────────────────────────────────────────────────────┘
                                               │
                    ┌──────────────────────────┼──────────────────────────────────┐
                    │                          │                                   │
                    v                          v                                   v
            ┌───────────────┐          ┌───────────────┐                  ┌───────────────┐
            │   Ingestion   │          │   Window      │                  │  Compaction   │
            │   Buffer      │─────────>│   Processor   │─────────────────>│   Engine      │
            └───────────────┘          └───────────────┘                  └───────────────┘
                    │                          │                                   │
                    v                          v                                   v
            ┌───────────────┐          ┌───────────────┐                  ┌───────────────┐
            │  Rate Monitor │          │   Emitter     │                  │  Checkpoint   │
            │               │          │               │                  │   Creator     │
            └───────────────┘          └───────────────┘                  └───────────────┘

            INGESTION                   PROCESSING                         STORAGE
```

### Core Components

#### 1. Adaptive Window Manager

```rust
/// Adaptive window that adjusts size based on load
pub struct AdaptiveWindowManager {
    /// Current window configuration
    current_config: RwLock<WindowConfig>,
    /// Ingestion buffer
    buffer: SegQueue<BufferedDelta>,
    /// Buffer size counter
    buffer_size: AtomicUsize,
    /// Rate monitor
    rate_monitor: RateMonitor,
    /// Window emitter
    emitter: WindowEmitter,
    /// Configuration bounds
    bounds: WindowBounds,
}

#[derive(Debug, Clone)]
pub struct WindowConfig {
    /// Window type
    pub window_type: WindowType,
    /// Current window duration
    pub duration: Duration,
    /// Maximum buffer size
    pub max_size: usize,
    /// Trigger conditions
    pub triggers: Vec<WindowTrigger>,
}

#[derive(Debug, Clone, Copy)]
pub enum WindowType {
    /// Fixed time interval
    Fixed { interval: Duration },
    /// Sliding window with step
    Sliding { size: Duration, step: Duration },
    /// Session-based (gap timeout)
    Session { gap_timeout: Duration },
    /// Non-overlapping fixed
    Tumbling { size: Duration },
    /// Dynamic sizing
    Adaptive {
        min_duration: Duration,
        max_duration: Duration,
        target_batch_size: usize,
    },
}

#[derive(Debug, Clone)]
pub enum WindowTrigger {
    /// Time-based trigger
    Time { interval: Duration },
    /// Count-based trigger
    Count { threshold: usize },
    /// Size-based trigger (bytes)
    Size { threshold: usize },
    /// Rate change trigger
    RateChange { threshold: f32 },
    /// Memory pressure trigger
    MemoryPressure { threshold: f32 },
}

impl AdaptiveWindowManager {
    /// Add delta to current window
    pub fn add_delta(&self, delta: VectorDelta) -> Result<()> {
        let buffered = BufferedDelta {
            delta,
            buffered_at: Instant::now(),
        };

        self.buffer.push(buffered);
        let new_size = self.buffer_size.fetch_add(1, Ordering::Relaxed) + 1;

        // Check if we should trigger window
        if self.should_trigger(new_size) {
            self.trigger_window().await?;
        }

        Ok(())
    }

    /// Check trigger conditions
    fn should_trigger(&self, buffer_size: usize) -> bool {
        let config = self.current_config.read().unwrap();

        for trigger in &config.triggers {
            match trigger {
                WindowTrigger::Count { threshold } => {
                    if buffer_size >= *threshold {
                        return true;
                    }
                }
                WindowTrigger::MemoryPressure { threshold } => {
                    if self.get_memory_pressure() >= *threshold {
                        return true;
                    }
                }
                // Other triggers checked by background task
                _ => {}
            }
        }

        false
    }

    /// Trigger window emission
    async fn trigger_window(&self) -> Result<()> {
        // Drain buffer
        let mut deltas = Vec::new();
        while let Some(buffered) = self.buffer.pop() {
            deltas.push(buffered);
        }
        self.buffer_size.store(0, Ordering::Relaxed);

        // Emit window
        self.emitter.emit(WindowedDeltas {
            deltas,
            window_start: Instant::now(), // Would be first delta timestamp
            window_end: Instant::now(),
            trigger_reason: WindowTriggerReason::Explicit,
        }).await?;

        // Adapt window size based on metrics
        self.adapt_window_size();

        Ok(())
    }

    /// Adapt window size based on load
    fn adapt_window_size(&self) {
        let rate = self.rate_monitor.current_rate();
        let mut config = self.current_config.write().unwrap();

        if let WindowType::Adaptive { min_duration, max_duration, target_batch_size } = &config.window_type {
            // Calculate optimal duration for target batch size
            let optimal_duration = if rate > 0.0 {
                Duration::from_secs_f64(*target_batch_size as f64 / rate)
            } else {
                *max_duration
            };

            // Clamp to bounds
            config.duration = optimal_duration.clamp(*min_duration, *max_duration);

            // Update time trigger
            for trigger in &mut config.triggers {
                if let WindowTrigger::Time { interval } = trigger {
                    *interval = config.duration;
                }
            }
        }
    }
}
```

#### 2. Rate Monitor

```rust
/// Monitors delta ingestion rate
pub struct RateMonitor {
    /// Sliding window of counts
    counts: VecDeque<(Instant, u64)>,
    /// Window duration for rate calculation
    window: Duration,
    /// Current rate estimate
    current_rate: AtomicF64,
    /// Rate change detection
    rate_history: VecDeque<f64>,
}

impl RateMonitor {
    /// Record delta arrival
    pub fn record(&self, count: u64) {
        let now = Instant::now();

        // Add new count
        self.counts.push_back((now, count));

        // Remove old entries
        let cutoff = now - self.window;
        while let Some((t, _)) = self.counts.front() {
            if *t < cutoff {
                self.counts.pop_front();
            } else {
                break;
            }
        }

        // Calculate current rate
        let total: u64 = self.counts.iter().map(|(_, c)| c).sum();
        let duration = self.counts.back()
            .map(|(t, _)| t.duration_since(self.counts.front().unwrap().0))
            .unwrap_or(Duration::from_secs(1));

        let rate = total as f64 / duration.as_secs_f64().max(0.001);
        self.current_rate.store(rate, Ordering::Relaxed);

        // Track rate history for change detection
        self.rate_history.push_back(rate);
        if self.rate_history.len() > 100 {
            self.rate_history.pop_front();
        }
    }

    /// Get current rate (deltas per second)
    pub fn current_rate(&self) -> f64 {
        self.current_rate.load(Ordering::Relaxed)
    }

    /// Detect significant rate change
    pub fn rate_change_detected(&self, threshold: f32) -> bool {
        if self.rate_history.len() < 10 {
            return false;
        }

        let recent: Vec<_> = self.rate_history.iter().rev().take(5).collect();
        let older: Vec<_> = self.rate_history.iter().rev().skip(5).take(10).collect();

        let recent_avg = recent.iter().copied().sum::<f64>() / recent.len() as f64;
        let older_avg = older.iter().copied().sum::<f64>() / older.len().max(1) as f64;

        let change = (recent_avg - older_avg).abs() / older_avg.max(1.0);
        change > threshold as f64
    }
}
```

#### 3. Compaction Engine

```rust
/// Compacts delta chains to reduce storage
pub struct CompactionEngine {
    /// Compaction configuration
    config: CompactionConfig,
    /// Active compaction tasks
    tasks: DashMap<VectorId, CompactionTask>,
    /// Compaction metrics
    metrics: CompactionMetrics,
}

#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Trigger compaction after N deltas
    pub delta_threshold: usize,
    /// Trigger compaction after duration
    pub time_threshold: Duration,
    /// Maximum chain length before forced compaction
    pub max_chain_length: usize,
    /// Compaction strategy
    pub strategy: CompactionStrategy,
    /// Background compaction enabled
    pub background: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum CompactionStrategy {
    /// Merge all deltas into single checkpoint
    FullMerge,
    /// Keep recent deltas, merge older
    TieredMerge { keep_recent: usize },
    /// Keep deltas at time boundaries
    TimeBoundary { interval: Duration },
    /// Adaptive based on access patterns
    Adaptive,
}

impl CompactionEngine {
    /// Check if vector needs compaction
    pub fn needs_compaction(&self, chain: &DeltaChain) -> bool {
        // Delta count threshold
        if chain.pending_deltas.len() >= self.config.delta_threshold {
            return true;
        }

        // Time threshold
        if let Some(first) = chain.pending_deltas.first() {
            if first.timestamp.elapsed() > self.config.time_threshold {
                return true;
            }
        }

        // Chain length threshold
        if chain.pending_deltas.len() >= self.config.max_chain_length {
            return true;
        }

        false
    }

    /// Compact a delta chain
    pub async fn compact(&self, chain: &mut DeltaChain) -> Result<CompactionResult> {
        match self.config.strategy {
            CompactionStrategy::FullMerge => {
                self.full_merge(chain).await
            }
            CompactionStrategy::TieredMerge { keep_recent } => {
                self.tiered_merge(chain, keep_recent).await
            }
            CompactionStrategy::TimeBoundary { interval } => {
                self.time_boundary_merge(chain, interval).await
            }
            CompactionStrategy::Adaptive => {
                self.adaptive_merge(chain).await
            }
        }
    }

    /// Full merge: create checkpoint from all deltas
    async fn full_merge(&self, chain: &mut DeltaChain) -> Result<CompactionResult> {
        // Compose current vector
        let current_vector = chain.compose()?;

        // Create new checkpoint
        let checkpoint = Checkpoint {
            vector: current_vector,
            at_delta: chain.pending_deltas.last()
                .map(|d| d.delta_id.clone())
                .unwrap_or_default(),
            timestamp: Utc::now(),
            delta_count: chain.pending_deltas.len() as u64,
        };

        let merged_count = chain.pending_deltas.len();

        // Clear deltas, set checkpoint
        chain.pending_deltas.clear();
        chain.checkpoint = Some(checkpoint);

        Ok(CompactionResult {
            deltas_merged: merged_count,
            space_saved: estimate_space_saved(merged_count),
            strategy: CompactionStrategy::FullMerge,
        })
    }

    /// Tiered merge: keep recent, merge older
    async fn tiered_merge(
        &self,
        chain: &mut DeltaChain,
        keep_recent: usize,
    ) -> Result<CompactionResult> {
        if chain.pending_deltas.len() <= keep_recent {
            return Ok(CompactionResult::no_op());
        }

        // Split into old and recent
        let split_point = chain.pending_deltas.len() - keep_recent;
        let old_deltas: Vec<_> = chain.pending_deltas.drain(..split_point).collect();

        // Compose checkpoint from old deltas
        let mut checkpoint_vector = chain.checkpoint
            .as_ref()
            .map(|c| c.vector.clone())
            .unwrap_or_else(|| vec![0.0; chain.dimensions()]);

        for delta in &old_deltas {
            chain.apply_operation(&mut checkpoint_vector, &delta.operation)?;
        }

        // Update checkpoint
        chain.checkpoint = Some(Checkpoint {
            vector: checkpoint_vector,
            at_delta: old_deltas.last().unwrap().delta_id.clone(),
            timestamp: Utc::now(),
            delta_count: old_deltas.len() as u64,
        });

        Ok(CompactionResult {
            deltas_merged: old_deltas.len(),
            space_saved: estimate_space_saved(old_deltas.len()),
            strategy: CompactionStrategy::TieredMerge { keep_recent },
        })
    }

    /// Time boundary merge: keep deltas at boundaries
    async fn time_boundary_merge(
        &self,
        chain: &mut DeltaChain,
        interval: Duration,
    ) -> Result<CompactionResult> {
        let now = Utc::now();
        let mut kept = Vec::new();
        let mut merged_count = 0;

        // Group by time boundaries
        let mut groups: HashMap<i64, Vec<&VectorDelta>> = HashMap::new();
        for delta in &chain.pending_deltas {
            let boundary = delta.timestamp.timestamp() / interval.as_secs() as i64;
            groups.entry(boundary).or_default().push(delta);
        }

        // Keep one delta per boundary
        for (_boundary, deltas) in groups {
            kept.push(deltas.last().unwrap().clone());
            merged_count += deltas.len() - 1;
        }

        chain.pending_deltas = kept;

        Ok(CompactionResult {
            deltas_merged: merged_count,
            space_saved: estimate_space_saved(merged_count),
            strategy: CompactionStrategy::TimeBoundary { interval },
        })
    }
}
```

### Window Processing Pipeline

```
Delta Stream
     │
     v
┌────────────────────────────────────────────────────────────────────────────┐
│                           WINDOW PROCESSOR                                  │
│                                                                            │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐ │
│  │   Buffer    │───>│   Window    │───>│  Aggregate  │───>│   Emit      │ │
│  │             │    │   Detect    │    │             │    │             │ │
│  └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘ │
│        │                  │                  │                  │         │
│        v                  v                  v                  v         │
│   Time Trigger      Size Trigger       Merge Deltas      Batch Output    │
│   Count Trigger     Rate Trigger       Deduplicate       Compress        │
│   Memory Trigger    Custom Trigger     Sort by Time      Propagate       │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
                                    │
                                    v
                    ┌───────────────────────────────────┐
                    │           Window Output           │
                    │   - Batched deltas                │
                    │   - Window metadata               │
                    │   - Aggregation stats             │
                    └───────────────────────────────────┘
```

---

## Memory Bounds

### Buffer Memory Management

```rust
/// Memory-bounded buffer configuration
pub struct MemoryBoundsConfig {
    /// Maximum buffer memory (bytes)
    pub max_memory: usize,
    /// High water mark for warning
    pub high_water_mark: f32,
    /// Emergency flush threshold
    pub emergency_threshold: f32,
}

impl Default for MemoryBoundsConfig {
    fn default() -> Self {
        Self {
            max_memory: 100 * 1024 * 1024, // 100MB
            high_water_mark: 0.8,
            emergency_threshold: 0.95,
        }
    }
}

/// Memory tracking for window buffers
pub struct MemoryTracker {
    /// Current usage
    current: AtomicUsize,
    /// Configuration
    config: MemoryBoundsConfig,
}

impl MemoryTracker {
    /// Track memory allocation
    pub fn allocate(&self, bytes: usize) -> Result<MemoryGuard, MemoryPressure> {
        let current = self.current.fetch_add(bytes, Ordering::Relaxed);
        let new_total = current + bytes;

        let usage_ratio = new_total as f32 / self.config.max_memory as f32;

        if usage_ratio > self.config.emergency_threshold {
            // Rollback and fail
            self.current.fetch_sub(bytes, Ordering::Relaxed);
            return Err(MemoryPressure::Emergency);
        }

        if usage_ratio > self.config.high_water_mark {
            return Err(MemoryPressure::Warning);
        }

        Ok(MemoryGuard {
            tracker: self,
            bytes,
        })
    }

    /// Get current pressure level
    pub fn pressure_level(&self) -> MemoryPressureLevel {
        let ratio = self.current.load(Ordering::Relaxed) as f32
            / self.config.max_memory as f32;

        if ratio > self.config.emergency_threshold {
            MemoryPressureLevel::Emergency
        } else if ratio > self.config.high_water_mark {
            MemoryPressureLevel::High
        } else if ratio > 0.5 {
            MemoryPressureLevel::Medium
        } else {
            MemoryPressureLevel::Low
        }
    }
}
```

### Memory Budget by Component

| Component | Default Budget | Scaling |
|-----------|----------------|---------|
| Ingestion buffer | 50MB | Per shard |
| Rate monitor | 1MB | Fixed |
| Compaction tasks | 20MB | Per active chain |
| Window metadata | 5MB | Per window |
| **Total** | **~100MB** | Per instance |

---

## Considered Options

### Option 1: Fixed Windows Only

**Description**: Simple fixed-interval windows.

**Pros**:
- Simple implementation
- Predictable behavior
- Easy debugging

**Cons**:
- Inefficient for variable load
- May batch too few or too many
- No load adaptation

**Verdict**: Available as configuration, not default.

### Option 2: Count-Based Batching

**Description**: Emit after N deltas.

**Pros**:
- Consistent batch sizes
- Predictable memory

**Cons**:
- Variable latency
- May hold deltas too long at low load
- No time bounds

**Verdict**: Available as trigger, combined with time.

### Option 3: Session Windows

**Description**: Window based on activity gaps.

**Pros**:
- Natural for user interactions
- Adapts to activity patterns

**Cons**:
- Unpredictable timing
- Complex to implement correctly
- Memory pressure with long sessions

**Verdict**: Available for specific use cases.

### Option 4: Adaptive Windows (Selected)

**Description**: Dynamic sizing based on load and memory.

**Pros**:
- Optimal batch sizes
- Respects memory bounds
- Adapts to load changes
- Multiple trigger types

**Cons**:
- More complex
- Requires tuning
- Less predictable

**Verdict**: Adopted - best for varying delta workloads.

---

## Technical Specification

### Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalConfig {
    /// Window type and parameters
    pub window_type: WindowType,
    /// Memory bounds
    pub memory_bounds: MemoryBoundsConfig,
    /// Compaction configuration
    pub compaction: CompactionConfig,
    /// Background task interval
    pub background_interval: Duration,
    /// Late data handling
    pub late_data: LateDataPolicy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LateDataPolicy {
    /// Discard late data
    Discard,
    /// Include in next window
    NextWindow,
    /// Reemit updated window
    Reemit { max_lateness: Duration },
}

impl Default for TemporalConfig {
    fn default() -> Self {
        Self {
            window_type: WindowType::Adaptive {
                min_duration: Duration::from_millis(10),
                max_duration: Duration::from_secs(5),
                target_batch_size: 100,
            },
            memory_bounds: MemoryBoundsConfig::default(),
            compaction: CompactionConfig {
                delta_threshold: 100,
                time_threshold: Duration::from_secs(60),
                max_chain_length: 1000,
                strategy: CompactionStrategy::TieredMerge { keep_recent: 10 },
                background: true,
            },
            background_interval: Duration::from_millis(100),
            late_data: LateDataPolicy::NextWindow,
        }
    }
}
```

### Window Output Format

```rust
#[derive(Debug, Clone)]
pub struct WindowOutput {
    /// Window identifier
    pub window_id: WindowId,
    /// Start timestamp
    pub start: DateTime<Utc>,
    /// End timestamp
    pub end: DateTime<Utc>,
    /// Deltas in window
    pub deltas: Vec<VectorDelta>,
    /// Window statistics
    pub stats: WindowStats,
    /// Trigger reason
    pub trigger: WindowTriggerReason,
}

#[derive(Debug, Clone)]
pub struct WindowStats {
    /// Number of deltas
    pub delta_count: usize,
    /// Unique vectors affected
    pub vectors_affected: usize,
    /// Total bytes
    pub total_bytes: usize,
    /// Average delta size
    pub avg_delta_size: f32,
    /// Window duration
    pub duration: Duration,
}
```

---

## Consequences

### Benefits

1. **Efficient Batching**: Optimal batch sizes for varying load
2. **Memory Safety**: Bounded memory usage
3. **Adaptive**: Responds to load changes
4. **Compaction**: Reduces long-term storage
5. **Flexible**: Multiple window types and triggers

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Over-batching | Medium | Low | Multiple triggers |
| Under-batching | Medium | Medium | Count-based fallback |
| Memory spikes | Low | High | Emergency flush |
| Data loss | Low | High | WAL before windowing |

---

## References

1. Akidau, T., et al. "The Dataflow Model: A Practical Approach to Balancing Correctness, Latency, and Cost in Massive-Scale, Unbounded, Out-of-Order Data Processing."
2. Carbone, P., et al. "State Management in Apache Flink."
3. ADR-DB-001: Delta Behavior Core Architecture

---

## Related Decisions

- **ADR-DB-001**: Delta Behavior Core Architecture
- **ADR-DB-003**: Delta Propagation Protocol
- **ADR-DB-006**: Delta Compression Strategy
