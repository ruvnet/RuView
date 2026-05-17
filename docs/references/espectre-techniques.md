# ESPectre (Francesco Pace) — Technique Reference

Source: *How I Turned My Wi-Fi Into a Motion Sensor — Part 2*
(Dec 2025), Medium / [francescopace/espectre](https://github.com/francescopace/espectre)
on GitHub, GPLv3.

Captures the three core techniques and the support tooling Pace
shipped. RuView has adopted some, partially adopted others, and not
adopted the rest. This doc is a living checklist — update when items
move.

## 1. Gain Lock (AGC + FFT scale)

The ESP32 PHY applies automatic gain control per packet. For normal
WiFi reception that keeps decoding optimal; for CSI sensing it
manifests as a 20-30 % slow drift in amplitude even in an empty
room, masking real body modulation. Two undocumented PHY routines
freeze the gain:

```c
extern void phy_fft_scale_force(bool force_en, int8_t force_value);
extern void phy_force_rx_gain(int force_en, int force_value);
```

Recipe:

1. After WiFi association, collect AGC and FFT gain values from
   each CSI packet.
2. At packet 300 (~3 s at 100 pps), take the **median** of each
   (more robust than mean against outliers).
3. Call the two PHY routines with the medians to lock the radio.
4. Safety branch: if median AGC < 30, skip the lock — forcing low
   gain freezes the RX path. Sensor must be moved further from AP.

Supported targets: ESP32-S3, ESP32-C3, ESP32-C5, ESP32-C6. Older
parts have no access to these PHY hooks.

**RuView status — DONE.** ADR-100 (commit `8aef8206`).
Implemented in `firmware/esp32-csi-node/main/csi_collector.c` as
`rv_gain_lock_process`. Boot log on both sensors:
`gain-lock APPLIED: AGC=42/44, FFT=-31/-42 (median of 300 packets)`.
Empty-room CV dropped from ~10 % (full broadband) to 3-4 % after
NBVI also kicked in.

## 2. NBVI — Normalized Baseline Variability Index

Per-subcarrier score that picks the K most useful subcarriers
automatically.

```
NBVI(k) = α · (σ_k / μ_k²) + (1 - α) · (σ_k / μ_k),    α = 0.5
```

* `σ_k / μ_k²` penalises weak subcarriers (low μ → high score → bad).
* `σ_k / μ_k`  is the standard coefficient of variation; rewards
  stability.
* α = 0.5 balances; pure σ/μ² picks stable-but-quiet bins, pure σ/μ
  picks loud-but-noisy bins.
* Amplitude-only (no phase) — phase has Temporal Phase Rotation
  artefacts that need extra calibration; amplitude is calibration-
  free.

Four-step pipeline at boot:

| Step | What | Detail |
|---|---|---|
| 1 | **Find quiet moments** | Slide a window across the calibration buffer, pick the windows with the lowest aggregate variance via percentile detection. Tolerates someone walking through during boot. |
| 2 | **Dead-zone gate** | Drop any subcarrier with mean amplitude below the 25th percentile across all subcarriers. Guard tones + null bins are excluded so they don't "win" σ/μ² → ∞. |
| 3 | **Rank + validate** | Sort by NBVI ascending. Run the motion detector on each candidate config, measure false-positive rate, take the config with the lowest FP. |
| 4 | **Pick winners** | Top-K by lowest NBVI (typically K = 12 for HT20). |

Memory: O(N) running with on-the-fly mean/variance updates ⇒ ≈ 256 B
for 64 subcarriers. Time: O(N · L) per recompute, milliseconds on a
$10 device.

**RuView status — PARTIALLY DONE.** ADR-102 (commit `2f12a223`).
Server-side port in `amp_presence_override` /
`nbvi_select_top_k`. What we have:

- ✅ NBVI formula with α = 0.5
- ✅ Top-12 selection
- ✅ Dead-zone gate (`NBVI_DEAD_GATE_PCT = 0.25`)
- ✅ Recompute throttled (`NBVI_REFRESH_TICKS = 200` ≈ every 5 s)

What we **do not** have:

- ❌ **Step 1 quiet-window finder** — we use the *whole* history
  buffer. If the buffer captures someone moving, ranking is biased.
  Pace's percentile-window detector should be added.
- ❌ **Step 3 FP-rate validation** — we accept the raw NBVI ranking
  without testing it on the calibration data.
- ❌ **Boot calibration sequence** (FW-side, 7 s post gain-lock).
  Ours is server-side rolling, which means selection drifts forever
  rather than locking after boot. Trade-off: adapts to room
  rearrangement, but never "settles".

Empirically on the operator's deployment NBVI alone gave a 1.5-2× CV
reduction:

| | Full 56 subc | NBVI top-12 |
|---|---|---|
| node 1 idle CV | 5.0 % | 3.1 % |
| node 2 idle CV | 7.0 % | 3.9 % |

## 3. Baseline-variance threshold normalization

Pace's third problem was that `threshold = 1.0` meant different
things on different devices. Fix:

```python
if baseline_variance > 0.25:
    scale = 0.25 / baseline_variance
else:
    scale = 1.0
```

Reference 0.25 is what a quiet room typically measures during NBVI
calibration. Apply the scale to the live motion score, so the user-
facing threshold (`= 1.0`) is universal across rooms.

**RuView status — NOT DONE.** Our `amp_node_level` uses fixed
thresholds tuned to one deployment (CV 10 % moving, CV 22 % active,
mean/baseline < 0.75 still). Other deployments will need re-tuning.

## 4. Two-phase boot calibration

```
PHASE 1: GAIN LOCK (3 s, 300 packets)
  Collect AGC/FFT → median → lock.
PHASE 2: NBVI CALIBRATION (7 s, 700 packets)
  With gain locked, rank subcarriers → pick top-K.
Total ≈ 10 s. Room must be mostly quiet during this window.
```

**RuView status — SPLIT.** Phase 1 is in FW (ADR-100). Phase 2 lives
in the server as a rolling refresh, not a boot-time fix-point. See
NBVI section above for the implications.

## 5. Persisted baseline / device threshold

After NBVI calibration, ESPectre writes the AGC/FFT lock values, the
chosen subcarrier set, the baseline variance, and the threshold into
NVS so reboots don't need re-calibration.

**RuView status — NOT DONE.** Each server restart triggers a fresh
60-second baseline learn. NBVI also re-ranks from scratch on restart.
Open item: persist `AMP_LATEST.baseline` to disk + load at startup.

## 6. Interactive Web Serial game (`espectre.dev/game`)

Browser ↔ ESP32 over USB Web Serial API. Shows live motion as a bar,
lets user tune `threshold` while playing a reaction game. Settings
persist via NVS.

**RuView status — NOT DONE.** Closest analogue is our `raw.html`
calibration console (per-node bars + RSSI trace), but it's read-only.

## 7. Native Home Assistant integration via ESPHome

Sensor exposes occupancy/motion entities directly to HA.

**RuView status — NOT DONE.** No HA integration path. Could be added
via MQTT or a custom ESPHome component.

## 8. Test suite

Pace ships 500+ unit tests, 90 % coverage, validated against a fixed
2000-packet capture (1000 idle + 1000 motion). CI runs PlatformIO,
pytest, ESPHome build, Codecov on every push.

**RuView status — PARTIAL.** Agent added 2 regression tests for the
binary CSI frame parser (`csi.rs:751`); no regression set captured
for the amplitude classifier or NBVI.

## Comparison summary (what RuView has, doesn't have, has differently)

| Item | Pace / ESPectre | RuView |
|---|---|---|
| Gain lock | FW, 300 pkt median, AGC+FFT, AGC<30 skip | ✅ Same, in `csi_collector.c` |
| NBVI formula | α·σ/μ² + (1-α)·σ/μ, α=0.5, top-12 | ✅ Same, server-side |
| Dead-zone gate | 25th percentile of mean | ✅ `NBVI_DEAD_GATE_PCT=0.25` |
| Quiet-window finder | Percentile-window in calibration buffer | ❌ Use full window verbatim |
| FP-rate validation of NBVI pick | Yes | ❌ Take raw ranking |
| Boot-time NBVI freeze | FW, ~7 s post-lock | ❌ Server-side rolling |
| Baseline variance normalization | `scale = 0.25 / σ²` | ❌ Fixed thresholds per deployment |
| NVS persistence of calibration | Yes | ❌ Fresh learn each restart |
| Universal threshold | One value across rooms | ❌ Re-tune per deployment |
| Calibration UI | Web Serial game | ❌ Read-only raw.html |
| HA integration | ESPHome native | ❌ None |
| Test suite | 500+ tests, 90 % coverage | ❌ ~2 parser tests only |
| Phase / amplitude | Amplitude only (TPR avoidance) | ✅ Same |
| Subcarrier count | 64 (HT20) | 56 (ESP32-S3 reports 56 non-guard) |

## Open items, ranked by expected impact on RuView

1. **Quiet-window finder for NBVI Step 1** — if the operator restarts
   the server while the room is occupied, current NBVI biases its
   ranking toward subcarriers stable on the *occupied* state. Bug:
   present_still then under-triggers. ~1 h.
2. **Persist `AMP_LATEST.baseline` to disk** — eliminates the
   "step outside for 60 s" ritual after every restart. ~30 min.
3. **Baseline variance normalization** — would let us ship one
   threshold set for any deployment. ~1 h.
4. **FP-rate validation of NBVI pick** — would catch the case where
   the top-12 ranked subcarriers happen to overlap with a noise
   source. ~1 h.
5. **Boot-time NBVI freeze** — if we want fully reproducible
   behaviour. Trade-off: doesn't adapt to room changes. ~2 h.
6. **HA / ESPHome integration** — depends on whether RuView wants
   to be a HA sensor or stay standalone. ~1 day.
7. **Web Serial calibration UI** — nice-to-have, lower priority than
   the algorithmic items. ~1 day.
