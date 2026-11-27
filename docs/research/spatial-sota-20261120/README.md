# Spatial research controls — 2026-11-20

This change implements bounded offline controls for the eight requested research
tracks. It does **not** reproduce the papers' trained systems, establish real-device
accuracy, or change the live sensing server. All included experiments are
**SYNTHETIC**. The independent variable in each experiment is explicit, and a
failure or abstention remains an observable result.

| Requested track | Implemented control | Remaining work before a measured claim |
|---|---|---|
| OpenCSI | Quiet-only per-link baseline, raw amplitude preservation, maturity/variance/quality/age abstention | Controlled S3/C6 capture swaps, task classifier evaluation, independent quiet gating and rate/channel-specific baseline policy |
| CSI-JEPA | Detached EMA teacher and exact predictor-only latent SGD | Full encoder training, variation-aware temporal-spectral masking and downstream label-budget comparison |
| WiFi-JEPA | Whole-link masking before normalization, original link/group identities | CSI-native temporal transformer, joint training, matched real/simulated pretraining and multi-person benchmark reproduction |
| PerceptAlign + RePos | Existing factorized-head evaluation, separate root/skeleton metrics, train-only mean-pose baseline and split checks | Measured geometry/pose calibration, factorized-head training and held-out layout/environment evaluation |
| RISE | Separate capture, completion and synthetic geometry admission | Bi-angular multipath reconstruction, learned diffusion completion, compatible radar and measured layout/object labels |
| GRaF | Training-only geometry-conditioned scalar propagation residual and held-out physical-link comparison | Directional spectra, neural field/ray rendering and cross-scene evaluation |
| 4D Point Splatting | Coherent moving-point return with physical path phase, Doppler and distance scaling | Raw radar sequences, synchronized initialization, normals/materials/occlusion and differentiable fitting |
| CoTrack | Multi-start candidate concentration and competing-mode abstention | Delay/AoA/Doppler extraction, joint trajectory/virtual-TX solver, local identifiability and CRLB analysis |

The controls intentionally retain the existing physical model, native frame and
factorized head as baselines. A smaller latent prediction loss, a corrected toy
channel gain or a concentrated candidate set is not evidence of downstream
recognition accuracy or physical identifiability.

## Source and experiment notes

- [Calibration and device transfer](calibration.md)
- [Predictive learning and pose](predictive-pose.md)
- [Scene reconstruction and tracking](scene-tracking.md)
- Source modules: `transfer_calibration.rs`, `predictive.rs`, and
  `scene_research.rs` under `v2/crates/ruview-unified/src/`.

The preferred predictive API is `latent_example_from_tensor`: masking raw links
before independently tokenizing the views prevents a withheld link from leaking
through window-wide amplitude normalization. The lower-level token API requires
the caller to establish that preprocessing isolation.

Capture receipts, quiet labels and train/holdout roles supplied to these offline
APIs remain caller assertions. They must be bound to trusted capture and split
manifests before measured evaluation. The APIs do not authenticate arbitrary
receipts, promote evidence, authorize actions or insert into the live map.

## Reproduce the controls

With a complete repository checkout and required submodules:

```sh
cd v2
cargo test -p ruview-unified --no-default-features
cargo run -p ruview-unified --example spatial_research_controls -- --synthetic
```

The example refuses to run without the explicit `--synthetic` argument. It prints
calibration behavior, predictor-only objective change, constructed pose metric
arithmetic and its train-mean baseline, held-out gain correction, separate
completion admission, an ambiguous virtual-TX initialization and an analytic
moving-point return. No dataset or trained encoder is silently substituted.

Local integration validation used an isolated Cargo workspace pointing to the
original core/unified library sources and the existing acceptance/security
integration tests. Production dependencies were retained. The unrelated hardware
development dependency, hardware example and benchmarks were excluded from that
temporary workspace. The normal repository command stopped at the absent
`vendor/rufield/crates/rufield-adapters/Cargo.toml` manifest before compilation.
Consequently this validation covers the unified source and controls, not the full
repository or hardware example.

## Swarm execution and coordination

Three actual concurrent Codex workers owned separate source files for calibration,
predictive/pose and scene/tracking. The parent integrated the modules, added the
explicit synthetic example, ran integration tests and reviewed the diff. Ruflo
MCP supplied persistent swarm/agent/task state and shared memory; registry entries
were mapped to the actual workers and were not mistaken for LLM execution.

The connected discovery endpoint returned MCP `-32603`; local stdio MCP discovery
and coordination succeeded. A separately configured worker task-update endpoint
refused writes because its existing policy ledger lacked an anchor. No anchor or
permission was changed to clear that refusal. Completion was recorded through
the already-working local MCP in the isolated worktree, and the local swarm was
shut down gracefully. Local coordinator state and private indexes are excluded
from the source changes.

## Measured-data acceptance

Keep complete rooms, days, people, chipsets, firmware, antenna layouts and capture
sessions available in the split manifest. Fit preprocessing, encoders, adapters,
physical maps and residuals only on their permitted training partitions; use
validation partitions for selection. Report which dimensions are actually held
out, the mean-pose baseline, explicit PCK thresholds, root and skeleton errors,
per-domain results, coverage/abstention and confidence intervals. Preserve the
physical-only result when a learned residual worsens it. Hardware performance
requires target-device captures and a real boot/runtime log.
