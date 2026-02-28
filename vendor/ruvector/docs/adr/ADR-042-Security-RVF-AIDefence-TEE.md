# ADR-042: Security RVF — AIDefence + TEE Hardened Cognitive Container

| Field       | Value                                          |
|-------------|------------------------------------------------|
| Status      | Accepted                                       |
| Date        | 2025-02-21                                     |
| Authors     | ruv                                            |
| Supersedes  | —                                              |
| Implements  | ADR-041 Tier 1 (Security Container)            |

## Context

ADR-041 identified 15 npm packages suitable for RVF cognitive containers. This ADR
specifies the **ultimate security RVF** — a single `.rvf` file that combines:

1. **AIDefence** — 5-layer adversarial defense (prompt injection, jailbreak, PII, behavioral, policy)
2. **TEE attestation** — Hardware-bound trust (SGX, SEV-SNP, TDX, ARM CCA)
3. **Hardened Linux microkernel** — Minimal attack surface boot image + KernelBinding anti-tamper
4. **Coherence Gate** — Anytime-valid permission authorization
5. **RBAC + Ed25519 signing** — Role-based access with cryptographic proof
6. **Witness chain audit** — Tamper-evident hash-chained event log
7. **Self-bootstrapping** — Dual WASM (Interpreter + Microkernel) for standalone execution
8. **Dashboard** — Embedded security monitoring UI (DASHBOARD_SEG)
9. **Quantization** — Scalar (int8, 4x) + Binary (1-bit, 32x) compression
10. **Lifecycle** — Filter deletion, compaction, and permanent freeze/seal

The result is a self-contained, bootable, cryptographically sealed security appliance
with 30 verified capabilities, end-to-end from silicon to application layer.

## Decision

Build `security_hardened.rvf` as a capstone example in `examples/rvf/examples/` that
exercises every security primitive in the RVF format.

## Architecture

```
security_hardened.rvf
├── KERNEL_SEG (0x0E)     Hardened Linux 6.x + KernelBinding (128B anti-tamper)
├── EBPF_SEG (0x0F)       Packet filter + syscall policy enforcer
├── WASM_SEG #1 (0x10)    AIDefence engine (prompt injection, PII, jailbreak)
├── WASM_SEG #2 (0x10)    Interpreter runtime (self-bootstrapping)
├── DASHBOARD_SEG (0x11)  Security monitoring web UI
├── VEC_SEG (0x01)        Threat signature embeddings (512-dim)
├── INDEX_SEG (0x02)      HNSW index over threat vectors (m=32)
├── CRYPTO_SEG (0x0C)     Ed25519 keys + TEE-bound key records
├── WITNESS_SEG (0x0A)    30-entry security lifecycle chain
├── META_SEG (0x07)       Security policy + RBAC config + AIDefence rules
├── PROFILE_SEG (0x0B)    Domain profile: RVSecurity
├── PolicyKernel (0x31)   Gate thresholds + coherence config
├── MANIFEST_SEG (0x05)   Signed manifest with hardening fields
└── Signature Footer      Ed25519 over entire artifact
```

### Segment Budget

| Segment | Purpose | Size Budget |
|---------|---------|-------------|
| KERNEL_SEG | Hardened Linux bzImage | ~1.6 MB |
| EBPF_SEG | Firewall + syscall filter | ~8 KB |
| WASM_SEG | AIDefence WASM engine | ~256 KB |
| VEC_SEG | Threat embeddings (1000 x 512) | ~2 MB |
| INDEX_SEG | HNSW graph | ~512 KB |
| CRYPTO_SEG | Keys + TEE attestation records | ~4 KB |
| WITNESS_SEG | 30-entry audit chain | ~2 KB |
| META_SEG | Policy JSON + RBAC matrix | ~4 KB |
| PROFILE_SEG | Domain profile | ~512 B |
| PolicyKernel | Gate config | ~1 KB |
| MANIFEST_SEG | Signed directory | ~512 B |
| **Total** | | **~4.4 MB** |

## Security Layers

### Layer 1: Hardware Root of Trust (TEE)

```
┌─────────────────────────────────────┐
│ AttestationHeader (112 bytes)       │
│ ├── platform: SGX/SEV-SNP/TDX/CCA  │
│ ├── measurement: MRENCLAVE          │
│ ├── signer_id: MRSIGNER            │
│ ├── nonce: anti-replay              │
│ ├── svn: security version           │
│ └── quote: opaque attestation blob  │
└─────────────────────────────────────┘
```

- Hardware TEE attestation records in CRYPTO_SEG
- TEE-bound key records: keys sealed to enclave measurement
- Platform verification: correct TEE + measurement + validity window
- Multi-platform: SGX, SEV-SNP, TDX, ARM CCA in single witness chain

### Layer 2: Kernel Hardening

```
KernelHeader flags:
  KERNEL_FLAG_SIGNED           = 0x0001
  KERNEL_FLAG_COMPRESSED       = 0x0002
  KERNEL_FLAG_REQUIRES_TEE     = 0x0004
  KERNEL_FLAG_MEASURED         = 0x0008
  KERNEL_FLAG_REQUIRES_KVM     = 0x0010
  KERNEL_FLAG_ATTESTATION_READY = 0x0400
```

Linux tinyconfig + hardening options:
- `CONFIG_SECURITY_LOCKDOWN_LSM=y` — Kernel lockdown
- `CONFIG_SECURITY_LANDLOCK=y` — Landlock sandboxing
- `CONFIG_SECCOMP=y` — Syscall filtering
- `CONFIG_STATIC_USERMODEHELPER=y` — No dynamic module loading
- `CONFIG_STRICT_KERNEL_RWX=y` — W^X enforcement
- `CONFIG_INIT_ON_ALLOC_DEFAULT_ON=y` — Memory init on alloc
- `CONFIG_BLK_DEV_INITRD=y` — Initramfs support
- No loadable modules, no debugfs, no procfs write, no sysfs write

### Layer 3: eBPF Enforcement

Two eBPF programs embedded:

1. **XDP Packet Filter** — Drop all traffic except allowed ports
   - Allow: TCP 8443 (HTTPS API), TCP 9090 (metrics)
   - Drop everything else at XDP layer (before kernel stack)

2. **Seccomp Syscall Filter** — Allowlist-only syscalls
   - Allow: read, write, mmap, munmap, close, exit, futex, epoll_*
   - Deny: execve, fork, clone3, ptrace, mount, umount, ioctl(TIOCSTI)

### Layer 4: AIDefence (WASM Engine)

The WASM segment contains a compiled AIDefence engine with:

| Detector | Latency | Description |
|----------|---------|-------------|
| Prompt Injection | <5ms | 30+ regex patterns + semantic similarity |
| Jailbreak | <5ms | DAN, role manipulation, system prompt extraction |
| PII Detection | <5ms | Email, phone, SSN, credit card, API keys, IP |
| Control Characters | <1ms | Unicode homoglyphs, null bytes, escape sequences |
| Behavioral Analysis | <100ms | EMA baseline deviation per user |
| Policy Verification | <500ms | Custom pattern matching + domain allowlists |

Threat levels: `none` → `low` → `medium` → `high` → `critical`

Default block threshold: `medium` (configurable via META_SEG policy)

### Layer 5: Cryptographic Integrity

- **Ed25519 signing** (RFC 8032): Every segment signed individually
- **Witness chain**: HMAC-SHA256 hash-chained audit entries
- **Content hashing**: SHAKE-256 truncated hashes in HardeningFields
- **SecurityPolicy::Paranoid**: Full chain verification on mount
- **Key rotation**: Witness entry records rotation event

### Layer 6: Access Control (RBAC + Coherence Gate)

```
Role Matrix:
┌──────────┬───────┬──────┬────────┬───────┬──────────┐
│ Role     │ Write │ Read │ Derive │ Audit │ Gate     │
├──────────┼───────┼──────┼────────┼───────┼──────────┤
│ Admin    │ ✓     │ ✓    │ ✓      │ ✓     │ permit   │
│ Operator │ ✓     │ ✓    │ ✗      │ ✓     │ permit   │
│ Analyst  │ ✗     │ ✓    │ ✗      │ ✓     │ defer    │
│ Reader   │ ✗     │ ✓    │ ✗      │ ✗     │ defer    │
│ Auditor  │ ✗     │ ✓    │ ✗      │ ✓     │ permit   │
│ Guest    │ ✗     │ ✗    │ ✗      │ ✗     │ deny     │
└──────────┴───────┴──────┴────────┴───────┴──────────┘
```

Coherence Gate thresholds (PolicyKernel segment):
- `permit_threshold`: 0.85
- `defer_threshold`: 0.50
- `deny_threshold`: 0.0
- `escalation_window_ns`: 300_000_000_000 (5 min)
- `max_deferred_queue`: 100

## Capabilities Confirmed

| # | Capability | Segment | Verification |
|---|-----------|---------|-------------|
| 1 | TEE attestation (SGX, SEV-SNP, TDX, ARM CCA) | CRYPTO_SEG | Quote validation + binding check |
| 2 | TEE-bound key records | CRYPTO_SEG | Platform + measurement + validity |
| 3 | Hardened kernel boot | KERNEL_SEG | Flags: SIGNED, REQUIRES_TEE, MEASURED |
| 4 | KernelBinding anti-tamper | KERNEL_SEG | manifest_root_hash + policy_hash binding |
| 5 | eBPF packet filter | EBPF_SEG | XDP drop except allowlisted ports |
| 6 | eBPF syscall filter | EBPF_SEG | Seccomp allowlist enforcement |
| 7 | AIDefence prompt injection | WASM_SEG | 12 pattern detection |
| 8 | AIDefence jailbreak detect | WASM_SEG | DAN, role manipulation, 8 patterns |
| 9 | AIDefence PII scanning | WASM_SEG | Email, SSN, credit card, API keys |
| 10 | AIDefence code/encoding attack | WASM_SEG | XSS, eval, base64, unicode tricks |
| 11 | Self-bootstrapping | WASM_SEG x2 | Interpreter + Microkernel dual WASM |
| 12 | Security monitoring dashboard | DASHBOARD_SEG | Embedded security UI |
| 13 | Ed25519 segment signing | CRYPTO_SEG | Per-segment cryptographic proof |
| 14 | Witness chain audit trail | WITNESS_SEG | 30-entry HMAC-SHA256 chain |
| 15 | Content hash hardening | MANIFEST_SEG | SHAKE-256 content verification |
| 16 | Security policy (Paranoid) | MANIFEST_SEG | Full chain verification on mount |
| 17 | RBAC access control | META_SEG | 6 roles with permission matrix |
| 18 | Coherence Gate authorization | PolicyKernel | Anytime-valid decision with witness receipts |
| 19 | Key rotation | CRYPTO_SEG + WITNESS | Old key rejected, new key active |
| 20 | Tamper detection | WITNESS_SEG | 3/3 attacks rejected |
| 21 | Multi-tenant isolation | Store derivation | Lineage-linked derived stores |
| 22 | COW branching | Store branching | Forensic-grade immutable snapshots |
| 23 | Audited k-NN queries | WITNESS_SEG | Witness entry on every search |
| 24 | Threat vector similarity | VEC_SEG + INDEX | k-NN over 1000 threat embeddings |
| 25 | Data exfiltration detection | WASM_SEG | curl/wget/fetch/webhook patterns |
| 26 | Scalar quantization (int8) | rvf-quant | 4x compression, L2 distance preserved |
| 27 | Binary quantization (1-bit) | rvf-quant | 32x compression, Hamming distance |
| 28 | Filter deletion + compaction | Store lifecycle | Purge + reclaim dead space |
| 29 | QEMU requirements check | rvf-launch | Bootability proof (dry-run) |
| 30 | Freeze/seal | Store freeze | Permanent read-only immutability |

## MCP Tools (Security Container)

When served via MCP, the security RVF exposes these tools:

| # | Tool | Description |
|---|------|-------------|
| 1 | `aidefence_scan` | Analyze input for all threat types |
| 2 | `aidefence_sanitize` | Remove/mask dangerous content |
| 3 | `aidefence_validate_response` | Check LLM output safety |
| 4 | `aidefence_audit_log` | Get audit trail entries |
| 5 | `gate_permit` | Request action authorization |
| 6 | `gate_receipt` | Retrieve witness receipt by sequence |
| 7 | `gate_replay` | Deterministic decision replay |
| 8 | `tee_attest` | Generate TEE attestation record |
| 9 | `tee_verify` | Verify attestation quote |
| 10 | `tee_bind_key` | Create TEE-bound key record |
| 11 | `rbac_check` | Verify role permissions |
| 12 | `rbac_assign` | Assign role to principal |
| 13 | `threat_search` | k-NN over threat embeddings |
| 14 | `threat_ingest` | Add new threat signatures |
| 15 | `witness_chain` | Get/verify witness chain |
| 16 | `policy_get` | Read security policy config |

## HTTP API Endpoints

```
Port 8443 (TLS required in production)

POST   /api/v1/scan              AIDefence threat analysis
POST   /api/v1/sanitize          Input sanitization
POST   /api/v1/validate          Response validation
GET    /api/v1/audit             Audit log (paginated)
POST   /api/v1/gate/permit       Gate authorization request
GET    /api/v1/gate/receipt/:seq Receipt by sequence
POST   /api/v1/tee/attest        Generate attestation
POST   /api/v1/tee/verify        Verify quote
POST   /api/v1/rbac/check        Permission check
POST   /api/v1/threats/search    Threat similarity search
GET    /api/v1/status             System health
GET    /api/v1/policy             Security policy config
```

## Implementation

### Files Created

| # | Path | Description |
|---|------|-------------|
| 1 | `examples/rvf/examples/security_hardened.rs` | Capstone security RVF example |
| 2 | `docs/adr/ADR-042-Security-RVF-AIDefence-TEE.md` | This ADR |

### Files Modified

| # | Path | Changes |
|---|------|---------|
| 1 | `examples/rvf/Cargo.toml` | Add `security_hardened` example entry |

## Verification

```bash
# Build the example
cd examples/rvf && cargo build --example security_hardened

# Run the example (creates + verifies the security RVF)
cargo run --example security_hardened

# Expected output (v3.0 — 30 capabilities):
#   Phase 1:  Threat vector knowledge base (1000 embeddings)
#   Phase 2:  Hardened kernel + KernelBinding (KERNEL_SEG)
#   Phase 3:  eBPF packet + syscall filters (EBPF_SEG)
#   Phase 4:  AIDefence WASM #1 Microkernel (WASM_SEG)
#   Phase 4b: WASM #2 Interpreter (self-bootstrapping)
#   Phase 5:  Security monitoring dashboard (DASHBOARD_SEG)
#   Phase 6:  TEE attestation (SGX, SEV-SNP, TDX, ARM CCA)
#   Phase 7:  TEE-bound key records
#   Phase 8:  RBAC access control (6 roles)
#   Phase 9:  Coherence Gate policy (PolicyKernel)
#   Phase 10: Scalar + Binary quantization
#   Phase 11: 30-entry witness chain
#   Phase 12: Ed25519 signing + Paranoid verification
#   Phase 13: Tamper detection (3 tests)
#   Phase 14: Filter deletion + compaction
#   Phase 15: Multi-tenant isolation + COW
#   Phase 16: AIDefence live tests (10 threat types)
#   Phase 17: QEMU requirements check
#   Phase 18: Component verification
#   Phase 19: Freeze — permanent immutability seal
#   All 30 capabilities verified.
```

## References

- ADR-033: Mandatory manifest signatures + HardeningFields
- ADR-041: RVF Cognitive Container identification
- ADR-041a: Detailed container implementations
- `rvf-types/src/attestation.rs`: AttestationHeader, TeePlatform
- `rvf-types/src/security.rs`: SecurityPolicy, HardeningFields
- `rvf-crypto`: Ed25519, witness chains, TEE attestation
- `ruvbot/src/security/AIDefenceGuard.ts`: AIDefence implementation
