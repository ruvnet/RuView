//! Minimal WASM DAG library optimized for browser and embedded systems
//!
//! Size optimizations:
//! - u8/u32/f32 instead of larger types
//! - Inline hot paths
//! - Minimal error handling
//! - No string operations in critical paths
//! - Optional wee_alloc for smaller binary

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// Use wee_alloc for smaller WASM binary (~10KB reduction)
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Minimal DAG node - 9 bytes (u32 + u8 + f32)
#[derive(Serialize, Deserialize, Clone, Copy)]
struct WasmNode {
    id: u32,
    op: u8,
    cost: f32,
}

/// Minimal DAG structure for WASM
/// Self-contained with no external dependencies beyond wasm-bindgen
#[wasm_bindgen]
pub struct WasmDag {
    nodes: Vec<WasmNode>,
    edges: Vec<(u32, u32)>,
}

#[wasm_bindgen]
impl WasmDag {
    /// Create new empty DAG
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Add a node with operator type and cost
    /// Returns node ID
    #[inline]
    pub fn add_node(&mut self, op: u8, cost: f32) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(WasmNode { id, op, cost });
        id
    }

    /// Add edge from -> to
    /// Returns false if creates cycle (simple check)
    #[inline]
    pub fn add_edge(&mut self, from: u32, to: u32) -> bool {
        // Basic validation - nodes must exist
        if from >= self.nodes.len() as u32 || to >= self.nodes.len() as u32 {
            return false;
        }

        // Simple cycle check: to must not reach from
        if self.has_path(to, from) {
            return false;
        }

        self.edges.push((from, to));
        true
    }

    /// Get number of nodes
    #[inline]
    pub fn node_count(&self) -> u32 {
        self.nodes.len() as u32
    }

    /// Get number of edges
    #[inline]
    pub fn edge_count(&self) -> u32 {
        self.edges.len() as u32
    }

    /// Topological sort using Kahn's algorithm
    /// Returns node IDs in topological order
    pub fn topo_sort(&self) -> Vec<u32> {
        let n = self.nodes.len();
        let mut in_degree = vec![0u32; n];

        // Calculate in-degrees
        for &(_, to) in &self.edges {
            in_degree[to as usize] += 1;
        }

        // Find nodes with no incoming edges
        let mut queue: Vec<u32> = (0..n as u32)
            .filter(|&i| in_degree[i as usize] == 0)
            .collect();

        let mut result = Vec::with_capacity(n);

        while let Some(node) = queue.pop() {
            result.push(node);

            // Reduce in-degree for neighbors
            for &(from, to) in &self.edges {
                if from == node {
                    in_degree[to as usize] -= 1;
                    if in_degree[to as usize] == 0 {
                        queue.push(to);
                    }
                }
            }
        }

        result
    }

    /// Find critical path (longest path by cost)
    /// Returns JSON: {"path": [node_ids], "cost": total}
    pub fn critical_path(&self) -> JsValue {
        let topo = self.topo_sort();
        let n = self.nodes.len();

        // dist[i] = (max_cost_to_i, predecessor)
        let mut dist = vec![(0.0f32, u32::MAX); n];

        // Initialize starting nodes
        for &node in &topo {
            if !self.has_incoming(node) {
                dist[node as usize] = (self.nodes[node as usize].cost, u32::MAX);
            }
        }

        // Relax edges in topological order
        for &from in &topo {
            let from_cost = dist[from as usize].0;

            for &(f, to) in &self.edges {
                if f == from {
                    let new_cost = from_cost + self.nodes[to as usize].cost;
                    if new_cost > dist[to as usize].0 {
                        dist[to as usize] = (new_cost, from);
                    }
                }
            }
        }

        // Find node with maximum cost
        let (max_idx, (max_cost, _)) = dist
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.0.partial_cmp(&b.0).unwrap())
            .unwrap();

        // Backtrack to build path
        let mut path = Vec::new();
        let mut current = max_idx as u32;

        while current != u32::MAX {
            path.push(current);
            current = dist[current as usize].1;
        }

        path.reverse();

        // Convert to JSON manually to avoid serde_json dependency
        let path_str = path
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let json = format!("{{\"path\":[{}],\"cost\":{}}}", path_str, max_cost);
        JsValue::from_str(&json)
    }

    /// Compute attention scores for nodes
    /// mechanism: 0=topological, 1=critical_path, 2=uniform
    pub fn attention(&self, mechanism: u8) -> Vec<f32> {
        compute_attention(self, mechanism)
    }

    /// Serialize to bytes (bincode format)
    pub fn to_bytes(&self) -> Vec<u8> {
        #[derive(Serialize)]
        struct SerDag<'a> {
            nodes: &'a [WasmNode],
            edges: &'a [(u32, u32)],
        }

        let data = SerDag {
            nodes: &self.nodes,
            edges: &self.edges,
        };

        bincode::serialize(&data).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<WasmDag, JsValue> {
        #[derive(Deserialize)]
        struct SerDag {
            nodes: Vec<WasmNode>,
            edges: Vec<(u32, u32)>,
        }

        bincode::deserialize::<SerDag>(data)
            .map(|d| WasmDag {
                nodes: d.nodes,
                edges: d.edges,
            })
            .map_err(|e| JsValue::from_str(&format!("Deserialize error: {}", e)))
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct SerDag<'a> {
            nodes: &'a [WasmNode],
            edges: &'a [(u32, u32)],
        }

        let data = SerDag {
            nodes: &self.nodes,
            edges: &self.edges,
        };

        serde_json::to_string(&data).unwrap_or_else(|_| String::from("{}"))
    }

    /// Deserialize from JSON
    pub fn from_json(json: &str) -> Result<WasmDag, JsValue> {
        #[derive(Deserialize)]
        struct SerDag {
            nodes: Vec<WasmNode>,
            edges: Vec<(u32, u32)>,
        }

        serde_json::from_str::<SerDag>(json)
            .map(|d| WasmDag {
                nodes: d.nodes,
                edges: d.edges,
            })
            .map_err(|e| JsValue::from_str(&format!("JSON error: {}", e)))
    }
}

// Internal helper methods (not exported to WASM)
impl WasmDag {
    /// Check if there's a path from 'from' to 'to' (for cycle detection)
    #[inline(always)]
    fn has_path(&self, from: u32, to: u32) -> bool {
        if from == to {
            return true;
        }

        let mut visited = vec![false; self.nodes.len()];
        let mut stack = Vec::with_capacity(8);
        stack.push(from);

        while let Some(node) = stack.pop() {
            if visited[node as usize] {
                continue;
            }
            visited[node as usize] = true;

            for &(f, t) in &self.edges {
                if f == node {
                    if t == to {
                        return true;
                    }
                    stack.push(t);
                }
            }
        }

        false
    }

    /// Check if node has incoming edges
    #[inline(always)]
    fn has_incoming(&self, node: u32) -> bool {
        self.edges.iter().any(|&(_, to)| to == node)
    }
}

/// Compute attention scores based on mechanism
///
/// Mechanisms:
/// - 0: Topological (position in topo sort)
/// - 1: Critical path (distance from critical path)
/// - 2: Uniform (all equal)
#[inline]
fn compute_attention(dag: &WasmDag, mechanism: u8) -> Vec<f32> {
    let n = dag.nodes.len();

    match mechanism {
        0 => {
            // Topological attention - earlier nodes get higher scores
            let topo = dag.topo_sort();
            let mut scores = vec![0.0f32; n];

            for (i, &node_id) in topo.iter().enumerate() {
                scores[node_id as usize] = 1.0 - (i as f32 / n as f32);
            }

            scores
        }

        1 => {
            // Critical path attention - nodes on/near critical path get higher scores
            let topo = dag.topo_sort();
            let mut dist = vec![0.0f32; n];

            // Forward pass - compute longest path to each node
            for &from in &topo {
                for &(f, to) in &dag.edges {
                    if f == from {
                        let new_dist = dist[from as usize] + dag.nodes[to as usize].cost;
                        if new_dist > dist[to as usize] {
                            dist[to as usize] = new_dist;
                        }
                    }
                }
            }

            // Normalize to [0, 1]
            let max_dist = dist.iter().fold(0.0f32, |a, &b| a.max(b));
            if max_dist > 0.0 {
                dist.iter_mut().for_each(|d| *d /= max_dist);
            }

            dist
        }

        _ => {
            // Uniform attention
            vec![1.0f32 / n as f32; n]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_dag() {
        let mut dag = WasmDag::new();

        let n0 = dag.add_node(1, 1.0);
        let n1 = dag.add_node(2, 2.0);
        let n2 = dag.add_node(3, 3.0);

        assert_eq!(dag.node_count(), 3);

        assert!(dag.add_edge(n0, n1));
        assert!(dag.add_edge(n1, n2));
        assert_eq!(dag.edge_count(), 2);

        // Should detect cycle
        assert!(!dag.add_edge(n2, n0));
    }

    #[test]
    fn test_topo_sort() {
        let mut dag = WasmDag::new();

        let n0 = dag.add_node(0, 1.0);
        let n1 = dag.add_node(1, 1.0);
        let n2 = dag.add_node(2, 1.0);

        dag.add_edge(n0, n1);
        dag.add_edge(n1, n2);

        let topo = dag.topo_sort();
        assert_eq!(topo, vec![0, 1, 2]);
    }

    #[test]
    fn test_attention() {
        let mut dag = WasmDag::new();

        dag.add_node(0, 1.0);
        dag.add_node(1, 2.0);
        dag.add_node(2, 3.0);

        // Uniform
        let uniform = dag.attention(2);
        assert_eq!(uniform.len(), 3);
        assert!((uniform[0] - 0.333).abs() < 0.01);

        // Topological
        let topo = dag.attention(0);
        assert_eq!(topo.len(), 3);
    }

    #[test]
    fn test_serialization() {
        let mut dag = WasmDag::new();

        dag.add_node(1, 1.5);
        dag.add_node(2, 2.5);
        dag.add_edge(0, 1);

        // Binary
        let bytes = dag.to_bytes();
        let restored = WasmDag::from_bytes(&bytes).unwrap();
        assert_eq!(restored.node_count(), 2);
        assert_eq!(restored.edge_count(), 1);

        // JSON
        let json = dag.to_json();
        let from_json = WasmDag::from_json(&json).unwrap();
        assert_eq!(from_json.node_count(), 2);
    }
}
