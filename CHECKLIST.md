# RuView · Implementation Checklist

Single source of truth for what's shipped and what's open. Updated
at the end of every session. Pair with
[`docs/references/espectre-gap-analysis.md`](docs/references/espectre-gap-analysis.md)
for the technical detail behind each line.

Last sweep: **2026-05-17**, branch `feat/ota-rssi-mobile`, head `eec3ca6c`.

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
- [x] **ADR-104** Per-sub drift exposed in WS `node_features[].drift_score`
      + raw.html sparkline per node (commit eec3ca6c)
- [x] **ADR-104** Baseline staleness watch — warn when on-disk baseline
      > 4 h old AND drift consistently fires during `absent` periods
      (commit eec3ca6c)
- [x] **ADR-105** Drop all synthetic data from runtime
  ([signal_field, pose_keypoints, persons, fake confidence — all gated)
- [x] **ADR-105** `n_aps_used: u8` uniform field on `enhanced_motion` +
      `enhanced_breathing` (commit 598a4b2f)
- [x] **ADR-106** Full complex CSI in WS (`amplitude` + `phases` + meta)
- [x] **ADR-106** Built-in CSI keepalive (managed `ping` per sensor)
- [x] **ADR-106** Server-side µs `timestamp_us`
- [x] **ADR-107** `POST /api/v1/baseline/calibrate` + UI button
- [x] **ADR-107** Auto-recalibrate on long-quiet periods (30 min default)
- [x] **ADR-107** `GET /api/v1/baseline` (status + cooldown)
- [x] **ADR-107** Progress bar in raw.html calibrate button
      (commit 432753e1)
- [x] **ADR-112** Multi-AP `signal_field` via `MultistaticFuser` —
      coverage × activity heatmap, non-zero only with ≥2 nodes +
      positions; preserves ADR-105 zero-grid otherwise (commit c8ac60f6)
- [x] **ADR-105** Hide pose canvas in Docker SPA when
      `model_loaded == false` + "no trained model" overlay
      (commit 2dcb30a6)

### Firmware (`firmware/esp32-csi-node`)

- [x] **ADR-100** Gain-lock (300-packet median, MIN_SAFE_AGC=30 safety)
- [x] **ADR-106** Sensor µs timestamp in CSI trailer (`rx_ctrl.timestamp`)
- [x] **ADR-108** NVS persistence of gain-lock — reboot ready in ~0.5 s
- [x] **ADR-109** `POST /ota/recalibrate` — clear gain-lock NVS via REST,
      no USB needed (commit f92807cd)
- [x] **ADR-109** Track AP MAC in `gl_ap_mac` NVS — auto-invalidate
      stale gain-lock on AP swap (commit f92807cd)
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

- [ ] **Tailscale-target in NVS** — sensor stream keeps working when
      Mac roams networks. ~30 min provision + reflash. (ADR-100 open)
      Deferred — Mac is stable on TP-Link, low ROI this session.

### High value, medium effort

- [ ] **2 000-packet fixed-replay test suite** — regression protection
      over classifier + NBVI. Pace's pattern (1 000 idle + 1 000 motion).
      ~1 day.
- [ ] **Phase-domain drift** — phase delta vs baseline phase, picks up
      sub-mm chest-wall motion for vital signs. Requires phase baseline
      in `baseline.json`. ~1 h script + ~30 min server. (ADR-104 open)

### Bigger, lower urgency (still active)

- [ ] **Multiple baseline profiles** (day/night/season). 2 h. — ADR-113
      target this session.

### One-time hygiene

- [ ] **Re-record `data/baseline.json`** via the new UI calibrate button
      so `per_subcarrier_mean` field is populated and ADR-104 drift
      channel activates. ~2 min operator time.

### Deferred — out of session scope

Marked here so future sessions don't re-litigate; each line carries
an explicit reason. Bring them back only if scope changes.

- **HA via MQTT** — new integration. Excluded by current session brief
  (no new integrations on current hardware).
- **ESPHome native component** — same reason as HA/MQTT.
- **Web Serial calibration game** — explicitly excluded.
- **Boot-time NBVI freeze in FW** — explicitly excluded.
- **Per-channel NVS cache for gain-lock** — explicitly excluded; only
  matters if channel hopping is reactivated, which is also excluded.
- **DensePose model train + load** — explicitly excluded.
- **AETHER contrastive pretrain on live data** — explicitly excluded.
- **MERIDIAN domain generalization** — explicitly excluded.
- **Channel hopping (ADR-029)** — explicitly excluded.
- **Multi-antenna support (`n_antennas` > 1)** — explicitly excluded.
- **README.md trim (542 lines)** — explicitly excluded.
- **CLAUDE.md trim (407 lines)** — explicitly excluded.
- **Tailscale-target in NVS** — Mac stable on TP-Link this session,
  low ROI. Not blocking.

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
