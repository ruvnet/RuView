# ruview

**Ambient intelligence from WiFi CSI.** Detect human presence, count
people, read breathing and heart rate, and estimate skeletal pose —
using only the WiFi signal already in your home. No cameras. No
wearables. Works through walls and in the dark.

`ruview` is the brand-facing meta-package for the
[RuView](https://github.com/ruvnet/RuView) sensing stack. It installs
the compiled PyO3 wheel published as
[`wifi-densepose`](https://pypi.org/project/wifi-densepose/) and
re-exports its full API under the `ruview` namespace — so you can
write either of these and they do the same thing:

```python
from ruview import BreathingExtractor, HeartRateExtractor
from wifi_densepose import BreathingExtractor, HeartRateExtractor
```

The WebSocket/MQTT clients live in `wifi_densepose.client` (install the
`[client]` extra).

## Install

```bash
pip install ruview                 # core DSP
pip install "ruview[client]"       # + WebSocket/MQTT clients
```

## Usage

Runs as-is after `pip install ruview` (synthetic signal, about 15 breaths/min):

```python
import math

from ruview import BreathingExtractor, CsiVitalPreprocessor

pre = CsiVitalPreprocessor.esp32_default()
br = BreathingExtractor.esp32_default()  # 56 subcarriers @ 100 Hz, 30s window
for i in range(3000):
    a = 10.0 + 0.5 * math.sin(2 * math.pi * 0.25 * i / 100.0)
    est = br.extract(pre.process([a] * 56), [])
print(est)
```

Full API + WebSocket / MQTT / Home Assistant integration docs:
[wifi-densepose on PyPI](https://pypi.org/project/wifi-densepose/).

## Why two PyPI names?

Historic: `wifi-densepose` is the technical / academic name (the
project started as a WiFi-based DensePose implementation).
`ruview` is the brand the v2 ambient-intelligence platform ships
under. Both are the same code. You pick the import that reads
better in your project.

## Links

- **Repository** — https://github.com/ruvnet/RuView
- **Modernization plan** — [ADR-117](https://github.com/ruvnet/RuView/blob/main/docs/adr/ADR-117-pip-wifi-densepose-modernization.md)
- **Issues** — https://github.com/ruvnet/RuView/issues

## License

MIT.
