# PostgreSQL Integration Specification

## Overview

This document specifies how the Neural DAG system integrates with PostgreSQL via the pgrx framework, including type definitions, operators, functions, and background workers.

## Module Structure

```
crates/ruvector-postgres/src/dag/
├── mod.rs                    # Module root
├── operators.rs              # SQL function definitions
├── types.rs                  # PostgreSQL type mappings
├── hooks.rs                  # Query execution hooks
├── worker.rs                 # Background worker
├── gucs.rs                   # GUC variable definitions
└── state.rs                  # Per-connection state
```

## GUC Variables

### Definition

```rust
// crates/ruvector-postgres/src/dag/gucs.rs

use pgrx::prelude::*;
use std::ffi::CStr;

// Global enable/disable
static NEURAL_DAG_ENABLED: GucSetting<bool> = GucSetting::new(true);

// Learning parameters
static DAG_LEARNING_RATE: GucSetting<f64> = GucSetting::new(0.002);
static DAG_PATTERN_CLUSTERS: GucSetting<i32> = GucSetting::new(100);
static DAG_QUALITY_THRESHOLD: GucSetting<f64> = GucSetting::new(0.3);
static DAG_MAX_TRAJECTORIES: GucSetting<i32> = GucSetting::new(10000);

// Attention parameters
static DAG_ATTENTION_TYPE: GucSetting<&'static CStr> =
    GucSetting::new(unsafe { CStr::from_bytes_with_nul_unchecked(b"auto\0") });
static DAG_ATTENTION_EXPLORATION: GucSetting<f64> = GucSetting::new(0.1);

// EWC parameters
static DAG_EWC_LAMBDA: GucSetting<f64> = GucSetting::new(2000.0);
static DAG_EWC_MAX_LAMBDA: GucSetting<f64> = GucSetting::new(15000.0);

// MinCut parameters
static DAG_MINCUT_ENABLED: GucSetting<bool> = GucSetting::new(true);
static DAG_MINCUT_THRESHOLD: GucSetting<f64> = GucSetting::new(0.5);

// Background worker parameters
static DAG_BACKGROUND_INTERVAL_MS: GucSetting<i32> = GucSetting::new(3600000);

pub fn register_gucs() {
    GucRegistry::define_bool_guc(
        "ruvector.neural_dag_enabled",
        "Enable neural DAG optimization",
        "When enabled, queries are optimized using learned patterns",
        &NEURAL_DAG_ENABLED,
        GucContext::Userset,
        GucFlags::default(),
    );

    GucRegistry::define_float_guc(
        "ruvector.dag_learning_rate",
        "Learning rate for DAG optimization",
        "Controls how fast the system adapts to new patterns",
        &DAG_LEARNING_RATE,
        0.0001,
        1.0,
        GucContext::Userset,
        GucFlags::default(),
    );

    GucRegistry::define_int_guc(
        "ruvector.dag_pattern_clusters",
        "Number of pattern clusters",
        "K-means clusters for pattern extraction",
        &DAG_PATTERN_CLUSTERS,
        10,
        1000,
        GucContext::Userset,
        GucFlags::default(),
    );

    GucRegistry::define_float_guc(
        "ruvector.dag_quality_threshold",
        "Minimum quality for learning",
        "Trajectories below this quality are not learned",
        &DAG_QUALITY_THRESHOLD,
        0.0,
        1.0,
        GucContext::Userset,
        GucFlags::default(),
    );

    GucRegistry::define_string_guc(
        "ruvector.dag_attention_type",
        "Default attention type",
        "Options: auto, topological, causal_cone, critical_path, mincut_gated, hierarchical_lorentz, parallel_branch, temporal_btsp",
        &DAG_ATTENTION_TYPE,
        GucContext::Userset,
        GucFlags::default(),
    );

    GucRegistry::define_float_guc(
        "ruvector.dag_ewc_lambda",
        "EWC regularization strength",
        "Higher values prevent forgetting more aggressively",
        &DAG_EWC_LAMBDA,
        100.0,
        50000.0,
        GucContext::Userset,
        GucFlags::default(),
    );

    GucRegistry::define_bool_guc(
        "ruvector.dag_mincut_enabled",
        "Enable MinCut analysis",
        "When enabled, bottleneck operators are identified",
        &DAG_MINCUT_ENABLED,
        GucContext::Userset,
        GucFlags::default(),
    );

    GucRegistry::define_int_guc(
        "ruvector.dag_background_interval_ms",
        "Background learning interval (milliseconds)",
        "How often the background learning cycle runs",
        &DAG_BACKGROUND_INTERVAL_MS,
        60000,     // 1 minute minimum
        86400000,  // 24 hours maximum
        GucContext::Sighup,
        GucFlags::default(),
    );
}

/// Get current configuration from GUCs
pub fn get_config() -> DagSonaConfig {
    DagSonaConfig {
        enabled: NEURAL_DAG_ENABLED.get(),
        learning_rate: DAG_LEARNING_RATE.get() as f32,
        pattern_clusters: DAG_PATTERN_CLUSTERS.get() as usize,
        quality_threshold: DAG_QUALITY_THRESHOLD.get() as f32,
        max_trajectories: DAG_MAX_TRAJECTORIES.get() as usize,
        attention_type: parse_attention_type(DAG_ATTENTION_TYPE.get()),
        attention_exploration: DAG_ATTENTION_EXPLORATION.get() as f32,
        ewc_lambda: DAG_EWC_LAMBDA.get() as f32,
        ewc_max_lambda: DAG_EWC_MAX_LAMBDA.get() as f32,
        mincut_enabled: DAG_MINCUT_ENABLED.get(),
        mincut_threshold: DAG_MINCUT_THRESHOLD.get() as f32,
        background_interval_ms: DAG_BACKGROUND_INTERVAL_MS.get() as u64,
        ..Default::default()
    }
}
```

## State Management

### Global State

```rust
// crates/ruvector-postgres/src/dag/state.rs

use once_cell::sync::Lazy;
use dashmap::DashMap;
use std::sync::Arc;

/// Global registry of DAG engines per table
static DAG_ENGINES: Lazy<DashMap<String, Arc<DagSonaEngine>>> =
    Lazy::new(|| DashMap::new());

/// Get or create DAG engine for a table
pub fn get_or_create_engine(table_name: &str) -> Arc<DagSonaEngine> {
    DAG_ENGINES.entry(table_name.to_string())
        .or_insert_with(|| {
            let config = gucs::get_config();
            Arc::new(DagSonaEngine::new(table_name, config).unwrap())
        })
        .clone()
}

/// Check if neural DAG is enabled for a table
pub fn is_enabled(table_name: &str) -> bool {
    DAG_ENGINES.contains_key(table_name) && gucs::get_config().enabled
}

/// Remove engine (on DROP TABLE or disable)
pub fn remove_engine(table_name: &str) {
    DAG_ENGINES.remove(table_name);
}

/// Get all engine names (for monitoring)
pub fn list_engines() -> Vec<String> {
    DAG_ENGINES.iter().map(|e| e.key().clone()).collect()
}
```

### Per-Connection State

```rust
/// Per-connection state for query tracking
pub struct ConnectionState {
    /// Current query trajectory being built
    current_trajectory: Option<TrajectoryBuilder>,

    /// Query start time
    query_start: Option<Instant>,

    /// Current table being queried
    current_table: Option<String>,
}

thread_local! {
    static CONNECTION_STATE: RefCell<ConnectionState> = RefCell::new(ConnectionState {
        current_trajectory: None,
        query_start: None,
        current_table: None,
    });
}

impl ConnectionState {
    pub fn start_query(&mut self, table_name: &str) {
        self.query_start = Some(Instant::now());
        self.current_table = Some(table_name.to_string());
        self.current_trajectory = Some(TrajectoryBuilder::new());
    }

    pub fn end_query(&mut self) -> Option<DagTrajectory> {
        let builder = self.current_trajectory.take()?;
        let start = self.query_start.take()?;
        let _table = self.current_table.take()?;

        Some(builder.build(start.elapsed()))
    }
}
```

## SQL Function Implementations

### Enable/Disable Functions

```rust
// crates/ruvector-postgres/src/dag/operators.rs

use pgrx::prelude::*;

/// Enable neural DAG learning for a table
#[pg_extern]
fn ruvector_enable_neural_dag(
    table_name: &str,
    config: Option<pgrx::JsonB>,
) -> bool {
    let base_config = gucs::get_config();

    let config = if let Some(pgrx::JsonB(json)) = config {
        // Merge JSON config with base config
        merge_config(base_config, &json)
    } else {
        base_config
    };

    // Validate table exists
    let table_oid = get_table_oid(table_name);
    if table_oid.is_none() {
        ereport!(
            PgLogLevel::ERROR,
            PgSqlErrorCode::ERRCODE_UNDEFINED_TABLE,
            format!("Table '{}' does not exist", table_name)
        );
        return false;
    }

    // Create engine
    let engine = DagSonaEngine::new(table_name, config)
        .map_err(|e| {
            ereport!(
                PgLogLevel::ERROR,
                PgSqlErrorCode::ERRCODE_INTERNAL_ERROR,
                format!("Failed to create DAG engine: {}", e)
            );
        })
        .unwrap();

    // Register engine
    DAG_ENGINES.insert(table_name.to_string(), Arc::new(engine));

    // Start background worker if not running
    start_background_worker_if_needed();

    true
}

/// Disable neural DAG learning for a table
#[pg_extern]
fn ruvector_disable_neural_dag(table_name: &str) -> bool {
    if DAG_ENGINES.remove(table_name).is_some() {
        true
    } else {
        warning!("Neural DAG was not enabled for table '{}'", table_name);
        false
    }
}

/// Check if neural DAG is enabled
#[pg_extern]
fn ruvector_neural_dag_enabled(table_name: &str) -> bool {
    state::is_enabled(table_name)
}
```

### Pattern Functions

```rust
/// Get learned patterns for a table
#[pg_extern]
fn ruvector_dag_patterns(
    table_name: &str,
) -> TableIterator<'static, (
    name!(pattern_id, i64),
    name!(centroid, Vec<f32>),
    name!(attention_type, String),
    name!(confidence, f32),
    name!(sample_count, i32),
    name!(avg_latency_us, f64),
    name!(avg_quality, f64),
)> {
    let engine = match DAG_ENGINES.get(table_name) {
        Some(e) => e.clone(),
        None => {
            warning!("Neural DAG not enabled for table '{}'", table_name);
            return TableIterator::new(vec![].into_iter());
        }
    };

    let patterns: Vec<_> = {
        let bank = engine.dag_reasoning_bank.read();
        bank.patterns.iter()
            .map(|entry| {
                let p = entry.value();
                (
                    p.id as i64,
                    p.centroid.clone(),
                    format!("{:?}", p.optimal_attention),
                    p.confidence,
                    p.sample_count as i32,
                    p.avg_metrics.latency_us,
                    p.avg_metrics.quality,
                )
            })
            .collect()
    };

    TableIterator::new(patterns.into_iter())
}

/// Force pattern extraction
#[pg_extern]
fn ruvector_dag_extract_patterns(
    table_name: &str,
    num_clusters: Option<i32>,
) -> i32 {
    let engine = match DAG_ENGINES.get(table_name) {
        Some(e) => e.clone(),
        None => {
            ereport!(
                PgLogLevel::ERROR,
                PgSqlErrorCode::ERRCODE_INVALID_PARAMETER_VALUE,
                format!("Neural DAG not enabled for table '{}'", table_name)
            );
            return 0;
        }
    };

    // Override cluster count if specified
    if let Some(k) = num_clusters {
        // Temporary config override
    }

    match engine.run_background_cycle() {
        Ok(BackgroundCycleResult::Completed { patterns_extracted, .. }) => {
            patterns_extracted as i32
        }
        Ok(BackgroundCycleResult::Skipped { reason, .. }) => {
            warning!("Pattern extraction skipped: {}", reason);
            0
        }
        Err(e) => {
            ereport!(
                PgLogLevel::ERROR,
                PgSqlErrorCode::ERRCODE_INTERNAL_ERROR,
                format!("Pattern extraction failed: {}", e)
            );
            0
        }
    }
}

/// Consolidate similar patterns
#[pg_extern]
fn ruvector_dag_consolidate_patterns(
    table_name: &str,
    similarity_threshold: Option<f32>,
) -> i32 {
    let engine = match DAG_ENGINES.get(table_name) {
        Some(e) => e.clone(),
        None => {
            warning!("Neural DAG not enabled for table '{}'", table_name);
            return 0;
        }
    };

    let threshold = similarity_threshold.unwrap_or(0.95);

    let before_count = {
        let bank = engine.dag_reasoning_bank.read();
        bank.patterns.len()
    };

    {
        let mut bank = engine.dag_reasoning_bank.write();
        bank.consolidate(threshold);
    }

    let after_count = {
        let bank = engine.dag_reasoning_bank.read();
        bank.patterns.len()
    };

    (before_count - after_count) as i32
}
```

### Attention Functions

```rust
/// Compute topological attention
#[pg_extern(immutable, parallel_safe)]
fn ruvector_attention_topological(
    query: Vec<f32>,
    ancestors: Vec<Vec<f32>>,
    config: Option<pgrx::JsonB>,
) -> Vec<f32> {
    let config = parse_attention_config(config);
    let attention = TopologicalAttention::new(config);

    let ctx = build_simple_context(&ancestors);
    let query_node = DagNode::from_embedding(query);

    match attention.forward(&query_node, &ctx, &config) {
        Ok(output) => output.weights,
        Err(e) => {
            warning!("Attention computation failed: {}", e);
            vec![1.0 / ancestors.len() as f32; ancestors.len()]  // Uniform fallback
        }
    }
}

/// Compute causal cone attention
#[pg_extern(immutable, parallel_safe)]
fn ruvector_attention_causal_cone(
    query: Vec<f32>,
    ancestors: Vec<Vec<f32>>,
    depths: Vec<i32>,
    decay_rate: f32,
    max_depth: i32,
) -> Vec<f32> {
    let config = AttentionConfig {
        hidden_dim: query.len(),
        ..Default::default()
    };

    let attention = CausalConeAttention::new(
        config.hidden_dim,
        8,  // num_heads
        decay_rate,
        max_depth as usize,
    );

    let ctx = build_context_with_depths(&ancestors, &depths);
    let query_node = DagNode::from_embedding(query);

    match attention.forward(&query_node, &ctx, &config) {
        Ok(output) => output.weights,
        Err(e) => {
            warning!("Causal cone attention failed: {}", e);
            vec![1.0 / ancestors.len() as f32; ancestors.len()]
        }
    }
}

/// Compute critical path attention
#[pg_extern(immutable, parallel_safe)]
fn ruvector_attention_critical_path(
    query: Vec<f32>,
    ancestors: Vec<Vec<f32>>,
    is_critical: Vec<bool>,
    boost: f32,
) -> Vec<f32> {
    let config = AttentionConfig {
        hidden_dim: query.len(),
        ..Default::default()
    };

    let attention = CriticalPathAttention::new(
        config.hidden_dim,
        8,
        boost,
    );

    let ctx = build_context_with_critical(&ancestors, &is_critical);
    let query_node = DagNode::from_embedding(query);

    match attention.forward(&query_node, &ctx, &config) {
        Ok(output) => output.weights,
        Err(e) => {
            warning!("Critical path attention failed: {}", e);
            vec![1.0 / ancestors.len() as f32; ancestors.len()]
        }
    }
}

/// Compute mincut gated attention
#[pg_extern(immutable, parallel_safe)]
fn ruvector_attention_mincut_gated(
    query: Vec<f32>,
    ancestors: Vec<Vec<f32>>,
    criticalities: Vec<f32>,
    threshold: f32,
) -> Vec<f32> {
    let config = AttentionConfig {
        hidden_dim: query.len(),
        ..Default::default()
    };

    let attention = MinCutGatedAttention::new(
        config.hidden_dim,
        8,
        threshold,
    );

    let ctx = build_context_with_criticalities(&ancestors, &criticalities);
    let query_node = DagNode::from_embedding(query);

    match attention.forward(&query_node, &ctx, &config) {
        Ok(output) => output.weights,
        Err(e) => {
            warning!("MinCut gated attention failed: {}", e);
            vec![1.0 / ancestors.len() as f32; ancestors.len()]
        }
    }
}

/// Compute hierarchical Lorentz attention
#[pg_extern(immutable, parallel_safe)]
fn ruvector_attention_hierarchical_lorentz(
    query: Vec<f32>,
    ancestors: Vec<Vec<f32>>,
    depths: Vec<i32>,
    curvature: f32,
    temperature: f32,
) -> Vec<f32> {
    let config = AttentionConfig {
        hidden_dim: query.len(),
        ..Default::default()
    };

    let attention = HierarchicalLorentzAttention::new(
        config.hidden_dim,
        8,
        curvature,
        temperature,
    );

    let ctx = build_context_with_depths(&ancestors, &depths);
    let query_node = DagNode::from_embedding(query);

    match attention.forward(&query_node, &ctx, &config) {
        Ok(output) => output.weights,
        Err(e) => {
            warning!("Hierarchical Lorentz attention failed: {}", e);
            vec![1.0 / ancestors.len() as f32; ancestors.len()]
        }
    }
}

/// Compute parallel branch attention
#[pg_extern(immutable, parallel_safe)]
fn ruvector_attention_parallel_branch(
    query: Vec<f32>,
    ancestors: Vec<Vec<f32>>,
    parallel_nodes: Vec<Vec<f32>>,
    common_ancestor_mask: Vec<bool>,
    cross_weight: f32,
    common_boost: f32,
) -> Vec<f32> {
    let config = AttentionConfig {
        hidden_dim: query.len(),
        ..Default::default()
    };

    let attention = ParallelBranchAttention::new(
        config.hidden_dim,
        8,
        cross_weight,
        common_boost,
    );

    let ctx = build_context_with_parallel(
        &ancestors,
        &parallel_nodes,
        &common_ancestor_mask,
    );
    let query_node = DagNode::from_embedding(query);

    match attention.forward(&query_node, &ctx, &config) {
        Ok(output) => output.weights,
        Err(e) => {
            warning!("Parallel branch attention failed: {}", e);
            let total = ancestors.len() + parallel_nodes.len();
            vec![1.0 / total as f32; total]
        }
    }
}

/// Compute temporal BTSP attention
#[pg_extern(immutable, parallel_safe)]
fn ruvector_attention_temporal_btsp(
    query: Vec<f32>,
    ancestors: Vec<Vec<f32>>,
    timestamps: Vec<f64>,
    window_ms: f32,
    boost: f32,
) -> Vec<f32> {
    let config = AttentionConfig {
        hidden_dim: query.len(),
        ..Default::default()
    };

    let attention = TemporalBTSPAttention::new(
        config.hidden_dim,
        8,
        window_ms,
        boost,
    );

    let ctx = build_context_with_timestamps(&ancestors, &timestamps);
    let query_node = DagNode::from_embedding(query);

    match attention.forward(&query_node, &ctx, &config) {
        Ok(output) => output.weights,
        Err(e) => {
            warning!("Temporal BTSP attention failed: {}", e);
            vec![1.0 / ancestors.len() as f32; ancestors.len()]
        }
    }
}

/// Compute ensemble attention
#[pg_extern(immutable, parallel_safe)]
fn ruvector_attention_ensemble(
    query: Vec<f32>,
    ancestors: Vec<Vec<f32>>,
    attention_types: Vec<String>,
    weights: Option<Vec<f32>>,
) -> Vec<f32> {
    let config = AttentionConfig {
        hidden_dim: query.len(),
        ..Default::default()
    };

    let types: Vec<DagAttentionType> = attention_types.iter()
        .filter_map(|s| parse_attention_type_str(s))
        .collect();

    let weights = weights.unwrap_or_else(|| {
        vec![1.0 / types.len() as f32; types.len()]
    });

    let attention = EnsembleAttention::new(types, weights);

    let ctx = build_simple_context(&ancestors);
    let query_node = DagNode::from_embedding(query);

    match attention.forward(&query_node, &ctx, &config) {
        Ok(output) => output.weights,
        Err(e) => {
            warning!("Ensemble attention failed: {}", e);
            vec![1.0 / ancestors.len() as f32; ancestors.len()]
        }
    }
}
```

### MinCut Functions

```rust
/// Compute mincut criticality for DAG nodes
#[pg_extern]
fn ruvector_dag_mincut_criticality(
    table_name: &str,
) -> TableIterator<'static, (
    name!(node_id, i64),
    name!(criticality, f32),
    name!(is_bottleneck, bool),
)> {
    let engine = match DAG_ENGINES.get(table_name) {
        Some(e) => e.clone(),
        None => {
            warning!("Neural DAG not enabled for table '{}'", table_name);
            return TableIterator::new(vec![].into_iter());
        }
    };

    let mincut = match &engine.mincut_engine {
        Some(m) => m.clone(),
        None => {
            warning!("MinCut not enabled for table '{}'", table_name);
            return TableIterator::new(vec![].into_iter());
        }
    };

    let threshold = gucs::get_config().mincut_threshold;

    // Compute criticalities
    let criticalities = mincut.compute_all_criticalities();

    let results: Vec<_> = criticalities.into_iter()
        .map(|(node_id, crit)| {
            (node_id as i64, crit, crit > threshold)
        })
        .collect();

    TableIterator::new(results.into_iter())
}

/// Analyze DAG bottlenecks
#[pg_extern]
fn ruvector_dag_mincut_analysis(
    table_name: &str,
) -> pgrx::JsonB {
    let engine = match DAG_ENGINES.get(table_name) {
        Some(e) => e.clone(),
        None => {
            return pgrx::JsonB(json!({
                "error": "Neural DAG not enabled"
            }));
        }
    };

    let mincut = match &engine.mincut_engine {
        Some(m) => m.clone(),
        None => {
            return pgrx::JsonB(json!({
                "error": "MinCut not enabled"
            }));
        }
    };

    let global_cut = mincut.query();
    let criticalities = mincut.compute_all_criticalities();

    let threshold = gucs::get_config().mincut_threshold;
    let bottlenecks: Vec<_> = criticalities.iter()
        .filter(|(_, c)| *c > threshold)
        .map(|(id, c)| json!({
            "node_id": *id,
            "criticality": *c,
        }))
        .collect();

    pgrx::JsonB(json!({
        "global_mincut": global_cut,
        "bottleneck_count": bottlenecks.len(),
        "bottlenecks": bottlenecks,
        "threshold": threshold,
    }))
}
```

### Monitoring Functions

```rust
/// Get learning statistics
#[pg_extern]
fn ruvector_dag_learning_stats(table_name: &str) -> pgrx::JsonB {
    let engine = match DAG_ENGINES.get(table_name) {
        Some(e) => e.clone(),
        None => {
            return pgrx::JsonB(json!({
                "enabled": false,
                "error": "Neural DAG not enabled"
            }));
        }
    };

    let metrics = engine.metrics.to_json();
    let buffer_stats = engine.dag_trajectory_buffer.stats();
    let pattern_count = {
        let bank = engine.dag_reasoning_bank.read();
        bank.patterns.len()
    };

    pgrx::JsonB(json!({
        "enabled": true,
        "queries_processed": metrics["queries_processed"],
        "pattern_hit_rate": metrics["pattern_hit_rate"],
        "avg_latency_us": metrics["avg_latency_us"],
        "patterns_stored": pattern_count,
        "trajectories_buffered": buffer_stats.current_size,
        "trajectories_dropped": buffer_stats.dropped,
        "background_cycles": metrics["background_cycles"],
        "ewc_tasks": metrics["ewc_tasks"],
    }))
}

/// Get health report
#[pg_extern]
fn ruvector_dag_health_report(table_name: &str) -> pgrx::JsonB {
    let engine = match DAG_ENGINES.get(table_name) {
        Some(e) => e.clone(),
        None => {
            return pgrx::JsonB(json!({
                "healthy": false,
                "error": "Neural DAG not enabled"
            }));
        }
    };

    let metrics = engine.metrics.to_json();
    let buffer_stats = engine.dag_trajectory_buffer.stats();

    // Health checks
    let mut issues = Vec::new();

    // Check drop rate
    let drop_rate = if buffer_stats.total_seen > 0 {
        buffer_stats.dropped as f64 / buffer_stats.total_seen as f64
    } else {
        0.0
    };
    if drop_rate > 0.1 {
        issues.push(format!("High trajectory drop rate: {:.1}%", drop_rate * 100.0));
    }

    // Check pattern hit rate
    let hit_rate = metrics["pattern_hit_rate"].as_f64().unwrap_or(0.0);
    if hit_rate < 0.1 && metrics["queries_processed"].as_u64().unwrap_or(0) > 1000 {
        issues.push(format!("Low pattern hit rate: {:.1}%", hit_rate * 100.0));
    }

    // Check MinCut bottlenecks
    if let Some(ref mincut) = engine.mincut_engine {
        let criticalities = mincut.compute_all_criticalities();
        let severe_bottlenecks = criticalities.values()
            .filter(|&&c| c > 0.8)
            .count();
        if severe_bottlenecks > 0 {
            issues.push(format!("{} severe bottlenecks detected", severe_bottlenecks));
        }
    }

    pgrx::JsonB(json!({
        "healthy": issues.is_empty(),
        "issues": issues,
        "metrics": metrics,
        "buffer": {
            "size": buffer_stats.current_size,
            "capacity": buffer_stats.capacity,
            "drop_rate": drop_rate,
        },
    }))
}

/// Force a learning cycle
#[pg_extern]
fn ruvector_dag_learn(table_name: &str) -> pgrx::JsonB {
    let engine = match DAG_ENGINES.get(table_name) {
        Some(e) => e.clone(),
        None => {
            return pgrx::JsonB(json!({
                "success": false,
                "error": "Neural DAG not enabled"
            }));
        }
    };

    match engine.run_background_cycle() {
        Ok(BackgroundCycleResult::Completed {
            trajectories_processed,
            patterns_extracted,
            duration,
        }) => {
            pgrx::JsonB(json!({
                "success": true,
                "trajectories_processed": trajectories_processed,
                "patterns_extracted": patterns_extracted,
                "duration_ms": duration.as_millis(),
            }))
        }
        Ok(BackgroundCycleResult::Skipped { reason, count }) => {
            pgrx::JsonB(json!({
                "success": false,
                "skipped": true,
                "reason": reason,
                "trajectory_count": count,
            }))
        }
        Err(e) => {
            pgrx::JsonB(json!({
                "success": false,
                "error": e.to_string(),
            }))
        }
    }
}
```

## Background Worker

```rust
// crates/ruvector-postgres/src/dag/worker.rs

use pgrx::bgworkers::*;
use std::time::Duration;

#[pg_guard]
pub extern "C" fn dag_background_worker_main(_arg: pg_sys::Datum) {
    BackgroundWorker::attach_signal_handlers(SignalWakeFlags::SIGHUP | SignalWakeFlags::SIGTERM);

    log!(
        "DAG background worker started with interval {}ms",
        gucs::DAG_BACKGROUND_INTERVAL_MS.get()
    );

    while BackgroundWorker::wait_latch(Some(Duration::from_millis(
        gucs::DAG_BACKGROUND_INTERVAL_MS.get() as u64
    ))) {
        if BackgroundWorker::sighup_received() {
            // Reload configuration
            log!("DAG background worker received SIGHUP, reloading config");
        }

        // Run learning cycle for all enabled tables
        for engine_ref in DAG_ENGINES.iter() {
            let table_name = engine_ref.key();
            let engine = engine_ref.value();

            match engine.run_background_cycle() {
                Ok(BackgroundCycleResult::Completed { patterns_extracted, .. }) => {
                    log!(
                        "DAG learning cycle completed for '{}': {} patterns",
                        table_name,
                        patterns_extracted
                    );
                }
                Ok(BackgroundCycleResult::Skipped { reason, .. }) => {
                    // Normal - not enough data
                }
                Err(e) => {
                    elog!(
                        WARNING,
                        "DAG learning cycle failed for '{}': {}",
                        table_name,
                        e
                    );
                }
            }
        }
    }

    log!("DAG background worker shutting down");
}

/// Register the background worker
pub fn register_background_worker() {
    BackgroundWorkerBuilder::new("ruvector_dag_worker")
        .set_function("dag_background_worker_main")
        .set_library("ruvector")
        .set_argument(0.into())
        .enable_spi_access()
        .set_start_time(BgWorkerStartTime::RecoveryFinished)
        .set_restart_time(Some(Duration::from_secs(10)))
        .load();
}
```

## Query Execution Hooks

```rust
// crates/ruvector-postgres/src/dag/hooks.rs

use pgrx::prelude::*;

/// Hook into query execution for learning
pub fn install_query_hooks() {
    // Install planner hook
    unsafe {
        prev_planner_hook = planner_hook;
        planner_hook = Some(dag_planner_hook);
    }

    // Install executor hooks
    unsafe {
        prev_ExecutorStart_hook = ExecutorStart_hook;
        ExecutorStart_hook = Some(dag_executor_start_hook);

        prev_ExecutorEnd_hook = ExecutorEnd_hook;
        ExecutorEnd_hook = Some(dag_executor_end_hook);
    }
}

/// Planner hook - optimize query plan
#[pg_guard]
unsafe extern "C" fn dag_planner_hook(
    parse: *mut pg_sys::Query,
    query_string: *const c_char,
    cursor_options: c_int,
    bound_params: *mut pg_sys::ParamListInfoData,
) -> *mut pg_sys::PlannedStmt {
    // Call original planner
    let stmt = if let Some(prev) = prev_planner_hook {
        prev(parse, query_string, cursor_options, bound_params)
    } else {
        pg_sys::standard_planner(parse, query_string, cursor_options, bound_params)
    };

    // Check if neural DAG optimization applies
    if !gucs::NEURAL_DAG_ENABLED.get() {
        return stmt;
    }

    // Extract table name from query
    if let Some(table_name) = extract_table_name(parse) {
        if let Some(engine) = DAG_ENGINES.get(&table_name) {
            // Build neural plan wrapper
            let mut neural_plan = NeuralDagPlan::from_planned_stmt(stmt);

            // Apply learned optimizations
            let _ = engine.pre_query(&mut neural_plan);

            // Store for post-query processing
            CONNECTION_STATE.with(|state| {
                state.borrow_mut().start_query(&table_name);
            });
        }
    }

    stmt
}

/// Executor start hook - record start time
#[pg_guard]
unsafe extern "C" fn dag_executor_start_hook(
    query_desc: *mut pg_sys::QueryDesc,
    eflags: c_int,
) {
    // Call original
    if let Some(prev) = prev_ExecutorStart_hook {
        prev(query_desc, eflags);
    } else {
        pg_sys::standard_ExecutorStart(query_desc, eflags);
    }

    // Record timing
    CONNECTION_STATE.with(|state| {
        if let Some(ref mut traj) = state.borrow_mut().current_trajectory {
            traj.mark_execution_start();
        }
    });
}

/// Executor end hook - record trajectory
#[pg_guard]
unsafe extern "C" fn dag_executor_end_hook(query_desc: *mut pg_sys::QueryDesc) {
    // Collect metrics before cleanup
    let metrics = extract_execution_metrics(query_desc);

    // Call original
    if let Some(prev) = prev_ExecutorEnd_hook {
        prev(query_desc);
    } else {
        pg_sys::standard_ExecutorEnd(query_desc);
    }

    // Record trajectory
    CONNECTION_STATE.with(|state| {
        let mut state = state.borrow_mut();
        if let (Some(trajectory), Some(table_name)) = (
            state.end_query(),
            state.current_table.as_ref(),
        ) {
            if let Some(engine) = DAG_ENGINES.get(table_name) {
                engine.post_query(&trajectory.plan, metrics);
            }
        }
    });
}

fn extract_execution_metrics(query_desc: *mut pg_sys::QueryDesc) -> ExecutionMetrics {
    unsafe {
        let estate = (*query_desc).estate;

        ExecutionMetrics {
            latency_us: 0,  // Computed from timestamps
            planning_us: 0,
            execution_us: 0,
            rows_processed: (*estate).es_processed as u64,
            memory_bytes: pg_sys::MemoryContextMemAllocated(
                (*estate).es_query_cxt,
                false,
            ) as u64,
            cache_hit_rate: 0.0,  // Would need buffer stats
            max_criticality: None,
        }
    }
}
```

## Extension Initialization

```rust
// crates/ruvector-postgres/src/lib.rs (additions)

#[pg_guard]
pub extern "C" fn _PG_init() {
    // ... existing initialization ...

    // Register DAG GUCs
    dag::gucs::register_gucs();

    // Install query hooks
    dag::hooks::install_query_hooks();

    // Register background worker
    dag::worker::register_background_worker();

    pgrx::log!("RuVector Neural DAG module initialized");
}
```

## SQL Installation Script

```sql
-- Extension installation additions

-- Create schema for DAG objects
CREATE SCHEMA IF NOT EXISTS ruvector_dag;

-- Pattern storage table (optional persistence)
CREATE TABLE IF NOT EXISTS ruvector_dag.patterns (
    id BIGSERIAL PRIMARY KEY,
    table_name TEXT NOT NULL,
    pattern_id BIGINT NOT NULL,
    centroid ruvector NOT NULL,
    attention_type TEXT NOT NULL,
    optimal_params JSONB NOT NULL,
    confidence FLOAT NOT NULL,
    sample_count INT NOT NULL,
    avg_latency_us FLOAT NOT NULL,
    avg_quality FLOAT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (table_name, pattern_id)
);

-- Trajectory history table (optional)
CREATE TABLE IF NOT EXISTS ruvector_dag.trajectory_history (
    id BIGSERIAL PRIMARY KEY,
    table_name TEXT NOT NULL,
    trajectory_id BIGINT NOT NULL,
    plan_embedding ruvector NOT NULL,
    attention_type TEXT NOT NULL,
    latency_us BIGINT NOT NULL,
    quality FLOAT NOT NULL,
    recorded_at TIMESTAMPTZ DEFAULT NOW()
);

-- Index for pattern lookup
CREATE INDEX IF NOT EXISTS idx_patterns_table
ON ruvector_dag.patterns (table_name);

-- Index for trajectory analysis
CREATE INDEX IF NOT EXISTS idx_trajectories_table_time
ON ruvector_dag.trajectory_history (table_name, recorded_at DESC);

-- Cleanup old trajectories (run periodically)
CREATE OR REPLACE FUNCTION ruvector_dag.cleanup_old_trajectories(
    retention_days INT DEFAULT 7
) RETURNS INT AS $$
DECLARE
    deleted_count INT;
BEGIN
    DELETE FROM ruvector_dag.trajectory_history
    WHERE recorded_at < NOW() - (retention_days || ' days')::INTERVAL;

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;
```
