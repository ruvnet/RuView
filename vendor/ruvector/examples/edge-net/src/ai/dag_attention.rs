//! DAG Attention for Task Orchestration
//!
//! Answers: "What computational steps matter?"
//!
//! Uses topological attention to focus compute on critical path tasks
//! in distributed workflows. Combines:
//! - Topological sort for dependency ordering
//! - Attention scores based on downstream impact
//! - Critical path analysis for priority allocation

use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Deserialize, Serialize};

/// A node in the task DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: String,
    pub cost: f32,           // Estimated compute cost
    pub priority: f32,       // Base priority (0-1)
    pub status: TaskStatus,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Ready,      // All dependencies satisfied
    Running,
    Completed,
    Failed,
}

/// Edge representing dependency between tasks
#[derive(Debug, Clone)]
pub struct TaskEdge {
    pub from: String,  // Dependency (must complete first)
    pub to: String,    // Dependent task
    pub weight: f32,   // Importance of this dependency
}

/// DAG Attention mechanism for task orchestration
#[derive(Debug)]
pub struct DagAttention {
    nodes: HashMap<String, TaskNode>,
    edges: Vec<TaskEdge>,
    adjacency: HashMap<String, Vec<String>>,      // Forward edges: task -> dependents
    reverse_adj: HashMap<String, Vec<String>>,    // Reverse edges: task -> dependencies
    attention_scores: HashMap<String, f32>,
    critical_path: Vec<String>,
    temperature: f32,
}

impl DagAttention {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            adjacency: HashMap::new(),
            reverse_adj: HashMap::new(),
            attention_scores: HashMap::new(),
            critical_path: Vec::new(),
            temperature: 1.0,
        }
    }

    /// Add a task node to the DAG
    pub fn add_task(&mut self, id: &str, cost: f32, priority: f32) {
        let node = TaskNode {
            id: id.to_string(),
            cost,
            priority,
            status: TaskStatus::Pending,
            metadata: HashMap::new(),
        };
        self.nodes.insert(id.to_string(), node);
        self.adjacency.entry(id.to_string()).or_default();
        self.reverse_adj.entry(id.to_string()).or_default();
    }

    /// Add dependency: `from` must complete before `to` can start
    pub fn add_dependency(&mut self, from: &str, to: &str, weight: f32) {
        self.edges.push(TaskEdge {
            from: from.to_string(),
            to: to.to_string(),
            weight,
        });
        self.adjacency.entry(from.to_string()).or_default().push(to.to_string());
        self.reverse_adj.entry(to.to_string()).or_default().push(from.to_string());
    }

    /// Check for cycles (DAG must be acyclic)
    pub fn has_cycle(&self) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node_id in self.nodes.keys() {
            if self.has_cycle_dfs(node_id, &mut visited, &mut rec_stack) {
                return true;
            }
        }
        false
    }

    fn has_cycle_dfs(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> bool {
        if rec_stack.contains(node) {
            return true;
        }
        if visited.contains(node) {
            return false;
        }

        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());

        if let Some(neighbors) = self.adjacency.get(node) {
            for neighbor in neighbors {
                if self.has_cycle_dfs(neighbor, visited, rec_stack) {
                    return true;
                }
            }
        }

        rec_stack.remove(node);
        false
    }

    /// Topological sort using Kahn's algorithm
    pub fn topological_sort(&self) -> Option<Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Initialize in-degrees
        for node_id in self.nodes.keys() {
            in_degree.insert(node_id.clone(), 0);
        }

        // Count incoming edges
        for edge in &self.edges {
            *in_degree.entry(edge.to.clone()).or_default() += 1;
        }

        // Queue nodes with no dependencies
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut sorted = Vec::new();

        while let Some(node) = queue.pop_front() {
            sorted.push(node.clone());

            if let Some(neighbors) = self.adjacency.get(&node) {
                for neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        if sorted.len() == self.nodes.len() {
            Some(sorted)
        } else {
            None // Cycle detected
        }
    }

    /// Compute critical path (longest path through DAG)
    pub fn compute_critical_path(&mut self) -> Vec<String> {
        let topo_order = match self.topological_sort() {
            Some(order) => order,
            None => return Vec::new(),
        };

        // Distance and predecessor for longest path
        let mut dist: HashMap<String, f32> = HashMap::new();
        let mut pred: HashMap<String, Option<String>> = HashMap::new();

        for node_id in &topo_order {
            let node_cost = self.nodes.get(node_id).map(|n| n.cost).unwrap_or(0.0);
            dist.insert(node_id.clone(), node_cost);
            pred.insert(node_id.clone(), None);
        }

        // Relax edges in topological order
        for node_id in &topo_order {
            let current_dist = dist.get(node_id).copied().unwrap_or(0.0);

            if let Some(neighbors) = self.adjacency.get(node_id) {
                for neighbor in neighbors {
                    let neighbor_cost = self.nodes.get(neighbor).map(|n| n.cost).unwrap_or(0.0);
                    let new_dist = current_dist + neighbor_cost;

                    if new_dist > dist.get(neighbor).copied().unwrap_or(0.0) {
                        dist.insert(neighbor.clone(), new_dist);
                        pred.insert(neighbor.clone(), Some(node_id.clone()));
                    }
                }
            }
        }

        // Find the end node with maximum distance
        let end_node = dist.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(id, _)| id.clone());

        // Reconstruct path
        let mut path = Vec::new();
        let mut current = end_node;

        while let Some(node_id) = current {
            path.push(node_id.clone());
            current = pred.get(&node_id).cloned().flatten();
        }

        path.reverse();
        self.critical_path = path.clone();
        path
    }

    /// Compute attention scores for all tasks
    ///
    /// Attention is based on:
    /// 1. Position on critical path (highest attention)
    /// 2. Number of downstream dependents (more = higher)
    /// 3. Task priority
    /// 4. Current status (ready tasks get boost)
    pub fn compute_attention(&mut self) {
        self.compute_critical_path();

        let critical_set: HashSet<_> = self.critical_path.iter().cloned().collect();

        // Compute downstream impact for each node
        let mut downstream_count: HashMap<String, usize> = HashMap::new();

        for node_id in self.nodes.keys() {
            let count = self.count_downstream(node_id);
            downstream_count.insert(node_id.clone(), count);
        }

        let max_downstream = downstream_count.values().max().copied().unwrap_or(1) as f32;

        // Compute attention scores
        for (node_id, node) in &self.nodes {
            let mut score = 0.0;

            // Critical path bonus (0.4 weight)
            if critical_set.contains(node_id) {
                score += 0.4;
            }

            // Downstream impact (0.3 weight)
            let downstream = downstream_count.get(node_id).copied().unwrap_or(0) as f32;
            score += 0.3 * (downstream / max_downstream);

            // Base priority (0.2 weight)
            score += 0.2 * node.priority;

            // Ready status boost (0.1 weight)
            if node.status == TaskStatus::Ready {
                score += 0.1;
            }

            self.attention_scores.insert(node_id.clone(), score);
        }

        // Apply softmax with temperature
        self.apply_softmax();
    }

    fn count_downstream(&self, node_id: &str) -> usize {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(node_id.to_string());

        while let Some(current) = queue.pop_front() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            if let Some(neighbors) = self.adjacency.get(&current) {
                for neighbor in neighbors {
                    queue.push_back(neighbor.clone());
                }
            }
        }

        visited.len().saturating_sub(1) // Exclude self
    }

    fn apply_softmax(&mut self) {
        let max_score = self.attention_scores.values()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .copied()
            .unwrap_or(0.0);

        let exp_sum: f32 = self.attention_scores.values()
            .map(|s| ((s - max_score) / self.temperature).exp())
            .sum();

        for score in self.attention_scores.values_mut() {
            *score = ((*score - max_score) / self.temperature).exp() / exp_sum;
        }
    }

    /// Get tasks sorted by attention (highest first)
    pub fn get_prioritized_tasks(&self) -> Vec<(String, f32)> {
        let mut tasks: Vec<_> = self.attention_scores.iter()
            .map(|(id, score)| (id.clone(), *score))
            .collect();

        tasks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        tasks
    }

    /// Get ready tasks (all dependencies satisfied) sorted by attention
    pub fn get_ready_tasks(&self) -> Vec<(String, f32)> {
        self.get_prioritized_tasks()
            .into_iter()
            .filter(|(id, _)| {
                self.nodes.get(id)
                    .map(|n| n.status == TaskStatus::Ready || n.status == TaskStatus::Pending)
                    .unwrap_or(false)
                    && self.all_deps_completed(id)
            })
            .collect()
    }

    fn all_deps_completed(&self, task_id: &str) -> bool {
        self.reverse_adj.get(task_id)
            .map(|deps| {
                deps.iter().all(|dep| {
                    self.nodes.get(dep)
                        .map(|n| n.status == TaskStatus::Completed)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(true)
    }

    /// Mark task as completed and update attention
    pub fn complete_task(&mut self, task_id: &str) {
        if let Some(node) = self.nodes.get_mut(task_id) {
            node.status = TaskStatus::Completed;
        }

        // Update status of dependent tasks
        if let Some(dependents) = self.adjacency.get(task_id).cloned() {
            for dep_id in dependents {
                if self.all_deps_completed(&dep_id) {
                    if let Some(node) = self.nodes.get_mut(&dep_id) {
                        if node.status == TaskStatus::Pending {
                            node.status = TaskStatus::Ready;
                        }
                    }
                }
            }
        }

        // Recompute attention
        self.compute_attention();
    }

    /// Get attention score for a specific task
    pub fn get_attention(&self, task_id: &str) -> f32 {
        self.attention_scores.get(task_id).copied().unwrap_or(0.0)
    }

    /// Get the critical path
    pub fn get_critical_path(&self) -> &[String] {
        &self.critical_path
    }

    /// Set temperature for softmax (higher = more uniform attention)
    pub fn set_temperature(&mut self, temp: f32) {
        self.temperature = temp.max(0.01);
    }

    /// Get total estimated time (critical path length)
    pub fn estimated_total_time(&self) -> f32 {
        self.critical_path.iter()
            .filter_map(|id| self.nodes.get(id))
            .map(|n| n.cost)
            .sum()
    }

    /// Get summary statistics
    pub fn summary(&self) -> DagSummary {
        let completed = self.nodes.values()
            .filter(|n| n.status == TaskStatus::Completed)
            .count();

        DagSummary {
            total_tasks: self.nodes.len(),
            completed_tasks: completed,
            critical_path_length: self.critical_path.len(),
            estimated_total_time: self.estimated_total_time(),
            max_parallelism: self.compute_max_parallelism(),
        }
    }

    fn compute_max_parallelism(&self) -> usize {
        // Compute level-based parallelism
        let topo = match self.topological_sort() {
            Some(t) => t,
            None => return 0,
        };

        let mut levels: HashMap<String, usize> = HashMap::new();

        for node_id in &topo {
            let deps = self.reverse_adj.get(node_id);
            let level = deps
                .map(|d| d.iter().filter_map(|dep| levels.get(dep)).max().copied().unwrap_or(0) + 1)
                .unwrap_or(0);
            levels.insert(node_id.clone(), level);
        }

        // Count nodes per level
        let mut level_counts: HashMap<usize, usize> = HashMap::new();
        for level in levels.values() {
            *level_counts.entry(*level).or_default() += 1;
        }

        level_counts.values().max().copied().unwrap_or(0)
    }
}

impl Default for DagAttention {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagSummary {
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub critical_path_length: usize,
    pub estimated_total_time: f32,
    pub max_parallelism: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_attention_basic() {
        let mut dag = DagAttention::new();

        // Create a simple diamond DAG:
        //     A
        //    / \
        //   B   C
        //    \ /
        //     D

        dag.add_task("A", 1.0, 0.5);
        dag.add_task("B", 2.0, 0.5);
        dag.add_task("C", 3.0, 0.5);
        dag.add_task("D", 1.0, 0.5);

        dag.add_dependency("A", "B", 1.0);
        dag.add_dependency("A", "C", 1.0);
        dag.add_dependency("B", "D", 1.0);
        dag.add_dependency("C", "D", 1.0);

        assert!(!dag.has_cycle());

        let topo = dag.topological_sort().unwrap();
        assert_eq!(topo[0], "A");
        assert_eq!(topo[3], "D");
    }

    #[test]
    fn test_critical_path() {
        let mut dag = DagAttention::new();

        dag.add_task("A", 1.0, 0.5);
        dag.add_task("B", 5.0, 0.5);  // Longer path through B
        dag.add_task("C", 1.0, 0.5);
        dag.add_task("D", 1.0, 0.5);

        dag.add_dependency("A", "B", 1.0);
        dag.add_dependency("A", "C", 1.0);
        dag.add_dependency("B", "D", 1.0);
        dag.add_dependency("C", "D", 1.0);

        let critical = dag.compute_critical_path();

        // Critical path should be A -> B -> D (cost 7)
        assert!(critical.contains(&"B".to_string()));
    }

    #[test]
    fn test_attention_scores() {
        let mut dag = DagAttention::new();

        dag.add_task("root", 1.0, 0.9);
        dag.add_task("leaf1", 1.0, 0.1);
        dag.add_task("leaf2", 1.0, 0.1);

        dag.add_dependency("root", "leaf1", 1.0);
        dag.add_dependency("root", "leaf2", 1.0);

        dag.compute_attention();

        // Root should have higher attention (more downstream impact)
        assert!(dag.get_attention("root") > dag.get_attention("leaf1"));
    }

    #[test]
    fn test_cycle_detection() {
        let mut dag = DagAttention::new();

        dag.add_task("A", 1.0, 0.5);
        dag.add_task("B", 1.0, 0.5);
        dag.add_task("C", 1.0, 0.5);

        dag.add_dependency("A", "B", 1.0);
        dag.add_dependency("B", "C", 1.0);
        dag.add_dependency("C", "A", 1.0); // Creates cycle

        assert!(dag.has_cycle());
        assert!(dag.topological_sort().is_none());
    }

    #[test]
    fn test_task_completion() {
        let mut dag = DagAttention::new();

        dag.add_task("A", 1.0, 0.5);
        dag.add_task("B", 1.0, 0.5);

        dag.add_dependency("A", "B", 1.0);
        dag.compute_attention();

        // B should not be ready yet
        let ready = dag.get_ready_tasks();
        assert!(ready.iter().any(|(id, _)| id == "A"));
        assert!(!ready.iter().any(|(id, _)| id == "B"));

        // Complete A
        dag.complete_task("A");

        // Now B should be ready
        let ready = dag.get_ready_tasks();
        assert!(ready.iter().any(|(id, _)| id == "B"));
    }
}
