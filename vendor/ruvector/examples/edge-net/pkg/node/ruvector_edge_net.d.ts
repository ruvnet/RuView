/* tslint:disable */
/* eslint-disable */

export class AdaptiveSecurity {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Choose action using epsilon-greedy policy
   */
  chooseAction(state: string, available_actions: string): string;
  /**
   * Detect if request matches known attack pattern
   */
  detectAttack(features: Float32Array): number;
  /**
   * Export learned patterns for persistence
   */
  exportPatterns(): Uint8Array;
  /**
   * Import learned patterns
   */
  importPatterns(data: Uint8Array): void;
  getMinReputation(): number;
  getRateLimitMax(): number;
  getSecurityLevel(): number;
  /**
   * Get current adaptive thresholds
   */
  getRateLimitWindow(): bigint;
  /**
   * Record attack pattern for learning
   */
  recordAttackPattern(pattern_type: string, features: Float32Array, severity: number): void;
  /**
   * Update network health metrics
   */
  updateNetworkHealth(active_nodes: number, suspicious_nodes: number, attacks_hour: number, false_positives: number, avg_response_ms: number): void;
  getSpotCheckProbability(): number;
  constructor();
  /**
   * Learn from security event outcome (batched for better performance)
   */
  learn(state: string, action: string, reward: number, next_state: string): void;
  /**
   * Get learning statistics
   */
  getStats(): string;
}

export class AdversarialSimulator {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Simulate DDoS attack
   */
  simulateDDoS(requests_per_second: number, duration_ms: bigint): string;
  /**
   * Simulate Sybil attack
   */
  simulateSybil(fake_nodes: number, same_fingerprint: boolean): string;
  /**
   * Enable chaos mode for continuous testing
   */
  enableChaosMode(enabled: boolean): void;
  /**
   * Run comprehensive security audit
   */
  runSecurityAudit(): string;
  /**
   * Simulate Byzantine node behavior
   */
  simulateByzantine(byzantine_nodes: number, total_nodes: number): string;
  /**
   * Get defence metrics
   */
  getDefenceMetrics(): string;
  /**
   * Get recommendations based on testing
   */
  getRecommendations(): string;
  /**
   * Generate chaos event
   */
  generateChaosEvent(): string | undefined;
  /**
   * Simulate free-riding attack
   */
  simulateFreeRiding(consumption_rate: number, contribution_rate: number): string;
  /**
   * Simulate double-spend attempt
   */
  simulateDoubleSpend(amount: bigint, concurrent_targets: number): string;
  /**
   * Simulate result tampering
   */
  simulateResultTampering(tamper_percentage: number): string;
  constructor();
}

export class AuditLog {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Export events as JSON
   */
  exportEvents(): string;
  /**
   * Get events for a node
   */
  getEventsForNode(node_id: string): number;
  /**
   * Get events by severity
   */
  getEventsBySeverity(min_severity: number): number;
  /**
   * Log an event
   */
  log(event_type: string, node_id: string, details: string, severity: number): void;
  constructor();
}

export class BrowserFingerprint {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Generate anonymous uniqueness score
   * This doesn't track users, just ensures one node per browser
   */
  static generate(): Promise<string>;
}

export class ByzantineDetector {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get maximum allowed magnitude
   */
  getMaxMagnitude(): number;
  /**
   * Create a new Byzantine detector
   */
  constructor(max_magnitude: number, zscore_threshold: number);
}

export class CoherenceEngine {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get event log length
   */
  eventCount(): number;
  /**
   * Check if context has drifted
   */
  hasDrifted(context_hex: string): boolean;
  /**
   * Check if a claim can be used in decisions
   */
  canUseClaim(claim_id: string): boolean;
  /**
   * Get witness count for a claim
   */
  witnessCount(claim_id: string): number;
  /**
   * Get conflict count
   */
  conflictCount(): number;
  /**
   * Get current Merkle root
   */
  getMerkleRoot(): string;
  /**
   * Get quarantined claim count
   */
  quarantinedCount(): number;
  /**
   * Check quarantine level for a claim
   */
  getQuarantineLevel(claim_id: string): number;
  /**
   * Check if claim has sufficient witnesses
   */
  hasSufficientWitnesses(claim_id: string): boolean;
  /**
   * Create a new coherence engine
   */
  constructor();
  /**
   * Get drift for a context
   */
  getDrift(context_hex: string): number;
  /**
   * Get statistics as JSON
   */
  getStats(): string;
}

export class CollectiveMemory {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get queue size
   */
  queueSize(): number;
  /**
   * Run consolidation (call during idle periods)
   */
  consolidate(): number;
  /**
   * Check if a pattern ID exists
   */
  hasPattern(pattern_id: string): boolean;
  /**
   * Get pattern count in shared index
   */
  patternCount(): number;
  /**
   * Create new collective memory with default config
   */
  constructor(node_id: string);
  /**
   * Search for similar patterns
   */
  search(query_json: string, k: number): string;
  /**
   * Get statistics as JSON
   */
  getStats(): string;
}

export class ContributionStream {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Check if streams are healthy
   */
  isHealthy(): boolean;
  /**
   * Process network fee distribution
   */
  processFees(total_fees: bigint, epoch: bigint): bigint;
  /**
   * Get total distributed
   */
  getTotalDistributed(): bigint;
  constructor();
}

export class DifferentialPrivacy {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Check if DP is enabled
   */
  isEnabled(): boolean;
  /**
   * Get epsilon value
   */
  getEpsilon(): number;
  /**
   * Enable/disable differential privacy
   */
  setEnabled(enabled: boolean): void;
  /**
   * Create a new differential privacy module
   */
  constructor(epsilon: number, sensitivity: number);
}

export class DriftTracker {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Check if context has drifted beyond threshold
   */
  hasDrifted(context_hex: string): boolean;
  /**
   * Get contexts with significant drift
   */
  getDriftedContexts(): string;
  /**
   * Create a new drift tracker
   */
  constructor(drift_threshold: number);
  /**
   * Get drift for a context
   */
  getDrift(context_hex: string): number;
}

export class EconomicEngine {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get economic health status
   */
  getHealth(): EconomicHealth;
  /**
   * Get treasury balance
   */
  getTreasury(): bigint;
  /**
   * Advance to next epoch
   */
  advanceEpoch(): void;
  /**
   * Process task completion and distribute rewards
   */
  processReward(base_amount: bigint, multiplier: number): RewardDistribution;
  /**
   * Get protocol fund balance (for development sustainability)
   */
  getProtocolFund(): bigint;
  /**
   * Check if network can sustain itself
   */
  isSelfSustaining(active_nodes: number, daily_tasks: bigint): boolean;
  constructor();
}

export class EconomicHealth {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Velocity of rUv (transactions per period)
   */
  velocity: number;
  /**
   * Network utilization rate
   */
  utilization: number;
  /**
   * Supply growth rate
   */
  growth_rate: number;
  /**
   * Stability index (0-1)
   */
  stability: number;
}

export class EdgeNetConfig {
  free(): void;
  [Symbol.dispose](): void;
  memoryLimit(bytes: number): EdgeNetConfig;
  minIdleTime(ms: number): EdgeNetConfig;
  respectBattery(respect: boolean): EdgeNetConfig;
  constructor(site_id: string);
  build(): EdgeNetNode;
  addRelay(url: string): EdgeNetConfig;
  cpuLimit(limit: number): EdgeNetConfig;
}

export class EdgeNetNode {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Disconnect from the network
   */
  disconnect(): void;
  /**
   * Enable HDC for hyperdimensional computing
   */
  enableHDC(): boolean;
  /**
   * Enable Neural Autonomous Organization for governance
   */
  enableNAO(quorum: number): boolean;
  /**
   * Enable WTA for instant decisions
   */
  enableWTA(num_neurons: number): boolean;
  /**
   * Enable BTSP for one-shot learning
   */
  enableBTSP(input_dim: number): boolean;
  /**
   * Propose an action in the NAO
   */
  proposeNAO(action: string): string;
  /**
   * Alias for creditBalance - returns rUv balance
   */
  ruvBalance(): bigint;
  /**
   * Submit a task to the network
   */
  submitTask(task_type: string, payload: Uint8Array, max_credits: bigint): Promise<any>;
  /**
   * Check for active celebration events
   */
  checkEvents(): string;
  /**
   * Get current throttle level (0.0 - 1.0)
   */
  getThrottle(): number;
  /**
   * Get treasury balance for operations
   */
  getTreasury(): bigint;
  /**
   * Check if a claim can be used (not quarantined)
   */
  canUseClaim(claim_id: string): boolean;
  /**
   * Process epoch for economic distribution
   */
  processEpoch(): void;
  /**
   * Store a learned pattern in the reasoning bank
   */
  storePattern(pattern_json: string): number;
  /**
   * Get current rUv (Resource Utility Voucher) balance
   */
  creditBalance(): bigint;
  /**
   * Get motivational message (subtle Easter egg)
   */
  getMotivation(): string;
  /**
   * Get current contribution multiplier based on network size
   */
  getMultiplier(): number;
  /**
   * Prune low-quality learned patterns
   */
  prunePatterns(min_usage: number, min_confidence: number): number;
  /**
   * Get current Merkle root for audit (Axiom 11: Equivocation detectable)
   */
  getMerkleRoot(): string;
  /**
   * Lookup similar patterns for task optimization
   */
  lookupPatterns(query_json: string, k: number): string;
  /**
   * Get all available exotic capabilities and their status
   */
  getCapabilities(): any;
  /**
   * Check if this node should replicate (high performer)
   */
  shouldReplicate(): boolean;
  /**
   * Enable MicroLoRA for self-learning
   */
  enableMicroLoRA(rank: number): boolean;
  /**
   * Get founding contributor count
   */
  getFounderCount(): number;
  /**
   * Get optimal peers for task routing
   */
  getOptimalPeers(count: number): string[];
  /**
   * Get stored pattern count
   */
  getPatternCount(): number;
  /**
   * Get protocol development fund balance
   */
  getProtocolFund(): bigint;
  /**
   * Get themed network status
   */
  getThemedStatus(node_count: number): string;
  /**
   * Get contribution stream health
   */
  isStreamHealthy(): boolean;
  /**
   * Process the next available task (called by worker)
   */
  processNextTask(): Promise<boolean>;
  /**
   * Step all exotic capabilities forward
   */
  stepCapabilities(dt: number): void;
  /**
   * Get active conflict count (Axiom 6: Disagreement is signal)
   */
  getConflictCount(): number;
  /**
   * Get learning statistics
   */
  getLearningStats(): string;
  /**
   * Check if network is self-sustaining
   */
  isSelfSustaining(active_nodes: number, daily_tasks: bigint): boolean;
  /**
   * Record node performance for evolution
   */
  recordPerformance(success_rate: number, throughput: number): void;
  /**
   * Run security audit (adversarial testing)
   */
  runSecurityAudit(): string;
  /**
   * Enable Time Crystal for P2P synchronization
   */
  enableTimeCrystal(oscillators: number): boolean;
  /**
   * Get coherence statistics
   */
  getCoherenceStats(): string;
  /**
   * Get economic health metrics
   */
  getEconomicHealth(): string;
  /**
   * Get network fitness score (0-1)
   */
  getNetworkFitness(): number;
  /**
   * Record task routing outcome for optimization
   */
  recordTaskRouting(task_type: string, node_id: string, latency_ms: bigint, success: boolean): void;
  /**
   * Enable Morphogenetic Network for emergent topology
   */
  enableMorphogenetic(size: number): boolean;
  /**
   * Get trajectory count for learning analysis
   */
  getTrajectoryCount(): number;
  /**
   * Get energy efficiency ratio from spike-driven attention
   */
  getEnergyEfficiency(seq_len: number, hidden_dim: number): number;
  /**
   * Get quarantined claim count (Axiom 9: Quarantine is mandatory)
   */
  getQuarantinedCount(): number;
  /**
   * Get Time Crystal synchronization level (0.0 - 1.0)
   */
  getTimeCrystalSync(): number;
  /**
   * Get optimization statistics
   */
  getOptimizationStats(): string;
  /**
   * Get recommended configuration for new nodes
   */
  getRecommendedConfig(): string;
  /**
   * Enable Global Workspace for attention
   */
  enableGlobalWorkspace(capacity: number): boolean;
  /**
   * Record peer interaction for topology optimization
   */
  recordPeerInteraction(peer_id: string, success_rate: number): void;
  /**
   * Get capabilities summary as JSON
   */
  getCapabilitiesSummary(): any;
  /**
   * Get coherence engine event count
   */
  getCoherenceEventCount(): number;
  /**
   * Get quarantine level for a claim
   */
  getClaimQuarantineLevel(claim_id: string): number;
  /**
   * Record a task execution trajectory for learning
   */
  recordLearningTrajectory(trajectory_json: string): boolean;
  /**
   * Create a new EdgeNet node
   */
  constructor(site_id: string, config?: NodeConfig | null);
  /**
   * Pause contribution
   */
  pause(): void;
  /**
   * Start contributing to the network
   */
  start(): void;
  /**
   * Resume contribution
   */
  resume(): void;
  /**
   * Check if user is currently idle
   */
  isIdle(): boolean;
  /**
   * Get the node's unique identifier
   */
  nodeId(): string;
  /**
   * Vote on a NAO proposal
   */
  voteNAO(proposal_id: string, weight: number): boolean;
  /**
   * Get node statistics
   */
  getStats(): NodeStats;
}

export class EntropyConsensus {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get belief probability for a decision
   */
  getBelief(decision_id: bigint): number;
  /**
   * Get number of negotiation rounds completed
   */
  getRounds(): number;
  /**
   * Set initial belief for a decision
   */
  setBelief(decision_id: bigint, probability: number): void;
  /**
   * Get the winning decision (if converged)
   */
  getDecision(): bigint | undefined;
  /**
   * Get number of decision options
   */
  optionCount(): number;
  /**
   * Check if negotiation has timed out
   */
  hasTimedOut(): boolean;
  /**
   * Set belief without normalizing (for batch updates)
   * Call normalize_beliefs() after all set_belief_raw calls
   */
  set_belief_raw(decision_id: bigint, probability: number): void;
  /**
   * Create with custom entropy threshold
   */
  static withThreshold(threshold: number): EntropyConsensus;
  /**
   * Get current temperature (for annealing)
   */
  getTemperature(): number;
  /**
   * Manually trigger normalization (for use after set_belief_raw)
   */
  finalize_beliefs(): void;
  /**
   * Get entropy history as JSON
   */
  getEntropyHistory(): string;
  /**
   * Get the entropy threshold for convergence
   */
  getEntropyThreshold(): number;
  /**
   * Create new entropy consensus with default configuration
   */
  constructor();
  /**
   * Reset consensus state for new decision
   */
  reset(): void;
  /**
   * Get current entropy of belief distribution
   */
  entropy(): number;
  /**
   * Check if consensus has been reached
   */
  converged(): boolean;
  /**
   * Get consensus statistics as JSON
   */
  getStats(): string;
}

export class EventLog {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get total event count
   */
  totalEvents(): number;
  /**
   * Get current event count (includes all events)
   */
  len(): number;
  /**
   * Create a new event log
   */
  constructor();
  /**
   * Get current Merkle root as hex string
   */
  getRoot(): string;
  /**
   * Check if log is empty
   */
  isEmpty(): boolean;
}

export class EvolutionEngine {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Check if node should replicate (spawn similar node)
   */
  shouldReplicate(node_id: string): boolean;
  /**
   * Record node performance for fitness evaluation
   */
  recordPerformance(node_id: string, success_rate: number, throughput: number): void;
  /**
   * Get network fitness score
   */
  getNetworkFitness(): number;
  /**
   * Get recommended configuration for new nodes
   */
  getRecommendedConfig(): string;
  constructor();
  /**
   * Evolve patterns for next generation
   */
  evolve(): void;
}

export class FederatedModel {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get parameter dimension
   */
  getDimension(): number;
  /**
   * Get parameters as array
   */
  getParameters(): Float32Array;
  /**
   * Set parameters from array
   */
  setParameters(params: Float32Array): void;
  /**
   * Apply aggregated gradients to update model
   */
  applyGradients(gradients: Float32Array): void;
  /**
   * Set local epochs per round
   */
  setLocalEpochs(epochs: number): void;
  /**
   * Set learning rate
   */
  setLearningRate(lr: number): void;
  /**
   * Create a new federated model
   */
  constructor(dimension: number, learning_rate: number, momentum: number);
  /**
   * Get current round
   */
  getRound(): bigint;
}

export class FoundingRegistry {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Process epoch distribution
   */
  processEpoch(current_epoch: bigint, available_amount: bigint): any[];
  /**
   * Calculate vested amount for current epoch
   */
  calculateVested(current_epoch: bigint, pool_balance: bigint): bigint;
  /**
   * Get founding contributor count
   */
  getFounderCount(): number;
  /**
   * Register additional founding contributor
   */
  registerContributor(id: string, category: string, weight: number): void;
  constructor();
}

export class GenesisKey {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get ID as hex
   */
  getIdHex(): string;
  /**
   * Export ultra-compact genesis key (21 bytes only)
   */
  exportUltraCompact(): Uint8Array;
  /**
   * Create a new genesis key
   */
  constructor(creator: PiKey, epoch: number);
  /**
   * Get the φ-sized genesis ID
   */
  getId(): Uint8Array;
  /**
   * Verify this genesis key was created by a specific Pi-Key
   */
  verify(creator_public_key: Uint8Array): boolean;
  /**
   * Get epoch
   */
  getEpoch(): number;
}

export class GenesisSunset {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Check if it's safe to retire genesis nodes
   */
  canRetire(): boolean;
  /**
   * Get sunset status
   */
  getStatus(): string;
  /**
   * Check if genesis nodes should be read-only
   */
  isReadOnly(): boolean;
  /**
   * Get current sunset phase
   * 0 = Active (genesis required)
   * 1 = Transition (stop new connections)
   * 2 = Read-only (genesis read-only)
   * 3 = Retired (genesis can be removed)
   */
  getCurrentPhase(): number;
  /**
   * Update network node count
   */
  updateNodeCount(count: number): number;
  /**
   * Check if network is self-sustaining
   */
  isSelfSustaining(): boolean;
  /**
   * Register a genesis node
   */
  registerGenesisNode(node_id: string): void;
  /**
   * Check if genesis nodes should accept new connections
   */
  shouldAcceptConnections(): boolean;
  constructor();
}

export class GradientGossip {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get number of active peers
   */
  peerCount(): number;
  /**
   * Prune stale peer gradients
   */
  pruneStale(): number;
  /**
   * Configure differential privacy
   */
  configureDifferentialPrivacy(epsilon: number, sensitivity: number): void;
  /**
   * Advance to next consensus round
   */
  advanceRound(): bigint;
  /**
   * Get gradient dimension
   */
  getDimension(): number;
  /**
   * Enable/disable differential privacy
   */
  setDPEnabled(enabled: boolean): void;
  /**
   * Set model hash for version compatibility
   */
  setModelHash(hash: Uint8Array): void;
  /**
   * Get current consensus round
   */
  getCurrentRound(): bigint;
  /**
   * Set local gradients from JavaScript
   */
  setLocalGradients(gradients: Float32Array): void;
  /**
   * Get compression ratio achieved
   */
  getCompressionRatio(): number;
  /**
   * Get aggregated gradients as JavaScript array
   */
  getAggregatedGradients(): Float32Array;
  /**
   * Create a new GradientGossip instance
   *
   * # Arguments
   * * `local_peer_id` - 32-byte peer identifier
   * * `dimension` - Gradient vector dimension
   * * `k_ratio` - TopK sparsification ratio (0.1 = keep top 10%)
   */
  constructor(local_peer_id: Uint8Array, dimension: number, k_ratio: number);
  /**
   * Get statistics as JSON
   */
  getStats(): string;
}

export class ModelConsensusManager {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get number of tracked models
   */
  modelCount(): number;
  /**
   * Get number of active disputes
   */
  disputeCount(): number;
  /**
   * Get number of quarantined updates
   */
  quarantinedUpdateCount(): number;
  /**
   * Create a new model consensus manager
   */
  constructor(min_witnesses: number);
  /**
   * Get statistics as JSON
   */
  getStats(): string;
}

export class MultiHeadAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get embedding dimension
   */
  dim(): number;
  /**
   * Create new multi-head attention
   */
  constructor(dim: number, num_heads: number);
  /**
   * Get number of heads
   */
  numHeads(): number;
}

export class NetworkEvents {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get a subtle motivational message
   */
  getMotivation(balance: bigint): string;
  /**
   * Check for discovery triggers (Easter eggs)
   */
  checkDiscovery(action: string, node_id: string): string | undefined;
  /**
   * Get ASCII art for special occasions
   */
  getSpecialArt(): string | undefined;
  /**
   * Check milestone achievements
   */
  checkMilestones(balance: bigint, node_id: string): string;
  /**
   * Set current time (for testing)
   */
  setCurrentTime(timestamp: bigint): void;
  /**
   * Get network status with thematic flair
   */
  getThemedStatus(node_count: number, total_ruv: bigint): string;
  /**
   * Check for active special events
   */
  checkActiveEvents(): string;
  /**
   * Get celebration multiplier boost
   */
  getCelebrationBoost(): number;
  constructor();
}

export class NetworkLearning {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get pattern count
   */
  patternCount(): number;
  /**
   * Store a learned pattern
   */
  storePattern(pattern_json: string): number;
  /**
   * Look up similar patterns
   */
  lookupPatterns(query_json: string, k: number): string;
  /**
   * Get energy savings ratio for spike-driven attention
   */
  getEnergyRatio(seq_len: number, hidden_dim: number): number;
  /**
   * Get trajectory count
   */
  trajectoryCount(): number;
  /**
   * Record a task execution trajectory
   */
  recordTrajectory(trajectory_json: string): boolean;
  /**
   * Create new network learning intelligence
   */
  constructor();
  /**
   * Prune low-quality patterns
   */
  prune(min_usage: number, min_confidence: number): number;
  /**
   * Get combined statistics
   */
  getStats(): string;
}

export class NetworkTopology {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Register a node in the topology
   */
  registerNode(node_id: string, capabilities: Float32Array): void;
  /**
   * Get optimal peers for a node
   */
  getOptimalPeers(node_id: string, count: number): string[];
  /**
   * Update connection strength between nodes
   */
  updateConnection(from: string, to: string, success_rate: number): void;
  constructor();
}

export class NodeConfig {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Maximum CPU usage when idle (0.0 - 1.0)
   */
  cpu_limit: number;
  /**
   * Maximum memory usage in bytes
   */
  memory_limit: number;
  /**
   * Maximum bandwidth in bytes/sec
   */
  bandwidth_limit: number;
  /**
   * Minimum idle time before contributing (ms)
   */
  min_idle_time: number;
  /**
   * Whether to reduce contribution on battery
   */
  respect_battery: boolean;
}

export class NodeStats {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Total rUv (Resource Utility Vouchers) earned
   */
  ruv_earned: bigint;
  /**
   * Total rUv spent
   */
  ruv_spent: bigint;
  /**
   * Tasks completed
   */
  tasks_completed: bigint;
  /**
   * Tasks submitted
   */
  tasks_submitted: bigint;
  /**
   * Total uptime in seconds
   */
  uptime_seconds: bigint;
  /**
   * Current reputation score (0.0 - 1.0)
   */
  reputation: number;
  /**
   * Current contribution multiplier
   */
  multiplier: number;
  /**
   * Active lifecycle events
   */
  celebration_boost: number;
}

export class OptimizationEngine {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Record task routing outcome
   */
  recordRouting(task_type: string, node_id: string, latency_ms: bigint, success: boolean): void;
  /**
   * Get optimal node for a task type
   */
  selectOptimalNode(task_type: string, candidates: string[]): string;
  constructor();
  /**
   * Get optimization stats
   */
  getStats(): string;
}

export class PiKey {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get the Pi-sized identity (40 bytes)
   */
  getIdentity(): Uint8Array;
  /**
   * Get short identity (first 8 bytes as hex)
   */
  getShortId(): string;
  /**
   * Export minimal key representation (Pi + Phi sized = 61 bytes total)
   */
  exportCompact(): Uint8Array;
  /**
   * Get public key for verification
   */
  getPublicKey(): Uint8Array;
  /**
   * Verify this key has Pi magic marker
   */
  verifyPiMagic(): boolean;
  /**
   * Get identity as hex string
   */
  getIdentityHex(): string;
  /**
   * Restore from encrypted backup (supports both v1 legacy and v2 Argon2id)
   */
  static restoreFromBackup(backup: Uint8Array, password: string): PiKey;
  /**
   * Create encrypted backup of private key using Argon2id KDF
   */
  createEncryptedBackup(password: string): Uint8Array;
  /**
   * Get the Phi-sized genesis fingerprint (21 bytes)
   */
  getGenesisFingerprint(): Uint8Array;
  /**
   * Sign data with this key
   */
  sign(data: Uint8Array): Uint8Array;
  /**
   * Verify signature from another Pi-Key
   */
  verify(data: Uint8Array, signature: Uint8Array, public_key: Uint8Array): boolean;
  /**
   * Generate a new Pi-Key with genesis linking
   */
  constructor(genesis_seed?: Uint8Array | null);
  /**
   * Get key statistics
   */
  getStats(): string;
}

export class QDAGLedger {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Export ledger state for sync
   */
  exportState(): Uint8Array;
  /**
   * Import ledger state from sync
   */
  importState(state_bytes: Uint8Array): number;
  /**
   * Get total supply
   */
  totalSupply(): bigint;
  /**
   * Get staked amount for a node
   */
  stakedAmount(node_id: string): bigint;
  /**
   * Create genesis transaction (called once at network start)
   */
  createGenesis(initial_supply: bigint, founder_pubkey: Uint8Array): Uint8Array;
  /**
   * Get transaction count
   */
  transactionCount(): number;
  /**
   * Create and validate a new transaction
   */
  createTransaction(sender_id: string, recipient_id: string, amount: bigint, tx_type: number, sender_privkey: Uint8Array, sender_pubkey: Uint8Array): Uint8Array;
  /**
   * Create a new QDAG ledger
   */
  constructor();
  /**
   * Get balance for a node
   */
  balance(node_id: string): bigint;
  /**
   * Get tip count
   */
  tipCount(): number;
}

export class QuarantineManager {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get number of quarantined claims
   */
  quarantinedCount(): number;
  /**
   * Create a new quarantine manager
   */
  constructor();
  /**
   * Check if claim can be used in decisions
   */
  canUse(claim_id: string): boolean;
  /**
   * Check quarantine level for a claim
   */
  getLevel(claim_id: string): number;
  /**
   * Set quarantine level
   */
  setLevel(claim_id: string, level: number): void;
}

export class RacEconomicEngine {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get summary statistics as JSON
   */
  getSummary(): string;
  /**
   * Check if node can participate (has stake + reputation)
   */
  canParticipate(node_id: Uint8Array): boolean;
  /**
   * Get combined score (stake-weighted reputation)
   */
  getCombinedScore(node_id: Uint8Array): number;
  /**
   * Create a new RAC economic engine
   */
  constructor();
}

export class RacSemanticRouter {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get peer count
   */
  peerCount(): number;
  /**
   * Create a new semantic router
   */
  constructor();
}

export class RateLimiter {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Check if request is allowed
   */
  checkAllowed(node_id: string): boolean;
  constructor(window_ms: bigint, max_requests: number);
  /**
   * Reset rate limiter
   */
  reset(): void;
  /**
   * Get current count for a node
   */
  getCount(node_id: string): number;
}

export class ReasoningBank {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new ReasoningBank
   */
  constructor();
  /**
   * Get total pattern count
   */
  count(): number;
  /**
   * Prune low-quality patterns
   */
  prune(min_usage: number, min_confidence: number): number;
  /**
   * Store a new pattern (JSON format)
   */
  store(pattern_json: string): number;
  /**
   * Lookup most similar patterns (OPTIMIZED with spatial indexing)
   */
  lookup(query_json: string, k: number): string;
  /**
   * Get bank statistics
   */
  getStats(): string;
}

export class ReputationManager {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get number of tracked nodes
   */
  nodeCount(): number;
  /**
   * Get effective reputation for a node (with decay applied)
   */
  getReputation(node_id: Uint8Array): number;
  /**
   * Get average network reputation
   */
  averageReputation(): number;
  /**
   * Check if node has sufficient reputation
   */
  hasSufficientReputation(node_id: Uint8Array): boolean;
  /**
   * Create a new reputation manager
   */
  constructor(decay_rate: number, decay_interval_ms: bigint);
}

export class ReputationSystem {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get reputation score for a node
   */
  getReputation(node_id: string): number;
  /**
   * Record failed task completion
   */
  recordFailure(node_id: string): void;
  /**
   * Record penalty (fraud, invalid result)
   */
  recordPenalty(node_id: string, severity: number): void;
  /**
   * Record successful task completion
   */
  recordSuccess(node_id: string): void;
  /**
   * Check if node can participate
   */
  canParticipate(node_id: string): boolean;
  constructor();
}

export class RewardDistribution {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  total: bigint;
  contributor_share: bigint;
  treasury_share: bigint;
  protocol_share: bigint;
  founder_share: bigint;
}

export class RewardManager {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get number of pending rewards
   */
  pendingCount(): number;
  /**
   * Get total pending reward amount
   */
  pendingAmount(): bigint;
  /**
   * Get claimable rewards for a node
   */
  claimableAmount(node_id: Uint8Array): bigint;
  /**
   * Create a new reward manager
   */
  constructor(default_vesting_ms: bigint);
}

export class SemanticRouter {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get peer count
   */
  peerCount(): number;
  /**
   * Get topic count
   */
  topicCount(): number;
  /**
   * Create with custom parameters
   */
  static withParams(embedding_dim: number, semantic_neighbors: number, random_sample: number): SemanticRouter;
  /**
   * Set my peer identity
   */
  setMyPeerId(peer_id: Uint8Array): void;
  /**
   * Get active peer count (seen in last 60 seconds)
   */
  activePeerCount(): number;
  /**
   * Set my capabilities and update my centroid
   */
  setMyCapabilities(capabilities: string[]): void;
  /**
   * Create a new semantic router
   */
  constructor();
  /**
   * Get statistics as JSON
   */
  getStats(): string;
}

export class SessionKey {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get ID as hex
   */
  getIdHex(): string;
  /**
   * Check if session is expired
   */
  isExpired(): boolean;
  /**
   * Get parent identity fingerprint
   */
  getParentIdentity(): Uint8Array;
  /**
   * Create a new session key linked to a Pi-Key identity
   */
  constructor(parent: PiKey, ttl_seconds: number);
  /**
   * Get the e-sized session ID
   */
  getId(): Uint8Array;
  /**
   * Decrypt data with this session key
   */
  decrypt(data: Uint8Array): Uint8Array;
  /**
   * Encrypt data with this session key
   */
  encrypt(plaintext: Uint8Array): Uint8Array;
}

export class SpikeDrivenAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create with custom parameters
   */
  static withConfig(threshold: number, steps: number, refractory: number): SpikeDrivenAttention;
  /**
   * Estimate energy savings ratio compared to standard attention
   */
  energyRatio(seq_len: number, hidden_dim: number): number;
  /**
   * Create new spike-driven attention with default config
   */
  constructor();
}

export class SpotChecker {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Check if a task should include a spot-check
   */
  shouldCheck(): boolean;
  /**
   * Add a known challenge-response pair
   */
  addChallenge(task_type: string, input: Uint8Array, expected_output: Uint8Array): void;
  /**
   * Get a random challenge for a task type
   */
  getChallenge(task_type: string): Uint8Array | undefined;
  /**
   * Verify a challenge response
   */
  verifyResponse(input_hash: Uint8Array, output: Uint8Array): boolean;
  constructor(check_probability: number);
}

export class StakeManager {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get number of stakers
   */
  stakerCount(): number;
  /**
   * Get total staked amount in network
   */
  totalStaked(): bigint;
  /**
   * Get minimum stake requirement
   */
  getMinStake(): bigint;
  /**
   * Check if node has sufficient stake
   */
  hasSufficientStake(node_id: Uint8Array): boolean;
  /**
   * Create a new stake manager
   */
  constructor(min_stake: bigint);
  /**
   * Get staked amount for a node
   */
  getStake(node_id: Uint8Array): bigint;
}

export class SwarmIntelligence {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get queue size
   */
  queueSize(): number;
  /**
   * Set belief for a topic's decision
   */
  setBelief(topic: string, decision_id: bigint, probability: number): void;
  /**
   * Add pattern to collective memory
   */
  addPattern(pattern_json: string): boolean;
  /**
   * Run memory consolidation
   */
  consolidate(): number;
  /**
   * Check if topic has reached consensus
   */
  hasConsensus(topic: string): boolean;
  /**
   * Get collective memory pattern count
   */
  patternCount(): number;
  /**
   * Search collective memory
   */
  searchPatterns(query_json: string, k: number): string;
  /**
   * Start a new consensus round for a topic
   */
  startConsensus(topic: string, threshold: number): void;
  /**
   * Negotiate beliefs for a topic
   */
  negotiateBeliefs(topic: string, beliefs_json: string): boolean;
  /**
   * Get consensus decision for topic
   */
  getConsensusDecision(topic: string): bigint | undefined;
  /**
   * Create new swarm intelligence coordinator
   */
  constructor(node_id: string);
  /**
   * Run hippocampal replay
   */
  replay(): number;
  /**
   * Get node ID
   */
  nodeId(): string;
  /**
   * Get combined statistics as JSON
   */
  getStats(): string;
}

export class SybilDefense {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Register a node with its fingerprint
   */
  registerNode(node_id: string, fingerprint: string): boolean;
  /**
   * Get sybil score (0.0 = likely unique, 1.0 = likely sybil)
   */
  getSybilScore(node_id: string): number;
  /**
   * Check if node is likely a sybil
   */
  isSuspectedSybil(node_id: string): boolean;
  constructor();
}

/**
 * Task priority levels
 */
export enum TaskPriority {
  Low = 0,
  Normal = 1,
  High = 2,
}

/**
 * Task types supported by the network
 */
export enum TaskType {
  /**
   * Vector search in HNSW index
   */
  VectorSearch = 0,
  /**
   * Vector insertion
   */
  VectorInsert = 1,
  /**
   * Generate embeddings
   */
  Embedding = 2,
  /**
   * Semantic task-to-agent matching
   */
  SemanticMatch = 3,
  /**
   * Neural network inference
   */
  NeuralInference = 4,
  /**
   * AES encryption/decryption
   */
  Encryption = 5,
  /**
   * Data compression
   */
  Compression = 6,
  /**
   * Custom WASM module (requires verification)
   */
  CustomWasm = 7,
}

export class TopKSparsifier {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Reset error feedback buffer
   */
  resetErrorFeedback(): void;
  /**
   * Get compression ratio
   */
  getCompressionRatio(): number;
  /**
   * Get error feedback buffer size
   */
  getErrorBufferSize(): number;
  /**
   * Create a new TopK sparsifier
   *
   * # Arguments
   * * `k_ratio` - Fraction of gradients to keep (0.1 = top 10%)
   */
  constructor(k_ratio: number);
}

export class TrajectoryTracker {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new trajectory tracker
   */
  constructor(max_size: number);
  /**
   * Get count of trajectories
   */
  count(): number;
  /**
   * Record a new trajectory
   */
  record(trajectory_json: string): boolean;
  /**
   * Get statistics as JSON
   */
  getStats(): string;
}

export class WasmAdapterPool {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get or create an adapter for a task type
   */
  getAdapter(task_type: string): any;
  /**
   * Get adapter count
   */
  adapterCount(): number;
  /**
   * Export adapter to bytes for P2P sharing
   */
  exportAdapter(task_type: string): Uint8Array;
  /**
   * Import adapter from bytes
   */
  importAdapter(task_type: string, bytes: Uint8Array): boolean;
  /**
   * Route to best adapter by task embedding
   */
  routeToAdapter(task_embedding: Float32Array): any;
  /**
   * Create a new adapter pool
   */
  constructor(hidden_dim: number, max_slots: number);
  /**
   * Apply adapter to input
   */
  forward(task_type: string, input: Float32Array): Float32Array;
  /**
   * Get pool statistics
   */
  getStats(): any;
}

export class WasmCapabilities {
  free(): void;
  [Symbol.dispose](): void;
  enableHDC(): boolean;
  enableNAO(_quorum: number): boolean;
  enableWTA(_num_neurons: number, _inhibition: number, _threshold: number): boolean;
  competeWTA(_activations: Float32Array): number;
  enableBTSP(_input_dim: number, _time_constant: number): boolean;
  executeNAO(_proposal_id: string): boolean;
  /**
   * Get a summary of all enabled capabilities
   */
  getSummary(): any;
  proposeNAO(_action: string): string;
  forwardBTSP(_input: Float32Array): number;
  getNAOSync(): number;
  retrieveHDC(_key: string, _threshold: number): any;
  addNAOMember(_member_id: string, _stake: bigint): boolean;
  adaptMicroLoRA(_operator_type: string, _gradient: Float32Array): boolean;
  applyMicroLoRA(_operator_type: string, input: Float32Array): Float32Array;
  /**
   * List all available exotic capabilities
   */
  getCapabilities(): any;
  enableMicroLoRA(_dim: number, _rank: number): boolean;
  tickTimeCrystal(): any;
  growMorphogenetic(_rate: number): void;
  oneShotAssociate(_pattern: Float32Array, _target: number): boolean;
  enableTimeCrystal(_oscillators: number, _period_ms: number): boolean;
  pruneMorphogenetic(_threshold: number): void;
  enableMorphogenetic(_width: number, _height: number): boolean;
  getTimeCrystalSync(): number;
  broadcastToWorkspace(_content: Float32Array, _salience: number, _source_module: number): boolean;
  getWorkspaceContents(): any;
  isTimeCrystalStable(): boolean;
  enableGlobalWorkspace(_capacity: number): boolean;
  getMorphogeneticStats(): any;
  differentiateMorphogenetic(): void;
  getMorphogeneticCellCount(): number;
  /**
   * Create a new capabilities manager for a node
   */
  constructor(node_id: string);
  /**
   * Step all enabled capabilities forward (for main loop integration)
   */
  step(dt: number): void;
  tickNAO(_dt: number): void;
  voteNAO(_proposal_id: string, _weight: number): boolean;
  storeHDC(_key: string): boolean;
}

export class WasmCreditLedger {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get total spent
   */
  totalSpent(): bigint;
  /**
   * Export spent counter for sync
   */
  exportSpent(): Uint8Array;
  /**
   * Get total earned (before spending)
   */
  totalEarned(): bigint;
  /**
   * Export earned counter for sync
   */
  exportEarned(): Uint8Array;
  /**
   * Get staked amount
   */
  stakedAmount(): bigint;
  /**
   * Get network compute hours (for multiplier)
   */
  networkCompute(): number;
  /**
   * Get current multiplier
   */
  currentMultiplier(): number;
  /**
   * Update network compute (from P2P sync)
   */
  updateNetworkCompute(hours: number): void;
  /**
   * Create a new credit ledger
   */
  constructor(node_id: string);
  /**
   * Merge with another ledger (CRDT merge) - optimized batch processing
   */
  merge(other_earned: Uint8Array, other_spent: Uint8Array): void;
  /**
   * Slash staked credits (penalty for bad behavior)
   */
  slash(amount: bigint): bigint;
  /**
   * Stake credits for participation
   */
  stake(amount: bigint): void;
  /**
   * Credit the ledger (earn credits)
   */
  credit(amount: bigint, reason: string): void;
  /**
   * Deduct from the ledger (spend credits)
   */
  deduct(amount: bigint): void;
  /**
   * Get current balance
   */
  balance(): bigint;
  /**
   * Unstake credits
   */
  unstake(amount: bigint): void;
}

export class WasmIdleDetector {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get status summary
   */
  getStatus(): any;
  /**
   * Update FPS measurement
   */
  updateFps(fps: number): void;
  /**
   * Check if we should be working
   */
  shouldWork(): boolean;
  /**
   * Get current throttle level (0.0 - max_cpu)
   */
  getThrottle(): number;
  /**
   * Record user interaction
   */
  recordInteraction(): void;
  /**
   * Set battery status (called from JS)
   */
  setBatteryStatus(on_battery: boolean): void;
  /**
   * Create a new idle detector
   */
  constructor(max_cpu: number, min_idle_time: number);
  /**
   * Stop monitoring
   */
  stop(): void;
  /**
   * Pause contribution (user-initiated)
   */
  pause(): void;
  /**
   * Start monitoring
   */
  start(): void;
  /**
   * Resume contribution
   */
  resume(): void;
  /**
   * Check if user is idle
   */
  isIdle(): boolean;
}

export class WasmMcpBroadcast {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Set as server mode (responds to requests)
   */
  setServer(server: WasmMcpServer): void;
  /**
   * Create a broadcast transport
   */
  constructor(channel_name: string);
  /**
   * Send a request (client mode)
   */
  send(request_json: string): void;
  /**
   * Close the channel
   */
  close(): void;
  /**
   * Start listening for requests (server mode)
   */
  listen(): void;
}

export class WasmMcpServer {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create with custom configuration
   */
  static withConfig(config: any): WasmMcpServer;
  /**
   * Set identity for authenticated operations
   */
  setIdentity(identity: WasmNodeIdentity): void;
  /**
   * Initialize learning engine
   */
  initLearning(): void;
  /**
   * Handle an MCP request (JSON string)
   */
  handleRequest(request_json: string): Promise<string>;
  /**
   * Get server info
   */
  getServerInfo(): any;
  /**
   * Handle MCP request from JsValue (for direct JS calls)
   */
  handleRequestJs(request: any): Promise<any>;
  /**
   * Create a new MCP server with default configuration
   */
  constructor();
}

export class WasmMcpTransport {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create transport from a Worker
   */
  constructor(worker: Worker);
  /**
   * Initialize transport (set up message handler)
   */
  init(): void;
  /**
   * Send an MCP request and get a Promise for the response
   */
  send(request: any): Promise<any>;
  /**
   * Close the transport
   */
  close(): void;
  /**
   * Create transport from existing MessagePort
   */
  static fromPort(port: MessagePort): WasmMcpTransport;
}

export class WasmMcpWorkerHandler {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create handler with MCP server
   */
  constructor(server: WasmMcpServer);
  /**
   * Start handling messages (call in worker)
   */
  start(): void;
}

export class WasmNetworkManager {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get peer count
   */
  peerCount(): number;
  /**
   * Check if connected
   */
  isConnected(): boolean;
  /**
   * Register a peer
   */
  registerPeer(node_id: string, pubkey: Uint8Array, capabilities: string[], stake: bigint): void;
  /**
   * Select workers for task execution (reputation-weighted random)
   */
  selectWorkers(capability: string, count: number): string[];
  /**
   * Get active peer count (seen in last 60s)
   */
  activePeerCount(): number;
  /**
   * Update peer reputation
   */
  updateReputation(node_id: string, delta: number): void;
  /**
   * Get peers with specific capability
   */
  getPeersWithCapability(capability: string): string[];
  constructor(node_id: string);
  /**
   * Add a relay URL
   */
  addRelay(url: string): void;
}

export class WasmNodeIdentity {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Verify a signature from another node
   */
  static verifyFrom(public_key: Uint8Array, message: Uint8Array, signature: Uint8Array): boolean;
  /**
   * Get the public key as hex string
   */
  publicKeyHex(): string;
  /**
   * Restore identity from secret key bytes
   */
  static fromSecretKey(secret_key: Uint8Array, site_id: string): WasmNodeIdentity;
  /**
   * Get browser fingerprint
   */
  getFingerprint(): string | undefined;
  /**
   * Set browser fingerprint for anti-sybil
   */
  setFingerprint(fingerprint: string): void;
  /**
   * Get the public key as bytes
   */
  publicKeyBytes(): Uint8Array;
  /**
   * Export secret key encrypted with password (secure backup)
   * Uses Argon2id for key derivation and AES-256-GCM for encryption
   */
  exportSecretKey(password: string): Uint8Array;
  /**
   * Import secret key from encrypted backup
   */
  static importSecretKey(encrypted: Uint8Array, password: string, site_id: string): WasmNodeIdentity;
  /**
   * Sign a message
   */
  sign(message: Uint8Array): Uint8Array;
  /**
   * Verify a signature
   */
  verify(message: Uint8Array, signature: Uint8Array): boolean;
  /**
   * Get the node's unique identifier
   */
  nodeId(): string;
  /**
   * Get the site ID
   */
  siteId(): string;
  /**
   * Generate a new node identity
   */
  static generate(site_id: string): WasmNodeIdentity;
}

export class WasmStigmergy {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create with custom parameters
   */
  static withParams(decay_rate: number, deposit_rate: number, evaporation_hours: number): WasmStigmergy;
  /**
   * Export current state for P2P sharing
   */
  exportState(): string;
  /**
   * Get raw pheromone intensity
   */
  getIntensity(task_type: string): number;
  /**
   * Set minimum stake for anti-sybil
   */
  setMinStake(min_stake: bigint): void;
  /**
   * Should this node accept a task? (combined decision)
   */
  shouldAccept(task_type: string): number;
  /**
   * Check and run evaporation if due
   */
  maybeEvaporate(): boolean;
  /**
   * Get all task types ranked by attractiveness
   */
  getRankedTasks(): string;
  /**
   * Get success rate for a task type
   */
  getSuccessRate(task_type: string): number;
  /**
   * Get node's specialization score
   */
  getSpecialization(task_type: string): number;
  /**
   * Deposit with success/failure outcome
   */
  depositWithOutcome(task_type: string, peer_id: string, success: boolean, stake: bigint): void;
  /**
   * Update node specialization based on outcome
   */
  updateSpecialization(task_type: string, success: boolean): void;
  /**
   * Get best specialization recommendation
   */
  getBestSpecialization(): string | undefined;
  /**
   * Create a new stigmergy engine
   */
  constructor();
  /**
   * Merge peer pheromone state (JSON format)
   */
  merge(peer_state_json: string): boolean;
  /**
   * Get acceptance probability for a task type
   */
  follow(task_type: string): number;
  /**
   * Deposit pheromone after task completion
   */
  deposit(task_type: string, peer_id: string, success_rate: number, stake: bigint): void;
  /**
   * Run evaporation (call periodically)
   */
  evaporate(): void;
  /**
   * Get statistics as JSON
   */
  getStats(): string;
}

export class WasmTaskExecutor {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Set encryption key for payload decryption
   */
  setTaskKey(key: Uint8Array): void;
  /**
   * Create a new task executor
   */
  constructor(max_memory: number);
}

export class WasmTaskQueue {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
}

export class WasmWorkScheduler {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Calculate how many tasks to run this frame
   */
  tasksThisFrame(throttle: number): number;
  /**
   * Set pending task count
   */
  setPendingTasks(count: number): void;
  /**
   * Record task completion for averaging
   */
  recordTaskDuration(duration_ms: number): void;
  constructor();
}

export class WitnessTracker {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get witness count for a claim
   */
  witnessCount(claim_id: string): number;
  /**
   * Get confidence score based on witness diversity
   */
  witnessConfidence(claim_id: string): number;
  /**
   * Check if claim has sufficient independent witnesses
   */
  hasSufficientWitnesses(claim_id: string): boolean;
  /**
   * Create a new witness tracker
   */
  constructor(min_witnesses: number);
}

/**
 * Initialize panic hook for better error messages in console
 */
export function init_panic_hook(): void;
