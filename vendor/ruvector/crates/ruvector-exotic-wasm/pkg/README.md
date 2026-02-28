# @ruvector/exotic-wasm - Exotic AI: NAO, Morphogenetic Networks, Time Crystals

[![npm version](https://img.shields.io/npm/v/ruvector-exotic-wasm.svg)](https://www.npmjs.com/package/ruvector-exotic-wasm)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/ruvnet/ruvector)
[![Bundle Size](https://img.shields.io/badge/bundle%20size-146KB%20gzip-green.svg)](https://www.npmjs.com/package/ruvector-exotic-wasm)
[![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?logo=webassembly&logoColor=white)](https://webassembly.org/)

**Exotic AI mechanisms** for emergent behavior in distributed systems. Implements novel coordination primitives inspired by decentralized governance (DAOs), developmental biology, and quantum physics.

## Key Features

- **Neural Autonomous Organization (NAO)**: Decentralized governance for AI agent collectives with quadratic voting
- **Morphogenetic Networks**: Bio-inspired network growth with cellular differentiation and synaptic pruning
- **Time Crystal Coordinator**: Robust distributed coordination using discrete time crystal dynamics
- **Exotic Ecosystem**: Interconnected simulation of all three mechanisms
- **WASM-Optimized**: Runs in browsers and edge environments

## Installation

```bash
npm install ruvector-exotic-wasm
# or
yarn add ruvector-exotic-wasm
# or
pnpm add ruvector-exotic-wasm
```

## Neural Autonomous Organization (NAO)

Decentralized governance for AI agent collectives with stake-weighted quadratic voting, oscillatory synchronization, and quorum-based consensus.

### Concept

Unlike traditional DAOs that govern humans, NAOs coordinate AI agents through:
- **Quadratic Voting**: Square root of stake as voting power (prevents plutocracy)
- **Oscillatory Synchronization**: Agents synchronize phases for coherent decision-making
- **Emergent Consensus**: Proposals pass when collective coherence exceeds quorum

```typescript
import init, { WasmNAO } from 'ruvector-exotic-wasm';

await init();

// Create NAO with 70% quorum threshold
const nao = new WasmNAO(0.7);

// Add agent members with stake
nao.addMember("agent_alpha", 100);
nao.addMember("agent_beta", 50);
nao.addMember("agent_gamma", 75);

// Create a proposal
const proposalId = nao.propose("Upgrade memory backend to vector store");

// Agents vote with conviction weights (0.0-1.0)
nao.vote(proposalId, "agent_alpha", 0.9);  // Strong support
nao.vote(proposalId, "agent_beta", 0.6);   // Moderate support
nao.vote(proposalId, "agent_gamma", 0.8);  // Support

// Advance simulation
for (let i = 0; i < 100; i++) {
  nao.tick(0.001);  // dt = 1ms
}

// Check synchronization
console.log(`Synchronization: ${(nao.synchronization() * 100).toFixed(1)}%`);

// Execute if quorum reached
if (nao.execute(proposalId)) {
  console.log("Proposal executed!");
}

// Check agent coherence
const coherence = nao.agentCoherence("agent_alpha", "agent_beta");
console.log(`Alpha-Beta coherence: ${coherence.toFixed(2)}`);

// Export state as JSON
const state = nao.toJson();
```

## Morphogenetic Networks

Bio-inspired network growth using morphogen gradients for cellular differentiation, emergent topology, and synaptic pruning - modeled after developmental biology.

### Concept

Cells in the network:
- **Stem Cells**: Undifferentiated, can become any type
- **Signaling Cells**: Produce morphogen gradients that guide differentiation
- **Compute Cells**: Specialized for processing tasks

```typescript
import { WasmMorphogeneticNetwork } from 'ruvector-exotic-wasm';

// Create 100x100 grid network
const network = new WasmMorphogeneticNetwork(100, 100);

// Seed initial cells
network.seedStem(50, 50);       // Central stem cell
network.seedSignaling(25, 25); // Growth signal source
network.seedSignaling(75, 75); // Another signal source

// Add growth factor sources (morphogen gradients)
network.addGrowthSource(50, 50, "differentiation", 1.0);

// Simulate growth
for (let step = 0; step < 1000; step++) {
  network.grow(0.1);  // Growth rate

  if (step % 10 === 0) {
    network.differentiate();  // Stem -> specialized cells
  }
}

// Optimize network through pruning
network.prune(0.1);  // Remove weak connections

// Get statistics
console.log(`Total cells: ${network.cellCount()}`);
console.log(`Stem cells: ${network.stemCount()}`);
console.log(`Compute cells: ${network.computeCount()}`);
console.log(`Signaling cells: ${network.signalingCount()}`);

// Get detailed stats as JSON
const stats = network.statsJson();
console.log(stats);
```

## Time Crystal Coordinator

Robust distributed coordination using discrete time crystal dynamics with period-doubled oscillations for stable, noise-resilient agent synchronization.

### Concept

Time crystals exhibit:
- **Period Doubling**: System oscillates at half the driving frequency
- **Floquet Engineering**: Noise-resilient through topological protection
- **Phase Locking**: Agents synchronize into stable coordination patterns

```typescript
import { WasmTimeCrystal } from 'ruvector-exotic-wasm';

// Create time crystal with 10 oscillators, 100ms period
const crystal = new WasmTimeCrystal(10, 100);

// Establish crystalline order
crystal.crystallize();

// Configure dynamics
crystal.setDriving(0.8);    // Driving strength
crystal.setCoupling(0.5);   // Inter-oscillator coupling
crystal.setDisorder(0.1);   // Disorder level (noise resilience)

// Run simulation
for (let t = 0; t < 200; t++) {
  const pattern = crystal.tick();  // Returns Uint8Array coordination pattern

  // Use pattern bits for coordination
  // Each bit indicates whether an agent should be active
}

// Check order parameter (synchronization level)
console.log(`Order parameter: ${crystal.orderParameter().toFixed(2)}`);
console.log(`Crystallized: ${crystal.isCrystallized()}`);
console.log(`Pattern type: ${crystal.patternType()}`);
console.log(`Robustness: ${crystal.robustness().toFixed(2)}`);

// Get collective spin (net magnetization)
console.log(`Collective spin: ${crystal.collectiveSpin()}`);

// Test perturbation resilience
crystal.perturb(0.3);  // 30% strength perturbation
// Crystal should recover due to topological protection
```

### Pre-synchronized Crystal

```typescript
// Create already-synchronized crystal
const syncedCrystal = WasmTimeCrystal.synchronized(8, 50);
console.log(`Initial order: ${syncedCrystal.orderParameter()}`);  // ~1.0
```

## Exotic Ecosystem

Interconnected simulation of all three mechanisms working together:

```typescript
import { ExoticEcosystem } from 'ruvector-exotic-wasm';

// Create ecosystem: 5 agents, 50x50 grid, 8 oscillators
const ecosystem = new ExoticEcosystem(5, 50, 8);

// Crystallize for stable coordination
ecosystem.crystallize();

// Run simulation
for (let step = 0; step < 500; step++) {
  ecosystem.step();
}

// Check integrated state
console.log(`Step: ${ecosystem.currentStep()}`);
console.log(`Synchronization: ${ecosystem.synchronization().toFixed(2)}`);
console.log(`NAO members: ${ecosystem.memberCount()}`);
console.log(`Network cells: ${ecosystem.cellCount()}`);

// Create and execute proposals in the ecosystem
const propId = ecosystem.propose("Scale compute capacity");
ecosystem.vote(propId, "agent_0", 1.0);
ecosystem.vote(propId, "agent_1", 0.8);
ecosystem.vote(propId, "agent_2", 0.9);

if (ecosystem.execute(propId)) {
  console.log("Ecosystem proposal executed!");
}

// Get full summary as JSON
const summary = ecosystem.summaryJson();
console.log(JSON.stringify(summary, null, 2));
```

## API Reference

### WasmNAO

| Method | Description |
|--------|-------------|
| `new(quorum_threshold)` | Create NAO (0.0-1.0 quorum) |
| `addMember(agent_id, stake)` | Add voting member |
| `removeMember(agent_id)` | Remove member |
| `propose(action)` | Create proposal, returns ID |
| `vote(proposal_id, agent_id, weight)` | Vote with conviction |
| `execute(proposal_id)` | Execute if quorum met |
| `tick(dt)` | Advance simulation |
| `synchronization()` | Get sync level (0.0-1.0) |
| `agentCoherence(a, b)` | Coherence between agents |
| `toJson()` | Export full state |

### WasmMorphogeneticNetwork

| Method | Description |
|--------|-------------|
| `new(width, height)` | Create grid network |
| `seedStem(x, y)` | Add stem cell |
| `seedSignaling(x, y)` | Add signaling cell |
| `addGrowthSource(x, y, name, concentration)` | Add morphogen source |
| `grow(dt)` | Simulate growth |
| `differentiate()` | Trigger differentiation |
| `prune(threshold)` | Remove weak connections |
| `cellCount()` / `stemCount()` / `computeCount()` | Get cell counts |
| `statsJson()` / `cellsJson()` | Export as JSON |

### WasmTimeCrystal

| Method | Description |
|--------|-------------|
| `new(n, period_ms)` | Create with n oscillators |
| `synchronized(n, period_ms)` | Create pre-synchronized (static) |
| `crystallize()` | Establish periodic order |
| `tick()` | Advance, returns pattern |
| `orderParameter()` | Sync level (0.0-1.0) |
| `isCrystallized()` | Check crystal state |
| `patternType()` | Current pattern name |
| `perturb(strength)` | Apply perturbation |
| `setDriving(strength)` / `setCoupling(coupling)` / `setDisorder(disorder)` | Configure dynamics |

## Use Cases

- **Multi-Agent Coordination**: Decentralized decision-making for AI swarms
- **Autonomous AI Governance**: Self-organizing agent collectives
- **Emergent Network Design**: Bio-inspired architecture evolution
- **Distributed Consensus**: Noise-resilient coordination patterns
- **Swarm Intelligence**: Collective behavior through synchronization
- **Self-Healing Systems**: Networks that grow and repair autonomously

## Bundle Size

- **WASM binary**: ~146KB (uncompressed)
- **Gzip compressed**: ~55KB
- **JavaScript glue**: ~7KB

## Related Packages

- [ruvector-economy-wasm](https://www.npmjs.com/package/ruvector-economy-wasm) - CRDT credit economy
- [ruvector-nervous-system-wasm](https://www.npmjs.com/package/ruvector-nervous-system-wasm) - Bio-inspired neural
- [ruvector-learning-wasm](https://www.npmjs.com/package/ruvector-learning-wasm) - MicroLoRA adaptation

## License

MIT

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [Full Documentation](https://ruv.io)
- [Bug Reports](https://github.com/ruvnet/ruvector/issues)

---

**Keywords**: DAO, AI governance, emergent behavior, distributed AI, NAO, Neural Autonomous Organization, morphogenetic, developmental biology, time crystal, quantum physics, swarm intelligence, multi-agent systems, WebAssembly, WASM, coordination, consensus, oscillatory, synchronization
