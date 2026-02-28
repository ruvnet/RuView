//! Core query DAG data structure

use std::collections::{HashMap, HashSet, VecDeque};

use super::operator_node::OperatorNode;

/// Error types for DAG operations
#[derive(Debug, thiserror::Error)]
pub enum DagError {
    #[error("Node {0} not found")]
    NodeNotFound(usize),
    #[error("Adding edge would create cycle")]
    CycleDetected,
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    #[error("DAG has cycles, cannot perform topological sort")]
    HasCycles,
}

/// A Directed Acyclic Graph representing a query plan
#[derive(Debug, Clone)]
pub struct QueryDag {
    pub(crate) nodes: HashMap<usize, OperatorNode>,
    pub(crate) edges: HashMap<usize, Vec<usize>>, // parent -> children
    pub(crate) reverse_edges: HashMap<usize, Vec<usize>>, // child -> parents
    pub(crate) root: Option<usize>,
    next_id: usize,
}

impl QueryDag {
    /// Create a new empty DAG
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
            root: None,
            next_id: 0,
        }
    }

    /// Add a node to the DAG, returns the node ID
    pub fn add_node(&mut self, mut node: OperatorNode) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        node.id = id;

        self.nodes.insert(id, node);
        self.edges.insert(id, Vec::new());
        self.reverse_edges.insert(id, Vec::new());

        // If this is the first node, set it as root
        if self.nodes.len() == 1 {
            self.root = Some(id);
        }

        id
    }

    /// Add an edge from parent to child
    pub fn add_edge(&mut self, parent: usize, child: usize) -> Result<(), DagError> {
        // Check both nodes exist
        if !self.nodes.contains_key(&parent) {
            return Err(DagError::NodeNotFound(parent));
        }
        if !self.nodes.contains_key(&child) {
            return Err(DagError::NodeNotFound(child));
        }

        // Check if adding this edge would create a cycle
        if self.would_create_cycle(parent, child) {
            return Err(DagError::CycleDetected);
        }

        // Add edge
        self.edges.get_mut(&parent).unwrap().push(child);
        self.reverse_edges.get_mut(&child).unwrap().push(parent);

        // Update root if child was previously root and now has parents
        if self.root == Some(child) && !self.reverse_edges[&child].is_empty() {
            // Find new root (node with no parents)
            self.root = self
                .nodes
                .keys()
                .find(|&&id| self.reverse_edges[&id].is_empty())
                .copied();
        }

        Ok(())
    }

    /// Remove a node from the DAG
    pub fn remove_node(&mut self, id: usize) -> Option<OperatorNode> {
        let node = self.nodes.remove(&id)?;

        // Remove all edges involving this node
        if let Some(children) = self.edges.remove(&id) {
            for child in children {
                if let Some(parents) = self.reverse_edges.get_mut(&child) {
                    parents.retain(|&p| p != id);
                }
            }
        }

        if let Some(parents) = self.reverse_edges.remove(&id) {
            for parent in parents {
                if let Some(children) = self.edges.get_mut(&parent) {
                    children.retain(|&c| c != id);
                }
            }
        }

        // Update root if necessary
        if self.root == Some(id) {
            self.root = self
                .nodes
                .keys()
                .find(|&&nid| self.reverse_edges[&nid].is_empty())
                .copied();
        }

        Some(node)
    }

    /// Get a reference to a node
    pub fn get_node(&self, id: usize) -> Option<&OperatorNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable reference to a node
    pub fn get_node_mut(&mut self, id: usize) -> Option<&mut OperatorNode> {
        self.nodes.get_mut(&id)
    }

    /// Get children of a node
    pub fn children(&self, id: usize) -> &[usize] {
        self.edges.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get parents of a node
    pub fn parents(&self, id: usize) -> &[usize] {
        self.reverse_edges
            .get(&id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get the root node ID
    pub fn root(&self) -> Option<usize> {
        self.root
    }

    /// Get all leaf nodes (nodes with no children)
    pub fn leaves(&self) -> Vec<usize> {
        self.nodes
            .keys()
            .filter(|&&id| self.edges[&id].is_empty())
            .copied()
            .collect()
    }

    /// Get the number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|v| v.len()).sum()
    }

    /// Get iterator over node IDs
    pub fn node_ids(&self) -> impl Iterator<Item = usize> + '_ {
        self.nodes.keys().copied()
    }

    /// Get iterator over all nodes
    pub fn nodes(&self) -> impl Iterator<Item = &OperatorNode> + '_ {
        self.nodes.values()
    }

    /// Check if adding an edge would create a cycle
    fn would_create_cycle(&self, from: usize, to: usize) -> bool {
        // If 'to' can reach 'from', adding edge from->to would create cycle
        self.can_reach(to, from)
    }

    /// Check if 'from' can reach 'to' through existing edges
    fn can_reach(&self, from: usize, to: usize) -> bool {
        if from == to {
            return true;
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(from);
        visited.insert(from);

        while let Some(current) = queue.pop_front() {
            if current == to {
                return true;
            }

            if let Some(children) = self.edges.get(&current) {
                for &child in children {
                    if visited.insert(child) {
                        queue.push_back(child);
                    }
                }
            }
        }

        false
    }

    /// Compute depth of each node from leaves (leaves have depth 0)
    pub fn compute_depths(&self) -> HashMap<usize, usize> {
        let mut depths = HashMap::new();
        let mut visited = HashSet::new();

        // Start from leaves
        let leaves = self.leaves();
        let mut queue: VecDeque<(usize, usize)> = leaves.iter().map(|&id| (id, 0)).collect();

        for &leaf in &leaves {
            visited.insert(leaf);
            depths.insert(leaf, 0);
        }

        while let Some((node, depth)) = queue.pop_front() {
            depths.insert(node, depth);

            // Process parents
            if let Some(parents) = self.reverse_edges.get(&node) {
                for &parent in parents {
                    if visited.insert(parent) {
                        queue.push_back((parent, depth + 1));
                    } else {
                        // Update depth if we found a longer path
                        let current_depth = depths.get(&parent).copied().unwrap_or(0);
                        if depth + 1 > current_depth {
                            depths.insert(parent, depth + 1);
                            queue.push_back((parent, depth + 1));
                        }
                    }
                }
            }
        }

        depths
    }

    /// Get all ancestors of a node
    pub fn ancestors(&self, id: usize) -> HashSet<usize> {
        let mut result = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(parents) = self.reverse_edges.get(&id) {
            for &parent in parents {
                queue.push_back(parent);
                result.insert(parent);
            }
        }

        while let Some(node) = queue.pop_front() {
            if let Some(parents) = self.reverse_edges.get(&node) {
                for &parent in parents {
                    if result.insert(parent) {
                        queue.push_back(parent);
                    }
                }
            }
        }

        result
    }

    /// Get all descendants of a node
    pub fn descendants(&self, id: usize) -> HashSet<usize> {
        let mut result = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(children) = self.edges.get(&id) {
            for &child in children {
                queue.push_back(child);
                result.insert(child);
            }
        }

        while let Some(node) = queue.pop_front() {
            if let Some(children) = self.edges.get(&node) {
                for &child in children {
                    if result.insert(child) {
                        queue.push_back(child);
                    }
                }
            }
        }

        result
    }

    /// Return nodes in topological order as Vec (dependencies first)
    pub fn topological_sort(&self) -> Result<Vec<usize>, DagError> {
        let mut result = Vec::new();
        let mut in_degree: HashMap<usize, usize> = self
            .nodes
            .keys()
            .map(|&id| (id, self.reverse_edges[&id].len()))
            .collect();

        let mut queue: VecDeque<usize> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(&id, _)| id)
            .collect();

        while let Some(node) = queue.pop_front() {
            result.push(node);

            if let Some(children) = self.edges.get(&node) {
                for &child in children {
                    let degree = in_degree.get_mut(&child).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(child);
                    }
                }
            }
        }

        if result.len() != self.nodes.len() {
            return Err(DagError::HasCycles);
        }

        Ok(result)
    }
}

impl Default for QueryDag {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OperatorNode;

    #[test]
    fn test_new_dag() {
        let dag = QueryDag::new();
        assert_eq!(dag.node_count(), 0);
        assert_eq!(dag.edge_count(), 0);
    }

    #[test]
    fn test_add_nodes() {
        let mut dag = QueryDag::new();
        let id1 = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let id2 = dag.add_node(OperatorNode::filter(0, "age > 18"));

        assert_eq!(dag.node_count(), 2);
        assert!(dag.get_node(id1).is_some());
        assert!(dag.get_node(id2).is_some());
    }

    #[test]
    fn test_add_edges() {
        let mut dag = QueryDag::new();
        let id1 = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let id2 = dag.add_node(OperatorNode::filter(0, "age > 18"));

        assert!(dag.add_edge(id1, id2).is_ok());
        assert_eq!(dag.edge_count(), 1);
        assert_eq!(dag.children(id1), &[id2]);
        assert_eq!(dag.parents(id2), &[id1]);
    }

    #[test]
    fn test_cycle_detection() {
        let mut dag = QueryDag::new();
        let id1 = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let id2 = dag.add_node(OperatorNode::filter(0, "age > 18"));
        let id3 = dag.add_node(OperatorNode::sort(0, vec!["name".to_string()]));

        dag.add_edge(id1, id2).unwrap();
        dag.add_edge(id2, id3).unwrap();

        // This would create a cycle
        assert!(matches!(
            dag.add_edge(id3, id1),
            Err(DagError::CycleDetected)
        ));
    }

    #[test]
    fn test_topological_sort() {
        let mut dag = QueryDag::new();
        let id1 = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let id2 = dag.add_node(OperatorNode::filter(0, "age > 18"));
        let id3 = dag.add_node(OperatorNode::sort(0, vec!["name".to_string()]));

        dag.add_edge(id1, id2).unwrap();
        dag.add_edge(id2, id3).unwrap();

        let sorted = dag.topological_sort().unwrap();
        assert_eq!(sorted.len(), 3);

        // id1 should come before id2, id2 before id3
        let pos1 = sorted.iter().position(|&x| x == id1).unwrap();
        let pos2 = sorted.iter().position(|&x| x == id2).unwrap();
        let pos3 = sorted.iter().position(|&x| x == id3).unwrap();

        assert!(pos1 < pos2);
        assert!(pos2 < pos3);
    }

    #[test]
    fn test_remove_node() {
        let mut dag = QueryDag::new();
        let id1 = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let id2 = dag.add_node(OperatorNode::filter(0, "age > 18"));

        dag.add_edge(id1, id2).unwrap();

        let removed = dag.remove_node(id1);
        assert!(removed.is_some());
        assert_eq!(dag.node_count(), 1);
        assert_eq!(dag.edge_count(), 0);
    }

    #[test]
    fn test_ancestors_descendants() {
        let mut dag = QueryDag::new();
        let id1 = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let id2 = dag.add_node(OperatorNode::filter(0, "age > 18"));
        let id3 = dag.add_node(OperatorNode::sort(0, vec!["name".to_string()]));

        dag.add_edge(id1, id2).unwrap();
        dag.add_edge(id2, id3).unwrap();

        let ancestors = dag.ancestors(id3);
        assert!(ancestors.contains(&id1));
        assert!(ancestors.contains(&id2));

        let descendants = dag.descendants(id1);
        assert!(descendants.contains(&id2));
        assert!(descendants.contains(&id3));
    }
}
