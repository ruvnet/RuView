# ADR-032: RVF WASM Integration into npx ruvector and rvlite

**Status**: Accepted
**Date**: 2026-02-14
**Deciders**: ruv.io Team
**Supersedes**: None
**Related**: ADR-030 (RVF Cognitive Container), ADR-031 (RVCOW Branching)

---

## Context

The RuVector Format (RVF) ecosystem now ships four npm packages:

| Package | Purpose | Size |
|---------|---------|------|
| `@ruvector/rvf` | Unified TypeScript SDK with auto backend selection | - |
| `@ruvector/rvf-node` | Native N-API bindings (Rust via napi-rs) with AGI methods | - |
| `@ruvector/rvf-wasm` | Browser/edge WASM build | ~46 KB control plane, ~5.5 KB tile |
| `@ruvector/rvf-solver` | Self-learning AGI solver (Thompson Sampling, ReasoningBank, witness chain) | ~160 KB WASM |
| `@ruvector/rvf-mcp-server` | MCP server for AI agent integration | - |

Two existing packages would benefit from RVF integration:

1. **`ruvector` (npx ruvector)** -- The main CLI and SDK package (v0.1.88). It has 28 CLI command groups (7,065 lines), depends on `@ruvector/core`, `@ruvector/attention`, `@ruvector/gnn`, `@ruvector/sona`, but has **no dependency on `@ruvector/rvf`**. It currently uses in-memory vector storage with no persistent file-backed option.

2. **`rvlite`** -- A lightweight multi-query vector database (SQL, SPARQL, Cypher) running entirely in WASM. It uses `ruvector-core` for vectors and IndexedDB for browser persistence. A Rust adapter already exists at `crates/rvf/rvf-adapters/rvlite/` wrapping `RvfStore` as `RvliteCollection`.

The main gap is operational truth: what happens on crash, partial migrate, concurrent writers, browser refresh, and mixed backends. This ADR locks the invariants that keep the integration boring and durable.

---

## Key Invariants

### 1. Single writer rule

Any open store has exactly one writer lease. Node uses a file lock (`flock`). Browser uses a lock record with heartbeat in IndexedDB. Readers are unlimited. A stale lease (heartbeat older than 30 seconds) is recoverable by a new writer.

### 2. Crash ordering rule (rvlite hybrid mode)

RVF is the source of truth for vectors. IndexedDB is a rebuildable cache for metadata.

**Write order:**
1. Write vectors to RVF (append-only, crash-safe)
2. Write metadata to IndexedDB
3. Commit a shared monotonic epoch value in both stores

**On startup:** Compare epochs. If RVF epoch > IndexedDB epoch, rebuild metadata from RVF. If IndexedDB epoch > RVF epoch (should not happen), log warning and trust RVF.

### 3. Backend selection rule

Explicit override beats auto detection. If user passes `--backend rvf`, do not silently fall back to `core` or `memory`. Fail loud with a clear install hint. This prevents data going to the wrong place.

```
Error: @ruvector/rvf is not installed.
  Run: npm install @ruvector/rvf
  The --backend rvf flag requires this package.
```

### 4. Cross-platform compatibility rule

Every `.rvf` file written by WASM must be readable by Node N-API and vice versa for the same RVF wire version. If a file uses features from a newer version, the header must declare it and the CLI must refuse with an upgrade path:

```
Error: vectors.rvf requires RVF wire version 2, but this CLI supports version 1.
  Run: npm update @ruvector/rvf
```

---

## Decision

Integrate `@ruvector/rvf` (and its WASM backend) into both packages in three phases:

### Phase 1: npx ruvector -- Add RVF as optional dependency + CLI command group

**Contract:**
- **Input**: path, dimension, vectors
- **Output**: deterministic `.rvf` file and status metadata
- **Failure**: missing `@ruvector/rvf` package gives error with install instruction (never silent fallback)
- **Success metric**: hooks memory persists across process restart

**Changes:**

1. **package.json** -- Add `@ruvector/rvf` as an optional dependency:
   ```json
   "optionalDependencies": {
     "@ruvector/rvf": "^0.1.0"
   }
   ```

2. **src/index.ts** -- Extend platform detection to try RVF after `@ruvector/core`:
   ```
   Detection order:
   1. @ruvector/core  (native Rust -- fastest)
   2. @ruvector/rvf   (RVF store -- persistent, file-backed)
   3. Stub fallback   (in-memory, testing only)
   ```
   If `--backend rvf` is explicit, skip detection and fail if unavailable.

3. **bin/cli.js** -- Add `rvf` command group before the `mcp` command (~line 7010):
   ```
   ruvector rvf create <path>           Create a new .rvf store
   ruvector rvf ingest <path> <file>    Ingest vectors from JSON/CSV
   ruvector rvf query <path> <vector>   k-NN search
   ruvector rvf status <path>           Show store statistics
   ruvector rvf segments <path>         List all segments
   ruvector rvf derive <path> <child>   Create derived store with lineage
   ruvector rvf compact <path>          Reclaim deleted space
   ruvector rvf export <path>           Export store
   ```

4. **src/core/rvf-wrapper.ts** -- Create wrapper module exposing `RvfDatabase` through the existing core interface pattern. Must match the core interface exactly so callers are backend-agnostic. Exports added to `src/core/index.ts`.

5. **Hooks integration** -- Add `ruvector hooks rvf-backend` subcommand to use `.rvf` files as persistent vector memory backend. The `--backend rvf` flag requires explicit selection; recall is read-only by default.

### Phase 2: rvlite -- RVF as storage backend for vector data

**Contract:**
- **Input**: existing rvlite database state (vectors + metadata + graphs)
- **Output**: `.rvf` file for vectors plus IndexedDB metadata cache
- **Failure**: crash mid-sync triggers epoch reconciliation on next open (self-healing)
- **Success metric**: migrate tool is idempotent and safe to rerun

**Changes:**

1. **Rust crate (`crates/rvlite`)** -- Add optional `rvf-runtime` dependency behind a feature flag:
   ```toml
   [features]
   default = []
   rvf-backend = ["rvf-runtime", "rvf-types"]
   ```
   Default stays unchanged. No behavior change unless feature is enabled.

2. **Hybrid persistence model:**
   - **Vectors**: Stored in `.rvf` file via `RvliteCollection` adapter (already exists at `rvf-adapters/rvlite/`)
   - **Metadata/Graphs**: Continue using IndexedDB JSON state (SQL tables, Cypher nodes/edges, SPARQL triples)
   - **Epoch reconciliation**: Both stores share a monotonic epoch. On startup, compare and rebuild the lagging side.
   - RVF vector IDs map directly to rvlite SQL primary keys (no internal mapping layer -- IDs are u64 in both systems).

3. **npm package (`npm/packages/rvlite`)** -- Add `@ruvector/rvf-wasm` as optional dependency. Extend `RvLite` TypeScript class:
   ```typescript
   // New factory method
   static async createWithRvf(config: RvLiteConfig & { rvfPath: string }): Promise<RvLite>

   // New methods
   async saveToRvf(path: string): Promise<void>
   async loadFromRvf(path: string): Promise<void>
   ```

4. **Migration utility** -- `rvlite rvf-migrate` CLI command to convert existing IndexedDB vector data into `.rvf` files. Supports `--dry-run` and `--verify` modes. Idempotent: rerunning on an already-migrated store is a no-op.

5. **Rebuild command** -- `rvlite rvf-rebuild` reconstructs IndexedDB metadata from RVF when cache is missing or corrupted.

### Phase 3: Shared WASM backend unification

**Contract:**
- **Input**: browser environment with both `ruvector` and `rvlite` installed
- **Output**: one shared WASM engine instance resolved through a single import path
- **Success metric**: bundle diff shows zero duplicate WASM; CI check enforces this

**Changes:**

1. **Single WASM build** -- Both `rvlite` and `ruvector` share `@ruvector/rvf-wasm` as the vector computation engine in browser environments, eliminating duplicate WASM binaries.

2. **MCP bridge** -- The existing `@ruvector/rvf-mcp-server` exposes all RVF operations to AI agents. Extend with rvlite-specific tools (read-only by default unless `--write` flag is set):
   ```
   rvlite_sql(storeId, query)       Execute SQL over RVF-backed store
   rvlite_cypher(storeId, query)    Execute Cypher query
   rvlite_sparql(storeId, query)    Execute SPARQL query
   ```

3. **Core export consolidation** -- `ruvector` re-exports `RvfDatabase` so downstream consumers use a single import:
   ```typescript
   import { RvfDatabase } from 'ruvector';
   ```

4. **CI duplicate check** -- Build step that fails if two copies of the WASM artifact are present in the bundle.

---

## API Mapping

### ruvector hooks system -> RVF

| Hooks Operation | Current Implementation | RVF Equivalent |
|----------------|----------------------|----------------|
| `hooks remember` | In-memory vector store | `RvfDatabase.ingestBatch()` |
| `hooks recall` | In-memory k-NN | `RvfDatabase.query()` (read-only) |
| `hooks export` | JSON dump | `RvfDatabase.segments()` + file copy |
| `hooks stats` | Runtime counters | `RvfDatabase.status()` |

### rvlite -> RVF

| RvLite Operation | Current Implementation | RVF Equivalent |
|-----------------|----------------------|----------------|
| `insert(vector)` | `VectorDB.add()` (ruvector-core) | `RvliteCollection.add()` |
| `search(query, k)` | `VectorDB.search()` | `RvliteCollection.search()` |
| `delete(id)` | `VectorDB.remove()` | `RvliteCollection.remove()` |
| `save()` | IndexedDB serialization | `RvfStore` file (automatic) |
| `load()` | IndexedDB deserialization | `RvliteCollection.open()` |

### RVF WASM exports used

| Export | Used By | Purpose |
|--------|---------|---------|
| `rvf_store_create` | Both | Initialize in-memory store |
| `rvf_store_ingest` | Both | Batch vector ingestion |
| `rvf_store_query` | Both | k-NN search |
| `rvf_store_delete` | Both | Soft-delete vectors |
| `rvf_store_export` | ruvector | Serialize to `.rvf` bytes |
| `rvf_store_open` | rvlite | Parse `.rvf` into queryable store |
| `rvf_store_count` | Both | Live vector count |
| `rvf_store_status` | ruvector | Store statistics |

---

## Consequences

### Positive

- **Persistent vector storage** -- `npx ruvector` gains file-backed vector storage (`.rvf` files) for the first time, enabling hooks intelligence to survive across sessions.
- **Single format** -- Both packages read/write the same `.rvf` binary format, enabling data interchange.
- **Reduced bundle size** -- Sharing `@ruvector/rvf-wasm` (~46 KB) between packages eliminates duplicate vector engines.
- **Lineage tracking** -- `RvfDatabase.derive()` brings COW branching and provenance to both packages.
- **Cross-platform** -- RVF auto-selects N-API (Node.js) or WASM (browser) without user configuration.
- **Self-healing** -- Epoch reconciliation means crashes never corrupt data permanently.

### Negative

- **Optional dependency complexity** -- Both packages must gracefully handle missing `@ruvector/rvf` at runtime.
- **Dual persistence in rvlite** -- Vectors in `.rvf` files + metadata in IndexedDB adds a split-brain risk. Mitigated by epoch reconciliation and treating IndexedDB as rebuildable cache.
- **API surface growth** -- `npx ruvector` gains 8 new CLI subcommands.

### Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| IndexedDB + RVF sync crash | High | Write RVF first (append-only, crash-safe). IndexedDB is rebuildable. Epoch reconciliation on startup. |
| WASM size budget | Low | Adding ~46 KB to rvlite's ~850 KB bundle is <6% increase. |
| Concurrent open in two tabs | Medium | Writer lease with heartbeat in IndexedDB. Stale lease (>30s) is recoverable. Second writer gets clear error. |
| Version skew across packages | Medium | RVF header version gate. CI compatibility test matrix: WASM-written files must be readable by Node and vice versa. |
| Migration data loss | Medium | Migrate tool has `--dry-run` and `--verify` modes. Idempotent. Never deletes source data. |

---

## Decision Matrix: Hybrid Persistence

| Criteria | Option A: Vectors in RVF, metadata in IndexedDB | Option B: Everything in IndexedDB |
|----------|----|----|
| **Durability** | High (RVF is append-only, crash-safe) | Medium (IndexedDB has no crash ordering guarantee) |
| **Simplicity** | Medium (two stores, epoch sync) | High (single store) |
| **Performance** | High (SIMD-aligned slabs, HNSW indexing) | Medium (JSON serialization) |
| **Recoverability** | High (rebuild metadata from RVF) | Medium (no independent source of truth) |
| **User surprise** | Medium (two persistence targets) | Low (familiar single-store model) |

**Decision**: Option A wins if we implement epoch reconciliation and writer leases (both specified in this ADR).

---

## Failure Modes to Test

| # | Scenario | Expected Behavior |
|---|----------|-------------------|
| 1 | Power loss during ingest | Reopen succeeds. Last committed epoch is consistent. Partial append is invisible. |
| 2 | Crash between RVF write and metadata write | Next open reconciles by epoch. Metadata rebuilt from RVF. |
| 3 | Two writers attempting to open same store | Second writer gets `ELOCK` error with clear message. |
| 4 | Migration rerun on already-migrated store | No-op. No duplication. Exit code 0. |
| 5 | Write in Node, read in browser, write, read back in Node | Top-10 nearest neighbors match within 1e-6 distance tolerance. |
| 6 | Browser refresh during write | Writer lease expires. Next open acquires fresh lease. No corruption. |
| 7 | Mixed RVF versions (v1 file opened by v2 reader) | Forward-compatible read succeeds. v1 file opened by v0 reader fails with upgrade hint. |

---

## Implementation Checklist

### npx ruvector (Phase 1)

- [x] Add backend adapter matching existing core interface exactly
- [x] Add `rvf` CLI group with create, ingest, query, status, segments, derive, compact, export
- [x] Add `rvf examples` and `rvf download` commands for example .rvf files
- [x] Add 10 RVF tools to main MCP server (rvf_create through rvf_examples)
- [x] Add hooks `--backend rvf` flag requiring explicit selection (no silent fallback)
- [x] Error messages for missing `@ruvector/rvf` include install command
- [x] Security: path validation, shell arg sanitization, redirect whitelist
- [x] Smoke test: 4 Rust integration tests (full lifecycle, cosine, multi-restart, metadata)

### rvlite (Phase 2)

- [x] Feature-flag RVF backend in Rust; default stays unchanged
- [x] Epoch reconciliation module (`crates/rvlite/src/storage/epoch.rs`)
- [x] Auto-detection of `@ruvector/rvf-wasm` in TypeScript SDK
- [x] `getStorageBackend()` and `isRvfAvailable()` exports
- [x] Security: Cypher injection prevention, relation type validation, depth clamping
- [x] Full epoch reconciliation algorithm (23 tests, `EpochTracker` with `AtomicU64`, thread-safe)
- [x] `rvf-migrate` CLI command with `--dry-run` and `--verify` modes (idempotent, 1e-6 tolerance)
- [x] `rvf-rebuild` CLI command to reconstruct metadata from RVF
- [x] Writer lease (`WriterLease` with file lock + PID-based stale detection, `BrowserWriterLease` with IndexedDB heartbeat)
- [x] Direct ID mapping: `IdMapping` trait, `DirectIdMap` (identity), `OffsetIdMap` (20 tests)

### Shared (Phase 3)

- [x] `@ruvector/rvf-wasm` as shared optional peer dependency in rvlite
- [x] CI build step (`wasm-dedup-check.yml`) fails if duplicate WASM artifacts detected
- [x] 3 MCP server rvlite tools (`rvlite_sql`, `rvlite_cypher`, `rvlite_sparql`) — read-only default
- [x] Cross-platform compatibility tests: 6 tests (cosine/L2/IP round-trip, segment preservation, byte-identical transfer)

---

## Acceptance Test

A clean machine with no prior data can:
1. `ruvector rvf create test.rvf --dimension 384`
2. `ruvector rvf ingest test.rvf --input vectors.json`
3. `ruvector rvf query test.rvf --vector "..." --k 10` -- returns results
4. Restart the process
5. `ruvector rvf query test.rvf --vector "..." --k 10` -- same results (persistence verified)
6. `rvlite rvf-migrate` converts an existing rvlite store
7. Open the migrated store in a browser via WASM
8. Top-10 nearest neighbors match Node results within 1e-6 distance tolerance

---

## Implementation Files

### npx ruvector (Phase 1)

| File | Action |
|------|--------|
| `npm/packages/ruvector/package.json` | Edit -- add `@ruvector/rvf` optional dep |
| `npm/packages/ruvector/src/index.ts` | Edit -- add RVF to platform detection with explicit backend support |
| `npm/packages/ruvector/src/core/rvf-wrapper.ts` | Create -- RVF wrapper matching core interface |
| `npm/packages/ruvector/src/core/index.ts` | Edit -- export rvf-wrapper |
| `npm/packages/ruvector/bin/cli.js` | Edit -- add `rvf` command group (~line 7010) |

### rvlite (Phase 2)

| File | Action |
|------|--------|
| `crates/rvlite/Cargo.toml` | Edit -- add optional `rvf-runtime` dep behind feature flag |
| `crates/rvlite/src/lib.rs` | Edit -- add RVF backend behind feature flag |
| `crates/rvlite/src/storage/epoch.rs` | Create -- epoch reconciliation algorithm |
| `npm/packages/rvlite/package.json` | Edit -- add `@ruvector/rvf-wasm` optional dep |
| `npm/packages/rvlite/src/index.ts` | Edit -- add `createWithRvf()` factory, migrate, rebuild |

### Shared (Phase 3)

| File | Action |
|------|--------|
| `npm/packages/rvf-mcp-server/src/server.ts` | Edit -- add rvlite query tools (read-only default) |

---

## Security Hardening (Phase 1 Addendum)

Applied security hardening across all three integration surfaces after audit.

### Vulnerabilities Addressed

| ID | Severity | Surface | Vulnerability | Fix |
|----|----------|---------|---------------|-----|
| S-01 | CRITICAL | CLI `rvf download` | Path traversal via crafted filenames | `sanitizeFileName()` + allowlist validation + path containment check |
| S-02 | CRITICAL | MCP server | Command injection via `execSync` with user args | `sanitizeShellArg()` strips shell metacharacters; numeric args parsed with `parseInt()` |
| S-03 | HIGH | MCP `rvf_*` tools | Path traversal via `args.path` | `validateRvfPath()` blocks `..`, null bytes, sensitive system paths |
| S-04 | HIGH | CLI `rvf download` | SSRF via blind redirect following | `ALLOWED_REDIRECT_HOSTS` whitelist (GitHub domains only) |
| S-05 | HIGH | CLI `rvf download` | URL injection | `encodeURIComponent()` on filenames in URLs |
| S-06 | MEDIUM | rvlite `SemanticMemory` | Cypher injection via unsanitized user strings | `sanitizeCypher()` escapes quotes/backslashes/control chars |
| S-07 | MEDIUM | rvlite `SemanticMemory` | Arbitrary relationship types in Cypher | `validateRelationType()` restricts to `[A-Za-z_][A-Za-z0-9_]*` |
| S-08 | MEDIUM | MCP server hooks | Numeric arg injection | All numeric args (`threshold`, `top_k`, `days`, etc.) parsed with `parseInt()` + fallback defaults |
| S-09 | MEDIUM | rvlite `SemanticMemory` | Graph traversal depth abuse | `findRelated()` depth clamped to `[1, 10]` |

### Security Helpers Added

**`mcp-server.js`** (3 functions):
- `validateRvfPath(filePath)` -- blocks path traversal, null bytes, and sensitive system paths
- `sanitizeShellArg(arg)` -- strips shell metacharacters (`\``, `$()`, `{}`, `|`, `;`, `&`, `<>`, `!`, `..`)
- Numeric args validated with `parseInt()` in all 15+ command handlers

**`cli.js`** (download command):
- `sanitizeFileName(name)` -- strips path separators, validates `/^[\w\-.]+$/`
- `ALLOWED_REDIRECT_HOSTS` -- whitelist: `raw.githubusercontent.com`, `objects.githubusercontent.com`, `github.com`
- Path containment: `path.resolve(dest).startsWith(path.resolve(outDir))`
- Allowlist: downloads validated against known `RVF_EXAMPLES` catalog

**`rvlite/src/index.ts`**:
- `sanitizeCypher(value)` -- escapes `\`, `"`, `'`, control characters
- `validateRelationType(rel)` -- validates `[A-Za-z_][A-Za-z0-9_]*`

### Files Modified

| File | Change |
|------|--------|
| `npm/packages/ruvector/bin/cli.js` | +25 lines: filename sanitization, redirect validation, path containment, allowlist |
| `npm/packages/ruvector/bin/mcp-server.js` | +40 lines: `validateRvfPath()`, `sanitizeShellArg()`, applied to all 25+ handlers |
| `npm/packages/rvlite/src/index.ts` | +20 lines: `sanitizeCypher()`, `validateRelationType()`, depth clamping |

---

## RVF Types Implementation (WASM Bootstrap)

The WASM self-bootstrapping types are fully implemented in `rvf-types/src/wasm_bootstrap.rs` (402 lines, 10 tests):

| Type | Size | Description |
|------|------|-------------|
| `WasmHeader` | 64 bytes (`repr(C)`) | Segment payload header with magic "RVWM" (0x5256574D), `to_bytes()`/`from_bytes()` serialization, compile-time size assertion |
| `WasmRole` | `u8` enum | Microkernel (0x00), Interpreter (0x01), Combined (0x02), Extension (0x03), ControlPlane (0x04) |
| `WasmTarget` | `u8` enum | Wasm32 (0x00), WasiP1 (0x01), WasiP2 (0x02), Browser (0x03), BareTile (0x04) |
| `WASM_FEAT_*` | 8 constants | SIMD, bulk memory, multi-value, reference types, threads, tail call, GC, exception handling |

Key fields in `WasmHeader`: `bytecode_size`, `compressed_size`, `bytecode_hash` (SHAKE-256-256), `bootstrap_priority`, `interpreter_type`, `min_memory_pages`, `max_memory_pages`.

All types are exported from `rvf-types/src/lib.rs` and available to downstream crates. The `SegmentType::Wasm = 0x10` discriminant is registered in `segment_type.rs` with `TryFrom<u8>` round-trip tests.

---

## Verification

```bash
# Phase 1: npx ruvector RVF integration
npx ruvector rvf create test.rvf --dimension 384
npx ruvector rvf ingest test.rvf --input vectors.json
npx ruvector rvf query test.rvf --vector "0.1,0.2,..." --k 10
npx ruvector rvf status test.rvf
npx ruvector hooks remember --backend rvf --store hooks.rvf "test pattern"
npx ruvector hooks recall --backend rvf --store hooks.rvf "test"

# Phase 1: Example download
npx ruvector rvf examples
npx ruvector rvf download basic_store agent_memory
npx ruvector rvf download --all -o ./rvf-examples

# Phase 2: rvlite RVF backend
cargo test -p rvlite --features rvf-backend
# npm test for rvlite with RVF factory

# Phase 3: Shared WASM
# Verify single @ruvector/rvf-wasm instance in node_modules
npm ls @ruvector/rvf-wasm

# Failure mode tests
cargo test --test rvf_crash_recovery
cargo test --test rvf_writer_lease
cargo test --test rvf_epoch_reconciliation
cargo test --test rvf_cross_platform_compat
cargo test --test rvf_migration_idempotent
```
