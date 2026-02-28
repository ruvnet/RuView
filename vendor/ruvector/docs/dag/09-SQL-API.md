# SQL API Reference

## Overview

Complete SQL API for the Neural DAG Learning system integrated with RuVector-Postgres.

## Configuration Functions

### System Configuration

```sql
-- Enable/disable neural DAG learning
SELECT ruvector.dag_set_enabled(enabled BOOLEAN) RETURNS VOID;

-- Configure learning rate
SELECT ruvector.dag_set_learning_rate(rate FLOAT8) RETURNS VOID;

-- Set attention mechanism
SELECT ruvector.dag_set_attention(
    mechanism TEXT  -- 'topological', 'causal_cone', 'critical_path',
                    -- 'mincut_gated', 'hierarchical_lorentz',
                    -- 'parallel_branch', 'temporal_btsp', 'auto'
) RETURNS VOID;

-- Configure SONA parameters
SELECT ruvector.dag_configure_sona(
    micro_lora_rank INT DEFAULT 2,
    base_lora_rank INT DEFAULT 8,
    ewc_lambda FLOAT8 DEFAULT 5000.0,
    pattern_clusters INT DEFAULT 100
) RETURNS VOID;

-- Set QuDAG network endpoint
SELECT ruvector.dag_set_qudag_endpoint(
    endpoint TEXT,
    stake_amount FLOAT8 DEFAULT 0.0
) RETURNS VOID;
```

### Runtime Status

```sql
-- Get current configuration
SELECT * FROM ruvector.dag_config();
-- Returns: (enabled, learning_rate, attention_mechanism,
--           micro_lora_rank, base_lora_rank, ewc_lambda, qudag_endpoint)

-- Get system status
SELECT * FROM ruvector.dag_status();
-- Returns: (active_patterns, total_trajectories, avg_improvement,
--           attention_hits, learning_rate_effective, qudag_connected)

-- Check health
SELECT * FROM ruvector.dag_health_check();
-- Returns: (component, status, last_check, message)
```

## Query Analysis Functions

### Plan Analysis

```sql
-- Analyze query plan and return neural DAG insights
SELECT * FROM ruvector.dag_analyze_plan(
    query_text TEXT
) RETURNS TABLE (
    node_id INT,
    operator_type TEXT,
    criticality FLOAT8,
    bottleneck_score FLOAT8,
    embedding VECTOR(256),
    parent_ids INT[],
    child_ids INT[],
    estimated_cost FLOAT8,
    recommendations TEXT[]
);

-- Get critical path for query
SELECT * FROM ruvector.dag_critical_path(
    query_text TEXT
) RETURNS TABLE (
    path_position INT,
    node_id INT,
    operator_type TEXT,
    accumulated_cost FLOAT8,
    attention_weight FLOAT8
);

-- Identify bottlenecks
SELECT * FROM ruvector.dag_bottlenecks(
    query_text TEXT,
    threshold FLOAT8 DEFAULT 0.7
) RETURNS TABLE (
    node_id INT,
    operator_type TEXT,
    bottleneck_score FLOAT8,
    impact_estimate FLOAT8,
    suggested_action TEXT
);

-- Get min-cut analysis
SELECT * FROM ruvector.dag_mincut_analysis(
    query_text TEXT
) RETURNS TABLE (
    cut_id INT,
    source_nodes INT[],
    sink_nodes INT[],
    cut_capacity FLOAT8,
    parallelization_opportunity BOOLEAN
);
```

### Query Optimization

```sql
-- Get optimization suggestions
SELECT * FROM ruvector.dag_suggest_optimizations(
    query_text TEXT
) RETURNS TABLE (
    suggestion_id INT,
    category TEXT,           -- 'index', 'join_order', 'parallelism', 'memory'
    description TEXT,
    expected_improvement FLOAT8,
    implementation_sql TEXT,
    confidence FLOAT8
);

-- Rewrite query using learned patterns
SELECT ruvector.dag_rewrite_query(
    query_text TEXT
) RETURNS TEXT;

-- Estimate query with neural predictions
SELECT * FROM ruvector.dag_estimate(
    query_text TEXT
) RETURNS TABLE (
    metric TEXT,
    postgres_estimate FLOAT8,
    neural_estimate FLOAT8,
    confidence FLOAT8
);
```

## Attention Mechanism Functions

### Attention Scores

```sql
-- Compute attention for query DAG
SELECT * FROM ruvector.dag_attention_scores(
    query_text TEXT,
    mechanism TEXT DEFAULT 'auto'
) RETURNS TABLE (
    node_id INT,
    attention_weight FLOAT8,
    query_contribution FLOAT8[],
    key_contribution FLOAT8[]
);

-- Get attention matrix
SELECT ruvector.dag_attention_matrix(
    query_text TEXT,
    mechanism TEXT DEFAULT 'auto'
) RETURNS FLOAT8[][];

-- Visualize attention (returns DOT graph)
SELECT ruvector.dag_attention_visualize(
    query_text TEXT,
    mechanism TEXT DEFAULT 'auto',
    format TEXT DEFAULT 'dot'  -- 'dot', 'json', 'ascii'
) RETURNS TEXT;
```

### Attention Configuration

```sql
-- Set attention hyperparameters
SELECT ruvector.dag_attention_configure(
    mechanism TEXT,
    params JSONB
    -- Example params:
    -- topological: {"max_depth": 5, "decay_factor": 0.9}
    -- causal_cone: {"time_window": 1000, "future_discount": 0.5}
    -- critical_path: {"path_weight": 2.0, "branch_penalty": 0.3}
    -- mincut_gated: {"gate_threshold": 0.1, "flow_capacity": "cost"}
    -- hierarchical_lorentz: {"curvature": -1.0, "time_scale": 0.1}
    -- parallel_branch: {"max_branches": 8, "sync_penalty": 0.2}
    -- temporal_btsp: {"plateau_duration": 100, "eligibility_decay": 0.95}
) RETURNS VOID;

-- Get attention statistics
SELECT * FROM ruvector.dag_attention_stats()
RETURNS TABLE (
    mechanism TEXT,
    invocations BIGINT,
    avg_latency_us FLOAT8,
    hit_rate FLOAT8,
    improvement_ratio FLOAT8
);
```

## SONA Learning Functions

### Pattern Management

```sql
-- Store a learned pattern
SELECT ruvector.dag_store_pattern(
    pattern_vector VECTOR(256),
    pattern_metadata JSONB,
    quality_score FLOAT8
) RETURNS BIGINT;  -- pattern_id

-- Query similar patterns
SELECT * FROM ruvector.dag_query_patterns(
    query_vector VECTOR(256),
    k INT DEFAULT 5,
    similarity_threshold FLOAT8 DEFAULT 0.7
) RETURNS TABLE (
    pattern_id BIGINT,
    similarity FLOAT8,
    quality_score FLOAT8,
    metadata JSONB,
    usage_count INT,
    last_used TIMESTAMPTZ
);

-- Get pattern clusters (ReasoningBank)
SELECT * FROM ruvector.dag_pattern_clusters()
RETURNS TABLE (
    cluster_id INT,
    centroid VECTOR(256),
    member_count INT,
    avg_quality FLOAT8,
    representative_query TEXT
);

-- Force pattern consolidation
SELECT ruvector.dag_consolidate_patterns(
    target_clusters INT DEFAULT 100
) RETURNS TABLE (
    clusters_before INT,
    clusters_after INT,
    patterns_merged INT,
    consolidation_time_ms FLOAT8
);
```

### Trajectory Management

```sql
-- Record a learning trajectory
SELECT ruvector.dag_record_trajectory(
    query_hash BIGINT,
    dag_structure JSONB,
    execution_time_ms FLOAT8,
    improvement_ratio FLOAT8,
    attention_mechanism TEXT
) RETURNS BIGINT;  -- trajectory_id

-- Get trajectory history
SELECT * FROM ruvector.dag_trajectory_history(
    time_range TSTZRANGE DEFAULT NULL,
    min_improvement FLOAT8 DEFAULT 0.0,
    limit_count INT DEFAULT 100
) RETURNS TABLE (
    trajectory_id BIGINT,
    query_hash BIGINT,
    recorded_at TIMESTAMPTZ,
    execution_time_ms FLOAT8,
    improvement_ratio FLOAT8,
    attention_mechanism TEXT
);

-- Analyze trajectory trends
SELECT * FROM ruvector.dag_trajectory_trends(
    window_size INTERVAL DEFAULT '1 hour'
) RETURNS TABLE (
    window_start TIMESTAMPTZ,
    trajectory_count INT,
    avg_improvement FLOAT8,
    best_mechanism TEXT,
    pattern_discoveries INT
);
```

### Learning Control

```sql
-- Trigger immediate learning cycle
SELECT ruvector.dag_learn_now() RETURNS TABLE (
    patterns_updated INT,
    new_clusters INT,
    ewc_constraints_updated INT,
    cycle_time_ms FLOAT8
);

-- Reset learning state (use with caution)
SELECT ruvector.dag_reset_learning(
    preserve_patterns BOOLEAN DEFAULT TRUE,
    preserve_trajectories BOOLEAN DEFAULT FALSE
) RETURNS VOID;

-- Export learned state
SELECT ruvector.dag_export_state() RETURNS BYTEA;

-- Import learned state
SELECT ruvector.dag_import_state(state_data BYTEA) RETURNS TABLE (
    patterns_imported INT,
    trajectories_imported INT,
    clusters_restored INT
);

-- Get EWC constraint info
SELECT * FROM ruvector.dag_ewc_constraints()
RETURNS TABLE (
    parameter_name TEXT,
    fisher_importance FLOAT8,
    optimal_value FLOAT8,
    last_updated TIMESTAMPTZ
);
```

## Self-Healing Functions

### Health Monitoring

```sql
-- Run comprehensive health check
SELECT * FROM ruvector.dag_health_report()
RETURNS TABLE (
    subsystem TEXT,
    status TEXT,
    score FLOAT8,
    issues TEXT[],
    recommendations TEXT[]
);

-- Get anomaly detection results
SELECT * FROM ruvector.dag_anomalies(
    time_range TSTZRANGE DEFAULT '[now - 1 hour, now]'::TSTZRANGE
) RETURNS TABLE (
    anomaly_id BIGINT,
    detected_at TIMESTAMPTZ,
    anomaly_type TEXT,
    severity TEXT,
    affected_component TEXT,
    z_score FLOAT8,
    resolved BOOLEAN
);

-- Check index health
SELECT * FROM ruvector.dag_index_health()
RETURNS TABLE (
    index_name TEXT,
    index_type TEXT,
    fragmentation FLOAT8,
    recall_estimate FLOAT8,
    recommended_action TEXT
);

-- Check learning drift
SELECT * FROM ruvector.dag_learning_drift()
RETURNS TABLE (
    metric TEXT,
    current_value FLOAT8,
    baseline_value FLOAT8,
    drift_magnitude FLOAT8,
    trend TEXT
);
```

### Repair Operations

```sql
-- Trigger automatic repair
SELECT * FROM ruvector.dag_auto_repair()
RETURNS TABLE (
    repair_id BIGINT,
    repair_type TEXT,
    target TEXT,
    status TEXT,
    duration_ms FLOAT8
);

-- Rebalance specific index
SELECT ruvector.dag_rebalance_index(
    index_name TEXT,
    target_recall FLOAT8 DEFAULT 0.95
) RETURNS TABLE (
    vectors_moved INT,
    new_recall FLOAT8,
    duration_ms FLOAT8
);

-- Reset pattern quality scores
SELECT ruvector.dag_reset_pattern_quality(
    pattern_ids BIGINT[] DEFAULT NULL  -- NULL = all patterns
) RETURNS INT;  -- patterns reset

-- Force cluster recomputation
SELECT ruvector.dag_recompute_clusters(
    algorithm TEXT DEFAULT 'kmeans_pp'
) RETURNS TABLE (
    old_clusters INT,
    new_clusters INT,
    silhouette_score FLOAT8
);
```

## QuDAG Integration Functions

### Network Operations

```sql
-- Connect to QuDAG network
SELECT ruvector.qudag_connect(
    endpoint TEXT,
    identity_key BYTEA DEFAULT NULL  -- auto-generate if NULL
) RETURNS TABLE (
    connected BOOLEAN,
    node_id TEXT,
    network_version TEXT
);

-- Get network status
SELECT * FROM ruvector.qudag_status()
RETURNS TABLE (
    connected BOOLEAN,
    node_id TEXT,
    peers INT,
    latest_round BIGINT,
    sync_status TEXT
);

-- Propose pattern to network
SELECT ruvector.qudag_propose_pattern(
    pattern_vector VECTOR(256),
    metadata JSONB,
    stake_amount FLOAT8 DEFAULT 0.0
) RETURNS TABLE (
    proposal_id TEXT,
    submitted_at TIMESTAMPTZ,
    status TEXT
);

-- Check proposal status
SELECT * FROM ruvector.qudag_proposal_status(
    proposal_id TEXT
) RETURNS TABLE (
    status TEXT,
    votes_for INT,
    votes_against INT,
    finalized BOOLEAN,
    finalized_at TIMESTAMPTZ
);

-- Sync patterns from network
SELECT * FROM ruvector.qudag_sync_patterns(
    since_round BIGINT DEFAULT 0
) RETURNS TABLE (
    patterns_received INT,
    patterns_applied INT,
    conflicts_resolved INT
);
```

### Token Operations

```sql
-- Get rUv balance
SELECT ruvector.qudag_balance() RETURNS FLOAT8;

-- Stake tokens
SELECT ruvector.qudag_stake(
    amount FLOAT8
) RETURNS TABLE (
    new_stake FLOAT8,
    tx_hash TEXT
);

-- Claim rewards
SELECT * FROM ruvector.qudag_claim_rewards()
RETURNS TABLE (
    amount FLOAT8,
    tx_hash TEXT,
    source TEXT
);

-- Get staking info
SELECT * FROM ruvector.qudag_staking_info()
RETURNS TABLE (
    staked_amount FLOAT8,
    pending_rewards FLOAT8,
    lock_until TIMESTAMPTZ,
    apr_estimate FLOAT8
);
```

### Cryptographic Operations

```sql
-- Generate ML-KEM keypair
SELECT ruvector.qudag_generate_kem_keypair()
RETURNS TABLE (
    public_key BYTEA,
    secret_key_id TEXT  -- stored securely
);

-- Encrypt data for peer
SELECT ruvector.qudag_encrypt(
    plaintext BYTEA,
    recipient_pubkey BYTEA
) RETURNS TABLE (
    ciphertext BYTEA,
    encapsulated_key BYTEA
);

-- Decrypt received data
SELECT ruvector.qudag_decrypt(
    ciphertext BYTEA,
    encapsulated_key BYTEA,
    secret_key_id TEXT
) RETURNS BYTEA;

-- Sign data
SELECT ruvector.qudag_sign(
    data BYTEA
) RETURNS BYTEA;  -- ML-DSA signature

-- Verify signature
SELECT ruvector.qudag_verify(
    data BYTEA,
    signature BYTEA,
    pubkey BYTEA
) RETURNS BOOLEAN;
```

## Monitoring and Statistics

### Performance Metrics

```sql
-- Get overall statistics
SELECT * FROM ruvector.dag_statistics()
RETURNS TABLE (
    metric TEXT,
    value FLOAT8,
    unit TEXT,
    updated_at TIMESTAMPTZ
);

-- Get latency breakdown
SELECT * FROM ruvector.dag_latency_breakdown(
    time_range TSTZRANGE DEFAULT '[now - 1 hour, now]'::TSTZRANGE
) RETURNS TABLE (
    component TEXT,
    p50_us FLOAT8,
    p95_us FLOAT8,
    p99_us FLOAT8,
    max_us FLOAT8
);

-- Get memory usage
SELECT * FROM ruvector.dag_memory_usage()
RETURNS TABLE (
    component TEXT,
    allocated_bytes BIGINT,
    used_bytes BIGINT,
    peak_bytes BIGINT
);

-- Get throughput metrics
SELECT * FROM ruvector.dag_throughput(
    window INTERVAL DEFAULT '1 minute'
) RETURNS TABLE (
    metric TEXT,
    count BIGINT,
    per_second FLOAT8
);
```

### Debugging

```sql
-- Enable debug logging
SELECT ruvector.dag_set_debug(
    enabled BOOLEAN,
    components TEXT[] DEFAULT ARRAY['all']
) RETURNS VOID;

-- Get recent debug logs
SELECT * FROM ruvector.dag_debug_logs(
    since TIMESTAMPTZ DEFAULT now() - interval '5 minutes',
    component TEXT DEFAULT NULL,
    severity TEXT DEFAULT NULL  -- 'debug', 'info', 'warn', 'error'
) RETURNS TABLE (
    logged_at TIMESTAMPTZ,
    component TEXT,
    severity TEXT,
    message TEXT,
    context JSONB
);

-- Trace single query
SELECT * FROM ruvector.dag_trace_query(
    query_text TEXT
) RETURNS TABLE (
    step INT,
    operation TEXT,
    duration_us FLOAT8,
    details JSONB
);

-- Export diagnostics bundle
SELECT ruvector.dag_export_diagnostics() RETURNS BYTEA;
```

## Batch Operations

### Bulk Processing

```sql
-- Analyze multiple queries
SELECT * FROM ruvector.dag_bulk_analyze(
    queries TEXT[]
) RETURNS TABLE (
    query_index INT,
    bottleneck_count INT,
    suggested_mechanism TEXT,
    estimated_improvement FLOAT8
);

-- Pre-warm patterns for workload
SELECT ruvector.dag_prewarm_patterns(
    representative_queries TEXT[]
) RETURNS TABLE (
    patterns_loaded INT,
    cache_hit_rate FLOAT8
);

-- Batch record trajectories
SELECT ruvector.dag_bulk_record_trajectories(
    trajectories JSONB[]
) RETURNS INT;  -- trajectories recorded
```

## Views

### System Views

```sql
-- Active configuration
CREATE VIEW ruvector.dag_active_config AS
SELECT * FROM ruvector.dag_config();

-- Recent patterns
CREATE VIEW ruvector.dag_recent_patterns AS
SELECT pattern_id, created_at, quality_score, usage_count
FROM ruvector.dag_patterns
WHERE created_at > now() - interval '24 hours'
ORDER BY quality_score DESC;

-- Attention effectiveness
CREATE VIEW ruvector.dag_attention_effectiveness AS
SELECT
    mechanism,
    count(*) as uses,
    avg(improvement_ratio) as avg_improvement,
    percentile_cont(0.95) WITHIN GROUP (ORDER BY improvement_ratio) as p95_improvement
FROM ruvector.dag_trajectories
WHERE recorded_at > now() - interval '7 days'
GROUP BY mechanism;

-- Health summary
CREATE VIEW ruvector.dag_health_summary AS
SELECT
    subsystem,
    status,
    score,
    array_length(issues, 1) as issue_count
FROM ruvector.dag_health_report();
```

## Installation SQL

```sql
-- Create extension
CREATE EXTENSION IF NOT EXISTS ruvector_dag CASCADE;

-- Required tables (auto-created by extension)
-- ruvector.dag_patterns - Learned patterns storage
-- ruvector.dag_trajectories - Learning trajectory history
-- ruvector.dag_clusters - Pattern clusters (ReasoningBank)
-- ruvector.dag_anomalies - Detected anomalies log
-- ruvector.dag_repairs - Repair history
-- ruvector.dag_qudag_proposals - QuDAG proposal tracking

-- Recommended indexes
CREATE INDEX ON ruvector.dag_patterns USING hnsw (pattern_vector vector_cosine_ops);
CREATE INDEX ON ruvector.dag_trajectories (recorded_at DESC);
CREATE INDEX ON ruvector.dag_trajectories (query_hash);
CREATE INDEX ON ruvector.dag_anomalies (detected_at DESC) WHERE NOT resolved;

-- Grant permissions
GRANT USAGE ON SCHEMA ruvector TO PUBLIC;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA ruvector TO PUBLIC;
GRANT SELECT ON ALL TABLES IN SCHEMA ruvector TO PUBLIC;
```

## Usage Examples

### Basic Query Optimization

```sql
-- Enable neural DAG learning
SELECT ruvector.dag_set_enabled(true);

-- Analyze a query
SELECT * FROM ruvector.dag_analyze_plan($$
    SELECT v.*, m.category
    FROM vectors v
    JOIN metadata m ON v.id = m.vector_id
    WHERE v.embedding <-> $1 < 0.5
    ORDER BY v.embedding <-> $1
    LIMIT 100
$$);

-- Get optimization suggestions
SELECT * FROM ruvector.dag_suggest_optimizations($$
    SELECT v.*, m.category
    FROM vectors v
    JOIN metadata m ON v.id = m.vector_id
    WHERE v.embedding <-> $1 < 0.5
    ORDER BY v.embedding <-> $1
    LIMIT 100
$$);
```

### Attention Mechanism Selection

```sql
-- Let system choose best attention
SELECT ruvector.dag_set_attention('auto');

-- Or manually select based on workload
-- For deep query plans:
SELECT ruvector.dag_set_attention('topological');

-- For time-series workloads:
SELECT ruvector.dag_set_attention('causal_cone');

-- For CPU-bound queries:
SELECT ruvector.dag_set_attention('critical_path');
```

### Distributed Learning with QuDAG

```sql
-- Connect to QuDAG network
SELECT * FROM ruvector.qudag_connect(
    'https://qudag.example.com:8443'
);

-- Stake tokens for participation
SELECT * FROM ruvector.qudag_stake(100.0);

-- Patterns are now automatically shared and validated
-- Check sync status
SELECT * FROM ruvector.qudag_status();
```

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| RV001 | DAG_DISABLED | Neural DAG learning is disabled |
| RV002 | INVALID_ATTENTION | Unknown attention mechanism |
| RV003 | PATTERN_NOT_FOUND | Referenced pattern does not exist |
| RV004 | LEARNING_FAILED | Learning cycle failed |
| RV005 | QUDAG_DISCONNECTED | Not connected to QuDAG network |
| RV006 | QUDAG_AUTH_FAILED | QuDAG authentication failed |
| RV007 | INSUFFICIENT_STAKE | Not enough staked tokens |
| RV008 | CRYPTO_ERROR | Cryptographic operation failed |
| RV009 | REPAIR_FAILED | Self-healing repair failed |
| RV010 | TRAJECTORY_OVERFLOW | Trajectory buffer full |

---

*Document: 09-SQL-API.md | Version: 1.0 | Last Updated: 2025-01-XX*
