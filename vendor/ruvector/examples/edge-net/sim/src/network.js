/**
 * Network Simulation Engine
 * Manages the overall network state and lifecycle phases
 */

import { SimNode } from './node.js';
import { EconomicTracker } from './economics.js';
import { PhaseManager } from './phases.js';

export class NetworkSimulation {
  constructor(config = {}) {
    this.config = {
      genesisNodes: config.genesisNodes || 10,
      targetNodes: config.targetNodes || 100000,
      tickInterval: config.tickInterval || 1000, // ms
      accelerationFactor: config.accelerationFactor || 1000, // Simulate faster
      ...config
    };

    this.nodes = new Map();
    this.currentTick = 0;
    this.startTime = Date.now();
    this.totalComputeHours = 0;

    this.economics = new EconomicTracker();
    this.phases = new PhaseManager();

    this.metrics = {
      totalTasksCompleted: 0,
      totalTasksSubmitted: 0,
      totalRuvCirculating: 0,
      networkHealth: 1.0,
      averageLatency: 0,
      averageSuccessRate: 0,
    };

    this.events = [];
    this.phaseTransitions = [];
  }

  /**
   * Initialize the network with genesis nodes
   */
  async initialize() {
    console.log(`🌱 Initializing network with ${this.config.genesisNodes} genesis nodes...`);

    const now = Date.now();

    // Create genesis nodes
    for (let i = 0; i < this.config.genesisNodes; i++) {
      const node = new SimNode(`genesis-${i}`, now, true);
      this.nodes.set(node.id, node);
    }

    // Connect genesis nodes to each other
    const genesisNodes = Array.from(this.nodes.values());
    for (let i = 0; i < genesisNodes.length; i++) {
      for (let j = i + 1; j < genesisNodes.length; j++) {
        genesisNodes[i].connectTo(genesisNodes[j].id);
        genesisNodes[j].connectTo(genesisNodes[i].id);
      }
    }

    this.logEvent('network_initialized', {
      genesisNodes: this.config.genesisNodes,
      timestamp: now
    });

    return this;
  }

  /**
   * Run simulation for a specific phase or all phases
   */
  async run(targetPhase = 'all') {
    console.log(`🚀 Starting simulation (target: ${targetPhase})...`);

    const phaseTargets = {
      genesis: 10000,
      transition: 50000,
      maturity: 100000,
      'post-genesis': 150000,
      all: this.config.targetNodes
    };

    const targetNodeCount = phaseTargets[targetPhase] || this.config.targetNodes;

    while (this.nodes.size < targetNodeCount) {
      await this.tick();

      // Add new nodes at varying rates based on phase
      const currentPhase = this.getCurrentPhase();
      const joinRate = this.getNodeJoinRate(currentPhase);

      if (Math.random() < joinRate) {
        this.addNode();
      }

      // Some nodes leave (churn)
      if (Math.random() < 0.001 && this.nodes.size > this.config.genesisNodes) {
        this.removeRandomNode();
      }

      // Log progress periodically
      if (this.currentTick % 100 === 0) {
        this.logProgress();
      }

      // Check for phase transitions
      this.checkPhaseTransition();
    }

    console.log('✅ Simulation complete!');
    return this.generateReport();
  }

  /**
   * Execute a single simulation tick
   */
  async tick() {
    this.currentTick++;

    // Accelerated time delta (ms)
    const deltaTime = this.config.tickInterval * this.config.accelerationFactor;

    // Update all active nodes
    const currentPhase = this.getCurrentPhase();
    let totalCompute = 0;

    for (const node of this.nodes.values()) {
      node.tick(deltaTime, this.totalComputeHours, currentPhase);
      totalCompute += node.totalComputeHours;
    }

    this.totalComputeHours = totalCompute;

    // Update network metrics
    this.updateMetrics();

    // Update economic state
    this.economics.tick(this.getActiveNodes(), this.metrics);

    // Small delay for visualization (optional)
    if (this.config.visualDelay) {
      await new Promise(resolve => setTimeout(resolve, this.config.visualDelay));
    }
  }

  /**
   * Add a new node to the network
   */
  addNode() {
    const nodeId = `node-${this.nodes.size}`;
    const node = new SimNode(nodeId, Date.now(), false);
    this.nodes.set(nodeId, node);

    // Connect to random existing nodes
    const existingNodes = Array.from(this.nodes.values())
      .filter(n => n.id !== nodeId && n.canAcceptConnections());

    const connectionsToMake = Math.min(5, existingNodes.length);
    for (let i = 0; i < connectionsToMake; i++) {
      const randomNode = existingNodes[Math.floor(Math.random() * existingNodes.length)];
      node.connectTo(randomNode.id);
      randomNode.connectTo(nodeId);
    }

    // Prefer connecting to genesis nodes initially
    const currentPhase = this.getCurrentPhase();
    if (currentPhase === 'genesis') {
      const genesisNodes = existingNodes.filter(n => n.isGenesis && n.canAcceptConnections());
      for (const gNode of genesisNodes.slice(0, 3)) {
        node.connectTo(gNode.id);
        gNode.connectTo(nodeId);
      }
    }

    return node;
  }

  /**
   * Remove a random non-genesis node (network churn)
   */
  removeRandomNode() {
    const regularNodes = Array.from(this.nodes.values()).filter(n => !n.isGenesis);
    if (regularNodes.length === 0) return;

    const nodeToRemove = regularNodes[Math.floor(Math.random() * regularNodes.length)];

    // Disconnect from all peers
    for (const node of this.nodes.values()) {
      node.disconnect(nodeToRemove.id);
    }

    this.nodes.delete(nodeToRemove.id);
  }

  /**
   * Get current network phase based on node count
   */
  getCurrentPhase() {
    const count = this.nodes.size;

    if (count < 10000) return 'genesis';
    if (count < 50000) return 'transition';
    if (count < 100000) return 'maturity';
    return 'post-genesis';
  }

  /**
   * Get node join rate for current phase
   */
  getNodeJoinRate(phase) {
    const rates = {
      genesis: 0.3,      // Slow initial growth
      transition: 0.5,   // Accelerating growth
      maturity: 0.7,     // Peak growth
      'post-genesis': 0.4 // Stable growth
    };

    return rates[phase] || 0.3;
  }

  /**
   * Check if a phase transition occurred
   */
  checkPhaseTransition() {
    const count = this.nodes.size;
    const previousPhase = this.phases.currentPhase;
    const currentPhase = this.getCurrentPhase();

    if (previousPhase !== currentPhase) {
      this.phases.transition(currentPhase);

      this.phaseTransitions.push({
        from: previousPhase,
        to: currentPhase,
        tick: this.currentTick,
        nodeCount: count,
        totalCompute: this.totalComputeHours,
        timestamp: Date.now()
      });

      this.logEvent('phase_transition', {
        from: previousPhase,
        to: currentPhase,
        nodeCount: count
      });

      console.log(`\n🔄 Phase Transition: ${previousPhase} → ${currentPhase} (${count} nodes)`);
    }
  }

  /**
   * Update network-wide metrics
   */
  updateMetrics() {
    const activeNodes = this.getActiveNodes();
    const nodeCount = activeNodes.length;

    if (nodeCount === 0) return;

    let totalTasks = 0;
    let totalSubmitted = 0;
    let totalRuv = 0;
    let totalLatency = 0;
    let totalSuccess = 0;

    for (const node of activeNodes) {
      totalTasks += node.tasksCompleted;
      totalSubmitted += node.tasksSubmitted;
      totalRuv += node.ruvEarned;
      totalLatency += node.avgLatency;
      totalSuccess += node.successRate;
    }

    this.metrics = {
      totalTasksCompleted: totalTasks,
      totalTasksSubmitted: totalSubmitted,
      totalRuvCirculating: totalRuv,
      averageLatency: totalLatency / nodeCount,
      averageSuccessRate: totalSuccess / nodeCount,
      activeNodeCount: nodeCount,
      genesisNodeCount: activeNodes.filter(n => n.isGenesis).length,
      networkHealth: this.calculateNetworkHealth(activeNodes),
    };
  }

  /**
   * Calculate overall network health score (0-1)
   */
  calculateNetworkHealth(nodes) {
    if (nodes.length === 0) return 0;

    // Factors: connectivity, success rate, economic velocity
    const avgConnections = nodes.reduce((sum, n) => sum + n.connections.size, 0) / nodes.length;
    const avgSuccess = nodes.reduce((sum, n) => sum + n.successRate, 0) / nodes.length;
    const economicVelocity = this.economics.getVelocity();

    const connectivityScore = Math.min(1.0, avgConnections / 20); // Target 20 connections
    const reliabilityScore = avgSuccess;
    const economicScore = Math.min(1.0, economicVelocity / 0.5); // Target 0.5 velocity

    return (connectivityScore * 0.3 + reliabilityScore * 0.4 + economicScore * 0.3);
  }

  /**
   * Get all active nodes
   */
  getActiveNodes() {
    return Array.from(this.nodes.values()).filter(n => n.active);
  }

  /**
   * Log an event
   */
  logEvent(type, data) {
    this.events.push({
      type,
      tick: this.currentTick,
      timestamp: Date.now(),
      ...data
    });
  }

  /**
   * Log progress to console
   */
  logProgress() {
    const phase = this.getCurrentPhase();
    const activeNodes = this.getActiveNodes();
    const genesisActive = activeNodes.filter(n => n.isGenesis).length;

    console.log(
      `📊 Tick ${this.currentTick} | ` +
      `Phase: ${phase.toUpperCase()} | ` +
      `Nodes: ${activeNodes.length} (${genesisActive} genesis) | ` +
      `Compute: ${Math.floor(this.totalComputeHours)}h | ` +
      `Health: ${(this.metrics.networkHealth * 100).toFixed(1)}%`
    );
  }

  /**
   * Generate final simulation report
   */
  generateReport() {
    const report = {
      summary: {
        totalTicks: this.currentTick,
        totalNodes: this.nodes.size,
        activeNodes: this.getActiveNodes().length,
        totalComputeHours: this.totalComputeHours,
        finalPhase: this.getCurrentPhase(),
        simulationDuration: Date.now() - this.startTime,
      },
      metrics: this.metrics,
      economics: this.economics.getReport(),
      phases: {
        transitions: this.phaseTransitions,
        current: this.getCurrentPhase(),
      },
      nodes: {
        genesis: Array.from(this.nodes.values())
          .filter(n => n.isGenesis)
          .map(n => n.getStats()),
        regular: Array.from(this.nodes.values())
          .filter(n => !n.isGenesis)
          .slice(0, 100) // Sample of regular nodes
          .map(n => n.getStats()),
      },
      events: this.events,
    };

    return report;
  }

  /**
   * Export metrics as time series
   */
  exportTimeSeries() {
    // This would be populated during simulation
    // For now, return current snapshot
    return {
      timestamp: Date.now(),
      tick: this.currentTick,
      nodeCount: this.nodes.size,
      activeNodes: this.getActiveNodes().length,
      totalCompute: this.totalComputeHours,
      phase: this.getCurrentPhase(),
      health: this.metrics.networkHealth,
      ...this.metrics,
    };
  }
}
