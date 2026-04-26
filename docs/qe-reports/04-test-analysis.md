# Test Suite Analysis Report

**Project:** wifi-densepose (ruview)
**Date:** 2026-04-05
**Analyst:** QE Test Architect (V3)
**Scope:** All test suites across Python (v1), Rust (v2), and Mobile (ui/mobile)

---

## Executive Summary

The wifi-densepose project contains **3,353 total test functions** across three technology stacks:

| Stack | Test Functions | Files | Frameworks |
|-------|---------------|-------|------------|
| Rust (inline + integration) | 2,658 | 292 source files + 16 integration test files | `#[test]`, Rust built-in |
| Python (archive/v1/tests/) | 491 | 30 test files | pytest, pytest-asyncio |
| Mobile (ui/mobile) | 204 | 25 test files | Jest, React Testing Library |
| **Total** | **3,353** | **363** | |

### Overall Quality Score: 6.5/10

**Strengths:** Comprehensive Rust coverage, strong domain-specific signal processing validation, well-structured Python TDD suites.

**Critical Weaknesses:** Massive test duplication in Python CSI extractor tests, over-reliance on mocks in integration tests, several E2E/performance tests use mock objects that defeat the testing purpose, and mobile tests are predominantly smoke tests with shallow assertions.

---

## 1. Python Test Suite Analysis (archive/v1/tests/)

### 1.1 Test Distribution

| Category | Files | Test Functions | % of Total |
|----------|-------|---------------|------------|
| Unit | 14 | 325 | 66.2% |
| Integration | 11 | 109 | 22.2% |
| Performance | 2 | 26 | 5.3% |
| E2E | 1 | 8 | 1.6% |
| Fixtures/Mocks | 3 | 23 (helpers) | 4.7% |
| **Total** | **31** | **491** | **100%** |

**Pyramid Assessment:** 66:22:7 (unit:integration:e2e+perf) -- Slightly integration-light but within acceptable bounds.

### 1.2 Critical Finding: Massive Test Duplication

The CSI extractor module has **five** test files testing nearly identical functionality:

1. `test_csi_extractor.py` -- 16 tests (original, older API)
2. `test_csi_extractor_tdd.py` -- 18 tests (TDD rewrite)
3. `test_csi_extractor_tdd_complete.py` -- 20 tests (expanded TDD)
4. `test_csi_extractor_direct.py` -- 38 tests (direct imports)
5. `test_csi_standalone.py` -- 40 tests (standalone with importlib)

**Total: 132 tests across 5 files for a single module.**

These files test the same validation logic repeatedly. For example, the "empty amplitude" validation test appears in 4 of the 5 files with nearly identical code:

- `test_csi_extractor_tdd_complete.py:171-188` -- `test_validation_empty_amplitude`
- `test_csi_extractor_direct.py:293-310` -- `test_validation_empty_amplitude`
- `test_csi_standalone.py:305-322` -- `test_validate_empty_amplitude`
- `test_csi_extractor_tdd.py:166-181` -- `test_should_reject_invalid_csi_data`

The same pattern repeats for empty phase, invalid frequency, invalid bandwidth, invalid subcarriers, invalid antennas, SNR too low, and SNR too high -- each duplicated 3-4 times.

**Impact:** ~90 redundant tests. This inflates the test count by approximately 18% and creates a maintenance burden where changes to the CSI extractor require updating 4-5 test files.

**Recommendation:** Consolidate to a single test file (`test_csi_extractor.py`) using the `test_csi_standalone.py` approach (importlib-based, most comprehensive). Delete the other four files.

Similarly, there are duplicate suites for:
- Phase sanitizer: `test_phase_sanitizer.py` (7 tests) + `test_phase_sanitizer_tdd.py` (31 tests)
- Router interface: `test_router_interface.py` (13 tests) + `test_router_interface_tdd.py` (23 tests)
- CSI processor: `test_csi_processor.py` (6 tests) + `test_csi_processor_tdd.py` (25 tests)

### 1.3 Test Naming Conventions

Two competing conventions are used:

**Convention A (older tests):** `test_<action>_<condition>` (imperative)
```python
# test_csi_extractor.py:46
def test_extractor_initialization_creates_correct_configuration(self, ...):
```

**Convention B (TDD tests):** `test_should_<behavior>` (BDD-style)
```python
# test_csi_extractor_tdd.py:64
def test_should_initialize_with_valid_config(self, ...):
```

**Assessment:** Convention B is more descriptive and follows London School TDD naming. The project should standardize on one convention. Convention A is used in 6 files; Convention B in 8 files.

### 1.4 AAA Pattern Adherence

**Good examples:**

`test_csi_extractor.py:62-74` follows AAA with explicit comments:
```python
def test_start_extraction_configures_monitor_mode(self, ...):
    # Arrange
    mock_router_interface.enable_monitor_mode.return_value = True
    # Act
    result = csi_extractor.start_extraction()
    # Assert
    assert result is True
```

`test_sensing.py` follows AAA implicitly without comments but with clean structure throughout all 45 tests. This file is the best-written test file in the Python suite.

**Poor examples:**

`test_csi_processor_tdd.py:168-182` mixes arrangement with assertion:
```python
def test_should_preprocess_csi_data_successfully(self, csi_processor, sample_csi_data):
    with patch.object(csi_processor, '_remove_noise') as mock_noise:
        with patch.object(csi_processor, '_apply_windowing') as mock_window:
            with patch.object(csi_processor, '_normalize_amplitude') as mock_normalize:
                mock_noise.return_value = sample_csi_data
                mock_window.return_value = sample_csi_data
                mock_normalize.return_value = sample_csi_data
                result = csi_processor.preprocess_csi_data(sample_csi_data)
                assert result == sample_csi_data
```
This is a 5-level deep `with` block that obscures the test's intent.

### 1.5 Mock Usage Analysis

**Over-mocking (Critical):**

The TDD test files suffer from severe over-mocking. In `test_csi_processor_tdd.py:168-182`, the preprocessing test mocks out `_remove_noise`, `_apply_windowing`, and `_normalize_amplitude` -- the very functions being tested. The test only verifies that the mocks were called, not that the pipeline works correctly. Compare with `test_csi_processor.py:56-61`:

```python
def test_preprocess_returns_csi_data(self, csi_processor, sample_csi):
    result = csi_processor.preprocess_csi_data(sample_csi)
    assert isinstance(result, CSIData)
```

This test actually exercises the real code and validates the output type.

**Over-mocking count:** 14 of 25 tests in `test_csi_processor_tdd.py` mock internal methods rather than collaborators. This violates the London School TDD principle -- London School mocks *collaborators*, not the system under test's own private methods.

Similarly in `test_phase_sanitizer_tdd.py`, 12 of 31 tests mock internal methods (`_detect_outliers`, `_interpolate_outliers`, `_apply_moving_average`, `_apply_low_pass_filter`).

**Appropriate mock usage:**

`test_router_interface.py` correctly uses `@patch('paramiko.SSHClient')` to mock the SSH external dependency. This is textbook London School TDD -- mocking the collaborator (SSH client) to test the router interface's behavior.

`test_esp32_binary_parser.py:129-177` uses a real UDP socket with `threading.Thread` for the mock server -- excellent integration test design that avoids over-mocking.

### 1.6 Edge Case Coverage

**Excellent edge case coverage:**

`test_sensing.py` (45 tests) provides outstanding edge case coverage:
- Constant signals (`test_constant_signal_features`, line 327)
- Too few samples (`test_too_few_samples`, line 339)
- Cross-receiver agreement (`test_cross_receiver_agreement_boosts_confidence`, line 513)
- Confidence bounds checking (`test_confidence_bounded_0_to_1`, line 501)
- Multi-frequency band isolation (`test_band_isolation_multi_frequency`, line 308)
- Empty band power (`test_band_power_zero_for_empty_band`, line 697)
- Platform availability detection with mocked proc filesystem (lines 716-807)

`test_esp32_binary_parser.py` covers:
- Valid frame parsing (line 72)
- Frame too short (line 98)
- Invalid magic number (line 103)
- Multi-antenna frames (line 111)
- UDP timeout (line 179)

**Poor edge case coverage:**

`test_densepose_head.py` lacks tests for:
- Batch size of 0
- Non-square input sizes
- Very large batch sizes (memory limits)
- NaN/Inf in input tensors
- Half-precision (float16) inputs

`test_modality_translation.py` lacks tests for:
- Gradient clipping behavior
- Learning rate sensitivity
- Numerical stability with extreme values

### 1.7 Test Isolation

**Shared state issues:**

`test_sensing.py` -- The `SimulatedCollector` tests are well-isolated using seeds, but `TestCommodityBackend.test_full_pipeline` (line 592) directly accesses `collector._buffer` (private attribute). If the internal buffer implementation changes, this test breaks.

`test_csi_processor_tdd.py:326-354` -- Tests manipulate `csi_processor._total_processed`, `_processing_errors`, and `_human_detections` directly. These are private attributes and the tests are coupled to implementation details.

**No test order dependencies found.** All test files use proper fixture setup via `@pytest.fixture` or `setup_method`.

### 1.8 Flakiness Indicators

**Timing-dependent tests:**

- `test_phase_sanitizer.py:89-95` -- Asserts processing time `< 0.005` (5ms). This is fragile on CI with variable load.
- `test_csi_processor.py:93-98` -- Asserts preprocessing time `< 0.010` (10ms). Same concern.
- `test_csi_pipeline.py:202-222` -- Asserts pipeline processing `< 0.1s`. Better but still fragile.

**Non-deterministic tests:**

- `test_densepose_head.py:256-267` -- Training mode dropout test asserts outputs are different. With very small dropout rates or specific random seeds, outputs could occasionally match. The `atol=1e-6` tolerance is tight.
- `test_modality_translation.py:145-155` -- Same dropout randomness concern.

**Network-dependent tests:**

- `test_esp32_binary_parser.py:129-177` -- Uses real UDP sockets with `time.sleep(0.2)`. Could fail under network congestion or slow CI.
- `test_esp32_binary_parser.py:179-206` -- UDP timeout test with `timeout=0.5`. Race condition possible.

### 1.9 E2E and Performance Test Quality

**E2E tests (`test_healthcare_scenario.py`):**

This 735-line file defines its own mock classes (`MockPatientMonitor`, `MockHealthcareNotificationSystem`) rather than using the actual system. This makes it a **component integration test**, not a true E2E test. The test names include "should_fail_initially" comments suggesting TDD red-phase artifacts that were never cleaned up:

```python
# Line 348
async def test_fall_detection_workflow_should_fail_initially(self, ...):
```

Despite the names, these tests actually pass (they test the mock objects successfully). The naming is misleading.

**Performance tests (`test_inference_speed.py`):**

All 14 tests use `MockPoseModel` with `asyncio.sleep()` simulating inference time. These tests measure sleep accuracy, not actual inference performance. They are **simulation tests**, not performance tests. Every assertion like `assert inference_time < 100` is testing asyncio scheduling, not model performance.

**Recommendation:** Either rename these to "simulation tests" or replace `MockPoseModel` with actual model inference.

### 1.10 Test Infrastructure Quality

**Fixtures (`archive/v1/tests/fixtures/csi_data.py`):**

Well-designed `CSIDataGenerator` class (487 lines) with:
- Multiple scenario generators (empty room, single person, multi-person)
- Noise injection (`add_noise`)
- Hardware artifact simulation (`simulate_hardware_artifacts`)
- Time series generation
- Validation utilities (`validate_csi_sample`)

**Mocks (`archive/v1/tests/mocks/hardware_mocks.py`):**

Comprehensive mock infrastructure (716 lines) including:
- `MockWiFiRouter` with realistic CSI streaming
- `MockRouterNetwork` for multi-router scenarios
- `MockSensorArray` for environmental monitoring
- Factory functions (`create_test_router_network`, `setup_test_hardware_environment`)

These are well-engineered but used in only 1-2 test files. The E2E test defines its own mocks instead of using these.

---

## 2. Rust Test Suite Analysis

### 2.1 Test Distribution

| Category | Test Count | Source |
|----------|-----------|--------|
| Inline unit tests (`#[cfg(test)]`) | ~2,600 | 292 source files |
| Integration tests (`crates/*/tests/`) | ~58 | 16 integration test files |
| **Total** | **~2,658** | |

The Rust suite is the largest by far, with 1,031+ tests confirmed passing per the project's pre-merge checklist.

### 2.2 Integration Test Quality

**`wifi-densepose-train/tests/test_losses.rs` (18 tests):**

Excellent test quality. Key observations:

- All tests use deterministic data (no `rand` crate, no OS entropy) -- explicitly documented in the module docstring (line 9).
- Feature-gated behind `#[cfg(feature = "tch-backend")]` with a fallback test (line 447) that ensures compilation when the feature is disabled.
- Tests validate mathematical properties, not just "it doesn't crash":
  - `gaussian_heatmap_peak_at_keypoint_location` (line 55) -- Verifies the peak value and location
  - `gaussian_heatmap_zero_outside_3sigma_radius` (line 84) -- Validates every pixel in the heatmap
  - `keypoint_heatmap_loss_invisible_joints_contribute_nothing` (line 229) -- Tests visibility masking
- Clear naming convention: `<function_name>_<expected_behavior>`

**`wifi-densepose-signal/tests/validation_test.rs` (10 tests):**

Outstanding validation tests that prove algorithm correctness against known mathematical results:

- `validate_phase_unwrapping_correctness` (line 17) -- Creates a linearly increasing phase from 0 to 4pi, wraps it, then validates unwrapping reconstructs the original.
- `validate_amplitude_rms` (line 58) -- Uses constant-amplitude data where RMS equals the constant.
- `validate_doppler_calculation` (line 89) -- Computes expected Doppler shift from physics (2 * v * f / c) and validates the implementation matches.
- `validate_complex_conversion` (line 171) -- Round-trip test: amplitude/phase to complex and back.
- `validate_correlation_features` (line 250) -- Uses perfectly correlated antenna data to validate correlation > 0.9.

These tests demonstrate mathematical rigor rarely seen in signal processing codebases.

**`wifi-densepose-mat/tests/integration_adr001.rs` (6 tests):**

Clean integration tests for the disaster response pipeline:
- Deterministic breathing signal generator (16 BPM sinusoid at 0.267 Hz)
- Triage logic verification with explicit expected outcomes per breathing pattern
- Input validation (mismatched lengths, empty data)
- Determinism verification test (line 190) -- runs generator twice and asserts bitwise equality

### 2.3 Inline Test Patterns

The 292 source files with `#[cfg(test)]` modules show consistent patterns:

**Builder pattern testing** is common across crates:
```rust
CsiData::builder()
    .amplitude(amplitude)
    .phase(phase)
    .build()
    .unwrap()
```

**Feature-gated tests** prevent compilation failures when optional dependencies are unavailable. The `tch-backend` feature gate pattern is well-applied.

### 2.4 Missing Rust Test Coverage

Based on the crate list and test file analysis:

- `wifi-densepose-api` -- No integration tests for API routes found
- `wifi-densepose-db` -- No database integration tests found
- `wifi-densepose-config` -- No configuration edge case tests found
- `wifi-densepose-wasm` -- No WASM-specific tests beyond budget compliance
- `wifi-densepose-cli` -- No CLI integration tests found

These gaps are less concerning for crates that are primarily thin wrappers, but the API and DB crates warrant integration testing.

---

## 3. Mobile Test Suite Analysis (ui/mobile)

### 3.1 Test Distribution

| Category | Files | Tests | % |
|----------|-------|-------|---|
| Components | 7 | 33 | 16.2% |
| Screens | 5 | 25 | 12.3% |
| Hooks | 3 | 13 | 6.4% |
| Services | 4 | 37 | 18.1% |
| Stores | 3 | 52 | 25.5% |
| Utils | 3 | 42 | 20.6% |
| Test Utils/Mocks | 2 | 2 | 1.0% |
| **Total** | **27** | **204** | **100%** |

### 3.2 Component Test Quality

**Shallow smoke tests dominate.** Most component tests only verify rendering without crashing:

`GaugeArc.test.tsx:28-63` -- All 4 tests follow the same pattern:
```typescript
it('renders without crashing', () => {
    const { toJSON } = renderWithTheme(<GaugeArc ... />);
    expect(toJSON()).not.toBeNull();
});
```

This verifies the component doesn't throw, but doesn't test:
- Visual output correctness (arc calculation, text rendering)
- Prop-driven behavior changes
- Accessibility attributes
- Edge cases (value > max, negative values, value = 0)

**Better examples:**

`ringBuffer.test.ts` (20 tests) -- Comprehensive boundary testing:
- Zero capacity (line 21)
- Negative capacity (line 25)
- NaN capacity (line 29)
- Infinity capacity (line 33)
- Overflow behavior (line 46)
- Copy semantics (line 67)
- Min/max without comparator (line 98, 129)

`matStore.test.ts` (18 tests) -- Good state management tests:
- Initial state verification (lines 69-87)
- Upsert idempotency (lines 97-107)
- Multiple distinct entities (lines 109-113)
- Selection and deselection (lines 187-197)

### 3.3 Service Test Quality

`api.service.test.ts` (14 tests) -- Well-structured service tests:
- URL building edge cases (trailing slash, absolute URLs, empty base)
- Error normalization (Axios errors, generic errors, unknown errors)
- Retry logic verification (3 total calls, recovery on second attempt)

This is the best-tested service in the mobile suite.

### 3.4 Hook Test Quality

`usePoseStream.test.ts` (4 tests) -- Minimal hook tests:
- Only verifies module exports and store shape
- Cannot test actual hook behavior without rendering context
- Line 20-38: Tests the store, not the hook

**Missing:** No `renderHook()` usage from `@testing-library/react-hooks`. Hooks should be tested with the `renderHook` utility.

### 3.5 Missing Mobile Test Coverage

- No gesture interaction tests
- No navigation flow tests
- No dark/light theme switching tests
- No offline/error state rendering tests
- No accessibility (a11y) tests
- No snapshot tests for UI regression
- No WebSocket reconnection logic tests

---

## 4. Cross-Cutting Analysis

### 4.1 Test Pyramid Balance

| Layer | Python | Rust | Mobile | Project Total | Ideal |
|-------|--------|------|--------|---------------|-------|
| Unit | 66% | ~98% | 62% | ~92% | 70% |
| Integration | 22% | ~2% | 20% | ~5% | 20% |
| E2E/Perf | 7% | ~0% | 0% | ~1% | 10% |
| System/Acceptance | 5% (mocked) | 0% | 18% (screens) | ~2% | -- |

**Assessment:** The pyramid is top-heavy on unit tests due to the massive Rust inline test suite. Integration and E2E layers are weak across the board.

### 4.2 Duplicate Coverage Map

| Module | Files Testing It | Redundant Tests |
|--------|-----------------|-----------------|
| CSI Extractor | 5 Python files | ~90 |
| Phase Sanitizer | 2 Python files | ~7 |
| Router Interface | 2 Python files | ~13 |
| CSI Processor | 2 Python files | ~6 |
| **Total redundant** | | **~116** |

### 4.3 Test Gap Analysis

**Untested or under-tested areas:**

| Component | Gap Description | Risk |
|-----------|----------------|------|
| REST API (Python) | `test_api_endpoints.py` exists but uses mocks for all HTTP | High |
| WebSocket streaming | `test_websocket_streaming.py` exists but no real connection | High |
| ESP32 firmware | C code has no automated tests | Critical |
| Database layer (Rust) | No integration tests for `wifi-densepose-db` | Medium |
| Cross-crate integration | No tests validating crate dependency chains | Medium |
| Configuration validation | `wifi-densepose-config` has minimal test coverage | Low |
| WASM edge deployment | Only budget compliance tests | Medium |
| Mobile navigation | No screen transition tests | Medium |
| Mobile WebSocket | `ws.service.test.ts` exists but limited coverage | High |

### 4.4 Test Maintenance Burden

**High maintenance cost files:**

1. `archive/v1/tests/mocks/hardware_mocks.py` (716 lines) -- Complex mock infrastructure that must evolve with the production code. Any hardware interface change requires updating this file.

2. `archive/v1/tests/fixtures/csi_data.py` (487 lines) -- Rich data generation but duplicates some logic from the production `SimulatedCollector`.

3. The 5 CSI extractor test files collectively contain ~3,000 lines of test code for a single module. Merging to one file would reduce this to ~600 lines.

**Brittle test indicators:**

- Tests that access private attributes (`_buffer`, `_total_processed`, etc.): 8 occurrences
- Tests with magic number assertions (`< 0.005`, `< 0.010`): 5 occurrences
- Tests with `asyncio.sleep()` for synchronization: 12 occurrences

---

## 5. Specific File-Level Findings

### 5.1 Best Test Files (Exemplary Quality)

| File | Why It's Good |
|------|---------------|
| `archive/v1/tests/unit/test_sensing.py` | 45 tests with mathematical rigor, known-signal validation, domain-specific edge cases, cross-receiver agreement, band isolation. No mocks for core logic. |
| `archive/v1/tests/unit/test_esp32_binary_parser.py` | Real UDP socket testing, struct-level binary validation, ADR-018 compliance. Tests actual I/Q to amplitude/phase math. |
| `v2/.../tests/validation_test.rs` | Physics-based validation (Doppler, phase unwrapping, spectral analysis). Tests prove algorithm correctness, not just non-failure. |
| `v2/.../tests/test_losses.rs` | Deterministic data, feature-gated, tests mathematical properties (zero loss for identical inputs, non-zero for mismatched). |
| `ui/mobile/.../utils/ringBuffer.test.ts` | Comprehensive boundary testing (NaN, Infinity, 0, negative, overflow). Tests copy semantics. |

### 5.2 Worst Test Files (Needs Improvement)

| File | Issues |
|------|--------|
| `archive/v1/tests/performance/test_inference_speed.py` | Tests `asyncio.sleep()` accuracy, not model performance. `MockPoseModel` simulates inference with sleep. |
| `archive/v1/tests/e2e/test_healthcare_scenario.py` | Not a real E2E test -- defines its own mock classes. Test names contain stale "should_fail_initially" text. |
| `archive/v1/tests/unit/test_csi_processor_tdd.py` | 14/25 tests mock the SUT's own private methods. Tests verify mock calls, not behavior. |
| `archive/v1/tests/unit/test_phase_sanitizer_tdd.py` | 12/31 tests mock internal methods. Same anti-pattern as csi_processor_tdd. |
| `ui/mobile/.../components/GaugeArc.test.tsx` | All 4 tests are `expect(toJSON()).not.toBeNull()` -- smoke tests with no behavioral verification. |

---

## 6. Recommendations

### Priority 1: Eliminate Duplication (Effort: Low, Impact: High)

1. **Consolidate CSI extractor tests** into a single file. Retain `test_csi_standalone.py` (most comprehensive), delete the other four. This removes ~90 redundant tests and ~2,400 lines of duplicate code.

2. **Consolidate TDD pairs** -- Merge `test_phase_sanitizer.py` into `test_phase_sanitizer_tdd.py`, `test_router_interface.py` into `test_router_interface_tdd.py`, `test_csi_processor.py` into `test_csi_processor_tdd.py`.

### Priority 2: Fix Mock Anti-Patterns (Effort: Medium, Impact: High)

3. **Replace internal-method mocking** in `test_csi_processor_tdd.py` and `test_phase_sanitizer_tdd.py` with real execution tests. Mock only external collaborators (SSH, hardware, network).

4. **Replace `MockPoseModel`** in performance tests with actual model inference or clearly label these as "simulation tests."

### Priority 3: Add Missing Test Coverage (Effort: High, Impact: High)

5. **Add real integration tests** for the REST API and WebSocket endpoints using `httpx.AsyncClient` or similar.

6. **Add Rust integration tests** for `wifi-densepose-api`, `wifi-densepose-db`, and `wifi-densepose-cli` crates.

7. **Upgrade mobile component tests** from smoke tests to behavioral tests with prop variation, user interaction, and accessibility checks.

### Priority 4: Reduce Flakiness Risk (Effort: Low, Impact: Medium)

8. **Remove or widen timing assertions** in `test_phase_sanitizer.py:89` and `test_csi_processor.py:93`. Use `pytest-benchmark` for performance measurement, not inline time assertions.

9. **Add retry logic to UDP socket tests** in `test_esp32_binary_parser.py` or use mock sockets for unit-level testing.

### Priority 5: Standardize Conventions (Effort: Low, Impact: Low)

10. **Standardize test naming** to `test_should_<behavior>` (BDD-style) across all Python tests.

11. **Add pytest markers** consistently: `@pytest.mark.unit`, `@pytest.mark.integration`, `@pytest.mark.slow` for performance tests.

---

## 7. Metrics Summary

| Metric | Value | Assessment |
|--------|-------|------------|
| Total test functions | 3,353 | Good volume |
| Unique test functions (estimated) | ~3,237 | ~116 duplicates |
| Test-to-source ratio (Python) | 1.8:1 | High (inflated by duplication) |
| Test-to-source ratio (Rust) | 2.0:1 | Good |
| Files with over-mocking | 4 | Needs remediation |
| Timing-dependent tests | 5 | Flakiness risk |
| Tests with private attribute access | 8 | Fragility risk |
| E2E tests using real services | 0 | Critical gap |
| Redundant test files | 6 | Consolidation needed |
| Test files following AAA pattern | ~80% | Good |
| Tests with meaningful assertions | ~75% | Could improve |

---

*Report generated by QE Test Architect V3*
*Analysis based on full source code review of 363 test files*
