# Ruvector-Scipix Integration Tests

Comprehensive integration test suite for the scipix OCR system.

## Test Structure

### Integration Tests (`integration/`)

1. **pipeline_tests.rs** (9,284 bytes)
   - Full pipeline tests: Image → Preprocess → OCR → Output
   - Multiple input formats (PNG, JPEG, WebP)
   - Multiple output formats (LaTeX, MathML, HTML, ASCII)
   - Error propagation and timeout handling
   - Batch processing and caching

2. **api_tests.rs** (2,100 bytes)
   - POST /v3/text with file upload
   - POST /v3/text with base64
   - POST /v3/text with URL
   - Rate limiting behavior
   - Authentication validation
   - Error response formats
   - Concurrent request handling

3. **cli_tests.rs** (6,226 bytes)
   - `ocr` command with file
   - `batch` command with directory
   - `serve` command startup
   - `config` command
   - Exit codes and error handling
   - Output format options

4. **cache_tests.rs** (10,907 bytes)
   - Cache hit/miss behavior
   - Similarity-based lookup
   - Cache eviction policies
   - Persistence across restarts
   - TTL expiration
   - Concurrent cache access

5. **accuracy_tests.rs** (11,864 bytes)
   - Im2latex-100k sample subset
   - CER (Character Error Rate) calculation
   - WER (Word Error Rate) calculation
   - BLEU score measurement
   - Regression detection
   - Confidence calibration

6. **performance_tests.rs** (10,638 bytes)
   - Latency within bounds (<100ms)
   - Memory usage limits
   - Memory leak detection
   - Throughput targets
   - Latency percentiles (P50, P95, P99)
   - Concurrent throughput

### Common Utilities (`common/`)

1. **server.rs** (6,700 bytes)
   - TestServer setup and teardown
   - Configuration management
   - Mock server implementation
   - Process management

2. **images.rs** (4,000 bytes)
   - Test image generation
   - Equation rendering
   - Fraction and symbol generation
   - Noise and variation injection

3. **latex.rs** (5,900 bytes)
   - LaTeX normalization
   - Expression comparison
   - Similarity calculation
   - Command extraction
   - Syntax validation

4. **metrics.rs** (6,000 bytes)
   - CER calculation
   - WER calculation
   - BLEU score
   - Precision/Recall/F1
   - Levenshtein distance

## Running Tests

### Run All Integration Tests
```bash
cargo test --test '*' --all-features
```

### Run Specific Test Suite
```bash
# Pipeline tests
cargo test --test integration::pipeline_tests

# API tests
cargo test --test integration::api_tests

# CLI tests
cargo test --test integration::cli_tests

# Cache tests
cargo test --test integration::cache_tests

# Accuracy tests
cargo test --test integration::accuracy_tests

# Performance tests
cargo test --test integration::performance_tests
```

### Run with Logging
```bash
RUST_LOG=debug cargo test --test '*' -- --nocapture
```

### Run Specific Test
```bash
cargo test test_pipeline_png_to_latex
```

## Test Dependencies

Add to `Cargo.toml`:

```toml
[dev-dependencies]
tokio = { version = "1", features = ["full"] }
tokio-test = "0.4"
reqwest = { version = "0.11", features = ["json", "multipart"] }
assert_cmd = "2.0"
predicates = "3.0"
serde_json = "1.0"
image = "0.24"
imageproc = "0.23"
rusttype = "0.9"
rand = "0.8"
futures = "0.3"
base64 = "0.21"
env_logger = "0.10"
```

## Test Data

Test images are generated programmatically or stored in:
- `/tmp/scipix_test/` - Generated test images
- `/tmp/scipix_cache/` - Cache testing
- `/tmp/scipix_results/` - Test results

## Metrics and Thresholds

### Accuracy
- Average CER: <0.03
- Average BLEU: >80.0
- Fraction accuracy: >85%
- Symbol accuracy: >80%

### Performance
- Simple equation latency: <100ms
- P50 latency: <100ms
- P95 latency: <200ms
- P99 latency: <500ms
- Throughput: >5 images/second
- Concurrent throughput: >10 req/second

### Memory
- Memory increase: <100MB after 100 images
- Memory leak rate: <1KB/iteration
- Cold start time: <5 seconds

## Test Coverage

Total lines of test code: **2,473+**

- Integration tests: ~1,500 lines
- Common utilities: ~900 lines
- Test infrastructure: ~100 lines

Target coverage: **80%+** for integration tests

## CI/CD Integration

These tests are designed to run in:
- GitHub Actions
- GitLab CI
- Jenkins
- Local development

See `.github/workflows/test.yml` for CI configuration.

## Troubleshooting

### Tests Failing
1. Ensure test dependencies are installed
2. Check if test server can start on port 18080
3. Verify test data directories are writable
4. Check model files are accessible

### Performance Tests Failing
- Performance tests may be environment-dependent
- Adjust thresholds in test configuration if needed
- Run on dedicated test machines for consistent results

### Memory Tests Failing
- Memory tests require stable baseline
- Close other applications during testing
- Use `--test-threads=1` for serial execution

## Contributing

When adding new integration tests:
1. Follow existing test structure
2. Add descriptive test names
3. Include error messages in assertions
4. Update this README with new tests
5. Ensure tests are deterministic and isolated

## License

Same as ruvector-scipix project.
