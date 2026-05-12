//! End-to-end test: write a deterministic-seeded weight blob to disk,
//! read it back, parse it. Mirrors what the host-side training tool
//! does (training run finishes → emit .rvne) and what the firmware
//! loader will do once the toolchain unblocks (boot → mmap NVS or
//! EMBED_FILES blob → parse → run kernel).
//!
//! Sized realistically (~26 KB for the AETHER default shape) so the
//! perf and CRC paths see a meaningful payload.

use std::fs;

use wifi_densepose_temporal::{WeightBlob, WeightBlobHeader, WeightDtype};

fn aether_default_header() -> WeightBlobHeader {
    WeightBlobHeader {
        dtype: WeightDtype::F32,
        input_dim: 16,
        n_q_heads: 4,
        n_kv_heads: 1,
        head_dim: 32,
        n_layers: 2,
        n_classes: 4,
    }
}

fn xorshift_step(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x.wrapping_mul(2685821657736338717u64)
}

fn deterministic_weights(byte_len: usize, seed: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(byte_len);
    let mut state = seed;
    while out.len() < byte_len {
        let bits = xorshift_step(&mut state) >> 32;
        let unit = (bits as u32 as f32) / (u32::MAX as f32);
        let f = (unit - 0.5) * 0.2;
        out.extend_from_slice(&f.to_le_bytes());
    }
    out.truncate(byte_len);
    out
}

#[test]
fn realistic_blob_roundtrips_through_filesystem() {
    // AETHER default + 2 layers + classifier head: enough to exercise
    // a non-trivial weights region without making the test slow.
    let header = aether_default_header();

    // Per-layer floats: input_dim*(q_heads*head_dim) for Wq, twice
    // input_dim*(kv_heads*head_dim) for Wk and Wv, q_heads*head_dim*input_dim
    // for Wo. Plus classifier head input_dim*n_classes.
    let per_layer = (header.input_dim as usize)
        * (header.n_q_heads as usize * header.head_dim as usize)
        + 2 * (header.input_dim as usize)
            * (header.n_kv_heads as usize * header.head_dim as usize)
        + (header.n_q_heads as usize * header.head_dim as usize)
            * (header.input_dim as usize);
    let total_floats = per_layer * header.n_layers as usize
        + header.input_dim as usize * header.n_classes as usize;
    let weights_bytes = total_floats * 4;
    assert!(weights_bytes > 25_000);

    let weights = deterministic_weights(weights_bytes, 0xC511_0007_DEAD_BEEFu64);
    let blob = WeightBlob::new(header, weights).expect("construct");
    let serialized = blob.serialize();

    // Filesystem leg — the realistic firmware loader path mmap or
    // streaming-reads from NVS / EMBED_FILES. We use a temp file
    // per platform; on Windows std::env::temp_dir() works fine.
    let mut tmp = std::env::temp_dir();
    tmp.push("wifi-densepose-temporal-e2e.rvne");
    fs::write(&tmp, &serialized).expect("write");
    let read_back = fs::read(&tmp).expect("read");
    assert_eq!(read_back, serialized, "filesystem corrupted bytes");

    let parsed = WeightBlob::parse(&read_back).expect("parse");
    assert_eq!(parsed.header.input_dim, 16);
    assert_eq!(parsed.header.n_q_heads, 4);
    assert_eq!(parsed.header.n_kv_heads, 1);
    assert_eq!(parsed.header.head_dim, 32);
    assert_eq!(parsed.header.n_layers, 2);
    assert_eq!(parsed.header.n_classes, 4);
    assert_eq!(parsed.weights.len(), weights_bytes);

    // Cleanup — best-effort, don't fail the test on Windows file lock.
    let _ = fs::remove_file(&tmp);
}

#[test]
fn deterministic_seed_produces_byte_identical_blobs() {
    // The training script needs reproducibility — given the same
    // config and seed, two runs must produce byte-identical output.
    // This is what makes a witness-bundle (ADR-028) over the trained
    // weights meaningful.
    let header = aether_default_header();
    let bytes = 4096;

    let w1 = deterministic_weights(bytes, 0x1234u64);
    let w2 = deterministic_weights(bytes, 0x1234u64);
    assert_eq!(w1, w2, "PRNG not deterministic at fixed seed");

    let blob1 = WeightBlob::new(header.clone(), w1).expect("ok");
    let blob2 = WeightBlob::new(header, w2).expect("ok");
    assert_eq!(
        blob1.serialize(),
        blob2.serialize(),
        "serialization not deterministic"
    );
}
