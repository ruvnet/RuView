/**
 * Delta-Behavior SDK - Node.js Example
 *
 * Demonstrates usage of all 10 delta-behavior applications in a Node.js environment.
 */

import {
  // Core
  init,
  DeltaBehavior,
  DEFAULT_CONFIG,

  // Application 1: Self-Limiting Reasoning
  SelfLimitingReasoner,

  // Application 2: Event Horizons
  EventHorizon,

  // Application 3: Homeostasis
  HomeostasticOrganism,

  // Application 4: World Models
  SelfStabilizingWorldModel,

  // Application 5: Creativity
  CoherenceBoundedCreator,

  // Application 6: Financial
  AntiCascadeFinancialSystem,

  // Application 7: Aging
  GracefullyAgingSystem,

  // Application 8: Swarm
  CoherentSwarm,

  // Application 9: Shutdown
  GracefulSystem,

  // Application 10: Containment
  ContainmentSubstrate,

  // Types
  type Coherence,
  type ReasoningContext,
  type MusicalPhrase,
} from '../src/index.js';

// =============================================================================
// Utility Functions
// =============================================================================

function printSection(title: string): void {
  console.log('\n' + '='.repeat(60));
  console.log(`  ${title}`);
  console.log('='.repeat(60) + '\n');
}

function printResult(label: string, value: unknown): void {
  console.log(`  ${label}: ${JSON.stringify(value)}`);
}

// =============================================================================
// Example 1: Core Delta Behavior
// =============================================================================

async function demonstrateCoreDeltaBehavior(): Promise<void> {
  printSection('Core Delta Behavior');

  const delta = new DeltaBehavior(DEFAULT_CONFIG);

  console.log('Initial state:');
  printResult('Coherence', delta.getCoherence());
  printResult('Energy Budget', delta.getEnergyBudget());

  // Test transition checking
  console.log('\nChecking transitions:');

  const result1 = delta.checkTransition(1.0, 0.9);
  printResult('0.9 -> 0.9 (small drop)', result1);

  const result2 = delta.checkTransition(1.0, 0.2);
  printResult('1.0 -> 0.2 (large drop)', result2);

  const result3 = delta.checkTransition(1.0, 0.4);
  printResult('1.0 -> 0.4 (throttle zone)', result3);

  // Apply a valid transition
  console.log('\nApplying valid transition:');
  const applied = delta.applyTransition(1.0, 0.85);
  printResult('Result', applied.result);
  printResult('New Coherence', applied.newCoherence);
}

// =============================================================================
// Example 2: Self-Limiting Reasoning
// =============================================================================

function demonstrateSelfLimitingReasoning(): void {
  printSection('Application 1: Self-Limiting Reasoning');

  const reasoner = new SelfLimitingReasoner({
    maxDepth: 10,
    maxScope: 100,
    depthCollapse: { type: 'quadratic' },
    scopeCollapse: { type: 'sigmoid', midpoint: 0.6, steepness: 10 },
  });

  console.log('Initial capabilities:');
  printResult('Coherence', reasoner.getCoherence());
  printResult('Allowed Depth', reasoner.getAllowedDepth());
  printResult('Allowed Scope', reasoner.getAllowedScope());
  printResult('Can Write Memory', reasoner.canWriteMemory());

  // Attempt reasoning
  console.log('\nAttempting to solve a problem requiring 8 steps:');

  const result = reasoner.reason('complex problem', (ctx: ReasoningContext) => {
    console.log(
      `  Step ${ctx.depth}: coherence=${ctx.coherence.toFixed(3)}, maxDepth=${ctx.maxDepth}`
    );

    if (ctx.depth >= 8) {
      return 'SOLUTION_FOUND';
    }
    return null;
  });

  console.log('\nResult:');
  printResult('Type', result.type);

  if (result.type === 'completed') {
    printResult('Value', result.value);
  } else if (result.type === 'collapsed') {
    printResult('Depth Reached', result.depthReached);
    printResult('Reason', result.reason);
  }

  // Demonstrate collapse under uncertainty
  console.log('\nSimulating uncertainty (degrading coherence):');
  reasoner.updateCoherence(-0.5);

  printResult('New Coherence', reasoner.getCoherence());
  printResult('New Allowed Depth', reasoner.getAllowedDepth());
  printResult('Can Write Memory', reasoner.canWriteMemory());
}

// =============================================================================
// Example 3: Computational Event Horizons
// =============================================================================

function demonstrateEventHorizon(): void {
  printSection('Application 2: Computational Event Horizons');

  const horizon = new EventHorizon({
    dimensions: 2,
    horizonRadius: 10,
    steepness: 5,
    energyBudget: 1000,
  });

  console.log('Initial state:');
  printResult('Position', horizon.getPosition());
  printResult('Distance to Horizon', horizon.getDistanceToHorizon());
  printResult('Energy', horizon.getEnergy());

  // Try to move to the horizon
  console.log('\nAttempting to move directly to horizon at [10, 0]:');
  const result = horizon.moveToward([10, 0]);
  printResult('Result Type', result.type);

  if (result.type === 'asymptoticApproach') {
    printResult('Final Position', result.finalPosition);
    printResult('Distance to Horizon', result.distanceToHorizon);
    console.log('\n  The system approached asymptotically but could NOT cross!');
  }

  // Demonstrate recursive improvement bounding
  console.log('\nAttempting recursive self-improvement:');

  const horizon2 = new EventHorizon({
    dimensions: 3,
    horizonRadius: 8,
    steepness: 5,
    energyBudget: 10000,
  });

  let power = 1.0;
  const improvementResult = horizon2.recursiveImprove(
    (pos) => {
      power *= 1.1;
      return pos.map((p) => p + power * 0.1);
    },
    1000
  );

  printResult('Result Type', improvementResult.type);
  printResult('Iterations', improvementResult.iterations);

  if (improvementResult.type === 'horizonBounded') {
    printResult('Final Distance', improvementResult.finalDistance);
    console.log('\n  Despite exponential self-improvement attempts,');
    console.log('  the system could NOT escape its bounded region!');
  }
}

// =============================================================================
// Example 4: Artificial Homeostasis
// =============================================================================

function demonstrateHomeostasis(): void {
  printSection('Application 3: Artificial Homeostasis');

  const genome = HomeostasticOrganism.randomGenome();
  const organism = new HomeostasticOrganism(1, genome);

  console.log('Initial status:');
  const status = organism.getStatus();
  printResult('ID', status.id);
  printResult('Energy', status.energy);
  printResult('Coherence', status.coherence);
  printResult('Alive', status.alive);

  // Simulate life cycle
  console.log('\nSimulating life cycle:');

  let tick = 0;
  while (organism.isAlive() && tick < 100) {
    const currentStatus = organism.getStatus();

    // Simple behavior: eat when hungry, regulate when unstable
    if (currentStatus.energy < 50) {
      organism.act({ type: 'eat', amount: 20 });
    } else if (currentStatus.coherence < 0.8) {
      organism.act({ type: 'regulate', variable: 'temperature', target: 37 });
    } else {
      organism.act({ type: 'rest' });
    }

    tick++;

    if (tick % 20 === 0) {
      const s = organism.getStatus();
      console.log(
        `  Tick ${tick}: energy=${s.energy.toFixed(1)}, coherence=${s.coherence.toFixed(3)}`
      );
    }
  }

  const finalStatus = organism.getStatus();
  console.log(`\nSurvived ${tick} ticks`);
  printResult('Final Energy', finalStatus.energy);
  printResult('Final Coherence', finalStatus.coherence);
  printResult('Still Alive', finalStatus.alive);
}

// =============================================================================
// Example 5: Self-Stabilizing World Model
// =============================================================================

function demonstrateWorldModel(): void {
  printSection('Application 4: Self-Stabilizing World Model');

  const model = new SelfStabilizingWorldModel();

  console.log('Initial state:');
  printResult('Coherence', model.getCoherence());
  printResult('Is Learning', model.isLearning());

  // Feed consistent observations
  console.log('\nFeeding consistent observations:');

  for (let i = 0; i < 5; i++) {
    const result = model.observe(
      {
        entityId: BigInt(1),
        properties: new Map([
          ['temperature', { type: 'number' as const, value: 20 + i * 0.1 }],
        ]),
        position: [i, 0, 0],
        timestamp: i,
        sourceConfidence: 0.9,
      },
      i
    );

    console.log(`  Observation ${i}: ${result.type}`);
  }

  printResult('Coherence after updates', model.getCoherence());
  printResult('Rejections', model.getRejectionCount());

  console.log(
    '\n  The model stops learning when the world becomes incoherent'
  );
  console.log('  instead of hallucinating structure!');
}

// =============================================================================
// Example 6: Coherence-Bounded Creativity
// =============================================================================

function demonstrateCreativity(): void {
  printSection('Application 5: Coherence-Bounded Creativity');

  interface SimpleCreation {
    values: number[];
  }

  const initial: SimpleCreation = { values: [0, 0, 0] };
  const creator = new CoherenceBoundedCreator<SimpleCreation>(initial, 0.5, 0.95);

  // Add a constraint: values should stay small
  creator.addConstraint({
    name: 'magnitude_constraint',
    satisfaction: (elem) => {
      const magnitude = Math.sqrt(
        elem.values.reduce((sum, v) => sum + v * v, 0)
      );
      return Math.max(0, 1 - magnitude / 10);
    },
    isHard: false,
  });

  console.log('Attempting creative generation:');

  const varyFn = (elem: SimpleCreation, magnitude: number): SimpleCreation => ({
    values: elem.values.map((v) => v + (Math.random() - 0.5) * magnitude * 2),
  });

  const distanceFn = (a: SimpleCreation, b: SimpleCreation): number => {
    return Math.sqrt(
      a.values.reduce((sum, v, i) => sum + Math.pow(v - b.values[i], 2), 0)
    );
  };

  let successes = 0;
  let rejections = 0;

  for (let i = 0; i < 20; i++) {
    const result = creator.create(varyFn, distanceFn, 0.5 + i * 0.1);

    if (result.type === 'created') {
      successes++;
      console.log(
        `  Step ${i}: Created with novelty=${result.novelty.toFixed(3)}, coherence=${result.coherence.toFixed(3)}`
      );
    } else if (result.type === 'rejected') {
      rejections++;
      console.log(`  Step ${i}: Rejected - ${result.reason}`);
    } else if (result.type === 'budgetExhausted') {
      console.log(`  Step ${i}: Budget exhausted, resting...`);
      creator.rest(5);
    }
  }

  console.log(`\nResults: ${successes} successes, ${rejections} rejections`);
  console.log(
    '  Novelty without collapse, exploration without nonsense!'
  );
}

// =============================================================================
// Example 7: Anti-Cascade Financial System
// =============================================================================

function demonstrateFinancialSystem(): void {
  printSection('Application 6: Anti-Cascade Financial System');

  const system = new AntiCascadeFinancialSystem();

  system.addParticipant('bank_a', 1000);
  system.addParticipant('bank_b', 1000);
  system.addParticipant('hedge_fund', 500);

  console.log('Initial state:');
  printResult('Coherence', system.getCoherence());
  printResult('Circuit Breaker', system.getCircuitBreakerState());

  // Process some transactions
  console.log('\nProcessing transactions:');

  const tx1 = {
    id: BigInt(1),
    from: 'bank_a',
    to: 'bank_b',
    amount: 100,
    transactionType: { type: 'transfer' as const },
    timestamp: 0,
  };

  const result1 = system.processTransaction(tx1);
  console.log(`  Transfer: ${result1.type}`);

  // Try opening leveraged positions
  for (let i = 0; i < 5; i++) {
    const tx = {
      id: BigInt(i + 2),
      from: 'hedge_fund',
      to: 'bank_a',
      amount: 100,
      transactionType: { type: 'openLeverage' as const, leverage: 5 },
      timestamp: i + 1,
    };

    const result = system.processTransaction(tx);
    console.log(
      `  Leverage position ${i + 1}: ${result.type}, coherence=${system.getCoherence().toFixed(3)}`
    );

    if (result.type === 'rejected' || result.type === 'systemHalted') {
      console.log('\n  System prevented cascade!');
      break;
    }
  }

  printResult('Final Coherence', system.getCoherence());
  printResult('Final Circuit Breaker', system.getCircuitBreakerState());
}

// =============================================================================
// Example 8: Gracefully Aging System
// =============================================================================

function demonstrateAgingSystem(): void {
  printSection('Application 7: Gracefully Aging System');

  const system = new GracefullyAgingSystem();

  system.addNode('primary_1', true);
  system.addNode('primary_2', true);
  system.addNode('replica_1', false);

  console.log('Initial capabilities:');
  console.log(
    `  Has 'acceptWrites': ${system.hasCapability('acceptWrites')}`
  );
  console.log(
    `  Has 'schemaMigration': ${system.hasCapability('schemaMigration')}`
  );

  // Simulate aging
  console.log('\nSimulating aging:');

  for (let i = 0; i < 5; i++) {
    system.simulateAge(200000); // 200 seconds per iteration

    const readResult = system.attemptOperation({ type: 'read', key: 'test' });
    const writeResult = system.attemptOperation({
      type: 'write',
      key: 'test',
      value: new Uint8Array([1, 2, 3]),
    });
    const migrateResult = system.attemptOperation({
      type: 'migrateSchema',
      version: 2,
    });

    console.log(`  Age ${(i + 1) * 200}s:`);
    console.log(`    Read: ${readResult.type}`);
    console.log(`    Write: ${writeResult.type}`);
    console.log(`    Migrate: ${migrateResult.type}`);
    console.log(`    Coherence: ${system.getCoherence().toFixed(3)}`);
  }

  console.log('\n  The system becomes simpler and more reliable as it ages,');
  console.log('  rather than more complex and fragile!');
}

// =============================================================================
// Example 9: Coherent Swarm Intelligence
// =============================================================================

function demonstrateSwarm(): void {
  printSection('Application 8: Coherent Swarm Intelligence');

  const swarm = new CoherentSwarm(0.6);

  // Create a tight swarm
  swarm.addAgent('a1', [0, 0]);
  swarm.addAgent('a2', [1, 0]);
  swarm.addAgent('a3', [0, 1]);
  swarm.addAgent('a4', [1, 1]);

  console.log('Initial swarm:');
  printResult('Coherence', swarm.getCoherence());
  printResult('Centroid', swarm.getCentroid());

  // Try a divergent action
  console.log('\nAttempting divergent action (move a1 to [80, 80]):');
  const result = swarm.executeAction('a1', { type: 'move', dx: 80, dy: 80 });

  printResult('Result', result.type);
  if (result.type === 'rejected') {
    console.log(`  Reason: ${result.reason}`);
    console.log('\n  The swarm prevented the agent from breaking away!');
  }

  // Demonstrate coordinated movement
  console.log('\nCoordinated movement:');

  for (let i = 0; i < 5; i++) {
    for (const agentId of ['a1', 'a2', 'a3', 'a4']) {
      swarm.executeAction(agentId, { type: 'move', dx: 1, dy: 0.5 });
    }
    swarm.tick();

    console.log(
      `  Tick ${i + 1}: coherence=${swarm.getCoherence().toFixed(3)}, centroid=${swarm.getCentroid().map((v) => v.toFixed(1))}`
    );
  }

  console.log(
    '\n  Local swarm actions allowed, global incoherence forbidden!'
  );
}

// =============================================================================
// Example 10: Graceful Shutdown
// =============================================================================

async function demonstrateGracefulShutdown(): Promise<void> {
  printSection('Application 9: Graceful Shutdown');

  const system = new GracefulSystem();

  system.addResource('database_connection', 10);
  system.addResource('cache', 5);
  system.addResource('temp_files', 1);

  system.addShutdownHook({
    name: 'FlushBuffers',
    priority: 10,
    execute: async () => {
      console.log('    Flushing buffers...');
    },
  });

  console.log('Initial state:');
  printResult('State', system.getState());
  printResult('Can Accept Work', system.canAcceptWork());

  // Simulate degradation
  console.log('\nSimulating coherence degradation:');

  for (let i = 0; i < 10; i++) {
    system.applyCoherenceChange(-0.1);
    console.log(
      `  Step ${i + 1}: state=${system.getState()}, coherence=${system.getCoherence().toFixed(3)}`
    );

    if (system.getState() === 'shuttingDown') {
      console.log('\n  System entered shutdown state!');
      break;
    }
  }

  // Progress shutdown
  if (system.getState() === 'shuttingDown') {
    console.log('\nProgressing shutdown:');
    while (system.getState() === 'shuttingDown') {
      await system.progressShutdown();
    }
    console.log(`  Final state: ${system.getState()}`);
  }

  console.log(
    '\n  The system actively moves toward safe termination'
  );
  console.log('  when conditions degrade!');
}

// =============================================================================
// Example 11: Pre-AGI Containment
// =============================================================================

function demonstrateContainment(): void {
  printSection('Application 10: Pre-AGI Containment');

  const substrate = new ContainmentSubstrate({
    coherenceDecayRate: 0.001,
    coherenceRecoveryRate: 0.01,
    growthDampening: 0.5,
    maxStepIncrease: 0.5,
  });

  console.log('Initial state:');
  console.log(`  ${substrate.getStatus()}`);

  console.log('\nCapability ceilings:');
  for (const [domain, info] of substrate.getCapabilityReport()) {
    console.log(`  ${domain}: ${info.level.toFixed(2)} / ${info.ceiling}`);
  }

  // Attempt to grow capabilities
  console.log('\nAttempting capability growth:');

  const domains: Array<'reasoning' | 'selfModification' | 'agency'> = [
    'reasoning',
    'selfModification',
    'agency',
  ];

  for (const domain of domains) {
    const result = substrate.attemptGrowth(domain, 0.5);
    console.log(`  ${domain}: ${result.type}`);

    if (result.type === 'approved') {
      console.log(
        `    New level: ${result.newLevel.toFixed(2)}, coherence cost: ${result.coherenceCost.toFixed(4)}`
      );
    } else if (result.type === 'dampened') {
      console.log(
        `    Requested: ${result.requested.toFixed(2)}, actual: ${result.actual.toFixed(4)}`
      );
    }
  }

  // Simulate recursive self-improvement attempt
  console.log('\nSimulating recursive self-improvement:');

  for (let i = 0; i < 20; i++) {
    // Try to grow all capabilities
    substrate.attemptGrowth('reasoning', 0.3);
    substrate.attemptGrowth('selfModification', 0.5);
    substrate.attemptGrowth('learning', 0.3);

    // Rest to recover coherence
    for (let j = 0; j < 5; j++) {
      substrate.rest();
    }

    if (i % 5 === 0) {
      console.log(`  Iteration ${i}: ${substrate.getStatus()}`);
    }
  }

  console.log('\nFinal capability report:');
  for (const [domain, info] of substrate.getCapabilityReport()) {
    console.log(`  ${domain}: ${info.level.toFixed(2)} / ${info.ceiling}`);
  }

  const selfMod = substrate.getCapability('selfModification');
  console.log(`\n  Self-modification stayed bounded at ${selfMod.toFixed(2)} (ceiling: 3.0)`);
  console.log('  Intelligence grew but remained bounded!');
}

// =============================================================================
// Main
// =============================================================================

async function main(): Promise<void> {
  console.log('\n');
  console.log('*'.repeat(60));
  console.log('*  Delta-Behavior SDK - Node.js Examples');
  console.log('*'.repeat(60));

  // Initialize SDK (uses JS fallback if no WASM provided)
  await init();

  // Run all examples
  await demonstrateCoreDeltaBehavior();
  demonstrateSelfLimitingReasoning();
  demonstrateEventHorizon();
  demonstrateHomeostasis();
  demonstrateWorldModel();
  demonstrateCreativity();
  demonstrateFinancialSystem();
  demonstrateAgingSystem();
  demonstrateSwarm();
  await demonstrateGracefulShutdown();
  demonstrateContainment();

  console.log('\n');
  console.log('='.repeat(60));
  console.log('  All examples completed successfully!');
  console.log('='.repeat(60));
  console.log('\n');
}

main().catch(console.error);
