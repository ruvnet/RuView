# Agentic-Synth Architecture

## Overview

Agentic-Synth is a TypeScript-based synthetic data generation package that provides both CLI and SDK interfaces for generating time-series, events, and structured data using AI models (Gemini and OpenRouter APIs). It integrates seamlessly with midstreamer for streaming and agentic-robotics for automation workflows.

## Architecture Decision Records

### ADR-001: TypeScript with ESM Modules

**Status:** Accepted

**Context:**
- Need modern JavaScript/TypeScript support
- Integration with Node.js ecosystem
- Support for both CLI and SDK usage
- Future-proof module system

**Decision:**
Use TypeScript with ESM (ECMAScript Modules) as the primary module system.

**Rationale:**
- ESM is the standard for modern JavaScript
- Better tree-shaking and optimization
- Native TypeScript support
- Aligns with Node.js 18+ best practices

**Consequences:**
- Requires Node.js 18+
- All imports must use `.js` extensions in output
- Better interoperability with modern tools

---

### ADR-002: No Redis Dependency

**Status:** Accepted

**Context:**
- Need caching for context and API responses
- Avoid external service dependencies
- Simplify deployment and usage

**Decision:**
Use in-memory caching with LRU (Least Recently Used) strategy and optional file-based persistence.

**Rationale:**
- Simpler deployment (no Redis server needed)
- Faster for most use cases (in-process memory)
- File-based persistence for session continuity
- Optional integration with ruvector for advanced caching

**Consequences:**
- Cache doesn't survive process restart (unless persisted to file)
- Memory-limited (configurable max size)
- Single-process only (no distributed caching)

---

### ADR-003: Dual Interface (CLI + SDK)

**Status:** Accepted

**Context:**
- Need both programmatic access and command-line usage
- Different user personas (developers vs. operators)
- Consistent behavior across interfaces

**Decision:**
Implement core logic in SDK with CLI as a thin wrapper.

**Rationale:**
- Single source of truth for logic
- CLI uses SDK internally
- Easy to test and maintain
- Clear separation of concerns

**Consequences:**
- SDK must be feature-complete
- CLI is primarily for ergonomics
- Documentation needed for both interfaces

---

### ADR-004: Model Router Architecture

**Status:** Accepted

**Context:**
- Support multiple AI providers (Gemini, OpenRouter)
- Different models for different data types
- Cost optimization and fallback strategies

**Decision:**
Implement a model router that selects appropriate models based on data type, cost, and availability.

**Rationale:**
- Flexibility in model selection
- Automatic fallback on failures
- Cost optimization through smart routing
- Provider-agnostic interface

**Consequences:**
- More complex routing logic
- Need configuration for routing rules
- Performance monitoring required

---

### ADR-005: Plugin Architecture for Generators

**Status:** Accepted

**Context:**
- Different data types need different generation strategies
- Extensibility for custom generators
- Community contributions

**Decision:**
Use a plugin-based architecture where each data type has a dedicated generator.

**Rationale:**
- Clear separation of concerns
- Easy to add new data types
- Testable in isolation
- Community can contribute generators

**Consequences:**
- Need generator registration system
- Consistent generator interface
- Documentation for custom generators

---

### ADR-006: Optional Integration Pattern

**Status:** Accepted

**Context:**
- Integration with midstreamer, agentic-robotics, and ruvector
- Not all users need all integrations
- Avoid mandatory dependencies

**Decision:**
Use optional peer dependencies with runtime detection.

**Rationale:**
- Lighter install for basic usage
- Pay-as-you-go complexity
- Clear integration boundaries
- Graceful degradation

**Consequences:**
- Runtime checks for integration availability
- Clear documentation about optional features
- Integration adapters with null implementations

## System Architecture

### High-Level Component Diagram (C4 Level 2)

```
┌─────────────────────────────────────────────────────────────────┐
│                         Agentic-Synth                            │
│                                                                   │
│  ┌──────────────┐                           ┌─────────────────┐  │
│  │     CLI      │                           │       SDK       │  │
│  │  (Commander) │◄──────────────────────────►  (Public API)  │  │
│  └──────┬───────┘                           └────────┬────────┘  │
│         │                                            │           │
│         └────────────────────┬───────────────────────┘           │
│                              │                                   │
│                    ┌─────────▼────────┐                          │
│                    │   Core Engine    │                          │
│                    │                  │                          │
│                    │  - Generator Hub │                          │
│                    │  - Model Router  │                          │
│                    │  - Cache Manager │                          │
│                    │  - Config System │                          │
│                    └─────────┬────────┘                          │
│                              │                                   │
│         ┌────────────────────┼────────────────────┐              │
│         │                    │                    │              │
│    ┌────▼─────┐      ┌──────▼──────┐      ┌─────▼──────┐       │
│    │Generator │      │   Models    │      │Integration │       │
│    │  System  │      │   System    │      │  Adapters  │       │
│    │          │      │             │      │            │       │
│    │-TimeSeries│     │- Gemini     │      │-Midstreamer│       │
│    │-Events    │     │- OpenRouter │      │-Robotics   │       │
│    │-Structured│     │- Router     │      │-Ruvector   │       │
│    └──────────┘      └─────────────┘      └────────────┘       │
└───────────────────────────────────────────────────────────────┘
         │                     │                       │
         ▼                     ▼                       ▼
┌─────────────┐      ┌──────────────┐       ┌──────────────────┐
│   Output    │      │   AI APIs    │       │    External      │
│  (Streams)  │      │              │       │  Integrations    │
│             │      │ - Gemini API │       │                  │
│ - JSON      │      │ - OpenRouter │       │ - Midstreamer    │
│ - CSV       │      │              │       │ - Agentic-Robot  │
│ - Parquet   │      └──────────────┘       │ - Ruvector DB    │
└─────────────┘                             └──────────────────┘
```

### Data Flow Diagram

```
┌─────────┐
│  User   │
└────┬────┘
     │
     │ (CLI Command or SDK Call)
     ▼
┌─────────────────┐
│ Request Parser  │ ──► Validate schema, parse options
└────┬────────────┘
     │
     ▼
┌─────────────────┐
│ Generator Hub   │ ──► Select appropriate generator
└────┬────────────┘
     │
     ▼
┌─────────────────┐
│  Model Router   │ ──► Choose AI model (Gemini/OpenRouter)
└────┬────────────┘
     │
     ├──► Check Cache ─────► Cache Hit? ─────► Return cached
     │                            │
     │                            │ (Miss)
     ▼                            ▼
┌─────────────────┐    ┌──────────────────┐
│   AI Provider   │───►│ Context Builder  │
│  (Gemini/OR)    │    │ (Prompt + Schema)│
└────┬────────────┘    └──────────────────┘
     │
     │ (Generated Data)
     ▼
┌─────────────────┐
│  Post-Processor │ ──► Validate, transform, format
└────┬────────────┘
     │
     ├──► Store in Cache
     │
     ├──► Stream via Midstreamer (if enabled)
     │
     ├──► Store in Ruvector (if enabled)
     │
     ▼
┌─────────────────┐
│ Output Handler  │ ──► JSON/CSV/Parquet/Stream
└─────────────────┘
```

## Core Components

### 1. Generator System

**Purpose:** Generate different types of synthetic data.

**Components:**
- `TimeSeriesGenerator`: Generate time-series data with trends, seasonality, noise
- `EventGenerator`: Generate event streams with timestamps and metadata
- `StructuredGenerator`: Generate structured records (JSON, tables)
- `CustomGenerator`: Base class for user-defined generators

**Interface:**
```typescript
interface Generator<T = any> {
  readonly type: string;
  readonly schema: z.ZodSchema<T>;

  generate(options: GenerateOptions): Promise<T>;
  generateBatch(count: number, options: GenerateOptions): AsyncIterator<T>;
  validate(data: unknown): T;
}
```

### 2. Model System

**Purpose:** Interface with AI providers for data generation.

**Components:**
- `GeminiProvider`: Google Gemini API integration
- `OpenRouterProvider`: OpenRouter API integration
- `ModelRouter`: Smart routing between providers
- `ContextCache`: Cache prompts and responses

**Interface:**
```typescript
interface ModelProvider {
  readonly name: string;
  readonly supportedModels: string[];

  generate(prompt: string, options: ModelOptions): Promise<string>;
  generateStream(prompt: string, options: ModelOptions): AsyncIterator<string>;
  getCost(model: string, tokens: number): number;
}
```

### 3. Cache Manager

**Purpose:** Cache API responses and context without Redis.

**Strategy:**
- In-memory LRU cache (configurable size)
- Optional file-based persistence
- Content-based cache keys (hash of prompt + options)
- TTL support

**Implementation:**
```typescript
class CacheManager {
  private cache: Map<string, CacheEntry>;
  private maxSize: number;
  private ttl: number;

  get(key: string): CacheEntry | undefined;
  set(key: string, value: any, ttl?: number): void;
  clear(): void;
  persist(path: string): Promise<void>;
  restore(path: string): Promise<void>;
}
```

### 4. Integration Adapters

**Purpose:** Optional integrations with external tools.

**Adapters:**

#### MidstreamerAdapter
```typescript
interface MidstreamerAdapter {
  isAvailable(): boolean;
  stream(data: AsyncIterator<any>): Promise<void>;
  createPipeline(config: PipelineConfig): StreamPipeline;
}
```

#### AgenticRoboticsAdapter
```typescript
interface AgenticRoboticsAdapter {
  isAvailable(): boolean;
  registerWorkflow(name: string, generator: Generator): void;
  triggerWorkflow(name: string, options: any): Promise<void>;
}
```

#### RuvectorAdapter
```typescript
interface RuvectorAdapter {
  isAvailable(): boolean;
  store(data: any, metadata?: any): Promise<string>;
  search(query: any, limit?: number): Promise<any[]>;
}
```

## API Design

### SDK API

#### Basic Usage
```typescript
import { AgenticSynth, TimeSeriesGenerator } from 'agentic-synth';

// Initialize
const synth = new AgenticSynth({
  apiKeys: {
    gemini: process.env.GEMINI_API_KEY,
    openRouter: process.env.OPENROUTER_API_KEY
  },
  cache: {
    enabled: true,
    maxSize: 1000,
    ttl: 3600000 // 1 hour
  }
});

// Generate time-series data
const data = await synth.generate('timeseries', {
  count: 1000,
  schema: {
    timestamp: 'datetime',
    temperature: { type: 'number', min: -20, max: 40 },
    humidity: { type: 'number', min: 0, max: 100 }
  },
  model: 'gemini-pro'
});

// Stream generation
for await (const record of synth.generateStream('events', options)) {
  console.log(record);
}
```

#### Advanced Usage with Integrations
```typescript
import { AgenticSynth } from 'agentic-synth';
import { enableMidstreamer, enableRuvector } from 'agentic-synth/integrations';

const synth = new AgenticSynth({
  apiKeys: { ... }
});

// Enable optional integrations
enableMidstreamer(synth, {
  pipeline: 'synthetic-data-stream'
});

enableRuvector(synth, {
  dbPath: './data/vectors.db'
});

// Generate and automatically stream + store
await synth.generate('structured', {
  count: 10000,
  stream: true,        // Auto-streams via midstreamer
  vectorize: true      // Auto-stores in ruvector
});
```

### CLI API

#### Basic Commands
```bash
# Generate time-series data
npx agentic-synth generate timeseries \
  --count 1000 \
  --schema ./schema.json \
  --output data.json \
  --model gemini-pro

# Generate events
npx agentic-synth generate events \
  --count 5000 \
  --rate 100/sec \
  --stream \
  --output events.jsonl

# Generate structured data
npx agentic-synth generate structured \
  --schema ./user-schema.json \
  --count 10000 \
  --format csv \
  --output users.csv
```

#### Advanced Commands
```bash
# With model routing
npx agentic-synth generate timeseries \
  --count 1000 \
  --auto-route \
  --fallback gemini-pro,gpt-4 \
  --budget 0.10

# With integrations
npx agentic-synth generate events \
  --count 10000 \
  --stream-to midstreamer \
  --vectorize-with ruvector \
  --cache-policy aggressive

# Batch generation
npx agentic-synth batch generate \
  --config ./batch-config.yaml \
  --parallel 4 \
  --output ./output-dir/
```

## Configuration System

### Configuration File Format (.agentic-synth.json)

```json
{
  "apiKeys": {
    "gemini": "${GEMINI_API_KEY}",
    "openRouter": "${OPENROUTER_API_KEY}"
  },
  "cache": {
    "enabled": true,
    "maxSize": 1000,
    "ttl": 3600000,
    "persistPath": "./.cache/agentic-synth"
  },
  "models": {
    "routing": {
      "strategy": "cost-optimized",
      "fallbackChain": ["gemini-pro", "gpt-4", "claude-3"]
    },
    "defaults": {
      "timeseries": "gemini-pro",
      "events": "gpt-4-turbo",
      "structured": "claude-3-sonnet"
    }
  },
  "integrations": {
    "midstreamer": {
      "enabled": true,
      "defaultPipeline": "synthetic-data"
    },
    "agenticRobotics": {
      "enabled": false
    },
    "ruvector": {
      "enabled": true,
      "dbPath": "./data/vectors.db"
    }
  },
  "generators": {
    "timeseries": {
      "defaultSampleRate": "1s",
      "defaultDuration": "1h"
    },
    "events": {
      "defaultRate": "100/sec"
    }
  }
}
```

## Technology Stack

### Core Dependencies
- **TypeScript 5.7+**: Type safety and modern JavaScript features
- **Zod 3.23+**: Runtime schema validation
- **Commander 12+**: CLI framework
- **Winston 3+**: Logging system

### AI Provider SDKs
- **@google/generative-ai**: Gemini API integration
- **openai**: OpenRouter API (compatible with OpenAI SDK)

### Optional Integrations
- **midstreamer**: Streaming data pipelines
- **agentic-robotics**: Automation workflows
- **ruvector**: Vector database for embeddings

### Development Tools
- **Vitest**: Testing framework
- **ESLint**: Linting
- **Prettier**: Code formatting

## Performance Considerations

### Context Caching Strategy
1. **Cache Key Generation**: Hash of (prompt template + schema + model options)
2. **Cache Storage**: In-memory Map with LRU eviction
3. **Cache Persistence**: Optional file-based storage for session continuity
4. **Cache Invalidation**: TTL-based + manual invalidation API

### Model Selection Optimization
1. **Cost-Based Routing**: Select cheapest model that meets requirements
2. **Performance-Based Routing**: Select fastest model
3. **Quality-Based Routing**: Select highest quality model
4. **Hybrid Routing**: Balance cost/performance/quality

### Memory Management
- Streaming generation for large datasets (avoid loading all in memory)
- Chunked processing for batch operations
- Configurable batch sizes
- Memory-efficient serialization formats (JSONL, Parquet)

## Security Considerations

### API Key Management
- Environment variable loading via dotenv
- Config file with environment variable substitution
- Never log API keys
- Secure storage in config files (encrypted or gitignored)

### Data Validation
- Input validation using Zod schemas
- Output validation before returning to user
- Sanitization of AI-generated content
- Rate limiting for API calls

### Error Handling
- Graceful degradation on provider failures
- Automatic retry with exponential backoff
- Detailed error logging without sensitive data
- User-friendly error messages

## Testing Strategy

### Unit Tests
- Individual generator tests
- Model provider mocks
- Cache manager tests
- Integration adapter tests (with mocks)

### Integration Tests
- End-to-end generation workflows
- Real API calls (with test API keys)
- Integration with midstreamer/robotics (optional)
- CLI command tests

### Performance Tests
- Benchmark generation speed
- Memory usage profiling
- Cache hit rate analysis
- Model routing efficiency

## Deployment & Distribution

### NPM Package
- Published as `agentic-synth`
- Dual CJS/ESM support (via tsconfig)
- Tree-shakeable exports
- Type definitions included

### CLI Distribution
- Available via `npx agentic-synth`
- Self-contained executable (includes dependencies)
- Automatic updates via npm

### Documentation
- README.md: Quick start guide
- API.md: Complete SDK reference
- CLI.md: Command-line reference
- EXAMPLES.md: Common use cases
- INTEGRATIONS.md: Optional integration guides

## Future Enhancements

### Phase 2 Features
- Support for more AI providers (Anthropic, Cohere, local models)
- Advanced schema generation from examples
- Multi-modal data generation (text + images)
- Distributed generation across multiple nodes
- Web UI for visual data generation

### Phase 3 Features
- Real-time data generation with WebSocket support
- Integration with data orchestration platforms (Airflow, Prefect)
- Custom model fine-tuning for domain-specific data
- Data quality metrics and validation
- Automated testing dataset generation

## Conclusion

This architecture provides a solid foundation for agentic-synth as a flexible, performant, and extensible synthetic data generation tool. The dual CLI/SDK interface, optional integrations, and plugin-based architecture ensure it can serve a wide range of use cases while remaining simple for basic usage.
