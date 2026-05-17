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

Memory: O(N) running with on-the-fly mean/variance, ≈ 256 B for 64
subcarriers. Time: O(N · L) per recompute, ms on a $10 device.

**RuView status — DONE (Steps 1, 2, 4).** ADR-102 (commits
`2f12a223` + `f4119924`). Server-side, see ADR-102 for detail.
Missing: ❌ Step 3 FP-rate validation, ❌ FW-side boot freeze.

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

**RuView status — DONE.** ADR-103 D3 (commit `2f4b2d53`).
`amp_node_level` and `amp_classify_from_latest` divide live CV by
`baseline_cv` loaded from `data/baseline.json` and gate at universal
`3×` (moving) / `6×` (active). Falls back to absolute gates
`0.10 / 0.22` when no calibration loaded — backwards compatible.

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

**RuView status — DONE for baseline; PARTIAL for the rest.** ADR-103
(commits `f4119924`, `2f4b2d53`). `data/baseline.json` persists per-
node full-broadband mean/p95/CV + per-subcarrier means, loaded at
server boot via `load_baseline_file`. Gain lock + NBVI selection
still happen fresh on each FW boot / server boot respectively (open
items 4 + 5 below).

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
| Gain lock | FW, 300 pkt median, AGC+FFT, AGC<30 skip | ✅ ADR-100 |
| NBVI formula α=0.5, top-12, dead-zone gate | ✅ | ✅ ADR-102 |
| Quiet-window finder (Step 1) | ✅ | ✅ ADR-102 v2 |
| FP-rate validation (Step 3) | ✅ | ❌ raw ranking |
| Boot-time NBVI freeze | FW, ~7 s post-lock | ❌ server-side rolling |
| Baseline variance normalization (universal threshold) | ✅ | ✅ ADR-103 D3 |
| Persisted baseline to disk | NVS | ✅ ADR-103 D1 (`data/baseline.json`) |
| NVS persistence of FW calibration | ✅ | ❌ fresh each FW boot |
| Calibration UI | Web Serial game | ❌ read-only `raw.html` |
| HA / ESPHome integration | ✅ | ❌ none |
| Test suite | 500+ tests, 90 % cov | ❌ 2 parser tests |
| Phase / amplitude | amplitude only | ✅ same |

## Open items, ranked by expected impact on RuView

1. **REST `POST /api/v1/baseline/calibrate`** — drives the recording
   script from a button in `raw.html` instead of CLI. ~30 min.
2. **FP-rate validation of NBVI pick** — defense against the top-12
   accidentally overlapping a noise source. ~1 h.
3. **Per-subcarrier baseline comparison (ADR-104 draft)** — uses the
   already-saved `per_subcarrier_mean` in `baseline.json` for L2
   distance instead of broadband mean ratio. Better off-axis
   presence sensing. ~1 h.
4. **Auto-recalibrate on long quiet periods** — if classifier sees
   `absent` with low variance for 30 min, refresh baseline in
   background. Eliminates manual script step entirely. ~1 h.
5. **FW-side NBVI boot-freeze + NVS persistence** — full
   reproducibility, sub-second post-boot ready. Trade-off: doesn't
   adapt to room changes. ~2 h.
6. **HA / ESPHome integration** — sensor as HA entity. ~1 day.
7. **Test suite vs fixed 2 000-packet replay** — regression
   protection for the classifier + NBVI. ~1 day.
8. **Web Serial calibration game** — nice-to-have. ~1 day.
