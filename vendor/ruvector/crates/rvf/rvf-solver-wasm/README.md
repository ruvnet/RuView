# rvf-solver-wasm

Self-learning temporal reasoning engine compiled to WebAssembly -- Thompson Sampling, three-loop adaptive solver, and cryptographic witness chains in ~160 KB.

## Overview

`rvf-solver-wasm` compiles the complete AGI temporal puzzle solver to `wasm32-unknown-unknown` for use in browsers, Node.js, and edge runtimes. It is a `no_std + alloc` crate (same architecture as `rvf-wasm`) with a pure C ABI export surface -- no `wasm-bindgen` required.

The solver learns which solving strategy works best for each problem context using Thompson Sampling, compiles successful patterns into a signature cache, and proves its learning through a three-mode ablation test with SHAKE-256 witness chains.

### Key Design Choices

| Choice | Rationale |
|--------|-----------|
| **no_std + alloc** | Matches `rvf-wasm` pattern; runs in any WASM runtime |
| **Pure-integer `Date` type** | Howard Hinnant algorithm replaces `chrono`; no std required |
| **`BTreeMap` over `HashMap`** | Available in `alloc`; deterministic iteration order |
| **`libm` for float math** | `sqrt`, `log`, `cos`, `pow` -- pure Rust, no_std compatible |
| **xorshift64 RNG** | Deterministic, zero dependencies, identical to benchmarks RNG |
| **C ABI exports** | Maximum compatibility -- works with any WASM host |
| **Handle-based API** | Up to 8 concurrent solver instances |

## Build

```bash
# Build the WASM module
cargo build --target wasm32-unknown-unknown --release -p rvf-solver-wasm

# Optimize with wasm-opt (optional, ~80-100 KB output)
wasm-opt -Oz target/wasm32-unknown-unknown/release/rvf_solver_wasm.wasm \
  -o rvf_solver_wasm.opt.wasm
```

### Binary Size

| Build | Size |
|-------|------|
| Release (`wasm32-unknown-unknown`) | ~160 KB |
| After `wasm-opt -Oz` | ~80-100 KB |

## Architecture

### Three-Loop Adaptive Solver

The engine uses a three-loop architecture where each loop operates on a different timescale:

```
 Fast loop (per puzzle)          Medium loop (per batch)       Slow loop (per cycle)
 ┌──────────────────────┐       ┌──────────────────────┐     ┌──────────────────────┐
 │ Constraint propagation│──────▶│ PolicyKernel selects  │────▶│ ReasoningBank tracks │
 │ Range narrowing       │       │ skip mode via Thompson│     │ trajectories         │
 │ Date enumeration      │       │ Sampling (two-signal) │     │ KnowledgeCompiler    │
 │ Solution validation   │       │ Speculative dual-path │     │ compiles patterns    │
 └──────────────────────┘       └──────────────────────┘     │ Checkpoint/rollback  │
                                                              └──────────────────────┘
```

| Loop | Frequency | What it does |
|------|-----------|--------------|
| **Fast** | Every puzzle | Constraint propagation, range narrowing, date enumeration, solution check |
| **Medium** | Every puzzle | Thompson Sampling selects `None`/`Weekday`/`Hybrid` skip mode per context bucket |
| **Slow** | Per training cycle | ReasoningBank promotes successful trajectories; KnowledgeCompiler caches signatures |

### Five AGI Capabilities

| # | Capability | Description |
|---|-----------|-------------|
| 1 | **Thompson Sampling** | Two-signal model: Beta posterior for safety (correct + no early-commit) + EMA for cost |
| 2 | **18 Context Buckets** | 3 range (small/medium/large) x 3 distractor (clean/some/heavy) x 2 noise = 18 independent bandits |
| 3 | **Speculative Dual-Path** | When top-2 arms within delta 0.15 and variance > 0.02, speculatively execute secondary arm |
| 4 | **KnowledgeCompiler** | Constraint signature cache (`v1:{difficulty}:{sorted_types}`); compiled skip-mode, step budget, confidence |
| 5 | **Acceptance Test** | Multi-cycle training/holdout with A/B/C ablation and checkpoint/rollback on regression |

### Ablation Modes

| Mode | Compiler | Router | Purpose |
|------|----------|--------|---------|
| **A** (Baseline) | Off | Off | Fixed heuristic policy; establishes cost/accuracy baseline |
| **B** (Compiler) | On | Off | KnowledgeCompiler active; must show >= 15% cost decrease vs A |
| **C** (Full) | On | On | Thompson Sampling + speculation; must show robustness gain vs B |

## WASM Export Surface

### Memory Management (2 exports)

| Export | Signature | Description |
|--------|-----------|-------------|
| `rvf_solver_alloc` | `(size: i32) -> i32` | Allocate WASM memory; returns pointer or 0 |
| `rvf_solver_free` | `(ptr: i32, size: i32)` | Free previously allocated memory |

### Lifecycle (2 exports)

| Export | Signature | Description |
|--------|-----------|-------------|
| `rvf_solver_create` | `() -> i32` | Create solver instance; returns handle (>0) or -1 |
| `rvf_solver_destroy` | `(handle: i32) -> i32` | Destroy solver; returns 0 on success |

### Training (1 export)

| Export | Signature | Description |
|--------|-----------|-------------|
| `rvf_solver_train` | `(handle, count, min_diff, max_diff, seed_lo, seed_hi) -> i32` | Train on `count` generated puzzles using three-loop learning; returns correct count |

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `handle` | `i32` | Solver instance handle |
| `count` | `i32` | Number of puzzles to generate and solve |
| `min_diff` | `i32` | Minimum puzzle difficulty (1-10) |
| `max_diff` | `i32` | Maximum puzzle difficulty (1-10) |
| `seed_lo` | `i32` | Lower 32 bits of RNG seed |
| `seed_hi` | `i32` | Upper 32 bits of RNG seed |

### Acceptance Test (1 export)

| Export | Signature | Description |
|--------|-----------|-------------|
| `rvf_solver_acceptance` | `(handle, holdout, training, cycles, budget, seed_lo, seed_hi) -> i32` | Run full A/B/C ablation test; returns 1 = passed, 0 = failed, -1 = error |

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `handle` | `i32` | Solver instance handle |
| `holdout` | `i32` | Number of holdout puzzles per evaluation |
| `training` | `i32` | Training puzzles per cycle |
| `cycles` | `i32` | Number of training/evaluation cycles |
| `budget` | `i32` | Maximum steps per puzzle solve |
| `seed_lo` | `i32` | Lower 32 bits of RNG seed |
| `seed_hi` | `i32` | Upper 32 bits of RNG seed |

### Result / Policy / Witness Reads (6 exports)

| Export | Signature | Description |
|--------|-----------|-------------|
| `rvf_solver_result_len` | `(handle: i32) -> i32` | Byte length of last result JSON |
| `rvf_solver_result_read` | `(handle: i32, out_ptr: i32) -> i32` | Copy result JSON to `out_ptr`; returns bytes written |
| `rvf_solver_policy_len` | `(handle: i32) -> i32` | Byte length of policy state JSON |
| `rvf_solver_policy_read` | `(handle: i32, out_ptr: i32) -> i32` | Copy policy JSON to `out_ptr`; returns bytes written |
| `rvf_solver_witness_len` | `(handle: i32) -> i32` | Byte length of witness chain (73 bytes/entry) |
| `rvf_solver_witness_read` | `(handle: i32, out_ptr: i32) -> i32` | Copy raw witness chain to `out_ptr`; returns bytes written |

## Usage from JavaScript

### Node.js / Browser

```javascript
import { readFile } from 'fs/promises';

// Load WASM module
const wasmBytes = await readFile('rvf_solver_wasm.wasm');
const { instance } = await WebAssembly.instantiate(wasmBytes);
const wasm = instance.exports;

// Create a solver instance
const handle = wasm.rvf_solver_create();
console.log('Solver handle:', handle); // 1

// Train on 500 puzzles (difficulty 1-8, seed 42)
const correct = wasm.rvf_solver_train(handle, 500, 1, 8, 42, 0);
console.log(`Training: ${correct}/500 correct`);

// Run full acceptance test (A/B/C ablation)
const passed = wasm.rvf_solver_acceptance(
  handle,
  100,  // holdout puzzles
  100,  // training per cycle
  5,    // cycles
  400,  // step budget
  42, 0 // seed
);
console.log('Acceptance test:', passed === 1 ? 'PASSED' : 'FAILED');

// Read the result manifest (JSON)
const resultLen = wasm.rvf_solver_result_len(handle);
const resultPtr = wasm.rvf_solver_alloc(resultLen);
wasm.rvf_solver_result_read(handle, resultPtr);
const resultJson = new TextDecoder().decode(
  new Uint8Array(wasm.memory.buffer, resultPtr, resultLen)
);
const manifest = JSON.parse(resultJson);
console.log('Mode A accuracy:', manifest.mode_a.cycles.at(-1).accuracy);
console.log('Mode B accuracy:', manifest.mode_b.cycles.at(-1).accuracy);
console.log('Mode C accuracy:', manifest.mode_c.cycles.at(-1).accuracy);
wasm.rvf_solver_free(resultPtr, resultLen);

// Read policy state (Thompson Sampling internals)
const policyLen = wasm.rvf_solver_policy_len(handle);
const policyPtr = wasm.rvf_solver_alloc(policyLen);
wasm.rvf_solver_policy_read(handle, policyPtr);
const policyJson = new TextDecoder().decode(
  new Uint8Array(wasm.memory.buffer, policyPtr, policyLen)
);
const policy = JSON.parse(policyJson);
console.log('Context buckets:', Object.keys(policy.context_stats).length);
console.log('Early commit rate:', (policy.early_commits_wrong / policy.early_commits_total * 100).toFixed(1) + '%');
wasm.rvf_solver_free(policyPtr, policyLen);

// Read witness chain (verifiable by rvf-wasm)
const witnessLen = wasm.rvf_solver_witness_len(handle);
const witnessPtr = wasm.rvf_solver_alloc(witnessLen);
wasm.rvf_solver_witness_read(handle, witnessPtr);
const witnessChain = new Uint8Array(
  wasm.memory.buffer, witnessPtr, witnessLen
).slice(); // copy out of WASM memory
console.log('Witness entries:', witnessLen / 73);
wasm.rvf_solver_free(witnessPtr, witnessLen);

// Clean up
wasm.rvf_solver_destroy(handle);
```

### Verify Witness Chain with rvf-wasm

```javascript
// Load both WASM modules
const solver = await WebAssembly.instantiate(solverWasmBytes);
const verifier = await WebAssembly.instantiate(rvfWasmBytes);

// Run acceptance test in solver
const handle = solver.instance.exports.rvf_solver_create();
solver.instance.exports.rvf_solver_acceptance(handle, 100, 100, 5, 400, 42, 0);

// Extract witness chain
const wLen = solver.instance.exports.rvf_solver_witness_len(handle);
const wPtr = solver.instance.exports.rvf_solver_alloc(wLen);
solver.instance.exports.rvf_solver_witness_read(handle, wPtr);
const chain = new Uint8Array(solver.instance.exports.memory.buffer, wPtr, wLen).slice();

// Copy into verifier memory and verify
const vPtr = verifier.instance.exports.rvf_alloc(wLen);
new Uint8Array(verifier.instance.exports.memory.buffer, vPtr, wLen).set(chain);
const entryCount = verifier.instance.exports.rvf_witness_verify(vPtr, wLen);

if (entryCount > 0) {
  console.log(`Witness chain verified: ${entryCount} entries`);
} else {
  console.error('Witness chain verification failed:', entryCount);
  // -2 = truncated, -3 = hash mismatch
}

verifier.instance.exports.rvf_free(vPtr, wLen);
solver.instance.exports.rvf_solver_destroy(handle);
```

## Module Structure

```
crates/rvf/rvf-solver-wasm/
├── Cargo.toml           # no_std + alloc, dlmalloc, libm, serde_json
├── README.md            # This file
└── src/
    ├── lib.rs           # 12 WASM exports, instance registry, panic handler
    ├── alloc_setup.rs   # dlmalloc global allocator, rvf_solver_alloc/free
    ├── types.rs         # Date arithmetic, Constraint, Puzzle, Rng64
    ├── policy.rs        # PolicyKernel, Thompson Sampling, KnowledgeCompiler
    └── engine.rs        # AdaptiveSolver, ReasoningBank, PuzzleGenerator, acceptance test
```

| File | Lines | Purpose |
|------|-------|---------|
| `types.rs` | 239 | Pure-integer date math (Howard Hinnant algorithm), 10 constraint types, puzzle checking, xorshift64 RNG |
| `policy.rs` | 505 | Thompson Sampling two-signal model, Marsaglia gamma sampling, 18 context buckets, KnowledgeCompiler signature cache |
| `engine.rs` | 690 | Three-loop solver, constraint propagation, ReasoningBank trajectory tracking, PuzzleGenerator, acceptance test runner |
| `lib.rs` | 396 | 12 C ABI WASM exports, handle-based registry (8 slots), SHAKE-256 witness chain, panic handler |
| `alloc_setup.rs` | 45 | dlmalloc global allocator, `rvf_solver_alloc`/`rvf_solver_free` interop |

## Temporal Constraint Types

The solver handles 10 constraint types for temporal puzzle solving:

| Constraint | Example | Description |
|------------|---------|-------------|
| `Exact(date)` | `2025-03-15` | Must be this exact date |
| `After(date)` | `> 2025-01-01` | Must be strictly after date |
| `Before(date)` | `< 2025-12-31` | Must be strictly before date |
| `Between(a, b)` | `2025-01-01..2025-06-30` | Must fall within range (inclusive) |
| `DayOfWeek(w)` | `Monday` | Must fall on this weekday |
| `DaysAfter(ref, n)` | `5 days after "meeting"` | Relative to named reference date |
| `DaysBefore(ref, n)` | `3 days before "deadline"` | Relative to named reference date |
| `InMonth(m)` | `March` | Must be in this month |
| `InYear(y)` | `2025` | Must be in this year |
| `DayOfMonth(d)` | `15th` | Must be this day of month |

## Thompson Sampling Details

### Two-Signal Model

Each skip-mode arm (`None`, `Weekday`, `Hybrid`) maintains two signals per context bucket:

| Signal | Distribution | Update Rule |
|--------|-------------|-------------|
| **Safety** | Beta(alpha, beta) | alpha += 1 on correct & no early-commit; beta += 1 on failure, beta += 1.5 on early-commit wrong |
| **Cost** | EMA (alpha = 0.1) | Normalized step count (steps / 200), exponentially weighted |

**Composite score:** `sample_beta(alpha, beta) - 0.3 * cost_ema`

### Context Bucketing

| Dimension | Levels | Thresholds |
|-----------|--------|------------|
| **Range** | small, medium, large | 0-60, 61-180, 181+ days |
| **Distractors** | clean, some, heavy | 0, 1, 2+ duplicate constraint types |
| **Noise** | clean, noisy | Whether puzzle has injected noise |

Total: 3 x 3 x 2 = **18 independent bandit contexts**

### Speculative Dual-Path

When the top-2 arms are within delta 0.15 of each other and the leading arm's variance exceeds 0.02, the solver speculatively executes the secondary arm. This accelerates convergence in uncertain contexts.

## Integration with RVF Ecosystem

```
┌──────────────────────┐         ┌──────────────────────┐
│   rvf-solver-wasm    │         │     rvf-wasm         │
│   (self-learning     │ ──────▶ │   (verification)     │
│    AGI engine)       │ witness │                      │
│                      │ chain   │ rvf_witness_verify   │
│ rvf_solver_train     │         │ rvf_witness_count    │
│ rvf_solver_acceptance│         │                      │
│ rvf_solver_witness_* │         │ rvf_store_*          │
└──────────┬───────────┘         └──────────────────────┘
           │ uses
    ┌──────▼──────┐
    │  rvf-crypto  │
    │  SHAKE-256   │
    │  witness     │
    │  chain       │
    └─────────────┘
```

- **rvf-solver-wasm** produces witness chains via `rvf-crypto::create_witness_chain`
- **rvf-wasm** verifies those chains via `rvf_witness_verify` (73 bytes per entry)
- Both modules run in the browser -- no backend required

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `rvf-types` | 0.1.0 | Shared RVF type definitions |
| `rvf-crypto` | 0.1.0 | SHAKE-256 hashing and witness chain creation |
| `dlmalloc` | 0.2 | Global allocator for WASM heap |
| `libm` | 0.2 | `no_std` float math (`sqrt`, `log`, `cos`, `pow`) |
| `serde` | 1.0 | Serialization (no_std, alloc features) |
| `serde_json` | 1.0 | JSON output for result/policy manifests (no_std, alloc) |

## Determinism

Given identical seeds, the WASM module produces identical results:

- Same seed produces same puzzles (xorshift64 RNG)
- Same puzzles produce same learning trajectory
- Same trajectory produces same witness chain hashes

Minor float precision differences between native and WASM (due to `libm` vs std `f64` methods) may cause Thompson Sampling to diverge over many iterations, but acceptance test outcomes should converge.

## Benchmarks

Run the native reference benchmark:

```bash
cargo run --bin wasm-solver-bench -- --holdout 50 --training 50 --cycles 3
```

Reference results (native):

| Mode | Accuracy | Cost/Solve | Noise Accuracy | Pass |
|------|----------|------------|----------------|------|
| A (baseline) | 100% | ~43 | ~100% | PASS |
| B (compiler) | 100% | ~10 | ~100% | PASS |
| C (learned) | 100% | ~10 | ~100% | PASS |

- B vs A cost decrease: ~76% (threshold: >= 15%)
- Thompson Sampling converges across 13+ context buckets with 3 unique skip modes

## Related ADRs

- [ADR-032](../../../docs/adr/ADR-032-rvf-wasm-integration.md) -- RVF WASM integration
- [ADR-037](../../../docs/adr/ADR-037-publishable-rvf-acceptance-test.md) -- Publishable RVF acceptance test
- [ADR-038](../../../docs/adr/ADR-038-npx-rvlite-witness-verification.md) -- npx/rvlite witness verification
- [ADR-039](../../../docs/adr/ADR-039-rvf-solver-wasm-agi-integration.md) -- RVF solver WASM AGI integration (this crate)

## License

MIT OR Apache-2.0
