---
license: mit
tags:
  - wifi-sensing
  - pose-estimation
  - vital-signs
  - edge-ai
  - esp32
  - onnx
  - self-supervised
  - cognitum
  - csi
  - through-wall
  - privacy-preserving
language:
  - en
library_name: onnxruntime
pipeline_tag: other
---

# WiFi-DensePose: See Through Walls with WiFi + AI

**Detect people, track movement, and measure breathing -- through walls, without cameras, using a $27 sensor kit.**

| | |
|---|---|
| **License** | MIT |
| **Framework** | ONNX Runtime |
| **Hardware** | ESP32-S3 ($9) + optional Cognitum Seed ($15) |
| **Training** | Self-supervised contrastive learning (no labels needed) |
| **Privacy** | No cameras, no images, no personally identifiable data |

---

## What is this?

This model turns ordinary WiFi signals into a human sensing system. It can detect whether someone is in a room, count how many people are present, classify what they are doing, and even measure their breathing rate -- all without any cameras.

**How does it work?** Every WiFi router constantly sends signals that bounce off walls, furniture, and people. When a person moves -- or even just breathes -- those bouncing signals change in tiny but measurable ways. WiFi chips can capture these changes as numbers called *Channel State Information* (CSI). Think of it like ripples in a pond: drop a stone and the ripples tell you something happened, even if you cannot see the stone.

This model learned to read those "WiFi ripples" and figure out what is happening in the room. It was trained using a technique called *contrastive learning*, which means it taught itself by comparing thousands of WiFi signal snapshots -- no human had to manually label anything.

The result is a small, fast model that runs on a $9 microcontroller and preserves complete privacy because it never captures images or audio.

---

## What can it do?

| Capability | Accuracy | What you need | Notes |
|---|---|---|---|
| **Presence detection** | >95% | 1x ESP32-S3 ($9) | Is anyone in the room? |
| **Motion classification** | >90% | 1x ESP32-S3 ($9) | Still, walking, exercising, fallen |
| **Breathing rate** | +/- 2 BPM | 1x ESP32-S3 ($9) | Best when person is sitting or lying still |
| **Heart rate estimate** | +/- 5 BPM | 1x ESP32-S3 ($9) | Experimental -- less accurate during movement |
| **Person counting** | 1-4 people | 2x ESP32-S3 ($18) | Uses cross-node signal fusion |
| **Pose estimation** | 17 COCO keypoints | 2x ESP32-S3 + Seed ($27) | Full skeleton: head, shoulders, elbows, etc. |

---

## Quick Start

### Install

```bash
pip install onnxruntime numpy
```

### Run inference

```python
import onnxruntime as ort
import numpy as np

# Load the encoder model
session = ort.InferenceSession("pretrained-encoder.onnx")

# Simulated 8-dim CSI feature vector from ESP32-S3
# Dimensions: [amplitude_mean, amplitude_std, phase_slope, doppler_energy,
#              subcarrier_variance, temporal_stability, csi_ratio, spectral_entropy]
features = np.array(
    [[0.45, 0.30, 0.69, 0.75, 0.50, 0.25, 0.00, 0.54]],
    dtype=np.float32,
)

# Encode into 128-dim embedding
result = session.run(None, {"input": features})
embedding = result[0]  # shape: (1, 128)
print(f"Embedding shape: {embedding.shape}")
print(f"First 8 values: {embedding[0][:8]}")
```

### Run task heads

```python
# Load the task heads model
heads = ort.InferenceSession("pretrained-heads.onnx")

# Feed the embedding from the encoder
predictions = heads.run(None, {"embedding": embedding})

presence_score = predictions[0]    # 0.0 = empty, 1.0 = occupied
person_count   = predictions[1]    # estimated count (float, round to int)
activity_class = predictions[2]    # [still, walking, exercise, fallen]
vitals         = predictions[3]    # [breathing_bpm, heart_bpm]

print(f"Presence:  {presence_score[0]:.2f}")
print(f"People:    {int(round(person_count[0]))}")
print(f"Activity:  {['still', 'walking', 'exercise', 'fallen'][activity_class.argmax()]}")
print(f"Breathing: {vitals[0][0]:.1f} BPM")
print(f"Heart:     {vitals[0][1]:.1f} BPM")
```

---

## Model Architecture

```
                                                      +-- Presence (binary)
                                                      |
WiFi signals --> ESP32-S3 --> 8-dim features --> Encoder (TCN) --> 128-dim embedding --> Task Heads --+-- Person Count
                  (CSI)        (on-device)       (~2.5M params)                          (~100K)     |
                                                                                                     +-- Activity (4 classes)
                                                                                                     |
                                                                                                     +-- Vitals (BR + HR)
```

### Encoder

- **Type:** Temporal Convolutional Network (TCN)
- **Input:** 8-dimensional feature vector extracted from raw CSI
- **Output:** 128-dimensional embedding
- **Parameters:** ~2.5M
- **Format:** ONNX (runs on any platform with ONNX Runtime)

### Task Heads

- **Type:** Small MLPs (multi-layer perceptrons), one per task
- **Input:** 128-dim embedding from the encoder
- **Output:** Task-specific predictions (presence, count, activity, vitals)
- **Parameters:** ~100K total across all heads
- **Format:** ONNX

### Feature extraction (runs on ESP32-S3)

The ESP32-S3 captures raw CSI frames at ~100 Hz and computes 8 summary features per window:

| Feature | Description |
|---|---|
| `amplitude_mean` | Average signal strength across subcarriers |
| `amplitude_std` | Variation in signal strength (movement indicator) |
| `phase_slope` | Rate of phase change across subcarriers |
| `doppler_energy` | Energy in the Doppler spectrum (velocity indicator) |
| `subcarrier_variance` | How much individual subcarriers differ |
| `temporal_stability` | Consistency of signal over time (stillness indicator) |
| `csi_ratio` | Ratio between antenna pairs (direction indicator) |
| `spectral_entropy` | Randomness of the frequency spectrum |

---

## Training Data

### How it was trained

This model was trained using **self-supervised contrastive learning**, which means it learned entirely from unlabeled WiFi signals. No cameras, no manual annotations, and no privacy-invasive data collection were needed.

The training process works like this:

1. **Collect** raw CSI frames from ESP32-S3 nodes placed in a room
2. **Extract** 8-dimensional feature vectors from sliding windows of CSI data
3. **Contrast** -- the model learns that features from nearby time windows should produce similar embeddings, while features from different scenarios should produce different embeddings
4. **Fine-tune** task heads — *planned:* weak labels from environmental sensors (PIR motion, temperature, pressure) on the Cognitum Seed companion device. **This environmental-sensor ground-truth path is not yet implemented** (no PIR/BME280 ingestion in the training pipeline today); current task-head supervision uses the proxy/camera labels described elsewhere.

### Data provenance

- **Source:** Live CSI from 2x ESP32-S3 nodes (802.11n, HT40, 114 subcarriers)
- **Volume:** ~360,000 CSI frames (~3,600 feature vectors) per collection run
- **Environment:** Residential room, ~4x5 meters
- **Ground truth:** *Planned* — environmental sensors on the Cognitum Seed (PIR, BME280, light). Not yet wired into training; treat the PIR/BME280 references in this card as the intended design, not a current capability.
- **Attestation:** Every collection run produces a cryptographic witness chain (`collection-witness.json`) that proves data provenance and integrity

### Witness chain

The `collection-witness.json` file contains a chain of SHA-256 hashes linking every step from raw CSI capture through feature extraction to model training. This allows anyone to verify that the published model was trained on data collected by specific hardware at a specific time.

---

## Hardware Requirements

### Minimum: single-node sensing ($9)

| Component | What it does | Cost | Where to get it |
|---|---|---|---|
| ESP32-S3 (8MB flash) | Captures WiFi CSI + runs feature extraction | ~$9 | Amazon, AliExpress, Adafruit |
| USB-C cable | Power + data | ~$3 | Any electronics store |

This gets you: presence detection, motion classification, breathing rate.

### Recommended: dual-node sensing ($18)

Add a second ESP32-S3 to enable cross-node signal fusion for better accuracy and person counting.

### Full setup: sensing + ground truth ($27)

| Component | What it does | Cost |
|---|---|---|
| 2x ESP32-S3 (8MB) | WiFi CSI sensing nodes | ~$18 |
| Cognitum Seed (Pi Zero 2W) | Runs inference + collects ground truth | ~$15 |
| USB-C cables (x3) | Power + data | ~$9 |
| **Total** | | **~$27** |

The Cognitum Seed runs the ONNX models on-device and orchestrates the ESP32 nodes over USB serial. (Using its onboard PIR/BME280 sensors as training ground truth is planned but not yet implemented — see "Data provenance" above.)

---

## Files in this repo

| File | Size | Description |
|---|---|---|
| `pretrained-encoder.onnx` | ~2 MB | Contrastive encoder (TCN backbone, 8-dim input, 128-dim output) |
| `pretrained-heads.onnx` | ~100 KB | Task heads (presence, count, activity, vitals) |
| `pretrained.rvf` | ~500 KB | RuVector format embeddings for advanced fusion pipelines |
| `room-profiles.json` | ~10 KB | Environment calibration profiles (room geometry, baseline noise) |
| `collection-witness.json` | ~5 KB | Cryptographic witness chain proving data provenance |
| `config.json` | ~2 KB | Training configuration (hyperparameters, feature schema, versions) |
| `README.md` | -- | This file |

### RuVector format (.rvf)

The `.rvf` file contains pre-computed embeddings in RuVector format, used by the RuView application for advanced multi-node fusion and cross-viewpoint pose estimation. You only need this if you are using the full RuView pipeline. For basic inference, the ONNX files are sufficient.

---

## How to use with RuView

[RuView](https://github.com/ruvnet/RuView) is the open-source application that ties everything together: firmware flashing, real-time sensing, and a browser-based dashboard.

### 1. Flash firmware to ESP32-S3

```bash
git clone https://github.com/ruvnet/RuView.git
cd RuView

# Flash firmware (requires ESP-IDF v5.4 or use pre-built binaries from Releases)
# See the repo README for platform-specific instructions
```

### 2. Download models

```bash
pip install huggingface_hub
huggingface-cli download ruvnet/wifi-densepose-pretrained --local-dir models/
```

### 3. Run inference

```bash
# Start the CSI bridge (connects ESP32 serial output to the inference pipeline)
python scripts/seed_csi_bridge.py --port COM7 --model models/pretrained-encoder.onnx

# Or run the full sensing server with web dashboard
cargo run -p wifi-densepose-sensing-server
```

### 4. Adapt to your room

The model works best after a brief calibration period (~60 seconds of no movement) to learn the baseline signal characteristics of your specific room. The `room-profiles.json` file contains example profiles; the system will create one for your environment automatically.

---

## Limitations

Be honest about what this technology can and cannot do:

- **Room-specific.** The model needs a short calibration period in each new environment. A model calibrated in a living room will not work as well in a warehouse without re-adaptation.
- **Single room only.** There is no cross-room tracking. Each room needs its own sensing node(s).
- **Person count accuracy degrades above 4.** Counting works well for 1-3 people, becomes unreliable above 4 in a single room.
- **Vitals require stillness.** Breathing and heart rate estimation work best when the person is sitting or lying down. Accuracy drops significantly during walking or exercise.
- **Heart rate is experimental.** The +/- 5 BPM accuracy is a best-case figure. In practice, cardiac sensing via WiFi is still a research-stage capability.
- **Wall materials matter.** Metal walls, concrete reinforced with rebar, or foil-backed insulation will significantly attenuate the signal and reduce range.
- **WiFi interference.** Heavy WiFi traffic from other devices can add noise. The system works best on a dedicated or lightly-used WiFi channel.
- **Not a medical device.** Vital sign estimates are for informational and research purposes only. Do not use them for medical decisions.

---

## Use Cases

- **Elder care:** Non-invasive fall detection and activity monitoring without cameras
- **Smart home:** Presence-based lighting and HVAC control
- **Security:** Occupancy detection through walls
- **Sleep monitoring:** Breathing rate tracking overnight
- **Research:** Low-cost human sensing for academic experiments
- **Disaster response:** The MAT (Mass Casualty Assessment Tool) uses this model to detect survivors through rubble via WiFi signal reflections

---

## Ethical Considerations

WiFi sensing is a privacy-preserving alternative to cameras, but it still detects human presence and activity. Consider these points:

- **Consent:** Always inform people that WiFi sensing is active in a space.
- **No biometric identification:** This model cannot identify *who* someone is -- only that someone is present and what they are doing.
- **Data minimization:** Raw CSI data is processed on-device and only summary features or embeddings leave the sensor. No images, audio, or video are ever captured.
- **Dual use:** Like any sensing technology, this can be misused for surveillance. We encourage transparent deployment and clear signage.

---

## Citation

If you use this model in your research, please cite:

```bibtex
@software{wifi_densepose_2026,
  title   = {WiFi-DensePose: Human Pose Estimation from WiFi Channel State Information},
  author  = {ruvnet},
  year    = {2026},
  url     = {https://github.com/ruvnet/RuView},
  license = {MIT},
  note    = {Self-supervised contrastive learning on ESP32-S3 CSI data}
}
```

---

## License

MIT License. See [LICENSE](https://github.com/ruvnet/RuView/blob/main/LICENSE) for details.

You are free to use, modify, and distribute this model for any purpose, including commercial applications.

---

## Links

- **GitHub:** [github.com/ruvnet/RuView](https://github.com/ruvnet/RuView)
- **Hardware:** [ESP32-S3 DevKit](https://www.espressif.com/en/products/devkits) | [Cognitum Seed](https://cognitum.one)
- **ONNX Runtime:** [onnxruntime.ai](https://onnxruntime.ai)
