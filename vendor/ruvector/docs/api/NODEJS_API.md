# Ruvector Node.js API Reference

Complete API reference for `ruvector` npm package.

## Installation

```bash
npm install ruvector
# or
yarn add ruvector
```

## Table of Contents

1. [VectorDB](#vectordb)
2. [AgenticDB](#agenticdb)
3. [Types](#types)
4. [Advanced Features](#advanced-features)
5. [Error Handling](#error-handling)

## VectorDB

Core vector database class.

### Constructor

```typescript
new VectorDB(options: DbOptions): VectorDB
```

Create a new vector database.

**Parameters**:
```typescript
interface DbOptions {
    dimensions: number;
    storagePath: string;
    distanceMetric?: 'euclidean' | 'cosine' | 'dotProduct' | 'manhattan';
    hnsw?: HnswConfig;
    quantization?: QuantizationConfig;
    mmapVectors?: boolean;
}
```

**Example**:
```javascript
const { VectorDB } = require('ruvector');

const db = new VectorDB({
    dimensions: 128,
    storagePath: './vectors.db',
    distanceMetric: 'cosine'
});
```

### insert

```typescript
async insert(entry: VectorEntry): Promise<string>
```

Insert a single vector.

**Parameters**:
```typescript
interface VectorEntry {
    id?: string;
    vector: Float32Array;
    metadata?: Record<string, any>;
}
```

**Returns**: Promise resolving to vector ID

**Example**:
```javascript
const id = await db.insert({
    vector: new Float32Array(128).fill(0.1),
    metadata: { text: 'Example document' }
});

console.log('Inserted:', id);
```

### insertBatch

```typescript
async insertBatch(entries: VectorEntry[]): Promise<string[]>
```

Insert multiple vectors efficiently.

**Parameters**: Array of vector entries

**Returns**: Promise resolving to array of IDs

**Example**:
```javascript
const entries = Array.from({ length: 1000 }, (_, i) => ({
    id: `vec_${i}`,
    vector: new Float32Array(128).map(() => Math.random()),
    metadata: { index: i }
}));

const ids = await db.insertBatch(entries);
console.log(`Inserted ${ids.length} vectors`);
```

### search

```typescript
async search(query: SearchQuery): Promise<SearchResult[]>
```

Search for similar vectors.

**Parameters**:
```typescript
interface SearchQuery {
    vector: Float32Array;
    k: number;
    filter?: any;
    includeVectors?: boolean;
    includeMetadata?: boolean;
}
```

**Returns**: Promise resolving to search results

**Example**:
```javascript
const results = await db.search({
    vector: new Float32Array(128).fill(0.1),
    k: 10,
    includeMetadata: true
});

results.forEach(result => {
    console.log(`ID: ${result.id}, Distance: ${result.distance}`);
    console.log(`Metadata:`, result.metadata);
});
```

### delete

```typescript
async delete(id: string): Promise<void>
```

Delete a vector by ID.

**Parameters**: Vector ID string

**Returns**: Promise resolving when complete

**Example**:
```javascript
await db.delete('vec_001');
console.log('Deleted vec_001');
```

### update

```typescript
async update(id: string, entry: VectorEntry): Promise<void>
```

Update an existing vector.

**Parameters**:
- `id`: Vector ID to update
- `entry`: New vector data

**Returns**: Promise resolving when complete

**Example**:
```javascript
await db.update('vec_001', {
    vector: new Float32Array(128).fill(0.2),
    metadata: { updated: true }
});
```

### count

```typescript
count(): number
```

Get total number of vectors.

**Returns**: Number of vectors

**Example**:
```javascript
const total = db.count();
console.log(`Total vectors: ${total}`);
```

## AgenticDB

Extended API for AI agents.

### Constructor

```typescript
new AgenticDB(options: DbOptions): AgenticDB
```

Create AgenticDB instance.

**Example**:
```javascript
const { AgenticDB } = require('ruvector');

const db = new AgenticDB({
    dimensions: 128,
    storagePath: './agenticdb.db'
});
```

### Reflexion Memory

#### storeEpisode

```typescript
async storeEpisode(
    task: string,
    actions: string[],
    observations: string[],
    critique: string
): Promise<string>
```

Store self-critique episode.

**Parameters**:
- `task`: Task description
- `actions`: Actions taken
- `observations`: Observations made
- `critique`: Self-generated critique

**Returns**: Episode ID

**Example**:
```javascript
const episodeId = await db.storeEpisode(
    'Solve coding problem',
    ['Read problem', 'Write solution', 'Submit'],
    ['Tests failed', 'Edge case missed'],
    'Should test edge cases before submitting'
);
```

#### retrieveEpisodes

```typescript
async retrieveEpisodes(
    queryEmbedding: Float32Array,
    k: number
): Promise<ReflexionEpisode[]>
```

Retrieve similar past episodes.

**Parameters**:
- `queryEmbedding`: Embedded critique or task
- `k`: Number of episodes

**Returns**: Similar episodes

**Example**:
```javascript
const episodes = await db.retrieveEpisodes(critiqueEmbedding, 5);

episodes.forEach(ep => {
    console.log(`Task: ${ep.task}`);
    console.log(`Critique: ${ep.critique}`);
    console.log(`Actions: ${ep.actions.join(', ')}`);
});
```

### Skill Library

#### createSkill

```typescript
async createSkill(
    name: string,
    description: string,
    parameters: Record<string, string>,
    examples: string[]
): Promise<string>
```

Create a reusable skill.

**Parameters**:
- `name`: Skill name
- `description`: What the skill does
- `parameters`: Required parameters
- `examples`: Usage examples

**Returns**: Skill ID

**Example**:
```javascript
const skillId = await db.createSkill(
    'authenticate_user',
    'Authenticate user with JWT token',
    {
        token: 'string',
        userId: 'string'
    },
    ['authenticate_user(token, userId)']
);
```

#### searchSkills

```typescript
async searchSkills(
    queryEmbedding: Float32Array,
    k: number
): Promise<Skill[]>
```

Search for relevant skills.

**Parameters**:
- `queryEmbedding`: Embedded task description
- `k`: Number of skills

**Returns**: Relevant skills

**Example**:
```javascript
const skills = await db.searchSkills(taskEmbedding, 3);

skills.forEach(skill => {
    console.log(`${skill.name}: ${skill.description}`);
    console.log(`Success rate: ${(skill.successRate * 100).toFixed(1)}%`);
    console.log(`Usage count: ${skill.usageCount}`);
});
```

### Causal Memory

#### addCausalEdge

```typescript
async addCausalEdge(
    causes: string[],
    effects: string[],
    confidence: number,
    context: string
): Promise<string>
```

Add cause-effect relationship.

**Parameters**:
- `causes`: Cause actions/states
- `effects`: Effect actions/states
- `confidence`: Confidence score (0-1)
- `context`: Context description

**Returns**: Edge ID

**Example**:
```javascript
const edgeId = await db.addCausalEdge(
    ['authenticate', 'validate_token'],
    ['access_granted'],
    0.95,
    'User authentication flow'
);
```

#### queryCausal

```typescript
async queryCausal(
    queryEmbedding: Float32Array,
    k: number
): Promise<CausalQueryResult[]>
```

Query causal relationships.

**Parameters**:
- `queryEmbedding`: Embedded context
- `k`: Number of results

**Returns**: Causal edges with utility scores

**Example**:
```javascript
const results = await db.queryCausal(contextEmbedding, 10);

results.forEach(result => {
    console.log(`${result.edge.causes.join(', ')} → ${result.edge.effects.join(', ')}`);
    console.log(`Confidence: ${result.edge.confidence}`);
    console.log(`Utility: ${result.utilityScore.toFixed(4)}`);
});
```

### Learning Sessions

#### createLearningSession

```typescript
async createLearningSession(
    algorithm: string,
    stateDim: number,
    actionDim: number
): Promise<string>
```

Create RL training session.

**Parameters**:
- `algorithm`: RL algorithm (Q-Learning, DQN, PPO, etc.)
- `stateDim`: State dimensionality
- `actionDim`: Action dimensionality

**Returns**: Session ID

**Example**:
```javascript
const sessionId = await db.createLearningSession('PPO', 64, 4);
```

#### addExperience

```typescript
async addExperience(
    sessionId: string,
    state: Float32Array,
    action: Float32Array,
    reward: number,
    nextState: Float32Array,
    done: boolean
): Promise<void>
```

Add experience to session.

**Example**:
```javascript
await db.addExperience(
    sessionId,
    state,
    action,
    1.0,      // reward
    nextState,
    false     // not done
);
```

#### predictWithConfidence

```typescript
async predictWithConfidence(
    sessionId: string,
    state: Float32Array
): Promise<Prediction>
```

Predict action with confidence intervals.

**Returns**:
```typescript
interface Prediction {
    action: Float32Array;
    confidenceLower: number;
    confidenceUpper: number;
    meanConfidence: number;
}
```

**Example**:
```javascript
const prediction = await db.predictWithConfidence(sessionId, state);

console.log('Action:', Array.from(prediction.action));
console.log(`Confidence: [${prediction.confidenceLower.toFixed(2)}, ${prediction.confidenceUpper.toFixed(2)}]`);
```

## Types

### VectorEntry

```typescript
interface VectorEntry {
    id?: string;
    vector: Float32Array;
    metadata?: Record<string, any>;
}
```

### SearchQuery

```typescript
interface SearchQuery {
    vector: Float32Array;
    k: number;
    filter?: any;
    includeVectors?: boolean;
    includeMetadata?: boolean;
}
```

### SearchResult

```typescript
interface SearchResult {
    id: string;
    distance: number;
    vector?: Float32Array;
    metadata?: Record<string, any>;
}
```

### ReflexionEpisode

```typescript
interface ReflexionEpisode {
    id: string;
    task: string;
    actions: string[];
    observations: string[];
    critique: string;
    embedding: Float32Array;
    timestamp: number;
    metadata?: Record<string, any>;
}
```

### Skill

```typescript
interface Skill {
    id: string;
    name: string;
    description: string;
    parameters: Record<string, string>;
    examples: string[];
    embedding: Float32Array;
    usageCount: number;
    successRate: number;
    createdAt: number;
    updatedAt: number;
}
```

### CausalEdge

```typescript
interface CausalEdge {
    id: string;
    causes: string[];
    effects: string[];
    confidence: number;
    context: string;
    embedding: Float32Array;
    observations: number;
    timestamp: number;
}
```

## Configuration

### DbOptions

```typescript
interface DbOptions {
    dimensions: number;
    storagePath: string;
    distanceMetric?: 'euclidean' | 'cosine' | 'dotProduct' | 'manhattan';
    hnsw?: HnswConfig;
    quantization?: QuantizationConfig;
    mmapVectors?: boolean;
}
```

### HnswConfig

```typescript
interface HnswConfig {
    m?: number;              // 16-64, default 32
    efConstruction?: number; // 100-400, default 200
    efSearch?: number;       // 50-500, default 100
    maxElements?: number;    // default 10_000_000
}
```

### QuantizationConfig

```typescript
interface QuantizationConfig {
    type: 'none' | 'scalar' | 'product' | 'binary';
    subspaces?: number;  // For product quantization
    k?: number;          // For product quantization
}
```

## Advanced Features

### HybridSearch

```javascript
const { HybridSearch } = require('ruvector');

const hybrid = new HybridSearch(db, {
    vectorWeight: 0.7,
    bm25Weight: 0.3,
    k1: 1.5,
    b: 0.75
});

const results = await hybrid.search(
    queryVector,
    ['machine', 'learning'],
    10
);
```

### FilteredSearch

```javascript
const { FilteredSearch } = require('ruvector');

const filtered = new FilteredSearch(db, 'preFilter');

const results = await filtered.search(queryVector, 10, {
    and: [
        { field: 'category', op: 'eq', value: 'tech' },
        { field: 'score', op: 'gte', value: 0.8 }
    ]
});
```

### MMRSearch

```javascript
const { MMRSearch } = require('ruvector');

const mmr = new MMRSearch(db, {
    lambda: 0.5,
    diversityWeight: 0.3
});

const results = await mmr.search(queryVector, 20);
```

## Error Handling

All async operations throw errors on failure:

```javascript
try {
    const id = await db.insert(entry);
    console.log('Success:', id);
} catch (error) {
    if (error.message.includes('dimension mismatch')) {
        console.error('Wrong vector dimensions');
    } else {
        console.error('Error:', error.message);
    }
}
```

## TypeScript Support

Full TypeScript type definitions included:

```typescript
import { VectorDB, VectorEntry, SearchResult } from 'ruvector';

const db = new VectorDB({
    dimensions: 128,
    storagePath: './vectors.db'
});

const entry: VectorEntry = {
    vector: new Float32Array(128),
    metadata: { text: 'Example' }
};

const id: string = await db.insert(entry);
const results: SearchResult[] = await db.search({
    vector: new Float32Array(128),
    k: 10
});
```

## Complete Examples

See [examples/nodejs/](../../examples/nodejs/) for complete working examples.
