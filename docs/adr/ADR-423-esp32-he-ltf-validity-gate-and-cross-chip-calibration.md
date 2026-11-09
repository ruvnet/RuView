# ADR-423: ESP32 HE-LTF CSI — validity gate, IDF lane, and cross-chip calibration

- **Status**: Proposed
- **Date**: 2026-11-09
- **Deciders**: rUv; RuView maintainers
- **Tags**: firmware, esp32-c6, esp32-c5, wifi-6, he-ltf, csi-quality, calibration, honesty
- **Related**: ADR-018 (CSI frame), ADR-110 (ESP32-C6 extension, WITNESS-LOG-110 §B1),
  ADR-135 / ADR-301 / ADR-357 (calibration), ADR-345 (C6 256-bin edge path),
  ADR-356 (rate-truthful CSI), PR #2485 (per-frame grid bounds)

## Context

A review of current ESP32 CSI work produced three proposed directions for RuView:

1. Move C6 nodes to ESP-IDF ≥ 5.5 behind an 802.11ax AP to get 256-bin HE-LTF CSI.
2. Calibrate the mixed S3/C3/C6 fleet in the style of OpenCSI.
3. Claim presence, vitals and activity, not pose.

Before deciding, we tested direction 1 on our own hardware. The result changes
the decision: the IDF version turned out not to matter on this bench, and a
per-frame validity flag that RuView ignores turned out to matter a great deal.

### Evidence from the literature (graded)

| Finding | Grade | Source |
|---|---|---|
| `wifi_csi_acquire_config_t` selects HE-LTF per PPDU type (`acquire_csi_su/mu/dcm/beamformed`, `acquire_csi_he_stbc_mode`, `val_scale_cfg`) | MEASURED (header) | esp-idf `esp_wifi_he_types.h` |
| On C6, invalid CSI can reach the callback; filter on `rx_ctrl.rx_channel_estimate_info_vld`. Maintainer says the C6 CSI docs are inaccurate | CLAIMED (maintainer, open issue) | esp-idf #14271 |
| HE20 HE-LTF covers subcarriers −122..122 | CLAIMED (docs; C5 v6.0) | Espressif C5 vendor-features guide |
| IDF v5.4 downconverts C6 CSI to 64-bin HT on an HE link; v5.5.2 gives 256-bin HE | MEASURED by two external/internal rigs, **contradicted below** | ADR-110, WITNESS-LOG-110 §B1 |
| Per-link Z-score normalisation using the link's own quiet-period std, learned from an empty-room bootstrap, gives zero-shot presence F1 up to 0.99 across S3/C3/C6 and three rooms; presence only, cannot separate static from moving | CLAIMED (preprint, not reproduced) | OpenCSI, arXiv 2607.26665; data CC BY 4.0, Zenodo 19861032 |
| Receiver AGC and fixed per-subcarrier non-linearity dominate cross-device error; gain alignment recovers up to 75% of lost cross-device HAR accuracy | CLAIMED (peer-reviewed, IEEE IoT-J 2026; chips not confirmed) | arXiv 2605.26836 |
| Espressif ships AGC/FFT gain compensation and gain locking for S3/C3/C5/C6 | CLAIMED (vendor component) | `esp_csi_gain_ctrl` 0.1.5, Apache-2.0 |
| In a 9-node, 72-link C3 home study, one well-placed link beat the full mesh (AUC 0.541 vs 0.489), but both are near chance | MEASURED (preprint) | arXiv 2602.10823 |
| ESP32 amplitude CSI cannot separate multiple people's gait (39–56% accuracy, 97–99% feature overlap) | MEASURED (preprint) | arXiv 2601.02177 |
| No ESP32 pose result was found with a mean-pose baseline and a leakage-free held-out split | absence of evidence | literature search, 2026-11-09 |

### Bench measurement (MEASURED, 2026-11-09)

**Setup**
- Board: ESP32-C6 (MAC `20:6e:f1:17:0e:34`, 8 MB flash, CP210x UART, COM11).
- AP: home 11ax 2.4 GHz mesh `ruv.net`, channel 10, HE20.
- Association: `phymode(0x7, 11ax) … he:1` on every run. Two mesh BSSIDs were seen across runs.
- Firmware: `esp32-csi-node` at PR #2485 head `9c277f127`, C6 4 MB overlay, plus the scratch diagnostic in
  [`ADR-423/he-quality-diag.patch`](ADR-423/he-quality-diag.patch) and the UART-console
  overlay [`ADR-423/sdkconfig.diag.overlay`](ADR-423/sdkconfig.diag.overlay).
- Only the app slot (`ota_0`) was flashed; NVS was untouched.
- Every run reported 0 panics, 0 watchdog resets and 0 assert failures.

**A. IDF version A/B.** Same board, AP, source and diagnostic; only the Docker IDF image
changes. Each run is about 150 s.

| IDF | app SHA-256 (prefix) | HE-SU frames (`cur_bb_format=4`) | HE-SU length | HE-SU flagged invalid |
|---|---|---|---|---|
| v5.4 | `3887cf07` | 5,447 / 6,000 = **90.8%** | 512 B (256 bins) | 4,886 / 5,447 = **89.7%** |
| v5.5.2 | `4ae9fff3` | 4,171 / 5,000 = **83.4%** | 512 B (256 bins) | 3,219 / 4,171 = **77.2%** |

On the same runs, HT frames (`bb=2`, 128–256 B) were 0–2% invalid. Legacy frames
(`bb=1`, 128 B) were 44–51% invalid.

**B. Is the flag meaningful?** IDF v5.5.2, 180 s. For each class, every frame is compared
with the previous frame of the same class *from the same transmitter MAC*, using
true magnitude |z| after per-frame gain normalisation. The statistic is the L1
distance between normalised distributions (0–2000‰).

| Class | Frames | Same-MAC pairs | Zero bins / frame | Frame-to-frame shape change |
|---|---|---|---|---|
| HE-SU 256-bin, `vld=1` | 1,021 | 551 | 10 | **3.2%** |
| HT 64-bin, `vld=1` | 266 | 92 | 8 | 5.9% |
| HE-SU 256-bin, `vld=0` | 4,265 | 2,963 | 12 | **51.6%** |

An earlier pass that used |I|+|Q| on AP-only HE frames gave 51% raw change against
53% gain-normalised. That rules out AGC steps as the explanation for the `vld=0`
instability.

**What this shows**
- On this bench, IDF v5.4 already delivers 256-bin HE-SU CSI. ADR-110's
  "v5.4 downconverts" result did not reproduce here; it may depend on the AP or the
  driver blob. ADR-110 §B1 now has a contrary data point.
- 512 B of CSI plus ADR-018's 20-byte header is the 532 B frame ADR-110 reports.
  The "245 subcarriers / 490 B" doc figure and the 256-bin buffer are not in conflict:
  the buffer includes 13–14 null and guard bins, which matches the zero bins measured above.
- **About 77–90% of HE-SU frames are flagged as invalid estimates, and those frames are
  about 16× less stable than valid ones.** Valid HE frames are the most stable class we
  measured, at about half the frame-to-frame change of HT, with four times the bins.
- Today `csi_collector.c` ignores `rx_channel_estimate_info_vld`. Every invalid HE
  frame goes to edge DSP (presence, vitals, top-K selection) and onto the network.

**Limits**
- One board, one AP family, one room, about 10 minutes of capture in total.
- The vld=0 and vld=1 HE populations may come mostly from different transmitters. The
  same-MAC pairing controls for that within each class, but not between classes.
- The flag's exact semantics are undocumented. We treat it as an empirical quality
  signal, not a specification.
- None of this is a sensing-accuracy result.

## Decision

### D1 — Gate HE-capable targets on the validity flag (firmware)

On `CONFIG_SOC_WIFI_HE_SUPPORT` targets (C5, C6), `wifi_csi_callback` reads
`info->rx_ctrl.rx_channel_estimate_info_vld`:

- **Edge DSP never sees `vld=0` frames.** They are counted and dropped before
  `edge_enqueue_csi` and before presence, vitals and top-K selection.
- **Streaming is configurable.**
  - New Kconfig `CSI_HE_INVALID_POLICY`: `drop` (default), `tag` or `pass`.
  - `tag` sets ADR-018 byte 19 bit 5 ("estimate invalid") so host tools can study the frames.
  - `pass` is today's behaviour, for A/B work.
- **Counters are reported.** Valid and invalid counts per PPDU class are exported in the
  existing health/sync telemetry, so the yield cost is visible. ADR-356 already requires
  rate-truthful reporting.
- **Pre-HE targets are unaffected.** S3 and C3 have no such field, so nothing changes there.

**Implemented outcome (MEASURED, 2026-11-09):**
- **Setup:** `main/csi_he_gate.h` plus the `csi_collector.c` wiring, built on the release
  lane (`espressif/idf:v5.4`, C6, app SHA-256 prefix `2889894f`). Default `drop` policy,
  same board and AP as §B, 150 s, 0 faults.
- **Drop rate:** the gate counted 6,650 valid and 24,063 invalid HE frames, and dropped
  **78.3%** of them before the rate gate.
- **Yield:** the post-gate callback yield was **36–42 pps**. That matches the pre-D1 runs
  (36–45 pps), because the 50 Hz rate gate already discarded most callbacks. Valid frames now
  fill the slots invalid ones used to take.
- **Correction:** the earlier extrapolation that "yield falls 75–85%" was wrong for this
  configuration. The cost would only appear where valid frames alone can't fill the
  rate-gate budget.

The host decision table, `test/test_csi_he_gate.c`, is part of `make host_tests`.

**Acceptance:**
1. A host test on `test/stubs` must cover the gate.
2. A hardware run with the ADR-423 diagnostic must show zero `vld=0` frames reaching
   `process_frame`.
3. A before/after presence-variance comparison on an empty room must be recorded as MEASURED.

### D2 — Do not migrate the C6 lane for HE-LTF alone; pin the evidence instead

The C6 release lane in `firmware-ci.yml` stays on `espressif/idf:v5.4` for now; C5 is
already on v6.0.2. The bench gave no HE-LTF reason to move it.

Any future IDF bump for C6 must re-run the A/B in Section A and record the result. Moving
C6 to v6.0.2 to share C5's lane is allowed, on its own merits.

ADR-110's version-gating claim is downgraded to "AP- or driver-dependent; contradicted
on one bench (ADR-423)" when ADR-110 is next revised.

### D3 — Cross-chip calibration: OpenCSI-style per-link normalisation, presence only

The host signal path adds a per-link calibration stage, keyed by `(node_id, peer MAC,
PPDU class, bin count)`:

- **Statistics:** a quiet-period bootstrap learns each link's temporal standard deviation.
  The output is a dimensionless per-link Z-score, extending ADR-135's empty-room baseline.
- **Maturity tag:** links with an incomplete or stale bootstrap are marked immature, and
  presence logic abstains on them rather than guessing.
- **Scope:** the stage feeds presence only. Its own authors say it removes the absolute
  magnitude that motion and vitals need.
- **Mixed classes:** HE-256, HT-64 and legacy streams from the same node are separate links.
  They are never mixed in one normalisation (see PR #2485 for the firmware-side grid bounds).
- **AGC alignment:** `esp_csi_gain_ctrl`-style compensation, or gain locking, is evaluated
  separately as a candidate for activity and vitals features. It is not adopted by this ADR.

**Acceptance:**
1. Reproduce OpenCSI's zero-shot presence result on its public dataset (Zenodo
   19861032, CC BY 4.0) before claiming anything.
2. Then run it on RuView's own S3, C3 and C6 nodes with a held-out room, following
   [`ADR-423/presence-benchmark-protocol.md`](ADR-423/presence-benchmark-protocol.md)
   (radar-gated labels, leave-one-room-out threshold, majority-class baseline).

### D4 — Claims policy for ESP32 CSI

This tightens the repository's existing rule; it doesn't replace it.

- Public statements about ESP32 nodes cover presence, coarse motion, breathing-band vitals and
  activity, each tagged MEASURED (with a reproducer), CLAIMED or SYNTHETIC.
- Multi-person identification and pose are not claimed for ESP32 nodes.
- Any pose figure must report the mean-pose baseline and a leakage-free held-out split.
- HE-LTF bin count (256) is a raw-signal property. It must not be presented as an accuracy gain.

## Consequences

- **Positive:** the C6's best CSI class (valid HE-SU) becomes the input to edge DSP,
  in place of a mix that is mostly unstable estimates. The IDF question is settled for
  this bench with data, not by assumption.
- **Negative:** C6 effective CSI yield falls substantially in environments like this
  one. Adaptive-rate logic (ADR-356) and the per-node rate budget must use the post-gate rate.
- **Risk:** the flag's semantics may change between driver blobs. The diagnostic patch is
  kept as a reproducer, so each IDF bump can re-check it.
- **Neutral:** nothing changes for S3 or C3 nodes or for existing ADR-018 readers. Bit 5 of
  byte 19 is only set in `tag` mode.

## Validation plan (reproducer)

From a clone, against an ESP32-C6 on an 11ax AP:

```bash
# --3way: the patch was cut before D1 landed. The overlay sets
# CSI_HE_INVALID_POLICY_PASS so vld=0 frames still reach the diagnostic.
git apply --3way docs/adr/ADR-423/he-quality-diag.patch
cp docs/adr/ADR-423/sdkconfig.diag.overlay firmware/esp32-csi-node/
MSYS_NO_PATHCONV=1 docker run --rm -v "$(pwd)/firmware/esp32-csi-node:/project" -w /project \
  espressif/idf:v5.5.2 bash -c "cat sdkconfig.defaults sdkconfig.defaults.esp32c6 \
  sdkconfig.diag.overlay > sdkconfig.defaults.build && \
  SDKCONFIG_DEFAULTS=sdkconfig.defaults.build idf.py set-target esp32c6 && idf.py build"
# Back up otadata + the active app slot first (RUNBOOK §3), then flash the app only:
python -m esptool --chip esp32c6 -p <PORT> -b 460800 write_flash --flash_size keep \
  0x20000 firmware/esp32-csi-node/build/esp32-csi-node.bin
# Read UART at 115200 for 3 minutes; compare the HEDIAG and HEQ lines with Section B.
```

The diagnostic is not production code: it computes `sqrtf` over 256 bins in the Wi-Fi
callback. It is published only as the reproducer for this ADR.

## Open questions

1. Does `vld=0` correlate with a specific transmitter, MCS, RU allocation or the
   beamformed flag? The next diagnostic pass should log `info->mac` and `rx_ctrl` rate fields.
2. On C5 at 5 GHz, are the invalid fraction and stability gap the same?
3. Does `acquire_csi_force_lltf` give stable 64-bin L-LTF for frames whose HE estimate is invalid?
   That would be a fallback that recovers yield.
