//! Local K-Cut: Sublinear min-cut approximation

use super::engine::{FlowEdge, MinCutResult};
use std::collections::{HashMap, HashSet, VecDeque};

/// Local K-Cut oracle for approximate min-cut
pub struct LocalKCut {
    visited: HashSet<usize>,
    distance: HashMap<usize, usize>,
}

impl LocalKCut {
    pub fn new() -> Self {
        Self {
            visited: HashSet::new(),
            distance: HashMap::new(),
        }
    }

    /// Compute approximate min-cut using local search
    /// Time complexity: O(k * local_depth) where k << n
    pub fn compute(
        &mut self,
        graph: &HashMap<usize, Vec<FlowEdge>>,
        source: usize,
        sink: usize,
        depth: usize,
    ) -> MinCutResult {
        self.visited.clear();
        self.distance.clear();

        // BFS from source with limited depth
        let source_reachable = self.limited_bfs(graph, source, depth);

        // BFS from sink with limited depth
        let sink_reachable = self.limited_bfs(graph, sink, depth);

        // Find cut edges
        let mut cut_edges = Vec::new();
        let mut cut_value = 0.0;

        for &node in &source_reachable {
            if let Some(edges) = graph.get(&node) {
                for edge in edges {
                    if !source_reachable.contains(&edge.to) && edge.capacity > 0.0 {
                        cut_edges.push((edge.from, edge.to));
                        cut_value += edge.capacity;
                    }
                }
            }
        }

        MinCutResult {
            cut_value,
            source_side: source_reachable,
            sink_side: sink_reachable,
            cut_edges,
        }
    }

    fn limited_bfs(
        &mut self,
        graph: &HashMap<usize, Vec<FlowEdge>>,
        start: usize,
        max_depth: usize,
    ) -> HashSet<usize> {
        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((start, 0));
        reachable.insert(start);

        while let Some((node, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            if let Some(edges) = graph.get(&node) {
                for edge in edges {
                    if edge.capacity > edge.flow && !reachable.contains(&edge.to) {
                        reachable.insert(edge.to);
                        queue.push_back((edge.to, depth + 1));
                    }
                }
            }
        }

        reachable
    }
}
