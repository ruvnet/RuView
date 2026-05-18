# ADR-121 — HLK-LD2402 24 GHz mmWave Radar (auxiliary modality)

**Status**: Accepted (single-modality readout). Fusion deferred.
**Date**: 2026-05-18
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/mmwave.rs` (new),
`Cargo.toml` (serialport dep), `main.rs` (CLI flags `--mmwave-port` /
`--mmwave-baud`, spawn reader, `mmwave_latest` REST handler, route),
`ui/components/SensingTab.js` (new card, poll integration).

## Context

The operator has an HLK-LD2402 24 GHz mmWave radar module attached via
a CP2102 USB-to-UART bridge. Factory firmware emits ASCII
`distance:<cm>\r\n` lines at 115200 8N1, ~6 Hz, in Normal Mode.

This module is a useful **auxiliary modality**: sub-mm range to a
moving target, very different physical principle than WiFi CSI, runs
fully independent. Concrete uses:

1. **Live readout in the UI** — easiest. Operator sees the radar
   distance alongside the WiFi sensing.
2. **Vitals ground-truth** — at 6 Hz the data is too slow for HR but
   captures breathing rate (0.2-0.4 Hz). Compare against the WiFi-CSI
   vitals detector (ADR-021) for calibration.
3. **Multi-modal fusion** — feed the mmWave distance + WiFi features
   into a future classifier. Different physics, very different
   confusion set — high-value addition.

This ADR ships #1 only. #2 and #3 are follow-ups.

## Decisions

### D1 — Dedicated blocking reader thread, not async

`serialport` is a sync API. Wrapping it with `tokio::spawn_blocking`
adds overhead for a single-port reader running indefinitely. A
plain `std::thread` named `mmwave-reader` reads the port, parses
lines, and writes the latest reading into a global
`OnceLock<Mutex<Option<MmwaveReading>>>`.

### D2 — Graceful absence

`--mmwave-port` is **optional**. When unset, the server runs as
before. When set but the port can't be opened, the reader thread
logs a single warning and exits — server keeps running with WiFi
sensing only. No retries, no panics. (Operator can hot-plug; if
auto-reconnect is wanted we add it later.)

### D3 — Stale-after policy

`mmwave::current(staleness)` returns `None` if the most recent
reading is older than `staleness`. The REST endpoint uses 2 seconds
— at the module's 6 Hz cadence, 2 s = ~12 missed frames, plenty of
slack for a brief USB hiccup but tight enough to flag a dead module.

### D4 — Single new REST endpoint, no SensingUpdate change

`GET /api/v1/mmwave/latest` returns:

```json
{ "available": true, "distance_cm": 152, "age_ms": 90 }
```

or

```json
{ "available": false }
```

Not embedded in `SensingUpdate` because:

* The WS stream is already busy with per-tick CSI broadcasts; a
  separate poll lets the UI throttle the mmWave refresh
  independently (saves bandwidth if many clients connect).
* Keeps the SensingUpdate schema stable — older WS consumers don't
  need a migration.

UI polls the endpoint once per visible WS tick. ~5-10 Hz refresh.

### D5 — UI badge in `SensingTab`, hidden when unavailable

New card "mmWave Radar (24 GHz)" with a blue badge showing
`<distance> cm` and an age bar (100 % at 0 ms → 0 % at 2 s). The
whole card hides via `display: none` when the endpoint reports
`available: false`, so deployments without the radar see no
clutter.

### D6 — Parse only the `distance:<n>` Normal Mode format

HLK-LD2402 also has an "Engineering Mode" emitting binary frames
with per-range-gate energy. Out of scope for v1 — Normal Mode
covers the live-readout use case. Engineering Mode parsing is a
separate ADR if/when we need per-gate data for vitals fusion.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/Cargo.toml
  + serialport.workspace = true
v2/crates/wifi-densepose-sensing-server/src/mmwave.rs   (new, ~130 LoC)
  + pub struct MmwaveReading { distance_cm: u32, at: Instant }
  + static LATEST: OnceLock<Mutex<Option<MmwaveReading>>>
  + pub fn current(staleness) -> Option<MmwaveReading>
  + pub fn spawn_reader(port, baud)
  + fn parse_distance(line: &str) -> Option<u32>
  + 1 unit test
v2/crates/wifi-densepose-sensing-server/src/lib.rs
  + pub mod mmwave;
v2/crates/wifi-densepose-sensing-server/src/main.rs
  + Args { mmwave_port, mmwave_baud }
  + spawn_reader call in main()
  + async fn mmwave_latest
  + route /api/v1/mmwave/latest
ui/components/SensingTab.js
  + #mmwaveCard hidden-by-default card with #mmwaveLabel + age bar
  + fetch /api/v1/mmwave/latest each visible tick, show/hide card
docs/adr/ADR-121-mmwave-hlk-ld2402.md  (this)
```

## Verified Acceptance

Live with the module attached:

```
$ ./target/release/sensing-server --mmwave-port /dev/cu.usbserial-1140 …
  ADR-121 mmWave reader: opened /dev/cu.usbserial-1140 @ 115200

$ curl :8080/api/v1/mmwave/latest
  {"age_ms":55,"available":true,"distance_cm":149}
  {"age_ms":90,"available":true,"distance_cm":152}
  {"age_ms":127,"available":true,"distance_cm":153}
```

Live without module attached (port arg omitted): server starts cleanly,
endpoint returns `{"available": false}`, Sensing tab card hidden.

## Out of Scope / Follow-ups

* **Engineering Mode binary parser** — needed if we want per-gate
  energy for vitals (breathing band) or person-counting from
  per-gate occupancy.
* **Vitals fusion (ADR-021 cross-check)** — log mmWave breathing
  rate side-by-side with WiFi-CSI vitals for 5 min, compute
  Pearson correlation, decide whether to weight one over the other
  in the final vitals output.
* **W-MLP feature input** — once vitals fusion proves out, expose
  mmWave distance as a 23rd feature in the W-MLP and retrain.
  Would warrant ADR-122.
* **Auto-reconnect** — current behaviour: open fails or read errors
  exit the reader thread. Add a retry loop with 2-second backoff
  if the operator wants USB hot-plug recovery.

## References

* ADR-021 — WiFi-CSI vitals detector (the candidate cross-check
  partner for HLK-LD2402 breathing-rate output).
* `assets/sensors/sensor_03.jpeg` / `_04.jpeg` / `_05.jpeg` —
  hardware photos and inventory entry for the module + CP2102
  bridge.
