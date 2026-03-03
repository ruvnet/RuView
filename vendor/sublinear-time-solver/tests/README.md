# Sublinear Time Solver - Test Suite

Comprehensive testing framework for the sublinear-time-solver MCP interface project.

## Test Structure

```
tests/
├── README.md                       # This file
├── mcp/                           # MCP tool integration tests
│   └── mcp-tool-tests.js         # Comprehensive MCP solver tool tests
├── rust/                          # Rust implementation tests
│   ├── hybrid_tests.rs           # Hybrid algorithm tests
│   ├── push_tests.rs             # Forward/backward push algorithm tests
│   └── standalone_benchmark.rs   # Performance benchmarks
├── performance/                   # Performance and optimization tests
│   ├── performance-test.js       # General performance tests
│   ├── optimization-benchmark.js # Optimization benchmarks
│   └── test-fast-solver.js       # Fast solver implementation tests
├── validation/                    # Validation and correctness tests
│   └── test-solver-fixes.js      # Solver bug fixes and edge cases
├── convergence/                   # Convergence analysis tests
│   ├── convergence-validation.js # Convergence validation
│   ├── mini-benchmark.js         # Small-scale benchmarks
│   └── quick-test.js             # Quick smoke tests
└── wasm/                          # WebAssembly tests
    ├── wasm_test.js               # WASM module tests
    └── verify-wasm.js             # WASM verification tests
```

## Quick Start

### Run All Tests
```bash
# Run comprehensive test suite with report generation
node tests/run_all.cjs --report

# Run with verbose output
node tests/run_all.cjs --verbose

# Run individual test suites
node tests/unit/matrix.test.cjs
node tests/unit/solver.test.cjs
node tests/integration/cli.test.cjs
node tests/integration/mcp.test.cjs
node tests/integration/wasm.test.cjs
node tests/performance/benchmark.test.cjs
```

### Prerequisites

1. **Node.js 16+** installed
2. **NPM packages** installed (`npm install`)
3. **For full WASM testing** (optional):
   ```bash
   # Install Rust toolchain
   curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Add WASM target
   rustup target add wasm32-unknown-unknown

   # Install wasm-pack
   cargo install wasm-pack

   # Build WASM
   ./scripts/build.sh
   ```

## Test Categories

### 1. Unit Tests (`unit/`)

**Matrix Tests** (`matrix.test.cjs`)
- Matrix constructor validation
- Static methods (zeros, identity, random)
- Access operations (get/set)
- Memory efficiency
- Mathematical properties
- Error handling

**Solver Tests** (`solver.test.cjs`)
- Solver initialization
- Basic solving operations
- Batch processing
- Memory management
- Resource cleanup
- Error classes

### 2. Integration Tests (`integration/`)

**CLI Tests** (`cli.test.cjs`)
- Command parsing
- File format support
- Error handling
- Service mode
- Signal handling

**MCP Tests** (`mcp.test.cjs`)
- Protocol compliance
- Tool definitions
- Resource providers
- JSON-RPC format
- Error responses

**WASM Tests** (`wasm.test.cjs`)
- Package structure
- JavaScript wrapper
- Performance testing
- Memory management
- Resource cleanup

### 3. Performance Tests (`performance/`)

**Benchmark Tests** (`benchmark.test.cjs`)
- Algorithm correctness
- Convergence analysis
- Scaling performance
- Memory efficiency
- Numerical stability
- Complexity validation

## Test Output

Each test suite provides:
- ✅/❌ Individual test results
- Execution duration
- Detailed error messages (with `--verbose`)
- Summary statistics
- Performance metrics

## Reports

The comprehensive test runner generates:
- **JSON Report** (`test_report.json`) - Machine-readable results
- **Markdown Report** (`TEST_REPORT.md`) - Human-readable analysis
- **Benchmark Report** (`benchmark_report.json`) - Performance data

## Mock Testing

Tests are designed to work with or without WASM build:
- **With WASM**: Full integration testing
- **Without WASM**: Mock interface testing
- **Benefits**: CI/CD friendly, fast execution, contract validation

## Test Development

### Adding New Tests

1. **Unit Tests**: Add to appropriate `unit/*.test.cjs` file
2. **Integration Tests**: Create new file in `integration/`
3. **Performance Tests**: Add to `performance/benchmark.test.cjs`

### Test Structure
```javascript
const runner = new TestRunner();

runner.test('Test description', async () => {
    // Test implementation
    assert.ok(condition, 'Error message');
});

runner.run().then(success => {
    process.exit(success ? 0 : 1);
});
```

## Continuous Integration

### GitHub Actions Example
```yaml
name: Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      - run: npm install
      - run: node tests/run_all.cjs --report
      - uses: actions/upload-artifact@v3
        with:
          name: test-reports
          path: |
            test_report.json
            TEST_REPORT.md
            benchmark_report.json
```

## Troubleshooting

### Common Issues

1. **ES Module Errors**
   - Tests use `.cjs` extension for CommonJS compatibility
   - Project uses ES modules (`"type": "module"` in package.json)

2. **WASM Not Built**
   - WASM tests will run with mock implementations
   - Build WASM for full testing capabilities

3. **Missing Dependencies**
   - Run `npm install` to install required packages
   - Check Node.js version (16+ required)

### Debug Mode
```bash
# Run with debug output
node tests/run_all.cjs --verbose

# Run individual test with stack traces
node tests/unit/matrix.test.cjs --verbose
```

## Performance Benchmarking

The benchmark suite validates:
- Algorithm correctness against known solutions
- Convergence rate analysis
- Memory usage patterns
- Scaling behavior
- Numerical stability

### Benchmark Metrics
- Execution time
- Memory usage
- Iteration counts
- Convergence rates
- Error rates

## Contributing

When adding new functionality:
1. Write tests first (TDD approach)
2. Ensure both mock and real implementations work
3. Add performance benchmarks for algorithms
4. Update test documentation
5. Run full test suite before committing

## Support

For test-related issues:
1. Check this README
2. Review test output and error messages
3. Run with `--verbose` for detailed diagnostics
4. Check the generated test reports

---

**Framework Version:** 1.0.0
**Last Updated:** 2025-09-19
**Compatibility:** Node.js 16+, CommonJS/ES Module hybrid