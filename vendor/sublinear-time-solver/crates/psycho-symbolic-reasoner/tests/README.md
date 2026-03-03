# Psycho-Symbolic Reasoner Test Suite

A comprehensive test suite for the psycho-symbolic-reasoner system, providing >90% code coverage across all components.

## Test Categories

### 1. Unit Tests (Rust)
- **Location**: `../graph_reasoner/tests/`, `../planner/tests/`, `../extractors/tests/`
- **Framework**: Built-in Rust testing
- **Coverage**: Core functionality of each module
- **Run**: `cargo test`

### 2. Integration Tests (TypeScript)
- **Location**: `src/integration/`
- **Framework**: Jest
- **Coverage**: WASM compilation, exports, and TypeScript integration
- **Run**: `npm run test:integration`

### 3. End-to-End Tests
- **Location**: `src/e2e/`
- **Framework**: Jest
- **Coverage**: Complete system workflows and agent interactions
- **Run**: `npm run test:e2e`

### 4. Performance Tests
- **Location**: `src/performance/`
- **Framework**: Jest + Benchmark.js
- **Coverage**: Performance characteristics and scalability
- **Run**: `npm run test:performance`

### 5. CLI Tests
- **Location**: `src/cli/`
- **Framework**: Jest
- **Coverage**: Command-line interface scenarios
- **Run**: `npm run test:cli`

### 6. Memory Leak Tests
- **Location**: `src/memory/`
- **Framework**: Jest
- **Coverage**: Memory management in WASM modules
- **Run**: `npm run test:memory`

### 7. MCP Integration Tests
- **Location**: `src/mcp/`
- **Framework**: Jest
- **Coverage**: MCP tools and mock agent interactions
- **Run**: `npm run test:mcp`

### 8. Regression Tests
- **Location**: `src/regression/`
- **Framework**: Jest
- **Coverage**: Core functionality stability
- **Run**: `npm run test:regression`

### 9. Error Handling Tests
- **Location**: `src/unit/`
- **Framework**: Jest
- **Coverage**: Edge cases and error conditions
- **Run**: `npm run test:unit`

## Quick Start

### Prerequisites
```bash
# Install Node.js dependencies
npm install

# Install Rust toolchain
rustup install stable
rustup target add wasm32-unknown-unknown

# Install wasm-pack (optional, for WASM tests)
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

### Running Tests

```bash
# Run all tests
npm run test:all

# Run specific test suites
npm run test:unit
npm run test:integration
npm run test:e2e

# Run with coverage
npm run test:coverage

# Watch mode for development
npm run test:watch

# Performance benchmarks
npm run test:performance
```

### Rust Tests
```bash
# Run all Rust tests
cd ..
cargo test --workspace

# Run specific module tests
cargo test -p graph_reasoner
cargo test -p planner
cargo test -p extractors

# Run with coverage (requires cargo-tarpaulin)
cargo install cargo-tarpaulin
cargo tarpaulin --workspace --out xml --output-dir tests/coverage
```

## Test Utilities

The test suite includes comprehensive utilities in `src/test-utils/test-helpers.ts`:

### WasmTestManager
Manages WASM module loading and lifecycle for integration tests.

```typescript
const wasmManager = testUtils.wasmManager;
const module = await wasmManager.loadModule('graph_reasoner', wasmPath);
```

### PerformanceCollector
Tracks execution time, memory usage, and operation counts.

```typescript
const collector = testUtils.performanceCollector;
collector.start();
// ... perform operations
const metrics = collector.stop();
```

### MockAgentFactory
Creates and manages mock agents for testing agent interactions.

```typescript
const agent = testUtils.mockAgentFactory.createAgent('agent1', 'researcher');
testUtils.mockAgentFactory.sendMessage('agent1', { type: 'task', data: {} });
```

### MemoryLeakDetector
Monitors memory usage patterns to detect leaks.

```typescript
const detector = testUtils.memoryLeakDetector;
detector.start();
// ... perform operations
const analysis = detector.checkForLeaks();
```

### CLITestRunner
Executes command-line operations with timeout and error handling.

```typescript
const result = await testUtils.cliRunner.runCommand('cargo', ['test']);
```

## Coverage Requirements

The test suite enforces >90% coverage across all metrics:

- **Statements**: 90%
- **Branches**: 90%
- **Functions**: 90%
- **Lines**: 90%

### Viewing Coverage Reports

```bash
# Generate coverage report
npm run test:coverage

# Open HTML report
open coverage/index.html
```

### Coverage Exclusions

The following are excluded from coverage requirements:
- Test files (`*.test.ts`, `*.spec.ts`)
- Test utilities (`src/test-utils/`)
- Setup files (`src/setup/`)
- Generated files (`dist/`, `wasm/`)

## Test Configuration

### Jest Configuration
- **File**: `jest.config.js`
- **Features**: ESM support, TypeScript compilation, parallel execution
- **Projects**: Separate configurations for each test category

### TypeScript Configuration
- **File**: `tsconfig.json`
- **Features**: Path mapping, strict mode, ESNext target

### Coverage Configuration
- **File**: `.nyc_config.json`
- **Features**: TypeScript support, HTML/LCOV reports, threshold enforcement

## Performance Benchmarks

The performance tests establish baselines for:

### Graph Reasoning
- Fact insertion: <1ms per fact
- Simple queries: <10ms per query
- Inference: <100ms per iteration

### Planning
- State space exploration: <100ms for 1000 states
- Plan generation: <1s for complex scenarios

### Text Extraction
- Sentiment analysis: <50ms per text
- Pattern matching: Variable by complexity
- Preference extraction: <30ms per text

### Memory Usage
- Graph operations: <100MB for 10K facts
- Planning: <150MB for complex scenarios
- Text processing: <100MB for 1K analyses

## Continuous Integration

### GitHub Actions Workflow
```yaml
name: Test Suite
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
      - uses: actions-rs/toolchain@v1
      - run: npm ci
      - run: cargo test --workspace
      - run: npm run test:all
      - run: npm run test:coverage
```

### Quality Gates
- All tests must pass
- Coverage must be >90%
- No memory leaks detected
- Performance benchmarks within limits

## Debugging Tests

### Debug Mode
```bash
# Run with debugging
DEBUG=* npm test

# Run specific test with logs
npm test -- --testNamePattern="specific test" --verbose
```

### Memory Debugging
```bash
# Run with memory profiling
node --inspect --max-old-space-size=8192 node_modules/.bin/jest
```

### Performance Profiling
```bash
# Run with performance timing
NODE_ENV=development npm run test:performance
```

## Test Data Management

### Generated Test Data
The test suite uses `TestDataGenerator` to create:
- Graph data with configurable node/edge counts
- Planning scenarios with various complexities
- Text samples for sentiment analysis
- Performance test vectors

### Reproducible Tests
All tests use deterministic data generation with fixed seeds for reproducibility.

## Contributing

### Adding New Tests

1. **Choose the appropriate category** based on test scope
2. **Follow naming conventions**: `*.test.ts` for test files
3. **Use test utilities** for common operations
4. **Include performance assertions** where applicable
5. **Add coverage for edge cases**

### Test Structure
```typescript
describe('Feature Name', () => {
  beforeAll(() => {
    // Global setup
  });

  beforeEach(() => {
    // Test setup
  });

  test('should handle normal case', () => {
    // Test implementation
  });

  test('should handle edge case', () => {
    // Edge case testing
  });

  afterEach(() => {
    // Test cleanup
  });
});
```

### Performance Test Guidelines
- Use `PerformanceCollector` for timing
- Include memory usage assertions
- Test scalability with varying input sizes
- Establish and verify performance baselines

## Troubleshooting

### Common Issues

1. **WASM modules not found**
   ```bash
   npm run build:wasm:all
   ```

2. **TypeScript compilation errors**
   ```bash
   npx tsc --noEmit
   ```

3. **Memory issues in tests**
   ```bash
   node --max-old-space-size=8192 node_modules/.bin/jest
   ```

4. **Test timeouts**
   - Increase timeout in Jest configuration
   - Use `--detectOpenHandles` to find hanging operations

### Performance Issues
- Run tests with `--runInBand` to avoid parallel execution
- Use `--maxWorkers=1` for debugging
- Monitor memory usage with heap snapshots

## License

This test suite is part of the psycho-symbolic-reasoner project and follows the same license terms.