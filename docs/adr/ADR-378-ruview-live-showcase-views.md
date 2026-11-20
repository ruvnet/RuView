# ADR-378: `ruview-live` showcase views — CSI waterfall, radar fan, animated cells

- **Status**: proposed
- **Date**: 2026-10-01
- **Deciders**: rUv ("make this a showcase … the art of the possible for mods"); RuView maintainers
- **Tags**: claude-code, mods, ui, raster, animation, csi, radar, honesty
- **Related**: ADR-377 (the `ruview-live` mod), ADR-373 (device access),
  ADR-375 (terminal UI), ADR-018 / ADR-323 (CSI formats)

## Context

ADR-377 shipped `/ruview`: a pane of cards built from text and sparklines. The
mod engine can do much more on the terminal surface:

- `Raster` is a grid of cells, each with a glyph and a 24-bit foreground and
  background.
- `$.ui.blit` repaints a mounted Raster in place, up to 60 frames a second,
  without redrawing the tree.
- Buttons take hotkeys.

The pane only showed summaries. It never showed the signal itself: per-subcarrier
CSI amplitude over time, or the radar's range as geometry.

## Decision

### Three views, keys `1` `2` `3`

| View | What it draws | Data |
|---|---|---|
| Overview | the ADR-377 cards | unchanged |
| CSI waterfall | subcarrier bins across, time down; two frames per text row through `▀` (foreground = upper frame, background = lower); 5th–95th percentile colour limits with a colour bar | `ruview esp32 --spectrum` (new) |
| Radar | top-down 120° fan: range rings, a fading trail of past distances, the current distance as an arc; braille line charts of heart, breathing and distance | ESPHome radar kit (ADR-373) |

`n` cycles nodes in the waterfall when more than one streams. The live views
refresh every `liveRefreshSeconds` (default 4, range 2–60); the overview keeps
`refreshSeconds`.

### Harness: `--spectrum`

`captureEsp32` and the `ruview_esp32_capture` tool take three new arguments:

- `spectrum`: boolean;
- `spectrum_bins`: 8–128, default 48;
- `spectrum_frames`: 8–256, default 64.

A `SpectrumCollector` keeps a ring of the newest frames for each node and CSI
shape. It averages each frame's amplitudes into equal bins, and reports the
dominant shape per node with its arrival rate and `synthetic` flag. The
collector is read-only and bounded (8 nodes), and it adds nothing to a capture
that does not ask for it.

### Animation with `$.ui.blit`

One 80 ms timer (about 12 fps) repaints only these keyed Rasters:

- **shimmer**: a highlight sweeping along the rule under the title. Pure
  decoration.
- **pulse**: a ♥ that beats at the device-reported heart rate and a gauge that
  fills and empties at the device-reported breathing rate. It is labelled *a
  metronome, not a waveform*.
- **fan**: a sonar ring that leaves the sensor and reaches the measured range.
- **waterfall replay**: frames arrive a capture at a time, so the view drains
  them at their measured arrival rate instead of jumping every refresh. It is
  labelled *replayed at the arrival rate*.

The first paint and every frame come from the same function (`picturesOf`). A
test asserts that each picture keeps its size across frames, because a blit of
another size is refused.

Blits are fire-and-forget: a blit resolves only once a frame is painted, and
the surface folds blits between frames anyway.

### Honesty rules (CLAUDE.md)

- The waterfall says **MEASURED — live CSI amplitude received on this host** or
  **SYNTHETIC — simulator frames**, from the packets' own flag.
- The radar view states *range only: this kit reports distance, not bearing*.
  The arc covers every bearing at that range; nothing implies a position.
- Vitals stay *device-reported, not validated*. Values outside 40–180 bpm
  (heart) or 4–40 bpm (breathing) are flagged.
- Chart scales fit the data so a rhythm is visible. The plausible band is
  shaded only where it overlaps the scale.
- Surfaces without `Raster` (desktop, VS Code, mobile) get a note in the
  showcase views instead of a tree the surface would refuse. The overview works
  everywhere.
- Only nodes in the latest capture stay in the waterfall. A node that stops
  streaming, or a failed capture, clears its frames, so old frames are never
  shown as live. Frames join across captures only when the CSI shape and bin
  count both match.
- Multi-antenna ESP32 nodes are drawn from their first chain. ADR-018 lays the
  tones out antenna-major; this applies to the waterfall only, and the vitals
  kernel still takes single-chain frames.
- Animation runs on the real clock, the same one the full render reads. The
  pulses keep the reported rates even when a frame fires late.

### Structure

Mods may import only their own files, so the hooks module is split into:

- `model.mjs`: settings, argv, results, model and history (pure);
- `raster.mjs`: Grid, base64, colour map, waterfall, braille, radar fan, line
  chart (pure);
- `anim.mjs`: frame functions of time (pure);
- `views.mjs`: drawing;
- `register.mjs`: wiring, timers and blits. It re-exports the rest so tests
  import one module.

## Evidence

- **MEASURED** (real silicon, 2026-10-01):
  - Hardware: an ESP32-C6 on COM11, flashed with the hardware-verified
    `release_bins/c6-adr110` image (v0.7.0; SHA-256 checked) and provisioned to
    ruv.net, node 6.
  - Boot log: `Got IP: 192.168.1.103` and `UDP sender initialized:
    192.168.1.45:5005`.
  - Captures: `ruview esp32 --spectrum` received 29–50 frames per 2 s at 7–18
    Hz and −37 to −78 dBm, in 1×64 and 1×256 shapes.
  - Waterfall: a 1×256 capture rendered as 114 bins showing the occupied band
    against guard and null tones.
  - Reproducer: `npx @ruvnet/ruview esp32 --seconds 4 --spectrum --json` with
    a node targeting the host.
- **MEASURED** (device-reported): the Seeed MR60BHA2 kit at 192.168.1.102 drives
  the radar view.
- **SYNTHETIC**: the `realtek-csi-sim` simulator exercises the waterfall in
  development; its frames carry the synthetic flag and the view says so.
- **Tests**:
  - 11 mod tests and 11 showcase tests: Grid/base64 against Node's encoder,
    colour map monotone, waterfall ordering, braille bits, fan scaling and
    ping, chart scaling, frame functions, constant picture sizes, MEASURED and
    SYNTHETIC labels, surfaces without Raster;
  - a loopback test of `--spectrum`;
  - 4 engine tests (`claude plugin test mod`), including the waterfall tab
    sending `--spectrum`, drawing the Raster and animating it with blits.

## Consequences

- The package grows by about 37 KB. The reviewed unpacked ceiling rises from
  384 KiB to 448 KiB in `npm-packages.yml` and `ruview-npm-release.yml`. There
  are still no runtime dependencies.
- The live views poll more often: a 2 s capture every 4 s by default. A
  capture binds the UDP port briefly, so a sensing server on the same port
  still reports `port_in_use`, as in ADR-377.
- The animation timer runs only while the pane is open, and stops on close and
  session end.
