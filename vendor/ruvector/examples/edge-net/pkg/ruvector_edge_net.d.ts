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

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_adaptivesecurity_free: (a: number, b: number) => void;
  readonly __wbg_adversarialsimulator_free: (a: number, b: number) => void;
  readonly __wbg_auditlog_free: (a: number, b: number) => void;
  readonly __wbg_browserfingerprint_free: (a: number, b: number) => void;
  readonly __wbg_byzantinedetector_free: (a: number, b: number) => void;
  readonly __wbg_coherenceengine_free: (a: number, b: number) => void;
  readonly __wbg_collectivememory_free: (a: number, b: number) => void;
  readonly __wbg_contributionstream_free: (a: number, b: number) => void;
  readonly __wbg_differentialprivacy_free: (a: number, b: number) => void;
  readonly __wbg_drifttracker_free: (a: number, b: number) => void;
  readonly __wbg_economicengine_free: (a: number, b: number) => void;
  readonly __wbg_economichealth_free: (a: number, b: number) => void;
  readonly __wbg_edgenetconfig_free: (a: number, b: number) => void;
  readonly __wbg_edgenetnode_free: (a: number, b: number) => void;
  readonly __wbg_entropyconsensus_free: (a: number, b: number) => void;
  readonly __wbg_eventlog_free: (a: number, b: number) => void;
  readonly __wbg_evolutionengine_free: (a: number, b: number) => void;
  readonly __wbg_federatedmodel_free: (a: number, b: number) => void;
  readonly __wbg_foundingregistry_free: (a: number, b: number) => void;
  readonly __wbg_genesiskey_free: (a: number, b: number) => void;
  readonly __wbg_genesissunset_free: (a: number, b: number) => void;
  readonly __wbg_get_economichealth_growth_rate: (a: number) => number;
  readonly __wbg_get_economichealth_stability: (a: number) => number;
  readonly __wbg_get_economichealth_utilization: (a: number) => number;
  readonly __wbg_get_economichealth_velocity: (a: number) => number;
  readonly __wbg_get_nodeconfig_bandwidth_limit: (a: number) => number;
  readonly __wbg_get_nodeconfig_memory_limit: (a: number) => number;
  readonly __wbg_get_nodeconfig_min_idle_time: (a: number) => number;
  readonly __wbg_get_nodeconfig_respect_battery: (a: number) => number;
  readonly __wbg_get_nodestats_celebration_boost: (a: number) => number;
  readonly __wbg_get_nodestats_multiplier: (a: number) => number;
  readonly __wbg_get_nodestats_reputation: (a: number) => number;
  readonly __wbg_get_nodestats_ruv_earned: (a: number) => bigint;
  readonly __wbg_get_nodestats_ruv_spent: (a: number) => bigint;
  readonly __wbg_get_nodestats_tasks_completed: (a: number) => bigint;
  readonly __wbg_get_nodestats_tasks_submitted: (a: number) => bigint;
  readonly __wbg_get_nodestats_uptime_seconds: (a: number) => bigint;
  readonly __wbg_gradientgossip_free: (a: number, b: number) => void;
  readonly __wbg_modelconsensusmanager_free: (a: number, b: number) => void;
  readonly __wbg_networkevents_free: (a: number, b: number) => void;
  readonly __wbg_networklearning_free: (a: number, b: number) => void;
  readonly __wbg_networktopology_free: (a: number, b: number) => void;
  readonly __wbg_nodeconfig_free: (a: number, b: number) => void;
  readonly __wbg_nodestats_free: (a: number, b: number) => void;
  readonly __wbg_optimizationengine_free: (a: number, b: number) => void;
  readonly __wbg_pikey_free: (a: number, b: number) => void;
  readonly __wbg_qdagledger_free: (a: number, b: number) => void;
  readonly __wbg_quarantinemanager_free: (a: number, b: number) => void;
  readonly __wbg_raceconomicengine_free: (a: number, b: number) => void;
  readonly __wbg_racsemanticrouter_free: (a: number, b: number) => void;
  readonly __wbg_ratelimiter_free: (a: number, b: number) => void;
  readonly __wbg_reasoningbank_free: (a: number, b: number) => void;
  readonly __wbg_reputationmanager_free: (a: number, b: number) => void;
  readonly __wbg_reputationsystem_free: (a: number, b: number) => void;
  readonly __wbg_rewarddistribution_free: (a: number, b: number) => void;
  readonly __wbg_rewardmanager_free: (a: number, b: number) => void;
  readonly __wbg_semanticrouter_free: (a: number, b: number) => void;
  readonly __wbg_sessionkey_free: (a: number, b: number) => void;
  readonly __wbg_set_economichealth_growth_rate: (a: number, b: number) => void;
  readonly __wbg_set_economichealth_stability: (a: number, b: number) => void;
  readonly __wbg_set_economichealth_utilization: (a: number, b: number) => void;
  readonly __wbg_set_economichealth_velocity: (a: number, b: number) => void;
  readonly __wbg_set_nodeconfig_bandwidth_limit: (a: number, b: number) => void;
  readonly __wbg_set_nodeconfig_memory_limit: (a: number, b: number) => void;
  readonly __wbg_set_nodeconfig_min_idle_time: (a: number, b: number) => void;
  readonly __wbg_set_nodeconfig_respect_battery: (a: number, b: number) => void;
  readonly __wbg_set_nodestats_celebration_boost: (a: number, b: number) => void;
  readonly __wbg_set_nodestats_multiplier: (a: number, b: number) => void;
  readonly __wbg_set_nodestats_reputation: (a: number, b: number) => void;
  readonly __wbg_set_nodestats_ruv_earned: (a: number, b: bigint) => void;
  readonly __wbg_set_nodestats_ruv_spent: (a: number, b: bigint) => void;
  readonly __wbg_set_nodestats_tasks_completed: (a: number, b: bigint) => void;
  readonly __wbg_set_nodestats_tasks_submitted: (a: number, b: bigint) => void;
  readonly __wbg_set_nodestats_uptime_seconds: (a: number, b: bigint) => void;
  readonly __wbg_spikedrivenattention_free: (a: number, b: number) => void;
  readonly __wbg_spotchecker_free: (a: number, b: number) => void;
  readonly __wbg_stakemanager_free: (a: number, b: number) => void;
  readonly __wbg_swarmintelligence_free: (a: number, b: number) => void;
  readonly __wbg_sybildefense_free: (a: number, b: number) => void;
  readonly __wbg_topksparsifier_free: (a: number, b: number) => void;
  readonly __wbg_trajectorytracker_free: (a: number, b: number) => void;
  readonly __wbg_wasmadapterpool_free: (a: number, b: number) => void;
  readonly __wbg_wasmcapabilities_free: (a: number, b: number) => void;
  readonly __wbg_wasmcreditledger_free: (a: number, b: number) => void;
  readonly __wbg_wasmidledetector_free: (a: number, b: number) => void;
  readonly __wbg_wasmmcpbroadcast_free: (a: number, b: number) => void;
  readonly __wbg_wasmmcpserver_free: (a: number, b: number) => void;
  readonly __wbg_wasmmcptransport_free: (a: number, b: number) => void;
  readonly __wbg_wasmmcpworkerhandler_free: (a: number, b: number) => void;
  readonly __wbg_wasmnetworkmanager_free: (a: number, b: number) => void;
  readonly __wbg_wasmnodeidentity_free: (a: number, b: number) => void;
  readonly __wbg_wasmstigmergy_free: (a: number, b: number) => void;
  readonly __wbg_wasmtaskexecutor_free: (a: number, b: number) => void;
  readonly __wbg_wasmtaskqueue_free: (a: number, b: number) => void;
  readonly __wbg_witnesstracker_free: (a: number, b: number) => void;
  readonly adaptivesecurity_chooseAction: (a: number, b: number, c: number, d: number, e: number) => [number, number];
  readonly adaptivesecurity_detectAttack: (a: number, b: number, c: number) => number;
  readonly adaptivesecurity_exportPatterns: (a: number) => [number, number, number, number];
  readonly adaptivesecurity_getMinReputation: (a: number) => number;
  readonly adaptivesecurity_getRateLimitMax: (a: number) => number;
  readonly adaptivesecurity_getRateLimitWindow: (a: number) => bigint;
  readonly adaptivesecurity_getSecurityLevel: (a: number) => number;
  readonly adaptivesecurity_getSpotCheckProbability: (a: number) => number;
  readonly adaptivesecurity_getStats: (a: number) => [number, number];
  readonly adaptivesecurity_importPatterns: (a: number, b: number, c: number) => [number, number];
  readonly adaptivesecurity_learn: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
  readonly adaptivesecurity_new: () => number;
  readonly adaptivesecurity_recordAttackPattern: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly adaptivesecurity_updateNetworkHealth: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly adversarialsimulator_enableChaosMode: (a: number, b: number) => void;
  readonly adversarialsimulator_generateChaosEvent: (a: number) => [number, number];
  readonly adversarialsimulator_getDefenceMetrics: (a: number) => [number, number];
  readonly adversarialsimulator_getRecommendations: (a: number) => [number, number];
  readonly adversarialsimulator_new: () => number;
  readonly adversarialsimulator_runSecurityAudit: (a: number) => [number, number];
  readonly adversarialsimulator_simulateByzantine: (a: number, b: number, c: number) => [number, number];
  readonly adversarialsimulator_simulateDDoS: (a: number, b: number, c: bigint) => [number, number];
  readonly adversarialsimulator_simulateDoubleSpend: (a: number, b: bigint, c: number) => [number, number];
  readonly adversarialsimulator_simulateFreeRiding: (a: number, b: number, c: number) => [number, number];
  readonly adversarialsimulator_simulateResultTampering: (a: number, b: number) => [number, number];
  readonly adversarialsimulator_simulateSybil: (a: number, b: number, c: number) => [number, number];
  readonly auditlog_exportEvents: (a: number) => [number, number];
  readonly auditlog_getEventsBySeverity: (a: number, b: number) => number;
  readonly auditlog_getEventsForNode: (a: number, b: number, c: number) => number;
  readonly auditlog_log: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
  readonly auditlog_new: () => number;
  readonly browserfingerprint_generate: () => any;
  readonly byzantinedetector_getMaxMagnitude: (a: number) => number;
  readonly byzantinedetector_new: (a: number, b: number) => number;
  readonly coherenceengine_canUseClaim: (a: number, b: number, c: number) => number;
  readonly coherenceengine_conflictCount: (a: number) => number;
  readonly coherenceengine_eventCount: (a: number) => number;
  readonly coherenceengine_getDrift: (a: number, b: number, c: number) => number;
  readonly coherenceengine_getMerkleRoot: (a: number) => [number, number];
  readonly coherenceengine_getQuarantineLevel: (a: number, b: number, c: number) => number;
  readonly coherenceengine_getStats: (a: number) => [number, number];
  readonly coherenceengine_hasDrifted: (a: number, b: number, c: number) => number;
  readonly coherenceengine_hasSufficientWitnesses: (a: number, b: number, c: number) => number;
  readonly coherenceengine_new: () => number;
  readonly coherenceengine_quarantinedCount: (a: number) => number;
  readonly coherenceengine_witnessCount: (a: number, b: number, c: number) => number;
  readonly collectivememory_consolidate: (a: number) => number;
  readonly collectivememory_getStats: (a: number) => [number, number];
  readonly collectivememory_hasPattern: (a: number, b: number, c: number) => number;
  readonly collectivememory_new: (a: number, b: number) => number;
  readonly collectivememory_patternCount: (a: number) => number;
  readonly collectivememory_queueSize: (a: number) => number;
  readonly collectivememory_search: (a: number, b: number, c: number, d: number) => [number, number];
  readonly contributionstream_getTotalDistributed: (a: number) => bigint;
  readonly contributionstream_isHealthy: (a: number) => number;
  readonly contributionstream_new: () => number;
  readonly contributionstream_processFees: (a: number, b: bigint, c: bigint) => bigint;
  readonly differentialprivacy_getEpsilon: (a: number) => number;
  readonly differentialprivacy_isEnabled: (a: number) => number;
  readonly differentialprivacy_new: (a: number, b: number) => number;
  readonly differentialprivacy_setEnabled: (a: number, b: number) => void;
  readonly drifttracker_getDrift: (a: number, b: number, c: number) => number;
  readonly drifttracker_getDriftedContexts: (a: number) => [number, number];
  readonly drifttracker_hasDrifted: (a: number, b: number, c: number) => number;
  readonly drifttracker_new: (a: number) => number;
  readonly economicengine_advanceEpoch: (a: number) => void;
  readonly economicengine_getHealth: (a: number) => number;
  readonly economicengine_getProtocolFund: (a: number) => bigint;
  readonly economicengine_getTreasury: (a: number) => bigint;
  readonly economicengine_isSelfSustaining: (a: number, b: number, c: bigint) => number;
  readonly economicengine_new: () => number;
  readonly economicengine_processReward: (a: number, b: bigint, c: number) => number;
  readonly edgenetconfig_addRelay: (a: number, b: number, c: number) => number;
  readonly edgenetconfig_build: (a: number) => [number, number, number];
  readonly edgenetconfig_cpuLimit: (a: number, b: number) => number;
  readonly edgenetconfig_memoryLimit: (a: number, b: number) => number;
  readonly edgenetconfig_minIdleTime: (a: number, b: number) => number;
  readonly edgenetconfig_new: (a: number, b: number) => number;
  readonly edgenetconfig_respectBattery: (a: number, b: number) => number;
  readonly edgenetnode_canUseClaim: (a: number, b: number, c: number) => number;
  readonly edgenetnode_checkEvents: (a: number) => [number, number];
  readonly edgenetnode_creditBalance: (a: number) => bigint;
  readonly edgenetnode_disconnect: (a: number) => [number, number];
  readonly edgenetnode_enableBTSP: (a: number, b: number) => number;
  readonly edgenetnode_enableHDC: (a: number) => number;
  readonly edgenetnode_enableNAO: (a: number, b: number) => number;
  readonly edgenetnode_getCapabilities: (a: number) => any;
  readonly edgenetnode_getCapabilitiesSummary: (a: number) => any;
  readonly edgenetnode_getClaimQuarantineLevel: (a: number, b: number, c: number) => number;
  readonly edgenetnode_getCoherenceEventCount: (a: number) => number;
  readonly edgenetnode_getCoherenceStats: (a: number) => [number, number];
  readonly edgenetnode_getConflictCount: (a: number) => number;
  readonly edgenetnode_getEconomicHealth: (a: number) => [number, number];
  readonly edgenetnode_getEnergyEfficiency: (a: number, b: number, c: number) => number;
  readonly edgenetnode_getFounderCount: (a: number) => number;
  readonly edgenetnode_getLearningStats: (a: number) => [number, number];
  readonly edgenetnode_getMerkleRoot: (a: number) => [number, number];
  readonly edgenetnode_getMotivation: (a: number) => [number, number];
  readonly edgenetnode_getMultiplier: (a: number) => number;
  readonly edgenetnode_getNetworkFitness: (a: number) => number;
  readonly edgenetnode_getOptimalPeers: (a: number, b: number) => [number, number];
  readonly edgenetnode_getOptimizationStats: (a: number) => [number, number];
  readonly edgenetnode_getPatternCount: (a: number) => number;
  readonly edgenetnode_getProtocolFund: (a: number) => bigint;
  readonly edgenetnode_getQuarantinedCount: (a: number) => number;
  readonly edgenetnode_getRecommendedConfig: (a: number) => [number, number];
  readonly edgenetnode_getStats: (a: number) => number;
  readonly edgenetnode_getThemedStatus: (a: number, b: number) => [number, number];
  readonly edgenetnode_getThrottle: (a: number) => number;
  readonly edgenetnode_getTimeCrystalSync: (a: number) => number;
  readonly edgenetnode_getTrajectoryCount: (a: number) => number;
  readonly edgenetnode_getTreasury: (a: number) => bigint;
  readonly edgenetnode_isIdle: (a: number) => number;
  readonly edgenetnode_isSelfSustaining: (a: number, b: number, c: bigint) => number;
  readonly edgenetnode_isStreamHealthy: (a: number) => number;
  readonly edgenetnode_lookupPatterns: (a: number, b: number, c: number, d: number) => [number, number];
  readonly edgenetnode_new: (a: number, b: number, c: number) => [number, number, number];
  readonly edgenetnode_nodeId: (a: number) => [number, number];
  readonly edgenetnode_pause: (a: number) => void;
  readonly edgenetnode_processEpoch: (a: number) => void;
  readonly edgenetnode_processNextTask: (a: number) => any;
  readonly edgenetnode_proposeNAO: (a: number, b: number, c: number) => [number, number];
  readonly edgenetnode_prunePatterns: (a: number, b: number, c: number) => number;
  readonly edgenetnode_recordLearningTrajectory: (a: number, b: number, c: number) => number;
  readonly edgenetnode_recordPeerInteraction: (a: number, b: number, c: number, d: number) => void;
  readonly edgenetnode_recordPerformance: (a: number, b: number, c: number) => void;
  readonly edgenetnode_recordTaskRouting: (a: number, b: number, c: number, d: number, e: number, f: bigint, g: number) => void;
  readonly edgenetnode_resume: (a: number) => void;
  readonly edgenetnode_runSecurityAudit: (a: number) => [number, number];
  readonly edgenetnode_shouldReplicate: (a: number) => number;
  readonly edgenetnode_start: (a: number) => [number, number];
  readonly edgenetnode_stepCapabilities: (a: number, b: number) => void;
  readonly edgenetnode_storePattern: (a: number, b: number, c: number) => number;
  readonly edgenetnode_submitTask: (a: number, b: number, c: number, d: number, e: number, f: bigint) => any;
  readonly edgenetnode_voteNAO: (a: number, b: number, c: number, d: number) => number;
  readonly entropyconsensus_converged: (a: number) => number;
  readonly entropyconsensus_entropy: (a: number) => number;
  readonly entropyconsensus_finalize_beliefs: (a: number) => void;
  readonly entropyconsensus_getBelief: (a: number, b: bigint) => number;
  readonly entropyconsensus_getDecision: (a: number) => [number, bigint];
  readonly entropyconsensus_getEntropyHistory: (a: number) => [number, number];
  readonly entropyconsensus_getEntropyThreshold: (a: number) => number;
  readonly entropyconsensus_getRounds: (a: number) => number;
  readonly entropyconsensus_getStats: (a: number) => [number, number];
  readonly entropyconsensus_getTemperature: (a: number) => number;
  readonly entropyconsensus_hasTimedOut: (a: number) => number;
  readonly entropyconsensus_new: () => number;
  readonly entropyconsensus_optionCount: (a: number) => number;
  readonly entropyconsensus_reset: (a: number) => void;
  readonly entropyconsensus_setBelief: (a: number, b: bigint, c: number) => void;
  readonly entropyconsensus_set_belief_raw: (a: number, b: bigint, c: number) => void;
  readonly entropyconsensus_withThreshold: (a: number) => number;
  readonly eventlog_getRoot: (a: number) => [number, number];
  readonly eventlog_isEmpty: (a: number) => number;
  readonly eventlog_len: (a: number) => number;
  readonly eventlog_new: () => number;
  readonly evolutionengine_evolve: (a: number) => void;
  readonly evolutionengine_getNetworkFitness: (a: number) => number;
  readonly evolutionengine_getRecommendedConfig: (a: number) => [number, number];
  readonly evolutionengine_new: () => number;
  readonly evolutionengine_recordPerformance: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly evolutionengine_shouldReplicate: (a: number, b: number, c: number) => number;
  readonly federatedmodel_applyGradients: (a: number, b: number, c: number) => [number, number];
  readonly federatedmodel_getDimension: (a: number) => number;
  readonly federatedmodel_getParameters: (a: number) => [number, number];
  readonly federatedmodel_getRound: (a: number) => bigint;
  readonly federatedmodel_new: (a: number, b: number, c: number) => number;
  readonly federatedmodel_setLearningRate: (a: number, b: number) => void;
  readonly federatedmodel_setLocalEpochs: (a: number, b: number) => void;
  readonly federatedmodel_setParameters: (a: number, b: number, c: number) => [number, number];
  readonly foundingregistry_calculateVested: (a: number, b: bigint, c: bigint) => bigint;
  readonly foundingregistry_getFounderCount: (a: number) => number;
  readonly foundingregistry_new: () => number;
  readonly foundingregistry_processEpoch: (a: number, b: bigint, c: bigint) => [number, number];
  readonly foundingregistry_registerContributor: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly genesiskey_create: (a: number, b: number) => [number, number, number];
  readonly genesiskey_exportUltraCompact: (a: number) => [number, number];
  readonly genesiskey_getEpoch: (a: number) => number;
  readonly genesiskey_getIdHex: (a: number) => [number, number];
  readonly genesiskey_verify: (a: number, b: number, c: number) => number;
  readonly genesissunset_canRetire: (a: number) => number;
  readonly genesissunset_getCurrentPhase: (a: number) => number;
  readonly genesissunset_getStatus: (a: number) => [number, number];
  readonly genesissunset_isReadOnly: (a: number) => number;
  readonly genesissunset_new: () => number;
  readonly genesissunset_registerGenesisNode: (a: number, b: number, c: number) => void;
  readonly genesissunset_shouldAcceptConnections: (a: number) => number;
  readonly genesissunset_updateNodeCount: (a: number, b: number) => number;
  readonly gradientgossip_advanceRound: (a: number) => bigint;
  readonly gradientgossip_configureDifferentialPrivacy: (a: number, b: number, c: number) => void;
  readonly gradientgossip_getAggregatedGradients: (a: number) => [number, number];
  readonly gradientgossip_getCompressionRatio: (a: number) => number;
  readonly gradientgossip_getCurrentRound: (a: number) => bigint;
  readonly gradientgossip_getDimension: (a: number) => number;
  readonly gradientgossip_getStats: (a: number) => [number, number];
  readonly gradientgossip_new: (a: number, b: number, c: number, d: number) => [number, number, number];
  readonly gradientgossip_peerCount: (a: number) => number;
  readonly gradientgossip_pruneStale: (a: number) => number;
  readonly gradientgossip_setDPEnabled: (a: number, b: number) => void;
  readonly gradientgossip_setLocalGradients: (a: number, b: number, c: number) => [number, number];
  readonly gradientgossip_setModelHash: (a: number, b: number, c: number) => [number, number];
  readonly init_panic_hook: () => void;
  readonly modelconsensusmanager_disputeCount: (a: number) => number;
  readonly modelconsensusmanager_getStats: (a: number) => [number, number];
  readonly modelconsensusmanager_modelCount: (a: number) => number;
  readonly modelconsensusmanager_new: (a: number) => number;
  readonly modelconsensusmanager_quarantinedUpdateCount: (a: number) => number;
  readonly multiheadattention_dim: (a: number) => number;
  readonly multiheadattention_new: (a: number, b: number) => number;
  readonly multiheadattention_numHeads: (a: number) => number;
  readonly networkevents_checkActiveEvents: (a: number) => [number, number];
  readonly networkevents_checkDiscovery: (a: number, b: number, c: number, d: number, e: number) => [number, number];
  readonly networkevents_checkMilestones: (a: number, b: bigint, c: number, d: number) => [number, number];
  readonly networkevents_getCelebrationBoost: (a: number) => number;
  readonly networkevents_getMotivation: (a: number, b: bigint) => [number, number];
  readonly networkevents_getSpecialArt: (a: number) => [number, number];
  readonly networkevents_getThemedStatus: (a: number, b: number, c: bigint) => [number, number];
  readonly networkevents_new: () => number;
  readonly networkevents_setCurrentTime: (a: number, b: bigint) => void;
  readonly networklearning_getEnergyRatio: (a: number, b: number, c: number) => number;
  readonly networklearning_getStats: (a: number) => [number, number];
  readonly networklearning_lookupPatterns: (a: number, b: number, c: number, d: number) => [number, number];
  readonly networklearning_new: () => number;
  readonly networklearning_patternCount: (a: number) => number;
  readonly networklearning_prune: (a: number, b: number, c: number) => number;
  readonly networklearning_recordTrajectory: (a: number, b: number, c: number) => number;
  readonly networklearning_storePattern: (a: number, b: number, c: number) => number;
  readonly networklearning_trajectoryCount: (a: number) => number;
  readonly networktopology_getOptimalPeers: (a: number, b: number, c: number, d: number) => [number, number];
  readonly networktopology_new: () => number;
  readonly networktopology_registerNode: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly networktopology_updateConnection: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly optimizationengine_getStats: (a: number) => [number, number];
  readonly optimizationengine_new: () => number;
  readonly optimizationengine_recordRouting: (a: number, b: number, c: number, d: number, e: number, f: bigint, g: number) => void;
  readonly optimizationengine_selectOptimalNode: (a: number, b: number, c: number, d: number, e: number) => [number, number];
  readonly pikey_createEncryptedBackup: (a: number, b: number, c: number) => [number, number, number, number];
  readonly pikey_exportCompact: (a: number) => [number, number];
  readonly pikey_generate: (a: number, b: number) => [number, number, number];
  readonly pikey_getGenesisFingerprint: (a: number) => [number, number];
  readonly pikey_getIdentity: (a: number) => [number, number];
  readonly pikey_getIdentityHex: (a: number) => [number, number];
  readonly pikey_getPublicKey: (a: number) => [number, number];
  readonly pikey_getShortId: (a: number) => [number, number];
  readonly pikey_getStats: (a: number) => [number, number];
  readonly pikey_restoreFromBackup: (a: number, b: number, c: number, d: number) => [number, number, number];
  readonly pikey_sign: (a: number, b: number, c: number) => [number, number];
  readonly pikey_verify: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
  readonly pikey_verifyPiMagic: (a: number) => number;
  readonly qdagledger_balance: (a: number, b: number, c: number) => bigint;
  readonly qdagledger_createGenesis: (a: number, b: bigint, c: number, d: number) => [number, number, number, number];
  readonly qdagledger_createTransaction: (a: number, b: number, c: number, d: number, e: number, f: bigint, g: number, h: number, i: number, j: number, k: number) => [number, number, number, number];
  readonly qdagledger_exportState: (a: number) => [number, number, number, number];
  readonly qdagledger_importState: (a: number, b: number, c: number) => [number, number, number];
  readonly qdagledger_new: () => number;
  readonly qdagledger_stakedAmount: (a: number, b: number, c: number) => bigint;
  readonly qdagledger_tipCount: (a: number) => number;
  readonly qdagledger_totalSupply: (a: number) => bigint;
  readonly qdagledger_transactionCount: (a: number) => number;
  readonly quarantinemanager_canUse: (a: number, b: number, c: number) => number;
  readonly quarantinemanager_getLevel: (a: number, b: number, c: number) => number;
  readonly quarantinemanager_new: () => number;
  readonly quarantinemanager_quarantinedCount: (a: number) => number;
  readonly quarantinemanager_setLevel: (a: number, b: number, c: number, d: number) => void;
  readonly raceconomicengine_canParticipate: (a: number, b: number, c: number) => number;
  readonly raceconomicengine_getCombinedScore: (a: number, b: number, c: number) => number;
  readonly raceconomicengine_getSummary: (a: number) => [number, number];
  readonly raceconomicengine_new: () => number;
  readonly racsemanticrouter_new: () => number;
  readonly racsemanticrouter_peerCount: (a: number) => number;
  readonly ratelimiter_checkAllowed: (a: number, b: number, c: number) => number;
  readonly ratelimiter_getCount: (a: number, b: number, c: number) => number;
  readonly ratelimiter_new: (a: bigint, b: number) => number;
  readonly ratelimiter_reset: (a: number) => void;
  readonly reasoningbank_count: (a: number) => number;
  readonly reasoningbank_getStats: (a: number) => [number, number];
  readonly reasoningbank_lookup: (a: number, b: number, c: number, d: number) => [number, number];
  readonly reasoningbank_new: () => number;
  readonly reasoningbank_prune: (a: number, b: number, c: number) => number;
  readonly reasoningbank_store: (a: number, b: number, c: number) => number;
  readonly reputationmanager_averageReputation: (a: number) => number;
  readonly reputationmanager_getReputation: (a: number, b: number, c: number) => number;
  readonly reputationmanager_hasSufficientReputation: (a: number, b: number, c: number) => number;
  readonly reputationmanager_new: (a: number, b: bigint) => number;
  readonly reputationmanager_nodeCount: (a: number) => number;
  readonly reputationsystem_canParticipate: (a: number, b: number, c: number) => number;
  readonly reputationsystem_getReputation: (a: number, b: number, c: number) => number;
  readonly reputationsystem_new: () => number;
  readonly reputationsystem_recordFailure: (a: number, b: number, c: number) => void;
  readonly reputationsystem_recordPenalty: (a: number, b: number, c: number, d: number) => void;
  readonly reputationsystem_recordSuccess: (a: number, b: number, c: number) => void;
  readonly rewardmanager_claimableAmount: (a: number, b: number, c: number) => bigint;
  readonly rewardmanager_new: (a: bigint) => number;
  readonly rewardmanager_pendingAmount: (a: number) => bigint;
  readonly rewardmanager_pendingCount: (a: number) => number;
  readonly semanticrouter_activePeerCount: (a: number) => number;
  readonly semanticrouter_getStats: (a: number) => [number, number];
  readonly semanticrouter_new: () => number;
  readonly semanticrouter_peerCount: (a: number) => number;
  readonly semanticrouter_setMyCapabilities: (a: number, b: number, c: number) => void;
  readonly semanticrouter_setMyPeerId: (a: number, b: number, c: number) => void;
  readonly semanticrouter_topicCount: (a: number) => number;
  readonly semanticrouter_withParams: (a: number, b: number, c: number) => number;
  readonly sessionkey_create: (a: number, b: number) => [number, number, number];
  readonly sessionkey_decrypt: (a: number, b: number, c: number) => [number, number, number, number];
  readonly sessionkey_encrypt: (a: number, b: number, c: number) => [number, number, number, number];
  readonly sessionkey_getId: (a: number) => [number, number];
  readonly sessionkey_getIdHex: (a: number) => [number, number];
  readonly sessionkey_getParentIdentity: (a: number) => [number, number];
  readonly sessionkey_isExpired: (a: number) => number;
  readonly spikedrivenattention_energyRatio: (a: number, b: number, c: number) => number;
  readonly spikedrivenattention_new: () => number;
  readonly spikedrivenattention_withConfig: (a: number, b: number, c: number) => number;
  readonly spotchecker_addChallenge: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
  readonly spotchecker_getChallenge: (a: number, b: number, c: number) => [number, number];
  readonly spotchecker_new: (a: number) => number;
  readonly spotchecker_shouldCheck: (a: number) => number;
  readonly spotchecker_verifyResponse: (a: number, b: number, c: number, d: number, e: number) => number;
  readonly stakemanager_getMinStake: (a: number) => bigint;
  readonly stakemanager_getStake: (a: number, b: number, c: number) => bigint;
  readonly stakemanager_hasSufficientStake: (a: number, b: number, c: number) => number;
  readonly stakemanager_new: (a: bigint) => number;
  readonly stakemanager_stakerCount: (a: number) => number;
  readonly stakemanager_totalStaked: (a: number) => bigint;
  readonly swarmintelligence_addPattern: (a: number, b: number, c: number) => number;
  readonly swarmintelligence_consolidate: (a: number) => number;
  readonly swarmintelligence_getConsensusDecision: (a: number, b: number, c: number) => [number, bigint];
  readonly swarmintelligence_getStats: (a: number) => [number, number];
  readonly swarmintelligence_hasConsensus: (a: number, b: number, c: number) => number;
  readonly swarmintelligence_negotiateBeliefs: (a: number, b: number, c: number, d: number, e: number) => number;
  readonly swarmintelligence_new: (a: number, b: number) => number;
  readonly swarmintelligence_nodeId: (a: number) => [number, number];
  readonly swarmintelligence_patternCount: (a: number) => number;
  readonly swarmintelligence_queueSize: (a: number) => number;
  readonly swarmintelligence_replay: (a: number) => number;
  readonly swarmintelligence_searchPatterns: (a: number, b: number, c: number, d: number) => [number, number];
  readonly swarmintelligence_setBelief: (a: number, b: number, c: number, d: bigint, e: number) => void;
  readonly swarmintelligence_startConsensus: (a: number, b: number, c: number, d: number) => void;
  readonly sybildefense_getSybilScore: (a: number, b: number, c: number) => number;
  readonly sybildefense_isSuspectedSybil: (a: number, b: number, c: number) => number;
  readonly sybildefense_new: () => number;
  readonly sybildefense_registerNode: (a: number, b: number, c: number, d: number, e: number) => number;
  readonly topksparsifier_getCompressionRatio: (a: number) => number;
  readonly topksparsifier_getErrorBufferSize: (a: number) => number;
  readonly topksparsifier_new: (a: number) => number;
  readonly topksparsifier_resetErrorFeedback: (a: number) => void;
  readonly trajectorytracker_count: (a: number) => number;
  readonly trajectorytracker_getStats: (a: number) => [number, number];
  readonly trajectorytracker_new: (a: number) => number;
  readonly trajectorytracker_record: (a: number, b: number, c: number) => number;
  readonly wasmadapterpool_adapterCount: (a: number) => number;
  readonly wasmadapterpool_exportAdapter: (a: number, b: number, c: number) => [number, number];
  readonly wasmadapterpool_forward: (a: number, b: number, c: number, d: number, e: number) => [number, number];
  readonly wasmadapterpool_getAdapter: (a: number, b: number, c: number) => any;
  readonly wasmadapterpool_getStats: (a: number) => any;
  readonly wasmadapterpool_importAdapter: (a: number, b: number, c: number, d: number, e: number) => number;
  readonly wasmadapterpool_new: (a: number, b: number) => number;
  readonly wasmadapterpool_routeToAdapter: (a: number, b: number, c: number) => any;
  readonly wasmcapabilities_adaptMicroLoRA: (a: number, b: number, c: number, d: number, e: number) => number;
  readonly wasmcapabilities_addNAOMember: (a: number, b: number, c: number, d: bigint) => number;
  readonly wasmcapabilities_applyMicroLoRA: (a: number, b: number, c: number, d: number, e: number) => [number, number];
  readonly wasmcapabilities_broadcastToWorkspace: (a: number, b: number, c: number, d: number, e: number) => number;
  readonly wasmcapabilities_competeWTA: (a: number, b: number, c: number) => number;
  readonly wasmcapabilities_differentiateMorphogenetic: (a: number) => void;
  readonly wasmcapabilities_enableBTSP: (a: number, b: number, c: number) => number;
  readonly wasmcapabilities_enableGlobalWorkspace: (a: number, b: number) => number;
  readonly wasmcapabilities_enableHDC: (a: number) => number;
  readonly wasmcapabilities_enableMicroLoRA: (a: number, b: number, c: number) => number;
  readonly wasmcapabilities_enableNAO: (a: number, b: number) => number;
  readonly wasmcapabilities_enableWTA: (a: number, b: number, c: number, d: number) => number;
  readonly wasmcapabilities_executeNAO: (a: number, b: number, c: number) => number;
  readonly wasmcapabilities_forwardBTSP: (a: number, b: number, c: number) => number;
  readonly wasmcapabilities_getCapabilities: (a: number) => any;
  readonly wasmcapabilities_getMorphogeneticCellCount: (a: number) => number;
  readonly wasmcapabilities_getMorphogeneticStats: (a: number) => any;
  readonly wasmcapabilities_getNAOSync: (a: number) => number;
  readonly wasmcapabilities_getSummary: (a: number) => any;
  readonly wasmcapabilities_growMorphogenetic: (a: number, b: number) => void;
  readonly wasmcapabilities_new: (a: number, b: number) => number;
  readonly wasmcapabilities_oneShotAssociate: (a: number, b: number, c: number, d: number) => number;
  readonly wasmcapabilities_proposeNAO: (a: number, b: number, c: number) => [number, number];
  readonly wasmcapabilities_retrieveHDC: (a: number, b: number, c: number, d: number) => any;
  readonly wasmcapabilities_tickTimeCrystal: (a: number) => any;
  readonly wasmcapabilities_voteNAO: (a: number, b: number, c: number, d: number) => number;
  readonly wasmcreditledger_balance: (a: number) => bigint;
  readonly wasmcreditledger_credit: (a: number, b: bigint, c: number, d: number) => [number, number];
  readonly wasmcreditledger_currentMultiplier: (a: number) => number;
  readonly wasmcreditledger_deduct: (a: number, b: bigint) => [number, number];
  readonly wasmcreditledger_exportEarned: (a: number) => [number, number, number, number];
  readonly wasmcreditledger_exportSpent: (a: number) => [number, number, number, number];
  readonly wasmcreditledger_merge: (a: number, b: number, c: number, d: number, e: number) => [number, number];
  readonly wasmcreditledger_networkCompute: (a: number) => number;
  readonly wasmcreditledger_new: (a: number, b: number) => [number, number, number];
  readonly wasmcreditledger_slash: (a: number, b: bigint) => [bigint, number, number];
  readonly wasmcreditledger_stake: (a: number, b: bigint) => [number, number];
  readonly wasmcreditledger_stakedAmount: (a: number) => bigint;
  readonly wasmcreditledger_totalEarned: (a: number) => bigint;
  readonly wasmcreditledger_totalSpent: (a: number) => bigint;
  readonly wasmcreditledger_unstake: (a: number, b: bigint) => [number, number];
  readonly wasmcreditledger_updateNetworkCompute: (a: number, b: number) => void;
  readonly wasmidledetector_getStatus: (a: number) => any;
  readonly wasmidledetector_getThrottle: (a: number) => number;
  readonly wasmidledetector_isIdle: (a: number) => number;
  readonly wasmidledetector_new: (a: number, b: number) => [number, number, number];
  readonly wasmidledetector_pause: (a: number) => void;
  readonly wasmidledetector_recordInteraction: (a: number) => void;
  readonly wasmidledetector_resume: (a: number) => void;
  readonly wasmidledetector_setBatteryStatus: (a: number, b: number) => void;
  readonly wasmidledetector_shouldWork: (a: number) => number;
  readonly wasmidledetector_start: (a: number) => [number, number];
  readonly wasmidledetector_stop: (a: number) => void;
  readonly wasmidledetector_updateFps: (a: number, b: number) => void;
  readonly wasmmcpbroadcast_close: (a: number) => void;
  readonly wasmmcpbroadcast_listen: (a: number) => [number, number];
  readonly wasmmcpbroadcast_new: (a: number, b: number) => [number, number, number];
  readonly wasmmcpbroadcast_send: (a: number, b: number, c: number) => [number, number];
  readonly wasmmcpbroadcast_setServer: (a: number, b: number) => void;
  readonly wasmmcpserver_getServerInfo: (a: number) => any;
  readonly wasmmcpserver_handleRequest: (a: number, b: number, c: number) => any;
  readonly wasmmcpserver_handleRequestJs: (a: number, b: any) => any;
  readonly wasmmcpserver_initLearning: (a: number) => [number, number];
  readonly wasmmcpserver_new: () => [number, number, number];
  readonly wasmmcpserver_setIdentity: (a: number, b: number) => void;
  readonly wasmmcpserver_withConfig: (a: any) => [number, number, number];
  readonly wasmmcptransport_close: (a: number) => void;
  readonly wasmmcptransport_fromPort: (a: any) => number;
  readonly wasmmcptransport_init: (a: number) => [number, number];
  readonly wasmmcptransport_new: (a: any) => [number, number, number];
  readonly wasmmcptransport_send: (a: number, b: any) => any;
  readonly wasmmcpworkerhandler_new: (a: number) => number;
  readonly wasmmcpworkerhandler_start: (a: number) => [number, number];
  readonly wasmnetworkmanager_activePeerCount: (a: number) => number;
  readonly wasmnetworkmanager_addRelay: (a: number, b: number, c: number) => void;
  readonly wasmnetworkmanager_getPeersWithCapability: (a: number, b: number, c: number) => [number, number];
  readonly wasmnetworkmanager_isConnected: (a: number) => number;
  readonly wasmnetworkmanager_new: (a: number, b: number) => number;
  readonly wasmnetworkmanager_peerCount: (a: number) => number;
  readonly wasmnetworkmanager_registerPeer: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: bigint) => void;
  readonly wasmnetworkmanager_selectWorkers: (a: number, b: number, c: number, d: number) => [number, number];
  readonly wasmnetworkmanager_updateReputation: (a: number, b: number, c: number, d: number) => void;
  readonly wasmnodeidentity_exportSecretKey: (a: number, b: number, c: number) => [number, number, number, number];
  readonly wasmnodeidentity_fromSecretKey: (a: number, b: number, c: number, d: number) => [number, number, number];
  readonly wasmnodeidentity_generate: (a: number, b: number) => [number, number, number];
  readonly wasmnodeidentity_getFingerprint: (a: number) => [number, number];
  readonly wasmnodeidentity_importSecretKey: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
  readonly wasmnodeidentity_nodeId: (a: number) => [number, number];
  readonly wasmnodeidentity_publicKeyBytes: (a: number) => [number, number];
  readonly wasmnodeidentity_publicKeyHex: (a: number) => [number, number];
  readonly wasmnodeidentity_setFingerprint: (a: number, b: number, c: number) => void;
  readonly wasmnodeidentity_sign: (a: number, b: number, c: number) => [number, number];
  readonly wasmnodeidentity_siteId: (a: number) => [number, number];
  readonly wasmnodeidentity_verify: (a: number, b: number, c: number, d: number, e: number) => number;
  readonly wasmnodeidentity_verifyFrom: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
  readonly wasmstigmergy_deposit: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => void;
  readonly wasmstigmergy_depositWithOutcome: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => void;
  readonly wasmstigmergy_evaporate: (a: number) => void;
  readonly wasmstigmergy_exportState: (a: number) => [number, number];
  readonly wasmstigmergy_follow: (a: number, b: number, c: number) => number;
  readonly wasmstigmergy_getBestSpecialization: (a: number) => [number, number];
  readonly wasmstigmergy_getIntensity: (a: number, b: number, c: number) => number;
  readonly wasmstigmergy_getRankedTasks: (a: number) => [number, number];
  readonly wasmstigmergy_getSpecialization: (a: number, b: number, c: number) => number;
  readonly wasmstigmergy_getStats: (a: number) => [number, number];
  readonly wasmstigmergy_getSuccessRate: (a: number, b: number, c: number) => number;
  readonly wasmstigmergy_maybeEvaporate: (a: number) => number;
  readonly wasmstigmergy_merge: (a: number, b: number, c: number) => number;
  readonly wasmstigmergy_new: () => number;
  readonly wasmstigmergy_setMinStake: (a: number, b: bigint) => void;
  readonly wasmstigmergy_shouldAccept: (a: number, b: number, c: number) => number;
  readonly wasmstigmergy_updateSpecialization: (a: number, b: number, c: number, d: number) => void;
  readonly wasmstigmergy_withParams: (a: number, b: number, c: number) => number;
  readonly wasmtaskexecutor_new: (a: number) => [number, number, number];
  readonly wasmtaskexecutor_setTaskKey: (a: number, b: number, c: number) => [number, number];
  readonly wasmworkscheduler_new: () => number;
  readonly wasmworkscheduler_recordTaskDuration: (a: number, b: number) => void;
  readonly wasmworkscheduler_setPendingTasks: (a: number, b: number) => void;
  readonly wasmworkscheduler_tasksThisFrame: (a: number, b: number) => number;
  readonly witnesstracker_hasSufficientWitnesses: (a: number, b: number, c: number) => number;
  readonly witnesstracker_new: (a: number) => number;
  readonly witnesstracker_witnessConfidence: (a: number, b: number, c: number) => number;
  readonly witnesstracker_witnessCount: (a: number, b: number, c: number) => number;
  readonly wasmcapabilities_getTimeCrystalSync: (a: number) => number;
  readonly __wbg_set_nodeconfig_cpu_limit: (a: number, b: number) => void;
  readonly __wbg_set_rewarddistribution_contributor_share: (a: number, b: bigint) => void;
  readonly __wbg_set_rewarddistribution_founder_share: (a: number, b: bigint) => void;
  readonly __wbg_set_rewarddistribution_protocol_share: (a: number, b: bigint) => void;
  readonly __wbg_set_rewarddistribution_total: (a: number, b: bigint) => void;
  readonly __wbg_set_rewarddistribution_treasury_share: (a: number, b: bigint) => void;
  readonly genesissunset_isSelfSustaining: (a: number) => number;
  readonly edgenetnode_ruvBalance: (a: number) => bigint;
  readonly eventlog_totalEvents: (a: number) => number;
  readonly edgenetnode_enableGlobalWorkspace: (a: number, b: number) => number;
  readonly edgenetnode_enableMicroLoRA: (a: number, b: number) => number;
  readonly edgenetnode_enableMorphogenetic: (a: number, b: number) => number;
  readonly edgenetnode_enableTimeCrystal: (a: number, b: number) => number;
  readonly edgenetnode_enableWTA: (a: number, b: number) => number;
  readonly wasmcapabilities_pruneMorphogenetic: (a: number, b: number) => void;
  readonly wasmcapabilities_step: (a: number, b: number) => void;
  readonly wasmcapabilities_tickNAO: (a: number, b: number) => void;
  readonly wasmcapabilities_getWorkspaceContents: (a: number) => any;
  readonly wasmcapabilities_isTimeCrystalStable: (a: number) => number;
  readonly wasmcapabilities_storeHDC: (a: number, b: number, c: number) => number;
  readonly wasmcapabilities_enableMorphogenetic: (a: number, b: number, c: number) => number;
  readonly wasmcapabilities_enableTimeCrystal: (a: number, b: number, c: number) => number;
  readonly __wbg_get_nodeconfig_cpu_limit: (a: number) => number;
  readonly __wbg_get_rewarddistribution_contributor_share: (a: number) => bigint;
  readonly __wbg_get_rewarddistribution_founder_share: (a: number) => bigint;
  readonly __wbg_get_rewarddistribution_protocol_share: (a: number) => bigint;
  readonly __wbg_get_rewarddistribution_total: (a: number) => bigint;
  readonly __wbg_get_rewarddistribution_treasury_share: (a: number) => bigint;
  readonly __wbg_wasmworkscheduler_free: (a: number, b: number) => void;
  readonly __wbg_multiheadattention_free: (a: number, b: number) => void;
  readonly genesiskey_getId: (a: number) => [number, number];
  readonly wasm_bindgen__convert__closures_____invoke__h8c81ca6cba4eba00: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h16844f6554aa4052: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h9a454594a18d3e6f: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h5a0fd3a052925ed0: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h094c87b54a975e5a: (a: number, b: number, c: any, d: any) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __externref_drop_slice: (a: number, b: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
