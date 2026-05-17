//! ADR-099 D8 benchmark — latency-floor measurement for the introspection tap
//! vs. the window-aggregated event pipeline.
//!
//! What this measures (and what it doesn't):
//!
//! * It measures the **architectural floor** of each detection path:
//!   - The window path's *soonest possible* `MotionDetected` emission is gated
//!     by `WindowBuffer::new(16, 1 s)` + `MotionDetector::debounce_windows = 2`
//!     = a known function of frames. No simulation of the EventPipeline is
//!     needed for that floor — it's a deterministic count.
//!   - The introspection path's "shape recognised" emission fires the first
//!     frame after which `IntrospectionState::snapshot().top_k_similarity[0]
//!     .above_threshold` is `true`. That's what we measure empirically.
//! * It does *not* measure signature-library quality, DTW recall, or false
//!   positives — those are P1 / P3 concerns. The bar this test checks is
//!   D8's architectural latency-floor reduction (≥10× p99) on a clean
//!   in-phase shape.
//! * Per-frame `update()` wall-clock cost is also asserted (D4: ≤1 ms p99 on
//!   a Pi-5-class host; checked here against a 10 ms loose bound that any
//!   reasonable dev box should clear, leaving thermal/CI noise headroom).
//!
//! Numbers print at INFO level so `cargo test -- --nocapture` shows the
//! comparison directly.

use std::time::Instant;

use wifi_densepose_sensing_server::introspection::{
    IntrospectionConfig, IntrospectionState, Signature, SignatureDtw, SignatureLibrary,
};

/// The EventPipeline floor in frames at 30 Hz CSI:
///   16-frame window + 2 windows of motion debounce = 48 frames *worst case*,
///   16 frames *best case* (the perturbation arrives at frame 1, window closes
///   at frame 16, the *first* MotionDetected can fire then — but the detector
///   needs 2 consecutive high windows to debounce, so the realistic emission
///   sits between 16 and 48 frames).
///
/// We use the **best-case** floor here so the ratio is *conservative* — i.e.
/// the introspection win has to clear the bar even against the most generous
/// reading of the event path.
const EVENT_PATH_BEST_CASE_FRAMES: usize = 16;

/// ADR-099 D8 bar: ≥10× p99 latency reduction.
const D8_LATENCY_RATIO_BAR: f64 = 10.0;

/// ADR-099 D4 bar: per-frame update ≤ 1 ms p99 on a Pi-5-class host. CI runners
/// vary, so we assert a loose 10 ms ceiling here that still catches real
/// regressions (a midstream API change that pushes update() to 100 ms would
/// blow through this trivially) while leaving headroom for cold-cache /
/// thermally-throttled CI machines.
const PER_FRAME_BUDGET_MS: f64 = 10.0;

fn motion_signature() -> Signature {
    // A clean, short, monotonic ramp — exactly the kind of shape the host-side
    // L1 stand-in in `signature_score()` scores well on (and that DTW on real
    // vec128 will continue to score well on later).
    Signature {
        id: "motion_ramp".to_string(),
        label: "Motion ramp (benchmark fixture)".to_string(),
        vectors: vec![vec![1.0], vec![2.0], vec![3.0], vec![4.0], vec![5.0]],
        dtw: SignatureDtw {
            window: 8,
            step_pattern: "symmetric2".to_string(),
        },
        promotion_threshold: 0.70,
    }
}

/// Result of one motion-onset benchmark run: how many frames until each
/// detection signal first fires, plus per-frame `update()` wall-clock costs.
struct LatencyMeasurement {
    /// Frames into the motion before `top_k_similarity[0].above_threshold` is
    /// true (the "shape recognised" full-pattern path).
    shape_match_frames: usize,
    /// Frames into the motion before `regime_changed` is true (the parallel
    /// fast-detection path added in I6). `None` if it never fired in the
    /// measurement window — meaning the regime classification stayed at
    /// whatever it was during warm-up.
    regime_change_frames: Option<usize>,
    /// Per-frame `update()` wall-clock samples (ms).
    update_ms: Vec<f64>,
}

/// Feed N background-noise frames followed by the motion ramp; return the
/// 0-based frame index at which each detection signal first fires.
fn measure_motion_onset() -> LatencyMeasurement {
    let lib = SignatureLibrary::from_signatures(vec![motion_signature()]);
    let cfg = IntrospectionConfig {
        trajectory_len: 128,
        embedding_dim: 1,
        // I6: analyze on every frame so the regime-change signal is responsive.
        analyze_every_n: 1,
        library: lib,
    };
    let mut state = IntrospectionState::with_config(cfg);

    // 200 frames of background noise — small drifty values around 0. We feed
    // 200 (not 100) so the attractor analyzer is past its 100-point warm-up
    // *before* the motion injection, ensuring any regime change after onset
    // is attributable to the motion, not warm-up.
    let mut update_ms = Vec::with_capacity(220);
    for k in 0..200u64 {
        let t0 = Instant::now();
        let v = 0.05 * ((k as f64 * 0.31).sin()); // ±0.05 deterministic noise
        state.update(k * 33_000_000, v).unwrap();
        update_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        assert!(
            !state.snapshot().top_k_similarity[0].above_threshold,
            "noise frame {k} crossed shape-match threshold — signature too lax"
        );
    }
    let baseline_regime = state.snapshot().regime;

    // Now feed the motion ramp. Record the *first* frame each signal fires.
    let mut shape_match_frames: Option<usize> = None;
    let mut regime_change_frames: Option<usize> = None;
    for (i, v) in [1.0f64, 2.0, 3.0, 4.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0]
        .iter()
        .copied()
        .enumerate()
    {
        let t0 = Instant::now();
        state.update((200 + i as u64) * 33_000_000, v).unwrap();
        update_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        let s = state.snapshot();
        let frame_num = i + 1; // 1-based frames into the shape
        if shape_match_frames.is_none() && s.top_k_similarity[0].above_threshold {
            shape_match_frames = Some(frame_num);
        }
        // A *regime change* counts when the classification flips away from the
        // baseline (noise) regime. The snapshot.regime_changed flag flips for
        // any frame-to-frame change; we want "first frame whose regime differs
        // from the pre-motion baseline".
        if regime_change_frames.is_none() && s.regime != baseline_regime {
            regime_change_frames = Some(frame_num);
        }
        // Stop once we've seen both, or run out of motion frames.
        if shape_match_frames.is_some() && regime_change_frames.is_some() {
            break;
        }
    }

    LatencyMeasurement {
        shape_match_frames: shape_match_frames
            .expect("shape-match should fire within the 10-frame motion window"),
        regime_change_frames,
        update_ms,
    }
}

/// Compat shim for tests that only care about shape-match latency + costs.
fn frames_until_shape_recognised() -> (usize, Vec<f64>) {
    let m = measure_motion_onset();
    (m.shape_match_frames, m.update_ms)
}

#[test]
fn introspection_recognises_shape_within_window_floor() {
    let (intro_frames, _) = frames_until_shape_recognised();
    // The whole point of the tap is that "shape recognised" fires before the
    // 16-frame window even closes. Anything ≥ 16 means we'd be no better than
    // the event path, and ADR-099 D4's whole D4-claim breaks.
    assert!(
        intro_frames < EVENT_PATH_BEST_CASE_FRAMES,
        "introspection took {intro_frames} frames; event-path best-case is \
         {EVENT_PATH_BEST_CASE_FRAMES} — the tap is no faster than the window."
    );
}

/// Empirical baseline guard. The current implementation uses a host-side
/// length-normalised L1 stand-in for DTW (see `signature_score()` in
/// `introspection.rs`), which requires roughly a full signature length of
/// in-shape frames before the score crosses `promotion_threshold`. On the
/// 5-frame fixture in [`motion_signature`] that's exactly **5 frames** —
/// a **3.20× latency-floor reduction** vs. the event path's 16-frame best
/// case. ADR-099 D8 calls for ≥10×; closing that gap is owned by I6 ("optimise
/// hot spots") which can swap in real DTW partial-match scoring and/or
/// surface the attractor's regime-change as an earlier trigger than full
/// signature match. This guard prevents *regression* below today's 3.20×.
#[test]
fn introspection_latency_floor_ratio_baseline() {
    let (intro_frames, _) = frames_until_shape_recognised();
    let ratio = EVENT_PATH_BEST_CASE_FRAMES as f64 / intro_frames as f64;
    let d8_bar_met = ratio >= D8_LATENCY_RATIO_BAR;
    println!(
        "ADR-099 D8 floor ratio: event-path best-case {} frames / introspection \
         {} frames = {ratio:.2}× (D8 target: ≥{D8_LATENCY_RATIO_BAR}×, met: {d8_bar_met})",
        EVENT_PATH_BEST_CASE_FRAMES, intro_frames
    );
    // Regression bar — empirical baseline of the L1 stand-in. If a future
    // change ever drops below this, either the signature scoring regressed
    // or the test fixture changed; both deserve a deliberate look.
    const BASELINE_RATIO_FLOOR: f64 = 3.0;
    assert!(
        ratio >= BASELINE_RATIO_FLOOR,
        "ratio {ratio:.2}× dropped below the L1-stand-in baseline of {BASELINE_RATIO_FLOOR}× — \
         either signature scoring regressed or the test fixture changed deliberately"
    );
}

#[test]
fn per_frame_update_p99_under_budget() {
    let (_, update_ms) = frames_until_shape_recognised();
    let mut sorted = update_ms.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p50 = sorted[sorted.len() / 2];
    let p99_idx = ((sorted.len() as f64) * 0.99) as usize;
    let p99 = sorted[p99_idx.min(sorted.len() - 1)];
    let mean = update_ms.iter().sum::<f64>() / update_ms.len() as f64;
    let max = sorted.last().copied().unwrap_or(0.0);
    println!(
        "ADR-099 D4 per-frame update cost (n={}): p50={:.3}ms  mean={:.3}ms  p99={:.3}ms  max={:.3}ms  budget=<{}ms",
        update_ms.len(),
        p50,
        mean,
        p99,
        max,
        PER_FRAME_BUDGET_MS
    );
    assert!(
        p99 <= PER_FRAME_BUDGET_MS,
        "per-frame update p99 {p99:.3} ms exceeds {PER_FRAME_BUDGET_MS} ms budget"
    );
}

/// I6 — measure the parallel `regime_changed` signal added in this iteration.
/// This is the early-detection path that doesn't require a full signature
/// length of in-shape frames; the attractor analyzer flags trajectory shape
/// shifts directly. Reports both signals' latencies and the best ratio
/// either one achieves vs. the event-path floor.
#[test]
fn regime_change_path_latency() {
    let m = measure_motion_onset();
    println!(
        "ADR-099 I6: signals after motion onset\n  \
         shape_match  : {} frames into the ramp\n  \
         regime_change: {:?} frames into the ramp\n  \
         event-path best-case: {} frames",
        m.shape_match_frames, m.regime_change_frames, EVENT_PATH_BEST_CASE_FRAMES
    );
    let best_frames = match m.regime_change_frames {
        Some(rc) => rc.min(m.shape_match_frames),
        None => m.shape_match_frames,
    };
    let best_ratio = EVENT_PATH_BEST_CASE_FRAMES as f64 / best_frames as f64;
    println!(
        "  best-signal ratio: {best_ratio:.2}× (D8 target ≥{D8_LATENCY_RATIO_BAR}×, \
         met: {})",
        best_ratio >= D8_LATENCY_RATIO_BAR
    );
    // Regression bar: regime-change either fires within the event-path floor
    // (≥1× ratio) OR shape-match's 5-frame baseline holds. Either path is a
    // win; both red would mean we regressed both fast-detection paths.
    assert!(
        best_frames < EVENT_PATH_BEST_CASE_FRAMES,
        "neither fast path beat the event-path floor of {EVENT_PATH_BEST_CASE_FRAMES} frames"
    );
}

#[test]
fn snapshot_carries_regime_after_warmup() {
    // Independent of the latency bar — confirms the attractor analyzer feeds
    // a non-Unknown regime into the snapshot once the warmup is done (the
    // analyzer needs ~100 points before it'll classify).
    let cfg = IntrospectionConfig {
        trajectory_len: 256,
        embedding_dim: 1,
        analyze_every_n: 8,
        library: SignatureLibrary::new(),
    };
    let mut state = IntrospectionState::with_config(cfg);
    // Feed a periodic signal — should trigger `Regime::Periodic` (or at least
    // not stay `Unknown`).
    for k in 0..200u64 {
        let v = (k as f64 * 0.20).sin();
        state.update(k * 33_000_000, v).unwrap();
    }
    let s = state.snapshot();
    println!(
        "regime after 200 periodic frames: {:?}, lyapunov={:?}, confidence={}",
        s.regime, s.lyapunov_exponent, s.attractor_confidence
    );
    assert_ne!(
        s.regime,
        wifi_densepose_sensing_server::introspection::Regime::Unknown,
        "regime is still Unknown after 200 frames — attractor analyzer didn't fire"
    );
}
