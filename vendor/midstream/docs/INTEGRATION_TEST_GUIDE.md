# MidStream Integration Test Guide

## Overview

This guide describes the comprehensive integration test suite for the MidStream system, covering end-to-end workflows across all published crates.

## Test Files

### 1. `/tests/integration_tests.rs` - Native Integration Tests

Comprehensive integration tests for native (non-WASM) environments covering:

#### Test 1: Scheduler + Temporal Compare Integration
- **Purpose**: Pattern-based task prioritization
- **Scenario**:
  - Historical execution patterns are analyzed using temporal comparison
  - Tasks are scheduled with priority based on pattern similarity
  - Verification of correct priority assignment and task retrieval
- **Coverage**: Cross-crate integration between `nanosecond-scheduler` and `temporal-compare`

#### Test 2: Scheduler + Attractor Analysis Integration
- **Purpose**: Dynamics-aware scheduling
- **Scenario**:
  - System behavior is tracked during task scheduling
  - Attractor analysis detects stability patterns
  - Scheduling adapts based on behavioral dynamics
- **Coverage**: Integration of `nanosecond-scheduler` with `temporal-attractor-studio`

#### Test 3: Attractor + Neural Solver Integration
- **Purpose**: Behavioral verification
- **Scenario**:
  - Detect limit cycle behavior in system dynamics
  - Verify temporal properties (boundedness, periodicity)
  - Ensure attractor classification matches temporal invariants
- **Coverage**: Deep integration between `temporal-attractor-studio` and `temporal-neural-solver`

#### Test 4: Temporal Compare + Neural Solver Integration
- **Purpose**: Pattern property verification
- **Scenario**:
  - Pattern matching identifies sequence similarities
  - Temporal logic verifies safety properties
  - Correlation between pattern similarity and verification confidence
- **Coverage**: Integration of `temporal-compare` with `temporal-neural-solver`

#### Test 5: Full System with Strange Loop
- **Purpose**: Meta-learning from complete workflows
- **Scenario**:
  - Multi-level meta-learning (Level 0, 1, 2)
  - Integration of all crates in hierarchical analysis
  - Behavioral dynamics analysis and verification
- **Coverage**: Complete system integration including `strange-loop`

#### Test 6: Error Propagation
- **Purpose**: Robust error handling
- **Scenarios**:
  - Dimension mismatch in attractor analysis
  - Insufficient data for analysis
  - Empty trace in temporal solver
  - Queue overflow in scheduler
  - Max depth exceeded in strange loop
  - Sequence length validation
- **Coverage**: Error handling across all crates

#### Test 7: Performance and Scalability
- **Purpose**: Performance validation
- **Metrics**:
  - Scheduler: 1000 tasks, <100ms total
  - Temporal Compare: Cache effectiveness
  - Attractor: 1000 points analysis
- **Coverage**: Performance characteristics of all crates

#### Test 8: Pattern Detection Pipeline
- **Purpose**: Real-world pattern detection
- **Scenario**:
  - Time series pattern matching
  - Repeating pattern detection
  - Pattern similarity scoring
- **Coverage**: `temporal-compare` advanced features

#### Test 9: State Management and Recovery
- **Purpose**: State lifecycle management
- **Scenarios**:
  - Clear operations for all components
  - Reset functionality
  - Memory leak prevention
- **Coverage**: State management across all crates

#### Test 10: Deadline and Priority Handling
- **Purpose**: Real-time scheduling validation
- **Scenarios**:
  - Multi-priority task scheduling
  - Execution order verification
  - Deadline miss detection
  - Scheduler lifecycle management
- **Coverage**: `nanosecond-scheduler` advanced features

### 2. `/tests/wasm_integration_test.rs` - WASM Integration Tests

WASM-specific integration tests for browser and WASM environments:

#### Test 1: WASM Temporal Comparison
- Basic temporal pattern matching in WASM environment
- Browser console logging

#### Test 2: WASM Scheduler
- Async task scheduling in WASM
- Statistics verification

#### Test 3: WASM Attractor Analysis
- Phase space analysis in browser
- Confidence scoring

#### Test 4: WASM Temporal Verification
- Temporal logic verification in WASM
- Safety property checking

#### Test 5: WASM Meta-Learning
- Strange loop operation in browser environment
- Pattern learning verification

#### Test 6: WASM Memory Limits
- Memory-constrained operation
- Limited allocation testing

#### Test 7: WASM Performance
- Browser performance API integration
- Timing validation (<5s for 500-element comparison)

#### Test 8: WASM Concurrent Operations
- Browser-based concurrency
- spawn_local integration
- Atomic counter verification

#### Test 9: WASM Error Handling
- Error propagation in WASM
- Browser-compatible error handling

#### Test 10: WASM Integration Workflow
- Complete workflow in browser
- Pattern detection → scheduling → verification

## Running Tests

### Native Tests

```bash
# Run all integration tests
cargo test --test integration_tests

# Run specific test
cargo test --test integration_tests test_scheduler_temporal_integration

# Run with output
cargo test --test integration_tests -- --nocapture

# Run with parallel execution disabled
cargo test --test integration_tests -- --test-threads=1
```

### WASM Tests

```bash
# Install wasm-pack if not already installed
cargo install wasm-pack

# Run WASM tests in headless browser
wasm-pack test --headless --chrome tests/wasm_integration_test.rs

# Run in Firefox
wasm-pack test --headless --firefox tests/wasm_integration_test.rs

# Run in actual browser (opens browser window)
wasm-pack test --chrome tests/wasm_integration_test.rs
```

## Test Coverage

### Cross-Crate Integration
- ✅ Scheduler + Temporal Compare
- ✅ Scheduler + Attractor Studio
- ✅ Attractor Studio + Neural Solver
- ✅ Temporal Compare + Neural Solver
- ✅ Full system with Strange Loop

### Functional Areas
- ✅ Pattern detection and matching
- ✅ Real-time scheduling
- ✅ Behavioral dynamics analysis
- ✅ Temporal logic verification
- ✅ Meta-learning and self-reference
- ✅ Error handling and propagation
- ✅ State management
- ✅ Performance validation

### Scenarios
- ✅ End-to-end workflows
- ✅ Error conditions
- ✅ Performance under load
- ✅ Concurrent operations
- ✅ State recovery
- ✅ WASM compatibility

## Expected Results

All tests should pass with the following characteristics:

### Performance Metrics
- **Scheduler**: <100ms for 1000 tasks
- **Temporal Compare**: Cache hit rate >50% on repeated comparisons
- **Attractor Analysis**: <1s for 1000 points
- **WASM Operations**: <5s for 500-element comparisons

### Quality Metrics
- **Code Coverage**: >80% across all crates
- **Error Detection**: 100% of invalid inputs rejected
- **Memory Safety**: Zero memory leaks
- **Thread Safety**: Safe concurrent access

## CI/CD Integration

### GitHub Actions Workflow

```yaml
name: Integration Tests

on: [push, pull_request]

jobs:
  test-native:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test --test integration_tests

  test-wasm:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: wasm32-unknown-unknown
      - run: cargo install wasm-pack
      - run: wasm-pack test --headless --chrome tests/wasm_integration_test.rs
```

## Troubleshooting

### Common Issues

#### Test Timeout
```bash
# Increase timeout for slow tests
cargo test --test integration_tests -- --test-threads=1 --timeout 300
```

#### WASM Browser Not Found
```bash
# Install Chrome/Firefox drivers
# Ubuntu/Debian:
sudo apt-get install chromium-chromedriver firefox-geckodriver

# macOS:
brew install chromedriver geckodriver
```

#### Cache Issues
```bash
# Clear cargo cache
cargo clean

# Rebuild tests
cargo test --test integration_tests --no-fail-fast
```

## Extending Tests

### Adding New Integration Tests

1. **Identify integration points** between crates
2. **Create test scenario** covering real-world use case
3. **Add test function** to appropriate file
4. **Document test** in this guide
5. **Update CI/CD** configuration if needed

### Test Template

```rust
/// Test N: [Test Name]
///
/// Scenario:
/// - [What this test does]
/// - [Integration points]
/// - [Expected outcomes]
#[test]
fn test_new_integration() {
    println!("\n=== Test N: [Test Name] ===");

    // Setup
    let component1 = Component1::new();
    let component2 = Component2::new();

    // Execute
    let result = component1.interact_with(&component2);

    // Verify
    assert!(result.is_ok());
    println!("  ✓ Integration verified");

    println!("=== Test N PASSED ===\n");
}
```

## Performance Benchmarking

Integration tests also serve as performance benchmarks. Key metrics:

```rust
use std::time::Instant;

let start = Instant::now();
// ... test code ...
let duration = start.elapsed();

assert!(duration.as_millis() < expected_ms, "Performance regression");
```

## Test Data

Tests use synthetic data representative of real-world scenarios:

- **Temporal sequences**: Workflow steps, state transitions
- **Phase space points**: System dynamics, behavioral patterns
- **Temporal traces**: State evolution, property verification
- **Meta-patterns**: Learning hierarchies, self-reference

## Reporting

Test results are formatted for easy reading:

```
╔═══════════════════════════════════════════════════════════════╗
║     MidStream Integration Test Suite                         ║
╠═══════════════════════════════════════════════════════════════╣
║                                                               ║
║  ✓ Test 1: Scheduler + Temporal Compare                      ║
║  ✓ Test 2: Scheduler + Attractor Analysis                    ║
...
╚═══════════════════════════════════════════════════════════════╝
```

## Contributing

When adding new integration tests:

1. Follow existing patterns and naming conventions
2. Add comprehensive documentation
3. Include performance assertions
4. Update this guide
5. Ensure WASM compatibility where applicable
6. Add to CI/CD pipeline

## License

Same as MidStream project (MIT OR Apache-2.0)
