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

This first published build ships the **pipeline scaffold + signed binary**, not a fully trained model. The encoder weights are initialised from the published HF presence model, and the pose head is initialised from a small camera-supervised training run with limited paired samples — current PCK@20 is **0% on all 17 joints** (see [#640](https://github.com/ruvnet/RuView/issues/640) for the data + GPU plan to reach ≥35%).

The cog is therefore production-quality plumbing with an aspirationally accurate model. As more paired data lands and the libtorch GPU training run completes, the binary will be re-released without any cog-side changes.

## See also

- ADR-100: Cognitum Cog Packaging Specification.
- ADR-101: Pose Estimation Cog (the design behind this directory).
- ADR-079: Camera-supervised pose training pipeline.
- v0-appliance companion crate: `cognitum-pose-estimation` (Hailo HEF runtime).
