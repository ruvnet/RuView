//! DagSonaEngine: Main orchestration for SONA learning

use super::{
    DagReasoningBank, DagTrajectory, DagTrajectoryBuffer, EwcConfig, EwcPlusPlus, MicroLoRA,
    MicroLoRAConfig, ReasoningBankConfig,
};
use crate::dag::{OperatorType, QueryDag};
use ndarray::Array1;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub struct DagSonaEngine {
    micro_lora: MicroLoRA,
    trajectory_buffer: DagTrajectoryBuffer,
    reasoning_bank: DagReasoningBank,
    #[allow(dead_code)]
    ewc: EwcPlusPlus,
    embedding_dim: usize,
}

impl DagSonaEngine {
    pub fn new(embedding_dim: usize) -> Self {
        Self {
            micro_lora: MicroLoRA::new(MicroLoRAConfig::default(), embedding_dim),
            trajectory_buffer: DagTrajectoryBuffer::new(1000),
            reasoning_bank: DagReasoningBank::new(ReasoningBankConfig {
                pattern_dim: embedding_dim,
                ..Default::default()
            }),
            ewc: EwcPlusPlus::new(EwcConfig::default()),
            embedding_dim,
        }
    }

    /// Pre-query instant adaptation (<100μs)
    pub fn pre_query(&mut self, dag: &QueryDag) -> Vec<f32> {
        let embedding = self.compute_dag_embedding(dag);

        // Query similar patterns
        let similar = self.reasoning_bank.query_similar(&embedding, 3);

        // If we have similar patterns, adapt MicroLoRA
        if !similar.is_empty() {
            let adaptation_signal = self.compute_adaptation_signal(&similar, &embedding);
            self.micro_lora
                .adapt(&Array1::from_vec(adaptation_signal), 0.01);
        }

        // Return enhanced embedding
        self.micro_lora
            .forward(&Array1::from_vec(embedding))
            .to_vec()
    }

    /// Post-query trajectory recording
    pub fn post_query(
        &mut self,
        dag: &QueryDag,
        execution_time_ms: f64,
        baseline_time_ms: f64,
        attention_mechanism: &str,
    ) {
        let embedding = self.compute_dag_embedding(dag);
        let trajectory = DagTrajectory::new(
            self.hash_dag(dag),
            embedding,
            attention_mechanism.to_string(),
            execution_time_ms,
            baseline_time_ms,
        );

        self.trajectory_buffer.push(trajectory);
    }

    /// Background learning cycle (called periodically)
    pub fn background_learn(&mut self) {
        let trajectories = self.trajectory_buffer.drain();
        if trajectories.is_empty() {
            return;
        }

        // Store high-quality patterns
        for t in &trajectories {
            if t.quality() > 0.6 {
                self.reasoning_bank
                    .store_pattern(t.dag_embedding.clone(), t.quality());
            }
        }

        // Recompute clusters periodically (every 100 patterns)
        if self.reasoning_bank.pattern_count() % 100 == 0 {
            self.reasoning_bank.recompute_clusters();
        }
    }

    fn compute_dag_embedding(&self, dag: &QueryDag) -> Vec<f32> {
        // Compute embedding from DAG structure
        let mut embedding = vec![0.0; self.embedding_dim];

        if dag.node_count() == 0 {
            return embedding;
        }

        // Encode operator type distribution (20 different types)
        let mut type_counts = vec![0usize; 20];
        for node in dag.nodes() {
            let type_idx = match &node.op_type {
                OperatorType::SeqScan { .. } => 0,
                OperatorType::IndexScan { .. } => 1,
                OperatorType::HnswScan { .. } => 2,
                OperatorType::IvfFlatScan { .. } => 3,
                OperatorType::NestedLoopJoin => 4,
                OperatorType::HashJoin { .. } => 5,
                OperatorType::MergeJoin { .. } => 6,
                OperatorType::Aggregate { .. } => 7,
                OperatorType::GroupBy { .. } => 8,
                OperatorType::Filter { .. } => 9,
                OperatorType::Project { .. } => 10,
                OperatorType::Sort { .. } => 11,
                OperatorType::Limit { .. } => 12,
                OperatorType::VectorDistance { .. } => 13,
                OperatorType::Rerank { .. } => 14,
                OperatorType::Materialize => 15,
                OperatorType::Result => 16,
                #[allow(deprecated)]
                OperatorType::Scan => 0, // Treat as SeqScan
                #[allow(deprecated)]
                OperatorType::Join => 4, // Treat as NestedLoopJoin
            };
            if type_idx < type_counts.len() {
                type_counts[type_idx] += 1;
            }
        }

        // Normalize and place in embedding
        let total = dag.node_count() as f32;
        for (i, count) in type_counts.iter().enumerate() {
            if i < self.embedding_dim / 2 {
                embedding[i] = *count as f32 / total;
            }
        }

        // Encode structural features (depth, breadth, connectivity)
        let depth = self.compute_dag_depth(dag);
        let avg_fanout = dag.node_count() as f32 / (dag.leaves().len().max(1) as f32);

        if self.embedding_dim > 20 {
            embedding[20] = (depth as f32) / 10.0; // Normalize depth
            embedding[21] = avg_fanout / 5.0; // Normalize fanout
        }

        // Encode cost statistics
        let costs: Vec<f64> = dag.nodes().map(|n| n.estimated_cost).collect();
        if !costs.is_empty() && self.embedding_dim > 22 {
            let avg_cost = costs.iter().sum::<f64>() / costs.len() as f64;
            let max_cost = costs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            embedding[22] = (avg_cost / 1000.0) as f32; // Normalize
            embedding[23] = (max_cost / 1000.0) as f32;
        }

        // Normalize entire embedding
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            embedding.iter_mut().for_each(|x| *x /= norm);
        }

        embedding
    }

    fn compute_dag_depth(&self, dag: &QueryDag) -> usize {
        // BFS to find maximum depth
        use std::collections::VecDeque;

        let mut max_depth = 0;
        let mut queue = VecDeque::new();

        if let Some(root) = dag.root() {
            queue.push_back((root, 0));
        }

        while let Some((node_id, depth)) = queue.pop_front() {
            max_depth = max_depth.max(depth);
            for &child in dag.children(node_id) {
                queue.push_back((child, depth + 1));
            }
        }

        max_depth
    }

    fn compute_adaptation_signal(
        &self,
        _similar: &[(u64, f32)],
        _current_embedding: &[f32],
    ) -> Vec<f32> {
        // Weighted average of similar pattern embeddings
        // For now, just return zeros as we'd need to store pattern vectors
        vec![0.0; self.embedding_dim]
    }

    fn hash_dag(&self, dag: &QueryDag) -> u64 {
        let mut hasher = DefaultHasher::new();

        // Hash node types and edges
        for node in dag.nodes() {
            node.id.hash(&mut hasher);
            // Hash operator type discriminant
            match &node.op_type {
                OperatorType::SeqScan { table } => {
                    0u8.hash(&mut hasher);
                    table.hash(&mut hasher);
                }
                OperatorType::IndexScan { index, table } => {
                    1u8.hash(&mut hasher);
                    index.hash(&mut hasher);
                    table.hash(&mut hasher);
                }
                OperatorType::HnswScan { index, ef_search } => {
                    2u8.hash(&mut hasher);
                    index.hash(&mut hasher);
                    ef_search.hash(&mut hasher);
                }
                OperatorType::IvfFlatScan { index, nprobe } => {
                    3u8.hash(&mut hasher);
                    index.hash(&mut hasher);
                    nprobe.hash(&mut hasher);
                }
                OperatorType::NestedLoopJoin => 4u8.hash(&mut hasher),
                OperatorType::HashJoin { hash_key } => {
                    5u8.hash(&mut hasher);
                    hash_key.hash(&mut hasher);
                }
                OperatorType::MergeJoin { merge_key } => {
                    6u8.hash(&mut hasher);
                    merge_key.hash(&mut hasher);
                }
                OperatorType::Aggregate { functions } => {
                    7u8.hash(&mut hasher);
                    for func in functions {
                        func.hash(&mut hasher);
                    }
                }
                OperatorType::GroupBy { keys } => {
                    8u8.hash(&mut hasher);
                    for key in keys {
                        key.hash(&mut hasher);
                    }
                }
                OperatorType::Filter { predicate } => {
                    9u8.hash(&mut hasher);
                    predicate.hash(&mut hasher);
                }
                OperatorType::Project { columns } => {
                    10u8.hash(&mut hasher);
                    for col in columns {
                        col.hash(&mut hasher);
                    }
                }
                OperatorType::Sort { keys, descending } => {
                    11u8.hash(&mut hasher);
                    for key in keys {
                        key.hash(&mut hasher);
                    }
                    for &desc in descending {
                        desc.hash(&mut hasher);
                    }
                }
                OperatorType::Limit { count } => {
                    12u8.hash(&mut hasher);
                    count.hash(&mut hasher);
                }
                OperatorType::VectorDistance { metric } => {
                    13u8.hash(&mut hasher);
                    metric.hash(&mut hasher);
                }
                OperatorType::Rerank { model } => {
                    14u8.hash(&mut hasher);
                    model.hash(&mut hasher);
                }
                OperatorType::Materialize => 15u8.hash(&mut hasher),
                OperatorType::Result => 16u8.hash(&mut hasher),
                #[allow(deprecated)]
                OperatorType::Scan => 0u8.hash(&mut hasher),
                #[allow(deprecated)]
                OperatorType::Join => 4u8.hash(&mut hasher),
            }
        }

        hasher.finish()
    }

    pub fn pattern_count(&self) -> usize {
        self.reasoning_bank.pattern_count()
    }

    pub fn trajectory_count(&self) -> usize {
        self.trajectory_buffer.total_count()
    }

    pub fn cluster_count(&self) -> usize {
        self.reasoning_bank.cluster_count()
    }
}

impl Default for DagSonaEngine {
    fn default() -> Self {
        Self::new(256)
    }
}
