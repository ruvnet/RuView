/**
 * Metrics Collection and Aggregation
 * Tracks network performance across all phases
 */

import { Network, NetworkPhase } from './network.js';

export interface PhaseMetrics {
  phase: NetworkPhase;
  startTick: number;
  endTick: number;
  duration: number;
  nodeCount: {
    start: number;
    end: number;
    peak: number;
  };
  energy: {
    totalEarned: number;
    totalSpent: number;
    netEnergy: number;
    avgPerNode: number;
    sustainability: number; // earned / spent ratio
  };
  genesis: {
    avgMultiplier: number;
    activeCount: number;
    readOnlyCount: number;
    retiredCount: number;
  };
  network: {
    avgConnections: number;
    avgSuccessRate: number;
    taskThroughput: number;
    tasksCompleted: number;
  };
  validation: {
    passed: boolean;
    reasons: string[];
  };
}

export class MetricsCollector {
  private network: Network;
  private phaseMetrics: Map<NetworkPhase, PhaseMetrics>;
  private currentPhaseStart: number;
  private currentPhaseNodeCount: number;
  private peakNodeCount: number;

  constructor(network: Network) {
    this.network = network;
    this.phaseMetrics = new Map();
    this.currentPhaseStart = 0;
    this.currentPhaseNodeCount = 0;
    this.peakNodeCount = 0;
  }

  /**
   * Initialize metrics collection
   */
  public initialize(): void {
    this.currentPhaseStart = this.network.currentTick;
    this.currentPhaseNodeCount = this.network.cells.size;
    this.peakNodeCount = this.network.cells.size;
  }

  /**
   * Collect metrics for the current tick
   */
  public collect(): void {
    const stats = this.network.getStats();

    // Update peak node count
    this.peakNodeCount = Math.max(this.peakNodeCount, stats.nodeCount);
  }

  /**
   * Handle phase transition
   */
  public onPhaseTransition(oldPhase: NetworkPhase, newPhase: NetworkPhase): void {
    // Finalize metrics for old phase
    this.finalizePhase(oldPhase);

    // Start tracking new phase
    this.currentPhaseStart = this.network.currentTick;
    this.currentPhaseNodeCount = this.network.cells.size;
    this.peakNodeCount = this.network.cells.size;
  }

  /**
   * Finalize metrics for a completed phase
   */
  private finalizePhase(phase: NetworkPhase): void {
    const stats = this.network.getStats();
    const endTick = this.network.currentTick;
    const duration = endTick - this.currentPhaseStart;

    const cells = Array.from(this.network.cells.values());
    const totalEarned = cells.reduce((sum, c) => sum + c.metrics.energyEarned, 0);
    const totalSpent = cells.reduce((sum, c) => sum + c.metrics.energySpent, 0);
    const totalTasks = cells.reduce((sum, c) => sum + c.metrics.tasksCompleted, 0);

    const metrics: PhaseMetrics = {
      phase,
      startTick: this.currentPhaseStart,
      endTick,
      duration,
      nodeCount: {
        start: this.currentPhaseNodeCount,
        end: stats.nodeCount,
        peak: this.peakNodeCount,
      },
      energy: {
        totalEarned,
        totalSpent,
        netEnergy: totalEarned - totalSpent,
        avgPerNode: stats.economy.avgEnergyPerNode,
        sustainability: totalSpent > 0 ? totalEarned / totalSpent : 0,
      },
      genesis: {
        avgMultiplier: stats.genesisNodes.avgMultiplier,
        activeCount: stats.genesisNodes.active,
        readOnlyCount: stats.genesisNodes.readOnly,
        retiredCount: stats.genesisNodes.retired,
      },
      network: {
        avgConnections: stats.network.avgConnections,
        avgSuccessRate: stats.network.avgSuccessRate,
        taskThroughput: duration > 0 ? totalTasks / duration : 0,
        tasksCompleted: totalTasks,
      },
      validation: this.validatePhase(phase, stats),
    };

    this.phaseMetrics.set(phase, metrics);
  }

  /**
   * Validate phase completion criteria
   */
  private validatePhase(phase: NetworkPhase, stats: any): { passed: boolean; reasons: string[] } {
    const reasons: string[] = [];
    let passed = true;

    switch (phase) {
      case NetworkPhase.GENESIS:
        // Verify 10x multiplier is active
        if (stats.genesisNodes.avgMultiplier < 9.0) {
          passed = false;
          reasons.push(`Genesis multiplier too low: ${stats.genesisNodes.avgMultiplier.toFixed(2)} (expected ~10.0)`);
        } else {
          reasons.push(`✓ Genesis multiplier active: ${stats.genesisNodes.avgMultiplier.toFixed(2)}x`);
        }

        // Verify energy accumulation
        if (stats.economy.totalEarned < 1000) {
          passed = false;
          reasons.push(`Insufficient energy accumulation: ${stats.economy.totalEarned.toFixed(2)}`);
        } else {
          reasons.push(`✓ Energy accumulated: ${stats.economy.totalEarned.toFixed(2)} rUv`);
        }

        // Verify network formation
        if (stats.network.avgConnections < 5) {
          passed = false;
          reasons.push(`Network poorly connected: ${stats.network.avgConnections.toFixed(2)} avg connections`);
        } else {
          reasons.push(`✓ Network connected: ${stats.network.avgConnections.toFixed(2)} avg connections`);
        }
        break;

      case NetworkPhase.GROWTH:
        // Verify genesis nodes stop accepting connections
        if (stats.genesisNodes.active > stats.genesisNodes.count * 0.1) {
          passed = false;
          reasons.push(`Too many genesis nodes still active: ${stats.genesisNodes.active}`);
        } else {
          reasons.push(`✓ Genesis nodes reducing activity: ${stats.genesisNodes.active} active`);
        }

        // Verify multiplier decay
        if (stats.genesisNodes.avgMultiplier > 5.0) {
          passed = false;
          reasons.push(`Genesis multiplier decay insufficient: ${stats.genesisNodes.avgMultiplier.toFixed(2)}`);
        } else {
          reasons.push(`✓ Multiplier decaying: ${stats.genesisNodes.avgMultiplier.toFixed(2)}x`);
        }

        // Verify task routing optimization
        if (stats.network.avgSuccessRate < 0.7) {
          passed = false;
          reasons.push(`Task success rate too low: ${(stats.network.avgSuccessRate * 100).toFixed(1)}%`);
        } else {
          reasons.push(`✓ Task routing optimized: ${(stats.network.avgSuccessRate * 100).toFixed(1)}% success`);
        }
        break;

      case NetworkPhase.MATURATION:
        // Verify genesis nodes are read-only
        if (stats.genesisNodes.readOnly < stats.genesisNodes.count * 0.8) {
          passed = false;
          reasons.push(`Genesis nodes not read-only: ${stats.genesisNodes.readOnly}/${stats.genesisNodes.count}`);
        } else {
          reasons.push(`✓ Genesis nodes read-only: ${stats.genesisNodes.readOnly}/${stats.genesisNodes.count}`);
        }

        // Verify economic sustainability
        const sustainability = stats.economy.totalEarned / Math.max(stats.economy.totalSpent, 1);
        if (sustainability < 1.0) {
          passed = false;
          reasons.push(`Network not sustainable: ${sustainability.toFixed(2)} earned/spent ratio`);
        } else {
          reasons.push(`✓ Economically sustainable: ${sustainability.toFixed(2)} ratio`);
        }

        // Verify network independence
        if (stats.network.avgConnections < 10) {
          passed = false;
          reasons.push(`Network connectivity too low for independence: ${stats.network.avgConnections.toFixed(2)}`);
        } else {
          reasons.push(`✓ Network ready for independence: ${stats.network.avgConnections.toFixed(2)} avg connections`);
        }
        break;

      case NetworkPhase.INDEPENDENCE:
        // Verify genesis nodes retired
        if (stats.genesisNodes.retired < stats.genesisNodes.count * 0.9) {
          passed = false;
          reasons.push(`Genesis nodes not fully retired: ${stats.genesisNodes.retired}/${stats.genesisNodes.count}`);
        } else {
          reasons.push(`✓ Genesis nodes retired: ${stats.genesisNodes.retired}/${stats.genesisNodes.count}`);
        }

        // Verify pure P2P operation
        if (stats.genesisNodes.avgMultiplier > 1.1) {
          passed = false;
          reasons.push(`Genesis multiplier still active: ${stats.genesisNodes.avgMultiplier.toFixed(2)}`);
        } else {
          reasons.push(`✓ Pure P2P operation: ${stats.genesisNodes.avgMultiplier.toFixed(2)}x multiplier`);
        }

        // Verify long-term stability
        if (stats.economy.netEnergy < 0) {
          passed = false;
          reasons.push(`Network losing energy: ${stats.economy.netEnergy.toFixed(2)}`);
        } else {
          reasons.push(`✓ Network stable: +${stats.economy.netEnergy.toFixed(2)} rUv net energy`);
        }
        break;
    }

    return { passed, reasons };
  }

  /**
   * Finalize current phase (for end of simulation)
   */
  public finalizeCurrent(): void {
    this.finalizePhase(this.network.currentPhase);
  }

  /**
   * Get all collected metrics
   */
  public getAllMetrics(): PhaseMetrics[] {
    return Array.from(this.phaseMetrics.values());
  }

  /**
   * Get metrics for a specific phase
   */
  public getPhaseMetrics(phase: NetworkPhase): PhaseMetrics | undefined {
    return this.phaseMetrics.get(phase);
  }

  /**
   * Get overall success rate
   */
  public getOverallSuccess(): { passed: boolean; totalPassed: number; totalPhases: number } {
    const metrics = this.getAllMetrics();
    const totalPassed = metrics.filter(m => m.validation.passed).length;
    const totalPhases = metrics.length;

    return {
      passed: totalPassed === totalPhases,
      totalPassed,
      totalPhases,
    };
  }
}
