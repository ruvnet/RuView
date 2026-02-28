# RuVector Postgres v2 - Integrity Events and Policy Specification

## Overview

This document specifies the integrity event schema, policy JSON format, and the dynamic mincut-based control plane that forms the key differentiator for RuVector v2. The integrity system provides operational safety guarantees for vector operations, including distributed deployments.

---

## Core Concepts

### What is Mincut Integrity?

Mincut integrity uses graph connectivity analysis to detect system stress:

1. **Contracted Graph**: A small operational graph representing partitions, shards, and dependencies (NOT the full similarity graph)
2. **Lambda Cut (λ_cut)**: The minimum cut value - the minimum total edge capacity that must be removed to disconnect the graph. This is the PRIMARY integrity metric.
3. **Lambda2 (λ₂) - Spectral Stress**: The algebraic connectivity (second-smallest eigenvalue of Laplacian) provides a complementary spectral measure of how well-connected the system is. OPTIONAL drift signal.
4. **Integrity States**: Based on lambda_cut, system enters normal/stress/critical states. Lambda2 trend can trigger preemptive rebalancing.
5. **Policy Actions**: Each state defines which operations are allowed/throttled/deferred/rejected

**IMPORTANT TERMINOLOGY**:
- **λ_cut (lambda_cut)**: Minimum cut value - computed via Stoer-Wagner or Push-Relabel algorithms. **Drives state transitions.**
- **λ₂ (lambda2)**: Algebraic connectivity - computed via eigenvalue decomposition. **Drift signal for preemptive action.**
- These are DIFFERENT metrics. λ_cut gates operations; λ₂ informs proactive rebalancing.

---

## Contracted Operational Graph Definition

**CRITICAL**: Mincut is NEVER computed over raw similarity edges or raw HNSW adjacency. Only over the contracted operational graph.

### Contracted Graph Nodes

| Node Type | Description | Example |
|-----------|-------------|---------|
| `shard` | Data shard/partition | `shard_0`, `shard_1` |
| `hnsw_layer` | HNSW index layer | `layer_0`, `layer_1`, `layer_2` |
| `centroid_bucket` | IVFFlat centroid group | `bucket_0..bucket_N` |
| `gateway` | Query routing endpoint | `primary_gateway`, `replica_gateway` |
| `maintenance` | Background job dependency | `compactor`, `gnn_trainer`, `tier_manager` |

### Contracted Graph Edges

| Edge Type | Source → Target | Weight Derivation |
|-----------|-----------------|-------------------|
| `routing` | gateway → shard | 1.0 - (queue_depth / max_queue) |
| `replication` | shard → shard | 1.0 - replication_lag_ratio |
| `layer_link` | hnsw_layer → hnsw_layer | 1.0 - error_rate |
| `maintenance_dep` | shard → maintenance | 1.0 if healthy, 0.1 if degraded |
| `centroid_route` | gateway → centroid_bucket | 1.0 - (latency_ms / budget_ms) |

### Edge Weight Calculation

```rust
/// Edge weights derived from operational metrics
pub fn compute_edge_weight(edge: &ContractedEdge, metrics: &Metrics) -> f64 {
    match edge.edge_type {
        EdgeType::Routing => {
            let queue_ratio = metrics.queue_depth as f64 / metrics.max_queue as f64;
            (1.0 - queue_ratio).max(0.01)  // Never zero
        }
        EdgeType::Replication => {
            let lag_ratio = metrics.replication_lag_ms as f64 / metrics.lag_budget_ms as f64;
            (1.0 - lag_ratio).clamp(0.01, 1.0)
        }
        EdgeType::LayerLink => {
            (1.0 - metrics.error_rate).max(0.01)
        }
        EdgeType::MaintenanceDep => {
            if metrics.service_healthy { 1.0 } else { 0.1 }
        }
        EdgeType::CentroidRoute => {
            let latency_ratio = metrics.latency_ms as f64 / metrics.latency_budget_ms as f64;
            (1.0 - latency_ratio).clamp(0.01, 1.0)
        }
    }
}
```

### What is NOT in the Contracted Graph

```
NEVER INCLUDED:
- Raw HNSW neighbor edges (millions of edges)
- Similarity edges between vectors
- Individual vector TIDs
- Full centroid-to-vector mappings

ALWAYS INCLUDED:
- Coarse operational entities only
- Typically 10-1000 nodes total
- Edges represent operational dependencies, not data similarity
```

### Why Contracted Graph?

```
CRITICAL DESIGN CONSTRAINT:
Never compute mincut on full similarity graph - always on contracted operational graph.

Full similarity graph: O(N^2) edges for N vectors - impossible at scale
Contracted graph: O(1000) nodes for partitions/centroids/shards - always tractable
```

### Contracted Graph Node Types

| Node Type | Description | Example Count |
|-----------|-------------|---------------|
| `partition` | Data partition (shard) | 16-256 |
| `centroid` | IVFFlat/clustering centroid | 100-10000 |
| `shard` | Distributed shard | 1-64 |
| `maintenance_dep` | Maintenance dependency | 10-100 |

### Contracted Graph Edge Types

| Edge Type | Description | Capacity Weight |
|-----------|-------------|----------------|
| `partition_link` | Data flow between partitions | 1.0 (normal), 0.5 (degraded) |
| `routing_link` | Query routing path | 1.0 - based on latency |
| `dependency` | Operational dependency | 1.0 |
| `replication` | Replication link | 0.0 (broken) - 1.0 (healthy) |

---

## Integrity State Machine

### States

```
                    lambda >= threshold_high
            +------------------------------------+
            |                                    |
            v                                    |
    +---------------+                    +---------------+
    |    NORMAL     |<-------------------|    STRESS     |
    | (Allow all)   |   lambda rises     | (Throttle)    |
    +-------+-------+                    +-------+-------+
            |                                    ^
            | lambda drops                       |
            | below threshold_high               | lambda rises
            |                                    | above threshold_low
            v                                    |
    +---------------+                    +-------+-------+
    |    STRESS     |------------------->|   CRITICAL    |
    | (Throttle)    |   lambda drops     | (Freeze)      |
    +---------------+   below low        +---------------+
```

### State Definitions

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityState {
    /// Lambda >= threshold_high
    /// All operations allowed
    Normal,

    /// threshold_low < Lambda < threshold_high
    /// Bulk operations throttled, mutations allowed
    Stress,

    /// Lambda <= threshold_low
    /// Only reads and replication allowed
    Critical,
}

impl IntegrityState {
    pub fn from_lambda(
        lambda: f64,
        threshold_high: f64,
        threshold_low: f64,
    ) -> Self {
        if lambda >= threshold_high {
            IntegrityState::Normal
        } else if lambda > threshold_low {
            IntegrityState::Stress
        } else {
            IntegrityState::Critical
        }
    }
}
```

---

## Hysteresis and Cooldown

### Problem: State Flapping

Without hysteresis, the system can rapidly oscillate between states when λ_cut hovers near thresholds. This causes:
- Excessive event logging
- Unstable operation permissions
- Poor user experience
- Potential cascading failures

### Hysteresis Model

```
State Transition Rules with Hysteresis:
+------------------------------------------------------------------+
|                                                                   |
|  NORMAL → STRESS:                                                 |
|    When λ_cut < T_high for N consecutive samples                 |
|    Default: N = 3 samples (~3 minutes at 60s interval)           |
|                                                                   |
|  STRESS → CRITICAL:                                               |
|    When λ_cut < T_low for M consecutive samples                  |
|    Default: M = 2 samples (~2 minutes)                            |
|                                                                   |
|  CRITICAL → STRESS:                                               |
|    When λ_cut > T_restore_low for H seconds                      |
|    Default: T_restore_low = T_low + 0.1, H = 300 seconds          |
|                                                                   |
|  STRESS → NORMAL:                                                 |
|    When λ_cut > T_restore_high for H seconds                     |
|    Default: T_restore_high = T_high + 0.05, H = 300 seconds       |
|                                                                   |
+------------------------------------------------------------------+
```

### Configuration Schema

```sql
-- Hysteresis configuration in policy
ALTER TABLE ruvector.integrity_policies ADD COLUMN IF NOT EXISTS hysteresis JSONB;

-- Default hysteresis configuration
UPDATE ruvector.integrity_policies SET hysteresis = '{
    "degrade_samples": 3,
    "critical_samples": 2,
    "restore_threshold_offset": 0.1,
    "restore_hold_seconds": 300,
    "cooldown_after_transition_seconds": 60
}'::jsonb
WHERE hysteresis IS NULL;
```

### Rust Implementation

```rust
/// Hysteresis state tracker
pub struct HysteresisTracker {
    /// Consecutive samples below threshold
    degrade_count: u32,
    /// Consecutive samples for critical
    critical_count: u32,
    /// Time when recovery started
    recovery_start: Option<Instant>,
    /// Last state transition time
    last_transition: Instant,
    /// Configuration
    config: HysteresisConfig,
}

#[derive(Debug, Clone)]
pub struct HysteresisConfig {
    pub degrade_samples: u32,        // Samples before NORMAL→STRESS
    pub critical_samples: u32,       // Samples before STRESS→CRITICAL
    pub restore_offset: f64,         // Offset above threshold for recovery
    pub restore_hold_secs: u64,      // Hold time before recovery
    pub cooldown_secs: u64,          // Cooldown after any transition
}

impl HysteresisTracker {
    /// Evaluate state with hysteresis
    pub fn evaluate(
        &mut self,
        current_state: IntegrityState,
        lambda_cut: f64,
        thresholds: &Thresholds,
    ) -> Option<IntegrityState> {
        // Enforce cooldown
        if self.last_transition.elapsed().as_secs() < self.config.cooldown_secs {
            return None;
        }

        match current_state {
            IntegrityState::Normal => {
                if lambda_cut < thresholds.high {
                    self.degrade_count += 1;
                    if self.degrade_count >= self.config.degrade_samples {
                        self.transition_to(IntegrityState::Stress)
                    } else {
                        None
                    }
                } else {
                    self.degrade_count = 0;
                    None
                }
            }
            IntegrityState::Stress => {
                if lambda_cut < thresholds.low {
                    self.critical_count += 1;
                    if self.critical_count >= self.config.critical_samples {
                        self.transition_to(IntegrityState::Critical)
                    } else {
                        None
                    }
                } else if lambda_cut > thresholds.high + self.config.restore_offset {
                    self.check_restore(IntegrityState::Normal)
                } else {
                    self.critical_count = 0;
                    self.recovery_start = None;
                    None
                }
            }
            IntegrityState::Critical => {
                if lambda_cut > thresholds.low + self.config.restore_offset {
                    self.check_restore(IntegrityState::Stress)
                } else {
                    self.recovery_start = None;
                    None
                }
            }
        }
    }

    fn check_restore(&mut self, target: IntegrityState) -> Option<IntegrityState> {
        match self.recovery_start {
            Some(start) if start.elapsed().as_secs() >= self.config.restore_hold_secs => {
                self.transition_to(target)
            }
            Some(_) => None,  // Still in hold period
            None => {
                self.recovery_start = Some(Instant::now());
                None
            }
        }
    }

    fn transition_to(&mut self, state: IntegrityState) -> Option<IntegrityState> {
        self.last_transition = Instant::now();
        self.degrade_count = 0;
        self.critical_count = 0;
        self.recovery_start = None;
        Some(state)
    }
}
```

---

## Operation Classification

### Risk Levels

Instead of treating all operations uniformly, classify by risk to the system:

```rust
/// Operation risk classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationRisk {
    /// Low risk: minimal system impact
    Low,
    /// Medium risk: moderate resource usage
    Medium,
    /// High risk: significant structural changes
    High,
}

/// Map operations to risk levels
pub fn classify_operation(op: &str) -> OperationRisk {
    match op {
        // LOW RISK - Point operations, minimal impact
        "search" | "read" | "point_insert" | "point_delete" => OperationRisk::Low,

        // MEDIUM RISK - Bulk operations, index updates
        "bulk_insert" | "bulk_delete" | "update" |
        "centroid_update" | "graph_edge_add" | "graph_edge_remove" => OperationRisk::Medium,

        // HIGH RISK - Structural changes, maintenance
        "hnsw_rewire" | "index_rebuild" | "compaction" |
        "tier_demotion" | "shard_move" | "replication_reshuffle" => OperationRisk::High,

        _ => OperationRisk::Medium,  // Default to medium
    }
}
```

### Gate Response Types

```rust
/// Integrity gate response
#[derive(Debug, Clone)]
pub enum GateResponse {
    /// Operation allowed immediately
    Allow,

    /// Operation allowed with throttling
    Throttle {
        factor: f32,  // 0.0 = full throttle, 1.0 = no throttle
    },

    /// Operation should be deferred to later
    Defer {
        retry_after_secs: u64,
    },

    /// Operation rejected
    Reject {
        reason: String,
    },
}
```

### Risk-Based Gating Matrix

```
+------------------------------------------------------------------+
|              INTEGRITY GATE RESPONSE MATRIX                       |
+------------------------------------------------------------------+
|                                                                   |
|  Operation Risk  |  NORMAL    |  STRESS        |  CRITICAL       |
|  ----------------+------------+----------------+-----------------+|
|  LOW             |  Allow     |  Allow         |  Throttle(0.8)  |
|  MEDIUM          |  Allow     |  Throttle(0.5) |  Defer(60s)     |
|  HIGH            |  Allow     |  Defer(300s)   |  Reject         |
|                                                                   |
+------------------------------------------------------------------+
```

### Implementation

```rust
/// Apply integrity gate with operation classification
pub fn gate_operation(
    collection_id: i32,
    operation: &str,
) -> GateResponse {
    let state = get_integrity_state(collection_id);
    let risk = classify_operation(operation);
    let policy = get_active_policy(collection_id);

    match (state, risk) {
        // NORMAL state - allow everything
        (IntegrityState::Normal, _) => GateResponse::Allow,

        // STRESS state - varies by risk
        (IntegrityState::Stress, OperationRisk::Low) => GateResponse::Allow,
        (IntegrityState::Stress, OperationRisk::Medium) => {
            GateResponse::Throttle { factor: 0.5 }
        }
        (IntegrityState::Stress, OperationRisk::High) => {
            GateResponse::Defer { retry_after_secs: 300 }
        }

        // CRITICAL state - strict controls
        (IntegrityState::Critical, OperationRisk::Low) => {
            GateResponse::Throttle { factor: 0.8 }
        }
        (IntegrityState::Critical, OperationRisk::Medium) => {
            GateResponse::Defer { retry_after_secs: 60 }
        }
        (IntegrityState::Critical, OperationRisk::High) => {
            GateResponse::Reject {
                reason: format!(
                    "High-risk operation '{}' blocked: system in critical state",
                    operation
                ),
            }
        }
    }
}
```

### SQL Function Update

```sql
-- Updated gate function with risk classification
CREATE OR REPLACE FUNCTION ruvector_integrity_gate(
    p_collection_name TEXT,
    p_operation TEXT
) RETURNS JSONB AS $$
DECLARE
    v_result JSONB;
BEGIN
    -- Call Rust function with classification
    v_result := _ruvector_gate_operation(p_collection_name, p_operation);

    -- Result structure:
    -- {
    --   "response": "allow" | "throttle" | "defer" | "reject",
    --   "risk_level": "low" | "medium" | "high",
    --   "state": "normal" | "stress" | "critical",
    --   "throttle_factor": 0.5,        -- for throttle response
    --   "retry_after_secs": 60,        -- for defer response
    --   "reason": "..."                -- for reject response
    -- }

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;
```

---

## Event Schema

### Integrity Event Structure

```sql
CREATE TABLE ruvector.integrity_events (
    -- Identification
    id              BIGSERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id),

    -- Event classification
    event_type      TEXT NOT NULL CHECK (event_type IN (
        'state_change',      -- State transition occurred
        'sample',            -- Periodic sample taken
        'policy_update',     -- Policy was modified
        'manual_override',   -- Admin manually set state
        'recovery',          -- System recovered from failure
        'alert',             -- Threshold warning
        'audit'              -- Audit trail entry
    )),

    -- State information
    previous_state  TEXT,    -- NULL for initial events
    new_state       TEXT,
    lambda_cut      REAL NOT NULL,    -- Minimum cut value (PRIMARY metric)
    lambda2         REAL,             -- Algebraic connectivity / spectral stress (OPTIONAL)

    -- Witness information (for forensics)
    witness_edges   JSONB,   -- Edges that form the mincut (bottleneck edges)

    -- Event metadata
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Cryptographic audit
    signature       BYTEA,   -- Ed25519 signature over event content
    signer_id       TEXT,    -- Key identifier

    -- Timestamps
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ  -- NULL = never expires
);

-- Indexes for efficient querying
CREATE INDEX idx_integrity_events_collection_time
    ON ruvector.integrity_events(collection_id, created_at DESC);

CREATE INDEX idx_integrity_events_type_time
    ON ruvector.integrity_events(event_type, created_at DESC);

CREATE INDEX idx_integrity_events_state
    ON ruvector.integrity_events(new_state)
    WHERE new_state IS NOT NULL;
```

### Event Metadata Schema

```typescript
// TypeScript interface for metadata field
interface IntegrityEventMetadata {
    // Common fields
    source: "worker" | "api" | "trigger" | "admin";
    session_id?: string;
    request_id?: string;

    // For state_change events
    transition_duration_ms?: number;
    affected_operations?: string[];
    blocked_operations?: string[];

    // For sample events
    sample_size?: number;
    node_count?: number;
    edge_count?: number;
    computation_time_ms?: number;

    // For policy_update events
    policy_name?: string;
    changes?: {
        field: string;
        old_value: any;
        new_value: any;
    }[];

    // For manual_override events
    operator_id?: string;
    reason?: string;
    expected_duration_secs?: number;

    // For recovery events
    recovery_type?: "automatic" | "manual";
    downtime_secs?: number;
    data_loss?: boolean;

    // For alert events
    alert_level?: "warning" | "error" | "critical";
    threshold_crossed?: string;
    recommended_action?: string;
}
```

### Witness Edges Schema

```typescript
// Witness edges that form the mincut
interface WitnessEdge {
    source: {
        type: "partition" | "centroid" | "shard" | "maintenance_dep";
        id: number;
        name?: string;
    };
    target: {
        type: "partition" | "centroid" | "shard" | "maintenance_dep";
        id: number;
        name?: string;
    };
    edge_type: "partition_link" | "routing_link" | "dependency" | "replication";
    capacity: number;
    flow: number;  // Actual flow through edge in mincut
}
```

---

## Policy Schema

### Policy Table Structure

```sql
CREATE TABLE ruvector.integrity_policies (
    id              SERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id),
    name            TEXT NOT NULL,
    description     TEXT,
    priority        INTEGER NOT NULL DEFAULT 0,  -- Higher = takes precedence

    -- Thresholds
    threshold_high  REAL NOT NULL DEFAULT 0.8,
    threshold_low   REAL NOT NULL DEFAULT 0.3,

    -- State actions (JSONB)
    normal_actions  JSONB NOT NULL DEFAULT '{}'::jsonb,
    stress_actions  JSONB NOT NULL DEFAULT '{}'::jsonb,
    critical_actions JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Sampling configuration
    sample_interval_secs INTEGER NOT NULL DEFAULT 60,
    sample_size     INTEGER NOT NULL DEFAULT 1000,
    sample_method   TEXT NOT NULL DEFAULT 'random'
                    CHECK (sample_method IN ('random', 'stratified', 'adaptive')),

    -- Notification configuration
    notifications   JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Status
    enabled         BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(collection_id, name)
);
```

### Policy Actions JSON Schema

```json
{
    "$schema": "http://json-schema.org/draft-07/schema#",
    "title": "Integrity Policy Actions",
    "type": "object",
    "properties": {
        "allow_reads": {
            "type": "boolean",
            "default": true,
            "description": "Allow read operations (searches)"
        },
        "allow_single_insert": {
            "type": "boolean",
            "default": true,
            "description": "Allow single vector inserts"
        },
        "allow_bulk_insert": {
            "type": "boolean",
            "default": true,
            "description": "Allow bulk insert operations"
        },
        "allow_delete": {
            "type": "boolean",
            "default": true,
            "description": "Allow delete operations"
        },
        "allow_update": {
            "type": "boolean",
            "default": true,
            "description": "Allow update operations"
        },
        "allow_index_rewire": {
            "type": "boolean",
            "default": true,
            "description": "Allow HNSW edge rewiring during maintenance"
        },
        "allow_compression": {
            "type": "boolean",
            "default": true,
            "description": "Allow tier compression/compaction"
        },
        "allow_replication": {
            "type": "boolean",
            "default": true,
            "description": "Allow replication streaming"
        },
        "allow_backup": {
            "type": "boolean",
            "default": true,
            "description": "Allow backup operations"
        },
        "throttle_inserts_pct": {
            "type": "integer",
            "minimum": 0,
            "maximum": 100,
            "default": 0,
            "description": "Percentage of inserts to reject (0 = none, 100 = all)"
        },
        "throttle_searches_pct": {
            "type": "integer",
            "minimum": 0,
            "maximum": 100,
            "default": 0,
            "description": "Percentage of searches to queue/delay"
        },
        "max_concurrent_searches": {
            "type": "integer",
            "minimum": 1,
            "default": null,
            "description": "Maximum concurrent searches (null = unlimited)"
        },
        "max_insert_batch_size": {
            "type": "integer",
            "minimum": 1,
            "default": null,
            "description": "Maximum vectors per insert batch (null = unlimited)"
        },
        "pause_gnn_training": {
            "type": "boolean",
            "default": false,
            "description": "Pause GNN model training"
        },
        "pause_tier_management": {
            "type": "boolean",
            "default": false,
            "description": "Pause tier promotion/demotion"
        },
        "emergency_compact": {
            "type": "boolean",
            "default": false,
            "description": "Trigger emergency compaction to free resources"
        },
        "custom_actions": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "command": { "type": "string" },
                    "args": { "type": "object" }
                },
                "required": ["name", "command"]
            },
            "description": "Custom actions to execute"
        }
    }
}
```

### Default Policies

```sql
-- Default policy values for each state
INSERT INTO ruvector.integrity_policies (
    collection_id,
    name,
    description,
    threshold_high,
    threshold_low,
    normal_actions,
    stress_actions,
    critical_actions,
    sample_interval_secs,
    sample_size
)
SELECT
    id,
    'default',
    'Default integrity policy',
    0.8,
    0.3,

    -- Normal state: everything allowed
    '{
        "allow_reads": true,
        "allow_single_insert": true,
        "allow_bulk_insert": true,
        "allow_delete": true,
        "allow_update": true,
        "allow_index_rewire": true,
        "allow_compression": true,
        "allow_replication": true,
        "allow_backup": true,
        "throttle_inserts_pct": 0,
        "throttle_searches_pct": 0,
        "pause_gnn_training": false,
        "pause_tier_management": false
    }'::jsonb,

    -- Stress state: throttle bulk operations
    '{
        "allow_reads": true,
        "allow_single_insert": true,
        "allow_bulk_insert": false,
        "allow_delete": true,
        "allow_update": true,
        "allow_index_rewire": true,
        "allow_compression": true,
        "allow_replication": true,
        "allow_backup": true,
        "throttle_inserts_pct": 50,
        "throttle_searches_pct": 0,
        "max_insert_batch_size": 100,
        "pause_gnn_training": true,
        "pause_tier_management": false
    }'::jsonb,

    -- Critical state: freeze mutations
    '{
        "allow_reads": true,
        "allow_single_insert": false,
        "allow_bulk_insert": false,
        "allow_delete": false,
        "allow_update": false,
        "allow_index_rewire": false,
        "allow_compression": false,
        "allow_replication": true,
        "allow_backup": true,
        "throttle_inserts_pct": 100,
        "throttle_searches_pct": 20,
        "max_concurrent_searches": 10,
        "pause_gnn_training": true,
        "pause_tier_management": true,
        "emergency_compact": true
    }'::jsonb,

    60,   -- Sample every minute
    1000  -- Sample 1000 edges

FROM ruvector.collections;
```

### Notification Configuration

```json
{
    "$schema": "http://json-schema.org/draft-07/schema#",
    "title": "Integrity Notification Configuration",
    "type": "object",
    "properties": {
        "on_state_change": {
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean", "default": true },
                "channels": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": ["pg_notify", "webhook", "email", "slack"]
                            },
                            "config": { "type": "object" }
                        },
                        "required": ["type"]
                    }
                },
                "include_witness_edges": { "type": "boolean", "default": false }
            }
        },
        "on_threshold_approach": {
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean", "default": true },
                "warning_threshold": {
                    "type": "number",
                    "description": "Warn when lambda within this % of threshold"
                }
            }
        },
        "on_recovery": {
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean", "default": true }
            }
        }
    }
}
```

---

## Cryptographic Signing

### Event Signature Format

```rust
/// Signed integrity event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedIntegrityEvent {
    /// Event content (signed portion)
    pub event: IntegrityEventContent,

    /// Ed25519 signature over serialized event
    pub signature: [u8; 64],

    /// Signer key identifier
    pub signer_id: String,

    /// Signature timestamp
    pub signed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityEventContent {
    pub collection_id: i32,
    pub event_type: String,
    pub previous_state: Option<String>,
    pub new_state: Option<String>,
    pub lambda_cut: Option<f64>,
    pub witness_edges: Option<Vec<WitnessEdge>>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl SignedIntegrityEvent {
    /// Create and sign a new event
    pub fn sign(
        event: IntegrityEventContent,
        signing_key: &ed25519_dalek::SigningKey,
        signer_id: &str,
    ) -> Self {
        // Canonical JSON serialization for signing
        let message = serde_json::to_vec(&event).unwrap();

        // Sign
        let signature = signing_key.sign(&message);

        Self {
            event,
            signature: signature.to_bytes(),
            signer_id: signer_id.to_string(),
            signed_at: Utc::now(),
        }
    }

    /// Verify event signature
    pub fn verify(&self, public_key: &ed25519_dalek::VerifyingKey) -> bool {
        let message = serde_json::to_vec(&self.event).unwrap();
        let signature = ed25519_dalek::Signature::from_bytes(&self.signature);

        public_key.verify_strict(&message, &signature).is_ok()
    }
}
```

### Key Management

```sql
-- Signing key registry (public keys only in DB)
CREATE TABLE ruvector.signing_keys (
    id              TEXT PRIMARY KEY,  -- Key identifier
    public_key      BYTEA NOT NULL,    -- Ed25519 public key (32 bytes)
    description     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,
    revocation_reason TEXT
);

-- Verify event signature SQL function
CREATE FUNCTION ruvector_verify_event_signature(
    p_event_id BIGINT
) RETURNS BOOLEAN AS $$
DECLARE
    v_event ruvector.integrity_events;
    v_key ruvector.signing_keys;
BEGIN
    SELECT * INTO v_event FROM ruvector.integrity_events WHERE id = p_event_id;
    IF NOT FOUND OR v_event.signature IS NULL THEN
        RETURN NULL;
    END IF;

    SELECT * INTO v_key FROM ruvector.signing_keys WHERE id = v_event.signer_id;
    IF NOT FOUND OR v_key.revoked_at IS NOT NULL THEN
        RETURN FALSE;
    END IF;

    -- Call Rust function for actual verification
    RETURN _ruvector_verify_signature(
        v_event.signature,
        v_key.public_key,
        row_to_json(v_event)::text
    );
END;
$$ LANGUAGE plpgsql;
```

---

## API Functions

### Query Integrity Status

```sql
-- Get current integrity status
CREATE FUNCTION ruvector_integrity_status(
    p_collection_name TEXT
) RETURNS JSONB AS $$
DECLARE
    v_result JSONB;
BEGIN
    SELECT jsonb_build_object(
        'collection', p_collection_name,
        'state', s.state,
        'lambda_cut', s.lambda_cut,
        'threshold_high', p.threshold_high,
        'threshold_low', p.threshold_low,
        'last_sample', s.last_sample,
        'sample_count', s.sample_count,
        'current_policy', p.name,
        'allowed_operations', CASE s.state
            WHEN 'normal' THEN p.normal_actions
            WHEN 'stress' THEN p.stress_actions
            WHEN 'critical' THEN p.critical_actions
        END,
        'witness_edges', s.witness_edges
    ) INTO v_result
    FROM ruvector.collections c
    JOIN ruvector.integrity_state s ON c.id = s.collection_id
    LEFT JOIN ruvector.integrity_policies p ON c.id = p.collection_id AND p.enabled
    WHERE c.name = p_collection_name
    ORDER BY p.priority DESC
    LIMIT 1;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;
```

### Check Operation Permission

```sql
-- Check if operation is allowed
CREATE FUNCTION ruvector_integrity_gate(
    p_collection_name TEXT,
    p_operation TEXT
) RETURNS JSONB AS $$
DECLARE
    v_state TEXT;
    v_actions JSONB;
    v_allowed BOOLEAN;
    v_throttle_pct INTEGER;
    v_reason TEXT;
BEGIN
    -- Get current state and actions
    SELECT
        s.state,
        CASE s.state
            WHEN 'normal' THEN p.normal_actions
            WHEN 'stress' THEN p.stress_actions
            WHEN 'critical' THEN p.critical_actions
        END
    INTO v_state, v_actions
    FROM ruvector.collections c
    JOIN ruvector.integrity_state s ON c.id = s.collection_id
    LEFT JOIN ruvector.integrity_policies p ON c.id = p.collection_id AND p.enabled
    WHERE c.name = p_collection_name
    ORDER BY p.priority DESC
    LIMIT 1;

    -- Map operation to action key
    v_allowed := CASE p_operation
        WHEN 'search' THEN (v_actions->>'allow_reads')::boolean
        WHEN 'insert' THEN (v_actions->>'allow_single_insert')::boolean
        WHEN 'bulk_insert' THEN (v_actions->>'allow_bulk_insert')::boolean
        WHEN 'delete' THEN (v_actions->>'allow_delete')::boolean
        WHEN 'update' THEN (v_actions->>'allow_update')::boolean
        WHEN 'index_rewire' THEN (v_actions->>'allow_index_rewire')::boolean
        WHEN 'compression' THEN (v_actions->>'allow_compression')::boolean
        WHEN 'replication' THEN (v_actions->>'allow_replication')::boolean
        WHEN 'backup' THEN (v_actions->>'allow_backup')::boolean
        ELSE TRUE
    END;

    -- Get throttle percentage
    v_throttle_pct := CASE p_operation
        WHEN 'insert' THEN (v_actions->>'throttle_inserts_pct')::integer
        WHEN 'search' THEN (v_actions->>'throttle_searches_pct')::integer
        ELSE 0
    END;

    -- Generate reason if blocked
    IF NOT v_allowed THEN
        v_reason := format(
            'Operation %s blocked: system in %s state',
            p_operation, v_state
        );
    END IF;

    RETURN jsonb_build_object(
        'allowed', v_allowed,
        'throttle_pct', COALESCE(v_throttle_pct, 0),
        'state', v_state,
        'reason', v_reason,
        'actions', v_actions
    );
END;
$$ LANGUAGE plpgsql;
```

### Set Policy

```sql
-- Set or update integrity policy
CREATE FUNCTION ruvector_integrity_policy_set(
    p_collection_name TEXT,
    p_policy_name TEXT,
    p_config JSONB
) RETURNS JSONB AS $$
DECLARE
    v_collection_id INTEGER;
    v_old_policy JSONB;
    v_changes JSONB;
BEGIN
    -- Get collection
    SELECT id INTO v_collection_id
    FROM ruvector.collections WHERE name = p_collection_name;

    IF NOT FOUND THEN
        RETURN jsonb_build_object('error', 'Collection not found');
    END IF;

    -- Get old policy for change tracking
    SELECT row_to_json(p)::jsonb INTO v_old_policy
    FROM ruvector.integrity_policies p
    WHERE collection_id = v_collection_id AND name = p_policy_name;

    -- Upsert policy
    INSERT INTO ruvector.integrity_policies (
        collection_id, name,
        threshold_high, threshold_low,
        normal_actions, stress_actions, critical_actions,
        sample_interval_secs, sample_size, sample_method,
        notifications, enabled
    )
    VALUES (
        v_collection_id,
        p_policy_name,
        COALESCE((p_config->>'threshold_high')::real, 0.8),
        COALESCE((p_config->>'threshold_low')::real, 0.3),
        COALESCE(p_config->'normal_actions', '{}'::jsonb),
        COALESCE(p_config->'stress_actions', '{}'::jsonb),
        COALESCE(p_config->'critical_actions', '{}'::jsonb),
        COALESCE((p_config->>'sample_interval_secs')::integer, 60),
        COALESCE((p_config->>'sample_size')::integer, 1000),
        COALESCE(p_config->>'sample_method', 'random'),
        COALESCE(p_config->'notifications', '{}'::jsonb),
        COALESCE((p_config->>'enabled')::boolean, true)
    )
    ON CONFLICT (collection_id, name) DO UPDATE SET
        threshold_high = EXCLUDED.threshold_high,
        threshold_low = EXCLUDED.threshold_low,
        normal_actions = EXCLUDED.normal_actions,
        stress_actions = EXCLUDED.stress_actions,
        critical_actions = EXCLUDED.critical_actions,
        sample_interval_secs = EXCLUDED.sample_interval_secs,
        sample_size = EXCLUDED.sample_size,
        sample_method = EXCLUDED.sample_method,
        notifications = EXCLUDED.notifications,
        enabled = EXCLUDED.enabled,
        updated_at = NOW();

    -- Log policy update event
    INSERT INTO ruvector.integrity_events (
        collection_id, event_type, metadata
    )
    VALUES (
        v_collection_id,
        'policy_update',
        jsonb_build_object(
            'policy_name', p_policy_name,
            'old_policy', v_old_policy,
            'new_config', p_config,
            'source', 'api'
        )
    );

    RETURN jsonb_build_object(
        'success', true,
        'policy', p_policy_name,
        'collection', p_collection_name
    );
END;
$$ LANGUAGE plpgsql;
```

### Manual Override

```sql
-- Manually override integrity state (admin function)
CREATE FUNCTION ruvector_integrity_override(
    p_collection_name TEXT,
    p_new_state TEXT,
    p_reason TEXT,
    p_duration_secs INTEGER DEFAULT NULL
) RETURNS JSONB AS $$
DECLARE
    v_collection_id INTEGER;
    v_old_state TEXT;
BEGIN
    -- Validate state
    IF p_new_state NOT IN ('normal', 'stress', 'critical') THEN
        RETURN jsonb_build_object('error', 'Invalid state');
    END IF;

    -- Get collection and current state
    SELECT c.id, s.state
    INTO v_collection_id, v_old_state
    FROM ruvector.collections c
    JOIN ruvector.integrity_state s ON c.id = s.collection_id
    WHERE c.name = p_collection_name;

    IF NOT FOUND THEN
        RETURN jsonb_build_object('error', 'Collection not found');
    END IF;

    -- Update state
    UPDATE ruvector.integrity_state
    SET state = p_new_state,
        updated_at = NOW()
    WHERE collection_id = v_collection_id;

    -- Log override event
    INSERT INTO ruvector.integrity_events (
        collection_id, event_type,
        previous_state, new_state,
        metadata
    )
    VALUES (
        v_collection_id,
        'manual_override',
        v_old_state,
        p_new_state,
        jsonb_build_object(
            'reason', p_reason,
            'duration_secs', p_duration_secs,
            'operator', current_user,
            'source', 'admin_api'
        )
    );

    -- If duration specified, schedule revert
    IF p_duration_secs IS NOT NULL THEN
        -- Implementation: use pg_cron or similar
        PERFORM ruvector_schedule_state_revert(
            v_collection_id,
            v_old_state,
            NOW() + (p_duration_secs || ' seconds')::interval
        );
    END IF;

    RETURN jsonb_build_object(
        'success', true,
        'collection', p_collection_name,
        'previous_state', v_old_state,
        'new_state', p_new_state,
        'auto_revert_at', CASE
            WHEN p_duration_secs IS NOT NULL
            THEN NOW() + (p_duration_secs || ' seconds')::interval
            ELSE NULL
        END
    );
END;
$$ LANGUAGE plpgsql;
```

### Query Event History

```sql
-- Get integrity event history
CREATE FUNCTION ruvector_integrity_history(
    p_collection_name TEXT,
    p_event_type TEXT DEFAULT NULL,
    p_since TIMESTAMPTZ DEFAULT NOW() - INTERVAL '24 hours',
    p_limit INTEGER DEFAULT 100
) RETURNS TABLE (
    id BIGINT,
    event_type TEXT,
    previous_state TEXT,
    new_state TEXT,
    lambda_cut REAL,
    witness_edge_count INTEGER,
    metadata JSONB,
    is_signed BOOLEAN,
    created_at TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        e.id,
        e.event_type,
        e.previous_state,
        e.new_state,
        e.lambda_cut,
        jsonb_array_length(COALESCE(e.witness_edges, '[]'::jsonb)),
        e.metadata,
        e.signature IS NOT NULL,
        e.created_at
    FROM ruvector.integrity_events e
    JOIN ruvector.collections c ON e.collection_id = c.id
    WHERE c.name = p_collection_name
      AND e.created_at >= p_since
      AND (p_event_type IS NULL OR e.event_type = p_event_type)
    ORDER BY e.created_at DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;
```

---

## Rust Implementation

### Contracted Graph Sampling

```rust
/// Sample the contracted operational graph
pub fn sample_contracted_graph(
    collection_id: i32,
    sample_size: usize,
) -> Result<ContractedGraphSample, Error> {
    // Query contracted graph from database
    let nodes = Spi::connect(|client| {
        client.select(
            "SELECT node_type, node_id, node_data
             FROM ruvector.contracted_graph
             WHERE collection_id = $1",
            None,
            &[collection_id.into()],
        )?.map(|row| {
            ContractedNode {
                node_type: row.get::<String>(1)?,
                id: row.get::<i64>(2)?,
                data: row.get::<JsonB>(3)?,
            }
        }).collect()
    })?;

    let edges = Spi::connect(|client| {
        client.select(
            "SELECT source_type, source_id, target_type, target_id,
                    edge_type, capacity
             FROM ruvector.contracted_edges
             WHERE collection_id = $1",
            None,
            &[collection_id.into()],
        )?.map(|row| {
            ContractedEdge {
                source_type: row.get::<String>(1)?,
                source_id: row.get::<i64>(2)?,
                target_type: row.get::<String>(3)?,
                target_id: row.get::<i64>(4)?,
                edge_type: row.get::<String>(5)?,
                capacity: row.get::<f32>(6)?,
            }
        }).collect()
    })?;

    // Random sampling if too large
    let sampled_edges = if edges.len() > sample_size {
        let mut rng = rand::thread_rng();
        edges.choose_multiple(&mut rng, sample_size)
            .cloned()
            .collect()
    } else {
        edges
    };

    // Find witness edges (those in the mincut)
    let witness_edges = compute_mincut_edges(&nodes, &sampled_edges)?;

    Ok(ContractedGraphSample {
        collection_id,
        nodes,
        edges: sampled_edges,
        witness_edges,
        sampled_at: Utc::now(),
    })
}

/// Compute the edges that form the mincut
fn compute_mincut_edges(
    nodes: &[ContractedNode],
    edges: &[ContractedEdge],
) -> Result<Vec<WitnessEdge>, Error> {
    // Build graph for max-flow computation
    let n = nodes.len();
    let node_map: HashMap<(String, i64), usize> = nodes.iter()
        .enumerate()
        .map(|(i, n)| ((n.node_type.clone(), n.id), i))
        .collect();

    // Use push-relabel or similar for max-flow/min-cut
    // The mincut edges are those saturated in the max-flow

    // For now, simplified: find edges with capacity < threshold
    let threshold = 0.5;
    let witness = edges.iter()
        .filter(|e| e.capacity < threshold)
        .map(|e| WitnessEdge {
            source: WitnessNode {
                node_type: e.source_type.clone(),
                id: e.source_id,
                name: None,
            },
            target: WitnessNode {
                node_type: e.target_type.clone(),
                id: e.target_id,
                name: None,
            },
            edge_type: e.edge_type.clone(),
            capacity: e.capacity,
            flow: e.capacity,  // Saturated
        })
        .collect();

    Ok(witness)
}
```

### Gate Check Implementation

```rust
/// Check if operation is allowed by integrity gate
#[pg_extern]
pub fn ruvector_integrity_gate(
    collection_name: &str,
    operation: &str,
) -> pgrx::JsonB {
    // Get cached state from shared memory (fast path)
    let shmem = SharedMemory::get();

    let state = shmem.get_integrity_state(collection_name);
    let actions = shmem.get_integrity_actions(collection_name);

    let (allowed, throttle_pct) = match operation {
        "search" => (actions.allow_reads, actions.throttle_searches_pct),
        "insert" => (actions.allow_single_insert, actions.throttle_inserts_pct),
        "bulk_insert" => (actions.allow_bulk_insert, 100),  // All or nothing
        "delete" => (actions.allow_delete, 0),
        "update" => (actions.allow_update, 0),
        "index_rewire" => (actions.allow_index_rewire, 0),
        "compression" => (actions.allow_compression, 0),
        "replication" => (actions.allow_replication, 0),
        "backup" => (actions.allow_backup, 0),
        _ => (true, 0),
    };

    let reason = if !allowed {
        Some(format!(
            "Operation '{}' blocked: system in {} state",
            operation, state
        ))
    } else {
        None
    };

    pgrx::JsonB(serde_json::json!({
        "allowed": allowed,
        "throttle_pct": throttle_pct,
        "state": state.to_string(),
        "reason": reason,
    }))
}
```

---

## Testing Requirements

### Unit Tests
- Policy JSON validation
- State transition logic
- Lambda cut computation
- Signature verification

### Integration Tests
- Full sample-compute-update cycle
- Policy application
- Event persistence
- Notification delivery

### Chaos Tests
- Network partition simulation
- Node failure scenarios
- Recovery behavior

---

## Monitoring Queries

```sql
-- Recent state changes
SELECT * FROM ruvector_integrity_history('my_collection', 'state_change');

-- Current system health
SELECT
    c.name,
    s.state,
    s.lambda_cut,
    s.last_sample,
    NOW() - s.last_sample AS sample_age
FROM ruvector.collections c
JOIN ruvector.integrity_state s ON c.id = s.collection_id
ORDER BY s.lambda_cut ASC;  -- Most stressed first

-- Unsigned events (potential tampering)
SELECT * FROM ruvector.integrity_events
WHERE signature IS NULL
  AND event_type = 'state_change'
ORDER BY created_at DESC;

-- Policy effectiveness
SELECT
    event_type,
    new_state,
    COUNT(*) as occurrences,
    AVG(lambda_cut) as avg_lambda
FROM ruvector.integrity_events
WHERE created_at > NOW() - INTERVAL '7 days'
GROUP BY event_type, new_state;
```
