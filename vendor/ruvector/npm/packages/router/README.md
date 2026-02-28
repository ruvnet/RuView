# @ruvector/router

Semantic router for AI agents - vector-based intent matching with HNSW indexing and SIMD acceleration.

## Features

- **Semantic Intent Matching**: Route queries to intents based on meaning, not keywords
- **HNSW Indexing**: Fast approximate nearest neighbor search
- **SIMD Optimized**: Native Rust performance with vectorized operations
- **Quantization**: Memory-efficient storage for large intent sets
- **Multi-Platform**: Works on Linux, macOS, and Windows

## Installation

```bash
npm install @ruvector/router
```

The package automatically installs the correct native binary for your platform.

## Quick Start

```typescript
import { SemanticRouter } from '@ruvector/router';

// Create router
const router = new SemanticRouter({ dimension: 384 });

// Add intents with example utterances
router.addIntent({
  name: 'weather',
  utterances: [
    'What is the weather today?',
    'Will it rain tomorrow?',
    'How hot will it be?'
  ],
  metadata: { handler: 'weather_agent' }
});

router.addIntent({
  name: 'greeting',
  utterances: [
    'Hello',
    'Hi there',
    'Good morning',
    'Hey'
  ],
  metadata: { handler: 'greeting_agent' }
});

router.addIntent({
  name: 'help',
  utterances: [
    'I need help',
    'Can you assist me?',
    'What can you do?'
  ],
  metadata: { handler: 'help_agent' }
});

// Route a query
const results = await router.route('What will the weather be like this weekend?');

console.log(results[0].intent);   // 'weather'
console.log(results[0].score);    // 0.92
console.log(results[0].metadata); // { handler: 'weather_agent' }
```

## API Reference

### `SemanticRouter`

Main class for semantic routing.

#### Constructor

```typescript
new SemanticRouter(config: RouterConfig)
```

**RouterConfig:**
| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `dimension` | number | required | Embedding dimension size |
| `metric` | string | 'cosine' | Distance metric: 'cosine', 'euclidean', 'dot' |
| `m` | number | 16 | HNSW M parameter |
| `efConstruction` | number | 200 | HNSW ef_construction |
| `quantization` | boolean | false | Enable memory-efficient quantization |

#### Methods

##### `addIntent(intent: Intent): void`

Add an intent to the router.

```typescript
router.addIntent({
  name: 'booking',
  utterances: ['Book a flight', 'Reserve a hotel'],
  metadata: { department: 'travel' }
});
```

##### `route(query: string | Float32Array, k?: number): Promise<RouteResult[]>`

Route a query to matching intents.

```typescript
const results = await router.route('I want to book a vacation');
// [{ intent: 'booking', score: 0.89, metadata: {...} }]
```

##### `routeWithEmbedding(embedding: Float32Array, k?: number): RouteResult[]`

Route with a pre-computed embedding (synchronous).

```typescript
const embedding = await getEmbedding('query text');
const results = router.routeWithEmbedding(embedding, 3);
```

##### `removeIntent(name: string): boolean`

Remove an intent from the router.

##### `getIntents(): string[]`

Get all registered intent names.

##### `clear(): void`

Remove all intents.

##### `save(path: string): Promise<void>`

Persist router state to disk.

##### `load(path: string): Promise<void>`

Load router state from disk.

### Types

#### `Intent`

```typescript
interface Intent {
  name: string;                        // Unique intent identifier
  utterances: string[];                // Example utterances
  embedding?: Float32Array | number[]; // Pre-computed embedding
  metadata?: Record<string, unknown>;  // Custom metadata
}
```

#### `RouteResult`

```typescript
interface RouteResult {
  intent: string;                      // Matched intent name
  score: number;                       // Similarity score (0-1)
  metadata?: Record<string, unknown>;  // Intent metadata
}
```

## Use Cases

### Chatbot Intent Detection

```typescript
const router = new SemanticRouter({ dimension: 384 });

// Define intents
const intents = [
  { name: 'faq', utterances: ['What are your hours?', 'How do I contact support?'] },
  { name: 'order', utterances: ['Track my order', 'Where is my package?'] },
  { name: 'return', utterances: ['I want to return this', 'How do I get a refund?'] }
];

intents.forEach(i => router.addIntent(i));

// Handle user message
async function handleMessage(text: string) {
  const [result] = await router.route(text);

  switch(result.intent) {
    case 'faq': return handleFAQ(text);
    case 'order': return handleOrder(text);
    case 'return': return handleReturn(text);
    default: return handleUnknown(text);
  }
}
```

### Multi-Agent Orchestration

```typescript
const agents = {
  'code': new CodeAgent(),
  'research': new ResearchAgent(),
  'creative': new CreativeAgent()
};

const router = new SemanticRouter({ dimension: 768 });

router.addIntent({
  name: 'code',
  utterances: ['Write code', 'Debug this', 'Implement a function'],
  metadata: { agent: 'code' }
});

router.addIntent({
  name: 'research',
  utterances: ['Find information', 'Search for', 'Look up'],
  metadata: { agent: 'research' }
});

// Route task to best agent
async function routeTask(task: string) {
  const [result] = await router.route(task);
  const agent = agents[result.metadata.agent];
  return agent.execute(task);
}
```

## Platform Support

| Platform | Architecture | Package |
|----------|--------------|---------|
| Linux | x64 | `@ruvector/router-linux-x64-gnu` |
| Linux | ARM64 | `@ruvector/router-linux-arm64-gnu` |
| macOS | x64 | `@ruvector/router-darwin-x64` |
| macOS | ARM64 | `@ruvector/router-darwin-arm64` |
| Windows | x64 | `@ruvector/router-win32-x64-msvc` |

## Performance

- **Routing**: < 1ms per query with HNSW
- **Throughput**: 100,000+ routes/second
- **Memory**: ~1KB per intent + embeddings

## Related Packages

- [`@ruvector/core`](https://www.npmjs.com/package/@ruvector/core) - Vector database
- [`@ruvector/tiny-dancer`](https://www.npmjs.com/package/@ruvector/tiny-dancer) - Neural routing
- [`@ruvector/gnn`](https://www.npmjs.com/package/@ruvector/gnn) - Graph Neural Networks

## License

MIT
