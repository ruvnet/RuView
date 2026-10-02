# ADR-180: Through-Wall Camera↔CSI Hand-off Demo ("Behind the Wall")

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-06-15 |
| **Deciders** | ruv |
| **Codename** | **BEHIND-THE-WALL** |
| **Builds on** | ADR-079 (camera ground-truth training), ADR-031 (sensing-first RF mode), ADR-134 (CSI→CIR multipath), ADR-029/030 (RuvSense multistatic + persistent field), ADR-024 (AETHER re-ID), ADR-151 (per-room calibration), ADR-173 (metric-locked PCK), ADR-095/096 (rvcsi nexmon) |

## Context

### The demo we want
A single self-contained **HTML page** that tells one honest, visceral story:

1. You stand in front of the laptop. The camera tracks your **full skeletal pose**;
   the WiFi-CSI model, trained on *your* movements moments earlier, infers the **same
   skeleton** in parallel — a side-by-side "camera vs RF agree" view.
2. You **walk out the door and behind the wall**. The camera **goes blind** (you are
   occluded — it honestly shows "no person in frame"). The CSI model **keeps inferring
   your skeleton** from the WiFi signal alone — the 3D figure keeps walking, behind the
   wall, smoothly. A badge flips from `CAMERA` to `RF-INFERRED (through-wall)`.
3. You **walk back into view**. The camera **re-acquires**; the badge flips back to
   `CAMERA`, and the RF-inferred and camera skeletons reconverge.

This is the "WiFi sees through walls" demo — and the user explicitly wants the **inferred
skeleton through the wall**, not just a blob. The project's "prove everything / no AI-slop"
bar means we make that claim **only because we measure it**: a second camera on the far side
of the wall records ground-truth pose *behind* the wall, so the through-wall skeleton's
accuracy is a **reported, reproducible number** — never an unfalsifiable "trust me."

### Honest capability framing (the load-bearing section)
Through-wall **per-joint skeletal inference from WiFi CSI is not a generally-validated
capability** in open settings — WiFi-DensePose (CMU) is camera-*co-located*. What makes it
defensible *here* is the tightly-controlled regime and the measurement:

- **Controlled regime:** one room, one subject (you), one doorway, a model **camera-supervised
  on your exact gait and your exact through-door transition** (ADR-079) minutes earlier. This
  is in-distribution for *this* demo, not a universal claim.
- **Measured, not asserted:** a far-side camera (cognitum-v0 has 17 `/dev/video*` nodes — use
  one, or a phone) records ground-truth pose behind the wall. The through-wall CSI skeleton is
  scored against it with the metric-locked PCK harness (ADR-173). **We publish the number.**
- **Uncertainty is rendered, not hidden:** the through-wall skeleton is drawn **translucent**,
  with a live **per-joint confidence** and an explicit `RF-INFERRED` badge. High-confidence
  joints render solid; low-confidence joints fade. It never masquerades as the camera's
  ground-truth pose.

| While… | Camera | WiFi CSI (S3 / Pi5 nexmon, fused) | 60 GHz mmWave (C6 + MR60BHA2) |
|--------|--------|-----------------------------------|-------------------------------|
| In frame | **Full 17-kpt pose** — ground truth | full skeleton (supervised model) — *agrees with camera* | presence + range + micro-motion |
| Behind a **drywall** | nothing (occluded) | **inferred full skeleton** (camera-supervised model + multistatic fusion), confidence-scored, **measured vs far-side camera** | presence + range + breathing — independent through-thin-wall confirm |
| Behind **brick/metal** | nothing | degrades to coarse motion/position only — report honestly | blocked |

**The claim — stated precisely:** *"A WiFi-CSI model, camera-supervised on this subject and
room, infers a continuous skeletal pose that tracks the subject through a drywall partition;
through-wall accuracy is measured at X% PCK@k against a far-side camera (declared, not
claimed)."* If X turns out low, that is the **honest result we report** — the skeleton is still
rendered (the user wants it) but flagged with its true confidence, and the headline number is
whatever we measured, good or bad.

### Why multistatic + supervision is the enabler
A single node behind a wall sees only "something moved." Three spatially-diverse vantage points
around the doorway (RuvSense multistatic + cross-viewpoint fusion, ADR-029/030) triangulate the
moving scatterer — drywall attenuates and diffracts 2.4/5 GHz but does not block it — giving the
model a rich enough multipath signature to regress a skeleton it was *trained* to associate with
your through-door motion. AETHER re-ID embeddings (ADR-024) keep it locked to **you** across the
camera→RF→camera hand-off.

### Available hardware (the user's actual rig)
| Role | Device | Where | Stream |
|------|--------|-------|--------|
| Near ground truth (visible) | Laptop / USB camera | front of workstation (ruvzen) | MediaPipe pose → keypoints |
| **Far ground truth (validation)** | cognitum-v0 camera (1 of 17 `/dev/video*`) or a phone | **behind the wall** | MediaPipe pose → keypoints (for MEASURING the through-wall skeleton) |
| CSI node A | ESP32-S3 (8 MB) | COM9 (ruvzen) | UDP CSI :5005 |
| CSI + mmWave node B | ESP32-C6 + Seeed MR60BHA2 | COM12 (ruvzen) | WiFi CSI + 60 GHz FMCW presence/range |
| CSI node C (through-wall vantage) | Pi 5, BCM43455c0 | cognitum-v0 (other room) | nexmon_csi `.pcap` → rvcsi → CsiFrame |
| Fusion + serving | sensing-server | ruvzen :3000/:8765 | `/ws/sensing`, `/ws/pose`, new `/ws/handoff` |

Place **node C (Pi 5) and the far camera on the far side of the wall** — the Pi 5 gives the
fuser a vantage the camera lacks, and the far camera turns the through-wall claim into a
measurement.

## Decision

Build a **camera↔CSI hand-off demo** as a thin, additive layer over existing components (no new
heavy crate). Five parts: a multi-source capture plane, a camera-supervised calibration walk
that **learns to infer the skeleton through the wall**, a **hand-off state machine**, a
**dead-reckoning smoother** so dropped CSI never makes the figure jump, and a single-file HTML
viewer that renders the inferred skeleton with honest confidence.

### 1. Capture plane (reuse, don't rebuild)
- **Near camera:** `scripts/collect-ground-truth.py` already does MediaPipe pose + ESP32 CSI
  paired capture (ADR-079). Extend it to also subscribe to the Pi 5 nexmon stream (rvcsi), the
  C6 mmWave presence, **and the far camera**, so every frame is
  `(near_pose|null, far_pose|null, csi_S3, csi_C6, mmwave_C6, csi_Pi5, t)`.
- **CSI nodes:** S3 over UDP :5005, Pi 5 via `rvcsi` (vendor/rvcsi nexmon adapter → `CsiFrame`),
  C6 WiFi CSI + the MR60BHA2 60 GHz presence/range/breathing.
- **Fusion:** all CSI sources into the existing `MultistaticFuser`
  (`signal/src/ruvsense/multistatic.rs`); node positions around the doorway via
  `--node-positions` (geometric-diversity index drives confidence). **#1049:** with 3
  independently-clocked nodes set `WDP_GUARD_INTERVAL_US` to the real inter-node spread or
  fusion demotes.

### 2. Calibration walk — "it learns my movements **and infers them through the wall**" (ADR-079)
A 3–5 minute guided routine. The HTML page scripts the walk: stand, step left/right, walk to the
door, **cross fully behind the wall and back**, repeat — covering the visible AND the occluded
zone, because **both cameras label ground truth**:
- **Visible-zone supervision:** near camera labels pose; synchronized CSI window is the input.
- **Through-wall supervision (the key part):** while you are behind the wall, the **far camera**
  labels your pose. So the CSI→skeleton model is trained on *real behind-wall poses* paired with
  the *behind-wall multistatic CSI* — the model genuinely learns to infer your skeleton through
  the wall, supervised by ground truth, not extrapolated blindly.
- Train/fine-tune on `ruvultra` (RTX 5080) if available, else the local recipe. Persist as a
  per-room calibration bank (ADR-151 `baseline → enroll → extract → train`). AETHER re-ID
  embeddings (ADR-024) bind the track to you across the hand-off.
- **Held-out split:** reserve some behind-wall passes for evaluation so through-wall PCK is
  measured on data the model never trained on (no leakage — the ADR-152 measurement discipline).

### 3. Hand-off state machine (`sensing-server/src/handoff.rs`, < 300 lines)
States: `CAMERA` → `HANDOFF_OUT` → `RF_INFERRED` → `HANDOFF_IN` → `CAMERA` (+ `LOST`).
- **`CAMERA`** — near camera has a confident pose → render it; RF-inferred skeleton ghosted
  alongside for the "they agree" effect.
- **`HANDOFF_OUT`** — near-camera confidence drops at the doorway **while** CSI motion stays high
  and the multistatic track heads into the door zone → cross-fade source camera→RF.
- **`RF_INFERRED`** — no camera pose; the CSI model emits a **full 17-kpt skeleton** + per-joint
  confidence; AETHER confirms it is still you. Render the translucent skeleton + confidence,
  badge `RF-INFERRED (through-wall)`. (When fusion confidence is too low for a credible skeleton,
  degrade gracefully to a coarse marker rather than a flailing one — honest fallback.)
- **`HANDOFF_IN`** — near camera re-acquires a pose positionally consistent with the last RF
  skeleton (continuity gate) → cross-fade RF→camera.
- **`LOST`** — neither source for N cycles → "no track," never invented.

Fail-closed: `RF_INFERRED` requires real multistatic motion energy + an AETHER identity match
above calibrated floors; absent that → `LOST`, never a phantom. Mirrors the governed-trust gate
(ADR-031 / ADR-141).

### 4. Dead reckoning & smoothing — fluid, never jumpy (the user's requirement)
CSI does **not** arrive cleanly: UDP frames drop, nexmon `.pcap` has gaps, the fuser skips
cycles when the #1049 guard rejects a spread, and the model's per-frame skeleton jitters. Render
only on real frames and the figure teleports and shakes — which also *reads as fake*. A
**predict/correct (dead-reckoning) layer** keeps the skeleton continuous and smooth between
measurements, with **bounded** extrapolation so we never invent motion that didn't happen:

- **Per-joint constant-velocity Kalman filter** — reuse `signal/src/ruvsense/pose_tracker.rs`
  (the project's existing 17-keypoint Kalman tracker with AETHER re-ID). The renderer runs at a
  **fixed ~30 Hz, decoupled from CSI arrival**:
  - **Measurement this tick** → Kalman *update* (correct) each joint with the new inferred pose.
  - **Dropped CSI this tick** → Kalman *predict* only: advance each joint by `x += v·dt`, so the
    skeleton keeps moving along its trajectory instead of freezing then snapping. **This is the
    dead reckoning** — the limbs keep their motion through a dropout.
- **Confidence decay (honesty governor):** every predict-only tick multiplies confidence and
  widens covariance. Dead reckoning is trusted for a **bounded** horizon (default ≤ ~500 ms,
  `WDP_DEADRECKON_MAX_MS`); past it, confidence hits the floor → state machine → `LOST`. **We
  coast briefly to stay smooth; we never coast forever to fake a track.** Someone who actually
  stopped behind the wall converges to a still pose then `LOST`, not perpetual phantom walking.
- **Re-acquire smoothing:** a returning measurement after a gap is blended in with a
  critically-damped step (no overshoot) over 2–3 ticks, so the skeleton eases onto truth.
- **Client render smoothing (already present):** `ui/observatory/js/figure-pool.js`
  `applyKeypoints` already `lerp`s joints with a small velocity overshoot for secondary motion;
  the hand-off viewer reuses it. The camera↔RF cross-fade is an alpha-lerp over ~300 ms.

**Dead-reckoning honesty invariants (testable):**
1. Predicted-only frames carry `"dead_reckoned": true` + `"age_ms"`; the UI dims them —
   extrapolation is never shown as a fresh measurement.
2. Confidence is **monotonically non-increasing** across consecutive predict-only ticks.
3. After `WDP_DEADRECKON_MAX_MS` of silence the state **must** become `LOST` (pinned test:
   measurements then silence → assert transition within the horizon; no perpetual motion).
4. Dead reckoning extrapolates an **existing** track only — no measurement ever ⇒ no track ⇒
   `LOST`, never a phantom from zero.

### 5. The HTML demo (single file, vanilla — mirrors the Observatory)
`ui/through-wall/index.html` (+ a small JS bundle, zero build step, like `ui/observatory/`):
- **Left:** near camera feed with the MediaPipe skeleton overlaid while visible; greys to
  "CAMERA BLIND" when occluded. (Optional second tile: the far camera, shown only in a
  "validation" view, not the hero view.)
- **Right:** a top-down 3D room (Three.js) with the **wall** drawn, the doorway, the three
  sensor positions, and the figure: a **solid skeleton** in `CAMERA`, a **translucent skeleton
  with per-joint confidence fade** in `RF_INFERRED`, eased by the dead-reckoning smoother.
- **Banner / `BannerState`** (strict, mirrors rufield-viewer): `CAMERA` / `RF-INFERRED — through
  wall (conf X%, measured Y% PCK@k)` / `DEAD-RECKONED (age N ms)` / `LOST` — mutually exclusive,
  with a one-line honesty caption. The measured through-wall PCK is shown, not invented.
- Consumes a new `GET /ws/handoff` WS/SSE topic of `HandoffFrame`s; `?demo=1` replays a recorded
  session badged `REPLAY`.

### Output contract (`HandoffFrame`, JSON)
```jsonc
{
  "t_ns": 1718400000000,
  "state": "RF_INFERRED",             // CAMERA | HANDOFF_OUT | RF_INFERRED | HANDOFF_IN | LOST
  "source": "fused_csi",              // camera | fused_csi | mmwave | dead_reckoned
  "pose": [[x,y,z,conf], …×17],       // inferred skeleton WITH per-joint confidence (present in CAMERA/HANDOFF/RF_INFERRED)
  "pose_confidence": 0.58,            // aggregate; the rendered translucency
  "identity_match": 0.81,             // AETHER re-ID — is it still you?
  "coarse": { "cell":[x,y], "zone":"behind_wall", "heading_deg":95, "node_diversity":0.48 },
  "dead_reckoned": false,             // true on predict-only (extrapolated) ticks
  "age_ms": 0,                        // ms since the last real measurement (0 = fresh)
  "camera_blind": true,
  "measured_pck": { "k": 20, "value": null },  // filled from the far-camera validation run; null until measured
  "caption": "RF-inferred skeleton — model camera-supervised on this room; through-wall PCK measured separately"
}
```

## Phased plan (each phase independently demoable + falsifiable)
- **P1 — wiring (no claim):** 3-source CSI capture (S3+C6+Pi5) + near camera into the multistatic
  fuser. Gate: `/ws/sensing` shows ≥3 active nodes + a fused position with the camera running.
- **P2 — supervised calibration + through-wall training:** the guided walk with **both cameras**;
  fine-tune CSI→skeleton on visible AND far-camera-labeled behind-wall poses (ADR-079). Gate:
  while-visible PCK declared (metric-locked, ADR-173) on a held-out segment.
- **P3 — MEASURE the through-wall skeleton:** score the RF-inferred skeleton against the far
  camera on held-out behind-wall passes → **publish the through-wall PCK@k** (good or bad). Gate:
  a committed eval script reproduces the number; honest negative if low.
- **P4 — hand-off + dead reckoning + HTML:** the camera→RF→camera transition renders end-to-end,
  smooth through dropped CSI. Gate: a recorded live walk where the camera goes blind, the inferred
  skeleton keeps walking fluidly behind the wall, dead-reckons through dropouts without jumps, and
  re-acquisition is position-continuous. **This is the demo.**
- **P5 — multi-modal corroboration (optional):** overlay C6 60 GHz presence/range as an
  independent through-thin-wall confirm (two physics, one conclusion).

## Consequences

### Positive
- A genuinely compelling demo that does what the user asked — **infers and renders the skeleton
  through the wall** — while staying honest because the through-wall accuracy is **measured**
  against a far-side camera, not claimed. Reuses the multistatic fuser, ADR-079 supervision, the
  Kalman pose tracker, AETHER re-ID, the calibration crate, and the Observatory UI: the new code
  is a hand-off module + dead-reckoning smoother + an HTML page.

### Negative / Risks
- **Through-wall skeletal accuracy may be modest or poor.** That is acceptable *iff* reported
  honestly — the headline is the measured PCK, whatever it is; the skeleton renders with its true
  per-joint confidence (low-confidence joints fade), never as fake certainty.
- **Material dependence:** drywall good; brick/metal degrades to coarse-only — shoot on drywall
  and say so.
- **3-node clock sync** is the #1049 hazard — tune `WDP_GUARD_INTERVAL_US`.
- **Per-room, per-subject:** the model that "learned your movements" does not transfer without
  re-calibration — stated on the page.
- **Over-claiming is the failure mode.** Mitigations baked in: translucent confidence-faded
  skeleton, `dead_reckoned`/`age_ms` flags, the measured-PCK banner, bounded extrapolation→`LOST`.

### Neutral
- No new heavy crate; signal-path proof (`verify.py`) untouched — capture/fusion/UI orchestration
  over hardened, already-reviewed components.

## Acceptance criteria (falsifiable — "prove the haters wrong")
On a recorded live session, all must hold:
1. A contiguous window where the **near camera reports no person** (verifiable from raw frames)
   **and** the system renders an `RF_INFERRED` skeleton.
2. The inferred skeleton's **gross motion matches reality** — direction of travel and rough gait
   phase — confirmed against the **far camera** (not eyeballed).
3. **Through-wall per-joint accuracy is MEASURED** against the far camera and **reported** as
   PCK@k from a committed script. Low is fine *if* honestly published; fabricated is not.
4. The figure is **smooth through dropped CSI** — no teleports/jitter — and every predicted-only
   frame is flagged `dead_reckoned`; after `WDP_DEADRECKON_MAX_MS` of silence it goes `LOST`.
5. Re-acquisition is **position-continuous** (camera re-detects within a cell of the last RF
   position), and AETHER confirms identity across the hand-off.
6. Every number (visible PCK, through-wall PCK, confidences) is MEASURED and reproducible — no
   hand-typed metrics.

A demo that cannot meet (1)–(2) and (4)–(5) on the available hardware is reported as a **negative
result** (honest), not dressed up; a poor (3) is published as the real number.

## Links
- ADR-079 — camera ground-truth training (supervision pipeline; extended here to a far camera)
- ADR-031 — sensing-first RF mode / coherence gate (fail-closed honesty pattern)
- ADR-134 — CSI→CIR multipath (through-wall multipath physics)
- ADR-029 / ADR-030 — RuvSense multistatic + persistent field (the localization engine)
- ADR-024 — AETHER contrastive re-ID (identity lock across the hand-off)
- ADR-151 — per-room calibration crate (bank persistence)
- ADR-152 / ADR-173 — measurement discipline + metric-locked PCK (the honest accuracy readout)
- ADR-095 / ADR-096 — rvcsi nexmon (Pi 5 BCM43455c0 capture)
- `signal/src/ruvsense/pose_tracker.rs` — 17-kpt Kalman tracker reused for dead reckoning
- `ui/observatory/` — the vanilla-JS 3D viewer pattern this demo mirrors
