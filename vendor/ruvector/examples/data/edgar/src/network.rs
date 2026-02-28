//! Peer network construction for financial coherence analysis

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{Company, Sector};

/// A company node in the peer network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyNode {
    /// Company CIK
    pub cik: String,

    /// Company name
    pub name: String,

    /// Ticker symbol
    pub ticker: Option<String>,

    /// Sector
    pub sector: Sector,

    /// Market cap (if known)
    pub market_cap: Option<f64>,

    /// Number of peer connections
    pub peer_count: usize,

    /// Average peer similarity
    pub avg_peer_similarity: f64,
}

/// An edge between peer companies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerEdge {
    /// Source company CIK
    pub source: String,

    /// Target company CIK
    pub target: String,

    /// Similarity score (0-1)
    pub similarity: f64,

    /// Relationship type
    pub relationship_type: PeerRelationType,

    /// Edge weight for min-cut
    pub weight: f64,

    /// Evidence for relationship
    pub evidence: Vec<String>,
}

/// Type of peer relationship
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerRelationType {
    /// Same sector/industry
    SameSector,
    /// Shared institutional investors
    SharedInvestors,
    /// Similar size (market cap)
    SimilarSize,
    /// Supply chain relationship
    SupplyChain,
    /// Competitor
    Competitor,
    /// Multiple relationship types
    Multiple,
}

/// Peer network graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerNetwork {
    /// Network identifier
    pub id: String,

    /// Nodes (companies)
    pub nodes: HashMap<String, CompanyNode>,

    /// Edges (peer relationships)
    pub edges: Vec<PeerEdge>,

    /// Creation time
    pub created_at: DateTime<Utc>,

    /// Network statistics
    pub stats: NetworkStats,
}

/// Network statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkStats {
    /// Number of nodes
    pub node_count: usize,

    /// Number of edges
    pub edge_count: usize,

    /// Average similarity
    pub avg_similarity: f64,

    /// Network density
    pub density: f64,

    /// Average degree
    pub avg_degree: f64,

    /// Number of connected components
    pub num_components: usize,

    /// Computed min-cut value
    pub min_cut_value: Option<f64>,
}

impl PeerNetwork {
    /// Create an empty network
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            nodes: HashMap::new(),
            edges: Vec::new(),
            created_at: Utc::now(),
            stats: NetworkStats::default(),
        }
    }

    /// Add a company node
    pub fn add_node(&mut self, node: CompanyNode) {
        self.nodes.insert(node.cik.clone(), node);
        self.update_stats();
    }

    /// Add a peer edge
    pub fn add_edge(&mut self, edge: PeerEdge) {
        self.edges.push(edge);
        self.update_stats();
    }

    /// Get a node by CIK
    pub fn get_node(&self, cik: &str) -> Option<&CompanyNode> {
        self.nodes.get(cik)
    }

    /// Get peer CIKs for a company
    pub fn get_peers(&self, cik: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter_map(|e| {
                if e.source == cik {
                    Some(e.target.as_str())
                } else if e.target == cik {
                    Some(e.source.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get edges for a company
    pub fn get_edges_for_company(&self, cik: &str) -> Vec<&PeerEdge> {
        self.edges
            .iter()
            .filter(|e| e.source == cik || e.target == cik)
            .collect()
    }

    /// Update statistics
    fn update_stats(&mut self) {
        self.stats.node_count = self.nodes.len();
        self.stats.edge_count = self.edges.len();

        if !self.edges.is_empty() {
            self.stats.avg_similarity = self.edges.iter().map(|e| e.similarity).sum::<f64>()
                / self.edges.len() as f64;
        }

        let max_edges = if self.nodes.len() > 1 {
            self.nodes.len() * (self.nodes.len() - 1) / 2
        } else {
            1
        };
        self.stats.density = self.edges.len() as f64 / max_edges as f64;

        if !self.nodes.is_empty() {
            self.stats.avg_degree = (2 * self.edges.len()) as f64 / self.nodes.len() as f64;
        }
    }

    /// Convert to format for RuVector min-cut
    pub fn to_mincut_edges(&self) -> Vec<(u64, u64, f64)> {
        let mut node_ids: HashMap<&str, u64> = HashMap::new();
        let mut next_id = 0u64;

        for cik in self.nodes.keys() {
            node_ids.insert(cik.as_str(), next_id);
            next_id += 1;
        }

        self.edges
            .iter()
            .filter_map(|e| {
                let src_id = node_ids.get(e.source.as_str())?;
                let tgt_id = node_ids.get(e.target.as_str())?;
                Some((*src_id, *tgt_id, e.weight))
            })
            .collect()
    }

    /// Get node ID mapping
    pub fn node_id_mapping(&self) -> HashMap<u64, String> {
        let mut mapping = HashMap::new();
        for (i, cik) in self.nodes.keys().enumerate() {
            mapping.insert(i as u64, cik.clone());
        }
        mapping
    }
}

/// Builder for peer networks
pub struct PeerNetworkBuilder {
    id: String,
    companies: Vec<Company>,
    min_similarity: f64,
    max_peers: usize,
    relationship_types: Vec<PeerRelationType>,
}

impl PeerNetworkBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            id: format!("network_{}", Utc::now().timestamp()),
            companies: Vec::new(),
            min_similarity: 0.3,
            max_peers: 20,
            relationship_types: vec![
                PeerRelationType::SameSector,
                PeerRelationType::SimilarSize,
            ],
        }
    }

    /// Set network ID
    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    /// Add companies
    pub fn add_companies(mut self, companies: Vec<Company>) -> Self {
        self.companies.extend(companies);
        self
    }

    /// Set minimum similarity threshold
    pub fn min_similarity(mut self, min: f64) -> Self {
        self.min_similarity = min;
        self
    }

    /// Set maximum peers per company
    pub fn max_peers(mut self, max: usize) -> Self {
        self.max_peers = max;
        self
    }

    /// Set relationship types to consider
    pub fn relationship_types(mut self, types: Vec<PeerRelationType>) -> Self {
        self.relationship_types = types;
        self
    }

    /// Build the network
    pub fn build(self) -> PeerNetwork {
        let mut network = PeerNetwork::new(&self.id);

        // Add nodes
        for company in &self.companies {
            let sector = company.sic_code
                .as_ref()
                .map(|s| Sector::from_sic(s))
                .unwrap_or(Sector::Other);

            let node = CompanyNode {
                cik: company.cik.clone(),
                name: company.name.clone(),
                ticker: company.ticker.clone(),
                sector,
                market_cap: None,
                peer_count: 0,
                avg_peer_similarity: 0.0,
            };

            network.add_node(node);
        }

        // Add edges based on relationships
        for i in 0..self.companies.len() {
            for j in (i + 1)..self.companies.len() {
                let company_i = &self.companies[i];
                let company_j = &self.companies[j];

                let (similarity, rel_type) = self.compute_similarity(company_i, company_j);

                if similarity >= self.min_similarity {
                    let edge = PeerEdge {
                        source: company_i.cik.clone(),
                        target: company_j.cik.clone(),
                        similarity,
                        relationship_type: rel_type,
                        weight: similarity,
                        evidence: self.collect_evidence(company_i, company_j),
                    };

                    network.add_edge(edge);
                }
            }
        }

        // Update node statistics
        for (cik, node) in network.nodes.iter_mut() {
            let edges = network.edges
                .iter()
                .filter(|e| e.source == *cik || e.target == *cik)
                .collect::<Vec<_>>();

            node.peer_count = edges.len();
            if !edges.is_empty() {
                node.avg_peer_similarity = edges.iter().map(|e| e.similarity).sum::<f64>()
                    / edges.len() as f64;
            }
        }

        network
    }

    /// Compute similarity between two companies
    fn compute_similarity(&self, a: &Company, b: &Company) -> (f64, PeerRelationType) {
        let mut total_similarity = 0.0;
        let mut relationship_count = 0;
        let mut rel_type = PeerRelationType::SameSector;

        // Sector similarity
        if self.relationship_types.contains(&PeerRelationType::SameSector) {
            let sector_a = a.sic_code.as_ref().map(|s| Sector::from_sic(s));
            let sector_b = b.sic_code.as_ref().map(|s| Sector::from_sic(s));

            if sector_a.is_some() && sector_a == sector_b {
                total_similarity += 0.5;
                relationship_count += 1;
            } else if a.sic_code.is_some() && b.sic_code.is_some() {
                // Same SIC division (first digit)
                let sic_a = a.sic_code.as_ref().unwrap();
                let sic_b = b.sic_code.as_ref().unwrap();
                if !sic_a.is_empty() && !sic_b.is_empty() &&
                   sic_a.chars().next() == sic_b.chars().next() {
                    total_similarity += 0.3;
                    relationship_count += 1;
                }
            }
        }

        // Same state
        if a.state.is_some() && a.state == b.state {
            total_similarity += 0.2;
            relationship_count += 1;
        }

        let similarity = if relationship_count > 0 {
            total_similarity / relationship_count as f64
        } else {
            0.0
        };

        if relationship_count > 1 {
            rel_type = PeerRelationType::Multiple;
        }

        (similarity, rel_type)
    }

    /// Collect evidence for relationship
    fn collect_evidence(&self, a: &Company, b: &Company) -> Vec<String> {
        let mut evidence = Vec::new();

        let sector_a = a.sic_code.as_ref().map(|s| Sector::from_sic(s));
        let sector_b = b.sic_code.as_ref().map(|s| Sector::from_sic(s));

        if sector_a.is_some() && sector_a == sector_b {
            evidence.push(format!("Same sector: {:?}", sector_a.unwrap()));
        }

        if a.state.is_some() && a.state == b.state {
            evidence.push(format!("Same state: {}", a.state.as_ref().unwrap()));
        }

        evidence
    }
}

impl Default for PeerNetworkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_network() {
        let network = PeerNetwork::new("test");
        assert_eq!(network.stats.node_count, 0);
        assert_eq!(network.stats.edge_count, 0);
    }

    #[test]
    fn test_builder() {
        let builder = PeerNetworkBuilder::new()
            .min_similarity(0.5)
            .max_peers(10);

        let network = builder.build();
        assert!(network.nodes.is_empty());
    }

    #[test]
    fn test_get_peers() {
        let mut network = PeerNetwork::new("test");

        network.add_node(CompanyNode {
            cik: "A".to_string(),
            name: "Company A".to_string(),
            ticker: None,
            sector: Sector::Technology,
            market_cap: None,
            peer_count: 0,
            avg_peer_similarity: 0.0,
        });

        network.add_node(CompanyNode {
            cik: "B".to_string(),
            name: "Company B".to_string(),
            ticker: None,
            sector: Sector::Technology,
            market_cap: None,
            peer_count: 0,
            avg_peer_similarity: 0.0,
        });

        network.add_edge(PeerEdge {
            source: "A".to_string(),
            target: "B".to_string(),
            similarity: 0.8,
            relationship_type: PeerRelationType::SameSector,
            weight: 0.8,
            evidence: vec![],
        });

        let peers = network.get_peers("A");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0], "B");
    }
}
