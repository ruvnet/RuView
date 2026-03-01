//! Parallel Branch Attention: Coordinates attention across parallel execution branches
//!
//! This mechanism identifies parallel branches in the DAG and distributes attention
//! to balance workload and minimize synchronization overhead.

use super::trait_def::{AttentionError, AttentionScores, DagAttentionMechanism};
use crate::dag::QueryDag;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ParallelBranchConfig {
    /// Maximum number of parallel branches to consider
    pub max_branches: usize,
    /// Penalty for synchronization between branches
    pub sync_penalty: f32,
    /// Weight for branch balance in attention computation
    pub balance_weight: f32,
    /// Temperature for softmax
    pub temperature: f32,
}

impl Default for ParallelBranchConfig {
    fn default() -> Self {
        Self {
            max_branches: 8,
            sync_penalty: 0.2,
            balance_weight: 0.5,
            temperature: 0.1,
        }
    }
}

pub struct ParallelBranchAttention {
    config: ParallelBranchConfig,
}

impl ParallelBranchAttention {
    pub fn new(config: ParallelBranchConfig) -> Self {
        Self { config }
    }

    /// Detect parallel branches (nodes with same parent, no edges between them)
    fn detect_branches(&self, dag: &QueryDag) -> Vec<Vec<usize>> {
        let n = dag.node_count();
        let mut children_of: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut parents_of: HashMap<usize, Vec<usize>> = HashMap::new();

        // Build parent-child relationships from adjacency
        for node_id in dag.node_ids() {
            let children = dag.children(node_id);
            if !children.is_empty() {
                for &child in children {
                    children_of
                        .entry(node_id)
                        .or_insert_with(Vec::new)
                        .push(child);
                    parents_of
                        .entry(child)
                        .or_insert_with(Vec::new)
                        .push(node_id);
                }
            }
        }

        let mut branches = Vec::new();
        let mut visited = HashSet::new();

        // For each node, check if its children form parallel branches
        for node_id in 0..n {
            if let Some(children) = children_of.get(&node_id) {
                if children.len() > 1 {
                    // Check if children are truly parallel (no edges between them)
                    let mut parallel_group = Vec::new();

                    for &child in children {
                        if !visited.contains(&child) {
                            // Check if this child has edges to any siblings
                            let child_children = dag.children(child);
                            let has_sibling_edge = children
                                .iter()
                                .any(|&other| other != child && child_children.contains(&other));

                            if !has_sibling_edge {
                                parallel_group.push(child);
                                visited.insert(child);
                            }
                        }
                    }

                    if parallel_group.len() > 1 {
                        branches.push(parallel_group);
                    }
                }
            }
        }

        branches
    }

    /// Compute branch balance score (lower is better balanced)
    fn branch_balance(&self, branches: &[Vec<usize>], dag: &QueryDag) -> f32 {
        if branches.is_empty() {
            return 1.0;
        }

        let mut total_variance = 0.0;

        for branch in branches {
            if branch.len() <= 1 {
                continue;
            }

            // Compute costs for each node in the branch
            let costs: Vec<f64> = branch
                .iter()
                .filter_map(|&id| dag.get_node(id).map(|n| n.estimated_cost))
                .collect();

            if costs.is_empty() {
                continue;
            }

            // Compute variance
            let mean = costs.iter().sum::<f64>() / costs.len() as f64;
            let variance =
                costs.iter().map(|&c| (c - mean).powi(2)).sum::<f64>() / costs.len() as f64;

            total_variance += variance as f32;
        }

        // Normalize by number of branches
        if branches.is_empty() {
            1.0
        } else {
            (total_variance / branches.len() as f32).sqrt()
        }
    }

    /// Compute criticality score for a branch
    fn branch_criticality(&self, branch: &[usize], dag: &QueryDag) -> f32 {
        if branch.is_empty() {
            return 0.0;
        }

        // Sum of costs in the branch
        let total_cost: f64 = branch
            .iter()
            .filter_map(|&id| dag.get_node(id).map(|n| n.estimated_cost))
            .sum();

        // Average rows (higher rows = more critical for filtering)
        let avg_rows: f64 = branch
            .iter()
            .filter_map(|&id| dag.get_node(id).map(|n| n.estimated_rows))
            .sum::<f64>()
            / branch.len().max(1) as f64;

        // Criticality is high cost + high row count
        (total_cost * (avg_rows / 1000.0).min(1.0)) as f32
    }

    /// Compute attention scores based on parallel branch analysis
    fn compute_branch_attention(&self, dag: &QueryDag, branches: &[Vec<usize>]) -> Vec<f32> {
        let n = dag.node_count();
        let mut scores = vec![0.0; n];

        // Base score for nodes not in any branch
        let base_score = 0.5;
        for i in 0..n {
            scores[i] = base_score;
        }

        // Compute balance metric
        let balance_penalty = self.branch_balance(branches, dag);

        // Assign scores based on branch criticality
        for branch in branches {
            let criticality = self.branch_criticality(branch, dag);

            // Higher criticality = higher attention
            // Apply balance penalty
            let branch_score = criticality * (1.0 - self.config.balance_weight * balance_penalty);

            for &node_id in branch {
                if node_id < n {
                    scores[node_id] = branch_score;
                }
            }
        }

        // Apply sync penalty to nodes that synchronize branches
        for from in dag.node_ids() {
            for &to in dag.children(from) {
                if from < n && to < n {
                    // Check if this edge connects different branches
                    let from_branch = branches.iter().position(|b| b.iter().any(|&x| x == from));
                    let to_branch = branches.iter().position(|b| b.iter().any(|&x| x == to));

                    if from_branch.is_some() && to_branch.is_some() && from_branch != to_branch {
                        scores[to] *= 1.0 - self.config.sync_penalty;
                    }
                }
            }
        }

        // Normalize using softmax
        let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = scores
            .iter()
            .map(|&s| ((s - max_score) / self.config.temperature).exp())
            .sum();

        if exp_sum > 0.0 {
            for score in scores.iter_mut() {
                *score = ((*score - max_score) / self.config.temperature).exp() / exp_sum;
            }
        } else {
            // Uniform if all scores are too low
            let uniform = 1.0 / n as f32;
            scores.fill(uniform);
        }

        scores
    }
}

impl DagAttentionMechanism for ParallelBranchAttention {
    fn forward(&self, dag: &QueryDag) -> Result<AttentionScores, AttentionError> {
        if dag.node_count() == 0 {
            return Err(AttentionError::InvalidDag("Empty DAG".to_string()));
        }

        // Step 1: Detect parallel branches
        let branches = self.detect_branches(dag);

        // Step 2: Compute attention based on branches
        let scores = self.compute_branch_attention(dag, &branches);

        // Step 3: Build result
        let mut result = AttentionScores::new(scores)
            .with_metadata("mechanism".to_string(), "parallel_branch".to_string())
            .with_metadata("num_branches".to_string(), branches.len().to_string());

        let balance = self.branch_balance(&branches, dag);
        result
            .metadata
            .insert("balance_score".to_string(), format!("{:.4}", balance));

        Ok(result)
    }

    fn name(&self) -> &'static str {
        "parallel_branch"
    }

    fn complexity(&self) -> &'static str {
        "O(n² + b·n)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{OperatorNode, OperatorType};

    #[test]
    fn test_detect_branches() {
        let config = ParallelBranchConfig::default();
        let attention = ParallelBranchAttention::new(config);

        let mut dag = QueryDag::new();
        for i in 0..4 {
            dag.add_node(OperatorNode::new(i, OperatorType::Scan));
        }

        // Create parallel branches: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        dag.add_edge(0, 1).unwrap();
        dag.add_edge(0, 2).unwrap();
        dag.add_edge(1, 3).unwrap();
        dag.add_edge(2, 3).unwrap();

        let branches = attention.detect_branches(&dag);
        assert!(!branches.is_empty());
    }

    #[test]
    fn test_parallel_attention() {
        let config = ParallelBranchConfig::default();
        let attention = ParallelBranchAttention::new(config);

        let mut dag = QueryDag::new();
        for i in 0..3 {
            let mut node = OperatorNode::new(i, OperatorType::Scan);
            node.estimated_cost = (i + 1) as f64;
            dag.add_node(node);
        }
        dag.add_edge(0, 1).unwrap();
        dag.add_edge(0, 2).unwrap();

        let result = attention.forward(&dag).unwrap();
        assert_eq!(result.scores.len(), 3);
    }
}
