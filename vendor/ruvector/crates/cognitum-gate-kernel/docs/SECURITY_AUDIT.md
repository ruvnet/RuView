# Security Audit Report: Cognitum Gate Implementation

**Audit Date:** 2026-01-17
**Auditor:** Claude Code Security Review Agent
**Scope:** cognitum-gate-kernel, cognitum-gate-tilezero, mcp-gate
**Risk Classification:** Uses CVSS-style severity (Critical/High/Medium/Low)

---

## Executive Summary

This security audit identified **17 security issues** across the cognitum-gate implementation:

| Severity | Count | Categories |
|----------|-------|------------|
| Critical | 2 | Cryptographic bypass, signature truncation |
| High | 4 | Memory safety, unsafe code, race conditions |
| Medium | 6 | Input validation, integer overflow, DoS vectors |
| Low | 5 | Information disclosure, edge cases |

**Recommendation:** The Critical issues in `permit.rs` must be fixed before production deployment as they completely bypass signature verification.

---

## Critical Issues

### CGK-001: Signature Verification Bypass (CRITICAL)

**Severity:** Critical
**Location:** `/home/user/ruvector/crates/cognitum-gate-tilezero/src/permit.rs:136-153`
**CVSS:** 9.8 (Critical)

**Description:**
The `Verifier::verify()` function does not actually verify signatures. It computes a hash from the token content and compares it to... the same hash computed from the same content. This comparison always succeeds.

```rust
// Lines 147-151 - BROKEN VERIFICATION
let expected_hash = blake3::hash(&content);
if hash.as_bytes() != expected_hash.as_bytes() {
    return Err(VerifyError::HashMismatch);
}
// hash == expected_hash ALWAYS - computed from same content!
```

**Impact:**
Any attacker can forge permit tokens. The cryptographic authentication is completely bypassed. All gate decisions can be spoofed.

**Recommended Fix:**
```rust
pub fn verify(&self, token: &PermitToken) -> Result<(), VerifyError> {
    let content = token.signable_content();
    let hash = blake3::hash(&content);

    // Reconstruct full 64-byte signature
    // REQUIRES: Store full signature in token, not truncated 32 bytes
    let signature = ed25519_dalek::Signature::from_bytes(&token.signature)
        .map_err(|_| VerifyError::SignatureFailed)?;

    // Actually verify the signature
    self.verifying_key
        .verify(hash.as_bytes(), &signature)
        .map_err(|_| VerifyError::SignatureFailed)?;

    Ok(())
}
```

---

### CGK-002: Ed25519 Signature Truncation (CRITICAL)

**Severity:** Critical
**Location:** `/home/user/ruvector/crates/cognitum-gate-tilezero/src/permit.rs:103-111`
**CVSS:** 9.1 (Critical)

**Description:**
The `sign_token` function truncates the 64-byte Ed25519 signature to 32 bytes:

```rust
// Line 109 - Discards half the signature!
token.mac.copy_from_slice(&signature.to_bytes()[..32]);
```

Ed25519 signatures are 64 bytes. Truncating to 32 bytes makes reconstruction impossible and verification meaningless.

**Impact:**
Combined with CGK-001, this makes signature verification completely non-functional. Even if verification was fixed, the stored signature cannot be reconstructed.

**Recommended Fix:**
```rust
// In PermitToken struct - change mac field:
pub signature: [u8; 64],  // Full Ed25519 signature

// In sign_token:
token.signature.copy_from_slice(&signature.to_bytes());
```

---

## High Severity Issues

### CGK-003: Unsafe Global Mutable State Without Synchronization

**Severity:** High
**Location:** `/home/user/ruvector/crates/cognitum-gate-kernel/src/lib.rs:413`
**CVSS:** 7.5

**Description:**
The global `TILE_STATE` is accessed through `static mut` without any synchronization primitives:

```rust
static mut TILE_STATE: Option<TileState> = None;
```

All WASM export functions (`init_tile`, `ingest_delta`, `tick`, etc.) access this mutable static unsafely.

**Impact:**
In multi-threaded contexts or if WASM threading is enabled, this creates data races leading to undefined behavior, memory corruption, or security bypasses.

**Recommended Fix:**
```rust
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

struct TileStateHolder {
    initialized: AtomicBool,
    state: UnsafeCell<Option<TileState>>,
}

// Or for single-threaded WASM, use OnceCell pattern
static TILE_STATE: once_cell::sync::OnceCell<RefCell<TileState>> = OnceCell::new();
```

---

### CGK-004: Unsafe Raw Pointer Dereference Without Validation

**Severity:** High
**Location:** `/home/user/ruvector/crates/cognitum-gate-kernel/src/lib.rs:207-210`
**CVSS:** 7.3

**Description:**
The `ingest_delta_raw` function casts a raw pointer without checking alignment:

```rust
pub unsafe fn ingest_delta_raw(&mut self, ptr: *const u8) -> bool {
    let delta = unsafe { &*(ptr as *const Delta) };  // No alignment check!
    self.ingest_delta(delta)
}
```

`Delta` likely requires alignment > 1 byte. Misaligned access is undefined behavior.

**Impact:**
Misaligned memory access causes undefined behavior on some architectures, potentially leading to crashes or exploitable memory corruption.

**Recommended Fix:**
```rust
pub unsafe fn ingest_delta_raw(&mut self, ptr: *const u8) -> bool {
    // Check alignment
    if (ptr as usize) % core::mem::align_of::<Delta>() != 0 {
        return false;
    }
    // Check null
    if ptr.is_null() {
        return false;
    }
    let delta = unsafe { &*(ptr as *const Delta) };
    self.ingest_delta(delta)
}
```

---

### CGK-005: Bump Allocator Race Condition

**Severity:** High
**Location:** `/home/user/ruvector/crates/cognitum-gate-kernel/src/lib.rs:70-99`
**CVSS:** 7.0

**Description:**
The bump allocator uses static mutable variables without synchronization:

```rust
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
static mut HEAP_PTR: usize = 0;

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe {
            let aligned = (HEAP_PTR + align - 1) & !(align - 1);  // Race condition!
            // ...
            HEAP_PTR = aligned + size;  // Non-atomic update!
        }
    }
}
```

**Impact:**
Concurrent allocations could return overlapping memory regions, leading to memory corruption.

**Recommended Fix:**
```rust
use core::sync::atomic::{AtomicUsize, Ordering};

static HEAP_PTR: AtomicUsize = AtomicUsize::new(0);

unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    loop {
        let current = HEAP_PTR.load(Ordering::Acquire);
        let aligned = (current + layout.align() - 1) & !(layout.align() - 1);
        let new_ptr = aligned + layout.size();

        if new_ptr > HEAP_SIZE {
            return core::ptr::null_mut();
        }

        if HEAP_PTR.compare_exchange_weak(current, new_ptr, Ordering::Release, Ordering::Relaxed).is_ok() {
            return unsafe { HEAP.as_mut_ptr().add(aligned) };
        }
    }
}
```

---

### CGK-006: Unchecked Union Access in Delta Processing

**Severity:** High
**Location:** `/home/user/ruvector/crates/cognitum-gate-kernel/src/lib.rs:288-321`
**CVSS:** 6.8

**Description:**
The `apply_delta` function uses unsafe union access based on a tag field:

```rust
DeltaTag::EdgeAdd => {
    let ea = unsafe { delta.get_edge_add() };  // Trusts tag
    // ...
}
```

If the tag is corrupted or maliciously set, accessing the wrong union variant leads to undefined behavior.

**Impact:**
A malformed delta with mismatched tag/data could cause memory corruption or information disclosure.

**Recommended Fix:**
- Add validation of delta integrity (checksum/hash)
- Use a safe enum representation instead of tagged union where possible
- Add bounds checking on union field values after extraction

---

## Medium Severity Issues

### CGK-007: Division by Zero in Threshold Computation

**Severity:** Medium
**Location:** `/home/user/ruvector/crates/cognitum-gate-tilezero/src/decision.rs:223-228`
**CVSS:** 5.9

**Description:**
Pre-computed reciprocals can cause division by zero:

```rust
let inv_min_cut = 1.0 / thresholds.min_cut;  // Zero if min_cut == 0
let inv_max_shift = 1.0 / thresholds.max_shift;  // Zero if max_shift == 0
let inv_tau_range = 1.0 / (thresholds.tau_permit - thresholds.tau_deny);  // Zero if equal
```

**Impact:**
Results in infinity/NaN values that propagate through decision logic, potentially causing incorrect permit/deny decisions.

**Recommended Fix:**
```rust
pub fn new(thresholds: GateThresholds) -> Result<Self, ThresholdError> {
    if thresholds.min_cut == 0.0 || thresholds.max_shift == 0.0 {
        return Err(ThresholdError::ZeroThreshold);
    }
    if (thresholds.tau_permit - thresholds.tau_deny).abs() < f64::EPSILON {
        return Err(ThresholdError::EqualTauRange);
    }
    // ... continue with safe reciprocal computation
}
```

---

### CGK-008: Integer Overflow in Token TTL Check

**Severity:** Medium
**Location:** `/home/user/ruvector/crates/cognitum-gate-tilezero/src/permit.rs:31-33`
**CVSS:** 5.3

**Description:**
The validity check can overflow:

```rust
pub fn is_valid_time(&self, now_ns: u64) -> bool {
    now_ns <= self.timestamp + self.ttl_ns  // Overflow possible!
}
```

If `timestamp + ttl_ns` overflows u64, the comparison becomes incorrect.

**Impact:**
Tokens with very large timestamps or TTLs could have incorrect validity checks, either expiring immediately or never expiring.

**Recommended Fix:**
```rust
pub fn is_valid_time(&self, now_ns: u64) -> bool {
    self.timestamp.checked_add(self.ttl_ns)
        .map(|expiry| now_ns <= expiry)
        .unwrap_or(true)  // If overflow, consider perpetually valid or use saturating
}
```

---

### CGK-009: Unbounded History Growth / DoS Vector

**Severity:** Medium
**Location:** `/home/user/ruvector/crates/cognitum-gate-tilezero/src/receipt.rs:124-132, 169-185`
**CVSS:** 5.0

**Description:**
The `ReceiptLog` uses a HashMap that grows unboundedly:

```rust
pub struct ReceiptLog {
    receipts: HashMap<u64, WitnessReceipt>,  // Grows forever
    // ...
}
```

Additionally, `verify_chain_to` iterates from 0 to sequence number, making it O(n) in chain length.

**Impact:**
Memory exhaustion attack by generating many decisions. Chain verification becomes increasingly slow.

**Recommended Fix:**
```rust
const MAX_RECEIPTS: usize = 100_000;

pub fn append(&mut self, receipt: WitnessReceipt) -> Result<(), LogFullError> {
    if self.receipts.len() >= MAX_RECEIPTS {
        // Implement pruning or return error
        self.prune_old_receipts();
    }
    // ...
}

// Use rolling window verification instead of full chain
pub fn verify_recent(&self, window: usize) -> Result<(), ChainVerifyError> {
    let start = self.latest_sequence.saturating_sub(window as u64);
    // Verify only recent entries
}
```

---

### CGK-010: Unchecked Array Index in Evidence Processing

**Severity:** Medium
**Location:** `/home/user/ruvector/crates/cognitum-gate-kernel/src/evidence.rs:407-416`
**CVSS:** 4.8

**Description:**
Window access uses unchecked indexing:

```rust
let idx = self.window_head as usize;
// Line 410: Assumes idx < WINDOW_SIZE
unsafe {
    *self.window.get_unchecked_mut(idx) = ObsRecord { obs, tick };
}
```

The bit masking on line 413 is correct, but it happens AFTER the unsafe access.

**Impact:**
If `window_head` is corrupted, out-of-bounds write occurs.

**Recommended Fix:**
```rust
// Apply mask BEFORE access
let idx = (self.window_head as usize) & (WINDOW_SIZE - 1);
self.window[idx] = ObsRecord { obs, tick };  // Safe bounds-checked access
self.window_head = (self.window_head + 1) as u16;
```

---

### CGK-011: Panic on System Time Before Epoch

**Severity:** Medium
**Location:** `/home/user/ruvector/crates/cognitum-gate-tilezero/src/lib.rs:173-176`
**CVSS:** 4.5

**Description:**
The time computation can panic:

```rust
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()  // Panics if system time < epoch!
    .as_nanos() as u64;
```

**Impact:**
If system time is misconfigured (before 1970), the gate panics and becomes unavailable.

**Recommended Fix:**
```rust
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or(std::time::Duration::ZERO)
    .as_nanos() as u64;
```

---

### CGK-012: Processing Rate Division by Zero

**Severity:** Medium
**Location:** `/home/user/ruvector/crates/cognitum-gate-kernel/src/report.rs:284-289`
**CVSS:** 4.0

**Description:**
```rust
pub fn processing_rate(&self) -> f32 {
    if self.tick_time_us == 0 {
        0.0  // Handled correctly
    } else {
        (self.deltas_processed as f32) / (self.tick_time_us as f32)
    }
}
```

This is actually handled correctly. However, the check should use floating point division behavior documentation.

**Status:** No action required - correctly implemented.

---

## Low Severity Issues

### CGK-013: Tick Time Truncation

**Severity:** Low
**Location:** `/home/user/ruvector/crates/cognitum-gate-kernel/src/lib.rs:257`
**CVSS:** 3.5

**Description:**
Tick time is cast from u32 to u16:

```rust
report.tick_time_us = (tick_end - tick_start) as u16;  // Truncates if > 65535
```

**Impact:**
Ticks longer than ~65ms will have incorrect timing metrics, affecting performance analysis.

**Recommended Fix:**
```rust
report.tick_time_us = (tick_end - tick_start).min(u16::MAX as u32) as u16;
```

---

### CGK-014: Silent JSON Serialization Failure

**Severity:** Low
**Location:** `/home/user/ruvector/crates/cognitum-gate-tilezero/src/receipt.rs:82-83`
**CVSS:** 3.1

**Description:**
```rust
pub fn hash(&self) -> [u8; 32] {
    let json = serde_json::to_vec(self).unwrap_or_default();  // Silent failure!
    *blake3::hash(&json).as_bytes()
}
```

**Impact:**
If serialization fails, an empty hash is computed, potentially causing hash collisions.

**Recommended Fix:**
```rust
pub fn hash(&self) -> Result<[u8; 32], HashError> {
    let json = serde_json::to_vec(self)?;
    Ok(*blake3::hash(&json).as_bytes())
}
```

---

### CGK-015: Information Disclosure in Error Messages

**Severity:** Low
**Location:** `/home/user/ruvector/crates/mcp-gate/src/tools.rs:292-355`
**CVSS:** 3.0

**Description:**
Error messages expose internal state details:

```rust
format!("Min-cut {:.3} below threshold {:.3}", mincut_value, self.thresholds.min_cut)
format!("E-value {:.4} indicates strong evidence of incoherence", summary.evidential.e_value)
```

**Impact:**
Exposes exact threshold values and internal metrics to clients, aiding targeted attacks.

**Recommended Fix:**
Return generic error codes to external clients; log detailed messages internally only.

---

### CGK-016: No Input Size Limits on Tool Calls

**Severity:** Low
**Location:** `/home/user/ruvector/crates/mcp-gate/src/tools.rs:126-159`
**CVSS:** 2.8

**Description:**
The `call_tool` function deserializes JSON without size limits:

```rust
let request: PermitActionRequest = serde_json::from_value(call.arguments)
    .map_err(|e| McpError::InvalidRequest(e.to_string()))?;
```

**Impact:**
Very large JSON payloads could cause memory exhaustion.

**Recommended Fix:**
Add a size limit check before deserialization or use `serde_json` with size limits.

---

### CGK-017: Hardcoded Escalation Timeout

**Severity:** Low
**Location:** `/home/user/ruvector/crates/mcp-gate/src/tools.rs:194`
**CVSS:** 2.5

**Description:**
```rust
timeout_ns: 300_000_000_000, // 5 minutes - hardcoded
```

**Impact:**
Cannot adjust escalation timeout without code changes; not a direct security issue but affects operational security.

**Recommended Fix:**
Make configurable via `GateThresholds` or environment variable.

---

## Recommendations Summary

### Immediate Actions (Critical/High)

1. **Fix signature verification** (CGK-001, CGK-002) - This is a complete authentication bypass
2. **Add synchronization to global state** (CGK-003, CGK-005) - Prevents data races
3. **Add alignment/null checks to raw pointer operations** (CGK-004)
4. **Add validation to delta processing** (CGK-006)

### Short-term Actions (Medium)

5. **Validate thresholds before computing reciprocals** (CGK-007)
6. **Use checked arithmetic for token TTL** (CGK-008)
7. **Bound receipt log size and optimize chain verification** (CGK-009)
8. **Reorder bit masking in evidence window** (CGK-010)
9. **Handle system time edge cases** (CGK-011)

### Long-term Actions (Low)

10. **Sanitize error messages for external clients** (CGK-015)
11. **Add input size limits** (CGK-016)
12. **Make operational parameters configurable** (CGK-017)

---

## Unsafe Code Audit Summary

| File | Unsafe Blocks | Safety Concerns |
|------|---------------|-----------------|
| kernel/lib.rs | 8 | Global state, raw pointers, union access |
| kernel/shard.rs | 14 | Unchecked array indexing (performance-critical) |
| kernel/evidence.rs | 4 | Unchecked window access |
| kernel/report.rs | 0 | None |
| tilezero/lib.rs | 0 | None |
| tilezero/permit.rs | 0 | None (but cryptographic issues) |
| tilezero/receipt.rs | 0 | None |
| tilezero/decision.rs | 0 | None |
| mcp-gate/tools.rs | 0 | None |

The kernel crate uses unsafe code extensively for performance optimization. Each instance should be audited against its safety invariants.

---

## Testing Recommendations

1. **Fuzzing:** Apply `cargo-fuzz` to delta parsing and token decoding
2. **Property testing:** Use `proptest` for invariant validation
3. **Miri:** Run `cargo miri test` to detect undefined behavior
4. **Memory sanitizers:** Test with AddressSanitizer and MemorySanitizer

---

## Compliance Notes

- **No timing attacks identified** in the cryptographic code (uses constant-time libraries)
- **Key generation** uses `OsRng` which is cryptographically secure
- **Hash function** (blake3) is modern and appropriate
- **Signature scheme** (Ed25519) is appropriate but implementation is broken

---

## Appendix A: Delta Module Analysis

**File:** `/home/user/ruvector/crates/cognitum-gate-kernel/src/delta.rs`

The delta module implements a tagged union (`DeltaPayload`) for graph updates. The design is sound but has some security considerations:

### Union Safety

The `DeltaPayload` union is correctly sized (8 bytes for all variants) with compile-time assertions. The unsafe accessor methods (`get_edge_add`, `get_edge_remove`, etc.) correctly require the caller to verify the tag before access.

**Current Implementation (Lines 379-401):**
```rust
/// Get the edge add payload (unsafe: caller must verify tag)
pub unsafe fn get_edge_add(&self) -> &EdgeAdd {
    unsafe { &self.payload.edge_add }
}
```

**Recommendation:** Consider adding debug assertions:
```rust
#[inline]
pub unsafe fn get_edge_add(&self) -> &EdgeAdd {
    debug_assert_eq!(self.tag, DeltaTag::EdgeAdd, "Invalid tag for EdgeAdd access");
    unsafe { &self.payload.edge_add }
}
```

### Alignment Considerations

The `Delta` struct is aligned to 16 bytes (`#[repr(C, align(16))]`), which is correct for WASM and most architectures. However, when deserializing from raw bytes (as in `ingest_delta_raw`), alignment must be verified.

---

## Appendix B: Threat Model Summary

| Threat | Likelihood | Impact | Mitigation Status |
|--------|------------|--------|------------------|
| Token forgery (CGK-001/002) | High | Critical | NOT MITIGATED |
| Memory corruption via malformed delta | Medium | High | Partial (tag check, no integrity check) |
| DoS via memory exhaustion | Medium | Medium | Partial (fixed buffers, but unbounded log) |
| Race condition exploitation | Low | High | NOT MITIGATED (single-threaded WASM assumed) |
| Timing side-channel | Low | Low | Mitigated (constant-time crypto libs) |

---

## Appendix C: Verification Status of Unsafe Code Invariants

| Location | Invariant | Verified By |
|----------|-----------|-------------|
| shard.rs:450 | source < MAX_SHARD_VERTICES | Bounds check at line 445 |
| shard.rs:457 | degree <= MAX_DEGREE | Struct invariant (add_edge checks) |
| shard.rs:576-577 | root < MAX_SHARD_VERTICES | Loop construction |
| evidence.rs:410 | idx < WINDOW_SIZE | **BROKEN** - mask applied after access |
| lib.rs:208 | ptr aligned to Delta alignment | **NOT VERIFIED** |
| lib.rs:292 | tag matches payload variant | Tag set during construction only |

---

*Report generated by Claude Code Security Review Agent*
*Classification: Internal Security Document*
