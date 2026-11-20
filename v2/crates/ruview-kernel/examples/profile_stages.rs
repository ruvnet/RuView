//! Stage timing for the ADR-021 pipeline on SYNTHETIC frames (ADR-368 amendment).
//!
//! Reproducer: `cargo run --release -p ruview-kernel --example profile_stages`
//! Prints wall time per stage over 120 s of 20 Hz, 56-subcarrier synthetic CSI.
use std::time::Instant;
use wifi_densepose_vitals::{
    BreathingExtractor, CsiFrame, CsiVitalPreprocessor, HeartRateExtractor,
};

fn main() {
    let frames = ruview_kernel::synthesize(&ruview_kernel::SynthRequest {
        seconds: 120.0,
        ..Default::default()
    })
    .expect("synthetic frames");
    let n = 56;
    let mut pre = CsiVitalPreprocessor::new(n, 0.05);
    let mut breathing = BreathingExtractor::new(n, 20.0, 30.0);
    let mut heart = HeartRateExtractor::new(n, 20.0, 15.0);
    let weights = vec![1.0 / n as f64; n];
    let (mut t_pre, mut t_br, mut t_hr) = (0u128, 0u128, 0u128);
    for (i, f) in frames.iter().enumerate() {
        let frame = CsiFrame {
            amplitudes: f.amplitudes.clone(),
            phases: f.phases.clone(),
            n_subcarriers: n,
            sample_index: i as u64,
            sample_rate_hz: 20.0,
        };
        let t = Instant::now();
        let residuals = pre.process(&frame).expect("residuals");
        t_pre += t.elapsed().as_nanos();
        let t = Instant::now();
        breathing.extract(&residuals, &weights);
        t_br += t.elapsed().as_nanos();
        let t = Instant::now();
        heart.extract(&residuals, &frame.phases);
        t_hr += t.elapsed().as_nanos();
    }
    let t = Instant::now();
    let mut session =
        ruview_kernel::Session::new(ruview_kernel::SessionConfig::default()).expect("session");
    session.push(&frames).expect("push");
    println!(
        "SYNTHETIC frames={} preprocess_us={} breathing_us={} heart_us={} full_session_us={}",
        frames.len(),
        t_pre / 1000,
        t_br / 1000,
        t_hr / 1000,
        t.elapsed().as_micros()
    );
}
