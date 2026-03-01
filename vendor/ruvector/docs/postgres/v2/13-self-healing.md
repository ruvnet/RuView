# RuVector Postgres v2 - Self-Healing System

## The Missing Piece

We built the sensor (mincut integrity monitoring).
Now we build the actuator (automated remediation).

Most systems detect problems and alert humans. We detect problems and **fix them automatically**.

```
Traditional:    Detect → Alert → Human → Diagnose → Fix → Verify
Self-Healing:   Detect → Classify → Remediate → Verify → Learn
```

This completes the control loop and makes RuVector truly autonomous.

---

## Design Principles

1. **Graduated response** — Small problems get small fixes
2. **Reversible actions** — Every remediation can be undone
3. **Blast radius limits** — Never make things worse
4. **Audit trail** — Every action is logged and signed
5. **Learn from outcomes** — Improve remediation over time

---

## Architecture

```
+------------------------------------------------------------------+
|                    Integrity Monitor                              |
|  - Computes lambda_cut on contracted graph                       |
|  - Detects state transitions (normal → stress → critical)        |
+------------------------------------------------------------------+
                              |
                              v
+------------------------------------------------------------------+
|                    Problem Classifier                             |
|  - Identifies root cause from witness edges                      |
|  - Maps symptoms to remediation strategies                       |
+------------------------------------------------------------------+
                              |
                              v
+------------------------------------------------------------------+
|                    Remediation Engine                             |
|  - Selects appropriate action                                    |
|  - Executes with timeout and rollback                            |
|  - Verifies improvement                                          |
+------------------------------------------------------------------+
                              |
                              v
+------------------------------------------------------------------+
|                    Outcome Tracker                                |
|  - Records success/failure                                       |
|  - Updates strategy weights                                      |
|  - Feeds learning pipeline                                       |
+------------------------------------------------------------------+
```

---

## Problem Classification

### Witness Edge Analysis

The mincut computation produces **witness edges** — the edges that would break the graph. These reveal the problem type.

```rust
// src/healing/classifier.rs

#[derive(Debug, Clone)]
pub enum ProblemClass {
    /// Hot partition overloaded
    HotspotCongestion {
        partition_ids: Vec<i64>,
        load_ratio: f32,  // vs average
    },

    /// Centroid imbalance in IVF index
    CentroidSkew {
        centroid_ids: Vec<i64>,
        skew_factor: f32,
    },

    /// Replication lag causing consistency issues
    ReplicationLag {
        replica_ids: Vec<i64>,
        lag_seconds: f32,
    },

    /// Background job contention
    MaintenanceContention {
        job_types: Vec<String>,
        queue_depth: usize,
    },

    /// Index fragmentation
    IndexFragmentation {
        index_ids: Vec<i64>,
        fragmentation_pct: f32,
    },

    /// Memory pressure
    MemoryPressure {
        current_usage_pct: f32,
        largest_consumers: Vec<(String, usize)>,
    },

    /// Unknown (needs human investigation)
    Unknown {
        witness_summary: String,
    },
}

pub fn classify_problem(
    witness_edges: &[WitnessEdge],
    metrics: &SystemMetrics,
) -> ProblemClass {
    // Analyze witness edge patterns
    let edge_types = count_edge_types(witness_edges);
    let node_types = count_node_types(witness_edges);

    // Pattern matching
    if edge_types.get("partition_link").unwrap_or(&0) > &3 {
        // Multiple partition links weak → hotspot
        let hot_partitions = find_hot_partitions(witness_edges, metrics);
        return ProblemClass::HotspotCongestion {
            partition_ids: hot_partitions,
            load_ratio: compute_load_ratio(&hot_partitions, metrics),
        };
    }

    if node_types.get("centroid").unwrap_or(&0) > &5 {
        // Centroid nodes in witness → skew
        let skewed = find_skewed_centroids(witness_edges, metrics);
        return ProblemClass::CentroidSkew {
            centroid_ids: skewed,
            skew_factor: compute_skew_factor(&skewed, metrics),
        };
    }

    if edge_types.get("replication").unwrap_or(&0) > &0 {
        // Replication edges weak
        let lagging = find_lagging_replicas(witness_edges, metrics);
        return ProblemClass::ReplicationLag {
            replica_ids: lagging,
            lag_seconds: get_max_lag(&lagging, metrics),
        };
    }

    if edge_types.get("dependency").unwrap_or(&0) > &2 {
        // Maintenance dependencies weak
        let jobs = find_contending_jobs(witness_edges, metrics);
        return ProblemClass::MaintenanceContention {
            job_types: jobs,
            queue_depth: metrics.maintenance_queue_depth,
        };
    }

    // Check metrics for other patterns
    if metrics.memory_usage_pct > 85.0 {
        return ProblemClass::MemoryPressure {
            current_usage_pct: metrics.memory_usage_pct,
            largest_consumers: metrics.top_memory_consumers.clone(),
        };
    }

    if metrics.index_fragmentation_pct > 30.0 {
        return ProblemClass::IndexFragmentation {
            index_ids: metrics.fragmented_indexes.clone(),
            fragmentation_pct: metrics.index_fragmentation_pct,
        };
    }

    ProblemClass::Unknown {
        witness_summary: summarize_witnesses(witness_edges),
    }
}
```

---

## Remediation Strategies

### Strategy Registry

```rust
// src/healing/strategies.rs

pub trait RemediationStrategy: Send + Sync {
    /// Human-readable name
    fn name(&self) -> &str;

    /// Problem classes this strategy handles
    fn handles(&self) -> Vec<ProblemClass>;

    /// Estimate impact (0-1, higher = more disruptive)
    fn impact(&self) -> f32;

    /// Estimate time to complete
    fn estimated_duration(&self) -> Duration;

    /// Can this be reversed?
    fn reversible(&self) -> bool;

    /// Execute the remediation
    fn execute(&self, context: &RemediationContext) -> Result<RemediationResult, Error>;

    /// Rollback if needed
    fn rollback(&self, context: &RemediationContext) -> Result<(), Error>;
}

/// Registry of all available strategies
pub struct StrategyRegistry {
    strategies: Vec<Box<dyn RemediationStrategy>>,
    weights: HashMap<String, f32>,  // Learned effectiveness weights
}

impl StrategyRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            strategies: vec![],
            weights: HashMap::new(),
        };

        // Register built-in strategies
        registry.register(Box::new(RebalancePartitions));
        registry.register(Box::new(RebuildCentroids));
        registry.register(Box::new(PauseMaintenanceJobs));
        registry.register(Box::new(ThrottleIngestion));
        registry.register(Box::new(EvictColdData));
        registry.register(Box::new(CompactFragmentedIndexes));
        registry.register(Box::new(ScaleReadReplicas));
        registry.register(Box::new(DrainHotPartition));

        registry
    }

    /// Select best strategy for a problem
    pub fn select(&self, problem: &ProblemClass, context: &SystemContext) -> Option<&dyn RemediationStrategy> {
        self.strategies.iter()
            .filter(|s| s.handles().iter().any(|h| matches_class(h, problem)))
            .filter(|s| s.impact() <= context.max_allowed_impact)
            .max_by(|a, b| {
                let weight_a = self.weights.get(a.name()).unwrap_or(&1.0);
                let weight_b = self.weights.get(b.name()).unwrap_or(&1.0);
                weight_a.partial_cmp(weight_b).unwrap()
            })
            .map(|s| s.as_ref())
    }
}
```

### Built-in Strategies

#### 1. Rebalance Partitions

```rust
pub struct RebalancePartitions;

impl RemediationStrategy for RebalancePartitions {
    fn name(&self) -> &str { "rebalance_partitions" }

    fn handles(&self) -> Vec<ProblemClass> {
        vec![ProblemClass::HotspotCongestion { .. }]
    }

    fn impact(&self) -> f32 { 0.3 }  // Medium impact

    fn reversible(&self) -> bool { true }

    fn execute(&self, ctx: &RemediationContext) -> Result<RemediationResult, Error> {
        let problem = ctx.problem.as_hotspot()?;

        // Find underutilized partitions
        let cold_partitions = find_cold_partitions(ctx.metrics);

        // Calculate rebalance plan
        let plan = compute_rebalance_plan(
            &problem.partition_ids,
            &cold_partitions,
            ctx.metrics,
        );

        // Execute moves incrementally
        for mv in plan.moves {
            // Move vectors from hot to cold partition
            move_vectors(mv.from_partition, mv.to_partition, mv.vector_ids)?;

            // Check if integrity improved
            let new_lambda = sample_integrity(ctx.collection_id)?;
            if new_lambda > ctx.initial_lambda * 1.1 {
                // Good progress, continue
            } else if new_lambda < ctx.initial_lambda * 0.9 {
                // Made things worse, rollback this move
                move_vectors(mv.to_partition, mv.from_partition, mv.vector_ids)?;
                break;
            }
        }

        Ok(RemediationResult {
            success: true,
            actions_taken: plan.moves.len(),
            improvement: compute_improvement(ctx),
        })
    }

    fn rollback(&self, ctx: &RemediationContext) -> Result<(), Error> {
        // Reverse all recorded moves
        for mv in ctx.recorded_moves.iter().rev() {
            move_vectors(mv.to_partition, mv.from_partition, mv.vector_ids)?;
        }
        Ok(())
    }
}
```

#### 2. Pause Maintenance Jobs

```rust
pub struct PauseMaintenanceJobs;

impl RemediationStrategy for PauseMaintenanceJobs {
    fn name(&self) -> &str { "pause_maintenance" }

    fn handles(&self) -> Vec<ProblemClass> {
        vec![ProblemClass::MaintenanceContention { .. }]
    }

    fn impact(&self) -> f32 { 0.1 }  // Low impact

    fn reversible(&self) -> bool { true }

    fn execute(&self, ctx: &RemediationContext) -> Result<RemediationResult, Error> {
        let problem = ctx.problem.as_maintenance_contention()?;

        // Pause low-priority jobs
        let paused = problem.job_types.iter()
            .filter(|j| job_priority(j) < Priority::High)
            .map(|j| {
                pause_job(j)?;
                j.clone()
            })
            .collect::<Vec<_>>();

        // Wait for current operations to drain
        wait_for_drain(Duration::from_secs(30))?;

        // Verify improvement
        let new_lambda = sample_integrity(ctx.collection_id)?;

        Ok(RemediationResult {
            success: new_lambda > ctx.initial_lambda,
            actions_taken: paused.len(),
            improvement: (new_lambda - ctx.initial_lambda) / ctx.initial_lambda,
            metadata: json!({ "paused_jobs": paused }),
        })
    }

    fn rollback(&self, ctx: &RemediationContext) -> Result<(), Error> {
        // Resume paused jobs
        let paused: Vec<String> = ctx.result.metadata["paused_jobs"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        for job in paused {
            resume_job(&job)?;
        }
        Ok(())
    }
}
```

#### 3. Throttle Ingestion

```rust
pub struct ThrottleIngestion;

impl RemediationStrategy for ThrottleIngestion {
    fn name(&self) -> &str { "throttle_ingestion" }

    fn handles(&self) -> Vec<ProblemClass> {
        vec![
            ProblemClass::HotspotCongestion { .. },
            ProblemClass::MemoryPressure { .. },
        ]
    }

    fn impact(&self) -> f32 { 0.4 }  // Medium-high (affects writes)

    fn reversible(&self) -> bool { true }

    fn execute(&self, ctx: &RemediationContext) -> Result<RemediationResult, Error> {
        // Calculate throttle percentage based on severity
        let throttle_pct = match ctx.state {
            IntegrityState::Stress => 50,
            IntegrityState::Critical => 90,
            _ => return Ok(RemediationResult::noop()),
        };

        // Apply throttle via shared memory
        set_throttle_percentage(ctx.collection_id, "insert", throttle_pct)?;
        set_throttle_percentage(ctx.collection_id, "bulk_insert", throttle_pct + 10)?;

        // Record for rollback
        ctx.record_action(ThrottleAction {
            collection_id: ctx.collection_id,
            previous_throttle: get_current_throttle(ctx.collection_id),
            new_throttle: throttle_pct,
        });

        Ok(RemediationResult {
            success: true,
            actions_taken: 1,
            improvement: 0.0,  // Preventive, not curative
            metadata: json!({ "throttle_pct": throttle_pct }),
        })
    }

    fn rollback(&self, ctx: &RemediationContext) -> Result<(), Error> {
        // Restore previous throttle level
        let action: ThrottleAction = ctx.get_action()?;
        set_throttle_percentage(ctx.collection_id, "insert", action.previous_throttle)?;
        Ok(())
    }
}
```

#### 4. Scale Read Replicas (Kubernetes)

```rust
pub struct ScaleReadReplicas;

impl RemediationStrategy for ScaleReadReplicas {
    fn name(&self) -> &str { "scale_replicas" }

    fn handles(&self) -> Vec<ProblemClass> {
        vec![ProblemClass::HotspotCongestion { .. }]
    }

    fn impact(&self) -> f32 { 0.2 }  // Low impact

    fn reversible(&self) -> bool { true }

    fn execute(&self, ctx: &RemediationContext) -> Result<RemediationResult, Error> {
        // Only available in K8s environment
        let k8s = K8sClient::try_new()?;

        // Get current replica count
        let deployment = k8s.get_deployment("ruvector-read")?;
        let current = deployment.spec.replicas;

        // Scale up by 50% (capped)
        let new_count = (current as f32 * 1.5).ceil() as i32;
        let new_count = new_count.min(ctx.config.max_replicas);

        if new_count == current {
            return Ok(RemediationResult::noop());
        }

        // Apply scale
        k8s.scale_deployment("ruvector-read", new_count)?;

        // Wait for pods to be ready
        k8s.wait_for_ready("ruvector-read", Duration::from_secs(300))?;

        Ok(RemediationResult {
            success: true,
            actions_taken: 1,
            improvement: 0.0,  // Measured later
            metadata: json!({
                "previous_replicas": current,
                "new_replicas": new_count,
            }),
        })
    }

    fn rollback(&self, ctx: &RemediationContext) -> Result<(), Error> {
        let k8s = K8sClient::try_new()?;
        let previous: i32 = ctx.result.metadata["previous_replicas"].as_i64()? as i32;
        k8s.scale_deployment("ruvector-read", previous)?;
        Ok(())
    }
}
```

#### 5. Compact Fragmented Indexes

```rust
pub struct CompactFragmentedIndexes;

impl RemediationStrategy for CompactFragmentedIndexes {
    fn name(&self) -> &str { "compact_indexes" }

    fn handles(&self) -> Vec<ProblemClass> {
        vec![ProblemClass::IndexFragmentation { .. }]
    }

    fn impact(&self) -> f32 { 0.5 }  // Higher impact (CPU intensive)

    fn reversible(&self) -> bool { false }  // Compaction is one-way

    fn execute(&self, ctx: &RemediationContext) -> Result<RemediationResult, Error> {
        let problem = ctx.problem.as_fragmentation()?;

        // Compact most fragmented indexes first
        let mut compacted = 0;
        for index_id in &problem.index_ids {
            // Check if we have time/resources
            if ctx.elapsed() > ctx.timeout / 2 {
                break;
            }

            // Run incremental compaction
            compact_index_incremental(*index_id, CompactConfig {
                max_duration: Duration::from_secs(60),
                batch_size: 10000,
            })?;

            compacted += 1;

            // Check improvement
            let new_lambda = sample_integrity(ctx.collection_id)?;
            if new_lambda > ctx.target_lambda {
                break;  // Good enough
            }
        }

        Ok(RemediationResult {
            success: compacted > 0,
            actions_taken: compacted,
            improvement: compute_improvement(ctx),
        })
    }

    fn rollback(&self, _ctx: &RemediationContext) -> Result<(), Error> {
        // Compaction is not reversible
        Err(Error::NotReversible)
    }
}
```

---

## Remediation Engine

### Execution Flow

```rust
// src/healing/engine.rs

pub struct RemediationEngine {
    registry: StrategyRegistry,
    config: HealingConfig,
    outcome_tracker: OutcomeTracker,
}

impl RemediationEngine {
    /// Main healing loop (called when integrity degrades)
    pub fn heal(&self, trigger: &IntegrityTrigger) -> HealingOutcome {
        let ctx = RemediationContext::new(trigger);

        // 1. Classify the problem
        let problem = classify_problem(&trigger.witness_edges, &ctx.metrics);
        log_problem(&problem);

        // 2. Check if we should auto-heal
        if !self.should_auto_heal(&problem, &ctx) {
            return HealingOutcome::Deferred {
                reason: "Problem requires human review",
                problem: problem.clone(),
            };
        }

        // 3. Select strategy
        let strategy = match self.registry.select(&problem, &ctx.system) {
            Some(s) => s,
            None => {
                return HealingOutcome::NoStrategy {
                    problem: problem.clone(),
                };
            }
        };

        log_strategy_selected(strategy.name(), &problem);

        // 4. Execute with timeout and monitoring
        let result = self.execute_with_safeguards(strategy, &ctx);

        // 5. Verify improvement
        let verified = self.verify_improvement(&ctx, &result);

        // 6. Rollback if needed
        if !verified && strategy.reversible() {
            log_rollback(strategy.name());
            if let Err(e) = strategy.rollback(&ctx) {
                log_rollback_failed(e);
            }
        }

        // 7. Record outcome for learning
        self.outcome_tracker.record(OutcomeRecord {
            problem: problem.clone(),
            strategy: strategy.name().to_string(),
            result: result.clone(),
            verified,
            timestamp: Utc::now(),
        });

        HealingOutcome::Completed {
            problem,
            strategy: strategy.name().to_string(),
            result,
            verified,
        }
    }

    fn should_auto_heal(&self, problem: &ProblemClass, ctx: &RemediationContext) -> bool {
        // Don't auto-heal Unknown problems
        if matches!(problem, ProblemClass::Unknown { .. }) {
            return false;
        }

        // Check cooldown
        if ctx.last_healing_attempt.elapsed() < self.config.min_healing_interval {
            return false;
        }

        // Check max attempts
        if ctx.healing_attempts_in_window > self.config.max_attempts_per_window {
            return false;
        }

        // Check if problem is getting worse despite healing
        if self.is_healing_ineffective(ctx) {
            return false;
        }

        true
    }

    fn execute_with_safeguards(
        &self,
        strategy: &dyn RemediationStrategy,
        ctx: &RemediationContext,
    ) -> RemediationResult {
        // Set up timeout
        let timeout = strategy.estimated_duration() * 2;

        // Execute in separate thread with panic catching
        let result = std::panic::catch_unwind(|| {
            tokio::time::timeout(timeout, async {
                strategy.execute(ctx)
            })
        });

        match result {
            Ok(Ok(Ok(r))) => r,
            Ok(Ok(Err(e))) => RemediationResult::failed(e.to_string()),
            Ok(Err(_)) => RemediationResult::failed("Timeout"),
            Err(_) => RemediationResult::failed("Panic during remediation"),
        }
    }

    fn verify_improvement(&self, ctx: &RemediationContext, result: &RemediationResult) -> bool {
        if !result.success {
            return false;
        }

        // Wait for system to stabilize
        std::thread::sleep(Duration::from_secs(10));

        // Sample integrity
        let new_lambda = sample_integrity(ctx.collection_id).unwrap_or(0.0);

        // Must improve by at least 10%
        new_lambda > ctx.initial_lambda * 1.1
    }
}
```

### Safety Limits

```rust
// src/healing/config.rs

pub struct HealingConfig {
    /// Minimum time between healing attempts
    pub min_healing_interval: Duration,

    /// Maximum attempts per time window
    pub max_attempts_per_window: usize,

    /// Time window for attempt counting
    pub attempt_window: Duration,

    /// Maximum impact level for auto-healing
    pub max_auto_heal_impact: f32,

    /// Problems that require human approval
    pub require_approval: Vec<ProblemClass>,

    /// Strategies that require human approval
    pub require_approval_strategies: Vec<String>,

    /// Enable learning from outcomes
    pub learning_enabled: bool,
}

impl Default for HealingConfig {
    fn default() -> Self {
        Self {
            min_healing_interval: Duration::from_secs(300),  // 5 min
            max_attempts_per_window: 3,
            attempt_window: Duration::from_secs(3600),       // 1 hour
            max_auto_heal_impact: 0.5,
            require_approval: vec![],
            require_approval_strategies: vec!["scale_replicas".to_string()],
            learning_enabled: true,
        }
    }
}
```

---

## Learning from Outcomes

### Outcome Tracking

```sql
CREATE TABLE ruvector.healing_outcomes (
    id              BIGSERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL,
    tenant_id       TEXT,

    -- Problem
    problem_class   TEXT NOT NULL,
    problem_details JSONB NOT NULL,

    -- Strategy
    strategy_name   TEXT NOT NULL,
    strategy_params JSONB,

    -- Execution
    started_at      TIMESTAMPTZ NOT NULL,
    completed_at    TIMESTAMPTZ,
    duration_ms     INTEGER,

    -- Result
    success         BOOLEAN NOT NULL,
    verified        BOOLEAN,
    actions_taken   INTEGER,
    improvement_pct REAL,
    error_message   TEXT,

    -- Context
    initial_lambda  REAL NOT NULL,
    final_lambda    REAL,
    witness_edges   JSONB,
    system_metrics  JSONB,

    -- Learning
    feedback_score  REAL,  -- Human feedback if provided

    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_healing_outcomes_class ON ruvector.healing_outcomes(problem_class);
CREATE INDEX idx_healing_outcomes_strategy ON ruvector.healing_outcomes(strategy_name);
CREATE INDEX idx_healing_outcomes_success ON ruvector.healing_outcomes(success, verified);
```

### Strategy Weight Updates

```rust
// src/healing/learning.rs

pub struct OutcomeTracker {
    db: DbPool,
    strategy_weights: RwLock<HashMap<String, f32>>,
}

impl OutcomeTracker {
    /// Update strategy weights based on outcomes
    pub fn update_weights(&self) {
        let outcomes = self.get_recent_outcomes(Duration::from_days(30));

        let mut new_weights = HashMap::new();

        for strategy in self.list_strategies() {
            let strategy_outcomes: Vec<_> = outcomes.iter()
                .filter(|o| o.strategy_name == strategy)
                .collect();

            if strategy_outcomes.is_empty() {
                continue;
            }

            // Calculate effectiveness
            let success_rate = strategy_outcomes.iter()
                .filter(|o| o.success && o.verified.unwrap_or(false))
                .count() as f32 / strategy_outcomes.len() as f32;

            let avg_improvement = strategy_outcomes.iter()
                .filter_map(|o| o.improvement_pct)
                .sum::<f32>() / strategy_outcomes.len() as f32;

            // Weight = success_rate * (1 + avg_improvement)
            let weight = success_rate * (1.0 + avg_improvement);
            new_weights.insert(strategy, weight);
        }

        *self.strategy_weights.write() = new_weights;
    }

    /// Get effectiveness report
    pub fn effectiveness_report(&self) -> EffectivenessReport {
        let weights = self.strategy_weights.read();

        EffectivenessReport {
            strategies: weights.iter()
                .map(|(name, weight)| StrategyEffectiveness {
                    name: name.clone(),
                    weight: *weight,
                    recent_outcomes: self.get_recent_outcomes_for(name, 10),
                })
                .collect(),
            overall_success_rate: self.compute_overall_success_rate(),
            avg_time_to_recovery: self.compute_avg_recovery_time(),
        }
    }
}
```

---

## SQL Interface

### Monitoring

```sql
-- View healing status
SELECT * FROM ruvector_healing_status();

-- Returns:
-- {
--   "enabled": true,
--   "last_healing": "2024-01-15T10:30:00Z",
--   "total_healings_24h": 3,
--   "success_rate": 0.67,
--   "active_remediations": [],
--   "cooldown_until": null
-- }

-- View recent healing events
SELECT * FROM ruvector_healing_history(
    since := NOW() - INTERVAL '24 hours',
    limit_ := 20
);

-- View strategy effectiveness
SELECT * FROM ruvector_healing_effectiveness();
```

### Configuration

```sql
-- Configure healing behavior
SELECT ruvector_healing_configure('{
    "enabled": true,
    "min_healing_interval_seconds": 300,
    "max_attempts_per_hour": 3,
    "max_auto_heal_impact": 0.5,
    "require_approval_strategies": ["scale_replicas"],
    "learning_enabled": true
}'::jsonb);

-- Manually trigger healing (for testing)
SELECT ruvector_healing_trigger('embeddings');

-- Approve pending healing action
SELECT ruvector_healing_approve(action_id := 123);

-- Abort active healing
SELECT ruvector_healing_abort(action_id := 123);
```

### Manual Remediation

```sql
-- Execute specific strategy manually
SELECT ruvector_healing_execute('embeddings', 'rebalance_partitions', '{
    "dry_run": false,
    "max_moves": 100
}'::jsonb);

-- Rollback last healing action
SELECT ruvector_healing_rollback('embeddings');
```

---

## Prometheus Metrics

```
# Healing activity
ruvector_healing_attempts_total{collection="embeddings",strategy="rebalance"} 15
ruvector_healing_success_total{collection="embeddings",strategy="rebalance"} 12
ruvector_healing_duration_seconds{collection="embeddings",strategy="rebalance",quantile="0.99"} 45.2

# Current state
ruvector_healing_active{collection="embeddings"} 0
ruvector_healing_cooldown{collection="embeddings"} 0

# Effectiveness
ruvector_healing_success_rate{collection="embeddings"} 0.80
ruvector_healing_avg_improvement{collection="embeddings"} 0.25
ruvector_healing_time_to_recovery_seconds{collection="embeddings"} 120
```

---

## Alerting Integration

```yaml
# Alert when healing is failing
- alert: RuVectorHealingIneffective
  expr: ruvector_healing_success_rate < 0.5
  for: 1h
  labels:
    severity: warning
  annotations:
    summary: "Self-healing is not effective"
    description: "Healing success rate is {{ $value }} - human intervention may be required"

# Alert when healing is disabled
- alert: RuVectorHealingDisabled
  expr: ruvector_healing_enabled == 0
  for: 5m
  labels:
    severity: info
  annotations:
    summary: "Self-healing is disabled for {{ $labels.collection }}"

# Alert when in prolonged degraded state
- alert: RuVectorProlongedDegradation
  expr: ruvector_integrity_state > 1 and ruvector_healing_attempts_total == 0
  for: 30m
  labels:
    severity: critical
  annotations:
    summary: "Prolonged degradation without healing attempts"
```

---

## Testing Requirements

### Unit Tests
- Problem classification accuracy
- Strategy selection logic
- Rollback correctness
- Weight update calculations

### Integration Tests
- End-to-end healing cycle
- Concurrent healing prevention
- Timeout and panic handling
- Kubernetes scaling (mock)

### Chaos Tests
- Random failure injection
- Healing under load
- Cascading failure prevention
- Recovery time objectives

---

## The Complete Loop

```
       +----------------+
       |  Normal State  |
       +-------+--------+
               |
               | (stress detected)
               v
       +-------+--------+
       |    Classify    |
       +-------+--------+
               |
               v
       +-------+--------+
       |    Remediate   |
       +-------+--------+
               |
               | (verify)
               v
       +-------+--------+
       |     Learn      |
       +-------+--------+
               |
               v
       +-------+--------+
       |  Normal State  |<----+
       +----------------+     |
                              |
                        (automatic recovery)
```

**This is what makes RuVector truly different:**

We don't just detect problems early. We **fix them automatically** before they become incidents.

The system is not just observable. It is **self-aware and self-repairing**.
