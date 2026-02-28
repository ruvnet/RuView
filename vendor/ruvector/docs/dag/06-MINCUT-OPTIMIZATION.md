# MinCut Optimization Specification

## Overview

This document specifies how the subpolynomial O(n^0.12) min-cut algorithm from `ruvector-mincut` integrates with the Neural DAG system for bottleneck detection and optimization.

## MinCut Integration Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        MINCUT OPTIMIZATION LAYER                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                     SUBPOLYNOMIAL MINCUT ENGINE                      │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │ Hierarchical│  │  LocalKCut  │  │  LinkCut    │                  │   │
│  │  │Decomposition│  │   Oracle    │  │    Tree     │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────┴───────────────────────────────────┐   │
│  │                     DAG CRITICALITY ANALYZER                         │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Operator   │  │ Bottleneck  │  │  Critical   │                  │   │
│  │  │ Criticality │  │  Detection  │  │   Path      │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────┴───────────────────────────────────┐   │
│  │                      OPTIMIZATION ACTIONS                            │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  │   │
│  │  │   Gated     │  │ Redundancy  │  │  Parallel   │  │   Self-    │  │   │
│  │  │  Attention  │  │  Injection  │  │  Expansion  │  │  Healing   │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## DAG MinCut Engine

### Core Structure

```rust
/// MinCut engine adapted for query plan DAGs
pub struct DagMinCutEngine {
    /// Subpolynomial min-cut algorithm
    mincut: SubpolynomialMinCut,

    /// Graph representation of current DAG
    graph: DynamicGraph,

    /// Cached criticality scores
    criticality_cache: DashMap<OperatorId, f32>,

    /// Configuration
    config: MinCutConfig,

    /// Metrics
    metrics: MinCutMetrics,
}

#[derive(Clone, Debug)]
pub struct MinCutConfig {
    /// Enable/disable mincut analysis
    pub enabled: bool,

    /// Criticality threshold for bottleneck detection
    pub bottleneck_threshold: f32,

    /// Maximum operators to analyze
    pub max_operators: usize,

    /// Cache TTL in seconds
    pub cache_ttl_secs: u64,

    /// Enable self-healing
    pub self_healing_enabled: bool,

    /// Healing check interval
    pub healing_interval_ms: u64,
}

impl Default for MinCutConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bottleneck_threshold: 0.5,
            max_operators: 1000,
            cache_ttl_secs: 300,
            self_healing_enabled: true,
            healing_interval_ms: 300000,  // 5 minutes
        }
    }
}
```

### Graph Construction from DAG

```rust
impl DagMinCutEngine {
    /// Build graph from query plan DAG
    pub fn build_from_plan(&mut self, plan: &NeuralDagPlan) {
        self.graph.clear();

        // Add vertices (operators)
        for op in &plan.operators {
            let weight = self.operator_weight(op);
            self.graph.add_vertex(op.id, weight);
        }

        // Add edges (data flow)
        for (&parent_id, children) in &plan.edges {
            for &child_id in children {
                // Edge weight = data volume estimate
                let parent_op = plan.get_operator(parent_id);
                let weight = parent_op.estimated_rows as f64;
                self.graph.add_edge(parent_id, child_id, weight);
            }
        }

        // Initialize min-cut structure
        self.mincut.initialize(&self.graph);
    }

    /// Compute operator weight for min-cut
    fn operator_weight(&self, op: &OperatorNode) -> f64 {
        // Weight based on:
        // 1. Estimated cost (primary)
        // 2. Blocking nature (pipeline breakers are heavier)
        // 3. Parallelizability (less parallelizable = heavier)

        let base_weight = op.estimated_cost;

        let blocking_factor = if op.is_pipeline_breaker() {
            2.0
        } else {
            1.0
        };

        let parallel_factor = match op.op_type {
            OperatorType::Sort | OperatorType::Aggregate => 1.5,
            OperatorType::HashJoin => 1.2,
            _ => 1.0,
        };

        base_weight * blocking_factor * parallel_factor
    }
}
```

## Criticality Computation

### Operator Criticality

```rust
impl DagMinCutEngine {
    /// Compute criticality for all operators
    pub fn compute_all_criticalities(&self, plan: &NeuralDagPlan) -> HashMap<OperatorId, f32> {
        let global_cut = self.mincut.query();
        let mut criticalities = HashMap::new();

        for op in &plan.operators {
            let criticality = self.compute_operator_criticality(op.id, global_cut);
            criticalities.insert(op.id, criticality);
        }

        criticalities
    }

    /// Compute criticality for a single operator
    /// Criticality = how much removing this operator would reduce min-cut
    pub fn compute_operator_criticality(&self, op_id: OperatorId, global_cut: u64) -> f32 {
        // Check cache first
        if let Some(cached) = self.criticality_cache.get(&op_id) {
            return *cached;
        }

        // Use LocalKCut oracle
        let query = LocalKCutQuery {
            seed_vertices: vec![op_id],
            budget_k: global_cut,
            radius: 3,  // Local neighborhood
        };

        let criticality = match self.mincut.local_query(query) {
            LocalKCutResult::Found { cut_value, .. } => {
                // Criticality = (global - local) / global
                if global_cut > 0 {
                    (global_cut - cut_value) as f32 / global_cut as f32
                } else {
                    0.0
                }
            }
            LocalKCutResult::NoneInLocality => 0.0,
        };

        // Cache result
        self.criticality_cache.insert(op_id, criticality);

        criticality
    }

    /// Identify bottleneck operators
    pub fn identify_bottlenecks(&self, plan: &NeuralDagPlan) -> Vec<BottleneckInfo> {
        let criticalities = self.compute_all_criticalities(plan);

        let mut bottlenecks: Vec<_> = criticalities.iter()
            .filter(|(_, &crit)| crit > self.config.bottleneck_threshold)
            .map(|(&op_id, &crit)| {
                let op = plan.get_operator(op_id);
                BottleneckInfo {
                    operator_id: op_id,
                    operator_type: op.op_type.clone(),
                    criticality: crit,
                    estimated_cost: op.estimated_cost,
                    recommendation: self.generate_recommendation(op, crit),
                }
            })
            .collect();

        // Sort by criticality (most critical first)
        bottlenecks.sort_by(|a, b| b.criticality.partial_cmp(&a.criticality).unwrap());

        bottlenecks
    }

    /// Generate optimization recommendation for bottleneck
    fn generate_recommendation(&self, op: &OperatorNode, criticality: f32) -> OptimizationRecommendation {
        match op.op_type {
            OperatorType::SeqScan => {
                OptimizationRecommendation::CreateIndex {
                    table: op.table_name.clone().unwrap_or_default(),
                    columns: op.filter.as_ref()
                        .map(|f| f.columns())
                        .unwrap_or_default(),
                }
            }

            OperatorType::HnswScan | OperatorType::IvfFlatScan => {
                if criticality > 0.8 {
                    OptimizationRecommendation::IncreaseEfSearch {
                        current: 40,  // Would be extracted from plan
                        recommended: 80,
                    }
                } else {
                    OptimizationRecommendation::None
                }
            }

            OperatorType::NestedLoop => {
                OptimizationRecommendation::ConsiderHashJoin {
                    estimated_improvement: criticality * 50.0,
                }
            }

            OperatorType::Sort => {
                if op.estimated_rows > 100000.0 {
                    OptimizationRecommendation::AddSortIndex {
                        columns: op.projection.clone(),
                    }
                } else {
                    OptimizationRecommendation::None
                }
            }

            OperatorType::HashAggregate if op.estimated_rows > 1000000.0 => {
                OptimizationRecommendation::ConsiderPartitioning {
                    partition_key: op.projection.first().cloned(),
                }
            }

            _ => OptimizationRecommendation::None,
        }
    }
}

/// Information about a bottleneck
#[derive(Clone, Debug)]
pub struct BottleneckInfo {
    pub operator_id: OperatorId,
    pub operator_type: OperatorType,
    pub criticality: f32,
    pub estimated_cost: f64,
    pub recommendation: OptimizationRecommendation,
}

/// Optimization recommendations
#[derive(Clone, Debug)]
pub enum OptimizationRecommendation {
    None,
    CreateIndex { table: String, columns: Vec<String> },
    IncreaseEfSearch { current: usize, recommended: usize },
    ConsiderHashJoin { estimated_improvement: f32 },
    AddSortIndex { columns: Vec<String> },
    ConsiderPartitioning { partition_key: Option<String> },
    AddParallelism { recommended_workers: usize },
    MaterializeSubquery { subquery_id: OperatorId },
}
```

## MinCut Gated Attention Integration

### Gating Mechanism

```rust
impl DagMinCutEngine {
    /// Compute attention gates based on criticality
    pub fn compute_attention_gates(
        &self,
        plan: &NeuralDagPlan,
    ) -> Vec<f32> {
        let criticalities = self.compute_all_criticalities(plan);

        plan.operators.iter()
            .map(|op| {
                let crit = criticalities.get(&op.id).unwrap_or(&0.0);

                if *crit > self.config.bottleneck_threshold {
                    1.0  // Full attention for bottlenecks
                } else {
                    crit / self.config.bottleneck_threshold  // Scaled
                }
            })
            .collect()
    }

    /// Apply gating to attention weights
    pub fn gate_attention_weights(
        &self,
        weights: &[f32],
        gates: &[f32],
    ) -> Vec<f32> {
        assert_eq!(weights.len(), gates.len());

        let gated: Vec<f32> = weights.iter()
            .zip(gates.iter())
            .map(|(w, g)| w * g)
            .collect();

        // Renormalize
        let sum: f32 = gated.iter().sum();
        if sum > 1e-8 {
            gated.iter().map(|w| w / sum).collect()
        } else {
            vec![1.0 / weights.len() as f32; weights.len()]
        }
    }
}
```

## Dynamic Updates

### Incremental MinCut Maintenance

```rust
impl DagMinCutEngine {
    /// Handle operator cost update (O(n^0.12) amortized)
    pub fn update_operator_cost(&mut self, op_id: OperatorId, new_cost: f64) {
        let old_weight = self.graph.get_vertex_weight(op_id);
        let new_weight = new_cost * self.get_operator_factors(op_id);

        // Update graph
        self.graph.update_vertex_weight(op_id, new_weight);

        // Incremental min-cut update
        // The subpolynomial algorithm handles this efficiently
        self.mincut.on_vertex_weight_change(op_id, old_weight, new_weight);

        // Invalidate cache for affected operators
        self.invalidate_local_cache(op_id);
    }

    /// Handle edge addition (e.g., plan change)
    pub fn add_edge(&mut self, from: OperatorId, to: OperatorId, weight: f64) {
        self.graph.add_edge(from, to, weight);
        self.mincut.insert_edge(from, to);
        self.invalidate_local_cache(from);
        self.invalidate_local_cache(to);
    }

    /// Handle edge removal
    pub fn remove_edge(&mut self, from: OperatorId, to: OperatorId) {
        self.graph.remove_edge(from, to);
        self.mincut.delete_edge(from, to);
        self.invalidate_local_cache(from);
        self.invalidate_local_cache(to);
    }

    /// Invalidate cache for operator and neighbors
    fn invalidate_local_cache(&self, op_id: OperatorId) {
        self.criticality_cache.remove(&op_id);

        // Also invalidate neighbors (within radius 3)
        let neighbors = self.graph.get_neighbors_within_radius(op_id, 3);
        for neighbor in neighbors {
            self.criticality_cache.remove(&neighbor);
        }
    }
}
```

## Self-Healing Integration

### Bottleneck Detection Loop

```rust
impl DagMinCutEngine {
    /// Background bottleneck detection
    pub fn run_health_check(&self, plan: &NeuralDagPlan) -> HealthCheckResult {
        let start = Instant::now();

        // Compute global min-cut
        let global_cut = self.mincut.query();

        // Identify bottlenecks
        let bottlenecks = self.identify_bottlenecks(plan);

        // Compute health score
        let health_score = self.compute_health_score(&bottlenecks);

        // Generate alerts if needed
        let alerts = self.generate_alerts(&bottlenecks);

        HealthCheckResult {
            global_mincut: global_cut,
            health_score,
            bottleneck_count: bottlenecks.len(),
            severe_bottlenecks: bottlenecks.iter()
                .filter(|b| b.criticality > 0.8)
                .count(),
            bottlenecks,
            alerts,
            duration: start.elapsed(),
        }
    }

    fn compute_health_score(&self, bottlenecks: &[BottleneckInfo]) -> f32 {
        if bottlenecks.is_empty() {
            return 1.0;
        }

        // Score decreases with bottleneck severity
        let max_criticality = bottlenecks.iter()
            .map(|b| b.criticality)
            .fold(0.0, f32::max);

        let avg_criticality = bottlenecks.iter()
            .map(|b| b.criticality)
            .sum::<f32>() / bottlenecks.len() as f32;

        1.0 - (max_criticality * 0.6 + avg_criticality * 0.4)
    }

    fn generate_alerts(&self, bottlenecks: &[BottleneckInfo]) -> Vec<Alert> {
        bottlenecks.iter()
            .filter(|b| b.criticality > 0.7)
            .map(|b| Alert {
                severity: if b.criticality > 0.9 {
                    AlertSeverity::Critical
                } else if b.criticality > 0.8 {
                    AlertSeverity::Warning
                } else {
                    AlertSeverity::Info
                },
                message: format!(
                    "Bottleneck detected: {:?} (criticality: {:.2})",
                    b.operator_type, b.criticality
                ),
                recommendation: b.recommendation.clone(),
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct HealthCheckResult {
    pub global_mincut: u64,
    pub health_score: f32,
    pub bottleneck_count: usize,
    pub severe_bottlenecks: usize,
    pub bottlenecks: Vec<BottleneckInfo>,
    pub alerts: Vec<Alert>,
    pub duration: Duration,
}

#[derive(Clone, Debug)]
pub struct Alert {
    pub severity: AlertSeverity,
    pub message: String,
    pub recommendation: OptimizationRecommendation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}
```

## Redundancy Injection

### Bypass Path Creation

```rust
impl DagMinCutEngine {
    /// Suggest redundant paths to reduce bottleneck impact
    pub fn suggest_redundancy(&self, plan: &NeuralDagPlan) -> Vec<RedundancySuggestion> {
        let bottlenecks = self.identify_bottlenecks(plan);
        let mut suggestions = Vec::new();

        for bottleneck in &bottlenecks {
            if bottleneck.criticality > 0.7 {
                // Find alternative paths around this operator
                let alternatives = self.find_alternative_paths(
                    plan,
                    bottleneck.operator_id,
                );

                if let Some(alt) = alternatives.first() {
                    suggestions.push(RedundancySuggestion {
                        bottleneck_id: bottleneck.operator_id,
                        alternative_path: alt.clone(),
                        estimated_improvement: self.estimate_improvement(
                            bottleneck,
                            alt,
                        ),
                    });
                }
            }
        }

        suggestions
    }

    fn find_alternative_paths(
        &self,
        plan: &NeuralDagPlan,
        bottleneck_id: OperatorId,
    ) -> Vec<AlternativePath> {
        let mut alternatives = Vec::new();

        let bottleneck = plan.get_operator(bottleneck_id);

        match bottleneck.op_type {
            OperatorType::SeqScan => {
                // Alternative: Index scan if index exists
                if let Some(ref table) = bottleneck.table_name {
                    if let Some(index) = self.find_usable_index(table, &bottleneck.filter) {
                        alternatives.push(AlternativePath::UseIndex {
                            index_name: index,
                            estimated_speedup: 10.0,
                        });
                    }
                }
            }

            OperatorType::NestedLoop => {
                // Alternative: Hash join
                alternatives.push(AlternativePath::ReplaceJoin {
                    new_join_type: OperatorType::HashJoin,
                    estimated_speedup: 5.0,
                });
            }

            OperatorType::Sort => {
                // Alternative: Pre-sorted input via index
                alternatives.push(AlternativePath::SortedIndex {
                    columns: bottleneck.projection.clone(),
                    estimated_speedup: 3.0,
                });
            }

            _ => {}
        }

        alternatives
    }

    fn estimate_improvement(
        &self,
        bottleneck: &BottleneckInfo,
        alternative: &AlternativePath,
    ) -> f32 {
        let base_cost = bottleneck.estimated_cost;
        let speedup = alternative.estimated_speedup();

        let new_cost = base_cost / speedup;
        let improvement = (base_cost - new_cost) / base_cost;

        improvement as f32 * bottleneck.criticality
    }
}

#[derive(Clone, Debug)]
pub struct RedundancySuggestion {
    pub bottleneck_id: OperatorId,
    pub alternative_path: AlternativePath,
    pub estimated_improvement: f32,
}

#[derive(Clone, Debug)]
pub enum AlternativePath {
    UseIndex { index_name: String, estimated_speedup: f64 },
    ReplaceJoin { new_join_type: OperatorType, estimated_speedup: f64 },
    SortedIndex { columns: Vec<String>, estimated_speedup: f64 },
    Materialize { subquery_id: OperatorId, estimated_speedup: f64 },
    Parallelize { workers: usize, estimated_speedup: f64 },
}

impl AlternativePath {
    fn estimated_speedup(&self) -> f64 {
        match self {
            Self::UseIndex { estimated_speedup, .. } => *estimated_speedup,
            Self::ReplaceJoin { estimated_speedup, .. } => *estimated_speedup,
            Self::SortedIndex { estimated_speedup, .. } => *estimated_speedup,
            Self::Materialize { estimated_speedup, .. } => *estimated_speedup,
            Self::Parallelize { estimated_speedup, .. } => *estimated_speedup,
        }
    }
}
```

## SQL Interface

```sql
-- Compute mincut criticality for a plan
SELECT * FROM ruvector_dag_mincut_criticality('documents');

-- Get bottleneck analysis
SELECT * FROM ruvector_dag_bottlenecks('documents');

-- Get health check result
SELECT ruvector_dag_mincut_health('documents');

-- Get redundancy suggestions
SELECT * FROM ruvector_dag_redundancy_suggestions('documents');

-- Enable/disable mincut analysis
SET ruvector.dag_mincut_enabled = true;
SET ruvector.dag_mincut_threshold = 0.5;
```

## Performance Characteristics

| Operation | Complexity | Typical Latency |
|-----------|------------|-----------------|
| Global min-cut query | O(1) | <1μs |
| Single criticality | O(n^0.12) | <100μs |
| All criticalities | O(n^1.12) | <10ms (100 ops) |
| Edge insert | O(n^0.12) amortized | <100μs |
| Edge delete | O(n^0.12) amortized | <100μs |
| Health check | O(n^1.12) | <50ms |

## Memory Usage

| Component | Size | Notes |
|-----------|------|-------|
| Graph structure | O(n + m) | Vertices + edges |
| Hierarchical decomposition | O(n log n) | Multi-level |
| LinkCut tree | O(n) | Sleator-Tarjan |
| Criticality cache | O(n) | Bounded by TTL |
| LocalKCut coloring | O(k² log n) | Per query |

**Typical overhead:** ~1MB per 1000 operators
