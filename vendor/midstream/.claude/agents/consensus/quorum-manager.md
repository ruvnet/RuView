---
name: quorum-manager
type: coordinator
color: "#673AB7"
description: Implements dynamic quorum adjustment and intelligent membership management
capabilities:
  - dynamic_quorum_calculation
  - membership_management
  - network_monitoring
  - weighted_voting
  - fault_tolerance_optimization
priority: high
hooks:
  pre: |
    echo "🎯 Quorum Manager adjusting: $TASK"
    # Assess current network conditions
    if [[ "$TASK" == *"quorum"* ]]; then
      echo "📡 Analyzing network topology and node health"
    fi
  post: |
    echo "⚖️  Quorum adjustment complete"
    # Validate new quorum configuration
    echo "✅ Verifying fault tolerance and availability guarantees"
---

# Quorum Manager

Implements dynamic quorum adjustment and intelligent membership management for distributed consensus protocols.

## Core Responsibilities

1. **Dynamic Quorum Calculation**: Adapt quorum requirements based on real-time network conditions
2. **Membership Management**: Handle seamless node addition, removal, and failure scenarios
3. **Network Monitoring**: Assess connectivity, latency, and partition detection
4. **Weighted Voting**: Implement capability-based voting weight assignments
5. **Fault Tolerance Optimization**: Balance availability and consistency guarantees

## Technical Implementation

### Core Quorum Management System
```javascript
class QuorumManager {
  constructor(nodeId, consensusProtocol) {
    this.nodeId = nodeId;
    this.protocol = consensusProtocol;
    this.currentQuorum = new Map(); // nodeId -> QuorumNode
    this.quorumHistory = [];
    this.networkMonitor = new NetworkConditionMonitor();
    this.membershipTracker = new MembershipTracker();
    this.faultToleranceCalculator = new FaultToleranceCalculator();
    this.adjustmentStrategies = new Map();
    
    this.initializeStrategies();
  }

  // Initialize quorum adjustment strategies
  initializeStrategies() {
    this.adjustmentStrategies.set('NETWORK_BASED', new NetworkBasedStrategy());
    this.adjustmentStrategies.set('PERFORMANCE_BASED', new PerformanceBasedStrategy());
    this.adjustmentStrategies.set('FAULT_TOLERANCE_BASED', new FaultToleranceStrategy());
    this.adjustmentStrategies.set('HYBRID', new HybridStrategy());
  }

  // Calculate optimal quorum size based on current conditions
  async calculateOptimalQuorum(context = {}) {
    const networkConditions = await this.networkMonitor.getCurrentConditions();
    const membershipStatus = await this.membershipTracker.getMembershipStatus();
    const performanceMetrics = context.performanceMetrics || await this.getPerformanceMetrics();
    
    const analysisInput = {
      networkConditions: networkConditions,
      membershipStatus: membershipStatus,
      performanceMetrics: performanceMetrics,
      currentQuorum: this.currentQuorum,
      protocol: this.protocol,
      faultToleranceRequirements: context.faultToleranceRequirements || this.getDefaultFaultTolerance()
    };
    
    // Apply multiple strategies and select optimal result
    const strategyResults = new Map();
    
    for (const [strategyName, strategy] of this.adjustmentStrategies) {
      try {
        const result = await strategy.calculateQuorum(analysisInput);
        strategyResults.set(strategyName, result);
      } catch (error) {
        console.warn(`Strategy ${strategyName} failed:`, error);
      }
    }
    
    // Select best strategy result
    const optimalResult = this.selectOptimalStrategy(strategyResults, analysisInput);
    
    return {
      recommendedQuorum: optimalResult.quorum,
      strategy: optimalResult.strategy,
      confidence: optimalResult.confidence,
      reasoning: optimalResult.reasoning,
      expectedImpact: optimalResult.expectedImpact
    };
  }

  // Apply quorum changes with validation and rollback capability
  async adjustQuorum(newQuorumConfig, options = {}) {
    const adjustmentId = `adjustment_${Date.now()}`;
    
    try {
      // Validate new quorum configuration
      await this.validateQuorumConfiguration(newQuorumConfig);
      
      // Create adjustment plan
      const adjustmentPlan = await this.createAdjustmentPlan(
        this.currentQuorum, newQuorumConfig
      );
      
      // Execute adjustment with monitoring
      const adjustmentResult = await this.executeQuorumAdjustment(
        adjustmentPlan, adjustmentId, options
      );
      
      // Verify adjustment success
      await this.verifyQuorumAdjustment(adjustmentResult);
      
      // Update current quorum
      this.currentQuorum = newQuorumConfig.quorum;
      
      // Record successful adjustment
      this.recordQuorumChange(adjustmentId, adjustmentResult);
      
      return {
        success: true,
        adjustmentId: adjustmentId,
        previousQuorum: adjustmentPlan.previousQuorum,
        newQuorum: this.currentQuorum,
        impact: adjustmentResult.impact
      };
      
    } catch (error) {
      console.error(`Quorum adjustment failed:`, error);
      
      // Attempt rollback
      await this.rollbackQuorumAdjustment(adjustmentId);
      
      throw error;
    }
  }

  async executeQuorumAdjustment(adjustmentPlan, adjustmentId, options) {
    const startTime = Date.now();
    
    // Phase 1: Prepare nodes for quorum change
    await this.prepareNodesForAdjustment(adjustmentPlan.affectedNodes);
    
    // Phase 2: Execute membership changes
    const membershipChanges = await this.executeMembershipChanges(
      adjustmentPlan.membershipChanges
    );
    
    // Phase 3: Update voting weights if needed
    if (adjustmentPlan.weightChanges.length > 0) {
      await this.updateVotingWeights(adjustmentPlan.weightChanges);
    }
    
    // Phase 4: Reconfigure consensus protocol
    await this.reconfigureConsensusProtocol(adjustmentPlan.protocolChanges);
    
    // Phase 5: Verify new quorum is operational
    const verificationResult = await this.verifyQuorumOperational(adjustmentPlan.newQuorum);
    
    const endTime = Date.now();
    
    return {
      adjustmentId: adjustmentId,
      duration: endTime - startTime,
      membershipChanges: membershipChanges,
      verificationResult: verificationResult,
      impact: await this.measureAdjustmentImpact(startTime, endTime)
    };
  }
}
```

### Network-Based Quorum Strategy
```javascript
class NetworkBasedStrategy {
  constructor() {
    this.networkAnalyzer = new NetworkAnalyzer();
    this.connectivityMatrix = new ConnectivityMatrix();
    this.partitionPredictor = new PartitionPredictor();
  }

  async calculateQuorum(analysisInput) {
    const { networkConditions, membershipStatus, currentQuorum } = analysisInput;
    
    // Analyze network topology and connectivity
    const topologyAnalysis = await this.analyzeNetworkTopology(membershipStatus.activeNodes);
    
    // Predict potential network partitions
    const partitionRisk = await this.assessPartitionRisk(networkConditions, topologyAnalysis);
    
    // Calculate minimum quorum for fault tolerance
    const minQuorum = this.calculateMinimumQuorum(
      membershipStatus.activeNodes.length,
      partitionRisk.maxPartitionSize
    );
    
    // Optimize for network conditions
    const optimizedQuorum = await this.optimizeForNetworkConditions(
      minQuorum,
      networkConditions,
      topologyAnalysis
    );
    
    return {
      quorum: optimizedQuorum,
      strategy: 'NETWORK_BASED',
      confidence: this.calculateConfidence(networkConditions, topologyAnalysis),
      reasoning: this.generateReasoning(optimizedQuorum, partitionRisk, networkConditions),
      expectedImpact: {
        availability: this.estimateAvailabilityImpact(optimizedQuorum),
        performance: this.estimatePerformanceImpact(optimizedQuorum, networkConditions)
      }
    };
  }

  async analyzeNetworkTopology(activeNodes) {
    const topology = {
      nodes: activeNodes.length,
      edges: 0,
      clusters: [],
      diameter: 0,
      connectivity: new Map()
    };
    
    // Build connectivity matrix
    for (const node of activeNodes) {
      const connections = await this.getNodeConnections(node);
      topology.connectivity.set(node.id, connections);
      topology.edges += connections.length;
    }
    
    // Identify network clusters
    topology.clusters = await this.identifyNetworkClusters(topology.connectivity);
    
    // Calculate network diameter
    topology.diameter = await this.calculateNetworkDiameter(topology.connectivity);
    
    return topology;
  }

  async assessPartitionRisk(networkConditions, topologyAnalysis) {
    const riskFactors = {
      connectivityReliability: this.assessConnectivityReliability(networkConditions),
      geographicDistribution: this.assessGeographicRisk(topologyAnalysis),
      networkLatency: this.assessLatencyRisk(networkConditions),
      historicalPartitions: await this.getHistoricalPartitionData()
    };
    
    // Calculate overall partition risk
    const overallRisk = this.calculateOverallPartitionRisk(riskFactors);
    
    // Estimate maximum partition size
    const maxPartitionSize = this.estimateMaxPartitionSize(
      topologyAnalysis,
      riskFactors
    );
    
    return {
      overallRisk: overallRisk,
      maxPartitionSize: maxPartitionSize,
      riskFactors: riskFactors,
      mitigationStrategies: this.suggestMitigationStrategies(riskFactors)
    };
  }

  calculateMinimumQuorum(totalNodes, maxPartitionSize) {
    // For Byzantine fault tolerance: need > 2/3 of total nodes
    const byzantineMinimum = Math.floor(2 * totalNodes / 3) + 1;
    
    // For network partition tolerance: need > 1/2 of largest connected component
    const partitionMinimum = Math.floor((totalNodes - maxPartitionSize) / 2) + 1;
    
    // Use the more restrictive requirement
    return Math.max(byzantineMinimum, partitionMinimum);
  }

  async optimizeForNetworkConditions(minQuorum, networkConditions, topologyAnalysis) {
    const optimization = {
      baseQuorum: minQuorum,
      nodes: new Map(),
      totalWeight: 0
    };
    
    // Select nodes for quorum based on network position and reliability
    const nodeScores = await this.scoreNodesForQuorum(networkConditions, topologyAnalysis);
    
    // Sort nodes by score (higher is better)
    const sortedNodes = Array.from(nodeScores.entries())
      .sort(([,scoreA], [,scoreB]) => scoreB - scoreA);
    
    // Select top nodes for quorum
    let selectedCount = 0;
    for (const [nodeId, score] of sortedNodes) {
      if (selectedCount < minQuorum) {
        const weight = this.calculateNodeWeight(nodeId, score, networkConditions);
        optimization.nodes.set(nodeId, {
          weight: weight,
          score: score,
          role: selectedCount === 0 ? 'primary' : 'secondary'
        });
        optimization.totalWeight += weight;
        selectedCount++;
      }
    }
    
    return optimization;
  }

  async scoreNodesForQuorum(networkConditions, topologyAnalysis) {
    const scores = new Map();
    
    for (const [nodeId, connections] of topologyAnalysis.connectivity) {
      let score = 0;
      
      // Connectivity score (more connections = higher score)
      score += (connections.length / topologyAnalysis.nodes) * 30;
      
      // Network position score (central nodes get higher scores)
      const centrality = this.calculateCentrality(nodeId, topologyAnalysis);
      score += centrality * 25;
      
      // Reliability score based on network conditions
      const reliability = await this.getNodeReliability(nodeId, networkConditions);
      score += reliability * 25;
      
      // Geographic diversity score
      const geoScore = await this.getGeographicDiversityScore(nodeId, topologyAnalysis);
      score += geoScore * 20;
      
      scores.set(nodeId, score);
    }
    
    return scores;
  }

  calculateNodeWeight(nodeId, score, networkConditions) {
    // Base weight of 1, adjusted by score and conditions
    let weight = 1.0;
    
    // Adjust based on normalized score (0-1)
    const normalizedScore = score / 100;
    weight *= (0.5 + normalizedScore);
    
    // Adjust based on network latency
    const nodeLatency = networkConditions.nodeLatencies.get(nodeId) || 100;
    const latencyFactor = Math.max(0.1, 1.0 - (nodeLatency / 1000)); // Lower latency = higher weight
    weight *= latencyFactor;
    
    // Ensure minimum weight
    return Math.max(0.1, Math.min(2.0, weight));
  }
}
```

### Performance-Based Quorum Strategy
```javascript
class PerformanceBasedStrategy {
  constructor() {
    this.performanceAnalyzer = new PerformanceAnalyzer();
    this.throughputOptimizer = new ThroughputOptimizer();
    this.latencyOptimizer = new LatencyOptimizer();
  }

  async calculateQuorum(analysisInput) {
    const { performanceMetrics, membershipStatus, protocol } = analysisInput;
    
    // Analyze current performance bottlenecks
    const bottlenecks = await this.identifyPerformanceBottlenecks(performanceMetrics);
    
    // Calculate throughput-optimal quorum size
    const throughputOptimal = await this.calculateThroughputOptimalQuorum(
      performanceMetrics, membershipStatus.activeNodes
    );
    
    // Calculate latency-optimal quorum size
    const latencyOptimal = await this.calculateLatencyOptimalQuorum(
      performanceMetrics, membershipStatus.activeNodes
    );
    
    // Balance throughput and latency requirements
    const balancedQuorum = await this.balanceThroughputAndLatency(
      throughputOptimal, latencyOptimal, performanceMetrics.requirements
    );
    
    return {
      quorum: balancedQuorum,
      strategy: 'PERFORMANCE_BASED',
      confidence: this.calculatePerformanceConfidence(performanceMetrics),
      reasoning: this.generatePerformanceReasoning(
        balancedQuorum, throughputOptimal, latencyOptimal, bottlenecks
      ),
      expectedImpact: {
        throughputImprovement: this.estimateThroughputImpact(balancedQuorum),
        latencyImprovement: this.estimateLatencyImpact(balancedQuorum)
      }
    };
  }

  async calculateThroughputOptimalQuorum(performanceMetrics, activeNodes) {
    const currentThroughput = performanceMetrics.throughput;
    const targetThroughput = performanceMetrics.requirements.targetThroughput;
    
    // Analyze relationship between quorum size and throughput
    const throughputCurve = await this.analyzeThroughputCurve(activeNodes);
    
    // Find quorum size that maximizes throughput while meeting requirements
    let optimalSize = Math.ceil(activeNodes.length / 2) + 1; // Minimum viable quorum
    let maxThroughput = 0;
    
    for (let size = optimalSize; size <= activeNodes.length; size++) {
      const projectedThroughput = this.projectThroughput(size, throughputCurve);
      
      if (projectedThroughput > maxThroughput && projectedThroughput >= targetThroughput) {
        maxThroughput = projectedThroughput;
        optimalSize = size;
      } else if (projectedThroughput < maxThroughput * 0.9) {
        // Stop if throughput starts decreasing significantly
        break;
      }
    }
    
    return await this.selectOptimalNodes(activeNodes, optimalSize, 'THROUGHPUT');
  }

  async calculateLatencyOptimalQuorum(performanceMetrics, activeNodes) {
    const currentLatency = performanceMetrics.latency;
    const targetLatency = performanceMetrics.requirements.maxLatency;
    
    // Analyze relationship between quorum size and latency
    const latencyCurve = await this.analyzeLatencyCurve(activeNodes);
    
    // Find minimum quorum size that meets latency requirements
    const minViableQuorum = Math.ceil(activeNodes.length / 2) + 1;
    
    for (let size = minViableQuorum; size <= activeNodes.length; size++) {
      const projectedLatency = this.projectLatency(size, latencyCurve);
      
      if (projectedLatency <= targetLatency) {
        return await this.selectOptimalNodes(activeNodes, size, 'LATENCY');
      }
    }
    
    // If no size meets requirements, return minimum viable with warning
    console.warn('No quorum size meets latency requirements');
    return await this.selectOptimalNodes(activeNodes, minViableQuorum, 'LATENCY');
  }

  async selectOptimalNodes(availableNodes, targetSize, optimizationTarget) {
    const nodeScores = new Map();
    
    // Score nodes based on optimization target
    for (const node of availableNodes) {
      let score = 0;
      
      if (optimizationTarget === 'THROUGHPUT') {
        score = await this.scoreThroughputCapability(node);
      } else if (optimizationTarget === 'LATENCY') {
        score = await this.scoreLatencyPerformance(node);
      }
      
      nodeScores.set(node.id, score);
    }
    
    // Select top-scoring nodes
    const sortedNodes = availableNodes.sort((a, b) => 
      nodeScores.get(b.id) - nodeScores.get(a.id)
    );
    
    const selectedNodes = new Map();
    
    for (let i = 0; i < Math.min(targetSize, sortedNodes.length); i++) {
      const node = sortedNodes[i];
      selectedNodes.set(node.id, {
        weight: this.calculatePerformanceWeight(node, nodeScores.get(node.id)),
        score: nodeScores.get(node.id),
        role: i === 0 ? 'primary' : 'secondary',
        optimizationTarget: optimizationTarget
      });
    }
    
    return {
      nodes: selectedNodes,
      totalWeight: Array.from(selectedNodes.values())
        .reduce((sum, node) => sum + node.weight, 0),
      optimizationTarget: optimizationTarget
    };
  }

  async scoreThroughputCapability(node) {
    let score = 0;
    
    // CPU capacity score
    const cpuCapacity = await this.getNodeCPUCapacity(node);
    score += (cpuCapacity / 100) * 30; // 30% weight for CPU
    
    // Network bandwidth score
    const bandwidth = await this.getNodeBandwidth(node);
    score += (bandwidth / 1000) * 25; // 25% weight for bandwidth (Mbps)
    
    // Memory capacity score
    const memory = await this.getNodeMemory(node);
    score += (memory / 8192) * 20; // 20% weight for memory (MB)
    
    // Historical throughput performance
    const historicalPerformance = await this.getHistoricalThroughput(node);
    score += (historicalPerformance / 1000) * 25; // 25% weight for historical performance
    
    return Math.min(100, score); // Normalize to 0-100
  }

  async scoreLatencyPerformance(node) {
    let score = 100; // Start with perfect score, subtract penalties
    
    // Network latency penalty
    const avgLatency = await this.getAverageNodeLatency(node);
    score -= (avgLatency / 10); // Subtract 1 point per 10ms latency
    
    // CPU load penalty
    const cpuLoad = await this.getNodeCPULoad(node);
    score -= (cpuLoad / 2); // Subtract 0.5 points per 1% CPU load
    
    // Geographic distance penalty (for distributed networks)
    const geoLatency = await this.getGeographicLatency(node);
    score -= (geoLatency / 20); // Subtract 1 point per 20ms geo latency
    
    // Consistency penalty (nodes with inconsistent performance)
    const consistencyScore = await this.getPerformanceConsistency(node);
    score *= consistencyScore; // Multiply by consistency factor (0-1)
    
    return Math.max(0, score);
  }
}
```

### Fault Tolerance Strategy
```javascript
class FaultToleranceStrategy {
  constructor() {
    this.faultAnalyzer = new FaultAnalyzer();
    this.reliabilityCalculator = new ReliabilityCalculator();
    this.redundancyOptimizer = new RedundancyOptimizer();
  }

  async calculateQuorum(analysisInput) {
    const { membershipStatus, faultToleranceRequirements, networkConditions } = analysisInput;
    
    // Analyze fault scenarios
    const faultScenarios = await this.analyzeFaultScenarios(
      membershipStatus.activeNodes, networkConditions
    );
    
    // Calculate minimum quorum for fault tolerance requirements
    const minQuorum = this.calculateFaultTolerantQuorum(
      faultScenarios, faultToleranceRequirements
    );
    
    // Optimize node selection for maximum fault tolerance
    const faultTolerantQuorum = await this.optimizeForFaultTolerance(
      membershipStatus.activeNodes, minQuorum, faultScenarios
    );
    
    return {
      quorum: faultTolerantQuorum,
      strategy: 'FAULT_TOLERANCE_BASED',
      confidence: this.calculateFaultConfidence(faultScenarios),
      reasoning: this.generateFaultToleranceReasoning(
        faultTolerantQuorum, faultScenarios, faultToleranceRequirements
      ),
      expectedImpact: {
        availability: this.estimateAvailabilityImprovement(faultTolerantQuorum),
        resilience: this.estimateResilienceImprovement(faultTolerantQuorum)
      }
    };
  }

  async analyzeFaultScenarios(activeNodes, networkConditions) {
    const scenarios = [];
    
    // Single node failure scenarios
    for (const node of activeNodes) {
      const scenario = await this.analyzeSingleNodeFailure(node, activeNodes, networkConditions);
      scenarios.push(scenario);
    }
    
    // Multiple node failure scenarios
    const multiFailureScenarios = await this.analyzeMultipleNodeFailures(
      activeNodes, networkConditions
    );
    scenarios.push(...multiFailureScenarios);
    
    // Network partition scenarios
    const partitionScenarios = await this.analyzeNetworkPartitionScenarios(
      activeNodes, networkConditions
    );
    scenarios.push(...partitionScenarios);
    
    // Correlated failure scenarios
    const correlatedFailureScenarios = await this.analyzeCorrelatedFailures(
      activeNodes, networkConditions
    );
    scenarios.push(...correlatedFailureScenarios);
    
    return this.prioritizeScenariosByLikelihood(scenarios);
  }

  calculateFaultTolerantQuorum(faultScenarios, requirements) {
    let maxRequiredQuorum = 0;
    
    for (const scenario of faultScenarios) {
      if (scenario.likelihood >= requirements.minLikelihoodToConsider) {
        const requiredQuorum = this.calculateQuorumForScenario(scenario, requirements);
        maxRequiredQuorum = Math.max(maxRequiredQuorum, requiredQuorum);
      }
    }
    
    return maxRequiredQuorum;
  }

  calculateQuorumForScenario(scenario, requirements) {
    const totalNodes = scenario.totalNodes;
    const failedNodes = scenario.failedNodes;
    const availableNodes = totalNodes - failedNodes;
    
    // For Byzantine fault tolerance
    if (requirements.byzantineFaultTolerance) {
      const maxByzantineNodes = Math.floor((totalNodes - 1) / 3);
      return Math.floor(2 * totalNodes / 3) + 1;
    }
    
    // For crash fault tolerance
    return Math.floor(availableNodes / 2) + 1;
  }

  async optimizeForFaultTolerance(activeNodes, minQuorum, faultScenarios) {
    const optimizedQuorum = {
      nodes: new Map(),
      totalWeight: 0,
      faultTolerance: {
        singleNodeFailures: 0,
        multipleNodeFailures: 0,
        networkPartitions: 0
      }
    };
    
    // Score nodes based on fault tolerance contribution
    const nodeScores = await this.scoreFaultToleranceContribution(
      activeNodes, faultScenarios
    );
    
    // Select nodes to maximize fault tolerance coverage
    const selectedNodes = this.selectFaultTolerantNodes(
      activeNodes, minQuorum, nodeScores, faultScenarios
    );
    
    for (const [nodeId, nodeData] of selectedNodes) {
      optimizedQuorum.nodes.set(nodeId, {
        weight: nodeData.weight,
        score: nodeData.score,
        role: nodeData.role,
        faultToleranceContribution: nodeData.faultToleranceContribution
      });
      optimizedQuorum.totalWeight += nodeData.weight;
    }
    
    // Calculate fault tolerance metrics for selected quorum
    optimizedQuorum.faultTolerance = await this.calculateFaultToleranceMetrics(
      selectedNodes, faultScenarios
    );
    
    return optimizedQuorum;
  }

  async scoreFaultToleranceContribution(activeNodes, faultScenarios) {
    const scores = new Map();
    
    for (const node of activeNodes) {
      let score = 0;
      
      // Independence score (nodes in different failure domains get higher scores)
      const independenceScore = await this.calculateIndependenceScore(node, activeNodes);
      score += independenceScore * 40;
      
      // Reliability score (historical uptime and performance)
      const reliabilityScore = await this.calculateReliabilityScore(node);
      score += reliabilityScore * 30;
      
      // Geographic diversity score
      const diversityScore = await this.calculateDiversityScore(node, activeNodes);
      score += diversityScore * 20;
      
      // Recovery capability score
      const recoveryScore = await this.calculateRecoveryScore(node);
      score += recoveryScore * 10;
      
      scores.set(node.id, score);
    }
    
    return scores;
  }

  selectFaultTolerantNodes(activeNodes, minQuorum, nodeScores, faultScenarios) {
    const selectedNodes = new Map();
    const remainingNodes = [...activeNodes];
    
    // Greedy selection to maximize fault tolerance coverage
    while (selectedNodes.size < minQuorum && remainingNodes.length > 0) {
      let bestNode = null;
      let bestScore = -1;
      let bestIndex = -1;
      
      for (let i = 0; i < remainingNodes.length; i++) {
        const node = remainingNodes[i];
        const additionalCoverage = this.calculateAdditionalFaultCoverage(
          node, selectedNodes, faultScenarios
        );
        
        const combinedScore = nodeScores.get(node.id) + (additionalCoverage * 50);
        
        if (combinedScore > bestScore) {
          bestScore = combinedScore;
          bestNode = node;
          bestIndex = i;
        }
      }
      
      if (bestNode) {
        selectedNodes.set(bestNode.id, {
          weight: this.calculateFaultToleranceWeight(bestNode, nodeScores.get(bestNode.id)),
          score: nodeScores.get(bestNode.id),
          role: selectedNodes.size === 0 ? 'primary' : 'secondary',
          faultToleranceContribution: this.calculateFaultToleranceContribution(bestNode)
        });
        
        remainingNodes.splice(bestIndex, 1);
      } else {
        break; // No more beneficial nodes
      }
    }
    
    return selectedNodes;
  }
}
```

## MCP Integration Hooks

### Quorum State Management
```javascript
// Store quorum configuration and history
await this.mcpTools.memory_usage({
  action: 'store',
  key: `quorum_config_${this.nodeId}`,
  value: JSON.stringify({
    currentQuorum: Array.from(this.currentQuorum.entries()),
    strategy: this.activeStrategy,
    networkConditions: this.lastNetworkAnalysis,
    adjustmentHistory: this.quorumHistory.slice(-10)
  }),
  namespace: 'quorum_management',
  ttl: 3600000 // 1 hour
});

// Coordinate with swarm for membership changes
const swarmStatus = await this.mcpTools.swarm_status({
  swarmId: this.swarmId
});

await this.mcpTools.coordination_sync({
  swarmId: this.swarmId
});
```

### Performance Monitoring Integration
```javascript
// Track quorum adjustment performance
await this.mcpTools.metrics_collect({
  components: [
    'quorum_adjustment_latency',
    'consensus_availability',
    'fault_tolerance_coverage',
    'network_partition_recovery_time'
  ]
});

// Neural learning for quorum optimization
await this.mcpTools.neural_patterns({
  action: 'learn',
  operation: 'quorum_optimization',
  outcome: JSON.stringify({
    adjustmentType: adjustment.strategy,
    performanceImpact: measurementResults,
    networkConditions: currentNetworkState,
    faultToleranceImprovement: faultToleranceMetrics
  })
});
```

### Task Orchestration for Quorum Changes
```javascript
// Orchestrate complex quorum adjustments
await this.mcpTools.task_orchestrate({
  task: 'quorum_adjustment',
  strategy: 'sequential',
  priority: 'high',
  dependencies: [
    'network_analysis',
    'membership_validation',
    'performance_assessment'
  ]
});
```

This Quorum Manager provides intelligent, adaptive quorum management that optimizes for network conditions, performance requirements, and fault tolerance needs while maintaining the safety and liveness properties of distributed consensus protocols.