# ADR-038: npx ruvector & rvlite Witness Verification Integration

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-02-16 |
| **Deciders** | RuVector core team |
| **Supersedes** | -- |
| **Related** | ADR-029 (RVF canonical format), ADR-032 (RVF WASM integration), ADR-037 (Publishable RVF acceptance test) |

## Context

ADR-037 introduced the publishable RVF acceptance test, which produces two artifacts:

1. **JSON manifest** -- human-readable scorecards, ablation assertions, and SHAKE-256 witness chain
2. **`.rvf` binary** -- native WITNESS_SEG (0x0A) + META_SEG (0x07), verifiable by `rvf_crypto::verify_witness_chain()`

ADR-032 added `rvf_witness_verify` and `rvf_witness_count` exports to `rvf-wasm`, enabling browser-side verification.

However, neither the `npx ruvector` CLI nor the `rvlite` browser runtime currently exposes witness chain verification to end users. The Rust `rvf-cli` has `rvf verify-witness` (17 subcommands), but the Node.js wrapper in `npm/packages/ruvector/bin/cli.js` does not surface it. Similarly, `rvlite` lists `@ruvector/rvf-wasm` as an optional peer dependency but does not call the witness verification exports.

This means an external developer who receives a `.rvf` acceptance test artifact currently needs the Rust toolchain to verify it. The goal is zero-friction verification via `npx` or a browser import.

## Decision

### 1. `npx ruvector rvf verify-witness <file.rvf>`

Add a `rvf verify-witness` subcommand to the ruvector Node.js CLI (`npm/packages/ruvector/bin/cli.js`):

```
npx ruvector rvf verify-witness acceptance_manifest.rvf
```

**Implementation path** (ordered by preference):

| Backend | Mechanism | Latency | Availability |
|---------|-----------|---------|--------------|
| Native N-API | `@ruvector/rvf-node` binding to `rvf_crypto::verify_witness_chain()` | <1ms | When native binary is installed |
| WASM | `@ruvector/rvf-wasm` `rvf_witness_verify()` export | ~5ms | Always (WASM is universal) |

The CLI auto-detects the best available backend (same pattern as the existing `VectorDB` platform detection). It loads the `.rvf` file, locates the first WITNESS_SEG, extracts the payload, and calls the verification function.

**Output format:**

```
Verifying witness chain: acceptance_manifest.rvf
  Segment type:  WITNESS_SEG (0x0A)
  Entry count:   147 entries (73 bytes each)
  Chain status:  INTACT -- all hashes verified (SHAKE-256)
  VERIFICATION:  PASSED
```

**Error cases:**

```
  Chain status:  BROKEN at entry 42 -- prev_hash mismatch
  VERIFICATION:  FAILED (exit code 1)
```

### 2. `npx ruvector rvf inspect <file.rvf>`

Extend the existing `rvf inspect` to parse and display acceptance test metadata from the META_SEG:

```
npx ruvector rvf inspect acceptance_manifest.rvf

Segments:
  [0] WITNESS_SEG  0x0A  10,731 bytes  (147 entries)
  [1] META_SEG     0x07   2,048 bytes  (JSON metadata)

Acceptance Test Metadata:
  Format:          rvf-acceptance-test v2
  Chain root hash: 7a3f...b2c1
  All passed:      true
  Scorecards:      3 modes (A/B/C)
```

### 3. `rvlite` browser SDK -- `verifyWitnessChain()`

Add a `verifyWitnessChain()` function to the rvlite SDK (`npm/packages/rvlite/src/index.ts`):

```typescript
import { verifyWitnessChain } from 'rvlite';

// Load .rvf file (e.g., from fetch or File API)
const rvfBytes = await fetch('acceptance_manifest.rvf').then(r => r.arrayBuffer());
const result = verifyWitnessChain(new Uint8Array(rvfBytes));

console.log(result.valid);      // true
console.log(result.entryCount); // 147
console.log(result.error);      // null or error description
```

**Implementation:**

```typescript
export interface WitnessVerifyResult {
  valid: boolean;
  entryCount: number;
  error: string | null;
}

export function verifyWitnessChain(rvfBytes: Uint8Array): WitnessVerifyResult {
  // 1. Parse segment header to find WITNESS_SEG
  // 2. Extract payload bytes
  // 3. Allocate WASM memory, copy payload
  // 4. Call rvf_witness_verify(ptr, len)
  // 5. Interpret result (positive = count, negative = error code)
  // 6. Free WASM memory
}
```

This function:
- Requires `@ruvector/rvf-wasm` (already an optional peer dep in rvlite)
- Throws a clear error if the WASM module is not available
- Handles WASM memory allocation/deallocation internally
- Returns a typed result object, not a raw integer

### 4. `rvlite` CLI -- `rvlite verify-witness <file.rvf>`

Register a `verify-witness` command in `cli-rvf.ts` alongside the existing `rvf-migrate` and `rvf-rebuild` commands:

```bash
npx rvlite verify-witness acceptance_manifest.rvf
```

This uses the same WASM backend as the SDK function above.

### 5. MCP tool -- `rvf_verify_witness`

Add to the ruvector MCP server (`npm/packages/ruvector/bin/mcp-server.js`) so Claude Code can verify acceptance test artifacts directly:

```json
{
  "name": "rvf_verify_witness",
  "description": "Verify SHAKE-256 witness chain in an .rvf file",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": { "type": "string", "description": "Path to .rvf file" }
    },
    "required": ["path"]
  }
}
```

## Integration Surface

```
                          ┌────────────────────────┐
                          │  acceptance-rvf (Rust)  │
                          │  generate + verify      │
                          └──────────┬─────────────┘
                                     │ produces
                          ┌──────────▼─────────────┐
                          │  acceptance_manifest.rvf │
                          │  WITNESS_SEG + META_SEG  │
                          └──────────┬─────────────┘
                    ┌────────────────┼────────────────┐
                    │                │                 │
          ┌─────────▼──────┐ ┌──────▼───────┐ ┌──────▼──────────┐
          │ npx ruvector    │ │ npx rvlite   │ │ Browser (rvlite │
          │ rvf             │ │ verify-      │ │ SDK)            │
          │ verify-witness  │ │ witness      │ │ verifyWitness   │
          └────────┬───────┘ └──────┬───────┘ │ Chain()         │
                   │                │          └──────┬──────────┘
          ┌────────▼────────────────▼─────────────────▼──────────┐
          │              @ruvector/rvf-wasm                       │
          │  rvf_witness_verify(chain_ptr, chain_len) -> i32     │
          │  rvf_witness_count(chain_len) -> i32                 │
          └──────────────────────────────────────────────────────┘
```

## Implementation Order

| Phase | Work | Package | Complexity |
|-------|------|---------|------------|
| **1** | `verifyWitnessChain()` SDK function | `rvlite` | Low -- WASM call + segment parsing |
| **2** | `verify-witness` CLI command | `rvlite` | Low -- wraps SDK function |
| **3** | `rvf verify-witness` CLI subcommand | `ruvector` | Medium -- N-API fallback + WASM detection |
| **4** | `rvf inspect` metadata display | `ruvector` | Low -- parse META_SEG JSON |
| **5** | `rvf_verify_witness` MCP tool | `ruvector` | Low -- wraps CLI logic |

Each phase is independently shippable. Phase 1+2 enable browser verification. Phase 3-5 enable CLI and agent verification.

## Consequences

### Positive

- External developers verify `.rvf` acceptance tests with `npx ruvector rvf verify-witness` -- zero Rust toolchain required
- Browser-based verification via `rvlite` SDK requires only `npm install rvlite @ruvector/rvf-wasm`
- Claude Code agents can verify witness chains via MCP tool without file manipulation
- Consistent verification path: Rust CLI, Node.js CLI, browser SDK, and WASM microkernel all use the same `rvf_witness_verify` implementation
- Auto-detection prefers native N-API when available for sub-millisecond verification

### Negative

- WASM module adds ~46 KB to rvlite when `@ruvector/rvf-wasm` is installed
- Segment header parsing must be duplicated in TypeScript (WASM only verifies the chain payload, not the segment framing)
- N-API binding for `verify_witness_chain` does not exist yet in `rvf-node` -- Phase 3 requires adding it

### Neutral

- The JSON manifest verification (`verify --input manifest.json`) remains available via the Rust binary for users who prefer JSON over binary `.rvf`
- `@ruvector/rvf-wasm` remains an optional peer dependency -- rvlite works without it but witness verification is unavailable
