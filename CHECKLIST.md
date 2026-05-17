# RuView · Implementation Checklist

Single source of truth for what's shipped and what's open. Updated
at the end of every session. Pair with
[`docs/references/espectre-gap-analysis.md`](docs/references/espectre-gap-analysis.md)
for the technical detail behind each line.

Last sweep: **2026-05-17**, branch `feat/ota-rssi-mobile`, head `e4204595`.

---

## ✅ Done

### Server (`v2/crates/wifi-densepose-sensing-server`)

- [x] **ADR-100** PHY gain-lock (AGC + FFT freeze, ESPectre port) — FW
- [x] **ADR-101** Raw-amplitude classifier (CV + baseline drop, hysteresis)
- [x] **ADR-101** Per-node classification badges in WS payload
- [x] **ADR-102** NBVI subcarrier selection (formula α=0.5, top-12)
- [x] **ADR-102** NBVI Step 1 quiet-window finder
- [x] **ADR-103** Persistent baseline at `data/baseline.json` (FULL broadband)
- [x] **ADR-103** Universal threshold via baseline-CV normalization
- [x] **ADR-104** Per-subcarrier drift channel (off-axis presence)
- [x] **ADR-104** NBVI Step 3 FP-rate validation (K ∈ {6,8,10,12,16,20})
- [x] **ADR-105** Drop all synthetic data from runtime
  ([signal_field, pose_keypoints, persons, fake confidence — all gated)
- [x] **ADR-106** Full complex CSI in WS (`amplitude` + `phases` + meta)
- [x] **ADR-106** Built-in CSI keepalive (managed `ping` per sensor)
- [x] **ADR-106** Server-side µs `timestamp_us`
- [x] **ADR-107** `POST /api/v1/baseline/calibrate` + UI button
- [x] **ADR-107** Auto-recalibrate on long-quiet periods (30 min default)
- [x] **ADR-107** `GET /api/v1/baseline` (status + cooldown)

### Firmware (`firmware/esp32-csi-node`)

- [x] **ADR-100** Gain-lock (300-packet median, MIN_SAFE_AGC=30 safety)
- [x] **ADR-106** Sensor µs timestamp in CSI trailer (`rx_ctrl.timestamp`)
- [x] **ADR-108** NVS persistence of gain-lock — reboot ready in ~0.5 s
- [x] (parallel agent) RSSI carry-through via feature_state header fix
- [x] (parallel agent) OTA: `OTA_SIZE_UNKNOWN`, httpd stack_size=8192,
      reset-reason log — all three FW prerequisites for working OTA

### Ops / tooling

- [x] `scripts/ota-deploy.sh` — WiFi OTA flash + auto-discovery + verify
- [x] `scripts/record-baseline.py` — headless baseline capture (CLI)
- [x] `data/baseline.json` v2 schema
- [x] `docs/references/ota-pipeline.md` — verbatim OTA recipe (port 8032)

### Documentation

- [x] **ADR-100..108** all written, each ≤ 200 lines
- [x] `docs/references/espectre-techniques.md` — Pace technique catalogue
- [x] `docs/references/espectre-gap-analysis.md` — section-by-section gap
- [x] Documentation actualization sweep — every Open Items section
      cross-checked against actual implementation state

---

## ⏳ Open, priority-sorted

### High value, low effort

- [ ] **Per-sub delta sparkline in `raw.html`** — operator sees off-axis
      drift channel firing in real time. ~30 min. (ADR-104 open)
- [ ] **`POST /ota/recalibrate`** — clear gain-lock NVS via REST,
      no USB needed. ~30 min FW + OTA. (ADR-108 open)
- [ ] **Track AP MAC in NVS alongside gain-lock** — auto-invalidate
      stale values on AP swap. ~1 h FW + OTA. (ADR-108 open)
- [ ] **Per-subcarrier baseline AGE check** — flag for re-calibration
      when channel slowly drifts. ~1 h. (ADR-104 open)
- [ ] **Tailscale-target in NVS** — sensor stream keeps working when
      Mac roams networks. ~30 min provision + reflash. (ADR-100 open)
- [ ] **`n_aps_used` field in `enhanced_*`** — let consumers know
      when multi-AP pipeline ran on a single sensor. ~30 min. (ADR-105 open)

### High value, medium effort

- [ ] **HA via MQTT** — sensor as HA entity (`binary_sensor.motion`).
      Wide ecosystem reach. ~1 day.
- [ ] **2 000-packet fixed-replay test suite** — regression protection
      over classifier + NBVI. Pace's pattern (1 000 idle + 1 000 motion).
      ~1 day.
- [ ] **Multi-AP `signal_field` via `MultistaticFuser`** — replaces
      zero-filled grid (ADR-105 D6) with physically real spatial map.
      ~2-3 h.
- [ ] **Phase-domain drift** — phase delta vs baseline phase, picks up
      sub-mm chest-wall motion for vital signs. Requires phase baseline
      in `baseline.json`. ~1 h script + ~30 min server. (ADR-104 open)
- [ ] **Hide pose canvas in Docker SPA when `model_loaded == false`**
      — stop the upstream UI from rendering empty skeletons.
      ~15 min UI patch. (ADR-105 open)

### Bigger, lower urgency

- [ ] **ESPHome native component** — tighter HA than MQTT bridge. 2-3 days.
- [ ] **Web Serial calibration game** — playful threshold tuning. 1 day.
- [ ] **Boot-time NBVI freeze in FW** — only if FP issues in real homes.
      Trade-off: doesn't adapt to channel changes. 2 h. (ADR-102 open)
- [ ] **Per-channel NVS cache for gain-lock** — needed only if channel
      hopping (ADR-029) is reactivated. 1 h. (ADR-108 open)
- [ ] **DensePose model train + load** — unlock 17-keypoint pose;
      needs dataset (MM-Fi or Wi-Pose) + training run. 1-3 days.
- [ ] **AETHER contrastive pretrain on live data** — code path exists
      via `--pretrain`. Self-supervised, no labels. 2-3 h to set up +
      hours of training time.
- [ ] **MERIDIAN domain generalization** — code present (parent
      project), not loaded. Cross-room transfer. 1 day to integrate.
- [ ] **Channel hopping (ADR-029)** — scaffold in FW, deactivated.
      Frequency diversity for anomaly detection. 2-3 h.
- [ ] **Multi-antenna support (`n_antennas` > 1)** — currently hard-
      coded to 1 in `csi_collector.c`. ESP32-S3 typically single-
      antenna so low value unless we ship on C6/MIMO. 1 h.
- [ ] **Multiple baseline profiles** (day/night/season). 2 h.
- [ ] **Progress bar in calibrate button** instead of text pill. 15 min.

### One-time hygiene

- [ ] **README.md** is 542 lines — review for current relevance, trim.
- [ ] **CLAUDE.md** is 407 lines — same.
- [ ] **Re-record `data/baseline.json`** via the new UI calibrate button
      so `per_subcarrier_mean` field is populated and ADR-104 drift
      channel activates. ~2 min operator time.

---

## Reference

| Doc | Purpose |
|---|---|
| [`docs/adr/`](docs/adr) | All ADRs 001-108; 100-108 are this session |
| [`docs/references/espectre-techniques.md`](docs/references/espectre-techniques.md) | Pace technique catalogue + RuView adoption |
| [`docs/references/espectre-gap-analysis.md`](docs/references/espectre-gap-analysis.md) | Section-by-section gap with priority table |
| [`docs/references/ota-pipeline.md`](docs/references/ota-pipeline.md) | OTA recipe — port 8032, three FW prereqs |

To mark an item done: tick the box, add `(ADR-XXX, commit-sha)` after
the line, move it from the priority section to the top "Done" section.
