# ADR-050: Quality Engineering Response — Security Hardening & Code Quality

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-06 |
| Deciders | ruv |
| Depends on | ADR-032 (Multistatic Mesh Security) |
| Issue | [#170](https://github.com/ruvnet/wifi-densepose/issues/170) |

## Context

An independent quality engineering analysis ([issue #170](https://github.com/ruvnet/wifi-densepose/issues/170)) identified 7 critical findings across the Rust codebase. After verification against the source code, the following findings are confirmed and require action:

### Confirmed Critical Findings

| # | Finding | Location | Verified |
|---|---------|----------|----------|
| 1 | Fake HMAC in `secure_tdm.rs` — XOR fold with hardcoded key | `hardware/src/esp32/secure_tdm.rs:253` | YES — comments say "sufficient for testing" |
| 2 | `sensing-server/main.rs` is 3,741 lines — CC=65, god object | `sensing-server/src/main.rs` | YES — confirmed 3,741 lines |
| 3 | WebSocket server has zero authentication | Rust WS codebase | YES — no auth/token checks found |
| 4 | Zero security tests in Rust codebase | Entire workspace | YES — no auth/injection/tampering tests |
| 5 | 54K fps claim has no supporting benchmark | No criterion benchmarks | YES — no benchmarks exist |

### Findings Requiring Further Investigation

| # | Finding | Status |
|---|---------|--------|
| 6 | Unauthenticated OTA firmware endpoint | Not found in Rust code — may be ESP32 C firmware level |
| 7 | WASM upload without mandatory signatures | Needs review of WASM loader |
| 8 | O(n^2) autocorrelation in heart rate detection | Needs profiling to confirm impact |

## Decision

Address findings in 3 priority sprints as recommended by the report.

### Sprint 1: Security (Blocks Deployment)

1. **Replace fake HMAC with real HMAC-SHA256** in `secure_tdm.rs`
   - Use the `hmac` + `sha2` crates (already in `Cargo.lock`)
   - Remove XOR fold implementation
   - Add key derivation (no more hardcoded keys)

2. **Add WebSocket authentication**
   - Token-based auth on WS upgrade handshake
   - Optional API key for local-network deployments
   - Configurable via environment variable

3. **Add security test suite**
   - Auth bypass attempts
   - Malformed CSI frame injection
   - Protocol tampering (TDM beacon replay, nonce reuse)

### Sprint 2: Code Quality & Testability

4. **Decompose `main.rs`** (3,741 lines -> ~14 focused modules)
   - Extract HTTP routes, WebSocket handler, CSI pipeline, config, state
   - Target: no file over 500 lines

5. **Add criterion benchmarks**
   - CSI frame parsing throughput
   - Signal processing pipeline latency
   - WebSocket broadcast fanout

### Sprint 3: Functional Verification

6. **Vital sign accuracy verification**
   - Reference signal tests with known BPM
   - False-negative rate measurement

7. **Fix O(n^2) autocorrelation** (if confirmed by profiling)
   - Replace brute-force lag with FFT-based autocorrelation

## Consequences

### Positive

- Addresses all critical security findings before any production deployment
- `main.rs` decomposition enables unit testing of server components
- Criterion benchmarks provide verifiable performance claims
- Security test suite prevents regression

### Negative

- Sprint 1 security changes are breaking for any existing TDM mesh deployments (fake HMAC -> real HMAC requires firmware update)
- `main.rs` decomposition is a large refactor with merge conflict risk

### Neutral

- The report correctly identifies that life-safety claims (disaster detection, vital signs) require rigorous verification — this is an ongoing process, not a single sprint

## Acknowledgment

Thanks to [@proffesor-for-testing](https://github.com/proffesor-for-testing) for the thorough 10-report analysis. The full report is archived at the [original gist](https://gist.github.com/proffesor-for-testing/02321e3f272720aa94484fffec6ab19b).

## References

- Issue #170: Quality Engineering Analysis
- ADR-032: Multistatic Mesh Security Hardening
- ADR-028: ESP32 Capability Audit
