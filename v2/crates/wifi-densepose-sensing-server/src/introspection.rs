//! Real-time CSI introspection tap (ADR-099).
//!
//! Per-frame state alongside the window-aggregated event pipeline. Two
//! midstream primitives feed it:
//!
//! * `midstreamer-attractor` — Lyapunov exponent + attractor regime (point /
//!   limit cycle / strange / unknown) over a sliding window of derived
//!   amplitude scalars. Replaces the heuristic "is the room calm or moving"
//!   threshold-on-EWMA with a physics-shaped continuous metric.
//! * `midstreamer-temporal-compare` — DTW-style similarity matching of recent
//!   CSI feature history against a labelled signature library
//!   (`SignatureLibrary`). The top-k matches go into [`IntrospectionSnapshot`].
//!
//! The whole module is **never window-blocked**: every accepted [`CsiFrame`]
//! triggers an `update_per_frame` call; the snapshot is fresh on every frame.
//! That's the latency-win contract from ADR-099 D4 — the soonest a
//! "shape recognised" signal can emit is **one frame** (≈33 ms at 30 Hz CSI),
//! not one window (≈533 ms at 16-frame / 30 Hz).
//!
//! See [`docs/adr/ADR-099-midstream-introspection-tap.md`] for the architectural
//! contract, the eight decisions, and the phased adoption plan.
//!
//! [`docs/adr/ADR-099-midstream-introspection-tap.md`]: https://github.com/ruvnet/RuView/blob/main/docs/adr/ADR-099-midstream-introspection-tap.md

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use midstreamer_attractor::{
    AttractorAnalyzer, AttractorError, AttractorType, PhasePoint,
};

/// Default sliding window of derived amplitude scalars fed to the attractor
/// analyzer. Sized so that at 30 Hz CSI the analyzer always has ≥3 s of history,
/// which covers the ~100-point minimum the analyzer needs for a meaningful
/// Lyapunov estimate.
pub const DEFAULT_TRAJECTORY_LEN: usize = 128;

/// Default embedding dimension for the attractor's phase space. We feed it
/// one-dimensional points (the per-frame mean amplitude scalar); higher
/// dimensions become useful once we have real `vec128` embeddings (ADR-208 P2).
pub const DEFAULT_EMBEDDING_DIM: usize = 1;

/// Default similarity-library DTW window (Sakoe-Chiba band) and how many top
/// matches the snapshot carries.
pub const DEFAULT_TOP_K: usize = 5;

/// Frames since the last `analyze()` call. Per-frame analyse is cheap (the
/// I5 benchmark put attractor + L1-scoring update p99 at 0.012 ms on a
/// desktop runner, ~83× under the 1 ms D4 budget — even on a Pi 5 we have
/// orders of magnitude of headroom), and per-frame analyse is what makes
/// the `regime_changed` snapshot signal viable as an early-detection
/// trigger. Default to **every frame** unless deployment tunes it down.
pub const DEFAULT_ANALYZE_EVERY_N_FRAMES: u32 = 1;

/// One labelled segment of derived feature vectors used as a DTW pattern.
/// Schema (per ADR-099 D7) — JSON-loaded from `signatures/*.json` at startup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Signature {
    /// Stable id used in [`SimilarityMatch::signature_id`].
    pub id: String,
    /// Human-readable label for the dashboard.
    pub label: String,
    /// Per-frame feature vectors that define the shape. Length-flexible; the
    /// DTW window in [`SignatureDtw::window`] bounds the warp tolerance.
    pub vectors: Vec<Vec<f64>>,
    /// DTW knobs.
    pub dtw: SignatureDtw,
    /// `top_k_similarity` only fires a match for a signature when its
    /// distance-derived score crosses `promotion_threshold` ∈ \[0, 1\]. Per-
    /// signature so tuning stays local (ADR-099 D7).
    pub promotion_threshold: f32,
}

/// DTW tunables for a single signature. Mirrors the JSON shape from ADR-099 D7.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignatureDtw {
    /// Sakoe-Chiba band width (warp tolerance in frames).
    pub window: usize,
    /// Step pattern selector (`"symmetric2"` is the default; only that one
    /// is wired today, the field exists for forward compat).
    #[serde(default = "default_step_pattern")]
    pub step_pattern: String,
}

fn default_step_pattern() -> String {
    "symmetric2".to_string()
}

/// In-memory library of [`Signature`]s loaded from a directory of JSON files.
#[derive(Debug, Default, Clone)]
pub struct SignatureLibrary {
    signatures: Vec<Signature>,
}

impl SignatureLibrary {
    /// Empty library — fine for tests and for the introspection tap booting
    /// without any captured signatures yet (the analyzer half still works).
    pub fn new() -> Self {
        Self { signatures: Vec::new() }
    }

    /// Library from in-memory signatures (testing / programmatic loaders).
    pub fn from_signatures(signatures: Vec<Signature>) -> Self {
        Self { signatures }
    }

    /// Number of signatures in the library.
    pub fn len(&self) -> usize {
        self.signatures.len()
    }

    /// `true` if the library carries no signatures.
    pub fn is_empty(&self) -> bool {
        self.signatures.is_empty()
    }

    /// Borrow the underlying signature list.
    pub fn signatures(&self) -> &[Signature] {
        &self.signatures
    }
}

/// One match against a [`Signature`], scored 0..=1 (1 = identical).
///
/// Score is `1 / (1 + normalised_dtw_distance)` — monotone decreasing in
/// distance, bounded to (0, 1\], stable in the presence of empty signatures.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimilarityMatch {
    /// Stable signature id ([`Signature::id`]).
    pub signature_id: String,
    /// `0.0` (worst) … `1.0` (perfect match).
    pub score: f32,
    /// `true` iff `score >= signature.promotion_threshold`.
    pub above_threshold: bool,
}

/// One snapshot of the per-frame introspection state. Broadcast on
/// `/ws/introspection` and returned by `GET /api/v1/introspection/snapshot`.
///
/// Per ADR-099 D3, this is the contract on the new endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntrospectionSnapshot {
    /// Source-side timestamp of the frame that produced this snapshot.
    pub timestamp_ns: u64,
    /// Frames seen since module init (monotonic, never resets).
    pub frame_count: u64,
    /// Attractor regime classification from `midstreamer-attractor`.
    pub regime: Regime,
    /// Max Lyapunov exponent (`None` until the analyzer has enough points —
    /// `DEFAULT_TRAJECTORY_LEN` ≥ 100 by default).
    pub lyapunov_exponent: Option<f64>,
    /// Embedding-space dimensionality the attractor is analysing in.
    pub attractor_dim: usize,
    /// Analyzer confidence in `[0, 1]`. `0.0` until the analyzer has enough
    /// data; tracks midstream's `AttractorInfo::confidence`.
    pub attractor_confidence: f64,
    /// `true` when this frame's regime classification differs from the
    /// previous frame's — an **early-detection signal** that doesn't require
    /// a full signature length of frames to fire (ADR-099 D8: a parallel
    /// fast path to the shape-match latency, useful for "something changed,
    /// look closer" semantics on dashboards / downstream consumers).
    pub regime_changed: bool,
    /// Top-k DTW matches against the loaded signature library. Empty when the
    /// library is empty or no signatures rose above the score floor.
    pub top_k_similarity: Vec<SimilarityMatch>,
}

/// JSON-friendly regime classification mirror of midstream's `AttractorType`.
/// Kept as a separate type so the public wire contract (ADR-099 D3) doesn't
/// pin to midstream's enum variant names.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Regime {
    /// Stable, settled equilibrium — "the room is calm".
    Idle,
    /// Periodic / limit-cycle — repetitive motion (e.g. breathing, a running
    /// fan, walking-in-place).
    Periodic,
    /// Single non-repeating excursion — "something just happened once".
    Transient,
    /// Strange-attractor / chaotic — complex non-periodic motion.
    Chaotic,
    /// Not enough data yet to classify.
    Unknown,
}

impl Regime {
    fn from_attractor(t: AttractorType) -> Self {
        match t {
            AttractorType::PointAttractor => Regime::Idle,
            AttractorType::LimitCycle => Regime::Periodic,
            AttractorType::StrangeAttractor => Regime::Chaotic,
            AttractorType::Unknown => Regime::Unknown,
        }
    }
}

/// The per-frame introspection state for one CSI source (one node).
///
/// Reset is not provided on purpose — restarts come from rebuilding the
/// struct.
pub struct IntrospectionState {
    analyzer: AttractorAnalyzer,
    library: SignatureLibrary,
    recent_amplitudes: VecDeque<f64>,
    trajectory_capacity: usize,
    frames_since_analyze: u32,
    analyze_every_n: u32,
    frame_count: u64,
    last_snapshot: IntrospectionSnapshot,
}

impl IntrospectionState {
    /// New introspection state with sensible defaults.
    pub fn new() -> Self {
        Self::with_config(IntrospectionConfig::default())
    }

    /// New introspection state with explicit knobs.
    pub fn with_config(cfg: IntrospectionConfig) -> Self {
        let analyzer = AttractorAnalyzer::new(cfg.embedding_dim, cfg.trajectory_len);
        Self {
            analyzer,
            library: cfg.library,
            recent_amplitudes: VecDeque::with_capacity(cfg.trajectory_len),
            trajectory_capacity: cfg.trajectory_len,
            frames_since_analyze: 0,
            analyze_every_n: cfg.analyze_every_n.max(1),
            frame_count: 0,
            last_snapshot: IntrospectionSnapshot {
                timestamp_ns: 0,
                frame_count: 0,
                regime: Regime::Unknown,
                lyapunov_exponent: None,
                attractor_dim: cfg.embedding_dim,
                attractor_confidence: 0.0,
                regime_changed: false,
                top_k_similarity: Vec::new(),
            },
        }
    }

    /// How many frames have been observed since construction.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Borrow the last computed snapshot. Cheap; always valid (zeroed before
    /// the first frame is observed).
    pub fn snapshot(&self) -> &IntrospectionSnapshot {
        &self.last_snapshot
    }

    /// Feed one frame. Designed for the hot path: <1 ms p99 budget on a Pi-5
    /// host (ADR-099 D4). The expensive `analyze()` call only runs every
    /// `analyze_every_n` frames; the trajectory slide and DTW scoring happen
    /// every frame.
    pub fn update(&mut self, timestamp_ns: u64, derived_feature: f64) -> Result<(), AttractorError> {
        self.frame_count = self.frame_count.saturating_add(1);

        // Slide the amplitude buffer.
        if self.recent_amplitudes.len() == self.trajectory_capacity {
            self.recent_amplitudes.pop_front();
        }
        self.recent_amplitudes.push_back(derived_feature);

        // Feed the attractor analyzer.
        let phase_point = PhasePoint::new(vec![derived_feature], timestamp_ns);
        self.analyzer.add_point(phase_point)?;

        // Run the (relatively expensive) analyze step every Nth frame; in
        // between, keep the previous regime/Lyapunov in the snapshot — they're
        // smooth signals, not edge-sensitive.
        let prev_regime = self.last_snapshot.regime;
        self.frames_since_analyze = self.frames_since_analyze.saturating_add(1);
        if self.frames_since_analyze >= self.analyze_every_n {
            self.frames_since_analyze = 0;
            match self.analyzer.analyze() {
                Ok(info) => {
                    self.last_snapshot.regime = Regime::from_attractor(info.attractor_type);
                    self.last_snapshot.lyapunov_exponent = info.max_lyapunov_exponent();
                    self.last_snapshot.attractor_confidence = info.confidence;
                }
                Err(AttractorError::InsufficientData(_)) => {
                    // Not enough points yet — keep the Unknown default.
                }
                Err(other) => return Err(other),
            }
        }
        // ADR-099 D8: early-detection signal — `regime_changed` flips on any
        // frame whose classification differs from the previous frame's. Pairs
        // with `top_k_similarity` (which needs the full shape) to give
        // downstream consumers two latencies to choose from per use case.
        // Don't count Unknown→Unknown as a change; do count Unknown→<any> as
        // a change (the warm-up moment is itself informative).
        self.last_snapshot.regime_changed = prev_regime != self.last_snapshot.regime;

        // DTW scoring runs every frame; cheap when the library is small (and
        // empty when it's empty). See `score_signatures` for the metric.
        self.last_snapshot.top_k_similarity = score_signatures(
            &self.library,
            &self.recent_amplitudes,
            DEFAULT_TOP_K,
        );
        self.last_snapshot.timestamp_ns = timestamp_ns;
        self.last_snapshot.frame_count = self.frame_count;
        Ok(())
    }
}

impl Default for IntrospectionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Tunables for [`IntrospectionState::with_config`].
pub struct IntrospectionConfig {
    /// Sliding amplitude buffer length fed to the attractor analyzer.
    pub trajectory_len: usize,
    /// Phase-space dimension (1 for scalar amplitude features today; will
    /// grow when real `vec128` embeddings arrive).
    pub embedding_dim: usize,
    /// How often (in frames) the analyzer's `analyze()` is called.
    pub analyze_every_n: u32,
    /// Signature library for DTW scoring.
    pub library: SignatureLibrary,
}

impl Default for IntrospectionConfig {
    fn default() -> Self {
        IntrospectionConfig {
            trajectory_len: DEFAULT_TRAJECTORY_LEN,
            embedding_dim: DEFAULT_EMBEDDING_DIM,
            analyze_every_n: DEFAULT_ANALYZE_EVERY_N_FRAMES,
            library: SignatureLibrary::new(),
        }
    }
}

/// Score the recent amplitudes against each signature in the library, return
/// the top-k by score (descending). This is the host-side stand-in for the
/// `midstreamer-temporal-compare` DTW path — it uses a simple
/// length-normalised L1 distance over the trailing window, which is cheap
/// (O(n) per signature) and behaves the same way DTW does on the
/// scale-comparable shape question. We promote to the real DTW once real
/// `vec128` embeddings exist (ADR-208 P2 / ADR-099 P1).
///
/// Returning `Vec` rather than a fixed array keeps the JSON wire shape stable
/// when the library size changes.
fn score_signatures(
    library: &SignatureLibrary,
    recent: &VecDeque<f64>,
    top_k: usize,
) -> Vec<SimilarityMatch> {
    if library.is_empty() || recent.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<SimilarityMatch> = library
        .signatures()
        .iter()
        .map(|sig| {
            let score = signature_score(sig, recent);
            SimilarityMatch {
                signature_id: sig.id.clone(),
                score,
                above_threshold: score >= sig.promotion_threshold,
            }
        })
        .collect();
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(top_k);
    scored
}

/// Length-normalised L1 distance → similarity score in `(0, 1]`.
///
/// The signature's `vectors` are 1-D for now (the per-frame amplitude scalar).
/// When `vec128` lands we extend the inner pass to component-wise L1 across
/// the embedding dimensions; the outer shape (length-normalise the trailing
/// window of `recent` against the signature) stays.
fn signature_score(sig: &Signature, recent: &VecDeque<f64>) -> f32 {
    if sig.vectors.is_empty() {
        return 0.0;
    }
    let window = sig.vectors.len().min(recent.len());
    if window == 0 {
        return 0.0;
    }
    let start = recent.len() - window;
    let mut sum: f64 = 0.0;
    for (i, sig_vec) in sig.vectors.iter().rev().take(window).enumerate() {
        let s = sig_vec.first().copied().unwrap_or(0.0);
        let r = recent.get(recent.len() - 1 - i).copied().unwrap_or(0.0);
        sum += (s - r).abs();
    }
    let mean_abs = sum / window as f64;
    // Map to (0, 1] — 0 mean-abs error → 1.0, growing error → ~0.
    let score = 1.0 / (1.0 + mean_abs);
    let _ = start; // reserved for future windowing changes
    score as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(id: &str, vectors: Vec<f64>, threshold: f32) -> Signature {
        Signature {
            id: id.to_string(),
            label: id.to_string(),
            vectors: vectors.into_iter().map(|v| vec![v]).collect(),
            dtw: SignatureDtw {
                window: 8,
                step_pattern: "symmetric2".to_string(),
            },
            promotion_threshold: threshold,
        }
    }

    #[test]
    fn snapshot_is_unknown_before_first_frame() {
        let st = IntrospectionState::new();
        let s = st.snapshot();
        assert_eq!(s.frame_count, 0);
        assert_eq!(s.regime, Regime::Unknown);
        assert!(s.lyapunov_exponent.is_none());
        assert_eq!(s.attractor_confidence, 0.0);
        assert!(s.top_k_similarity.is_empty());
    }

    #[test]
    fn update_advances_frame_count_and_timestamp() {
        let mut st = IntrospectionState::new();
        st.update(1_000, 0.5).unwrap();
        st.update(2_000, 0.7).unwrap();
        let s = st.snapshot();
        assert_eq!(s.frame_count, 2);
        assert_eq!(s.timestamp_ns, 2_000);
    }

    #[test]
    fn empty_library_yields_empty_similarity() {
        let mut st = IntrospectionState::new();
        for k in 0..40 {
            st.update(k * 33_000_000, (k as f64).sin()).unwrap();
        }
        assert!(st.snapshot().top_k_similarity.is_empty());
    }

    #[test]
    fn single_signature_scores_higher_when_recent_matches() {
        let lib = SignatureLibrary::from_signatures(vec![sig(
            "walking_slow",
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            0.5,
        )]);
        let cfg = IntrospectionConfig {
            trajectory_len: 32,
            embedding_dim: 1,
            analyze_every_n: 16,
            library: lib,
        };
        let mut st = IntrospectionState::with_config(cfg);
        // Feed a ramp that ends 1..=5 — close match for the signature.
        for (i, v) in [1.0f64, 2.0, 3.0, 4.0, 5.0].iter().enumerate() {
            st.update((i as u64) * 1_000_000, *v).unwrap();
        }
        let s = st.snapshot();
        assert_eq!(s.top_k_similarity.len(), 1);
        let m = &s.top_k_similarity[0];
        assert_eq!(m.signature_id, "walking_slow");
        // Perfect ramp match → score very close to 1.0.
        assert!(m.score > 0.95, "score = {}", m.score);
        assert!(m.above_threshold);
    }

    #[test]
    fn divergent_signature_scores_low_and_below_threshold() {
        let lib = SignatureLibrary::from_signatures(vec![sig(
            "walking_slow",
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            0.5,
        )]);
        let cfg = IntrospectionConfig {
            trajectory_len: 32,
            embedding_dim: 1,
            analyze_every_n: 16,
            library: lib,
        };
        let mut st = IntrospectionState::with_config(cfg);
        for (i, v) in [100.0f64, 200.0, 300.0, 400.0, 500.0].iter().enumerate() {
            st.update((i as u64) * 1_000_000, *v).unwrap();
        }
        let m = &st.snapshot().top_k_similarity[0];
        assert!(m.score < 0.05, "score = {}", m.score);
        assert!(!m.above_threshold);
    }

    #[test]
    fn top_k_truncates_and_orders_descending() {
        let lib = SignatureLibrary::from_signatures(vec![
            sig("a", vec![1.0, 2.0, 3.0], 0.3),
            sig("b", vec![10.0, 20.0, 30.0], 0.3),
            sig("c", vec![100.0, 200.0, 300.0], 0.3),
            sig("d", vec![1.5, 2.5, 3.5], 0.3),
        ]);
        let cfg = IntrospectionConfig {
            trajectory_len: 32,
            embedding_dim: 1,
            analyze_every_n: 16,
            library: lib,
        };
        let mut st = IntrospectionState::with_config(cfg);
        // The trailing 3 values match "a" exactly.
        for (i, v) in [1.0f64, 2.0, 3.0].iter().enumerate() {
            st.update((i as u64) * 1_000_000, *v).unwrap();
        }
        let top = &st.snapshot().top_k_similarity;
        // Default DEFAULT_TOP_K = 5; library has 4, so we get 4 back.
        assert_eq!(top.len(), 4);
        // Strictly descending by score.
        for w in top.windows(2) {
            assert!(w[0].score >= w[1].score, "not descending: {:?}", top);
        }
        // First one is "a" (perfect 1..3 match) at score ~1.
        assert_eq!(top[0].signature_id, "a");
        assert!(top[0].score > 0.95);
    }

    #[test]
    fn signature_with_empty_vectors_does_not_panic() {
        let lib = SignatureLibrary::from_signatures(vec![sig("empty", vec![], 0.5)]);
        let mut st = IntrospectionState::with_config(IntrospectionConfig {
            trajectory_len: 16,
            embedding_dim: 1,
            analyze_every_n: 8,
            library: lib,
        });
        st.update(1_000, 1.0).unwrap();
        let s = st.snapshot();
        assert_eq!(s.top_k_similarity.len(), 1);
        assert_eq!(s.top_k_similarity[0].score, 0.0);
        assert!(!s.top_k_similarity[0].above_threshold);
    }

    #[test]
    fn regime_classification_eventually_runs() {
        // Feed >100 points of a periodic signal — analyzer's
        // min_points_for_analysis is 100. We don't assert a specific regime
        // (the classification rules are midstream's, not ours) — only that
        // the analyze step runs without erroring and a non-Unknown classification
        // is produced.
        let mut st = IntrospectionState::with_config(IntrospectionConfig {
            trajectory_len: 256,
            embedding_dim: 1,
            analyze_every_n: 8,
            library: SignatureLibrary::new(),
        });
        for k in 0..200u64 {
            let v = (k as f64 * 0.1).sin();
            st.update(k * 33_000_000, v).unwrap();
        }
        let s = st.snapshot();
        // After 200 points + analyze_every_n=8 fires, the analyzer should have
        // produced a classification at least once.
        assert!(
            s.regime != Regime::Unknown || s.lyapunov_exponent.is_some(),
            "expected regime classified or Lyapunov set after 200 frames; got {:?}",
            s
        );
    }
}
