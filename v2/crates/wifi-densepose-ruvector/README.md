# wifi-densepose-ruvector

RuVector v2.0.4 integration layer for WiFi-DensePose — ADR-017.

This crate implements all 7 ADR-017 ruvector integration points for the
signal-processing pipeline and the Multi-AP Triage (MAT) disaster-detection
module.

## Integration Points

| File | ruvector crate | What it does | Benefit |
|------|----------------|--------------|---------|
| `signal/subcarrier` | ruvector-mincut | Graph min-cut partitions subcarriers into sensitive / insensitive groups based on body-motion correlation | Automatic subcarrier selection without hand-tuned thresholds |
| `signal/spectrogram` | ruvector-attn-mincut | Attention-guided min-cut gating suppresses noise frames, amplifies body-motion periods | Cleaner Doppler spectrogram input to DensePose head |
| `signal/bvp` | ruvector-attention | Scaled dot-product attention aggregates per-subcarrier STFT rows weighted by sensitivity | Robust body velocity profile even with missing subcarriers |
| `signal/fresnel` | ruvector-solver | Sparse regularized least-squares estimates TX-body (d1) and body-RX (d2) distances from multi-subcarrier Fresnel amplitude observations | Physics-grounded geometry without extra hardware |
| `mat/triangulation` | ruvector-solver | Neumann series solver linearises TDoA hyperbolic equations to estimate 2-D survivor position across multi-AP deployments | Sub-5 m accuracy from ≥3 TDoA pairs |
| `mat/breathing` | ruvector-temporal-tensor | Tiered quantized streaming buffer: hot ~10 frames at 8-bit, warm at 5–7-bit, cold at 3-bit | 13.4 MB raw → 3.4–6.7 MB for 56 sc × 60 s × 100 Hz |
| `mat/heartbeat` | ruvector-temporal-tensor | Per-frequency-bin tiered compressor for heartbeat spectrogram; `band_power()` extracts mean squared energy in any band | Independent tiering per bin; no cross-bin quantization coupling |

## Usage

Add to your `Cargo.toml` (workspace member or direct dependency):

```toml
[dependencies]
wifi-densepose-ruvector = { path = "../wifi-densepose-ruvector" }
```

### Signal processing

```rust
use wifi_densepose_ruvector::signal::{
    mincut_subcarrier_partition,
    gate_spectrogram,
    attention_weighted_bvp,
    solve_fresnel_geometry,
};

// Partition 56 subcarriers by body-motion sensitivity.
let (sensitive, insensitive) = mincut_subcarrier_partition(&sensitivity_scores);

// Gate a 32×64 Doppler spectrogram (mild).
let gated = gate_spectrogram(&flat_spectrogram, 32, 64, 0.1);

// Aggregate 56 STFT rows into one BVP vector.
let bvp = attention_weighted_bvp(&stft_rows, &sensitivity_scores, 128);

// Solve TX-body / body-RX geometry from 5-subcarrier Fresnel observations.
if let Some((d1, d2)) = solve_fresnel_geometry(&observations, d_total) {
    println!("d1={d1:.2} m, d2={d2:.2} m");
}
```

### MAT disaster detection

```rust
use wifi_densepose_ruvector::mat::{
    solve_triangulation,
    CompressedBreathingBuffer,
    CompressedHeartbeatSpectrogram,
};

// Localise a survivor from 4 TDoA measurements.
let pos = solve_triangulation(&tdoa_measurements, &ap_positions);

// Stream 6000 breathing frames at < 50% memory cost.
let mut buf = CompressedBreathingBuffer::new(56, zone_id);
for frame in frames {
    buf.push_frame(&frame);
}

// 128-bin heartbeat spectrogram with band-power extraction.
let mut hb = CompressedHeartbeatSpectrogram::new(128);
hb.push_column(&freq_column);
let cardiac_power = hb.band_power(10, 30); // ~0.8–2.0 Hz range
```

## Memory Reduction

Breathing buffer for 56 subcarriers × 60 s × 100 Hz:

| Tier | Bits/value | Size |
|------|-----------|------|
| Raw f32 | 32 | 13.4 MB |
| Hot (8-bit) | 8 | 3.4 MB |
| Mixed hot/warm/cold | 3–8 | 3.4–6.7 MB |
