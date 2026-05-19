# Pose Estimation Cog

17-keypoint COCO pose estimation from WiFi CSI, deployed as a [Cognitum Cog](../../../../docs/adr/ADR-100-cog-packaging-specification.md).

## What it does

Subscribes to the local sensing-server's CSI stream, runs each window through a contrastive encoder (initialised from [`ruvnet/wifi-densepose-pretrained`](https://huggingface.co/ruvnet/wifi-densepose-pretrained)) and a 17-keypoint regression head, and emits one `pose.frame` event per inferred window on stdout. The appliance's cog-gateway picks up those events and routes them to the dashboard.

## Inputs

- `[56 subcarriers × 20 frames]` CSI windows (matches the `[56, 20]` shape produced by `scripts/align-ground-truth.js`).
- Sensing-server frame poll URL configured via `config.json` (`sensing_url`, default loopback).

## Outputs

```json
{"ts": 1779210883.444, "level": "info", "event": "pose.frame",
 "fields": {
   "tick": 12345,
   "n_persons": 1,
   "persons": [{"keypoints": [[0.48, 0.31], ...], "confidence": 0.81}]
 }}
```

## Status — v0.0.1

Pipeline scaffold + a first-cut trained model. The model is stored at `cog/artifacts/pose_v1.safetensors` (507 KB) and trained from `data/paired/wiflow-p7-1779210883.paired.jsonl` (1,077 samples, avg conf 0.44) using `candle-core 0.9` on an RTX 5080 — see the full training-result dump at `cog/artifacts/train_results.json`.

### Measured accuracy (validation set, 217 held-out samples)

```
                Overall:   PCK@20 = 3.0%   PCK@50 = 18.5%   MPJPE (normalized) = 0.0931

   Per-joint    PCK@20   PCK@50      Per-joint   PCK@20   PCK@50
   ─────────   ──────   ──────      ─────────   ──────   ──────
   nose          0.5%     5.1%      l_hip         0.0%    27.3%
   l_eye         2.8%     8.3%      r_hip        25.0%    76.9%   ← strongest signal
   r_eye         1.9%    15.7%      l_knee        2.3%    20.8%
   l_ear         0.0%     3.2%      r_knee        0.9%    35.2%
   r_ear         1.9%     9.7%      l_ankle       1.4%     7.9%
   l_shoulder    4.6%     8.8%      r_ankle       0.9%     9.3%
   r_shoulder    1.9%    19.9%      l_elbow       1.9%    26.4%
   l_wrist       3.2%    24.1%      r_elbow       0.0%     4.2%
   r_wrist       1.4%    12.0%
```

Loss curve: 0.181 (epoch 0) → 0.014 (epoch 399), eval loss 0.010. **400 epochs in 2.1 s** on the RTX 5080 (~5 ms/epoch full-batch).

### Honest reading

- The model **learns coarse body structure** — `r_hip` 77% PCK@50, `r_knee` 35%, `l_elbow` 26% all show real signal. PCK@50 = 18.5% averaged across joints is well above the random-baseline 0% that the pure-JS SPSA training produced.
- It is **below the ADR-079 target of PCK@20 ≥ 35%**. The bottleneck is data quality and quantity, not infra. The single 30-min seated-at-desk recording produced 1,077 paired samples at avg confidence 0.44 — strong asymmetry between left/right side (r_hip 77% vs l_hip 27%) reflects the camera framing more than any model defect.
- Distal joints (wrists, ankles) and face joints are still near-random: 56-subcarrier CSI at our 20-frame window doesn't carry enough fine-grained spatial information.

### Next-iteration plan (tracked in [#640](https://github.com/ruvnet/RuView/issues/640))

- Multi-session, multi-room recordings with **full-body framing** (target ≥ 30K paired samples at conf ≥ 0.7).
- Re-train with the same Candle pipeline (already validated to converge in seconds on RTX 5080).
- Hailo HEF export via the Dataflow Compiler on a self-hosted runner.

The cog's runtime inference path is currently a centred-skeleton stub returning `confidence=0`. Wiring the `pose_v1.safetensors` weights into `src/inference.rs` is the next code change — separate PR.

## See also

- ADR-100: Cognitum Cog Packaging Specification.
- ADR-101: Pose Estimation Cog (the design behind this directory).
- ADR-079: Camera-supervised pose training pipeline.
- v0-appliance companion crate: `cognitum-pose-estimation` (Hailo HEF runtime).
