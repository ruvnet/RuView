# Directory Structure

Complete directory structure for agentic-synth package.

```
packages/agentic-synth/
├── src/
│   ├── index.ts                    # Main SDK entry point
│   ├── types/
│   │   └── index.ts                # Core type definitions
│   │
│   ├── sdk/
│   │   ├── AgenticSynth.ts         # Main SDK class
│   │   └── index.ts                # SDK exports
│   │
│   ├── core/
│   │   ├── Config.ts               # Configuration management
│   │   ├── Cache.ts                # Cache manager (LRU, no Redis)
│   │   ├── Logger.ts               # Logging system
│   │   └── index.ts
│   │
│   ├── generators/
│   │   ├── base.ts                 # Base generator interface
│   │   ├── Hub.ts                  # Generator registry
│   │   ├── TimeSeries.ts           # Time-series generator
│   │   ├── Events.ts               # Event generator
│   │   ├── Structured.ts           # Structured data generator
│   │   └── index.ts
│   │
│   ├── models/
│   │   ├── base.ts                 # Model provider interface
│   │   ├── Router.ts               # Model routing logic
│   │   ├── providers/
│   │   │   ├── Gemini.ts          # Gemini API provider
│   │   │   ├── OpenRouter.ts      # OpenRouter API provider
│   │   │   └── index.ts
│   │   └── index.ts
│   │
│   ├── integrations/
│   │   ├── Manager.ts              # Integration manager
│   │   ├── base.ts                 # Integration adapter interface
│   │   ├── Midstreamer.ts          # Midstreamer adapter
│   │   ├── AgenticRobotics.ts      # Agentic-Robotics adapter
│   │   ├── Ruvector.ts             # Ruvector adapter
│   │   └── index.ts
│   │
│   ├── bin/
│   │   ├── cli.ts                  # CLI entry point
│   │   ├── commands/
│   │   │   ├── generate.ts        # Generate command
│   │   │   ├── batch.ts           # Batch command
│   │   │   ├── cache.ts           # Cache management
│   │   │   ├── config.ts          # Config management
│   │   │   └── index.ts
│   │   └── index.ts
│   │
│   └── utils/
│       ├── validation.ts           # Validation helpers
│       ├── serialization.ts        # Serialization helpers
│       ├── prompts.ts              # AI prompt templates
│       └── index.ts
│
├── tests/
│   ├── unit/
│   │   ├── generators/
│   │   │   ├── TimeSeries.test.ts
│   │   │   ├── Events.test.ts
│   │   │   └── Structured.test.ts
│   │   ├── models/
│   │   │   └── Router.test.ts
│   │   ├── core/
│   │   │   ├── Cache.test.ts
│   │   │   └── Config.test.ts
│   │   └── sdk/
│   │       └── AgenticSynth.test.ts
│   │
│   ├── integration/
│   │   ├── e2e.test.ts
│   │   ├── midstreamer.test.ts
│   │   ├── robotics.test.ts
│   │   └── ruvector.test.ts
│   │
│   └── fixtures/
│       ├── schemas/
│       │   ├── timeseries.json
│       │   ├── events.json
│       │   └── structured.json
│       └── configs/
│           └── test-config.json
│
├── examples/
│   ├── basic/
│   │   ├── timeseries.ts
│   │   ├── events.ts
│   │   └── structured.ts
│   │
│   ├── integrations/
│   │   ├── midstreamer-pipeline.ts
│   │   ├── robotics-workflow.ts
│   │   ├── ruvector-search.ts
│   │   └── full-integration.ts
│   │
│   ├── advanced/
│   │   ├── custom-generator.ts
│   │   ├── model-routing.ts
│   │   └── batch-generation.ts
│   │
│   └── cli/
│       ├── basic-usage.sh
│       ├── batch-config.yaml
│       └── advanced-usage.sh
│
├── docs/
│   ├── ARCHITECTURE.md             # Architecture documentation
│   ├── API.md                      # API reference
│   ├── INTEGRATION.md              # Integration guide
│   ├── DIRECTORY_STRUCTURE.md      # This file
│   └── DEVELOPMENT.md              # Development guide
│
├── config/
│   ├── .agentic-synth.example.json # Example config file
│   └── schemas/
│       ├── config.schema.json     # Config JSON schema
│       └── generation.schema.json # Generation options schema
│
├── bin/
│   └── cli.js                      # Compiled CLI entry (after build)
│
├── dist/                           # Compiled output (generated)
│   ├── index.js
│   ├── index.d.ts
│   └── ...
│
├── package.json
├── tsconfig.json
├── .eslintrc.json
├── .prettierrc
├── .gitignore
├── README.md
├── LICENSE
└── CHANGELOG.md
```

## Key Directories

### `/src`

Source code directory containing all TypeScript files.

**Subdirectories:**
- `sdk/` - Main SDK implementation
- `core/` - Core utilities (config, cache, logger)
- `generators/` - Data generation logic
- `models/` - AI model integrations
- `integrations/` - External tool adapters
- `bin/` - CLI implementation
- `utils/` - Helper functions
- `types/` - TypeScript type definitions

### `/tests`

Test files using Vitest framework.

**Subdirectories:**
- `unit/` - Unit tests for individual modules
- `integration/` - Integration tests with external services
- `fixtures/` - Test data and configurations

### `/examples`

Example code demonstrating usage patterns.

**Subdirectories:**
- `basic/` - Simple usage examples
- `integrations/` - Integration examples
- `advanced/` - Advanced patterns
- `cli/` - CLI usage examples

### `/docs`

Documentation files.

**Files:**
- `ARCHITECTURE.md` - System architecture and ADRs
- `API.md` - Complete API reference
- `INTEGRATION.md` - Integration guide
- `DEVELOPMENT.md` - Development guide

### `/config`

Configuration files and schemas.

### `/dist`

Compiled JavaScript output (generated by TypeScript compiler).

## Module Organization

### Core Module (`src/core/`)

Provides foundational functionality:
- Configuration loading and management
- Caching without Redis
- Logging system
- Error handling

### Generator Module (`src/generators/`)

Implements data generation:
- Base generator interface
- Generator registry (Hub)
- Built-in generators (TimeSeries, Events, Structured)
- Custom generator support

### Model Module (`src/models/`)

AI model integration:
- Provider interface
- Model router with fallback
- Gemini integration
- OpenRouter integration
- Cost calculation

### Integration Module (`src/integrations/`)

Optional external integrations:
- Integration manager
- Midstreamer adapter
- Agentic-Robotics adapter
- Ruvector adapter
- Custom integration support

### SDK Module (`src/sdk/`)

Public SDK interface:
- `AgenticSynth` main class
- High-level API methods
- Integration coordination

### CLI Module (`src/bin/`)

Command-line interface:
- CLI entry point
- Command implementations
- Argument parsing
- Output formatting

### Utils Module (`src/utils/`)

Utility functions:
- Validation helpers
- Serialization (JSON, CSV, Parquet)
- Prompt templates
- Common helpers

## File Naming Conventions

- **PascalCase**: Classes and main modules (`AgenticSynth.ts`, `ModelRouter.ts`)
- **camelCase**: Utility files (`validation.ts`, `prompts.ts`)
- **lowercase**: Base interfaces and types (`base.ts`, `index.ts`)
- **kebab-case**: Config files (`.agentic-synth.json`)

## Import/Export Pattern

Each directory has an `index.ts` that exports public APIs:

```typescript
// src/generators/index.ts
export { Generator, BaseGenerator } from './base.js';
export { GeneratorHub } from './Hub.js';
export { TimeSeriesGenerator } from './TimeSeries.js';
export { EventGenerator } from './Events.js';
export { StructuredGenerator } from './Structured.js';
```

## Build Output Structure

After `npm run build`, the `dist/` directory mirrors `src/`:

```
dist/
├── index.js
├── index.d.ts
├── sdk/
│   ├── AgenticSynth.js
│   └── AgenticSynth.d.ts
├── generators/
│   ├── base.js
│   ├── base.d.ts
│   └── ...
└── ...
```

## Package Exports

`package.json` defines multiple entry points:

```json
{
  "exports": {
    ".": "./dist/index.js",
    "./sdk": "./dist/sdk/index.js",
    "./generators": "./dist/generators/index.js",
    "./integrations": "./dist/integrations/index.js"
  }
}
```

## Development Workflow

1. **Source files** in `src/` (TypeScript)
2. **Build** with `tsc` → outputs to `dist/`
3. **Test** with `vitest` → runs from `tests/`
4. **Examples** in `examples/` → use built SDK
5. **Documentation** in `docs/` → reference for users

## Future Additions

Planned additions to directory structure:

- `src/plugins/` - Plugin system for custom generators
- `src/middleware/` - Middleware for request/response processing
- `benchmarks/` - Performance benchmarks
- `scripts/` - Build and deployment scripts
- `.github/` - GitHub Actions workflows

---

This structure provides:
- ✅ Clear separation of concerns
- ✅ Modular architecture
- ✅ Easy to navigate and maintain
- ✅ Scalable for future additions
- ✅ Standard TypeScript/Node.js patterns
