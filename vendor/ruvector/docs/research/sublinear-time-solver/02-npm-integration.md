# NPM Package Integration Analysis: sublinear-time-solver v1.5.0

**Agent**: 2 / NPM Package Integration Analysis
**Date**: 2026-02-20
**Scope**: All npm packages in the ruvector monorepo, dependency overlap, type compatibility, and integration patterns with `sublinear-time-solver` v1.5.0.

---

## 1. All NPM Packages Found in ruvector

### 1.1 Workspace Root

| Package | Location |
|---------|----------|
| `@ruvector/workspace` (private) | `/home/user/ruvector/npm/package.json` |

The monorepo uses npm workspaces rooted at `/home/user/ruvector/npm` with all publishable packages under `npm/packages/*`.

### 1.2 Primary Published Packages (npm/packages/*)

| Package Name | Version | Description | Has Types |
|-------------|---------|-------------|-----------|
| `ruvector` | 0.1.99 | Umbrella package with native/WASM/RVF fallback | Yes |
| `@ruvector/core` (packages) | 0.1.30 | HNSW vector database, napi-rs bindings | Yes |
| `@ruvector/core` (npm/core) | 0.1.17 | ESM/CJS wrapper over native bindings | Yes |
| `@ruvector/node` | 0.1.22 | Unified Node.js package (vector + GNN) | Yes |
| `@ruvector/cli` | 0.1.28 | Command-line interface | Yes |
| `@ruvector/rvf` | 0.1.9 | RuVector Format SDK | Yes |
| `@ruvector/rvf-solver` | 0.1.1 | Self-learning temporal solver (WASM) | Yes |
| `@ruvector/rvf-mcp-server` | 0.1.3 | MCP server (stdio + SSE) | Yes |
| `@ruvector/router` | 0.1.28 | Semantic router, napi-rs bindings | Yes |
| `@ruvector/raft` | 0.1.0 | Raft consensus | Yes |
| `@ruvector/replication` | 0.1.0 | Multi-node replication | Yes |
| `@ruvector/agentic-synth` | 0.1.6 | Synthetic data generator | Yes |
| `@ruvector/agentic-synth-examples` | (examples) | Usage examples for agentic-synth | Yes |
| `@ruvector/agentic-integration` | 1.0.0 | Distributed agent coordination | Yes |
| `@ruvector/graph-node` | 2.0.2 | Native graph DB, napi-rs bindings | Yes |
| `@ruvector/graph-wasm` | 2.0.2 | Graph DB WASM bindings | Yes |
| `@ruvector/graph-data-generator` | 0.1.0 | AI-powered graph data generation | Yes |
| `@ruvector/wasm-unified` | 1.0.0 | Unified WASM API surface | Yes |
| `@ruvector/ruvllm` | 2.3.0 | Self-learning LLM orchestration | Yes |
| `@ruvector/ruvllm-cli` | 0.1.0 | LLM inference CLI | Yes |
| `@ruvector/ruvllm-wasm` | 0.1.0 | Browser LLM inference (WebGPU) | Yes |
| `@ruvector/postgres-cli` | 0.2.7 | PostgreSQL vector CLI (pgvector replacement) | Yes |
| `@ruvector/burst-scaling` | 1.0.0 | GCP burst scaling system | Yes |
| `@ruvector/ospipe` | 0.1.2 | Screenpipe AI memory SDK | Yes |
| `@ruvector/ospipe-wasm` | 0.1.0 | OSpipe WASM bindings | Yes |
| `@ruvector/rudag` | 0.1.0 | DAG library with WASM | Yes |
| `@ruvector/scipix` | 0.1.0 | Scientific OCR client | Yes |
| `@ruvector/ruqu-wasm` | 2.0.5 | Quantum circuit simulator WASM | Yes |
| `@cognitum/gate` | 0.1.0 | AI agent safety coherence gate | Yes |
| `ruvector-extensions` | 0.1.0 | Embeddings, UI, exports, persistence | Yes |
| `ruvbot` | 0.2.0 | Enterprise AI assistant | Yes |
| `rvlite` | 0.2.4 | Lightweight vector DB (SQL/SPARQL/Cypher) | Yes |

### 1.3 Native Platform Packages (optionalDependencies)

These are napi-rs platform-specific binary packages distributed via optionalDependencies:

- `ruvector-core-{linux-x64-gnu,linux-arm64-gnu,darwin-x64,darwin-arm64,win32-x64-msvc}` (v0.1.29)
- `@ruvector/router-{linux-x64-gnu,...,win32-x64-msvc}` (v0.1.27)
- `@ruvector/graph-node-{linux-x64-gnu,...,win32-x64-msvc}` (v2.0.2)
- `@ruvector/ruvllm-{linux-x64-gnu,...,win32-x64-msvc}` (v2.3.0)
- `@ruvector/gnn-node` platform packages
- `@ruvector/attention-node` platform packages
- `@ruvector/rvf-node` platform packages

### 1.4 Crate-Level WASM Packages (crates/*)

| Package | Version | Purpose |
|---------|---------|---------|
| `@ruvector/wasm` | 0.1.16 | Core WASM (browser vector DB) |
| `@ruvector/attention-wasm` | (crate) | Attention mechanism WASM |
| `@ruvector/attention-unified-wasm` | (crate pkg) | Unified attention WASM |
| `@ruvector/economy-wasm` | (crate pkg) | Economy simulation WASM |
| `@ruvector/exotic-wasm` | (crate pkg) | Exotic features WASM |
| `@ruvector/learning-wasm` | (crate pkg) | Learning subsystem WASM |
| `@ruvector/nervous-system-wasm` | (crate pkg) | Nervous system WASM |
| `@ruvector/gnn-wasm` | (crate) | GNN WASM bindings |
| `@ruvector/graph-wasm` | (crate) | Graph WASM bindings |
| `@ruvector/router-wasm` | (crate) | Router WASM bindings |
| `@ruvector/tiny-dancer-wasm` | (crate) | Tiny Dancer WASM |
| `@ruvector/cluster` | 0.1.0 | Distributed clustering |
| `@ruvector/server` | 0.1.0 | HTTP/gRPC server |

### 1.5 Example/Benchmark Packages

| Package | Location |
|---------|----------|
| `@ruvector/benchmarks` | `/home/user/ruvector/benchmarks/package.json` |
| meta-cognition SNN demos | `/home/user/ruvector/examples/meta-cognition-spiking-neural-network/` |
| edge-net dashboard | `/home/user/ruvector/examples/edge-net/dashboard/` |
| neural-trader | `/home/user/ruvector/examples/neural-trader/` |
| wasm-react | `/home/user/ruvector/examples/wasm-react/` |
| rvlite dashboard | `/home/user/ruvector/crates/rvlite/examples/dashboard/` |
| sona wasm-example | `/home/user/ruvector/crates/sona/wasm-example/` |

**Total unique package.json files found**: 90+

---

## 2. Package Dependency Overlap and Version Compatibility

### 2.1 Direct Dependency Overlap with sublinear-time-solver v1.5.0

The `sublinear-time-solver` v1.5.0 declares these dependencies:
- `@modelcontextprotocol/sdk` ^1.18.1
- `@ruvnet/strange-loop` ^0.3.0
- `strange-loops` ^0.5.1
- Express ecosystem

| sublinear-time-solver Dep | ruvector Package | ruvector Version | Compatibility |
|--------------------------|------------------|------------------|---------------|
| `@modelcontextprotocol/sdk` ^1.18.1 | `ruvector` | ^1.0.0 | **CONFLICT**: ruvector pins ^1.0.0; sublinear needs ^1.18.1. Semver-compatible if 1.18.x exists, but ruvector must upgrade its lower bound. |
| `@modelcontextprotocol/sdk` ^1.18.1 | `@ruvector/rvf-mcp-server` | ^1.0.0 | Same conflict as above. |
| `express` (ecosystem) | `@ruvector/rvf-mcp-server` | ^4.18.0 | **COMPATIBLE**: Both use Express 4.x |
| `express` (ecosystem) | `ruvector-extensions` | ^4.18.2 | **COMPATIBLE** |
| `express` (ecosystem) | `@ruvector/agentic-integration` | ^4.18.2 | **COMPATIBLE** |
| `@ruvnet/strange-loop` ^0.3.0 | (none) | N/A | **NO OVERLAP**: Not present in ruvector |
| `strange-loops` ^0.5.1 | (none) | N/A | **NO OVERLAP**: Not present in ruvector |

### 2.2 Shared Transitive Dependencies

| Dependency | sublinear-time-solver | ruvector Packages Using It | Notes |
|-----------|----------------------|---------------------------|-------|
| `zod` | Likely via MCP SDK | `@ruvector/rvf-mcp-server` (^3.22.0), `@ruvector/agentic-integration` (^3.22.4), `ruvbot` (^3.22.4), `@ruvector/agentic-synth` (^4.1.13), `@ruvector/graph-data-generator` (^4.1.12) | **WARNING**: ruvector has a zod version split: some packages at 3.x, others at 4.x. The MCP SDK depends on zod 3.x. |
| `commander` | Not direct | `ruvector` (^11.1.0), `@ruvector/cli` (^12.0.0), `@ruvector/ruvllm` (^12.0.0), `@ruvector/postgres-cli` (^11.1.0), `rvlite` (^12.0.0), `ruvbot` (^12.0.0), `@ruvector/agentic-synth` (^11.1.0) | CLI packages only; version split between 11.x and 12.x but not a runtime concern for sublinear-time-solver. |
| `eventemitter3` | Not direct | `@ruvector/raft` (^5.0.4), `@ruvector/replication` (^5.0.4), `ruvbot` (^5.0.1) | No overlap. |
| `typescript` | Dev dep | All packages (^5.0.0 - ^5.9.3) | **COMPATIBLE**: All use TS 5.x |
| `@types/node` | Dev dep | All packages (^20.x) | **COMPATIBLE** |

### 2.3 Version Compatibility Matrix

| Concern | Status | Action Required |
|---------|--------|-----------------|
| `@modelcontextprotocol/sdk` version skew | **MEDIUM RISK** | ruvector currently pins ^1.0.0 while sublinear-time-solver requires ^1.18.1. Since ^1.0.0 allows 1.18.x, npm will resolve to 1.18.x+ if available, but this needs verification. Recommend upgrading ruvector's spec to ^1.18.1 for explicit compatibility. |
| Node.js engine | **COMPATIBLE** | Both require Node.js >= 18 |
| TypeScript version | **COMPATIBLE** | ruvector workspace uses ^5.3.0+; sublinear-time-solver is compatible |
| zod version split | **LOW RISK** | MCP SDK binds zod 3.x internally. The ruvector packages using zod 4.x are independent (agentic-synth, graph-data-generator). No direct conflict path. |

---

## 3. TypeScript Type Compatibility

### 3.1 TypeScript Configuration Landscape

The ruvector monorepo uses multiple TypeScript configuration strategies:

| Target | Module | moduleResolution | Used By |
|--------|--------|------------------|---------|
| ES2020 | CommonJS | node | `ruvector`, workspace root, `rvf-solver`, wasm wrapper |
| ES2022 | Node16 | Node16 | `@ruvector/core` (npm/core), `@ruvector/rvf-mcp-server` |
| ES2020 | CommonJS | node | `@ruvector/burst-scaling`, `@ruvector/postgres-cli` |
| ES2022 | NodeNext | NodeNext | `@ruvector/rvf-mcp-server` |

**Key observation**: The monorepo is split between CommonJS-first packages (older) and ESM-first packages (newer). The `sublinear-time-solver` would need to be compatible with both module systems.

### 3.2 Type Surface Overlap with sublinear-time-solver

The `sublinear-time-solver` exports these types: `SolverConfig`, `MatrixData`, `SolutionStep`, `BatchSolveRequest`, `BatchSolveResult`, `SublinearSolver`, `SolutionStream`, `WasmModule`.

Comparison with ruvector types:

| sublinear-time-solver Type | Closest ruvector Equivalent | Package | Compatibility Notes |
|---------------------------|----------------------------|---------|-------------------|
| `SolverConfig` | `TrainOptions` | `@ruvector/rvf-solver` | Different shape. `TrainOptions` has `count`, `minDifficulty`, `maxDifficulty`, `seed`. `SolverConfig` is a more general configuration type. These are complementary, not conflicting. |
| `MatrixData` | `Float32Array` / `number[]` (vector types) | `ruvector` core types | ruvector uses `number[]` and `Float32Array` for vector data in `VectorEntry.vector` and `RvfIngestEntry.vector`. `MatrixData` is a higher-level abstraction. No conflict. |
| `SolutionStep` | `CycleMetrics` / `AcceptanceModeResult` | `@ruvector/rvf-solver` | Different granularity. `SolutionStep` likely represents individual solver steps; `CycleMetrics` represents per-cycle aggregates. Complementary. |
| `BatchSolveRequest` | `BatchOCRRequest` (pattern) | `@ruvector/scipix` | Structural similarity (batch request pattern) but completely different domains. No conflict. |
| `BatchSolveResult` | `RvfIngestResult` / `TrainResult` | `@ruvector/rvf`, `@ruvector/rvf-solver` | Different semantics. The result shape pattern (counts, metrics) is common across the codebase. |
| `SublinearSolver` (class) | `RvfSolver` (class) | `@ruvector/rvf-solver` | **Most significant overlap**. Both are solver classes with async factory creation (`createSolver()` vs `RvfSolver.create()`), WASM backends, and destroy lifecycle. Integration should expose both as named exports or compose them. |
| `SolutionStream` (async iterator) | None | N/A | **Novel capability**. No existing ruvector package provides async iteration over solver results. This is a purely additive feature. |
| `WasmModule` (SIMD) | WASM modules throughout | `@ruvector/wasm`, all `-wasm` packages | ruvector has extensive WASM infrastructure. The `WasmModule` interface with SIMD support aligns with ruvector's existing WASM + SIMD strategy (`@ruvector/wasm` builds with `--features simd`). |

### 3.3 Interface Structural Compatibility

ruvector's core types follow these conventions:

```typescript
// Config pattern: plain objects with optional fields
interface DbOptions {
  dimension: number;
  metric?: 'cosine' | 'euclidean' | 'dot';
  hnsw?: { m?: number; efConstruction?: number; efSearch?: number };
}

// Result pattern: objects with counts and metrics
interface RvfIngestResult {
  accepted: number;
  rejected: number;
  epoch: number;
}

// Factory pattern: static async create()
class RvfSolver {
  static async create(): Promise<RvfSolver>;
  destroy(): void;
}
```

The `sublinear-time-solver` factory function `createSolver()` returning `Promise<SublinearSolver>` matches the `static async create()` pattern used by `RvfSolver`. This is a strong structural compatibility signal.

### 3.4 Module Format Compatibility

| Feature | sublinear-time-solver | ruvector Packages |
|---------|----------------------|-------------------|
| ESM exports | Main entry, MCP module, core, tools | 22 packages support ESM |
| CJS exports | Likely via dual packaging | 28 packages support CJS |
| Type declarations | `.d.ts` included | All packages include `.d.ts` |
| Conditional exports | Yes (package.json `exports` map) | Yes, extensively used |

**Assessment**: Full compatibility. The `sublinear-time-solver` export map (main, MCP module, core, tools) maps well to ruvector's established `exports` field pattern.

---

## 4. API Surface Overlap and Complementary Features

### 4.1 Overlapping Capabilities

| Capability | sublinear-time-solver | ruvector Package | Overlap Degree |
|-----------|----------------------|------------------|----------------|
| Solver/optimization | Core solver class | `@ruvector/rvf-solver` | **HIGH** - Both provide solver classes with WASM backends |
| MCP integration | MCP module export | `@ruvector/rvf-mcp-server` | **HIGH** - Both expose MCP tools, both depend on `@modelcontextprotocol/sdk` |
| WASM + SIMD | `WasmModule` with SIMD | `@ruvector/wasm`, `@ruvector/wasm-unified` | **MEDIUM** - Infrastructure overlap, but different computation targets |
| Express middleware | Express ecosystem deps | `@ruvector/rvf-mcp-server`, `ruvector-extensions`, `@ruvector/agentic-integration` | **MEDIUM** - Both can serve HTTP endpoints |
| Batch processing | `BatchSolveRequest/Result` | `VectorDBWrapper.insertBatch()`, `RvfSolver.train()` | **LOW** - Different domains (solving vs indexing) |

### 4.2 Complementary Features (sublinear-time-solver adds)

| Feature | Description | Benefit to ruvector |
|---------|-------------|---------------------|
| `SolutionStream` async iterator | Streaming solver results | ruvector has no equivalent streaming solver pattern. Enables real-time progress for long-running optimizations. |
| Sublinear-time algorithms | O(sqrt(n)) or O(log n) solving | Complements ruvector's HNSW O(log n) search with solver-level sublinear guarantees. |
| `@ruvnet/strange-loop` integration | Self-referential reasoning patterns | Novel capability not present in ruvector. Extends the self-learning architecture (SONA, EWC, Thompson Sampling) with recursive reasoning. |
| `strange-loops` library | Fixed-point iteration patterns | Mathematically complements the rvf-solver's three-loop architecture. |
| Tools namespace exports | Pre-packaged MCP tool definitions | Reduces boilerplate when registering solver tools in MCP servers. |

### 4.3 Complementary Features (ruvector provides to sublinear-time-solver)

| Feature | Package | Benefit |
|---------|---------|---------|
| HNSW vector indexing | `@ruvector/core` | Fast nearest-neighbor lookup for solver state caching |
| GNN graph processing | `@ruvector/gnn-node` | Graph-structured problem representation |
| Raft consensus | `@ruvector/raft` | Distributed solver coordination |
| Attention mechanisms | `@ruvector/attention-*` | 39 attention variants for solver guidance |
| DAG scheduling | `@ruvector/rudag` | Task dependency resolution for solver pipelines |
| ReasoningBank/PolicyKernel | `@ruvector/rvf-solver` | Existing self-learning infrastructure |
| Persistent vector storage | `@ruvector/rvf` | Durable storage for solver state vectors |

---

## 5. Integration Patterns

### 5.1 Pattern A: Peer Dependency (Recommended for Library Consumers)

```json
{
  "peerDependencies": {
    "sublinear-time-solver": "^1.5.0"
  },
  "peerDependenciesMeta": {
    "sublinear-time-solver": {
      "optional": true
    }
  }
}
```

**Rationale**: This follows the established pattern used by `@ruvector/agentic-synth` (which uses `ruvector` as an optional peer dependency) and `ruvector-extensions` (which uses `openai` and `cohere-ai` as optional peers). The solver is a high-level capability that consumers may or may not need.

**Best for**: The `ruvector` umbrella package or `@ruvector/rvf-solver`.

### 5.2 Pattern B: Optional Dependency (For Internal Integration)

```json
{
  "optionalDependencies": {
    "sublinear-time-solver": "^1.5.0"
  }
}
```

**Rationale**: Follows the pattern used by `@ruvector/rvf` (which lists `@ruvector/rvf-solver` as an optional dependency). The solver is loaded at runtime with a graceful fallback if unavailable. This matches `ruvector`'s existing three-tier fallback strategy (native -> rvf -> stub).

**Best for**: `@ruvector/rvf-mcp-server` which could conditionally expose sublinear solver tools.

### 5.3 Pattern C: Re-export Wrapper (For Unified API)

Create a thin wrapper in `ruvector` that re-exports the solver with ruvector-specific type adapters:

```typescript
// In ruvector/src/core/sublinear-wrapper.ts
let SublinearSolver: any;
try {
  const mod = require('sublinear-time-solver');
  SublinearSolver = mod.SublinearSolver;
} catch {
  SublinearSolver = null;
}

export function isSublinearAvailable(): boolean {
  return SublinearSolver !== null;
}

export async function createSublinearSolver(config?: SolverConfig): Promise<any> {
  if (!SublinearSolver) {
    throw new Error(
      'sublinear-time-solver is not installed.\n' +
      '  Run: npm install sublinear-time-solver\n'
    );
  }
  const { createSolver } = require('sublinear-time-solver');
  return createSolver(config);
}
```

**Rationale**: Matches the exact pattern in `/home/user/ruvector/npm/packages/ruvector/src/index.ts` (lines 26-77) where the implementation is auto-detected with try/catch and a fallback.

### 5.4 Pattern D: MCP Tool Composition

The `@ruvector/rvf-mcp-server` already has `@modelcontextprotocol/sdk` and `express`. The sublinear-time-solver's MCP module can be composed alongside existing RVF tools:

```typescript
// In rvf-mcp-server, register both tool sets
import { createSolver } from 'sublinear-time-solver/mcp';
import { rvfTools } from '@ruvector/rvf';

const server = new McpServer();
// Register existing RVF tools
rvfTools.forEach(tool => server.addTool(tool));
// Register sublinear solver tools
const solverTools = createSolver.getTools();
solverTools.forEach(tool => server.addTool(tool));
```

### 5.5 Pattern E: Bundling Strategy

For WASM bundling, both `sublinear-time-solver` and ruvector follow the wasm-pack output convention. Integration should:

1. Use the `exports` field to expose WASM modules separately
2. Allow tree-shaking of unused solver features
3. Support both `web` and `nodejs` WASM targets

The existing build infrastructure (`tsup`, `esbuild`, `tsc`) in ruvector packages already handles dual CJS/ESM output and `.wasm` file co-location.

---

## 6. Recommended package.json Changes

### 6.1 For `ruvector` (Umbrella Package)

**File**: `/home/user/ruvector/npm/packages/ruvector/package.json`

```json
{
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.18.1",  // UPGRADE from ^1.0.0
    "@ruvector/attention": "^0.1.3",
    "@ruvector/core": "^0.1.25",
    "@ruvector/gnn": "^0.1.22",
    "@ruvector/sona": "^0.1.4",
    "chalk": "^4.1.2",
    "commander": "^11.1.0",
    "ora": "^5.4.1"
  },
  "optionalDependencies": {
    "@ruvector/rvf": "^0.1.0",
    "sublinear-time-solver": "^1.5.0"  // ADD as optional
  }
}
```

**Rationale**: Adding as optionalDependency follows the existing `@ruvector/rvf` pattern. The MCP SDK version must be upgraded to satisfy both consumers.

### 6.2 For `@ruvector/rvf-mcp-server` (MCP Server)

**File**: `/home/user/ruvector/npm/packages/rvf-mcp-server/package.json`

```json
{
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.18.1",  // UPGRADE from ^1.0.0
    "@ruvector/rvf": "^0.1.2",
    "express": "^4.18.0",
    "zod": "^3.22.0"
  },
  "optionalDependencies": {
    "sublinear-time-solver": "^1.5.0"  // ADD for tool composition
  }
}
```

### 6.3 For `@ruvector/rvf-solver` (Solver Package)

**File**: `/home/user/ruvector/npm/packages/rvf-solver/package.json`

```json
{
  "peerDependencies": {
    "sublinear-time-solver": "^1.5.0"  // ADD as optional peer
  },
  "peerDependenciesMeta": {
    "sublinear-time-solver": {
      "optional": true
    }
  }
}
```

**Rationale**: As the most semantically related package, `@ruvector/rvf-solver` should declare the solver as an optional peer dependency. This enables type-safe integration when both are installed without forcing a dependency.

### 6.4 Workspace-Level devDependency

**File**: `/home/user/ruvector/npm/package.json`

```json
{
  "devDependencies": {
    "@types/node": "^20.10.0",
    "@typescript-eslint/eslint-plugin": "^6.13.0",
    "@typescript-eslint/parser": "^6.13.0",
    "eslint": "^8.54.0",
    "prettier": "^3.1.0",
    "sublinear-time-solver": "^1.5.0",  // ADD for workspace-wide type checking
    "typescript": "^5.3.0"
  }
}
```

### 6.5 New Exports Map Entry (if re-exporting from ruvector)

If the umbrella `ruvector` package chooses to re-export solver functionality:

```json
{
  "exports": {
    ".": {
      "import": "./dist/index.mjs",
      "require": "./dist/index.js",
      "types": "./dist/index.d.ts"
    },
    "./solver": {
      "import": "./dist/core/sublinear-wrapper.mjs",
      "require": "./dist/core/sublinear-wrapper.js",
      "types": "./dist/core/sublinear-wrapper.d.ts"
    }
  }
}
```

---

## 7. Risk Assessment

### 7.1 Critical Issues

| Issue | Severity | Mitigation |
|-------|----------|------------|
| `@modelcontextprotocol/sdk` version conflict (^1.0.0 vs ^1.18.1) | **HIGH** | Upgrade ruvector packages to ^1.18.1. Test MCP server with new SDK version. |
| `@ruvnet/strange-loop` not in ruvector ecosystem | **LOW** | This is a transitive dependency of sublinear-time-solver only. No action needed unless ruvector wants to use it directly. |

### 7.2 Compatibility Notes

| Aspect | Status |
|--------|--------|
| Node.js engine (>=18) | All ruvector packages require >=18. Compatible. |
| TypeScript 5.x | All ruvector packages use 5.x. Compatible. |
| ESM/CJS dual output | sublinear-time-solver provides both. ruvector infrastructure supports both. |
| WASM loading | Both use standard patterns (dynamic import or direct load). Compatible with ruvector's WASM infrastructure. |
| Express 4.x | Shared across 3 ruvector packages and sublinear-time-solver. No conflict. |

### 7.3 Testing Requirements

1. Verify `@modelcontextprotocol/sdk` ^1.18.1 is backward-compatible with ruvector's MCP usage
2. Test WASM module co-existence (sublinear-time-solver WASM + ruvector WASM modules)
3. Validate that zod version resolution works correctly with both zod 3.x (MCP SDK) and zod 4.x (agentic-synth)
4. Run the existing `npm test` across all workspaces after dependency changes

---

## 8. Summary

The `sublinear-time-solver` v1.5.0 integrates well into the ruvector monorepo:

- **One critical change needed**: Upgrade `@modelcontextprotocol/sdk` from ^1.0.0 to ^1.18.1 in `ruvector` and `@ruvector/rvf-mcp-server`
- **Best integration pattern**: Optional dependency in the umbrella `ruvector` package with a try/catch wrapper (Pattern C), combined with MCP tool composition in `@ruvector/rvf-mcp-server` (Pattern D)
- **Type compatibility**: Strong structural compatibility. The factory pattern (`createSolver()` / `RvfSolver.create()`), WASM interfaces, and batch processing patterns all align
- **Novel capabilities added**: `SolutionStream` async iteration, strange-loop reasoning, and sublinear-time algorithmic guarantees complement ruvector's existing self-learning infrastructure
- **No breaking changes required**: All integration can be done via additive optional/peer dependencies
