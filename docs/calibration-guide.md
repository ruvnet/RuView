# Calibration & Room Training Guide

This guide explains what actually happens — and what is actually *enforced* —
when you run `wifi-densepose calibrate`, `enroll`, and `train-room`. It is
written for the person setting up a room, not for developers.

Everything below was checked against the real Rust implementation in
`v2/crates/wifi-densepose-calibration/`, `v2/crates/wifi-densepose-signal/src/ruvsense/calibration.rs`,
and `v2/crates/wifi-densepose-cli/`, not just the design ADRs. Where the design
documents (ADR-135, ADR-151) describe something that isn't actually built yet,
this guide says so explicitly.

## Guided occupancy readiness (calibration HTTP service)

The `calibrate-serve` command now provides a focused occupancy-signal check
without training the full room specialist bank. This check asks whether one
operator-labelled empty window and one occupied window separate **in the
current CSI stream**. It does not measure detection accuracy or create the
signed capability certificate described in ADR-301/318.

1. Start the service and point **one** supported ESP32 CSI node at its UDP
   port. This first slice supports one stream/room per service instance;
   additional nodes on the same port invalidate readiness rather than mixing
   their evidence:

   ```bash
   wifi-densepose calibrate-serve --udp-port 5005 --http-port 8090 --output-dir ./baselines --input-kind live
   ```

   `live` is a **declared mode**; the current UDP packet format does not
   authenticate sensor origin or rule out packet replay. Keep the HTTP API on
   its loopback default. If exposing it on a network, set `--token` and send
   an `Authorization: Bearer <token>` header on every request.

2. Clear the room, capture the usual empty-room baseline, and wait for
   `state: complete`. Use the persisted filename stem from the `output_path`
   returned by `/api/v1/calibration/result`. For example, if the result path
   ends in `office-<uuid>.bin`, the baseline parameter below is
   `office-<uuid>`.

   ```bash
   curl -sS -X POST http://127.0.0.1:8090/api/v1/calibration/start \
     -H 'content-type: application/json' \
     -d '{"room_id":"office","duration_s":60}'
   curl -sS http://127.0.0.1:8090/api/v1/calibration/status
   curl -sS http://127.0.0.1:8090/api/v1/calibration/result
   ```

3. With the room still empty, verify an empty window. Wait until
   `phase` becomes `null` and `observed_empty_amp` appears, then stand in the
   room and verify the occupied window:

   ```bash
   curl -sS -X POST http://127.0.0.1:8090/api/v1/occupancy/verify \
     -H 'content-type: application/json' \
     -d '{"room_id":"office","baseline":"office-<uuid>","phase":"empty"}'
   curl -sS http://127.0.0.1:8090/api/v1/occupancy/readiness
   curl -sS -X POST http://127.0.0.1:8090/api/v1/occupancy/verify \
     -H 'content-type: application/json' \
     -d '{"room_id":"office","baseline":"office-<uuid>","phase":"occupied"}'
   curl -sS http://127.0.0.1:8090/api/v1/occupancy/readiness
   ```

   The endpoint defaults to 600 frames per phase (allowed 32–4096) and stops
   a stalled phase after a timeout sized for at least 10 frames/s plus 30
   seconds of overhead (minimum two minutes). A request below 600 frames can exercise
   the flow, but cannot yield live VALID. `VALID` means this local heuristic saw
   enough fresh frames and a mean-amplitude shift between the labelled windows
   above 3× the combined estimated uncertainty of their medians
   (1.857×MAD/√frames per window) and a provisional 0.75% relative floor.
   This uncertainty estimate does not correct for serially correlated frames.
   The empty median must also stay
   within a provisional 10% of the saved baseline mean. These are conservative
   setup heuristics, not calibrated accuracy guarantees. The mean-amplitude
   choice follows [measured ESP32 observations in PR #1909](https://github.com/ruvnet/RuView/pull/1909),
   which found that median baseline z scores often failed to separate empty
   and occupied captures. This endpoint then reports
   `occupancy: empty|occupied` from the rolling live window. `DEGRADED` means
   the empty check differs from the baseline or the two windows lack contrast.
   `UNKNOWN` means the setup is
   incomplete, the stream is stale (over 2 seconds), the CSI grid changed,
   its source or AP channel changed, verification timed out, or the server
   restarted. In DEGRADED and UNKNOWN,
   `occupancy` is null; read `reason` and `next_action` for the specific fix.

4. After moving furniture, changing the sensor or AP, or seeing an unexpected
   reading, run the empty verification again. That immediately clears the
   previous occupied evidence; repeat the occupied phase before readiness can
   return to VALID. A stream outage likewise clears old evidence on resumption.
   A sustained live window outside the labelled amplitude range plus a margin
   based on observed contrast, noise, and baseline amplitude also latches
   DEGRADED. Recovery
   then requires both guided phases again. This flags an out-of-range signal,
   **not** a diagnosis of furniture drift: changed occupant position, a pet,
   and a changed room can look similar. If the empty check remains DEGRADED,
   recapture the baseline after checking placement.

For a packet replay or generated CSI, start the service with
`--input-kind replay` or `--input-kind synthetic`. Both modes exercise the
same ingest and contrast path and report their declared source, but they can
never promote a **live room** to VALID. These labels are operator declarations,
not cryptographic source attestations. No real-room accuracy, false-alarm rate,
or certificate claim follows from the synthetic tests in this PR. Keep this
heuristic out of safety-critical or actuator decisions.

## The three-step pipeline

```
wifi-densepose calibrate --port <PORT>      # Stage 1: empty-room baseline (no people)
wifi-densepose enroll --room <NAME>         # Stage 2+3: 8 guided anchors (~4 minutes)
wifi-densepose train-room --room <NAME>     # Stage 4: fit the specialist bank
wifi-densepose room-status --room <NAME>    # check what trained / what's stale
wifi-densepose room-watch --room <NAME>     # live inference
```

`calibrate` must run first — `enroll` refuses to start without a baseline file
(`--baseline ./baseline.bin` by default), and `train-room` refuses to start
without an enrollment file. Each step writes a file the next step reads; there
is no way to skip a step.

---

## 1. Is there a minimum amount of data required?

**Yes, and for the empty-room baseline it is a hard, enforced minimum — not a
recommendation.**

`wifi-densepose calibrate` will not produce a baseline file with fewer than
**600 recorded frames** (the default for every PHY tier: HT20, HT40, HE20,
HE40). This is `DEFAULT_MIN_FRAMES = 600` in
`v2/crates/wifi-densepose-signal/src/ruvsense/calibration.rs:48`, and it is
checked in `CalibrationRecorder::finalize()`
(`ruvsense/calibration.rs:532-538`): if fewer than `config.min_frames` frames
were recorded, `finalize()` returns
`CalibrationError::InsufficientFrames { got, need }` and calibration fails
outright — there is no partial/degraded baseline. This is pinned by a unit
test (`finalize_requires_min_frames`, same file) so it isn't accidental
behavior.

**Important subtlety:** the CLI's `--duration-s` flag (default 30 seconds)
and the 600-frame minimum are checked *independently*. The capture loop in
`v2/crates/wifi-densepose-cli/src/calibrate.rs:135-183` stops as soon as
**either** the duration timer expires **or** 600 frames have been recorded,
whichever comes first. If your node streams CSI slower than the assumed 20 Hz
(e.g. congested WiFi, a busier ESP32), 30 seconds may not be enough to reach
600 frames, and `calibrate` will fail with an explicit
`"insufficient frames: have X, need 600"` error rather than silently
producing a short baseline. If you hit this, raise `--duration-s` rather than
overriding `--min-frames`.

You *can* override the 600-frame floor with `--min-frames <N>` (0 = use the
tier default). The code prints an explicit warning when you do:

> `[calibrate] WARN: --min-frames=N overrides ADR-135 tier default (600 for
> ht20). This relaxes the phase-concentration guarantee; do not use in
> production.`

(`v2/crates/wifi-densepose-cli/src/calibrate.rs:112-119`). Treat this as a
debugging escape hatch, not a supported way to shorten setup.

The CLI also independently rejects `--duration-s` below 10 seconds
(`"Fewer frames produce unreliable phase-concentration estimates"`,
`calibrate.rs:341-348`) and prints (but does not block on) a warning above 300
seconds.

**Guided enrollment (`enroll`) has a much lower, per-anchor floor.** Each of
the 8 guided anchors (`empty`, `stand_still`, `sit`, `lie_down`,
`breathe_slow`, `breathe_normal`, `small_move`, `sleep_posture`) is captured
for a fixed duration baked into the code — 20 seconds for the static/motion
anchors, 30 seconds for the two breathing anchors and `sleep_posture`
(`AnchorLabel::duration_s()`, `v2/crates/wifi-densepose-calibration/src/anchor.rs:98-104`).
This is **not** a CLI flag — you cannot currently shorten or lengthen an
individual anchor capture from the command line.

Underneath that fixed duration, the anchor is only *accepted* if it clears a
quality gate (`AnchorQualityGate`, `v2/crates/wifi-densepose-calibration/src/enrollment.rs:43-53`):

| Threshold | Default | What it checks |
|---|---|---|
| `min_frames` | **60 frames** | Anchor is rejected if fewer than 60 frames were captured — mainly catches "the ESP32 stopped streaming" mid-capture, not a real duration requirement (60 frames is a fraction of a second of streaming at typical rates) |
| `min_presence_z` | 1.5 | For anchors that expect a person, the mean amplitude z-score must exceed this or the anchor is rejected as "no person detected" |
| `empty_max_z` | 1.0 | For the `empty` anchor, the z-score must stay under this or it's rejected as "room not empty" |
| `max_still_motion` | 0.6 (60%) | For still anchors, motion-flagged frame fraction above this is rejected as "too much motion" |
| `min_move_motion` | 0.3 (30%) | For `small_move`, motion-flagged fraction below this is rejected as "not enough motion" |

A rejected anchor is re-prompted, up to `--attempts` times (default **2**).
If an anchor is still rejected after all attempts, `enroll` moves on without
it and logs `"moving on without '<label>'"` — enrollment does **not** abort;
you end up with a partial anchor set.

**`train-room` itself enforces almost nothing.** It only bails if the
enrollment file has *zero* accepted anchors at all
(`v2/crates/wifi-densepose-cli/src/room.rs:246-248`, `"no accepted anchors …
re-run enroll"`). There is no minimum anchor count beyond that. What actually
happens with a partial anchor set is that individual specialists silently
fail to train and are simply absent from the resulting bank — for example
(from `v2/crates/wifi-densepose-calibration/src/specialist.rs`):

- **presence** needs the `empty` anchor plus at least one anchor where a
  person was expected present — missing either, `PresenceSpecialist::train()`
  returns `None` and presence detection is unavailable in that bank.
- **anomaly** needs at least 2 anchors total, of any kind.
- **restlessness** needs `sleep_posture` (or `lie_down` as a fallback) *and*
  `small_move`.
- **posture** needs at least one anchor that establishes a posture
  (`stand_still`, `sit`, `lie_down`, or `sleep_posture`).

So a "successful" `train-room` run can still produce a bank missing one or
more specialists if enrollment didn't collect the anchors those specialists
need. `room-status` (`v2/crates/wifi-densepose-cli/src/room.rs`) is the way
to check what actually trained.

### What we could not verify

The ADR-151 design document (§2.2) claims total guided enrollment is
"~4 minutes of wall-clock" — that arithmetic checks out against the coded
per-anchor durations (5 × 20s + 3 × 30s = 190s ≈ 3.2 min, plus a 3-second
countdown before each anchor ≈ +24s, so ~3.5–4 minutes is consistent with the
code). But we found **no integration test or measurement showing that this
duration is sufficient for reliable specialist accuracy** — the ADR's own
status section says the full `baseline → enroll → train-room → infer` loop is
proven only against **deterministic synthetic CSI** (`tests/full_loop.rs`),
not yet run start-to-finish on real hardware in an empty room. Treat the
default durations as reasonable code defaults, not as a validated minimum for
real-world accuracy.

---

## 2. Recommended duration if there's no hard minimum

Where a hard minimum *does* exist (the 600-frame baseline, the 60-frame
per-anchor floor), it's documented above. Beyond that:

- **Baseline capture**: the CLI default (`--duration-s 30`) is the number to
  use; it's what the 600-frame minimum is designed around at the assumed
  20 Hz sensing rate. ADR-135 §2.3 argues 30 s is the shortest duration that
  keeps the phase-concentration estimate's standard deviation under
  0.02 rad², citing published circular-statistics error bounds — but this is
  a paper-derived justification for the *default value*, not a code-enforced
  floor beyond the 600-frame check itself.
- **Enrollment anchors**: use the built-in per-anchor durations (20s/30s) —
  there's currently no way to change them from the CLI anyway.

---

## 3. Will a pet get classified as "occupied"?

**Honest answer: the code has no way to distinguish a pet (or any small/animal-scale
motion) from a person.** This is a real limitation, not a solved problem —
flagging it here rather than guessing.

Presence detection (`PresenceSpecialist`,
`v2/crates/wifi-densepose-calibration/src/specialist.rs:100-198`) is trained
purely from two scalar channels measured during enrollment:

- **variance** of the CSI amplitude series, thresholded at the midpoint
  between the `empty` anchor's variance and the mean variance of the
  person-present anchors;
- **mean shift** — `|mean − empty_mean|`, thresholded at half the
  empty→occupied mean distance.

Presence fires if **either** channel crosses its threshold. Both thresholds
are learned entirely from the amplitude statistics of your enrollment
anchors — there is no body-size, RCS (radar cross-section), Doppler-signature,
or any other physical feature in this code that separates "a full-grown
adult moved" from "a cat walked past" or "a dog jumped on the couch." If a
pet's motion perturbs the CSI amplitude by roughly the same amount as the
`small_move` anchor did during your enrollment, `PresenceSpecialist` will read
it as occupied, because that's mechanically what the threshold measures.

The closest thing to a safeguard is `AnomalySpecialist`
(`specialist.rs:386-448`), a generic novelty detector that flags a live
window as "anomalous" when it's far (in embedding distance) from every
enrolled anchor prototype. It is **not** a validated pet filter — it will
flag *any* statistically unusual signal as anomalous or normal depending on
how close it happens to land to your anchors, with no guarantee it
distinguishes species or motion source. A pet whose motion pattern happens
to resemble the `small_move` anchor would not be flagged as anomalous at all.

**Practical takeaway for a homeowner with pets:** expect presence/posture
readings to occasionally trigger on pet motion, especially larger animals or
motion near the sensor. There is currently no configuration option or code
path to suppress this.

---

## 4. Does the empty-room baseline need "typical" conditions (HVAC running) or true silence?

The short answer, grounded in how the baseline is actually computed: **a
stationary, continuously-running interferer (a fan, HVAC blower, humidifier)
that is present for the *entire* capture window becomes part of what "empty"
means, and gets subtracted out naturally** — that's a direct consequence of
how the statistics are computed, not a documented feature you have to
configure.

`CalibrationRecorder` uses Welford's online algorithm to accumulate a running
mean and variance per subcarrier over however many frames you feed it
(`ruvsense/calibration.rs`). If a fan is running steadily the whole time you
capture the baseline, its contribution is baked into `amp_mean`/`amp_variance`
for every frame equally, so the resulting baseline already represents "empty
room with the fan on" — and at runtime, `BaselineCalibration::subtract()`
removes exactly that reference, so a room in the same steady state reads as
quiet. The design intent documented in ADR-135 §1.1 is explicit about this:
the whole point of baseline subtraction is to remove "hardware-induced gain
bias and environment-fixed multipath" so downstream motion detectors aren't
tripped by things that are always there.

**What actually matters is consistency, not silence**: capture the baseline
under whatever background conditions the room will normally be in during
real use (HVAC/fans running as usual), and try to keep the room in that same
steady state for the entire capture window. What the code cannot correct
for is a background condition that **changes partway through** the capture
(e.g. HVAC cycles on 15 seconds into a 30-second capture) — that would bias
the Welford mean/variance toward an in-between state that matches neither
"HVAC off" nor "HVAC on" well.

There is a **real-time guard during capture** that can catch gross problems:
`--abort-z-threshold` (default `2.0`) aborts the capture if the per-frame
amplitude z-score median stays above that threshold for 20 consecutive
banner intervals (`v2/crates/wifi-densepose-cli/src/calibrate.rs:82-83,
163-178`). This is designed to catch someone walking through mid-capture, not
necessarily short-duration mechanical noise — we found no test exercising it
against an HVAC-cycling scenario specifically, so how it behaves for
"appliance turns on mid-capture" is unverified.

### What we could not verify — and a design gap worth knowing about

ADR-135 §2.5 describes a much more sophisticated staleness-detection system:
a `drift_score` computed from ongoing z-scores, a `BaselineDrift` event fired
after sustained drift, and a `baseline_stale` flag published over the
sensing WebSocket. **We searched the actual `calibration.rs` implementation
and none of that exists in code** — there is no `drift_score` field, no
`BaselineDrift` event, and no `baseline_stale` flag anywhere in
`v2/crates/wifi-densepose-signal/src/ruvsense/calibration.rs`. That part of
ADR-135 is aspirational design, not shipped behavior.

What *is* implemented, at a different layer, is a much simpler check on the
**trained specialist bank** (not the raw baseline): `SpecialistBank` stores
the `baseline_id` it was trained against, and `SpecialistBank::is_stale()`
(`v2/crates/wifi-densepose-calibration/src/bank.rs:102-104`) returns `true`
whenever the *current* baseline's id doesn't match the id the bank was
trained on. Re-running `calibrate` always produces a new baseline id, so
**any** recalibration — whether because of furniture moving, a genuinely
stale reference, or just re-running the command — immediately marks every
previously trained specialist bank stale, and you'll need to re-run `enroll`
and `train-room` afterward. There is no partial/graded staleness signal
(no "how stale"), only this all-or-nothing id comparison.

**Practical guidance:**

1. Calibrate with the room in its normal, steady background state (HVAC,
   fans, fridge compressor, etc. running as they normally would) and keep
   that state constant for the whole `--duration-s` window.
2. If you significantly change background conditions later (move furniture,
   add a permanent appliance, change HVAC routine) or notice the sensing
   quality degrade, re-run `calibrate` — this is an explicit, operator-driven
   step; there is no code path that recalibrates for you.
3. Re-running `calibrate` invalidates every specialist bank trained against
   the old baseline (via the `baseline_id` mismatch above) — plan to re-run
   `enroll` and `train-room` right after.

---

## Quick reference: commands and defaults actually in the code

```bash
# Stage 1 — empty-room baseline. Room must be empty for the whole window.
wifi-densepose calibrate \
  --udp-port 5005 --duration-s 30 --tier ht20 --output ./baseline.bin
# Hard requirement: >= 600 recorded frames, or calibration fails.

# Stage 2+3 — guided enrollment (8 fixed anchors, ~4 minutes total)
wifi-densepose enroll --baseline ./baseline.bin --room living-room \
  --output ./enrollment.json --attempts 2

# Stage 4 — train the specialist bank from whatever anchors were accepted
wifi-densepose train-room --enrollment ./enrollment.json \
  --output ./room-bank.json

# Check what actually trained (and whether the bank is stale)
wifi-densepose room-status --room living-room
```

Source references for everything above:
- `v2/crates/wifi-densepose-cli/src/calibrate.rs`
- `v2/crates/wifi-densepose-cli/src/room.rs`
- `v2/crates/wifi-densepose-signal/src/ruvsense/calibration.rs`
- `v2/crates/wifi-densepose-calibration/src/enrollment.rs`
- `v2/crates/wifi-densepose-calibration/src/anchor.rs`
- `v2/crates/wifi-densepose-calibration/src/specialist.rs`
- `v2/crates/wifi-densepose-calibration/src/bank.rs`
- `docs/adr/ADR-135-empty-room-baseline-calibration.md`
- `docs/adr/ADR-151-room-calibration-specialist-training.md`
