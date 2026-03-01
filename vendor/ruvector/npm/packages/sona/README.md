# @ruvector/sona

**Self-Optimizing Neural Architecture (SONA)** - Node.js bindings for adaptive learning with ReasoningBank.

SONA is a cutting-edge adaptive learning system that combines:
- **Micro-LoRA** (rank 1-2): Ultra-fast inference-time adaptation
- **Base LoRA** (rank 8+): Deeper background learning
- **EWC++**: Catastrophic forgetting prevention
- **ReasoningBank**: Pattern extraction and storage
- **Dual Learning Loops**: Instant (<1ms) and background (periodic) learning

## Features

- 🚀 **Instant Adaptation**: Sub-millisecond learning updates during inference
- 🧠 **Pattern Recognition**: Automatic extraction and clustering of learned patterns
- 🔄 **Dual Learning Loops**: Balance speed and depth with instant and background learning
- 💾 **Memory Preservation**: EWC++ prevents catastrophic forgetting
- ⚡ **High Performance**: Native Rust implementation with SIMD optimizations
- 🎯 **Production Ready**: Used in large-scale LLM deployments

## Installation

```bash
npm install @ruvector/sona
```

## Quick Start

```typescript
import { SonaEngine } from '@ruvector/sona';

// Create engine with hidden dimension
const engine = new SonaEngine(512);

// Or with custom configuration
const engine = SonaEngine.withConfig({
  hiddenDim: 512,
  microLoraRank: 2,
  baseLoraRank: 16,
  microLoraLr: 0.002,
  qualityThreshold: 0.7,
});

// Start a trajectory
const builder = engine.beginTrajectory(queryEmbedding);

// Record inference steps
builder.addStep(activations, attentionWeights, 0.8);
builder.addStep(activations2, attentionWeights2, 0.9);

// Complete trajectory
engine.endTrajectory(builder, 0.85); // quality score

// Apply learned transformations
const output = engine.applyMicroLora(input);

// Force learning cycle
const result = engine.forceLearn();
console.log(result);

// Find similar patterns
const patterns = engine.findPatterns(queryEmbedding, 5);
patterns.forEach(p => {
  console.log(`Pattern ${p.id}: quality=${p.avgQuality}, size=${p.clusterSize}`);
});
```

## API Reference

### SonaEngine

Main class for adaptive learning.

#### Constructor

```typescript
new SonaEngine(hiddenDim: number)
```

Create a new SONA engine with default configuration.

**Parameters:**
- `hiddenDim`: Hidden dimension size (e.g., 256, 512, 1024)

#### Static Methods

##### `SonaEngine.withConfig(config: SonaConfig): SonaEngine`

Create engine with custom configuration.

**Configuration Options:**
```typescript
interface SonaConfig {
  hiddenDim: number;              // Required: Hidden dimension
  embeddingDim?: number;          // Default: hiddenDim
  microLoraRank?: number;         // Default: 1 (range: 1-2)
  baseLoraRank?: number;          // Default: 8
  microLoraLr?: number;           // Default: 0.001
  baseLoraLr?: number;            // Default: 0.0001
  ewcLambda?: number;             // Default: 1000.0
  patternClusters?: number;       // Default: 50
  trajectoryCapacity?: number;    // Default: 10000
  backgroundIntervalMs?: number;  // Default: 3600000 (1 hour)
  qualityThreshold?: number;      // Default: 0.5
  enableSimd?: boolean;           // Default: true
}
```

#### Instance Methods

##### `beginTrajectory(queryEmbedding: Float64Array | number[]): TrajectoryBuilder`

Start recording a new inference trajectory.

##### `endTrajectory(builder: TrajectoryBuilder, quality: number): void`

Complete and submit trajectory for learning.

**Parameters:**
- `builder`: TrajectoryBuilder instance
- `quality`: Final quality score [0.0, 1.0]

##### `applyMicroLora(input: Float64Array | number[]): Float64Array`

Apply micro-LoRA transformation (instant learning).

##### `applyBaseLora(layerIdx: number, input: Float64Array | number[]): Float64Array`

Apply base-LoRA transformation to specific layer.

##### `tick(): string | null`

Run background learning cycle if due. Returns status message if executed.

##### `forceLearn(): string`

Force immediate background learning cycle.

##### `flush(): void`

Flush instant loop updates.

##### `findPatterns(queryEmbedding: Float64Array | number[], k: number): LearnedPattern[]`

Find k most similar learned patterns.

##### `getStats(): string`

Get engine statistics as JSON string.

##### `setEnabled(enabled: boolean): void`

Enable or disable learning.

##### `isEnabled(): boolean`

Check if engine is enabled.

### TrajectoryBuilder

Builder for recording inference trajectories.

#### Methods

##### `addStep(activations: Float64Array | number[], attentionWeights: Float64Array | number[], reward: number): void`

Add a step to the trajectory.

**Parameters:**
- `activations`: Layer activations
- `attentionWeights`: Attention weights
- `reward`: Reward signal for this step

##### `setRoute(route: string): void`

Set model route identifier.

##### `addContext(contextId: string): void`

Add context ID to trajectory.

### LearnedPattern

Represents a learned pattern from trajectory clustering.

```typescript
interface LearnedPattern {
  id: string;
  centroid: Float64Array;
  clusterSize: number;
  totalWeight: number;
  avgQuality: number;
  createdAt: string;
  lastAccessed: string;
  accessCount: number;
  patternType: PatternType;
}
```

### PatternType

Pattern classification enumeration.

```typescript
enum PatternType {
  General = 'General',
  Reasoning = 'Reasoning',
  Factual = 'Factual',
  Creative = 'Creative',
  CodeGen = 'CodeGen',
  Conversational = 'Conversational',
}
```

## Advanced Usage

### LLM Integration Example

```typescript
import { SonaEngine } from '@ruvector/sona';

class AdaptiveLLM {
  private sona: SonaEngine;

  constructor() {
    this.sona = SonaEngine.withConfig({
      hiddenDim: 4096,
      microLoraRank: 2,
      baseLoraRank: 16,
      microLoraLr: 0.002,
      qualityThreshold: 0.7,
      backgroundIntervalMs: 1800000, // 30 minutes
    });
  }

  async generate(prompt: string): Promise<string> {
    const embedding = await this.embed(prompt);
    const builder = this.sona.beginTrajectory(embedding);

    // Generate with SONA-enhanced layers
    const output = await this.runInference(builder);

    // Calculate quality score
    const quality = this.assessQuality(output);

    // Submit trajectory for learning
    this.sona.endTrajectory(builder, quality);

    // Periodic background learning
    const status = this.sona.tick();
    if (status) {
      console.log('Background learning:', status);
    }

    return output;
  }

  private async runInference(builder: TrajectoryBuilder): Promise<string> {
    let output = '';

    for (const layer of this.layers) {
      // Get layer activations
      const activations = layer.forward(/* ... */);
      const attention = layer.getAttention();

      // Apply micro-LoRA enhancement
      const enhanced = this.sona.applyMicroLora(activations);

      // Record step
      const reward = this.calculateReward(enhanced);
      builder.addStep(activations, attention, reward);

      // Continue generation with enhanced activations
      output += this.decode(enhanced);
    }

    return output;
  }
}
```

### Pattern-Based Routing

```typescript
// Find similar patterns for routing decisions
const patterns = engine.findPatterns(queryEmbedding, 3);

if (patterns.length > 0) {
  const topPattern = patterns[0];

  if (topPattern.patternType === 'CodeGen' && topPattern.avgQuality > 0.8) {
    // Route to specialized code generation model
    await routeToCodeModel(query);
  } else if (topPattern.patternType === 'Reasoning') {
    // Use chain-of-thought prompting
    await useCoTPrompting(query);
  }
}
```

### Performance Monitoring

```typescript
// Get statistics
const stats = JSON.parse(engine.getStats());
console.log(`
  Trajectories buffered: ${stats.trajectories_buffered}
  Patterns learned: ${stats.patterns_learned}
  Micro-LoRA updates: ${stats.micro_updates}
  Background cycles: ${stats.background_cycles}
`);

// Force learning when needed
if (stats.trajectories_buffered > 100) {
  const result = engine.forceLearn();
  console.log('Forced learning:', result);
}
```

## Performance Characteristics

- **Micro-LoRA Application**: <1ms per forward pass
- **Trajectory Recording**: ~10μs per step
- **Background Learning**: Depends on buffer size (typically 100-500ms for 1000 trajectories)
- **Pattern Search**: O(k * n) where k = number of results, n = total patterns
- **Memory Usage**: ~50MB base + ~1KB per trajectory + ~10KB per pattern

## Architecture

SONA implements a dual-loop learning architecture:

1. **Instant Loop** (<1ms):
   - Accumulates micro-LoRA gradients during inference
   - Updates on every trajectory
   - Rank-1 or rank-2 LoRA for minimal overhead

2. **Background Loop** (periodic):
   - Extracts patterns via k-means clustering
   - Updates base LoRA weights
   - Applies EWC++ for stability
   - Prunes low-quality patterns

## Requirements

- Node.js >= 16
- Native bindings for your platform (automatically installed)

## Supported Platforms

- Linux (x64, ARM64, ARM)
- macOS (x64, ARM64, Universal)
- Windows (x64, ARM64)
- FreeBSD (x64)

## License

MIT OR Apache-2.0

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [Documentation](https://github.com/ruvnet/ruvector/tree/main/crates/sona)
- [rUvector Project](https://github.com/ruvnet/ruvector)

## Contributing

Contributions are welcome! Please see the main rUvector repository for contribution guidelines.

## Acknowledgments

SONA is part of the rUvector project, building on research in:
- Low-Rank Adaptation (LoRA)
- Elastic Weight Consolidation (EWC)
- Continual Learning
- Neural Architecture Search

---

Built with ❤️ by the rUv Team
