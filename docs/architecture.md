# RuView · How It Works

Extracted from the main README to keep the landing page short. See
[`../README.md`](../README.md) for the high-level overview.

---


WiFi routers flood every room with radio waves. When a person moves — or even breathes — those waves scatter differently. WiFi DensePose reads that scattering pattern and reconstructs what happened:

```
WiFi Router → radio waves pass through room → hit human body → scatter
    ↓
ESP32 mesh (4-6 nodes) captures CSI on channels 1/6/11 via TDM protocol
    ↓
Multi-Band Fusion: 3 channels × 56 subcarriers = 168 virtual subcarriers per link
    ↓
Multistatic Fusion: N×(N-1) links → attention-weighted cross-viewpoint embedding
    ↓
Coherence Gate: accept/reject measurements → stable for days without tuning
    ↓
Signal Processing: Hampel, SpotFi, Fresnel, BVP, spectrogram → clean features
    ↓
AI Backbone (RuVector): attention, graph algorithms, compression, field model
    ↓
Signal-Line Protocol (CRV): 6-stage gestalt → sensory → topology → coherence → search → model
    ↓
Neural Network: processed signals → 17 body keypoints + vital signs + room model
    ↓
Output: real-time pose, breathing, heart rate, room fingerprint, drift alerts
```

No training cameras required — the [Self-Learning system (ADR-024)](adr/ADR-024-contrastive-csi-embedding-model.md) bootstraps from raw WiFi data alone. [MERIDIAN (ADR-027)](adr/ADR-027-cross-environment-domain-generalization.md) ensures the model works in any room, not just the one it trained in.

