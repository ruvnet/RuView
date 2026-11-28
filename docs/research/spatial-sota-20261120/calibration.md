# Quiet-baseline transfer calibration experiment

This track implements an **OpenCSI-inspired experimental primitive**, not an
OpenCSI reproduction or a deployment accuracy claim. Its deterministic fixtures
are **SYNTHETIC**. No S3/C6 transfer captures or target-device validation were run.

The primary [OpenCSI v2 paper](https://arxiv.org/html/2607.26665v2), revised August
4, 2026, learns quiet-period temporal moments and exposes unsigned amplitude
deviation in baseline standard deviations. Its reported transfer is scoped to
binary occupancy; additive noise and loss of absolute magnitude remain limits.
The [authors' code](https://github.com/sensing-group/OpenCSI) additionally provides
automatic quiet detection, phase processing, rate/time buckets and reliability
scoring. Those components have not been ported here, and none of the authors'
hardware accuracy results are RuView results.

## Implementation

`v2/crates/ruview-unified/src/transfer_calibration.rs` provides one
`LinkTransferCalibrator` per link and hardware/rate/channel configuration:

- A trusted upstream `QuietState::Quiet` gate admits baseline samples. Occupied,
  unknown, insufficient-validity and low-quality observations never train.
- Welford moments describe the temporal distribution of the valid-bin amplitude
  mean. The derived scalar is `abs(current_mean - quiet_mean) / quiet_std`.
- Outputs retain the original amplitude vector, explicit mask and absolute mean.
  Magnitude-dependent tasks can consume that separate view instead of trying to
  reconstruct magnitude from a Z-score.
- Count, elapsed quiet span, baseline age, variance, SNR, loss and interference
  determine whether a score is available. No epsilon-clamped zero-variance
  denominator silently creates a usable feature. Expired baselines must warm
  again before scoring resumes.
- Existing `FrameProvenance`, evidence levels and `SignalQuality` are reused.
  Device, firmware, native bin count and provenance class changes reject reuse.
  Output evidence is capped at the weakest input/baseline level. Synthetic
  inputs remain L0; this module cannot create measured evidence or certificates.
- Invalid observations leave state unchanged. Samples are bounded in size;
  only moments and identity metadata persist in the calibrator.

Channel, link geometry and sampling-rate bucket identity are owned by the caller.
The integration must allocate a new calibrator when any changes. The module does
not infer those properties from scalar amplitude. It also deliberately avoids
assuming that low temporal variance proves an empty room: quietness must be
established independently. A still occupant can otherwise contaminate bootstrap.

The default maturity span is a policy starting point, not an experimentally
validated optimum. Freeze all thresholds using training/calibration sessions;
changing them after reading held-out metrics invalidates the experiment.

## Verification and limits

Unit tests exercise affine gain/offset transfer while retaining native magnitude,
occupied/unknown/poor-quality contamination prevention, missing/warming/stale and
variance abstention, stale rewarming, mask preservation, transactional input
validation and provenance consistency. This establishes **SYNTHETIC behavior of
the primitive**, not real-hardware generalization.

After module registration, run:

```powershell
cd v2
cargo test -p ruview-unified --no-default-features transfer_calibration --lib
```

In this isolated worktree, the workspace test initially stops before compilation
because `vendor/rufield/crates/rufield-adapters/Cargo.toml` is absent. A temporary
`rustc --test` harness verified the module against the repository's actual
provenance/quality type declarations with serialization derives removed. That
focused check is not a full crate build.

## Required controlled S3/C6 experiment

1. Hold room, TX/RX positions, antenna orientation and carrier fixed. Record chip,
   firmware, bandwidth, rate bucket, packet loss and the independent quiet gate.
2. Establish empty-room baselines separately on S3 and C6. Include deliberately
   occupied cold starts to verify abstention rather than accidental learning.
3. Freeze the decision threshold and calibration policy on source sessions.
   Compare raw amplitude, session normalization and quiet-baseline Z-score under
   the same downstream model and label budget.
4. Hold out whole days/sessions, people and device swaps. Include changed furniture,
   interference and low-SNR cells; report all cells, including failed transfers.
5. Report binary F1, false-alarm rate per hour, abstention coverage, maturity time,
   drift recovery and baseline contamination. Test magnitude-dependent static vs
   moving behavior using the preserved absolute feature separately.
6. Bind captured receipts to `FrameProvenance` and the split manifest. Promote an
   evidence grade only with actual target-device captures and reviewed evaluation.

No automatic quiet detector, phase sanitization, occupancy classifier, signed
calibration certificate, firmware change, or live-server integration is supplied
by this module. Those are separate integration/reproduction tasks.
