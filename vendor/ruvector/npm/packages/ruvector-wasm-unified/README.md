# @ruvector/wasm-unified

Unified TypeScript API surface for RuVector WASM - exposing attention, learning, economy, and exotic computation features through a clean, type-safe interface.

## Features

- **14+ Attention Mechanisms**: Neural (scaled-dot, multi-head, hyperbolic, linear, flash, local-global, MoE, Mamba) and DAG (topological, mincut-gated, hierarchical, spectral, flow, causal, sparse)
- **Adaptive Learning**: Micro-LoRA adaptation, SONA pre-query, BTSP one-shot learning, RL algorithms, meta-learning
- **Nervous System Simulation**: Spiking neural networks, synaptic plasticity, multiple neuron models
- **Compute Credit Economy**: Balance management, staking, rewards, contribution multipliers
- **Exotic Computation**: Quantum-inspired, hyperbolic geometry, topological data analysis, fractal operations

## Installation

```bash
npm install @ruvector/wasm-unified
# or
pnpm add @ruvector/wasm-unified
# or
yarn add @ruvector/wasm-unified
```

## Quick Start

```typescript
import { createUnifiedEngine } from '@ruvector/wasm-unified';

// Create and initialize the unified engine
const engine = await createUnifiedEngine();
await engine.init();

// Use attention mechanisms
const Q = new Float32Array([1, 2, 3, 4]);
const K = new Float32Array([1, 2, 3, 4]);
const V = new Float32Array([1, 2, 3, 4]);
const output = engine.attention.scaledDot(Q, K, V);

// Use learning capabilities
engine.learning.btspOneShotLearn(pattern, 1.0);

// Simulate nervous system
const neuronId = engine.nervous.createNeuron({ neuronType: 'excitatory' });
engine.nervous.step();

// Manage economy
const balance = engine.economy.creditBalance();
engine.economy.stakeDeposit(100);

// Exotic computations
const qstate = engine.exotic.quantumInit(4);
const measured = engine.exotic.quantumMeasure(qstate);

// Cleanup when done
engine.dispose();
```

## Module Usage

### Attention Engine

```typescript
import { createAttentionEngine, listAttentionMechanisms } from '@ruvector/wasm-unified';

const attention = createAttentionEngine();

// List available mechanisms
console.log(listAttentionMechanisms());
// ['scaled-dot', 'multi-head', 'hyperbolic', 'linear', 'flash', ...]

// Scaled dot-product attention
const output = attention.scaledDot(Q, K, V);

// Multi-head attention
const multiHeadOutput = attention.multiHead(query, keys, values, {
  numHeads: 8,
  headDim: 64,
  dropout: 0.1,
});

// Hyperbolic attention (for hierarchical data)
const hyperbolicOutput = attention.hyperbolic(query, keys, values, -1.0);

// Flash attention (memory-efficient)
const flashOutput = attention.flash(query, keys, values, 256);

// Mixture of Experts attention
const moeResult = attention.moe(query, keys, values, 8, 2);
console.log(moeResult.loadBalanceLoss);

// Mamba (state-space model)
const mambaResult = attention.mamba(input, state);
console.log(mambaResult.newState);

// DAG-based attention
const dag = {
  nodes: [
    { id: 'n1', embedding: new Float32Array([1, 2]), nodeType: 'query' },
    { id: 'n2', embedding: new Float32Array([3, 4]), nodeType: 'key' },
  ],
  edges: [{ source: 'n1', target: 'n2', weight: 1.0, edgeType: 'attention' }],
  rootIds: ['n1'],
  leafIds: ['n2'],
};

const dagScores = attention.dagTopological(dag);
const gatedScores = attention.dagMincutGated(dag, {
  gateValues: new Float32Array([0.5, 0.8]),
  threshold: 0.3,
  mode: 'soft',
});
```

### Learning Engine

```typescript
import {
  createLearningEngine,
  createMicroLoraConfig,
  createBtspConfig,
  cosineAnnealingLr,
} from '@ruvector/wasm-unified';

const learning = createLearningEngine({
  defaultLearningRate: 0.001,
  batchSize: 32,
});

// Micro-LoRA adaptation
const loraConfig = createMicroLoraConfig(8, 16, ['attention', 'ffn']);
const adapted = learning.microLoraAdapt(embedding, 'attention', loraConfig);

// SONA pre-query enhancement
const enhanced = learning.sonaPreQuery(dag, 128);
console.log(enhanced.confidence);

// BTSP one-shot learning
const btspConfig = createBtspConfig(0.1, 0.95, 100);
learning.btspOneShotLearn(pattern, rewardSignal, btspConfig);

// Reinforcement learning
const trajectory = {
  states: [state1, state2, state3],
  actions: [0, 1, 0],
  rewards: [0.1, 0.5, 1.0],
  dones: [false, false, true],
};
const policyUpdate = learning.updateFromTrajectory(trajectory, 'ppo');
console.log(policyUpdate.loss, policyUpdate.entropy);

// Compute advantages with GAE
const advantages = learning.computeAdvantages(rewards, values, 0.99, 0.95);

// Experience replay
const batch = learning.experienceReplay(10000, 32);

// Meta-learning with MAML
const adaptedParams = learning.mamlInnerLoop(supportSet, 5, 0.01);

// Learning rate scheduling
const lr = cosineAnnealingLr(step, totalSteps, 0.001, 0.00001);

// Get statistics
const stats = learning.getStats();
console.log(stats.patternsLearned, stats.totalSteps);
```

### Nervous System Engine

```typescript
import {
  createNervousEngine,
  createStdpConfig,
  izhikevichParams,
} from '@ruvector/wasm-unified';

const nervous = createNervousEngine({
  maxNeurons: 10000,
  simulationDt: 0.1,
  enablePlasticity: true,
});

// Create neurons
const excitatory = nervous.createNeuron({
  neuronType: 'excitatory',
  model: 'izhikevich',
  threshold: -55,
});

const inhibitory = nervous.createNeuron({
  neuronType: 'inhibitory',
  model: 'lif',
});

// Create synapses
nervous.createSynapse(excitatory, inhibitory, {
  weight: 0.5,
  delay: 1.0,
  plasticity: { type: 'stdp', params: {} },
});

// Create network topologies
nervous.createReservoir(500, 0.9, 10);  // Echo State Network
nervous.createSmallWorld(100, 4, 0.1);  // Small-world network
nervous.createFeedforward([10, 50, 20, 5], 0.8);  // Feedforward

// Simulate
nervous.injectCurrent(new Map([[excitatory, 10.0]]));
const result = nervous.step(0.1);
console.log('Spikes:', result.spikes);

// Apply plasticity
const stdpConfig = createStdpConfig();
nervous.applyStdp(stdpConfig);
nervous.applyHomeostasis(10);  // Target 10 Hz firing rate

// Record activity
nervous.startRecording([excitatory, inhibitory]);
for (let i = 0; i < 1000; i++) {
  nervous.step();
}
const recording = nervous.stopRecording();
const raster = nervous.getSpikeRaster(0, 100);

// Get topology statistics
const topoStats = nervous.getTopologyStats();
console.log('Neurons:', topoStats.neuronCount);
console.log('Clustering:', topoStats.clusteringCoefficient);
```

### Economy Engine

```typescript
import {
  createEconomyEngine,
  calculateStakingApy,
  formatCredits,
} from '@ruvector/wasm-unified';

const economy = createEconomyEngine({
  initialBalance: 1000,
  stakingEnabled: true,
  rewardRate: 0.05,
});

// Check balance
console.log('Balance:', formatCredits(economy.creditBalance()));
console.log('Multiplier:', economy.contributionMultiplier());

// Staking
if (economy.canAfford(500)) {
  const position = economy.stakeDeposit(500, 86400 * 30);  // 30-day lock
  console.log('Expected reward:', position.expectedReward);
}

// Calculate APY
const apy = calculateStakingApy(0.05, 365);
console.log('APY:', (apy * 100).toFixed(2) + '%');

// Transactions
economy.deposit(100, 'external-source');
const withdrawTx = economy.withdraw(50, 'external-dest');
console.log('Transaction ID:', withdrawTx.id);

// Record contributions
economy.recordContribution('compute', 1000);
economy.recordContribution('validation', 500);

// Claim rewards
const pending = economy.getPendingRewards();
const claimed = economy.claimRewards();
console.log('Claimed:', formatCredits(claimed));

// Operation pricing
const cost = economy.getCost('attention_flash');
console.log('Flash attention cost:', cost);

// Analytics
const analytics = economy.getAnalytics('week');
console.log('Net flow:', formatCredits(analytics.netFlow));
```

### Exotic Engine

```typescript
import {
  createExoticEngine,
  createCircuitBuilder,
  projectToPoincare,
  poincareToLorentz,
} from '@ruvector/wasm-unified';

const exotic = createExoticEngine({
  quantumSimulationDepth: 10,
  hyperbolicPrecision: 1e-10,
  topologicalMaxDimension: 3,
});

// Quantum-inspired computation
const qstate = exotic.quantumInit(4);
let state = exotic.quantumHadamard(qstate, 0);  // Superposition
state = exotic.quantumCnot(state, 0, 1);         // Entanglement
state = exotic.quantumPhase(state, 1, Math.PI / 4);
const measurement = exotic.quantumMeasure(state);
console.log('Measured:', measurement.bitstring);

// Build quantum circuits
const circuit = createCircuitBuilder(3);
circuit.h(0);
circuit.cnot(0, 1);
circuit.ry(2, Math.PI / 3);
const qc = circuit.build();

// VQE for ground state
const vqeResult = exotic.quantumVqe(hamiltonian, qc, 'cobyla');
console.log('Ground state energy:', vqeResult.energy);

// Hyperbolic geometry
const p1 = exotic.hyperbolicPoint(new Float32Array([0.1, 0.2]), 'poincare', -1);
const p2 = exotic.hyperbolicPoint(new Float32Array([0.3, 0.1]), 'poincare', -1);
const distance = exotic.hyperbolicDistance(p1, p2);
console.log('Hyperbolic distance:', distance);

// Mobius operations
const sum = exotic.mobiusAdd(p1, p2);
const centroid = exotic.hyperbolicCentroid([p1, p2]);

// Convert between models
const euclidean = new Float32Array([0.5, 0.3]);
const poincare = projectToPoincare(euclidean);
const lorentz = poincareToLorentz(poincare);

// Topological data analysis
const pointCloud = [
  new Float32Array([0, 0]),
  new Float32Array([1, 0]),
  new Float32Array([0.5, 0.866]),
];
const features = exotic.persistentHomology(pointCloud, 2);
const betti = exotic.bettiNumbers(features, 0.1);
console.log('Betti numbers:', betti);

// Persistence diagram
const diagram = exotic.persistenceDiagram(features);
const bottleneck = exotic.bottleneckDistance(diagram1, diagram2);

// Mapper algorithm
const graph = exotic.mapper(data, undefined, 10, 0.5);
console.log('Mapper nodes:', graph.nodes.length);

// Fractal analysis
const fractalDim = exotic.fractalDimension(data);
const lyapunov = exotic.lyapunovExponents(timeSeries, 3, 1);

// Non-Euclidean neural layers
const hyperbolicOutput = exotic.hyperbolicLayer(inputs, weights, bias);
const sphericalOutput = exotic.sphericalLayer(inputs, weights);
```

## Type Safety

All APIs are fully typed with TypeScript:

```typescript
import type {
  AttentionEngine,
  LearningEngine,
  NervousEngine,
  EconomyEngine,
  ExoticEngine,
  MultiHeadConfig,
  MoEResult,
  QueryDag,
  EnhancedEmbedding,
  Neuron,
  Synapse,
  Transaction,
  QuantumState,
  HyperbolicPoint,
  TopologicalFeature,
} from '@ruvector/wasm-unified';
```

## Benchmarking

```typescript
import { benchmarkAttention, listAttentionMechanisms } from '@ruvector/wasm-unified';

// Benchmark specific mechanism
const results = await benchmarkAttention('flash', 1024, 100);
console.log(`Flash attention: ${results.avgTimeMs}ms avg, ${results.throughputOpsPerSec} ops/sec`);

// Benchmark all mechanisms
for (const mechanism of listAttentionMechanisms()) {
  const result = await benchmarkAttention(mechanism, 512, 50);
  console.log(`${mechanism}: ${result.avgTimeMs.toFixed(3)}ms`);
}
```

## Configuration

```typescript
import { createUnifiedEngine } from '@ruvector/wasm-unified';

const engine = await createUnifiedEngine({
  // Global settings
  wasmPath: '/wasm/ruvector.wasm',
  enableSimd: true,
  enableThreads: true,
  memoryLimit: 1024 * 1024 * 512,  // 512MB
  logLevel: 'info',

  // Module-specific settings
  attention: {
    defaultMechanism: 'flash',
    cacheSize: 1024,
    precisionMode: 'mixed',
  },
  learning: {
    defaultLearningRate: 0.001,
    batchSize: 64,
    enableGradientCheckpointing: true,
  },
  nervous: {
    maxNeurons: 100000,
    simulationDt: 0.05,
    enablePlasticity: true,
  },
  economy: {
    initialBalance: 10000,
    stakingEnabled: true,
    rewardRate: 0.08,
  },
  exotic: {
    quantumSimulationDepth: 20,
    hyperbolicPrecision: 1e-12,
    topologicalMaxDimension: 4,
  },
});
```

## Statistics and Monitoring

```typescript
const engine = await createUnifiedEngine();
await engine.init();

// ... perform operations ...

// Get comprehensive statistics
const stats = engine.getStats();

console.log('Attention ops:', stats.attention.operationCount);
console.log('Learning steps:', stats.learning.stepsCompleted);
console.log('Neurons:', stats.nervous.neuronCount);
console.log('Balance:', stats.economy.balance);
console.log('Quantum ops:', stats.exotic.quantumOps);
console.log('Uptime:', stats.system.uptime, 'ms');
```

## API Reference

See the [TypeScript definitions](./src/index.ts) for complete API documentation.

## License

MIT
