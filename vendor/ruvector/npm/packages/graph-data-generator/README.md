# @ruvector/graph-data-generator

AI-powered synthetic graph data generation with OpenRouter/Kimi K2 integration for Neo4j knowledge graphs, social networks, and temporal events.

## Features

- **Knowledge Graph Generation**: Create realistic knowledge graphs with entities and relationships
- **Social Network Generation**: Generate social networks with various topology patterns
- **Temporal Events**: Create time-series graph data with events and entities
- **Entity Relationships**: Generate domain-specific entity-relationship graphs
- **Cypher Generation**: Automatic Neo4j Cypher statement generation
- **Vector Embeddings**: Enrich graphs with semantic embeddings
- **OpenRouter Integration**: Powered by Kimi K2 and other OpenRouter models
- **Type-Safe**: Full TypeScript support with Zod validation

## Installation

```bash
npm install @ruvector/graph-data-generator
```

## Quick Start

```typescript
import { createGraphDataGenerator } from '@ruvector/graph-data-generator';

// Initialize with OpenRouter API key
const generator = createGraphDataGenerator({
  apiKey: process.env.OPENROUTER_API_KEY,
  model: 'moonshot/kimi-k2-instruct'
});

// Generate a knowledge graph
const result = await generator.generateKnowledgeGraph({
  domain: 'technology',
  entities: 100,
  relationships: 300,
  includeEmbeddings: true
});

// Get Cypher statements for Neo4j
const cypher = generator.generateCypher(result.data, {
  useConstraints: true,
  useIndexes: true
});

console.log(cypher);
```

## Usage Examples

### Knowledge Graph

```typescript
const knowledgeGraph = await generator.generateKnowledgeGraph({
  domain: 'artificial intelligence',
  entities: 200,
  relationships: 500,
  entityTypes: ['Concept', 'Technology', 'Person', 'Organization'],
  relationshipTypes: ['RELATES_TO', 'DEVELOPED_BY', 'PART_OF'],
  includeEmbeddings: true,
  embeddingDimension: 1536
});
```

### Social Network

```typescript
const socialNetwork = await generator.generateSocialNetwork({
  users: 1000,
  avgConnections: 50,
  networkType: 'small-world', // or 'scale-free', 'clustered', 'random'
  communities: 5,
  includeMetadata: true,
  includeEmbeddings: true
});
```

### Temporal Events

```typescript
const temporalEvents = await generator.generateTemporalEvents({
  startDate: '2024-01-01',
  endDate: '2024-12-31',
  eventTypes: ['login', 'purchase', 'logout', 'error'],
  eventsPerDay: 100,
  entities: 50,
  includeEmbeddings: true
});
```

### Entity Relationships

```typescript
const erGraph = await generator.generateEntityRelationships({
  domain: 'e-commerce',
  entityCount: 500,
  relationshipDensity: 0.3,
  entitySchema: {
    Product: {
      properties: { name: 'string', price: 'number' }
    },
    Category: {
      properties: { name: 'string' }
    }
  },
  relationshipTypes: ['BELONGS_TO', 'SIMILAR_TO', 'PURCHASED_WITH'],
  includeEmbeddings: true
});
```

## Cypher Generation

Generate Neo4j Cypher statements from graph data:

```typescript
// Basic Cypher generation
const cypher = generator.generateCypher(graphData);

// With constraints and indexes
const cypher = generator.generateCypher(graphData, {
  useConstraints: true,
  useIndexes: true,
  useMerge: true // Use MERGE instead of CREATE
});

// Save to file
import fs from 'fs';
fs.writeFileSync('graph-setup.cypher', cypher);
```

## Vector Embeddings

Enrich graph data with semantic embeddings:

```typescript
// Add embeddings to existing graph data
const enrichedData = await generator.enrichWithEmbeddings(graphData, {
  provider: 'openrouter',
  dimensions: 1536,
  batchSize: 100
});

// Find similar nodes
const embeddingEnrichment = generator.getEmbeddingEnrichment();
const similar = embeddingEnrichment.findSimilarNodes(
  targetNode,
  allNodes,
  10, // top 10
  'cosine' // similarity metric
);
```

## Configuration

### Environment Variables

```bash
OPENROUTER_API_KEY=your_api_key
OPENROUTER_MODEL=moonshot/kimi-k2-instruct
OPENROUTER_RATE_LIMIT_REQUESTS=10
OPENROUTER_RATE_LIMIT_INTERVAL=1000
EMBEDDING_DIMENSIONS=1536
```

### Programmatic Configuration

```typescript
const generator = createGraphDataGenerator({
  apiKey: 'your_api_key',
  model: 'moonshot/kimi-k2-instruct',
  baseURL: 'https://openrouter.ai/api/v1',
  timeout: 60000,
  maxRetries: 3,
  rateLimit: {
    requests: 10,
    interval: 1000
  }
});
```

## Integration with agentic-synth

This package extends `@ruvector/agentic-synth` with graph-specific data generation:

```typescript
import { createSynth } from '@ruvector/agentic-synth';
import { createGraphDataGenerator } from '@ruvector/graph-data-generator';

// Use both together
const synth = createSynth({ provider: 'gemini' });
const graphGen = createGraphDataGenerator({
  apiKey: process.env.OPENROUTER_API_KEY
});

// Generate structured data with synth
const structuredData = await synth.generateStructured({
  schema: { /* ... */ }
});

// Generate graph data
const graphData = await graphGen.generateKnowledgeGraph({
  domain: 'technology',
  entities: 100,
  relationships: 300
});
```

## API Reference

### GraphDataGenerator

Main class for graph data generation.

#### Methods

- `generateKnowledgeGraph(options)` - Generate knowledge graph
- `generateSocialNetwork(options)` - Generate social network
- `generateTemporalEvents(options)` - Generate temporal events
- `generateEntityRelationships(options)` - Generate entity-relationship graph
- `enrichWithEmbeddings(data, config)` - Add embeddings to graph data
- `generateCypher(data, options)` - Generate Cypher statements
- `getClient()` - Get OpenRouter client
- `getCypherGenerator()` - Get Cypher generator
- `getEmbeddingEnrichment()` - Get embedding enrichment

### OpenRouterClient

Client for OpenRouter API.

#### Methods

- `createCompletion(messages, options)` - Create chat completion
- `createStreamingCompletion(messages, options)` - Stream completion
- `generateStructured(systemPrompt, userPrompt, options)` - Generate structured data

### CypherGenerator

Generate Neo4j Cypher statements.

#### Methods

- `generate(data)` - Generate CREATE statements
- `generateMergeStatements(data)` - Generate MERGE statements
- `generateIndexStatements(data)` - Generate index creation
- `generateConstraintStatements(data)` - Generate constraints
- `generateSetupScript(data, options)` - Complete setup script
- `generateBatchInsert(data, batchSize)` - Batch insert statements

### EmbeddingEnrichment

Add vector embeddings to graph data.

#### Methods

- `enrichGraphData(data)` - Enrich entire graph
- `calculateSimilarity(emb1, emb2, metric)` - Calculate similarity
- `findSimilarNodes(node, allNodes, topK, metric)` - Find similar nodes

## License

MIT

## Author

rUv - https://github.com/ruvnet

## Repository

https://github.com/ruvnet/ruvector/tree/main/packages/graph-data-generator
