# RuVector Postgres v2 - Background Workers Specification

## Overview

RuVector v2 uses PostgreSQL background workers to maintain long-lived engine instances, perform maintenance operations, and run continuous learning pipelines. This document specifies the architecture, responsibilities, and communication patterns for each worker type.

---

## Worker Architecture

### Worker Registry

```
+------------------------------------------------------------------+
|                     PostgreSQL Server                             |
+------------------------------------------------------------------+
|                                                                    |
|  +------------------------+  +------------------------+            |
|  |   Engine Worker (1)    |  |  Maintenance Worker    |            |
|  |  - Per database        |  |  - Per server          |            |
|  |  - Long-lived          |  |  - Periodic            |            |
|  +------------------------+  +------------------------+            |
|                                                                    |
|  +------------------------+  +------------------------+            |
|  |   GNN Training Worker  |  |  Integrity Worker      |            |
|  |  - On-demand           |  |  - Per database        |            |
|  |  - Resource-intensive  |  |  - Continuous          |            |
|  +------------------------+  +------------------------+            |
|                                                                    |
+------------------------------------------------------------------+
|                     Shared Memory Region                          |
|  +------------------+  +------------------+  +------------------+  |
|  | Work Queues      |  | Index State      |  | Integrity State  |  |
|  +------------------+  +------------------+  +------------------+  |
+------------------------------------------------------------------+
```

---

## 1. Engine Worker

### Purpose
The Engine Worker is the core RuVector instance that handles all vector operations. It maintains in-memory indexes and processes queries/mutations submitted via shared memory.

### Configuration

```rust
/// Engine worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineWorkerConfig {
    /// Maximum memory for indexes (bytes)
    pub max_index_memory: usize,

    /// Maximum concurrent search operations
    pub max_concurrent_searches: usize,

    /// Work queue depth
    pub work_queue_size: usize,

    /// Shutdown timeout (seconds)
    pub shutdown_timeout_secs: u64,

    /// Enable SIMD optimizations
    pub enable_simd: bool,

    /// Prefetch distance for search
    pub prefetch_distance: usize,
}

impl Default for EngineWorkerConfig {
    fn default() -> Self {
        Self {
            max_index_memory: 4 * 1024 * 1024 * 1024,  // 4GB
            max_concurrent_searches: 64,
            work_queue_size: 1024,
            shutdown_timeout_secs: 30,
            enable_simd: true,
            prefetch_distance: 4,
        }
    }
}
```

### Lifecycle

```rust
/// Engine worker main loop
#[pg_guard]
pub extern "C" fn ruvector_engine_worker_main(_arg: pg_sys::Datum) {
    pgrx::log!("RuVector engine worker starting");

    // Initialize shared memory
    let shmem = SharedMemory::attach().expect("Failed to attach shared memory");

    // Initialize engine
    let mut engine = RuVectorEngine::new(EngineWorkerConfig::default());

    // Load persisted indexes
    if let Err(e) = engine.load_from_storage() {
        pgrx::warning!("Failed to load indexes: {}", e);
    }

    // Main loop
    loop {
        // Check for shutdown
        if unsafe { pg_sys::ShutdownRequestPending } {
            break;
        }

        // Process work queue
        while let Some(work_item) = shmem.work_queue.try_pop() {
            let result = match work_item.operation {
                Operation::Search(req) => engine.search(&req),
                Operation::Insert(req) => engine.insert(&req),
                Operation::Delete(req) => engine.delete(&req),
                Operation::BuildIndex(req) => engine.build_index(&req),
                Operation::UpdateIndex(req) => engine.update_index(&req),
            };

            // Post result
            shmem.result_queue.push(WorkResult {
                request_id: work_item.request_id,
                result,
            });

            // Signal waiter
            shmem.signal_completion(work_item.request_id);
        }

        // Yield to avoid spinning
        unsafe {
            pg_sys::WaitLatch(
                pg_sys::MyLatch,
                pg_sys::WL_LATCH_SET as i32 | pg_sys::WL_TIMEOUT as i32,
                1, // 1ms timeout
                pg_sys::PG_WAIT_EXTENSION as u32,
            );
            pg_sys::ResetLatch(pg_sys::MyLatch);
        }
    }

    // Graceful shutdown
    engine.persist_to_storage();
    pgrx::log!("RuVector engine worker stopped");
}
```

### Work Item Protocol

```rust
/// Work item submitted to engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    /// Unique request ID
    pub request_id: u64,

    /// Operation to perform
    pub operation: Operation,

    /// Priority (higher = more urgent)
    pub priority: u8,

    /// Deadline (epoch ms, 0 = no deadline)
    pub deadline_ms: u64,

    /// Submitting backend PID
    pub backend_pid: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    Search(SearchRequest),
    Insert(InsertRequest),
    Delete(DeleteRequest),
    BuildIndex(BuildIndexRequest),
    UpdateIndex(UpdateIndexRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub collection_id: i32,
    pub query: Vec<f32>,
    pub k: usize,
    pub ef_search: Option<usize>,
    pub filter: Option<FilterExpr>,
    pub use_gnn: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub ids: Vec<TupleId>,
    pub distances: Vec<f32>,
    pub search_time_us: u64,
}
```

### Registration

```rust
/// Register engine worker with PostgreSQL
pub fn register_engine_worker(database: &str) {
    let mut worker = pg_sys::BackgroundWorker::default();

    // Set name
    let name = format!("ruvector engine [{}]", database);
    worker.bgw_name = name.as_ptr() as *const i8;

    // Configuration
    worker.bgw_flags = pg_sys::BGWORKER_SHMEM_ACCESS
                     | pg_sys::BGWORKER_BACKEND_DATABASE_CONNECTION;
    worker.bgw_start_time = pg_sys::BgWorkerStart_RecoveryFinished;
    worker.bgw_restart_time = 10;  // Restart after 10s on crash
    worker.bgw_main = Some(ruvector_engine_worker_main);

    // Register
    unsafe {
        pg_sys::RegisterBackgroundWorker(&mut worker);
    }
}
```

---

## 2. Maintenance Worker

### Purpose
The Maintenance Worker performs periodic operations including:
- Index optimization and compaction
- Tier management (promote/demote vectors)
- Statistics collection
- Dead tuple cleanup

### Configuration

```rust
/// Maintenance worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceConfig {
    /// Interval between maintenance cycles (seconds)
    pub interval_secs: u64,

    /// Maximum indexes to process per cycle
    pub max_indexes_per_cycle: usize,

    /// Enable automatic tier management
    pub enable_tiering: bool,

    /// Enable automatic compaction
    pub enable_compaction: bool,

    /// Enable statistics collection
    pub enable_stats: bool,

    /// Compaction threshold (fragmentation ratio)
    pub compaction_threshold: f32,

    /// Tier check interval (separate from main interval)
    pub tier_check_interval_secs: u64,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self {
            interval_secs: 300,  // 5 minutes
            max_indexes_per_cycle: 10,
            enable_tiering: true,
            enable_compaction: true,
            enable_stats: true,
            compaction_threshold: 0.15,  // 15% fragmentation
            tier_check_interval_secs: 3600,  // 1 hour
        }
    }
}
```

### Main Loop

```rust
#[pg_guard]
pub extern "C" fn ruvector_maintenance_worker_main(_arg: pg_sys::Datum) {
    pgrx::log!("RuVector maintenance worker starting");

    let config = MaintenanceConfig::default();
    let mut last_tier_check = Instant::now();

    loop {
        if unsafe { pg_sys::ShutdownRequestPending } {
            break;
        }

        // Collect indexes needing maintenance
        let indexes = match find_ruvector_indexes(config.max_indexes_per_cycle) {
            Ok(idx) => idx,
            Err(e) => {
                pgrx::warning!("Failed to find indexes: {}", e);
                Vec::new()
            }
        };

        for index in indexes {
            // Statistics collection
            if config.enable_stats {
                if let Err(e) = collect_index_stats(&index) {
                    pgrx::warning!("Stats collection failed for {}: {}", index.name, e);
                }
            }

            // Compaction check
            if config.enable_compaction {
                let fragmentation = calculate_fragmentation(&index);
                if fragmentation > config.compaction_threshold {
                    pgrx::log!("Compacting index {} (frag: {:.1}%)",
                        index.name, fragmentation * 100.0);
                    if let Err(e) = compact_index(&index) {
                        pgrx::warning!("Compaction failed for {}: {}", index.name, e);
                    }
                }
            }
        }

        // Tier management (less frequent)
        if config.enable_tiering &&
           last_tier_check.elapsed().as_secs() > config.tier_check_interval_secs {
            if let Err(e) = perform_tier_management() {
                pgrx::warning!("Tier management failed: {}", e);
            }
            last_tier_check = Instant::now();
        }

        // Sleep
        sleep_interruptible(config.interval_secs);
    }

    pgrx::log!("RuVector maintenance worker stopped");
}
```

### Tier Management

```rust
/// Perform tier management for all collections
fn perform_tier_management() -> Result<(), String> {
    // Query collections with tiering enabled
    let collections = Spi::connect(|client| {
        let query = r#"
            SELECT c.id, c.name, c.table_schema, c.table_name, c.column_name
            FROM ruvector.collections c
            JOIN ruvector.tier_policies tp ON c.id = tp.collection_id
            WHERE tp.enabled = true
            GROUP BY c.id
        "#;

        client.select(query, None, &[])
            .map(|row| {
                CollectionInfo {
                    id: row.get::<i32>(1).unwrap(),
                    name: row.get::<String>(2).unwrap(),
                    table_ref: format!("{}.{}",
                        row.get::<String>(3).unwrap(),
                        row.get::<String>(4).unwrap()),
                    column: row.get::<String>(5).unwrap(),
                }
            })
            .collect::<Vec<_>>()
    })?;

    for collection in collections {
        // Get tier policies
        let policies = get_tier_policies(collection.id)?;

        // Get access counters that need promotion/demotion
        let candidates = get_tier_candidates(collection.id, &policies)?;

        for candidate in candidates {
            if candidate.needs_promotion {
                promote_vector(collection.id, candidate.vector_tid, candidate.target_tier)?;
            } else if candidate.needs_demotion {
                demote_vector(collection.id, candidate.vector_tid, candidate.target_tier)?;
            }
        }

        // Update tier statistics
        update_tier_stats(collection.id)?;
    }

    Ok(())
}

/// Promote vector to hotter tier
fn promote_vector(collection_id: i32, vector_tid: TupleId, target_tier: &str) -> Result<(), String> {
    // 1. Decompress if needed
    // 2. Move to hot storage
    // 3. Update access counter tier
    // 4. Log promotion event
    Ok(())
}

/// Demote vector to colder tier
fn demote_vector(collection_id: i32, vector_tid: TupleId, target_tier: &str) -> Result<(), String> {
    // 1. Apply compression (SQ8, PQ, etc.)
    // 2. Move to cold storage
    // 3. Update access counter tier
    // 4. Log demotion event
    Ok(())
}
```

---

## 3. GNN Training Worker

### Purpose
The GNN Training Worker performs model training on captured interaction data. It runs on-demand when triggered via `ruvector_gnn_train()` or when sufficient new training data is available.

### Configuration

```rust
/// GNN training configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnTrainingConfig {
    /// Minimum samples before auto-training
    pub min_samples_for_training: usize,

    /// Maximum training time (seconds)
    pub max_training_time_secs: u64,

    /// Epochs per training run
    pub default_epochs: usize,

    /// Batch size
    pub batch_size: usize,

    /// Learning rate
    pub learning_rate: f32,

    /// Validation split ratio
    pub validation_split: f32,

    /// Early stopping patience
    pub early_stopping_patience: usize,
}

impl Default for GnnTrainingConfig {
    fn default() -> Self {
        Self {
            min_samples_for_training: 1000,
            max_training_time_secs: 3600,  // 1 hour
            default_epochs: 100,
            batch_size: 64,
            learning_rate: 0.001,
            validation_split: 0.2,
            early_stopping_patience: 10,
        }
    }
}
```

### Training Pipeline

```rust
#[pg_guard]
pub extern "C" fn ruvector_gnn_training_worker_main(arg: pg_sys::Datum) {
    // Decode training request from arg
    let request: GnnTrainingRequest = unsafe {
        decode_training_request(arg)
    };

    pgrx::log!("Starting GNN training for collection: {}", request.collection_name);

    // Load training data
    let training_data = match load_training_data(request.collection_id) {
        Ok(data) => data,
        Err(e) => {
            record_training_failure(request.collection_id, &e);
            return;
        }
    };

    pgrx::log!("Loaded {} training samples", training_data.len());

    // Build training graph
    let graph = match build_training_graph(&training_data) {
        Ok(g) => g,
        Err(e) => {
            record_training_failure(request.collection_id, &e);
            return;
        }
    };

    // Initialize model
    let mut model = match request.model_type.as_str() {
        "gcn" => Box::new(GCNModel::new(request.config.clone())) as Box<dyn GnnModel>,
        "graphsage" => Box::new(GraphSAGEModel::new(request.config.clone())),
        "gat" => Box::new(GATModel::new(request.config.clone())),
        _ => {
            record_training_failure(request.collection_id, "Unknown model type");
            return;
        }
    };

    // Training loop
    let config = GnnTrainingConfig::default();
    let start_time = Instant::now();
    let mut best_loss = f32::MAX;
    let mut patience_counter = 0;

    for epoch in 0..request.epochs.unwrap_or(config.default_epochs) {
        // Check timeout
        if start_time.elapsed().as_secs() > config.max_training_time_secs {
            pgrx::warning!("Training timeout reached");
            break;
        }

        // Check shutdown
        if unsafe { pg_sys::ShutdownRequestPending } {
            break;
        }

        // Training step
        let train_loss = model.train_epoch(&graph, config.batch_size);

        // Validation
        let val_loss = model.validate(&graph);

        // Early stopping
        if val_loss < best_loss {
            best_loss = val_loss;
            patience_counter = 0;
        } else {
            patience_counter += 1;
            if patience_counter >= config.early_stopping_patience {
                pgrx::log!("Early stopping at epoch {}", epoch);
                break;
            }
        }

        if epoch % 10 == 0 {
            pgrx::log!("Epoch {}: train_loss={:.4}, val_loss={:.4}",
                epoch, train_loss, val_loss);
        }
    }

    // Save model
    let model_data = model.serialize();
    let training_time = start_time.elapsed();

    if let Err(e) = save_trained_model(
        request.collection_id,
        &request.model_name,
        &request.model_type,
        &model_data,
        training_data.len(),
        request.epochs.unwrap_or(config.default_epochs),
        best_loss,
        model.evaluate_recall(&graph),
    ) {
        pgrx::warning!("Failed to save model: {}", e);
    } else {
        pgrx::log!("Model saved successfully (training time: {:?})", training_time);
    }
}
```

### Dynamic Worker Spawning

```rust
/// Spawn GNN training worker
pub fn spawn_gnn_training_worker(request: &GnnTrainingRequest) -> Result<bool, String> {
    let mut worker = pg_sys::BackgroundWorker::default();

    let name = format!("ruvector gnn trainer [{}]", request.collection_name);
    worker.bgw_name = name.as_ptr() as *const i8;

    worker.bgw_flags = pg_sys::BGWORKER_SHMEM_ACCESS
                     | pg_sys::BGWORKER_BACKEND_DATABASE_CONNECTION;
    worker.bgw_start_time = pg_sys::BgWorkerStart_RecoveryFinished;
    worker.bgw_restart_time = pg_sys::BGW_NEVER_RESTART as i32;
    worker.bgw_main = Some(ruvector_gnn_training_worker_main);

    // Encode request as datum
    worker.bgw_main_arg = encode_training_request(request);

    // Register dynamically
    let mut handle: pg_sys::BackgroundWorkerHandle = std::ptr::null_mut();
    let registered = unsafe {
        pg_sys::RegisterDynamicBackgroundWorker(&mut worker, &mut handle)
    };

    if registered {
        // Wait for worker to start
        unsafe {
            pg_sys::WaitForBackgroundWorkerStartup(handle, std::ptr::null_mut())
        };
        Ok(true)
    } else {
        Err("Failed to register background worker".to_string())
    }
}
```

---

## 4. Integrity Worker (Mincut Integration)

### Purpose
The Integrity Worker continuously monitors the contracted operational graph and computes mincut-based integrity metrics. It updates the integrity state and triggers policy actions.

**IMPORTANT TERMINOLOGY**:
- **λ_cut (lambda_cut)**: Minimum cut value - computed via max-flow algorithms (PRIMARY metric)
- **λ₂ (lambda2)**: Algebraic connectivity / spectral stress - eigenvalue metric (OPTIONAL)

### Mincut Worker Architecture

```
+------------------------------------------------------------------+
|              MINCUT WORKER INTEGRATION WITH POSTGRES              |
+------------------------------------------------------------------+
|                                                                   |
|  Worker Types:                                                    |
|                                                                   |
|  1. MINCUT SAMPLER (lightweight, frequent)                        |
|     - Runs every 10-60 seconds                                    |
|     - Samples contracted graph edges                              |
|     - Updates edge capacities based on recent metrics             |
|     - Low CPU: O(|E|) where E is contracted edges (~1000)         |
|                                                                   |
|  2. MINCUT COMPUTER (heavier, less frequent)                      |
|     - Runs every 1-5 minutes OR on-demand                         |
|     - Computes actual mincut via Push-Relabel algorithm           |
|     - Optionally computes λ₂ for spectral stress                  |
|     - Moderate CPU: O(V²E) but V,E are small (~1000)              |
|                                                                   |
|  3. INTEGRITY CONTROLLER (always running)                         |
|     - Monitors mincut values with hysteresis                      |
|     - Updates shared memory permissions                           |
|     - Logs signed events on state changes                         |
|     - Minimal CPU: event-driven                                   |
|                                                                   |
+------------------------------------------------------------------+
```

### Worker Separation Strategy

```rust
/// Why separate workers instead of one monolithic integrity worker?
///
/// 1. ISOLATION: Sampling failures don't block control decisions
/// 2. SCHEDULING: Different frequencies for different work
/// 3. RESOURCE CONTROL: Heavy computation in dedicated worker
/// 4. TESTABILITY: Each component testable in isolation
```

### Configuration

```rust
/// Integrity worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityConfig {
    /// Sampling interval (seconds) - SAMPLER worker
    pub sample_interval_secs: u64,

    /// Mincut computation interval (seconds) - COMPUTER worker
    pub compute_interval_secs: u64,

    /// Number of edges to sample per check
    pub sample_size: usize,

    /// Threshold for normal state (λ_cut value)
    pub threshold_high: f64,

    /// Threshold for critical state (λ_cut value)
    pub threshold_low: f64,

    /// Also compute λ₂ (spectral stress) - more expensive
    pub compute_lambda2: bool,

    /// Enable audit logging
    pub enable_audit: bool,

    /// Ed25519 signing key path (for audit signatures)
    pub signing_key_path: Option<String>,

    /// Hysteresis configuration
    pub hysteresis: HysteresisConfig,
}

impl Default for IntegrityConfig {
    fn default() -> Self {
        Self {
            sample_interval_secs: 60,
            sample_size: 1000,
            threshold_high: 0.8,
            threshold_low: 0.3,
            enable_audit: true,
            signing_key_path: None,
        }
    }
}
```

### Main Loop

```rust
#[pg_guard]
pub extern "C" fn ruvector_integrity_worker_main(_arg: pg_sys::Datum) {
    pgrx::log!("RuVector integrity worker starting");

    let config = IntegrityConfig::default();

    // Load signing key if configured
    let signing_key = config.signing_key_path
        .as_ref()
        .and_then(|path| load_signing_key(path).ok());

    loop {
        if unsafe { pg_sys::ShutdownRequestPending } {
            break;
        }

        // Get collections with integrity policies
        let collections = match get_collections_with_integrity() {
            Ok(c) => c,
            Err(e) => {
                pgrx::warning!("Failed to get collections: {}", e);
                sleep_interruptible(config.sample_interval_secs);
                continue;
            }
        };

        for collection in collections {
            // Sample contracted graph
            let sample_result = sample_contracted_graph(
                collection.id,
                config.sample_size
            );

            match sample_result {
                Ok(sample) => {
                    // Compute lambda_cut (minimum cut value) on contracted graph
                    let mincut_result = compute_mincut(&sample);
                    let lambda_cut = mincut_result.lambda_cut;

                    // Optionally compute lambda2 (spectral stress) as drift signal
                    let lambda2 = if config.compute_lambda2 {
                        compute_lambda2(&sample)
                    } else {
                        None
                    };

                    // Determine new state
                    let new_state = determine_state(
                        lambda_cut,
                        config.threshold_high,
                        config.threshold_low
                    );

                    // Get current state
                    let current_state = get_current_state(collection.id);

                    // State transition
                    if new_state != current_state.state {
                        handle_state_transition(
                            collection.id,
                            &current_state,
                            new_state,
                            lambda_cut,
                            &sample.witness_edges,
                            &signing_key,
                        );
                    }

                    // Update state
                    update_integrity_state(collection.id, new_state, lambda_cut, &sample);
                }
                Err(e) => {
                    pgrx::warning!(
                        "Integrity sample failed for collection {}: {}",
                        collection.id, e
                    );
                }
            }
        }

        sleep_interruptible(config.sample_interval_secs);
    }

    pgrx::log!("RuVector integrity worker stopped");
}
```

### Mincut and Spectral Computation

**CRITICAL DISTINCTION**:
- **λ_cut (mincut)**: Minimum cut VALUE - computed via max-flow algorithms. PRIMARY metric.
- **λ₂ (lambda2)**: Algebraic connectivity - second smallest eigenvalue of Laplacian. OPTIONAL spectral metric.

```rust
/// Compute minimum cut value using Push-Relabel algorithm
/// This is the PRIMARY integrity metric
fn compute_mincut(graph: &ContractedGraph) -> MincutResult {
    let n = graph.nodes.len();
    if n < 2 {
        return MincutResult { lambda_cut: 0.0, witness_edges: vec![] };
    }

    // Build capacity matrix from edges
    let mut capacity = vec![vec![0.0f64; n]; n];
    let node_index: HashMap<_, _> = graph.nodes.iter()
        .enumerate()
        .map(|(i, node)| ((node.node_type, node.node_id), i))
        .collect();

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

    // Find global mincut using Stoer-Wagner or iterated max-flow
    // Stoer-Wagner is O(VE + V² log V) - efficient for small graphs
    let (min_cut_value, cut_edges) = stoer_wagner_mincut(&capacity);

    // Map cut edges back to witness edges
    let witness_edges = cut_edges.iter()
        .map(|&(i, j)| {
            let src = &graph.nodes[i];
            let tgt = &graph.nodes[j];
            WitnessEdge {
                source_type: src.node_type.to_string(),
                source_id: src.node_id,
                target_type: tgt.node_type.to_string(),
                target_id: tgt.node_id,
                capacity: capacity[i][j] as f32,
            }
        })
        .collect();

    MincutResult {
        lambda_cut: min_cut_value,
        witness_edges,
    }
}

/// Stoer-Wagner algorithm for global minimum cut
/// O(VE + V² log V) - efficient for contracted graphs with ~1000 nodes
fn stoer_wagner_mincut(capacity: &[Vec<f64>]) -> (f64, Vec<(usize, usize)>) {
    let n = capacity.len();
    let mut best_cut = f64::MAX;
    let mut best_partition = vec![];

    // Working copy of vertices (for contraction)
    let mut vertices: Vec<usize> = (0..n).collect();
    let mut merged: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut cap = capacity.to_vec();

    while vertices.len() > 1 {
        // Find minimum s-t cut using maximum adjacency ordering
        let (s, t, cut_of_phase) = minimum_cut_phase(&vertices, &cap);

        if cut_of_phase < best_cut {
            best_cut = cut_of_phase;
            best_partition = merged[t].clone();
        }

        // Merge s and t
        merge_vertices(&mut vertices, &mut merged, &mut cap, s, t);
    }

    // Reconstruct cut edges from partition
    let partition_set: std::collections::HashSet<_> = best_partition.iter().collect();
    let mut cut_edges = vec![];
    for i in 0..n {
        for j in (i+1)..n {
            if capacity[i][j] > 0.0 {
                let i_in = partition_set.contains(&i);
                let j_in = partition_set.contains(&j);
                if i_in != j_in {
                    cut_edges.push((i, j));
                }
            }
        }
    }

    (best_cut, cut_edges)
}

/// OPTIONAL: Compute algebraic connectivity (λ₂) for spectral stress insight
/// This is a DIFFERENT metric from mincut - provides complementary information
fn compute_lambda2(graph: &ContractedGraph) -> Option<f64> {
    let n = graph.nodes.len();
    if n < 2 {
        return Some(0.0);
    }

    // Build Laplacian matrix L = D - A
    let mut laplacian = vec![vec![0.0f64; n]; n];
    let node_index: HashMap<_, _> = graph.nodes.iter()
        .enumerate()
        .map(|(i, node)| ((node.node_type, node.node_id), i))
        .collect();

    // Build adjacency and degree
    for edge in &graph.edges {
        if let (Some(&i), Some(&j)) = (
            node_index.get(&(edge.source_type, edge.source_id)),
            node_index.get(&(edge.target_type, edge.target_id)),
        ) {
            let weight = edge.capacity as f64;
            laplacian[i][i] += weight;
            laplacian[j][j] += weight;
            laplacian[i][j] -= weight;
            laplacian[j][i] -= weight;
        }
    }

    // Use ARPACK-style Lanczos iteration for λ₂
    // Or simple power iteration on shifted inverse
    compute_fiedler_value(&laplacian)
}

/// Compute Fiedler value (second smallest eigenvalue of Laplacian)
/// Used only for λ₂ spectral stress metric
fn compute_fiedler_value(laplacian: &[Vec<f64>]) -> Option<f64> {
    let n = laplacian.len();
    if n < 2 {
        return Some(0.0);
    }

    // Inverse power iteration to find second smallest eigenvalue
    // (First eigenvalue is 0 with eigenvector all-ones)

    // Initialize random vector orthogonal to all-ones
    let mut v: Vec<f64> = (0..n).map(|i| (i as f64) - (n as f64 / 2.0)).collect();
    let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    v.iter_mut().for_each(|x| *x /= norm);

    // Shift to make positive definite
    let shift = 0.001;
    let mut shifted = laplacian.to_vec();
    for i in 0..n {
        shifted[i][i] += shift;
    }

    // Power iteration
    for _ in 0..100 {
        // Solve shifted system (in production, use LU decomposition)
        let mut new_v = solve_linear_system(&shifted, &v)?;

        // Orthogonalize against constant vector
        let mean: f64 = new_v.iter().sum::<f64>() / n as f64;
        new_v.iter_mut().for_each(|x| *x -= mean);

        // Normalize
        let norm: f64 = new_v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm < 1e-10 {
            return Some(0.0);
        }
        new_v.iter_mut().for_each(|x| *x /= norm);
        v = new_v;
    }

    // Rayleigh quotient gives λ₂
    let mut numerator = 0.0;
    for i in 0..n {
        for j in 0..n {
            numerator += v[i] * laplacian[i][j] * v[j];
        }
    }

    Some(numerator.max(0.0))
}

#[derive(Debug)]
struct MincutResult {
    lambda_cut: f64,
    witness_edges: Vec<WitnessEdge>,
}
```

### State Transitions

```rust
/// Handle integrity state transition
fn handle_state_transition(
    collection_id: i32,
    current: &IntegrityState,
    new_state: IntegrityStateType,
    lambda_cut: f64,
    witness_edges: &[EdgeId],
    signing_key: &Option<SigningKey>,
) {
    // Log state change
    pgrx::log!(
        "Integrity state change for collection {}: {} -> {} (lambda={:.4})",
        collection_id, current.state, new_state, lambda_cut
    );

    // Create event
    let event = IntegrityEvent {
        collection_id,
        event_type: "state_change".to_string(),
        previous_state: Some(current.state.to_string()),
        new_state: Some(new_state.to_string()),
        lambda_cut: Some(lambda_cut),
        witness_edges: Some(witness_edges.to_vec()),
        metadata: serde_json::json!({
            "transition_time": chrono::Utc::now().to_rfc3339(),
        }),
        signature: None,
    };

    // Sign if key available
    let signed_event = if let Some(key) = signing_key {
        let msg = serde_json::to_vec(&event).unwrap();
        let sig = key.sign(&msg);
        IntegrityEvent {
            signature: Some(sig.to_bytes().to_vec()),
            ..event
        }
    } else {
        event
    };

    // Persist event
    if let Err(e) = record_integrity_event(&signed_event) {
        pgrx::warning!("Failed to record integrity event: {}", e);
    }

    // Apply policy actions based on new state
    apply_policy_actions(collection_id, new_state);

    // Notify waiting operations
    notify_integrity_change(collection_id, new_state);
}

/// Apply policy actions for given state
fn apply_policy_actions(collection_id: i32, state: IntegrityStateType) {
    let policy = get_active_policy(collection_id);

    let actions = match state {
        IntegrityStateType::Normal => &policy.normal_actions,
        IntegrityStateType::Stress => &policy.stress_actions,
        IntegrityStateType::Critical => &policy.critical_actions,
    };

    // Update shared memory with current permissions
    let shmem = SharedMemory::get();
    shmem.update_integrity_permissions(collection_id, actions);
}
```

---

## 5. Shared Memory Communication

### IPC Contract (PRECISE SPECIFICATION)

**NOTE**: This replaces vague "zero-copy communication" with a bounded, implementable IPC surface.

```
+------------------------------------------------------------------+
|                   IPC CONTRACT SPECIFICATION                      |
+------------------------------------------------------------------+

ARCHITECTURE:
  Shared memory request queue with bounded payloads, plus optional
  shared segment for large vectors referenced by offset and length.

HARD CONSTRAINTS:
  +----------------------------------+----------------------------+
  | Parameter                        | Value                      |
  +----------------------------------+----------------------------+
  | Max request size (inline)        | 64 KB                      |
  | Max response size (inline)       | 64 KB                      |
  | Max vector payload (shared seg)  | 16 MB                      |
  | Request queue depth              | 1024 entries               |
  | Response queue depth             | 1024 entries               |
  | Request timeout                  | 30 seconds (configurable)  |
  | Cancellation supported           | Yes, via request_id        |
  +----------------------------------+----------------------------+

BACKPRESSURE BEHAVIOR:
  1. Queue full: Return EAGAIN, caller retries with exponential backoff
  2. Worker overloaded: Shed load by rejecting low-priority requests
  3. Memory pressure: Reject new requests, process existing queue

TIMEOUT AND CANCELLATION:
  1. Client sets deadline in request header
  2. Worker checks deadline before processing
  3. Expired requests: return ETIMEDOUT without processing
  4. Cancellation: client writes cancel flag, worker checks periodically

LARGE PAYLOAD HANDLING:
  For vectors > 64KB (e.g., batch insert of 1000+ vectors):
  1. Client allocates in shared segment
  2. Request contains (offset, length) reference
  3. Worker reads from shared segment
  4. Client frees after response received

+------------------------------------------------------------------+
```

### Shared Memory Layout

```rust
/// Shared memory region layout
#[repr(C)]
pub struct SharedMemoryLayout {
    /// Version for compatibility checking
    pub version: u32,

    /// Global lock for initialization
    pub init_lock: AtomicU32,

    /// Work queue for operations
    pub work_queue: WorkQueue,

    /// Result queue for responses
    pub result_queue: ResultQueue,

    /// Large payload shared segment
    pub large_payload_segment: LargePayloadSegment,

    /// Per-collection index state
    pub index_states: [IndexState; MAX_COLLECTIONS],

    /// Per-collection integrity state
    pub integrity_states: [IntegrityPermissions; MAX_COLLECTIONS],

    /// Statistics counters
    pub stats: GlobalStats,
}

/// Large payload segment for vectors > 64KB
#[repr(C)]
pub struct LargePayloadSegment {
    /// Segment size (default 16MB)
    pub size: usize,
    /// Allocation bitmap
    pub alloc_bitmap: [AtomicU64; 256],  // 16MB / 64KB = 256 slots
    /// Actual data
    pub data: [u8; 16 * 1024 * 1024],
}

impl LargePayloadSegment {
    /// Allocate a slot for large payload
    pub fn allocate(&self, size: usize) -> Option<PayloadRef> {
        let slots_needed = (size + 65535) / 65536;  // 64KB slots
        // Find contiguous free slots using CAS on bitmap
        // Returns PayloadRef { offset, length }
        todo!()
    }

    /// Free a previously allocated payload
    pub fn free(&self, payload_ref: &PayloadRef) {
        // Clear bits in allocation bitmap
        todo!()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PayloadRef {
    pub offset: u32,
    pub length: u32,
}

/// Work queue (lock-free MPSC)
#[repr(C)]
pub struct WorkQueue {
    pub head: AtomicU64,
    pub tail: AtomicU64,
    pub buffer: [WorkItem; QUEUE_SIZE],
}

impl WorkQueue {
    pub fn push(&self, item: WorkItem) -> Result<(), QueueFull> {
        // CAS-based insertion
        loop {
            let tail = self.tail.load(Ordering::Acquire);
            let head = self.head.load(Ordering::Acquire);

            if tail - head >= QUEUE_SIZE as u64 {
                return Err(QueueFull);
            }

            if self.tail.compare_exchange_weak(
                tail,
                tail + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ).is_ok() {
                let slot = (tail % QUEUE_SIZE as u64) as usize;
                self.buffer[slot] = item;
                return Ok(());
            }
        }
    }

    pub fn try_pop(&self) -> Option<WorkItem> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Acquire);

            if head >= tail {
                return None;
            }

            let slot = (head % QUEUE_SIZE as u64) as usize;
            let item = self.buffer[slot].clone();

            if self.head.compare_exchange_weak(
                head,
                head + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ).is_ok() {
                return Some(item);
            }
        }
    }
}
```

### Request/Response Pattern

```rust
/// Submit work to engine and wait for result
/// Implements timeout, cancellation, and backpressure handling per IPC contract
pub fn submit_and_wait(operation: Operation, timeout_ms: u64) -> Result<WorkResult, Error> {
    let shmem = SharedMemory::get();

    // Generate request ID
    let request_id = shmem.next_request_id();

    // Check payload size - use large segment if needed
    let payload_ref = if operation.serialized_size() > MAX_INLINE_SIZE {
        let size = operation.serialized_size();
        let payload_ref = shmem.large_payload_segment.allocate(size)
            .ok_or(Error::PayloadTooLarge)?;

        // Copy data to shared segment
        operation.serialize_to(&shmem.large_payload_segment.data[payload_ref.offset as usize..]);

        Some(payload_ref)
    } else {
        None
    };

    // Create work item with IPC contract fields
    let work_item = WorkItem {
        request_id,
        operation: if payload_ref.is_some() {
            Operation::LargePayloadRef(payload_ref.unwrap())
        } else {
            operation
        },
        priority: 0,
        deadline_ms: current_epoch_ms() + timeout_ms.min(MAX_REQUEST_TIMEOUT_MS),
        cancel_flag: AtomicBool::new(false),
        backend_pid: pg_sys::MyProcPid,
    };

    // Submit to work queue with backpressure handling
    let mut retry_count = 0;
    loop {
        match shmem.work_queue.push(work_item.clone()) {
            Ok(()) => break,
            Err(QueueFull) => {
                retry_count += 1;
                if retry_count > MAX_SUBMIT_RETRIES {
                    // Free large payload if allocated
                    if let Some(ref pr) = payload_ref {
                        shmem.large_payload_segment.free(pr);
                    }
                    return Err(Error::QueueFull);
                }
                // Exponential backoff: 1ms, 2ms, 4ms, 8ms...
                std::thread::sleep(Duration::from_millis(1 << retry_count.min(6)));
            }
        }
    }

    // Signal engine worker
    shmem.signal_engine();

    // Wait for result with proper timeout and cancellation handling
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        // Check result queue
        if let Some(result) = shmem.result_queue.try_get(request_id) {
            // Free large payload if allocated
            if let Some(ref pr) = payload_ref {
                shmem.large_payload_segment.free(pr);
            }
            return Ok(result);
        }

        // Check timeout
        if Instant::now() > deadline {
            // Mark request as cancelled so worker can skip if not started
            shmem.cancel_request(request_id);
            // Free large payload if allocated
            if let Some(ref pr) = payload_ref {
                shmem.large_payload_segment.free(pr);
            }
            return Err(Error::Timeout);
        }

        // Check for query cancellation
        if unsafe { pg_sys::QueryCancelPending } {
            shmem.cancel_request(request_id);
            if let Some(ref pr) = payload_ref {
                shmem.large_payload_segment.free(pr);
            }
            return Err(Error::Cancelled);
        }

        // Wait with latch
        unsafe {
            pg_sys::WaitLatch(
                pg_sys::MyLatch,
                pg_sys::WL_LATCH_SET as i32 | pg_sys::WL_TIMEOUT as i32,
                10, // 10ms
                pg_sys::PG_WAIT_EXTENSION as u32,
            );
            pg_sys::ResetLatch(pg_sys::MyLatch);
        }
    }
}

/// IPC Contract constants
const MAX_INLINE_SIZE: usize = 64 * 1024;           // 64 KB
const MAX_REQUEST_TIMEOUT_MS: u64 = 30_000;         // 30 seconds
const MAX_SUBMIT_RETRIES: u32 = 10;
```

---

## 6. SQL Control Functions

```sql
-- Start engine worker
CREATE FUNCTION ruvector_worker_start() RETURNS BOOLEAN
    AS 'MODULE_PATHNAME' LANGUAGE C;

-- Stop engine worker
CREATE FUNCTION ruvector_worker_stop() RETURNS BOOLEAN
    AS 'MODULE_PATHNAME' LANGUAGE C;

-- Get worker status
CREATE FUNCTION ruvector_worker_status() RETURNS JSONB
    AS 'MODULE_PATHNAME' LANGUAGE C;

-- Configure workers
CREATE FUNCTION ruvector_worker_config(
    engine_memory_mb INTEGER DEFAULT NULL,
    maintenance_interval_secs INTEGER DEFAULT NULL,
    integrity_sample_interval_secs INTEGER DEFAULT NULL
) RETURNS JSONB
    AS 'MODULE_PATHNAME' LANGUAGE C;

-- Get worker statistics
CREATE FUNCTION ruvector_worker_stats() RETURNS JSONB
    AS 'MODULE_PATHNAME' LANGUAGE C;
```

---

## Testing Requirements

### Unit Tests
- Worker configuration parsing
- Shared memory operations
- Queue push/pop correctness
- Lambda cut computation

### Integration Tests
- Worker startup/shutdown
- Request/response round-trip
- State transition handling
- Graceful degradation

### Stress Tests
- Queue saturation
- Concurrent requests
- Memory pressure
- Worker crash recovery

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `pgrx` | PostgreSQL extension framework |
| `parking_lot` | Synchronization primitives |
| `crossbeam` | Lock-free data structures |
| `ed25519-dalek` | Signature generation |
| `serde` | Serialization |
