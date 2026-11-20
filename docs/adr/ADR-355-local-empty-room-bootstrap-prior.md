# ADR 355: Local empty room bootstrap prior

Status: Accepted

Date: 2026-09-04

## Context

Most installations are occupied when RuView first starts. An uncalibrated
signal heuristic can interpret HVAC, clock error, neighboring motion, or a
short covariance window as a person. It can also surface plausible numeric
vitals before a room model establishes occupant attribution.

One empty home is not representative training data for all homes and contains
no positive human examples. It is useful only as a recent negative prior for
the same installation. It cannot define a universal human signal model.

The field model was memory only. The server also used a relative `data/`
directory, which is unreliable for embedded application hosts.

## Decision

RuView may persist a completed empty room field model as a local bootstrap
prior only after two server controlled stages:

1. Normal calibration reaches both 1,000 accepted frames and 600 seconds.
   The frame target represents twenty complete fifty frame runtime windows.
   A caller may select one currently live ESP32 source. The server checks its
   freshness before replacing an active bootstrap prior, binds collection to
   that source, and rejects frames from other radios. When no source is
   selected, the first accepted ESP32 source preserves legacy behavior.
2. Promotion observes 12 new samples spaced by at least one second. Each
   sample has a four second bounded acquisition timeout. At least 10 must
   resolve empty, all ticks must be fresh, and none may publish numeric vital
   signs.

The stored image contains aggregate means, environmental modes, mode energies,
noise statistics, sorted scalar residual references, configuration, one source
node ID, and a server generated model ID. It excludes raw CSI, waveforms,
device addresses, room names, pose, identity, credentials, and positive human
labels. Runtime matching compares a fifty frame mean with the learned residual
distribution. Its score is empirical background conformance, not an absence
probability.

The image is bound to a SHA 256 digest of the stable installation ID, carries
an integrity digest over the exact stored payload bytes, is capped at 1 MiB,
and inherits the field model expiry.
Files with an invalid schema, dimensions, values, node set, installation
binding, digest, size, or lifetime fail closed.

Restoring an image creates a field model in `bootstrap_only` mode. It may
reduce an uncalibrated false person count, but it cannot authorize calibrated
evidence or numeric heart and breathing rates. Explicit calibration replaces
it with fresh statistics. Cancelling an unfinished capture restores a valid
prior. Administrator scoped reset removes it.

A restored image may receive one operator confirmed held out empty refinement.
The server measures 12 fresh, source bound runtime residuals at one second
spacing and requires zero stale samples and zero numeric vital samples. It
persists only the scalar residuals. The new boundary is the larger of the
existing threshold and 110 percent of the held out 95th percentile, capped at
125 percent of the prior threshold. The one use marker persists in the model,
so repeated requests cannot ratchet the threshold upward.

The status endpoint reports the learned runtime window, residual threshold,
reference count, current background conformance, and the negative only
authority boundary. A restored model never resumes collection and never
persists raw calibration frames.

The server accepts `--data-dir` or `RUVIEW_DATA_DIR`, allowing an application
host to provide a protected state directory.

## Consequences

Startup can use recent measured background structure instead of only generic
heuristics. No accuracy lift is claimed until occupied and empty held out data
from multiple installations is recorded.

The largest failure mode is overfitting to a temporarily quiet home and
suppressing a weak occupant. Controls are low authority, 24 hour expiry,
installation binding, vital abstention, explicit recalibration, and reset.

A human signal model still requires positive, consented, leakage separated
occupied sequences. Empty data is never relabeled as human training.

## Implementation

1. `wifi-densepose-signal::ruvsense::field_model` exports and validates
   `FieldModelSnapshotV1`.
2. `wifi-densepose-sensing-server::bootstrap_baseline` owns schema, integrity,
   storage, lifetime, and the held out gate.
3. `POST /api/v1/calibration/bootstrap/promote` measures the validation window
   and stores the image only on pass.
4. `GET /api/v1/calibration/status` reports bootstrap authority and explicitly
   denies calibrated evidence and numeric vital authority.
5. Numeric vitals fail closed unless an explicit fresh calibration resolves
   exactly one occupant at sufficient signal quality and confidence.
6. The legacy `edge_vitals` WebSocket message applies the same publication
   gate as `sensing_update`. When the bootstrap background matches, it emits
   absent state, zero people, null numeric rates, and an explicit abstention
   reason instead of forwarding raw edge candidates.
7. `POST /api/v1/calibration/start?source_node_id=N` selects a live source for
   deterministic single link capture. A missing or stale selection fails
   without disabling the active bootstrap prior.
8. `POST /api/v1/calibration/bootstrap/refine?confirmed_empty=true&source_node_id=N`
   performs the bounded one time held out refinement. It requires administrator
   authority when transport authentication is enabled.

## Measured installation evidence

One physical empty home run on source Node 5 collected 1,762 accepted frames
over 655 seconds and produced 35 privacy reduced reference windows. Promotion
observed 12 of 12 fresh empty samples, zero stale samples, and zero numeric
vital samples. After process restart, 12 of 12 fresh samples matched the empty
background and contained no numeric vital signs. Background conformance scores
ranged from 50.0 to 64.3 percent with a median of 54.3 percent.

This is measured negative evidence for one installation. No occupied positive
holdout was recorded, so it does not prove person recall, person counting, or
cross home accuracy improvement.

## Acceptance test

With a physically empty room, complete calibration on stable nodes, promote
the single source model, and restart the server with the same installation ID
and data directory. Status must report `binding_mode=bootstrap_only`, zero
collection frames, and false authority for calibrated evidence and numeric
vitals. Twelve fresh empty samples must contain no numeric vital signs.
Starting a new room calibration must replace the prior and begin at frame zero.
When several radios are live, selecting Node 5 must leave Nodes 3 and 7 out of
the source set for the entire capture.
After restart, a confirmed empty holdout may refine the restored boundary once.
The stored snapshot must reject a second refinement and must continue to deny
calibrated evidence and numeric vital authority.
