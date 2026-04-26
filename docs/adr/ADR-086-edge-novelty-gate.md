# ADR-086: Edge Novelty Gate — Push the RaBitQ Sensor Down to the Sensor MCU

| Field          | Value                                                                                                                                        |
|----------------|----------------------------------------------------------------------------------------------------------------------------------------------|
| **Status**     | Proposed                                                                                                                                     |
| **Date**       | 2026-04-26                                                                                                                                   |
| **Authors**    | ruv                                                                                                                                          |
| **Refines**    | ADR-081 (5-layer adaptive CSI mesh firmware kernel — Layer 4 / On-device feature extraction), ADR-084 (RaBitQ similarity sensor) |
| **Touches**    | ADR-018 (binary CSI frame magic discipline), ADR-028 (capability audit / witness verification), ADR-082 (confirmed-track output filter), ADR-085 (RaBitQ pipeline expansion) |
| **Companion**  | `firmware/esp32-csi-node/main/rv_feature_state.h` (current `0xC5110006` v6 wire format), `docs/research/architecture/three-tier-rust-node.md` (BQ24074 power budget context), `vendor/ruvector/crates/ruvector-core/src/quantization.rs::BinaryQuantized` (std reference implementation that this ADR will not directly reuse on-MCU) |

## Context

ADR-081's 5-layer firmware kernel today emits one `rv_feature_state_t`
packet per node every 100–1000 ms (1–10 Hz, default 5 Hz on COM7),
60 bytes payload, magic `0xC5110006`, regardless of how interesting
the underlying CSI window was. At a 5 Hz baseline the per-node steady-
state load is ~300 B/s of UDP plus the radio TX duty that emits it.
Across a 12-node deployment the cluster Pi sees ~3.6 kB/s of
feature-state — not a bandwidth crisis on its own, but every one of
those packets also costs sensor-MCU radio TX energy, every one
contends for ESP-WIFI-MESH airtime per ADR-081 Layer 3, and every one
runs through the cluster-Pi novelty bank ADR-084 Pass 3 only to be
classified as "nothing new" most of the time in a quiet room.

ADR-084 made novelty cheap on the cluster-Pi side. The same novelty
sensor is structurally local: a sketch, a small ring of recent
sketches, and a hamming-distance compare. Pushing that gate down into
the sensor MCU's Layer 4 (On-device feature extraction) lets the node
*not transmit* a frame the cluster-Pi would have filed under
"familiar" anyway. Bandwidth, sensor-MCU TX energy, and RF airtime
all win, and the cluster-Pi novelty path stops re-doing work the edge
already proved pointless. This is the natural ADR-085 follow-up
flagged but deliberately left out of the ADR-085 scope because it
requires a `no_std` sketch port, a Kconfig-gated rollout, a wire-
format bump, and a fresh witness regeneration — none of which are
appropriate inside an in-flight cluster-Pi work loop.

The crux of the decision is whether the cost of (a) hand-porting the
sketch primitive to `no_std` Xtensa LX7, (b) sizing the in-IRAM ring
without disturbing the existing Layer 4 budget, (c) bumping the
`rv_feature_state_t` magic and teaching the cluster-Pi a graceful
v6/v7 fallback, and (d) re-cutting the ADR-028 witness bundle is
justified by the suppression rate the gate actually achieves on real
deployments. The answer should be obvious in stable rooms (≥50 %
suppression looks easy) and ambiguous in active rooms (suppression
should drop sharply, which is exactly what we want). This ADR commits
to numbers up front so the decision is falsifiable.

## Decision

Adopt an **edge novelty gate** in the sensor MCU's Layer 4 of
ADR-081's 5-layer kernel. The gate sits between feature extraction
and the existing UDP send path; when novelty is below a configurable
threshold the frame is **not transmitted**, and the node accumulates
a per-source `suppressed_since_last` counter that is folded into the
next non-suppressed packet. This keeps the cluster-Pi's books
honest — the edge can suppress *bandwidth*, but it can never
silently suppress the *fact of suppression*.

### Components

The implementation is two pieces, both new in
`firmware/esp32-csi-node/main/`:

1. **`rv_sketch.{h,c}`** — a `no_std`-equivalent (plain C, ESP-IDF)
   1-bit sketch primitive. Sign-quantize a feature vector, pack into
   bytes (`(dim + 7) / 8` bytes), hamming distance via 8-bit
   table-lookup popcount. Xtensa LX7 has no hardware POPCNT
   instruction (no primary source consulted; conjecture based on the
   ESP32-S3 TRM not advertising one — to be confirmed by checking
   the [TRM](https://www.espressif.com/sites/default/files/documentation/esp32-s3_technical_reference_manual_en.pdf)
   under bit-manipulation extensions); the table-lookup scalar
   baseline is the right starting point and is already what
   `BinaryQuantized` falls back to on architectures without a SIMD
   POPCNT path (`vendor/ruvector/crates/ruvector-core/src/quantization.rs`,
   lines 332–340).
2. **An IRAM-resident sketch ring.** Fixed size at compile time:
   `RV_EDGE_BANK_SIZE` slots × `RV_EDGE_VECTOR_DIM_BYTES` bytes.
   For the default Layer 4 feature dimension of 56 (matching the
   subcarrier-selection / interpolation target widely used in this
   codebase), the ring at the default 32 slots costs
   `32 × 7 = 224 bytes`. A 64-slot ring at 56 d costs 448 bytes — both
   sit comfortably inside the existing static-memory budget on either
   the 4 MB or 8 MB Waveshare AMOLED ESP32-S3 board, well clear of
   ADR-081 Layer 4's existing window buffers. Eviction is FIFO; on
   each new sketch the oldest is overwritten.

### Gating policy

For each completed Layer 4 feature window:

```text
1. compute feature vector (existing)
2. sketch = sign_quantize(feature_vector)        // new
3. nearest_hamming = ring_min_distance(sketch)   // new
4. novelty = nearest_hamming / dim               // 0..1, new
5. if novelty >= CONFIG_RV_EDGE_NOVELTY_THRESHOLD
       OR suppressed_since_last >= CONFIG_RV_EDGE_MAX_CONSEC_SUPPRESS
       OR CONFIG_RV_EDGE_FORCE_SEND:
           ring_insert(sketch)
           emit rv_feature_state_t v7 with suppressed_since_last
           suppressed_since_last = 0
   else:
           suppressed_since_last += 1
           // do not insert into ring — only confirmed-emitted sketches anchor the bank
```

Threshold default: `CONFIG_RV_EDGE_NOVELTY_THRESHOLD = 500`
basis-points (= 5.0 % of dimension). Kconfig does not accept floats
without contortion (the standard Espressif practice in our codebase
is to express thresholds as `int` basis-points or scaled fixed-point);
this preserves the Kconfig-as-truth discipline ADR-081 already
follows.

Suppression cap default:
`CONFIG_RV_EDGE_MAX_CONSEC_SUPPRESS = 50`. At 5 Hz that is 10 s of
forced silence at most before a "stuck gate" self-heals into a
forced send — comparable to ADR-081's slow-loop 30 s recalibration
cadence and well below any user-visible UI staleness threshold.

Default-off gate: `CONFIG_RV_EDGE_NOVELTY_GATE_ENABLE = n`. Existing
deployments behave identically until they opt in.

### Wire format — v7

Bump the `rv_feature_state_t` magic to `0xC5110007` and add three
bytes by reusing the existing 2-byte `reserved` field plus one byte
borrowed from the 16-bit `quality_flags` budget (only 8 of 16 flags
are defined today; we narrow to `uint8_t quality_flags`):

| Offset (v7) | Field                       | Notes                                |
|-------------|-----------------------------|--------------------------------------|
| 0..3        | `magic = 0xC5110007`        | new; differentiates from `0xC5110006` |
| 4           | `node_id`                   | unchanged                             |
| 5           | `mode`                      | unchanged                             |
| 6..7        | `seq`                       | unchanged                             |
| 8..15       | `ts_us`                     | unchanged                             |
| 16..51      | nine `float` features       | unchanged                             |
| 52          | `quality_flags` (`uint8_t`) | narrowed from u16 — see Open Q3       |
| 53          | `gate_version` (`uint8_t`)  | new                                   |
| 54..55      | `suppressed_since_last`     | new (`uint16_t` LE)                   |
| 56..59      | `crc32`                     | unchanged, computed over [0..56)      |

Total size: still 60 bytes, **wire-compatible at packet length but
not at field semantics** — magic is the discriminator. Cluster-Pi
receivers that recognize `0xC5110007` interpret the new fields;
receivers that recognize `0xC5110006` continue to work but do not
see the suppression count. The receiver gracefully falls back when
it sees the v6 magic; this is the explicit graceful-fallback contract
ADR-081 already established for Layer 5 stream parsing.

The choice to narrow `quality_flags` from 16 to 8 bits relies on the
fact that `rv_feature_state.h` defines exactly 8 `RV_QFLAG_*` bits
today (lines 33–40); future flag growth is a separate ADR slot, and
the alternative — adding a 4th `uint8_t` and growing the packet to
64 bytes — costs a recompute of every Layer 5 parser and is more
intrusive than the magic bump.

## Consequences

### Positive

- **Sensor-MCU UDP TX duty cycle drops by the suppression rate.** A
  back-of-envelope at 5 Hz: at 50 % suppression, ~150 B/s and
  ~2.5 packets/s per node instead of ~300 B/s and 5; at 90 %
  suppression, ~30 B/s and 0.5 packets/s. ESP32-S3 TX energy at
  +20 dBm is the dominant per-packet cost on the BQ24074-class node
  (`docs/research/architecture/three-tier-rust-node.md` §3.3 power
  budget shows ~80 mA active-CSI baseline with TX-burst spikes at
  ~150 mA peak; the gate primarily cuts the burst-frequency rather
  than the baseline). ≥30 % TX-energy reduction in steady-state quiet
  rooms is the validation target.
- **Cluster-Pi novelty path runs on a smaller stream.** ADR-084
  Pass 3 is unchanged in code, but the input rate it processes drops
  by the suppression rate. The Pi-side bank stops accumulating
  redundant "stable" anchors and concentrates its bank slots on
  actually-different frames. This is a quality win, not just a cost
  win.
- **Mesh airtime contention drops, which improves ADR-081 Layer 3
  for everyone else.** Less feature-state traffic frees airtime for
  TIME_SYNC, ROLE_ASSIGN, FEATURE_DELTA, HEALTH, and ANOMALY_ALERT
  — the high-priority mesh-control traffic that today competes with
  routine feature-state in the same channel.
- **`suppressed_since_last` is observable.** The cluster-Pi can
  detect a node that has been suppressing for too long, a node
  whose suppression rate suddenly drops (occupant entered the
  room — the right behaviour), and a node whose suppression cap is
  triggering frequently (gate is mistuned). All three are useful
  signals and all three live in fields the receiver already parses.

### Negative / risks

- **The cluster-Pi-side novelty sensor sees fewer data points.** This
  is the load-bearing negative consequence and the most likely
  source of regression. ADR-084 Pass 3's bank ages out anchors based
  on insertion time; if the edge gate suppresses 70 % of frames in
  a quiet room, the Pi bank receives 30 % of its expected anchor
  rate and may take 3× longer to converge to a useful steady state
  on a freshly-rebooted Pi. Mitigation: the validation acceptance
  test runs the Pi-side novelty top-K coverage against an
  unsuppressed baseline and budgets ≤5 percentage points regression.
  If the cluster-Pi cold-start convergence becomes a real problem
  the simplest patch is to force-send the first
  `CONFIG_RV_EDGE_FORCE_SEND_BURST` (default 32) frames per
  Layer 2 slow-loop recalibration window — but this lives outside
  the ADR-086 baseline and is called out as a follow-up if needed.
- **Witness chain.** Per ADR-028, every change to firmware
  invalidates the witness bundle. Edge novelty gate is a non-trivial
  firmware change: it touches Layer 4, adds a wire-format magic,
  and ships a Kconfig surface. The witness bundle must be re-cut
  and the SHA-256 of the proof bundle is **expected** to change
  (which is the whole point of the witness — the change must be
  visible). The post-change validation step is to run
  `bash scripts/generate-witness-bundle.sh` and confirm 7/7 PASS
  via `dist/witness-bundle-ADR028-*/VERIFY.sh`.
- **Two wire-format magics in the field at once.** During rollout
  some nodes emit v6 and some v7. The cluster-Pi receiver must
  handle both, and the WebSocket "latest snapshot" path must not
  accidentally null-out the new fields when re-encoding for v6
  consumers. The graceful-fallback contract is small (~30 LOC on
  the Pi), but it is a contract and breaking it loses observability
  for the v7 nodes. Validation includes a mixed-version soak.
- **Pose-tracker interaction (Open Q4).** ADR-082 added a confirmed-
  track output filter that already drops single-frame phantom poses
  before they reach the WebSocket. The edge gate could *suppress
  the very frames* that would have promoted a pose track from
  Tentative to Active — i.e., a person walks through a quiet room
  and the first 1–2 frames look "low novelty" because the gate
  hasn't seen them yet, then the gate suddenly fires and emits the
  third frame. ADR-082's three-frame minimum could miss a real pose.
  Mitigation candidates: (a) lower the threshold during ADR-082
  Tentative-state minutes; (b) treat motion_score above a fixed
  floor as a force-send signal regardless of sketch novelty;
  (c) accept the regression as part of the "novelty is precisely
  what we wanted to gate on" framing. Decision deferred — Open Q4.
- **Operator debuggability.** A development-time
  `CONFIG_RV_EDGE_FORCE_SEND` Kconfig flag bypasses the gate
  entirely and is the right tool for diffing
  with-gate vs without-gate behaviour during a deployment. Required.

### Neutral

- ADR-018's binary CSI frame stream is unchanged; the gate operates
  on Layer 4 feature state, not on the debug raw-CSI path.
- ADR-085's seven cluster-Pi-side sketch sites that consume
  `rv_feature_state_t` see *fewer* inputs but the same shape;
  Sites 6 (swarm routing) and 7 (event-stream anomaly) will be
  slightly less sensitive under v7. Re-measurement is recommended
  but is not a blocker for ADR-086.

## Implementation

Six numbered passes, ordered cheapest-first / lowest-risk-first.
Each is independently shippable, each has a one-line acceptance
criterion that must pass before the next pass starts. Default-off
Kconfig means none of these passes can break a deployment that has
not opted in.

| # | Pass | Target | Acceptance |
|---|------|--------|------------|
| 1 | **`no_std` sketch primitive port** (`firmware/esp32-csi-node/main/rv_sketch.{h,c}`) | sensor-MCU C | QEMU unit test: 56-d sign-quantize of a fixed seed produces the bit-pattern matching the host-side reference; hamming distance round-trips. |
| 2 | **IRAM ring + insert/min-distance API** | sensor-MCU C | On-target benchmark on COM7: insert + ring-min on 32 slots ≤ 200 µs at 240 MHz. |
| 3 | **Kconfig flags** (`CONFIG_RV_EDGE_NOVELTY_GATE_ENABLE`, `_THRESHOLD`, `_MAX_CONSEC_SUPPRESS`, `_FORCE_SEND`) | `firmware/esp32-csi-node/main/Kconfig.projbuild` | Build with each flag toggled produces the expected `sdkconfig.defaults` merge; unit test asserts threshold of 500 bps maps to 5.0 % decision boundary. |
| 4 | **`rv_feature_state_t` v7 wire format + finalize() update** | `firmware/esp32-csi-node/main/rv_feature_state.{h,c}` | `_Static_assert(sizeof == 60)` still holds; CRC32 over the new layout round-trips; v6 receiver test reads a v7 packet without panic and ignores the new fields. |
| 5 | **Cluster-Pi reconciliation** | `crates/wifi-densepose-sensing-server/` UDP intake + ADR-084 Pass 3 novelty bank | A v7 packet with `suppressed_since_last = N` causes the Pi-side bank to interpret the gap as low-novelty stable-baseline contribution rather than as missing data; integration test on a synthetic v7 stream. |
| 6 | **QEMU + COM7 hardware-in-loop validation** | end-to-end | Stable-room recording: ≥50 % suppression rate; cluster-Pi novelty top-K coverage regression ≤ 5 pp vs unsuppressed baseline; stuck-gate self-heal exercised in a unit test. |

Pass 1 deliberately does not depend on
`vendor/ruvector/crates/ruvector-core::BinaryQuantized`. That crate
is `std`-bound (`Vec<u8>`, `is_x86_feature_detected!`, NEON
intrinsics — `quantization.rs` lines 289–340) and porting it to
`no_std` Xtensa LX7 is not a one-line `#![no_std]` flip. The clean
path is a fresh minimal C primitive that matches the
`BinaryQuantized` *behaviour* (sign quantization, byte-table popcount
fallback, `(dim+7)/8` packed bytes); the host-side reference becomes
a **spec**, not a dependency. A future `no_std`-clean Rust port may
unify both once `esp-radio` / `esp-csi-rs` matures (three-tier node
research §7.3) — out of scope here.

## Validation

This ADR is **Proposed**. Acceptance requires every numbered Pass to
meet its acceptance criterion *and* the following system-level
numbers to hold on the COM7 hardware-in-loop run:

- **Computation budget**: sketch insert + ring-min ≤ 200 µs;
  total per-frame Layer 4 overhead (existing feature extraction +
  new gate) ≤ 500 µs at 240 MHz Xtensa LX7.
- **Energy**: ≥ 30 % UDP TX-energy reduction in stable-room
  scenarios, measured by packets-per-second × per-packet TX duty
  against an unsuppressed baseline. Direct mA-level measurement is
  out of scope for this ADR; the proxy metric is sufficient.
- **Cluster-Pi accuracy**: ≤ 5 percentage-point drop on the
  ADR-084 Pass 3 novelty top-K coverage metric vs an unsuppressed
  baseline run on the same recorded CSI.
- **Bandwidth**: ≥ 50 % reduction in steady-state quiet-room UDP
  byte rate per node.
- **Stuck-gate self-heal**: a unit test that pins the sketch
  primitive output to "always low novelty" must observe a forced
  send within ≤ 10 s (≤ 50 frames at 5 Hz).
- **Existing test gates**: `cargo test --workspace
  --no-default-features` stays green; `python v1/data/proof/verify.py`
  stays green (the proof harness sees no firmware-side change and
  the SHA-256 should not move because the proof exercises Python
  pipeline math, not firmware behaviour); the witness bundle
  (`scripts/generate-witness-bundle.sh`) runs and the resulting
  `VERIFY.sh` reports 7/7 PASS — **the bundle's own SHA-256 will
  differ**, which is the witness-chain signal that firmware
  changed.

If any system-level number fails, the gate ships behind
`CONFIG_RV_EDGE_NOVELTY_GATE_ENABLE = n` (default-off) and the ADR
moves to **Rejected** for that hardware target while the wire-format
v7 changes are kept (they cost nothing dormant). If only the cluster-
Pi accuracy number fails, the gate is allowed to ship at a more
conservative `CONFIG_RV_EDGE_NOVELTY_THRESHOLD` until the cluster-
Pi-side reconciliation logic catches up.

## Open questions

1. **Does Xtensa LX7's lack of POPCNT make the table-lookup scalar
   baseline fast enough at 5 Hz?** **No primary-source confirmation
   performed — conjecture** (the ESP32-S3 TRM is the primary
   source). At 7 bytes/sketch × 32 slots = 224 bytes of popcount
   per frame, even a pessimistic 100-cycles-per-byte estimate sits
   well under 200 µs at 240 MHz; Pass 2 bench resolves it.
2. **Should the IRAM ring be replaced by PSRAM-backed storage when
   the board has it?** The 8 MB-flash Waveshare AMOLED ESP32-S3
   ships with 8 MB PSRAM (CLAUDE.md hardware table; not a primary
   source — the board datasheet is); the ring at 32 slots × 7 bytes
   does not need PSRAM. A larger ring (1024 slots × 7 bytes ≈ 7 kB)
   to keep a longer history would benefit from PSRAM. The default
   IRAM-only sizing is the correct ship-now choice; PSRAM-backed
   is an open follow-up if the cluster-Pi reconciliation logic
   needs more history than 32 slots provides.
3. **Where does `gate_version: u8` come from?** Three options:
   (a) Kconfig-pinned at firmware build time;
   (b) NVS-stored and bumped at provision time;
   (c) embedded as a build-id byte derived from the firmware
   manifest. Default: option (a), Kconfig-pinned. Rationale: the
   gate version is part of the firmware contract, not the per-
   deployment configuration. NVS is the wrong namespace; the build-
   id approach is more robust to provisioning slips but harder to
   compare across deployments. The decision is reversible — the
   field width is fixed at 8 bits regardless of source.
4. **Interaction with ADR-082 (pose-tracker confirmed-track
   filter).** The gate could legitimately suppress the very frames
   that would have promoted a Tentative track to Active in
   ADR-082's three-frame minimum. The risk is asymmetric: false-
   positive ghost poses are filtered by ADR-082 (correct), but
   false-negative-real poses are *enabled* by the edge gate
   suppressing real-but-quiet first frames. Mitigations are listed
   in Consequences; the ADR commits to (a) Tentative-state-aware
   threshold tuning if the validation regression on the pose
   recall metric exceeds 2 percentage points, and (b) keeping
   `motion_score >= 0.05` as an unconditional force-send override
   inside the gate. Open Q because the right mitigation depends on
   the measured regression.

## Related

- **ADR-018** (Accepted) — Binary CSI frame magic discipline. The
  v7 wire format follows the same magic-bump pattern.
- **ADR-028** (Accepted) — Capability audit / witness verification.
  Re-cut the bundle after this ADR ships; the SHA is *expected* to
  change.
- **ADR-081** (Accepted) — 5-layer adaptive CSI mesh firmware
  kernel. ADR-086 is a Layer 4 refinement.
- **ADR-082** (Accepted) — Pose-tracker confirmed-track filter.
  Open Q4 above.
- **ADR-084** (Proposed) — RaBitQ similarity sensor. The cluster-
  Pi reference for the same gate this ADR pushes to the edge.
- **ADR-085** (Proposed) — RaBitQ pipeline expansion. Seven
  cluster-Pi-side sites; ADR-086 is the deliberately-out-of-scope
  edge follow-up flagged at ADR-085 publication time.

## Related ADR slots

The user prompt that produced this ADR identified two further
follow-ups that should land as their own ADRs *if and when* the
triggering condition occurs. They are recorded here as pointer-stubs
rather than full ADRs because each is a one-paragraph commitment, not
a structured decision; opening a full ADR for either prematurely
would inflate the ledger without buying decision resolution.

### ADR-087 (prospective) — Pass-4 mesh-exchange scope clarification

ADR-084 §"Decision" lists "mesh-exchange compression" between sensor
nodes when reporting cross-cluster events as the fourth of its five
sites. The binding intent of that text is **cluster-Pi to cluster-Pi
exchange** — i.e., the ADR-066 swarm-bridge channel between peer
Cognitum Seeds — not sensor-MCU to cluster-Pi UDP traffic. The two
are different problems: cluster-to-cluster is std Rust on Linux/Mac
and reuses `BinaryQuantized` directly; sensor-to-Pi is what ADR-086
addresses. If the team later reinterprets Pass 4 as
sensor→cluster-Pi UDP compression, that would be ADR-086's twin and
should land as **ADR-087** with its own firmware release, distinct
from ADR-086's release. The clarification is one paragraph because
the only decision is "which interpretation does ADR-084's Pass 4
mean", and the answer is currently the cluster-to-cluster reading.
ADR-087 only opens if that reading is contested.

### ADR-088 (prospective) — Firmware-release coordination policy

Issues #386 and #396 (firmware-only fixes — the MGMT-only
promiscuous filter and the 50 Hz callback-rate gate) demonstrate
that the firmware can need a release independent of any cluster-Pi
ADR work. ADR-086 is itself an example: it requires a firmware
release that is not driven by ADR-084 or ADR-085, both of which are
cluster-Pi-only. Today the implicit policy is "firmware releases
when something firmware-only ships." That works but is undocumented.
**ADR-088** would formalize *when* a firmware release is required vs
deferred, with concrete examples: a Kconfig flag flip (#386 / #396)
must release; a Pi-side parser-only addition (ADR-085 Sites 1–7)
must not; a wire-format magic bump (ADR-086) must release and must
re-cut the witness bundle; a feature-flag-default flip on a shipped
v7 firmware should release a config bundle but not a firmware
binary. ADR-088 opens when the next firmware-only change after
ADR-086 lands and forces the decision; it is recorded here as a
slot rather than written speculatively because the actual release-
gating questions only become concrete in the presence of a real
shipping change.
