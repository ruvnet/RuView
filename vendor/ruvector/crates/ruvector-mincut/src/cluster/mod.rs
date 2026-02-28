//! Multi-level Cluster Hierarchy for Dynamic Minimum Cut
//!
//! Implements hierarchical clustering from the December 2024 paper.
//! Enables efficient cut maintenance through recursive decomposition.

pub mod hierarchy;

use crate::graph::{DynamicGraph, EdgeId, VertexId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// A cluster at a specific level in the hierarchy
#[derive(Debug, Clone)]
pub struct Cluster {
    /// Unique cluster ID
    pub id: u64,
    /// Level in hierarchy (0 = leaf level)
    pub level: usize,
    /// Vertices contained in this cluster
    pub vertices: HashSet<VertexId>,
    /// Boundary edges (edges leaving the cluster)
    pub boundary_edges: Vec<EdgeId>,
    /// Boundary size (cut value if this cluster were separated)
    pub boundary_size: u64,
    /// Parent cluster ID (None for root)
    pub parent: Option<u64>,
    /// Child cluster IDs (empty for leaf clusters)
    pub children: Vec<u64>,
}

/// Multi-level cluster hierarchy
pub struct ClusterHierarchy {
    /// All clusters indexed by ID
    pub clusters: HashMap<u64, Cluster>,
    /// Root cluster ID
    root_id: Option<u64>,
    /// Number of levels
    num_levels: usize,
    /// Vertex to leaf cluster mapping
    vertex_cluster: HashMap<VertexId, u64>,
    /// Next cluster ID
    next_id: u64,
    /// Reference to graph
    graph: Arc<DynamicGraph>,
    /// Target cluster size at each level
    target_sizes: Vec<usize>,
}

impl ClusterHierarchy {
    /// Create a new hierarchy for the given graph
    pub fn new(graph: Arc<DynamicGraph>) -> Self {
        let mut hierarchy = Self {
            clusters: HashMap::new(),
            root_id: None,
            num_levels: 0,
            vertex_cluster: HashMap::new(),
            next_id: 0,
            graph,
            target_sizes: Vec::new(),
        };
        hierarchy.rebuild();
        hierarchy
    }

    /// Rebuild the entire hierarchy from scratch
    pub fn rebuild(&mut self) {
        self.clusters.clear();
        self.vertex_cluster.clear();
        self.next_id = 0;

        let vertices = self.graph.vertices();
        if vertices.is_empty() {
            self.root_id = None;
            self.num_levels = 0;
            return;
        }

        // Compute number of levels: O(log n)
        let n = vertices.len();
        self.num_levels = (n as f64).log2().ceil() as usize + 1;

        // Compute target sizes for each level
        self.target_sizes = (0..self.num_levels)
            .map(|l| 2usize.pow(l as u32).min(n))
            .collect();

        // Build leaf clusters (level 0)
        let leaf_ids = self.build_leaf_clusters(&vertices);

        // Build upper levels recursively
        let mut current_level_ids = leaf_ids;
        for level in 1..self.num_levels {
            current_level_ids = self.build_level(level, &current_level_ids);
            if current_level_ids.len() == 1 {
                self.root_id = Some(current_level_ids[0]);
                break;
            }
        }

        if self.root_id.is_none() && !current_level_ids.is_empty() {
            self.root_id = Some(current_level_ids[0]);
        }
    }

    /// Build leaf clusters (each vertex is its own cluster initially)
    fn build_leaf_clusters(&mut self, vertices: &[VertexId]) -> Vec<u64> {
        vertices
            .iter()
            .map(|&v| {
                let cluster_id = self.next_id;
                self.next_id += 1;

                // Compute boundary
                let (boundary_edges, boundary_size) = self.compute_vertex_boundary(v);

                let cluster = Cluster {
                    id: cluster_id,
                    level: 0,
                    vertices: [v].into_iter().collect(),
                    boundary_edges,
                    boundary_size,
                    parent: None,
                    children: Vec::new(),
                };

                self.clusters.insert(cluster_id, cluster);
                self.vertex_cluster.insert(v, cluster_id);
                cluster_id
            })
            .collect()
    }

    /// Build a level by merging clusters from the previous level
    fn build_level(&mut self, level: usize, child_ids: &[u64]) -> Vec<u64> {
        // Group children into parent clusters
        // Target: reduce number of clusters by factor of 2
        let _target_count = (child_ids.len() + 1) / 2;
        let mut parent_ids = Vec::new();

        for chunk in child_ids.chunks(2) {
            let parent_id = self.next_id;
            self.next_id += 1;

            // Merge child vertices
            let mut vertices = HashSet::new();
            for &child_id in chunk {
                if let Some(child) = self.clusters.get_mut(&child_id) {
                    vertices.extend(child.vertices.iter().copied());
                    child.parent = Some(parent_id);
                }
            }

            // Compute boundary for merged cluster
            let (boundary_edges, boundary_size) = self.compute_cluster_boundary(&vertices);

            let parent = Cluster {
                id: parent_id,
                level,
                vertices,
                boundary_edges,
                boundary_size,
                parent: None,
                children: chunk.to_vec(),
            };

            self.clusters.insert(parent_id, parent);
            parent_ids.push(parent_id);
        }

        parent_ids
    }

    /// Compute boundary edges and size for a single vertex
    fn compute_vertex_boundary(&self, v: VertexId) -> (Vec<EdgeId>, u64) {
        let mut boundary_edges = Vec::new();
        let mut boundary_size = 0u64;

        for edge in self.graph.edges() {
            if edge.source == v || edge.target == v {
                boundary_edges.push(edge.id);
                boundary_size += 1;
            }
        }

        (boundary_edges, boundary_size)
    }

    /// Compute boundary edges and size for a cluster
    fn compute_cluster_boundary(&self, vertices: &HashSet<VertexId>) -> (Vec<EdgeId>, u64) {
        let mut boundary_edges = Vec::new();
        let mut boundary_size = 0u64;

        for edge in self.graph.edges() {
            let src_in = vertices.contains(&edge.source);
            let tgt_in = vertices.contains(&edge.target);

            // Edge crosses boundary if exactly one endpoint is inside
            if src_in != tgt_in {
                boundary_edges.push(edge.id);
                boundary_size += 1;
            }
        }

        (boundary_edges, boundary_size)
    }

    /// Handle edge insertion
    pub fn insert_edge(&mut self, _edge_id: EdgeId, u: VertexId, v: VertexId) {
        // Update boundaries for all clusters containing u or v
        self.update_boundaries_for_vertices(&[u, v]);
    }

    /// Handle edge deletion
    pub fn delete_edge(&mut self, _edge_id: EdgeId, u: VertexId, v: VertexId) {
        // Update boundaries for all clusters containing u or v
        self.update_boundaries_for_vertices(&[u, v]);
    }

    /// Update boundary information for clusters containing given vertices
    fn update_boundaries_for_vertices(&mut self, vertices: &[VertexId]) {
        // Find all clusters that need updating (traverse up the hierarchy)
        let mut clusters_to_update = HashSet::new();

        for &v in vertices {
            if let Some(&cluster_id) = self.vertex_cluster.get(&v) {
                let mut current = Some(cluster_id);
                while let Some(id) = current {
                    clusters_to_update.insert(id);
                    current = self.clusters.get(&id).and_then(|c| c.parent);
                }
            }
        }

        // Update each cluster's boundary
        for cluster_id in clusters_to_update {
            if let Some(cluster) = self.clusters.get(&cluster_id) {
                let vertices = cluster.vertices.clone();
                let (boundary_edges, boundary_size) = self.compute_cluster_boundary(&vertices);

                if let Some(cluster) = self.clusters.get_mut(&cluster_id) {
                    cluster.boundary_edges = boundary_edges;
                    cluster.boundary_size = boundary_size;
                }
            }
        }
    }

    /// Find the smallest cluster containing both vertices
    pub fn lowest_common_cluster(&self, u: VertexId, v: VertexId) -> Option<u64> {
        let u_cluster = self.vertex_cluster.get(&u)?;
        let v_cluster = self.vertex_cluster.get(&v)?;

        // Build path from u to root
        let mut u_path = HashSet::new();
        let mut current = Some(*u_cluster);
        while let Some(id) = current {
            u_path.insert(id);
            current = self.clusters.get(&id).and_then(|c| c.parent);
        }

        // Find first intersection with v's path
        current = Some(*v_cluster);
        while let Some(id) = current {
            if u_path.contains(&id) {
                return Some(id);
            }
            current = self.clusters.get(&id).and_then(|c| c.parent);
        }

        None
    }

    /// Get minimum boundary size across all clusters
    pub fn min_boundary(&self) -> u64 {
        self.clusters
            .values()
            .filter(|c| !c.vertices.is_empty() && c.vertices.len() < self.graph.num_vertices())
            .map(|c| c.boundary_size)
            .min()
            .unwrap_or(u64::MAX)
    }

    /// Get cluster by ID
    pub fn get_cluster(&self, id: u64) -> Option<&Cluster> {
        self.clusters.get(&id)
    }

    /// Get number of levels
    pub fn num_levels(&self) -> usize {
        self.num_levels
    }

    /// Get root cluster
    pub fn root(&self) -> Option<&Cluster> {
        self.root_id.and_then(|id| self.clusters.get(&id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let graph = Arc::new(DynamicGraph::new());
        let hierarchy = ClusterHierarchy::new(graph);
        assert_eq!(hierarchy.num_levels(), 0);
        assert!(hierarchy.root().is_none());
    }

    #[test]
    fn test_single_vertex() {
        let graph = Arc::new(DynamicGraph::new());
        graph.add_vertex(1);
        let hierarchy = ClusterHierarchy::new(graph);
        assert!(hierarchy.num_levels() >= 1);
    }

    #[test]
    fn test_path_graph() {
        let graph = Arc::new(DynamicGraph::new());
        for i in 0..9 {
            graph.insert_edge(i, i + 1, 1.0).unwrap();
        }
        let hierarchy = ClusterHierarchy::new(graph);
        assert!(hierarchy.num_levels() > 1);
        assert_eq!(hierarchy.min_boundary(), 1); // Path has min cut 1
    }

    #[test]
    fn test_cycle_graph() {
        let graph = Arc::new(DynamicGraph::new());
        for i in 0..5 {
            graph.insert_edge(i, (i + 1) % 5, 1.0).unwrap();
        }
        let hierarchy = ClusterHierarchy::new(graph);
        assert_eq!(hierarchy.min_boundary(), 2); // Cycle has min cut 2
    }

    #[test]
    fn test_lowest_common_cluster() {
        let graph = Arc::new(DynamicGraph::new());
        graph.insert_edge(0, 1, 1.0).unwrap();
        graph.insert_edge(1, 2, 1.0).unwrap();

        let hierarchy = ClusterHierarchy::new(graph);
        let lcc = hierarchy.lowest_common_cluster(0, 2);
        assert!(lcc.is_some());
    }

    #[test]
    fn test_dynamic_update() {
        let graph = Arc::new(DynamicGraph::new());
        graph.insert_edge(0, 1, 1.0).unwrap();
        graph.insert_edge(1, 2, 1.0).unwrap();

        let mut hierarchy = ClusterHierarchy::new(Arc::clone(&graph));
        let before = hierarchy.min_boundary();

        // Add edge to form cycle
        let edge_id = graph.insert_edge(0, 2, 1.0).unwrap();
        hierarchy.insert_edge(edge_id, 0, 2);

        let after = hierarchy.min_boundary();
        assert!(after >= before); // Adding edge can only increase min cut
    }
}
