/**
 * Economic Tracking and Analysis
 * Monitors economic health, sustainability, and distribution
 */

export class EconomicTracker {
  constructor() {
    this.totalSupply = 0;
    this.treasury = 0;
    this.contributorPool = 0;
    this.protocolFund = 0;
    this.founderPool = 0;

    // Distribution ratios
    this.distribution = {
      contributors: 0.70,
      treasury: 0.15,
      protocol: 0.10,
      founders: 0.05,
    };

    // Health metrics
    this.velocity = 0;
    this.utilization = 0;
    this.growthRate = 0;
    this.stability = 1.0;

    // Historical data
    this.history = [];
    this.epochCount = 0;
  }

  /**
   * Process a simulation tick
   */
  tick(nodes, metrics) {
    // Calculate new rUv minted this tick
    const totalEarned = nodes.reduce((sum, n) => sum + n.ruvEarned, 0);
    const totalSpent = nodes.reduce((sum, n) => sum + n.ruvSpent, 0);

    const newSupply = totalEarned - this.totalSupply;
    this.totalSupply = totalEarned;

    if (newSupply > 0) {
      // Distribute according to ratios
      this.contributorPool += Math.floor(newSupply * this.distribution.contributors);
      this.treasury += Math.floor(newSupply * this.distribution.treasury);
      this.protocolFund += Math.floor(newSupply * this.distribution.protocol);
      this.founderPool += Math.floor(newSupply * this.distribution.founders);
    }

    // Update health metrics
    this.updateHealthMetrics(nodes, metrics, totalSpent);

    // Record snapshot periodically
    if (this.epochCount % 10 === 0) {
      this.recordSnapshot(nodes.length, metrics);
    }

    this.epochCount++;
  }

  /**
   * Update economic health metrics
   */
  updateHealthMetrics(nodes, metrics, totalSpent) {
    // Velocity: how fast rUv circulates (spent / supply)
    this.velocity = this.totalSupply > 0
      ? totalSpent / this.totalSupply
      : 0;

    // Utilization: active nodes / total supply capacity
    const activeNodes = nodes.filter(n => n.active).length;
    this.utilization = activeNodes > 0
      ? Math.min(1.0, metrics.totalTasksCompleted / (activeNodes * 100))
      : 0;

    // Growth rate: change in supply (simplified)
    this.growthRate = this.totalSupply > 0
      ? 0.01 // Simplified constant growth
      : 0;

    // Stability: balance across pools
    this.stability = this.calculateStability();
  }

  /**
   * Calculate stability index based on pool distribution
   */
  calculateStability() {
    const totalPools = this.treasury + this.contributorPool + this.protocolFund;
    if (totalPools === 0) return 1.0;

    const treasuryRatio = this.treasury / totalPools;
    const contributorRatio = this.contributorPool / totalPools;
    const protocolRatio = this.protocolFund / totalPools;

    // Ideal is 33% each
    const ideal = 0.33;
    const variance = Math.pow(treasuryRatio - ideal, 2) +
                     Math.pow(contributorRatio - ideal, 2) +
                     Math.pow(protocolRatio - ideal, 2);

    return Math.max(0, Math.min(1.0, 1.0 - Math.sqrt(variance)));
  }

  /**
   * Check if network is economically self-sustaining
   */
  isSelfSustaining(activeNodes, dailyTasks) {
    const minNodes = 100;
    const minDailyTasks = 1000;
    const treasuryRunwayDays = 90;
    const estimatedDailyCost = activeNodes * 10; // 10 rUv per node per day

    return (
      activeNodes >= minNodes &&
      dailyTasks >= minDailyTasks &&
      this.treasury >= estimatedDailyCost * treasuryRunwayDays &&
      this.growthRate >= 0.0
    );
  }

  /**
   * Get economic velocity (transactions per period)
   */
  getVelocity() {
    return this.velocity;
  }

  /**
   * Record economic snapshot
   */
  recordSnapshot(nodeCount, metrics) {
    this.history.push({
      epoch: this.epochCount,
      timestamp: Date.now(),
      totalSupply: this.totalSupply,
      treasury: this.treasury,
      contributorPool: this.contributorPool,
      protocolFund: this.protocolFund,
      founderPool: this.founderPool,
      velocity: this.velocity,
      utilization: this.utilization,
      growthRate: this.growthRate,
      stability: this.stability,
      nodeCount,
      health: this.getHealthScore(),
    });
  }

  /**
   * Get overall economic health score (0-1)
   */
  getHealthScore() {
    // Weighted combination of metrics
    return (
      this.velocity * 0.3 +
      this.utilization * 0.3 +
      this.stability * 0.4
    );
  }

  /**
   * Generate economic report
   */
  getReport() {
    return {
      supply: {
        total: this.totalSupply,
        treasury: this.treasury,
        contributors: this.contributorPool,
        protocol: this.protocolFund,
        founders: this.founderPool,
      },
      health: {
        velocity: this.velocity,
        utilization: this.utilization,
        growthRate: this.growthRate,
        stability: this.stability,
        overall: this.getHealthScore(),
      },
      sustainability: {
        selfSustaining: this.isSelfSustaining(1000, 10000), // Example values
        treasuryRunway: Math.floor(this.treasury / 100), // Days
      },
      history: this.history,
    };
  }
}
