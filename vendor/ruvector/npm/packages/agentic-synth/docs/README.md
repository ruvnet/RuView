# agentic-synth

AI-powered synthetic data generation with Gemini and OpenRouter integration.

## Features

- 🤖 **Multi-Provider Support**: Gemini and OpenRouter APIs
- ⚡ **High Performance**: Context caching and request optimization
- 📊 **Multiple Data Types**: Time-series, events, and structured data
- 🔄 **Streaming Support**: Real-time data generation with npx midstreamer
- 🤝 **Automation Ready**: Hooks integration with npx agentic-robotics
- 💾 **Optional Vector DB**: Integration with ruvector
- 🎯 **Type-Safe**: Full TypeScript support

## Installation

```bash
npm install agentic-synth
```

## Quick Start

### As SDK

```typescript
import { createSynth } from 'agentic-synth';

const synth = createSynth({
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY
});

// Generate time-series data
const result = await synth.generateTimeSeries({
  count: 100,
  interval: '1h',
  metrics: ['temperature', 'humidity'],
  trend: 'up'
});

console.log(result.data);
```

### As CLI

```bash
# Generate time-series data
npx agentic-synth generate timeseries --count 100 --output data.json

# Generate events
npx agentic-synth generate events --count 50 --schema events.json

# Generate structured data
npx agentic-synth generate structured --count 20 --format csv
```

## Configuration

### Environment Variables

```bash
GEMINI_API_KEY=your_gemini_api_key
OPENROUTER_API_KEY=your_openrouter_api_key
```

### Config File (synth.config.json)

```json
{
  "provider": "gemini",
  "model": "gemini-2.0-flash-exp",
  "cacheStrategy": "memory",
  "cacheTTL": 3600,
  "maxRetries": 3,
  "timeout": 30000
}
```

## Data Types

### Time-Series

Generate temporal data with trends and seasonality:

```typescript
const result = await synth.generateTimeSeries({
  count: 100,
  startDate: new Date(),
  interval: '1h',
  metrics: ['cpu', 'memory', 'disk'],
  trend: 'up',
  seasonality: true,
  noise: 0.1
});
```

### Events

Generate event logs with realistic distributions:

```typescript
const result = await synth.generateEvents({
  count: 1000,
  eventTypes: ['click', 'view', 'purchase'],
  distribution: 'poisson',
  userCount: 50,
  timeRange: {
    start: new Date('2024-01-01'),
    end: new Date('2024-12-31')
  }
});
```

### Structured Data

Generate structured records with custom schemas:

```typescript
const result = await synth.generateStructured({
  count: 50,
  schema: {
    id: { type: 'string', required: true },
    name: { type: 'string', required: true },
    email: { type: 'string', required: true },
    age: { type: 'number', required: true }
  },
  format: 'json'
});
```

## Advanced Features

### Streaming

```typescript
const synth = createSynth({ streaming: true });

for await (const dataPoint of synth.generateStream('timeseries', {
  count: 1000,
  interval: '1m',
  metrics: ['value']
})) {
  console.log(dataPoint);
}
```

### Batch Generation

```typescript
const batches = [
  { count: 100, metrics: ['temperature'] },
  { count: 200, metrics: ['humidity'] },
  { count: 150, metrics: ['pressure'] }
];

const results = await synth.generateBatch('timeseries', batches, 3);
```

### Caching

```typescript
const synth = createSynth({
  cacheStrategy: 'memory',
  cacheTTL: 3600 // 1 hour
});

// First call generates, second call uses cache
const result1 = await synth.generate('timeseries', options);
const result2 = await synth.generate('timeseries', options); // Cached
```

### Model Routing

```typescript
// Automatic fallback chain
const synth = createSynth({
  provider: 'gemini',
  fallbackChain: ['openrouter']
});

// Or specify model directly
const result = await synth.generate('timeseries', {
  ...options,
  model: 'gemini-1.5-pro'
});
```

## CLI Reference

### Commands

```bash
# Generate data
agentic-synth generate <type> [options]

# Interactive mode
agentic-synth interactive

# Manage config
agentic-synth config [init|show|set]

# Show examples
agentic-synth examples
```

### Options

```
-c, --count <number>       Number of records
-o, --output <file>        Output file path
-f, --format <format>      Output format (json, csv)
--provider <provider>      AI provider (gemini, openrouter)
--model <model>           Model name
--schema <file>           Schema file (JSON)
--config <file>           Config file path
--stream                  Enable streaming
--cache                   Enable caching
```

## Integration

### With Midstreamer

```typescript
import { createSynth } from 'agentic-synth';
import { createStreamer } from 'midstreamer';

const synth = createSynth({ streaming: true });
const streamer = createStreamer();

for await (const data of synth.generateStream('timeseries', options)) {
  await streamer.send(data);
}
```

### With Agentic-Robotics

```typescript
import { createSynth } from 'agentic-synth';
import { createHooks } from 'agentic-robotics';

const synth = createSynth({ automation: true });
const hooks = createHooks();

hooks.on('generate:before', async (options) => {
  console.log('Generating data...', options);
});

hooks.on('generate:after', async (result) => {
  console.log('Generated:', result.metadata);
});
```

## API Reference

See [API.md](./API.md) for complete API documentation.

## Examples

Check the [examples/](../examples/) directory for more usage examples.

## License

MIT
