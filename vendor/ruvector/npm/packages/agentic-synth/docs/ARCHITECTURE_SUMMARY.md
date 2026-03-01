# Agentic-Synth Architecture Summary

## Overview

Complete architecture design for **agentic-synth** - a TypeScript-based synthetic data generator using Gemini and OpenRouter APIs with streaming and automation support.

## Key Design Decisions

### 1. Technology Stack

**Core:**
- TypeScript 5.7+ with strict mode
- ESM modules (NodeNext)
- Zod for runtime validation
- Winston for logging
- Commander for CLI

**AI Providers:**
- Google Gemini API via `@google/generative-ai`
- OpenRouter API via OpenAI-compatible SDK

**Optional Integrations:**
- Midstreamer (streaming pipelines)
- Agentic-Robotics (automation workflows)
- Ruvector (vector database) - workspace dependency

### 2. Architecture Patterns

**Dual Interface:**
- SDK for programmatic access
- CLI for command-line usage
- CLI uses SDK internally (single source of truth)

**Plugin Architecture:**
- Generator plugins for different data types
- Model provider plugins for AI APIs
- Integration adapters for external tools

**Caching Strategy:**
- In-memory LRU cache (no Redis)
- Optional file-based persistence
- Content-based cache keys

**Model Routing:**
- Cost-optimized routing
- Performance-optimized routing
- Quality-optimized routing
- Fallback chains for reliability

### 3. Integration Design

**Optional Dependencies:**
All integrations are optional with runtime detection:
- Package works standalone
- Graceful degradation if integrations unavailable
- Clear documentation about optional features

**Integration Points:**
1. **Midstreamer**: Stream generated data through pipelines
2. **Agentic-Robotics**: Register data generation workflows
3. **Ruvector**: Store generated data as vectors

## Project Structure

```
packages/agentic-synth/
├── src/
│   ├── index.ts                 # Main SDK entry
│   ├── types/index.ts           # Type definitions
│   ├── sdk/AgenticSynth.ts      # Main SDK class
│   ├── core/
│   │   ├── Config.ts            # Configuration system
│   │   ├── Cache.ts             # LRU cache manager
│   │   └── Logger.ts            # Logging system
│   ├── generators/
│   │   ├── base.ts              # Generator interface
│   │   ├── Hub.ts               # Generator registry
│   │   ├── TimeSeries.ts        # Time-series generator
│   │   ├── Events.ts            # Event generator
│   │   └── Structured.ts        # Structured data generator
│   ├── models/
│   │   ├── base.ts              # Model provider interface
│   │   ├── Router.ts            # Model routing logic
│   │   └── providers/
│   │       ├── Gemini.ts        # Gemini integration
│   │       └── OpenRouter.ts    # OpenRouter integration
│   ├── integrations/
│   │   ├── Manager.ts           # Integration lifecycle
│   │   ├── Midstreamer.ts       # Streaming adapter
│   │   ├── AgenticRobotics.ts   # Automation adapter
│   │   └── Ruvector.ts          # Vector DB adapter
│   ├── bin/
│   │   ├── cli.ts               # CLI entry point
│   │   └── commands/            # CLI commands
│   └── utils/
│       ├── validation.ts        # Validation helpers
│       ├── serialization.ts     # Output formatting
│       └── prompts.ts           # AI prompt templates
├── tests/
│   ├── unit/                    # Unit tests
│   └── integration/             # Integration tests
├── examples/                    # Usage examples
├── docs/
│   ├── ARCHITECTURE.md          # Complete architecture
│   ├── API.md                   # API reference
│   ├── INTEGRATION.md           # Integration guide
│   ├── DIRECTORY_STRUCTURE.md   # Project layout
│   └── IMPLEMENTATION_PLAN.md   # Implementation guide
├── config/
│   └── .agentic-synth.example.json
├── package.json
├── tsconfig.json
└── README.md
```

## API Design

### SDK API

```typescript
import { AgenticSynth } from 'agentic-synth';

// Initialize
const synth = new AgenticSynth({
  apiKeys: {
    gemini: process.env.GEMINI_API_KEY,
    openRouter: process.env.OPENROUTER_API_KEY
  },
  cache: { enabled: true, maxSize: 1000 }
});

// Generate data
const result = await synth.generate('timeseries', {
  count: 1000,
  schema: { temperature: { type: 'number', min: -20, max: 40 } }
});

// Stream generation
for await (const record of synth.generateStream('events', { count: 1000 })) {
  console.log(record);
}
```

### CLI API

```bash
# Generate time-series data
npx agentic-synth generate timeseries \
  --count 1000 \
  --schema ./schema.json \
  --output data.json

# Batch generation
npx agentic-synth batch generate \
  --config ./batch-config.yaml \
  --parallel 4
```

## Data Flow

```
User Request
    ↓
Request Parser (validate schema, options)
    ↓
Generator Hub (select appropriate generator)
    ↓
Model Router (choose AI model: Gemini/OpenRouter)
    ↓
Cache Check ──→ Cache Hit? ──→ Return cached
    ↓ (Miss)
AI Provider (Gemini/OpenRouter)
    ↓
Generated Data
    ↓
Post-Processor (validate, transform)
    ↓
├─→ Store in Cache
├─→ Stream via Midstreamer (if enabled)
├─→ Store in Ruvector (if enabled)
└─→ Output Handler (JSON/CSV/Parquet/Stream)
```

## Key Components

### 1. Generator System

**TimeSeriesGenerator**
- Generate time-series data with trends, seasonality, noise
- Configurable sample rates and time ranges
- Statistical distribution control

**EventGenerator**
- Generate event streams with timestamps
- Rate control (events per second/minute)
- Distribution types (uniform, poisson, bursty)
- Event correlations

**StructuredGenerator**
- Generate structured records based on schema
- Field type support (string, number, boolean, datetime, enum)
- Constraint enforcement (unique, range, foreign keys)
- Output formats (JSON, CSV, Parquet)

### 2. Model System

**GeminiProvider**
- Google Gemini API integration
- Context caching support
- Streaming responses
- Cost tracking

**OpenRouterProvider**
- OpenRouter API integration
- Multi-model access
- Automatic fallback
- Cost optimization

**ModelRouter**
- Smart routing strategies
- Fallback chain management
- Cost/performance/quality optimization
- Request caching

### 3. Integration System

**MidstreamerAdapter**
- Stream data through pipelines
- Buffer management
- Transform support
- Multiple output targets

**AgenticRoboticsAdapter**
- Workflow registration
- Scheduled generation
- Event-driven triggers
- Automation integration

**RuvectorAdapter**
- Vector storage
- Similarity search
- Batch operations
- Embedding generation

## Configuration

### Environment Variables

```bash
GEMINI_API_KEY=your-gemini-key
OPENROUTER_API_KEY=your-openrouter-key
```

### Config File (`.agentic-synth.json`)

```json
{
  "apiKeys": {
    "gemini": "${GEMINI_API_KEY}",
    "openRouter": "${OPENROUTER_API_KEY}"
  },
  "cache": {
    "enabled": true,
    "maxSize": 1000,
    "ttl": 3600000
  },
  "models": {
    "routing": {
      "strategy": "cost-optimized",
      "fallbackChain": ["gemini-pro", "gpt-4"]
    }
  },
  "integrations": {
    "midstreamer": { "enabled": false },
    "agenticRobotics": { "enabled": false },
    "ruvector": { "enabled": false }
  }
}
```

## Performance Considerations

**Context Caching:**
- Hash-based cache keys (prompt + schema + options)
- LRU eviction strategy
- Configurable TTL
- Optional file persistence

**Memory Management:**
- Streaming for large datasets
- Chunked processing
- Configurable batch sizes
- Memory-efficient formats (JSONL, Parquet)

**Model Selection:**
- Cost-based: Cheapest model that meets requirements
- Performance-based: Fastest response time
- Quality-based: Highest quality output
- Balanced: Optimize all three factors

## Security

**API Key Management:**
- Environment variable loading
- Config file with variable substitution
- Never log sensitive data
- Secure config file patterns

**Data Validation:**
- Input validation (Zod schemas)
- Output validation
- Sanitization
- Rate limiting

## Testing Strategy

**Unit Tests:**
- Component isolation
- Mock dependencies
- Logic correctness

**Integration Tests:**
- Component interactions
- Real dependencies
- Error scenarios

**E2E Tests:**
- Complete workflows
- CLI commands
- Real API calls (test keys)

## Implementation Status

### Completed ✅
- Complete architecture design
- Type system definitions
- Core configuration system
- SDK class structure
- Generator interfaces
- Comprehensive documentation
- Package.json with correct dependencies
- TypeScript configuration
- Directory structure

### Remaining 🔨
- Cache Manager implementation
- Logger implementation
- Generator implementations
- Model provider implementations
- Model router implementation
- Integration adapters
- CLI commands
- Utilities (serialization, prompts)
- Tests
- Examples

## Next Steps for Builder Agent

1. **Start with Core Infrastructure**
   - Implement Cache Manager (`/src/core/Cache.ts`)
   - Implement Logger (`/src/core/Logger.ts`)

2. **Implement Model System**
   - Gemini provider
   - OpenRouter provider
   - Model router

3. **Implement Generator System**
   - Generator Hub
   - TimeSeries, Events, Structured generators

4. **Wire SDK Together**
   - Complete AgenticSynth implementation
   - Add event emitters
   - Add progress tracking

5. **Build CLI**
   - CLI entry point
   - Commands (generate, batch, cache, config)

6. **Add Integrations**
   - Midstreamer adapter
   - AgenticRobotics adapter
   - Ruvector adapter

7. **Testing & Examples**
   - Unit tests
   - Integration tests
   - Example code

## Success Criteria

✅ All TypeScript compiles without errors
✅ `npm run build` succeeds
✅ `npm test` passes all tests
✅ `npx agentic-synth --help` works
✅ Examples run successfully
✅ Documentation is comprehensive
✅ Package ready for npm publish

## Resources

- **Architecture**: `/docs/ARCHITECTURE.md`
- **API Reference**: `/docs/API.md`
- **Integration Guide**: `/docs/INTEGRATION.md`
- **Implementation Plan**: `/docs/IMPLEMENTATION_PLAN.md`
- **Directory Structure**: `/docs/DIRECTORY_STRUCTURE.md`

---

**Architecture design is complete. Ready for builder agent implementation!** 🚀
