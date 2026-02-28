# Sparse Inference Engine - Test Suite

Comprehensive test suite for the RuVector sparse inference engine with 78+ tests and 10 benchmarks across 1516 lines of test code.

## Test Structure

### Unit Tests (`tests/unit/`)

**Predictor Tests** (`predictor_tests.rs` - 12 tests)
- Low-rank predictor creation and configuration
- Active neuron prediction validation
- Top-K mode functionality
- Calibration effectiveness
- Input validation and edge cases
- Consistency and determinism

**Sparse FFN Tests** (`sparse_ffn_tests.rs` - 14 tests)
- Sparse vs dense computation equivalence
- Different activation functions (ReLU, GeLU, SiLU)
- SwiGLU paired neuron handling
- Empty and partial activation sets
- Out-of-bounds and duplicate neuron handling
- Deterministic output verification

**Quantization Tests** (`quantization_tests.rs` - 15 tests)
- INT8 quantization roundtrip accuracy
- INT4 compression ratios
- Different group sizes (16, 32, 64, 128)
- Selective row dequantization
- Range preservation
- Uniform and zero value handling
- Odd-length array support

### Integration Tests (`tests/integration/`)

**Model Loading Tests** (`model_loading_tests.rs` - 15 tests)
- GGUF header parsing
- Invalid format detection
- Model structure validation
- Forward pass execution
- Configuration handling
- Multiple model sizes

**Sparse Inference Tests** (`sparse_inference_tests.rs` - 12 tests)
- Full sparse pipeline execution
- Dense vs sparse accuracy comparison
- Batch processing
- Calibration improvements
- Different sparsity levels (10%-90%)
- Consistency verification
- Extreme input handling

### Property-Based Tests (`tests/property/mod.rs` - 10 tests)
Using `proptest` for generative testing:
- Output finiteness invariants
- Valid index generation
- Dense/sparse equivalence
- Quantization ordering preservation
- Top-K constraints
- Dimension correctness
- INT4 roundtrip properties
- Output dimension consistency
- SwiGLU output validation
- Calibration robustness

### Benchmark Tests (`benches/sparse_inference_bench.rs` - 10 benchmarks)

**Performance Comparisons:**
1. **Sparse vs Dense**: Baseline comparison
2. **Sparsity Levels**: 30%, 50%, 70%, 90% sparsity
3. **Predictor Performance**: Prediction latency
4. **Top-K Modes**: K=100, 500, 1000, 2000
5. **Sparse FFN**: Dense vs 10% vs 50% sparse
6. **Activation Functions**: ReLU, GeLU, SiLU comparison
7. **Quantization**: Dequantization of 1, 10, 100 rows
8. **INT4 vs INT8**: Quantization speed and accuracy
9. **Calibration**: Sample sizes 10, 50, 100, 500
10. **SwiGLU**: Dense vs sparse comparison

## Common Test Utilities (`tests/common/mod.rs`)

Helper functions for all tests:
- `random_vector(dim)` - Generate test vectors
- `random_activations(max)` - Generate activation patterns
- `create_test_ffn(input, hidden)` - FFN factory
- `create_calibrated_predictor()` - Pre-calibrated predictor
- `create_quantized_matrix(rows, cols)` - Quantized weights
- `load_test_llama_model()` - Test model loader
- `assert_vectors_close(a, b, tol)` - Approximate equality
- `mse(a, b)` - Mean squared error
- `generate_calibration_data(n)` - Calibration dataset

## Running Tests

```bash
# Run all tests
cargo test -p ruvector-sparse-inference

# Run specific test categories
cargo test -p ruvector-sparse-inference --test unit
cargo test -p ruvector-sparse-inference --test integration
cargo test -p ruvector-sparse-inference --test property

# Run unit tests for a specific module
cargo test -p ruvector-sparse-inference predictor_tests
cargo test -p ruvector-sparse-inference quantization_tests
cargo test -p ruvector-sparse-inference sparse_ffn_tests

# Run benchmarks
cargo bench -p ruvector-sparse-inference

# Run specific benchmark
cargo bench -p ruvector-sparse-inference -- sparse_vs_dense
cargo bench -p ruvector-sparse-inference -- sparsity_levels
cargo bench -p ruvector-sparse-inference -- quantization
```

## Test Coverage Goals

- **Statements**: >80%
- **Branches**: >75%
- **Functions**: >80%
- **Lines**: >80%

## Test Characteristics

Tests follow the **FIRST** principles:
- **Fast**: Unit tests <100ms
- **Isolated**: No dependencies between tests
- **Repeatable**: Same result every time
- **Self-validating**: Clear pass/fail
- **Timely**: Written with implementation

## Property-Based Testing

Tests use `proptest` to verify invariants across wide input ranges:
- Input values: -10.0 to 10.0
- Vector dimensions: 256 to 1024
- Hidden dimensions: 512 to 4096
- Group sizes: 16, 32, 64, 128
- Sample counts: 1 to 100

## Edge Cases Tested

1. **Empty inputs**: Zero-length vectors, no active neurons
2. **Boundary values**: Maximum dimensions, extreme values
3. **Invalid inputs**: Wrong dimensions, out-of-bounds indices
4. **Numerical stability**: Very large/small values, precision loss
5. **Concurrent operations**: Parallel inference requests
6. **Memory efficiency**: Large datasets, quantization compression

## Test Organization

```
tests/
├── common/
│   └── mod.rs                    # Shared test utilities
├── unit/
│   ├── predictor_tests.rs        # Neuron prediction tests
│   ├── sparse_ffn_tests.rs       # Sparse computation tests
│   └── quantization_tests.rs     # Weight compression tests
├── integration/
│   ├── model_loading_tests.rs    # GGUF parsing tests
│   └── sparse_inference_tests.rs # End-to-end pipeline tests
└── property/
    └── mod.rs                     # Property-based tests

benches/
└── sparse_inference_bench.rs      # Performance benchmarks
```

## Future Test Additions

Potential areas for expansion:
1. Stress tests for memory limits
2. Concurrent inference benchmarks
3. Hardware-specific SIMD tests
4. Model-specific accuracy tests
5. Calibration strategy comparisons
6. Cache effectiveness tests
7. Quantization accuracy analysis

---

**Total Test Coverage**: 78+ tests across 1516 lines
- 68 unit/integration tests
- 10 property-based tests
- 10 performance benchmarks
