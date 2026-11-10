//! Synthetic CSI frame generator for offline demos and development.
//!
//! Complies with ELMAS.md: pure deterministic generation, <= 350 lines.

use crate::ingest::Esp32Frame;

/// Generate a simulated CSI frame with realistic synthetic multipath ripple.
pub fn generate_simulated_frame(tick: u64) -> Esp32Frame {
    let t = tick as f64 * 0.1;
    let n_sub = 56usize;
    let mut amplitudes = Vec::with_capacity(n_sub);
    let mut phases = Vec::with_capacity(n_sub);

    for i in 0..n_sub {
        let base = 15.0 + 5.0 * (i as f64 * 0.1 + t * 0.3).sin();
        let noise = (i as f64 * 7.3 + t * 13.7).sin() * 2.0;
        amplitudes.push((base + noise).max(0.1));
        phases.push((i as f64 * 0.2 + t * 0.5).sin() * std::f64::consts::PI);
    }

    Esp32Frame {
        magic: 0xC511_0001,
        node_id: 1,
        n_antennas: 1,
        n_subcarriers: n_sub as u16,
        freq_mhz: 2437,
        sequence: tick as u32,
        rssi: (-40.0 + 5.0 * (t * 0.2).sin()) as i8,
        noise_floor: -90,
        ppdu_type: wifi_densepose_hardware::PpduType::HtLegacy,
        adr018_flags: wifi_densepose_hardware::Adr018Flags::default(),
        amplitudes,
        phases,
    }
}

