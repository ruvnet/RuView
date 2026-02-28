//! Research frontier detection using coherence signals

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{TopicEdge, TopicGraph, TopicNode, Work};

/// An emerging research frontier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergingFrontier {
    /// Frontier identifier
    pub id: String,

    /// Primary topic name
    pub name: String,

    /// Related topic names
    pub related_topics: Vec<String>,

    /// Growth rate (works per year)
    pub growth_rate: f64,

    /// Coherence delta (change in min-cut boundary)
    pub coherence_delta: f64,

    /// Citation momentum (trend in citation rates)
    pub citation_momentum: f64,

    /// Detected boundary nodes (topics at the frontier edge)
    pub boundary_topics: Vec<String>,

    /// First detected
    pub detected_at: DateTime<Utc>,

    /// Confidence score (0-1)
    pub confidence: f64,

    /// Evidence supporting this frontier
    pub evidence: Vec<FrontierEvidence>,
}

/// Evidence for a frontier detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontierEvidence {
    /// Evidence type
    pub evidence_type: String,

    /// Value
    pub value: f64,

    /// Explanation
    pub explanation: String,
}

/// A cross-domain bridge connecting two research areas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossDomainBridge {
    /// Bridge identifier
    pub id: String,

    /// Source domain/topic
    pub source_domain: String,

    /// Target domain/topic
    pub target_domain: String,

    /// Bridge topics (connector nodes)
    pub bridge_topics: Vec<String>,

    /// Citation flow (source → target)
    pub citation_flow: f64,

    /// Reverse flow (target → source)
    pub reverse_flow: f64,

    /// Bridge strength (combined normalized flow)
    pub strength: f64,

    /// Is this a new connection?
    pub is_emerging: bool,

    /// First observed
    pub first_observed: DateTime<Utc>,

    /// Key papers establishing the bridge
    pub key_works: Vec<String>,
}

/// Research frontier radar for detecting emerging fields
pub struct FrontierRadar {
    /// Topic graph snapshots over time
    snapshots: Vec<(DateTime<Utc>, TopicGraph)>,

    /// Minimum growth rate to consider
    min_growth_rate: f64,

    /// Minimum coherence shift to detect
    min_coherence_shift: f64,

    /// Detected frontiers
    frontiers: Vec<EmergingFrontier>,

    /// Detected bridges
    bridges: Vec<CrossDomainBridge>,
}

impl FrontierRadar {
    /// Create a new frontier radar
    pub fn new(min_growth_rate: f64, min_coherence_shift: f64) -> Self {
        Self {
            snapshots: Vec::new(),
            min_growth_rate,
            min_coherence_shift,
            frontiers: Vec::new(),
            bridges: Vec::new(),
        }
    }

    /// Add a topic graph snapshot
    pub fn add_snapshot(&mut self, timestamp: DateTime<Utc>, graph: TopicGraph) {
        self.snapshots.push((timestamp, graph));
        self.snapshots.sort_by_key(|(ts, _)| *ts);
    }

    /// Build snapshots from works partitioned by time
    pub fn build_from_works(&mut self, works: &[Work], window_days: i64) {
        if works.is_empty() {
            return;
        }

        // Find time range
        let mut min_date = Utc::now();
        let mut max_date = DateTime::<Utc>::MIN_UTC;

        for work in works {
            if let Some(date) = work.publication_date {
                if date < min_date {
                    min_date = date;
                }
                if date > max_date {
                    max_date = date;
                }
            }
        }

        // Partition works into time windows
        let window_duration = chrono::Duration::days(window_days);
        let mut current_start = min_date;

        while current_start < max_date {
            let current_end = current_start + window_duration;

            let window_works: Vec<_> = works
                .iter()
                .filter(|w| {
                    w.publication_date
                        .map(|d| d >= current_start && d < current_end)
                        .unwrap_or(false)
                })
                .cloned()
                .collect();

            if !window_works.is_empty() {
                let graph = TopicGraph::from_works(&window_works);
                self.add_snapshot(current_start, graph);
            }

            current_start = current_end;
        }
    }

    /// Detect emerging frontiers from snapshots
    pub fn detect_frontiers(&mut self) -> Vec<EmergingFrontier> {
        if self.snapshots.len() < 2 {
            return vec![];
        }

        let mut frontiers = Vec::new();
        let mut frontier_counter = 0;

        // Compare consecutive snapshots
        for i in 1..self.snapshots.len() {
            let (prev_ts, prev_graph) = &self.snapshots[i - 1];
            let (curr_ts, curr_graph) = &self.snapshots[i];

            // Find topics with significant growth
            for (topic_id, curr_node) in &curr_graph.topics {
                let prev_node = prev_graph.topics.get(topic_id);

                let growth = if let Some(prev) = prev_node {
                    if prev.work_count > 0 {
                        (curr_node.work_count as f64 - prev.work_count as f64)
                            / prev.work_count as f64
                    } else {
                        f64::INFINITY
                    }
                } else {
                    // New topic
                    f64::INFINITY
                };

                if growth > self.min_growth_rate {
                    // Calculate coherence shift
                    let coherence_delta = self.compute_topic_coherence_delta(
                        topic_id,
                        prev_graph,
                        curr_graph,
                    );

                    if coherence_delta.abs() > self.min_coherence_shift {
                        // Calculate citation momentum
                        let citation_momentum = curr_node.avg_citations
                            - prev_node.map(|n| n.avg_citations).unwrap_or(0.0);

                        // Find boundary topics
                        let boundary_topics = self.find_boundary_topics(topic_id, curr_graph);

                        // Build evidence
                        let mut evidence = vec![
                            FrontierEvidence {
                                evidence_type: "growth_rate".to_string(),
                                value: growth,
                                explanation: format!(
                                    "{:.0}% increase in works",
                                    growth * 100.0
                                ),
                            },
                            FrontierEvidence {
                                evidence_type: "coherence_delta".to_string(),
                                value: coherence_delta,
                                explanation: format!(
                                    "Coherence {} by {:.2}",
                                    if coherence_delta > 0.0 {
                                        "increased"
                                    } else {
                                        "decreased"
                                    },
                                    coherence_delta.abs()
                                ),
                            },
                        ];

                        if citation_momentum > 0.0 {
                            evidence.push(FrontierEvidence {
                                evidence_type: "citation_momentum".to_string(),
                                value: citation_momentum,
                                explanation: format!(
                                    "+{:.1} avg citations",
                                    citation_momentum
                                ),
                            });
                        }

                        // Calculate confidence based on evidence strength
                        let confidence = self.calculate_confidence(growth, coherence_delta, citation_momentum);

                        if confidence >= 0.3 {
                            frontiers.push(EmergingFrontier {
                                id: format!("frontier_{}", frontier_counter),
                                name: curr_node.name.clone(),
                                related_topics: self.find_related_topics(topic_id, curr_graph),
                                growth_rate: curr_node.growth_rate,
                                coherence_delta,
                                citation_momentum,
                                boundary_topics,
                                detected_at: *curr_ts,
                                confidence,
                                evidence,
                            });
                            frontier_counter += 1;
                        }
                    }
                }
            }
        }

        // Sort by confidence
        frontiers.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        self.frontiers = frontiers.clone();
        frontiers
    }

    /// Detect cross-domain bridges
    pub fn detect_bridges(&mut self) -> Vec<CrossDomainBridge> {
        if self.snapshots.is_empty() {
            return vec![];
        }

        let mut bridges = Vec::new();
        let mut bridge_counter = 0;

        let (curr_ts, curr_graph) = self.snapshots.last().unwrap();

        // Build domain → topics mapping (simplified: use top-level grouping)
        let mut domain_topics: HashMap<String, Vec<String>> = HashMap::new();
        for (topic_id, node) in &curr_graph.topics {
            // Use first word as domain (simplified)
            let domain = node
                .name
                .split_whitespace()
                .next()
                .unwrap_or("Unknown")
                .to_string();
            domain_topics
                .entry(domain.clone())
                .or_default()
                .push(topic_id.clone());
        }

        // Find cross-domain edges
        let mut domain_flows: HashMap<(String, String), Vec<&TopicEdge>> = HashMap::new();

        for edge in &curr_graph.edges {
            let src_domain = self.get_domain(&edge.source, curr_graph);
            let tgt_domain = self.get_domain(&edge.target, curr_graph);

            if src_domain != tgt_domain {
                domain_flows
                    .entry((src_domain.clone(), tgt_domain.clone()))
                    .or_default()
                    .push(edge);
            }
        }

        // Create bridge records
        for ((src_domain, tgt_domain), edges) in domain_flows {
            let total_flow: f64 = edges.iter().map(|e| e.weight).sum();
            let citation_count: usize = edges.iter().map(|e| e.citation_count).sum();

            if citation_count >= 5 {
                // Minimum threshold
                let bridge_topics: Vec<String> = edges
                    .iter()
                    .flat_map(|e| vec![e.source.clone(), e.target.clone()])
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();

                // Check if this is emerging (compare with previous snapshot)
                let is_emerging = if self.snapshots.len() >= 2 {
                    let (_, prev_graph) = &self.snapshots[self.snapshots.len() - 2];
                    let prev_flow: f64 = prev_graph
                        .edges
                        .iter()
                        .filter(|e| {
                            self.get_domain(&e.source, prev_graph) == src_domain
                                && self.get_domain(&e.target, prev_graph) == tgt_domain
                        })
                        .map(|e| e.weight)
                        .sum();
                    total_flow > prev_flow * 1.5 // 50% growth
                } else {
                    true
                };

                bridges.push(CrossDomainBridge {
                    id: format!("bridge_{}", bridge_counter),
                    source_domain: src_domain.clone(),
                    target_domain: tgt_domain.clone(),
                    bridge_topics,
                    citation_flow: total_flow,
                    reverse_flow: 0.0, // Would need to compute reverse direction
                    strength: total_flow / citation_count as f64,
                    is_emerging,
                    first_observed: *curr_ts,
                    key_works: vec![], // Would need work-level data
                });
                bridge_counter += 1;
            }
        }

        // Sort by strength
        bridges.sort_by(|a, b| {
            b.strength
                .partial_cmp(&a.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        self.bridges = bridges.clone();
        bridges
    }

    /// Compute coherence delta for a topic between snapshots
    fn compute_topic_coherence_delta(
        &self,
        topic_id: &str,
        prev_graph: &TopicGraph,
        curr_graph: &TopicGraph,
    ) -> f64 {
        // Compute local coherence as ratio of intra-topic to inter-topic edges
        let prev_coherence = self.compute_local_coherence(topic_id, prev_graph);
        let curr_coherence = self.compute_local_coherence(topic_id, curr_graph);

        curr_coherence - prev_coherence
    }

    /// Compute local coherence for a topic
    fn compute_local_coherence(&self, topic_id: &str, graph: &TopicGraph) -> f64 {
        // Find edges involving this topic
        let edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.source == topic_id || e.target == topic_id)
            .collect();

        if edges.is_empty() {
            return 0.0;
        }

        // Coherence = sum of weights
        edges.iter().map(|e| e.weight).sum::<f64>() / edges.len() as f64
    }

    /// Find topics at the boundary (connected to other clusters)
    fn find_boundary_topics(&self, topic_id: &str, graph: &TopicGraph) -> Vec<String> {
        // Find topics connected to this topic that have high connectivity elsewhere
        graph
            .edges
            .iter()
            .filter(|e| e.source == topic_id)
            .map(|e| e.target.clone())
            .take(5)
            .collect()
    }

    /// Find related topics
    fn find_related_topics(&self, topic_id: &str, graph: &TopicGraph) -> Vec<String> {
        graph
            .edges
            .iter()
            .filter(|e| e.source == topic_id || e.target == topic_id)
            .flat_map(|e| {
                if e.source == topic_id {
                    vec![e.target.clone()]
                } else {
                    vec![e.source.clone()]
                }
            })
            .take(10)
            .collect()
    }

    /// Get domain for a topic (simplified)
    fn get_domain(&self, topic_id: &str, graph: &TopicGraph) -> String {
        graph
            .topics
            .get(topic_id)
            .map(|n| {
                n.name
                    .split_whitespace()
                    .next()
                    .unwrap_or("Unknown")
                    .to_string()
            })
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// Calculate confidence score
    fn calculate_confidence(
        &self,
        growth: f64,
        coherence_delta: f64,
        citation_momentum: f64,
    ) -> f64 {
        let growth_score = (growth.min(5.0) / 5.0).max(0.0);
        let coherence_score = (coherence_delta.abs().min(1.0)).max(0.0);
        let citation_score = (citation_momentum / 10.0).min(1.0).max(0.0);

        (growth_score * 0.4 + coherence_score * 0.4 + citation_score * 0.2).min(1.0)
    }

    /// Get detected frontiers
    pub fn frontiers(&self) -> &[EmergingFrontier] {
        &self.frontiers
    }

    /// Get detected bridges
    pub fn bridges(&self) -> &[CrossDomainBridge] {
        &self.bridges
    }

    /// Get highest confidence frontiers
    pub fn top_frontiers(&self, n: usize) -> Vec<&EmergingFrontier> {
        self.frontiers.iter().take(n).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontier_radar_creation() {
        let radar = FrontierRadar::new(0.1, 0.2);
        assert!(radar.frontiers().is_empty());
        assert!(radar.bridges().is_empty());
    }

    #[test]
    fn test_confidence_calculation() {
        let radar = FrontierRadar::new(0.1, 0.2);

        // High confidence
        let high = radar.calculate_confidence(2.0, 0.5, 5.0);
        assert!(high > 0.5);

        // Low confidence
        let low = radar.calculate_confidence(0.05, 0.01, 0.1);
        assert!(low < 0.3);
    }
}
