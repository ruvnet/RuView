// Graph traversal algorithms

use super::storage::GraphStore;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

/// Result of a path search
#[derive(Debug, Clone)]
pub struct PathResult {
    pub nodes: Vec<u64>,
    pub edges: Vec<u64>,
    pub cost: f64,
}

impl PathResult {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            cost: 0.0,
        }
    }

    pub fn with_nodes(mut self, nodes: Vec<u64>) -> Self {
        self.nodes = nodes;
        self
    }

    pub fn with_edges(mut self, edges: Vec<u64>) -> Self {
        self.edges = edges;
        self
    }

    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost = cost;
        self
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Breadth-First Search to find shortest path (by hop count)
///
/// # Arguments
/// * `graph` - The graph to search
/// * `start` - Starting node ID
/// * `end` - Target node ID
/// * `edge_types` - Optional filter for edge types (None means all types)
/// * `max_hops` - Maximum path length
///
/// # Returns
/// Some(PathResult) if path found, None otherwise
pub fn bfs(
    graph: &GraphStore,
    start: u64,
    end: u64,
    edge_types: Option<&[String]>,
    max_hops: usize,
) -> Option<PathResult> {
    if start == end {
        return Some(PathResult::new().with_nodes(vec![start]));
    }

    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let mut parent: HashMap<u64, (u64, u64)> = HashMap::new(); // node -> (parent_node, edge_id)

    queue.push_back((start, 0)); // (node_id, depth)
    visited.insert(start);

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_hops {
            continue;
        }

        // Get outgoing edges
        let edges = graph.edges.get_outgoing(current);

        for edge in edges {
            // Filter by edge type if specified
            if let Some(types) = edge_types {
                if !types.contains(&edge.edge_type) {
                    continue;
                }
            }

            let next = edge.target;

            if !visited.contains(&next) {
                visited.insert(next);
                parent.insert(next, (current, edge.id));

                if next == end {
                    // Reconstruct path
                    return Some(reconstruct_path(&parent, start, end));
                }

                queue.push_back((next, depth + 1));
            }
        }
    }

    None
}

/// Depth-First Search with visitor pattern
///
/// # Arguments
/// * `graph` - The graph to search
/// * `start` - Starting node ID
/// * `visitor` - Function called for each visited node, returns false to stop traversal
pub fn dfs<F>(graph: &GraphStore, start: u64, mut visitor: F)
where
    F: FnMut(u64) -> bool,
{
    let mut visited = HashSet::new();
    let mut stack = vec![start];

    while let Some(current) = stack.pop() {
        if visited.contains(&current) {
            continue;
        }

        visited.insert(current);

        // Call visitor
        if !visitor(current) {
            break;
        }

        // Add neighbors to stack
        let neighbors = graph.edges.get_neighbors(current);
        for neighbor in neighbors.into_iter().rev() {
            if !visited.contains(&neighbor) {
                stack.push(neighbor);
            }
        }
    }
}

/// State for Dijkstra's algorithm
#[derive(Debug, Clone)]
struct DijkstraState {
    node: u64,
    cost: f64,
    #[allow(dead_code)]
    edge: Option<u64>,
}

impl PartialEq for DijkstraState {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

impl Eq for DijkstraState {}

impl PartialOrd for DijkstraState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse ordering for min-heap
        other.cost.partial_cmp(&self.cost)
    }
}

impl Ord for DijkstraState {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Dijkstra's shortest path algorithm with weighted edges
///
/// # Arguments
/// * `graph` - The graph to search
/// * `start` - Starting node ID
/// * `end` - Target node ID
/// * `weight_property` - Name of edge property to use as weight (defaults to 1.0 if missing)
///
/// # Returns
/// Some(PathResult) with weighted cost if path found, None otherwise
pub fn shortest_path_dijkstra(
    graph: &GraphStore,
    start: u64,
    end: u64,
    weight_property: &str,
) -> Option<PathResult> {
    if start == end {
        return Some(PathResult::new().with_nodes(vec![start]).with_cost(0.0));
    }

    let mut heap = BinaryHeap::new();
    let mut distances: HashMap<u64, f64> = HashMap::new();
    let mut parent: HashMap<u64, (u64, u64)> = HashMap::new();

    distances.insert(start, 0.0);
    heap.push(DijkstraState {
        node: start,
        cost: 0.0,
        edge: None,
    });

    while let Some(DijkstraState { node, cost, .. }) = heap.pop() {
        if node == end {
            let mut result = reconstruct_path(&parent, start, end);
            result.cost = cost;
            return Some(result);
        }

        // Skip if we've found a better path already
        if let Some(&best_cost) = distances.get(&node) {
            if cost > best_cost {
                continue;
            }
        }

        // Check all neighbors
        let edges = graph.edges.get_outgoing(node);

        for edge in edges {
            let next = edge.target;
            let weight = edge.weight(weight_property);
            let next_cost = cost + weight;

            let is_better = distances
                .get(&next)
                .map_or(true, |&current_cost| next_cost < current_cost);

            if is_better {
                distances.insert(next, next_cost);
                parent.insert(next, (node, edge.id));
                heap.push(DijkstraState {
                    node: next,
                    cost: next_cost,
                    edge: Some(edge.id),
                });
            }
        }
    }

    None
}

/// Reconstruct path from parent map
fn reconstruct_path(parent: &HashMap<u64, (u64, u64)>, start: u64, end: u64) -> PathResult {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut current = end;

    nodes.push(current);

    while current != start {
        if let Some(&(prev, edge_id)) = parent.get(&current) {
            edges.push(edge_id);
            nodes.push(prev);
            current = prev;
        } else {
            // Path broken, should not happen
            break;
        }
    }

    nodes.reverse();
    edges.reverse();

    PathResult::new().with_nodes(nodes).with_edges(edges)
}

/// Find all paths between two nodes (up to max_paths)
pub fn find_all_paths(
    graph: &GraphStore,
    start: u64,
    end: u64,
    max_hops: usize,
    max_paths: usize,
) -> Vec<PathResult> {
    let mut paths = Vec::new();
    let mut current_path = Vec::new();
    let mut visited = HashSet::new();

    fn dfs_all_paths(
        graph: &GraphStore,
        current: u64,
        end: u64,
        max_hops: usize,
        max_paths: usize,
        current_path: &mut Vec<u64>,
        visited: &mut HashSet<u64>,
        paths: &mut Vec<PathResult>,
    ) {
        if paths.len() >= max_paths {
            return;
        }

        if current_path.len() > max_hops {
            return;
        }

        current_path.push(current);
        visited.insert(current);

        if current == end {
            paths.push(PathResult::new().with_nodes(current_path.clone()));
        } else {
            let neighbors = graph.edges.get_neighbors(current);
            for neighbor in neighbors {
                if !visited.contains(&neighbor) {
                    dfs_all_paths(
                        graph,
                        neighbor,
                        end,
                        max_hops,
                        max_paths,
                        current_path,
                        visited,
                        paths,
                    );
                }
            }
        }

        current_path.pop();
        visited.remove(&current);
    }

    dfs_all_paths(
        graph,
        start,
        end,
        max_hops,
        max_paths,
        &mut current_path,
        &mut visited,
        &mut paths,
    );

    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_graph() -> GraphStore {
        let graph = GraphStore::new();

        // Create nodes: 1 -> 2 -> 3 -> 4
        //                \-> 5 ->/
        let n1 = graph.add_node(vec![], HashMap::new());
        let n2 = graph.add_node(vec![], HashMap::new());
        let n3 = graph.add_node(vec![], HashMap::new());
        let n4 = graph.add_node(vec![], HashMap::new());
        let n5 = graph.add_node(vec![], HashMap::new());

        graph
            .add_edge(n1, n2, "KNOWS".to_string(), HashMap::new())
            .unwrap();
        graph
            .add_edge(n2, n3, "KNOWS".to_string(), HashMap::new())
            .unwrap();
        graph
            .add_edge(n3, n4, "KNOWS".to_string(), HashMap::new())
            .unwrap();
        graph
            .add_edge(n1, n5, "KNOWS".to_string(), HashMap::new())
            .unwrap();
        graph
            .add_edge(n5, n4, "KNOWS".to_string(), HashMap::new())
            .unwrap();

        graph
    }

    #[test]
    fn test_bfs() {
        let graph = create_test_graph();

        let path = bfs(&graph, 1, 4, None, 10).unwrap();
        assert_eq!(path.len(), 3); // Shortest path: 1 -> 5 -> 4
        assert_eq!(path.nodes, vec![1, 5, 4]);
    }

    #[test]
    fn test_dfs() {
        let graph = create_test_graph();

        let mut visited = Vec::new();
        dfs(&graph, 1, |node| {
            visited.push(node);
            true
        });

        assert!(visited.contains(&1));
        assert!(visited.len() <= 5);
    }

    #[test]
    fn test_dijkstra() {
        let graph = GraphStore::new();

        let n1 = graph.add_node(vec![], HashMap::new());
        let n2 = graph.add_node(vec![], HashMap::new());
        let n3 = graph.add_node(vec![], HashMap::new());

        graph
            .add_edge(
                n1,
                n2,
                "KNOWS".to_string(),
                HashMap::from([("weight".to_string(), 5.0.into())]),
            )
            .unwrap();

        graph
            .add_edge(
                n2,
                n3,
                "KNOWS".to_string(),
                HashMap::from([("weight".to_string(), 3.0.into())]),
            )
            .unwrap();

        graph
            .add_edge(
                n1,
                n3,
                "KNOWS".to_string(),
                HashMap::from([("weight".to_string(), 10.0.into())]),
            )
            .unwrap();

        let path = shortest_path_dijkstra(&graph, n1, n3, "weight").unwrap();
        assert_eq!(path.cost, 8.0); // 5 + 3
        assert_eq!(path.nodes, vec![n1, n2, n3]);
    }

    #[test]
    fn test_find_all_paths() {
        let graph = create_test_graph();

        let paths = find_all_paths(&graph, 1, 4, 10, 10);
        assert!(paths.len() >= 2); // At least two paths from 1 to 4
    }
}
