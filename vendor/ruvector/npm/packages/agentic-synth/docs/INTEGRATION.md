# Integration Guide

This document describes how agentic-synth integrates with external tools and libraries.

## Integration Overview

Agentic-synth supports optional integrations with:

1. **Midstreamer** - Streaming data pipelines
2. **Agentic-Robotics** - Automation workflows
3. **Ruvector** - Vector database for embeddings

All integrations are:
- **Optional** - Package works without them
- **Peer dependencies** - Installed separately
- **Runtime detected** - Gracefully degrade if unavailable
- **Adapter-based** - Clean integration boundaries

---

## Midstreamer Integration

### Purpose

Stream generated data through pipelines for real-time processing.

### Installation

```bash
npm install midstreamer
```

### Usage

#### Basic Streaming

```typescript
import { AgenticSynth } from 'agentic-synth';
import { enableMidstreamer } from 'agentic-synth/integrations';

const synth = new AgenticSynth();

// Enable midstreamer integration
enableMidstreamer(synth, {
  pipeline: 'synthetic-data-stream',
  bufferSize: 1000,
  flushInterval: 5000 // ms
});

// Generate with streaming
const result = await synth.generate('timeseries', {
  count: 10000,
  stream: true // Automatically streams via midstreamer
});
```

#### Custom Pipeline

```typescript
import { createPipeline } from 'midstreamer';

const pipeline = createPipeline({
  name: 'data-processing',
  transforms: [
    { type: 'filter', predicate: (data) => data.value > 0 },
    { type: 'map', fn: (data) => ({ ...data, doubled: data.value * 2 }) }
  ],
  outputs: [
    { type: 'file', path: './output/processed.jsonl' },
    { type: 'http', url: 'https://api.example.com/data' }
  ]
});

enableMidstreamer(synth, {
  pipeline
});
```

#### CLI Usage

```bash
npx agentic-synth generate events \
  --count 10000 \
  --stream \
  --stream-to midstreamer \
  --stream-pipeline data-processing
```

### API Reference

```typescript
interface MidstreamerAdapter {
  isAvailable(): boolean;
  stream(data: AsyncIterator<any>): Promise<void>;
  createPipeline(config: PipelineConfig): StreamPipeline;
}
```

---

## Agentic-Robotics Integration

### Purpose

Integrate synthetic data generation into automation workflows.

### Installation

```bash
npm install agentic-robotics
```

### Usage

#### Register Workflows

```typescript
import { AgenticSynth } from 'agentic-synth';
import { enableAgenticRobotics } from 'agentic-synth/integrations';

const synth = new AgenticSynth();

enableAgenticRobotics(synth, {
  workflowEngine: 'default'
});

// Register data generation workflow
synth.integrations.robotics.registerWorkflow('daily-timeseries', async (params) => {
  return await synth.generate('timeseries', {
    count: params.count || 1000,
    startTime: params.startTime,
    endTime: params.endTime
  });
});

// Trigger workflow
await synth.integrations.robotics.triggerWorkflow('daily-timeseries', {
  count: 5000,
  startTime: '2024-01-01',
  endTime: '2024-01-31'
});
```

#### Scheduled Generation

```typescript
import { createSchedule } from 'agentic-robotics';

const schedule = createSchedule({
  workflow: 'daily-timeseries',
  cron: '0 0 * * *', // Daily at midnight
  params: {
    count: 10000
  }
});

synth.integrations.robotics.addSchedule(schedule);
```

#### CLI Usage

```bash
# Register workflow
npx agentic-synth workflow register \
  --name daily-data \
  --generator timeseries \
  --options '{"count": 1000}'

# Trigger workflow
npx agentic-synth workflow trigger daily-data
```

### API Reference

```typescript
interface AgenticRoboticsAdapter {
  isAvailable(): boolean;
  registerWorkflow(name: string, generator: Generator): void;
  triggerWorkflow(name: string, options: any): Promise<void>;
  addSchedule(schedule: Schedule): void;
}
```

---

## Ruvector Integration

### Purpose

Store generated data in vector database for similarity search and retrieval.

### Installation

```bash
# Ruvector is in the same monorepo, no external install needed
```

### Usage

#### Basic Vector Storage

```typescript
import { AgenticSynth } from 'agentic-synth';
import { enableRuvector } from 'agentic-synth/integrations';

const synth = new AgenticSynth();

enableRuvector(synth, {
  dbPath: './data/vectors.db',
  collectionName: 'synthetic-data',
  embeddingModel: 'text-embedding-004',
  dimensions: 768
});

// Generate and automatically vectorize
const result = await synth.generate('structured', {
  count: 1000,
  vectorize: true // Automatically stores in ruvector
});

// Search similar records
const similar = await synth.integrations.ruvector.search({
  query: 'sample query',
  limit: 10,
  threshold: 0.8
});
```

#### Custom Embeddings

```typescript
enableRuvector(synth, {
  dbPath: './data/vectors.db',
  embeddingFn: async (data) => {
    // Custom embedding logic
    const text = JSON.stringify(data);
    return await generateEmbedding(text);
  }
});
```

#### Semantic Search

```typescript
// Generate data with metadata for better search
const result = await synth.generate('structured', {
  count: 1000,
  schema: {
    id: { type: 'string', format: 'uuid' },
    content: { type: 'string' },
    category: { type: 'enum', enum: ['tech', 'science', 'art'] }
  },
  vectorize: true
});

// Search by content similarity
const results = await synth.integrations.ruvector.search({
  query: 'artificial intelligence',
  filter: { category: 'tech' },
  limit: 20
});
```

#### CLI Usage

```bash
# Generate with vectorization
npx agentic-synth generate structured \
  --count 1000 \
  --schema ./schema.json \
  --vectorize-with ruvector \
  --vector-db ./data/vectors.db

# Search vectors
npx agentic-synth vector search \
  --query "sample query" \
  --db ./data/vectors.db \
  --limit 10
```

### API Reference

```typescript
interface RuvectorAdapter {
  isAvailable(): boolean;
  store(data: any, metadata?: any): Promise<string>;
  storeBatch(data: any[], metadata?: any[]): Promise<string[]>;
  search(query: SearchQuery, limit?: number): Promise<SearchResult[]>;
  delete(id: string): Promise<void>;
  update(id: string, data: any): Promise<void>;
}

interface SearchQuery {
  query: string | number[];
  filter?: Record<string, any>;
  threshold?: number;
}

interface SearchResult {
  id: string;
  score: number;
  data: any;
  metadata?: any;
}
```

---

## Combined Integration Example

### Multi-Integration Workflow

```typescript
import { AgenticSynth } from 'agentic-synth';
import {
  enableMidstreamer,
  enableAgenticRobotics,
  enableRuvector
} from 'agentic-synth/integrations';

const synth = new AgenticSynth({
  apiKeys: {
    gemini: process.env.GEMINI_API_KEY
  }
});

// Enable all integrations
enableMidstreamer(synth, {
  pipeline: 'data-stream'
});

enableAgenticRobotics(synth, {
  workflowEngine: 'default'
});

enableRuvector(synth, {
  dbPath: './data/vectors.db'
});

// Register comprehensive workflow
synth.integrations.robotics.registerWorkflow('process-and-store', async (params) => {
  // Generate data
  const result = await synth.generate('structured', {
    count: params.count,
    stream: true,      // Streams via midstreamer
    vectorize: true    // Stores in ruvector
  });

  return result;
});

// Execute workflow
await synth.integrations.robotics.triggerWorkflow('process-and-store', {
  count: 10000
});

// Data is now:
// 1. Generated via AI models
// 2. Streamed through midstreamer pipeline
// 3. Stored in ruvector for search
```

---

## Integration Availability Detection

### Runtime Detection

```typescript
import { AgenticSynth } from 'agentic-synth';

const synth = new AgenticSynth();

// Check which integrations are available
if (synth.integrations.hasMidstreamer()) {
  console.log('Midstreamer is available');
}

if (synth.integrations.hasAgenticRobotics()) {
  console.log('Agentic-Robotics is available');
}

if (synth.integrations.hasRuvector()) {
  console.log('Ruvector is available');
}
```

### Graceful Degradation

```typescript
// Code works with or without integrations
const result = await synth.generate('timeseries', {
  count: 1000,
  stream: true,      // Only streams if midstreamer available
  vectorize: true    // Only vectorizes if ruvector available
});

// Always works, integrations are optional
```

---

## Custom Integrations

### Creating Custom Integration Adapters

```typescript
import { IntegrationAdapter } from 'agentic-synth/integrations';

class MyCustomAdapter implements IntegrationAdapter {
  readonly name = 'my-custom-integration';
  private available = false;

  constructor(private config: any) {
    this.detectAvailability();
  }

  isAvailable(): boolean {
    return this.available;
  }

  async initialize(): Promise<void> {
    // Setup logic
  }

  async processData(data: any[]): Promise<void> {
    // Custom processing logic
  }

  async shutdown(): Promise<void> {
    // Cleanup logic
  }

  private detectAvailability(): void {
    try {
      require('my-custom-package');
      this.available = true;
    } catch {
      this.available = false;
    }
  }
}

// Register custom adapter
synth.integrations.register(new MyCustomAdapter(config));
```

---

## Configuration

### Integration Configuration File

```json
{
  "integrations": {
    "midstreamer": {
      "enabled": true,
      "pipeline": "synthetic-data-stream",
      "bufferSize": 1000,
      "flushInterval": 5000,
      "transforms": [
        {
          "type": "filter",
          "predicate": "data.value > 0"
        }
      ]
    },
    "agenticRobotics": {
      "enabled": true,
      "workflowEngine": "default",
      "defaultWorkflow": "data-generation",
      "schedules": [
        {
          "name": "daily-data",
          "cron": "0 0 * * *",
          "workflow": "daily-timeseries"
        }
      ]
    },
    "ruvector": {
      "enabled": true,
      "dbPath": "./data/vectors.db",
      "collectionName": "synthetic-data",
      "embeddingModel": "text-embedding-004",
      "dimensions": 768,
      "indexType": "hnsw",
      "distanceMetric": "cosine"
    }
  }
}
```

---

## Troubleshooting

### Integration Not Detected

**Problem:** Integration marked as unavailable

**Solutions:**
1. Ensure peer dependency is installed: `npm install <package>`
2. Check import/require paths are correct
3. Verify package version compatibility
4. Check logs for initialization errors

### Performance Issues

**Problem:** Slow generation with integrations

**Solutions:**
1. Adjust buffer sizes for streaming
2. Use batch operations instead of individual calls
3. Enable caching to avoid redundant processing
4. Profile with `synth.integrations.getMetrics()`

### Memory Issues

**Problem:** High memory usage with integrations

**Solutions:**
1. Use streaming mode instead of loading all data
2. Adjust batch sizes to smaller values
3. Clear caches periodically
4. Configure TTL for cached data

---

## Best Practices

1. **Optional Dependencies**: Always check `isAvailable()` before using integration features
2. **Error Handling**: Wrap integration calls in try-catch blocks
3. **Configuration**: Use config files for complex integration setups
4. **Testing**: Test with and without integrations enabled
5. **Documentation**: Document which integrations your workflows depend on
6. **Monitoring**: Track integration metrics and performance
7. **Versioning**: Pin peer dependency versions for stability

---

## Example Projects

See the `/examples` directory for complete integration examples:

- `examples/midstreamer-pipeline/` - Real-time data streaming
- `examples/robotics-workflow/` - Automated generation workflows
- `examples/ruvector-search/` - Vector search and retrieval
- `examples/full-integration/` - All integrations combined
