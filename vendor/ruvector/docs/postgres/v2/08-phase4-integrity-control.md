# RuVector Postgres v2 - Phase 4: Integrity Control Plane

## Overview

Phase 4 implements the **Dynamic Mincut Integrity Gating** system - the key differentiator for RuVector v2. This control plane monitors system health via graph connectivity analysis and gates operations based on integrity state.

---

## Objectives

### Primary Goals
1. Contracted operational graph construction
2. Lambda cut (minimum cut value) computation + optional λ₂ (spectral stress)
3. Policy-based operation gating with hysteresis
4. Signed audit event trail with operation classification

### Success Criteria
- Real-time integrity state updates
- < 100ms gating check latency
- Cryptographic audit trail
- Zero false positives in critical state

---

## Critical Design Constraint

```
NEVER compute mincut on full similarity graph!
Always use the contracted operational graph.

Full graph: N vectors = O(N^2) potential edges
             1M vectors = 1 trillion edges = IMPOSSIBLE

Contracted graph: Fixed size ~1000 nodes
                  Partitions, centroids, shards, dependencies
                  Always tractable: O(1000^2) = 1M edges max
```

---

## Architecture

### Integrity Control Flow

```
+------------------------------------------------------------------+
|                    Operation Request                              |
|  (INSERT, BULK_INSERT, INDEX_REWIRE, COMPRESSION, etc.)          |
+------------------------------------------------------------------+
                              |
                              v
+------------------------------------------------------------------+
|                    Integrity Gate Check                           |
|                                                                   |
|  1. Read current state from shared memory (fast path)            |
|  2. Classify operation risk (low/medium/high)                    |
|  3. Look up policy for operation + risk combination              |
|  4. Return: allow / throttle(factor) / defer(secs) / reject      |
+------------------------------------------------------------------+
                              |
              +-------+-------+-------+-------+
              |       |       |       |       |
              v       v       v       v       v
         +-------+ +-------+ +-------+ +-------+
         | ALLOW | |THROTTLE| | DEFER | |REJECT |
         +-------+ +-------+ +-------+ +-------+


Background: Integrity Worker (continuous)
+------------------------------------------------------------------+
| SAMPLER (every 10-60s):                                           |
|   1. Sample contracted graph edges                                |
|   2. Update edge capacities from metrics                          |
|                                                                   |
| COMPUTER (every 1-5m):                                            |
|   3. Compute λ_cut (minimum cut value) via Stoer-Wagner          |
|   4. Optionally compute λ₂ (spectral stress) via Lanczos         |
|                                                                   |
| CONTROLLER (event-driven):                                        |
|   5. Apply hysteresis to state transitions                        |
|   6. If state changed:                                            |
|      - Log signed event                                           |
|      - Update shared memory permissions                           |
|      - Notify waiting operations                                  |
+------------------------------------------------------------------+
```

### Contracted Graph Structure

```
                        +------------------+
                        |   Contracted     |
                        |     Graph        |
                        +--------+---------+
                                 |
              +------------------+------------------+
              |                  |                  |
    +---------v---------+  +-----v------+  +-------v-------+
    |    Partitions     |  |  Centroids |  |    Shards     |
    | (data segments)   |  | (IVFFlat)  |  | (distributed) |
    +-------------------+  +------------+  +---------------+
              |                  |                  |
              +------------------+------------------+
                                 |
                        +--------v---------+
                        |  Maintenance     |
                        |  Dependencies    |
                        +------------------+

    Edge Types:
    - partition_link: Data flow between partitions
    - routing_link: Query routing paths
    - dependency: Operational dependencies
    - replication: Replication streams
```

---

## Deliverables

### 1. Contracted Graph Schema

```sql
-- Contracted graph nodes (small, fixed size)
CREATE TABLE ruvector.contracted_graph (
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    node_type       TEXT NOT NULL CHECK (node_type IN (
        'partition',        -- Data partition/segment
        'centroid',         -- IVFFlat centroid
        'shard',            -- Distributed shard
        'maintenance_dep',  -- Maintenance dependency
        'replication_target' -- Replication endpoint
    )),
    node_id         BIGINT NOT NULL,
    node_name       TEXT,
    node_data       JSONB NOT NULL DEFAULT '{}'::jsonb,
    health_score    REAL NOT NULL DEFAULT 1.0,  -- 0.0 = failed, 1.0 = healthy
    last_updated    TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (collection_id, node_type, node_id)
);

CREATE INDEX idx_contracted_graph_health
    ON ruvector.contracted_graph(collection_id, health_score);

-- Contracted graph edges
CREATE TABLE ruvector.contracted_edges (
    id              BIGSERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id) ON DELETE CASCADE,

    -- Source node
    source_type     TEXT NOT NULL,
    source_id       BIGINT NOT NULL,

    -- Target node
    target_type     TEXT NOT NULL,
    target_id       BIGINT NOT NULL,

    -- Edge properties
    edge_type       TEXT NOT NULL CHECK (edge_type IN (
        'partition_link',   -- Data flow
        'routing_link',     -- Query routing
        'dependency',       -- Operational dependency
        'replication'       -- Replication stream
    )),
    capacity        REAL NOT NULL DEFAULT 1.0,  -- Max-flow capacity
    current_flow    REAL NOT NULL DEFAULT 0.0,  -- Current utilization
    latency_ms      REAL,                       -- Edge latency
    error_rate      REAL NOT NULL DEFAULT 0.0,  -- Recent error rate

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    FOREIGN KEY (collection_id, source_type, source_id)
        REFERENCES ruvector.contracted_graph(collection_id, node_type, node_id)
        ON DELETE CASCADE,
    FOREIGN KEY (collection_id, target_type, target_id)
        REFERENCES ruvector.contracted_graph(collection_id, node_type, node_id)
        ON DELETE CASCADE
);

CREATE INDEX idx_contracted_edges_source
    ON ruvector.contracted_edges(collection_id, source_type, source_id);
CREATE INDEX idx_contracted_edges_target
    ON ruvector.contracted_edges(collection_id, target_type, target_id);
CREATE INDEX idx_contracted_edges_capacity
    ON ruvector.contracted_edges(collection_id, capacity);
```

### 2. Contracted Graph Builder

```rust
// src/integrity/contracted_graph.rs

use std::collections::HashMap;

/// Build contracted graph from collection state
pub struct ContractedGraphBuilder {
    collection_id: i32,
}

impl ContractedGraphBuilder {
    pub fn new(collection_id: i32) -> Self {
        Self { collection_id }
    }

    /// Build or update the contracted graph
    pub fn build(&self) -> Result<ContractedGraph, Error> {
        // Clear existing graph
        self.clear_graph()?;

        // Build nodes from different sources
        let partition_nodes = self.build_partition_nodes()?;
        let centroid_nodes = self.build_centroid_nodes()?;
        let shard_nodes = self.build_shard_nodes()?;
        let maintenance_nodes = self.build_maintenance_nodes()?;

        // Build edges
        let edges = self.build_edges(
            &partition_nodes,
            &centroid_nodes,
            &shard_nodes,
            &maintenance_nodes,
        )?;

        // Persist to database
        self.persist_nodes(&partition_nodes)?;
        self.persist_nodes(&centroid_nodes)?;
        self.persist_nodes(&shard_nodes)?;
        self.persist_nodes(&maintenance_nodes)?;
        self.persist_edges(&edges)?;

        Ok(ContractedGraph {
            nodes: [partition_nodes, centroid_nodes, shard_nodes, maintenance_nodes]
                .concat(),
            edges,
        })
    }

    /// Build partition nodes from index segments
    fn build_partition_nodes(&self) -> Result<Vec<ContractedNode>, Error> {
        Spi::connect(|client| {
            // Query index segments/partitions
            let result = client.select(
                "SELECT
                    partition_id,
                    vector_count,
                    size_bytes,
                    last_access
                 FROM ruvector.partitions  -- hypothetical table
                 WHERE collection_id = $1",
                None,
                &[self.collection_id.into()],
            )?;

            result.map(|row| {
                let partition_id: i64 = row.get(1)?;
                let vector_count: i64 = row.get(2)?;
                let size_bytes: i64 = row.get(3)?;

                Ok(ContractedNode {
                    collection_id: self.collection_id,
                    node_type: NodeType::Partition,
                    node_id: partition_id,
                    node_name: Some(format!("partition_{}", partition_id)),
                    node_data: serde_json::json!({
                        "vector_count": vector_count,
                        "size_bytes": size_bytes,
                    }),
                    health_score: 1.0,
                })
            }).collect()
        })
    }

    /// Build centroid nodes from IVFFlat index
    fn build_centroid_nodes(&self) -> Result<Vec<ContractedNode>, Error> {
        Spi::connect(|client| {
            let result = client.select(
                "SELECT
                    list_id,
                    vector_count,
                    avg_distance_to_centroid
                 FROM ruvector.ivf_lists  -- hypothetical table
                 WHERE collection_id = $1
                 ORDER BY vector_count DESC
                 LIMIT 1000",  -- Cap at 1000 centroids
                None,
                &[self.collection_id.into()],
            )?;

            result.map(|row| {
                let list_id: i64 = row.get(1)?;
                let vector_count: i64 = row.get(2)?;
                let avg_distance: f32 = row.get::<Option<f32>>(3)?.unwrap_or(0.0);

                // Health based on cluster quality
                let health = if avg_distance > 0.0 {
                    (1.0 / (1.0 + avg_distance)).min(1.0)
                } else {
                    1.0
                };

                Ok(ContractedNode {
                    collection_id: self.collection_id,
                    node_type: NodeType::Centroid,
                    node_id: list_id,
                    node_name: Some(format!("centroid_{}", list_id)),
                    node_data: serde_json::json!({
                        "vector_count": vector_count,
                        "avg_distance": avg_distance,
                    }),
                    health_score: health,
                })
            }).collect()
        })
    }

    /// Build shard nodes for distributed deployment
    fn build_shard_nodes(&self) -> Result<Vec<ContractedNode>, Error> {
        // In single-node mode, return single shard
        // In distributed mode, query shard registry
        Ok(vec![ContractedNode {
            collection_id: self.collection_id,
            node_type: NodeType::Shard,
            node_id: 0,
            node_name: Some("primary_shard".to_string()),
            node_data: serde_json::json!({"type": "primary"}),
            health_score: 1.0,
        }])
    }

    /// Build maintenance dependency nodes
    fn build_maintenance_nodes(&self) -> Result<Vec<ContractedNode>, Error> {
        // Dependencies like: backup, compaction, GNN training
        Ok(vec![
            ContractedNode {
                collection_id: self.collection_id,
                node_type: NodeType::MaintenanceDep,
                node_id: 1,
                node_name: Some("backup_service".to_string()),
                node_data: serde_json::json!({"type": "backup"}),
                health_score: check_backup_health()?,
            },
            ContractedNode {
                collection_id: self.collection_id,
                node_type: NodeType::MaintenanceDep,
                node_id: 2,
                node_name: Some("compaction_service".to_string()),
                node_data: serde_json::json!({"type": "compaction"}),
                health_score: check_compaction_health()?,
            },
        ])
    }

    /// Build edges between nodes
    fn build_edges(
        &self,
        partitions: &[ContractedNode],
        centroids: &[ContractedNode],
        shards: &[ContractedNode],
        maintenance: &[ContractedNode],
    ) -> Result<Vec<ContractedEdge>, Error> {
        let mut edges = Vec::new();

        // Partition-to-partition links (data flow)
        for i in 0..partitions.len() {
            for j in (i+1)..partitions.len() {
                edges.push(ContractedEdge {
                    collection_id: self.collection_id,
                    source_type: NodeType::Partition,
                    source_id: partitions[i].node_id,
                    target_type: NodeType::Partition,
                    target_id: partitions[j].node_id,
                    edge_type: EdgeType::PartitionLink,
                    capacity: 1.0,
                    current_flow: 0.0,
                    latency_ms: None,
                    error_rate: 0.0,
                });
            }
        }

        // Centroid-to-shard links (routing)
        for centroid in centroids {
            for shard in shards {
                edges.push(ContractedEdge {
                    collection_id: self.collection_id,
                    source_type: NodeType::Centroid,
                    source_id: centroid.node_id,
                    target_type: NodeType::Shard,
                    target_id: shard.node_id,
                    edge_type: EdgeType::RoutingLink,
                    capacity: centroid.health_score,
                    current_flow: 0.0,
                    latency_ms: None,
                    error_rate: 0.0,
                });
            }
        }

        // Shard-to-maintenance dependencies
        for shard in shards {
            for maint in maintenance {
                edges.push(ContractedEdge {
                    collection_id: self.collection_id,
                    source_type: NodeType::Shard,
                    source_id: shard.node_id,
                    target_type: NodeType::MaintenanceDep,
                    target_id: maint.node_id,
                    edge_type: EdgeType::Dependency,
                    capacity: maint.health_score,
                    current_flow: 0.0,
                    latency_ms: None,
                    error_rate: 0.0,
                });
            }
        }

        Ok(edges)
    }

    fn clear_graph(&self) -> Result<(), Error> {
        Spi::run(|client| {
            client.update(
                "DELETE FROM ruvector.contracted_edges WHERE collection_id = $1",
                None,
                &[self.collection_id.into()],
            )?;
            client.update(
                "DELETE FROM ruvector.contracted_graph WHERE collection_id = $1",
                None,
                &[self.collection_id.into()],
            )
        })
    }

    fn persist_nodes(&self, nodes: &[ContractedNode]) -> Result<(), Error> {
        Spi::run(|client| {
            for node in nodes {
                client.update(
                    "INSERT INTO ruvector.contracted_graph
                     (collection_id, node_type, node_id, node_name, node_data, health_score)
                     VALUES ($1, $2, $3, $4, $5, $6)
                     ON CONFLICT (collection_id, node_type, node_id) DO UPDATE SET
                         node_name = EXCLUDED.node_name,
                         node_data = EXCLUDED.node_data,
                         health_score = EXCLUDED.health_score,
                         last_updated = NOW()",
                    None,
                    &[
                        node.collection_id.into(),
                        node.node_type.to_string().into(),
                        node.node_id.into(),
                        node.node_name.clone().into(),
                        pgrx::JsonB(node.node_data.clone()).into(),
                        node.health_score.into(),
                    ],
                )?;
            }
            Ok(())
        })
    }

    fn persist_edges(&self, edges: &[ContractedEdge]) -> Result<(), Error> {
        Spi::run(|client| {
            for edge in edges {
                client.update(
                    "INSERT INTO ruvector.contracted_edges
                     (collection_id, source_type, source_id, target_type, target_id,
                      edge_type, capacity, current_flow, latency_ms, error_rate)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                    None,
                    &[
                        edge.collection_id.into(),
                        edge.source_type.to_string().into(),
                        edge.source_id.into(),
                        edge.target_type.to_string().into(),
                        edge.target_id.into(),
                        edge.edge_type.to_string().into(),
                        edge.capacity.into(),
                        edge.current_flow.into(),
                        edge.latency_ms.into(),
                        edge.error_rate.into(),
                    ],
                )?;
            }
            Ok(())
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Partition,
    Centroid,
    Shard,
    MaintenanceDep,
    ReplicationTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    PartitionLink,
    RoutingLink,
    Dependency,
    Replication,
}

#[derive(Debug, Clone)]
pub struct ContractedNode {
    pub collection_id: i32,
    pub node_type: NodeType,
    pub node_id: i64,
    pub node_name: Option<String>,
    pub node_data: serde_json::Value,
    pub health_score: f32,
}

#[derive(Debug, Clone)]
pub struct ContractedEdge {
    pub collection_id: i32,
    pub source_type: NodeType,
    pub source_id: i64,
    pub target_type: NodeType,
    pub target_id: i64,
    pub edge_type: EdgeType,
    pub capacity: f32,
    pub current_flow: f32,
    pub latency_ms: Option<f32>,
    pub error_rate: f32,
}

#[derive(Debug, Clone)]
pub struct ContractedGraph {
    pub nodes: Vec<ContractedNode>,
    pub edges: Vec<ContractedEdge>,
}
```

### 3. Mincut Computation

```rust
// src/integrity/mincut.rs

use std::collections::HashMap;

/// Compute minimum cut value (NOT algebraic connectivity) on contracted graph.
/// Uses Stoer-Wagner algorithm for global mincut.
///
/// KEY DISTINCTION:
/// - lambda_cut: Minimum cut value from Stoer-Wagner - PRIMARY integrity metric
/// - lambda2: Algebraic connectivity (2nd eigenvalue of Laplacian) - OPTIONAL drift signal
///
/// These are DIFFERENT concepts. Do not confuse them!
pub struct MincutComputer {
    /// Also compute lambda2 (spectral stress) as drift signal
    compute_lambda2: bool,
}

impl MincutComputer {
    pub fn new(compute_lambda2: bool) -> Self {
        Self { compute_lambda2 }
    }

    /// Compute lambda_cut (minimum cut value) - PRIMARY METRIC
    /// Optionally compute lambda2 (algebraic connectivity) - DRIFT SIGNAL
    pub fn compute(&self, graph: &ContractedGraph) -> MincutResult {
        let n = graph.nodes.len();

        if n < 2 {
            return MincutResult {
                lambda_cut: 0.0,
                lambda2: None,
                witness_edges: vec![],
                computation_time_ms: 0,
            };
        }

        let start = std::time::Instant::now();

        // Build node index
        let node_index: HashMap<_, _> = graph.nodes.iter()
            .enumerate()
            .map(|(i, n)| ((n.node_type, n.node_id), i))
            .collect();

        // Build capacity matrix
        let mut capacity = vec![vec![0.0f64; n]; n];
        for edge in &graph.edges {
            if let (Some(&i), Some(&j)) = (
                node_index.get(&(edge.source_type, edge.source_id)),
                node_index.get(&(edge.target_type, edge.target_id)),
            ) {
                let cap = edge.capacity as f64 * (1.0 - edge.error_rate as f64);
                capacity[i][j] = cap;
                capacity[j][i] = cap;  // Undirected
            }
        }

        // Compute global mincut using Stoer-Wagner
        let (lambda_cut, cut_partition) = self.stoer_wagner_mincut(&capacity);

        // Find witness edges (edges crossing the cut)
        let witness_edges = self.find_witness_edges(graph, &node_index, &capacity, &cut_partition);

        // Optionally compute lambda2 (spectral stress)
        let lambda2 = if self.compute_lambda2 {
            Some(self.compute_algebraic_connectivity(&capacity, n))
        } else {
            None
        };

        let computation_time_ms = start.elapsed().as_millis() as u64;

        MincutResult {
            lambda_cut: lambda_cut as f32,
            lambda2: lambda2.map(|v| v as f32),
            witness_edges,
            computation_time_ms,
        }
    }

    /// Stoer-Wagner algorithm for global minimum cut
    /// Returns (mincut_value, partition of nodes on one side)
    fn stoer_wagner_mincut(&self, capacity: &[Vec<f64>]) -> (f64, Vec<usize>) {
        let n = capacity.len();
        let mut best_cut = f64::MAX;
        let mut best_partition = vec![];

        // Working copies
        let mut vertices: Vec<usize> = (0..n).collect();
        let mut merged: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
        let mut cap = capacity.to_vec();

        while vertices.len() > 1 {
            // Maximum adjacency search to find s-t cut
            let (s, t, cut_of_phase) = self.minimum_cut_phase(&vertices, &cap);

            if cut_of_phase < best_cut {
                best_cut = cut_of_phase;
                best_partition = merged[vertices[t]].clone();
            }

            // Merge t into s
            let t_idx = vertices[t];
            let s_idx = vertices[s];

            // Update capacities
            for &v in &vertices {
                if v != s_idx && v != t_idx {
                    cap[s_idx][v] += cap[t_idx][v];
                    cap[v][s_idx] += cap[v][t_idx];
                }
            }

            // Merge vertex sets
            merged[s_idx].extend(merged[t_idx].clone());

            // Remove t from active vertices
            vertices.remove(t);
        }

        (best_cut, best_partition)
    }

    /// One phase of Stoer-Wagner: find minimum s-t cut
    fn minimum_cut_phase(&self, vertices: &[usize], cap: &[Vec<f64>]) -> (usize, usize, f64) {
        let mut in_a = vec![false; cap.len()];
        let mut cut_weight = vec![0.0f64; cap.len()];

        let mut last = 0;
        let mut before_last = 0;

        for i in 0..vertices.len() {
            // Find most tightly connected vertex
            let mut max_weight = -1.0;
            let mut max_v = 0;

            for (idx, &v) in vertices.iter().enumerate() {
                if !in_a[v] && cut_weight[v] > max_weight {
                    max_weight = cut_weight[v];
                    max_v = idx;
                }
            }

            in_a[vertices[max_v]] = true;
            before_last = last;
            last = max_v;

            // Update cut weights
            for (idx, &v) in vertices.iter().enumerate() {
                if !in_a[v] {
                    cut_weight[v] += cap[vertices[max_v]][v];
                }
            }
        }

        (before_last, last, cut_weight[vertices[last]])
    }

    /// Find edges crossing the minimum cut (witness edges)
    fn find_witness_edges(
        &self,
        graph: &ContractedGraph,
        node_index: &HashMap<(NodeType, i64), usize>,
        capacity: &[Vec<f64>],
        partition: &[usize],
    ) -> Vec<WitnessEdge> {
        use std::collections::HashSet;
        let partition_set: HashSet<_> = partition.iter().collect();

        graph.edges.iter()
            .filter_map(|edge| {
                let i = node_index.get(&(edge.source_type, edge.source_id))?;
                let j = node_index.get(&(edge.target_type, edge.target_id))?;

                // Edge crosses cut if exactly one endpoint in partition
                let i_in = partition_set.contains(i);
                let j_in = partition_set.contains(j);

                if i_in != j_in {
                    Some(WitnessEdge {
                        source_type: edge.source_type.to_string(),
                        source_id: edge.source_id,
                        target_type: edge.target_type.to_string(),
                        target_id: edge.target_id,
                        edge_type: edge.edge_type.to_string(),
                        capacity: edge.capacity,
                        flow: edge.current_flow,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Compute algebraic connectivity (lambda2) as optional drift signal
    /// This is DIFFERENT from mincut - provides spectral stress insight
    fn compute_algebraic_connectivity(&self, capacity: &[Vec<f64>], n: usize) -> f64 {
        // Build Laplacian: L = D - A
        let mut laplacian = vec![vec![0.0f64; n]; n];
        for i in 0..n {
            let degree: f64 = capacity[i].iter().sum();
            laplacian[i][i] = degree;
            for j in 0..n {
                laplacian[i][j] -= capacity[i][j];
            }
        }

        // Power iteration for second smallest eigenvalue
        // (Simplified - production should use ARPACK)

        // Inverse power iteration to find smallest non-zero eigenvalue
        for _ in 0..self.max_iterations {
            // Solve (L + shift*I) * w = v
            let w = lu.solve(&v).unwrap_or(v.clone());

            // Orthogonalize against constant vector
            let mean = w.mean();
            let mut v_new = w.add_scalar(-mean);
            v_new.normalize_mut();

            // Check convergence
            let diff = (&v_new - &v).norm();
            v = v_new;

            if diff < self.tolerance {
                break;
            }
        }

        // Rayleigh quotient gives eigenvalue estimate
        let lv = laplacian * &v;
        let lambda = v.dot(&lv) / v.dot(&v);

        lambda.max(0.0)  // Ensure non-negative
    }

}

#[derive(Debug, Clone)]
pub struct MincutResult {
    pub lambda_cut: f32,           // Minimum cut value (PRIMARY METRIC)
    pub lambda2: Option<f32>,      // Algebraic connectivity (OPTIONAL DRIFT SIGNAL)
    pub witness_edges: Vec<WitnessEdge>,
    pub computation_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessEdge {
    pub source_type: String,
    pub source_id: i64,
    pub target_type: String,
    pub target_id: i64,
    pub edge_type: String,
    pub capacity: f32,
    pub flow: f32,
}
```

### 4. Integrity Worker

```rust
// src/integrity/worker.rs

/// Background worker for continuous integrity monitoring
#[pg_guard]
pub extern "C" fn ruvector_integrity_worker_main(_arg: pg_sys::Datum) {
    pgrx::log!("RuVector integrity worker starting");

    let config = IntegrityWorkerConfig::default();
    let computer = LambdaCutComputer::new();

    // Load or generate signing key
    let signing_key = load_or_generate_signing_key();

    loop {
        if unsafe { pg_sys::ShutdownRequestPending } {
            break;
        }

        // Get collections with integrity policies
        let collections = match get_integrity_collections() {
            Ok(c) => c,
            Err(e) => {
                pgrx::warning!("Failed to get collections: {}", e);
                sleep_interruptible(config.sample_interval_secs);
                continue;
            }
        };

        for collection in collections {
            // Rebuild contracted graph if stale
            if should_rebuild_graph(&collection) {
                if let Err(e) = rebuild_contracted_graph(collection.id) {
                    pgrx::warning!(
                        "Failed to rebuild contracted graph for {}: {}",
                        collection.name, e
                    );
                    continue;
                }
            }

            // Load contracted graph
            let graph = match load_contracted_graph(collection.id) {
                Ok(g) => g,
                Err(e) => {
                    pgrx::warning!(
                        "Failed to load contracted graph for {}: {}",
                        collection.name, e
                    );
                    continue;
                }
            };

            // Compute lambda cut
            let result = computer.compute(&graph);

            pgrx::debug1!(
                "Lambda cut for {}: {:.4} (computed in {}ms)",
                collection.name,
                result.lambda_cut,
                result.computation_time_ms
            );

            // Get current state
            let current_state = get_current_state(collection.id);

            // Determine new state
            let policy = get_active_policy(collection.id);
            let new_state = IntegrityState::from_lambda(
                result.lambda_cut as f64,
                policy.threshold_high as f64,
                policy.threshold_low as f64,
            );

            // Handle state transition
            if new_state != current_state.state {
                handle_state_transition(
                    collection.id,
                    &collection.name,
                    &current_state,
                    new_state,
                    result.lambda_cut,
                    &result.witness_edges,
                    &signing_key,
                );
            }

            // Update state
            update_integrity_state(
                collection.id,
                new_state,
                result.lambda_cut,
                &result.witness_edges,
            );

            // Update shared memory for fast gate checks
            update_shared_memory_state(collection.id, new_state, &policy);

            // Record sample event
            record_sample_event(collection.id, result.lambda_cut);
        }

        sleep_interruptible(config.sample_interval_secs);
    }

    pgrx::log!("RuVector integrity worker stopped");
}

fn handle_state_transition(
    collection_id: i32,
    collection_name: &str,
    current: &IntegrityStateRecord,
    new_state: IntegrityState,
    lambda_cut: f32,
    witness_edges: &[WitnessEdge],
    signing_key: &ed25519_dalek::SigningKey,
) {
    pgrx::log!(
        "Integrity state transition for {}: {} -> {} (lambda={:.4})",
        collection_name,
        current.state,
        new_state,
        lambda_cut
    );

    // Create event
    let event = IntegrityEventContent {
        collection_id,
        event_type: "state_change".to_string(),
        previous_state: Some(current.state.to_string()),
        new_state: Some(new_state.to_string()),
        lambda_cut: Some(lambda_cut),
        witness_edges: Some(witness_edges.to_vec()),
        metadata: serde_json::json!({
            "transition_time": chrono::Utc::now().to_rfc3339(),
            "direction": if new_state > current.state { "degrading" } else { "improving" },
        }),
        created_at: chrono::Utc::now(),
    };

    // Sign event
    let signed_event = SignedIntegrityEvent::sign(
        event,
        signing_key,
        "integrity-worker",
    );

    // Persist signed event
    if let Err(e) = persist_signed_event(&signed_event) {
        pgrx::warning!("Failed to persist integrity event: {}", e);
    }

    // Apply policy actions
    let policy = get_active_policy(collection_id);
    apply_policy_actions(collection_id, new_state, &policy);

    // Send notifications
    if let Some(notifications) = policy.notifications {
        send_notifications(collection_id, collection_name, &signed_event, &notifications);
    }
}

fn apply_policy_actions(
    collection_id: i32,
    state: IntegrityState,
    policy: &IntegrityPolicy,
) {
    let actions = match state {
        IntegrityState::Normal => &policy.normal_actions,
        IntegrityState::Stress => &policy.stress_actions,
        IntegrityState::Critical => &policy.critical_actions,
    };

    // Update operation permissions in shared memory
    let shmem = SharedMemory::get();
    shmem.update_permissions(collection_id, actions);

    // Execute any immediate actions
    if actions.get("emergency_compact").and_then(|v| v.as_bool()).unwrap_or(false) {
        spawn_emergency_compaction(collection_id);
    }

    if actions.get("pause_gnn_training").and_then(|v| v.as_bool()).unwrap_or(false) {
        signal_pause_gnn_training(collection_id);
    }

    if actions.get("pause_tier_management").and_then(|v| v.as_bool()).unwrap_or(false) {
        signal_pause_tier_management(collection_id);
    }
}

#[derive(Debug, Clone)]
struct IntegrityWorkerConfig {
    sample_interval_secs: u64,
    graph_rebuild_interval_secs: u64,
}

impl Default for IntegrityWorkerConfig {
    fn default() -> Self {
        Self {
            sample_interval_secs: 60,
            graph_rebuild_interval_secs: 3600,
        }
    }
}
```

### 5. Integrity Gate

```rust
// src/integrity/gate.rs

/// Fast integrity gate check using shared memory
pub fn check_integrity_gate(
    collection_id: i32,
    operation: &str,
) -> GateResult {
    // Fast path: read from shared memory
    let shmem = SharedMemory::get();

    let state = shmem.get_integrity_state(collection_id);
    let permissions = shmem.get_permissions(collection_id);

    // Map operation to permission key
    let allowed = match operation {
        "search" | "read" => permissions.allow_reads,
        "insert" => permissions.allow_single_insert,
        "bulk_insert" => permissions.allow_bulk_insert,
        "delete" => permissions.allow_delete,
        "update" => permissions.allow_update,
        "index_build" | "index_rewire" => permissions.allow_index_rewire,
        "compression" | "compact" => permissions.allow_compression,
        "replication" => permissions.allow_replication,
        "backup" => permissions.allow_backup,
        "gnn_train" => !permissions.pause_gnn_training,
        "tier_manage" => !permissions.pause_tier_management,
        _ => true,  // Unknown operations allowed by default
    };

    // Get throttle percentage
    let throttle_pct = match operation {
        "insert" => permissions.throttle_inserts_pct,
        "search" => permissions.throttle_searches_pct,
        _ => 0,
    };

    // Check concurrent limits
    let within_limit = match operation {
        "search" => {
            permissions.max_concurrent_searches.map_or(true, |max| {
                shmem.get_concurrent_searches(collection_id) < max
            })
        }
        _ => true,
    };

    let reason = if !allowed {
        Some(format!(
            "Operation '{}' blocked: system in {} state",
            operation, state
        ))
    } else if !within_limit {
        Some(format!(
            "Operation '{}' blocked: concurrent limit exceeded",
            operation
        ))
    } else {
        None
    };

    GateResult {
        allowed: allowed && within_limit,
        throttle_pct,
        state,
        reason,
    }
}

/// Apply throttling (probabilistic rejection)
pub fn apply_throttle(throttle_pct: u8) -> bool {
    if throttle_pct == 0 {
        return true;  // Not throttled
    }
    if throttle_pct >= 100 {
        return false;  // Fully throttled
    }

    // Random rejection based on percentage
    let mut rng = rand::thread_rng();
    rng.gen_range(0..100) >= throttle_pct
}

#[derive(Debug, Clone)]
pub struct GateResult {
    pub allowed: bool,
    pub throttle_pct: u8,
    pub state: IntegrityState,
    pub reason: Option<String>,
}

/// SQL function for gate check
#[pg_extern]
pub fn ruvector_integrity_gate(
    collection_name: &str,
    operation: &str,
) -> pgrx::JsonB {
    let collection_id = match get_collection_id(collection_name) {
        Some(id) => id,
        None => {
            return pgrx::JsonB(serde_json::json!({
                "error": format!("Collection not found: {}", collection_name),
                "allowed": false,
            }));
        }
    };

    let result = check_integrity_gate(collection_id, operation);

    pgrx::JsonB(serde_json::json!({
        "allowed": result.allowed,
        "throttle_pct": result.throttle_pct,
        "state": result.state.to_string(),
        "reason": result.reason,
    }))
}
```

### 6. Cryptographic Signing

```rust
// src/integrity/signing.rs

use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use rand::rngs::OsRng;

/// Load or generate signing key
pub fn load_or_generate_signing_key() -> SigningKey {
    // Try to load from secure storage
    if let Ok(key) = load_signing_key_from_storage() {
        return key;
    }

    // Generate new key
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);

    // Store for future use
    if let Err(e) = store_signing_key(&signing_key) {
        pgrx::warning!("Failed to persist signing key: {}", e);
    }

    // Register public key in database
    register_public_key(&signing_key.verifying_key());

    signing_key
}

fn load_signing_key_from_storage() -> Result<SigningKey, Error> {
    // Load from secure file or PostgreSQL config
    let path = std::env::var("RUVECTOR_SIGNING_KEY_PATH")
        .unwrap_or_else(|_| "/var/lib/postgresql/ruvector_signing_key".to_string());

    let key_bytes = std::fs::read(&path)?;
    if key_bytes.len() != 32 {
        return Err(Error::InvalidKeyLength);
    }

    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&key_bytes);

    Ok(SigningKey::from_bytes(&bytes))
}

fn store_signing_key(key: &SigningKey) -> Result<(), Error> {
    let path = std::env::var("RUVECTOR_SIGNING_KEY_PATH")
        .unwrap_or_else(|_| "/var/lib/postgresql/ruvector_signing_key".to_string());

    std::fs::write(&path, key.to_bytes())?;

    // Set restrictive permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

fn register_public_key(verifying_key: &VerifyingKey) {
    let public_bytes = verifying_key.to_bytes();

    Spi::run(|client| {
        client.update(
            "INSERT INTO ruvector.signing_keys (id, public_key, description)
             VALUES ('integrity-worker', $1, 'Auto-generated integrity worker key')
             ON CONFLICT (id) DO UPDATE SET
                 public_key = EXCLUDED.public_key,
                 created_at = NOW()",
            None,
            &[public_bytes.as_slice().into()],
        )
    }).ok();
}

/// Sign an integrity event
impl SignedIntegrityEvent {
    pub fn sign(
        event: IntegrityEventContent,
        signing_key: &SigningKey,
        signer_id: &str,
    ) -> Self {
        // Serialize event for signing
        let message = serde_json::to_vec(&event).unwrap();

        // Sign
        let signature = signing_key.sign(&message);

        Self {
            event,
            signature: signature.to_bytes(),
            signer_id: signer_id.to_string(),
            signed_at: chrono::Utc::now(),
        }
    }

    pub fn verify(&self, verifying_key: &VerifyingKey) -> bool {
        let message = serde_json::to_vec(&self.event).unwrap();
        let signature = Signature::from_bytes(&self.signature);

        verifying_key.verify_strict(&message, &signature).is_ok()
    }
}

/// SQL function to verify event signature
#[pg_extern]
pub fn ruvector_verify_event(event_id: i64) -> Option<bool> {
    Spi::connect(|client| {
        // Get event
        let event = client.select(
            "SELECT signature, signer_id,
                    collection_id, event_type, previous_state, new_state,
                    lambda_cut, witness_edges, metadata, created_at
             FROM ruvector.integrity_events
             WHERE id = $1",
            None,
            &[event_id.into()],
        )?.first();

        let event = match event {
            Some(e) => e,
            None => return Ok(None),
        };

        let signature_bytes: Option<Vec<u8>> = event.get(1)?;
        let signer_id: Option<String> = event.get(2)?;

        let (signature_bytes, signer_id) = match (signature_bytes, signer_id) {
            (Some(s), Some(id)) => (s, id),
            _ => return Ok(Some(false)),  // Unsigned event
        };

        // Get public key
        let key_row = client.select(
            "SELECT public_key FROM ruvector.signing_keys
             WHERE id = $1 AND revoked_at IS NULL",
            None,
            &[signer_id.into()],
        )?.first();

        let public_key_bytes: Vec<u8> = match key_row {
            Some(r) => r.get(1)?,
            None => return Ok(Some(false)),
        };

        // Verify
        let verifying_key = match VerifyingKey::from_bytes(
            &public_key_bytes.try_into().map_err(|_| "Invalid key length")?
        ) {
            Ok(k) => k,
            Err(_) => return Ok(Some(false)),
        };

        // Reconstruct event content
        let content = IntegrityEventContent {
            collection_id: event.get(3)?,
            event_type: event.get(4)?,
            previous_state: event.get(5)?,
            new_state: event.get(6)?,
            lambda_cut: event.get(7)?,
            witness_edges: event.get::<Option<pgrx::JsonB>>(8)?
                .map(|j| serde_json::from_value(j.0).unwrap()),
            metadata: event.get::<pgrx::JsonB>(9)?.0,
            created_at: event.get(10)?,
        };

        let signed = SignedIntegrityEvent {
            event: content,
            signature: signature_bytes.try_into().map_err(|_| "Invalid signature length")?,
            signer_id,
            signed_at: chrono::Utc::now(),
        };

        Ok(Some(signed.verify(&verifying_key)))
    }).unwrap_or(None)
}
```

### 7. SQL Functions

```sql
-- Sample integrity state manually
CREATE FUNCTION ruvector_integrity_sample(p_collection_name TEXT)
RETURNS JSONB AS 'MODULE_PATHNAME', 'ruvector_integrity_sample' LANGUAGE C;

-- Get current integrity status
CREATE FUNCTION ruvector_integrity_status(p_collection_name TEXT)
RETURNS JSONB AS $$
DECLARE
    v_result JSONB;
BEGIN
    SELECT jsonb_build_object(
        'collection', p_collection_name,
        'state', s.state,
        'lambda_cut', s.lambda_cut,
        'last_sample', s.last_sample,
        'sample_count', s.sample_count,
        'witness_edges', s.witness_edges,
        'policy', jsonb_build_object(
            'name', p.name,
            'threshold_high', p.threshold_high,
            'threshold_low', p.threshold_low
        )
    ) INTO v_result
    FROM ruvector.collections c
    JOIN ruvector.integrity_state s ON c.id = s.collection_id
    LEFT JOIN ruvector.integrity_policies p ON c.id = p.collection_id AND p.enabled
    WHERE c.name = p_collection_name
    ORDER BY p.priority DESC NULLS LAST
    LIMIT 1;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- View contracted graph
CREATE FUNCTION ruvector_contracted_graph(p_collection_name TEXT)
RETURNS JSONB AS $$
DECLARE
    v_collection_id INTEGER;
    v_result JSONB;
BEGIN
    SELECT id INTO v_collection_id FROM ruvector.collections WHERE name = p_collection_name;

    SELECT jsonb_build_object(
        'nodes', (
            SELECT jsonb_agg(jsonb_build_object(
                'type', node_type,
                'id', node_id,
                'name', node_name,
                'health', health_score
            ))
            FROM ruvector.contracted_graph
            WHERE collection_id = v_collection_id
        ),
        'edges', (
            SELECT jsonb_agg(jsonb_build_object(
                'source', source_type || ':' || source_id,
                'target', target_type || ':' || target_id,
                'type', edge_type,
                'capacity', capacity,
                'error_rate', error_rate
            ))
            FROM ruvector.contracted_edges
            WHERE collection_id = v_collection_id
        ),
        'node_count', (SELECT COUNT(*) FROM ruvector.contracted_graph WHERE collection_id = v_collection_id),
        'edge_count', (SELECT COUNT(*) FROM ruvector.contracted_edges WHERE collection_id = v_collection_id)
    ) INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Rebuild contracted graph
CREATE FUNCTION ruvector_rebuild_contracted_graph(p_collection_name TEXT)
RETURNS JSONB AS 'MODULE_PATHNAME', 'ruvector_rebuild_contracted_graph' LANGUAGE C;

-- Verify event signature
CREATE FUNCTION ruvector_verify_event_signature(p_event_id BIGINT)
RETURNS BOOLEAN AS 'MODULE_PATHNAME', 'ruvector_verify_event' LANGUAGE C;

-- Get integrity history
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
    is_signed BOOLEAN,
    is_verified BOOLEAN,
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
        jsonb_array_length(COALESCE(e.witness_edges, '[]'::jsonb))::integer,
        e.signature IS NOT NULL,
        CASE WHEN e.signature IS NOT NULL
             THEN ruvector_verify_event_signature(e.id)
             ELSE NULL
        END,
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

## Usage Examples

```sql
-- Check current integrity status
SELECT ruvector_integrity_status('embeddings');

-- Check if operation is allowed
SELECT ruvector_integrity_gate('embeddings', 'bulk_insert');

-- View contracted graph structure
SELECT ruvector_contracted_graph('embeddings');

-- View recent integrity events
SELECT * FROM ruvector_integrity_history('embeddings', 'state_change');

-- Verify event signatures
SELECT
    id,
    event_type,
    new_state,
    ruvector_verify_event_signature(id) AS signature_valid
FROM ruvector.integrity_events
WHERE collection_id = 1
  AND signature IS NOT NULL
ORDER BY created_at DESC
LIMIT 10;

-- Set custom policy
SELECT ruvector_integrity_policy_set('embeddings', 'custom', '{
    "threshold_high": 0.7,
    "threshold_low": 0.2,
    "stress_actions": {
        "allow_bulk_insert": false,
        "throttle_inserts_pct": 75,
        "pause_gnn_training": true
    }
}'::jsonb);
```

---

## Testing Requirements

### Unit Tests
- Lambda cut computation accuracy
- Gate check logic
- Signature generation/verification
- Policy application

### Integration Tests
- Full integrity cycle
- State transitions
- Event persistence
- Shared memory updates

### Chaos Tests
- Node failures
- Network partitions
- Rapid state oscillation

---

## Timeline

| Week | Deliverable |
|------|-------------|
| 13 | Contracted graph schema and builder |
| 14 | Lambda cut computation |
| 15 | Integrity worker and gate |
| 16 | Signing, policies, testing |
