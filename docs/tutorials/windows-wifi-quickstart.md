# Windows WiFi sensing quick start

A version-pinned path from a Windows laptop to presence and motion detection
over its own WiFi adapter, and the three ways the documented walk-through
fails on a current machine.

This guide replaces the tutorial body of
[RuView#36](https://github.com/ruvnet/RuView/issues/36), which was written
against the v1 layout before the archive move and has not been re-run since.

## Contents

- [What does this guide cover?](#what-does-this-guide-cover)
- [What do I need before I start?](#what-do-i-need-before-i-start)
- [How do I import the pipeline without installing the whole app?](#how-do-i-import-the-pipeline-without-installing-the-whole-app)
- [How do I run the unit tests?](#how-do-i-run-the-unit-tests)
- [How do I run the deterministic proof?](#how-do-i-run-the-deterministic-proof)
- [How do I take a single RSSI sample?](#how-do-i-take-a-single-rssi-sample)
- [Why does netsh print nothing on Windows 11?](#why-does-netsh-print-nothing-on-windows-11)
- [What did this guide measure, and on what?](#what-did-this-guide-measure-and-on-what)
- [Where does each piece live?](#where-does-each-piece-live)
- [See also](#see-also)
- [References](#references)

## What does this guide cover?

Presence and motion detection from the RSSI of a Windows WiFi adapter, using
the archived v1 Python reference pipeline: `netsh` for the reading, a
Hann-windowed FFT for the spectral features, a CUSUM for the change points,
and a rule-based classifier.

It does not cover breathing rate or heart rate. `netsh` reports the link
strength in whole dBm, and the resolution is too coarse for the sub-dBm
variation that respiration produces. It does not cover pose estimation: that
needs CSI, which needs an ESP32 (see
[the ESP32-S3 quick start](esp32-s3-csi-quickstart.md)).

The maintained implementation is the Rust workspace in `v2/`, which serves the
same source through `wifi-densepose-sensing-server --source wifi`. This guide
covers the archived Python path because that is the one the walk-through
documents.

## What do I need before I start?

| Item | Requirement | Why |
|---|---|---|
| Host | Windows 10 or 11, a WiFi adapter, an associated network | The reading comes from the adapter |
| Python | 3.10 to 3.14 | The pipeline needs `numpy` and `scipy`; see the note below |
| Packages | `numpy`, `scipy` | The feature extractor |
| Time | About five minutes | No hardware to build or flash |

Python 3.15 is not a good fit yet: the `torch` wheel for that interpreter
cannot load its own libraries on Windows, and `torch` is pulled in by the
package's application import. The pipeline itself does not need `torch`, and
the fix in this branch keeps that broken optional dependency from taking the
whole package down, so the pipeline runs — but a 3.14 interpreter is the
smoother route.

## How do I import the pipeline without installing the whole app?

The v1 code lives under `archive/v1/`, and the `v1` package has to be
importable for any of it to be used:

```powershell
$env:PYTHONPATH = "archive"
```

That is the whole setup. The path is `archive`, not `archive/v1`: the files
import `v1.src.sensing...`, so the directory that has to be on the path is
the one that *contains* `v1`.

```python
from v1.src.sensing.rssi_collector import WindowsWifiCollector
from v1.src.sensing.feature_extractor import RssiFeatureExtractor
from v1.src.sensing.classifier import PresenceClassifier
```

Importing `v1.src` runs `archive/v1/src/__init__.py`, which also tries to
import the application. That import is optional and its failures are warned
about, not raised — with one exception that this branch fixes, described
below.

## How do I run the unit tests?

```powershell
$env:PYTHONPATH = "archive"
python -m pytest archive/v1/tests/unit/test_sensing.py -q -o "addopts="
```

45 tests pass. They use the `SimulatedCollector`, so no WiFi adapter and no
network are needed.

The live suite skips itself when it cannot see a connected interface:

```powershell
python -m pytest archive/v1/tests/integration/test_windows_live_sensing.py -q -rs
```

5 tests, skipped with `Requires Windows with connected WiFi`.

## How do I run the deterministic proof?

The proof replays 100 frames through the production CSI pipeline and compares
the result against a pinned hash. On Windows, run the Python entry point
directly — the `./verify` wrapper is a bash script and needs Git Bash or WSL:

```powershell
python archive/v1/data/proof/verify.py
```

It ends with:

```
VERDICT: PASS
```

`VERDICT: PASS` is the string to look for, and the pipeline hash printed next
to it must be the one the repository pins:

```
Pipeline hash: f8e76f21a0f9852b70b6d9dd5318239f6b20cbcb4cdd995863263cecdc446f7a
```

This is the ADR-028 trust kill switch: it is the check for "the project is
mocked".

## How do I take a single RSSI sample?

```python
from v1.src.sensing.rssi_collector import WindowsWifiCollector

collector = WindowsWifiCollector(interface="Wi-Fi")
sample = collector.collect_once()
print(f"RSSI: {sample.rssi_dbm} dBm, Quality: {sample.link_quality:.0%}")
```

The interface name is the one `netsh wlan show interfaces` prints, and it is
case-sensitive in the `netsh` output. A connected adapter with a strong link
gives something like:

```
RSSI: -39.0 dBm, Quality: 94%
```

For the full pipeline, collect a window and classify it:

```python
import time

from v1.src.sensing.classifier import PresenceClassifier
from v1.src.sensing.feature_extractor import RssiFeatureExtractor
from v1.src.sensing.rssi_collector import WindowsWifiCollector

collector = WindowsWifiCollector(interface="Wi-Fi", sample_rate_hz=2.0)
extractor = RssiFeatureExtractor(window_seconds=15.0)
classifier = PresenceClassifier(presence_variance_threshold=0.3)

collector.start()
print("Collecting 15 seconds of RSSI data...")
time.sleep(15)
collector.stop()

samples = collector.get_samples()
features = extractor.extract(samples)
result = classifier.classify(features)

print(f"Samples:   {len(samples)}")
print(f"RSSI mean: {features.mean:.1f} dBm")
print(f"Variance:  {features.variance:.4f}")
print(f"Motion:    {features.motion_band_power:.4f}")
print(f"Verdict:   {result.motion_level.value} ({result.confidence:.0%})")
```

`netsh` is slow: 2 Hz is the practical maximum, and a 15 second window at
that rate gives 30 samples.

To watch it live, run the monitor the repository already ships:

```powershell
$env:PYTHONPATH = "archive"
python archive/v1/tests/integration/live_sense_monitor.py
```

## Why does netsh print nothing on Windows 11?

Windows 11 gates the WLAN interface information behind Location services. With
the gate closed, `netsh wlan show interfaces` still exits successfully and
prints this instead of the interface block:

```
There is 1 interface on the system:
Network shell commands need location permission to access WLAN information.
Turn on Location services on the Location page in Privacy & security settings.
...
Function WlanQueryInterface returns error 5:
The requested operation requires elevation (Run as administrator).
```

No `Signal:` line and no `Rssi:` line, although the adapter is associated and
working. A reader that only looks for a signal line cannot tell this apart
from a weak link, and reports the -80 dBm default as if it were a measurement.
The classifier then reads "absent" for ever, and the dashboard sits at
"absent (100%)" with nothing in the log to explain it.

This branch changes that. The collector now refuses:

```
RuntimeError: netsh cannot read the WLAN interfaces: Windows requires
Location services for this query. Enable Settings > Privacy & security >
Location, or run the shell as administrator.
```

Two smaller faults came with it. Windows builds differ in how they print the
RSSI: one prints `Rssi : -72`, another prints `Rssi : -39 dBm`. Only the
first was parsed, so on a build that appends the unit the value was thrown
away and the -80 dBm default was used in its place. Both forms are read now.

The Rust server in `v2/` already refused to guess — it treats a `netsh`
output without an interface block as "no WiFi connected" and says so — so
this change brings the Python path to the same behaviour.

## What did this guide measure, and on what?

MEASURED on Windows 11 (24H2, build 26200), Intel(R) Wi-Fi 6E AX210, Python
3.15.0, with the adapter associated with a network and the saved profile for
it present on the host.

Reproducer: the capture committed with this guide,
[`evidence/windows-wifi-quickstart-2026-10-22/before-after.txt`](evidence/windows-wifi-quickstart-2026-10-22/before-after.txt),
holds the before and after of each command.

| Fact | Value | Where it came from |
|---|---|---|
| The proof before the fix | died on `OSError: [WinError 126]` while importing `torch` | capture: "BEFORE: the deterministic proof dies on an unrelated broken dependency" |
| The proof after the fix | `VERDICT: PASS` | capture: "AFTER: the proof runs and passes" |
| Pipeline hash before and after | `f8e76f21a0f9852b...`, unchanged | capture, both runs |
| The collector before the fix | `RSSI: -80.0 dBm, Quality: 0%` | capture: "BEFORE: the collector invents a signal" |
| The collector after the fix | `RuntimeError: netsh cannot read the WLAN interfaces: ...` | capture: "AFTER: the collector refuses" |
| Unit tests | 45 passed | `pytest archive/v1/tests/unit/test_sensing.py` |
| The new gate tests | 5 passed | `pytest archive/v1/tests/unit/test_rssi_collector_gate.py` |
| Live integration tests | 5 skipped, `Requires Windows with connected WiFi` | `pytest archive/v1/tests/integration/test_windows_live_sensing.py -rs` |

Not measured here: a live RSSI reading. The Location services gate on this
host closed the interface block, so the reading path could not be exercised
end to end. The unit tests pin the parsing against the documented `netsh`
output, and the collector's refusal is the behaviour that was measured.

Not claimed: any accuracy figure. The presence and motion thresholds in the
classifier are rule-based, and their behaviour depends on the room, the router
and the occupant.

## Where does each piece live?

| Piece | Path |
|---|---|
| The collector | `archive/v1/src/sensing/rssi_collector.py` |
| The feature extractor | `archive/v1/src/sensing/feature_extractor.py` |
| The classifier | `archive/v1/src/sensing/classifier.py` |
| The backend API | `archive/v1/src/sensing/backend.py` |
| The unit tests | `archive/v1/tests/unit/test_sensing.py` |
| The gate tests | `archive/v1/tests/unit/test_rssi_collector_gate.py` |
| The live integration tests | `archive/v1/tests/integration/test_windows_live_sensing.py` |
| The live monitor | `archive/v1/tests/integration/live_sense_monitor.py` |
| The deterministic proof | `archive/v1/data/proof/verify.py` |
| The trust kill switch | `verify` |

## See also

- [docs/user-guide.md](../user-guide.md) — the maintained server, its
  `--source wifi` path and the REST API.
- [docs/TROUBLESHOOTING.md](../TROUBLESHOOTING.md) — symptom-first fixes for a
  running system.
- [the ESP32-S3 CSI quick start](esp32-s3-csi-quickstart.md) — the higher
  fidelity path, with CSI.

## References

- RuView#36 — "Windows WiFi Sensing Quick Start (ADR-013)".
- ADR-013 — the commodity WiFi sensing design.
- ADR-028 — the ESP32 capability audit and the trust kill switch.
- ADR-299 — the CSI data policy: real CSI is person data.
