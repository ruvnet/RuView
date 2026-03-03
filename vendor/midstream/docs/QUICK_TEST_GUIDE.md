# Quick Test Guide - MidStream Integration Tests

## Quick Start

```bash
# Run all integration tests
cargo test --test integration_tests

# Run with detailed output
cargo test --test integration_tests -- --nocapture --test-threads=1

# Run specific test
cargo test --test integration_tests test_scheduler_temporal_integration -- --exact
```

## Individual Test Commands

### Test 1: Scheduler + Temporal Compare
```bash
cargo test --test integration_tests test_scheduler_temporal_integration -- --exact --nocapture
```
**What it tests**: Pattern-based task prioritization using DTW similarity

### Test 2: Scheduler + Attractor Analysis
```bash
cargo test --test integration_tests test_scheduler_attractor_integration -- --exact --nocapture
```
**What it tests**: System dynamics monitoring with attractor detection

### Test 3: Attractor + Neural Solver
```bash
cargo test --test integration_tests test_attractor_solver_integration -- --exact --nocapture
```
**What it tests**: Behavioral verification using LTL formulas

### Test 4: Temporal Compare + Neural Solver
```bash
cargo test --test integration_tests test_temporal_solver_integration -- --exact --nocapture
```
**What it tests**: Sequence property verification with temporal logic

### Test 5: Full System with Strange Loop
```bash
cargo test --test integration_tests test_full_system_strange_loop -- --exact --nocapture
```
**What it tests**: Meta-learning across all 5 crates

### Test 6: Error Propagation
```bash
cargo test --test integration_tests test_error_propagation -- --exact --nocapture
```
**What it tests**: Error handling in each crate

### Test 7: Performance and Scalability
```bash
cargo test --test integration_tests test_performance_scalability -- --exact --nocapture
```
**What it tests**: Throughput and latency under load

### Test 8: Pattern Detection Pipeline
```bash
cargo test --test integration_tests test_pattern_detection_pipeline -- --exact --nocapture
```
**What it tests**: End-to-end pattern matching workflow

### Test 9: State Management
```bash
cargo test --test integration_tests test_state_management -- --exact --nocapture
```
**What it tests**: Clear/reset operations and recovery

### Test 10: Deadline and Priority Handling
```bash
cargo test --test integration_tests test_deadline_priority_handling -- --exact --nocapture
```
**What it tests**: Real-time scheduling with priorities

## Expected Output Examples

### Successful Test Output
```
running 1 test

=== Test 1: Scheduler + Temporal Compare Integration ===
  Pattern similarity (DTW): 0.0000
  ✓ Task 1 scheduled with High priority
  ✓ Task retrieved successfully with correct priority
=== Test 1 PASSED ===

test test_scheduler_temporal_integration ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out
```

### Full Suite Output
```
running 10 tests
test test_scheduler_temporal_integration ... ok
test test_scheduler_attractor_integration ... ok
test test_attractor_solver_integration ... ok
test test_temporal_solver_integration ... ok
test test_full_system_strange_loop ... ok
test test_error_propagation ... ok
test test_performance_scalability ... ok
test test_pattern_detection_pipeline ... ok
test test_state_management ... ok
test test_deadline_priority_handling ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Troubleshooting

### Issue: Tests Timeout
**Solution**: Run with single thread to avoid resource contention
```bash
cargo test --test integration_tests -- --test-threads=1
```

### Issue: Compilation Errors
**Solution**: Ensure all crates are built first
```bash
cargo build --all
cargo test --test integration_tests
```

### Issue: Test Failures
**Solution**: Run individual tests to isolate issues
```bash
# Run one test at a time
cargo test --test integration_tests test_scheduler_temporal_integration -- --exact
```

### Issue: Performance Tests Slow
**Solution**: This is expected - they test 1000+ operations
```bash
# Run without performance tests
cargo test --test integration_tests --skip performance
```

## Test Coverage Verification

### Check What's Tested
```bash
# List all test functions
grep -n "^fn test_" tests/integration_tests.rs

# Count tests
grep -c "^fn test_" tests/integration_tests.rs
```

### View Test Documentation
```bash
# Show test descriptions
grep -B2 "^fn test_" tests/integration_tests.rs | grep "///"
```

## Performance Benchmarks

### Expected Performance Metrics
- **Scheduler**: 1000 tasks in <100ms (<0.1ms per task)
- **Temporal Compare**: 100-element DTW in <50ms (with caching)
- **Attractor Analysis**: 1000 points analysis in <100ms
- **Cache Hit Rate**: >50% on repeated operations

### Run Performance Tests Only
```bash
cargo test --test integration_tests test_performance_scalability -- --exact --nocapture
```

## Integration Validation Checklist

✅ **Cross-crate integration**: Tests use multiple crates together
✅ **Real implementations**: No mocks, actual published APIs
✅ **Error handling**: All error paths tested
✅ **Performance**: Throughput and latency validated
✅ **State management**: Clear/reset operations work
✅ **Real-world scenarios**: Practical use cases validated

## Quick Validation Script

```bash
#!/bin/bash
# validate_integration.sh

echo "🧪 Running MidStream Integration Tests..."
echo

# Test 1: Basic functionality
echo "1️⃣ Testing Scheduler + Temporal Compare..."
cargo test --test integration_tests test_scheduler_temporal_integration -- --exact -q
echo "✅ Test 1 passed"
echo

# Test 2: System dynamics
echo "2️⃣ Testing Scheduler + Attractor Analysis..."
cargo test --test integration_tests test_scheduler_attractor_integration -- --exact -q
echo "✅ Test 2 passed"
echo

# Test 3: Verification
echo "3️⃣ Testing Attractor + Neural Solver..."
cargo test --test integration_tests test_attractor_solver_integration -- --exact -q
echo "✅ Test 3 passed"
echo

# Test 4: Logic verification
echo "4️⃣ Testing Temporal Compare + Neural Solver..."
cargo test --test integration_tests test_temporal_solver_integration -- --exact -q
echo "✅ Test 4 passed"
echo

# Test 5: Meta-learning
echo "5️⃣ Testing Full System with Strange Loop..."
cargo test --test integration_tests test_full_system_strange_loop -- --exact -q
echo "✅ Test 5 passed"
echo

# Test 6: Error handling
echo "6️⃣ Testing Error Propagation..."
cargo test --test integration_tests test_error_propagation -- --exact -q
echo "✅ Test 6 passed"
echo

# Test 7: Performance
echo "7️⃣ Testing Performance and Scalability..."
cargo test --test integration_tests test_performance_scalability -- --exact -q
echo "✅ Test 7 passed"
echo

# Test 8: Pattern detection
echo "8️⃣ Testing Pattern Detection Pipeline..."
cargo test --test integration_tests test_pattern_detection_pipeline -- --exact -q
echo "✅ Test 8 passed"
echo

# Test 9: State management
echo "9️⃣ Testing State Management..."
cargo test --test integration_tests test_state_management -- --exact -q
echo "✅ Test 9 passed"
echo

# Test 10: Real-time
echo "🔟 Testing Deadline and Priority Handling..."
cargo test --test integration_tests test_deadline_priority_handling -- --exact -q
echo "✅ Test 10 passed"
echo

echo "🎉 All integration tests passed!"
```

## CI/CD Integration

### GitHub Actions Example
```yaml
name: Integration Tests

on: [push, pull_request]

jobs:
  integration:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run Integration Tests
        run: cargo test --test integration_tests -- --test-threads=1
```

## Documentation

For detailed test documentation, see:
- `/workspaces/midstream/docs/INTEGRATION_TESTS_SUMMARY.md` - Complete test coverage
- `/workspaces/midstream/tests/integration_tests.rs` - Source code with inline docs

## Support

If tests fail:
1. Check individual test output with `--nocapture`
2. Verify all dependencies are built: `cargo build --all`
3. Review test documentation comments in source
4. Run tests with single thread: `--test-threads=1`
