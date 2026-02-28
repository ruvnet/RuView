# Security Integration Analysis: sublinear-time-solver

**Agent**: 9 / Security Integration Analysis
**Date**: 2026-02-20
**Scope**: Security posture assessment of ruvector and attack surface changes from sublinear-time-solver integration
**Classification**: Internal Engineering Reference

---

## Table of Contents

1. [Current Security Posture of ruvector](#1-current-security-posture-of-ruvector)
2. [Attack Surface Changes from Integration](#2-attack-surface-changes-from-integration)
3. [WASM Sandbox Security](#3-wasm-sandbox-security)
4. [Serialization and Deserialization Safety](#4-serialization-and-deserialization-safety)
5. [MCP Tool Access Control](#5-mcp-tool-access-control)
6. [Dependency Supply Chain Risks](#6-dependency-supply-chain-risks)
7. [Input Validation Requirements for Solver APIs](#7-input-validation-requirements-for-solver-apis)
8. [Recommended Security Mitigations](#8-recommended-security-mitigations)

---

## 1. Current Security Posture of ruvector

### 1.1 Strengths

The ruvector codebase demonstrates a mature, defense-in-depth security architecture across multiple layers:

**Cryptographic Foundation (rvf-crypto)**

- Ed25519 signature verification for all kernel packs and RVF segments (`/crates/rvf/rvf-crypto/src/sign.rs`)
- SHAKE-256 hash binding for tamper-evident witness chains (`/crates/rvf/rvf-crypto/src/witness.rs`)
- Attestation module with TEE platform support (SGX, SEV-SNP) including measurement-based key binding (`/crates/rvf/rvf-crypto/src/attestation.rs`)
- Domain separation in signature construction (`RVF-v1-segment` context string prevents cross-protocol replay)
- Proper canonical serialization for signed data (avoids unsafe transmute, uses explicit byte layout)

**WASM Kernel Pack Security (ruvector-wasm)**

- Ed25519 manifest signature verification (`/crates/ruvector-wasm/src/kernel/signature.rs`)
- SHA256 hash-based kernel allowlist with per-kernel granularity (`/crates/ruvector-wasm/src/kernel/allowlist.rs`)
- Epoch-based execution interruption prevents infinite loops (`/crates/ruvector-wasm/src/kernel/epoch.rs`)
- Memory layout validation prevents overlapping regions and out-of-bounds access (`/crates/ruvector-wasm/src/kernel/memory.rs`)
- Resource limits per kernel (max memory pages, max epoch ticks, max table elements)

**MCP Coherence Gate (mcp-gate)**

- Three-tier decision system (Permit/Defer/Deny) with cryptographic witness receipts (`/crates/mcp-gate/src/tools.rs`)
- Hash-chain integrity verification for audit replay (`verify_chain_to`)
- Deterministic decision replay for forensic analysis
- Structured escalation protocol for deferred actions with timeout-to-deny default

**Edge-Net Security**

- Comprehensive relay security test suite covering 7 attack vectors (`/examples/edge-net/tests/relay-security.test.ts`)
- Task completion spoofing protection (assignment-based authorization)
- Replay attack prevention (duplicate completion rejection)
- Credit self-reporting rejection (server-side ledger authority)
- Per-IP connection limiting, rate limiting, message size limits
- WASM-based Ed25519 identity management with challenge-response verification (`/examples/edge-net/pkg/secure-access.js`)
- Adaptive security with self-learning attack pattern detection
- Adapter security with quarantine-before-activation, signature verification, and quality gates (`/examples/edge-net/pkg/models/adapter-security.js`)

**Storage Layer**

- Path traversal prevention in `VectorStorage::new()` (`/crates/ruvector-core/src/storage.rs`, line 78: `path_str.contains("..")` check)
- Database connection pooling to prevent resource exhaustion
- Feature-gated storage (WASM builds use in-memory only)

### 1.2 Weaknesses and Gaps

**SEC-W1: Server CORS Configuration is Fully Permissive**

In `/crates/ruvector-server/src/lib.rs` (lines 85-88):

```rust
let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods(Any)
    .allow_headers(Any);
```

This allows any origin to make requests to the vector database API, enabling cross-site data exfiltration attacks. An attacker could embed JavaScript on any website that silently queries or modifies collections in a user's locally-running ruvector instance.

**DREAD Score**: D:6 R:9 E:8 A:7 D:9 = **7.8 (High)**

**SEC-W2: No Authentication or Authorization on REST API**

The ruvector-server exposes collection CRUD and vector search/upsert endpoints with zero authentication. Any process with network access to port 6333 can:
- Create, list, and delete collections
- Insert arbitrary vectors
- Search and exfiltrate all stored data

This is acceptable for development but represents a critical gap for any deployment beyond localhost.

**DREAD Score**: D:8 R:10 E:10 A:8 D:10 = **9.2 (Critical)**

**SEC-W3: Unbounded Search Parameters**

In `/crates/ruvector-server/src/routes/points.rs`, the `SearchRequest.k` parameter has a default of 10 but no upper bound. A malicious client can set `k` to `usize::MAX`, potentially causing:
- Memory exhaustion (allocating a result vector of billions of entries)
- CPU exhaustion (scanning entire index)

**SEC-W4: Unsafe Code in SIMD and Arena Allocator**

The ruvector-core contains 90 `unsafe` blocks across 4 files:
- `/crates/ruvector-core/src/simd_intrinsics.rs` (40 occurrences) - SIMD intrinsics with `assert_eq!` length guards
- `/crates/ruvector-core/src/arena.rs` (23 occurrences) - Custom arena allocator with raw pointer arithmetic
- `/crates/ruvector-core/src/cache_optimized.rs` (19 occurrences)
- `/crates/ruvector-core/src/quantization.rs` (8 occurrences)

The SIMD code includes proper length assertions before unsafe operations, which is good. However, the arena allocator performs raw pointer arithmetic (`chunk.data.add(aligned)`) that relies on alignment invariants not enforced by the type system.

**SEC-W5: Development-Mode Bypass Switches**

Both the kernel signature verifier and the kernel allowlist provide `insecure_*` constructors:
- `KernelPackVerifier::insecure_no_verify()` - Bypasses all signature checks
- `TrustedKernelAllowlist::insecure_allow_all()` - Bypasses all hash allowlist checks

These methods are documented with warnings but there is no compile-time gating (e.g., `#[cfg(not(feature = "production"))]`) to prevent accidental use in release builds.

**SEC-W6: Default Backup Password in Edge-Net Identity**

In `/examples/edge-net/pkg/secure-access.js` (line 141):
```javascript
const password = this.options.backupPassword || 'edge-net-default-key';
```

Identity key material is encrypted with a hardcoded default password. If no backup password is provided, any party who obtains the stored encrypted identity can decrypt the private key.

**SEC-W7: Missing Input Validation on Collection Names**

The `CreateCollectionRequest.name` field in `/crates/ruvector-server/src/routes/collections.rs` accepts arbitrary strings. This could lead to issues if collection names are used in file paths for persistent storage (directory traversal) or contain control characters.

### 1.3 Security Architecture Summary

| Component | Auth | Encryption | Integrity | Audit | Rating |
|-----------|------|-----------|-----------|-------|--------|
| ruvector-server | None | None (HTTP) | Serde validation | Trace logging | Low |
| ruvector-wasm kernel | Ed25519 + SHA256 | N/A | Hash allowlist | Epoch monitoring | High |
| mcp-gate | Action-based | N/A | Witness chain | Full replay | High |
| rvf-crypto | Ed25519 | TEE-bound keys | SHAKE-256 chain | Witness segments | Very High |
| edge-net | PiKey Ed25519 | Session-based | Challenge-response | Adaptive learning | High |
| ruvector-core | N/A | N/A | Dimension checks | None | Medium |

---

## 2. Attack Surface Changes from Integration

### 2.1 New Attack Surface from sublinear-time-solver

Integrating the sublinear-time-solver introduces the following new attack vectors:

**AS-1: Express Server Endpoints**

The solver includes an Express-based HTTP server with `helmet` and `cors` middleware. While `helmet` provides reasonable HTTP security headers, the integration creates a new network-accessible service that:
- Accepts solver problem definitions over HTTP
- Returns computed solutions
- Must validate all input parameters before passing to the Rust/WASM solver core

The net effect is a second HTTP service alongside ruvector-server, doubling the network-accessible API surface.

**AS-2: WASM Sandbox Boundary**

The solver executes optimization algorithms in WASM modules. Each WASM invocation represents a trust boundary crossing where:
- Input data flows from JavaScript host into WASM linear memory
- Computed results flow from WASM back to the host
- Shared memory regions must be validated on both sides

Unlike ruvector's existing WASM kernels (which have Ed25519 + allowlist verification), the solver's WASM modules need their own verification pipeline or must be integrated into ruvector's `KernelManager` framework.

**AS-3: Serde Deserialization from External Sources**

The solver uses serde for serializing/deserializing problem definitions and solution state. Deserialization of untrusted input is a well-known attack vector in Rust:
- `serde_json::from_str` can be safe but may allocate unbounded memory for deeply nested or large inputs
- `rkyv` (used elsewhere in ruvector) provides zero-copy deserialization which is more efficient but historically more prone to safety issues
- `bincode` deserialization can panic on malformed input if not configured with size limits

**AS-4: Session Management State**

The solver includes a session management module. Sessions introduce:
- Session fixation risks (predictable session IDs)
- Session hijacking via token theft
- Resource exhaustion through session flooding (creating millions of sessions)
- State consistency issues in multi-tenant scenarios

**AS-5: MCP Tool Registration**

If the solver registers as an MCP tool, it becomes callable by AI agents. This introduces:
- Agent-initiated solver invocations that could be computationally expensive
- Prompt injection attacks that cause agents to invoke the solver with adversarial inputs
- Recursive invocations if the solver itself uses MCP tools

### 2.2 Attack Surface Quantification

| Surface | Pre-Integration | Post-Integration | Delta |
|---------|----------------|-----------------|-------|
| HTTP Endpoints | 6 (ruvector-server) | 6 + N (solver) | +N |
| WASM Modules | Verified kernel packs | + Solver WASM | +1 boundary |
| Deserialization Points | serde_json (API) | + solver serde | +M |
| Session State | None (stateless) | Session manager | +1 state store |
| MCP Tools | 3 (mcp-gate) | 3 + solver tools | +K tools |
| Dependency Count | ~100 Rust crates | + solver deps | +D crates |

### 2.3 Trust Boundary Diagram

```
                    [External Client]
                          |
              +-----------+-----------+
              |                       |
    [ruvector-server]       [solver Express Server]
         |                       |
    [ruvector-core]        [solver-core (WASM)]
         |                       |
    [redb/mmap storage]    [solver session mgmt]
         |                       |
    [WASM kernel packs]    [serde serialization]
         |                       |
    [mcp-gate] <--MCP--> [solver MCP tools]
```

Each arrow represents a trust boundary where input validation is required.

---

## 3. WASM Sandbox Security

### 3.1 Existing ruvector WASM Sandbox Model

The ruvector WASM kernel system (`/crates/ruvector-wasm/src/kernel/`) implements a robust sandbox with multiple defense layers:

**Layer 1: Supply Chain Verification**
- Ed25519 signature verification of kernel pack manifests
- SHA256 hash verification of individual WASM kernel binaries
- Trusted key and hash allowlists with per-kernel granularity

**Layer 2: Runtime Constraints**
- Epoch-based execution interruption (configurable tick interval and budget)
- Maximum memory page limits (server: 1024 pages = 64MB; embedded: 64 pages = 4MB)
- Table element limits for indirect function calls

**Layer 3: Memory Safety**
- `MemoryLayoutValidator` prevents overlapping memory regions
- Bounds checking on all descriptor offsets (`MemoryAccessViolation` error)
- Aligned memory allocation (16-byte default)
- Read-only vs. writable region enforcement (output cannot overlap inputs)

**Layer 4: Instance Isolation**
- Each `WasmKernelInstance` has its own memory allocation
- Epoch deadlines are per-invocation
- Instance pooling with configurable pool size

### 3.2 Solver WASM Sandbox Requirements

The sublinear-time-solver's WASM modules need equivalent protections. Key considerations:

**3.2.1 Memory Bounds**

Solver algorithms may require large working memory for optimization state. The default 64MB limit for server workloads may be insufficient for large problem instances. However, increasing memory limits increases the risk of memory exhaustion attacks.

Recommendation: Use dynamic memory limits based on problem size, with an absolute ceiling:
```
solver_memory_pages = min(problem_size_pages * 1.5, MAX_SOLVER_PAGES)
```
where `MAX_SOLVER_PAGES` is configurable but defaults to 2048 (128MB).

**3.2.2 Execution Time Limits**

Sublinear-time algorithms should complete faster than linear-time alternatives by definition. This creates a natural execution time bound that should be enforced:
- Expected: O(n^alpha) for alpha < 1
- Deadline: Set epoch budget proportional to `n^alpha * safety_factor`
- If deadline is exceeded, this indicates either a malicious input designed to trigger worst-case behavior or a bug

**3.2.3 Solver-Specific WASM Risks**

- **Nondeterministic behavior**: If the solver uses randomized algorithms, WASM determinism guarantees may not hold across platforms. This is acceptable for optimization but problematic for audit replay.
- **Floating-point precision**: WASM f32/f64 operations are IEEE 754 compliant but may produce different results on different CPUs due to fused-multiply-add variations. Solver results should include tolerance bounds.
- **Stack overflow**: Deeply recursive solver algorithms could exhaust the WASM stack. Wasmtime's configurable stack size should be explicitly set.

### 3.3 WASM Sandbox Recommendations

| Control | Current (ruvector) | Required (solver) | Gap |
|---------|-------------------|-------------------|-----|
| Signature verification | Ed25519 | Must integrate | Yes |
| Hash allowlist | Per-kernel SHA256 | Must integrate | Yes |
| Epoch interruption | Configurable | Required, problem-size-proportional | Partial |
| Memory limits | 64MB server / 4MB embedded | 128MB max, dynamic | Enhancement |
| Stack limits | Wasmtime default | Explicit 1MB limit | Yes |
| Instance isolation | Per-invocation | Per-invocation required | None |
| Determinism | Not enforced | Not required for optimization | None |

---

## 4. Serialization and Deserialization Safety

### 4.1 Current Serialization Stack

ruvector uses three serialization frameworks:

| Framework | Location | Purpose | Risk Level |
|-----------|----------|---------|------------|
| `serde_json` | Server API, MCP protocol | JSON API requests/responses | Medium |
| `bincode` (2.0 rc3) | Storage, wire protocol | Binary vector encoding | High |
| `rkyv` (0.8) | Performance-critical paths | Zero-copy deserialization | Very High |
| `serde` traits | Everywhere | Derive macros for (de)serialization | Low |

### 4.2 serde_json Safety Analysis

`serde_json` is the safest of the three for untrusted input:
- Memory allocation is bounded by input size (no amplification attacks)
- Deeply nested JSON is limited by stack depth (configurable via `serde_json::Deserializer::disable_recursion_limit`)
- Unicode handling is correct per RFC 8259

**Remaining risks**:
- No built-in size limits. A multi-GB JSON payload will be allocated in full before being rejected by application-level validation. Mitigation: Use `hyper`/`axum` body size limits.
- Numeric precision: JSON numbers are parsed as f64 or i64/u64. Large integers may lose precision silently.

### 4.3 bincode Safety Analysis

bincode 2.0 (release candidate) is used with the `serde` feature for storage serialization. Key risks:

- **Allocation amplification**: A malicious bincode payload can declare a vector length of 2^64 elements, causing the allocator to attempt a multi-exabyte allocation. bincode 2.0 provides `Configuration::with_limit()` to cap maximum allocation size. **This MUST be used for any untrusted input.**
- **Type confusion**: bincode does not encode type information. If the wrong type is deserialized, the result is garbage data rather than an error. This can lead to logic errors in security-critical paths.

### 4.4 rkyv Safety Analysis

rkyv 0.8 provides zero-copy deserialization by directly interpreting byte buffers as Rust structs. This is extremely fast but carries significant safety implications:

- **Alignment requirements**: rkyv archived types must be properly aligned. Misaligned access on some architectures causes undefined behavior or hardware faults.
- **Validation requirement**: rkyv 0.8 provides `check_archived_root()` for validating archived data before access. **Skipping validation on untrusted input is equivalent to accepting arbitrary memory layouts as valid Rust structs.**
- **Historical CVEs**: Earlier rkyv versions had soundness issues. Version 0.8 addresses many of these but is still relatively new.

### 4.5 Solver Integration Serialization Risks

The sublinear-time-solver adds serde deserialization of:
- Problem definitions (graph structures, constraint matrices, objective functions)
- Solution state (intermediate solver state for session persistence)
- Configuration parameters

**Critical requirement**: All solver deserialization points MUST enforce:
1. Maximum input size (reject payloads > configured limit before parsing)
2. Maximum nesting depth (prevent stack overflow during parsing)
3. Maximum collection sizes (prevent allocation amplification)
4. Type validation (ensure deserialized types match expected schema)

### 4.6 Deserialization Attack Scenarios

**Scenario D1: Memory Exhaustion via Vector Length**
```json
{
  "graph": {
    "nodes": 999999999,
    "edges": []
  }
}
```
If the nodes count is used to pre-allocate a vector, this causes a ~4GB allocation attempt (999999999 * 4 bytes for f32 weights). Defense: Validate `nodes <= MAX_SOLVER_NODES` before allocation.

**Scenario D2: Nested Object Bomb**
```json
{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":
  ... (1000+ levels deep)
}}}}}}}}}}}}}
```
Deeply nested JSON can overflow the stack during recursive deserialization. Defense: Configure serde_json with recursion depth limits.

**Scenario D3: Billion-Laughs Equivalent**
If the solver supports any form of reference-based serialization (which standard serde_json does not), a small input could expand into massive in-memory structures. Defense: Ensure no reference/entity expansion in deserialization.

---

## 5. MCP Tool Access Control

### 5.1 Current MCP Architecture

The mcp-gate (`/crates/mcp-gate/`) implements a coherence gate with three tools:

| Tool | Purpose | Authorization | Audit |
|------|---------|---------------|-------|
| `permit_action` | Request permission for an action | Context-based (agent_id, session, prior_actions) | Witness receipt + hash chain |
| `get_receipt` | Retrieve audit receipt | Sequence number only | Read-only |
| `replay_decision` | Deterministic decision replay | Sequence number, optional chain verify | Read-only |

The authorization model is based on the TileZero coherence gate, which uses:
- **Structural analysis**: Graph cut values and partition stability
- **Predictive analysis**: Prediction set sizes and coverage targets
- **Evidential analysis**: E-value accumulation for evidence strength

### 5.2 Access Control Gaps

**AC-1: No Caller Authentication in MCP Protocol**

The MCP server (`/crates/mcp-gate/src/server.rs`) accepts JSON-RPC messages over stdio without authenticating the caller. Any process that can write to the server's stdin can invoke tools. In the standard MCP deployment model (tool orchestrator spawns MCP server as child process), this is acceptable because the parent process is trusted. However:

- If the MCP server is exposed over a network (not standard but possible), there is zero authentication.
- The `agent_id` field in `PermitActionRequest` is self-reported and not verified.

**AC-2: Receipt Enumeration**

The `get_receipt` tool accepts a sequence number and returns the full receipt. An attacker who knows or can guess sequence numbers can enumerate all past decisions, extracting:
- Action IDs and types
- Target device and path information
- Agent and session identifiers
- Structural/predictive/evidential scores

This is an information disclosure risk if the MCP server is accessible to untrusted parties.

**AC-3: No Rate Limiting on MCP Tools**

Unlike the edge-net relay (which enforces per-node rate limits), the MCP server has no rate limiting. An agent could:
- Flood `permit_action` to cause computational denial-of-service
- Rapidly enumerate `get_receipt` with sequential sequence numbers
- Request `replay_decision` with `verify_chain: true` for expensive chain verification

### 5.3 Solver MCP Integration Risks

If the sublinear-time-solver registers as MCP tools, the following risks emerge:

**AC-4: Computational Cost Amplification**

Solver invocations are inherently more expensive than gate decisions. A single `solve_problem` MCP call could consume seconds of CPU time and hundreds of megabytes of memory. Without per-agent resource quotas, a compromised or malicious agent could:
- Submit maximum-size problems continuously
- Exhaust all available compute resources
- Prevent legitimate operations from completing

**AC-5: Problem Data as Attack Vector**

If solver problem definitions are passed through MCP tool arguments (which are `serde_json::Value`), the deserialization risks from Section 4 apply directly in the MCP context. Agent-submitted JSON is inherently untrusted.

**AC-6: Cross-Tool Information Flow**

If the solver can invoke mcp-gate tools (or vice versa), there is a risk of:
- Privilege escalation (solver uses gate token to authorize its own actions)
- Information leakage (solver reads gate receipts to learn about other agents' actions)
- Circular dependencies (gate defers to solver, solver calls gate)

### 5.4 Recommended Access Control Model

```
Agent --[MCP]--> mcp-gate (permit_action)
                      |
                      v
              [Coherence Decision]
                      |
         +------+----+----+------+
         |      |         |      |
      Permit  Defer     Deny   (log)
         |      |
         v      v
  [Solver MCP Tool]  [Escalation]
         |
    [Resource Quota Check]
         |
    [Input Validation]
         |
    [Solver Execution (sandboxed WASM)]
         |
    [Result + Witness Receipt]
```

Key additions for solver integration:
1. Solver MCP tools MUST require a valid `PermitToken` from mcp-gate
2. Resource quotas MUST be enforced per-agent before solver invocation
3. Solver results SHOULD generate witness receipts for audit
4. Cross-tool calls MUST be prevented (unidirectional flow only)

---

## 6. Dependency Supply Chain Risks

### 6.1 Current Dependency Profile

The ruvector workspace contains approximately 100 direct Rust crate dependencies (12,884 lines in `Cargo.lock`). Key security-sensitive dependencies:

| Dependency | Version | Purpose | Supply Chain Risk |
|-----------|---------|---------|-------------------|
| `ed25519-dalek` | (latest) | Cryptographic signatures | Low (well-audited) |
| `hnsw_rs` | 0.3 (patched) | Vector indexing | Medium (patched locally) |
| `redb` | 2.1 | Persistent storage | Low (Rust-native) |
| `rkyv` | 0.8 | Zero-copy deserialization | Medium (complex unsafe) |
| `bincode` | 2.0-rc3 | Binary serialization | Medium (pre-release) |
| `axum` | (latest) | HTTP server | Low (Tokio ecosystem) |
| `tower-http` | (latest) | HTTP middleware | Low (Tokio ecosystem) |
| `dashmap` | 6.1 | Concurrent map | Low |
| `rayon` | 1.10 | Parallel processing | Low (well-audited) |
| `simsimd` | 5.9 | SIMD distance computation | Medium (C FFI) |
| `wasm-bindgen` | 0.2 | WASM bindings | Low (Rust WASM ecosystem) |
| `@claude-flow/memory` | ^3.0.0-alpha.7 | Agent memory (npm) | Medium (alpha pre-release) |

### 6.2 Notable Supply Chain Concerns

**SC-1: Patched hnsw_rs**

ruvector patches `hnsw_rs` locally (`/patches/hnsw_rs`) to resolve a `rand` version conflict (0.8 vs 0.9) for WASM compatibility. Local patches:
- Freeze the dependency at a known state (good for reproducibility)
- Prevent receiving upstream security fixes automatically (bad for security)
- Require manual review and re-patching when upstream publishes fixes

**SC-2: bincode Pre-Release**

Using `bincode 2.0.0-rc3` means:
- API may change before stable release
- Less community testing than stable versions
- Potential for undiscovered safety issues

**SC-3: simsimd C FFI Boundary**

`simsimd` (5.9) provides C-language SIMD implementations called via FFI. This introduces:
- Memory safety risks at the FFI boundary
- Potential for ABI mismatches if simsimd is compiled with different flags
- Dependencies on system-level C library versions

**SC-4: npm Dependency Tree**

The npm workspace (ruvector-node, ruvector-wasm, etc.) brings a separate dependency tree. Notable overrides in `package.json`:
```json
"overrides": {
    "axios": "^1.13.2",
    "body-parser": "^2.2.1"
}
```
These overrides suggest known vulnerabilities in transitive dependencies that required manual pinning.

### 6.3 Solver-Introduced Dependencies

The sublinear-time-solver adds:

| Dependency | Risk Assessment |
|-----------|----------------|
| Express.js | Low risk (mature, well-maintained) |
| helmet | Low risk (security-focused, minimal surface) |
| cors (npm) | Low risk (widely used) |
| serde (Rust) | Already present in ruvector |
| wasm-bindgen (Rust) | Already present in ruvector |

**New unique risks from solver dependencies**:
- Express middleware chain introduces potential request smuggling if reverse-proxied
- Any solver-specific npm packages must be audited for supply chain attacks
- MIT/Apache-2.0 dual licensing is compatible with ruvector's MIT license (no legal risk)

### 6.4 Supply Chain Mitigation Recommendations

1. **Lock files**: Ensure both `Cargo.lock` and `package-lock.json` are committed and used in CI
2. **Audit automation**: Run `cargo audit` and `npm audit` in CI pipeline
3. **Dependency review**: Use `cargo-deny` to enforce license compliance and ban known-vulnerable crates
4. **SBOM generation**: Generate Software Bill of Materials for all builds
5. **Upstream monitoring**: Set up alerts for upstream security advisories on critical dependencies
6. **Minimal solver dependencies**: Prefer solver implementations that minimize additional dependency count

---

## 7. Input Validation Requirements for Solver APIs

### 7.1 Problem Definition Validation

All solver API inputs must be validated before processing. The following validation rules apply:

**7.1.1 Graph/Network Inputs**

| Parameter | Type | Constraint | Rationale |
|-----------|------|-----------|-----------|
| `node_count` | usize | 1 <= n <= MAX_NODES (default: 10,000,000) | Prevent memory exhaustion |
| `edge_count` | usize | 0 <= e <= MAX_EDGES (default: 100,000,000) | Prevent memory exhaustion |
| `edge_weights` | f32/f64 | Finite, not NaN, not Inf | Prevent arithmetic errors |
| `node_ids` | string | <= 256 chars, alphanumeric + hyphens | Prevent injection |
| `adjacency` | sparse | e <= n * (n-1) / 2 (undirected), e <= n * (n-1) (directed) | Graph consistency |

**7.1.2 Optimization Parameters**

| Parameter | Type | Constraint | Rationale |
|-----------|------|-----------|-----------|
| `max_iterations` | u64 | 1 <= iter <= MAX_ITER (default: 1,000,000) | Prevent infinite computation |
| `tolerance` | f64 | 0 < tol <= 1.0 | Meaningful convergence criterion |
| `timeout_ms` | u64 | 100 <= t <= MAX_TIMEOUT (default: 300,000) | Prevent resource lock |
| `seed` | u64 | Any | No constraint needed |
| `alpha` (sublinearity) | f64 | 0 < alpha < 1 | Must be sublinear by definition |

**7.1.3 Vector/Matrix Inputs**

| Parameter | Type | Constraint | Rationale |
|-----------|------|-----------|-----------|
| `dimension` | usize | 1 <= d <= MAX_DIM (default: 65,536) | Prevent memory exhaustion |
| `values` | Vec<f32> | len == declared dimension, all finite | Memory safety, arithmetic safety |
| `matrix` | nested Vec | rows * cols <= MAX_MATRIX_ELEMENTS | Memory bounds |

### 7.2 Session Management Validation

| Parameter | Type | Constraint | Rationale |
|-----------|------|-----------|-----------|
| `session_id` | string | UUID v4 format, server-generated only | Prevent session fixation |
| `session_ttl` | u64 | 60 <= ttl <= 86400 seconds | Prevent permanent sessions |
| `max_sessions_per_client` | usize | Default: 10 | Prevent session flooding |
| `session_data_size` | usize | <= MAX_SESSION_DATA (default: 10MB) | Prevent storage exhaustion |

### 7.3 API Rate Limits

| Endpoint | Rate Limit | Burst | Rationale |
|----------|-----------|-------|-----------|
| Problem submission | 10/minute per client | 3 | Prevent compute exhaustion |
| Solution retrieval | 100/minute per client | 20 | Allow polling |
| Session operations | 30/minute per client | 5 | Prevent session flooding |
| Health/status | 60/minute per client | 10 | Allow monitoring |

### 7.4 Input Validation Implementation Pattern

```rust
// Recommended validation pattern for solver inputs
pub fn validate_problem_input(input: &ProblemDefinition) -> Result<(), ValidationError> {
    // 1. Size bounds
    if input.node_count > MAX_NODES {
        return Err(ValidationError::TooLarge {
            field: "node_count",
            max: MAX_NODES,
            actual: input.node_count,
        });
    }

    // 2. Numeric sanity
    for weight in &input.edge_weights {
        if !weight.is_finite() {
            return Err(ValidationError::InvalidNumber {
                field: "edge_weights",
                reason: "non-finite value",
            });
        }
    }

    // 3. Structural consistency
    if input.edge_count > input.node_count * (input.node_count - 1) {
        return Err(ValidationError::InconsistentGraph {
            reason: "more edges than possible for given node count",
        });
    }

    // 4. Parameter ranges
    if input.alpha <= 0.0 || input.alpha >= 1.0 {
        return Err(ValidationError::OutOfRange {
            field: "alpha",
            min: 0.0,
            max: 1.0,
            actual: input.alpha,
        });
    }

    Ok(())
}
```

---

## 8. Recommended Security Mitigations

### 8.1 Critical Priority (Address Before Integration)

**MIT-1: Add Authentication to ruvector-server**

Implement API key or JWT-based authentication for the REST API. At minimum:
- Require `Authorization: Bearer <token>` header on all mutating endpoints
- Support API key rotation without server restart
- Log authentication failures with client IP

```rust
// Suggested middleware addition
async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = request.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match token {
        Some(t) if state.verify_token(t) => Ok(next.run(request).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}
```

**MIT-2: Restrict CORS Configuration**

Replace `Any` CORS origins with an explicit allowlist:
```rust
let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::list([
        "http://localhost:3000".parse().unwrap(),
        "http://127.0.0.1:3000".parse().unwrap(),
    ]))
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
    .allow_headers([AUTHORIZATION, CONTENT_TYPE]);
```

**MIT-3: Add Request Body Size Limits**

Add axum body size limits to prevent memory exhaustion:
```rust
router = router.layer(DefaultBodyLimit::max(10 * 1024 * 1024)); // 10MB max
```

**MIT-4: Bound Search Parameters**

Add upper bounds to `SearchRequest.k` and all vector dimensions:
```rust
const MAX_K: usize = 10_000;
const MAX_VECTOR_DIM: usize = 65_536;

// In search handler:
let k = req.k.min(MAX_K);
if req.vector.len() > MAX_VECTOR_DIM {
    return Err(Error::InvalidRequest("vector dimension too large".into()));
}
```

### 8.2 High Priority (Address During Integration)

**MIT-5: Integrate Solver WASM into Kernel Pack Framework**

The solver's WASM modules should be treated as kernel packs:
1. Sign solver WASM modules with Ed25519
2. Add solver kernel hashes to the `TrustedKernelAllowlist`
3. Execute solver WASM through the `KernelManager` with epoch deadlines
4. Set memory limits proportional to problem size with an absolute ceiling

**MIT-6: Enforce Serialization Size Limits**

For all deserialization of untrusted input:
```rust
// For bincode:
let config = bincode::config::standard()
    .with_limit::<{ 10 * 1024 * 1024 }>(); // 10MB max

// For serde_json, use axum body limits + custom deserializer:
let value: ProblemDefinition = serde_json::from_slice(&body)?;
validate_problem_input(&value)?; // Application-level validation
```

**MIT-7: Add MCP Tool Rate Limiting**

Implement per-agent rate limiting for MCP tools:
```rust
struct RateLimiter {
    windows: DashMap<String, (Instant, u32)>,
    max_per_minute: u32,
}

impl RateLimiter {
    fn check(&self, agent_id: &str) -> Result<(), McpError> {
        let mut entry = self.windows.entry(agent_id.to_string())
            .or_insert((Instant::now(), 0));
        if entry.0.elapsed() > Duration::from_secs(60) {
            *entry = (Instant::now(), 0);
        }
        entry.1 += 1;
        if entry.1 > self.max_per_minute {
            return Err(McpError::RateLimited);
        }
        Ok(())
    }
}
```

**MIT-8: Require PermitToken for Solver MCP Tools**

Solver MCP tools should require a valid `PermitToken` from the coherence gate:
```rust
pub async fn solve_problem(&self, call: McpToolCall) -> Result<McpToolResult, McpError> {
    // 1. Extract and validate permit token
    let token = call.arguments.get("permit_token")
        .ok_or(McpError::InvalidRequest("missing permit_token".into()))?;
    self.gate.verify_token(token).await?;

    // 2. Validate problem input
    let problem: ProblemDefinition = serde_json::from_value(call.arguments.clone())?;
    validate_problem_input(&problem)?;

    // 3. Execute solver with resource limits
    self.execute_solver(problem).await
}
```

### 8.3 Medium Priority (Address Post-Integration)

**MIT-9: Compile-Time Gating of Insecure Modes**

Add feature gates to prevent insecure constructors in release builds:
```rust
#[cfg(any(test, feature = "insecure-dev"))]
pub fn insecure_no_verify() -> Self { ... }

#[cfg(not(any(test, feature = "insecure-dev")))]
pub fn insecure_no_verify() -> Self {
    compile_error!("insecure_no_verify is not available in production builds");
}
```

**MIT-10: Remove Hardcoded Default Backup Password**

Replace the hardcoded default password in edge-net identity management:
```javascript
// Instead of:
const password = this.options.backupPassword || 'edge-net-default-key';

// Require explicit password:
if (!this.options.backupPassword) {
    throw new Error('backupPassword is required for identity persistence');
}
```

**MIT-11: Validate Collection Names**

Add collection name validation to prevent injection and path traversal:
```rust
fn validate_collection_name(name: &str) -> Result<(), Error> {
    if name.is_empty() || name.len() > 128 {
        return Err(Error::InvalidRequest("collection name must be 1-128 chars".into()));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(Error::InvalidRequest("collection name must be alphanumeric".into()));
    }
    Ok(())
}
```

**MIT-12: Add Solver-Specific Audit Trail**

Extend the witness chain to include solver invocations:
```rust
let witness_entry = WitnessEntry {
    prev_hash: previous_hash,
    action_hash: shake256_256(&solver_invocation_bytes),
    timestamp_ns: current_time_ns(),
    witness_type: WITNESS_TYPE_SOLVER_INVOCATION,
};
```

### 8.4 Low Priority (Long-Term Hardening)

**MIT-13: Fuzz Testing for Deserialization Paths**

Set up `cargo-fuzz` targets for all deserialization entry points:
- `serde_json::from_str::<ProblemDefinition>()`
- `bincode::decode_from_slice::<VectorEntry>()`
- `KernelManifest::from_json()`
- All `decode_*` functions in rvf-crypto

**MIT-14: Security Headers for Solver Express Server**

Verify that the solver's Express server includes:
```javascript
app.use(helmet({
    contentSecurityPolicy: { directives: { defaultSrc: ["'self'"] } },
    crossOriginEmbedderPolicy: true,
    crossOriginOpenerPolicy: true,
    crossOriginResourcePolicy: { policy: "same-origin" },
    hsts: { maxAge: 31536000, includeSubDomains: true },
    referrerPolicy: { policy: "no-referrer" },
}));
```

**MIT-15: unsafe Code Audit**

Commission a focused audit of the 90 `unsafe` blocks in ruvector-core:
- `/crates/ruvector-core/src/simd_intrinsics.rs` (40 blocks) - SIMD intrinsics
- `/crates/ruvector-core/src/arena.rs` (23 blocks) - Arena allocator
- `/crates/ruvector-core/src/cache_optimized.rs` (19 blocks) - Cache-optimized structures
- `/crates/ruvector-core/src/quantization.rs` (8 blocks) - Quantization

Priority areas: arena allocator pointer arithmetic and cache-optimized data structures where bounds checking may be insufficient.

**MIT-16: TLS for All Network Communication**

Both ruvector-server and the solver Express server should support TLS:
- Require TLS for non-localhost deployments
- Support mTLS for service-to-service communication
- Use certificate pinning for MCP tool connections

---

## Appendix A: STRIDE Analysis for Solver Integration

| Threat | Category | Risk | Mitigation |
|--------|----------|------|------------|
| Attacker submits malicious problem to solver via API | Tampering | High | MIT-6, MIT-4, Section 7 validation |
| Attacker bypasses solver resource limits via crafted WASM | Elevation of Privilege | High | MIT-5 (kernel pack framework) |
| Attacker enumerates gate decisions via receipt API | Information Disclosure | Medium | MIT-7 (rate limiting), AC-2 auth |
| Attacker floods solver with expensive problems | Denial of Service | High | MIT-7, MIT-8, Section 7.3 rate limits |
| Attacker replays valid permit token for unauthorized solver use | Spoofing | Medium | Token TTL, nonce in token |
| Agent makes solver calls without audit trail | Repudiation | Medium | MIT-12 (solver audit trail) |
| Attacker modifies solver WASM binary | Tampering | High | MIT-5 (Ed25519 + allowlist) |
| Compromised dependency injects malicious code | Tampering | Medium | MIT-14, Section 6.4 supply chain |

## Appendix B: Security Testing Checklist for Integration

- [ ] All solver API endpoints reject payloads > 10MB
- [ ] `k` parameter in search is bounded to MAX_K
- [ ] Collection names are validated (alphanumeric + hyphens, max 128 chars)
- [ ] Solver WASM modules are signed and allowlisted
- [ ] Solver WASM execution has epoch deadlines proportional to problem size
- [ ] Solver WASM memory is limited to MAX_SOLVER_PAGES
- [ ] MCP solver tools require valid PermitToken
- [ ] MCP tools have per-agent rate limiting
- [ ] Deserialization uses size limits (bincode `with_limit`, JSON body limit)
- [ ] Session IDs are server-generated UUIDs (not client-provided)
- [ ] Session count per client is bounded
- [ ] Express server has helmet with strict CSP
- [ ] CORS is restricted to known origins (not `Any`)
- [ ] Authentication is required on mutating endpoints
- [ ] All `unsafe` code has been reviewed for solver integration paths
- [ ] `cargo audit` and `npm audit` pass with no critical vulnerabilities
- [ ] Fuzz testing targets exist for all deserialization entry points
- [ ] Solver results include tolerance bounds for floating-point results
- [ ] Cross-tool MCP calls are prevented (unidirectional flow)
- [ ] Witness chain entries are created for solver invocations
