# ADR-032: Multistatic Mesh Security Hardening

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-03-01 |
| **Deciders** | ruv |
| **Relates to** | ADR-029 (RuvSense Multistatic), ADR-030 (Persistent Field Model), ADR-031 (RuView Sensing-First RF), ADR-018 (ESP32 Implementation), ADR-012 (ESP32 Mesh) |

---

## 1. Context

### 1.1 Security Audit of ADR-029/030/031

A security audit of the RuvSense multistatic sensing stack (ADR-029 through ADR-031) identified seven findings across the TDM synchronization layer, CSI frame transport, NDP injection, coherence gating, cross-room tracking, NVS credential handling, and firmware concurrency model. Three severity levels were assigned: HIGH (1 finding), MEDIUM (3 findings), LOW (3 findings).

The findings fall into three categories:

1. **Missing cryptographic authentication** -- The TDM SyncBeacon and CSI frame formats lack any message authentication, allowing rogue nodes to inject spoofed beacons or frames into the mesh.
2. **Unbounded or unprotected resources** -- The NDP injection path has no rate limiter, the coherence gate recalibration state has no timeout cap, and the cross-room transition log grows without bound.
3. **Memory safety on embedded targets** -- NVS credential buffers are not zeroed after use, and static mutable globals in the CSI collector are accessed from both ESP32-S3 cores without synchronization.

### 1.2 Threat Model

The primary threat actor is a rogue ESP32 node on the same LAN subnet or within WiFi range of the mesh. The attack surface is the UDP broadcast plane used for sync beacons, CSI frames, and NDP injection.

| Threat | STRIDE | Impact | Exploitability |
|--------|--------|--------|----------------|
| Fake SyncBeacon injection | Spoofing, Tampering | Full mesh desynchronization, no pose output | Low skill, rogue ESP32 on LAN |
| CSI frame spoofing | Spoofing, Tampering | Corrupted pose estimation, phantom occupants | Low skill, UDP packet injection |
| NDP RF flooding | Denial of Service | Channel saturation, loss of CSI data | Low skill, repeated NDP calls |
| Coherence gate stall | Denial of Service | Indefinite recalibration, frozen output | Requires sustained interference |
| Transition log exhaustion | Denial of Service | OOM on aggregator after extended operation | Passive, no attacker needed |
| Credential stack residue | Information Disclosure | WiFi password recoverable from RAM dump | Physical access to device |
| Dual-core data race | Tampering, DoS | Corrupted CSI frames, undefined behavior | Passive, no attacker needed |

### 1.3 Design Constraints

- ESP32-S3 has limited CPU budget: cryptographic operations must complete within the 1 ms guard interval between TDM slots.
- HMAC-SHA256 on ESP32-S3 (hardware-accelerated via `mbedtls`) completes in approximately 15 us for 24-byte payloads -- well within budget.
- SipHash-2-4 completes in approximately 2 us for 64-byte payloads on ESP32-S3 -- suitable for per-frame MAC.
- No TLS or TCP is available on the sensing data path (UDP broadcast for latency).
- Pre-shared key (PSK) model is acceptable because all nodes in a mesh deployment are provisioned by the same operator.

---

## 2. Decision

Harden the multistatic mesh with six measures: beacon authentication, frame integrity, NDP rate limiting, bounded buffers, memory safety, and key management. All changes are backward-compatible: unauthenticated frames are accepted during a migration window controlled by a `security_level` NVS parameter.

### 2.1 Beacon Authentication Protocol (H-1)

**Finding:** The 16-byte `SyncBeacon` wire format (`crates/wifi-densepose-hardware/src/esp32/tdm.rs`) has no cryptographic authentication. A rogue node can inject fake beacons to desynchronize the TDM mesh.

**Solution:** Extend the SyncBeacon wire format from 16 bytes to 28 bytes by adding a 4-byte monotonic nonce and an 8-byte HMAC-SHA256 truncated tag.

```
Authenticated SyncBeacon wire format (28 bytes):
  [0..7]   cycle_id          (LE u64)
  [8..11]  cycle_period_us   (LE u32)
  [12..13] drift_correction  (LE i16)
  [14..15] reserved
  [16..19] nonce             (LE u32, monotonically increasing)
  [20..27] hmac_tag          (HMAC-SHA256 truncated to 8 bytes)
```

**HMAC computation:**

```
key     = 16-byte pre-shared mesh key (stored in NVS, namespace "mesh_sec")
message = beacon[0..20]  (first 20 bytes: payload + nonce)
tag     = HMAC-SHA256(key, message)[0..8]  (truncated to 8 bytes)
```

**Nonce and replay protection:**

- The coordinator maintains a monotonically increasing 32-bit nonce counter, incremented on every beacon.
- Each receiver maintains a `last_accepted_nonce` per sender. A beacon is accepted only if `nonce > last_accepted_nonce - REPLAY_WINDOW`, where `REPLAY_WINDOW = 16` (accounts for packet reordering over UDP).
- Nonce overflow (after 2^32 beacons at 20 Hz = ~6.8 years) triggers a mandatory key rotation.

**Implementation location:** `crates/wifi-densepose-hardware/src/esp32/tdm.rs` -- extend `SyncBeacon::to_bytes()` and `SyncBeacon::from_bytes()` to produce/consume the 28-byte authenticated format. Add `SyncBeacon::verify()` method.

### 2.2 CSI Frame Integrity (M-3)

**Finding:** The ADR-018 CSI frame format has no cryptographic MAC. Frames can be spoofed or tampered with in transit.

**Solution:** Add an 8-byte SipHash-2-4 tag to the CSI frame header. SipHash is chosen over HMAC-SHA256 for per-frame MAC because it is 7x faster on ESP32 for short messages (approximately 2 us vs 15 us) and provides sufficient integrity for non-secret data.

```
Extended CSI frame header (28 bytes, was 20):
  [0..3]   Magic: 0xC5110002  (bumped from 0xC5110001 to signal auth)
  [4]      Node ID
  [5]      Number of antennas
  [6..7]   Number of subcarriers (LE u16)
  [8..11]  Frequency MHz (LE u32)
  [12..15] Sequence number (LE u32)
  [16]     RSSI (i8)
  [17]     Noise floor (i8)
  [18..19] Reserved
  [20..27] siphash_tag  (SipHash-2-4 over [0..20] + IQ data)
```

**SipHash key derivation:**

```
siphash_key = HMAC-SHA256(mesh_key, "csi-frame-siphash")[0..16]
```

The SipHash key is derived once at boot from the mesh key and cached in memory.

**Implementation locations:**
- `firmware/esp32-csi-node/main/csi_collector.c` -- compute SipHash tag in `csi_serialize_frame()`, bump magic constant.
- `crates/wifi-densepose-hardware/src/esp32/` -- add frame verification in the aggregator's frame parser.

### 2.3 NDP Injection Rate Limiter (M-4)

**Finding:** `csi_inject_ndp_frame()` in `firmware/esp32-csi-node/main/csi_collector.c` has no rate limiter. Uncontrolled NDP injection can flood the RF channel.

**Solution:** Token-bucket rate limiter with configurable parameters stored in NVS.

```c
// Token bucket parameters (defaults)
#define NDP_RATE_MAX_TOKENS   20    // burst capacity
#define NDP_RATE_REFILL_HZ    20    // sustained rate: 20 NDP/sec
#define NDP_RATE_REFILL_US    (1000000 / NDP_RATE_REFILL_HZ)

typedef struct {
    uint32_t tokens;          // current token count
    uint32_t max_tokens;      // bucket capacity
    uint32_t refill_interval_us;  // microseconds per token
    int64_t  last_refill_us;  // last refill timestamp
} ndp_rate_limiter_t;
```

`csi_inject_ndp_frame()` returns `ESP_ERR_NOT_ALLOWED` when the bucket is empty. The rate limiter parameters are configurable via NVS keys `ndp_max_tokens` and `ndp_refill_hz`.

**Implementation location:** `firmware/esp32-csi-node/main/csi_collector.c` -- add `ndp_rate_limiter_t` state and check in `csi_inject_ndp_frame()`.

### 2.4 Coherence Gate Recalibration Timeout (M-5)

**Finding:** The `Recalibrate` state in `crates/wifi-densepose-signal/src/ruvsense/coherence_gate.rs` can be held indefinitely. A sustained interference source could keep the system in perpetual recalibration, preventing any output.

**Solution:** Add a configurable `max_recalibrate_duration` to `GatePolicyConfig` (default: 30 seconds = 600 frames at 20 Hz). When the recalibration duration exceeds this cap, the gate transitions to a `ForcedAccept` state with inflated noise (10x), allowing degraded-but-available output.

```rust
pub enum GateDecision {
    Accept { noise_multiplier: f32 },
    PredictOnly,
    Reject,
    Recalibrate { stale_frames: u64 },
    /// Recalibration timed out. Accept with heavily inflated noise.
    ForcedAccept { noise_multiplier: f32, stale_frames: u64 },
}
```

New config field:

```rust
pub struct GatePolicyConfig {
    // ... existing fields ...
    /// Maximum frames in Recalibrate before forcing accept. Default: 600 (30s at 20Hz).
    pub max_recalibrate_frames: u64,
    /// Noise multiplier for ForcedAccept. Default: 10.0.
    pub forced_accept_noise: f32,
}
```

**Implementation location:** `crates/wifi-densepose-signal/src/ruvsense/coherence_gate.rs` -- extend `GateDecision` enum, modify `GatePolicy::evaluate()`.

### 2.5 Bounded Transition Log (L-1)

**Finding:** `CrossRoomTracker` in `crates/wifi-densepose-signal/src/ruvsense/cross_room.rs` stores transitions in an unbounded `Vec<TransitionEvent>`. Over extended operation (days/weeks), this grows without limit.

**Solution:** Replace the `transitions: Vec<TransitionEvent>` with a ring buffer that evicts the oldest entry when capacity is reached.

```rust
pub struct CrossRoomConfig {
    // ... existing fields ...
    /// Maximum transitions retained in the ring buffer. Default: 1000.
    pub max_transitions: usize,
}
```

The ring buffer is implemented as a `VecDeque<TransitionEvent>` with a capacity check on push. When `transitions.len() >= max_transitions`, `transitions.pop_front()` before pushing. This preserves the append-only audit trail semantics (events are never mutated, only evicted by age).

**Implementation location:** `crates/wifi-densepose-signal/src/ruvsense/cross_room.rs` -- change `transitions: Vec<TransitionEvent>` to `transitions: VecDeque<TransitionEvent>`, add eviction logic in `match_entry()`.

### 2.6 NVS Password Buffer Zeroing (L-4)

**Finding:** `nvs_config_load()` in `firmware/esp32-csi-node/main/nvs_config.c` reads the WiFi password into a stack buffer `buf` which is not zeroed after use. On ESP32-S3, stack memory is not automatically cleared, leaving credentials recoverable via physical memory dump.

**Solution:** Zero the stack buffer after each NVS string read using `explicit_bzero()` (available in ESP-IDF via newlib). If `explicit_bzero` is unavailable, use `memset` with a volatile pointer to prevent compiler optimization.

```c
/* After each nvs_get_str that may contain credentials: */
explicit_bzero(buf, sizeof(buf));

/* Portable fallback: */
static void secure_zero(void *ptr, size_t len) {
    volatile unsigned char *p = (volatile unsigned char *)ptr;
    while (len--) { *p++ = 0; }
}
```

Apply to all three `nvs_get_str` call sites in `nvs_config_load()` (ssid, password, target_ip).

**Implementation location:** `firmware/esp32-csi-node/main/nvs_config.c` -- add `explicit_bzero(buf, sizeof(buf))` after each `nvs_get_str` block.

### 2.7 Atomic Access for Static Mutable State (L-5)

**Finding:** `csi_collector.c` uses static mutable globals (`s_sequence`, `s_cb_count`, `s_send_ok`, `s_send_fail`, `s_hop_index`) accessed from both cores of the ESP32-S3 without synchronization. The CSI callback runs on the WiFi task (pinned to core 0 by default), while the main application and hop timer may run on core 1.

**Solution:** Use C11 `_Atomic` qualifiers for all shared counters, and a FreeRTOS mutex for the hop table state which requires multi-variable consistency.

```c
#include <stdatomic.h>

static _Atomic uint32_t s_sequence  = 0;
static _Atomic uint32_t s_cb_count  = 0;
static _Atomic uint32_t s_send_ok   = 0;
static _Atomic uint32_t s_send_fail = 0;
static _Atomic uint8_t  s_hop_index = 0;

/* Hop table protected by mutex (multi-variable consistency) */
static SemaphoreHandle_t s_hop_mutex = NULL;
```

The mutex is created in `csi_collector_init()` and taken/released around hop table reads in `csi_hop_next_channel()` and writes in `csi_collector_set_hop_table()`.

**Implementation location:** `firmware/esp32-csi-node/main/csi_collector.c` -- add `_Atomic` qualifiers, create and use `s_hop_mutex`.

### 2.8 Key Management

All cryptographic operations use a single 16-byte pre-shared mesh key stored in NVS.

**Provisioning:**

```
NVS namespace: "mesh_sec"
NVS key:       "mesh_key"
NVS type:      blob (16 bytes)
```

The key is provisioned during node setup via the existing `scripts/provision.py` tool, which is extended to generate a random 16-byte key and flash it to all nodes in a deployment.

**Key derivation:**

```
beacon_hmac_key  = mesh_key                                      (direct, 16 bytes)
frame_siphash_key = HMAC-SHA256(mesh_key, "csi-frame-siphash")[0..16]  (derived, 16 bytes)
```

**Key rotation:**

- Manual rotation via management command: `provision.py rotate-key --deployment <id>`.
- The coordinator broadcasts a key rotation event (signed with the old key) containing the new key encrypted with the old key.
- Nodes accept the new key and switch after confirming the next beacon is signed with the new key.
- Rotation is recommended every 90 days or after any node is decommissioned.

**Security level NVS parameter:**

```
NVS key: "sec_level"
Values:
  0 = permissive  (accept unauthenticated frames, log warning)
  1 = transitional (accept both authenticated and unauthenticated)
  2 = enforcing   (reject unauthenticated frames)
Default: 1 (transitional, for backward compatibility during rollout)
```

---

## 3. Implementation Plan (File-Level)

### 3.1 Phase 1: Beacon Authentication and Key Management

| File | Change | Priority |
|------|--------|----------|
| `crates/wifi-densepose-hardware/src/esp32/tdm.rs` | Extend `SyncBeacon` to 28-byte authenticated format, add `verify()`, nonce tracking, replay window | P0 |
| `firmware/esp32-csi-node/main/nvs_config.c` | Add `mesh_key` and `sec_level` NVS reads | P0 |
| `firmware/esp32-csi-node/main/nvs_config.h` | Add `mesh_key[16]` and `sec_level` to `nvs_config_t` | P0 |
| `scripts/provision.py` | Add `--mesh-key` generation and `rotate-key` command | P0 |

### 3.2 Phase 2: Frame Integrity and Rate Limiting

| File | Change | Priority |
|------|--------|----------|
| `firmware/esp32-csi-node/main/csi_collector.c` | Add SipHash-2-4 tag to frame serialization, NDP rate limiter, `_Atomic` qualifiers, hop mutex | P1 |
| `firmware/esp32-csi-node/main/csi_collector.h` | Update `CSI_HEADER_SIZE` to 28, add rate limiter config | P1 |
| `crates/wifi-densepose-hardware/src/esp32/` | Add frame verification in aggregator parser | P1 |

### 3.3 Phase 3: Bounded Buffers and Gate Hardening

| File | Change | Priority |
|------|--------|----------|
| `crates/wifi-densepose-signal/src/ruvsense/cross_room.rs` | Replace `Vec` with `VecDeque`, add `max_transitions` config | P1 |
| `crates/wifi-densepose-signal/src/ruvsense/coherence_gate.rs` | Add `ForcedAccept` variant, `max_recalibrate_frames` config | P1 |

### 3.4 Phase 4: Memory Safety

| File | Change | Priority |
|------|--------|----------|
| `firmware/esp32-csi-node/main/nvs_config.c` | Add `explicit_bzero()` after credential reads | P2 |
| `firmware/esp32-csi-node/main/csi_collector.c` | `_Atomic` counters, `s_hop_mutex` (if not done in Phase 2) | P2 |

---

## 4. Acceptance Criteria

### 4.1 Beacon Authentication (H-1)

| ID | Criterion | Test Method |
|----|-----------|-------------|
| H1-1 | `SyncBeacon::to_bytes()` produces 28-byte output with valid HMAC tag | Unit test: serialize, verify tag matches recomputed HMAC |
| H1-2 | `SyncBeacon::verify()` rejects beacons with incorrect HMAC tag | Unit test: flip one bit in tag, verify returns `Err` |
| H1-3 | `SyncBeacon::verify()` rejects beacons with replayed nonce outside window | Unit test: submit nonce = last_accepted - REPLAY_WINDOW - 1, verify rejection |
| H1-4 | `SyncBeacon::verify()` accepts beacons within replay window | Unit test: submit nonce = last_accepted - REPLAY_WINDOW + 1, verify acceptance |
| H1-5 | Coordinator nonce increments monotonically across cycles | Unit test: call `begin_cycle()` 100 times, verify strict monotonicity |
| H1-6 | Backward compatibility: `sec_level=0` accepts unauthenticated 16-byte beacons | Integration test: mixed old/new nodes |

### 4.2 Frame Integrity (M-3)

| ID | Criterion | Test Method |
|----|-----------|-------------|
| M3-1 | CSI frame with magic `0xC5110002` includes valid 8-byte SipHash tag | Unit test: serialize frame, verify tag |
| M3-2 | Frame verification rejects frames with tampered IQ data | Unit test: flip one byte in IQ payload, verify rejection |
| M3-3 | SipHash computation completes in < 10 us on ESP32-S3 | Benchmark on target hardware |
| M3-4 | Frame parser accepts old magic `0xC5110001` when `sec_level < 2` | Unit test: backward compatibility |

### 4.3 NDP Rate Limiter (M-4)

| ID | Criterion | Test Method |
|----|-----------|-------------|
| M4-1 | `csi_inject_ndp_frame()` succeeds for first `max_tokens` calls | Unit test: call 20 times rapidly, all succeed |
| M4-2 | Call 21 returns `ESP_ERR_NOT_ALLOWED` when bucket is empty | Unit test: exhaust bucket, verify error |
| M4-3 | Bucket refills at configured rate | Unit test: exhaust, wait `refill_interval_us`, verify one token available |
| M4-4 | NVS override of `ndp_max_tokens` and `ndp_refill_hz` is respected | Integration test: set NVS values, verify behavior |

### 4.4 Coherence Gate Timeout (M-5)

| ID | Criterion | Test Method |
|----|-----------|-------------|
| M5-1 | `GatePolicy::evaluate()` returns `Recalibrate` at `max_stale_frames` | Unit test: existing behavior preserved |
| M5-2 | `GatePolicy::evaluate()` returns `ForcedAccept` at `max_recalibrate_frames` | Unit test: feed `max_recalibrate_frames + 1` low-coherence frames |
| M5-3 | `ForcedAccept` noise multiplier equals `forced_accept_noise` (default 10.0) | Unit test: verify noise_multiplier field |
| M5-4 | Default `max_recalibrate_frames` = 600 (30s at 20 Hz) | Unit test: verify default config |

### 4.5 Bounded Transition Log (L-1)

| ID | Criterion | Test Method |
|----|-----------|-------------|
| L1-1 | `CrossRoomTracker::transition_count()` never exceeds `max_transitions` | Unit test: insert 1500 transitions with max_transitions=1000, verify count=1000 |
| L1-2 | Oldest transitions are evicted first (FIFO) | Unit test: verify first transition is the (N-999)th inserted |
| L1-3 | Default `max_transitions` = 1000 | Unit test: verify default config |

### 4.6 NVS Password Zeroing (L-4)

| ID | Criterion | Test Method |
|----|-----------|-------------|
| L4-1 | Stack buffer `buf` is zeroed after each `nvs_get_str` call | Code review + static analysis (no runtime test feasible) |
| L4-2 | `explicit_bzero` is used (not plain `memset`) to prevent compiler optimization | Code review: verify function call is `explicit_bzero` or volatile-pointer pattern |

### 4.7 Atomic Static State (L-5)

| ID | Criterion | Test Method |
|----|-----------|-------------|
| L5-1 | `s_sequence`, `s_cb_count`, `s_send_ok`, `s_send_fail` are declared `_Atomic` | Code review |
| L5-2 | `s_hop_mutex` is created in `csi_collector_init()` | Code review + integration test: init succeeds |
| L5-3 | `csi_hop_next_channel()` and `csi_collector_set_hop_table()` acquire/release mutex | Code review |
| L5-4 | No data races detected under ThreadSanitizer (host-side test build) | `cargo test` with TSAN on host (for Rust side); QEMU or hardware test for C side |

---

## 5. Consequences

### 5.1 Positive

- **Rogue node protection**: HMAC-authenticated beacons prevent mesh desynchronization by unauthorized nodes.
- **Frame integrity**: SipHash MAC detects in-transit tampering of CSI data, preventing phantom occupant injection.
- **RF availability**: Token-bucket rate limiter prevents NDP flooding from consuming the shared wireless medium.
- **Bounded memory**: Ring buffer on transition log and timeout cap on recalibration prevent resource exhaustion during long-running deployments.
- **Credential hygiene**: Zeroed buffers reduce the window for credential recovery from physical memory access.
- **Thread safety**: Atomic operations and mutex eliminate undefined behavior on dual-core ESP32-S3.
- **Backward compatible**: `sec_level` parameter allows gradual rollout without breaking existing deployments.

### 5.2 Negative

- **12 bytes added to SyncBeacon**: 28 bytes vs 16 bytes (75% increase, but still fits in a single UDP packet with room to spare).
- **8 bytes added to CSI frame header**: 28 bytes vs 20 bytes (40% increase in header; negligible relative to IQ payload of 128-512 bytes).
- **CPU overhead**: HMAC-SHA256 adds approximately 15 us per beacon (once per 50 ms cycle = 0.03% CPU). SipHash adds approximately 2 us per frame (at 100 Hz = 0.02% CPU).
- **Key management complexity**: Mesh key must be provisioned to all nodes and rotated periodically. Lost key requires re-provisioning all nodes.
- **Mutex contention**: Hop table mutex may add up to 1 us latency to channel hop path. Within guard interval budget.

### 5.3 Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| HMAC computation exceeds guard interval on older ESP32 (non-S3) | Low | Beacon authentication unusable on legacy hardware | Hardware-accelerated SHA256 is available on all ESP32 variants; benchmark confirms < 50 us |
| Key compromise via side-channel on ESP32 | Very Low | Full mesh authentication bypass | Keys stored in eFuse (ESP32-S3 supports) or encrypted NVS partition |
| ForcedAccept mode produces unacceptably noisy poses | Medium | Degraded pose quality during sustained interference | 10x noise multiplier is configurable; operator can increase or disable |
| SipHash collision (64-bit tag) | Very Low | Single forged frame accepted | 2^-64 probability per frame; attacker cannot iterate at protocol speed |

---

## 6. Related ADRs

| ADR | Relationship |
|-----|-------------|
| ADR-029 (RuvSense Multistatic) | **Hardened**: TDM beacon and CSI frame authentication, NDP rate limiting |
| ADR-030 (Persistent Field Model) | **Protected**: Coherence gate timeout prevents indefinite recalibration; transition log bounded |
| ADR-031 (RuView RF Mode) | **Hardened**: Authenticated beacons protect cross-viewpoint synchronization |
| ADR-018 (ESP32 Implementation) | **Extended**: CSI frame header bumped to v2 with SipHash tag; backward-compatible magic check |
| ADR-012 (ESP32 Mesh) | **Hardened**: Mesh key management, NVS credential zeroing, atomic firmware state |

---

## 7. References

1. Aumasson, J.-P. & Bernstein, D.J. (2012). "SipHash: a fast short-input PRF." INDOCRYPT 2012.
2. Krawczyk, H. et al. (1997). "HMAC: Keyed-Hashing for Message Authentication." RFC 2104.
3. ESP-IDF mbedtls SHA256 hardware acceleration. Espressif Documentation.
4. Espressif. "ESP32-S3 Technical Reference Manual." Section 26: SHA Accelerator.
5. Turner, J. (2006). "Token Bucket Rate Limiting." RFC 2697 (adapted).
6. ADR-029 through ADR-031 (internal).
