# Predictive representation and factorized-pose experiments

This is an **experimental control implementation**, inspired by the papers
below. It does not reproduce their architectures, training corpora, pretrained
weights, or reported accuracy. The included fixtures are **SYNTHETIC**. No
measured-data training or hardware evaluation was performed for this module.

## Primary research and scope

| Track | Source | Experiment enabled here | Remaining reproduction requirement |
|---|---|---|---|
| CSI-JEPA | [CSI-JEPA, revision 2, November 4, 2026](https://arxiv.org/abs/2605.14171v2) | Detached EMA target and a frozen-encoder latent-prediction control; downstream dataset split validation | Temporal-spectral tokenizer, variation-aware mask, full encoder/predictor optimization, matched data/label budgets, and external task evaluation |
| WiFi-JEPA | [WiFi-JEPA, July 13, 2026](https://arxiv.org/abs/2607.11064) | Whole-link withholding before normalization; unchanged source link/group addresses | CSI-native channel/time/link transformer, full masked-latent training, Person-in-WiFi-3D-compatible annotations/hardware, and real-plus-simulation experiments |
| PerceptAlign | [Geometry-aware cross-layout pose, January 18, 2026](https://arxiv.org/abs/2601.12252) | Geometry remains explicit conditioning metadata in each visible/target view; separate root-location evaluation | Physical TX/RX calibration in the ground-truth coordinate system and complete held-out layouts |
| RePos | [Relative-to-absolute pose factorization, revision 2, July 17, 2026](https://arxiv.org/abs/2607.02986v2) | Evaluation of the existing `FactorizedPoseHead` with absolute, root-relative and root-position metrics and a train-only mean-pose baseline | Measured CSI/pose pairs, a root/joint definition compatible with the dataset, and the paper's strict cross-environment/few-shot protocols |

These sources motivate controls; their performance claims do not become RuView
results. In particular, label-efficiency improvements on classification do not
establish pose transfer, and same-domain pose error does not establish unseen-room
accuracy.

## Implementation

`v2/crates/ruview-unified/src/predictive.rs` adds:

- `tokenize_link_views` splits `RfTensor` along the link axis **before** each
  view's normalization/tokenization. The existing tokenizer computes a
  window-wide median; masking after that operation could let held-out CSI affect
  visible features. The new path avoids that contamination and restores original
  link IDs after independent tokenization.
- `LinkMask` is a lower-level token partition control. It validates that the
  selected links exist, that the mask leaves visible context, and that token
  addresses/features are valid. Callers of this lower-level API remain responsible
  for leakage-free upstream preprocessing.
- `EmaTeacher` owns detached encoder weights and accepts a validated explicit
  exponential-moving-average update. Invalid updates leave it unchanged.
- `latent_example_from_tensor` produces owned visible-context / held-out-target
  vectors, with no teacher optimizer path. Its target is the existing encoder's
  **pooled withheld-view latent**, not the papers' contextual transformer token
  target. Known geometry is valid conditioning information.
- `LatentPredictor` trains only its small linear head with an exact SGD update.
  Both encoders stay frozen in this control. Lower prediction loss verifies the
  optimization seam; it is not evidence of useful downstream features or a
  collapse-free JEPA representation.
- `validate_protocol` checks indices, nonempty train/test sets, declared holdout
  dimensions, capture-session separation, and exclusion of test domains/sessions
  from pretraining. It returns the existing `SplitManifest`; it does not claim
  that dimensions shared by a supplied protocol are held out.
- `evaluate_pose` scores the test partition and compares it with an absolute
  mean pose formed exclusively from training labels. It reports MPJPE in metres,
  root-relative MPJPE, root-position error, and absolute PCK at an explicit metre
  threshold. The caller supplies the dataset's root-joint index.

The existing `RfTokenizer` pools temporal features and performs engineered
feature extraction. It is useful as a baseline but does not preserve the full
raw temporal-spectral tensor as a transformer sequence. Its lossy representation
must not be described as a CSI-JEPA or WiFi-JEPA reproduction.

## Running and extending the experiment

Once the module is registered in `lib.rs`:

```powershell
cd v2
cargo test -p ruview-unified --no-default-features predictive::tests
```

The deterministic tests exercise withheld raw values changing the target but
not the student context, coordinate retention, detached exact EMA updates,
predictor optimization, invalid-input rejection, pretraining/session isolation,
and separate translation/skeleton errors. All fixture results remain SYNTHETIC.

For a measured-data run:

1. Import native complex RF tensors with TX/RX geometry and a provenance key for
   room, day, subject, chipset, firmware, layout and capture session. Keep capture
   data local and untracked. Construct splits before any normalization fitting,
   pretraining, model selection or adapter fitting.
2. Compare scratch, existing masked reconstruction, contrastive pretraining and
   predictive pretraining with identical architecture/data/label budgets. The
   current module supplies a predictive **control**, not all of these trainers.
3. Keep frozen-backbone and full-fine-tuning protocols separate. Use validation
   domains for hyperparameters and reserve test domains for the final evaluation.
4. Train the existing factorized head using root-relative skeleton targets and
   absolute root targets. Supply its `absolute_joints_m()` predictions to
   `evaluate_pose`; report the mean-pose control and each holdout dimension.
5. Report per-domain outcomes and uncertainty intervals, including moved
   transmitters and unseen chipsets, alongside same-domain results. Numerical
   reconstruction/prediction loss alone is insufficient for promotion.

No public benchmark dataset, externally trained model, measured ESP32 transfer
capture, or synchronized pose ground truth was downloaded or assumed available
by this implementation. Measured reproduction remains gated on those inputs.
