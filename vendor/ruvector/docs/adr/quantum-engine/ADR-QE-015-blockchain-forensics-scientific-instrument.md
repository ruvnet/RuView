# ADR-QE-015: Quantum Hardware Integration & Scientific Instrument Layer

**Status**: Accepted
**Date**: 2026-02-12
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board
**Supersedes**: None
**Extends**: ADR-QE-001, ADR-QE-002, ADR-QE-004

## Context

### Problem Statement

ruqu-core is currently a closed-world simulator: circuits run locally on state
vector, stabilizer, or tensor network backends with no path to real quantum
hardware, no cryptographic proof of execution, and no statistical rigor around
measurement confidence. For blockchain forensics and scientific applications,
three gaps must be closed:

1. **Hardware bridge**: Export circuits to OpenQASM 3.0, submit to IBM Quantum /
   IonQ / Rigetti / Amazon Braket, and import calibration-aware noise models.
2. **Scientific rigor**: Every simulation result must carry confidence bounds,
   be deterministically replayable, and be verifiable across backends.
3. **Audit trail**: A tamper-evident witness log must chain every execution so
   results can be independently reproduced and verified.

These capabilities transform ruqu from a simulator into a **scientific
instrument** suitable for peer-reviewed quantum-enhanced forensics.

### Current State

| Component | Exists | Gap |
|-----------|--------|-----|
| State vector backend | Yes (ruqu-core) | No hardware export |
| Stabilizer backend | Yes (ruqu-core) | No cross-backend verification |
| Tensor network backend | Yes (ruqu-core) | No confidence bounds |
| Basic noise model | Yes (depolarizing, bit/phase flip) | No T1/T2/readout/crosstalk |
| Seeded RNG | Yes (SimConfig.seed) | No snapshot/restore, no replay log |
| Gate set | Complete (H,X,Y,Z,S,T,Rx,Ry,Rz,CNOT,CZ,SWAP,Rzz) | No QASM export |
| Circuit analyzer | Yes (Clifford fraction, depth) | No automatic verification |

## Decision

### Architecture Overview

```
                          ruqu-core (existing)
                               |
            +------------------+------------------+
            |                  |                  |
     [OpenQASM 3.0]    [Noise Models]    [Scientific Layer]
      Export Bridge      Enhanced           |
            |                |         +----+----+--------+
            |                |         |         |        |
      [Hardware HAL]   [Error         [Replay]  [Witness] [Confidence]
       IBM/IonQ/       Mitigation]    Engine    Logger    Bounds
       Rigetti/Braket   Pipeline
            |                |              \     |     /
            +--------+------+               \    |    /
                     |                    [Cross-Backend
               [Transpiler]                Verification]
            Noise-Aware with
            Live Calibration
```

All new code lives in `crates/ruqu-core/src/` as new modules, extending the
existing crate without breaking the public API.

### 1. OpenQASM 3.0 Export Bridge

**Module**: `src/qasm.rs`

Serializes any `QuantumCircuit` to valid OpenQASM 3.0 text. Supports the full
gate set in `Gate` enum, parameterized rotations, barriers, measurement, and
reset.

```
OPENQASM 3.0;
include "stdgates.inc";
qubit[n] q;
bit[n] c;

h q[0];
cx q[0], q[1];
rz(0.785398) q[2];
c[0] = measure q[0];
```

**Design decisions**:
- Gate names follow the OpenQASM 3.0 `stdgates.inc` naming convention
- `Unitary1Q` fused gates decompose to `U(theta, phi, lambda)` form
- Round-trip fidelity: `circuit -> qasm -> parse -> circuit` preserves
  gate identity (not implemented here; parsing is out of scope)
- Output validated against IBM Quantum and IonQ acceptance criteria

### 2. Enhanced Noise Models

**Module**: `src/noise.rs`

Extends the existing `NoiseModel` with physically-motivated channels:

| Channel | Parameters | Kraus Operators |
|---------|-----------|-----------------|
| Depolarizing | p (error rate) | K0=sqrt(1-p)I, K1-3=sqrt(p/3){X,Y,Z} |
| Amplitude damping (T1) | gamma=1-exp(-t/T1) | K0=[[1,0],[0,sqrt(1-γ)]], K1=[[0,sqrt(γ)],[0,0]] |
| Phase damping (T2) | lambda=1-exp(-t/T2') | K0=[[1,0],[0,sqrt(1-λ)]], K1=[[0,0],[0,sqrt(λ)]] |
| Readout error | p01, p10 | Confusion matrix applied at measurement |
| Thermal relaxation | T1, T2, gate_time | Combined T1+T2 during idle periods |
| Crosstalk (ZZ) | zz_strength | Unitary Rzz rotation on adjacent qubits |

**Simulation approach**: Monte Carlo trajectories on the state vector. For each
gate, sample which Kraus operator to apply based on probabilities. This avoids
the 2x memory overhead of density matrix representation while giving correct
statistics over many shots.

**Calibration import**: `DeviceCalibration` struct holds per-qubit T1/T2/readout
errors and per-gate error rates, importable from hardware API JSON responses.

### 3. Error Mitigation Pipeline

**Module**: `src/mitigation.rs`

Post-processing techniques that improve result accuracy without modifying the
quantum circuit:

| Technique | Input | Output | Overhead |
|-----------|-------|--------|----------|
| Zero-Noise Extrapolation (ZNE) | Results at noise scales [1, 1.5, 2, 3] | Extrapolated zero-noise value | 3-4x shots |
| Measurement Error Mitigation | Raw counts + calibration matrix | Corrected counts | O(2^n) for n measured qubits |
| Clifford Data Regression (CDR) | Noisy results + stabilizer reference | Bias-corrected expectation | 2x circuits |

**ZNE implementation**: Gate folding (G -> G G^dag G) amplifies noise by
integer/half-integer factors. Richardson extrapolation fits a polynomial and
evaluates at noise_factor = 0.

**Measurement correction**: For <= 12 qubits, build full confusion matrix from
calibration data and invert via least-squares. For > 12 qubits, use tensor
product approximation assuming independent qubit readout errors.

### 4. Hardware Abstraction Layer

**Module**: `src/hardware.rs`

Trait-based provider abstraction for submitting circuits to real hardware:

```rust
pub trait HardwareProvider: Send + Sync {
    fn name(&self) -> &str;
    fn available_devices(&self) -> Vec<DeviceInfo>;
    fn device_calibration(&self, device: &str) -> Option<DeviceCalibration>;
    fn submit_circuit(&self, qasm: &str, shots: u32, device: &str)
        -> Result<JobHandle>;
    fn job_status(&self, handle: &JobHandle) -> Result<JobStatus>;
    fn job_results(&self, handle: &JobHandle) -> Result<HardwareResult>;
}
```

**Provider adapters** (stubbed, not implementing actual HTTP clients):

| Provider | Auth | Circuit Format | API Style |
|----------|------|---------------|-----------|
| IBM Quantum | API key + token | OpenQASM 3.0 | REST |
| IonQ | API key (header) | OpenQASM 2.0 / native JSON | REST |
| Rigetti | OAuth2 / API key | Quil / OpenQASM | REST + gRPC |
| Amazon Braket | AWS credentials | OpenQASM 3.0 | AWS SDK |

Each adapter is a zero-dependency stub implementing the trait. Actual HTTP
clients are injected by the consumer, keeping ruqu-core `no_std`-compatible.

### 5. Noise-Aware Transpiler

**Module**: `src/transpiler.rs`

Maps abstract circuits to hardware-native gate sets using device calibration:

1. **Gate decomposition**: Decompose non-native gates into the target basis
   (e.g., IBM: {CX, ID, RZ, SX, X}; IonQ: {GPI, GPI2, MS}).
2. **Qubit routing**: Map logical qubits to physical qubits respecting the
   device coupling map (greedy nearest-neighbor heuristic).
3. **Noise-aware optimization**: Prefer gates/qubits with lower error rates
   from live calibration data.
4. **Gate cancellation**: Cancel adjacent inverse gates (H-H, S-Sdg, etc.)
   after routing.

### 6. Deterministic Replay Engine

**Module**: `src/replay.rs`

Every simulation execution is fully reproducible:

```rust
pub struct ExecutionRecord {
    pub circuit_hash: [u8; 32],    // SHA-256 of QASM representation
    pub seed: u64,                  // ChaCha20 RNG seed
    pub backend: BackendType,       // Which backend was used
    pub noise_config: Option<NoiseModelConfig>,
    pub shots: u32,
    pub software_version: &'static str,
    pub timestamp_utc: u64,
}
```

**Replay guarantee**: Given an `ExecutionRecord`, calling
`replay(record, circuit)` produces bit-identical results. This requires:
- Deterministic RNG: `ChaCha20Rng` (via `rand_chacha`), seeded per-shot as
  `base_seed.wrapping_add(shot_index)`
- Deterministic gate application order (already guaranteed by `Vec<Gate>`)
- Deterministic noise sampling (same RNG stream)

**Snapshot/restore**: For long-running VQE iterations, the engine can serialize
the state vector to a checkpoint and restore it, enabling resumable computation.

### 7. Witness Logging (Cryptographic Audit Trail)

**Module**: `src/witness.rs`

A tamper-evident append-only log where each entry contains:

```rust
pub struct WitnessEntry {
    pub sequence: u64,              // Monotonic counter
    pub prev_hash: [u8; 32],       // SHA-256 of previous entry
    pub execution: ExecutionRecord, // Full replay metadata
    pub result_hash: [u8; 32],     // SHA-256 of measurement outcomes
    pub entry_hash: [u8; 32],      // SHA-256(sequence || prev_hash || execution || result_hash)
}
```

**Hash chain**: Each entry's `entry_hash` incorporates the previous entry's
hash, forming a blockchain-style chain. Tampering with any entry invalidates
all subsequent hashes.

**Verification**: `verify_witness_chain(entries)` walks the chain and confirms:
1. Hash linkage: `entry[i].prev_hash == entry[i-1].entry_hash`
2. Self-consistency: Recomputed `entry_hash` matches stored value
3. Optional replay: Re-execute the circuit and confirm `result_hash` matches

**Format**: Entries are serialized as length-prefixed bincode with CRC32
checksums, stored in an append-only file. JSON export available for
interoperability.

### 8. Confidence Bounds

**Module**: `src/confidence.rs`

Every measurement result carries statistical confidence:

| Metric | Method | Formula |
|--------|--------|---------|
| Probability CI | Wilson score | p_hat +/- z*sqrt(p*(1-p)/n + z^2/(4n^2)) / (1 + z^2/n) |
| Expectation value SE | Standard error | sigma / sqrt(n_shots) |
| Shot budget | Hoeffding bound | N >= ln(2/delta) / (2*epsilon^2) |
| Distribution distance | Total variation | TVD = 0.5 * sum(|p_i - q_i|) |
| Distribution test | Chi-squared | sum((O_i - E_i)^2 / E_i) |

**Confidence levels**: Results include 95% and 99% confidence intervals by
default. The user can request custom confidence levels.

**Convergence monitoring**: As shots accumulate, the engine tracks whether
confidence intervals have stabilized, enabling early termination when the
desired precision is reached.

### 9. Automatic Cross-Backend Verification

**Module**: `src/verification.rs`

Every simulation can be independently verified across backends:

```
Verification Protocol:
1. Analyze circuit (existing CircuitAnalysis)
2. If pure Clifford -> run on BOTH StateVector AND Stabilizer
   -> compare measurement distributions (must match exactly)
3. If small enough for StateVector -> run on StateVector
   -> compare with hardware results using chi-squared test
4. Report: {match_level, p_value, tvd, explanation}
```

**Verification levels**:

| Level | Comparison | Test | Threshold |
|-------|-----------|------|-----------|
| Exact | Stabilizer vs StateVector | Bitwise match | All probabilities equal |
| Statistical | Simulator vs Hardware | Chi-squared, p > 0.05 | TVD < 0.1 |
| Trend | VQE energy curves | Pearson correlation | r > 0.95 |

**Automatic Clifford detection**: Uses the existing `CircuitAnalysis.clifford_fraction`
to determine if stabilizer verification is applicable.

**Discrepancy report**: When backends disagree beyond statistical tolerance,
the engine produces a structured report identifying which qubits/gates show
the largest divergence.

## New Module Map

```
crates/ruqu-core/src/
  lib.rs            (existing, add mod declarations)
  qasm.rs           NEW - OpenQASM 3.0 serializer
  noise.rs          NEW - Enhanced noise models (T1/T2/readout/crosstalk)
  mitigation.rs     NEW - Error mitigation pipeline (ZNE, measurement correction)
  hardware.rs       NEW - Hardware abstraction layer + provider stubs
  transpiler.rs     NEW - Noise-aware circuit transpilation
  replay.rs         NEW - Deterministic replay engine
  witness.rs        NEW - Cryptographic witness logging
  confidence.rs     NEW - Statistical confidence bounds
  verification.rs   NEW - Cross-backend automatic verification
```

## Dependencies

New dependencies required in `ruqu-core/Cargo.toml`:

| Crate | Version | Feature | Purpose |
|-------|---------|---------|---------|
| `sha2` | 0.10 | optional: `witness` | SHA-256 hashing for witness chain |
| `rand_chacha` | 0.3 | optional: `replay` | Deterministic ChaCha20 RNG |
| `bincode` | 1.3 | optional: `witness` | Binary serialization for witness entries |

All new features are behind optional feature flags to keep the default build
minimal and `no_std`-compatible.

## Consequences

### Positive

- **Scientific credibility**: Every result carries confidence bounds, is
  replayable, and has a tamper-evident audit trail
- **Hardware-ready**: Circuits can target real quantum processors via the HAL
- **Verifiable**: Cross-backend verification catches simulation bugs and
  hardware errors automatically
- **Non-breaking**: All new modules are additive; existing API is unchanged
- **Minimal dependencies**: Core scientific features (confidence, replay) need
  only `rand_chacha`; witness logging adds `sha2` + `bincode`

### Negative

- **Increased surface area**: 9 new modules add maintenance burden
- **Feature interaction complexity**: Noise + mitigation + verification creates
  a combinatorial test space
- **Performance overhead**: Witness logging and confidence computation add
  ~5-10% per-shot overhead

### Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| RNG non-determinism across platforms | Low | High | Pin ChaCha20, test on x86+ARM+WASM |
| Hash chain corruption | Low | High | CRC32 per entry + full chain verification |
| Confidence bound miscalculation | Medium | High | Property-based testing with known distributions |
| Hardware API rate limits | Medium | Low | Exponential backoff + circuit batching |

## References

- [ADR-QE-001: Quantum Engine Core Architecture](./ADR-QE-001-quantum-engine-core-architecture.md)
- [ADR-QE-002: Crate Structure & Integration](./ADR-QE-002-crate-structure-integration.md)
- [ADR-QE-004: Performance Optimization & Benchmarks](./ADR-QE-004-performance-optimization-benchmarks.md)
- Wilson, E.B. "Probable inference, the law of succession, and statistical inference" (1927)
- Aaronson & Gottesman, "Improved simulation of stabilizer circuits" (2004)
- Temme, Bravyi, Gambetta, "Error mitigation for short-depth quantum circuits" (2017)
- OpenQASM 3.0 Specification, arXiv:2104.14722
