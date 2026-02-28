//! Network Evolution and Economic Sustainability
//!
//! Provides mechanisms for the network to adapt, optimize, and sustain itself
//! through intelligent resource allocation and contribution incentives.

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use rustc_hash::FxHashMap;  // 30-50% faster than std HashMap
use std::collections::VecDeque;

/// Network topology adaptation for self-organization
#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct NetworkTopology {
    /// Current network structure fingerprint
    topology_hash: String,
    /// Node connectivity graph (adjacency scores) - FxHashMap for faster lookups
    connectivity: FxHashMap<String, Vec<(String, f32)>>,
    /// Cluster assignments for efficient routing - FxHashMap for O(1) lookups
    clusters: FxHashMap<String, u32>,
    /// Adaptation learning rate
    learning_rate: f32,
    /// Optimization generation
    generation: u64,
    /// Max connections per node (bounded to prevent memory growth)
    max_connections_per_node: usize,
}

#[wasm_bindgen]
impl NetworkTopology {
    #[wasm_bindgen(constructor)]
    pub fn new() -> NetworkTopology {
        NetworkTopology {
            topology_hash: String::new(),
            connectivity: FxHashMap::default(),
            clusters: FxHashMap::default(),
            learning_rate: 0.1,
            generation: 0,
            max_connections_per_node: 100,  // Bounded connectivity
        }
    }

    /// Register a node in the topology
    #[wasm_bindgen(js_name = registerNode)]
    pub fn register_node(&mut self, node_id: &str, capabilities: &[f32]) {
        // Assign to cluster based on capability similarity
        let cluster_id = self.determine_cluster(capabilities);
        self.clusters.insert(node_id.to_string(), cluster_id);
        self.connectivity.insert(node_id.to_string(), Vec::new());
        self.generation += 1;
    }

    /// Update connection strength between nodes
    #[wasm_bindgen(js_name = updateConnection)]
    pub fn update_connection(&mut self, from: &str, to: &str, success_rate: f32) {
        if let Some(connections) = self.connectivity.get_mut(from) {
            if let Some(conn) = connections.iter_mut().find(|(id, _)| id == to) {
                // Exponential moving average
                conn.1 = conn.1 * (1.0 - self.learning_rate) + success_rate * self.learning_rate;
            } else {
                // Bounded connections: evict lowest score if at limit
                if connections.len() >= self.max_connections_per_node {
                    if let Some(min_idx) = connections.iter()
                        .enumerate()
                        .min_by(|(_, a), (_, b)| a.1.partial_cmp(&b.1).unwrap())
                        .map(|(i, _)| i)
                    {
                        connections.swap_remove(min_idx);
                    }
                }
                connections.push((to.to_string(), success_rate));
            }
        }
    }

    /// Get optimal peers for a node
    #[wasm_bindgen(js_name = getOptimalPeers)]
    pub fn get_optimal_peers(&self, node_id: &str, count: usize) -> Vec<String> {
        let mut peers = Vec::new();

        if let Some(connections) = self.connectivity.get(node_id) {
            let mut sorted: Vec<_> = connections.iter().collect();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            for (peer_id, _score) in sorted.into_iter().take(count) {
                peers.push(peer_id.clone());
            }
        }

        peers
    }

    fn determine_cluster(&self, capabilities: &[f32]) -> u32 {
        // Simple clustering based on primary capability
        if capabilities.is_empty() { return 0; }
        let max_idx = capabilities.iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);
        max_idx as u32
    }
}

/// Economic distribution system for sustainable operations
#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct EconomicEngine {
    /// Total rUv in circulation
    total_supply: u64,
    /// Treasury reserve for network operations
    treasury: u64,
    /// Contributor allocation pool
    contributor_pool: u64,
    /// Protocol development fund (sustains core development)
    protocol_fund: u64,
    /// Distribution ratios (must sum to 1.0)
    distribution: DistributionRatios,
    /// Economic health metrics
    health: EconomicHealth,
    /// Epoch for tracking periods
    current_epoch: u64,
}

#[derive(Clone, Serialize, Deserialize, Default)]
struct DistributionRatios {
    /// Share to active contributors
    contributors: f32,
    /// Share to treasury for operations
    treasury: f32,
    /// Share to protocol development (sustains innovation)
    protocol: f32,
    /// Share to founding contributors (vested over time)
    founders: f32,
}

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct EconomicHealth {
    /// Velocity of rUv (transactions per period)
    pub velocity: f32,
    /// Network utilization rate
    pub utilization: f32,
    /// Supply growth rate
    pub growth_rate: f32,
    /// Stability index (0-1)
    pub stability: f32,
}

#[wasm_bindgen]
impl EconomicEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> EconomicEngine {
        EconomicEngine {
            total_supply: 0,
            treasury: 0,
            contributor_pool: 0,
            protocol_fund: 0,
            distribution: DistributionRatios {
                contributors: 0.70,   // 70% to contributors
                treasury: 0.15,       // 15% to operations
                protocol: 0.10,       // 10% to protocol development
                founders: 0.05,       // 5% to founding contributors
            },
            health: EconomicHealth::default(),
            current_epoch: 0,
        }
    }

    /// Process task completion and distribute rewards
    #[wasm_bindgen(js_name = processReward)]
    pub fn process_reward(&mut self, base_amount: u64, multiplier: f32) -> RewardDistribution {
        let total = (base_amount as f32 * multiplier) as u64;

        // Mint new rUv
        self.total_supply += total;

        // Calculate distributions
        let to_contributor = (total as f32 * self.distribution.contributors) as u64;
        let to_treasury = (total as f32 * self.distribution.treasury) as u64;
        let to_protocol = (total as f32 * self.distribution.protocol) as u64;
        let to_founders = total - to_contributor - to_treasury - to_protocol;

        // Update pools
        self.contributor_pool += to_contributor;
        self.treasury += to_treasury;
        self.protocol_fund += to_protocol;

        // Update health metrics
        self.health.velocity = (self.health.velocity * 0.99) + 0.01;

        RewardDistribution {
            total,
            contributor_share: to_contributor,
            treasury_share: to_treasury,
            protocol_share: to_protocol,
            founder_share: to_founders,
        }
    }

    /// Check if network can sustain itself
    #[wasm_bindgen(js_name = isSelfSustaining)]
    pub fn is_self_sustaining(&self, active_nodes: u32, daily_tasks: u64) -> bool {
        // Network is self-sustaining when:
        // 1. Enough nodes for redundancy (100+)
        // 2. Sufficient daily activity (1000+ tasks)
        // 3. Treasury can cover 90 days of operations
        // 4. Positive growth rate
        let min_nodes = 100;
        let min_daily_tasks = 1000;
        let treasury_runway_days = 90;
        let estimated_daily_cost = (active_nodes as u64) * 10; // 10 rUv per node per day

        active_nodes >= min_nodes &&
        daily_tasks >= min_daily_tasks &&
        self.treasury >= estimated_daily_cost * treasury_runway_days &&
        self.health.growth_rate >= 0.0
    }

    /// Get protocol fund balance (for development sustainability)
    #[wasm_bindgen(js_name = getProtocolFund)]
    pub fn get_protocol_fund(&self) -> u64 {
        self.protocol_fund
    }

    /// Get treasury balance
    #[wasm_bindgen(js_name = getTreasury)]
    pub fn get_treasury(&self) -> u64 {
        self.treasury
    }

    /// Get economic health status
    #[wasm_bindgen(js_name = getHealth)]
    pub fn get_health(&self) -> EconomicHealth {
        self.health.clone()
    }

    /// Advance to next epoch
    #[wasm_bindgen(js_name = advanceEpoch)]
    pub fn advance_epoch(&mut self) {
        self.current_epoch += 1;
        // Recalculate health metrics
        self.health.stability = self.calculate_stability();
    }

    fn calculate_stability(&self) -> f32 {
        // Stability based on balanced pools
        let total_pools = self.treasury + self.contributor_pool + self.protocol_fund;
        if total_pools == 0 { return 0.5; }

        let treasury_ratio = self.treasury as f32 / total_pools as f32;
        let contributor_ratio = self.contributor_pool as f32 / total_pools as f32;
        let protocol_ratio = self.protocol_fund as f32 / total_pools as f32;

        // Penalize imbalanced distribution
        let ideal = 0.33f32;
        let variance = (treasury_ratio - ideal).powi(2) +
                      (contributor_ratio - ideal).powi(2) +
                      (protocol_ratio - ideal).powi(2);

        (1.0 - variance.sqrt()).max(0.0).min(1.0)
    }
}

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct RewardDistribution {
    pub total: u64,
    pub contributor_share: u64,
    pub treasury_share: u64,
    pub protocol_share: u64,
    pub founder_share: u64,
}

/// Node replication and evolution guidance
#[wasm_bindgen]
#[derive(Clone)]
pub struct EvolutionEngine {
    /// Fitness scores by capability - FxHashMap for faster lookups
    fitness_scores: FxHashMap<String, f32>,
    /// Successful patterns for replication (bounded to 100)
    successful_patterns: Vec<NodePattern>,
    /// Evolution generation
    generation: u64,
    /// Mutation rate for variation
    mutation_rate: f32,
    /// Max patterns to track
    max_patterns: usize,
}

#[derive(Clone, Serialize, Deserialize)]
struct NodePattern {
    pattern_id: String,
    capabilities: Vec<f32>,
    configuration: FxHashMap<String, String>,
    success_rate: f32,
    replications: u32,
}

#[wasm_bindgen]
impl EvolutionEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> EvolutionEngine {
        EvolutionEngine {
            fitness_scores: FxHashMap::default(),
            successful_patterns: Vec::with_capacity(100),  // Pre-allocate
            generation: 0,
            mutation_rate: 0.05,
            max_patterns: 100,
        }
    }

    /// Record node performance for fitness evaluation
    #[wasm_bindgen(js_name = recordPerformance)]
    pub fn record_performance(&mut self, node_id: &str, success_rate: f32, throughput: f32) {
        let fitness = success_rate * 0.6 + (throughput / 100.0).min(1.0) * 0.4;

        if let Some(existing) = self.fitness_scores.get_mut(node_id) {
            *existing = *existing * 0.9 + fitness * 0.1; // Exponential moving average
        } else {
            self.fitness_scores.insert(node_id.to_string(), fitness);
        }
    }

    /// Get recommended configuration for new nodes
    #[wasm_bindgen(js_name = getRecommendedConfig)]
    pub fn get_recommended_config(&self) -> String {
        // Find highest performing pattern
        let best = self.successful_patterns.iter()
            .max_by(|a, b| a.success_rate.partial_cmp(&b.success_rate).unwrap_or(std::cmp::Ordering::Equal));

        match best {
            Some(pattern) => serde_json::to_string(&pattern.configuration).unwrap_or_default(),
            None => r#"{"cpu_limit":0.3,"memory_limit":268435456,"min_idle_time":5000}"#.to_string(),
        }
    }

    /// Check if node should replicate (spawn similar node)
    #[wasm_bindgen(js_name = shouldReplicate)]
    pub fn should_replicate(&self, node_id: &str) -> bool {
        if let Some(&fitness) = self.fitness_scores.get(node_id) {
            // High performers should replicate
            fitness > 0.85
        } else {
            false
        }
    }

    /// Get network fitness score
    #[wasm_bindgen(js_name = getNetworkFitness)]
    pub fn get_network_fitness(&self) -> f32 {
        if self.fitness_scores.is_empty() { return 0.0; }
        let sum: f32 = self.fitness_scores.values().sum();
        sum / self.fitness_scores.len() as f32
    }

    /// Evolve patterns for next generation
    #[wasm_bindgen(js_name = evolve)]
    pub fn evolve(&mut self) {
        self.generation += 1;

        // Remove underperforming patterns
        self.successful_patterns.retain(|p| p.success_rate > 0.5);

        // Decrease mutation rate over generations (stabilization)
        self.mutation_rate = (0.05 * (0.99f32).powi(self.generation as i32)).max(0.01);
    }
}

/// Network optimization for resource efficiency
#[wasm_bindgen]
#[derive(Clone)]
pub struct OptimizationEngine {
    /// Task routing decisions and outcomes (VecDeque for efficient trimming)
    routing_history: VecDeque<RoutingDecision>,
    /// Resource utilization by node - FxHashMap for faster lookups
    resource_usage: FxHashMap<String, ResourceMetrics>,
    /// Optimization policies
    policies: OptimizationPolicies,
    /// Learning from outcomes
    learning_enabled: bool,
    /// Max routing history to keep
    max_history: usize,
}

#[derive(Clone, Serialize, Deserialize)]
struct RoutingDecision {
    task_type: String,
    selected_node: String,
    alternatives: Vec<String>,
    latency_ms: u64,
    success: bool,
    timestamp: u64,
}

#[derive(Clone, Serialize, Deserialize, Default)]
struct ResourceMetrics {
    cpu_avg: f32,
    memory_avg: f32,
    bandwidth_avg: f32,
    uptime_seconds: u64,
    tasks_completed: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct OptimizationPolicies {
    /// Prefer nodes with lower latency
    latency_weight: f32,
    /// Prefer nodes with higher success rate
    reliability_weight: f32,
    /// Balance load across nodes
    load_balance_weight: f32,
}

impl Default for OptimizationPolicies {
    fn default() -> Self {
        OptimizationPolicies {
            latency_weight: 0.3,
            reliability_weight: 0.5,
            load_balance_weight: 0.2,
        }
    }
}

#[wasm_bindgen]
impl OptimizationEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> OptimizationEngine {
        OptimizationEngine {
            routing_history: VecDeque::with_capacity(10000),  // Pre-allocate
            resource_usage: FxHashMap::default(),
            policies: OptimizationPolicies::default(),
            learning_enabled: true,
            max_history: 10000,
        }
    }

    /// Record task routing outcome
    #[wasm_bindgen(js_name = recordRouting)]
    pub fn record_routing(
        &mut self,
        task_type: &str,
        node_id: &str,
        latency_ms: u64,
        success: bool,
    ) {
        let decision = RoutingDecision {
            task_type: task_type.to_string(),
            selected_node: node_id.to_string(),
            alternatives: Vec::new(),
            latency_ms,
            success,
            timestamp: js_sys::Date::now() as u64,
        };

        self.routing_history.push_back(decision);

        // Keep history bounded (O(1) amortized vs O(n) drain)
        while self.routing_history.len() > self.max_history {
            self.routing_history.pop_front();
        }

        // Update resource usage
        if let Some(metrics) = self.resource_usage.get_mut(node_id) {
            if success {
                metrics.tasks_completed += 1;
            }
        } else {
            self.resource_usage.insert(node_id.to_string(), ResourceMetrics {
                tasks_completed: if success { 1 } else { 0 },
                ..Default::default()
            });
        }
    }

    /// Get optimal node for a task type
    #[wasm_bindgen(js_name = selectOptimalNode)]
    pub fn select_optimal_node(&self, task_type: &str, candidates: Vec<String>) -> String {
        if candidates.is_empty() {
            return String::new();
        }

        // Score each candidate
        let mut scored: Vec<(String, f32)> = candidates.into_iter()
            .map(|node| {
                let score = self.calculate_node_score(&node, task_type);
                (node, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter().next().map(|(node, _)| node).unwrap_or_default()
    }

    fn calculate_node_score(&self, node_id: &str, task_type: &str) -> f32 {
        let history: Vec<_> = self.routing_history.iter()
            .filter(|d| d.selected_node == node_id && d.task_type == task_type)
            .collect();

        if history.is_empty() {
            return 0.5; // Unknown nodes get neutral score
        }

        let success_rate = history.iter().filter(|d| d.success).count() as f32 / history.len() as f32;
        let avg_latency: f32 = history.iter().map(|d| d.latency_ms as f32).sum::<f32>() / history.len() as f32;
        let latency_score = 1.0 - (avg_latency / 1000.0).min(1.0);

        success_rate * self.policies.reliability_weight +
        latency_score * self.policies.latency_weight +
        0.5 * self.policies.load_balance_weight // TODO: actual load balance
    }

    /// Get optimization stats
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let total_decisions = self.routing_history.len();
        let successes = self.routing_history.iter().filter(|d| d.success).count();
        let success_rate = if total_decisions > 0 {
            successes as f32 / total_decisions as f32
        } else {
            0.0
        };

        format!(
            r#"{{"total_decisions":{},"success_rate":{:.3},"nodes_tracked":{}}}"#,
            total_decisions,
            success_rate,
            self.resource_usage.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_economic_engine() {
        let mut engine = EconomicEngine::new();
        let reward = engine.process_reward(100, 1.5);

        assert_eq!(reward.total, 150);
        assert!(reward.contributor_share > reward.treasury_share);
    }

    #[test]
    fn test_evolution_engine() {
        let mut engine = EvolutionEngine::new();
        // Record multiple high performances to reach replication threshold (0.85)
        for _ in 0..10 {
            engine.record_performance("node-1", 0.98, 80.0);
        }

        assert!(engine.should_replicate("node-1"));
        assert!(!engine.should_replicate("node-unknown"));
    }

    #[test]
    fn test_optimization_select() {
        // Test selection logic without using js_sys::Date
        let engine = OptimizationEngine::new();

        // With empty history, all candidates should get neutral score
        let result = engine.select_optimal_node("vectors", vec!["node-1".into(), "node-2".into()]);
        assert!(!result.is_empty());
    }
}
