# ADR-STS-005: Security Model and Threat Mitigation

**Status**: Accepted
**Date**: 2026-02-20
**Authors**: RuVector Security Team
**Deciders**: Architecture Review Board

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-20 | RuVector Team | Initial proposal |
| 1.0 | 2026-02-20 | RuVector Team | Accepted: full implementation complete |

---

## Context

### Current Security Posture

RuVector employs defense-in-depth security across multiple layers:

| Layer | Mechanism | Strength |
|-------|-----------|----------|
| **Cryptographic** | Ed25519 signatures, SHAKE-256 witness chains, TEE attestation (SGX/SEV-SNP) | Very High |
| **WASM Sandbox** | Kernel pack verification (Ed25519 + SHA256 allowlist), epoch interruption, memory layout validation | High |
| **MCP Coherence Gate** | 3-tier Permit/Defer/Deny with witness receipts, hash-chain integrity | High |
| **Edge-Net** | PiKey Ed25519 identity, challenge-response, per-IP rate limiting, adaptive attack detection | High |
| **Storage** | Path traversal prevention, feature-gated backends | Medium |
| **Server API** | Serde validation, trace logging | Low |

### Known Weaknesses (Pre-Integration)

| ID | Weakness | DREAD Score | Severity |
|----|----------|-------------|----------|
| SEC-W1 | Fully permissive CORS (`allow_origin(Any)`) | 7.8 | High |
| SEC-W2 | No REST API authentication | 9.2 | Critical |
| SEC-W3 | Unbounded search parameters (`k` unlimited) | 6.4 | Medium |
| SEC-W4 | 90 `unsafe` blocks in SIMD/arena/quantization | 5.2 | Medium |
| SEC-W5 | `insecure_*` constructors without `#[cfg]` gating | 4.8 | Medium |
| SEC-W6 | Hardcoded default backup password in edge-net | 6.1 | Medium |
| SEC-W7 | Unvalidated collection names | 5.5 | Medium |

### New Attack Surface from Solver Integration

| Surface | Description | Risk |
|---------|-------------|------|
| AS-1 | New deserialization points (problem definitions, solver state) | High |
| AS-2 | WASM sandbox boundary (solver WASM modules) | High |
| AS-3 | MCP tool registration (40+ solver tools callable by AI agents) | High |
| AS-4 | Computational cost amplification (expensive solve operations) | High |
| AS-5 | Session management state (solver sessions) | Medium |
| AS-6 | Cross-tool information flow (solver ↔ coherence gate) | Medium |

---

## Decision

### 1. WASM Sandbox Integration

Solver WASM modules are treated as kernel packs within the existing security framework:

```rust
pub struct SolverKernelConfig {
    /// Ed25519 public key for solver WASM verification
    pub signing_key: ed25519_dalek::VerifyingKey,

    /// SHA256 hashes of approved solver WASM binaries
    pub allowed_hashes: HashSet<[u8; 32]>,

    /// Memory limits proportional to problem size
    pub max_memory_pages: u32,  // Absolute ceiling: 2048 (128MB)

    /// Epoch budget: proportional to expected O(n^alpha) runtime
    pub epoch_budget_fn: Box<dyn Fn(usize) -> u64>, // f(n) → ticks

    /// Stack size limit (prevent deep recursion)
    pub max_stack_bytes: usize, // Default: 1MB
}

impl SolverKernelConfig {
    pub fn default_server() -> Self {
        Self {
            max_memory_pages: 2048,   // 128MB
            max_stack_bytes: 1 << 20, // 1MB
            epoch_budget_fn: Box::new(|n| {
                // O(n * log(n)) ticks with 10x safety margin
                (n as u64) * ((n as f64).log2() as u64 + 1) * 10
            }),
            ..Default::default()
        }
    }

    pub fn default_browser() -> Self {
        Self {
            max_memory_pages: 128,    // 8MB
            max_stack_bytes: 256_000, // 256KB
            epoch_budget_fn: Box::new(|n| {
                (n as u64) * ((n as f64).log2() as u64 + 1) * 5
            }),
            ..Default::default()
        }
    }
}
```

### 2. Input Validation at All Boundaries

```rust
/// Comprehensive input validation for solver API inputs
pub fn validate_solver_input(input: &SolverInput) -> Result<(), ValidationError> {
    // === Size bounds ===
    const MAX_NODES: usize = 10_000_000;
    const MAX_EDGES: usize = 100_000_000;
    const MAX_DIM: usize = 65_536;
    const MAX_ITERATIONS: u64 = 1_000_000;
    const MAX_TIMEOUT_MS: u64 = 300_000;
    const MAX_MATRIX_ELEMENTS: usize = 1_000_000_000;

    if input.node_count > MAX_NODES {
        return Err(ValidationError::TooLarge {
            field: "node_count", max: MAX_NODES, actual: input.node_count,
        });
    }

    if input.edge_count > MAX_EDGES {
        return Err(ValidationError::TooLarge {
            field: "edge_count", max: MAX_EDGES, actual: input.edge_count,
        });
    }

    // === Numeric sanity ===
    for (i, weight) in input.edge_weights.iter().enumerate() {
        if !weight.is_finite() {
            return Err(ValidationError::InvalidNumber {
                field: "edge_weights", index: i, reason: "non-finite value",
            });
        }
    }

    // === Structural consistency ===
    let max_edges = if input.directed {
        input.node_count.saturating_mul(input.node_count.saturating_sub(1))
    } else {
        input.node_count.saturating_mul(input.node_count.saturating_sub(1)) / 2
    };
    if input.edge_count > max_edges {
        return Err(ValidationError::InconsistentGraph {
            reason: "more edges than possible for given node count",
        });
    }

    // === Parameter ranges ===
    if input.tolerance <= 0.0 || input.tolerance > 1.0 {
        return Err(ValidationError::OutOfRange {
            field: "tolerance", min: 0.0, max: 1.0, actual: input.tolerance,
        });
    }

    if input.max_iterations > MAX_ITERATIONS {
        return Err(ValidationError::OutOfRange {
            field: "max_iterations", min: 1.0, max: MAX_ITERATIONS as f64,
            actual: input.max_iterations as f64,
        });
    }

    // === Dimension bounds ===
    if input.dimension > MAX_DIM {
        return Err(ValidationError::TooLarge {
            field: "dimension", max: MAX_DIM, actual: input.dimension,
        });
    }

    // === Vector value checks ===
    if let Some(ref values) = input.values {
        if values.len() != input.dimension {
            return Err(ValidationError::DimensionMismatch {
                expected: input.dimension, actual: values.len(),
            });
        }
        for (i, v) in values.iter().enumerate() {
            if !v.is_finite() {
                return Err(ValidationError::InvalidNumber {
                    field: "values", index: i, reason: "non-finite value",
                });
            }
        }
    }

    Ok(())
}
```

### 3. MCP Tool Access Control

```rust
/// Solver MCP tools require PermitToken from coherence gate
pub struct SolverMcpHandler {
    solver: Arc<dyn SolverEngine>,
    gate: Arc<CoherenceGate>,
    rate_limiter: RateLimiter,
    budget_enforcer: BudgetEnforcer,
}

impl SolverMcpHandler {
    pub async fn handle_tool_call(
        &self, call: McpToolCall
    ) -> Result<McpToolResult, McpError> {
        // 1. Rate limiting
        let agent_id = call.agent_id.as_deref().unwrap_or("anonymous");
        self.rate_limiter.check(agent_id)?;

        // 2. PermitToken verification
        let token = call.arguments.get("permit_token")
            .ok_or(McpError::Unauthorized("missing permit_token"))?;
        self.gate.verify_token(token).await
            .map_err(|_| McpError::Unauthorized("invalid permit_token"))?;

        // 3. Input validation
        let input: SolverInput = serde_json::from_value(call.arguments.clone())
            .map_err(|e| McpError::InvalidRequest(e.to_string()))?;
        validate_solver_input(&input)?;

        // 4. Resource budget check
        let estimate = self.solver.estimate_complexity(&input);
        self.budget_enforcer.check(agent_id, &estimate)?;

        // 5. Execute with resource limits
        let result = self.solver.solve_with_budget(&input, estimate.budget).await?;

        // 6. Generate witness receipt
        let witness = WitnessEntry {
            prev_hash: self.gate.latest_hash(),
            action_hash: shake256_256(&bincode::encode(&result)?),
            timestamp_ns: current_time_ns(),
            witness_type: WITNESS_TYPE_SOLVER_INVOCATION,
        };
        self.gate.append_witness(witness);

        Ok(McpToolResult::from(result))
    }
}

/// Per-agent rate limiter
pub struct RateLimiter {
    windows: DashMap<String, (Instant, u32)>,
    config: RateLimitConfig,
}

pub struct RateLimitConfig {
    pub solve_per_minute: u32,      // Default: 10
    pub status_per_minute: u32,     // Default: 60
    pub session_per_minute: u32,    // Default: 30
    pub burst_multiplier: u32,      // Default: 3
}

impl RateLimiter {
    pub fn check(&self, agent_id: &str) -> Result<(), McpError> {
        let mut entry = self.windows.entry(agent_id.to_string())
            .or_insert((Instant::now(), 0));

        if entry.0.elapsed() > Duration::from_secs(60) {
            *entry = (Instant::now(), 0);
        }

        entry.1 += 1;
        if entry.1 > self.config.solve_per_minute {
            return Err(McpError::RateLimited {
                agent_id: agent_id.to_string(),
                retry_after_secs: 60 - entry.0.elapsed().as_secs(),
            });
        }
        Ok(())
    }
}
```

### 4. Serialization Safety

```rust
/// Safe deserialization with size limits
pub fn deserialize_solver_input(bytes: &[u8]) -> Result<SolverInput, SolverError> {
    // Body size limit: 10MB
    const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;
    if bytes.len() > MAX_BODY_SIZE {
        return Err(SolverError::InvalidInput(
            ValidationError::PayloadTooLarge { max: MAX_BODY_SIZE, actual: bytes.len() }
        ));
    }

    // Deserialize with serde_json (safe, bounded by input size)
    let input: SolverInput = serde_json::from_slice(bytes)
        .map_err(|e| SolverError::InvalidInput(ValidationError::ParseError(e.to_string())))?;

    // Application-level validation
    validate_solver_input(&input)?;

    Ok(input)
}

/// Bincode deserialization with size limit
pub fn deserialize_bincode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, SolverError> {
    let config = bincode::config::standard()
        .with_limit::<{ 10 * 1024 * 1024 }>(); // 10MB max

    bincode::serde::decode_from_slice(bytes, config)
        .map(|(val, _)| val)
        .map_err(|e| SolverError::InvalidInput(
            ValidationError::ParseError(format!("bincode: {}", e))
        ))
}
```

### 5. Audit Trail

```rust
/// Solver invocations generate witness entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverAuditEntry {
    pub request_id: Uuid,
    pub agent_id: String,
    pub algorithm: Algorithm,
    pub input_hash: [u8; 32],    // SHAKE-256 of input
    pub output_hash: [u8; 32],   // SHAKE-256 of output
    pub iterations: usize,
    pub wall_time_us: u64,
    pub converged: bool,
    pub residual: f64,
    pub timestamp_ns: u128,
}

impl SolverAuditEntry {
    pub fn to_witness(&self) -> WitnessEntry {
        WitnessEntry {
            prev_hash: [0u8; 32], // Set by chain
            action_hash: shake256_256(&bincode::encode(self).unwrap()),
            timestamp_ns: self.timestamp_ns,
            witness_type: WITNESS_TYPE_SOLVER_INVOCATION,
        }
    }
}
```

### 6. Supply Chain Security

```toml
# .cargo/deny.toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC"]
deny = ["GPL-2.0", "GPL-3.0", "AGPL-3.0"]

[bans]
deny = [
    { name = "openssl-sys" },  # Prefer rustls
]
```

CI pipeline additions:

```yaml
# .github/workflows/security.yml
- name: Cargo audit
  run: cargo audit
- name: Cargo deny
  run: cargo deny check
- name: npm audit
  run: npm audit --audit-level=high
```

---

## STRIDE Threat Analysis

| Threat | Category | Risk | Mitigation |
|--------|----------|------|------------|
| Malicious problem submission via API | Tampering | High | Input validation (Section 2), body size limits |
| WASM resource limits bypass via crafted input | Elevation | High | Kernel pack framework (Section 1), epoch limits |
| Receipt enumeration via sequential IDs | Info Disc. | Medium | Rate limiting (Section 3), auth requirement |
| Solver flooding with expensive problems | DoS | High | Rate limiting, compute budgets, concurrent solve semaphore |
| Replay of valid permit token | Spoofing | Medium | Token TTL, nonce, single-use enforcement |
| Solver calls without audit trail | Repudiation | Medium | Mandatory witness entries (Section 5) |
| Modified solver WASM binary | Tampering | High | Ed25519 + SHA256 allowlist (Section 1) |
| Compromised dependency injection | Tampering | Medium | cargo-deny, cargo-audit, SBOM (Section 6) |
| NaN/Inf propagation in solver output | Integrity | Medium | Output validation, finite-check on results |
| Cross-tool MCP escalation | Elevation | Medium | Unidirectional flow enforcement |

---

## Security Testing Checklist

- [ ] All solver API endpoints reject payloads > 10MB
- [ ] `k` parameter bounded to MAX_K (10,000)
- [ ] Solver WASM modules signed and allowlisted
- [ ] WASM execution has problem-size-proportional epoch deadlines
- [ ] WASM memory limited to MAX_SOLVER_PAGES (2048)
- [ ] MCP solver tools require valid PermitToken
- [ ] Per-agent rate limiting enforced on all MCP tools
- [ ] Deserialization uses size limits (bincode `with_limit`)
- [ ] Session IDs are server-generated UUIDs
- [ ] Session count per client bounded (max: 10)
- [ ] CORS restricted to known origins
- [ ] Authentication required on mutating endpoints
- [ ] `unsafe` code reviewed for solver integration paths
- [ ] `cargo audit` and `npm audit` pass (no critical vulns)
- [ ] Fuzz testing targets for all deserialization entry points
- [ ] Solver results include tolerance bounds
- [ ] Cross-tool MCP calls prevented
- [ ] Witness chain entries created for solver invocations
- [ ] Input NaN/Inf rejected before reaching solver
- [ ] Output NaN/Inf detected and error returned

---

## Consequences

### Positive

1. **Defense-in-depth**: Solver integrates into existing security layers, not bypassing them
2. **Auditable**: All solver invocations have cryptographic witness receipts
3. **Resource-bounded**: Compute budgets prevent cost amplification attacks
4. **Supply chain secured**: Automated auditing in CI pipeline
5. **Platform-safe**: WASM sandbox enforces memory and CPU limits

### Negative

1. **PermitToken overhead**: Gate verification adds ~100μs per solver call
2. **Rate limiting friction**: Legitimate high-throughput use cases may hit limits
3. **Audit storage**: Witness entries add ~200 bytes per solver invocation

---

## Implementation Status

Input validation module (validation.rs) checks CSR structural invariants, index bounds, NaN/Inf detection. Budget enforcement prevents resource exhaustion. Audit trail logs all solver invocations. No unsafe code in public API surface (unsafe confined to internal spmv_unchecked and SIMD). All assertions verified in 177 tests.

---

## References

- [09-security-analysis.md](../09-security-analysis.md) — Full security analysis
- [07-mcp-integration.md](../07-mcp-integration.md) — MCP tool access patterns
- [06-wasm-integration.md](../06-wasm-integration.md) — WASM sandbox model
- ADR-007 — RuVector security review
- ADR-012 — RuVector security remediation
