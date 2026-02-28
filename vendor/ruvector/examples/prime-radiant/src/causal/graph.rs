//! Directed Acyclic Graph (DAG) implementation for causal models
//!
//! This module provides a validated DAG structure that ensures:
//! - No cycles (acyclicity constraint)
//! - Efficient topological ordering
//! - Parent/child relationship queries
//! - D-separation computations

use std::collections::{HashMap, HashSet, VecDeque};
use thiserror::Error;

/// Error types for DAG operations
#[derive(Debug, Clone, Error)]
pub enum DAGValidationError {
    /// Cycle detected in graph
    #[error("Cycle detected involving nodes: {0:?}")]
    CycleDetected(Vec<u32>),

    /// Node not found
    #[error("Node {0} not found in graph")]
    NodeNotFound(u32),

    /// Edge already exists
    #[error("Edge from {0} to {1} already exists")]
    EdgeExists(u32, u32),

    /// Self-loop detected
    #[error("Self-loop detected at node {0}")]
    SelfLoop(u32),

    /// Invalid operation on empty graph
    #[error("Graph is empty")]
    EmptyGraph,
}

/// A directed acyclic graph for causal relationships
#[derive(Debug, Clone)]
pub struct DirectedGraph {
    /// Number of nodes
    num_nodes: usize,

    /// Adjacency list: node -> children
    children: HashMap<u32, HashSet<u32>>,

    /// Reverse adjacency: node -> parents
    parents: HashMap<u32, HashSet<u32>>,

    /// Node labels (optional)
    labels: HashMap<u32, String>,

    /// Cached topological order (invalidated on structural changes)
    cached_topo_order: Option<Vec<u32>>,
}

impl DirectedGraph {
    /// Create a new empty directed graph
    pub fn new() -> Self {
        Self {
            num_nodes: 0,
            children: HashMap::new(),
            parents: HashMap::new(),
            labels: HashMap::new(),
            cached_topo_order: None,
        }
    }

    /// Create a graph with pre-allocated capacity
    pub fn with_capacity(nodes: usize) -> Self {
        Self {
            num_nodes: 0,
            children: HashMap::with_capacity(nodes),
            parents: HashMap::with_capacity(nodes),
            labels: HashMap::with_capacity(nodes),
            cached_topo_order: None,
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, id: u32) -> u32 {
        if !self.children.contains_key(&id) {
            self.children.insert(id, HashSet::new());
            self.parents.insert(id, HashSet::new());
            self.num_nodes += 1;
            self.cached_topo_order = None;
        }
        id
    }

    /// Add a node with a label
    pub fn add_node_with_label(&mut self, id: u32, label: &str) -> u32 {
        self.add_node(id);
        self.labels.insert(id, label.to_string());
        id
    }

    /// Add a directed edge from `from` to `to`
    ///
    /// Returns error if edge would create a cycle
    pub fn add_edge(&mut self, from: u32, to: u32) -> Result<(), DAGValidationError> {
        // Check for self-loop
        if from == to {
            return Err(DAGValidationError::SelfLoop(from));
        }

        // Ensure nodes exist
        self.add_node(from);
        self.add_node(to);

        // Check if edge already exists
        if self.children.get(&from).map(|c| c.contains(&to)).unwrap_or(false) {
            return Err(DAGValidationError::EdgeExists(from, to));
        }

        // Temporarily add edge and check for cycles
        self.children.get_mut(&from).unwrap().insert(to);
        self.parents.get_mut(&to).unwrap().insert(from);

        if self.has_cycle() {
            // Remove edge if cycle detected
            self.children.get_mut(&from).unwrap().remove(&to);
            self.parents.get_mut(&to).unwrap().remove(&from);
            return Err(DAGValidationError::CycleDetected(self.find_cycle_nodes(from, to)));
        }

        self.cached_topo_order = None;
        Ok(())
    }

    /// Remove an edge from the graph
    pub fn remove_edge(&mut self, from: u32, to: u32) -> Result<(), DAGValidationError> {
        if !self.children.contains_key(&from) {
            return Err(DAGValidationError::NodeNotFound(from));
        }
        if !self.children.contains_key(&to) {
            return Err(DAGValidationError::NodeNotFound(to));
        }

        self.children.get_mut(&from).unwrap().remove(&to);
        self.parents.get_mut(&to).unwrap().remove(&from);
        self.cached_topo_order = None;

        Ok(())
    }

    /// Check if the graph has a cycle (using DFS)
    fn has_cycle(&self) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for &node in self.children.keys() {
            if self.has_cycle_util(node, &mut visited, &mut rec_stack) {
                return true;
            }
        }
        false
    }

    fn has_cycle_util(
        &self,
        node: u32,
        visited: &mut HashSet<u32>,
        rec_stack: &mut HashSet<u32>,
    ) -> bool {
        if rec_stack.contains(&node) {
            return true;
        }
        if visited.contains(&node) {
            return false;
        }

        visited.insert(node);
        rec_stack.insert(node);

        if let Some(children) = self.children.get(&node) {
            for &child in children {
                if self.has_cycle_util(child, visited, rec_stack) {
                    return true;
                }
            }
        }

        rec_stack.remove(&node);
        false
    }

    /// Find nodes involved in a potential cycle
    fn find_cycle_nodes(&self, from: u32, to: u32) -> Vec<u32> {
        // Find path from `to` back to `from`
        let mut path = Vec::new();
        let mut visited = HashSet::new();

        fn dfs(
            graph: &DirectedGraph,
            current: u32,
            target: u32,
            visited: &mut HashSet<u32>,
            path: &mut Vec<u32>,
        ) -> bool {
            if current == target {
                path.push(current);
                return true;
            }
            if visited.contains(&current) {
                return false;
            }
            visited.insert(current);
            path.push(current);

            if let Some(children) = graph.children.get(&current) {
                for &child in children {
                    if dfs(graph, child, target, visited, path) {
                        return true;
                    }
                }
            }

            path.pop();
            false
        }

        if dfs(self, to, from, &mut visited, &mut path) {
            path.push(to);
        }

        path
    }

    /// Get children of a node
    pub fn children_of(&self, node: u32) -> Option<&HashSet<u32>> {
        self.children.get(&node)
    }

    /// Get parents of a node
    pub fn parents_of(&self, node: u32) -> Option<&HashSet<u32>> {
        self.parents.get(&node)
    }

    /// Check if node exists
    pub fn contains_node(&self, node: u32) -> bool {
        self.children.contains_key(&node)
    }

    /// Check if edge exists
    pub fn contains_edge(&self, from: u32, to: u32) -> bool {
        self.children.get(&from).map(|c| c.contains(&to)).unwrap_or(false)
    }

    /// Get number of nodes
    pub fn node_count(&self) -> usize {
        self.num_nodes
    }

    /// Get number of edges
    pub fn edge_count(&self) -> usize {
        self.children.values().map(|c| c.len()).sum()
    }

    /// Get all nodes
    pub fn nodes(&self) -> impl Iterator<Item = u32> + '_ {
        self.children.keys().copied()
    }

    /// Get all edges as (from, to) pairs
    pub fn edges(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.children.iter().flat_map(|(&from, children)| {
            children.iter().map(move |&to| (from, to))
        })
    }

    /// Get node label
    pub fn get_label(&self, node: u32) -> Option<&str> {
        self.labels.get(&node).map(|s| s.as_str())
    }

    /// Find node by label
    pub fn find_node_by_label(&self, label: &str) -> Option<u32> {
        self.labels.iter()
            .find(|(_, l)| l.as_str() == label)
            .map(|(&id, _)| id)
    }

    /// Compute topological ordering using Kahn's algorithm
    pub fn topological_order(&mut self) -> Result<TopologicalOrder, DAGValidationError> {
        if self.num_nodes == 0 {
            return Err(DAGValidationError::EmptyGraph);
        }

        // Use cached order if available
        if let Some(ref order) = self.cached_topo_order {
            return Ok(TopologicalOrder { order: order.clone() });
        }

        // Compute in-degrees
        let mut in_degree: HashMap<u32, usize> = HashMap::new();
        for &node in self.children.keys() {
            in_degree.insert(node, 0);
        }
        for children in self.children.values() {
            for &child in children {
                *in_degree.entry(child).or_insert(0) += 1;
            }
        }

        // Initialize queue with nodes having in-degree 0
        let mut queue: VecDeque<u32> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(&node, _)| node)
            .collect();

        let mut order = Vec::with_capacity(self.num_nodes);

        while let Some(node) = queue.pop_front() {
            order.push(node);

            if let Some(children) = self.children.get(&node) {
                for &child in children {
                    if let Some(deg) = in_degree.get_mut(&child) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(child);
                        }
                    }
                }
            }
        }

        if order.len() != self.num_nodes {
            return Err(DAGValidationError::CycleDetected(
                in_degree.iter()
                    .filter(|&(_, &deg)| deg > 0)
                    .map(|(&node, _)| node)
                    .collect()
            ));
        }

        self.cached_topo_order = Some(order.clone());
        Ok(TopologicalOrder { order })
    }

    /// Get ancestors of a node (all nodes that can reach it)
    pub fn ancestors(&self, node: u32) -> HashSet<u32> {
        let mut ancestors = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(parents) = self.parents.get(&node) {
            for &parent in parents {
                queue.push_back(parent);
            }
        }

        while let Some(current) = queue.pop_front() {
            if ancestors.insert(current) {
                if let Some(parents) = self.parents.get(&current) {
                    for &parent in parents {
                        if !ancestors.contains(&parent) {
                            queue.push_back(parent);
                        }
                    }
                }
            }
        }

        ancestors
    }

    /// Get descendants of a node (all nodes reachable from it)
    pub fn descendants(&self, node: u32) -> HashSet<u32> {
        let mut descendants = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(children) = self.children.get(&node) {
            for &child in children {
                queue.push_back(child);
            }
        }

        while let Some(current) = queue.pop_front() {
            if descendants.insert(current) {
                if let Some(children) = self.children.get(&current) {
                    for &child in children {
                        if !descendants.contains(&child) {
                            queue.push_back(child);
                        }
                    }
                }
            }
        }

        descendants
    }

    /// Check d-separation between X and Y given conditioning set Z
    ///
    /// Two sets X and Y are d-separated by Z if all paths between X and Y
    /// are blocked by Z.
    pub fn d_separated(
        &self,
        x: &HashSet<u32>,
        y: &HashSet<u32>,
        z: &HashSet<u32>,
    ) -> bool {
        // Use Bayes Ball algorithm for d-separation
        let reachable = self.bayes_ball_reachable(x, z);

        // X and Y are d-separated if no node in Y is reachable
        reachable.intersection(y).next().is_none()
    }

    /// Bayes Ball algorithm to find reachable nodes
    ///
    /// Returns the set of nodes reachable from `source` given evidence `evidence`
    fn bayes_ball_reachable(&self, source: &HashSet<u32>, evidence: &HashSet<u32>) -> HashSet<u32> {
        let mut visited_up: HashSet<u32> = HashSet::new();
        let mut visited_down: HashSet<u32> = HashSet::new();
        let mut reachable: HashSet<u32> = HashSet::new();

        // Queue entries: (node, direction_is_up)
        let mut queue: VecDeque<(u32, bool)> = VecDeque::new();

        // Initialize with source nodes going up (as if we observed them)
        for &node in source {
            queue.push_back((node, true));  // Going up from source
            queue.push_back((node, false)); // Going down from source
        }

        while let Some((node, going_up)) = queue.pop_front() {
            // Skip if already visited in this direction
            if going_up && visited_up.contains(&node) {
                continue;
            }
            if !going_up && visited_down.contains(&node) {
                continue;
            }

            if going_up {
                visited_up.insert(node);
            } else {
                visited_down.insert(node);
            }

            let is_evidence = evidence.contains(&node);

            if going_up && !is_evidence {
                // Ball going up, node not observed: continue to parents and children
                reachable.insert(node);

                if let Some(parents) = self.parents.get(&node) {
                    for &parent in parents {
                        queue.push_back((parent, true));
                    }
                }
                if let Some(children) = self.children.get(&node) {
                    for &child in children {
                        queue.push_back((child, false));
                    }
                }
            } else if going_up && is_evidence {
                // Ball going up, node observed: continue to parents only
                if let Some(parents) = self.parents.get(&node) {
                    for &parent in parents {
                        queue.push_back((parent, true));
                    }
                }
            } else if !going_up && !is_evidence {
                // Ball going down, node not observed: continue to children only
                reachable.insert(node);

                if let Some(children) = self.children.get(&node) {
                    for &child in children {
                        queue.push_back((child, false));
                    }
                }
            } else {
                // Ball going down, node observed: bounce back up to parents
                reachable.insert(node);

                if let Some(parents) = self.parents.get(&node) {
                    for &parent in parents {
                        queue.push_back((parent, true));
                    }
                }
            }
        }

        reachable
    }

    /// Find all paths between two nodes
    pub fn find_all_paths(&self, from: u32, to: u32, max_length: usize) -> Vec<Vec<u32>> {
        let mut all_paths = Vec::new();
        let mut current_path = vec![from];

        self.find_paths_dfs(from, to, &mut current_path, &mut all_paths, max_length);

        all_paths
    }

    fn find_paths_dfs(
        &self,
        current: u32,
        target: u32,
        path: &mut Vec<u32>,
        all_paths: &mut Vec<Vec<u32>>,
        max_length: usize,
    ) {
        if current == target {
            all_paths.push(path.clone());
            return;
        }

        if path.len() >= max_length {
            return;
        }

        if let Some(children) = self.children.get(&current) {
            for &child in children {
                if !path.contains(&child) {
                    path.push(child);
                    self.find_paths_dfs(child, target, path, all_paths, max_length);
                    path.pop();
                }
            }
        }
    }

    /// Get the skeleton (undirected version) of the graph
    pub fn skeleton(&self) -> HashSet<(u32, u32)> {
        let mut skeleton = HashSet::new();

        for (&from, children) in &self.children {
            for &to in children {
                let edge = if from < to { (from, to) } else { (to, from) };
                skeleton.insert(edge);
            }
        }

        skeleton
    }

    /// Find all v-structures (colliders) in the graph
    ///
    /// A v-structure is a triple (A, B, C) where A -> B <- C and A and C are not adjacent
    pub fn v_structures(&self) -> Vec<(u32, u32, u32)> {
        let mut v_structs = Vec::new();
        let skeleton = self.skeleton();

        for (&node, parents) in &self.parents {
            if parents.len() < 2 {
                continue;
            }

            let parents_vec: Vec<_> = parents.iter().copied().collect();

            for i in 0..parents_vec.len() {
                for j in (i + 1)..parents_vec.len() {
                    let p1 = parents_vec[i];
                    let p2 = parents_vec[j];

                    // Check if parents are not adjacent
                    let edge = if p1 < p2 { (p1, p2) } else { (p2, p1) };
                    if !skeleton.contains(&edge) {
                        v_structs.push((p1, node, p2));
                    }
                }
            }
        }

        v_structs
    }
}

impl Default for DirectedGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Topological ordering of nodes in a DAG
#[derive(Debug, Clone)]
pub struct TopologicalOrder {
    order: Vec<u32>,
}

impl TopologicalOrder {
    /// Get the ordering as a slice
    pub fn as_slice(&self) -> &[u32] {
        &self.order
    }

    /// Get the position of a node in the ordering
    pub fn position(&self, node: u32) -> Option<usize> {
        self.order.iter().position(|&n| n == node)
    }

    /// Check if node A comes before node B in the ordering
    pub fn comes_before(&self, a: u32, b: u32) -> bool {
        match (self.position(a), self.position(b)) {
            (Some(pos_a), Some(pos_b)) => pos_a < pos_b,
            _ => false,
        }
    }

    /// Iterate over nodes in topological order
    pub fn iter(&self) -> impl Iterator<Item = &u32> {
        self.order.iter()
    }

    /// Get number of nodes
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Check if ordering is empty
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

impl IntoIterator for TopologicalOrder {
    type Item = u32;
    type IntoIter = std::vec::IntoIter<u32>;

    fn into_iter(self) -> Self::IntoIter {
        self.order.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_nodes_and_edges() {
        let mut graph = DirectedGraph::new();
        graph.add_node(0);
        graph.add_node(1);
        graph.add_edge(0, 1).unwrap();

        assert!(graph.contains_node(0));
        assert!(graph.contains_node(1));
        assert!(graph.contains_edge(0, 1));
        assert!(!graph.contains_edge(1, 0));
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = DirectedGraph::new();
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(1, 2).unwrap();

        // This should fail - would create cycle
        let result = graph.add_edge(2, 0);
        assert!(matches!(result, Err(DAGValidationError::CycleDetected(_))));
    }

    #[test]
    fn test_self_loop_detection() {
        let mut graph = DirectedGraph::new();
        let result = graph.add_edge(0, 0);
        assert!(matches!(result, Err(DAGValidationError::SelfLoop(0))));
    }

    #[test]
    fn test_topological_order() {
        let mut graph = DirectedGraph::new();
        // Diamond: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(0, 2).unwrap();
        graph.add_edge(1, 3).unwrap();
        graph.add_edge(2, 3).unwrap();

        let order = graph.topological_order().unwrap();

        assert_eq!(order.len(), 4);
        assert!(order.comes_before(0, 1));
        assert!(order.comes_before(0, 2));
        assert!(order.comes_before(1, 3));
        assert!(order.comes_before(2, 3));
    }

    #[test]
    fn test_ancestors_and_descendants() {
        let mut graph = DirectedGraph::new();
        // Chain: 0 -> 1 -> 2 -> 3
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(1, 2).unwrap();
        graph.add_edge(2, 3).unwrap();

        let ancestors = graph.ancestors(3);
        assert!(ancestors.contains(&0));
        assert!(ancestors.contains(&1));
        assert!(ancestors.contains(&2));
        assert!(!ancestors.contains(&3));

        let descendants = graph.descendants(0);
        assert!(descendants.contains(&1));
        assert!(descendants.contains(&2));
        assert!(descendants.contains(&3));
        assert!(!descendants.contains(&0));
    }

    #[test]
    fn test_d_separation_chain() {
        // Chain: X -> Z -> Y
        // X and Y should be d-separated given Z
        let mut graph = DirectedGraph::new();
        graph.add_node_with_label(0, "X");
        graph.add_node_with_label(1, "Z");
        graph.add_node_with_label(2, "Y");
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(1, 2).unwrap();

        let x: HashSet<u32> = [0].into_iter().collect();
        let y: HashSet<u32> = [2].into_iter().collect();
        let z: HashSet<u32> = [1].into_iter().collect();
        let empty: HashSet<u32> = HashSet::new();

        // X and Y are NOT d-separated given empty set
        assert!(!graph.d_separated(&x, &y, &empty));

        // X and Y ARE d-separated given Z
        assert!(graph.d_separated(&x, &y, &z));
    }

    #[test]
    fn test_d_separation_fork() {
        // Fork: X <- Z -> Y
        // X and Y should be d-separated given Z
        let mut graph = DirectedGraph::new();
        graph.add_node_with_label(0, "X");
        graph.add_node_with_label(1, "Z");
        graph.add_node_with_label(2, "Y");
        graph.add_edge(1, 0).unwrap();
        graph.add_edge(1, 2).unwrap();

        let x: HashSet<u32> = [0].into_iter().collect();
        let y: HashSet<u32> = [2].into_iter().collect();
        let z: HashSet<u32> = [1].into_iter().collect();
        let empty: HashSet<u32> = HashSet::new();

        // X and Y are NOT d-separated given empty set
        assert!(!graph.d_separated(&x, &y, &empty));

        // X and Y ARE d-separated given Z
        assert!(graph.d_separated(&x, &y, &z));
    }

    #[test]
    fn test_d_separation_collider() {
        // Collider: X -> Z <- Y
        // X and Y should NOT be d-separated given Z (explaining away)
        let mut graph = DirectedGraph::new();
        graph.add_node_with_label(0, "X");
        graph.add_node_with_label(1, "Z");
        graph.add_node_with_label(2, "Y");
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(2, 1).unwrap();

        let x: HashSet<u32> = [0].into_iter().collect();
        let y: HashSet<u32> = [2].into_iter().collect();
        let z: HashSet<u32> = [1].into_iter().collect();
        let empty: HashSet<u32> = HashSet::new();

        // X and Y ARE d-separated given empty set (collider blocks)
        assert!(graph.d_separated(&x, &y, &empty));

        // X and Y are NOT d-separated given Z (conditioning opens collider)
        assert!(!graph.d_separated(&x, &y, &z));
    }

    #[test]
    fn test_v_structures() {
        // Collider: X -> Z <- Y
        let mut graph = DirectedGraph::new();
        graph.add_edge(0, 2).unwrap(); // X -> Z
        graph.add_edge(1, 2).unwrap(); // Y -> Z

        let v_structs = graph.v_structures();

        assert_eq!(v_structs.len(), 1);
        let (a, b, c) = v_structs[0];
        assert_eq!(b, 2); // Z is the collider
        assert!(a == 0 || a == 1);
        assert!(c == 0 || c == 1);
        assert_ne!(a, c);
    }

    #[test]
    fn test_labels() {
        let mut graph = DirectedGraph::new();
        graph.add_node_with_label(0, "Age");
        graph.add_node_with_label(1, "Income");
        graph.add_edge(0, 1).unwrap();

        assert_eq!(graph.get_label(0), Some("Age"));
        assert_eq!(graph.get_label(1), Some("Income"));
        assert_eq!(graph.find_node_by_label("Age"), Some(0));
        assert_eq!(graph.find_node_by_label("Unknown"), None);
    }

    #[test]
    fn test_find_all_paths() {
        let mut graph = DirectedGraph::new();
        // Diamond: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(0, 2).unwrap();
        graph.add_edge(1, 3).unwrap();
        graph.add_edge(2, 3).unwrap();

        let paths = graph.find_all_paths(0, 3, 10);

        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&vec![0, 1, 3]));
        assert!(paths.contains(&vec![0, 2, 3]));
    }

    #[test]
    fn test_skeleton() {
        let mut graph = DirectedGraph::new();
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(1, 2).unwrap();
        graph.add_edge(2, 0).ok(); // This will fail due to cycle

        // Add a valid edge instead
        graph.add_edge(0, 2).unwrap();

        let skeleton = graph.skeleton();

        assert_eq!(skeleton.len(), 3);
        assert!(skeleton.contains(&(0, 1)));
        assert!(skeleton.contains(&(0, 2)));
        assert!(skeleton.contains(&(1, 2)));
    }

    #[test]
    fn test_remove_edge() {
        let mut graph = DirectedGraph::new();
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(1, 2).unwrap();

        assert!(graph.contains_edge(0, 1));
        graph.remove_edge(0, 1).unwrap();
        assert!(!graph.contains_edge(0, 1));
    }
}
