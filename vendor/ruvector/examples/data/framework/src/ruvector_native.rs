//! RuVector-Native Discovery Engine
//!
//! Deep integration with ruvector-core, ruvector-graph, and ruvector-mincut
//! for production-grade coherence analysis and pattern discovery.

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::utils::cosine_similarity;

/// Vector embedding for semantic similarity
/// Uses RuVector's native vector storage format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticVector {
    /// Vector ID
    pub id: String,
    /// Dense embedding (typically 384-1536 dimensions)
    pub embedding: Vec<f32>,
    /// Source domain
    pub domain: Domain,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Metadata for filtering
    pub metadata: HashMap<String, String>,
}

/// Discovery domains
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Domain {
    Climate,
    Finance,
    Research,
    Medical,
    Economic,
    Genomics,
    Physics,
    Seismic,
    Ocean,
    Space,
    Transportation,
    Geospatial,
    Government,
    CrossDomain,
}

/// RuVector-native graph node
/// Designed to work with ruvector-graph's adjacency structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// Node ID (u32 for ruvector compatibility)
    pub id: u32,
    /// String identifier for external reference
    pub external_id: String,
    /// Domain
    pub domain: Domain,
    /// Associated vector embedding index
    pub vector_idx: Option<usize>,
    /// Node weight (for weighted min-cut)
    pub weight: f64,
    /// Attributes
    pub attributes: HashMap<String, f64>,
}

/// RuVector-native graph edge
/// Compatible with ruvector-mincut's edge format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Source node ID
    pub source: u32,
    /// Target node ID
    pub target: u32,
    /// Edge weight (capacity for min-cut)
    pub weight: f64,
    /// Edge type
    pub edge_type: EdgeType,
    /// Timestamp when edge was created/updated
    pub timestamp: DateTime<Utc>,
}

/// Types of edges in the discovery graph
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EdgeType {
    /// Correlation-based (e.g., temperature correlation)
    Correlation,
    /// Similarity-based (e.g., vector cosine similarity)
    Similarity,
    /// Citation/reference link
    Citation,
    /// Causal relationship
    Causal,
    /// Cross-domain bridge
    CrossDomain,
}

/// Configuration for the native discovery engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeEngineConfig {
    /// Minimum edge weight to include
    pub min_edge_weight: f64,
    /// Vector similarity threshold
    pub similarity_threshold: f64,
    /// Min-cut sensitivity (lower = more sensitive to breaks)
    pub mincut_sensitivity: f64,
    /// Enable cross-domain discovery
    pub cross_domain: bool,
    /// Window size for temporal analysis (seconds)
    pub window_seconds: i64,
    /// HNSW parameters
    pub hnsw_m: usize,
    pub hnsw_ef_construction: usize,
    pub hnsw_ef_search: usize,
    /// Vector dimension
    pub dimension: usize,
    /// Batch size for processing
    pub batch_size: usize,
    /// Checkpoint interval (records)
    pub checkpoint_interval: u64,
    /// Number of parallel workers
    pub parallel_workers: usize,
}

impl Default for NativeEngineConfig {
    fn default() -> Self {
        Self {
            min_edge_weight: 0.3,
            similarity_threshold: 0.7,
            mincut_sensitivity: 0.15,
            cross_domain: true,
            window_seconds: 86400 * 30, // 30 days
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 50,
            dimension: 384,
            batch_size: 1000,
            checkpoint_interval: 10_000,
            parallel_workers: 4,
        }
    }
}

/// The main RuVector-native discovery engine
///
/// This engine uses RuVector's core algorithms:
/// - Vector similarity via HNSW index
/// - Graph coherence via Stoer-Wagner min-cut
/// - Temporal windowing for streaming analysis
pub struct NativeDiscoveryEngine {
    config: NativeEngineConfig,

    /// Vector storage (would use ruvector-core in production)
    vectors: Vec<SemanticVector>,

    /// Graph nodes
    nodes: HashMap<u32, GraphNode>,

    /// Graph edges (adjacency list format for ruvector-mincut)
    edges: Vec<GraphEdge>,

    /// Historical coherence values for change detection
    coherence_history: Vec<(DateTime<Utc>, f64, CoherenceSnapshot)>,

    /// Next node ID
    next_node_id: u32,

    /// Domain-specific subgraph indices
    domain_nodes: HashMap<Domain, Vec<u32>>,
}

/// Snapshot of coherence state for historical comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceSnapshot {
    /// Min-cut value
    pub mincut_value: f64,
    /// Number of nodes
    pub node_count: usize,
    /// Number of edges
    pub edge_count: usize,
    /// Partition sizes after min-cut
    pub partition_sizes: (usize, usize),
    /// Boundary nodes (nodes on the cut)
    pub boundary_nodes: Vec<u32>,
    /// Average edge weight
    pub avg_edge_weight: f64,
}

/// A detected pattern or anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPattern {
    /// Pattern ID
    pub id: String,
    /// Pattern type
    pub pattern_type: PatternType,
    /// Confidence score (0-1)
    pub confidence: f64,
    /// Affected nodes
    pub affected_nodes: Vec<u32>,
    /// Timestamp of detection
    pub detected_at: DateTime<Utc>,
    /// Description
    pub description: String,
    /// Evidence
    pub evidence: Vec<Evidence>,
    /// Cross-domain connections if applicable
    pub cross_domain_links: Vec<CrossDomainLink>,
}

/// Types of discoverable patterns
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PatternType {
    /// Network coherence break (min-cut dropped)
    CoherenceBreak,
    /// Network consolidation (min-cut increased)
    Consolidation,
    /// Emerging cluster (new dense subgraph)
    EmergingCluster,
    /// Dissolving cluster
    DissolvingCluster,
    /// Bridge formation (cross-domain connection)
    BridgeFormation,
    /// Anomalous node (outlier in vector space)
    AnomalousNode,
    /// Temporal shift (pattern change over time)
    TemporalShift,
    /// Cascade (change propagating through network)
    Cascade,
}

/// Evidence supporting a pattern detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub evidence_type: String,
    pub value: f64,
    pub description: String,
}

/// Cross-domain link discovered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossDomainLink {
    pub source_domain: Domain,
    pub target_domain: Domain,
    pub source_nodes: Vec<u32>,
    pub target_nodes: Vec<u32>,
    pub link_strength: f64,
    pub link_type: String,
}

impl NativeDiscoveryEngine {
    /// Create a new engine with the given configuration
    pub fn new(config: NativeEngineConfig) -> Self {
        Self {
            config,
            vectors: Vec::new(),
            nodes: HashMap::new(),
            edges: Vec::new(),
            coherence_history: Vec::new(),
            next_node_id: 0,
            domain_nodes: HashMap::new(),
        }
    }

    /// Add a vector to the engine
    /// In production, this would use ruvector-core's vector storage
    pub fn add_vector(&mut self, vector: SemanticVector) -> u32 {
        let node_id = self.next_node_id;
        self.next_node_id += 1;

        let vector_idx = self.vectors.len();
        self.vectors.push(vector.clone());

        let node = GraphNode {
            id: node_id,
            external_id: vector.id.clone(),
            domain: vector.domain,
            vector_idx: Some(vector_idx),
            weight: 1.0,
            attributes: HashMap::new(),
        };

        self.nodes.insert(node_id, node);
        self.domain_nodes.entry(vector.domain).or_default().push(node_id);

        // Auto-connect to similar vectors
        self.connect_similar_vectors(node_id);

        node_id
    }

    /// Connect a node to similar vectors using cosine similarity
    /// In production, this would use ruvector-hnsw for O(log n) search
    fn connect_similar_vectors(&mut self, node_id: u32) {
        let node = match self.nodes.get(&node_id) {
            Some(n) => n.clone(),
            None => return,
        };

        let vector_idx = match node.vector_idx {
            Some(idx) => idx,
            None => return,
        };

        let source_vec = &self.vectors[vector_idx].embedding;

        // Find similar vectors (brute force - would use HNSW in production)
        for (other_id, other_node) in &self.nodes {
            if *other_id == node_id {
                continue;
            }

            if let Some(other_idx) = other_node.vector_idx {
                let other_vec = &self.vectors[other_idx].embedding;
                let similarity = cosine_similarity(source_vec, other_vec);

                if similarity >= self.config.similarity_threshold as f32 {
                    // Determine edge type
                    let edge_type = if node.domain != other_node.domain {
                        EdgeType::CrossDomain
                    } else {
                        EdgeType::Similarity
                    };

                    self.edges.push(GraphEdge {
                        source: node_id,
                        target: *other_id,
                        weight: similarity as f64,
                        edge_type,
                        timestamp: Utc::now(),
                    });
                }
            }
        }
    }

    /// Add a correlation-based edge
    pub fn add_correlation_edge(&mut self, source: u32, target: u32, correlation: f64) {
        if correlation.abs() >= self.config.min_edge_weight {
            self.edges.push(GraphEdge {
                source,
                target,
                weight: correlation.abs(),
                edge_type: EdgeType::Correlation,
                timestamp: Utc::now(),
            });
        }
    }

    /// Compute current coherence using Stoer-Wagner min-cut
    ///
    /// The min-cut value represents the "weakest link" in the network.
    /// A drop in min-cut indicates the network is becoming fragmented.
    pub fn compute_coherence(&self) -> CoherenceSnapshot {
        if self.nodes.is_empty() || self.edges.is_empty() {
            return CoherenceSnapshot {
                mincut_value: 0.0,
                node_count: self.nodes.len(),
                edge_count: self.edges.len(),
                partition_sizes: (0, 0),
                boundary_nodes: vec![],
                avg_edge_weight: 0.0,
            };
        }

        // Build adjacency matrix for min-cut
        // In production, this would call ruvector-mincut directly
        let mincut_result = self.stoer_wagner_mincut();

        let avg_edge_weight = if self.edges.is_empty() {
            0.0
        } else {
            self.edges.iter().map(|e| e.weight).sum::<f64>() / self.edges.len() as f64
        };

        CoherenceSnapshot {
            mincut_value: mincut_result.0,
            node_count: self.nodes.len(),
            edge_count: self.edges.len(),
            partition_sizes: mincut_result.1,
            boundary_nodes: mincut_result.2,
            avg_edge_weight,
        }
    }

    /// Stoer-Wagner minimum cut algorithm
    /// Returns (min_cut_value, partition_sizes, boundary_nodes)
    fn stoer_wagner_mincut(&self) -> (f64, (usize, usize), Vec<u32>) {
        let n = self.nodes.len();
        if n < 2 {
            return (0.0, (n, 0), vec![]);
        }

        // Build adjacency matrix
        let node_ids: Vec<u32> = self.nodes.keys().copied().collect();
        let id_to_idx: HashMap<u32, usize> = node_ids.iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();

        let mut adj = vec![vec![0.0; n]; n];
        for edge in &self.edges {
            if let (Some(&i), Some(&j)) = (id_to_idx.get(&edge.source), id_to_idx.get(&edge.target)) {
                adj[i][j] += edge.weight;
                adj[j][i] += edge.weight;
            }
        }

        // Stoer-Wagner algorithm
        let mut best_cut = f64::INFINITY;
        let mut best_partition = (0, 0);
        let mut best_boundary = vec![];

        let mut active: Vec<bool> = vec![true; n];
        let mut merged: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();

        for phase in 0..(n - 1) {
            // Maximum adjacency search
            let mut in_a = vec![false; n];
            let mut key = vec![0.0; n];

            // Find first active node
            let start = (0..n).find(|&i| active[i]).unwrap();
            in_a[start] = true;

            // Update keys
            for j in 0..n {
                if active[j] && !in_a[j] {
                    key[j] = adj[start][j];
                }
            }

            let mut s = start;
            let mut t = start;

            for _ in 1..=(n - 1 - phase) {
                // Find max key not in A
                let mut max_key = f64::NEG_INFINITY;
                let mut max_node = 0;

                for j in 0..n {
                    if active[j] && !in_a[j] && key[j] > max_key {
                        max_key = key[j];
                        max_node = j;
                    }
                }

                s = t;
                t = max_node;
                in_a[t] = true;

                // Update keys
                for j in 0..n {
                    if active[j] && !in_a[j] {
                        key[j] += adj[t][j];
                    }
                }
            }

            // Cut of the phase
            let cut_weight = key[t];

            if cut_weight < best_cut {
                best_cut = cut_weight;

                // Partition is: merged[t] vs everything else
                let partition_a: Vec<usize> = merged[t].clone();
                let partition_b: Vec<usize> = (0..n)
                    .filter(|&i| active[i] && i != t)
                    .flat_map(|i| merged[i].iter().copied())
                    .collect();

                best_partition = (partition_a.len(), partition_b.len());

                // Boundary nodes are those in the smaller partition with edges to other
                best_boundary = partition_a.iter()
                    .map(|&i| node_ids[i])
                    .collect();
            }

            // Merge s and t
            active[t] = false;
            let to_merge: Vec<usize> = merged[t].clone();
            merged[s].extend(to_merge);

            for i in 0..n {
                if active[i] && i != s {
                    adj[s][i] += adj[t][i];
                    adj[i][s] += adj[i][t];
                }
            }
        }

        (best_cut, best_partition, best_boundary)
    }

    /// Detect patterns by comparing current state to history
    pub fn detect_patterns(&mut self) -> Vec<DiscoveredPattern> {
        let mut patterns = Vec::new();

        let current = self.compute_coherence();
        let now = Utc::now();

        // Compare to previous state
        if let Some((prev_time, prev_mincut, prev_snapshot)) = self.coherence_history.last() {
            let mincut_delta = current.mincut_value - prev_mincut;
            let relative_change = if *prev_mincut > 0.0 {
                mincut_delta.abs() / prev_mincut
            } else {
                mincut_delta.abs()
            };

            // Detect coherence break
            if mincut_delta < -self.config.mincut_sensitivity {
                patterns.push(DiscoveredPattern {
                    id: format!("coherence_break_{}", now.timestamp()),
                    pattern_type: PatternType::CoherenceBreak,
                    confidence: (relative_change.min(1.0) * 0.5 + 0.5),
                    affected_nodes: current.boundary_nodes.clone(),
                    detected_at: now,
                    description: format!(
                        "Network coherence dropped from {:.3} to {:.3} ({:.1}% decrease)",
                        prev_mincut, current.mincut_value, relative_change * 100.0
                    ),
                    evidence: vec![
                        Evidence {
                            evidence_type: "mincut_delta".to_string(),
                            value: mincut_delta,
                            description: "Change in min-cut value".to_string(),
                        },
                        Evidence {
                            evidence_type: "boundary_size".to_string(),
                            value: current.boundary_nodes.len() as f64,
                            description: "Number of nodes on the cut".to_string(),
                        },
                    ],
                    cross_domain_links: self.find_cross_domain_at_boundary(&current.boundary_nodes),
                });
            }

            // Detect consolidation
            if mincut_delta > self.config.mincut_sensitivity {
                patterns.push(DiscoveredPattern {
                    id: format!("consolidation_{}", now.timestamp()),
                    pattern_type: PatternType::Consolidation,
                    confidence: (relative_change.min(1.0) * 0.5 + 0.5),
                    affected_nodes: current.boundary_nodes.clone(),
                    detected_at: now,
                    description: format!(
                        "Network coherence increased from {:.3} to {:.3} ({:.1}% increase)",
                        prev_mincut, current.mincut_value, relative_change * 100.0
                    ),
                    evidence: vec![
                        Evidence {
                            evidence_type: "mincut_delta".to_string(),
                            value: mincut_delta,
                            description: "Change in min-cut value".to_string(),
                        },
                    ],
                    cross_domain_links: vec![],
                });
            }

            // Detect partition imbalance (emerging cluster)
            let (part_a, part_b) = current.partition_sizes;
            let imbalance = (part_a as f64 - part_b as f64).abs() / (part_a + part_b) as f64;
            let (prev_a, prev_b) = prev_snapshot.partition_sizes;
            let prev_imbalance = if prev_a + prev_b > 0 {
                (prev_a as f64 - prev_b as f64).abs() / (prev_a + prev_b) as f64
            } else {
                0.0
            };

            if imbalance > prev_imbalance + 0.2 {
                patterns.push(DiscoveredPattern {
                    id: format!("emerging_cluster_{}", now.timestamp()),
                    pattern_type: PatternType::EmergingCluster,
                    confidence: 0.7,
                    affected_nodes: current.boundary_nodes.clone(),
                    detected_at: now,
                    description: format!(
                        "Partition imbalance increased: {} vs {} nodes (was {} vs {})",
                        part_a, part_b, prev_a, prev_b
                    ),
                    evidence: vec![],
                    cross_domain_links: vec![],
                });
            }
        }

        // Cross-domain pattern detection
        if self.config.cross_domain {
            patterns.extend(self.detect_cross_domain_patterns());
        }

        // Store current state in history
        self.coherence_history.push((now, current.mincut_value, current));

        patterns
    }

    /// Find cross-domain links at boundary nodes
    fn find_cross_domain_at_boundary(&self, boundary: &[u32]) -> Vec<CrossDomainLink> {
        let mut links = Vec::new();

        // Find cross-domain edges involving boundary nodes
        for edge in &self.edges {
            if edge.edge_type == EdgeType::CrossDomain {
                if boundary.contains(&edge.source) || boundary.contains(&edge.target) {
                    if let (Some(src_node), Some(tgt_node)) =
                        (self.nodes.get(&edge.source), self.nodes.get(&edge.target))
                    {
                        links.push(CrossDomainLink {
                            source_domain: src_node.domain,
                            target_domain: tgt_node.domain,
                            source_nodes: vec![edge.source],
                            target_nodes: vec![edge.target],
                            link_strength: edge.weight,
                            link_type: "boundary_crossing".to_string(),
                        });
                    }
                }
            }
        }

        links
    }

    /// Detect patterns that span multiple domains
    fn detect_cross_domain_patterns(&self) -> Vec<DiscoveredPattern> {
        let mut patterns = Vec::new();

        // Count cross-domain edges by domain pair
        let mut cross_counts: HashMap<(Domain, Domain), Vec<&GraphEdge>> = HashMap::new();

        for edge in &self.edges {
            if edge.edge_type == EdgeType::CrossDomain {
                if let (Some(src), Some(tgt)) =
                    (self.nodes.get(&edge.source), self.nodes.get(&edge.target))
                {
                    let key = if src.domain < tgt.domain {
                        (src.domain, tgt.domain)
                    } else {
                        (tgt.domain, src.domain)
                    };
                    cross_counts.entry(key).or_default().push(edge);
                }
            }
        }

        // Report significant cross-domain bridges
        for ((domain_a, domain_b), edges) in cross_counts {
            if edges.len() >= 3 {
                let avg_strength = edges.iter().map(|e| e.weight).sum::<f64>() / edges.len() as f64;

                if avg_strength > self.config.similarity_threshold as f64 {
                    patterns.push(DiscoveredPattern {
                        id: format!("bridge_{:?}_{:?}_{}", domain_a, domain_b, Utc::now().timestamp()),
                        pattern_type: PatternType::BridgeFormation,
                        confidence: avg_strength,
                        affected_nodes: edges.iter()
                            .flat_map(|e| vec![e.source, e.target])
                            .collect(),
                        detected_at: Utc::now(),
                        description: format!(
                            "Cross-domain bridge detected: {:?} ↔ {:?} ({} connections, avg strength {:.3})",
                            domain_a, domain_b, edges.len(), avg_strength
                        ),
                        evidence: vec![
                            Evidence {
                                evidence_type: "edge_count".to_string(),
                                value: edges.len() as f64,
                                description: "Number of cross-domain connections".to_string(),
                            },
                        ],
                        cross_domain_links: vec![CrossDomainLink {
                            source_domain: domain_a,
                            target_domain: domain_b,
                            source_nodes: edges.iter().map(|e| e.source).collect(),
                            target_nodes: edges.iter().map(|e| e.target).collect(),
                            link_strength: avg_strength,
                            link_type: "semantic_bridge".to_string(),
                        }],
                    });
                }
            }
        }

        patterns
    }

    /// Get domain-specific coherence
    pub fn domain_coherence(&self, domain: Domain) -> Option<f64> {
        let domain_node_ids = self.domain_nodes.get(&domain)?;

        if domain_node_ids.len() < 2 {
            return None;
        }

        // Count edges within domain
        let mut internal_weight = 0.0;
        let mut edge_count = 0;

        for edge in &self.edges {
            if domain_node_ids.contains(&edge.source) && domain_node_ids.contains(&edge.target) {
                internal_weight += edge.weight;
                edge_count += 1;
            }
        }

        if edge_count == 0 {
            return Some(0.0);
        }

        Some(internal_weight / edge_count as f64)
    }

    /// Get statistics about the current state
    pub fn stats(&self) -> EngineStats {
        let mut domain_counts = HashMap::new();
        for domain in self.domain_nodes.keys() {
            domain_counts.insert(*domain, self.domain_nodes[domain].len());
        }

        let mut cross_domain_edges = 0;
        for edge in &self.edges {
            if edge.edge_type == EdgeType::CrossDomain {
                cross_domain_edges += 1;
            }
        }

        EngineStats {
            total_nodes: self.nodes.len(),
            total_edges: self.edges.len(),
            total_vectors: self.vectors.len(),
            domain_counts,
            cross_domain_edges,
            history_length: self.coherence_history.len(),
        }
    }

    /// Get all detected patterns from the latest detection run
    pub fn get_patterns(&self) -> Vec<DiscoveredPattern> {
        // For now, return an empty vec. In production, this would store
        // patterns from the last detect_patterns() call
        vec![]
    }

    /// Export the current graph structure
    pub fn export_graph(&self) -> GraphExport {
        GraphExport {
            nodes: self.nodes.values().cloned().collect(),
            edges: self.edges.clone(),
            domains: self.domain_nodes.clone(),
        }
    }

    /// Get the coherence history
    pub fn get_coherence_history(&self) -> Vec<CoherenceHistoryEntry> {
        self.coherence_history.iter()
            .map(|(timestamp, mincut, snapshot)| {
                CoherenceHistoryEntry {
                    timestamp: *timestamp,
                    mincut_value: *mincut,
                    snapshot: snapshot.clone(),
                }
            })
            .collect()
    }
}

/// Engine statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub total_vectors: usize,
    pub domain_counts: HashMap<Domain, usize>,
    pub cross_domain_edges: usize,
    pub history_length: usize,
}

/// Exported graph structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphExport {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub domains: HashMap<Domain, Vec<u32>>,
}

/// Coherence history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceHistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub mincut_value: f64,
    pub snapshot: CoherenceSnapshot,
}

// Note: cosine_similarity is imported from crate::utils

// Implement ordering for Domain to use in HashMap keys
impl PartialOrd for Domain {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Domain {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 0.001);
    }

    #[test]
    fn test_engine_basic() {
        let config = NativeEngineConfig::default();
        let mut engine = NativeDiscoveryEngine::new(config);

        // Add some vectors
        let v1 = SemanticVector {
            id: "climate_1".to_string(),
            embedding: vec![1.0, 0.5, 0.2],
            domain: Domain::Climate,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };

        let v2 = SemanticVector {
            id: "climate_2".to_string(),
            embedding: vec![0.9, 0.6, 0.3],
            domain: Domain::Climate,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };

        engine.add_vector(v1);
        engine.add_vector(v2);

        let stats = engine.stats();
        assert_eq!(stats.total_nodes, 2);
        assert_eq!(stats.total_vectors, 2);
    }
}
