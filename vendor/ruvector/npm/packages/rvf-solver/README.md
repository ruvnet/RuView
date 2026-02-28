# @ruvector/rvf-solver

[![npm](https://img.shields.io/npm/v/@ruvector/rvf-solver)](https://www.npmjs.com/package/@ruvector/rvf-solver)
[![license](https://img.shields.io/npm/l/@ruvector/rvf-solver)](https://github.com/ruvnet/ruvector/blob/main/LICENSE)
![platforms](https://img.shields.io/badge/platforms-Node.js%20%7C%20Browser%20%7C%20Edge-blue)

Self-learning temporal solver with Thompson Sampling, PolicyKernel, ReasoningBank, and SHAKE-256 tamper-evident witness chains. Runs in the browser, Node.js, and edge runtimes via WebAssembly.

## Install

```bash
npm install @ruvector/rvf-solver
```

Or via the unified SDK:

```bash
npm install @ruvector/rvf
```

## Features

- **Thompson Sampling two-signal model** — safety Beta distribution + cost EMA for adaptive policy selection
- **18 context-bucketed bandits** — 3 range x 3 distractor x 2 noise levels for fine-grained context awareness
- **KnowledgeCompiler with signature-based pattern cache** — distills learned patterns into reusable compiled configurations
- **Speculative dual-path execution** — runs two candidate arms in parallel, picks the winner
- **Three-loop adaptive solver** — fast: constraint propagation solve, medium: PolicyKernel skip-mode selection, slow: KnowledgeCompiler pattern distillation
- **SHAKE-256 tamper-evident witness chain** — 73 bytes per entry, cryptographically linked proof of all operations
- **Full acceptance test with A/B/C ablation modes** — validates learned policy outperforms fixed and compiler baselines
- **~160 KB WASM binary, `no_std`** — runs anywhere WebAssembly does (browsers, Node.js, Deno, Cloudflare Workers, edge runtimes)

## Quick Start

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
console.log(`Mode A (fixed):    ${manifest.modeA.finalAccuracy.toFixed(3)}`);
console.log(`Mode B (compiler): ${manifest.modeB.finalAccuracy.toFixed(3)}`);
console.log(`Mode C (learned):  ${manifest.modeC.finalAccuracy.toFixed(3)}`);
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

## API Reference

### `RvfSolver.create(): Promise<RvfSolver>`

Creates a new solver instance. Initializes the WASM module on the first call; subsequent calls reuse the loaded module. Up to 7 concurrent instances are supported.

```typescript
const solver = await RvfSolver.create();
```

### `solver.train(options: TrainOptions): TrainResult`

Trains the solver on randomly generated puzzles using the three-loop architecture. The fast loop applies constraint propagation, the medium loop selects skip modes via Thompson Sampling, and the slow loop distills patterns into the KnowledgeCompiler cache.

```typescript
const result = solver.train({ count: 200, minDifficulty: 1, maxDifficulty: 10 });
```

### `solver.acceptance(options?: AcceptanceOptions): AcceptanceManifest`

Runs the full acceptance test with training/holdout cycles across all three ablation modes (A, B, C). Returns a manifest with per-cycle metrics, pass/fail status, and witness chain metadata.

```typescript
const manifest = solver.acceptance({ cycles: 5, holdoutSize: 50 });
```

### `solver.policy(): PolicyState | null`

Returns the current Thompson Sampling policy state including per-context-bucket arm statistics, KnowledgeCompiler cache stats, and speculative execution counters. Returns `null` if no training has been performed.

```typescript
const policy = solver.policy();
```

### `solver.witnessChain(): Uint8Array | null`

Returns the raw SHAKE-256 witness chain bytes. Each entry is 73 bytes and provides tamper-evident proof of all training and acceptance operations. Returns `null` if the chain is empty. The returned `Uint8Array` is a copy safe to use after `destroy()`.

```typescript
const chain = solver.witnessChain();
```

### `solver.destroy(): void`

Frees the WASM solver instance and releases all associated memory. The instance must not be used after calling `destroy()`.

```typescript
solver.destroy();
```

## Types

### TrainOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `count` | `number` | required | Number of puzzles to generate and solve |
| `minDifficulty` | `number` | `1` | Minimum puzzle difficulty (1-10) |
| `maxDifficulty` | `number` | `10` | Maximum puzzle difficulty (1-10) |
| `seed` | `bigint \| number` | random | RNG seed for reproducible runs |

### TrainResult

| Field | Type | Description |
|-------|------|-------------|
| `trained` | `number` | Number of puzzles trained on |
| `correct` | `number` | Number solved correctly |
| `accuracy` | `number` | Accuracy ratio (correct / trained) |
| `patternsLearned` | `number` | Patterns distilled by the ReasoningBank |

### AcceptanceOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `holdoutSize` | `number` | `50` | Number of holdout puzzles per cycle |
| `trainingPerCycle` | `number` | `200` | Number of training puzzles per cycle |
| `cycles` | `number` | `5` | Number of train/test cycles |
| `stepBudget` | `number` | `500` | Maximum constraint propagation steps per puzzle |
| `seed` | `bigint \| number` | random | RNG seed for reproducible runs |

### AcceptanceManifest

| Field | Type | Description |
|-------|------|-------------|
| `version` | `number` | Manifest schema version |
| `modeA` | `AcceptanceModeResult` | Mode A results (fixed heuristic) |
| `modeB` | `AcceptanceModeResult` | Mode B results (compiler-suggested) |
| `modeC` | `AcceptanceModeResult` | Mode C results (learned policy) |
| `allPassed` | `boolean` | `true` if Mode C passed |
| `witnessEntries` | `number` | Number of entries in the witness chain |
| `witnessChainBytes` | `number` | Total witness chain size in bytes |

### AcceptanceModeResult

| Field | Type | Description |
|-------|------|-------------|
| `passed` | `boolean` | Whether this mode met the accuracy threshold |
| `finalAccuracy` | `number` | Accuracy on the final holdout cycle |
| `cycles` | `CycleMetrics[]` | Per-cycle accuracy and cost metrics |

### PolicyState

| Field | Type | Description |
|-------|------|-------------|
| `contextStats` | `Record<string, Record<string, SkipModeStats>>` | Per-context-bucket, per-arm Thompson Sampling statistics |
| `earlyCommitPenalties` | `number` | Total early-commit penalty cost |
| `earlyCommitsTotal` | `number` | Total early-commit attempts |
| `earlyCommitsWrong` | `number` | Early commits that were incorrect |
| `prepass` | `string` | Current prepass strategy identifier |
| `speculativeAttempts` | `number` | Number of speculative dual-path executions |
| `speculativeArm2Wins` | `number` | Times the second speculative arm won |

## Acceptance Test Modes

The acceptance test validates the solver's learning capability through three ablation modes run across multiple train/test cycles:

**Mode A (Fixed)** -- Uses a fixed heuristic skip-mode policy. This establishes the baseline performance without any learning. The policy does not adapt regardless of puzzle characteristics.

**Mode B (Compiler)** -- Uses the KnowledgeCompiler's signature-based pattern cache to select skip modes. The compiler distills observed patterns into compiled configurations but does not perform online Thompson Sampling updates.

**Mode C (Learned)** -- Uses the full Thompson Sampling two-signal model with context-bucketed bandits. This is the complete system: the fast loop solves, the medium loop selects arms based on safety Beta and cost EMA, and the slow loop feeds patterns back to the compiler. Mode C should outperform both A and B, demonstrating genuine self-improvement.

The test passes when Mode C achieves the accuracy threshold on holdout puzzles. The witness chain records every training and evaluation operation for tamper-evident auditability.

## Architecture

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

The fast loop runs on every puzzle, the medium loop updates policy parameters after each solve, and the slow loop periodically compiles accumulated observations into cached patterns. All operations are recorded in the SHAKE-256 witness chain.

## Unified SDK

When using the `@ruvector/rvf` unified SDK, the solver is available as a sub-module:

```typescript
import { RvfSolver } from '@ruvector/rvf';

const solver = await RvfSolver.create();
const result = solver.train({ count: 100 });
console.log(`Accuracy: ${(result.accuracy * 100).toFixed(1)}%`);
solver.destroy();
```

## Related Packages

| Package | Description |
|---------|-------------|
| [`@ruvector/rvf`](https://www.npmjs.com/package/@ruvector/rvf) | Unified TypeScript SDK |
| [`@ruvector/rvf-node`](https://www.npmjs.com/package/@ruvector/rvf-node) | Native N-API bindings for Node.js |
| [`@ruvector/rvf-wasm`](https://www.npmjs.com/package/@ruvector/rvf-wasm) | Browser WASM package |
| [`@ruvector/rvf-mcp-server`](https://www.npmjs.com/package/@ruvector/rvf-mcp-server) | MCP server for AI agents |

## License

MIT OR Apache-2.0

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [RVF Format Specification](https://github.com/ruvnet/ruvector/tree/main/crates/rvf)
- [npm Package](https://www.npmjs.com/package/@ruvector/rvf-solver)
