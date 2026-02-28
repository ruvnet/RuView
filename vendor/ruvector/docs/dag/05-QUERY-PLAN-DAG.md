# Query Plan as Learnable DAG

## Overview

This document specifies how PostgreSQL query plans are represented as DAGs (Directed Acyclic Graphs) and how they become targets for neural learning.

## Query Plan DAG Structure

### Conceptual Model

```
                    ┌─────────────┐
                    │   RESULT    │  (Root)
                    └──────┬──────┘
                           │
                    ┌──────┴──────┐
                    │    SORT     │
                    └──────┬──────┘
                           │
              ┌────────────┴────────────┐
              │                         │
       ┌──────┴──────┐           ┌──────┴──────┐
       │   FILTER    │           │   FILTER    │
       └──────┬──────┘           └──────┬──────┘
              │                         │
       ┌──────┴──────┐           ┌──────┴──────┐
       │  HNSW SCAN  │           │  SEQ SCAN   │
       │ (documents) │           │  (authors)  │
       └─────────────┘           └─────────────┘

       Leaf Nodes                 Leaf Nodes
```

### NeuralDagPlan Structure

```rust
/// Query plan enhanced with neural learning capabilities
#[derive(Clone, Debug)]
pub struct NeuralDagPlan {
    // ═══════════════════════════════════════════════════════════════
    // BASE PLAN STRUCTURE
    // ═══════════════════════════════════════════════════════════════

    /// Plan ID (unique per execution)
    pub plan_id: u64,

    /// Root operator
    pub root: OperatorNode,

    /// All operators in topological order (leaves first)
    pub operators: Vec<OperatorNode>,

    /// Edges: parent_id -> Vec<child_id>
    pub edges: HashMap<OperatorId, Vec<OperatorId>>,

    /// Reverse edges: child_id -> parent_id
    pub reverse_edges: HashMap<OperatorId, OperatorId>,

    /// Pipeline breakers (blocking operators)
    pub pipeline_breakers: Vec<OperatorId>,

    /// Parallelism configuration
    pub parallelism: usize,

    // ═══════════════════════════════════════════════════════════════
    // NEURAL ENHANCEMENTS
    // ═══════════════════════════════════════════════════════════════

    /// Operator embeddings (256-dim per operator)
    pub operator_embeddings: Vec<Vec<f32>>,

    /// Plan embedding (computed from operators)
    pub plan_embedding: Option<Vec<f32>>,

    /// Attention weights between operators
    pub attention_weights: Vec<Vec<f32>>,

    /// Selected attention type
    pub attention_type: DagAttentionType,

    /// Trajectory ID (links to ReasoningBank)
    pub trajectory_id: Option<u64>,

    // ═══════════════════════════════════════════════════════════════
    // LEARNED PARAMETERS
    // ═══════════════════════════════════════════════════════════════

    /// Learned cost estimates per operator
    pub learned_costs: Option<Vec<f32>>,

    /// Execution parameters
    pub params: ExecutionParams,

    /// Pattern match info (if pattern was applied)
    pub pattern_match: Option<PatternMatch>,

    // ═══════════════════════════════════════════════════════════════
    // OPTIMIZATION STATE
    // ═══════════════════════════════════════════════════════════════

    /// MinCut criticality per operator
    pub criticalities: Option<Vec<f32>>,

    /// Critical path operators
    pub critical_path: Option<Vec<OperatorId>>,

    /// Bottleneck score (0.0 - 1.0)
    pub bottleneck_score: Option<f32>,
}

/// Single operator in the plan DAG
#[derive(Clone, Debug)]
pub struct OperatorNode {
    /// Unique operator ID
    pub id: OperatorId,

    /// Operator type
    pub op_type: OperatorType,

    /// Target table (if applicable)
    pub table_name: Option<String>,

    /// Index used (if applicable)
    pub index_name: Option<String>,

    /// Filter predicate (if applicable)
    pub filter: Option<FilterExpr>,

    /// Join condition (if join)
    pub join_condition: Option<JoinCondition>,

    /// Projected columns
    pub projection: Vec<String>,

    /// Estimated rows
    pub estimated_rows: f64,

    /// Estimated cost
    pub estimated_cost: f64,

    /// Operator embedding (learned)
    pub embedding: Vec<f32>,

    /// Depth in DAG (0 = leaf)
    pub depth: usize,

    /// Is this on critical path?
    pub is_critical: bool,

    /// MinCut criticality score
    pub criticality: f32,
}

/// Operator types
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OperatorType {
    // Scan operators (leaves)
    SeqScan,
    IndexScan,
    IndexOnlyScan,
    HnswScan,
    IvfFlatScan,
    BitmapScan,

    // Join operators
    NestedLoop,
    HashJoin,
    MergeJoin,

    // Aggregation operators
    Aggregate,
    GroupAggregate,
    HashAggregate,

    // Sort operators
    Sort,
    IncrementalSort,

    // Filter operators
    Filter,
    Result,

    // Set operators
    Append,
    MergeAppend,
    Union,
    Intersect,
    Except,

    // Subquery operators
    SubqueryScan,
    CteScan,
    MaterializeNode,

    // Utility
    Limit,
    Unique,
    WindowAgg,

    // Parallel
    Gather,
    GatherMerge,
}

/// Pattern match information
#[derive(Clone, Debug)]
pub struct PatternMatch {
    pub pattern_id: PatternId,
    pub confidence: f32,
    pub similarity: f32,
    pub applied_params: ExecutionParams,
}
```

### Operator Embedding

```rust
impl OperatorNode {
    /// Generate embedding for this operator
    pub fn generate_embedding(&mut self, config: &EmbeddingConfig) {
        let dim = config.hidden_dim;
        let mut embedding = vec![0.0; dim];

        // 1. Operator type encoding (one-hot style, but dense)
        let type_offset = self.op_type.type_index() * 16;
        for i in 0..16 {
            embedding[type_offset + i] = self.op_type.type_features()[i];
        }

        // 2. Cardinality encoding (log scale)
        let card_offset = 128;
        let log_rows = (self.estimated_rows + 1.0).ln();
        embedding[card_offset] = log_rows / 20.0;  // Normalize

        // 3. Cost encoding (log scale)
        let cost_offset = 129;
        let log_cost = (self.estimated_cost + 1.0).ln();
        embedding[cost_offset] = log_cost / 30.0;  // Normalize

        // 4. Depth encoding
        let depth_offset = 130;
        embedding[depth_offset] = self.depth as f32 / 20.0;

        // 5. Table/index encoding (if applicable)
        if let Some(ref table) = self.table_name {
            let table_hash = hash_string(table);
            let table_offset = 132;
            for i in 0..16 {
                embedding[table_offset + i] = ((table_hash >> (i * 4)) & 0xF) as f32 / 16.0;
            }
        }

        // 6. Filter complexity encoding
        if let Some(ref filter) = self.filter {
            let filter_offset = 148;
            embedding[filter_offset] = filter.complexity() as f32 / 10.0;
            embedding[filter_offset + 1] = filter.selectivity_estimate();
        }

        // 7. Join encoding
        if let Some(ref join) = self.join_condition {
            let join_offset = 150;
            embedding[join_offset] = join.join_type.type_index() as f32 / 4.0;
            embedding[join_offset + 1] = join.estimated_selectivity;
        }

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        self.embedding = embedding;
    }
}

impl OperatorType {
    /// Get feature vector for operator type
    fn type_features(&self) -> [f32; 16] {
        match self {
            // Scans - low cost per row
            OperatorType::SeqScan => [1.0, 0.0, 0.0, 0.0, 0.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            OperatorType::IndexScan => [0.8, 0.2, 0.0, 0.0, 0.1, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            OperatorType::HnswScan => [0.6, 0.4, 0.0, 0.0, 0.05, 0.8, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            OperatorType::IvfFlatScan => [0.7, 0.3, 0.0, 0.0, 0.08, 0.7, 0.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],

            // Joins - high cost
            OperatorType::NestedLoop => [0.0, 0.0, 1.0, 0.0, 0.9, 0.0, 0.0, 0.8, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            OperatorType::HashJoin => [0.0, 0.0, 0.8, 0.2, 0.5, 0.0, 0.0, 0.6, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            OperatorType::MergeJoin => [0.0, 0.0, 0.6, 0.4, 0.4, 0.0, 0.0, 0.4, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],

            // Aggregation - blocking
            OperatorType::Aggregate => [0.0, 0.0, 0.0, 0.0, 0.3, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0],
            OperatorType::HashAggregate => [0.0, 0.0, 0.0, 0.0, 0.4, 0.0, 0.0, 0.0, 0.5, 0.8, 0.0, 0.6, 0.0, 0.0, 0.0, 0.0],

            // Sort - blocking
            OperatorType::Sort => [0.0, 0.0, 0.0, 0.0, 0.6, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.7, 0.0, 0.0, 0.0, 0.0],

            // Default
            _ => [0.5; 16],
        }
    }

    fn type_index(&self) -> usize {
        match self {
            OperatorType::SeqScan => 0,
            OperatorType::IndexScan => 1,
            OperatorType::IndexOnlyScan => 1,
            OperatorType::HnswScan => 2,
            OperatorType::IvfFlatScan => 3,
            OperatorType::BitmapScan => 4,
            OperatorType::NestedLoop => 5,
            OperatorType::HashJoin => 6,
            OperatorType::MergeJoin => 7,
            OperatorType::Aggregate | OperatorType::GroupAggregate | OperatorType::HashAggregate => 8,
            OperatorType::Sort | OperatorType::IncrementalSort => 9,
            OperatorType::Filter => 10,
            OperatorType::Limit => 11,
            _ => 12,
        }
    }
}
```

## Plan Conversion from PostgreSQL

### PlannedStmt to NeuralDagPlan

```rust
impl NeuralDagPlan {
    /// Convert PostgreSQL PlannedStmt to NeuralDagPlan
    pub unsafe fn from_planned_stmt(stmt: *mut pg_sys::PlannedStmt) -> Self {
        let mut plan = NeuralDagPlan::new();

        // Extract plan tree
        let plan_tree = (*stmt).planTree;
        plan.root = Self::convert_plan_node(plan_tree, &mut plan, 0);

        // Compute topological order
        plan.compute_topological_order();

        // Generate embeddings
        plan.generate_embeddings();

        // Identify pipeline breakers
        plan.identify_pipeline_breakers();

        plan
    }

    /// Recursively convert plan nodes
    unsafe fn convert_plan_node(
        node: *mut pg_sys::Plan,
        plan: &mut NeuralDagPlan,
        depth: usize,
    ) -> OperatorNode {
        if node.is_null() {
            panic!("Null plan node");
        }

        let node_type = (*node).type_;
        let estimated_rows = (*node).plan_rows;
        let estimated_cost = (*node).total_cost;

        let op_type = Self::pg_node_to_op_type(node_type, node);
        let op_id = plan.next_operator_id();

        let mut operator = OperatorNode {
            id: op_id,
            op_type,
            table_name: Self::extract_table_name(node),
            index_name: Self::extract_index_name(node),
            filter: Self::extract_filter(node),
            join_condition: Self::extract_join_condition(node),
            projection: Self::extract_projection(node),
            estimated_rows,
            estimated_cost,
            embedding: vec![],
            depth,
            is_critical: false,
            criticality: 0.0,
        };

        // Process children
        let left_plan = (*node).lefttree;
        let right_plan = (*node).righttree;

        let mut child_ids = Vec::new();

        if !left_plan.is_null() {
            let left_op = Self::convert_plan_node(left_plan, plan, depth + 1);
            child_ids.push(left_op.id);
            plan.reverse_edges.insert(left_op.id, op_id);
            plan.operators.push(left_op);
        }

        if !right_plan.is_null() {
            let right_op = Self::convert_plan_node(right_plan, plan, depth + 1);
            child_ids.push(right_op.id);
            plan.reverse_edges.insert(right_op.id, op_id);
            plan.operators.push(right_op);
        }

        if !child_ids.is_empty() {
            plan.edges.insert(op_id, child_ids);
        }

        operator
    }

    /// Map PostgreSQL node type to OperatorType
    unsafe fn pg_node_to_op_type(node_type: pg_sys::NodeTag, node: *mut pg_sys::Plan) -> OperatorType {
        match node_type {
            pg_sys::NodeTag::T_SeqScan => OperatorType::SeqScan,
            pg_sys::NodeTag::T_IndexScan => {
                // Check if it's HNSW or IVFFlat
                let index_scan = node as *mut pg_sys::IndexScan;
                let index_oid = (*index_scan).indexid;

                if Self::is_hnsw_index(index_oid) {
                    OperatorType::HnswScan
                } else if Self::is_ivfflat_index(index_oid) {
                    OperatorType::IvfFlatScan
                } else {
                    OperatorType::IndexScan
                }
            }
            pg_sys::NodeTag::T_IndexOnlyScan => OperatorType::IndexOnlyScan,
            pg_sys::NodeTag::T_BitmapHeapScan => OperatorType::BitmapScan,
            pg_sys::NodeTag::T_NestLoop => OperatorType::NestedLoop,
            pg_sys::NodeTag::T_HashJoin => OperatorType::HashJoin,
            pg_sys::NodeTag::T_MergeJoin => OperatorType::MergeJoin,
            pg_sys::NodeTag::T_Agg => {
                let agg = node as *mut pg_sys::Agg;
                match (*agg).aggstrategy {
                    pg_sys::AggStrategy::AGG_HASHED => OperatorType::HashAggregate,
                    pg_sys::AggStrategy::AGG_SORTED => OperatorType::GroupAggregate,
                    _ => OperatorType::Aggregate,
                }
            }
            pg_sys::NodeTag::T_Sort => OperatorType::Sort,
            pg_sys::NodeTag::T_IncrementalSort => OperatorType::IncrementalSort,
            pg_sys::NodeTag::T_Limit => OperatorType::Limit,
            pg_sys::NodeTag::T_Unique => OperatorType::Unique,
            pg_sys::NodeTag::T_Append => OperatorType::Append,
            pg_sys::NodeTag::T_MergeAppend => OperatorType::MergeAppend,
            pg_sys::NodeTag::T_Gather => OperatorType::Gather,
            pg_sys::NodeTag::T_GatherMerge => OperatorType::GatherMerge,
            pg_sys::NodeTag::T_WindowAgg => OperatorType::WindowAgg,
            pg_sys::NodeTag::T_SubqueryScan => OperatorType::SubqueryScan,
            pg_sys::NodeTag::T_CteScan => OperatorType::CteScan,
            pg_sys::NodeTag::T_Material => OperatorType::MaterializeNode,
            pg_sys::NodeTag::T_Result => OperatorType::Result,
            _ => OperatorType::Filter,  // Default
        }
    }
}
```

## Plan Embedding Computation

### Hierarchical Aggregation

```rust
impl NeuralDagPlan {
    /// Generate plan-level embedding from operator embeddings
    pub fn generate_plan_embedding(&mut self) {
        let dim = self.operator_embeddings[0].len();
        let mut plan_embedding = vec![0.0; dim];

        // Method 1: Weighted sum by depth (deeper = lower weight)
        let max_depth = self.operators.iter().map(|o| o.depth).max().unwrap_or(0);

        for (i, op) in self.operators.iter().enumerate() {
            let depth_weight = 1.0 / (op.depth as f32 + 1.0);
            let cost_weight = (op.estimated_cost / self.total_cost()).min(1.0) as f32;
            let weight = depth_weight * 0.5 + cost_weight * 0.5;

            for (j, &val) in self.operator_embeddings[i].iter().enumerate() {
                plan_embedding[j] += weight * val;
            }
        }

        // L2 normalize
        let norm: f32 = plan_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            for x in &mut plan_embedding {
                *x /= norm;
            }
        }

        self.plan_embedding = Some(plan_embedding);
    }

    /// Generate embedding using attention over operators
    pub fn generate_plan_embedding_with_attention(&mut self, attention: &dyn DagAttention) {
        // Use root operator as query
        let root_embedding = &self.operator_embeddings[0];

        // Build context from all operators
        let ctx = self.build_dag_context();

        // Compute attention weights
        let query_node = DagNode {
            id: self.root.id,
            embedding: root_embedding.clone(),
        };

        let output = attention.forward(&query_node, &ctx, &AttentionConfig::default())
            .expect("Attention computation failed");

        // Store attention weights
        self.attention_weights = vec![output.weights.clone()];

        // Use aggregated output as plan embedding
        self.plan_embedding = Some(output.aggregated);
    }

    fn build_dag_context(&self) -> DagContext {
        DagContext {
            nodes: self.operators.iter()
                .map(|op| DagNode {
                    id: op.id,
                    embedding: op.embedding.clone(),
                })
                .collect(),
            edges: self.edges.clone(),
            reverse_edges: self.reverse_edges.iter()
                .map(|(&child, &parent)| (child, vec![parent]))
                .collect(),
            depths: self.operators.iter()
                .map(|op| (op.id, op.depth))
                .collect(),
            timestamps: None,
            criticalities: self.criticalities.as_ref().map(|c| {
                self.operators.iter()
                    .enumerate()
                    .map(|(i, op)| (op.id, c[i]))
                    .collect()
            }),
        }
    }
}
```

## Plan Optimization

### Learned Cost Adjustment

```rust
impl NeuralDagPlan {
    /// Apply learned cost adjustments
    pub fn apply_learned_costs(&mut self) {
        if let Some(ref learned_costs) = self.learned_costs {
            for (i, op) in self.operators.iter_mut().enumerate() {
                if i < learned_costs.len() {
                    // Adjust estimated cost by learned factor
                    let adjustment = learned_costs[i];
                    op.estimated_cost *= (1.0 + adjustment) as f64;
                }
            }
        }
    }

    /// Reorder operators based on learned pattern
    pub fn reorder_operators(&mut self, optimal_ordering: &[OperatorId]) {
        // Only reorder within commutative operators (e.g., join order)
        let join_ops: Vec<_> = self.operators.iter()
            .filter(|op| matches!(op.op_type,
                OperatorType::HashJoin |
                OperatorType::MergeJoin |
                OperatorType::NestedLoop))
            .map(|op| op.id)
            .collect();

        if join_ops.len() < 2 {
            return;  // Nothing to reorder
        }

        // Apply learned ordering
        // This is a simplified version - real implementation needs
        // to preserve DAG constraints
        for (i, &target_id) in optimal_ordering.iter().enumerate() {
            if i < join_ops.len() {
                // Swap join operators to match target ordering
                // (preserving child relationships)
            }
        }
    }

    /// Apply learned execution parameters
    pub fn apply_params(&mut self, params: &ExecutionParams) {
        self.params = params.clone();

        // Apply to relevant operators
        for op in &mut self.operators {
            match op.op_type {
                OperatorType::HnswScan => {
                    if let Some(ef) = params.ef_search {
                        op.embedding[160] = ef as f32 / 100.0;  // Encode in embedding
                    }
                }
                OperatorType::IvfFlatScan => {
                    if let Some(probes) = params.probes {
                        op.embedding[161] = probes as f32 / 50.0;
                    }
                }
                _ => {}
            }
        }
    }
}
```

### Critical Path Analysis

```rust
impl NeuralDagPlan {
    /// Compute critical path through the plan DAG
    pub fn compute_critical_path(&mut self) {
        // Dynamic programming: longest path
        let mut longest_to: HashMap<OperatorId, f64> = HashMap::new();
        let mut longest_from: HashMap<OperatorId, f64> = HashMap::new();
        let mut predecessor: HashMap<OperatorId, OperatorId> = HashMap::new();

        // Forward pass (leaves to root) - longest path TO each node
        for op in self.operators.iter().rev() {  // Reverse topo order
            let mut max_cost = 0.0;
            let mut max_pred = None;

            if let Some(children) = self.edges.get(&op.id) {
                for &child_id in children {
                    let child_cost = longest_to.get(&child_id).unwrap_or(&0.0);
                    if *child_cost > max_cost {
                        max_cost = *child_cost;
                        max_pred = Some(child_id);
                    }
                }
            }

            longest_to.insert(op.id, max_cost + op.estimated_cost);
            if let Some(pred) = max_pred {
                predecessor.insert(op.id, pred);
            }
        }

        // Backward pass (root to leaves) - longest path FROM each node
        for op in &self.operators {
            let mut max_cost = 0.0;

            if let Some(&parent_id) = self.reverse_edges.get(&op.id) {
                let parent_cost = longest_from.get(&parent_id).unwrap_or(&0.0);
                max_cost = max_cost.max(*parent_cost + self.get_operator(parent_id).estimated_cost);
            }

            longest_from.insert(op.id, max_cost);
        }

        // Find critical path
        let global_longest = longest_to.values().cloned().fold(0.0, f64::max);

        let mut critical_path = Vec::new();
        for op in &self.operators {
            let total_through = longest_to[&op.id] + longest_from[&op.id];
            if (total_through - global_longest).abs() < 1e-6 {
                critical_path.push(op.id);
            }
        }

        // Mark operators
        for op in &mut self.operators {
            op.is_critical = critical_path.contains(&op.id);
        }

        self.critical_path = Some(critical_path);
    }

    /// Compute bottleneck score (0.0 - 1.0)
    pub fn compute_bottleneck_score(&mut self) {
        if let Some(ref critical_path) = self.critical_path {
            if critical_path.is_empty() {
                self.bottleneck_score = Some(0.0);
                return;
            }

            // Bottleneck = max(single_op_cost / total_cost)
            let total_cost = self.total_cost();
            let max_single = critical_path.iter()
                .map(|&id| self.get_operator(id).estimated_cost)
                .fold(0.0, f64::max);

            self.bottleneck_score = Some((max_single / total_cost) as f32);
        }
    }
}
```

## Learning Target: Plan Quality

### Quality Computation

```rust
/// Compute quality score for a plan execution
pub fn compute_plan_quality(plan: &NeuralDagPlan, metrics: &ExecutionMetrics) -> f32 {
    // Multi-objective quality function

    // 1. Latency score (lower is better)
    // Target: 10ms for simple queries, 1s for complex
    let complexity = plan.operators.len() as f32;
    let target_latency_us = 10000.0 * complexity.sqrt();
    let latency_score = (target_latency_us / (metrics.latency_us as f32 + 1.0)).min(1.0);

    // 2. Accuracy score (for vector queries)
    // If we have relevance feedback
    let accuracy_score = if let Some(precision) = metrics.precision {
        precision
    } else {
        1.0  // Assume accurate if no feedback
    };

    // 3. Efficiency score (rows per microsecond)
    let efficiency_score = if metrics.latency_us > 0 {
        (metrics.rows_processed as f32 / metrics.latency_us as f32 * 1000.0).min(1.0)
    } else {
        1.0
    };

    // 4. Memory score (lower is better)
    let target_memory = 10_000_000.0 * complexity;  // 10MB per operator
    let memory_score = (target_memory / (metrics.memory_bytes as f32 + 1.0)).min(1.0);

    // 5. Cache efficiency
    let cache_score = metrics.cache_hit_rate;

    // Weighted combination
    let weights = [0.35, 0.25, 0.15, 0.15, 0.10];
    let scores = [latency_score, accuracy_score, efficiency_score, memory_score, cache_score];

    weights.iter().zip(scores.iter())
        .map(|(w, s)| w * s)
        .sum()
}
```

### Gradient Estimation

```rust
impl DagTrajectory {
    /// Estimate gradient for REINFORCE-style learning
    pub fn estimate_gradient(&self) -> Vec<f32> {
        let dim = self.plan_embedding.len();
        let mut gradient = vec![0.0; dim];

        // REINFORCE with baseline
        let baseline = 0.5;  // Could be learned
        let advantage = self.quality - baseline;

        // gradient += advantage * activation
        // Simplified: use plan embedding as "activation"
        for (i, &val) in self.plan_embedding.iter().enumerate() {
            gradient[i] = advantage * val;
        }

        // Also incorporate operator-level signals
        for (op_idx, op_embedding) in self.operator_embeddings.iter().enumerate() {
            // Weight by attention
            let attention_weight = if op_idx < self.attention_weights.len() {
                self.attention_weights.get(0)
                    .and_then(|w| w.get(op_idx))
                    .unwrap_or(&(1.0 / self.operator_embeddings.len() as f32))
                    .clone()
            } else {
                1.0 / self.operator_embeddings.len() as f32
            };

            for (i, &val) in op_embedding.iter().enumerate() {
                if i < dim {
                    gradient[i] += advantage * val * attention_weight * 0.5;
                }
            }
        }

        // L2 normalize
        let norm: f32 = gradient.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            for x in &mut gradient {
                *x /= norm;
            }
        }

        gradient
    }
}
```

## Integration with PostgreSQL Planner

### Plan Modification Points

```rust
/// Points where neural DAG can influence planning
pub enum PlanModificationPoint {
    /// Before any planning
    PrePlanning,

    /// After join enumeration, before selecting best join order
    JoinOrdering,

    /// After creating base plan, before optimization
    PreOptimization,

    /// After optimization, before execution
    PostOptimization,

    /// During execution (adaptive)
    DuringExecution,
}

impl NeuralDagPlan {
    /// Apply neural modifications at specified point
    pub fn apply_modifications(&mut self, point: PlanModificationPoint, engine: &DagSonaEngine) {
        match point {
            PlanModificationPoint::PrePlanning => {
                // Hint optimal parameters based on query pattern
                self.apply_pre_planning_hints(engine);
            }

            PlanModificationPoint::JoinOrdering => {
                // Suggest optimal join order
                if let Some(ordering) = engine.suggest_join_order(&self.plan_embedding) {
                    self.reorder_operators(&ordering);
                }
            }

            PlanModificationPoint::PreOptimization => {
                // Adjust cost estimates
                if let Some(costs) = engine.predict_costs(&self.plan_embedding) {
                    self.learned_costs = Some(costs);
                    self.apply_learned_costs();
                }
            }

            PlanModificationPoint::PostOptimization => {
                // Final parameter tuning
                if let Some(params) = engine.suggest_params(&self.plan_embedding) {
                    self.apply_params(&params);
                }
            }

            PlanModificationPoint::DuringExecution => {
                // Adaptive re-planning (future work)
            }
        }
    }
}
```

## Serialization

### Plan Persistence

```rust
impl NeuralDagPlan {
    /// Serialize plan for storage
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Serialization failed")
    }

    /// Deserialize plan
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }

    /// Export to JSON for debugging
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "plan_id": self.plan_id,
            "operators": self.operators.iter().map(|op| json!({
                "id": op.id,
                "type": format!("{:?}", op.op_type),
                "table": op.table_name,
                "estimated_rows": op.estimated_rows,
                "estimated_cost": op.estimated_cost,
                "depth": op.depth,
                "is_critical": op.is_critical,
                "criticality": op.criticality,
            })).collect::<Vec<_>>(),
            "edges": self.edges,
            "attention_type": format!("{:?}", self.attention_type),
            "bottleneck_score": self.bottleneck_score,
            "params": {
                "ef_search": self.params.ef_search,
                "probes": self.params.probes,
                "parallelism": self.params.parallelism,
            }
        })
    }
}
```

## Performance Considerations

| Operation | Complexity | Target Latency |
|-----------|------------|----------------|
| Plan conversion | O(n) | <1ms |
| Embedding generation | O(n × d) | <500μs |
| Plan embedding | O(n × d) | <200μs |
| Critical path | O(n²) | <1ms |
| MinCut criticality | O(n^0.12) | <10ms |
| Pattern matching | O(k × d) | <1ms |

Where n = operators, d = embedding dimension (256), k = patterns (100).
