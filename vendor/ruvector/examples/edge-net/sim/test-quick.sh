#!/bin/bash
# Quick test of the simulation with reduced node count

echo "Running quick simulation test (20K nodes)..."

# Temporarily modify target to 20K for quick test
node --loader ts-node/esm -e "
import { Network } from './src/network.js';
import { MetricsCollector } from './src/metrics.js';
import { PhaseManager } from './src/phases.js';
import { ReportGenerator } from './src/report.js';
import { NetworkPhase } from './src/network.js';

const network = new Network({
  genesisNodeCount: 50,
  targetNodeCount: 20000,
  nodesPerTick: 100,
  taskGenerationRate: 5,
  baseTaskReward: 1.0,
  connectionCost: 0.5,
  maxConnectionsPerNode: 50,
});

const metrics = new MetricsCollector(network);
const phaseManager = new PhaseManager(network, metrics);
const reportGenerator = new ReportGenerator(network, metrics);

console.log('Initializing network...');
network.initialize();
metrics.initialize();

let lastUpdate = 0;
while (network.cells.size < 20000 && network.currentTick < 5000) {
  network.tick();
  metrics.collect();
  phaseManager.checkTransition();

  if (network.currentTick - lastUpdate >= 50) {
    const stats = network.getStats();
    console.log(\`Tick \${network.currentTick}: \${stats.nodeCount} nodes | Phase: \${network.currentPhase}\`);
    lastUpdate = network.currentTick;
  }
}

metrics.finalizeCurrent();
console.log('\\nGenerating report...');
reportGenerator.printSummary();
reportGenerator.saveReport('/workspaces/ruvector/examples/edge-net/sim/test-report.json');
console.log('✅ Quick test complete!');
"
