//! Model release sanity gates (ADR-298) — block degenerate and mislabeled
//! classifier artifacts before they can ship.
//!
//! # Why this module exists
//!
//! The external review (corroborating issue 1521) showed the published presence
//! head is mathematically degenerate: with L2-normalized embeddings, a weight
//! norm `‖w‖ ≈ 3.67` against a bias `≈ 8.19` makes the *smallest possible*
//! logit positive (`8.19 − 3.67 = 4.52 > 0`), so predicted presence probability
//! is `≥ ~0.989` for **every** valid input — the decision boundary is
//! unreachable and the head is effectively constant. The README then labeled a
//! temporal-triplet accuracy (a representation-ordering metric) as "presence
//! accuracy" — a category error. Nothing in the release path caught any of it.
//!
//! This module adds structural machine checks for those failure shapes:
//!
//! - [`check_unreachable_boundary`] — for a normalized-embedding linear head,
//!   fail when `‖bias‖` dominates `‖weight‖` so the logit cannot change sign.
//! - [`check_constant_output`] — fail when the head's output variance across a
//!   diverse, L2-normalized probe set is below a threshold.
//! - [`check_class_balance`] — fail when the predicted-positive rate on a
//!   balanced probe set sits at/above a ceiling (e.g. `> 99 %`).
//! - [`check_baseline`] — fail a report with no paired mean-pose/majority
//!   baseline (ties into the ADR-291 [`EvaluationReport`]).
//! - [`check_metric_provenance`] — a metric carries its computed
//!   [`MetricKind`] and its display label derives from it, so a temporal-triplet
//!   metric can never be surfaced under the `presence` task name.
//!
//! Each gate emits a structured, human-readable [`GateFailure`] naming the
//! defect and the offending numbers.
//!
//! # This is prevention, not a correctness proof
//!
//! The gates are heuristic. They catch the *known* failure shapes above, not
//! all bad models (ADR-298 §Consequences). This module does **not** withdraw
//! any already-published artifact — an outward-facing action requiring
//! maintainer sign-off — it prevents recurrence.
//!
//! All checks are deterministic: probe sets are generated in code from a linear
//! head with no RNG state, and the sweep exercises the full reachable logit
//! range so the constant-output verdict is dimension-robust (random unit
//! vectors concentrate near-orthogonal to the weight in high dimensions and
//! would hide a live boundary).

use thiserror::Error;

use crate::protocols::leakage::EvaluationReport;

// ---------------------------------------------------------------------------
// Tunable thresholds (documented defaults; every gate also takes an explicit
// argument so a workflow can tighten them).
// ---------------------------------------------------------------------------

/// Default population-variance floor for [`check_constant_output`]. Below this,
/// the head's probability output is treated as effectively constant. The
/// issue-1521 head sits far under it (all logits in `[4.52, 11.86]` ⇒ sigmoid in
/// `[0.989, 1.0)`); a head that sweeps `[-‖w‖, ‖w‖]` around a reachable boundary
/// sits far above it.
pub const DEFAULT_MIN_OUTPUT_VARIANCE: f64 = 1e-4;

/// Default predicted-positive-rate ceiling for [`check_class_balance`]. A head
/// that predicts positive for `> 99 %` of a balanced probe is degenerate.
pub const DEFAULT_MAX_POSITIVE_RATE: f64 = 0.99;

/// Default number of probe points swept across the reachable logit range.
pub const DEFAULT_PROBE_POINTS: usize = 64;

// ---------------------------------------------------------------------------
// GateError — malformed input at the construction boundary (never a panic).
// ---------------------------------------------------------------------------

/// Errors from constructing a gate input from untrusted numbers.
///
/// These are *malformed artifact* errors (empty weight vector, non-finite
/// parameters), distinct from a [`GateFailure`] which is a well-formed head
/// that a gate rejected.
#[derive(Debug, Error, PartialEq)]
pub enum GateError {
    /// The weight vector was empty; a linear head needs at least one dimension.
    #[error("linear head weight vector is empty")]
    EmptyWeight,

    /// A weight or bias value was NaN or ±inf; corrupted parameters must be
    /// rejected upstream, never scored.
    #[error("non-finite parameter `{what}`: {value}")]
    NonFiniteParameter {
        /// Which parameter (`"weight[i]"` or `"bias"`).
        what: String,
        /// The offending value.
        value: f64,
    },

    /// A probe set was empty.
    #[error("probe set is empty")]
    EmptyProbe,

    /// A probe embedding length did not match the head dimension.
    #[error("probe embedding has dimension {actual}, expected {expected}")]
    ProbeDimMismatch {
        /// Head dimension.
        expected: usize,
        /// Probe embedding length.
        actual: usize,
    },

    /// A metric value was NaN or ±inf.
    #[error("metric value is not finite: {0}")]
    NonFiniteMetric(f64),
}

// ---------------------------------------------------------------------------
// GateFailure — a well-formed head/report that a gate rejected.
// ---------------------------------------------------------------------------

/// A structured, human-readable gate failure carrying the offending numbers.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum GateFailure {
    /// The decision boundary (`logit = 0`) is analytically unreachable: with
    /// L2-normalized embeddings the logit is confined to
    /// `[bias − ‖w‖, bias + ‖w‖]`, and this interval does not straddle zero.
    #[error(
        "unreachable decision boundary: L2-normalized logit is confined to \
         [{min_logit:.4}, {max_logit:.4}] (bias {bias:.4}, ‖weight‖ {weight_norm:.4}); \
         it never crosses 0, so the classifier is effectively constant"
    )]
    UnreachableBoundary {
        /// `‖weight‖₂`.
        weight_norm: f64,
        /// Head bias.
        bias: f64,
        /// Minimum achievable logit (`bias − ‖w‖`).
        min_logit: f64,
        /// Maximum achievable logit (`bias + ‖w‖`).
        max_logit: f64,
    },

    /// The output variance across the probe set is below the threshold — the
    /// head is effectively constant.
    #[error(
        "constant output: probability variance {variance:.3e} across {probe_size} \
         probes is below threshold {threshold:.3e} (outputs in [{min_output:.4}, \
         {max_output:.4}])"
    )]
    ConstantOutput {
        /// Population variance of the probability outputs.
        variance: f64,
        /// Variance floor that was violated.
        threshold: f64,
        /// Minimum probability output over the probe set.
        min_output: f64,
        /// Maximum probability output over the probe set.
        max_output: f64,
        /// Number of probes evaluated.
        probe_size: usize,
    },

    /// The predicted-positive rate on a balanced probe is at/above the ceiling.
    #[error(
        "degenerate class balance: predicted-positive rate {positive_rate:.4} on \
         {probe_size} balanced probes is at/above ceiling {ceiling:.4}"
    )]
    ClassBalance {
        /// Fraction of probes classified positive (`logit ≥ 0`).
        positive_rate: f64,
        /// Ceiling that was violated.
        ceiling: f64,
        /// Number of probes evaluated.
        probe_size: usize,
    },

    /// A report was surfaced without a paired baseline (ADR-291).
    #[error("missing baseline: {reason}")]
    MissingBaseline {
        /// Why the baseline is considered missing/blank.
        reason: String,
    },

    /// A metric computed under one kind was surfaced under another task name.
    #[error(
        "metric-name provenance mismatch: metric computed as `{computed_kind}` \
         ({computed_label}) may not be surfaced as `{surfaced_kind}` \
         ({surfaced_label})"
    )]
    MetricProvenance {
        /// Kind the metric was actually computed under.
        computed_kind: &'static str,
        /// Display label of the computed kind.
        computed_label: &'static str,
        /// Kind the metric was being surfaced under.
        surfaced_kind: &'static str,
        /// Display label of the surfaced kind.
        surfaced_label: &'static str,
    },
}

// ---------------------------------------------------------------------------
// LinearHead
// ---------------------------------------------------------------------------

/// A single-logit linear classification head `logit(x) = weight · x + bias`,
/// the shape of the published presence head. Kept parameter-only (no `tch`) so
/// the gates run on the workspace test gate without libtorch.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearHead {
    weight: Vec<f32>,
    bias: f32,
}

impl LinearHead {
    /// Build a head from raw parameters, validating them at the boundary.
    ///
    /// # Errors
    ///
    /// - [`GateError::EmptyWeight`] when `weight` is empty.
    /// - [`GateError::NonFiniteParameter`] when any weight or the bias is
    ///   NaN/±inf.
    pub fn new(weight: Vec<f32>, bias: f32) -> Result<Self, GateError> {
        if weight.is_empty() {
            return Err(GateError::EmptyWeight);
        }
        for (i, &w) in weight.iter().enumerate() {
            if !w.is_finite() {
                return Err(GateError::NonFiniteParameter {
                    what: format!("weight[{i}]"),
                    value: w as f64,
                });
            }
        }
        if !bias.is_finite() {
            return Err(GateError::NonFiniteParameter {
                what: "bias".to_string(),
                value: bias as f64,
            });
        }
        Ok(LinearHead { weight, bias })
    }

    /// Embedding dimension.
    #[must_use]
    pub fn dim(&self) -> usize {
        self.weight.len()
    }

    /// Head bias as `f64`.
    #[must_use]
    pub fn bias(&self) -> f64 {
        self.bias as f64
    }

    /// `‖weight‖₂`.
    #[must_use]
    pub fn weight_norm(&self) -> f64 {
        self.weight
            .iter()
            .map(|&w| (w as f64) * (w as f64))
            .sum::<f64>()
            .sqrt()
    }

    /// Minimum achievable logit for a unit-norm embedding (`bias − ‖w‖`).
    #[must_use]
    pub fn min_logit_normalized(&self) -> f64 {
        self.bias() - self.weight_norm()
    }

    /// Maximum achievable logit for a unit-norm embedding (`bias + ‖w‖`).
    #[must_use]
    pub fn max_logit_normalized(&self) -> f64 {
        self.bias() + self.weight_norm()
    }

    /// Compute the logit `weight · embedding + bias`.
    ///
    /// # Errors
    ///
    /// [`GateError::ProbeDimMismatch`] when `embedding.len() != self.dim()`.
    pub fn logit(&self, embedding: &[f32]) -> Result<f64, GateError> {
        if embedding.len() != self.dim() {
            return Err(GateError::ProbeDimMismatch {
                expected: self.dim(),
                actual: embedding.len(),
            });
        }
        let dot: f64 = self
            .weight
            .iter()
            .zip(embedding)
            .map(|(&w, &x)| (w as f64) * (x as f64))
            .sum();
        Ok(dot + self.bias())
    }

    /// Presence probability `σ(logit)`, numerically stable.
    ///
    /// # Errors
    ///
    /// [`GateError::ProbeDimMismatch`] when `embedding.len() != self.dim()`.
    pub fn presence_prob(&self, embedding: &[f32]) -> Result<f64, GateError> {
        Ok(sigmoid(self.logit(embedding)?))
    }
}

/// Numerically stable logistic sigmoid.
fn sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}

// ---------------------------------------------------------------------------
// ProbeSet
// ---------------------------------------------------------------------------

/// A deterministic set of L2-normalized embeddings used to probe a head.
///
/// [`ProbeSet::sweep_for_head`] sweeps the embedding whose projection onto the
/// weight direction ranges over `[-1, 1]`, so the logit ranges over the full
/// reachable interval `[bias − ‖w‖, bias + ‖w‖]`. This is the degenerate
/// normalized-embedding probe from issue 1521: every embedding is unit norm,
/// and the sweep is exactly what exposes an unreachable boundary or a constant
/// output, independent of embedding dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeSet {
    embeddings: Vec<Vec<f32>>,
}

impl ProbeSet {
    /// Build a probe set from explicit embeddings, validating shape/finiteness.
    ///
    /// # Errors
    ///
    /// - [`GateError::EmptyProbe`] when `embeddings` is empty.
    /// - [`GateError::NonFiniteParameter`] when any coordinate is NaN/±inf.
    pub fn from_embeddings(embeddings: Vec<Vec<f32>>) -> Result<Self, GateError> {
        if embeddings.is_empty() {
            return Err(GateError::EmptyProbe);
        }
        for row in &embeddings {
            if row.is_empty() {
                return Err(GateError::EmptyProbe);
            }
            for (i, &v) in row.iter().enumerate() {
                if !v.is_finite() {
                    return Err(GateError::NonFiniteParameter {
                        what: format!("probe[{i}]"),
                        value: v as f64,
                    });
                }
            }
        }
        Ok(ProbeSet { embeddings })
    }

    /// Sweep the reachable logit range with `num` L2-normalized embeddings.
    ///
    /// Each embedding is `x = c·û_w + √(1−c²)·û⊥` for a cosine `c` linearly
    /// spaced over `[-1, 1]`, where `û_w` is the weight direction and `û⊥` is a
    /// fixed unit vector orthogonal to it — so `‖x‖ = 1` and the projection
    /// onto `w` is exactly `c·‖w‖`. `num` is clamped to at least 2. When the
    /// weight norm is ~0 (a genuinely constant head) the sweep falls back to
    /// signed basis vectors, which still expose the constant output.
    #[must_use]
    pub fn sweep_for_head(head: &LinearHead, num: usize) -> Self {
        let num = num.max(2);
        let dim = head.dim();
        let norm = head.weight_norm();

        // Degenerate weight: no direction to sweep. Use signed basis vectors;
        // the head is constant regardless of input, which the gate will catch.
        if norm < 1e-12 {
            let mut embeddings = Vec::with_capacity(num);
            for k in 0..num {
                let mut e = vec![0.0f32; dim];
                let idx = k % dim;
                e[idx] = if k % 2 == 0 { 1.0 } else { -1.0 };
                embeddings.push(e);
            }
            return ProbeSet { embeddings };
        }

        let u_w: Vec<f64> = head.weight.iter().map(|&w| (w as f64) / norm).collect();

        // dim == 1: the only unit embeddings are ±1.
        if dim == 1 {
            return ProbeSet {
                embeddings: vec![vec![-1.0f32], vec![1.0f32]],
            };
        }

        // Pick the axis least aligned with the weight for a stable orthogonal
        // direction; project it off u_w and normalize.
        let k = argmin_abs(&u_w);
        let dot_k = u_w[k];
        let perp_norm = (1.0 - dot_k * dot_k).sqrt();
        let u_perp: Vec<f64> = (0..dim)
            .map(|i| {
                let e_i = if i == k { 1.0 } else { 0.0 };
                (e_i - dot_k * u_w[i]) / perp_norm
            })
            .collect();

        let mut embeddings = Vec::with_capacity(num);
        for j in 0..num {
            let c = -1.0 + 2.0 * (j as f64) / ((num - 1) as f64);
            let s = (1.0 - c * c).max(0.0).sqrt();
            let x: Vec<f32> = (0..dim)
                .map(|i| (c * u_w[i] + s * u_perp[i]) as f32)
                .collect();
            embeddings.push(x);
        }
        ProbeSet { embeddings }
    }

    /// Number of probes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Whether the probe set is empty (never true after construction).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Probability outputs of `head` over every probe.
    ///
    /// # Errors
    ///
    /// [`GateError::ProbeDimMismatch`] when a probe length differs from the head
    /// dimension.
    pub fn probabilities(&self, head: &LinearHead) -> Result<Vec<f64>, GateError> {
        self.embeddings
            .iter()
            .map(|e| head.presence_prob(e))
            .collect()
    }
}

/// Index of the smallest-magnitude entry (ties resolved to the first).
fn argmin_abs(v: &[f64]) -> usize {
    let mut best = 0usize;
    let mut best_abs = f64::INFINITY;
    for (i, &x) in v.iter().enumerate() {
        let a = x.abs();
        if a < best_abs {
            best_abs = a;
            best = i;
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Gates
// ---------------------------------------------------------------------------

/// Fail when the decision boundary is analytically unreachable for a
/// normalized-embedding linear head.
///
/// With `‖x‖ = 1`, Cauchy–Schwarz confines the logit to
/// `[bias − ‖w‖, bias + ‖w‖]`. If this interval does not straddle `0` (i.e.
/// `|bias| ≥ ‖w‖`), the sign of the logit is fixed and the classifier is
/// effectively constant. The issue-1521 head (`‖w‖ ≈ 3.67`, `bias ≈ 8.19`) has
/// `min_logit ≈ 4.52 > 0` and fails here.
///
/// # Errors
///
/// [`GateFailure::UnreachableBoundary`] with the confining interval.
pub fn check_unreachable_boundary(head: &LinearHead) -> Result<(), GateFailure> {
    let min_logit = head.min_logit_normalized();
    let max_logit = head.max_logit_normalized();
    // The boundary is reachable only if the interval straddles zero. At the
    // exact touch (`min_logit == 0` or `max_logit == 0`) it is reachable at a
    // single antipodal point only — treat as unreachable.
    if min_logit >= 0.0 || max_logit <= 0.0 {
        return Err(GateFailure::UnreachableBoundary {
            weight_norm: head.weight_norm(),
            bias: head.bias(),
            min_logit,
            max_logit,
        });
    }
    Ok(())
}

/// Fail when the head's probability output has variance below `min_variance`
/// across the probe set — an effectively constant classifier.
///
/// # Errors
///
/// - [`GateError`] when a probe length is wrong.
/// - [`GateFailure::ConstantOutput`] when the variance is below `min_variance`.
pub fn check_constant_output(
    head: &LinearHead,
    probe: &ProbeSet,
    min_variance: f64,
) -> Result<(), GateOutcomeError> {
    let probs = probe.probabilities(head).map_err(GateOutcomeError::Input)?;
    let n = probs.len() as f64;
    let mean = probs.iter().sum::<f64>() / n;
    let variance = probs.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / n;
    if variance < min_variance {
        let min_output = probs.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_output = probs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        return Err(GateOutcomeError::Failure(GateFailure::ConstantOutput {
            variance,
            threshold: min_variance,
            min_output,
            max_output,
            probe_size: probs.len(),
        }));
    }
    Ok(())
}

/// Fail when the predicted-positive rate (`logit ≥ 0`) on a balanced probe set
/// is at/above `max_positive_rate`.
///
/// # Errors
///
/// - [`GateError`] when a probe length is wrong.
/// - [`GateFailure::ClassBalance`] when the positive rate is at/above the
///   ceiling.
pub fn check_class_balance(
    head: &LinearHead,
    probe: &ProbeSet,
    max_positive_rate: f64,
) -> Result<(), GateOutcomeError> {
    let probs = probe.probabilities(head).map_err(GateOutcomeError::Input)?;
    let positives = probs.iter().filter(|&&p| p >= 0.5).count();
    let positive_rate = positives as f64 / probs.len() as f64;
    if positive_rate >= max_positive_rate {
        return Err(GateOutcomeError::Failure(GateFailure::ClassBalance {
            positive_rate,
            ceiling: max_positive_rate,
            probe_size: probs.len(),
        }));
    }
    Ok(())
}

/// Fail when a metric is surfaced without a paired baseline (ADR-291).
///
/// A well-formed [`EvaluationReport`] structurally carries its `baseline_metric`
/// (a model number can never be built without one), so this gate's job is to
/// reject the *absence* of a report and any non-finite baseline slipped in from
/// elsewhere.
///
/// # Errors
///
/// [`GateFailure::MissingBaseline`] when `report` is `None` or its baseline is
/// not finite.
pub fn check_baseline(report: Option<&EvaluationReport>) -> Result<(), GateFailure> {
    match report {
        None => Err(GateFailure::MissingBaseline {
            reason: "no EvaluationReport was provided; a model number must be \
                     paired with a mean-pose/majority baseline (ADR-291)"
                .to_string(),
        }),
        Some(r) if !r.baseline_metric.is_finite() => Err(GateFailure::MissingBaseline {
            reason: format!(
                "baseline for `{}` is not finite ({})",
                r.metric_name, r.baseline_metric
            ),
        }),
        Some(_) => Ok(()),
    }
}

/// The result of running a gate that consumes a probe set: either a malformed
/// input ([`GateError`]) or a well-formed head that the gate rejected
/// ([`GateFailure`]).
#[derive(Debug, Error)]
pub enum GateOutcomeError {
    /// Malformed input reached the gate.
    #[error("gate input error: {0}")]
    Input(#[from] GateError),

    /// The gate rejected a well-formed artifact.
    #[error("gate failed: {0}")]
    Failure(#[from] GateFailure),
}

// ---------------------------------------------------------------------------
// Metric-name provenance
// ---------------------------------------------------------------------------

/// The computed kind of a scalar metric. The kind is the source of truth; the
/// display label ([`MetricKind::label`]) derives from it, so a metric computed
/// as [`MetricKind::TemporalTriplet`] can never present itself as
/// [`MetricKind::Presence`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    /// Binary presence-detection accuracy.
    Presence,
    /// Temporal-triplet (representation-ordering) accuracy — an ordering task,
    /// **not** presence detection.
    TemporalTriplet,
    /// Percentage of correct keypoints.
    Pck,
    /// Mean per-joint position error.
    Mpjpe,
}

impl MetricKind {
    /// Stable short kind name (kebab-case), for machine comparison.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            MetricKind::Presence => "presence",
            MetricKind::TemporalTriplet => "temporal-triplet",
            MetricKind::Pck => "pck",
            MetricKind::Mpjpe => "mpjpe",
        }
    }

    /// Human display label derived from the kind — the *only* way a metric is
    /// named, so the label always matches how the number was computed.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            MetricKind::Presence => "presence accuracy",
            MetricKind::TemporalTriplet => "temporal-triplet accuracy",
            MetricKind::Pck => "PCK",
            MetricKind::Mpjpe => "MPJPE",
        }
    }
}

/// A scalar metric that carries its computed [`MetricKind`]. There is no
/// constructor that accepts a free-form label — the label is always derived
/// from `kind` — so a temporal-triplet number cannot be built as "presence".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabeledMetric {
    kind: MetricKind,
    value: f64,
}

impl LabeledMetric {
    /// Build a metric of a given kind, validating the value is finite.
    ///
    /// # Errors
    ///
    /// [`GateError::NonFiniteMetric`] when `value` is NaN/±inf.
    pub fn new(kind: MetricKind, value: f64) -> Result<Self, GateError> {
        if !value.is_finite() {
            return Err(GateError::NonFiniteMetric(value));
        }
        Ok(LabeledMetric { kind, value })
    }

    /// The computed kind (source of truth).
    #[must_use]
    pub fn kind(&self) -> MetricKind {
        self.kind
    }

    /// The scalar value.
    #[must_use]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Display label derived from [`MetricKind::label`].
    #[must_use]
    pub fn label(&self) -> &'static str {
        self.kind.label()
    }

    /// One-line self-describing string, e.g. `"temporal-triplet accuracy: 0.9000"`.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("{}: {:.4}", self.label(), self.value)
    }
}

/// Fail when a metric would be surfaced under a task name that does not match
/// the kind it was computed as (temporal-triplet ≠ presence).
///
/// # Errors
///
/// [`GateFailure::MetricProvenance`] when `metric.kind() != surfaced_as`.
pub fn check_metric_provenance(
    metric: &LabeledMetric,
    surfaced_as: MetricKind,
) -> Result<(), GateFailure> {
    if metric.kind() != surfaced_as {
        return Err(GateFailure::MetricProvenance {
            computed_kind: metric.kind().name(),
            computed_label: metric.kind().label(),
            surfaced_kind: surfaced_as.name(),
            surfaced_label: surfaced_as.label(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Aggregate report — a single entry point for the CI model-check gate.
// ---------------------------------------------------------------------------

/// Aggregated verdict of the head-level release gates. Runs
/// [`check_unreachable_boundary`], [`check_constant_output`], and
/// [`check_class_balance`] against a swept probe set, plus [`check_baseline`]
/// when a report is supplied. Collects *every* failure rather than short-
/// circuiting so a maintainer sees all defects at once.
#[derive(Debug, Clone, Default)]
pub struct ModelGateReport {
    /// Failures collected across the gates.
    pub failures: Vec<GateFailure>,
}

impl ModelGateReport {
    /// Whether every gate passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }

    /// Human-readable multi-line summary of the failures (or a pass line).
    #[must_use]
    pub fn summary(&self) -> String {
        if self.passed() {
            return "model release gates: PASS".to_string();
        }
        let mut s = format!("model release gates: FAIL ({} issue(s))", self.failures.len());
        for f in &self.failures {
            s.push_str("\n  - ");
            s.push_str(&f.to_string());
        }
        s
    }
}

/// Run the head-level release gates and collect all failures.
///
/// Uses [`DEFAULT_MIN_OUTPUT_VARIANCE`], [`DEFAULT_MAX_POSITIVE_RATE`], and a
/// [`DEFAULT_PROBE_POINTS`] sweep. Malformed probe evaluation is impossible here
/// because the sweep is generated to match the head dimension.
///
/// This is the *aggregate* form: it adds the report-level baseline gate on top
/// of the head gates, so it needs an [`EvaluationReport`]. Use
/// [`evaluate_head_gates`] when all you have is a head — an artifact file
/// carries no report.
#[must_use]
pub fn evaluate_linear_head(
    head: &LinearHead,
    baseline: Option<&EvaluationReport>,
) -> ModelGateReport {
    let mut report = evaluate_head_gates(head);

    if let Err(f) = check_baseline(baseline) {
        report.failures.push(f);
    }

    report
}

/// Run only the head-level release gates: unreachable decision boundary,
/// constant output, and degenerate class balance.
///
/// These three are the gates a *classifier artifact* can be judged by on its
/// own — they read nothing but the head's parameters and the probe set the
/// sweep derives from them. The report-level gates ([`check_baseline`] and
/// [`check_metric_provenance`]) are deliberately excluded: they judge a
/// *reported metric*, which an artifact file does not carry.
///
/// The probe set is the L2-normalized sweep from [`ProbeSet::sweep_for_head`],
/// so these gates are only meaningful for a head that consumes L2-normalized
/// embeddings. Applying them to a head with unnormalized inputs would produce
/// a false failure; the caller is responsible for that assumption.
#[must_use]
pub fn evaluate_head_gates(head: &LinearHead) -> ModelGateReport {
    let mut failures = Vec::new();

    if let Err(f) = check_unreachable_boundary(head) {
        failures.push(f);
    }

    let probe = ProbeSet::sweep_for_head(head, DEFAULT_PROBE_POINTS);
    if let Err(GateOutcomeError::Failure(f)) =
        check_constant_output(head, &probe, DEFAULT_MIN_OUTPUT_VARIANCE)
    {
        failures.push(f);
    }
    if let Err(GateOutcomeError::Failure(f)) =
        check_class_balance(head, &probe, DEFAULT_MAX_POSITIVE_RATE)
    {
        failures.push(f);
    }

    ModelGateReport { failures }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::SplitProtocol;

    /// The issue-1521 presence head: `‖w‖ ≈ 3.67`, `bias ≈ 8.19`. We build a
    /// weight vector whose norm is 3.67 by spreading it over 4 dims.
    fn issue_1521_head() -> LinearHead {
        // 4 equal components with norm 3.67 ⇒ each = 3.67 / 2 = 1.835.
        let c = 3.67f32 / 2.0;
        LinearHead::new(vec![c, c, c, c], 8.19).unwrap()
    }

    /// A healthy synthetic head: reachable boundary, balanced, diverse output.
    fn healthy_head() -> LinearHead {
        // ‖w‖ = 4, bias = 0 ⇒ logit sweeps [-4, 4], sigmoid [0.018, 0.982].
        LinearHead::new(vec![2.0, 2.0, 2.0, 2.0], 0.0).unwrap()
    }

    #[test]
    fn issue_1521_norm_and_bias_match_the_review() {
        let h = issue_1521_head();
        assert!((h.weight_norm() - 3.67).abs() < 1e-4, "norm {}", h.weight_norm());
        assert!((h.bias() - 8.19).abs() < 1e-6);
        // Smallest achievable logit stays positive — the degenerate signature.
        assert!(h.min_logit_normalized() > 0.0);
    }

    #[test]
    fn issue_1521_fails_unreachable_boundary() {
        let h = issue_1521_head();
        let err = check_unreachable_boundary(&h).unwrap_err();
        match err {
            GateFailure::UnreachableBoundary { min_logit, max_logit, .. } => {
                assert!(min_logit > 0.0);
                assert!(max_logit > 0.0);
            }
            other => panic!("expected UnreachableBoundary, got {other:?}"),
        }
        // The failure message must carry the offending numbers.
        let msg = check_unreachable_boundary(&h).unwrap_err().to_string();
        assert!(msg.contains("unreachable decision boundary"), "{msg}");
    }

    #[test]
    fn issue_1521_fails_constant_output() {
        let h = issue_1521_head();
        let probe = ProbeSet::sweep_for_head(&h, DEFAULT_PROBE_POINTS);
        let err = check_constant_output(&h, &probe, DEFAULT_MIN_OUTPUT_VARIANCE).unwrap_err();
        match err {
            GateOutcomeError::Failure(GateFailure::ConstantOutput {
                variance,
                threshold,
                min_output,
                ..
            }) => {
                assert!(variance < threshold);
                // Even the minimum output is a near-certain positive.
                assert!(min_output > 0.98, "min_output {min_output}");
            }
            other => panic!("expected ConstantOutput, got {other:?}"),
        }
    }

    #[test]
    fn issue_1521_aggregate_fails_boundary_and_constant() {
        let h = issue_1521_head();
        let report = evaluate_linear_head(&h, None);
        assert!(!report.passed());
        let has_boundary = report
            .failures
            .iter()
            .any(|f| matches!(f, GateFailure::UnreachableBoundary { .. }));
        let has_constant = report
            .failures
            .iter()
            .any(|f| matches!(f, GateFailure::ConstantOutput { .. }));
        assert!(has_boundary, "expected unreachable-boundary failure");
        assert!(has_constant, "expected constant-output failure");
    }

    #[test]
    fn healthy_head_passes_head_gates() {
        let h = healthy_head();
        assert!(check_unreachable_boundary(&h).is_ok());

        let probe = ProbeSet::sweep_for_head(&h, DEFAULT_PROBE_POINTS);
        assert!(check_constant_output(&h, &probe, DEFAULT_MIN_OUTPUT_VARIANCE).is_ok());
        assert!(check_class_balance(&h, &probe, DEFAULT_MAX_POSITIVE_RATE).is_ok());

        // With a baseline report supplied, the aggregate passes entirely.
        let rep = EvaluationReport::synthetic(
            SplitProtocol::CrossSubject,
            "presence",
            0.71,
            0.50,
        )
        .unwrap();
        let gate = evaluate_linear_head(&h, Some(&rep));
        assert!(gate.passed(), "{}", gate.summary());
    }

    #[test]
    fn head_gates_alone_pass_a_healthy_head_without_a_report() {
        // The aggregate needs a baseline report; the head-only form does not,
        // because an artifact file carries no report.
        let h = healthy_head();
        let gate = evaluate_head_gates(&h);
        assert!(gate.passed(), "{}", gate.summary());
        assert!(gate.failures.is_empty());
    }

    #[test]
    fn head_gates_alone_reject_the_issue_1521_head() {
        let h = issue_1521_head();
        let gate = evaluate_head_gates(&h);
        assert!(!gate.passed());
        let has_boundary = gate
            .failures
            .iter()
            .any(|f| matches!(f, GateFailure::UnreachableBoundary { .. }));
        let has_constant = gate
            .failures
            .iter()
            .any(|f| matches!(f, GateFailure::ConstantOutput { .. }));
        let has_balance = gate
            .failures
            .iter()
            .any(|f| matches!(f, GateFailure::ClassBalance { .. }));
        assert!(has_boundary, "expected unreachable-boundary failure");
        assert!(has_constant, "expected constant-output failure");
        assert!(has_balance, "expected class-balance failure");
        // The baseline gate is a report-level gate and is not part of the
        // head-only form.
        assert!(
            !gate.failures
                .iter()
                .any(|f| matches!(f, GateFailure::MissingBaseline { .. })),
            "the head-only form must not require a baseline report"
        );
    }

    #[test]
    fn constant_weight_head_fails_constant_output() {
        // Zero weight ⇒ logit == bias for every input ⇒ genuinely constant.
        let h = LinearHead::new(vec![0.0, 0.0, 0.0], 0.3).unwrap();
        let probe = ProbeSet::sweep_for_head(&h, DEFAULT_PROBE_POINTS);
        let err = check_constant_output(&h, &probe, DEFAULT_MIN_OUTPUT_VARIANCE).unwrap_err();
        assert!(matches!(
            err,
            GateOutcomeError::Failure(GateFailure::ConstantOutput { .. })
        ));
    }

    #[test]
    fn degenerate_class_balance_fails() {
        // Issue-1521 head classifies 100% positive.
        let h = issue_1521_head();
        let probe = ProbeSet::sweep_for_head(&h, DEFAULT_PROBE_POINTS);
        let err = check_class_balance(&h, &probe, DEFAULT_MAX_POSITIVE_RATE).unwrap_err();
        match err {
            GateOutcomeError::Failure(GateFailure::ClassBalance { positive_rate, .. }) => {
                assert!((positive_rate - 1.0).abs() < 1e-9);
            }
            other => panic!("expected ClassBalance, got {other:?}"),
        }
    }

    #[test]
    fn missing_baseline_fails_and_present_passes() {
        assert!(check_baseline(None).is_err());
        let rep = EvaluationReport::synthetic(
            SplitProtocol::CrossSubject,
            "pck@0.2",
            0.61,
            0.41,
        )
        .unwrap();
        assert!(check_baseline(Some(&rep)).is_ok());
    }

    #[test]
    fn temporal_triplet_cannot_be_labeled_presence() {
        let m = LabeledMetric::new(MetricKind::TemporalTriplet, 0.90).unwrap();
        // The label derives from the computed kind — it is NOT "presence accuracy".
        assert_eq!(m.label(), "temporal-triplet accuracy");
        assert_ne!(m.label(), MetricKind::Presence.label());
        assert!(m.describe().contains("temporal-triplet accuracy"));

        // Surfacing it under the presence task name is a structural error.
        let err = check_metric_provenance(&m, MetricKind::Presence).unwrap_err();
        match err {
            GateFailure::MetricProvenance {
                computed_kind,
                surfaced_kind,
                ..
            } => {
                assert_eq!(computed_kind, "temporal-triplet");
                assert_eq!(surfaced_kind, "presence");
            }
            other => panic!("expected MetricProvenance, got {other:?}"),
        }
    }

    #[test]
    fn matching_metric_provenance_passes() {
        let m = LabeledMetric::new(MetricKind::Presence, 0.88).unwrap();
        assert!(check_metric_provenance(&m, MetricKind::Presence).is_ok());
    }

    #[test]
    fn malformed_inputs_are_errors_not_panics() {
        assert_eq!(LinearHead::new(vec![], 0.0).unwrap_err(), GateError::EmptyWeight);
        assert!(matches!(
            LinearHead::new(vec![f32::NAN], 0.0).unwrap_err(),
            GateError::NonFiniteParameter { .. }
        ));
        assert!(matches!(
            LinearHead::new(vec![1.0], f32::INFINITY).unwrap_err(),
            GateError::NonFiniteParameter { .. }
        ));
        assert!(matches!(
            LabeledMetric::new(MetricKind::Pck, f64::NAN).unwrap_err(),
            GateError::NonFiniteMetric(_)
        ));
        assert_eq!(
            ProbeSet::from_embeddings(vec![]).unwrap_err(),
            GateError::EmptyProbe
        );
    }

    #[test]
    fn logit_rejects_dimension_mismatch() {
        let h = healthy_head(); // dim 4
        assert!(matches!(
            h.logit(&[1.0, 2.0]).unwrap_err(),
            GateError::ProbeDimMismatch { expected: 4, actual: 2 }
        ));
    }

    #[test]
    fn sweep_embeddings_are_unit_norm() {
        let h = healthy_head();
        let probe = ProbeSet::sweep_for_head(&h, 16);
        for e in &probe.embeddings {
            let n: f64 = e.iter().map(|&x| (x as f64) * (x as f64)).sum::<f64>().sqrt();
            assert!((n - 1.0).abs() < 1e-5, "norm {n}");
        }
    }
}
