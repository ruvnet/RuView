//! Bottleneck Detection

use crate::dag::{OperatorType, QueryDag};
use std::collections::HashMap;

/// A detected bottleneck in the DAG
#[derive(Debug, Clone)]
pub struct Bottleneck {
    pub node_id: usize,
    pub score: f64,
    pub impact_estimate: f64,
    pub suggested_action: String,
}

/// Analysis of bottlenecks in a DAG
#[derive(Debug)]
pub struct BottleneckAnalysis {
    pub bottlenecks: Vec<Bottleneck>,
    pub total_cost: f64,
    pub critical_path_cost: f64,
    pub parallelization_potential: f64,
}

impl BottleneckAnalysis {
    pub fn analyze(dag: &QueryDag, criticality: &HashMap<usize, f64>) -> Self {
        let mut bottlenecks = Vec::new();

        for (&node_id, &score) in criticality {
            if score > 0.5 {
                // Threshold for bottleneck
                let node = dag.get_node(node_id).unwrap();
                let action = Self::suggest_action(&node.op_type);

                bottlenecks.push(Bottleneck {
                    node_id,
                    score,
                    impact_estimate: node.estimated_cost * score,
                    suggested_action: action,
                });
            }
        }

        // Sort by score descending
        bottlenecks.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Calculate total cost by iterating over all node IDs
        let total_cost: f64 = (0..dag.node_count())
            .filter_map(|id| dag.get_node(id))
            .map(|n| n.estimated_cost)
            .sum();

        let critical_path_cost = Self::compute_critical_path_cost(dag);
        let parallelization_potential = 1.0 - (critical_path_cost / total_cost.max(1.0));

        Self {
            bottlenecks,
            total_cost,
            critical_path_cost,
            parallelization_potential,
        }
    }

    fn suggest_action(op_type: &OperatorType) -> String {
        match op_type {
            OperatorType::SeqScan { table } => {
                format!("Consider adding index on {}", table)
            }
            OperatorType::NestedLoopJoin => "Consider using hash join instead".to_string(),
            OperatorType::Sort { .. } => "Consider adding sorted index".to_string(),
            OperatorType::HnswScan { .. } => "Consider increasing ef_search parameter".to_string(),
            _ => "Review operator parameters".to_string(),
        }
    }

    fn compute_critical_path_cost(dag: &QueryDag) -> f64 {
        // Longest path by cost
        let mut max_cost: HashMap<usize, f64> = HashMap::new();

        // Get topological sort, return 0 if there's a cycle
        let sorted = match dag.topological_sort() {
            Ok(s) => s,
            Err(_) => return 0.0,
        };

        for node_id in sorted {
            let node = dag.get_node(node_id).unwrap();
            let parent_max = dag
                .parents(node_id)
                .iter()
                .filter_map(|&p| max_cost.get(&p))
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .copied()
                .unwrap_or(0.0);

            max_cost.insert(node_id, parent_max + node.estimated_cost);
        }

        max_cost
            .values()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .copied()
            .unwrap_or(0.0)
    }
}
