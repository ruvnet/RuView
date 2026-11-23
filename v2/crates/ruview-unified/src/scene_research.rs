//! Bounded offline research primitives for RF scene experiments.
//!
//! These are controls and integration seams inspired by RISE, GRaF, 4D Point
//! Splatting and CoTrack, not implementations or reproductions of those models.
//! They never update the live scene map or establish hardware accuracy. Captured
//! observations and model completions remain distinct throughout admission.

use std::collections::BTreeSet;

/// Errors at an experimental input boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneResearchError(pub String);

impl std::fmt::Display for SceneResearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for SceneResearchError {}

type Result<T> = std::result::Result<T, SceneResearchError>;

fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(SceneResearchError(message.into()))
    }
}

fn finite_point(p: [f64; 3]) -> bool {
    p.iter().all(|v| v.is_finite() && v.abs() <= 1e6)
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// Evidence origin. This class is retained rather than inferred from confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryOrigin {
    /// Geometry inferred from a real capture, still subject to uncertainty gates.
    Capture,
    /// Geometry inferred from a synthetic capture.
    Synthetic,
    /// Geometry filled in by a model rather than supported by a capture.
    Completion,
}

/// Immutable geometry candidate with an explicit provenance class.
#[derive(Debug, Clone)]
pub struct GeometryCandidate {
    origin: GeometryOrigin,
    position_m: [f64; 3],
    sigma_m: f64,
    timestamp_ns: u64,
    source_receipt: Option<u128>,
}

impl GeometryCandidate {
    /// Create a candidate; only capture-derived candidates require a receipt.
    /// Receipt validation against a trusted capture ledger belongs to the caller.
    pub fn new(
        origin: GeometryOrigin,
        position_m: [f64; 3],
        sigma_m: f64,
        timestamp_ns: u64,
        source_receipt: Option<u128>,
    ) -> Result<Self> {
        require(finite_point(position_m), "invalid geometry position")?;
        require(
            sigma_m.is_finite() && sigma_m > 0.0 && sigma_m <= 1e4,
            "geometry uncertainty must be finite and positive",
        )?;
        if origin != GeometryOrigin::Completion {
            require(
                source_receipt.is_some_and(|r| r != 0),
                "capture receipt is required",
            )?;
        }
        Ok(Self {
            origin,
            position_m,
            sigma_m,
            timestamp_ns,
            source_receipt,
        })
    }

    /// Original evidence class; confidence or repeated predictions cannot change it.
    #[must_use]
    pub fn origin(&self) -> GeometryOrigin {
        self.origin
    }

    /// Position in the shared building frame, metres.
    #[must_use]
    pub fn position_m(&self) -> [f64; 3] {
        self.position_m
    }

    /// Source capture receipt, when supplied. This is lineage, not authentication.
    #[must_use]
    pub fn source_receipt(&self) -> Option<u128> {
        self.source_receipt
    }

    /// Admit into a separate observed/completed/synthetic lane, or abstain.
    /// This does not insert into GaussianMap or authorize downstream actions.
    pub fn disposition(
        &self,
        now_ns: u64,
        max_age_ns: u64,
        max_sigma_m: f64,
    ) -> Result<GeometryDisposition> {
        require(
            max_sigma_m.is_finite() && max_sigma_m > 0.0,
            "invalid geometry uncertainty threshold",
        )?;
        if self.timestamp_ns > now_ns
            || now_ns - self.timestamp_ns > max_age_ns
            || self.sigma_m > max_sigma_m
        {
            return Ok(GeometryDisposition::Abstain);
        }
        Ok(match self.origin {
            GeometryOrigin::Capture => GeometryDisposition::CaptureDerived,
            GeometryOrigin::Synthetic => GeometryDisposition::SyntheticOnly,
            GeometryOrigin::Completion => GeometryDisposition::CompletionOnly,
        })
    }
}

/// Separate admission lanes. None of these values certifies measured accuracy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryDisposition {
    /// A sufficiently fresh, uncertainty-gated real-capture inference.
    CaptureDerived,
    /// Synthetic-only evidence; never enter the real observation lane.
    SyntheticOnly,
    /// Model completion; retain separately from observed geometry.
    CompletionOnly,
    /// Stale, future-dated or overly uncertain evidence.
    Abstain,
}

/// Explicit data role, checked at fitting and held-out evaluation boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRole {
    /// Source training observations.
    Train,
    /// Observations reserved for held-out evaluation.
    Holdout,
}

/// A scalar link observation around an existing physical gain prediction.
///
/// `physical_db` should come from `gaussian::gain_db` on a training-only or
/// frozen scene map. Metadata and the map's own training split require a trusted
/// external manifest; relabeling data cannot be detected by this scalar seam.
#[derive(Debug, Clone)]
pub struct LinkObservation {
    /// Globally unique capture-window identity, not a file basename.
    pub capture_id: String,
    /// Explicit training/evaluation role.
    pub role: DataRole,
    /// Transmitter position in metres.
    pub tx_m: [f64; 3],
    /// Receiver position in metres.
    pub rx_m: [f64; 3],
    /// Actual carrier frequency in Hz.
    pub frequency_hz: f64,
    /// Frozen physical-model gain in dB, before learned correction.
    pub physical_db: f64,
    /// Captured or synthetic reference gain, in the same units.
    pub observed_db: f64,
    /// Reference uncertainty, positive standard deviation in dB.
    pub sigma_db: f64,
}

impl LinkObservation {
    fn validate(&self) -> Result<()> {
        require(
            !self.capture_id.is_empty() && self.capture_id.len() <= 256,
            "capture id must contain 1..=256 bytes",
        )?;
        require(
            finite_point(self.tx_m)
                && finite_point(self.rx_m)
                && distance(self.tx_m, self.rx_m) >= 1e-3,
            "invalid link geometry",
        )?;
        require(
            self.frequency_hz.is_finite() && (1e6..=1e12).contains(&self.frequency_hz),
            "invalid carrier frequency",
        )?;
        require(
            self.physical_db.is_finite()
                && self.observed_db.is_finite()
                && self.physical_db.abs() <= 1e4
                && self.observed_db.abs() <= 1e4,
            "invalid link gain",
        )?;
        require(
            self.sigma_db.is_finite() && (1e-6..=1e4).contains(&self.sigma_db),
            "invalid link uncertainty",
        )
    }
}

fn geometry_distance(a: &LinkObservation, b: &LinkObservation) -> f64 {
    // Reciprocity: swapping TX and RX does not create a held-out physical link.
    let direct = distance(a.tx_m, b.tx_m).hypot(distance(a.rx_m, b.rx_m));
    let reverse = distance(a.tx_m, b.rx_m).hypot(distance(a.rx_m, b.tx_m));
    direct.min(reverse)
}

/// A geometry-conditioned local residual control, not GRaF's neural renderer.
#[derive(Debug, Clone)]
pub struct PropagationResidual {
    training: Vec<LinkObservation>,
    max_neighbor_distance_m: f64,
}

/// Corrected gain and an uncalibrated support-spread proxy around the baseline.
#[derive(Debug, Clone, Copy)]
pub struct GainPrediction {
    /// Physical gain plus the locally learned correction, dB.
    pub gain_db: f64,
    /// Square root of weighted measurement variance plus residual dispersion,
    /// dB. This is not a calibrated confidence interval or posterior.
    pub sigma_db: f64,
    /// Number of supporting training observations.
    pub support: usize,
}

/// Paired physical-only and residual-control errors on held-out links.
#[derive(Debug, Clone, Copy)]
pub struct HeldoutGainMetrics {
    /// Evaluated link count.
    pub samples: usize,
    /// Mean absolute physical-model error, dB.
    pub physical_mae_db: f64,
    /// Mean absolute corrected error, dB.
    pub corrected_mae_db: f64,
}

impl PropagationResidual {
    /// Fit by retaining bounded training-only residual observations.
    /// No target/holdout observation is accepted, even if its gain is finite.
    pub fn fit(training: &[LinkObservation], max_neighbor_distance_m: f64) -> Result<Self> {
        require(
            !training.is_empty() && training.len() <= 4096,
            "training set must contain 1..=4096 observations",
        )?;
        require(
            max_neighbor_distance_m.is_finite() && (1e-3..=1e4).contains(&max_neighbor_distance_m),
            "invalid support radius",
        )?;
        let mut ids = BTreeSet::new();
        for sample in training {
            sample.validate()?;
            require(
                sample.role == DataRole::Train,
                "holdout data cannot fit the residual",
            )?;
            require(
                ids.insert(sample.capture_id.as_str()),
                "duplicate training capture id",
            )?;
        }
        Ok(Self {
            training: training.to_vec(),
            max_neighbor_distance_m,
        })
    }

    /// Predict from same-frequency neighbors. The query's observed gain is never
    /// used; it exists on the shared record only for subsequent evaluation.
    pub fn predict(&self, query: &LinkObservation, neighbors: usize) -> Result<GainPrediction> {
        query.validate()?;
        require(
            (1..=128).contains(&neighbors),
            "neighbor count must be in 1..=128",
        )?;
        let mut support: Vec<_> = self
            .training
            .iter()
            .filter_map(|s| {
                let d = geometry_distance(s, query);
                let same_frequency =
                    (s.frequency_hz - query.frequency_hz).abs() <= query.frequency_hz * 1e-6;
                (same_frequency && d <= self.max_neighbor_distance_m).then_some((d, s))
            })
            .collect();
        require(
            !support.is_empty(),
            "no nearby same-frequency training support; abstain",
        )?;
        support.sort_by(|a, b| a.0.total_cmp(&b.0));
        support.truncate(neighbors);
        let weights: Vec<_> = support
            .iter()
            .map(|(d, s)| 1.0 / ((d * d + 0.01) * s.sigma_db.powi(2)))
            .collect();
        let total: f64 = weights.iter().sum();
        let correction: f64 = support
            .iter()
            .zip(&weights)
            .map(|((_, s), w)| w * (s.observed_db - s.physical_db))
            .sum::<f64>()
            / total;
        let variance = support
            .iter()
            .zip(&weights)
            .map(|((_, s), w)| {
                w * (s.sigma_db.powi(2) + (s.observed_db - s.physical_db - correction).powi(2))
            })
            .sum::<f64>()
            / total;
        let gain_db = query.physical_db + correction;
        require(
            gain_db.is_finite() && variance.is_finite(),
            "residual arithmetic overflow",
        )?;
        Ok(GainPrediction {
            gain_db,
            sigma_db: variance.sqrt(),
            support: support.len(),
        })
    }

    /// Evaluate without changing learned state. Reject repeated capture ids or
    /// physical links (including reciprocal links) across train and holdout.
    pub fn evaluate(
        &self,
        holdout: &[LinkObservation],
        neighbors: usize,
    ) -> Result<HeldoutGainMetrics> {
        require(
            !holdout.is_empty() && holdout.len() <= 4096,
            "holdout must contain 1..=4096 observations",
        )?;
        let mut ids = BTreeSet::new();
        let (mut physical, mut corrected) = (0.0, 0.0);
        for sample in holdout {
            sample.validate()?;
            require(
                sample.role == DataRole::Holdout,
                "evaluation needs holdout records",
            )?;
            require(
                ids.insert(sample.capture_id.as_str()),
                "duplicate holdout capture id",
            )?;
            require(
                !self.training.iter().any(|s| {
                    s.capture_id == sample.capture_id
                        || (geometry_distance(s, sample) <= 1e-6
                            && (s.frequency_hz - sample.frequency_hz).abs()
                                <= sample.frequency_hz * 1e-6)
                }),
                "capture or physical link leaks across train/holdout",
            )?;
            let p = self.predict(sample, neighbors)?;
            physical += (sample.physical_db - sample.observed_db).abs();
            corrected += (p.gain_db - sample.observed_db).abs();
        }
        Ok(HeldoutGainMetrics {
            samples: holdout.len(),
            physical_mae_db: physical / holdout.len() as f64,
            corrected_mae_db: corrected / holdout.len() as f64,
        })
    }
}

/// One externally solved virtual-transmitter initialization candidate.
#[derive(Debug, Clone, Copy)]
pub struct VirtualTxCandidate {
    /// Candidate virtual TX in metres, shared building frame.
    pub position_m: [f64; 3],
    /// Comparable, nonnegative objective cost from the same solver/data.
    pub cost: f64,
}

/// CoTrack-inspired concentration result. Passing is necessary, not sufficient,
/// for identifiability; this module does not solve a trajectory or compute CRLB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InitializationDecision {
    /// Too few competitive solutions to establish concentration.
    InsufficientSupport,
    /// Competitive solutions disagree spatially; abstain.
    Ambiguous,
    /// All competitive candidates lie within the diameter bound.
    Concentrated {
        /// Mean virtual TX estimate in metres.
        position_m: [f64; 3],
        /// Competitive candidate count.
        support: usize,
    },
}

/// Keep every candidate within `cost_slack` of the best cost, then require a
/// common bounded spatial cluster. A distant equally good mode cannot be hidden
/// by selecting only the nearest solutions to the best initialization.
pub fn virtual_tx_concentration(
    candidates: &[VirtualTxCandidate],
    cost_slack: f64,
    max_diameter_m: f64,
    min_support: usize,
) -> Result<InitializationDecision> {
    require(
        !candidates.is_empty() && candidates.len() <= 512,
        "candidate set must contain 1..=512 entries",
    )?;
    require(
        cost_slack.is_finite()
            && cost_slack >= 0.0
            && max_diameter_m.is_finite()
            && (1e-6..=1e4).contains(&max_diameter_m)
            && (2..=512).contains(&min_support),
        "invalid concentration policy",
    )?;
    for c in candidates {
        require(
            finite_point(c.position_m) && c.cost.is_finite() && (0.0..=1e100).contains(&c.cost),
            "invalid initialization candidate",
        )?;
    }
    let best = candidates
        .iter()
        .map(|c| c.cost)
        .fold(f64::INFINITY, f64::min);
    let active: Vec<_> = candidates
        .iter()
        .filter(|c| c.cost - best <= cost_slack)
        .collect();
    if active.len() < min_support {
        return Ok(InitializationDecision::InsufficientSupport);
    }
    for (i, a) in active.iter().enumerate() {
        if active[i + 1..]
            .iter()
            .any(|b| distance(a.position_m, b.position_m) > max_diameter_m)
        {
            return Ok(InitializationDecision::Ambiguous);
        }
    }
    let mut position_m = [0.0; 3];
    for c in &active {
        for (p, x) in position_m.iter_mut().zip(c.position_m) {
            *p += x / active.len() as f64;
        }
    }
    Ok(InitializationDecision::Concentrated {
        position_m,
        support: active.len(),
    })
}

/// A single-point, constant-velocity bistatic signal control for future motion
/// fitting. It omits antenna patterns, normals, multipath and occlusion, and is
/// not the 4DPS renderer or a differentiable scene-fitting implementation.
#[derive(Debug, Clone, Copy)]
pub struct MovingScatterer {
    position_m: [f64; 3],
    velocity_mps: [f64; 3],
    rcs_m2: f64,
}

/// Coherent narrowband return and the physical Doppler derivative.
#[derive(Debug, Clone, Copy)]
pub struct ScattererReturn {
    /// Real component of the dimensionless complex channel amplitude.
    pub real: f64,
    /// Imaginary component of the dimensionless complex channel amplitude.
    pub imag: f64,
    /// TX-scatterer-RX total path length, metres.
    pub path_length_m: f64,
    /// `-d(path_length)/dt / wavelength`, Hz.
    pub doppler_hz: f64,
}

impl MovingScatterer {
    /// Validate the initial position, constant velocity and radar cross-section.
    pub fn new(position_m: [f64; 3], velocity_mps: [f64; 3], rcs_m2: f64) -> Result<Self> {
        require(
            finite_point(position_m)
                && velocity_mps.iter().all(|v| v.is_finite() && v.abs() <= 1e3),
            "invalid scatterer kinematics",
        )?;
        require(
            rcs_m2.is_finite() && (0.0..=1e4).contains(&rcs_m2),
            "invalid radar cross-section",
        )?;
        Ok(Self {
            position_m,
            velocity_mps,
            rcs_m2,
        })
    }

    /// Render from physical path length. Native complex samples can be compared
    /// against this control without inventing radar range/Doppler resolution.
    pub fn render(
        &self,
        tx_m: [f64; 3],
        rx_m: [f64; 3],
        time_s: f64,
        frequency_hz: f64,
    ) -> Result<ScattererReturn> {
        require(
            finite_point(tx_m)
                && finite_point(rx_m)
                && time_s.is_finite()
                && time_s.abs() <= 1e6
                && frequency_hz.is_finite()
                && (1e6..=1e12).contains(&frequency_hz),
            "invalid rendering geometry/time/frequency",
        )?;
        let p = std::array::from_fn(|i| self.position_m[i] + self.velocity_mps[i] * time_s);
        require(
            finite_point(p),
            "scatterer moved outside supported geometry",
        )?;
        let a = distance(p, tx_m);
        let b = distance(p, rx_m);
        require(a >= 1e-3 && b >= 1e-3, "scatterer intersects a sensor")?;
        let lambda = 299_792_458.0 / frequency_hz;
        let amplitude =
            lambda * self.rcs_m2.sqrt() / ((4.0 * std::f64::consts::PI).powf(1.5) * a * b);
        let phase = -std::f64::consts::TAU * (a + b) / lambda;
        let path_rate = (0..3)
            .map(|i| self.velocity_mps[i] * ((p[i] - tx_m[i]) / a + (p[i] - rx_m[i]) / b))
            .sum::<f64>();
        Ok(ScattererReturn {
            real: amplitude * phase.cos(),
            imag: amplitude * phase.sin(),
            path_length_m: a + b,
            doppler_hz: -path_rate / lambda,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(id: &str, role: DataRole, y: f64, correction: f64) -> LinkObservation {
        LinkObservation {
            capture_id: id.into(),
            role,
            tx_m: [0.0, y, 0.0],
            rx_m: [3.0, y, 0.0],
            frequency_hz: 2.4e9,
            physical_db: -60.0,
            observed_db: -60.0 + correction,
            sigma_db: 0.1,
        }
    }

    #[test]
    fn completion_and_synthetic_never_enter_capture_lane() {
        for (origin, expected) in [
            (GeometryOrigin::Capture, GeometryDisposition::CaptureDerived),
            (
                GeometryOrigin::Synthetic,
                GeometryDisposition::SyntheticOnly,
            ),
            (
                GeometryOrigin::Completion,
                GeometryDisposition::CompletionOnly,
            ),
        ] {
            let g = GeometryCandidate::new(origin, [0.0; 3], 0.001, 10, Some(1)).unwrap();
            assert_eq!(g.disposition(11, 5, 1.0).unwrap(), expected);
            assert_eq!(g.origin(), origin);
            assert_eq!(
                g.disposition(9, 5, 1.0).unwrap(),
                GeometryDisposition::Abstain
            );
            assert_eq!(
                g.disposition(20, 5, 1.0).unwrap(),
                GeometryDisposition::Abstain
            );
        }
        assert!(GeometryCandidate::new(GeometryOrigin::Capture, [0.0; 3], 0.1, 0, None).is_err());
        assert!(
            GeometryCandidate::new(GeometryOrigin::Capture, [0.0; 3], f64::NAN, 0, Some(1))
                .is_err()
        );
    }

    #[test]
    fn residual_fits_training_only_and_is_unchanged_by_evaluation() {
        let train = vec![
            link("train-a", DataRole::Train, 0.0, 3.0),
            link("train-b", DataRole::Train, 2.0, 3.0),
        ];
        let query = link("test", DataRole::Holdout, 1.0, 3.0);
        assert!(PropagationResidual::fit(&[query.clone()], 5.0).is_err());
        let model = PropagationResidual::fit(&train, 5.0).unwrap();
        let before = model.predict(&query, 2).unwrap();
        let scores = model.evaluate(&[query.clone()], 2).unwrap();
        assert_eq!(scores.samples, 1);
        assert!((scores.physical_mae_db - 3.0).abs() < 1e-10);
        assert!(scores.corrected_mae_db < 1e-10);
        let mut changed_target = query.clone();
        changed_target.observed_db += 20.0;
        assert_eq!(
            model.predict(&changed_target, 2).unwrap().gain_db,
            before.gain_db
        );
        assert_eq!(model.predict(&query, 2).unwrap().gain_db, before.gain_db);
        let changed_train = vec![link("different", DataRole::Train, 0.0, -2.0)];
        let changed_model = PropagationResidual::fit(&changed_train, 5.0).unwrap();
        assert!((changed_model.predict(&query, 1).unwrap().gain_db - before.gain_db).abs() > 4.9);
    }

    #[test]
    fn capture_and_reciprocal_link_leakage_are_rejected() {
        let train = link("same-capture", DataRole::Train, 0.0, 1.0);
        let model = PropagationResidual::fit(&[train.clone()], 5.0).unwrap();
        let same_capture = link("same-capture", DataRole::Holdout, 0.5, 1.0);
        assert!(model.evaluate(&[same_capture], 1).is_err());
        let mut reciprocal = link("new-id", DataRole::Holdout, 0.0, 1.0);
        std::mem::swap(&mut reciprocal.tx_m, &mut reciprocal.rx_m);
        assert!(model.evaluate(&[reciprocal], 1).is_err());
        assert!(PropagationResidual::fit(&[train.clone(), train], 5.0).is_err());
    }

    #[test]
    fn unsupported_geometry_frequency_and_invalid_values_abstain() {
        let model =
            PropagationResidual::fit(&[link("train", DataRole::Train, 0.0, 1.0)], 1.0).unwrap();
        let mut query = link("query", DataRole::Holdout, 2.0, 1.0);
        assert!(model.predict(&query, 1).is_err());
        query.tx_m[1] = 0.0;
        query.rx_m[1] = 0.0;
        query.frequency_hz = 5e9;
        assert!(model.predict(&query, 1).is_err());
        query.frequency_hz = 2.4e9;
        query.sigma_db = f64::INFINITY;
        assert!(model.predict(&query, 1).is_err());
        assert!(model
            .predict(&link("q", DataRole::Holdout, 0.0, 1.0), 0)
            .is_err());
    }

    #[test]
    fn virtual_tx_competing_modes_force_abstention() {
        let near = VirtualTxCandidate {
            position_m: [1.0, 2.0, 0.0],
            cost: 1.0,
        };
        let same = VirtualTxCandidate {
            position_m: [1.1, 2.0, 0.0],
            cost: 1.01,
        };
        let far = VirtualTxCandidate {
            position_m: [10.0, 2.0, 0.0],
            cost: 1.02,
        };
        assert!(matches!(
            virtual_tx_concentration(&[near, same], 0.1, 0.5, 2).unwrap(),
            InitializationDecision::Concentrated { support: 2, .. }
        ));
        assert_eq!(
            virtual_tx_concentration(&[near, same, far], 0.1, 0.5, 2).unwrap(),
            InitializationDecision::Ambiguous
        );
        assert_eq!(
            virtual_tx_concentration(&[near], 0.1, 0.5, 2).unwrap(),
            InitializationDecision::InsufficientSupport
        );
        let bad = VirtualTxCandidate {
            cost: f64::NAN,
            ..near
        };
        assert!(virtual_tx_concentration(&[near, bad], 0.1, 0.5, 2).is_err());
    }

    #[test]
    fn coherent_moving_return_has_correct_phase_doppler_and_distance_scaling() {
        let fc = 299_792_458.0; // wavelength 1 m keeps the analytic anchors simple.
        let tx = [0.0; 3];
        let body = MovingScatterer::new([2.0, 0.0, 0.0], [0.25, 0.0, 0.0], 1.0).unwrap();
        let start = body.render(tx, tx, 0.0, fc).unwrap();
        let quarter = body.render(tx, tx, 0.5, fc).unwrap();
        assert!((start.path_length_m - 4.0).abs() < 1e-12);
        assert!((start.doppler_hz + 0.5).abs() < 1e-12);
        assert!(start.imag.abs() < 1e-12 && start.real > 0.0);
        assert!(quarter.real.abs() < 1e-12 && quarter.imag < 0.0);
        let far = MovingScatterer::new([4.0, 0.0, 0.0], [0.0; 3], 1.0)
            .unwrap()
            .render(tx, tx, 0.0, fc)
            .unwrap();
        assert!((far.real / start.real - 0.25).abs() < 1e-12);
        assert_eq!(far.doppler_hz, 0.0);
        assert!(body.render([2.0, 0.0, 0.0], tx, 0.0, fc).is_err());
        assert!(body.render(tx, tx, f64::NAN, fc).is_err());
    }
}
