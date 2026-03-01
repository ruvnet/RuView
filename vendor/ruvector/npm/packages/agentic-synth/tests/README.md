# Agentic Synth - Test Suite

Comprehensive test suite for the agentic-synth package with 90%+ code coverage.

## Test Structure

```
tests/
├── unit/                  # Unit tests (isolated component testing)
│   ├── generators/        # Data generator tests
│   ├── api/              # API client tests
│   ├── cache/            # Context cache tests
│   ├── routing/          # Model router tests
│   └── config/           # Configuration tests
├── integration/          # Integration tests
│   ├── midstreamer.test.js    # Midstreamer adapter integration
│   ├── robotics.test.js       # Robotics adapter integration
│   └── ruvector.test.js       # Ruvector adapter integration
├── cli/                  # CLI tests
│   └── cli.test.js       # Command-line interface tests
└── fixtures/             # Test fixtures and sample data
    ├── schemas.js        # Sample data schemas
    └── configs.js        # Sample configurations
```

## Running Tests

### All Tests
```bash
npm test
```

### Watch Mode
```bash
npm run test:watch
```

### Coverage Report
```bash
npm run test:coverage
```

### Specific Test Suites
```bash
# Unit tests only
npm run test:unit

# Integration tests only
npm run test:integration

# CLI tests only
npm run test:cli
```

## Test Coverage Goals

- **Lines**: 90%+
- **Functions**: 90%+
- **Branches**: 85%+
- **Statements**: 90%+

## Unit Tests

### Data Generator (`tests/unit/generators/data-generator.test.js`)
- Constructor with default/custom options
- Data generation with various schemas
- Field generation (strings, numbers, booleans, arrays, vectors)
- Seed-based reproducibility
- Performance benchmarks

### API Client (`tests/unit/api/client.test.js`)
- HTTP request methods (GET, POST)
- Request/response handling
- Error handling and retries
- Timeout handling
- Authorization headers

### Context Cache (`tests/unit/cache/context-cache.test.js`)
- Get/set operations
- TTL (Time To Live) expiration
- LRU (Least Recently Used) eviction
- Cache statistics (hits, misses, hit rate)
- Performance with large datasets

### Model Router (`tests/unit/routing/model-router.test.js`)
- Routing strategies (round-robin, least-latency, cost-optimized, capability-based)
- Model registration
- Performance metrics tracking
- Load balancing

### Config (`tests/unit/config/config.test.js`)
- Configuration loading (JSON, YAML)
- Environment variable support
- Nested configuration access
- Configuration validation

## Integration Tests

### Midstreamer Integration (`tests/integration/midstreamer.test.js`)
- Connection management
- Data streaming workflows
- Error handling
- Performance benchmarks

### Robotics Integration (`tests/integration/robotics.test.js`)
- Adapter initialization
- Command execution
- Status monitoring
- Batch operations

### Ruvector Integration (`tests/integration/ruvector.test.js`)
- Vector insertion
- Similarity search
- Vector retrieval
- Performance with large datasets
- Accuracy validation

## CLI Tests

### Command-Line Interface (`tests/cli/cli.test.js`)
- `generate` command with various options
- `config` command
- `validate` command
- Error handling
- Output formatting

## Test Fixtures

### Schemas (`tests/fixtures/schemas.js`)
- `basicSchema`: Simple data structure
- `complexSchema`: Multi-field schema with metadata
- `vectorSchema`: Vector embeddings for semantic search
- `roboticsSchema`: Robotics command structure
- `streamingSchema`: Event streaming data

### Configurations (`tests/fixtures/configs.js`)
- `defaultConfig`: Default settings
- `productionConfig`: Production-ready configuration
- `testConfig`: Test environment settings
- `minimalConfig`: Minimal required configuration

## Writing New Tests

### Unit Test Template
```javascript
import { describe, it, expect, beforeEach } from 'vitest';
import { YourClass } from '../../../src/path/to/class.js';

describe('YourClass', () => {
  let instance;

  beforeEach(() => {
    instance = new YourClass();
  });

  it('should do something', () => {
    const result = instance.method();
    expect(result).toBeDefined();
  });
});
```

### Integration Test Template
```javascript
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { Adapter } from '../../src/adapters/adapter.js';

describe('Adapter Integration', () => {
  let adapter;

  beforeEach(async () => {
    adapter = new Adapter();
    await adapter.initialize();
  });

  afterEach(async () => {
    await adapter.cleanup();
  });

  it('should perform end-to-end workflow', async () => {
    // Test implementation
  });
});
```

## Best Practices

1. **Isolation**: Each test should be independent
2. **Cleanup**: Always clean up resources in `afterEach`
3. **Mocking**: Mock external dependencies (APIs, file system)
4. **Assertions**: Use clear, specific assertions
5. **Performance**: Include performance benchmarks for critical paths
6. **Edge Cases**: Test boundary conditions and error states

## Performance Benchmarks

Tests include performance benchmarks to ensure:
- Data generation: < 1ms per record
- API requests: < 100ms (mocked)
- Cache operations: < 1ms per operation
- Vector search: < 100ms for 1000 vectors
- CLI operations: < 2 seconds for typical workloads

## Continuous Integration

Tests are designed to run in CI/CD pipelines with:
- Fast execution (< 30 seconds for full suite)
- No external dependencies
- Deterministic results
- Clear failure messages

## Troubleshooting

### Tests Failing Locally
1. Run `npm install` to ensure dependencies are installed
2. Check Node.js version (requires 18+)
3. Clear test cache: `npx vitest --clearCache`

### Coverage Issues
1. Run `npm run test:coverage` to generate detailed report
2. Check `coverage/` directory for HTML report
3. Focus on untested branches and edge cases

### Integration Test Failures
1. Ensure mock services are properly initialized
2. Check for port conflicts
3. Verify cleanup in `afterEach` hooks

## Contributing

When adding new features:
1. Write tests first (TDD approach)
2. Ensure 90%+ coverage for new code
3. Add integration tests for new adapters
4. Update this README with new test sections
