# ruvector-exotic-wasm

Exotic AI mechanisms for emergent behavior in distributed systems. This WASM module provides novel coordination primitives inspired by decentralized governance, developmental biology, and quantum physics.

## Installation

```bash
npm install ruvector-exotic-wasm
```

## Quick Start

```javascript
import init, {
  WasmNAO,
  WasmMorphogeneticNetwork,
  WasmTimeCrystal,
  ExoticEcosystem,
  version,
  available_mechanisms
} from 'ruvector-exotic-wasm';

// Initialize the WASM module
await init();

console.log('Version:', version());
console.log('Available mechanisms:', available_mechanisms());
```

## API Reference

### Neural Autonomous Organization (NAO)

Decentralized governance for AI agent collectives using stake-weighted quadratic voting and oscillatory synchronization for coherence.

#### Constructor

```typescript
new WasmNAO(quorum_threshold: number): WasmNAO
```

Creates a new NAO with the specified quorum threshold (0.0 - 1.0).

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `addMember` | `(agent_id: string, stake: number): void` | Add a member agent with initial stake |
| `removeMember` | `(agent_id: string): void` | Remove a member agent |
| `memberCount` | `(): number` | Get the number of members |
| `propose` | `(action: string): string` | Create a proposal, returns proposal ID |
| `vote` | `(proposal_id: string, agent_id: string, weight: number): boolean` | Vote on a proposal (-1.0 to 1.0) |
| `execute` | `(proposal_id: string): boolean` | Execute a proposal if consensus reached |
| `tick` | `(dt: number): void` | Advance simulation by one time step |
| `synchronization` | `(): number` | Get current sync level (0-1) |
| `agentCoherence` | `(agent_a: string, agent_b: string): number` | Get coherence between two agents |
| `activeProposalCount` | `(): number` | Get number of active proposals |
| `totalVotingPower` | `(): number` | Get total voting power in the org |
| `currentTick` | `(): number` | Get current simulation tick |
| `toJson` | `(): any` | Export all data as JSON |
| `free` | `(): void` | Free memory (or use `Symbol.dispose`) |

#### Example

```javascript
import init, { WasmNAO } from 'ruvector-exotic-wasm';

await init();

// Create NAO with 70% quorum requirement
const nao = new WasmNAO(0.7);

// Add agents with stake (voting power = sqrt(stake))
nao.addMember("agent_alpha", 100);  // 10 voting power
nao.addMember("agent_beta", 50);    // ~7.07 voting power
nao.addMember("agent_gamma", 25);   // 5 voting power

// Create a proposal
const proposalId = nao.propose("Upgrade to quantum backend");

// Agents vote (-1.0 = strongly against, 1.0 = strongly for)
nao.vote(proposalId, "agent_alpha", 0.9);
nao.vote(proposalId, "agent_beta", 0.6);
nao.vote(proposalId, "agent_gamma", 0.8);

// Run oscillatory synchronization
for (let i = 0; i < 100; i++) {
  nao.tick(0.001);
}

console.log("Synchronization level:", nao.synchronization());
console.log("Agent coherence:", nao.agentCoherence("agent_alpha", "agent_beta"));

// Execute if consensus reached
if (nao.execute(proposalId)) {
  console.log("Proposal executed!");
}

// Clean up
nao.free();
```

---

### Morphogenetic Network

Biologically-inspired network growth with cellular differentiation through morphogen gradients, emergent network topology, and synaptic pruning.

#### Constructor

```typescript
new WasmMorphogeneticNetwork(width: number, height: number): WasmMorphogeneticNetwork
```

Creates a new morphogenetic network with the specified grid dimensions.

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `seedStem` | `(x: number, y: number): number` | Seed a stem cell, returns cell ID |
| `seedSignaling` | `(x: number, y: number): number` | Seed a signaling cell, returns cell ID |
| `addGrowthSource` | `(x: number, y: number, name: string, concentration: number): void` | Add a growth factor source |
| `grow` | `(dt: number): void` | Grow the network for one time step |
| `differentiate` | `(): void` | Differentiate stem cells based on signals |
| `prune` | `(threshold: number): void` | Remove weak connections and dead cells |
| `cellCount` | `(): number` | Get total cell count |
| `stemCount` | `(): number` | Get stem cell count |
| `computeCount` | `(): number` | Get compute cell count |
| `signalingCount` | `(): number` | Get signaling cell count |
| `currentTick` | `(): number` | Get current simulation tick |
| `statsJson` | `(): any` | Get network statistics as JSON |
| `cellsJson` | `(): any` | Get all cells as JSON |
| `free` | `(): void` | Free memory (or use `Symbol.dispose`) |

#### Cell Types

- **Stem**: Undifferentiated cells that can become any type
- **Signaling**: Produce growth factors (morphogens)
- **Receptor**: Respond to signals from signaling cells
- **Structural**: Form the network backbone
- **Compute**: Perform local computation with internal state

#### Example

```javascript
import init, { WasmMorphogeneticNetwork } from 'ruvector-exotic-wasm';

await init();

// Create a 100x100 grid
const network = new WasmMorphogeneticNetwork(100, 100);

// Seed signaling cells (morphogen sources)
network.seedSignaling(50, 50);
network.seedSignaling(25, 75);
network.seedSignaling(75, 25);

// Seed stem cells that will differentiate
for (let i = 0; i < 20; i++) {
  const x = Math.floor(Math.random() * 100);
  const y = Math.floor(Math.random() * 100);
  network.seedStem(x, y);
}

// Add growth factor sources
network.addGrowthSource(50, 50, "compute", 1.0);

// Run growth simulation
for (let step = 0; step < 500; step++) {
  network.grow(0.1);

  // Differentiate every 10 steps
  if (step % 10 === 0) {
    network.differentiate();
  }

  // Prune every 100 steps
  if (step % 100 === 0) {
    network.prune(0.1);
  }
}

// Get statistics
const stats = network.statsJson();
console.log("Total cells:", stats.total_cells);
console.log("Connections:", stats.total_connections);
console.log("Average fitness:", stats.average_fitness);
console.log("Cell types:", stats.type_counts);

// Get all cell data
const cells = network.cellsJson();
console.log("First cell:", cells[0]);

network.free();
```

---

### Time Crystal Coordinator

Robust distributed coordination using discrete time crystal dynamics with period-doubled oscillations (Floquet engineering) for noise-resilient phase-locked agent synchronization.

#### Constructor

```typescript
new WasmTimeCrystal(n: number, period_ms: number): WasmTimeCrystal
```

Creates a new time crystal with `n` oscillators and the specified period in milliseconds.

#### Static Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `synchronized` | `(n: number, period_ms: number): WasmTimeCrystal` | Create a pre-synchronized crystal |

#### Instance Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `crystallize` | `(): void` | Establish stable periodic order |
| `tick` | `(): Uint8Array` | Advance one step, returns coordination pattern |
| `orderParameter` | `(): number` | Get synchronization level (0-1) |
| `oscillatorCount` | `(): number` | Get number of oscillators |
| `isCrystallized` | `(): boolean` | Check if crystal is in ordered phase |
| `currentStep` | `(): number` | Get current time step |
| `periodMs` | `(): number` | Get period in milliseconds |
| `robustness` | `(): number` | Get robustness measure |
| `collectiveSpin` | `(): number` | Get collective spin (-1 to 1) |
| `patternType` | `(): string` | Get current pattern type |
| `perturb` | `(strength: number): void` | Apply external perturbation |
| `setCoupling` | `(coupling: number): void` | Set oscillator coupling strength |
| `setDriving` | `(strength: number): void` | Set Floquet driving strength |
| `setDisorder` | `(disorder: number): void` | Set noise/disorder level |
| `phasesJson` | `(): any` | Get all phases as JSON array |
| `signalsJson` | `(): any` | Get all signals as JSON array |
| `free` | `(): void` | Free memory (or use `Symbol.dispose`) |

#### Coordination Patterns

- **Coherent**: All oscillators in phase (full coherence)
- **PeriodDoubled**: Time crystal signature (period-doubled oscillation)
- **AntiPhase**: Two-group anti-phase clustering
- **Quasiperiodic**: Complex multi-frequency pattern
- **Disordered**: No stable pattern (thermal/noisy state)

#### Example

```javascript
import init, { WasmTimeCrystal } from 'ruvector-exotic-wasm';

await init();

// Create a 16-oscillator time crystal with 100ms period
const crystal = new WasmTimeCrystal(16, 100);

// Crystallize to establish periodic order
crystal.crystallize();
console.log("Crystallized:", crystal.isCrystallized());

// Configure crystal parameters
crystal.setCoupling(3.0);
crystal.setDriving(Math.PI);  // Pi pulse
crystal.setDisorder(0.05);     // Low noise

// Run coordination loop
for (let i = 0; i < 200; i++) {
  // Get coordination pattern (bit pattern)
  const pattern = crystal.tick();

  // Use pattern for agent coordination
  // Each bit indicates whether oscillator i is in "up" state
  const activeAgents = [];
  for (let j = 0; j < crystal.oscillatorCount(); j++) {
    const byteIdx = Math.floor(j / 8);
    const bitIdx = j % 8;
    if (pattern[byteIdx] & (1 << bitIdx)) {
      activeAgents.push(j);
    }
  }

  if (i % 50 === 0) {
    console.log(`Step ${i}:`, {
      order: crystal.orderParameter().toFixed(3),
      pattern: crystal.patternType(),
      activeAgents: activeAgents.length,
      spin: crystal.collectiveSpin().toFixed(3)
    });
  }
}

// Test perturbation resilience
console.log("Before perturbation:", crystal.orderParameter());
crystal.perturb(0.3);
console.log("After perturbation:", crystal.orderParameter());

// Recovery
for (let i = 0; i < 100; i++) {
  crystal.tick();
}
console.log("After recovery:", crystal.orderParameter());

crystal.free();
```

---

### Exotic Ecosystem

Unified demonstration combining all three mechanisms (NAO, Morphogenetic Network, Time Crystal) working together.

#### Constructor

```typescript
new ExoticEcosystem(agents: number, grid_size: number, oscillators: number): ExoticEcosystem
```

Creates an ecosystem with the specified number of agents, grid size, and oscillators.

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `step` | `(): void` | Advance all systems by one step |
| `crystallize` | `(): void` | Crystallize the time crystal |
| `synchronization` | `(): number` | Get time crystal sync level |
| `cellCount` | `(): number` | Get morphogenetic network cell count |
| `memberCount` | `(): number` | Get NAO member count |
| `currentStep` | `(): number` | Get current simulation step |
| `propose` | `(action: string): string` | Create NAO proposal |
| `vote` | `(proposal_id: string, agent_id: string, weight: number): boolean` | Vote on proposal |
| `execute` | `(proposal_id: string): boolean` | Execute proposal |
| `summaryJson` | `(): any` | Get comprehensive ecosystem summary |
| `free` | `(): void` | Free memory |

#### Example

```javascript
import init, { ExoticEcosystem } from 'ruvector-exotic-wasm';

await init();

// Create ecosystem: 5 agents, 50x50 grid, 8 oscillators
const ecosystem = new ExoticEcosystem(5, 50, 8);

// Crystallize for stable coordination
ecosystem.crystallize();

// Create and vote on proposals
const propId = ecosystem.propose("Initialize swarm protocol");
ecosystem.vote(propId, "agent_0", 1.0);
ecosystem.vote(propId, "agent_1", 0.8);
ecosystem.vote(propId, "agent_2", 0.9);

// Run integrated simulation
for (let i = 0; i < 200; i++) {
  ecosystem.step();

  if (i % 50 === 0) {
    const summary = ecosystem.summaryJson();
    console.log(`Step ${i}:`, {
      sync: summary.crystal.order.toFixed(3),
      cells: summary.network.cells,
      members: summary.nao.members,
      crystallized: summary.crystal.crystallized
    });
  }
}

// Execute proposal after sufficient synchronization
if (ecosystem.execute(propId)) {
  console.log("Proposal executed with ecosystem consensus!");
}

ecosystem.free();
```

---

### Utility Functions

```javascript
import init, { version, available_mechanisms } from 'ruvector-exotic-wasm';

await init();

// Get module version
console.log(version());  // "0.1.29"

// Get list of available mechanisms
console.log(available_mechanisms());
// ["NeuralAutonomousOrg", "MorphogeneticNetwork", "TimeCrystal"]
```

---

## Physics Background

### Time Crystals

This implementation is inspired by discrete time crystals (DTCs) demonstrated in:
- Trapped ion experiments (Monroe group, University of Maryland)
- NV center diamond systems (Lukin group, Harvard)
- Superconducting qubits (Google Quantum AI)

Key insight: Period-doubling (or n-tupling) provides robust coordination signals resilient to perturbations.

### Morphogenesis

Concepts from developmental biology:
- **Morphogens**: Diffusible signaling molecules creating concentration gradients
- **Positional information**: Cells read local concentrations to determine fate
- **Growth factors**: Control cell division and network expansion
- **Apoptosis**: Programmed removal of non-functional components

### Oscillatory Synchronization

Based on Kuramoto model dynamics for neural synchronization:
- Agents modeled as coupled oscillators
- Synchronization emerges from local interactions
- Order parameter measures collective coherence

---

## Use Cases

1. **Decentralized AI Governance**: Use NAO for stake-weighted collective decision-making in multi-agent systems.

2. **Adaptive Network Topology**: Use Morphogenetic Networks for self-organizing distributed system architecture.

3. **Robust Coordination**: Use Time Crystals for noise-resilient scheduling and synchronization in distributed systems.

4. **Emergent Behavior**: Combine all mechanisms for complex adaptive systems with governance, growth, and coordination.

---

## Build from Source

```bash
cd crates/ruvector-exotic-wasm
wasm-pack build --target web --release --out-dir pkg
```

## License

MIT
