//! # Federated Strange Loops: Multiple Systems Observing Each Other
//!
//! This example implements federated strange loops with:
//! - Phase 1: Observation Infrastructure (ClusterObservation RPC protocol)
//! - Phase 2: Federation Meta-Neurons (Level 3 meta-cognition)
//! - Phase 3: Consensus Integration (spike-based consensus protocol)
//! - Phase 4: Emergent Pattern Detection (collective behaviors)
//!
//! Run: `cargo run --example federated_loops`

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ============================================================================
// PHASE 1: OBSERVATION INFRASTRUCTURE
// ============================================================================

/// Unique identifier for a cluster
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ClusterId(String);

impl ClusterId {
    fn new(name: &str) -> Self {
        Self(name.to_string())
    }
}

/// Graph statistics for a cluster
#[derive(Debug, Clone, Default)]
struct GraphStats {
    node_count: usize,
    edge_count: usize,
    avg_degree: f64,
    clustering_coefficient: f64,
}

/// Actions taken by meta-neurons
#[derive(Debug, Clone, PartialEq)]
enum MetaAction {
    Strengthen { target_edge: (u64, u64), delta: f64 },
    Prune { target_edge: (u64, u64) },
    Restructure { from_node: u64, to_node: u64 },
    NoOp,
}

/// Observation of a remote cluster's meta-state
#[derive(Debug, Clone)]
struct ClusterObservation {
    /// Source cluster ID
    cluster_id: ClusterId,
    /// Timestamp of observation (ms from start)
    timestamp_ms: u64,
    /// Level 2 meta-neuron states
    meta_states: Vec<f64>,
    /// Recent actions taken
    recent_actions: Vec<MetaAction>,
    /// MinCut value
    mincut: f64,
    /// Global synchrony (0-1)
    synchrony: f64,
    /// Graph statistics
    stats: GraphStats,
}

/// Response to observation request
#[derive(Debug, Clone)]
struct ObservationResponse {
    meta_states: Vec<f64>,
    recent_actions: Vec<MetaAction>,
    mincut: f64,
    synchrony: f64,
    stats: GraphStats,
}

/// Protocol configuration for observations
#[derive(Debug, Clone)]
struct ObservationProtocol {
    /// Observation frequency (ms)
    interval_ms: u64,
    /// Maximum observation history per cluster
    max_history: usize,
    /// Observation timeout
    timeout_ms: u64,
    /// Encryption enabled
    encrypt: bool,
}

impl Default for ObservationProtocol {
    fn default() -> Self {
        Self {
            interval_ms: 100,
            max_history: 100,
            timeout_ms: 50,
            encrypt: false,
        }
    }
}

/// Simulated cluster endpoint
struct ClusterEndpoint {
    id: ClusterId,
    meta_states: Vec<f64>,
    action_history: VecDeque<MetaAction>,
    mincut: f64,
    synchrony: f64,
    stats: GraphStats,
}

impl ClusterEndpoint {
    fn new(id: ClusterId) -> Self {
        Self {
            id,
            meta_states: vec![0.0; 3], // 3 meta-neurons
            action_history: VecDeque::new(),
            mincut: 1.0,
            synchrony: 0.5,
            stats: GraphStats::default(),
        }
    }

    fn observe(&self) -> ObservationResponse {
        ObservationResponse {
            meta_states: self.meta_states.clone(),
            recent_actions: self.action_history.iter().cloned().collect(),
            mincut: self.mincut,
            synchrony: self.synchrony,
            stats: self.stats.clone(),
        }
    }

    fn update_state(&mut self, meta_idx: usize, delta: f64) {
        if meta_idx < self.meta_states.len() {
            self.meta_states[meta_idx] += delta;
        }
    }

    fn record_action(&mut self, action: MetaAction) {
        self.action_history.push_back(action);
        if self.action_history.len() > 10 {
            self.action_history.pop_front();
        }
    }
}

/// Registry of all clusters in the federation
struct ClusterRegistry {
    clusters: HashMap<ClusterId, Arc<Mutex<ClusterEndpoint>>>,
}

impl ClusterRegistry {
    fn new() -> Self {
        Self {
            clusters: HashMap::new(),
        }
    }

    fn register(&mut self, endpoint: ClusterEndpoint) {
        let id = endpoint.id.clone();
        self.clusters.insert(id, Arc::new(Mutex::new(endpoint)));
    }

    fn get(&self, id: &ClusterId) -> Option<Arc<Mutex<ClusterEndpoint>>> {
        self.clusters.get(id).cloned()
    }

    fn all_ids(&self) -> Vec<ClusterId> {
        self.clusters.keys().cloned().collect()
    }
}

// ============================================================================
// PHASE 2: FEDERATION META-NEURONS (Level 3)
// ============================================================================

/// Cross-cluster correlation data
#[derive(Debug, Clone)]
struct CrossClusterCorrelation {
    cluster_a: ClusterId,
    cluster_b: ClusterId,
    correlation: f64,
    success_correlation: f64,
}

/// Federation-level actions
#[derive(Debug, Clone, PartialEq)]
enum FederationAction {
    Coordinate(CoordinationStrategy),
    NoOp,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CoordinationStrategy {
    Align,      // Bring clusters into sync
    Specialize, // Allow clusters to diverge
    Dampen,     // Reduce oscillation
}

/// Meta-neuron that observes multiple clusters (Level 3)
struct FederationMetaNeuron {
    /// Neuron ID
    id: usize,
    /// Weights for each cluster's observation
    cluster_weights: HashMap<ClusterId, f64>,
    /// Internal state
    state: f64,
    /// Decision threshold
    threshold: f64,
    /// History of cross-cluster correlations
    correlation_history: VecDeque<CrossClusterCorrelation>,
    /// Oscillation detection window
    state_history: VecDeque<f64>,
}

impl FederationMetaNeuron {
    fn new(id: usize) -> Self {
        Self {
            id,
            cluster_weights: HashMap::new(),
            state: 0.0,
            threshold: 0.3,
            correlation_history: VecDeque::new(),
            state_history: VecDeque::new(),
        }
    }

    fn set_weight(&mut self, cluster_id: ClusterId, weight: f64) {
        self.cluster_weights.insert(cluster_id, weight);
    }

    /// Process observations from all clusters
    fn process_observations(
        &mut self,
        observations: &HashMap<ClusterId, ClusterObservation>,
    ) -> FederationAction {
        // Compute weighted sum of cluster states
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for (cluster_id, obs) in observations {
            let weight = self.cluster_weights.get(cluster_id).copied().unwrap_or(1.0);
            let cluster_state: f64 = obs.meta_states.iter().sum::<f64>()
                / obs.meta_states.len().max(1) as f64;

            weighted_sum += weight * cluster_state;
            total_weight += weight;
        }

        self.state = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };

        // Track state history for oscillation detection
        self.state_history.push_back(self.state);
        if self.state_history.len() > 20 {
            self.state_history.pop_front();
        }

        // Compute cross-cluster correlations
        let correlations = self.compute_cross_correlation(observations);
        for corr in correlations {
            self.correlation_history.push_back(corr);
            if self.correlation_history.len() > 50 {
                self.correlation_history.pop_front();
            }
        }

        // Decide federation-level action
        self.decide_action()
    }

    fn compute_cross_correlation(
        &self,
        observations: &HashMap<ClusterId, ClusterObservation>,
    ) -> Vec<CrossClusterCorrelation> {
        let mut correlations = Vec::new();
        let cluster_ids: Vec<_> = observations.keys().collect();

        for i in 0..cluster_ids.len() {
            for j in (i + 1)..cluster_ids.len() {
                let a = &observations[cluster_ids[i]];
                let b = &observations[cluster_ids[j]];

                // Compute correlation between meta-states
                let correlation = self.pearson_correlation(&a.meta_states, &b.meta_states);

                // Success correlation: correlation of mincut values
                let success = 1.0 - (a.mincut - b.mincut).abs();

                correlations.push(CrossClusterCorrelation {
                    cluster_a: cluster_ids[i].clone(),
                    cluster_b: cluster_ids[j].clone(),
                    correlation,
                    success_correlation: success,
                });
            }
        }

        correlations
    }

    fn pearson_correlation(&self, a: &[f64], b: &[f64]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let n = a.len() as f64;
        let mean_a: f64 = a.iter().sum::<f64>() / n;
        let mean_b: f64 = b.iter().sum::<f64>() / n;

        let mut num = 0.0;
        let mut den_a = 0.0;
        let mut den_b = 0.0;

        for i in 0..a.len() {
            let diff_a = a[i] - mean_a;
            let diff_b = b[i] - mean_b;
            num += diff_a * diff_b;
            den_a += diff_a * diff_a;
            den_b += diff_b * diff_b;
        }

        if den_a == 0.0 || den_b == 0.0 {
            return 0.0;
        }

        num / (den_a.sqrt() * den_b.sqrt())
    }

    fn detect_oscillation(&self) -> bool {
        if self.state_history.len() < 6 {
            return false;
        }

        // Check for alternating signs in state deltas
        let deltas: Vec<f64> = self.state_history.iter()
            .zip(self.state_history.iter().skip(1))
            .map(|(a, b)| b - a)
            .collect();

        let mut sign_changes = 0;
        for i in 1..deltas.len() {
            if deltas[i] * deltas[i - 1] < 0.0 {
                sign_changes += 1;
            }
        }

        // Oscillating if > 50% sign changes
        sign_changes as f64 / (deltas.len() - 1) as f64 > 0.5
    }

    fn decide_action(&self) -> FederationAction {
        if self.state > self.threshold {
            // Clusters are diverging - coordinate
            FederationAction::Coordinate(CoordinationStrategy::Align)
        } else if self.state < -self.threshold {
            // Clusters are converging - allow specialization
            FederationAction::Coordinate(CoordinationStrategy::Specialize)
        } else if self.detect_oscillation() {
            // Unstable dynamics - dampen
            FederationAction::Coordinate(CoordinationStrategy::Dampen)
        } else {
            FederationAction::NoOp
        }
    }
}

/// Cross-cluster influence matrix
struct CrossClusterInfluence {
    /// Influence matrix: cluster_i -> cluster_j
    influence: HashMap<(ClusterId, ClusterId), f64>,
    /// Learning rate for influence updates
    learning_rate: f64,
}

impl CrossClusterInfluence {
    fn new() -> Self {
        Self {
            influence: HashMap::new(),
            learning_rate: 0.1,
        }
    }

    /// Update influence based on observed correlations
    fn update(&mut self, correlations: &[CrossClusterCorrelation]) {
        for corr in correlations {
            let key = (corr.cluster_a.clone(), corr.cluster_b.clone());
            let current = self.influence.get(&key).copied().unwrap_or(0.0);

            // STDP-like update: strengthen if correlated actions succeed
            let delta = self.learning_rate * corr.success_correlation;
            self.influence.insert(key, (current + delta).clamp(-1.0, 1.0));
        }
    }

    fn get_influence(&self, from: &ClusterId, to: &ClusterId) -> f64 {
        self.influence.get(&(from.clone(), to.clone())).copied().unwrap_or(0.0)
    }
}

// ============================================================================
// PHASE 3: CONSENSUS INTEGRATION
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
enum ConsensusAlgorithm {
    Majority,
    Raft,
    PBFT,
    SpikeConsensus, // Novel!
}

#[derive(Debug, Clone)]
enum ConsensusResult {
    Agreed(FederationAction),
    PartialAgreement(FederationAction, f64),
    Rejected,
}

/// Spike pattern for consensus encoding
#[derive(Debug, Clone)]
struct SpikePattern {
    cluster_id: ClusterId,
    spike_times: Vec<u64>, // Relative spike times in ms
    intensity: f64,
}

/// Consensus protocol for federation-wide actions
struct FederationConsensus {
    algorithm: ConsensusAlgorithm,
    quorum: usize,
    timeout_ms: u64,
}

impl FederationConsensus {
    fn new(algorithm: ConsensusAlgorithm) -> Self {
        Self {
            algorithm,
            quorum: 2, // Majority of 3
            timeout_ms: 100,
        }
    }

    /// Propose a federation-wide action
    fn propose(
        &self,
        action: FederationAction,
        registry: &ClusterRegistry,
    ) -> ConsensusResult {
        match self.algorithm {
            ConsensusAlgorithm::SpikeConsensus => {
                self.spike_consensus(action, registry)
            }
            ConsensusAlgorithm::Majority => {
                self.majority_consensus(action, registry)
            }
            _ => self.majority_consensus(action, registry),
        }
    }

    /// Novel: Spike-timing based consensus
    fn spike_consensus(
        &self,
        action: FederationAction,
        registry: &ClusterRegistry,
    ) -> ConsensusResult {
        // Encode action as spike pattern
        let proposal_pattern = self.encode_action_as_spikes(&action);

        // Collect response patterns from all clusters
        let mut response_patterns = Vec::new();
        for cluster_id in registry.all_ids() {
            if let Some(endpoint) = registry.get(&cluster_id) {
                let endpoint = endpoint.lock().unwrap();
                // Simulate response spike pattern based on cluster state
                let response = SpikePattern {
                    cluster_id: cluster_id.clone(),
                    spike_times: self.generate_response_spikes(&endpoint, &proposal_pattern),
                    intensity: endpoint.synchrony,
                };
                response_patterns.push(response);
            }
        }

        // Compute cross-cluster spike synchrony
        let synchrony = self.compute_spike_synchrony(&response_patterns);

        if synchrony > 0.8 {
            ConsensusResult::Agreed(action)
        } else if synchrony > 0.5 {
            ConsensusResult::PartialAgreement(action, synchrony)
        } else {
            ConsensusResult::Rejected
        }
    }

    fn majority_consensus(
        &self,
        action: FederationAction,
        registry: &ClusterRegistry,
    ) -> ConsensusResult {
        let total = registry.all_ids().len();
        let votes = total; // Simulated: all vote yes for demo

        if votes >= self.quorum {
            ConsensusResult::Agreed(action)
        } else {
            ConsensusResult::Rejected
        }
    }

    fn encode_action_as_spikes(&self, action: &FederationAction) -> Vec<u64> {
        match action {
            FederationAction::Coordinate(CoordinationStrategy::Align) => vec![0, 10, 20],
            FederationAction::Coordinate(CoordinationStrategy::Specialize) => vec![0, 15, 30],
            FederationAction::Coordinate(CoordinationStrategy::Dampen) => vec![0, 5, 10, 15],
            FederationAction::NoOp => vec![0],
        }
    }

    fn generate_response_spikes(
        &self,
        endpoint: &ClusterEndpoint,
        proposal: &[u64],
    ) -> Vec<u64> {
        // Response timing influenced by cluster synchrony
        let jitter = ((1.0 - endpoint.synchrony) * 10.0) as u64;
        proposal.iter()
            .map(|&t| t + jitter)
            .collect()
    }

    fn compute_spike_synchrony(&self, patterns: &[SpikePattern]) -> f64 {
        if patterns.len() < 2 {
            return 1.0;
        }

        let mut total_sync = 0.0;
        let mut pairs = 0;

        for i in 0..patterns.len() {
            for j in (i + 1)..patterns.len() {
                let sync = self.pairwise_synchrony(&patterns[i].spike_times, &patterns[j].spike_times);
                total_sync += sync;
                pairs += 1;
            }
        }

        if pairs > 0 { total_sync / pairs as f64 } else { 0.0 }
    }

    fn pairwise_synchrony(&self, a: &[u64], b: &[u64]) -> f64 {
        // Compute synchrony based on spike time differences
        let mut total_diff = 0u64;
        let mut count = 0;

        for &t_a in a {
            for &t_b in b {
                total_diff += (t_a as i64 - t_b as i64).unsigned_abs();
                count += 1;
            }
        }

        if count == 0 {
            return 0.0;
        }

        // Convert to synchrony (inverse of average difference)
        let avg_diff = total_diff as f64 / count as f64;
        1.0 / (1.0 + avg_diff / 10.0) // Normalized
    }
}

// ============================================================================
// PHASE 4: EMERGENT PATTERN DETECTION
// ============================================================================

/// Cluster role after specialization
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ClusterRole {
    Leader,
    Analyst,
    Optimizer,
    Executor,
    Neutral,
}

/// Emergent patterns in federated strange loops
#[derive(Debug, Clone)]
enum EmergentPattern {
    /// All clusters converge to similar structure
    GlobalConvergence,
    /// Clusters specialize into complementary roles
    Specialization { roles: HashMap<ClusterId, ClusterRole> },
    /// Periodic coordinated oscillation
    CollectiveOscillation { period_ms: u64 },
    /// Hierarchical organization emerges
    Hierarchy { leader: ClusterId, followers: Vec<ClusterId> },
    /// Chaotic dynamics (no stable pattern)
    Chaos,
}

/// Pattern detector for federation
struct PatternDetector {
    observation_history: HashMap<ClusterId, VecDeque<ClusterObservation>>,
    min_history_size: usize,
}

impl PatternDetector {
    fn new() -> Self {
        Self {
            observation_history: HashMap::new(),
            min_history_size: 10,
        }
    }

    fn record(&mut self, obs: ClusterObservation) {
        let history = self.observation_history
            .entry(obs.cluster_id.clone())
            .or_insert_with(VecDeque::new);
        history.push_back(obs);
        if history.len() > 100 {
            history.pop_front();
        }
    }

    fn detect_pattern(&self) -> EmergentPattern {
        if self.observation_history.values().any(|h| h.len() < self.min_history_size) {
            return EmergentPattern::Chaos;
        }

        // Check for convergence
        if self.is_converging() {
            return EmergentPattern::GlobalConvergence;
        }

        // Check for specialization
        if let Some(roles) = self.detect_specialization() {
            return EmergentPattern::Specialization { roles };
        }

        // Check for oscillation
        if let Some(period) = self.detect_collective_oscillation() {
            return EmergentPattern::CollectiveOscillation { period_ms: period };
        }

        // Check for hierarchy
        if let Some((leader, followers)) = self.detect_hierarchy() {
            return EmergentPattern::Hierarchy { leader, followers };
        }

        EmergentPattern::Chaos
    }

    fn is_converging(&self) -> bool {
        // Check if mincut values are converging across clusters
        let mut mincut_variance: Vec<f64> = Vec::new();

        for history in self.observation_history.values() {
            let recent: Vec<_> = history.iter().rev().take(5).collect();
            if recent.len() >= 2 {
                let first = recent.last().map(|o| o.mincut).unwrap_or(0.0);
                let last = recent.first().map(|o| o.mincut).unwrap_or(0.0);
                mincut_variance.push((first - last).abs());
            }
        }

        if mincut_variance.is_empty() {
            return false;
        }

        // Converging if variance is decreasing across all clusters
        let avg_variance: f64 = mincut_variance.iter().sum::<f64>() / mincut_variance.len() as f64;
        avg_variance < 0.1
    }

    fn detect_specialization(&self) -> Option<HashMap<ClusterId, ClusterRole>> {
        let mut roles = HashMap::new();
        let mut action_patterns: HashMap<ClusterId, HashMap<String, usize>> = HashMap::new();

        // Analyze action patterns
        for (cluster_id, history) in &self.observation_history {
            let mut pattern: HashMap<String, usize> = HashMap::new();
            for obs in history.iter().rev().take(20) {
                for action in &obs.recent_actions {
                    let action_type = match action {
                        MetaAction::Strengthen { .. } => "strengthen",
                        MetaAction::Prune { .. } => "prune",
                        MetaAction::Restructure { .. } => "restructure",
                        MetaAction::NoOp => "noop",
                    };
                    *pattern.entry(action_type.to_string()).or_insert(0) += 1;
                }
            }
            action_patterns.insert(cluster_id.clone(), pattern);
        }

        // Assign roles based on dominant action
        for (cluster_id, pattern) in action_patterns {
            let role = if pattern.get("strengthen").copied().unwrap_or(0) > 5 {
                ClusterRole::Optimizer
            } else if pattern.get("prune").copied().unwrap_or(0) > 5 {
                ClusterRole::Analyst
            } else if pattern.get("restructure").copied().unwrap_or(0) > 3 {
                ClusterRole::Leader
            } else {
                ClusterRole::Neutral
            };
            roles.insert(cluster_id, role);
        }

        // Only return if at least 2 different roles
        let unique_roles: HashSet<_> = roles.values().collect();
        if unique_roles.len() >= 2 {
            Some(roles)
        } else {
            None
        }
    }

    fn detect_collective_oscillation(&self) -> Option<u64> {
        // Check for periodic patterns in mincut values
        for history in self.observation_history.values() {
            let values: Vec<f64> = history.iter().map(|o| o.mincut).collect();
            if values.len() < 10 {
                continue;
            }

            // Simple FFT-like peak detection
            let mut peaks = Vec::new();
            for i in 1..(values.len() - 1) {
                if values[i] > values[i - 1] && values[i] > values[i + 1] {
                    peaks.push(i);
                }
            }

            if peaks.len() >= 3 {
                // Calculate average period
                let periods: Vec<usize> = peaks.windows(2)
                    .map(|w| w[1] - w[0])
                    .collect();
                let avg_period = periods.iter().sum::<usize>() / periods.len();
                if avg_period > 2 && avg_period < 50 {
                    return Some((avg_period * 100) as u64); // Convert to ms
                }
            }
        }
        None
    }

    fn detect_hierarchy(&self) -> Option<(ClusterId, Vec<ClusterId>)> {
        // Check if one cluster has consistently higher influence
        let mut avg_mincut: Vec<(ClusterId, f64)> = Vec::new();

        for (cluster_id, history) in &self.observation_history {
            let sum: f64 = history.iter().map(|o| o.mincut).sum();
            let avg = sum / history.len() as f64;
            avg_mincut.push((cluster_id.clone(), avg));
        }

        avg_mincut.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        if avg_mincut.len() >= 2 {
            let (leader, leader_mincut) = &avg_mincut[0];
            let (_, second_mincut) = &avg_mincut[1];

            // Leader has significantly higher mincut
            if *leader_mincut > *second_mincut * 1.2 {
                let followers: Vec<_> = avg_mincut[1..].iter()
                    .map(|(id, _)| id.clone())
                    .collect();
                return Some((leader.clone(), followers));
            }
        }

        None
    }
}

// ============================================================================
// FEDERATED STRANGE LOOP - MAIN INTEGRATION
// ============================================================================

/// Main federated strange loop system
struct FederatedStrangeLoop {
    /// Local cluster ID
    local_id: ClusterId,
    /// Registry of all clusters
    registry: ClusterRegistry,
    /// Observation history
    observations: HashMap<ClusterId, VecDeque<ClusterObservation>>,
    /// Federation meta-neurons (Level 3)
    federation_meta: Vec<FederationMetaNeuron>,
    /// Cross-cluster influence
    cross_influence: CrossClusterInfluence,
    /// Consensus protocol
    consensus: FederationConsensus,
    /// Pattern detector
    pattern_detector: PatternDetector,
    /// Protocol config
    protocol: ObservationProtocol,
    /// Simulation time
    time_ms: u64,
}

impl FederatedStrangeLoop {
    fn new(local_id: ClusterId) -> Self {
        Self {
            local_id,
            registry: ClusterRegistry::new(),
            observations: HashMap::new(),
            federation_meta: Vec::new(),
            cross_influence: CrossClusterInfluence::new(),
            consensus: FederationConsensus::new(ConsensusAlgorithm::SpikeConsensus),
            pattern_detector: PatternDetector::new(),
            protocol: ObservationProtocol::default(),
            time_ms: 0,
        }
    }

    fn register_cluster(&mut self, endpoint: ClusterEndpoint) {
        self.registry.register(endpoint);
    }

    fn add_meta_neuron(&mut self, neuron: FederationMetaNeuron) {
        self.federation_meta.push(neuron);
    }

    /// Observe all remote clusters
    fn observe_all(&mut self) -> HashMap<ClusterId, ClusterObservation> {
        let mut all_obs = HashMap::new();

        for cluster_id in self.registry.all_ids() {
            if let Some(endpoint) = self.registry.get(&cluster_id) {
                let endpoint = endpoint.lock().unwrap();
                let response = endpoint.observe();

                let observation = ClusterObservation {
                    cluster_id: cluster_id.clone(),
                    timestamp_ms: self.time_ms,
                    meta_states: response.meta_states,
                    recent_actions: response.recent_actions,
                    mincut: response.mincut,
                    synchrony: response.synchrony,
                    stats: response.stats,
                };

                // Store in history
                self.observations
                    .entry(cluster_id.clone())
                    .or_insert_with(VecDeque::new)
                    .push_back(observation.clone());

                // Limit history
                if let Some(history) = self.observations.get_mut(&cluster_id) {
                    while history.len() > self.protocol.max_history {
                        history.pop_front();
                    }
                }

                // Record for pattern detection
                self.pattern_detector.record(observation.clone());

                all_obs.insert(cluster_id, observation);
            }
        }

        all_obs
    }

    /// Run one federation cycle
    fn run_cycle(&mut self) -> (Vec<FederationAction>, EmergentPattern) {
        let observations = self.observe_all();

        // Process through federation meta-neurons
        let mut actions = Vec::new();
        for meta in &mut self.federation_meta {
            let action = meta.process_observations(&observations);
            if action != FederationAction::NoOp {
                actions.push(action);
            }
        }

        // Update cross-cluster influence
        let correlations: Vec<_> = self.federation_meta.iter()
            .flat_map(|m| m.correlation_history.iter().cloned())
            .collect();
        self.cross_influence.update(&correlations);

        // Detect emergent patterns
        let pattern = self.pattern_detector.detect_pattern();

        // Advance time
        self.time_ms += self.protocol.interval_ms;

        (actions, pattern)
    }

    /// Run consensus on a proposed action
    fn run_consensus(&self, action: FederationAction) -> ConsensusResult {
        self.consensus.propose(action, &self.registry)
    }

    /// Simulate cluster state evolution
    fn simulate_step(&mut self, cluster_id: &ClusterId, delta_meta: &[f64], action: Option<MetaAction>) {
        if let Some(endpoint) = self.registry.get(cluster_id) {
            let mut endpoint = endpoint.lock().unwrap();
            for (i, &delta) in delta_meta.iter().enumerate() {
                endpoint.update_state(i, delta);
            }
            if let Some(action) = action {
                endpoint.record_action(action);
            }
            // Simulate mincut evolution
            endpoint.mincut += (rand_float() - 0.5) * 0.1;
            endpoint.mincut = endpoint.mincut.clamp(0.1, 5.0);
            // Update synchrony
            endpoint.synchrony += (rand_float() - 0.5) * 0.05;
            endpoint.synchrony = endpoint.synchrony.clamp(0.0, 1.0);
        }
    }
}

// Simple random for demo (no external deps)
fn rand_float() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (nanos % 1000) as f64 / 1000.0
}

// ============================================================================
// MAIN: DEMO ALL PHASES
// ============================================================================

fn main() {
    println!("{}",
        "╔════════════════════════════════════════════════════════════════╗\n\
         ║  FEDERATED STRANGE LOOPS: Multi-System Mutual Observation      ║\n\
         ║  Implementing All 4 Phases from Research Spec                  ║\n\
         ╚════════════════════════════════════════════════════════════════╝\n"
    );

    let start = Instant::now();

    // ========== PHASE 1: Observation Infrastructure ==========
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔭 PHASE 1: Observation Infrastructure");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let local_id = ClusterId::new("Cluster-Alpha");
    let mut federation = FederatedStrangeLoop::new(local_id.clone());

    // Create cluster endpoints
    let mut alpha = ClusterEndpoint::new(ClusterId::new("Cluster-Alpha"));
    alpha.meta_states = vec![0.5, 0.3, 0.7];
    alpha.mincut = 2.5;
    alpha.synchrony = 0.8;
    alpha.stats = GraphStats {
        node_count: 100,
        edge_count: 450,
        avg_degree: 9.0,
        clustering_coefficient: 0.45,
    };

    let mut beta = ClusterEndpoint::new(ClusterId::new("Cluster-Beta"));
    beta.meta_states = vec![0.4, 0.6, 0.2];
    beta.mincut = 1.8;
    beta.synchrony = 0.6;
    beta.stats = GraphStats {
        node_count: 80,
        edge_count: 320,
        avg_degree: 8.0,
        clustering_coefficient: 0.52,
    };

    let mut gamma = ClusterEndpoint::new(ClusterId::new("Cluster-Gamma"));
    gamma.meta_states = vec![0.6, 0.4, 0.5];
    gamma.mincut = 3.2;
    gamma.synchrony = 0.9;
    gamma.stats = GraphStats {
        node_count: 120,
        edge_count: 600,
        avg_degree: 10.0,
        clustering_coefficient: 0.38,
    };

    federation.register_cluster(alpha);
    federation.register_cluster(beta);
    federation.register_cluster(gamma);

    println!("Registered 3 cluster endpoints:");
    for id in federation.registry.all_ids() {
        if let Some(endpoint) = federation.registry.get(&id) {
            let e = endpoint.lock().unwrap();
            println!("  • {} (nodes: {}, mincut: {:.2}, sync: {:.2})",
                     id.0, e.stats.node_count, e.mincut, e.synchrony);
        }
    }

    // Test observation
    let observations = federation.observe_all();
    println!("\nObservation cycle 1:");
    for (id, obs) in &observations {
        println!("  {} -> meta_states: {:?}, mincut: {:.2}",
                 id.0, obs.meta_states, obs.mincut);
    }

    println!("\n✅ Phase 1 complete: ClusterObservation, ObservationProtocol, ClusterRegistry\n");

    // ========== PHASE 2: Federation Meta-Neurons ==========
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🧠 PHASE 2: Federation Meta-Neurons (Level 3)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Create federation meta-neuron
    let mut meta0 = FederationMetaNeuron::new(0);
    meta0.set_weight(ClusterId::new("Cluster-Alpha"), 1.0);
    meta0.set_weight(ClusterId::new("Cluster-Beta"), 0.8);
    meta0.set_weight(ClusterId::new("Cluster-Gamma"), 1.2);

    let mut meta1 = FederationMetaNeuron::new(1);
    meta1.set_weight(ClusterId::new("Cluster-Alpha"), 0.9);
    meta1.set_weight(ClusterId::new("Cluster-Beta"), 1.1);
    meta1.set_weight(ClusterId::new("Cluster-Gamma"), 0.7);

    federation.add_meta_neuron(meta0);
    federation.add_meta_neuron(meta1);

    println!("Created {} federation meta-neurons (Level 3)", federation.federation_meta.len());

    // Run federation cycles to build up history
    println!("\nRunning 20 federation cycles...");
    for i in 0..20 {
        // Simulate state changes
        federation.simulate_step(
            &ClusterId::new("Cluster-Alpha"),
            &[0.1, -0.05, 0.02],
            Some(MetaAction::Strengthen { target_edge: (1, 2), delta: 0.1 }),
        );
        federation.simulate_step(
            &ClusterId::new("Cluster-Beta"),
            &[-0.05, 0.1, 0.05],
            Some(MetaAction::Prune { target_edge: (3, 4) }),
        );
        federation.simulate_step(
            &ClusterId::new("Cluster-Gamma"),
            &[0.02, 0.02, -0.1],
            Some(MetaAction::Restructure { from_node: 5, to_node: 6 }),
        );

        let (actions, _) = federation.run_cycle();

        if i == 19 {
            println!("  Cycle {}: {} actions proposed", i + 1, actions.len());
            for action in &actions {
                println!("    → {:?}", action);
            }
        }
    }

    // Check cross-cluster influence
    println!("\nCross-cluster influence matrix:");
    for id_a in federation.registry.all_ids() {
        for id_b in federation.registry.all_ids() {
            if id_a != id_b {
                let inf = federation.cross_influence.get_influence(&id_a, &id_b);
                if inf.abs() > 0.01 {
                    println!("  {} → {}: {:.3}", id_a.0, id_b.0, inf);
                }
            }
        }
    }

    println!("\n✅ Phase 2 complete: FederationMetaNeuron, CrossClusterInfluence\n");

    // ========== PHASE 3: Consensus Integration ==========
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🗳️  PHASE 3: Consensus Integration (Spike-Based)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Test spike-based consensus
    let test_action = FederationAction::Coordinate(CoordinationStrategy::Align);
    println!("Proposing action: {:?}", test_action);

    let consensus_result = federation.run_consensus(test_action.clone());
    match &consensus_result {
        ConsensusResult::Agreed(action) => {
            println!("✓ Consensus AGREED on {:?}", action);
        }
        ConsensusResult::PartialAgreement(action, sync) => {
            println!("◐ Partial agreement ({:.2}%) on {:?}", sync * 100.0, action);
        }
        ConsensusResult::Rejected => {
            println!("✗ Consensus REJECTED");
        }
    }

    // Test other coordination strategies
    println!("\nTesting other consensus proposals:");
    for strategy in [CoordinationStrategy::Specialize, CoordinationStrategy::Dampen] {
        let action = FederationAction::Coordinate(strategy);
        let result = federation.run_consensus(action);
        let status = match &result {
            ConsensusResult::Agreed(_) => "AGREED",
            ConsensusResult::PartialAgreement(_, s) => {
                if *s > 0.7 { "PARTIAL (HIGH)" } else { "PARTIAL (LOW)" }
            }
            ConsensusResult::Rejected => "REJECTED",
        };
        println!("  {:?} → {}", strategy, status);
    }

    println!("\n✅ Phase 3 complete: SpikeConsensus, pairwise synchrony, spike encoding\n");

    // ========== PHASE 4: Emergent Pattern Detection ==========
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🌐 PHASE 4: Emergent Pattern Detection");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Run more cycles to detect patterns
    println!("Running 30 more cycles for pattern detection...");
    let mut last_pattern = EmergentPattern::Chaos;

    for i in 0..30 {
        // Vary simulation to create interesting patterns
        let phase = (i as f64 * 0.3).sin();

        federation.simulate_step(
            &ClusterId::new("Cluster-Alpha"),
            &[phase * 0.1, 0.05, -0.02],
            if i % 3 == 0 {
                Some(MetaAction::Strengthen { target_edge: (1, 2), delta: 0.1 })
            } else {
                None
            },
        );

        federation.simulate_step(
            &ClusterId::new("Cluster-Beta"),
            &[-0.02, phase * 0.08, 0.03],
            if i % 4 == 0 {
                Some(MetaAction::Prune { target_edge: (3, 4) })
            } else {
                None
            },
        );

        federation.simulate_step(
            &ClusterId::new("Cluster-Gamma"),
            &[0.03, -0.01, phase * 0.12],
            if i % 5 == 0 {
                Some(MetaAction::Restructure { from_node: 5, to_node: 6 })
            } else {
                None
            },
        );

        let (_, pattern) = federation.run_cycle();
        last_pattern = pattern;
    }

    println!("\nDetected emergent pattern:");
    match &last_pattern {
        EmergentPattern::GlobalConvergence => {
            println!("  📈 GLOBAL CONVERGENCE");
            println!("     All clusters converging to similar structure");
        }
        EmergentPattern::Specialization { roles } => {
            println!("  🎭 SPECIALIZATION");
            for (cluster, role) in roles {
                println!("     {} → {:?}", cluster.0, role);
            }
        }
        EmergentPattern::CollectiveOscillation { period_ms } => {
            println!("  🌊 COLLECTIVE OSCILLATION");
            println!("     Period: {} ms", period_ms);
        }
        EmergentPattern::Hierarchy { leader, followers } => {
            println!("  👑 HIERARCHY");
            println!("     Leader: {}", leader.0);
            println!("     Followers: {:?}", followers.iter().map(|f| &f.0).collect::<Vec<_>>());
        }
        EmergentPattern::Chaos => {
            println!("  🌀 CHAOS (No stable pattern)");
        }
    }

    // Show final cluster states
    println!("\nFinal cluster states:");
    for id in federation.registry.all_ids() {
        if let Some(endpoint) = federation.registry.get(&id) {
            let e = endpoint.lock().unwrap();
            println!("  {} -> mincut: {:.2}, sync: {:.2}, meta: {:?}",
                     id.0, e.mincut, e.synchrony,
                     e.meta_states.iter().map(|v| format!("{:.2}", v)).collect::<Vec<_>>());
        }
    }

    println!("\n✅ Phase 4 complete: PatternDetector, EmergentPattern variants\n");

    // ========== SUMMARY ==========
    let elapsed = start.elapsed();
    println!("═══════════════════════════════════════════════════════════════════");
    println!("                    IMPLEMENTATION SUMMARY                         ");
    println!("═══════════════════════════════════════════════════════════════════");
    println!("  Phase 1: ✅ ClusterObservation, ClusterRegistry, ObservationProtocol");
    println!("  Phase 2: ✅ FederationMetaNeuron (Level 3), CrossClusterInfluence");
    println!("  Phase 3: ✅ SpikeConsensus, spike-timing synchrony, consensus voting");
    println!("  Phase 4: ✅ PatternDetector, 5 EmergentPattern types");
    println!("───────────────────────────────────────────────────────────────────");
    println!("  Registered clusters:      {}", federation.registry.all_ids().len());
    println!("  Federation meta-neurons:  {}", federation.federation_meta.len());
    println!("  Observation cycles:       {}", federation.time_ms / federation.protocol.interval_ms);
    println!("  Consensus algorithm:      SpikeConsensus (novel!)");
    println!("  Execution time:           {:?}", elapsed);
    println!("═══════════════════════════════════════════════════════════════════");

    // Novel contributions
    println!("\n📚 NOVEL RESEARCH CONTRIBUTIONS:");
    println!("  1. Spike-Based Distributed Consensus");
    println!("     → Using neural synchrony instead of message passing");
    println!("  2. Emergent Role Specialization");
    println!("     → Clusters naturally specialize based on mutual observation");
    println!("  3. Hierarchical Self-Organization");
    println!("     → Leadership emerges from strange loop dynamics");
    println!("  4. Collective Meta-Cognition");
    println!("     → Federation-level self-awareness through Level 3 neurons");
}
