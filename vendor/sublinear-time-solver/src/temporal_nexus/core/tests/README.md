# NanosecondScheduler Test Suite

This directory contains a comprehensive test suite for the NanosecondScheduler implementation, focusing on real hardware validation with TSC (Time Stamp Counter) precision timing.

## Test Structure

### Core Test Files

- **`mod.rs`** - Test infrastructure, utilities, and shared data structures
- **`test_main.rs`** - Main test execution entry point and report generation
- **`test_runner.rs`** - Test suite orchestration and execution framework

### Test Categories

1. **`scheduler_tests.rs`** - Core scheduler functionality
   - Scheduler creation and initialization
   - Task scheduling and execution
   - Tick processing and timing
   - MCP integration validation
   - Memory state management

2. **`timing_precision_tests.rs`** - Hardware timing validation
   - TSC timestamp accuracy (nanosecond precision)
   - Timing overhead measurements (<1μs target)
   - Performance under CPU load and memory pressure
   - Scheduler interference testing

3. **`strange_loop_tests.rs`** - Mathematical convergence validation
   - Lipschitz constraint enforcement (< 1.0)
   - Contraction mapping properties
   - Fixed point convergence
   - Self-reference and emergence patterns

4. **`temporal_window_tests.rs`** - Window overlap management
   - Window creation and lifecycle
   - Overlap calculation (50-100% target)
   - Boundary handling and cleanup
   - Performance optimization

5. **`identity_continuity_tests.rs`** - Identity tracking validation
   - Feature extraction consistency
   - Similarity calculation accuracy
   - Continuity break detection
   - Identity drift measurement

6. **`quantum_validation_tests.rs`** - Physics constraint compliance
   - Energy tracking and conservation
   - Margolus-Levitin limit compliance
   - Uncertainty principle validation
   - Coherence preservation testing

7. **`performance_benchmarks.rs`** - Performance validation
   - Tick performance (<1μs overhead target)
   - Throughput testing (>1M ticks/second)
   - Sustained load and burst performance
   - Memory efficiency benchmarks

8. **`edge_case_tests.rs`** - Boundary and error conditions
   - Boundary value testing
   - Error recovery and resilience
   - Resource limit handling
   - Stress testing and failure isolation

9. **`integration_tests.rs`** - End-to-end validation
   - Complete consciousness workflows
   - Component coordination
   - Real-world scenario simulation
   - Long-running session testing

## Running Tests

### Full Test Suite

```bash
# Run all tests with comprehensive reporting
cargo test --package temporal_nexus --lib tests::test_main::run_all_tests

# Run with detailed output
cargo test --package temporal_nexus --lib tests::test_main::run_all_tests -- --nocapture
```

### Individual Test Categories

```bash
# Timing precision tests (requires TSC hardware)
cargo test --package temporal_nexus --lib tests::timing_precision_tests

# Performance benchmarks
cargo test --package temporal_nexus --lib tests::performance_benchmarks

# Strange loop convergence
cargo test --package temporal_nexus --lib tests::strange_loop_tests

# Quantum validation
cargo test --package temporal_nexus --lib tests::quantum_validation_tests
```

### Benchmark Mode

```bash
# Run performance benchmarks only
cargo test --package temporal_nexus --lib tests::test_main::run_benchmark_tests
```

## Hardware Requirements

### TSC Support
- **x86_64 architecture** with Time Stamp Counter support
- **RDTSC instruction** available (standard on modern CPUs)
- **Invariant TSC** recommended for consistent timing

### Performance Validation
- **CPU frequency detection** for accurate timing calculations
- **Minimal system load** for precision measurements
- **Root/admin privileges** may be required for low-level timing

## Test Targets and Validation

### Performance Targets
- **Tick Processing**: <1μs average overhead
- **Throughput**: >1M ticks per second sustained
- **Memory Usage**: <100MB for standard workloads
- **Timing Precision**: Nanosecond accuracy with TSC

### Mathematical Constraints
- **Lipschitz Constant**: < 1.0 for strange loop convergence
- **Window Overlap**: 50-100% for consciousness continuity
- **Identity Similarity**: >0.8 threshold for continuity
- **Quantum Limits**: Margolus-Levitin compliance

### Error Tolerance
- **Timing Jitter**: <10ns standard deviation
- **Convergence**: 99.9% success rate for fixed points
- **Memory Leaks**: Zero tolerance in long-running tests
- **Error Recovery**: 100% success rate for recoverable errors

## Test Report Generation

### Automated Reports
Tests automatically generate comprehensive reports including:
- **Performance metrics** with target validation
- **Hardware information** and TSC frequency
- **Test coverage** by category and assertions
- **Failure analysis** with detailed diagnostics
- **Recommendations** for optimization

### Report Formats
- **Console output** with real-time progress and results
- **JSON files** with detailed metrics and timestamps
- **Benchmark data** for CI/CD integration
- **Performance graphs** (when visualization enabled)

### Sample Report Output
```
🚀 Starting NanosecondScheduler Comprehensive Test Suite
===============================================

📊 COMPREHENSIVE TEST REPORT
============================

🎯 OVERALL SUMMARY
Total Duration: 2847.23ms
Total Tests: 156
Passed: 156 ✅
Failed: 0 ❌
Success Rate: 100.0%

⚡ PERFORMANCE METRICS
Average Tick Time: 0.87μs
Max Tick Time: 1.23μs
Throughput: 1,247,832 ticks/sec
Memory Usage: 23.45 MB
Target (<1μs): ✅ MET

🔍 CRITICAL VALIDATIONS
✅ Timing Precision: TSC-based nanosecond accuracy validated
✅ Strange Loop: Lipschitz < 1 constraint satisfied
✅ Quantum Validation: Physics constraints satisfied
✅ Performance: <1μs overhead target achieved
```

## Integration with CI/CD

### GitHub Actions
```yaml
- name: Run NanosecondScheduler Tests
  run: |
    cargo test --package temporal_nexus --lib tests::test_main::run_all_tests
    cargo test --package temporal_nexus --lib tests::test_main::run_benchmark_tests
```

### Performance Monitoring
- **Benchmark tracking** across commits
- **Performance regression** detection
- **Hardware compatibility** validation
- **Automated alerts** for target violations

## Troubleshooting

### Common Issues

1. **TSC Not Available**
   - Verify x86_64 architecture
   - Check RDTSC instruction support
   - Consider fallback timing methods

2. **Performance Target Failures**
   - Reduce system load during testing
   - Check CPU frequency scaling
   - Verify memory bandwidth

3. **Quantum Validation Errors**
   - Review energy conservation calculations
   - Validate uncertainty principle constraints
   - Check coherence preservation logic

4. **Convergence Failures**
   - Verify Lipschitz constant calculations
   - Check contraction mapping properties
   - Review fixed point algorithms

### Debug Mode
```bash
# Enable debug output
RUST_LOG=debug cargo test --package temporal_nexus --lib tests::test_main::run_all_tests

# Memory debugging
valgrind cargo test --package temporal_nexus --lib tests::performance_benchmarks
```

## Contributing

When adding new tests:
1. Follow the existing test structure and naming conventions
2. Include both positive and negative test cases
3. Add performance benchmarks for new features
4. Update this README with new test categories
5. Ensure hardware independence where possible
6. Add appropriate error handling and cleanup

## Dependencies

### Test Dependencies
- `tokio` - Async runtime for test execution
- `criterion` - Performance benchmarking
- `serde_json` - Report serialization
- `chrono` - Timestamp generation

### Hardware Dependencies
- `std::arch::x86_64` - TSC instruction access
- Platform-specific timing APIs
- CPU performance counters

This test suite provides comprehensive validation of the NanosecondScheduler implementation with real hardware timing precision and mathematical correctness verification.