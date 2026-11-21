# RF scene and tracking experiments

The `ruview-unified::scene_research` module supplies bounded offline controls and
evaluation seams for the RISE, GRaF, 4D Point Splatting and CoTrack tracks. It does
not implement their trained networks, reproduce their reported performance, or
change the live sensing server. Current validation is **SYNTHETIC**. No hardware
or measured-dataset validation was performed for this work.

The scope follows [ADR-278](../../adr/ADR-278-radar-inverse-rendering-research-program.md):
radar inverse rendering remains a gated research program. Existing Gaussian gain
queries remain the physical prediction baseline.

| Track | Implemented research control | Required reproduction evidence |
|---|---|---|
| RISE | `GeometryCandidate` retains capture-derived, synthetic and model-completed geometry in separate admission lanes. A high-confidence completion cannot enter the capture lane; uncertainty, age and future timestamps can force abstention. | Raw compatible mmWave measurements, measured floor plans/object boxes, held-out rooms, and both raw geometric recovery and learned completion results. Report layout Chamfer distance and object IoU separately. |
| GRaF | `PropagationResidual` learns local scalar gain corrections conditioned on reciprocal TX/RX geometry and same-frequency training observations. Paired held-out metrics compare physical-only and corrected gain. | Directional measured spectra and the original neural/ray-based architecture are needed for GRaF reproduction. The scalar control must not be described as a spectrum renderer or zero-shot cross-scene foundation model. |
| 4D Point Splatting | `MovingScatterer` renders a coherent narrowband bistatic complex return from constant-velocity geometry and RCS. Doppler follows the derivative of physical path length. | Raw complex radar returns, timing/calibration, synchronized geometry initialization and the full differentiable fitting/rendering pipeline. Point-normal/material models, occlusion, multipath and range/Doppler splatting are not implemented here. |
| CoTrack | `virtual_tx_concentration` rejects competing low-cost initialization modes, including distant modes that would disappear if only neighbors of the best candidate were retained. | A multi-start nonlinear solver, delay/AoA/Doppler extraction, trajectories, the local identifiability calculation and CRLB analysis. Concentration alone is a necessary diagnostic, not proof of identifiability or a complete tracker. |

## Reproduction boundaries

RISE combines bi-angular multipath reasoning with learned scene completion. Its
reported layout/object results use a stationary mmWave radar. The provenance
gate here is a downstream bookkeeping control; it does not reconstruct walls
or objects. Source: [RISE paper](https://arxiv.org/abs/2511.14019).

GRaF conditions predictions on neighboring measured spectra and geometry and
combines learned representation with neural ray tracing. The local residual
control tests whether simple geometry-conditioned correction helps before a
larger renderer is warranted. Source: [GRaF author project](https://kangyangg.com/projects/graf/).

4D Point Splatting fits moving scenes to real radar captures with complex
signal-domain rendering and synchronized camera initialization. The analytic
single-point control provides phase, amplitude and Doppler anchors for future
fitting work. It has no learned fitting loop. Source:
[4D Point Splatting, November 2026 preprint](https://arxiv.org/abs/2611.08342).

CoTrack jointly estimates a virtual transmitter and human trajectory. Its
initialization rule requires agreement among good multi-start solutions before
online tracking. The concentration control implements that conservative gating
idea without implementing the trajectory optimizer. Source:
[CoTrack, September 2026 preprint](https://arxiv.org/abs/2609.34797).

## Leakage and evidence contract

`PropagationResidual::fit` accepts only records explicitly tagged `Train`,
rejects duplicate capture IDs and keeps training state private. Predictions do
not read the query's `observed_db`. `evaluate` is immutable and requires
`Holdout` records, rejects repeated capture IDs, and rejects training/holdout
physical-link overlap even when TX and RX are exchanged. Unsupported carrier
frequencies or distant geometries return an error representing abstention.

This is a link-level guard, not a complete room/person/session split verifier.
The caller must bind records to a trusted split manifest and use a frozen or
training-only Gaussian map to produce `physical_db`. Fitting that physical map
on held-out measurements would still leak information. The caller must also
validate capture receipts against its trusted ledger; merely supplying a
receipt does not authenticate a capture. Geometry admission never changes an
origin class and does not issue a capability certificate.

Neighbor dispersion is an uncalibrated support-spread proxy. It must not be
reported as a calibrated confidence interval. A report on actual measurements
should state support coverage and abstention rate alongside gain errors, and
must retain the baseline result when the correction worsens it.

## Run the deterministic controls

From the repository root, the module's standalone tests avoid the larger
workspace's optional hardware/submodule dependencies:

```powershell
$sceneTestExe = Join-Path $env:TEMP 'ruview-scene-research-tests.exe'
rustc --edition 2021 --test v2/crates/ruview-unified/src/scene_research.rs -o $sceneTestExe
if ($LASTEXITCODE -eq 0) { & $sceneTestExe --nocapture }
```

The checks cover capture/completion separation, stale geometry abstention,
held-out gain correction, immutable evaluation, reciprocal-link leakage,
unsupported-domain abstention, competing virtual-TX modes, and analytic
phase/Doppler/range scaling. They demonstrate implementation behavior on
deterministic fixtures; they establish no real-world accuracy.

With a complete workspace checkout, also run:

```sh
cd v2
cargo test -p ruview-unified --no-default-features
```
