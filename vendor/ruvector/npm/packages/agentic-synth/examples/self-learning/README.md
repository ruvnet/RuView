# Self-Learning System Examples

This directory contains comprehensive examples for generating synthetic data for self-learning AI systems, including reinforcement learning, feedback loops, and continual learning scenarios.

## Overview

These examples demonstrate how to use **agentic-synth** to generate training data for adaptive AI systems that learn continuously from experience and feedback.

## Files

### 1. `reinforcement-learning.ts`

Generates synthetic data for reinforcement learning systems.

**Key Features:**
- State-Action-Reward (SAR) tuples for Q-learning
- Complete episodes with temporal consistency
- Exploration vs exploitation scenarios (multi-armed bandits)
- Reward function testing data
- Policy gradient training data
- Multi-agent RL scenarios

**Examples:**
```typescript
import {
  generateSARTuples,
  generateEpisodes,
  generateExplorationData,
  generatePolicyGradientData,
  generateMultiAgentData
} from './reinforcement-learning.js';

// Generate SAR tuples for Q-learning
const sarData = await generateSARTuples();

// Generate complete episodes
const episodes = await generateEpisodes();

// Generate exploration data
const explorationData = await generateExplorationData();
```

**Use Cases:**
- Training RL agents (DQN, PPO, A3C, SAC)
- Testing reward functions
- Evaluating exploration strategies
- Multi-agent coordination research

---

### 2. `feedback-loop.ts`

Generates data for self-improving systems with feedback mechanisms.

**Key Features:**
- Quality scoring and automatic regeneration
- A/B testing data for model comparison
- Pattern learning from production data
- Adaptive schema evolution
- Active learning sample selection
- Continuous model evaluation

**Examples:**
```typescript
import {
  qualityScoringLoop,
  abTestingData,
  patternLearningLoop,
  adaptiveSchemaEvolution,
  activeLearningData
} from './feedback-loop.js';

// Generate and improve low-quality samples
await qualityScoringLoop();

// Generate A/B test data
const abTests = await abTestingData();

// Learn patterns from production
const syntheticData = await patternLearningLoop();
```

**Use Cases:**
- Model improvement iterations
- Quality assurance pipelines
- Production data simulation
- Active learning systems
- Continuous integration/deployment

---

### 3. `continual-learning.ts`

Generates data for continual learning systems that adapt over time.

**Key Features:**
- Incremental training data (multiple phases)
- Domain adaptation (source → target)
- Catastrophic forgetting prevention (replay buffers)
- Transfer learning datasets (pre-training → fine-tuning)
- Curriculum learning (easy → hard)
- Online learning streams with concept drift

**Examples:**
```typescript
import {
  generateIncrementalData,
  generateDomainAdaptationData,
  generateAntiCatastrophicData,
  generateTransferLearningData,
  generateCurriculumData,
  generateOnlineLearningStream
} from './continual-learning.js';

// Generate incremental training phases
const phases = await generateIncrementalData();

// Generate domain adaptation data
const { source, target, labeledTarget } = await generateDomainAdaptationData();

// Generate anti-forgetting data
const { task1, task2, replay } = await generateAntiCatastrophicData();
```

**Use Cases:**
- Lifelong learning systems
- Domain adaptation research
- Transfer learning pipelines
- Curriculum learning
- Online/streaming learning

---

## Installation

Ensure you have agentic-synth installed:

```bash
npm install agentic-synth
```

## Configuration

Set up your API key:

```bash
# Gemini API (recommended)
export GEMINI_API_KEY=your_api_key_here

# Or OpenRouter
export OPENROUTER_API_KEY=your_api_key_here
```

## Running Examples

### Run Individual Examples

```bash
# Reinforcement learning examples
npx tsx examples/self-learning/reinforcement-learning.ts

# Feedback loop examples
npx tsx examples/self-learning/feedback-loop.ts

# Continual learning examples
npx tsx examples/self-learning/continual-learning.ts
```

### Run Specific Functions

```typescript
import { generateSARTuples } from './reinforcement-learning.js';

// Run specific example
await generateSARTuples();
```

## Common Patterns

### 1. Training Loop Integration

```typescript
import { createSynth } from 'agentic-synth';

const synth = createSynth({
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY,
  cacheStrategy: 'memory', // Cache for faster iterations
});

// Generate training batch
const batch = await synth.generateStructured({
  count: 1000,
  schema: {
    features: ['array of 10 numbers (0-1)'],
    label: 'number (0-4)',
  },
});

// Use in training
for (const sample of batch.data) {
  // Train model with sample.features and sample.label
}
```

### 2. Quality-Based Regeneration

```typescript
// Generate initial data
const data = await synth.generateStructured({ count: 100, schema });

// Filter low-quality samples
const lowQuality = data.data.filter(d => d.quality_score < 0.7);

// Regenerate with improved constraints
const improved = await synth.generateStructured({
  count: lowQuality.length,
  schema: improvedSchema,
  constraints: ['quality_score should be >= 0.7', 'improve coherence'],
});
```

### 3. Incremental Learning Pipeline

```typescript
const phases = [];

// Generate multiple phases
for (let phase = 1; phase <= 5; phase++) {
  const phaseData = await synth.generateStructured({
    count: 200,
    schema: {
      phase: `number (${phase})`,
      features: { /* evolving features */ },
      label: 'number',
    },
    constraints: [`Bias toward new patterns in phase ${phase}`],
  });

  phases.push(phaseData);

  // Train model incrementally
  // model.train(phaseData.data);
}
```

### 4. Experience Replay

```typescript
// Generate new task data
const newTask = await synth.generateStructured({
  count: 200,
  schema: newTaskSchema,
});

// Generate replay buffer from old task
const replay = await synth.generateStructured({
  count: 50, // 25% of new data
  schema: oldTaskSchema,
  constraints: ['High importance samples', 'Diverse and difficult'],
});

// Interleave for training
const mixedBatch = [...newTask.data, ...replay.data];
// model.train(shuffle(mixedBatch));
```

## ML Framework Integration

### TensorFlow.js

```typescript
import * as tf from '@tensorflow/tfjs';

const trainingData = await synth.generateStructured({
  count: 1000,
  schema: {
    features: ['array of 4 numbers (0-1)'],
    label: 'number (0 or 1)',
  },
});

// Convert to tensors
const xs = tf.tensor2d(trainingData.data.map(d => d.features));
const ys = tf.tensor2d(trainingData.data.map(d => [d.label]));

// Train model
await model.fit(xs, ys, { epochs: 100 });
```

### PyTorch (via data export)

```typescript
import { writeFileSync } from 'fs';

const data = await synth.generateStructured({
  count: 10000,
  schema: pytorchSchema,
});

// Export as JSON for PyTorch DataLoader
writeFileSync('training_data.json', JSON.stringify(data.data));
```

```python
# In Python
import json
import torch
from torch.utils.data import Dataset, DataLoader

class SyntheticDataset(Dataset):
    def __init__(self, json_path):
        with open(json_path) as f:
            self.data = json.load(f)

    def __len__(self):
        return len(self.data)

    def __getitem__(self, idx):
        item = self.data[idx]
        return torch.tensor(item['features']), torch.tensor(item['label'])

dataset = SyntheticDataset('training_data.json')
loader = DataLoader(dataset, batch_size=32, shuffle=True)
```

### scikit-learn

```typescript
const data = await synth.generateStructured({
  count: 500,
  schema: {
    feature1: 'number (0-100)',
    feature2: 'number (0-100)',
    feature3: 'number (0-100)',
    label: 'number (0 or 1)',
  },
});

// Export for sklearn
const X = data.data.map(d => [d.feature1, d.feature2, d.feature3]);
const y = data.data.map(d => d.label);

console.log(JSON.stringify({ X, y }));
```

## Advanced Use Cases

### 1. Curriculum Learning

Start with easy examples and gradually increase difficulty:

```typescript
const curriculum = ['easy', 'medium', 'hard', 'expert'];

for (const level of curriculum) {
  const data = await generateCurriculumData(level);
  // Train model on current difficulty level
  await trainModel(data);
}
```

### 2. Domain Adaptation

Adapt from source domain to target domain:

```typescript
// Pre-train on source domain
const sourceData = await generateSourceDomain();
await model.pretrain(sourceData);

// Fine-tune on small labeled target set
const targetData = await generateLabeledTarget(count: 50);
await model.finetune(targetData);
```

### 3. Multi-Task Learning

Generate data for multiple related tasks:

```typescript
const tasks = ['task1', 'task2', 'task3'];
const taskData = {};

for (const task of tasks) {
  taskData[task] = await synth.generateStructured({
    count: 200,
    schema: taskSchemas[task],
  });
}

// Train multi-task model
await multiTaskModel.train(taskData);
```

### 4. Meta-Learning (Learning to Learn)

Generate few-shot learning episodes:

```typescript
const episodes = await synth.generateStructured({
  count: 100,
  schema: {
    support_set: [{ features: [], label: 'number' }],
    query_set: [{ features: [], label: 'number' }],
    task_id: 'UUID',
  },
});

// Meta-train
for (const episode of episodes.data) {
  await metalearner.adapt(episode.support_set);
  const loss = metalearner.evaluate(episode.query_set);
  metalearner.metaUpdate(loss);
}
```

## Performance Tips

1. **Use Caching**: Enable memory or disk caching for repeated generations
   ```typescript
   const synth = createSynth({
     cacheStrategy: 'memory',
     cacheTTL: 3600, // 1 hour
   });
   ```

2. **Batch Generation**: Generate multiple datasets in parallel
   ```typescript
   const batches = await synth.generateBatch('structured', [
     { count: 100, schema: schema1 },
     { count: 100, schema: schema2 },
     { count: 100, schema: schema3 },
   ], 3); // 3 concurrent generations
   ```

3. **Streaming**: Use streaming for large datasets
   ```typescript
   for await (const sample of synth.generateStream('structured', options)) {
     // Process sample immediately
     processAndTrain(sample);
   }
   ```

## Citation

If you use these examples in your research, please cite:

```bibtex
@software{agentic_synth,
  title = {Agentic-Synth: AI-Powered Synthetic Data Generation},
  author = {Your Name},
  year = {2024},
  url = {https://github.com/yourusername/agentic-synth}
}
```

## Contributing

Contributions are welcome! Please feel free to submit pull requests with:
- New self-learning examples
- Improved ML framework integrations
- Performance optimizations
- Bug fixes

## License

MIT License - see LICENSE file for details

## Support

- Documentation: [Main README](../../README.md)
- Issues: [GitHub Issues](https://github.com/yourusername/agentic-synth/issues)
- Discussions: [GitHub Discussions](https://github.com/yourusername/agentic-synth/discussions)

## Related Examples

- [Basic Usage](../basic-usage.ts) - Getting started with agentic-synth
- [Integration Examples](../integration-examples.ts) - Framework integrations
- [Benchmark Example](../benchmark-example.ts) - Performance testing

---

**Happy Learning!** 🚀🤖📈
