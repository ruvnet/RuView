# Delta-Behavior WASM SDK

A TypeScript/JavaScript SDK for coherence-preserving state transitions and self-limiting AI systems.

## Overview

The Delta-Behavior SDK provides implementations of 10 "exotic" mathematical properties that enable AI systems to be self-limiting, bounded, and safe by construction. Based on the research paper "Delta-Behavior: A Mathematical Framework for Coherence-Preserving State Transitions."

## Installation

```bash
npm install @ruvector/delta-behavior
```

Or with yarn:

```bash
yarn add @ruvector/delta-behavior
```

## Quick Start

```typescript
import { init, DeltaBehavior, ContainmentSubstrate } from '@ruvector/delta-behavior';

// Initialize the SDK (optional WASM for performance)
await init();

// Core delta behavior system
const delta = new DeltaBehavior();

// Check if a transition is allowed
const result = delta.checkTransition(1.0, 0.8);
// => { type: 'allowed' }

// Pre-AGI containment substrate
const substrate = new ContainmentSubstrate();
const growth = substrate.attemptGrowth('reasoning', 0.5);
// Growth is bounded by coherence requirements
```

## Applications

The SDK implements 10 applications, each demonstrating a different "exotic" property:

### 1. Self-Limiting Reasoning

A reasoning system that automatically limits its depth and scope based on coherence.

```typescript
import { SelfLimitingReasoner } from '@ruvector/delta-behavior';

const reasoner = new SelfLimitingReasoner({
  maxDepth: 10,
  maxScope: 100,
  depthCollapse: { type: 'quadratic' },
});

// Reasoning depth collapses as coherence drops
console.log(reasoner.getAllowedDepth()); // 10 at full coherence

reasoner.updateCoherence(-0.5);
console.log(reasoner.getAllowedDepth()); // ~2 at 50% coherence

// Attempt reasoning that requires 8 steps
const result = reasoner.reason('complex problem', (ctx) => {
  if (ctx.depth >= 8) return 'SOLUTION';
  return null;
});
// result.type may be 'collapsed' if coherence is too low
```

### 2. Computational Event Horizons

Define boundaries in state space that cannot be crossed.

```typescript
import { EventHorizon } from '@ruvector/delta-behavior';

const horizon = new EventHorizon({
  dimensions: 2,
  horizonRadius: 10,
  energyBudget: 1000,
});

// Try to move to the edge
const result = horizon.moveToward([10, 0]);
// result.type === 'asymptoticApproach'
// Cannot cross the horizon - approaches asymptotically

// Recursive self-improvement is bounded
const improvement = horizon.recursiveImprove(
  (pos) => pos.map(p => p + 0.5), // Always try to go further
  1000 // Max iterations
);
// improvement.type === 'horizonBounded'
// System finds its own stopping point
```

### 3. Artificial Homeostasis

Organisms with coherence-enforced internal regulation.

```typescript
import { HomeostasticOrganism } from '@ruvector/delta-behavior';

const genome = HomeostasticOrganism.randomGenome();
const organism = new HomeostasticOrganism(1, genome);

// Actions cost more energy when coherence is low
organism.act({ type: 'eat', amount: 20 });
organism.act({ type: 'regulate', variable: 'temperature', target: 37 });
organism.act({ type: 'rest' });

// Memory is lost under uncertainty (low coherence)
// Organism naturally maintains homeostasis or dies trying
```

### 4. Self-Stabilizing World Models

World models that refuse to learn incoherent updates.

```typescript
import { SelfStabilizingWorldModel } from '@ruvector/delta-behavior';

const model = new SelfStabilizingWorldModel();

// Feed observations
const result = model.observe({
  entityId: BigInt(1),
  properties: new Map([['temperature', { type: 'number', value: 20 }]]),
  position: [0, 0, 0],
  timestamp: 0,
  sourceConfidence: 0.9,
}, 0);

// Model rejects updates that would make the world incoherent
// Instead of hallucinating structure, it freezes learning
```

### 5. Coherence-Bounded Creativity

Creative generation within coherence-preserving manifolds.

```typescript
import { CoherenceBoundedCreator } from '@ruvector/delta-behavior';

const creator = new CoherenceBoundedCreator(
  { values: [0, 0, 0] }, // Initial element
  0.6,  // Min coherence
  0.95  // Max coherence (too high = boring)
);

creator.addConstraint({
  name: 'magnitude',
  satisfaction: (elem) => Math.max(0, 1 - magnitude(elem) / 10),
  isHard: false,
});

const result = creator.create(varyFn, distanceFn, 0.5);
// Novelty without collapse, exploration without nonsense
```

### 6. Anti-Cascade Financial System

Financial systems that cannot cascade into collapse by construction.

```typescript
import { AntiCascadeFinancialSystem } from '@ruvector/delta-behavior';

const system = new AntiCascadeFinancialSystem();
system.addParticipant('bank_a', 1000);
system.addParticipant('bank_b', 1000);

// Transactions that would reduce coherence are blocked
const result = system.processTransaction({
  id: BigInt(1),
  from: 'bank_a',
  to: 'bank_b',
  amount: 100,
  transactionType: { type: 'openLeverage', leverage: 10 },
  timestamp: 0,
});
// result.type may be 'rejected' if it threatens coherence

// Circuit breaker activates automatically
console.log(system.getCircuitBreakerState()); // 'open' | 'cautious' | 'restricted' | 'halted'
```

### 7. Gracefully Aging Systems

Distributed systems that become simpler and more reliable with age.

```typescript
import { GracefullyAgingSystem } from '@ruvector/delta-behavior';

const system = new GracefullyAgingSystem();
system.addNode('primary', true);

// As the system ages, capabilities are gracefully removed
system.simulateAge(600000); // 10 minutes

console.log(system.hasCapability('schemaMigration')); // false
console.log(system.hasCapability('basicReads'));       // true (always available)

// Operations become more conservative over time
const result = system.attemptOperation({ type: 'write', key: 'test', value: new Uint8Array() });
// Latency penalty increases with age
```

### 8. Coherent Swarm Intelligence

Swarms where local actions are allowed but global incoherence is forbidden.

```typescript
import { CoherentSwarm } from '@ruvector/delta-behavior';

const swarm = new CoherentSwarm(0.6); // Min coherence threshold

swarm.addAgent('a1', [0, 0]);
swarm.addAgent('a2', [1, 0]);
swarm.addAgent('a3', [0, 1]);

// Divergent actions are rejected or modified
const result = swarm.executeAction('a1', { type: 'move', dx: 80, dy: 80 });
// result.type === 'rejected' - would break swarm coherence

// Emergent intelligence that cannot emerge pathological behaviors
```

### 9. Graceful Shutdown

Systems that actively move toward safe termination when unstable.

```typescript
import { GracefulSystem } from '@ruvector/delta-behavior';

const system = new GracefulSystem();
system.addResource('database', 10);
system.addShutdownHook({
  name: 'FlushBuffers',
  priority: 10,
  execute: async () => { /* cleanup */ },
});

// As coherence degrades, system moves toward shutdown
system.applyCoherenceChange(-0.5);
console.log(system.getState()); // 'degraded' | 'shuttingDown'

// Shutdown is an attractor, not a failure
await system.progressShutdown();
```

### 10. Pre-AGI Containment

Bounded intelligence growth with coherence-enforced safety.

```typescript
import { ContainmentSubstrate } from '@ruvector/delta-behavior';

const substrate = new ContainmentSubstrate();

// Capabilities have ceilings
// self-modification is highly restricted (ceiling: 3)
const result = substrate.attemptGrowth('selfModification', 1.0);
// result.type may be 'dampened' - growth reduced to preserve coherence

// Recursive self-improvement is bounded
for (let i = 0; i < 100; i++) {
  substrate.attemptGrowth('reasoning', 0.3);
  substrate.attemptGrowth('selfModification', 0.5);
  substrate.rest(); // Recover coherence
}

// Intelligence grows but remains bounded
console.log(substrate.getCapability('selfModification')); // <= 3.0
```

## Core API

### DeltaBehavior

The core class for coherence-preserving state transitions.

```typescript
const delta = new DeltaBehavior(config);

// Check if a transition is allowed
delta.checkTransition(currentCoherence, predictedCoherence);
// => { type: 'allowed' | 'throttled' | 'blocked' | 'energyExhausted' }

// Find attractors in state trajectory
delta.findAttractors(trajectory);

// Calculate guidance force toward attractor
delta.calculateGuidance(currentState, attractor);
```

### Configuration

```typescript
const config: DeltaConfig = {
  bounds: {
    minCoherence: 0.3,        // Hard floor
    throttleThreshold: 0.5,   // Slow down below this
    targetCoherence: 0.8,     // Optimal level
    maxDeltaDrop: 0.1,        // Max drop per transition
  },
  energy: {
    baseCost: 1.0,
    instabilityExponent: 2.0,
    maxCost: 100.0,
    budgetPerTick: 10.0,
  },
  scheduling: {
    priorityThresholds: [0.0, 0.3, 0.5, 0.7, 0.9],
    rateLimits: [100, 50, 20, 10, 5],
  },
  gating: {
    minWriteCoherence: 0.3,
    minPostWriteCoherence: 0.25,
    recoveryMargin: 0.2,
  },
  guidanceStrength: 0.5,
};
```

## WASM Support

For maximum performance, you can load the WASM module:

```typescript
import { init } from '@ruvector/delta-behavior';

// Browser
await init({ wasmPath: '/delta_behavior_bg.wasm' });

// Node.js
await init({ wasmPath: './node_modules/@ruvector/delta-behavior/delta_behavior_bg.wasm' });

// Or with pre-loaded bytes
const wasmBytes = await fetch('/delta_behavior_bg.wasm').then(r => r.arrayBuffer());
await init({ wasmBytes: new Uint8Array(wasmBytes) });
```

The SDK works without WASM using a JavaScript fallback.

## Examples

### Node.js

```bash
npx tsx examples/node-example.ts
```

### Browser

Open `examples/browser-example.html` in a browser.

## TypeScript Support

Full TypeScript types are included. Import types as needed:

```typescript
import type {
  Coherence,
  DeltaConfig,
  TransitionResult,
  GrowthResult,
  CapabilityDomain,
} from '@ruvector/delta-behavior';
```

## Key Concepts

### Coherence

A value from 0.0 to 1.0 representing system stability and internal consistency. Higher coherence means the system is more stable and can perform more operations.

### Attractors

Stable states in phase space that the system naturally moves toward. Used for guidance and to detect stable operating regimes.

### Energy Budget

Operations cost energy based on how destabilizing they are. When energy is exhausted, operations are blocked until the budget regenerates.

### Collapse Functions

Functions that determine how capabilities scale with coherence:

- **Linear**: Capability = coherence * maxCapability
- **Quadratic**: Capability = coherence^2 * maxCapability
- **Sigmoid**: Smooth transition at a midpoint
- **Step**: Binary on/off at threshold

## Safety Properties

The delta-behavior framework provides several safety guarantees:

1. **Bounded Growth**: Intelligence/capability cannot exceed defined ceilings
2. **Graceful Degradation**: Systems become simpler (not chaotic) under stress
3. **Self-Limiting**: Operations that would destabilize are automatically blocked
4. **Shutdown Attractors**: Unstable systems naturally move toward safe termination
5. **Coherence Preservation**: All transitions must maintain minimum coherence

## Research Paper

For the full mathematical framework, see:
"Delta-Behavior: A Mathematical Framework for Coherence-Preserving State Transitions"

## License

MIT

## Contributing

Contributions are welcome. Please ensure all changes maintain the safety properties described above.
