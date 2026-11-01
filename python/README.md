# wifi-densepose

[![PyPI version](https://img.shields.io/pypi/v/wifi-densepose.svg)](https://pypi.org/project/wifi-densepose/)
[![Python](https://img.shields.io/pypi/pyversions/wifi-densepose.svg)](https://pypi.org/project/wifi-densepose/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

**Detect human presence, count people, read breathing and heart rate, and
estimate skeletal pose — using only the WiFi signal already in your home.**

No cameras. No wearables. Works through walls and in the dark.

`wifi-densepose` is the Python binding for the [RuView](https://github.com/ruvnet/RuView)
sensing stack: a Rust core that turns the Channel State Information (CSI)
emitted by ordinary WiFi chips into ambient-intelligence signals. The wheel
ships compiled DSP for fast offline analysis, plus an opt-in Python client
for talking to a live RuView sensing-server over WebSocket or MQTT.

## Features

- **17-keypoint pose** — full-body skeletal estimate from WiFi CSI, no camera
- **Vital signs** — respiratory rate (6–30 BPM) and heart rate (40–120 BPM)
  with a confidence score and clinical-grade / degraded / unreliable status
- **Presence, person count, fall detection, motion** — fused outputs from
  the same CSI stream
- **10 semantic primitives** (HA-MIND) — someone-sleeping, possible-distress,
  room-active, bathroom-occupied, fall-risk-elevated, bed-exit, … — ready
  to wire into Home Assistant or Apple Home automations
- **Beamforming Feedback (BFLD) support** — 802.11ac/ax/be compressed feedback
  matrices on top of the receiver-side CSI path
- **GIL-releasing DSP** — extract loops run with the GIL released, so a
  tokio-backed web server can call into the pipeline without stalling its
  event loop
- **Tiny wheel** — ~240 KB compiled (one binary per OS/arch covers Python
  3.10+ via the stable ABI)

## Install

```bash
pip install wifi-densepose                 # core DSP only
pip install "wifi-densepose[client]"       # + WebSocket/MQTT clients
```

Wheels are published for Linux (x86_64, aarch64), macOS (x86_64, arm64), and
Windows (amd64).

### SOTA extras (ADR-185)

Three optional subsystems bind the Rust SOTA modules as compiled-feature
wheels. Each raises a clear `ImportError` if you import it without the extra:

| Extra | Module | What it adds |
|-------|--------|--------------|
| `[aether]` | `wifi_densepose.aether` | Contrastive CSI embeddings / re-identification (ADR-024) — `EmbeddingExtractor`, `cosine_similarity`, `info_nce_loss` |
| `[meridian]` | `wifi_densepose.meridian` | Cross-environment domain generalization (ADR-027) — `HardwareNormalizer`, `GeometryEncoder`, `RapidAdaptation`, `CrossDomainEvaluator` |
| `[mat]` | `wifi_densepose.mat` | Mass-Casualty Assessment disaster-survivor detection + START triage — `DisasterResponse`, `Survivor`, `TriageStatus` |
| `[sota]` | all three | Convenience superset |

```bash
pip install "wifi-densepose[aether]"       # re-identification embeddings
pip install "wifi-densepose[meridian]"     # cross-room calibration
pip install "wifi-densepose[mat]"          # disaster triage
pip install "wifi-densepose[sota]"         # all three
```

Runnable examples: [`examples/reid_from_csi.py`](examples/reid_from_csi.py),
[`examples/cross_room_calibrate.py`](examples/cross_room_calibrate.py),
[`examples/mat_triage.py`](examples/mat_triage.py).

## Usage

### Extract breathing rate from a CSI stream

```python
from wifi_densepose import BreathingExtractor

br = BreathingExtractor.esp32_default()     # 56 subcarriers @ 100 Hz, 30s window

for residuals, weights in your_csi_source:  # one frame at a time
    est = br.extract(residuals=residuals, weights=weights)
    if est is not None:
        print(f"{est.value_bpm:.1f} BPM  (confidence={est.confidence:.2f})")
```

For heart rate, use `HeartRateExtractor.esp32_default()` with a 0.8–2.0 Hz
bandpass and a 15-second window. Its second argument is `phases`, not the
breathing extractor's `weights`:

```python
hr = HeartRateExtractor.esp32_default()
est = hr.extract(residuals=residuals, phases=phases)
# If phase measurements are unavailable, phases=[] uses equal weighting.
```

The current binding rejects the old `weights` keyword with `TypeError`.
Heart-rate estimation requires a local autocorrelation peak; a declining
breathing-only correlation is not treated as a pulse at the band edge.
Additional filtering reduces respiratory leakage, while confidence remains
capped by evidence from the original filter stage. Synthetic regression tests
cover these cases; they do not establish accuracy on real CSI or medical use.

### Subscribe to a live sensing-server

```python
import asyncio
from wifi_densepose.client import SensingClient, EdgeVitalsMessage

async def main():
    async with SensingClient("ws://your-ruview-node:8765/ws/sensing") as c:
        async for msg in c.stream():
            if isinstance(msg, EdgeVitalsMessage):
                print(msg.presence, msg.breathing_rate_bpm, msg.heartrate_bpm)

asyncio.run(main())
```

### React to Home Assistant semantic primitives

```python
from wifi_densepose.client import (
    RuViewMqttClient, SemanticPrimitive, SemanticPrimitiveListener,
)

listener = SemanticPrimitiveListener()
listener.on(SemanticPrimitive.BedExit, lambda e: print("bed exit:", e.node_id))
listener.on(SemanticPrimitive.PossibleDistress, lambda e: alert(e))

client = RuViewMqttClient(broker_host="homeassistant.local")
client.on_message(
    # MQTT `+` must fill a whole topic level; the listener skips non-RuView topics.
    "homeassistant/+/+/+/state",
    listener.handle_mqtt_message,
)
client.start()
client.wait_connected()
```

### Decode 802.11ax beamforming feedback

```python
import numpy as np
from wifi_densepose import BfldFrame, BfldKind

# Parse compressed BFR from a Wireshark capture into a complex128 ndarray ...
fb = np.zeros((2, 1, 996), dtype=np.complex128)  # Nr=2 Nc=1 Nsc=996 for HE80

frame = BfldFrame.from_compressed_feedback(
    timestamp_ms=ts,
    sounding_index=seq,
    sta_mac="aa:bb:cc:dd:ee:ff",
    kind=BfldKind.CompressedHE80,
    feedback_matrix=fb,
)
print(frame.n_subcarriers, frame.mean_amplitude)
```

## Hardware

Works with any WiFi chip that exposes CSI. Reference setups (ESP-IDF firmware,
build scripts, witness-verified test bundles) are in the
[RuView repo](https://github.com/ruvnet/RuView):

| Device | Cost | Role |
|---|---|---|
| ESP32-S3 (8MB flash) | ~$9 | WiFi CSI sensing node |
| ESP32-S3 SuperMini (4MB) | ~$6 | WiFi CSI (compact) |
| ESP32-C6 + Seeed MR60BHA2 | ~$15 | mmWave HR/BR/presence add-on |

The legacy v1 line (Wi-Pose-style FastAPI server) is end-of-life;
`wifi-densepose==1.99.0` is a tombstone that raises `ImportError` pointing
to v2 with a migration URL.

## Links

- **Repository** — https://github.com/ruvnet/RuView
- **Modernization plan** — [ADR-117](https://github.com/ruvnet/RuView/blob/main/docs/adr/ADR-117-pip-wifi-densepose-modernization.md)
- **Home Assistant integration** — [ADR-115](https://github.com/ruvnet/RuView/blob/main/docs/adr/ADR-115-home-assistant-integration.md)
- **Issues** — https://github.com/ruvnet/RuView/issues

## License

MIT.
