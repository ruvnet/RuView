//! # Federated Coherence Network
//!
//! Distributed coherence-sensing substrates that maintain collective
//! homeostasis across nodes without central coordination.
//!
//! Key concepts:
//! - Consensus through coherence, not voting
//! - Tension propagates across federation boundaries
//! - Patterns learned locally, validated globally
//! - Network-wide instinct alignment
//! - Graceful partition handling
//!
//! This is not distributed computing. This is distributed feeling.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

/// A node in the federated coherence network
pub struct FederatedNode {
    pub id: String,

    /// Local tension level
    tension: f64,

    /// Coherence with each peer
    peer_coherence: HashMap<String, f64>,

    /// Patterns learned locally
    local_patterns: Vec<LearnedPattern>,

    /// Patterns received from federation
    federated_patterns: Vec<FederatedPattern>,

    /// Pending pattern proposals to validate
    pending_proposals: VecDeque<PatternProposal>,

    /// Network partition detector
    partition_detector: PartitionDetector,

    /// Federation configuration
    config: FederationConfig,
}

#[derive(Clone, Debug)]
pub struct LearnedPattern {
    pub signature: Vec<f64>,
    pub response: String,
    pub local_efficacy: f64,
    pub observation_count: usize,
}

#[derive(Clone, Debug)]
pub struct FederatedPattern {
    pub signature: Vec<f64>,
    pub response: String,
    pub originator: String,
    pub global_efficacy: f64,
    pub validations: usize,
    pub rejections: usize,
}

#[derive(Clone, Debug)]
pub struct PatternProposal {
    pub pattern: LearnedPattern,
    pub proposer: String,
    pub timestamp: Instant,
    pub coherence_at_proposal: f64,
}

struct PartitionDetector {
    last_heard: HashMap<String, Instant>,
    partition_threshold: Duration,
    suspected_partitions: HashSet<String>,
}

pub struct FederationConfig {
    /// Minimum local efficacy to propose pattern
    pub proposal_threshold: f64,
    /// Minimum global coherence to accept pattern
    pub acceptance_coherence: f64,
    /// How much peer tension affects local tension
    pub tension_coupling: f64,
    /// Partition detection timeout
    pub partition_timeout: Duration,
    /// Maximum patterns to federate
    pub max_federated_patterns: usize,
}

impl Default for FederationConfig {
    fn default() -> Self {
        Self {
            proposal_threshold: 0.7,
            acceptance_coherence: 0.6,
            tension_coupling: 0.3,
            partition_timeout: Duration::from_secs(30),
            max_federated_patterns: 1000,
        }
    }
}

/// Message types for federation protocol
#[derive(Clone, Debug)]
pub enum FederationMessage {
    /// Heartbeat with current tension
    Heartbeat { tension: f64, pattern_count: usize },

    /// Propose a pattern for federation
    ProposePattern { pattern: LearnedPattern },

    /// Validate a proposed pattern
    ValidatePattern { signature: Vec<f64>, efficacy: f64 },

    /// Reject a proposed pattern
    RejectPattern { signature: Vec<f64>, reason: String },

    /// Tension spike alert
    TensionAlert { severity: f64, source: String },

    /// Request pattern sync
    SyncRequest { since_pattern_count: usize },

    /// Pattern sync response
    SyncResponse { patterns: Vec<FederatedPattern> },
}

/// Result of federation operations
#[derive(Debug)]
pub enum FederationResult {
    /// Pattern accepted into federation
    PatternAccepted { validations: usize },
    /// Pattern rejected by federation
    PatternRejected { rejections: usize, reason: String },
    /// Tension propagated to peers
    TensionPropagated { affected_peers: usize },
    /// Partition detected
    PartitionDetected { isolated_peers: Vec<String> },
    /// Coherence restored after partition
    CoherenceRestored { rejoined_peers: Vec<String> },
}

impl FederatedNode {
    pub fn new(id: &str, config: FederationConfig) -> Self {
        Self {
            id: id.to_string(),
            tension: 0.0,
            peer_coherence: HashMap::new(),
            local_patterns: Vec::new(),
            federated_patterns: Vec::new(),
            pending_proposals: VecDeque::new(),
            partition_detector: PartitionDetector {
                last_heard: HashMap::new(),
                partition_threshold: config.partition_timeout,
                suspected_partitions: HashSet::new(),
            },
            config,
        }
    }

    /// Add a peer to the federation
    pub fn add_peer(&mut self, peer_id: &str) {
        self.peer_coherence.insert(peer_id.to_string(), 1.0);
        self.partition_detector
            .last_heard
            .insert(peer_id.to_string(), Instant::now());
    }

    /// Update local tension and propagate if significant
    pub fn update_tension(&mut self, new_tension: f64) -> Option<FederationMessage> {
        let old_tension = self.tension;
        self.tension = new_tension;

        // Significant spike? Alert federation
        if new_tension - old_tension > 0.3 {
            Some(FederationMessage::TensionAlert {
                severity: new_tension,
                source: self.id.clone(),
            })
        } else {
            None
        }
    }

    /// Learn a pattern locally
    pub fn learn_pattern(&mut self, signature: Vec<f64>, response: String, efficacy: f64) {
        // Check if pattern already exists
        if let Some(existing) = self
            .local_patterns
            .iter_mut()
            .find(|p| Self::signature_match(&p.signature, &signature))
        {
            existing.local_efficacy = existing.local_efficacy * 0.9 + efficacy * 0.1;
            existing.observation_count += 1;
        } else {
            self.local_patterns.push(LearnedPattern {
                signature,
                response,
                local_efficacy: efficacy,
                observation_count: 1,
            });
        }
    }

    /// Propose mature patterns to federation
    pub fn propose_patterns(&self) -> Vec<FederationMessage> {
        self.local_patterns
            .iter()
            .filter(|p| {
                p.local_efficacy >= self.config.proposal_threshold
                    && p.observation_count >= 5
                    && !self.is_already_federated(&p.signature)
            })
            .map(|p| FederationMessage::ProposePattern { pattern: p.clone() })
            .collect()
    }

    /// Handle incoming federation message
    pub fn handle_message(
        &mut self,
        from: &str,
        msg: FederationMessage,
    ) -> Option<FederationMessage> {
        // Update partition detector
        self.partition_detector
            .last_heard
            .insert(from.to_string(), Instant::now());
        self.partition_detector.suspected_partitions.remove(from);

        match msg {
            FederationMessage::Heartbeat {
                tension,
                pattern_count: _,
            } => {
                // Update peer coherence based on tension similarity
                let tension_diff = (self.tension - tension).abs();
                let coherence = 1.0 - tension_diff;
                self.peer_coherence.insert(from.to_string(), coherence);

                // Couple tension
                self.tension = self.tension * (1.0 - self.config.tension_coupling)
                    + tension * self.config.tension_coupling;

                None
            }

            FederationMessage::ProposePattern { pattern } => {
                // Validate against local experience
                let local_match = self
                    .local_patterns
                    .iter()
                    .find(|p| Self::signature_match(&p.signature, &pattern.signature));

                if let Some(local) = local_match {
                    // We have local evidence - validate or reject
                    if local.local_efficacy >= 0.5 {
                        Some(FederationMessage::ValidatePattern {
                            signature: pattern.signature,
                            efficacy: local.local_efficacy,
                        })
                    } else {
                        Some(FederationMessage::RejectPattern {
                            signature: pattern.signature,
                            reason: format!("Low local efficacy: {:.2}", local.local_efficacy),
                        })
                    }
                } else {
                    // No local evidence - accept if coherence is high
                    if self.peer_coherence.get(from).copied().unwrap_or(0.0)
                        >= self.config.acceptance_coherence
                    {
                        self.pending_proposals.push_back(PatternProposal {
                            pattern,
                            proposer: from.to_string(),
                            timestamp: Instant::now(),
                            coherence_at_proposal: self.federation_coherence(),
                        });
                        Some(FederationMessage::ValidatePattern {
                            signature: self
                                .pending_proposals
                                .back()
                                .unwrap()
                                .pattern
                                .signature
                                .clone(),
                            efficacy: 0.5, // Neutral validation
                        })
                    } else {
                        Some(FederationMessage::RejectPattern {
                            signature: pattern.signature,
                            reason: "Insufficient coherence with proposer".into(),
                        })
                    }
                }
            }

            FederationMessage::ValidatePattern {
                signature,
                efficacy,
            } => {
                // Update federated pattern
                if let Some(fp) = self
                    .federated_patterns
                    .iter_mut()
                    .find(|p| Self::signature_match(&p.signature, &signature))
                {
                    fp.validations += 1;
                    fp.global_efficacy = (fp.global_efficacy * fp.validations as f64 + efficacy)
                        / (fp.validations + 1) as f64;
                }
                None
            }

            FederationMessage::RejectPattern {
                signature,
                reason: _,
            } => {
                if let Some(fp) = self
                    .federated_patterns
                    .iter_mut()
                    .find(|p| Self::signature_match(&p.signature, &signature))
                {
                    fp.rejections += 1;
                }
                None
            }

            FederationMessage::TensionAlert { severity, source } => {
                // Propagate tension through coherence coupling
                let coherence_with_source =
                    self.peer_coherence.get(&source).copied().unwrap_or(0.5);
                let propagated = severity * coherence_with_source * 0.5;
                self.tension = (self.tension + propagated).min(1.0);
                None
            }

            FederationMessage::SyncRequest {
                since_pattern_count,
            } => {
                let patterns: Vec<FederatedPattern> = self
                    .federated_patterns
                    .iter()
                    .skip(since_pattern_count)
                    .cloned()
                    .collect();
                Some(FederationMessage::SyncResponse { patterns })
            }

            FederationMessage::SyncResponse { patterns } => {
                for pattern in patterns {
                    if !self.is_already_federated(&pattern.signature) {
                        self.federated_patterns.push(pattern);
                    }
                }
                None
            }
        }
    }

    /// Check for network partitions
    pub fn detect_partitions(&mut self) -> Vec<String> {
        let now = Instant::now();
        let mut newly_partitioned = Vec::new();

        for (peer, last_heard) in &self.partition_detector.last_heard {
            if now.duration_since(*last_heard) > self.partition_detector.partition_threshold {
                if !self.partition_detector.suspected_partitions.contains(peer) {
                    self.partition_detector
                        .suspected_partitions
                        .insert(peer.clone());
                    newly_partitioned.push(peer.clone());

                    // Reduce coherence with partitioned peer
                    if let Some(c) = self.peer_coherence.get_mut(peer) {
                        *c *= 0.5;
                    }
                }
            }
        }

        newly_partitioned
    }

    /// Get overall federation coherence
    pub fn federation_coherence(&self) -> f64 {
        if self.peer_coherence.is_empty() {
            return 1.0;
        }
        self.peer_coherence.values().sum::<f64>() / self.peer_coherence.len() as f64
    }

    /// Get federation status
    pub fn status(&self) -> FederationStatus {
        FederationStatus {
            node_id: self.id.clone(),
            tension: self.tension,
            federation_coherence: self.federation_coherence(),
            peer_count: self.peer_coherence.len(),
            local_patterns: self.local_patterns.len(),
            federated_patterns: self.federated_patterns.len(),
            partitioned_peers: self.partition_detector.suspected_partitions.len(),
        }
    }

    fn signature_match(a: &[f64], b: &[f64]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let diff: f64 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum();
        (diff / a.len() as f64) < 0.1
    }

    fn is_already_federated(&self, signature: &[f64]) -> bool {
        self.federated_patterns
            .iter()
            .any(|p| Self::signature_match(&p.signature, signature))
    }
}

#[derive(Debug)]
pub struct FederationStatus {
    pub node_id: String,
    pub tension: f64,
    pub federation_coherence: f64,
    pub peer_count: usize,
    pub local_patterns: usize,
    pub federated_patterns: usize,
    pub partitioned_peers: usize,
}

/// A federation of coherence-sensing nodes
pub struct CoherenceFederation {
    nodes: HashMap<String, FederatedNode>,
    message_queue: VecDeque<(String, String, FederationMessage)>, // (from, to, msg)
}

impl CoherenceFederation {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            message_queue: VecDeque::new(),
        }
    }

    pub fn add_node(&mut self, id: &str, config: FederationConfig) {
        let mut node = FederatedNode::new(id, config);

        // Connect to existing nodes
        for existing_id in self.nodes.keys() {
            node.add_peer(existing_id);
        }

        // Add this node as peer to existing nodes
        for existing in self.nodes.values_mut() {
            existing.add_peer(id);
        }

        self.nodes.insert(id.to_string(), node);
    }

    pub fn inject_tension(&mut self, node_id: &str, tension: f64) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            if let Some(msg) = node.update_tension(tension) {
                // Broadcast alert to all peers
                for peer_id in node.peer_coherence.keys() {
                    self.message_queue.push_back((
                        node_id.to_string(),
                        peer_id.clone(),
                        msg.clone(),
                    ));
                }
            }
        }
    }

    pub fn learn_pattern(
        &mut self,
        node_id: &str,
        signature: Vec<f64>,
        response: &str,
        efficacy: f64,
    ) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.learn_pattern(signature, response.to_string(), efficacy);
        }
    }

    /// Run one tick of the federation
    pub fn tick(&mut self) {
        // Generate heartbeats
        let heartbeats: Vec<(String, Vec<String>, FederationMessage)> = self
            .nodes
            .iter()
            .map(|(id, node)| {
                let peers: Vec<String> = node.peer_coherence.keys().cloned().collect();
                let msg = FederationMessage::Heartbeat {
                    tension: node.tension,
                    pattern_count: node.federated_patterns.len(),
                };
                (id.clone(), peers, msg)
            })
            .collect();

        for (from, peers, msg) in heartbeats {
            for to in peers {
                self.message_queue
                    .push_back((from.clone(), to, msg.clone()));
            }
        }

        // Generate pattern proposals
        let proposals: Vec<(String, Vec<String>, FederationMessage)> = self
            .nodes
            .iter()
            .flat_map(|(id, node)| {
                let peers: Vec<String> = node.peer_coherence.keys().cloned().collect();
                node.propose_patterns()
                    .into_iter()
                    .map(|msg| (id.clone(), peers.clone(), msg))
                    .collect::<Vec<_>>()
            })
            .collect();

        for (from, peers, msg) in proposals {
            for to in peers {
                self.message_queue
                    .push_back((from.clone(), to, msg.clone()));
            }
        }

        // Process message queue
        while let Some((from, to, msg)) = self.message_queue.pop_front() {
            if let Some(node) = self.nodes.get_mut(&to) {
                if let Some(response) = node.handle_message(&from, msg) {
                    self.message_queue.push_back((to.clone(), from, response));
                }
            }
        }

        // Detect partitions
        for node in self.nodes.values_mut() {
            node.detect_partitions();
        }
    }

    pub fn status(&self) -> Vec<FederationStatus> {
        self.nodes.values().map(|n| n.status()).collect()
    }

    pub fn global_coherence(&self) -> f64 {
        if self.nodes.is_empty() {
            return 1.0;
        }
        self.nodes
            .values()
            .map(|n| n.federation_coherence())
            .sum::<f64>()
            / self.nodes.len() as f64
    }

    pub fn global_tension(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        self.nodes.values().map(|n| n.tension).sum::<f64>() / self.nodes.len() as f64
    }
}

fn main() {
    println!("=== Federated Coherence Network ===\n");
    println!("Consensus through coherence, not voting.\n");

    let mut federation = CoherenceFederation::new();

    // Create 5-node federation
    for i in 0..5 {
        federation.add_node(&format!("node_{}", i), FederationConfig::default());
    }

    println!("Created 5-node federation\n");

    // Run baseline
    println!("Phase 1: Establishing coherence");
    for _ in 0..5 {
        federation.tick();
    }
    println!("Global coherence: {:.2}\n", federation.global_coherence());

    // Node 0 learns a pattern
    println!("Phase 2: node_0 learns a pattern");
    federation.learn_pattern("node_0", vec![0.5, 0.3, 0.2], "rebalance", 0.85);
    federation.learn_pattern("node_0", vec![0.5, 0.3, 0.2], "rebalance", 0.88);
    federation.learn_pattern("node_0", vec![0.5, 0.3, 0.2], "rebalance", 0.82);
    federation.learn_pattern("node_0", vec![0.5, 0.3, 0.2], "rebalance", 0.90);
    federation.learn_pattern("node_0", vec![0.5, 0.3, 0.2], "rebalance", 0.87);

    // Run ticks to propagate
    for _ in 0..10 {
        federation.tick();
    }

    // Inject tension
    println!("\nPhase 3: Tension spike at node_2");
    federation.inject_tension("node_2", 0.8);

    println!("Tick | Global Tension | Global Coherence | node_2 tension");
    println!("-----|----------------|------------------|---------------");

    for i in 0..15 {
        federation.tick();
        let statuses = federation.status();
        let node2 = statuses.iter().find(|s| s.node_id == "node_2").unwrap();

        println!(
            "{:4} | {:.3}          | {:.3}            | {:.3}",
            i,
            federation.global_tension(),
            federation.global_coherence(),
            node2.tension
        );
    }

    println!("\n=== Final Status ===");
    for status in federation.status() {
        println!(
            "{}: tension={:.2}, coherence={:.2}, local={}, federated={}",
            status.node_id,
            status.tension,
            status.federation_coherence,
            status.local_patterns,
            status.federated_patterns
        );
    }

    println!("\n\"Not distributed computing. Distributed feeling.\"");
}
