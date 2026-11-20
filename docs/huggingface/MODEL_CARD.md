---
license: mit
tags:
  - wifi-sensing
  - vital-signs
  - presence-detection
  - edge-ai
  - esp32
  - self-supervised
  - cognitum
  - through-wall
  - privacy-preserving
  - spiking-neural-network
  - ruvector
language:
  - en
library_name: onnxruntime
pipeline_tag: other
---

<!--
This file mirrors the README.md actually published at
https://huggingface.co/ruvnet/wifi-densepose-pretrained (issue #1481: the two
had drifted, and every filename in the old "Files in this repo" table pointed
at files that were never uploaded). Update this file whenever the Hub README
changes — `scripts/publish-huggingface.sh` uploads whatever is in
`dist/models/README.md`, which is a separate file from this one, so keeping
them in sync is a manual step until that's automated.

The "Using with the Rust sensing server" section below is not on the Hub page
— it documents the `--convert-model` / `--model` RVF conversion path
(issue #894, #1480) that HF users of this repo actually need and that the
upstream card doesn't cover.
-->

# RuView — WiFi Sensing Models

**Turn WiFi signals into spatial intelligence.** Detect people, measure breathing and heart rate, track movement, and monitor rooms — through walls, in the dark, with no cameras. Just radio physics.

## What This Does

WiFi signals bounce off people. When someone breathes, their chest moves the air, which subtly changes the WiFi signal. When they walk, the changes are bigger. This model learned to read those changes from a $9 ESP32 chip.

| What it senses | How well | Without |
|----------------|----------|---------|
| **Is someone there?** | presence detection (v1 "100%" retracted — single-class) | No camera needed |
| **Are they moving?** | Detects typing vs walking vs standing | No wearable needed |
| **Breathing rate** | 6-30 BPM, contactless | No chest strap |
| **Heart rate** | 40-120 BPM, through clothes | No smartwatch |
| **How many people?** | 1-4, via subcarrier graph analysis | No headcount camera |
| **Through walls** | Works through drywall, wood, fabric | No line of sight |
| **Sleep quality** | Deep/Light/REM/Awake classification | No mattress sensor |
| **Fall detection** | <2 second alert | No pendant |

## 🆕 v2 update — honest re-benchmark + properly-converged encoder (2026-05-31)

The v1 contrastive encoder shipped with a **flat training loss** (every epoch logged the
same `0.13517` — the optimizer was not actually learning), and its headline **"100% presence
accuracy" was measured on a single-class recording** (an overnight capture of one sleeping
person: **6,062 of 6,063** frames are labelled "present", 1 is "absent"). A constant
"yes" predictor scores 99.98% on that split — so the number is real but **says nothing about
generalization.** We are correcting that publicly rather than leaving it to stand.

**v2 retrains the same `8 -> 64 -> 128` encoder with a working InfoNCE objective** and reports
an **honest, label-free, time-disjoint metric**: held-out **temporal-triplet accuracy** =
P( d(anchor, temporal-positive) < d(anchor, temporal-negative) ), evaluated on the **last 20%
of the recording by time** (no leakage into training).

| Encoder | Held-out temporal-triplet accuracy | Notes |
|---------|-----------------------------------:|-------|
| Raw 8-dim features (no encoder) | 66.4% | baseline |
| Random-init encoder | 69.6% | untrained |
| **v2 trained encoder** | **82.3%** | **+15.9 pts over raw, properly converged** |

**Plain language:** the embedding now reliably places two CSI snapshots taken moments apart
*closer together* than two taken far apart — i.e. it has learned the temporal structure of the
radio environment, which is exactly what a useful self-supervised sensing embedding should do.
v1, with its flat loss, was barely better than random on this same test.

**Technical:** 2-layer FC (BatchNorm + GELU) -> L2-normalized 128-dim embedding, 9,280
params, trained with InfoNCE (temperature 0.1, in-batch + temporal-far negatives), AdamW, 60 epochs.
Temporal positives within 2 s; negatives >30 s apart. Time-disjoint 80/20 split.

**EWC (catastrophic-forgetting protection):** measured forgetting rate on the 4 protected
tasks is **32.97%** — real and non-trivial, disclosed here rather than only in the raw
`training-metrics.json`. Fisher information is computed and applied (protection strength
2000), but this is not "solved forgetting" — a user fine-tuning per-room LoRA adapters
should expect some drift on earlier tasks, not none.

## 🔧 v2.0.1 — fixed the published training-metrics.json itself (2026-09-09)

Separately from the v1→v2 accuracy retraction above, an unrelated bug was found and fixed
in the **export code** (`scripts/train-ruvllm.js`) that produces `config.json` and
`training-metrics.json` for this file set (`model.safetensors`, `model-q2/q4/q8.bin`,
`presence-head.json`, `node-1/2.json`) — separate from the `csi-embed-v2*` files, which
come from a different script.

The exported `lossHistory` and `contrastive.durationMs` were being read from a disconnected
baseline pass (`ContrastiveTrainer.train()`, which computes loss but never updates the
encoder) instead of from the loop that actually performs gradient updates on the encoder
weights. Concretely: `lossHistory` published a flat, non-decreasing curve around `0.1352`
for all 20 epochs, and `durationMs` reported ~12.4s — both measuring the wrong computation.

**This did not affect `finalLoss`/`improvement`/the 82.3% held-out temporal-triplet
accuracy above** — those were already computed from the real training loop and are
unchanged by this fix. It only affected the exported diagnostic curve and duration, which
anyone auditing convergence from the published JSON would have been misled by. Re-run
against the same real capture (`data/recordings/overnight-1775217646.csi.jsonl`) with the
fix applied: `lossHistory` now shows a genuine decreasing curve (0.1037 -> 0.0654 across 20
epochs) and `durationMs` reports the real ~40s gradient-loop time. `config.json`'s
`training.steps` field (`12212300` — epochs × triplets, arithmetically consistent but read
like optimizer-step count) is now reported as separate `epochs`/`tripletUpdatesPerEpoch`
fields instead.

Repro: `node scripts/train-ruvllm.js --data data/recordings/overnight-1775217646.csi.jsonl
--output <dir> --epochs 20` in [github.com/ruvnet/RuView](https://github.com/ruvnet/RuView)
(the `aether-arena/staging/train_csi_embed.py` path cited above is for the separate
`csi-embed-v2*` files and was not touched by this fix).

### v2 files & proof
| File | Size | Use |
|------|------|-----|
| `csi-embed-v2.safetensors` | ~40 KB | fp32 trained encoder |
| `csi-embed-v2-int4.bin` | **4.56 KB** | 4-bit packed encoder + fp16 standardizer — **fits the 8 KB ESP32 SRAM budget** |
| `csi-embed-v2.py` | <1 KB | `Enc` definition + loader |
| `csi-embed-v2-metrics.json` | — | full honest metrics + quantization scales |

- Encoder weights SHA-256: `3b37bca66e6050c50ccbc0f6e0501824f258bfdd8675dc0f4541b1e2e96feecd`
- Repro: `python aether-arena/staging/train_csi_embed.py` in [github.com/ruvnet/RuView](https://github.com/ruvnet/RuView)
- Trained on the same local capture (`data/recordings/overnight-1775217646.csi.jsonl`, 6,063 feature frames).

> **What v2 does *not* claim.** This is one room, one capture, two nodes. The triplet metric
> measures embedding quality, not downstream presence/vitals accuracy (which needs multi-class,
> multi-room labelled data we don't yet have for this 2.4 GHz feature). For *pose* SOTA on a
> public benchmark, see the separate 5 GHz model
> [`ruvnet/wifi-densepose-mmfi-pose`](https://huggingface.co/ruvnet/wifi-densepose-mmfi-pose)
> (82.69% torso-PCK@20 on MM-Fi).

## Benchmarks

Validated on real hardware (Apple M4 Pro + 2x ESP32-S3):

| Metric | Result | Context |
|--------|--------|---------|
| **CSI embedding quality** | **82.3% held-out** | Honest temporal-triplet metric; v1 single-class "100% presence" retracted (#882) |
| **Inference speed** | **0.008 ms** | 125,000x faster than real-time |
| **Throughput** | **164,183 emb/sec** | One laptop handles 1,600+ sensors |
| **Contrastive learning** | **51.6% improvement** | Trained on 8 hours of overnight data |
| **Model size** | **8 KB** (4-bit quantized) | Fits in ESP32 SRAM |
| **Training time** | **12 minutes** | On Mac Mini M4 Pro, no GPU needed |
| **Camera required** | **No** | Trained from 10 sensor signals |

## Models in This Repo

| File | Size | Use |
|------|------|-----|
| `model.safetensors` | 48 KB | Full contrastive encoder (128-dim embeddings) |
| `model-q4.bin` | 8 KB | **Recommended** — 4-bit quantized, 8x compression |
| `model-q2.bin` | 4 KB | Ultra-compact for ESP32 edge inference |
| `model-q8.bin` | 16 KB | High quality 8-bit |
| `presence-head.json` | 2.6 KB | Presence detection head (v1 "100%" retracted — single-class; #882) |
| `node-1.json` | 21 KB | LoRA adapter for room/node 1 |
| `node-2.json` | 21 KB | LoRA adapter for room/node 2 |
| `config.json` | 586 B | Model configuration |
| `training-metrics.json` | 3.1 KB | Loss curves and training history |

## Quick Start

```bash
# Download models
pip install huggingface_hub
huggingface-cli download ruv/ruview --local-dir models/

# Use with RuView sensing pipeline
git clone https://github.com/ruvnet/RuView.git
cd RuView

# Flash an ESP32-S3 ($9 on Amazon/AliExpress)
python -m esptool --chip esp32s3 --port COM9 --baud 460800 \
  write_flash 0x0 bootloader.bin 0x8000 partition-table.bin \
  0xf000 ota_data_initial.bin 0x20000 esp32-csi-node.bin

# Provision WiFi
python firmware/esp32-csi-node/provision.py --port COM9 \
  --ssid "YourWiFi" --password "secret" --target-ip YOUR_IP

# See what WiFi reveals about your room
node scripts/deep-scan.js --bind YOUR_IP --duration 10
```

## Using with the Rust sensing server (RVF conversion)

`model.safetensors` does not carry the `RVFS` binary-container magic that
`wifi-densepose-sensing-server`'s `--model` loader expects natively — it needs
converting first (issue #894). As of #1480, `--model` auto-detects and
converts `model.safetensors` / `model.rvf.jsonl` in-memory, so the one-liner
below is enough for most uses:

```bash
cargo run -p wifi-densepose-sensing-server -- --model model.safetensors
```

To pre-convert once and skip re-conversion on every startup (recommended for
repeated runs, or to inspect the converted container), use `--convert-model`:

```bash
cargo run -p wifi-densepose-sensing-server -- \
  --convert-model model.safetensors --convert-out model.rvf

cargo run -p wifi-densepose-sensing-server -- \
  --model model.rvf --load-rvf model.rvf
```

`--model` loads the weights for inference; `--load-rvf` separately populates
the container metadata that `/api/v1/model/info` reports — pass both if you
want that endpoint to reflect the loaded container.

Converting `model.safetensors` wires the format/load path (magic, version,
segments, weights all valid) but the pose-decoder *architecture* published on
HF differs from this crate's inference head, so the converted weights are not
claimed to reproduce pose accuracy end-to-end (tracked in #894). `model-q2/q4/q8.bin`
(quantized HF blobs) have no reader in this build yet — convert the
full-precision `model.safetensors` instead.

## Architecture

```
WiFi signals → ESP32-S3 ($9) → 8-dim features @ 1 Hz → Encoder → 128-dim embedding
                                                                    ↓
                                         ┌──────────────────────────┼──────────────────┐
                                         ↓                          ↓                  ↓
                                    Presence head            Activity head         Vitals head
                                    (v1 "100%" retracted)          (still/walk/talk)     (BR, HR)
```

The encoder converts 8 WiFi Channel State Information (CSI) features into a 128-dimensional embedding:

| Dim | Feature | What it captures |
|-----|---------|-----------------|
| 0 | Presence | How much the WiFi signal is disturbed |
| 1 | Motion | Rate of signal change (walking > typing > still) |
| 2 | Breathing | Chest movement modulates subcarrier phase at 6-30 BPM |
| 3 | Heart rate | Blood pulse creates micro-Doppler at 40-120 BPM |
| 4 | Phase variance | Signal quality — higher = more movement |
| 5 | Person count | Independent motion clusters via min-cut graph |
| 6 | Fall detected | Sudden phase acceleration followed by stillness |
| 7 | RSSI | Signal strength — indicates distance from sensor |

## Training Details

**No camera was used.** Trained using self-supervised contrastive learning:

- **Data**: 60,630 samples from 2 ESP32-S3 nodes over 8 hours
- **Method**: Triplet loss + InfoNCE (nearby frames = similar, distant = different)
- **Augmentation**: 10x via temporal interpolation, noise, cross-node blending
- **Supervision**: PIR sensor, BME280, RSSI triangulation, subcarrier asymmetry
- **Quantization**: TurboQuant 2/4/8-bit with <0.5% quality loss
- **Adaptation**: LoRA rank-4 per room, EWC to prevent forgetting

## 17 Sensing Applications

Built on these embeddings ([RuView](https://github.com/ruvnet/RuView)):

**Core:** Presence, person counting, RF scanning, SNN learning, CNN fingerprinting

**Health:** Sleep monitoring, apnea screening, stress detection, gait analysis

**Environment:** Room fingerprinting, material detection, device fingerprinting

**Multi-frequency:** RF tomography, passive radar, material classification, through-wall motion

## Hardware

| Component | Cost | Purpose |
|-----------|------|---------|
| ESP32-S3 (8MB) | ~$9 | WiFi CSI sensing |
| [Cognitum Seed](https://cognitum.one) (optional) | $131 | Persistent storage, kNN, witness chain, AI proxy |

## Limitations

- Room-specific (use LoRA adapters for new rooms)
- Camera-free pose: 2.5% PCK@20 (camera labels improve significantly)
- Health features are for screening only, not medical diagnosis
- Breathing/HR less accurate during active movement

## Citation

```bibtex
@software{ruview2026,
  title={RuView: WiFi Sensing with Self-Supervised Contrastive Learning},
  author={rUv},
  year={2026},
  url={https://github.com/ruvnet/RuView},
  note={Models: https://huggingface.co/ruv/ruview}
}
```

## Links

- **GitHub**: https://github.com/ruvnet/RuView
- **Cognitum Seed**: https://cognitum.one
- **RuVector**: https://github.com/ruvnet/ruvector
- **License**: MIT
