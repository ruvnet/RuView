//! Experimental whole-link latent prediction and leakage-checked pose evaluation.
//!
//! These are small, deterministic research controls inspired by CSI-JEPA,
//! WiFi-JEPA and factorized pose work. The predictor training below freezes the
//! existing RF encoder; it is not a full JEPA trainer or a pretrained model.
//! Tests use SYNTHETIC fixtures and establish no real-device accuracy.

use std::collections::BTreeSet;

use crate::encoder::{Linear, RfEncoder, WindowContext};
use crate::eval::{PartitionDim, PartitionKey, SplitManifest, StrictSplit};
use crate::heads::NUM_JOINTS;
use crate::tensor::RfTensor;
use crate::tokenizer::{RfToken, RfTokenizer, TokenizedWindow, D_IN, D_POS, GROUP_BINS};
use crate::{Result, UnifiedError};

fn invalid(message: &str) -> UnifiedError {
    UnifiedError::InvalidInput(message.into())
}

fn finite(values: impl IntoIterator<Item = f64>) -> bool {
    values.into_iter().all(|x| x.is_finite() && x.abs() <= 1e6)
}

fn validate_window(window: &TokenizedWindow) -> Result<()> {
    if window.tokens.is_empty()
        || window.tokens.len() > 65_536
        || !window.age_s.is_finite()
        || !(0.0..=1e6).contains(&window.age_s)
        || !finite(window.geometry)
    {
        return Err(invalid("invalid predictive window dimensions or context"));
    }
    let mut coordinates = BTreeSet::new();
    for token in &window.tokens {
        if !finite(token.features) || !coordinates.insert((token.link, token.group)) {
            return Err(invalid(
                "non-finite/oversized features or duplicate link-group coordinate",
            ));
        }
    }
    Ok(())
}

/// Validated set of entire antenna links withheld from the student context.
#[derive(Debug, Clone)]
pub struct LinkMask {
    links: BTreeSet<usize>,
}

impl LinkMask {
    /// Rejects unknown, repeated, empty, or all-link masks.
    pub fn new(window: &TokenizedWindow, target_links: &[usize]) -> Result<Self> {
        validate_window(window)?;
        let available: BTreeSet<_> = window.tokens.iter().map(|t| t.link).collect();
        let links: BTreeSet<_> = target_links.iter().copied().collect();
        if links.is_empty()
            || links.len() != target_links.len()
            || !links.is_subset(&available)
            || links.len() == available.len()
        {
            return Err(invalid(
                "link mask must withhold known whole links and leave context",
            ));
        }
        Ok(Self { links })
    }

    /// Returns context/target tokens without changing their link/group axes.
    /// Geometry is permitted conditioning metadata, not a masked CSI value.
    pub fn partition(&self, window: &TokenizedWindow) -> Result<(Vec<RfToken>, Vec<RfToken>)> {
        // Revalidate against the supplied window so a mask cannot be reused on
        // a different topology and silently become empty/all-masked.
        let links: Vec<_> = self.links.iter().copied().collect();
        Self::new(window, &links)?;
        let (target, context): (Vec<_>, Vec<_>) = window
            .tokens
            .iter()
            .cloned()
            .partition(|token| self.links.contains(&token.link));
        Ok((context, target))
    }
}

/// Splits raw tensor links before normalization/tokenization. This prevents a
/// withheld link from leaking into visible features through the tokenizer's
/// window-median amplitude normalization. Link/group addresses are remapped to
/// their original IDs after each independent tokenizer pass.
pub fn tokenize_link_views(
    tensor: &RfTensor,
    target_links: &[usize],
) -> Result<(TokenizedWindow, TokenizedWindow)> {
    let (links, bins, snapshots) = tensor.dims();
    let targets: BTreeSet<_> = target_links.iter().copied().collect();
    if links > 128
        || !(GROUP_BINS..=4096).contains(&bins)
        || snapshots == 0
        || tensor.data.len() > 4_194_304
        || tensor.links.len() != links
        || targets.is_empty()
        || targets.len() != target_links.len()
        || targets.len() >= links
        || targets.iter().any(|&l| l >= links)
    {
        return Err(invalid("invalid raw tensor dimensions or target-link mask"));
    }
    let tokenize = |indices: Vec<usize>| -> Result<TokenizedWindow> {
        let subset = RfTensor::new(
            tensor.modality,
            tensor.center_freq_hz,
            tensor.bandwidth_hz,
            tensor.data.select(ndarray::Axis(0), &indices),
            indices.iter().map(|&i| tensor.links[i]).collect(),
            tensor.sample_age_s,
            tensor.timestamp_ns,
            tensor.device_id.clone(),
            tensor.clock_quality,
            tensor.uncertainty,
            tensor.calibration.clone(),
        )?;
        let mut window = RfTokenizer::new().tokenize(&subset);
        for token in &mut window.tokens {
            token.link = indices[token.link];
        }
        validate_window(&window)?;
        Ok(window)
    };
    let context = tokenize((0..links).filter(|l| !targets.contains(l)).collect())?;
    let target = tokenize(targets.into_iter().collect())?;
    Ok((context, target))
}

fn validate_linear(layer: &Linear, rows: usize, cols: usize) -> bool {
    layer.rows == rows
        && layer.cols == cols
        && layer.w.len() == rows * cols
        && layer.b.len() == rows
        && finite(layer.w.iter().chain(&layer.b).copied())
}

fn validate_encoder(encoder: &RfEncoder) -> Result<()> {
    let c = encoder.cfg;
    let d = c.d_model;
    if c.d_in != D_IN
        || c.d_pos > D_POS
        || !(1..=512).contains(&d)
        || !validate_linear(&encoder.w1, d, D_IN)
        || !validate_linear(&encoder.w2, d, d)
        || !validate_linear(&encoder.w2b, d, d)
        || !validate_linear(&encoder.wg, d, 6)
        || !validate_linear(&encoder.w3, D_IN, d + c.d_pos)
        || encoder.age_w.len() != d
        || encoder.age_b.len() != d
        || !finite(encoder.age_w.iter().chain(&encoder.age_b).copied())
    {
        return Err(invalid("malformed or unsupported predictive encoder"));
    }
    Ok(())
}

/// Detached exponential-moving-average teacher. No optimizer can mutate its
/// weights through this API; updates require an explicitly supplied student.
#[derive(Debug, Clone)]
pub struct EmaTeacher {
    encoder: RfEncoder,
}

impl EmaTeacher {
    /// Starts from validated student weights (no random fallback).
    pub fn from_student(student: &RfEncoder) -> Result<Self> {
        validate_encoder(student)?;
        Ok(Self {
            encoder: student.clone(),
        })
    }

    /// Immutable teacher encoder, for evaluation or serialization by the host.
    #[must_use]
    pub fn encoder(&self) -> &RfEncoder {
        &self.encoder
    }

    /// Applies `teacher = momentum * teacher + (1-momentum) * student`.
    /// Dimension/input validation completes before any teacher mutation.
    pub fn update(&mut self, student: &RfEncoder, momentum: f64) -> Result<()> {
        validate_encoder(student)?;
        if !momentum.is_finite()
            || !(0.0..=1.0).contains(&momentum)
            || self.encoder.cfg.d_model != student.cfg.d_model
            || self.encoder.cfg.d_pos != student.cfg.d_pos
        {
            return Err(invalid("invalid EMA momentum or encoder dimensions"));
        }
        fn blend(to: &mut [f64], from: &[f64], m: f64) {
            for (target, value) in to.iter_mut().zip(from) {
                *target = m * *target + (1.0 - m) * value;
            }
        }
        for (to, from) in [
            (&mut self.encoder.w1, &student.w1),
            (&mut self.encoder.w2, &student.w2),
            (&mut self.encoder.w2b, &student.w2b),
            (&mut self.encoder.wg, &student.wg),
            (&mut self.encoder.w3, &student.w3),
        ] {
            blend(&mut to.w, &from.w, momentum);
            blend(&mut to.b, &from.b, momentum);
        }
        blend(&mut self.encoder.age_w, &student.age_w, momentum);
        blend(&mut self.encoder.age_b, &student.age_b, momentum);
        Ok(())
    }
}

/// One owned context/latent-target pair. Owning the vectors makes the teacher
/// target detached from subsequent encoder updates.
#[derive(Debug, Clone)]
pub struct LatentExample {
    /// Student representation computed only from visible antenna links.
    pub context: Vec<f64>,
    /// Teacher's pooled latent representation of the withheld link tokens.
    pub target: Vec<f64>,
}

/// Encodes visible and held-out tokens separately, preserving original token
/// axes and geometry. This low-level API assumes preprocessing has not mixed
/// held-out CSI into visible features. Prefer [`latent_example_from_tensor`]
/// when starting with raw captures. The target is a pooled held-out latent,
/// not raw CSI or a full-transformer per-token JEPA target.
pub fn latent_example(
    student: &RfEncoder,
    teacher: &EmaTeacher,
    window: &TokenizedWindow,
    mask: &LinkMask,
) -> Result<LatentExample> {
    validate_encoder(student)?;
    if student.cfg.d_model != teacher.encoder.cfg.d_model {
        return Err(invalid("student/teacher latent dimensions differ"));
    }
    let (visible, targets) = mask.partition(window)?;
    let ctx = WindowContext::from(window);
    let context = student.forward(&visible, &[], ctx).z;
    let target = teacher.encoder.forward(&targets, &[], ctx).z;
    if !finite(context.iter().chain(&target).copied()) {
        return Err(invalid("predictive encoder produced invalid latents"));
    }
    Ok(LatentExample { context, target })
}

/// Recommended raw-capture path: masks before normalization and then computes
/// owned student context / detached teacher target representations.
pub fn latent_example_from_tensor(
    student: &RfEncoder,
    teacher: &EmaTeacher,
    tensor: &RfTensor,
    target_links: &[usize],
) -> Result<LatentExample> {
    validate_encoder(student)?;
    if student.cfg.d_model != teacher.encoder.cfg.d_model {
        return Err(invalid("student/teacher latent dimensions differ"));
    }
    let (visible, targets) = tokenize_link_views(tensor, target_links)?;
    let context = student.encode(&visible);
    let target = teacher.encoder.encode(&targets);
    if !finite(context.iter().chain(&target).copied()) {
        return Err(invalid("predictive encoder produced invalid latents"));
    }
    Ok(LatentExample { context, target })
}

/// Linear latent predictor with an exact SGD update. Training this module
/// alone freezes both encoders; it is a reproducible objective control.
#[derive(Debug, Clone)]
pub struct LatentPredictor {
    layer: Linear,
}

impl LatentPredictor {
    /// Deterministic predictor initialization, limited to the encoder budget.
    pub fn new(dimension: usize, seed: u64) -> Result<Self> {
        if !(1..=512).contains(&dimension) {
            return Err(invalid("invalid latent predictor dimension"));
        }
        Ok(Self {
            layer: Linear::new(&mut crate::math::seeded_rng(seed), dimension, dimension),
        })
    }

    fn validate_example(&self, example: &LatentExample) -> Result<()> {
        if example.context.len() != self.layer.cols
            || example.target.len() != self.layer.rows
            || !finite(example.context.iter().chain(&example.target).copied())
        {
            return Err(invalid("invalid latent predictor example"));
        }
        Ok(())
    }

    /// Mean-squared latent prediction error. It is not a recognition metric.
    pub fn loss(&self, example: &LatentExample) -> Result<f64> {
        self.validate_example(example)?;
        let prediction = self.layer.forward(&example.context);
        let loss = prediction
            .iter()
            .zip(&example.target)
            .map(|(p, t)| (p - t).powi(2))
            .sum::<f64>()
            / prediction.len() as f64;
        if !loss.is_finite() {
            return Err(invalid("non-finite predictive loss"));
        }
        Ok(loss)
    }

    /// One exact predictor-only SGD step, returning loss before the update.
    /// An invalid or overflowing step leaves the predictor unchanged.
    pub fn train_step(&mut self, example: &LatentExample, lr: f64) -> Result<f64> {
        let loss = self.loss(example)?;
        if !lr.is_finite() || !(0.0..=1.0).contains(&lr) {
            return Err(invalid("predictor learning rate must be in [0,1]"));
        }
        let prediction = self.layer.forward(&example.context);
        let delta: Vec<_> = prediction
            .iter()
            .zip(&example.target)
            .map(|(p, t)| 2.0 * (p - t) / prediction.len() as f64)
            .collect();
        let mut gradient = self.layer.zeros_like();
        self.layer
            .accumulate_grad(&mut gradient, &delta, &example.context);
        let mut candidate = self.layer.clone();
        candidate.sgd(&gradient, lr);
        if !validate_linear(&candidate, self.layer.rows, self.layer.cols) {
            return Err(invalid("predictor update exceeds finite parameter bounds"));
        }
        self.layer = candidate;
        Ok(loss)
    }
}

/// Verifies index boundaries, the declared holdout, and pretraining isolation.
/// Pretraining may overlap supervised training, but never test sessions or
/// held-out values of the selected partition dimension.
pub fn validate_protocol(
    keys: &[PartitionKey],
    split: &StrictSplit,
    pretraining: &[usize],
) -> Result<SplitManifest> {
    if split.train.is_empty() || split.test.is_empty() {
        return Err(invalid(
            "evaluation requires non-empty train and test partitions",
        ));
    }
    for indices in [&split.train[..], &split.test[..], pretraining] {
        if indices.iter().any(|&i| i >= keys.len())
            || indices.iter().copied().collect::<BTreeSet<_>>().len() != indices.len()
        {
            return Err(invalid("out-of-range or repeated split index"));
        }
    }
    for key in keys {
        if [
            &key.room,
            &key.day,
            &key.person,
            &key.chipset,
            &key.firmware,
            &key.layout,
            &key.session,
        ]
        .iter()
        .any(|v| v.trim().is_empty())
        {
            return Err(invalid("evaluation partition identity is missing"));
        }
    }
    let manifest = SplitManifest::build(keys, &split.train, &split.test);
    if !manifest.is_disjoint(split.dim) || !manifest.is_disjoint(PartitionDim::Session) {
        return Err(invalid("train/test holdout or capture session leakage"));
    }
    if !pretraining.is_empty() {
        let pretrain_manifest = SplitManifest::build(keys, pretraining, &split.test);
        if !pretrain_manifest.is_disjoint(split.dim)
            || !pretrain_manifest.is_disjoint(PartitionDim::Session)
        {
            return Err(invalid(
                "pretraining contains held-out domains or test sessions",
            ));
        }
    }
    Ok(manifest)
}

/// Absolute 17-joint pose in metres, sharing the existing factorized head shape.
pub type Pose = [[f64; 3]; NUM_JOINTS];

/// Separate geometry metrics prevent root translation from hiding skeleton error.
#[derive(Debug, Clone, Copy)]
pub struct PoseMetrics {
    /// Absolute mean per-joint error, metres.
    pub absolute_mpjpe_m: f64,
    /// MPJPE after subtracting each pose's selected root, metres.
    pub root_relative_mpjpe_m: f64,
    /// Root localization Euclidean error, metres.
    pub root_error_m: f64,
    /// Fraction of absolute joints within the explicit metre threshold.
    pub pck: f64,
    /// Threshold used for PCK, metres.
    pub pck_threshold_m: f64,
}

/// Model metrics and the mandatory mean-pose control, with holdout evidence.
#[derive(Debug, Clone)]
pub struct PoseComparison {
    /// Held-out model metrics.
    pub model: PoseMetrics,
    /// Held-out metrics of an absolute mean pose computed from training only.
    pub mean_pose: PoseMetrics,
    /// Dimensions that the supplied split actually holds out.
    pub split_manifest: SplitManifest,
}

/// Evaluates held-out predictions and a train-only mean-pose baseline. Predictions
/// and truth are indexed by the same provenance manifest; no fitting uses test
/// labels. Supply `FactorizedPoseHead::predict(...).absolute_joints_m()` outputs.
pub fn evaluate_pose(
    keys: &[PartitionKey],
    split: &StrictSplit,
    pretraining: &[usize],
    predictions: &[Pose],
    truth: &[Pose],
    root_joint: usize,
    pck_threshold_m: f64,
) -> Result<PoseComparison> {
    let manifest = validate_protocol(keys, split, pretraining)?;
    if predictions.len() != keys.len()
        || truth.len() != keys.len()
        || root_joint >= NUM_JOINTS
        || !pck_threshold_m.is_finite()
        || !(0.0..=100.0).contains(&pck_threshold_m)
        || pck_threshold_m == 0.0
        || !finite(
            predictions
                .iter()
                .chain(truth)
                .flat_map(|p| p.iter().flatten())
                .copied(),
        )
    {
        return Err(invalid("invalid pose arrays, root joint or PCK threshold"));
    }
    let mut mean = [[0.0; 3]; NUM_JOINTS];
    for &i in &split.train {
        for j in 0..NUM_JOINTS {
            for k in 0..3 {
                mean[j][k] += truth[i][j][k] / split.train.len() as f64;
            }
        }
    }
    fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
        (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
    }
    let score = |baseline: bool| {
        let mut absolute = 0.0;
        let mut relative = 0.0;
        let mut root = 0.0;
        let mut correct = 0;
        for &i in &split.test {
            let prediction = if baseline { &mean } else { &predictions[i] };
            root += distance(prediction[root_joint], truth[i][root_joint]);
            for j in 0..NUM_JOINTS {
                let error = distance(prediction[j], truth[i][j]);
                absolute += error;
                correct += usize::from(error <= pck_threshold_m);
                let p = std::array::from_fn(|k| prediction[j][k] - prediction[root_joint][k]);
                let t = std::array::from_fn(|k| truth[i][j][k] - truth[i][root_joint][k]);
                relative += distance(p, t);
            }
        }
        let n = split.test.len() as f64;
        PoseMetrics {
            absolute_mpjpe_m: absolute / (n * NUM_JOINTS as f64),
            root_relative_mpjpe_m: relative / (n * NUM_JOINTS as f64),
            root_error_m: root / n,
            pck: correct as f64 / (n * NUM_JOINTS as f64),
            pck_threshold_m,
        }
    };
    Ok(PoseComparison {
        model: score(false),
        mean_pose: score(true),
        split_manifest: manifest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::EncoderConfig;

    fn window() -> TokenizedWindow {
        TokenizedWindow {
            tokens: (0..3)
                .flat_map(|l| {
                    (0..2).map(move |g| RfToken {
                        features: [0.1 * (1 + l + g) as f64; D_IN],
                        link: l * 7,
                        group: g,
                    })
                })
                .collect(),
            age_s: 0.2,
            geometry: [0.1, 0.2, 0.3, 0.7, 0.8, 0.9],
        }
    }

    fn encoder() -> RfEncoder {
        RfEncoder::new(
            EncoderConfig {
                d_model: 4,
                ..Default::default()
            },
            19,
        )
    }

    #[test]
    fn raw_mask_precedes_normalization_so_target_csi_cannot_leak() {
        use crate::tensor::{CalibrationMeta, LinkGeometry, RfModality};
        use num_complex::Complex64;
        let mut tensor = RfTensor::new(
            RfModality::Synthetic,
            2.4e9,
            20e6,
            ndarray::Array3::from_shape_fn((3, 56, 8), |(l, b, s)| {
                Complex64::new(
                    0.1 + 0.1 * l as f64 + 0.003 * b as f64 + 0.02 * s as f64,
                    0.01,
                )
            }),
            (0..3)
                .map(|l| LinkGeometry {
                    tx_pos: [l as f64, 0.0, 1.0],
                    rx_pos: [l as f64, 2.0, 1.0],
                })
                .collect(),
            0.0,
            1,
            "synthetic".into(),
            1.0,
            0.0,
            CalibrationMeta::default(),
        )
        .unwrap();
        let student = encoder();
        let teacher = EmaTeacher::from_student(&student).unwrap();
        let before = latent_example_from_tensor(&student, &teacher, &tensor, &[1, 2]).unwrap();
        for l in [1, 2] {
            for b in 0..56 {
                for s in 0..8 {
                    tensor.data[[l, b, s]].re += 0.7 + b as f64 * 0.01;
                }
            }
        }
        let after = latent_example_from_tensor(&student, &teacher, &tensor, &[1, 2]).unwrap();
        assert_eq!(
            before.context, after.context,
            "raw masked CSI leaked through normalization"
        );
        assert_ne!(before.target, after.target);
        let (visible, targets) = tokenize_link_views(&tensor, &[1, 2]).unwrap();
        assert!(visible.tokens.iter().all(|t| t.link == 0));
        assert!(targets.tokens.iter().all(|t| t.link == 1 || t.link == 2));
    }

    #[test]
    fn whole_link_mask_preserves_axes_and_excludes_all_target_values() {
        let student = encoder();
        let teacher = EmaTeacher::from_student(&student).unwrap();
        let mut w = window();
        let mask = LinkMask::new(&w, &[7]).unwrap();
        let (visible, target) = mask.partition(&w).unwrap();
        assert_eq!(
            visible
                .iter()
                .map(|t| (t.link, t.group))
                .collect::<Vec<_>>(),
            vec![(0, 0), (0, 1), (14, 0), (14, 1)]
        );
        assert_eq!(target.len(), 2);
        let before = latent_example(&student, &teacher, &w, &mask).unwrap();
        for token in &mut w.tokens {
            if token.link == 7 {
                token.features.fill(9.0);
            }
        }
        let after = latent_example(&student, &teacher, &w, &mask).unwrap();
        assert_eq!(
            before.context, after.context,
            "target leaked into student context"
        );
        assert_ne!(before.target, after.target);
        w.geometry[0] += 1.0;
        assert_ne!(
            after.context,
            latent_example(&student, &teacher, &w, &mask)
                .unwrap()
                .context
        );
    }

    #[test]
    fn invalid_masks_features_and_encoder_dimensions_fail_without_panics() {
        let mut w = window();
        for links in [vec![], vec![777], vec![7, 7], vec![0, 7, 14]] {
            assert!(LinkMask::new(&w, &links).is_err());
        }
        w.tokens[0].features[0] = f64::NAN;
        assert!(LinkMask::new(&w, &[7]).is_err());
        let mut e = encoder();
        e.w1.w.pop();
        assert!(EmaTeacher::from_student(&e).is_err());
    }

    #[test]
    fn ema_is_exact_detached_and_invalid_updates_are_atomic() {
        let mut student = encoder();
        let mut teacher = EmaTeacher::from_student(&student).unwrap();
        let original = student.w1.w[0];
        student.w1.w[0] += 2.0;
        student.age_b[0] = 2.0;
        assert_eq!(teacher.encoder().w1.w[0], original);
        teacher.update(&student, 0.75).unwrap();
        assert!((teacher.encoder().w1.w[0] - original - 0.5).abs() < 1e-12);
        assert_eq!(teacher.encoder().age_b[0], 0.5);
        let before = teacher.encoder().w1.w.clone();
        assert!(teacher.update(&student, f64::NAN).is_err());
        student.age_b[0] = f64::INFINITY;
        assert!(teacher.update(&student, 0.9).is_err());
        assert_eq!(teacher.encoder().w1.w, before);
    }

    #[test]
    fn predictor_sgd_reduces_fixed_latent_objective_without_mutating_encoder() {
        let student = encoder();
        let teacher = EmaTeacher::from_student(&student).unwrap();
        let w = window();
        let mask = LinkMask::new(&w, &[7]).unwrap();
        let pair = latent_example(&student, &teacher, &w, &mask).unwrap();
        let before_encoder = student.w1.w.clone();
        let mut predictor = LatentPredictor::new(4, 33).unwrap();
        let initial = predictor.loss(&pair).unwrap();
        for _ in 0..100 {
            predictor.train_step(&pair, 0.1).unwrap();
        }
        assert!(predictor.loss(&pair).unwrap() < initial * 0.1);
        assert_eq!(student.w1.w, before_encoder);
        let before = predictor.layer.w.clone();
        assert!(predictor.train_step(&pair, 2.0).is_err());
        assert_eq!(before, predictor.layer.w);
    }

    fn keys() -> Vec<PartitionKey> {
        (0..4)
            .map(|i| PartitionKey {
                room: format!("room-{}", i / 2),
                day: format!("day-{i}"),
                person: format!("person-{i}"),
                chipset: format!("chip-{i}"),
                firmware: format!("fw-{i}"),
                layout: format!("layout-{i}"),
                session: format!("session-{i}"),
            })
            .collect()
    }

    #[test]
    fn protocol_rejects_test_pretraining_sessions_and_invalid_indices() {
        let mut k = keys();
        let mut split = StrictSplit::holdout(&k, PartitionDim::Room, &["room-1"]);
        assert!(validate_protocol(&k, &split, &[0, 1]).is_ok());
        assert!(validate_protocol(&k, &split, &[2]).is_err());
        split.train.push(500);
        assert!(validate_protocol(&k, &split, &[]).is_err());
        split.train.pop();
        k[2].session = k[0].session.clone();
        assert!(validate_protocol(&k, &split, &[]).is_err());
    }

    #[test]
    fn pose_metrics_separate_translation_and_use_training_only_mean() {
        let k = keys();
        let split = StrictSplit::holdout(&k, PartitionDim::Room, &["room-1"]);
        let mut truth = vec![[[0.0; 3]; NUM_JOINTS]; 4];
        for pose in truth.iter_mut().skip(2) {
            for joint in pose {
                joint[0] = 10.0;
            }
        }
        let mut predictions = truth.clone();
        for pose in predictions.iter_mut().skip(2) {
            for joint in pose {
                joint[0] += 1.0;
            }
        }
        let metrics = evaluate_pose(&k, &split, &[0], &predictions, &truth, 0, 0.2).unwrap();
        assert_eq!(metrics.model.absolute_mpjpe_m, 1.0);
        assert_eq!(metrics.model.root_relative_mpjpe_m, 0.0);
        assert_eq!(metrics.model.root_error_m, 1.0);
        assert_eq!(metrics.model.pck, 0.0);
        assert_eq!(
            metrics.mean_pose.absolute_mpjpe_m, 10.0,
            "held-out labels must not change the training-only mean pose"
        );
        assert!(evaluate_pose(&k, &split, &[], &predictions, &truth, NUM_JOINTS, 0.2).is_err());
        assert!(evaluate_pose(&k, &split, &[], &predictions, &truth, 0, f64::NAN).is_err());
    }
}
