//! Delta-aware graph traversal
//!
//! Provides traversal algorithms that account for pending deltas.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::{EdgeId, GraphDelta, GraphState, NodeId, Result};

/// Traversal mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalMode {
    /// Breadth-first search
    Bfs,
    /// Depth-first search
    Dfs,
    /// Dijkstra's shortest path
    Dijkstra,
    /// Best-first (with heuristic)
    BestFirst,
}

/// Configuration for delta-aware traversal
#[derive(Debug, Clone)]
pub struct TraversalConfig {
    /// Maximum depth
    pub max_depth: usize,
    /// Maximum nodes to visit
    pub max_nodes: usize,
    /// Edge types to follow (empty = all)
    pub edge_types: Vec<String>,
    /// Direction
    pub direction: TraversalDirection,
}

impl Default for TraversalConfig {
    fn default() -> Self {
        Self {
            max_depth: usize::MAX,
            max_nodes: usize::MAX,
            edge_types: Vec::new(),
            direction: TraversalDirection::Outgoing,
        }
    }
}

/// Traversal direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalDirection {
    /// Follow outgoing edges
    Outgoing,
    /// Follow incoming edges
    Incoming,
    /// Follow both directions
    Both,
}

/// Delta-aware graph traversal
pub struct DeltaAwareTraversal<'a> {
    state: &'a GraphState,
    pending_delta: Option<&'a GraphDelta>,
    config: TraversalConfig,
}

impl<'a> DeltaAwareTraversal<'a> {
    /// Create new traversal
    pub fn new(state: &'a GraphState) -> Self {
        Self {
            state,
            pending_delta: None,
            config: TraversalConfig::default(),
        }
    }

    /// Set pending delta to consider
    pub fn with_delta(mut self, delta: &'a GraphDelta) -> Self {
        self.pending_delta = Some(delta);
        self
    }

    /// Set configuration
    pub fn with_config(mut self, config: TraversalConfig) -> Self {
        self.config = config;
        self
    }

    /// BFS from start node
    pub fn bfs(&self, start: &NodeId) -> Vec<NodeId> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        if !self.node_exists(start) {
            return result;
        }

        queue.push_back((start.clone(), 0usize));
        visited.insert(start.clone());

        while let Some((node, depth)) = queue.pop_front() {
            if depth > self.config.max_depth {
                continue;
            }
            if result.len() >= self.config.max_nodes {
                break;
            }

            result.push(node.clone());

            for neighbor in self.neighbors(&node) {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor.clone());
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }

        result
    }

    /// DFS from start node
    pub fn dfs(&self, start: &NodeId) -> Vec<NodeId> {
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        let mut result = Vec::new();

        if !self.node_exists(start) {
            return result;
        }

        stack.push((start.clone(), 0usize));

        while let Some((node, depth)) = stack.pop() {
            if visited.contains(&node) {
                continue;
            }
            if depth > self.config.max_depth {
                continue;
            }
            if result.len() >= self.config.max_nodes {
                break;
            }

            visited.insert(node.clone());
            result.push(node.clone());

            for neighbor in self.neighbors(&node) {
                if !visited.contains(&neighbor) {
                    stack.push((neighbor, depth + 1));
                }
            }
        }

        result
    }

    /// Find shortest path
    pub fn shortest_path(&self, from: &NodeId, to: &NodeId) -> Option<Vec<NodeId>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut parent: HashMap<NodeId, NodeId> = HashMap::new();

        if !self.node_exists(from) || !self.node_exists(to) {
            return None;
        }

        queue.push_back(from.clone());
        visited.insert(from.clone());

        while let Some(node) = queue.pop_front() {
            if &node == to {
                // Reconstruct path
                let mut path = vec![node.clone()];
                let mut current = &node;
                while let Some(p) = parent.get(current) {
                    path.push(p.clone());
                    current = p;
                }
                path.reverse();
                return Some(path);
            }

            for neighbor in self.neighbors(&node) {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor.clone());
                    parent.insert(neighbor.clone(), node.clone());
                    queue.push_back(neighbor);
                }
            }
        }

        None
    }

    /// Count connected components
    pub fn connected_components(&self) -> Vec<Vec<NodeId>> {
        let mut visited = HashSet::new();
        let mut components = Vec::new();

        let all_nodes = self.all_nodes();

        for node in all_nodes {
            if !visited.contains(&node) {
                // Use BFS to find component
                let mut component = Vec::new();
                let mut queue = VecDeque::new();
                queue.push_back(node.clone());
                visited.insert(node.clone());

                while let Some(n) = queue.pop_front() {
                    component.push(n.clone());

                    for neighbor in self.neighbors_both_directions(&n) {
                        if !visited.contains(&neighbor) {
                            visited.insert(neighbor.clone());
                            queue.push_back(neighbor);
                        }
                    }
                }

                components.push(component);
            }
        }

        components
    }

    // Helper methods

    fn node_exists(&self, id: &NodeId) -> bool {
        // Check base state
        if self.state.nodes.contains_key(id) {
            // Check if removed by delta
            if let Some(delta) = self.pending_delta {
                if delta.node_removes.contains(id) {
                    return false;
                }
            }
            return true;
        }

        // Check if added by delta
        if let Some(delta) = self.pending_delta {
            if delta.node_adds.iter().any(|(nid, _)| nid == id) {
                return true;
            }
        }

        false
    }

    fn neighbors(&self, id: &NodeId) -> Vec<NodeId> {
        let mut neighbors = Vec::new();

        // Get neighbors from base state
        for (edge_id, (source, target, edge_type, _)) in &self.state.edges {
            // Skip if removed by delta
            if let Some(delta) = self.pending_delta {
                if delta.edge_removes.contains(edge_id) {
                    continue;
                }
            }

            // Filter by edge type
            if !self.config.edge_types.is_empty() && !self.config.edge_types.contains(edge_type) {
                continue;
            }

            match self.config.direction {
                TraversalDirection::Outgoing => {
                    if source == id {
                        neighbors.push(target.clone());
                    }
                }
                TraversalDirection::Incoming => {
                    if target == id {
                        neighbors.push(source.clone());
                    }
                }
                TraversalDirection::Both => {
                    if source == id {
                        neighbors.push(target.clone());
                    } else if target == id {
                        neighbors.push(source.clone());
                    }
                }
            }
        }

        // Add neighbors from delta edges
        if let Some(delta) = self.pending_delta {
            for edge in &delta.edge_adds {
                if !self.config.edge_types.is_empty()
                    && !self.config.edge_types.contains(&edge.edge_type)
                {
                    continue;
                }

                match self.config.direction {
                    TraversalDirection::Outgoing => {
                        if &edge.source == id {
                            neighbors.push(edge.target.clone());
                        }
                    }
                    TraversalDirection::Incoming => {
                        if &edge.target == id {
                            neighbors.push(edge.source.clone());
                        }
                    }
                    TraversalDirection::Both => {
                        if &edge.source == id {
                            neighbors.push(edge.target.clone());
                        } else if &edge.target == id {
                            neighbors.push(edge.source.clone());
                        }
                    }
                }
            }
        }

        neighbors
    }

    fn neighbors_both_directions(&self, id: &NodeId) -> Vec<NodeId> {
        let config = TraversalConfig {
            direction: TraversalDirection::Both,
            ..self.config.clone()
        };

        let traversal = DeltaAwareTraversal {
            state: self.state,
            pending_delta: self.pending_delta,
            config,
        };

        traversal.neighbors(id)
    }

    fn all_nodes(&self) -> Vec<NodeId> {
        let mut nodes: HashSet<NodeId> = self.state.nodes.keys().cloned().collect();

        if let Some(delta) = self.pending_delta {
            // Remove deleted nodes
            for id in &delta.node_removes {
                nodes.remove(id);
            }

            // Add new nodes
            for (id, _) in &delta.node_adds {
                nodes.insert(id.clone());
            }
        }

        nodes.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GraphDeltaBuilder;

    fn create_test_state() -> GraphState {
        let mut state = GraphState::new();

        let delta = GraphDeltaBuilder::new()
            .add_node("a")
            .add_node("b")
            .add_node("c")
            .add_node("d")
            .add_edge("e1", "a", "b", "KNOWS")
            .add_edge("e2", "b", "c", "KNOWS")
            .add_edge("e3", "c", "d", "KNOWS")
            .build();

        state.apply_delta(&delta).unwrap();
        state
    }

    #[test]
    fn test_bfs() {
        let state = create_test_state();
        let traversal = DeltaAwareTraversal::new(&state);

        let result = traversal.bfs(&"a".to_string());

        assert_eq!(result.len(), 4);
        assert_eq!(result[0], "a");
    }

    #[test]
    fn test_dfs() {
        let state = create_test_state();
        let traversal = DeltaAwareTraversal::new(&state);

        let result = traversal.dfs(&"a".to_string());

        assert_eq!(result.len(), 4);
        assert_eq!(result[0], "a");
    }

    #[test]
    fn test_shortest_path() {
        let state = create_test_state();
        let traversal = DeltaAwareTraversal::new(&state);

        let path = traversal
            .shortest_path(&"a".to_string(), &"d".to_string())
            .unwrap();

        assert_eq!(path.len(), 4);
        assert_eq!(path[0], "a");
        assert_eq!(path[3], "d");
    }

    #[test]
    fn test_with_pending_delta() {
        let state = create_test_state();

        // Add new edge directly from a to d
        let delta = GraphDeltaBuilder::new()
            .add_edge("e4", "a", "d", "SHORTCUT")
            .build();

        let traversal = DeltaAwareTraversal::new(&state).with_delta(&delta);

        let path = traversal
            .shortest_path(&"a".to_string(), &"d".to_string())
            .unwrap();

        // Should find shorter path via new edge
        assert_eq!(path.len(), 2);
    }

    #[test]
    fn test_connected_components() {
        let mut state = create_test_state();

        // Add isolated nodes
        let delta = GraphDeltaBuilder::new()
            .add_node("x")
            .add_node("y")
            .add_edge("ex", "x", "y", "RELATED")
            .build();

        state.apply_delta(&delta).unwrap();

        let traversal = DeltaAwareTraversal::new(&state);
        let components = traversal.connected_components();

        assert_eq!(components.len(), 2);
    }
}
