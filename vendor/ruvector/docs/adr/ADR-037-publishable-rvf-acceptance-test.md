# ADR-037: Publishable RVF Acceptance Test

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-02-16 |
| **Deciders** | RuVector core team |
| **Supersedes** | — |
| **Related** | ADR-029 (RVF canonical format), ADR-032 (RVF WASM integration), ADR-039 (RVF Solver WASM AGI integration) |

## Context

Temporal reasoning benchmarks produce results that are difficult for external developers to verify independently. Traditional benchmark reports rely on trust: the publisher runs the tests and shares aggregate metrics, but there is no mechanism for a third party to prove that the exact same computations produced those results. This gap matters for publishable research artifacts and for building confidence in the ablation study methodology.

The RVF format already provides a cryptographic witness chain infrastructure (WITNESS_SEG 0x0A) using SHAKE-256 hash linking, but this capability had not been applied to acceptance testing.

## Decision

We integrate the publishable acceptance test directly with the native RVF crate infrastructure to produce a self-contained, offline-verifiable artifact:

### 1. SHAKE-256 witness chain (rvf-crypto native)

The acceptance test replaces the standalone SHA-256 chain with `rvf_crypto::shake256_256` for all hash computations. Every puzzle decision (skip mode, context bucket, solve outcome, step count) is hashed into a SHAKE-256 chain where `chain_hash[i] = SHAKE-256(prev_hash || canonical_bytes(record))`. The chain is deterministic: frozen seeds produce identical puzzles, identical solve paths, and identical root hashes.

The parallel `rvf_crypto::WitnessEntry` list (73 bytes each: `prev_hash[32] + action_hash[32] + timestamp_ns[8] + witness_type[1]`) is built alongside the JSON chain, enabling native `.rvf` binary export.

### 2. Dual-format output (JSON + .rvf binary)

The `generate_manifest_with_rvf()` function produces both:

- **JSON manifest**: Human-readable scorecard, ablation assertions, full witness chain with hex hashes. Suitable for review, CI comparison, and documentation.
- **`.rvf` binary**: A valid RVF file containing:
  - `WITNESS_SEG` (0x0A): Native 73-byte entries created by `rvf_crypto::create_witness_chain()`, verifiable by `rvf_crypto::verify_witness_chain()`.
  - `META_SEG` (0x07): JSON-encoded scorecards, assertions, and config metadata.

### 3. WASM witness verification

Two new exports added to `rvf-wasm`:

| Export | Signature | Description |
|--------|-----------|-------------|
| `rvf_witness_verify` | `(chain_ptr, chain_len) -> i32` | Verify SHAKE-256 chain integrity. Returns entry count or negative error. |
| `rvf_witness_count` | `(chain_len) -> i32` | Count entries without full verification. |

This enables browser-side verification of acceptance test `.rvf` files without any backend.

### 4. Feature-gated ed25519 in rvf-crypto

To add `rvf-crypto` as a dependency to the no_std WASM microkernel without pulling in the heavy `ed25519-dalek` crate, the `sign` module is now gated behind an `ed25519` feature flag:

```toml
[features]
default = ["std", "ed25519"]
ed25519 = ["dep:ed25519-dalek"]
```

The hash, witness, attestation, lineage, and footer modules remain available without `ed25519`. Existing callers that use default features are unaffected.

### 5. Three-mode ablation grading

The acceptance test runs all three ablation modes and asserts six properties:

| Assertion | Criterion |
|-----------|-----------|
| B beats A on cost | >= 15% cost reduction |
| C beats B on robustness | >= 10% noise accuracy gain |
| Compiler safe | < 5% false-hit rate |
| A skip nonzero | Fixed policy uses skip modes |
| C multi-mode | Learned policy uses >= 2 skip modes |
| C penalty < B penalty | Learned policy reduces early-commit penalty |

All assertions, per-mode scorecards, and the witness chain root hash are included in the publishable artifact.

## Verification Protocol

An external developer reproduces the test:

```bash
# 1. Generate with default config (Rust)
cargo run --bin acceptance-rvf -- generate -o manifest.json

# 2. Compare chain root hash
# If chain_root_hash matches, outcomes are bit-for-bit identical

# 3. Verify the .rvf binary witness chain
cargo run --bin acceptance-rvf -- verify-rvf -i acceptance_manifest.rvf

# 4. Or verify in-browser via WASM:
#    const count = rvf_witness_verify(chainPtr, chainLen);
```

An npm-based verification path is also available via `@ruvector/rvf-solver`:

```typescript
import { RvfSolver } from '@ruvector/rvf-solver';

// Run the same acceptance test from JavaScript/TypeScript
const solver = await RvfSolver.create();
const manifest = solver.acceptance({
  holdoutSize: 100,
  trainingPerCycle: 100,
  cycles: 5,
  stepBudget: 400,
  seed: 42n,
});

// manifest.allPassed === true means Mode C (learned policy) passed
// manifest.witnessEntries gives the chain entry count
// solver.witnessChain() returns the raw SHAKE-256 bytes for verification

solver.destroy();
```

## Consequences

### Positive

- External developers can independently verify benchmark outcomes offline
- The `.rvf` binary is compatible with all RVF tooling (CLI, WASM, Node.js)
- Browser-side verification via `rvf_witness_verify` requires zero backend
- Deterministic replay means same config always produces same root hash
- The SHAKE-256 chain is forward-compatible with RVF's attestation infrastructure

### Negative

- Switching from SHA-256 to SHAKE-256 changes existing chain root hashes (version bumped to 2)
- The `ed25519` feature gate adds a minor complexity to rvf-crypto's feature matrix
- The WASM binary size increases slightly with the sha3 dependency

### Neutral

- JSON and .rvf outputs are independent — either can be used alone
- The `rvf_witness_count` export is a convenience that avoids full verification cost
