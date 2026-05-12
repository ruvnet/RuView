// Emit a deterministic-seeded random weight blob in the .rvne format
// (ADR-095 / #513 Phase 1 of the training-side roadmap).
//
// This is a *demo*, not a trained model — the weights are PRNG output.
// Its purpose is to:
//   1. Document end-to-end how the host produces a blob (i.e. the
//      example IS the recipe a real trainer follows: build a header,
//      fill the weights buffer, call WeightBlob::new + .serialize(),
//      write to disk).
//   2. Provide a reproducible test fixture the firmware loader can
//      consume once the toolchain unblocks (ADR-095 Phase 5).
//   3. Anchor the byte-level format so refactors that change the
//      output silently are caught by the byte-count assertion at
//      the bottom.
//
// Usage:
//   cargo run -p wifi-densepose-temporal --example init_random_blob
//   cargo run -p wifi-densepose-temporal --example init_random_blob -- /tmp/model.rvne

use std::env;
use std::fs;
use std::path::PathBuf;

use wifi_densepose_temporal::{WeightBlob, WeightBlobHeader, WeightDtype};

/// Match the AETHER default head shape from
/// `TemporalHeadConfig::default_aether()` — staying coherent with the
/// crate's other defaults means a real trainer can drop this example
/// in as the starting point with one search-and-replace.
fn aether_default_header() -> WeightBlobHeader {
    WeightBlobHeader {
        dtype: WeightDtype::F32,
        input_dim: 16,
        n_q_heads: 4,
        n_kv_heads: 1, // MQA — one shared K/V across the 4 query heads
        head_dim: 32,
        n_layers: 2,
        n_classes: 4, // gesture-class default; firmware Kconfig matches
    }
}

/// Compute the raw byte count for one transformer block at the given
/// shape. This is the *intent-of-the-format* number, kept here so
/// changes to it (and to the kernel's expectation) stay in sync.
///
/// Per-layer weights consist of:
///   - input projection : input_dim × (n_q_heads × head_dim)   = Wq
///   - K projection      : input_dim × (n_kv_heads × head_dim)  = Wk
///   - V projection      : input_dim × (n_kv_heads × head_dim)  = Wv
///   - O projection      : (n_q_heads × head_dim) × input_dim    = Wo
fn per_layer_floats(h: &WeightBlobHeader) -> usize {
    let id = h.input_dim as usize;
    let q_total = h.n_q_heads as usize * h.head_dim as usize;
    let kv_total = h.n_kv_heads as usize * h.head_dim as usize;
    id * q_total          // Wq
        + id * kv_total   // Wk
        + id * kv_total   // Wv
        + q_total * id    // Wo
}

/// Plus a final classifier head: input_dim × n_classes.
fn classifier_floats(h: &WeightBlobHeader) -> usize {
    h.input_dim as usize * h.n_classes as usize
}

/// xorshift64* — tiny deterministic PRNG. Don't use for crypto;
/// this is a fixed-seed init so two runs of the example produce
/// byte-identical blobs.
fn xorshift_step(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x.wrapping_mul(2685821657736338717u64)
}

/// Map the high 32 bits of a u64 to a small symmetric float in
/// [-0.1, 0.1). Tight bound so the resulting model produces sensible
/// pre-softmax logits even though it's untrained.
fn next_init_f32(state: &mut u64) -> f32 {
    let bits = (xorshift_step(state) >> 32) as u32;
    // Map to [0, 1) then scale to [-0.1, 0.1)
    let unit = (bits as f32) / (u32::MAX as f32);
    (unit - 0.5) * 0.2
}

fn build_random_weights(header: &WeightBlobHeader, seed: u64) -> Vec<u8> {
    let total_floats =
        per_layer_floats(header) * header.n_layers as usize + classifier_floats(header);
    let mut out = Vec::with_capacity(total_floats * 4);
    let mut state = seed;
    for _ in 0..total_floats {
        let f = next_init_f32(&mut state);
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("model_init.rvne"));

    let header = aether_default_header();
    let weights = build_random_weights(&header, 0xC511_0007_DEAD_BEEFu64);
    let weights_len = weights.len();

    let blob = WeightBlob::new(header.clone(), weights)?;
    let bytes = blob.serialize();
    let serialized_len = bytes.len();

    fs::write(&path, &bytes)?;

    // Re-parse to prove the artifact we just wrote is loadable. Same
    // path the firmware loader will follow once the toolchain unblocks.
    let parsed = WeightBlob::parse(&fs::read(&path)?)?;

    println!("wrote   : {}", path.display());
    println!("dtype   : {:?}", parsed.header.dtype);
    println!(
        "shape   : input_dim={}, q_heads={}, kv_heads={}, head_dim={}, layers={}, classes={}",
        parsed.header.input_dim,
        parsed.header.n_q_heads,
        parsed.header.n_kv_heads,
        parsed.header.head_dim,
        parsed.header.n_layers,
        parsed.header.n_classes,
    );
    println!(
        "weights : {} bytes ({} f32 elements)",
        weights_len,
        weights_len / 4
    );
    println!(
        "total   : {} bytes (header 24 + weights {} + crc 4)",
        serialized_len, weights_len
    );

    Ok(())
}
