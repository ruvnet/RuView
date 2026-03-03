# Lean Agentic Learning System - TypeScript/JavaScript Client

A revolutionary learning framework combining formal reasoning, agentic AI, and stream learning for real-time adaptation.

## Features

- 🎯 **Formal Reasoning** - Lean-style theorem proving for verified knowledge
- 🤖 **Agentic AI** - Autonomous decision-making with Plan-Act-Observe-Learn loops
- 📊 **Stream Learning** - Real-time online adaptation from data streams
- 🧠 **Knowledge Graph** - Dynamic knowledge representation and evolution
- ⚡ **Real-Time Processing** - Low-latency stream processing
- 🔒 **Type Safety** - Full TypeScript support with comprehensive types

## Installation

```bash
npm install @midstream/lean-agentic
```

## Quick Start

```typescript
import { LeanAgenticClient, StreamProcessor } from '@midstream/lean-agentic';

// Initialize client
const client = new LeanAgenticClient('http://localhost:8080', {
  enableFormalVerification: true,
  learningRate: 0.01,
  maxPlanningDepth: 5,
});

// Create stream processor
const processor = new StreamProcessor(client, 'session_001');

// Process stream chunks
const chunk = {
  content: 'Hello, I need weather information',
  timestamp: Date.now(),
};

const result = await processor.processChunk(chunk);

console.log('Action:', result.action.description);
console.log('Reward:', result.reward);
console.log('Verified:', result.verified);
```

## Core Concepts

### 1. Agentic Loop (Plan-Act-Observe-Learn)

```typescript
import { AgenticLoop } from '@midstream/lean-agentic';

const loop = new AgenticLoop();
const context = client.createContext('my_session');

// Plan
const plan = await loop.plan(context, 'Get weather for Tokyo');

// The system autonomously:
// - Generates action candidates
// - Ranks by expected reward
// - Executes highest-value action
// - Observes results
// - Learns from experience
```

### 2. Knowledge Graph

```typescript
import { KnowledgeGraph, EntityType } from '@midstream/lean-agentic';

const kg = new KnowledgeGraph();

// Extract entities from text
const entities = kg.extractEntities('Alice works at Google in California');

// Update knowledge graph
kg.update(entities);

// Query entities
const people = kg.queryEntities(EntityType.Person);
const orgs = kg.queryEntities(EntityType.Organization);

// Find related entities
const related = kg.findRelated('alice_entity_id', 2);
```

### 3. Stream Processing

```typescript
import { StreamProcessor } from '@midstream/lean-agentic';

const processor = new StreamProcessor(client, 'session_id');

// Listen to events
processor.on('chunk_processed', ({ chunk, result }) => {
  console.log(`Processed: ${chunk.content}`);
  console.log(`Reward: ${result.reward}`);
});

processor.on('high_reward', (result) => {
  console.log('High reward action detected!', result);
});

// Process stream
const chunks = [
  { content: 'chunk 1', timestamp: Date.now() },
  { content: 'chunk 2', timestamp: Date.now() },
];

const results = await processor.processStream(chunks);
```

### 4. Batched Processing

```typescript
import { BatchedStreamProcessor } from '@midstream/lean-agentic';

// Process in batches of 10
const batchProcessor = new BatchedStreamProcessor(
  client,
  'session_id',
  10
);

batchProcessor.on('batch_processed', ({ result }) => {
  console.log('Batch processed with reward:', result.reward);
});
```

## Advanced Usage

### Configuration

```typescript
const client = new LeanAgenticClient('http://localhost:8080', {
  // Enable formal verification of all actions
  enableFormalVerification: true,

  // Learning rate for online adaptation (0.0 - 1.0)
  learningRate: 0.01,

  // Maximum depth for action planning
  maxPlanningDepth: 5,

  // Confidence threshold for executing actions
  actionThreshold: 0.7,

  // Enable multi-agent collaboration
  enableMultiAgent: true,

  // Knowledge graph update frequency
  kgUpdateFreq: 100,
});
```

### Context Management

```typescript
const context = client.createContext('session_001');

// Add to history
context.history.push('Previous message');

// Set preferences
context.preferences['preferred_language'] = 0.9;
context.preferences['detail_level'] = 0.5;

// Update environment
context.environment['user_location'] = 'Tokyo';
context.environment['time_of_day'] = 'morning';
```

### Querying System State

```typescript
// Get system statistics
const stats = await client.getStats();
console.log(`Entities: ${stats.totalEntities}`);
console.log(`Theorems: ${stats.totalTheorems}`);
console.log(`Actions: ${stats.totalActions}`);
console.log(`Avg Reward: ${stats.averageReward}`);

// Get learning statistics
const learningStats = await client.getLearningStats();
console.log(`Iterations: ${learningStats.iterations}`);
console.log(`Parameters: ${learningStats.modelParameters}`);

// Query knowledge graph
const entities = await client.queryEntities({
  entityType: 'Person',
  searchText: 'Alice',
  limit: 10,
});

// Get theorems
const theorems = await client.getTheorems(['safety', 'verified']);
```

## Examples

### Real-time Chat Assistant

```typescript
import { LeanAgenticClient, StreamProcessor } from '@midstream/lean-agentic';

async function chatAssistant() {
  const client = new LeanAgenticClient('http://localhost:8080');
  const processor = new StreamProcessor(client, 'chat_session');

  // Track high-value interactions
  processor.on('high_reward', (result) => {
    console.log('✨ Learned something valuable!');
  });

  // Process user messages
  const messages = [
    'What is the weather like?',
    'Remember I prefer detailed forecasts',
    'How about tomorrow?',
  ];

  for (const msg of messages) {
    const result = await processor.processChunk({
      content: msg,
      timestamp: Date.now(),
    });

    console.log(`User: ${msg}`);
    console.log(`Action: ${result.action.description}`);
    console.log(`Verified: ${result.verified ? '✓' : '✗'}`);
    console.log('---');
  }

  // Get final statistics
  const stats = await client.getStats();
  console.log('Session Stats:', stats);
}
```

### Learning from Feedback

```typescript
async function learningExample() {
  const client = new LeanAgenticClient('http://localhost:8080', {
    learningRate: 0.05, // Higher learning rate
  });

  const context = client.createContext('learning_session');

  // Process with feedback loop
  for (let i = 0; i < 100; i++) {
    const result = await client.processChunk(
      `Training example ${i}`,
      context
    );

    // System automatically learns from rewards
    // Higher rewards reinforce action selection
  }

  const stats = await client.getLearningStats();
  console.log(`Learned from ${stats.iterations} iterations`);
  console.log(`Average reward: ${stats.averageReward}`);
}
```

## API Reference

See [API Documentation](./docs/API.md) for complete reference.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Lean Agentic Learning System               │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────┐      ┌──────────────┐               │
│  │   Formal     │      │   Agentic    │               │
│  │  Reasoning   │◄────►│    Loop      │               │
│  │   Engine     │      │  (P-A-O-L)   │               │
│  └──────┬───────┘      └──────┬───────┘               │
│         │                     │                        │
│         │    ┌────────────────▼─────┐                 │
│         └───►│  Knowledge Graph &   │                 │
│              │   Theorem Store      │                 │
│              └────────────┬─────────┘                 │
│                           │                            │
│              ┌────────────▼─────────┐                 │
│              │  Stream Learning &   │                 │
│              │  Online Adaptation   │                 │
│              └──────────────────────┘                 │
└─────────────────────────────────────────────────────────┘
```

## License

MIT

## Contributing

Contributions welcome! See [CONTRIBUTING.md](./CONTRIBUTING.md)

## Support

- Issues: https://github.com/ruvnet/midstream/issues
- Discussions: https://github.com/ruvnet/midstream/discussions
