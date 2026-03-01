/**
 * Simulated Edge-Net Node
 * Represents a single node in the distributed network
 */

export class SimNode {
  constructor(id, joinedAt, isGenesis = false) {
    this.id = id;
    this.joinedAt = joinedAt;
    this.isGenesis = isGenesis;

    // Node state
    this.active = true;
    this.uptime = 0;
    this.lastSeen = joinedAt;

    // Economic state
    this.ruvEarned = 0;
    this.ruvSpent = 0;
    this.ruvStaked = 0;

    // Performance metrics
    this.tasksCompleted = 0;
    this.tasksSubmitted = 0;
    this.successRate = 0.95;
    this.avgLatency = 100 + Math.random() * 200; // ms

    // Network state
    this.connections = new Set();
    this.maxConnections = isGenesis ? 1000 : 50;
    this.reputation = 1.0;

    // Contribution metrics
    this.cpuContribution = 0.2 + Math.random() * 0.3; // 20-50%
    this.totalComputeHours = 0;
  }

  /**
   * Update node state for a time step
   */
  tick(deltaTime, networkCompute, currentPhase) {
    if (!this.active) return;

    this.uptime += deltaTime;
    this.lastSeen = Date.now();

    // Calculate contribution for this tick
    const hoursThisTick = deltaTime / 3600000; // ms to hours
    const contribution = this.cpuContribution * hoursThisTick;
    this.totalComputeHours += contribution;

    // Simulate task completion
    const tasksThisTick = Math.floor(Math.random() * 3);
    if (tasksThisTick > 0) {
      this.tasksCompleted += tasksThisTick;

      // Calculate rewards with multiplier
      const baseReward = tasksThisTick * 10; // 10 rUv per task
      const multiplier = this.calculateMultiplier(networkCompute, currentPhase);
      const reward = Math.floor(baseReward * multiplier);

      this.ruvEarned += reward;
    }

    // Simulate task submission (nodes also consume)
    if (Math.random() < 0.1) { // 10% chance per tick
      this.tasksSubmitted += 1;
      const cost = 5 + Math.floor(Math.random() * 15); // 5-20 rUv

      if (this.getBalance() >= cost) {
        this.ruvSpent += cost;
      }
    }

    // Update success rate (small random walk)
    this.successRate = Math.max(0.7, Math.min(0.99,
      this.successRate + (Math.random() - 0.5) * 0.01
    ));

    // Genesis nodes in transition phase have connection limits
    if (this.isGenesis && currentPhase === 'transition') {
      this.maxConnections = Math.max(100, this.maxConnections - 1);
    }

    // Genesis nodes become read-only in maturity phase
    if (this.isGenesis && currentPhase === 'maturity') {
      this.maxConnections = 0; // No new connections
    }

    // Genesis nodes retire in post-genesis
    if (this.isGenesis && currentPhase === 'post-genesis') {
      this.active = false;
    }
  }

  /**
   * Calculate contribution multiplier based on network state
   */
  calculateMultiplier(networkCompute, phase) {
    // Base multiplier from contribution curve
    const MAX_BONUS = 10.0;
    const DECAY_CONSTANT = 1000000.0;
    const decay = Math.exp(-networkCompute / DECAY_CONSTANT);
    const baseMultiplier = 1.0 + (MAX_BONUS - 1.0) * decay;

    // Early adopter bonus for genesis nodes
    let earlyBonus = 1.0;
    if (this.isGenesis && phase === 'genesis') {
      earlyBonus = 10.0; // 10x for genesis contributors
    } else if (this.isGenesis && phase === 'transition') {
      earlyBonus = 5.0 - (networkCompute / 1000000.0) * 4.0; // Decay from 5x to 1x
      earlyBonus = Math.max(1.0, earlyBonus);
    }

    return baseMultiplier * earlyBonus;
  }

  /**
   * Get current rUv balance
   */
  getBalance() {
    return Math.max(0, this.ruvEarned - this.ruvSpent - this.ruvStaked);
  }

  /**
   * Connect to another node
   */
  connectTo(nodeId) {
    if (this.connections.size < this.maxConnections) {
      this.connections.add(nodeId);
      return true;
    }
    return false;
  }

  /**
   * Disconnect from a node
   */
  disconnect(nodeId) {
    this.connections.delete(nodeId);
  }

  /**
   * Check if node can accept connections
   */
  canAcceptConnections() {
    return this.active && this.connections.size < this.maxConnections;
  }

  /**
   * Get node statistics
   */
  getStats() {
    return {
      id: this.id,
      isGenesis: this.isGenesis,
      active: this.active,
      uptime: this.uptime,
      ruvBalance: this.getBalance(),
      ruvEarned: this.ruvEarned,
      ruvSpent: this.ruvSpent,
      tasksCompleted: this.tasksCompleted,
      tasksSubmitted: this.tasksSubmitted,
      successRate: this.successRate,
      reputation: this.reputation,
      connections: this.connections.size,
      maxConnections: this.maxConnections,
      totalComputeHours: this.totalComputeHours,
    };
  }
}
