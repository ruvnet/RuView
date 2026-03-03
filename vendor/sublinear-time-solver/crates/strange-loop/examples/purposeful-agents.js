#!/usr/bin/env node

const StrangeLoop = require('strange-loops');

/**
 * Strange Loops Purposeful Agent Examples
 *
 * This demonstrates how to create nano-agents with specific purposes and behaviors.
 * Each agent operates within nanosecond budgets while collectively solving complex problems.
 */

// ============================================================================
// 1. MARKET PREDICTION AGENTS
// ============================================================================

async function createMarketPredictionSwarm() {
  console.log('📈 Creating Market Prediction Swarm...\n');

  // Initialize temporal predictor for financial data
  const predictor = await StrangeLoop.createTemporalPredictor({
    horizonNs: 50_000_000, // 50ms prediction horizon
    historySize: 1000      // Track 1000 historical data points
  });

  // Create specialized agent swarm
  const swarm = await StrangeLoop.createSwarm({
    agentCount: 5000,
    topology: 'hierarchical', // Hierarchical for decision aggregation
    tickDurationNs: 10000     // 10 microsecond budget per tick
  });

  // Define agent behaviors
  const agents = {
    // Pattern recognition agents (40% of swarm)
    patternDetectors: {
      count: 2000,
      behavior: async (data) => {
        // Each agent looks for different patterns
        const patterns = [
          'ascending_triangle',
          'head_shoulders',
          'double_bottom',
          'breakout',
          'reversal'
        ];
        return detectPattern(data, patterns);
      }
    },

    // Sentiment analysis agents (30% of swarm)
    sentimentAnalyzers: {
      count: 1500,
      behavior: async (news, social) => {
        // Analyze market sentiment from multiple sources
        return analyzeSentiment(news, social);
      }
    },

    // Risk assessment agents (20% of swarm)
    riskAssessors: {
      count: 1000,
      behavior: async (position, market) => {
        // Calculate risk metrics
        return calculateRisk(position, market);
      }
    },

    // Decision aggregators (10% of swarm)
    aggregators: {
      count: 500,
      behavior: async (signals) => {
        // Aggregate signals from other agents
        return aggregateDecisions(signals);
      }
    }
  };

  // Run prediction cycle
  const marketData = generateMarketData();

  for (let t = 0; t < 100; t++) {
    // Feed current data to predictor
    await predictor.updateHistory([marketData[t]]);

    // Get temporal prediction
    const prediction = await predictor.predict([marketData[t]]);

    // Run swarm analysis
    const swarmResult = await swarm.run(100); // 100ms analysis window

    console.log(`Time ${t}: Price=${marketData[t].toFixed(2)}, Predicted=${prediction[0].toFixed(2)}`);
  }

  return { predictor, swarm, agents };
}

// ============================================================================
// 2. DISTRIBUTED SEARCH AGENTS
// ============================================================================

async function createSearchSwarm() {
  console.log('🔍 Creating Distributed Search Swarm...\n');

  // Create mesh topology for collaborative search
  const swarm = await StrangeLoop.createSwarm({
    agentCount: 10000,
    topology: 'mesh', // Mesh for peer-to-peer communication
    tickDurationNs: 5000 // 5 microsecond budget
  });

  // Quantum-enhanced search space exploration
  const quantum = await StrangeLoop.createQuantumContainer(4); // 16 states
  await quantum.createSuperposition();

  const searchSpace = {
    dimensions: 100,
    target: generateRandomTarget(100),

    // Agent explores a quantum-influenced region
    exploreRegion: async (agentId, quantumState) => {
      const region = mapQuantumToRegion(quantumState, agentId);
      return evaluateFitness(region, searchSpace.target);
    }
  };

  // Run distributed search
  let bestSolution = null;
  let bestFitness = -Infinity;

  for (let iteration = 0; iteration < 50; iteration++) {
    // Quantum measurement influences search direction
    const quantumState = await quantum.measure();

    // Run swarm exploration
    const result = await swarm.run(1000); // 1 second search iteration

    // Simulate agent discoveries
    const agentFitness = Math.random() * 100 - 50 + iteration;

    if (agentFitness > bestFitness) {
      bestFitness = agentFitness;
      bestSolution = { iteration, fitness: agentFitness, quantumState };
      console.log(`🎯 New best solution found! Fitness: ${bestFitness.toFixed(2)}`);
    }
  }

  return { swarm, quantum, bestSolution };
}

// ============================================================================
// 3. OPTIMIZATION AGENTS
// ============================================================================

async function createOptimizationSwarm() {
  console.log('⚡ Creating Optimization Swarm...\n');

  // Create star topology with central coordinator
  const swarm = await StrangeLoop.createSwarm({
    agentCount: 3000,
    topology: 'star', // Star for centralized optimization
    tickDurationNs: 20000 // 20 microsecond budget
  });

  // Temporal consciousness for meta-learning
  const consciousness = await StrangeLoop.createTemporalConsciousness({
    maxIterations: 1000,
    integrationSteps: 100,
    enableQuantum: true
  });

  // Optimization problem: minimize complex function
  const problem = {
    dimensions: 50,
    objective: (x) => {
      // Rastrigin function (highly multimodal)
      const A = 10;
      return A * x.length + x.reduce((sum, xi) =>
        sum + xi * xi - A * Math.cos(2 * Math.PI * xi), 0
      );
    }
  };

  // Agent strategies
  const strategies = {
    explorers: {
      count: 1000,
      behavior: 'random_walk',
      temperature: 1.0
    },
    exploiters: {
      count: 1000,
      behavior: 'gradient_descent',
      learningRate: 0.01
    },
    innovators: {
      count: 1000,
      behavior: 'quantum_leap',
      quantumProbability: 0.1
    }
  };

  // Run optimization
  for (let gen = 0; gen < 100; gen++) {
    // Evolve consciousness
    const consciousnessState = await consciousness.evolveStep();

    // Adjust strategy based on consciousness index
    if (consciousnessState.consciousnessIndex > 0.8) {
      strategies.innovators.quantumProbability *= 1.5;
      console.log(`🧠 High consciousness detected! Increasing innovation.`);
    }

    // Run swarm optimization
    const result = await swarm.run(500);

    // Simulate optimization progress
    const currentBest = 1000 * Math.exp(-gen / 20) + Math.random() * 10;
    console.log(`Generation ${gen}: Best fitness = ${currentBest.toFixed(2)}`);
  }

  return { swarm, consciousness, strategies };
}

// ============================================================================
// 4. MONITORING & ALERTING AGENTS
// ============================================================================

async function createMonitoringSwarm() {
  console.log('🚨 Creating Monitoring & Alerting Swarm...\n');

  // Ring topology for sequential monitoring
  const swarm = await StrangeLoop.createSwarm({
    agentCount: 1000,
    topology: 'ring', // Ring for round-robin monitoring
    tickDurationNs: 1000 // 1 microsecond for rapid checks
  });

  // Temporal predictor for anomaly detection
  const predictor = await StrangeLoop.createTemporalPredictor({
    horizonNs: 100_000_000, // 100ms ahead
    historySize: 10000      // Large history for pattern learning
  });

  // Monitoring targets
  const monitors = {
    systemHealth: {
      agents: 250,
      metrics: ['cpu', 'memory', 'disk', 'network'],
      threshold: 0.8,
      action: 'alert'
    },
    securityThreats: {
      agents: 250,
      patterns: ['ddos', 'intrusion', 'malware', 'anomaly'],
      sensitivity: 0.95,
      action: 'isolate'
    },
    performanceBottlenecks: {
      agents: 250,
      targets: ['latency', 'throughput', 'errors', 'timeouts'],
      baseline: 'adaptive',
      action: 'scale'
    },
    dataIntegrity: {
      agents: 250,
      checks: ['consistency', 'corruption', 'drift', 'staleness'],
      frequency: 'continuous',
      action: 'repair'
    }
  };

  // Simulate monitoring cycle
  for (let cycle = 0; cycle < 1000; cycle++) {
    // Generate system metrics
    const metrics = {
      cpu: 0.5 + Math.random() * 0.5,
      memory: 0.6 + Math.random() * 0.4,
      latency: 10 + Math.random() * 90,
      errors: Math.floor(Math.random() * 10)
    };

    // Predict future state
    const prediction = await predictor.predict([
      metrics.cpu,
      metrics.memory,
      metrics.latency / 100,
      metrics.errors / 10
    ]);

    // Run monitoring swarm
    const alerts = await swarm.run(10); // 10ms monitoring window

    // Check for anomalies
    if (prediction[0] > 0.9 || metrics.errors > 5) {
      console.log(`⚠️  Alert at cycle ${cycle}: CPU prediction=${(prediction[0]*100).toFixed(1)}%, Errors=${metrics.errors}`);
    }

    // Update predictor history
    await predictor.updateHistory([
      metrics.cpu,
      metrics.memory,
      metrics.latency / 100,
      metrics.errors / 10
    ]);
  }

  return { swarm, predictor, monitors };
}

// ============================================================================
// 5. COLLABORATIVE PROBLEM-SOLVING AGENTS
// ============================================================================

async function createCollaborativeSwarm() {
  console.log('🤝 Creating Collaborative Problem-Solving Swarm...\n');

  // Create multiple swarms for different sub-problems
  const swarms = {
    analysis: await StrangeLoop.createSwarm({
      agentCount: 2000,
      topology: 'hierarchical',
      tickDurationNs: 15000
    }),

    synthesis: await StrangeLoop.createSwarm({
      agentCount: 2000,
      topology: 'mesh',
      tickDurationNs: 15000
    }),

    validation: await StrangeLoop.createSwarm({
      agentCount: 1000,
      topology: 'star',
      tickDurationNs: 10000
    })
  };

  // Quantum entanglement for instant coordination
  const quantum1 = await StrangeLoop.createQuantumContainer(3);
  const quantum2 = await StrangeLoop.createQuantumContainer(3);

  // Create entangled state
  await quantum1.createSuperposition();
  await quantum2.createSuperposition();

  // Collaborative task: Solve complex optimization with constraints
  const task = {
    objective: 'minimize_cost',
    constraints: ['budget', 'time', 'resources', 'quality'],

    phases: {
      1: 'decompose_problem',
      2: 'parallel_exploration',
      3: 'solution_synthesis',
      4: 'constraint_validation',
      5: 'consensus_building'
    }
  };

  // Run collaborative solving
  for (const [phase, description] of Object.entries(task.phases)) {
    console.log(`\nPhase ${phase}: ${description}`);

    // Quantum measurement for phase coordination
    const q1State = await quantum1.measure();
    const q2State = await quantum2.measure();

    // Different swarms handle different phases
    if (phase <= 2) {
      const result = await swarms.analysis.run(2000);
      console.log(`  Analysis swarm: ${result.totalTicks} operations`);
    } else if (phase == 3) {
      const result = await swarms.synthesis.run(2000);
      console.log(`  Synthesis swarm: ${result.totalTicks} operations`);
    } else {
      const result = await swarms.validation.run(1000);
      console.log(`  Validation swarm: ${result.totalTicks} operations`);
    }

    // Re-create superposition for next phase
    await quantum1.createSuperposition();
    await quantum2.createSuperposition();
  }

  return { swarms, quantum: [quantum1, quantum2], task };
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

function generateMarketData() {
  const data = [];
  let price = 100;
  for (let i = 0; i < 1000; i++) {
    price += (Math.random() - 0.5) * 2;
    price = Math.max(price, 10);
    data.push(price);
  }
  return data;
}

function generateRandomTarget(dimensions) {
  return Array(dimensions).fill(0).map(() => Math.random() * 10 - 5);
}

function mapQuantumToRegion(quantumState, agentId) {
  return {
    center: quantumState * agentId % 100,
    radius: 10
  };
}

function detectPattern(data, patterns) {
  return patterns[Math.floor(Math.random() * patterns.length)];
}

function analyzeSentiment(news, social) {
  return Math.random() * 2 - 1; // -1 to 1
}

function calculateRisk(position, market) {
  return Math.random();
}

function aggregateDecisions(signals) {
  return signals.reduce((a, b) => a + b, 0) / signals.length;
}

function evaluateFitness(region, target) {
  return -Math.abs(region.center - target[0]);
}

// ============================================================================
// MAIN EXECUTION
// ============================================================================

async function main() {
  console.log('╔══════════════════════════════════════════════════════════╗');
  console.log('║     STRANGE LOOPS: PURPOSEFUL AGENT DEMONSTRATIONS       ║');
  console.log('╚══════════════════════════════════════════════════════════╝\n');

  try {
    // Initialize Strange Loops
    await StrangeLoop.init();

    // Demonstrate each type of purposeful agent system
    const demos = [
      { name: 'Market Prediction', fn: createMarketPredictionSwarm },
      { name: 'Distributed Search', fn: createSearchSwarm },
      { name: 'Optimization', fn: createOptimizationSwarm },
      { name: 'Monitoring & Alerting', fn: createMonitoringSwarm },
      { name: 'Collaborative Problem-Solving', fn: createCollaborativeSwarm }
    ];

    for (const demo of demos) {
      console.log('\n' + '='.repeat(60));
      console.log(`Running: ${demo.name}`);
      console.log('='.repeat(60) + '\n');

      await demo.fn();

      console.log(`\n✅ ${demo.name} demonstration completed!\n`);
    }

    console.log('\n╔══════════════════════════════════════════════════════════╗');
    console.log('║              ALL DEMONSTRATIONS COMPLETED!               ║');
    console.log('╚══════════════════════════════════════════════════════════╝\n');

  } catch (error) {
    console.error('❌ Error:', error.message);
    process.exit(1);
  }
}

// Run if executed directly
if (require.main === module) {
  main().catch(console.error);
}

// Export for use as library
module.exports = {
  createMarketPredictionSwarm,
  createSearchSwarm,
  createOptimizationSwarm,
  createMonitoringSwarm,
  createCollaborativeSwarm
};