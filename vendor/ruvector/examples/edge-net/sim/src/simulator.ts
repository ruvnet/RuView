#!/usr/bin/env node
/**
 * Main Simulation Engine
 * Orchestrates the complete edge-net lifecycle simulation
 */

import { Network, NetworkPhase } from './network.js';
import { MetricsCollector } from './metrics.js';
import { PhaseManager } from './phases.js';
import { ReportGenerator } from './report.js';

interface SimulationConfig {
  verbose: boolean;
  fast: boolean;
  outputFile: string;
}

class EdgeNetSimulator {
  private network: Network;
  private metrics: MetricsCollector;
  private phaseManager: PhaseManager;
  private reportGenerator: ReportGenerator;
  private config: SimulationConfig;
  private progressInterval: number;

  constructor(config: SimulationConfig) {
    this.config = config;
    this.progressInterval = config.fast ? 1000 : 100;

    // Initialize components
    this.network = new Network({
      genesisNodeCount: 100,
      targetNodeCount: 120000,
      nodesPerTick: config.fast ? 100 : 10, // Faster node spawning in fast mode
      taskGenerationRate: 5,
      baseTaskReward: 1.0,
      connectionCost: 0.5,
      maxConnectionsPerNode: 50,
    });

    this.metrics = new MetricsCollector(this.network);
    this.phaseManager = new PhaseManager(this.network, this.metrics);
    this.reportGenerator = new ReportGenerator(this.network, this.metrics);
  }

  /**
   * Run the complete simulation
   */
  public async run(): Promise<void> {
    console.log('╔════════════════════════════════════════════════════════════╗');
    console.log('║    EDGE-NET LIFECYCLE SIMULATION - Starting...            ║');
    console.log('╚════════════════════════════════════════════════════════════╝\n');

    console.log('⚙️  Configuration:');
    console.log(`   Genesis Nodes: ${this.network.config.genesisNodeCount}`);
    console.log(`   Target Nodes: ${this.network.config.targetNodeCount.toLocaleString()}`);
    console.log(`   Nodes/Tick: ${this.network.config.nodesPerTick}`);
    console.log(`   Mode: ${this.config.fast ? 'FAST' : 'NORMAL'}`);
    console.log('');

    // Initialize network with genesis nodes
    this.network.initialize();
    this.metrics.initialize();

    console.log('🌱 Genesis nodes deployed. Starting simulation...\n');

    let lastProgressUpdate = 0;
    const startTime = Date.now();

    // Main simulation loop
    while (this.network.currentPhase !== NetworkPhase.INDEPENDENCE ||
           this.network.cells.size < this.network.config.targetNodeCount) {

      // Simulate one tick
      this.network.tick();
      this.metrics.collect();
      this.phaseManager.checkTransition();

      // Progress updates
      if (this.network.currentTick - lastProgressUpdate >= this.progressInterval) {
        this.printProgress();
        lastProgressUpdate = this.network.currentTick;
      }

      // Safety check - don't run forever
      if (this.network.currentTick > 50000) {
        console.log('\n⚠️  Simulation timeout reached (50,000 ticks)');
        break;
      }
    }

    const endTime = Date.now();
    const duration = (endTime - startTime) / 1000;

    console.log('\n✨ Simulation complete!\n');
    console.log(`   Total Ticks: ${this.network.currentTick.toLocaleString()}`);
    console.log(`   Duration: ${duration.toFixed(2)}s`);
    console.log(`   Final Nodes: ${this.network.cells.size.toLocaleString()}`);
    console.log(`   Final Phase: ${this.network.currentPhase.toUpperCase()}\n`);

    // Finalize metrics
    this.metrics.finalizeCurrent();

    // Generate and save report
    this.reportGenerator.printSummary();
    this.reportGenerator.saveReport(this.config.outputFile);

    // Exit with appropriate code
    const report = this.reportGenerator.generateReport();
    process.exit(report.summary.totalPassed ? 0 : 1);
  }

  /**
   * Print simulation progress
   */
  private printProgress(): void {
    const stats = this.network.getStats();
    const progress = this.phaseManager.getPhaseProgress();
    const ticksToNext = this.phaseManager.getTicksToNextPhase();

    if (this.config.verbose) {
      console.log(`[Tick ${this.network.currentTick}] ${this.network.currentPhase.toUpperCase()}`);
      console.log(`  Nodes: ${stats.nodeCount.toLocaleString()} | Energy: ${stats.economy.totalEnergy.toFixed(2)} rUv`);
      console.log(`  Tasks: ${stats.tasks.completed.toLocaleString()} | Success: ${(stats.network.avgSuccessRate * 100).toFixed(1)}%`);
      console.log(`  Genesis: ${stats.genesisNodes.active} active, ${stats.genesisNodes.readOnly} read-only, ${stats.genesisNodes.retired} retired`);
      console.log(`  Progress: ${(progress * 100).toFixed(1)}% | Next phase: ${ticksToNext >= 0 ? `~${ticksToNext} ticks` : 'N/A'}`);
      console.log('');
    } else {
      // Compact progress bar
      const barLength = 40;
      const filled = Math.floor(progress * barLength);
      const bar = '█'.repeat(filled) + '░'.repeat(barLength - filled);

      process.stdout.write(
        `\r[${bar}] ${this.network.currentPhase.padEnd(12)} | ` +
        `${stats.nodeCount.toLocaleString().padStart(7)} nodes | ` +
        `${stats.tasks.completed.toLocaleString().padStart(8)} tasks | ` +
        `Genesis: ${stats.genesisNodes.retired}/${stats.genesisNodes.count} retired`
      );
    }
  }
}

// Parse command line arguments
function parseArgs(): SimulationConfig {
  const args = process.argv.slice(2);

  return {
    verbose: args.includes('--verbose') || args.includes('-v'),
    fast: args.includes('--fast') || args.includes('-f'),
    outputFile: args.find(arg => arg.startsWith('--output='))?.split('=')[1] ||
                '/workspaces/ruvector/examples/edge-net/sim/simulation-report.json',
  };
}

// Run simulation
const config = parseArgs();
const simulator = new EdgeNetSimulator(config);

simulator.run().catch(error => {
  console.error('❌ Simulation failed:', error);
  process.exit(1);
});
