//! # @ruvector/edge-net
//!
//! Distributed compute intelligence network for browser-based compute contribution.
//! Earn **rUv** (Resource Utility Vouchers) by sharing idle compute.
//!
//! ## Overview
//!
//! edge-net enables websites to participate in a P2P compute marketplace where:
//! - Contributors donate idle CPU cycles via Web Workers
//! - Tasks are distributed across the network
//! - rUv (Resource Utility Vouchers) earned based on contribution
//! - Early adopter multipliers up to 10x
//! - rUv spent to access the network's compute power
//!
//! ## Quick Start
//!
//! ```html
//! <script type="module">
//!   import { EdgeNet } from '@ruvector/edge-net';
//!
//!   const node = await EdgeNet.init({
//!     siteId: 'my-site',
//!     contribution: 0.3,  // 30% CPU when idle
//!   });
//!
//!   console.log(`Balance: ${node.creditBalance()} rUv`);
//! </script>
//! ```
//!
//! ## Features
//!
//! - Self-learning adaptive security
//! - Genesis node sunset when network matures
//! - Lifecycle events and celebrations
//! - Adversarial testing framework
//! - Network evolution and self-organization
//! - Sustainable economic model

use wasm_bindgen::prelude::*;

pub mod identity;
pub mod credits;
pub mod tasks;
pub mod network;
pub mod scheduler;
pub mod security;
pub mod events;
pub mod adversarial;
pub mod evolution;
pub mod tribute;
pub mod pikey;
pub mod learning;
pub mod rac;
pub mod mcp;
pub mod swarm;
pub mod capabilities;
pub mod compute;
pub mod ai;

use identity::WasmNodeIdentity;
use learning::NetworkLearning;
use rac::CoherenceEngine;
use credits::{WasmCreditLedger, ContributionCurve};
use tasks::{WasmTaskExecutor, WasmTaskQueue};
use scheduler::WasmIdleDetector;
use events::NetworkEvents;
use adversarial::AdversarialSimulator;
use evolution::{EconomicEngine, EvolutionEngine, NetworkTopology, OptimizationEngine};
use tribute::{FoundingRegistry, ContributionStream};
pub use capabilities::WasmCapabilities;

/// Initialize panic hook for better error messages in console
#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Main EdgeNet node - the entry point for participating in the network
#[wasm_bindgen]
pub struct EdgeNetNode {
    identity: WasmNodeIdentity,
    ledger: WasmCreditLedger,
    executor: WasmTaskExecutor,
    queue: WasmTaskQueue,
    idle_detector: WasmIdleDetector,
    config: NodeConfig,
    stats: NodeStats,
    /// Lifecycle events and celebrations
    events: NetworkEvents,
    /// Adversarial testing (for security validation)
    adversarial: AdversarialSimulator,
    /// Economic sustainability engine
    economics: EconomicEngine,
    /// Network evolution engine
    evolution: EvolutionEngine,
    /// Topology self-organization
    topology: NetworkTopology,
    /// Task optimization engine
    optimization: OptimizationEngine,
    /// Founding contributor registry
    founding: FoundingRegistry,
    /// Contribution streams
    streams: ContributionStream,
    /// Network learning intelligence
    learning: NetworkLearning,
    /// Adversarial coherence engine (RAC)
    coherence: CoherenceEngine,
    /// Exotic AI capabilities (Time Crystal, NAO, MicroLoRA, HDC, etc.)
    capabilities: WasmCapabilities,
}

#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct NodeConfig {
    /// Maximum CPU usage when idle (0.0 - 1.0)
    pub cpu_limit: f32,
    /// Maximum memory usage in bytes
    pub memory_limit: usize,
    /// Maximum bandwidth in bytes/sec
    pub bandwidth_limit: usize,
    /// Minimum idle time before contributing (ms)
    pub min_idle_time: u32,
    /// Whether to reduce contribution on battery
    pub respect_battery: bool,
}

#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct NodeStats {
    /// Total rUv (Resource Utility Vouchers) earned
    pub ruv_earned: u64,
    /// Total rUv spent
    pub ruv_spent: u64,
    /// Tasks completed
    pub tasks_completed: u64,
    /// Tasks submitted
    pub tasks_submitted: u64,
    /// Total uptime in seconds
    pub uptime_seconds: u64,
    /// Current reputation score (0.0 - 1.0)
    pub reputation: f32,
    /// Current contribution multiplier
    pub multiplier: f32,
    /// Active lifecycle events
    pub celebration_boost: f32,
}

#[wasm_bindgen]
impl EdgeNetNode {
    /// Create a new EdgeNet node
    #[wasm_bindgen(constructor)]
    pub fn new(site_id: &str, config: Option<NodeConfig>) -> Result<EdgeNetNode, JsValue> {
        let config = config.unwrap_or_default();

        // Generate or restore identity
        let identity = WasmNodeIdentity::generate(site_id)?;

        // Initialize credit ledger
        let ledger = WasmCreditLedger::new(identity.node_id())?;

        // Initialize task executor
        let executor = WasmTaskExecutor::new(config.memory_limit)?;

        // Initialize task queue
        let queue = WasmTaskQueue::new()?;

        // Initialize idle detector
        let idle_detector = WasmIdleDetector::new(
            config.cpu_limit,
            config.min_idle_time,
        )?;

        // Initialize economic and evolution engines
        let mut topology = NetworkTopology::new();
        topology.register_node(&identity.node_id(), &[1.0, 0.5, 0.3]);

        let node_id = identity.node_id();
        Ok(EdgeNetNode {
            identity,
            ledger,
            executor,
            queue,
            idle_detector,
            config,
            stats: NodeStats::default(),
            events: NetworkEvents::new(),
            adversarial: AdversarialSimulator::new(),
            economics: EconomicEngine::new(),
            evolution: EvolutionEngine::new(),
            topology,
            optimization: OptimizationEngine::new(),
            founding: FoundingRegistry::new(),
            streams: ContributionStream::new(),
            learning: NetworkLearning::new(),
            coherence: CoherenceEngine::new(),
            capabilities: WasmCapabilities::new(&node_id),
        })
    }

    /// Get the node's unique identifier
    #[wasm_bindgen(js_name = nodeId)]
    pub fn node_id(&self) -> String {
        self.identity.node_id()
    }

    /// Get current rUv (Resource Utility Voucher) balance
    #[wasm_bindgen(js_name = creditBalance)]
    pub fn credit_balance(&self) -> u64 {
        self.ledger.balance()
    }

    /// Alias for creditBalance - returns rUv balance
    #[wasm_bindgen(js_name = ruvBalance)]
    pub fn ruv_balance(&self) -> u64 {
        self.ledger.balance()
    }

    /// Get current contribution multiplier based on network size
    #[wasm_bindgen(js_name = getMultiplier)]
    pub fn get_multiplier(&self) -> f32 {
        let base = ContributionCurve::current_multiplier(self.ledger.network_compute());
        let celebration = self.stats.celebration_boost;
        base * celebration.max(1.0)
    }

    /// Check for active celebration events
    #[wasm_bindgen(js_name = checkEvents)]
    pub fn check_events(&mut self) -> String {
        let events_json = self.events.check_active_events();
        self.stats.celebration_boost = self.events.get_celebration_boost();
        events_json
    }

    /// Get motivational message (subtle Easter egg)
    #[wasm_bindgen(js_name = getMotivation)]
    pub fn get_motivation(&self) -> String {
        self.events.get_motivation(self.ledger.balance())
    }

    /// Run security audit (adversarial testing)
    #[wasm_bindgen(js_name = runSecurityAudit)]
    pub fn run_security_audit(&mut self) -> String {
        self.adversarial.run_security_audit()
    }

    /// Get themed network status
    #[wasm_bindgen(js_name = getThemedStatus)]
    pub fn get_themed_status(&self, node_count: u32) -> String {
        self.events.get_themed_status(node_count, self.ledger.total_earned())
    }

    /// Get node statistics
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> NodeStats {
        self.stats.clone()
    }

    /// Check if user is currently idle
    #[wasm_bindgen(js_name = isIdle)]
    pub fn is_idle(&self) -> bool {
        self.idle_detector.is_idle()
    }

    /// Get current throttle level (0.0 - 1.0)
    #[wasm_bindgen(js_name = getThrottle)]
    pub fn get_throttle(&self) -> f32 {
        self.idle_detector.get_throttle()
    }

    /// Submit a task to the network
    #[wasm_bindgen(js_name = submitTask)]
    pub async fn submit_task(
        &mut self,
        task_type: &str,
        payload: &[u8],
        max_credits: u64,
    ) -> Result<JsValue, JsValue> {
        // Check balance
        if self.ledger.balance() < max_credits {
            return Err(JsValue::from_str("Insufficient credits"));
        }

        // Create task
        let task = self.queue.create_task(
            task_type,
            payload,
            max_credits,
            &self.identity,
        )?;

        // Submit to network
        let result = self.queue.submit(task).await?;

        // Deduct credits
        self.ledger.deduct(result.cost)?;
        self.stats.tasks_submitted += 1;
        self.stats.ruv_spent += result.cost;

        Ok(result.into())
    }

    /// Process the next available task (called by worker)
    #[wasm_bindgen(js_name = processNextTask)]
    pub async fn process_next_task(&mut self) -> Result<bool, JsValue> {
        // Check if we should be working
        if !self.idle_detector.should_work() {
            return Ok(false);
        }

        // Claim next task
        let task = match self.queue.claim_next(&self.identity).await? {
            Some(t) => t,
            None => return Ok(false),
        };

        // Execute task
        let result = self.executor.execute(&task).await?;

        // Save task info before moving
        let task_id = task.id.clone();
        let base_reward = task.base_reward;

        // Submit result
        self.queue.complete(task_id.clone(), result, &self.identity).await?;

        // Earn credits (with multiplier)
        let multiplier = self.get_multiplier();
        let credits = (base_reward as f32 * multiplier) as u64;
        self.ledger.credit(credits, &format!("task:{}", task_id))?;

        self.stats.tasks_completed += 1;
        self.stats.ruv_earned += credits;

        // Check for milestone achievements
        let _ = self.events.check_milestones(self.ledger.balance(), &self.identity.node_id());

        Ok(true)
    }

    /// Start contributing to the network
    #[wasm_bindgen]
    pub fn start(&mut self) -> Result<(), JsValue> {
        self.idle_detector.start()?;
        Ok(())
    }

    /// Pause contribution
    #[wasm_bindgen]
    pub fn pause(&mut self) {
        self.idle_detector.pause();
    }

    /// Resume contribution
    #[wasm_bindgen]
    pub fn resume(&mut self) {
        self.idle_detector.resume();
    }

    /// Disconnect from the network
    #[wasm_bindgen]
    pub fn disconnect(&mut self) -> Result<(), JsValue> {
        self.queue.disconnect()?;
        self.idle_detector.stop();
        Ok(())
    }

    // ========== Network Evolution & Sustainability ==========

    /// Check if network is self-sustaining
    #[wasm_bindgen(js_name = isSelfSustaining)]
    pub fn is_self_sustaining(&self, active_nodes: u32, daily_tasks: u64) -> bool {
        self.economics.is_self_sustaining(active_nodes, daily_tasks)
    }

    /// Get economic health metrics
    #[wasm_bindgen(js_name = getEconomicHealth)]
    pub fn get_economic_health(&self) -> String {
        let health = self.economics.get_health();
        format!(
            r#"{{"velocity":{:.3},"utilization":{:.3},"growth":{:.3},"stability":{:.3}}}"#,
            health.velocity, health.utilization, health.growth_rate, health.stability
        )
    }

    /// Get network fitness score (0-1)
    #[wasm_bindgen(js_name = getNetworkFitness)]
    pub fn get_network_fitness(&self) -> f32 {
        self.evolution.get_network_fitness()
    }

    /// Check if this node should replicate (high performer)
    #[wasm_bindgen(js_name = shouldReplicate)]
    pub fn should_replicate(&self) -> bool {
        self.evolution.should_replicate(&self.identity.node_id())
    }

    /// Get recommended configuration for new nodes
    #[wasm_bindgen(js_name = getRecommendedConfig)]
    pub fn get_recommended_config(&self) -> String {
        self.evolution.get_recommended_config()
    }

    /// Get optimal peers for task routing
    #[wasm_bindgen(js_name = getOptimalPeers)]
    pub fn get_optimal_peers(&self, count: usize) -> Vec<String> {
        self.topology.get_optimal_peers(&self.identity.node_id(), count)
    }

    /// Get optimization statistics
    #[wasm_bindgen(js_name = getOptimizationStats)]
    pub fn get_optimization_stats(&self) -> String {
        self.optimization.get_stats()
    }

    /// Get protocol development fund balance
    #[wasm_bindgen(js_name = getProtocolFund)]
    pub fn get_protocol_fund(&self) -> u64 {
        self.economics.get_protocol_fund()
    }

    /// Get treasury balance for operations
    #[wasm_bindgen(js_name = getTreasury)]
    pub fn get_treasury(&self) -> u64 {
        self.economics.get_treasury()
    }

    /// Process epoch for economic distribution
    #[wasm_bindgen(js_name = processEpoch)]
    pub fn process_epoch(&mut self) {
        self.economics.advance_epoch();
        self.evolution.evolve();
    }

    /// Record peer interaction for topology optimization
    #[wasm_bindgen(js_name = recordPeerInteraction)]
    pub fn record_peer_interaction(&mut self, peer_id: &str, success_rate: f32) {
        self.topology.update_connection(&self.identity.node_id(), peer_id, success_rate);
    }

    /// Record task routing outcome for optimization
    #[wasm_bindgen(js_name = recordTaskRouting)]
    pub fn record_task_routing(&mut self, task_type: &str, node_id: &str, latency_ms: u64, success: bool) {
        self.optimization.record_routing(task_type, node_id, latency_ms, success);
    }

    /// Record node performance for evolution
    #[wasm_bindgen(js_name = recordPerformance)]
    pub fn record_performance(&mut self, success_rate: f32, throughput: f32) {
        self.evolution.record_performance(&self.identity.node_id(), success_rate, throughput);
    }

    /// Get contribution stream health
    #[wasm_bindgen(js_name = isStreamHealthy)]
    pub fn is_stream_healthy(&self) -> bool {
        self.streams.is_healthy()
    }

    /// Get founding contributor count
    #[wasm_bindgen(js_name = getFounderCount)]
    pub fn get_founder_count(&self) -> usize {
        self.founding.get_founder_count()
    }

    // ========================================================================
    // Learning Intelligence Methods
    // ========================================================================

    /// Record a task execution trajectory for learning
    #[wasm_bindgen(js_name = recordLearningTrajectory)]
    pub fn record_learning_trajectory(&self, trajectory_json: &str) -> bool {
        self.learning.record_trajectory(trajectory_json)
    }

    /// Store a learned pattern in the reasoning bank
    #[wasm_bindgen(js_name = storePattern)]
    pub fn store_pattern(&self, pattern_json: &str) -> i32 {
        self.learning.store_pattern(pattern_json)
    }

    /// Lookup similar patterns for task optimization
    #[wasm_bindgen(js_name = lookupPatterns)]
    pub fn lookup_patterns(&self, query_json: &str, k: usize) -> String {
        self.learning.lookup_patterns(query_json, k)
    }

    /// Get learning statistics
    #[wasm_bindgen(js_name = getLearningStats)]
    pub fn get_learning_stats(&self) -> String {
        self.learning.get_stats()
    }

    /// Get energy efficiency ratio from spike-driven attention
    #[wasm_bindgen(js_name = getEnergyEfficiency)]
    pub fn get_energy_efficiency(&self, seq_len: usize, hidden_dim: usize) -> f32 {
        self.learning.get_energy_ratio(seq_len, hidden_dim)
    }

    /// Prune low-quality learned patterns
    #[wasm_bindgen(js_name = prunePatterns)]
    pub fn prune_patterns(&self, min_usage: usize, min_confidence: f64) -> usize {
        self.learning.prune(min_usage, min_confidence)
    }

    /// Get trajectory count for learning analysis
    #[wasm_bindgen(js_name = getTrajectoryCount)]
    pub fn get_trajectory_count(&self) -> usize {
        self.learning.trajectory_count()
    }

    /// Get stored pattern count
    #[wasm_bindgen(js_name = getPatternCount)]
    pub fn get_pattern_count(&self) -> usize {
        self.learning.pattern_count()
    }

    // ========================================================================
    // RAC Adversarial Coherence Methods (12 Axioms)
    // ========================================================================

    /// Get coherence engine event count
    #[wasm_bindgen(js_name = getCoherenceEventCount)]
    pub fn get_coherence_event_count(&self) -> usize {
        self.coherence.event_count()
    }

    /// Get current Merkle root for audit (Axiom 11: Equivocation detectable)
    #[wasm_bindgen(js_name = getMerkleRoot)]
    pub fn get_merkle_root(&self) -> String {
        self.coherence.get_merkle_root()
    }

    /// Get quarantined claim count (Axiom 9: Quarantine is mandatory)
    #[wasm_bindgen(js_name = getQuarantinedCount)]
    pub fn get_quarantined_count(&self) -> usize {
        self.coherence.quarantined_count()
    }

    /// Get active conflict count (Axiom 6: Disagreement is signal)
    #[wasm_bindgen(js_name = getConflictCount)]
    pub fn get_conflict_count(&self) -> usize {
        self.coherence.conflict_count()
    }

    /// Get coherence statistics
    #[wasm_bindgen(js_name = getCoherenceStats)]
    pub fn get_coherence_stats(&self) -> String {
        self.coherence.get_stats()
    }

    /// Check if a claim can be used (not quarantined)
    #[wasm_bindgen(js_name = canUseClaim)]
    pub fn can_use_claim(&self, claim_id: &str) -> bool {
        self.coherence.can_use_claim(claim_id)
    }

    /// Get quarantine level for a claim
    #[wasm_bindgen(js_name = getClaimQuarantineLevel)]
    pub fn get_claim_quarantine_level(&self, claim_id: &str) -> u8 {
        self.coherence.get_quarantine_level(claim_id)
    }

    // ========================================================================
    // Exotic AI Capabilities Methods
    // ========================================================================

    /// Get all available exotic capabilities and their status
    #[wasm_bindgen(js_name = getCapabilities)]
    pub fn get_capabilities(&self) -> JsValue {
        self.capabilities.get_capabilities()
    }

    /// Get capabilities summary as JSON
    #[wasm_bindgen(js_name = getCapabilitiesSummary)]
    pub fn get_capabilities_summary(&self) -> JsValue {
        self.capabilities.get_summary()
    }

    /// Enable Time Crystal for P2P synchronization
    #[wasm_bindgen(js_name = enableTimeCrystal)]
    pub fn enable_time_crystal(&mut self, oscillators: usize) -> bool {
        self.capabilities.enable_time_crystal(oscillators, 100)
    }

    /// Get Time Crystal synchronization level (0.0 - 1.0)
    #[wasm_bindgen(js_name = getTimeCrystalSync)]
    pub fn get_time_crystal_sync(&self) -> f32 {
        self.capabilities.get_time_crystal_sync()
    }

    /// Enable Neural Autonomous Organization for governance
    #[wasm_bindgen(js_name = enableNAO)]
    pub fn enable_nao(&mut self, quorum: f32) -> bool {
        self.capabilities.enable_nao(quorum)
    }

    /// Propose an action in the NAO
    #[wasm_bindgen(js_name = proposeNAO)]
    pub fn propose_nao(&mut self, action: &str) -> String {
        self.capabilities.propose_nao(action)
    }

    /// Vote on a NAO proposal
    #[wasm_bindgen(js_name = voteNAO)]
    pub fn vote_nao(&mut self, proposal_id: &str, weight: f32) -> bool {
        self.capabilities.vote_nao(proposal_id, weight)
    }

    /// Enable MicroLoRA for self-learning
    #[wasm_bindgen(js_name = enableMicroLoRA)]
    pub fn enable_micro_lora(&mut self, rank: usize) -> bool {
        self.capabilities.enable_micro_lora(128, rank)
    }

    /// Enable HDC for hyperdimensional computing
    #[wasm_bindgen(js_name = enableHDC)]
    pub fn enable_hdc(&mut self) -> bool {
        self.capabilities.enable_hdc()
    }

    /// Enable WTA for instant decisions
    #[wasm_bindgen(js_name = enableWTA)]
    pub fn enable_wta(&mut self, num_neurons: usize) -> bool {
        self.capabilities.enable_wta(num_neurons, 0.5, 0.8)
    }

    /// Enable Global Workspace for attention
    #[wasm_bindgen(js_name = enableGlobalWorkspace)]
    pub fn enable_global_workspace(&mut self, capacity: usize) -> bool {
        self.capabilities.enable_global_workspace(capacity)
    }

    /// Enable BTSP for one-shot learning
    #[wasm_bindgen(js_name = enableBTSP)]
    pub fn enable_btsp(&mut self, input_dim: usize) -> bool {
        self.capabilities.enable_btsp(input_dim, 2000.0)
    }

    /// Enable Morphogenetic Network for emergent topology
    #[wasm_bindgen(js_name = enableMorphogenetic)]
    pub fn enable_morphogenetic(&mut self, size: i32) -> bool {
        self.capabilities.enable_morphogenetic(size, size)
    }

    /// Step all exotic capabilities forward
    #[wasm_bindgen(js_name = stepCapabilities)]
    pub fn step_capabilities(&mut self, dt: f32) {
        self.capabilities.step(dt);
    }
}

/// Configuration builder for EdgeNet
#[wasm_bindgen]
pub struct EdgeNetConfig {
    site_id: String,
    cpu_limit: f32,
    memory_limit: usize,
    bandwidth_limit: usize,
    min_idle_time: u32,
    respect_battery: bool,
    allowed_tasks: Vec<String>,
    relay_urls: Vec<String>,
}

#[wasm_bindgen]
impl EdgeNetConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(site_id: &str) -> EdgeNetConfig {
        EdgeNetConfig {
            site_id: site_id.to_string(),
            cpu_limit: 0.3,
            memory_limit: 256 * 1024 * 1024, // 256MB
            bandwidth_limit: 1024 * 1024,     // 1MB/s
            min_idle_time: 5000,              // 5s
            respect_battery: true,
            allowed_tasks: vec![
                "vectors".to_string(),
                "embeddings".to_string(),
                "encryption".to_string(),
            ],
            relay_urls: vec![
                "https://gun-manhattan.herokuapp.com/gun".to_string(),
            ],
        }
    }

    #[wasm_bindgen(js_name = cpuLimit)]
    pub fn cpu_limit(mut self, limit: f32) -> EdgeNetConfig {
        self.cpu_limit = limit.clamp(0.0, 1.0);
        self
    }

    #[wasm_bindgen(js_name = memoryLimit)]
    pub fn memory_limit(mut self, bytes: usize) -> EdgeNetConfig {
        self.memory_limit = bytes;
        self
    }

    #[wasm_bindgen(js_name = minIdleTime)]
    pub fn min_idle_time(mut self, ms: u32) -> EdgeNetConfig {
        self.min_idle_time = ms;
        self
    }

    #[wasm_bindgen(js_name = respectBattery)]
    pub fn respect_battery(mut self, respect: bool) -> EdgeNetConfig {
        self.respect_battery = respect;
        self
    }

    #[wasm_bindgen(js_name = addRelay)]
    pub fn add_relay(mut self, url: &str) -> EdgeNetConfig {
        self.relay_urls.push(url.to_string());
        self
    }

    #[wasm_bindgen]
    pub fn build(self) -> Result<EdgeNetNode, JsValue> {
        let config = NodeConfig {
            cpu_limit: self.cpu_limit,
            memory_limit: self.memory_limit,
            bandwidth_limit: self.bandwidth_limit,
            min_idle_time: self.min_idle_time,
            respect_battery: self.respect_battery,
        };

        EdgeNetNode::new(&self.site_id, Some(config))
    }
}

#[cfg(all(test, feature = "bench"))]
mod bench;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = EdgeNetConfig::new("test-site")
            .cpu_limit(0.5)
            .memory_limit(512 * 1024 * 1024)
            .min_idle_time(10000);

        assert_eq!(config.cpu_limit, 0.5);
        assert_eq!(config.memory_limit, 512 * 1024 * 1024);
        assert_eq!(config.min_idle_time, 10000);
    }
}
