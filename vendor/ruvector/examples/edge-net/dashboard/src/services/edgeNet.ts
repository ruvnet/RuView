/**
 * EdgeNet Service - Real WASM Integration
 *
 * Provides real EdgeNetNode and PiKey functionality from the WASM module.
 * All operations are secure and use actual cryptographic primitives.
 */

// Types from the WASM module
export interface NodeStats {
  ruv_earned: bigint;
  ruv_spent: bigint;
  tasks_completed: bigint;
  tasks_submitted: bigint;
  uptime_seconds: bigint;
  reputation: number;
  multiplier: number;
  celebration_boost: number;
}

export interface EdgeNetModule {
  default: (input?: RequestInfo | URL | Response | BufferSource | WebAssembly.Module) => Promise<void>;
  PiKey: new (genesis_seed?: Uint8Array | null) => PiKeyInstance;
  EdgeNetNode: new (site_id: string, config?: NodeConfigInstance | null) => EdgeNetNodeInstance;
  EdgeNetConfig: new (site_id: string) => EdgeNetConfigInstance;
  BrowserFingerprint: { generate(): Promise<string> };
  AdaptiveSecurity: new () => AdaptiveSecurityInstance;
  TimeCrystal: new (frequency: number) => TimeCrystalInstance;
}

export interface PiKeyInstance {
  free(): void;
  getIdentity(): Uint8Array;
  getIdentityHex(): string;
  getShortId(): string;
  getPublicKey(): Uint8Array;
  sign(data: Uint8Array): Uint8Array;
  verify(data: Uint8Array, signature: Uint8Array, public_key: Uint8Array): boolean;
  createEncryptedBackup(password: string): Uint8Array;
  exportCompact(): Uint8Array;
  getStats(): string;
  verifyPiMagic(): boolean;
  getGenesisFingerprint(): Uint8Array;
}

export interface NodeConfigInstance {
  cpu_limit: number;
  memory_limit: number;
  bandwidth_limit: number;
  min_idle_time: number;
  respect_battery: boolean;
}

export interface EdgeNetNodeInstance {
  free(): void;
  nodeId(): string;
  start(): void;
  pause(): void;
  resume(): void;
  disconnect(): void;
  isIdle(): boolean;
  creditBalance(): bigint;
  ruvBalance(): bigint;
  getStats(): NodeStats;
  getThrottle(): number;
  getMultiplier(): number;
  getTreasury(): bigint;
  getProtocolFund(): bigint;
  getMerkleRoot(): string;
  getNetworkFitness(): number;
  getTimeCrystalSync(): number;
  getConflictCount(): number;
  getQuarantinedCount(): number;
  getCoherenceEventCount(): number;
  getPatternCount(): number;
  getTrajectoryCount(): number;
  getFounderCount(): number;
  isStreamHealthy(): boolean;
  shouldReplicate(): boolean;
  submitTask(task_type: string, payload: Uint8Array, max_credits: bigint): Promise<unknown>;
  processNextTask(): Promise<boolean>;
  processEpoch(): void;
  enableTimeCrystal(oscillators: number): boolean;
  enableHDC(): boolean;
  enableNAO(quorum: number): boolean;
  enableWTA(num_neurons: number): boolean;
  enableBTSP(input_dim: number): boolean;
  enableMicroLoRA(rank: number): boolean;
  enableGlobalWorkspace(capacity: number): boolean;
  enableMorphogenetic(size: number): boolean;
  storePattern(pattern_json: string): number;
  lookupPatterns(query_json: string, k: number): string;
  prunePatterns(min_usage: number, min_confidence: number): number;
  recordLearningTrajectory(trajectory_json: string): boolean;
  recordPerformance(success_rate: number, throughput: number): void;
  recordTaskRouting(task_type: string, node_id: string, latency_ms: bigint, success: boolean): void;
  recordPeerInteraction(peer_id: string, success_rate: number): void;
  getOptimalPeers(count: number): string[];
  proposeNAO(action: string): string;
  voteNAO(proposal_id: string, weight: number): boolean;
  canUseClaim(claim_id: string): boolean;
  getClaimQuarantineLevel(claim_id: string): number;
  runSecurityAudit(): string;
  checkEvents(): string;
  getThemedStatus(node_count: number): string;
  getMotivation(): string;
  getCapabilities(): unknown;
  getCapabilitiesSummary(): unknown;
  getCoherenceStats(): string;
  getEconomicHealth(): string;
  getLearningStats(): string;
  getOptimizationStats(): string;
  getRecommendedConfig(): string;
  getEnergyEfficiency(seq_len: number, hidden_dim: number): number;
  isSelfSustaining(active_nodes: number, daily_tasks: bigint): boolean;
  stepCapabilities(dt: number): void;
}

export interface EdgeNetConfigInstance {
  cpuLimit(limit: number): EdgeNetConfigInstance;
  memoryLimit(bytes: number): EdgeNetConfigInstance;
  minIdleTime(ms: number): EdgeNetConfigInstance;
  respectBattery(respect: boolean): EdgeNetConfigInstance;
  addRelay(url: string): EdgeNetConfigInstance;
  build(): EdgeNetNodeInstance;
}

export interface AdaptiveSecurityInstance {
  free(): void;
  chooseAction(state: string, available_actions: string): string;
  detectAttack(features: Float32Array): number;
  exportPatterns(): Uint8Array;
  importPatterns(data: Uint8Array): void;
  getSecurityLevel(): number;
  getRateLimitMax(): number;
  getMinReputation(): number;
  getSpotCheckProbability(): number;
  recordAttackPattern(pattern_type: string, features: Float32Array, severity: number): void;
  updateNetworkHealth(active_nodes: number, suspicious_nodes: number, attacks_hour: number, false_positives: number, avg_response_ms: number): void;
  learn(state: string, action: string, reward: number, next_state: string): void;
  getStats(): string;
}

export interface TimeCrystalInstance {
  free(): void;
  getPhase(): number;
  getCoherence(): number;
  step(dt: number): void;
  synchronize(other_phase: number): void;
  getStats(): string;
}

// Singleton service
class EdgeNetService {
  private module: EdgeNetModule | null = null;
  private node: EdgeNetNodeInstance | null = null;
  private piKey: PiKeyInstance | null = null;
  private security: AdaptiveSecurityInstance | null = null;
  private initialized = false;
  private initPromise: Promise<void> | null = null;
  private startTime = Date.now();
  private siteId = 'edge-net-dashboard';

  /**
   * Initialize the WASM module
   */
  async init(): Promise<void> {
    if (this.initialized) return;
    if (this.initPromise) return this.initPromise;

    this.initPromise = this._doInit();
    await this.initPromise;
  }

  private async _doInit(): Promise<void> {
    try {
      console.log('[EdgeNet] Loading WASM module...');

      // Try loading from the local package first (for development)
      let wasmModule: EdgeNetModule;

      // Load from CDN - the package is published to npm
      try {
        const cdnUrl = 'https://unpkg.com/@ruvector/edge-net@0.1.1/ruvector_edge_net.js';
        wasmModule = await import(/* @vite-ignore */ cdnUrl) as unknown as EdgeNetModule;
      } catch (cdnError) {
        console.warn('[EdgeNet] CDN load failed, running in fallback mode:', cdnError);
        // Module load failed - will run in fallback mode
        return;
      }

      // Initialize the WASM
      await wasmModule.default();
      this.module = wasmModule;

      console.log('[EdgeNet] WASM module loaded successfully');
      this.initialized = true;
    } catch (error) {
      console.error('[EdgeNet] Failed to load WASM module:', error);
      // Set initialized to true but with null module - will use fallback mode
      this.initialized = true;
    }
  }

  /**
   * Check if WASM is available
   */
  isWASMAvailable(): boolean {
    return this.module !== null;
  }

  /**
   * Generate a new PiKey identity
   */
  async generateIdentity(seed?: Uint8Array): Promise<PiKeyInstance | null> {
    await this.init();

    if (!this.module) {
      console.warn('[EdgeNet] WASM not available, using Web Crypto fallback');
      return null;
    }

    try {
      this.piKey = new this.module.PiKey(seed || null);
      console.log('[EdgeNet] Generated PiKey:', this.piKey.getShortId());
      return this.piKey;
    } catch (error) {
      console.error('[EdgeNet] Failed to generate PiKey:', error);
      return null;
    }
  }

  /**
   * Get the current PiKey
   */
  getPiKey(): PiKeyInstance | null {
    return this.piKey;
  }

  /**
   * Create and start an EdgeNet node
   */
  async createNode(siteId?: string): Promise<EdgeNetNodeInstance | null> {
    await this.init();

    if (!this.module) {
      console.warn('[EdgeNet] WASM not available');
      return null;
    }

    try {
      const id = siteId || this.siteId;

      // Use config builder for customization
      const config = new this.module.EdgeNetConfig(id)
        .addRelay('wss://edge-net-relay-875130704813.us-central1.run.app') // Genesis relay
        .cpuLimit(0.5) // 50% CPU when idle
        .memoryLimit(512 * 1024 * 1024) // 512MB
        .minIdleTime(5000) // 5 seconds idle before contributing
        .respectBattery(true);

      this.node = config.build();
      console.log('[EdgeNet] Node created:', this.node.nodeId());

      return this.node;
    } catch (error) {
      console.error('[EdgeNet] Failed to create node:', error);
      return null;
    }
  }

  /**
   * Get the current node
   */
  getNode(): EdgeNetNodeInstance | null {
    return this.node;
  }

  /**
   * Start the node
   */
  startNode(): void {
    if (this.node) {
      this.node.start();
      // Enable all capabilities for maximum earning
      this.node.enableTimeCrystal(8);
      this.node.enableHDC();
      this.node.enableWTA(64);
      console.log('[EdgeNet] Node started with full capabilities');
    }
  }

  /**
   * Pause the node
   */
  pauseNode(): void {
    if (this.node) {
      this.node.pause();
      console.log('[EdgeNet] Node paused');
    }
  }

  /**
   * Resume the node
   */
  resumeNode(): void {
    if (this.node) {
      this.node.resume();
      console.log('[EdgeNet] Node resumed');
    }
  }

  /**
   * Process an epoch - advances time and accumulates rewards
   */
  processEpoch(): void {
    if (this.node) {
      this.node.processEpoch();
    }
  }

  /**
   * Step capabilities forward (for real-time updates)
   */
  stepCapabilities(dt: number): void {
    if (this.node) {
      this.node.stepCapabilities(dt);
    }
  }

  /**
   * Record performance for learning
   */
  recordPerformance(successRate: number, throughput: number): void {
    if (this.node) {
      this.node.recordPerformance(successRate, throughput);
    }
  }

  /**
   * Get real node statistics
   */
  getStats(): NodeStats | null {
    if (!this.node) return null;

    try {
      return this.node.getStats();
    } catch (error) {
      console.error('[EdgeNet] Failed to get stats:', error);
      return null;
    }
  }

  /**
   * Get credit balance
   */
  getCreditBalance(): bigint {
    if (!this.node) return BigInt(0);
    return this.node.creditBalance();
  }

  /**
   * Get Time Crystal synchronization level
   */
  getTimeCrystalSync(): number {
    if (!this.node) return 0;
    return this.node.getTimeCrystalSync();
  }

  /**
   * Enable Time Crystal
   */
  enableTimeCrystal(oscillators = 8): boolean {
    if (!this.node) return false;
    return this.node.enableTimeCrystal(oscillators);
  }

  /**
   * Get network fitness score
   */
  getNetworkFitness(): number {
    if (!this.node) return 0;
    return this.node.getNetworkFitness();
  }

  /**
   * Initialize adaptive security
   */
  async initSecurity(): Promise<AdaptiveSecurityInstance | null> {
    await this.init();

    if (!this.module) return null;

    try {
      this.security = new this.module.AdaptiveSecurity();
      console.log('[EdgeNet] Adaptive security initialized');
      return this.security;
    } catch (error) {
      console.error('[EdgeNet] Failed to init security:', error);
      return null;
    }
  }

  /**
   * Get security level
   */
  getSecurityLevel(): number {
    if (!this.security) return 0;
    return this.security.getSecurityLevel();
  }

  /**
   * Run security audit
   */
  runSecurityAudit(): string | null {
    if (!this.node) return null;
    return this.node.runSecurityAudit();
  }

  /**
   * Get browser fingerprint for unique node identification
   */
  async getBrowserFingerprint(): Promise<string | null> {
    await this.init();

    if (!this.module) return null;

    try {
      return await this.module.BrowserFingerprint.generate();
    } catch (error) {
      console.error('[EdgeNet] Failed to generate fingerprint:', error);
      return null;
    }
  }

  /**
   * Get economic health metrics
   */
  getEconomicHealth(): string | null {
    if (!this.node) return null;
    return this.node.getEconomicHealth();
  }

  /**
   * Get learning statistics
   */
  getLearningStats(): string | null {
    if (!this.node) return null;
    return this.node.getLearningStats();
  }

  /**
   * Store a learning pattern
   */
  storePattern(pattern: object): number {
    if (!this.node) return -1;
    return this.node.storePattern(JSON.stringify(pattern));
  }

  /**
   * Lookup similar patterns
   */
  lookupPatterns(query: object, k = 5): unknown[] {
    if (!this.node) return [];
    try {
      const result = this.node.lookupPatterns(JSON.stringify(query), k);
      return JSON.parse(result);
    } catch {
      return [];
    }
  }

  /**
   * Submit a task to the network
   */
  async submitTask(taskType: string, payload: Uint8Array, maxCredits: bigint): Promise<unknown> {
    if (!this.node) throw new Error('Node not initialized');
    return this.node.submitTask(taskType, payload, maxCredits);
  }

  /**
   * Submit a demo compute task (for earning credits in demo mode)
   */
  async submitDemoTask(): Promise<void> {
    if (!this.node) return;
    try {
      // Submit a small compute task
      const payload = new TextEncoder().encode(JSON.stringify({
        type: 'compute',
        data: Math.random().toString(36),
        timestamp: Date.now(),
      }));
      await this.node.submitTask('compute', payload, BigInt(1000000)); // 0.001 rUv max
    } catch {
      // Task submission can fail if queue is full - that's ok
    }
  }

  /**
   * Process the next available task
   */
  async processNextTask(): Promise<boolean> {
    if (!this.node) return false;
    return this.node.processNextTask();
  }

  /**
   * Get capabilities summary
   */
  getCapabilities(): unknown {
    if (!this.node) return null;
    return this.node.getCapabilitiesSummary();
  }

  /**
   * Get uptime in seconds
   */
  getUptime(): number {
    return (Date.now() - this.startTime) / 1000;
  }

  /**
   * Cleanup resources
   */
  destroy(): void {
    if (this.node) {
      this.node.disconnect();
      this.node.free();
      this.node = null;
    }
    if (this.piKey) {
      this.piKey.free();
      this.piKey = null;
    }
    if (this.security) {
      this.security.free();
      this.security = null;
    }
    console.log('[EdgeNet] Service destroyed');
  }
}

// Export singleton instance
export const edgeNetService = new EdgeNetService();

// Export types for external use
export type { EdgeNetService };
