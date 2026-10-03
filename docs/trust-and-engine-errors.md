# Trust State & Engine Errors

If you've seen the sensing-server log a growing `engine_error_count`, or
noticed your deployment reads as `"demoted": true` on the status endpoint,
this page explains — from the actual code, not the design docs — what those
two things mean, what triggers them, where you can see them, and what your
real options are.

Everything below is grounded in
`v2/crates/wifi-densepose-sensing-server/src/engine_bridge.rs`,
`v2/crates/wifi-densepose-sensing-server/src/main.rs`,
`v2/crates/wifi-densepose-engine/src/lib.rs`, and
`v2/crates/wifi-densepose-signal/src/ruvsense/multistatic.rs`.

## Two different things, easy to conflate

The sensing-server runs a "governed trust cycle" every sensing tick
(`StreamingEngine::process_cycle`, driven by `EngineBridge::observe_cycle` in
`engine_bridge.rs:193-223`). Each cycle produces **one of two outcomes**, and
they are tracked completely separately:

1. **The cycle fails outright** (`Result::Err(EngineError)`) — nothing is
   published for that tick. This increments `engine_error_count`, a
   monotonically increasing counter.
2. **The cycle succeeds but under a demoted privacy class**
   (`Result::Ok(TrustedOutput { demoted: true, .. })`) — a belief *is*
   published, just at a more restricted privacy class than normal. This sets
   the `demoted` flag, which is recomputed fresh on every successful cycle.

A deployment can have a high `engine_error_count` with `demoted: false` (lots
of failed cycles, but the ones that succeed are clean), or `demoted: true`
with `engine_error_count: 0` (every cycle succeeds, but under a downgraded
privacy class), or both at once — which is what the issue reporter saw.

## 1. Exact conditions for each

### Engine errors (`engine_error_count`)

`engine_error_count` increments only when
`StreamingEngine::process_cycle` returns `Err(EngineError::Fusion(..))`
(`engine_bridge.rs:198-222`). `EngineError` wraps
`wifi_densepose_signal::ruvsense::multistatic::MultistaticError`
(`wifi-densepose-engine/src/lib.rs:54-69`), which has exactly four variants
(`multistatic.rs:36-56`):

| Variant | Condition |
|---|---|
| `NoFrames` | No node frames were passed to fusion. In practice not reachable through the bridge: `process_cycle_from_states` returns `None` (not an error, not counted) before calling the engine at all if there are no frames (`engine_bridge.rs:168-171`). |
| `InsufficientNodes(n)` | Fewer than 2 nodes contributing in multistatic mode. |
| `TimestampMismatch { spread_us, guard_us }` | The spread between contributing nodes' frame timestamps exceeds the **hard guard interval**, default **60,000 µs (60 ms)** (`MultistaticConfig::default()`, `multistatic.rs:133`). |
| `DimensionMismatch { node_idx, expected, got }` | A node's subcarrier count doesn't match the others. As of #1170 the live bridge canonicalizes every node onto a common 56-tone grid before fusion, so this is now rare on real hardware — see the comment on `observe_cycle_counts_engine_errors` in `engine_bridge.rs`. |

Regardless of cause, an error is **rate-limited in the log** to one
`tracing::warn!` line per 10 seconds (`ENGINE_ERROR_WARN_INTERVAL`,
`engine_bridge.rs:50, 206-219`) — errors are still counted every cycle, only
the *log line* is throttled, so a 20 Hz loop failing continuously won't flood
your log with 20 lines/second.

### Trust demotion (`demoted`)

This is a **separate mechanism** from engine errors: it happens on cycles
that *succeed*, and it downgrades the privacy class the output is emitted
under, one step, rather than failing the cycle. From
`wifi-densepose-engine/src/lib.rs:514-515`:

```rust
let demoted = quality.forces_privacy_demotion() || array_contradiction || mesh_at_risk;
let effective_class = if demoted { demote_one(base_class) } else { base_class };
```

Three independent conditions can trigger it:

- **`quality.forces_privacy_demotion()`** — true whenever the fusion
  quality record carries any non-empty `contradiction_flags`
  (`fusion_quality.rs:111-118`). These are *tolerated* disagreements, distinct
  from a hard fusion failure:
  - `TimestampMismatch` — spread within the **hard** guard but beyond the
    **soft** guard (default **20,000 µs / 20 ms**, `soft_guard_us`,
    `multistatic.rs:104-113`) — i.e. loose-but-tolerable timing alignment.
  - `CalibrationIdMismatch` — contributing frames disagree on which
    calibration epoch (baseline) they were captured under.
  - `PhaseAlignmentFailed`, `DriftProfileConflict`, `CoherenceDrop`,
    `GeometryInsufficient` — raised upstream by the array coordinator /
    baseline drift checks.
- **`array_contradiction`** — a separate array-level directional-fusion
  contradiction check.
- **`mesh_at_risk`** — the mesh is close to partitioning (`mesh_guard.rs`).

`demote_one()` (`lib.rs:688-690`) steps the privacy class exactly one notch
toward `Restricted` (it never jumps more than one step, and never relaxes a
class in the same cycle — proven by the `forced_contradiction_never_relaxes_class`
test). At `PrivacyClass::Restricted`, `EngineBridge::suppress_raw_outputs()`
becomes true and `main.rs` strips per-node raw amplitude vectors from the
published `SensingUpdate` (`engine_bridge.rs:251-260`).

**Crucially, `demoted` is not sticky.** It is overwritten on every
successful cycle to reflect *that cycle's* outcome
(`self.demoted = trust.demoted;`, `engine_bridge.rs:203`). If the
contradiction that caused demotion was transient, the very next clean cycle
reports `demoted: false` again with no action from you. If you see
`demoted: true` *persistently*, that means the underlying condition (usually
clock drift beyond the guard, or a geometry/calibration disagreement) is
itself persistent, not that something got "stuck."

## 2. Where this is exposed

Both `GET /health/ready` and `GET /api/v1/status` are wired to the same
handler (`health_ready`, `main.rs:8128,8133`) and return a `trust` block
(`main.rs:4589-4606`):

```json
{
  "status": "ready",
  "trust": {
    "node_positions": { "1": [0.5, 0.0, 1.0], "2": [4.0, 3.0, 1.0] },
    "last_witness": "…64 hex chars or null…",
    "effective_class": "Anonymous | Restricted | …",
    "demoted": false,
    "recalibration_recommended": false,
    "engine_error_count": 0,
    "raw_outputs_suppressed": false
  }
}
```

`node_positions` is the `--node-positions` map the governed fuser holds,
keyed by node id. It is `{}` when no positions are configured, and then every
node fuses at the origin. A node the map does not name also fuses at the
origin.

**This is a real, currently-shipped diagnostic surface — but it is honestly
limited.** It tells you *that* errors are occurring and *that* the current
class is demoted, and the total count, but not *why* for your specific run:

- `engine_error_count` is a single running total. There is **no breakdown by
  error type** anywhere in the API or in `EngineBridge`'s state — you cannot
  tell from `/health/ready` whether your 20,000 errors are 20,000
  `TimestampMismatch`es or 20,000 `DimensionMismatch`es.
- `demoted` is a boolean with no accompanying list of which
  `ContradictionFlag`s actually fired. The underlying `contradiction_flags`
  vector exists in `QualityScore` (`fusion_quality.rs:106`) but is not
  surfaced over the wire anywhere we found.
- There's no error history/timeline, and no per-node breakdown (which node
  is the one whose clock is drifting, for instance).

**The closest thing to a real diagnostic today is the rate-limited log
line itself.** Unlike the API, the log message includes the `Display` text
of the actual `EngineError`, which for `TimestampMismatch` and
`DimensionMismatch` includes the concrete numbers (e.g. `"Timestamp spread
87000 us exceeds guard interval 60000 us"`, `"Dimension mismatch: node 2 has
114 subcarriers, expected 56"`). If you're trying to diagnose a specific
demotion/error episode today, grepping the sensing-server log for
`"governed trust cycle failed"` is the most concrete answer available — the
status endpoint alone will not tell you the underlying cause. Treat this as
the honest state of the diagnostics, not a missing feature we're pretending
exists.

## 3. Is a demoted / errored state permanent? Is there a reset?

**`demoted` never needs resetting** — as described above, it's recomputed
every successful cycle from that cycle's own contradiction/mesh state. There
is no persistence, no counter, no cooldown timer for it in the code.

**`engine_error_count` has no reset mechanism at all.** It is a plain `u64`
field on `EngineBridge`, initialized to `0` in `EngineBridge::new`
(`engine_bridge.rs:111`) and only ever incremented
(`self.engine_error_count += 1;`, line 207) — there is no method, admin
endpoint, or timer anywhere in the crate that decrements or clears it. The
only way to bring it back to zero is to **restart the sensing-server
process**, which constructs a brand-new `EngineBridge`. If your count is
growing and you want to confirm whether a fix actually worked, restart the
server and watch whether the count starts climbing again — there is
currently no lighter-weight way to "clear the counter" without a restart.

**If demotion (or errors) are persistent rather than one-off**, the
documented, real fix for the most common cause — clock drift between nodes
exceeding the fixed 60 ms hard guard — is an environment-variable override,
not a restart or a wait:

- `WDP_GUARD_INTERVAL_US` — directly overrides the hard guard (e.g.
  `WDP_GUARD_INTERVAL_US=200000` for a 200 ms guard). This is the escape
  hatch a real deployment (issue #1049) needed: WiFi/ESP-NOW-synced ESP32
  nodes were measured drifting 10–150 ms, which the published 60 ms default
  could not absorb, causing **every** cycle to demote with "no escape hatch"
  (see the comment at `main.rs:8336-8339`).
- `WDP_SOFT_GUARD_US` — optionally overrides the soft (tolerated-contradiction)
  guard, always clamped below the hard guard.
- `WDP_TDM_SLOTS` + `WDP_TDM_SLOT_US` — derive the guard from your actual TDM
  schedule instead of setting it directly.

See `multistatic_guard_config_from_env` / `multistatic_guard_config_from`
(`main.rs:6791-6856`) for the exact precedence rules (a direct
`WDP_GUARD_INTERVAL_US` always wins over the TDM-derived value).

## 4. Does a converted Hugging Face model explain this?

**We could not find a code path connecting `--convert-model` to engine
errors or trust demotion — they appear to be entirely separate subsystems.**
Saying this plainly rather than speculating:

- `--convert-model` (`main.rs:6976-7028`, `run_convert_model` /
  `load_or_convert_model` at `main.rs:6925-6974`) converts a **pose-model
  weights file** — Hugging Face `safetensors` or a `jsonl` manifest — into
  this project's own RVF binary container format, so it can be loaded via
  `--model`. This is entirely about which neural-network weights the pose
  estimator uses.
- `engine_error_count` and `demoted` come from `StreamingEngine::process_cycle`
  in `wifi-densepose-engine`, which performs **multistatic CSI sensor
  fusion** — checking node count, per-node timestamp spread, and per-node
  subcarrier dimensions across your ESP32 nodes. This code path has no
  dependency on which pose model is loaded, and `load_or_convert_model` /
  `run_convert_model` never call into `engine_bridge` or
  `StreamingEngine` at all.

Because the code shows no coupling between the two, we are not going to
invent one. Two possibilities that the code doesn't rule out, but also
doesn't confirm, if you hit both symptoms together:

- **Coincidence** — the deployment that had trouble loading/using a
  converted model separately had a fusion-timing or node-count problem
  (e.g. the #1049-style clock-drift issue, or fewer than 2 active nodes),
  unrelated to the model conversion itself.
- **A configuration change made alongside the model swap** — e.g. changing
  node count, geometry, or guard settings at the same time as switching
  models — could produce both symptoms together without the model itself
  being the cause.

If you're hitting this, the actionable step from the code is to check
`engine_error_count` and the log line's error text (per §2 above)
**independently** of whatever model you have loaded — if the errors are
`TimestampMismatch`/`DimensionMismatch`/`InsufficientNodes`, the fix is on
the sensor-fusion side (§3), not the model side, regardless of which model
produced the report.

## Quick reference

```bash
# Check current trust state
curl -s http://localhost:3000/api/v1/status | jq .trust

# Watch for the rate-limited error log line (most specific diagnostic today)
# — look for "governed trust cycle failed" in the sensing-server's stderr/log.

# If demotion/errors are persistent due to node clock drift, raise the guard:
WDP_GUARD_INTERVAL_US=200000 wifi-densepose-sensing-server ...

# The only way to reset engine_error_count is a process restart.
```

Source references for everything above:
- `v2/crates/wifi-densepose-sensing-server/src/engine_bridge.rs`
- `v2/crates/wifi-densepose-sensing-server/src/main.rs` (search `trust`, `health_ready`, `multistatic_guard_config_from`, `convert_model`)
- `v2/crates/wifi-densepose-engine/src/lib.rs`
- `v2/crates/wifi-densepose-engine/src/mesh_guard.rs`
- `v2/crates/wifi-densepose-signal/src/ruvsense/multistatic.rs`
- `v2/crates/wifi-densepose-signal/src/ruvsense/fusion_quality.rs`
