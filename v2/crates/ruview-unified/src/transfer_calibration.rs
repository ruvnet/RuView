//! OpenCSI-inspired amplitude calibration for controlled transfer experiments.
//!
//! This bounded prototype learns a link's temporal quiet-period moments and
//! emits an unsigned amplitude Z-score while retaining the original amplitudes.
//! It is not an OpenCSI reproduction: quiet labels are supplied by a trusted
//! caller, and phase blending, rate/time buckets and automatic quiet detection
//! are deliberately outside this module. No baseline is a capability certificate.

use crate::frame::{EvidenceLevel, FrameProvenance, ProvenanceClass, SignalQuality};
use crate::{Result, UnifiedError};

/// Maximum bins accepted from a single link observation.
pub const MAX_AMPLITUDE_BINS: usize = 65_536;

/// Quality and maturity policy, to be selected before an evaluation split.
#[derive(Debug, Clone, Copy)]
pub struct TransferCalibrationConfig {
    /// Number of accepted quiet observations required to trust the baseline.
    pub min_quiet_samples: u64,
    /// Minimum elapsed quiet-baseline span, in nanoseconds.
    pub min_quiet_span_ns: u64,
    /// Maximum age since the latest accepted quiet update.
    pub max_baseline_age_ns: u64,
    /// Lower bound on temporal sample variance; zero variance cannot normalize.
    pub min_variance: f64,
    /// Upper bound on temporal sample variance; excessive noise must abstain.
    pub max_variance: f64,
    /// Minimum instantaneous RSSI minus noise floor, in dB.
    pub min_snr_db: f64,
    /// Maximum packet-loss fraction accepted for learning and scoring.
    pub max_packet_loss: f64,
    /// Maximum declared interference accepted for learning and scoring.
    pub max_interference: f64,
    /// Minimum fraction of valid amplitude bins.
    pub min_valid_fraction: f64,
}

impl Default for TransferCalibrationConfig {
    fn default() -> Self {
        Self {
            min_quiet_samples: 32,
            min_quiet_span_ns: 120_000_000_000,
            max_baseline_age_ns: 600_000_000_000,
            min_variance: 1e-8,
            max_variance: 1e8,
            min_snr_db: 5.0,
            max_packet_loss: 0.2,
            max_interference: 0.5,
            min_valid_fraction: 0.8,
        }
    }
}

impl TransferCalibrationConfig {
    fn validate(self) -> Result<()> {
        if self.min_quiet_samples < 2
            || self.min_quiet_span_ns == 0
            || self.max_baseline_age_ns == 0
            || !self.min_variance.is_finite()
            || !self.max_variance.is_finite()
            || self.min_variance <= 0.0
            || self.max_variance < self.min_variance
            || !self.min_snr_db.is_finite()
            || !(0.0..=1.0).contains(&self.max_packet_loss)
            || !(0.0..=1.0).contains(&self.max_interference)
            || !(0.0..=1.0).contains(&self.min_valid_fraction)
            || self.min_valid_fraction == 0.0
        {
            return invalid("invalid transfer-calibration policy");
        }
        Ok(())
    }
}

/// Trusted upstream gating state. Unknown and occupied frames never train.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuietState {
    /// An independent gate established that the link is quiet.
    Quiet,
    /// Occupancy/motion was detected by the independent gate.
    Occupied,
    /// Quietness cannot be established.
    Unknown,
}

/// A native-amplitude observation from one hardware-bound link.
pub struct AmplitudeObservation<'a> {
    /// Original native amplitudes, without gain correction or replacement.
    pub amplitudes: &'a [f64],
    /// Explicit native validity mask, of exactly the same length.
    pub valid_mask: &'a [bool],
    /// Monotonically increasing observation timestamp, nanoseconds.
    pub timestamp_ns: u64,
    /// Existing frame provenance; never upgraded by this module.
    pub provenance: &'a FrameProvenance,
    /// Existing native-frame quality indicators.
    pub quality: SignalQuality,
    /// Independent quiet classification, never inferred from this Z-score.
    pub quiet: QuietState,
}

/// Baseline state derived from actual accepted quiet observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineMaturity {
    /// No accepted quiet observations.
    Missing,
    /// Count or elapsed-span policy has not yet been satisfied.
    Warming,
    /// Count and span policy satisfied, and update is fresh.
    Ready,
    /// Latest quiet update has expired, including an incomplete baseline.
    Stale,
}

/// Why a calibrated scalar is unavailable; raw amplitude remains available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationAbstention {
    /// No quiet baseline has been observed.
    MissingBaseline,
    /// Baseline has not accumulated sufficient duration and sample count.
    WarmingBaseline,
    /// Baseline is older than the configured maximum age.
    StaleBaseline,
    /// Too few native bins are valid.
    InsufficientValidBins,
    /// SNR, packet loss or interference failed the quality policy.
    LowSignalQuality,
    /// Quiet temporal variance is too small to support a stable division.
    LowVariance,
    /// Quiet temporal variance exceeds the configured noise envelope.
    HighVariance,
}

/// A usable calibrated feature or an explicit abstention.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CalibrationDecision {
    /// Absolute deviation of the valid-bin mean from the quiet mean, in sigmas.
    Ready {
        /// Unsigned Z-score; not an occupancy prediction or probability.
        z_score: f64,
    },
    /// No calibrated inference may be made from this observation.
    Abstain(CalibrationAbstention),
}

/// Bounded quiet-baseline moments and freshness; contains no raw history.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaselineSummary {
    /// Number of accepted quiet observations.
    pub quiet_samples: u64,
    /// Mean valid-bin amplitude across accepted observations.
    pub mean: Option<f64>,
    /// Unbiased temporal sample variance, available after two observations.
    pub variance: Option<f64>,
    /// Elapsed time since the latest accepted quiet observation.
    pub age_ns: Option<u64>,
    /// Baseline count/span/freshness status.
    pub maturity: BaselineMaturity,
}

/// Derived view retaining native amplitude, its mask and source provenance.
#[derive(Debug, Clone)]
pub struct TransferFeatures {
    /// Link name supplied when the calibrator was constructed.
    pub link_id: String,
    /// Original amplitudes, including masked bins, copied without replacement.
    pub amplitudes: Vec<f64>,
    /// Original mask, copied without replacement.
    pub valid_mask: Vec<bool>,
    /// Native valid-bin mean, retaining absolute amplitude for downstream tasks.
    pub mean_amplitude: Option<f64>,
    /// Baseline moments and maturity at this observation.
    pub baseline: BaselineSummary,
    /// Available calibrated feature or precise abstention reason.
    pub decision: CalibrationDecision,
    /// Source provenance, capped at the minimum evidence of baseline and input.
    pub provenance: FrameProvenance,
}

/// A single link's quiet temporal baseline; one instance per hardware/rate band.
///
/// State is deliberately not deserializable. A different device, firmware,
/// provenance class, native bin count, radio channel or sampling-rate bucket
/// must use a new instance (the caller owns channel/rate identity).
#[derive(Debug, Clone)]
pub struct LinkTransferCalibrator {
    link_id: String,
    binding: FrameProvenance,
    config: TransferCalibrationConfig,
    bins: Option<usize>,
    count: u64,
    mean: f64,
    m2: f64,
    first_quiet_ns: Option<u64>,
    last_quiet_ns: Option<u64>,
    last_seen_ns: Option<u64>,
    evidence: EvidenceLevel,
}

impl LinkTransferCalibrator {
    /// Construct a fresh, untrusted baseline with an explicit provenance binding.
    pub fn new(
        link_id: &str,
        binding: FrameProvenance,
        config: TransferCalibrationConfig,
    ) -> Result<Self> {
        config.validate()?;
        validate_provenance(&binding)?;
        if link_id.is_empty() || link_id.len() > 128 || link_id.chars().any(char::is_control) {
            return invalid("link id must contain 1..=128 non-control bytes");
        }
        let evidence = binding.evidence;
        Ok(Self {
            link_id: link_id.into(),
            binding,
            config,
            bins: None,
            count: 0,
            mean: 0.0,
            m2: 0.0,
            first_quiet_ns: None,
            last_quiet_ns: None,
            last_seen_ns: None,
            evidence,
        })
    }

    /// Validate a frame, optionally learn a quiet baseline, and derive features.
    /// Invalid input leaves all state unchanged. Quality failures never train.
    pub fn observe(&mut self, input: AmplitudeObservation<'_>) -> Result<TransferFeatures> {
        let mut next = self.clone();
        let features = next.observe_inner(input)?;
        *self = next;
        Ok(features)
    }

    fn observe_inner(&mut self, input: AmplitudeObservation<'_>) -> Result<TransferFeatures> {
        validate_provenance(input.provenance)?;
        if input.provenance.device_id != self.binding.device_id
            || input.provenance.firmware != self.binding.firmware
            || input.provenance.class != self.binding.class
        {
            return invalid("calibration hardware or provenance binding changed");
        }
        let n = input.amplitudes.len();
        if n == 0
            || n > MAX_AMPLITUDE_BINS
            || n != input.valid_mask.len()
            || self.bins.is_some_and(|bins| bins != n)
        {
            return invalid("invalid or changed native amplitude/mask shape");
        }
        if input.amplitudes.iter().any(|v| !v.is_finite() || *v < 0.0)
            || !input.quality.rssi_dbm.is_finite()
            || !input.quality.noise_floor_dbm.is_finite()
            || !(0.0..=1.0).contains(&input.quality.packet_loss)
            || !(0.0..=1.0).contains(&input.quality.interference)
            || self.last_seen_ns.is_some_and(|t| input.timestamp_ns <= t)
        {
            return invalid("non-finite amplitude/quality or non-increasing timestamp");
        }
        let valid_count = input.valid_mask.iter().filter(|v| **v).count();
        let mean = if valid_count == 0 {
            None
        } else {
            // Divide before summing so finite amplitudes cannot overflow the sum.
            Some(
                input
                    .amplitudes
                    .iter()
                    .zip(input.valid_mask)
                    .filter(|(_, valid)| **valid)
                    .map(|(v, _)| v / valid_count as f64)
                    .sum::<f64>(),
            )
        };
        if mean.is_some_and(|v| !v.is_finite())
            || !(input.quality.rssi_dbm - input.quality.noise_floor_dbm).is_finite()
        {
            return invalid("amplitude mean or SNR exceeds numeric range");
        }
        let enough_bins = valid_count as f64 / n as f64 >= self.config.min_valid_fraction;
        let good_quality = input.quality.rssi_dbm - input.quality.noise_floor_dbm
            >= self.config.min_snr_db
            && input.quality.packet_loss <= self.config.max_packet_loss
            && input.quality.interference <= self.config.max_interference;
        if input.quiet == QuietState::Quiet && enough_bins && good_quality {
            let value = mean.expect("positive valid-bin policy implies mean");
            // An expired baseline must earn maturity again; one quiet frame
            // cannot refresh old statistics and immediately restore authority.
            if self
                .last_quiet_ns
                .is_some_and(|t| input.timestamp_ns - t > self.config.max_baseline_age_ns)
            {
                self.count = 0;
                self.mean = 0.0;
                self.m2 = 0.0;
                self.first_quiet_ns = None;
            }
            let count = self
                .count
                .checked_add(1)
                .ok_or_else(|| UnifiedError::InvalidInput("quiet count overflow".into()))?;
            let delta = value - self.mean;
            let next_mean = self.mean + delta / count as f64;
            let next_m2 = self.m2 + delta * (value - next_mean);
            if !next_mean.is_finite() || !next_m2.is_finite() || next_m2 < 0.0 {
                return invalid("quiet moments exceed numeric range");
            }
            self.count = count;
            self.mean = next_mean;
            self.m2 = next_m2;
            self.first_quiet_ns.get_or_insert(input.timestamp_ns);
            self.last_quiet_ns = Some(input.timestamp_ns);
            self.evidence = self.evidence.min(input.provenance.evidence);
        }
        self.bins = Some(n);
        self.last_seen_ns = Some(input.timestamp_ns);
        let baseline = self.summary(input.timestamp_ns);
        let reason = if !enough_bins {
            Some(CalibrationAbstention::InsufficientValidBins)
        } else if !good_quality {
            Some(CalibrationAbstention::LowSignalQuality)
        } else {
            match baseline.maturity {
                BaselineMaturity::Missing => Some(CalibrationAbstention::MissingBaseline),
                BaselineMaturity::Warming => Some(CalibrationAbstention::WarmingBaseline),
                BaselineMaturity::Stale => Some(CalibrationAbstention::StaleBaseline),
                BaselineMaturity::Ready => {
                    let variance = baseline.variance.expect("ready requires two samples");
                    if variance < self.config.min_variance {
                        Some(CalibrationAbstention::LowVariance)
                    } else if variance > self.config.max_variance {
                        Some(CalibrationAbstention::HighVariance)
                    } else {
                        None
                    }
                }
            }
        };
        let decision = match reason {
            Some(reason) => CalibrationDecision::Abstain(reason),
            None => {
                let score = (mean.unwrap() - self.mean).abs() / baseline.variance.unwrap().sqrt();
                if !score.is_finite() {
                    return invalid("calibrated Z-score exceeds numeric range");
                }
                CalibrationDecision::Ready { z_score: score }
            }
        };
        let mut provenance = input.provenance.clone();
        provenance.evidence = provenance.evidence.min(self.evidence);
        Ok(TransferFeatures {
            link_id: self.link_id.clone(),
            amplitudes: input.amplitudes.to_vec(),
            valid_mask: input.valid_mask.to_vec(),
            mean_amplitude: mean,
            baseline,
            decision,
            provenance,
        })
    }

    fn summary(&self, now_ns: u64) -> BaselineSummary {
        let age = self.last_quiet_ns.map(|t| now_ns - t);
        let maturity = if self.count == 0 {
            BaselineMaturity::Missing
        } else if age.unwrap() > self.config.max_baseline_age_ns {
            BaselineMaturity::Stale
        } else if self.count < self.config.min_quiet_samples
            || self.last_quiet_ns.unwrap() - self.first_quiet_ns.unwrap()
                < self.config.min_quiet_span_ns
        {
            BaselineMaturity::Warming
        } else {
            BaselineMaturity::Ready
        };
        BaselineSummary {
            quiet_samples: self.count,
            mean: (self.count > 0).then_some(self.mean),
            variance: (self.count > 1).then(|| self.m2 / (self.count - 1) as f64),
            age_ns: age,
            maturity,
        }
    }
}

fn invalid<T>(message: &str) -> Result<T> {
    Err(UnifiedError::InvalidInput(message.into()))
}

fn validate_provenance(p: &FrameProvenance) -> Result<()> {
    match (p.class, p.evidence) {
        (ProvenanceClass::Synthetic, EvidenceLevel::L0Simulation) => {}
        (ProvenanceClass::Measured, level) if level >= EvidenceLevel::L1CapturedReplay => {}
        _ => return invalid("inconsistent synthetic/measured evidence"),
    }
    if p.device_id.is_empty()
        || p.device_id.len() > 256
        || p.firmware.is_empty()
        || p.firmware.len() > 128
    {
        return invalid("missing or oversized device/firmware identity");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance() -> FrameProvenance {
        FrameProvenance {
            class: ProvenanceClass::Synthetic,
            evidence: EvidenceLevel::L0Simulation,
            device_id: "synthetic-chip".into(),
            firmware: "fixture-1".into(),
            receipt_id: 1,
        }
    }
    fn policy() -> TransferCalibrationConfig {
        TransferCalibrationConfig {
            min_quiet_samples: 4,
            min_quiet_span_ns: 3,
            max_baseline_age_ns: 10,
            min_variance: 1e-6,
            max_variance: 100.0,
            ..Default::default()
        }
    }
    fn quality() -> SignalQuality {
        SignalQuality {
            rssi_dbm: -40.0,
            noise_floor_dbm: -90.0,
            packet_loss: 0.0,
            interference: 0.0,
        }
    }
    fn observe(
        c: &mut LinkTransferCalibrator,
        t: u64,
        value: f64,
        quiet: QuietState,
    ) -> TransferFeatures {
        c.observe(AmplitudeObservation {
            amplitudes: &[value, value],
            valid_mask: &[true, true],
            timestamp_ns: t,
            provenance: &provenance(),
            quality: quality(),
            quiet,
        })
        .unwrap()
    }
    fn warmed(gain: f64, offset: f64) -> LinkTransferCalibrator {
        let mut c = LinkTransferCalibrator::new("tx-rx", provenance(), policy()).unwrap();
        for (i, value) in [9.0, 11.0, 9.0, 11.0].iter().enumerate() {
            observe(&mut c, i as u64, value * gain + offset, QuietState::Quiet);
        }
        c
    }

    #[test]
    fn synthetic_affine_transfer_preserves_raw_motion_magnitude() {
        let mut a = warmed(1.0, 0.0);
        let mut b = warmed(2.0, 5.0);
        let first = observe(&mut a, 4, 13.0, QuietState::Occupied);
        let other = observe(&mut b, 4, 31.0, QuietState::Occupied);
        let (
            CalibrationDecision::Ready { z_score: za },
            CalibrationDecision::Ready { z_score: zb },
        ) = (first.decision, other.decision)
        else {
            panic!("warm baseline must score")
        };
        assert!((za - zb).abs() < 1e-12);
        assert_eq!(first.amplitudes, vec![13.0, 13.0]);
        assert_eq!(other.amplitudes, vec![31.0, 31.0]);
        let second = observe(&mut a, 5, 16.0, QuietState::Occupied);
        assert_ne!(first.mean_amplitude, second.mean_amplitude);
        assert_eq!(first.provenance.class, ProvenanceClass::Synthetic);
        assert_eq!(first.provenance.evidence, EvidenceLevel::L0Simulation);
    }

    #[test]
    fn occupied_unknown_and_poor_quality_observations_do_not_train() {
        let mut c = warmed(1.0, 0.0);
        let original = c.summary(3);
        for quiet in [QuietState::Occupied, QuietState::Unknown] {
            let t = c.last_seen_ns.unwrap() + 1;
            observe(&mut c, t, 1e6, quiet);
        }
        let poor = c
            .observe(AmplitudeObservation {
                amplitudes: &[1e6, 1e6],
                valid_mask: &[true, true],
                timestamp_ns: 6,
                provenance: &provenance(),
                quality: SignalQuality {
                    interference: 0.9,
                    ..quality()
                },
                quiet: QuietState::Quiet,
            })
            .unwrap();
        assert_eq!(
            poor.decision,
            CalibrationDecision::Abstain(CalibrationAbstention::LowSignalQuality)
        );
        assert_eq!(poor.baseline.quiet_samples, original.quiet_samples);
        assert_eq!(poor.baseline.mean, original.mean);
        assert_eq!(poor.baseline.variance, original.variance);
    }

    #[test]
    fn missing_warming_stale_and_variance_failures_abstain() {
        let mut c = LinkTransferCalibrator::new("tx-rx", provenance(), policy()).unwrap();
        assert_eq!(
            observe(&mut c, 0, 10.0, QuietState::Unknown).decision,
            CalibrationDecision::Abstain(CalibrationAbstention::MissingBaseline)
        );
        assert_eq!(
            observe(&mut c, 1, 10.0, QuietState::Quiet)
                .baseline
                .maturity,
            BaselineMaturity::Warming
        );
        for t in 2..=4 {
            observe(&mut c, t, 10.0, QuietState::Quiet);
        }
        assert_eq!(
            observe(&mut c, 5, 12.0, QuietState::Occupied).decision,
            CalibrationDecision::Abstain(CalibrationAbstention::LowVariance)
        );
        assert_eq!(
            observe(&mut c, 15, 12.0, QuietState::Occupied).decision,
            CalibrationDecision::Abstain(CalibrationAbstention::StaleBaseline)
        );
        let reanchor = observe(&mut c, 16, 20.0, QuietState::Quiet);
        assert_eq!(reanchor.baseline.quiet_samples, 1);
        assert_eq!(reanchor.baseline.maturity, BaselineMaturity::Warming);
        let mut c = LinkTransferCalibrator::new("tx-rx", provenance(), policy()).unwrap();
        for t in 0..4 {
            observe(
                &mut c,
                t,
                if t % 2 == 0 { 0.0 } else { 100.0 },
                QuietState::Quiet,
            );
        }
        assert_eq!(
            observe(&mut c, 4, 70.0, QuietState::Occupied).decision,
            CalibrationDecision::Abstain(CalibrationAbstention::HighVariance)
        );
    }

    #[test]
    fn malformed_inputs_are_transactional_and_cannot_promote_evidence() {
        let mut c = warmed(1.0, 0.0);
        let before = c.summary(3);
        let mut p = provenance();
        p.evidence = EvidenceLevel::L2Lab;
        assert!(c
            .observe(AmplitudeObservation {
                amplitudes: &[10.0, 10.0],
                valid_mask: &[true, true],
                timestamp_ns: 4,
                provenance: &p,
                quality: quality(),
                quiet: QuietState::Quiet
            })
            .is_err());
        assert_eq!(c.summary(3), before);
        assert_eq!(c.last_seen_ns, Some(3));
        for amplitudes in [vec![f64::NAN, 1.0], vec![-1.0, 1.0], vec![1.0]] {
            assert!(c
                .observe(AmplitudeObservation {
                    amplitudes: &amplitudes,
                    valid_mask: &[true, true],
                    timestamp_ns: 4,
                    provenance: &provenance(),
                    quality: quality(),
                    quiet: QuietState::Quiet
                })
                .is_err());
        }
        assert_eq!(c.summary(3), before);
        let empty_mask = c
            .observe(AmplitudeObservation {
                amplitudes: &[10.0, 20.0],
                valid_mask: &[false, false],
                timestamp_ns: 4,
                provenance: &provenance(),
                quality: quality(),
                quiet: QuietState::Quiet,
            })
            .unwrap();
        assert_eq!(
            empty_mask.decision,
            CalibrationDecision::Abstain(CalibrationAbstention::InsufficientValidBins)
        );
        assert_eq!(empty_mask.mean_amplitude, None);
        assert_eq!(empty_mask.amplitudes, vec![10.0, 20.0]);
    }
    #[test]
    fn measured_feature_is_capped_by_quiet_evidence_and_invalid_numeric_input_is_atomic() {
        let mut bound = provenance();
        bound.class = ProvenanceClass::Measured;
        bound.evidence = EvidenceLevel::L3HeldOutValidation;
        let mut quiet = bound.clone();
        quiet.evidence = EvidenceLevel::L1CapturedReplay;
        let mut c = LinkTransferCalibrator::new("tx-rx", bound.clone(), policy()).unwrap();
        for (i, value) in [9.0, 11.0, 9.0, 11.0].iter().enumerate() {
            c.observe(AmplitudeObservation {
                amplitudes: &[*value, *value],
                valid_mask: &[true, true],
                timestamp_ns: i as u64,
                provenance: &quiet,
                quality: quality(),
                quiet: QuietState::Quiet,
            })
            .unwrap();
        }
        let features = c
            .observe(AmplitudeObservation {
                amplitudes: &[13.0, 13.0],
                valid_mask: &[true, true],
                timestamp_ns: 4,
                provenance: &bound,
                quality: quality(),
                quiet: QuietState::Occupied,
            })
            .unwrap();
        assert_eq!(
            features.provenance.evidence,
            EvidenceLevel::L1CapturedReplay
        );
        assert_eq!(features.link_id, "tx-rx");
        assert_eq!(features.valid_mask, vec![true, true]);
        let before = c.summary(4);
        assert!(c
            .observe(AmplitudeObservation {
                amplitudes: &[13.0, 13.0],
                valid_mask: &[true, true],
                timestamp_ns: 3,
                provenance: &bound,
                quality: quality(),
                quiet: QuietState::Quiet
            })
            .is_err());
        assert_eq!(c.summary(4), before);
        assert!(c
            .observe(AmplitudeObservation {
                amplitudes: &[13.0, 13.0],
                valid_mask: &[true, true],
                timestamp_ns: 5,
                provenance: &bound,
                quality: SignalQuality {
                    rssi_dbm: f64::MAX,
                    noise_floor_dbm: -f64::MAX,
                    ..quality()
                },
                quiet: QuietState::Quiet
            })
            .is_err());
        assert_eq!(c.last_seen_ns, Some(4));
        assert_eq!(c.summary(4), before);
    }
}
