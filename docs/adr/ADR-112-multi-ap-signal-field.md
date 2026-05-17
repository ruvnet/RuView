# ADR-112 — Multi-AP `signal_field` via `MultistaticFuser`

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/main.rs`
(`signal_field_from_multistatic`, two ESP32 vitals call sites). Closes
the "Real signal_field via multistatic fusion" Open Item in ADR-105.

## Context

ADR-105 D6 stripped the synthetic `signal_field` paint and left a 20×20
zero grid in its place. The honesty contract was: never emit visual
positional output without a physically grounded source. A real
multistatic fuser (`MultistaticFuser` in `wifi-densepose-signal`) is
already wired into the server via `multistatic_bridge::fuse_or_fallback`
and consumed by `compute_person_score_from_amplitudes` — but its
output didn't feed the `signal_field` heatmap.

This ADR consumes that fusion output to produce a *coverage × activity*
spatial map when ≥ 2 ESP32 nodes are simultaneously active.

## What the new map honestly is (and isn't)

* **Is**: a 20×20 floor-plane heatmap where each cell value =
  Σ over active nodes of `global_activity · exp(-d²/2σ²)`, with `d`
  the Euclidean distance from the cell to that node's configured
  position, σ a fixed radius, and `global_activity` =
  `cv²(fused_amplitude) · cross_node_coherence`. Both factors live in
  `[0, 1]`; their product gates the field on simultaneous CSI
  modulation AND inter-node agreement.
* **Is not**: a person-location estimate. Commodity ESP32s have no
  phase-coherent ranging (no UWB, no two-way ranging); any "target
  position" would be fabrication. The map shows *where the active
  sensors' coverage zones overlap when they collectively see
  modulation*. That's a real, derivable quantity. A "where is the
  person" claim is not, and is deliberately withheld.

## Decisions

### D1 — `signal_field_from_multistatic(fuser, node_states) -> SignalField`

New function in `main.rs`. Re-runs `multistatic_bridge::fuse_or_fallback`
(cheap — attention-weighted mean across O(N_nodes × N_subcarriers)),
discards the count-fallback path, and proceeds only when:

* `fused.active_nodes >= 2`, AND
* `fused.node_positions` non-empty, AND
* `fused.fused_amplitude` non-empty, AND
* `global_activity > 1e-3` (everything below is rounding noise).

Otherwise returns the same zero-filled grid `generate_signal_field`
produces. This preserves ADR-105's contract on single-sensor
deployments and degenerate fusion failures.

### D2 — Render constants

* Grid `20 × 1 × 20` (matches the existing `SignalField` shape and the
  UI's heatmap consumer).
* `ROOM_EXTENT_M = 3.0` m (half-width of the square the grid spans —
  6 m × 6 m floor). Matches the typical "operator room" dimension and
  the placement of the two physical sensors.
* `SIGMA_M = ROOM_EXTENT_M / 4.0 = 0.75 m` for the isotropic Gaussian.
  Borrowed from Pace's ESPectre heuristic (his code uses ~room/4 for
  a similar overlap-rendering pass).
* `(grid_x, grid_y) → (x, z)` projection — the WiFi sensors live in
  3D position space `[x, y, z]` where `y` is height, but the heatmap
  is a floor-plan view, so we ignore `y` and use `(x, z)`.

### D3 — `cv² × coherence` as the activity scalar

Two factors so that EITHER a quiet channel (low cv²) OR disagreeing
sensors (low coherence) collapses the field to zeros. This means:

* Empty room (low cv²) → blank map. Truthful.
* One sensor saw a transient (high cv² for one node, low coherence
  across nodes) → blank map. Truthful — no multistatic signal.
* All sensors see synchronized modulation → bright map. Truthful —
  there really is something in the shared coverage.

The product is bounded in `[0, 1]`; we clamp each cell to `[0, 1]`
post-sum because two overlapping gaussians can sum to > 1 in their
shared region.

### D4 — Call-site contract: prefer multistatic, else zero

Both ESP32 vitals paths build the field as:

```rust
let multi = signal_field_from_multistatic(&s.multistatic_fuser, &s.node_states);
if multi.values.iter().any(|&v| v > 0.0) { multi } else { /* zero */ }
```

A `multi` that is all-zero — either because `< 2` nodes are active or
because the activity threshold wasn't met — gets discarded and the
existing `generate_signal_field` zero is emitted. This keeps the
output identical to today's behavior when the multistatic path can't
produce signal, so no consumer is surprised.

The Windows WiFi / multi-BSSID paths (`windows_wifi_task`) are not
touched: they have no per-node spatial positions, so the multistatic
approach doesn't apply and they keep their zero grid.

## Trade-offs

* **Node positions must be configured.** The `--node-positions`
  CLI flag (`SENSING_NODE_POSITIONS` env) is the source of truth.
  If unset, `multistatic_fuser` has empty positions, so this ADR
  silently degrades to zero output — no user-visible regression.
* **Coverage map ≠ target map.** Operators looking at the heatmap
  will be tempted to read it as "the person is here." Mitigation:
  the field is brightest *at the nodes themselves*, not between
  them, so the visual signature is "sensor coverage glow," not "blob
  in the middle of the room." A future ADR (e.g. ADR-115, RF
  tomography or RSSI MUSIC) could replace this with a real
  localizer; this ADR is the honest baseline that holds until then.
* **σ is fixed.** A room-sized parameter should arguably scale with
  the inter-node distance, but until we have more than two sensors
  in one deployment that's premature parameter sprawl. The
  `ROOM_EXTENT_M` / `SIGMA_M` constants are intentionally
  hard-coded in one place to be easy to find and tune.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/main.rs
  - signal_field_from_multistatic (D1, D2, D3)
  - two vitals-path call sites adopt the prefer-multistatic-else-zero
    contract (D4)

docs/adr/ADR-112-multi-ap-signal-field.md  (this)
docs/adr/ADR-105-no-synthetic-data-in-production-runtime.md
  - close "Real signal_field via multistatic fusion" Open Item
```

## Verified Acceptance

* `cargo build --release -p wifi-densepose-sensing-server` clean.
* `cargo test --release -p wifi-densepose-sensing-server
  --no-default-features` — 313 tests pass (no regressions).
* With one sensor active, `signal_field.values` are all zero —
  matches ADR-105 behaviour.
* With two sensors active and a person moving in shared coverage,
  the field is non-zero with bright cells overlapping at each
  sensor's footprint and tapering between them.

## References

* ADR-105 D6 — the "no synthetic signal_field" honesty contract.
* `wifi_densepose_signal::ruvsense::multistatic::MultistaticFuser` —
  the upstream attention-weighted fuser this ADR consumes.
* `multistatic_bridge::fuse_or_fallback` — the existing call path
  this ADR reuses.
* Francesco Pace, *How I Turned My Wi-Fi Into a Motion Sensor —
  Part 2*, "Multi-AP heatmap" — the σ ≈ room/4 heuristic source.
