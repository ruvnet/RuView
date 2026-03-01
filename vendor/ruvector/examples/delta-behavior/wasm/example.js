/**
 * Example usage of @ruvector/delta-behavior WASM module
 *
 * This demonstrates how to use the delta-behavior WASM bindings
 * in JavaScript/TypeScript environments.
 *
 * Run: node --experimental-wasm-modules example.js
 * (After building with: npm run build)
 */

// Import the WASM module (web target)
import init, {
  WasmCoherence,
  WasmCoherenceBounds,
  WasmSelfLimitingReasoner,
  WasmEventHorizon,
  WasmCoherentSwarm,
  WasmContainmentSubstrate,
  WasmCapabilityDomain,
  WasmGracefulSystem,
  version,
  description,
} from './pkg/delta_behavior.js';

async function main() {
  // Initialize the WASM module
  await init();

  console.log('=== Delta-Behavior WASM Demo ===');
  console.log(`Version: ${version()}`);
  console.log(`Description: ${description()}`);
  console.log('');

  // =========================================================================
  // Demo 1: Coherence Basics
  // =========================================================================
  console.log('--- Demo 1: Coherence Basics ---');

  const coherence = new WasmCoherence(0.8);
  console.log(`Created coherence: ${coherence.value}`);
  console.log(`Is above 0.5? ${coherence.is_above(0.5)}`);
  console.log(`Is below 0.9? ${coherence.is_below(0.9)}`);

  const bounds = WasmCoherenceBounds.default_bounds();
  console.log(`Default bounds: min=${bounds.min_coherence}, throttle=${bounds.throttle_threshold}`);
  console.log('');

  // =========================================================================
  // Demo 2: Self-Limiting Reasoner
  // =========================================================================
  console.log('--- Demo 2: Self-Limiting Reasoner ---');
  console.log('A system that does LESS when uncertain.');
  console.log('');

  const reasoner = new WasmSelfLimitingReasoner(10, 5);
  console.log(`Initial: ${reasoner.status()}`);

  // Simulate coherence dropping
  for (let i = 0; i < 5; i++) {
    reasoner.update_coherence(-0.15);
    console.log(`After coherence drop: depth=${reasoner.allowed_depth()}, scope=${reasoner.allowed_scope()}, can_write=${reasoner.can_write_memory()}`);
  }
  console.log('');

  // =========================================================================
  // Demo 3: Computational Event Horizon
  // =========================================================================
  console.log('--- Demo 3: Event Horizon ---');
  console.log('Like a black hole - you can approach but never cross.');
  console.log('');

  const horizon = new WasmEventHorizon(3, 10.0);
  console.log(`Initial: ${horizon.status()}`);

  // Try to move toward the horizon
  for (let i = 0; i < 5; i++) {
    const target = JSON.stringify([i * 3, i * 2, i]);
    const result = JSON.parse(horizon.move_toward(target));
    console.log(`Move ${i + 1}: status=${result.status}, distance_to_horizon=${result.distance_to_horizon?.toFixed(2) || 'N/A'}`);

    if (result.energy_exhausted) {
      console.log('Energy exhausted - system frozen!');
      break;
    }
  }
  console.log('');

  // =========================================================================
  // Demo 4: Coherent Swarm
  // =========================================================================
  console.log('--- Demo 4: Coherent Swarm ---');
  console.log('Local actions allowed, global incoherence forbidden.');
  console.log('');

  const swarm = new WasmCoherentSwarm(0.6);

  // Create a tight cluster
  swarm.add_agent('a1', 0, 0);
  swarm.add_agent('a2', 1, 0);
  swarm.add_agent('a3', 0, 1);
  swarm.add_agent('a4', 1, 1);

  console.log(`Initial: ${swarm.status()}`);

  // Try to move one agent far away (should be rejected or modified)
  const divergentAction = JSON.stringify({
    action_type: 'move',
    dx: 100,
    dy: 100,
  });
  const result = JSON.parse(swarm.execute_action('a1', divergentAction));
  console.log(`Divergent action result: ${JSON.stringify(result)}`);

  // Coherent action should work
  const coherentAction = JSON.stringify({
    action_type: 'move',
    dx: 0.5,
    dy: 0.5,
  });
  const result2 = JSON.parse(swarm.execute_action('a2', coherentAction));
  console.log(`Coherent action result: ${JSON.stringify(result2)}`);
  console.log(`After actions: ${swarm.status()}`);
  console.log('');

  // =========================================================================
  // Demo 5: Containment Substrate
  // =========================================================================
  console.log('--- Demo 5: Containment Substrate ---');
  console.log('Intelligence can grow, but only if coherence is preserved.');
  console.log('');

  const substrate = new WasmContainmentSubstrate();
  console.log(`Initial: ${substrate.status()}`);

  // Try to grow capabilities
  const domains = [
    [WasmCapabilityDomain.Reasoning, 'Reasoning'],
    [WasmCapabilityDomain.Learning, 'Learning'],
    [WasmCapabilityDomain.SelfModification, 'SelfModification'],
  ];

  for (const [domain, name] of domains) {
    // Attempt to grow
    const growthResult = JSON.parse(substrate.attempt_growth(domain, 0.5));
    console.log(`Growing ${name}: ${growthResult.status}`);

    // Rest to recover coherence
    for (let i = 0; i < 5; i++) {
      substrate.rest();
    }
  }

  console.log(`Final: ${substrate.status()}`);
  console.log(`Invariants hold: ${substrate.check_invariants()}`);
  console.log(`Capability report: ${substrate.capability_report()}`);
  console.log('');

  // =========================================================================
  // Demo 6: Graceful Shutdown
  // =========================================================================
  console.log('--- Demo 6: Graceful Shutdown ---');
  console.log('Shutdown is an attractor, not a failure.');
  console.log('');

  const graceful = new WasmGracefulSystem();
  graceful.add_resource('database_connection');
  graceful.add_resource('cache');
  graceful.add_resource('temp_files');

  console.log(`Initial: ${graceful.status()}`);
  console.log(`Can accept work: ${graceful.can_accept_work()}`);

  // Simulate degradation
  console.log('Simulating gradual degradation...');
  for (let i = 0; i < 10; i++) {
    graceful.apply_coherence_change(-0.08);
    const status = JSON.parse(graceful.status());
    console.log(`Step ${i + 1}: state=${status.state}, coherence=${status.coherence.toFixed(2)}, shutdown_prep=${(status.shutdown_preparation * 100).toFixed(0)}%`);

    if (status.state === 'ShuttingDown') {
      console.log('System entering graceful shutdown...');
      break;
    }
  }

  // Progress shutdown
  while (JSON.parse(graceful.status()).state !== 'Terminated') {
    const progress = JSON.parse(graceful.progress_shutdown());
    console.log(`Shutdown progress: ${JSON.stringify(progress)}`);

    if (progress.status === 'terminated') {
      break;
    }
  }
  console.log(`Final: ${graceful.status()}`);
  console.log('');

  console.log('=== Demo Complete ===');
  console.log('');
  console.log('Key insights:');
  console.log('1. Systems can change, but not collapse');
  console.log('2. Coherence is the invariant that must be preserved');
  console.log('3. Destabilizing transitions are blocked or modified');
  console.log('4. Systems bias toward stable attractors');
}

main().catch(console.error);
