#!/usr/bin/env node

const wasm = require('../wasm/strange_loop.js');
const chalk = require('chalk');
const ora = require('ora');

// Initialize WASM
wasm.init_wasm();

console.log(chalk.cyan.bold('\n╔════════════════════════════════════════════════════════════════════╗'));
console.log(chalk.cyan.bold('║            STRANGE LOOPS - NANO-AGENT SWARM EXECUTION             ║'));
console.log(chalk.cyan.bold('╚════════════════════════════════════════════════════════════════════╝\n'));

// Agent class to simulate nano-agents
class NanoAgent {
  constructor(id, type, capability) {
    this.id = id;
    this.type = type;
    this.capability = capability;
    this.tickBudgetUs = 25; // 25 microseconds per tick
    this.results = [];
  }

  async execute(task) {
    const start = Date.now();
    let result;

    switch(this.capability) {
      case 'quantum':
        result = this.executeQuantum(task);
        break;
      case 'consciousness':
        result = this.executeConsciousness(task);
        break;
      case 'temporal':
        result = this.executeTemporal(task);
        break;
      case 'solver':
        result = this.executeSolver(task);
        break;
      case 'attractor':
        result = this.executeAttractor(task);
        break;
      default:
        result = { error: 'Unknown capability' };
    }

    const duration = Date.now() - start;
    this.results.push({ task, result, duration });
    return result;
  }

  executeQuantum(task) {
    const results = [];

    // Create Bell state
    results.push(wasm.create_bell_state(0));

    // Quantum superposition
    results.push(wasm.quantum_superposition(4));

    // Measure quantum state
    const measurement = wasm.measure_quantum_state(4);
    results.push(`Measured state: |${measurement.toString(2).padStart(4, '0')}⟩`);

    // Calculate entanglement entropy
    const entropy = wasm.quantum_entanglement_entropy(4);
    results.push(`Entanglement entropy: ${entropy.toFixed(3)} bits`);

    // Quantum teleportation
    results.push(wasm.quantum_gate_teleportation(0.5));

    return {
      agent: `Quantum-${this.id}`,
      operations: results
    };
  }

  executeConsciousness(task) {
    const results = [];

    // Evolve consciousness
    const level = wasm.evolve_consciousness(task.iterations || 500);
    results.push(`Consciousness level: ${(level * 100).toFixed(1)}%`);

    // Calculate Phi (integrated information)
    const phi = wasm.calculate_phi(10, 30);
    results.push(`Φ (integrated information): ${phi.toFixed(3)}`);

    // Verify consciousness
    results.push(wasm.verify_consciousness(phi, level, 0.7));

    // Detect temporal patterns
    results.push(wasm.detect_temporal_patterns(1000));

    return {
      agent: `Consciousness-${this.id}`,
      operations: results
    };
  }

  executeTemporal(task) {
    const results = [];

    // Create retrocausal loop
    results.push(wasm.create_retrocausal_loop(100));

    // Predict future state
    const prediction = wasm.predict_future_state(10.0, 500);
    results.push(`Future state prediction: ${prediction.toFixed(3)}`);

    // Temporal patterns
    results.push(wasm.detect_temporal_patterns(2000));

    // Decoherence time
    const t2 = wasm.quantum_decoherence_time(4, 20);
    results.push(`Decoherence time (T2): ${t2.toFixed(1)}μs`);

    return {
      agent: `Temporal-${this.id}`,
      operations: results
    };
  }

  executeSolver(task) {
    const results = [];

    // Sublinear solver
    results.push(wasm.solve_linear_system_sublinear(1000, 0.001));

    // PageRank computation
    results.push(wasm.compute_pagerank(10000, 0.85));

    // Grover iterations
    const grover = wasm.quantum_grover_iterations(1000000);
    results.push(`Grover search: ${grover} iterations for 1M items (${(1000000/grover).toFixed(0)}x speedup)`);

    // Phase estimation
    results.push(wasm.quantum_phase_estimation(Math.PI / 4));

    return {
      agent: `Solver-${this.id}`,
      operations: results
    };
  }

  executeAttractor(task) {
    const results = [];

    // Create Lorenz attractor
    results.push(wasm.create_lorenz_attractor(10, 28, 2.667));

    // Step through attractor states
    let state = [1, 1, 1];
    for (let i = 0; i < 3; i++) {
      const result = wasm.step_attractor(state[0], state[1], state[2], 0.01);
      results.push(`Step ${i + 1}: ${result}`);
      // Parse the result to update state
      const matches = result.match(/\[([\d.-]+), ([\d.-]+), ([\d.-]+)\]/);
      if (matches) {
        state = [parseFloat(matches[1]), parseFloat(matches[2]), parseFloat(matches[3])];
      }
    }

    // Create Lipschitz loop
    results.push(wasm.create_lipschitz_loop(0.9));

    return {
      agent: `Attractor-${this.id}`,
      operations: results
    };
  }
}

// Swarm coordinator
class SwarmCoordinator {
  constructor() {
    this.agents = [];
    this.topology = 'mesh'; // mesh, hierarchical, ring, star
  }

  createSwarm(agentConfigs) {
    console.log(chalk.green('\n▶ Initializing Nano-Agent Swarm...'));

    // Create swarm in WASM
    const swarmInfo = wasm.create_nano_swarm(agentConfigs.length);
    console.log(chalk.gray(`  ${swarmInfo}`));

    // Create agents
    agentConfigs.forEach(config => {
      const agent = new NanoAgent(config.id, config.type, config.capability);
      this.agents.push(agent);
      console.log(chalk.gray(`  ✓ Agent ${config.id} (${config.type}): ${config.capability} capability`));
    });

    // Benchmark the swarm
    const benchmark = wasm.benchmark_nano_agents(this.agents.length);
    console.log(chalk.gray(`  ${benchmark}`));
  }

  async runParallel(tasks) {
    console.log(chalk.green('\n▶ Executing Parallel Agent Tasks...'));

    const spinner = ora('Processing...').start();

    // Run swarm ticks
    const ticks = wasm.run_swarm_ticks(1000);

    // Execute tasks in parallel
    const promises = this.agents.map(async (agent, index) => {
      const task = tasks[index % tasks.length];
      return await agent.execute(task);
    });

    const results = await Promise.all(promises);

    spinner.succeed(`Completed ${ticks.toLocaleString()} operations`);

    return results;
  }

  displayResults(results) {
    console.log(chalk.green('\n▶ Agent Execution Results:\n'));

    results.forEach(result => {
      console.log(chalk.yellow(`━━━ ${result.agent} ━━━`));
      result.operations.forEach(op => {
        console.log(chalk.white(`  • ${op}`));
      });
      console.log();
    });
  }
}

// Main execution
async function main() {
  // Define agent configurations
  const agentConfigs = [
    { id: 'Q1', type: 'quantum', capability: 'quantum' },
    { id: 'C1', type: 'consciousness', capability: 'consciousness' },
    { id: 'T1', type: 'temporal', capability: 'temporal' },
    { id: 'S1', type: 'solver', capability: 'solver' },
    { id: 'A1', type: 'attractor', capability: 'attractor' },
    { id: 'Q2', type: 'quantum', capability: 'quantum' },
    { id: 'C2', type: 'consciousness', capability: 'consciousness' },
    { id: 'T2', type: 'temporal', capability: 'temporal' },
  ];

  // Define tasks
  const tasks = [
    { type: 'quantum', iterations: 100 },
    { type: 'consciousness', iterations: 500 },
    { type: 'temporal', horizon: 1000 },
    { type: 'solver', size: 10000 },
    { type: 'attractor', steps: 10 },
  ];

  // Create and run swarm
  const coordinator = new SwarmCoordinator();
  coordinator.createSwarm(agentConfigs);

  const results = await coordinator.runParallel(tasks);
  coordinator.displayResults(results);

  // Show swarm statistics
  console.log(chalk.cyan('╔════════════════════════════════════════════════════════════════════╗'));
  console.log(chalk.cyan('║                         SWARM STATISTICS                          ║'));
  console.log(chalk.cyan('╚════════════════════════════════════════════════════════════════════╝\n'));

  console.log(chalk.white(`Total Agents: ${agentConfigs.length}`));
  console.log(chalk.white(`Tasks Executed: ${results.length}`));
  console.log(chalk.white(`Topology: Mesh (fully connected)`));
  console.log(chalk.white(`Tick Budget: 25μs per agent`));

  // Calculate total operations
  let totalOps = 0;
  results.forEach(r => totalOps += r.operations.length);
  console.log(chalk.white(`Total Operations: ${totalOps}`));

  // Show system info
  console.log(chalk.gray(`\n${wasm.get_system_info()}`));
}

// Error handling
process.on('unhandledRejection', (err) => {
  console.error(chalk.red('\n✗ Error:'), err);
  process.exit(1);
});

// Run the demonstration
main().catch(console.error);