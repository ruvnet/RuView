# Psycho-Symbolic Reasoner MCP Integration

A production-ready TypeScript/FastMCP integration layer for the psycho-symbolic-reasoner WASM modules.

## Features

- **Complete WASM Integration**: TypeScript wrappers for all three core modules
  - Graph Reasoner (symbolic reasoning)
  - Text Extractor (sentiment, preferences, emotions)
  - Planner (GOAP action planning)

- **MCP Tool Registration**: 15+ tools for Model Context Protocol
  - `queryGraph` - Query knowledge graph
  - `addFact`, `addRule` - Build knowledge base
  - `extractAffect` - Sentiment analysis
  - `extractPreferences` - Preference extraction
  - `extractEmotions` - Emotion detection
  - `planAction` - GOAP planning
  - And more utility tools

- **Production Features**:
  - Memory management with automatic cleanup
  - Error handling and type safety
  - JSON schema validation
  - Comprehensive test coverage
  - Graceful shutdown handling

## Installation

```bash
npm install
npm run build:wasm-pack  # Build WASM modules
npm run build           # Build TypeScript
```

## Usage

### As MCP Server

```bash
# Start the MCP server
npm start

# Check health status
npm run health

# List available tools
npm run tools
```

### Programmatic Usage

```typescript
import { PsychoSymbolicMcpTools } from './src/index.js';

const tools = new PsychoSymbolicMcpTools();

// Initialize with WASM modules
await tools.initialize({
  graphReasonerWasmPath: './wasm/graph_reasoner.wasm',
  textExtractorWasmPath: './wasm/extractors.wasm',
  plannerWasmPath: './wasm/planner.wasm'
});

// Use tools
const result = await tools.callTool({
  name: 'analyzeText',
  arguments: {
    text: 'I love this product! It makes me happy.',
    includeSentiment: true,
    includeEmotions: true
  }
});
```

## MCP Tools

### Graph Reasoner Tools

- **queryGraph**: Query the knowledge graph using symbolic reasoning
- **addFact**: Add facts to the knowledge base
- **addRule**: Add inference rules
- **runInference**: Run inference to derive new facts

### Text Extractor Tools

- **extractAffect**: Extract sentiment and emotional affect
- **extractPreferences**: Extract user preferences and patterns
- **extractEmotions**: Detect and analyze emotions
- **analyzeText**: Comprehensive text analysis

### Planner Tools

- **planAction**: Create action plans using GOAP
- **addAction**: Add actions to the planner
- **addGoal**: Add goals to the planner
- **setState/getState**: Manage world state

### Utility Tools

- **getMemoryStats**: Get memory usage statistics
- **createInstance**: Create new WASM instances
- **removeInstance**: Remove and cleanup instances

## Examples

### Sentiment Analysis

```json
{
  "name": "extractAffect",
  "arguments": {
    "text": "I absolutely love this new feature!",
    "options": {
      "confidenceThreshold": 0.7
    }
  }
}
```

### Knowledge Graph Query

```json
{
  "name": "queryGraph",
  "arguments": {
    "query": {
      "pattern": {
        "subject": "user",
        "predicate": "likes"
      },
      "options": {
        "limit": 10
      }
    }
  }
}
```

### Action Planning

```json
{
  "name": "planAction",
  "arguments": {
    "targetState": {
      "states": {
        "player_position_x": 10,
        "player_position_y": 5
      }
    },
    "options": {
      "heuristic": "astar",
      "maxDepth": 15
    }
  }
}
```

## Architecture

```
src/
├── index.ts              # Main MCP server
├── tools/                # MCP tool implementations
├── wrappers/             # TypeScript WASM wrappers
│   ├── graph-reasoner.ts
│   ├── text-extractor.ts
│   └── planner.ts
├── wasm/                 # WASM loading and management
│   ├── loader.ts
│   └── memory-manager.ts
├── types/                # TypeScript type definitions
├── schemas/              # Zod validation schemas
└── tests/                # Integration tests
```

## Configuration

Environment variables:

- `WASM_DIR`: Directory containing WASM files
- `LOG_LEVEL`: Logging level (debug, info, warn, error)
- `MAX_INSTANCES`: Maximum WASM instances (default: 100)
- `INSTANCE_TIMEOUT`: Instance timeout in ms (default: 300000)

## Testing

```bash
# Run all tests
npm test

# Run tests with coverage
npm run test:coverage

# Run tests in watch mode
npm run test:watch
```

## Development

```bash
# Development mode with auto-reload
npm run dev

# Build WASM modules
npm run build:wasm-pack

# Type checking
npm run typecheck

# Linting
npm run lint
npm run lint:fix
```

## Memory Management

The integration includes sophisticated memory management:

- **Automatic Cleanup**: Idle instances are automatically freed
- **Instance Pooling**: Reuse instances for better performance
- **Memory Monitoring**: Track memory usage and detect leaks
- **Graceful Shutdown**: Clean up all resources on exit

## Error Handling

Comprehensive error handling with:

- **Type Safety**: Full TypeScript types and Zod validation
- **Error Classification**: Specific error types for different failures
- **Graceful Degradation**: Continue operation when possible
- **Detailed Logging**: Debug information for troubleshooting

## Performance

- **Lazy Loading**: WASM modules loaded on demand
- **Concurrent Operations**: Parallel processing where possible
- **Efficient Memory Usage**: Automatic cleanup and pooling
- **Optimized Builds**: Production builds with size optimization

## Integration with Claude

This MCP server can be used with Claude Desktop or other MCP-compatible tools:

1. Build and start the server
2. Configure Claude Desktop to use this server
3. Use the tools in conversations with Claude

## Contributing

1. Follow TypeScript best practices
2. Add tests for new functionality
3. Update documentation
4. Run linting and type checking
5. Test with actual WASM modules

## License

MIT License - see LICENSE file for details.