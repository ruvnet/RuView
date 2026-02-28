# ADR-039: RVF Solver WASM — Self-Learning AGI Engine Integration

| Field | Value |
|-------|-------|
| **Status** | Implemented |
| **Date** | 2026-02-16 (updated 2026-02-17) |
| **Deciders** | RuVector core team |
| **Supersedes** | -- |
| **Related** | ADR-032 (RVF WASM integration), ADR-037 (Publishable RVF acceptance test), ADR-038 (npx/rvlite witness verification) |

## Context

ADR-037 established the publishable RVF acceptance test with a SHAKE-256 witness chain, and ADR-038 planned npm integration for **verifying** those artifacts. However, neither the existing `rvf-wasm` microkernel nor the npm packages expose the actual self-learning engine that produces the AGI benchmarks.

The core AGI capabilities live exclusively in the Rust benchmarks crate (`examples/benchmarks/src/`):
- **PolicyKernel**: Thompson Sampling two-signal model (safety Beta + cost EMA)
- **KnowledgeCompiler**: Signature-based pattern cache with compiled skip-mode configs
- **AdaptiveSolver**: Three-loop architecture (fast: solve, medium: policy, slow: compiler)
- **ReasoningBank**: Trajectory tracking with checkpoint/rollback and non-regression gating
- **Acceptance test**: Multi-cycle training/holdout evaluation with three ablation modes

These components have no FFI dependencies, no filesystem access during solve, and no system clock requirements — making them ideal candidates for WASM compilation.

## Decision

### Create `rvf-solver-wasm` as a standalone no_std WASM module

A new crate at `crates/rvf/rvf-solver-wasm/` compiles the complete self-learning solver to `wasm32-unknown-unknown`. It is a `no_std + alloc` crate (same architecture as `rvf-wasm`) with a C ABI export surface.

**Key design choices:**

| Choice | Rationale |
|--------|-----------|
| **no_std + alloc** | Matches rvf-wasm pattern, runs in any WASM runtime (browser, Node.js, edge) |
| **Self-contained types** | Pure-integer `Date` type replaces `chrono` dependency; `BTreeMap` replaces `HashMap` |
| **libm for float math** | `sqrt`, `log`, `cos`, `pow` via `libm` crate (pure Rust, no_std compatible) |
| **xorshift64 RNG** | Deterministic, no `rand` crate dependency, identical to benchmarks RNG |
| **C ABI exports** | Maximum compatibility — works with any WASM host (no wasm-bindgen required) |
| **Handle-based API** | Up to 8 concurrent solver instances, same pattern as `rvf_store_*` exports |

### WASM Export Surface

```
┌─────────────────────────────────────────────────────┐
│              rvf-solver-wasm exports                │
├─────────────────────────────────────────────────────┤
│ Memory:                                             │
│   rvf_solver_alloc(size) -> ptr                     │
│   rvf_solver_free(ptr, size)                        │
│                                                     │
│ Lifecycle:                                          │
│   rvf_solver_create() -> handle                     │
│   rvf_solver_destroy(handle)                        │
│                                                     │
│ Training (three-loop learning):                     │
│   rvf_solver_train(handle, count,                   │
│     min_diff, max_diff, seed_lo, seed_hi) -> i32    │
│                                                     │
│ Acceptance test (full ablation):                    │
│   rvf_solver_acceptance(handle, holdout,            │
│     training, cycles, budget,                       │
│     seed_lo, seed_hi) -> i32                        │
│                                                     │
│ Result / Policy / Witness reads:                    │
│   rvf_solver_result_len(handle) -> i32              │
│   rvf_solver_result_read(handle, out_ptr) -> i32    │
│   rvf_solver_policy_len(handle) -> i32              │
│   rvf_solver_policy_read(handle, out_ptr) -> i32    │
│   rvf_solver_witness_len(handle) -> i32             │
│   rvf_solver_witness_read(handle, out_ptr) -> i32   │
└─────────────────────────────────────────────────────┘
```

### Architecture Preserved in WASM

The WASM module preserves all five AGI capabilities:

1. **Thompson Sampling two-signal model** — Beta posterior for safety (correct & no early-commit) + EMA for cost. Gamma sampling via Marsaglia's method using `libm`.

2. **18 context buckets** — 3 range (small/medium/large) x 3 distractor (clean/some/heavy) x 2 noise = 18 buckets. Each bucket maintains per-arm stats for `None`, `Weekday`, `Hybrid` skip modes.

3. **Speculative dual-path** — When top-2 arms are within delta 0.15 and variance > 0.02, the solver speculatively executes the secondary arm. This is preserved identically in WASM.

4. **KnowledgeCompiler** — Constraint signature cache (`v1:{difficulty}:{sorted_constraint_types}`). Compiles successful trajectories into optimized configs with compiled skip-mode, step budget, and confidence scores.

5. **Three-loop solver** — Fast (constraint propagation + solve), Medium (PolicyKernel selection), Slow (ReasoningBank → KnowledgeCompiler). Checkpoint/rollback on accuracy regression.

### Integration with RVF Ecosystem

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

The solver produces a SHAKE-256 witness chain (via `rvf_crypto::create_witness_chain`) for every acceptance test run. This chain is in the native 73-byte-per-entry format, directly verifiable by `rvf_witness_verify` in the rvf-wasm microkernel.

### npm Integration Path

#### High-Level SDK (`@ruvector/rvf-solver`)

The `@ruvector/rvf-solver` npm package provides a typed TypeScript wrapper around the raw WASM C-ABI exports, with automatic WASM loading, memory management, and JSON deserialization.

```typescript
import { RvfSolver } from '@ruvector/rvf-solver';

// Create solver (lazy-loads WASM on first call)
const solver = await RvfSolver.create();

// Train on 1000 puzzles (three-loop learning)
const result = solver.train({ count: 1000, minDifficulty: 1, maxDifficulty: 10, seed: 42n });
console.log(`Accuracy: ${(result.accuracy * 100).toFixed(1)}%`);

// Run full acceptance test (A/B/C ablation)
const manifest = solver.acceptance({ holdoutSize: 100, trainingPerCycle: 100, cycles: 5, seed: 42n });
console.log(`Mode C passed: ${manifest.allPassed}`);

// Inspect policy state (Thompson Sampling parameters, context buckets)
const policy = solver.policy();
console.log(`Context buckets: ${Object.keys(policy?.contextStats ?? {}).length}`);

// Get tamper-evident witness chain (73 bytes per entry, SHAKE-256)
const chain = solver.witnessChain();
console.log(`Witness chain: ${chain?.length ?? 0} bytes`);

solver.destroy();
```

The SDK also re-exports through the unified `@ruvector/rvf` package:

```typescript
// Unified import — solver + database in one package
import { RvfDatabase, RvfSolver } from '@ruvector/rvf';
```

#### npm Package Structure

```
npm/packages/rvf-solver/
├── package.json          # @ruvector/rvf-solver, CJS/ESM dual exports
├── tsconfig.json         # ES2020 target, strict mode, declarations
├── pkg/
│   ├── rvf_solver.js     # WASM loader (singleton, Node CJS/ESM + browser)
│   ├── rvf_solver.d.ts   # Low-level WASM C-ABI type declarations
│   └── rvf_solver_bg.wasm  # Built from rvf-solver-wasm crate
└── src/
    ├── index.ts          # Barrel exports: RvfSolver + all types
    ├── solver.ts         # RvfSolver class (create/train/acceptance/policy/witnessChain/destroy)
    └── types.ts          # TrainOptions, AcceptanceManifest, PolicyState, etc.
```

| Type | Fields | Purpose |
|------|--------|---------|
| `TrainOptions` | `count`, `minDifficulty?`, `maxDifficulty?`, `seed?` | Configure training run |
| `TrainResult` | `trained`, `correct`, `accuracy`, `patternsLearned` | Training outcome |
| `AcceptanceOptions` | `holdoutSize?`, `trainingPerCycle?`, `cycles?`, `stepBudget?`, `seed?` | Configure acceptance test |
| `AcceptanceManifest` | `modeA`, `modeB`, `modeC`, `allPassed`, `witnessEntries`, `witnessChainBytes` | Full ablation results |
| `PolicyState` | `contextStats`, `earlyCommitPenalties`, `prepass`, `speculativeAttempts` | Thompson Sampling state |
| `SkipModeStats` | `attempts`, `successes`, `alphaSafety`, `betaSafety`, `costEma` | Per-arm bandit stats |

#### Low-Level WASM Usage (advanced)

```javascript
// Direct WASM C-ABI usage (without the SDK wrapper)
const wasm = await WebAssembly.instantiate(solverModule);

const handle = wasm.exports.rvf_solver_create();
const correct = wasm.exports.rvf_solver_train(handle, 1000, 1, 10, 42, 0);

const len = wasm.exports.rvf_solver_result_len(handle);
const ptr = wasm.exports.rvf_solver_alloc(len);
wasm.exports.rvf_solver_result_read(handle, ptr);
const json = new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, ptr, len));

// Witness chain verifiable by rvf-wasm
const wLen = wasm.exports.rvf_solver_witness_len(handle);
const wPtr = wasm.exports.rvf_solver_alloc(wLen);
wasm.exports.rvf_solver_witness_read(handle, wPtr);
const chain = new Uint8Array(wasm.memory.buffer, wPtr, wLen);
const verified = rvfWasm.exports.rvf_witness_verify(chainPtr, wLen);

wasm.exports.rvf_solver_destroy(handle);
```

## Module Structure

```
crates/rvf/rvf-solver-wasm/
├── Cargo.toml          # no_std + alloc, dlmalloc, libm, serde_json
├── src/
│   ├── lib.rs          # WASM exports, instance registry, panic handler
│   ├── alloc_setup.rs  # dlmalloc global allocator, rvf_solver_alloc/free
│   ├── types.rs        # Date arithmetic, Constraint, Puzzle, Rng64
│   ├── policy.rs       # PolicyKernel, Thompson Sampling, KnowledgeCompiler
│   └── engine.rs       # AdaptiveSolver, ReasoningBank, PuzzleGenerator, acceptance test
```

| File | Lines | Purpose |
|------|-------|---------|
| `types.rs` | 239 | Pure-integer date math (Howard Hinnant algorithm), constraints, puzzle type |
| `policy.rs` | ~480 | Full Thompson Sampling with Marsaglia gamma sampling, 18-bucket context |
| `engine.rs` | ~490 | Three-loop solver, acceptance test runner, puzzle generator |
| `lib.rs` | ~280 | 12 WASM exports, handle registry (8 slots), witness chain integration |

## Binary Size

| Build | Size |
|-------|------|
| Release (wasm32-unknown-unknown) | ~171 KB |
| After wasm-opt -Oz | 132 KB |

## npm Package Ecosystem

The AGI solver is exposed through a layered npm package architecture:

| Package | Version | Role | Install |
|---------|---------|------|---------|
| `@ruvector/rvf-solver` | 0.1.3 | Typed TypeScript SDK for the self-learning solver | `npm i @ruvector/rvf-solver` |
| `@ruvector/rvf` | 0.1.9 | Unified SDK re-exporting solver + database | `npm i @ruvector/rvf` |
| `@ruvector/rvf-node` | 0.1.7 | Native NAPI bindings with AGI methods (`indexStats`, `verifyWitness`, `freeze`, `metric`) | `npm i @ruvector/rvf-node` |
| `@ruvector/rvf-wasm` | 0.1.6 | WASM microkernel with witness verification | `npm i @ruvector/rvf-wasm` |

### Dependency Graph

```
@ruvector/rvf (unified SDK)
├── @ruvector/rvf-node (required, native NAPI)
├── @ruvector/rvf-wasm (optional, browser fallback)
└── @ruvector/rvf-solver (optional, AGI solver)
    └── rvf-solver-wasm WASM binary (loaded at runtime)
```

### AGI NAPI Methods (rvf-node)

The native NAPI bindings expose AGI-relevant methods beyond basic vector CRUD:

| Method | Returns | Purpose |
|--------|---------|---------|
| `indexStats()` | `RvfIndexStats` | HNSW index statistics (layers, M, ef_construction, indexed count) |
| `verifyWitness()` | `RvfWitnessResult` | Verify tamper-evident SHAKE-256 witness chain integrity |
| `freeze()` | `void` | Snapshot-freeze current state for deterministic replay |
| `metric()` | `string` | Get distance metric name (`l2`, `cosine`, `dotproduct`) |

## Consequences

### Positive

- The actual self-learning AGI engine runs in the browser, Node.js, and edge runtimes via WASM
- No Rust toolchain required for end users — `npm install` + WASM load is sufficient
- Deterministic: same seed → same puzzles → same learning → same witness chain
- Witness chains produced in WASM are verifiable by the existing `rvf_witness_verify` export
- PolicyKernel state is inspectable via `rvf_solver_policy_read` (JSON serializable)
- Handle-based API supports up to 8 concurrent solver instances
- 132 KB binary (after wasm-opt -Oz) includes the complete solver, Thompson Sampling, and serde_json
- TypeScript SDK (`@ruvector/rvf-solver`) provides ergonomic async API with automatic WASM memory management
- Unified SDK (`@ruvector/rvf`) re-exports solver alongside database for single-import usage
- Native NAPI bindings expose AGI methods (index stats, witness verification, freeze) for server-side usage

### Negative

- Date arithmetic is reimplemented (pure-integer) rather than using `chrono`, requiring validation against the original
- `HashMap` → `BTreeMap` changes iteration order (sorted vs hash-order), which may produce different witness chain hashes than the native benchmarks
- Float math via `libm` may have minor precision differences vs std `f64` methods, affecting Thompson Sampling distributions
- The puzzle generator is simplified compared to the full benchmarks generator (no cross-cultural constraints)

### Neutral

- The native benchmarks crate remains the reference implementation for full-fidelity acceptance tests
- The WASM module is a faithful port, not a binding — both implementations should converge on the same acceptance test outcomes given identical seeds
- `rvf-solver-wasm` is a member of the `crates/rvf` workspace alongside `rvf-wasm`

### Implementation Notes (2026-02-17)

- WASM loader (`pkg/rvf_solver.js`) rewritten as pure CJS to fix ESM/CJS interop — `import.meta.url` and `export default` removed
- Snake_case → camelCase field mapping added in `solver.ts` for `train()`, `policy()`, and `acceptance()` methods
- `AcceptanceModeResult` type updated to match actual WASM output: `passed`, `accuracyMaintained`, `costImproved`, `robustnessImproved`, `zeroViolations`, `dimensionsImproved`, `cycles[]`
- SDK tests added at `npm/packages/rvf-solver/test/solver.test.mjs` (import validation, type structure, WASM integration)
- Security review completed: WASM loader path validation flagged as LOW risk (library-internal API), `JSON.parse` on WASM memory is trusted

---

## Appendix: Public Package Documentation

# @ruvector/rvf-solver

[![npm](https://img.shields.io/npm/v/@ruvector/rvf-solver)](https://www.npmjs.com/package/@ruvector/rvf-solver)
[![license](https://img.shields.io/npm/l/@ruvector/rvf-solver)](https://github.com/ruvnet/ruvector/blob/main/LICENSE)
![platforms](https://img.shields.io/badge/platforms-Node.js%20%7C%20Browser%20%7C%20Edge-blue)

Self-learning temporal solver with Thompson Sampling, PolicyKernel, ReasoningBank, and SHAKE-256 tamper-evident witness chains. Runs in the browser, Node.js, and edge runtimes via WebAssembly.

### Install

```bash
npm install @ruvector/rvf-solver
```

Or via the unified SDK:

```bash
npm install @ruvector/rvf
```

### Features

- **Thompson Sampling two-signal model** — safety Beta distribution + cost EMA for adaptive policy selection
- **18 context-bucketed bandits** — 3 range x 3 distractor x 2 noise levels for fine-grained context awareness
- **KnowledgeCompiler with signature-based pattern cache** — distills learned patterns into reusable compiled configurations
- **Speculative dual-path execution** — runs two candidate arms in parallel, picks the winner
- **Three-loop adaptive solver** — fast: constraint propagation solve, medium: PolicyKernel skip-mode selection, slow: KnowledgeCompiler pattern distillation
- **SHAKE-256 tamper-evident witness chain** — 73 bytes per entry, cryptographically linked proof of all operations
- **Full acceptance test with A/B/C ablation modes** — validates learned policy outperforms fixed and compiler baselines
- **~132 KB WASM binary, `no_std`** — runs anywhere WebAssembly does (browsers, Node.js, Deno, Cloudflare Workers, edge runtimes)

### Quick Start

```typescript
import { RvfSolver } from '@ruvector/rvf-solver';

// Create a solver instance (loads WASM on first call)
const solver = await RvfSolver.create();

// Train on 100 puzzles (difficulty 1-5)
const result = solver.train({ count: 100, minDifficulty: 1, maxDifficulty: 5 });
console.log(`Accuracy: ${(result.accuracy * 100).toFixed(1)}%`);
console.log(`Patterns learned: ${result.patternsLearned}`);

// Run full acceptance test (A/B/C ablation)
const manifest = solver.acceptance({ cycles: 3 });
console.log(`All passed: ${manifest.allPassed}`);

// Inspect Thompson Sampling policy state
const policy = solver.policy();
console.log(`Context buckets: ${Object.keys(policy?.contextStats ?? {}).length}`);
console.log(`Speculative attempts: ${policy?.speculativeAttempts}`);

// Get raw SHAKE-256 witness chain
const chain = solver.witnessChain();
console.log(`Witness chain: ${chain?.length ?? 0} bytes`);

// Free WASM resources
solver.destroy();
```

### API Reference

#### `RvfSolver.create(): Promise<RvfSolver>`

Creates a new solver instance. Initializes the WASM module on the first call; subsequent calls reuse the loaded module. Up to 8 concurrent instances are supported.

#### `solver.train(options: TrainOptions): TrainResult`

Trains the solver on randomly generated puzzles using the three-loop architecture. The fast loop applies constraint propagation, the medium loop selects skip modes via Thompson Sampling, and the slow loop distills patterns into the KnowledgeCompiler cache.

#### `solver.acceptance(options?: AcceptanceOptions): AcceptanceManifest`

Runs the full acceptance test with training/holdout cycles across all three ablation modes (A, B, C). Returns a manifest with per-cycle metrics, pass/fail status, and witness chain metadata.

#### `solver.policy(): PolicyState | null`

Returns the current Thompson Sampling policy state including per-context-bucket arm statistics, KnowledgeCompiler cache stats, and speculative execution counters. Returns `null` if no training has been performed.

#### `solver.witnessChain(): Uint8Array | null`

Returns the raw SHAKE-256 witness chain bytes. Each entry is 73 bytes and provides tamper-evident proof of all training and acceptance operations. Returns `null` if the chain is empty. The returned `Uint8Array` is a copy safe to use after `destroy()`.

#### `solver.destroy(): void`

Frees the WASM solver instance and releases all associated memory. The instance must not be used after calling `destroy()`.

### Types

#### TrainOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `count` | `number` | required | Number of puzzles to generate and solve |
| `minDifficulty` | `number` | `1` | Minimum puzzle difficulty (1-10) |
| `maxDifficulty` | `number` | `10` | Maximum puzzle difficulty (1-10) |
| `seed` | `bigint \| number` | random | RNG seed for reproducible runs |

#### TrainResult

| Field | Type | Description |
|-------|------|-------------|
| `trained` | `number` | Number of puzzles trained on |
| `correct` | `number` | Number solved correctly |
| `accuracy` | `number` | Accuracy ratio (correct / trained) |
| `patternsLearned` | `number` | Patterns distilled by the ReasoningBank |

#### AcceptanceOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `holdoutSize` | `number` | `50` | Number of holdout puzzles per cycle |
| `trainingPerCycle` | `number` | `200` | Number of training puzzles per cycle |
| `cycles` | `number` | `5` | Number of train/test cycles |
| `stepBudget` | `number` | `500` | Maximum constraint propagation steps per puzzle |
| `seed` | `bigint \| number` | random | RNG seed for reproducible runs |

#### AcceptanceManifest

| Field | Type | Description |
|-------|------|-------------|
| `version` | `number` | Manifest schema version |
| `modeA` | `AcceptanceModeResult` | Mode A results (fixed heuristic) |
| `modeB` | `AcceptanceModeResult` | Mode B results (compiler-suggested) |
| `modeC` | `AcceptanceModeResult` | Mode C results (learned policy) |
| `allPassed` | `boolean` | `true` if Mode C passed |
| `witnessEntries` | `number` | Number of entries in the witness chain |
| `witnessChainBytes` | `number` | Total witness chain size in bytes |

#### AcceptanceModeResult

| Field | Type | Description |
|-------|------|-------------|
| `passed` | `boolean` | Whether this mode met the accuracy threshold |
| `accuracyMaintained` | `boolean` | Accuracy maintained across cycles |
| `costImproved` | `boolean` | Cost per solve improved |
| `robustnessImproved` | `boolean` | Noise robustness improved |
| `zeroViolations` | `boolean` | No constraint violations |
| `dimensionsImproved` | `number` | Number of dimensions that improved |
| `cycles` | `CycleMetrics[]` | Per-cycle accuracy and cost metrics |

#### PolicyState

| Field | Type | Description |
|-------|------|-------------|
| `contextStats` | `Record<string, Record<string, SkipModeStats>>` | Per-context-bucket, per-arm Thompson Sampling statistics |
| `earlyCommitPenalties` | `number` | Total early-commit penalty cost |
| `earlyCommitsTotal` | `number` | Total early-commit attempts |
| `earlyCommitsWrong` | `number` | Early commits that were incorrect |
| `prepass` | `string` | Current prepass strategy identifier |
| `speculativeAttempts` | `number` | Number of speculative dual-path executions |
| `speculativeArm2Wins` | `number` | Times the second speculative arm won |

### Acceptance Test Modes

The acceptance test validates the solver's learning capability through three ablation modes run across multiple train/test cycles:

**Mode A (Fixed)** -- Uses a fixed heuristic skip-mode policy. This establishes the baseline performance without any learning. The policy does not adapt regardless of puzzle characteristics.

**Mode B (Compiler)** -- Uses the KnowledgeCompiler's signature-based pattern cache to select skip modes. The compiler distills observed patterns into compiled configurations but does not perform online Thompson Sampling updates.

**Mode C (Learned)** -- Uses the full Thompson Sampling two-signal model with context-bucketed bandits. This is the complete system: the fast loop solves, the medium loop selects arms based on safety Beta and cost EMA, and the slow loop feeds patterns back to the compiler. Mode C should outperform both A and B, demonstrating genuine self-improvement.

The test passes when Mode C achieves the accuracy threshold on holdout puzzles. The witness chain records every training and evaluation operation for tamper-evident auditability.

### Architecture

The solver uses a three-loop adaptive architecture:

```
+-----------------------------------------------+
|  Slow Loop: KnowledgeCompiler                  |
|  - Signature-based pattern cache               |
|  - Distills observations into compiled configs  |
+-----------------------------------------------+
        |                          ^
        v                          |
+-----------------------------------------------+
|  Medium Loop: PolicyKernel                     |
|  - Thompson Sampling (safety Beta + cost EMA)  |
|  - 18 context buckets (range x distractor x noise) |
|  - Speculative dual-path execution             |
+-----------------------------------------------+
        |                          ^
        v                          |
+-----------------------------------------------+
|  Fast Loop: Constraint Propagation Solver      |
|  - Generates and solves puzzles                |
|  - Reports outcomes back to PolicyKernel       |
+-----------------------------------------------+
        |
        v
+-----------------------------------------------+
|  SHAKE-256 Witness Chain (73 bytes/entry)      |
|  - Tamper-evident proof of all operations      |
+-----------------------------------------------+
```

### Unified SDK

When using the `@ruvector/rvf` unified SDK, the solver is available as a sub-module:

```typescript
import { RvfSolver } from '@ruvector/rvf';

const solver = await RvfSolver.create();
const result = solver.train({ count: 100 });
console.log(`Accuracy: ${(result.accuracy * 100).toFixed(1)}%`);
solver.destroy();
```

### Related Packages

| Package | Description |
|---------|-------------|
| [`@ruvector/rvf`](https://www.npmjs.com/package/@ruvector/rvf) | Unified TypeScript SDK |
| [`@ruvector/rvf-node`](https://www.npmjs.com/package/@ruvector/rvf-node) | Native N-API bindings for Node.js |
| [`@ruvector/rvf-wasm`](https://www.npmjs.com/package/@ruvector/rvf-wasm) | Browser WASM package |
| [`@ruvector/rvf-mcp-server`](https://www.npmjs.com/package/@ruvector/rvf-mcp-server) | MCP server for AI agents |
