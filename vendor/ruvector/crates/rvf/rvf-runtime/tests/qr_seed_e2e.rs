//! End-to-end integration tests for the QR Cognitive Seed pipeline.
//!
//! Tests the full zero-dependency chain:
//! Build → Compress → Hash → Sign → Serialize → Parse → Verify → Decompress

use rvf_runtime::compress;
use rvf_runtime::qr_seed::*;
use rvf_runtime::seed_crypto;
use rvf_types::qr_seed::*;

const SIGNING_KEY: &[u8] = b"test-secret-key-for-hmac-sha256!";

/// Build a realistic fake WASM module.
fn fake_wasm(size: usize) -> Vec<u8> {
    let mut wasm = Vec::with_capacity(size);
    // WASM magic + version.
    wasm.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00]);
    // Repeated section patterns (compressible).
    while wasm.len() < size {
        wasm.extend_from_slice(&[0x01, 0x06, 0x01, 0x60, 0x01, 0x7F, 0x01, 0x7F]);
    }
    wasm.truncate(size);
    wasm
}

fn default_layers_with_real_hashes() -> Vec<(LayerEntry, Vec<u8>)> {
    let layer_data_0 = vec![0x42u8; 4096];
    let layer_data_1 = vec![0x43u8; 51200];
    let layer_data_2 = vec![0x44u8; 204800];

    vec![
        (
            LayerEntry {
                offset: 0,
                size: 4096,
                content_hash: seed_crypto::layer_content_hash(&layer_data_0),
                layer_id: layer_id::LEVEL0,
                priority: 0,
                required: 1,
                _pad: 0,
            },
            layer_data_0,
        ),
        (
            LayerEntry {
                offset: 4096,
                size: 51200,
                content_hash: seed_crypto::layer_content_hash(&layer_data_1),
                layer_id: layer_id::HOT_CACHE,
                priority: 1,
                required: 1,
                _pad: 0,
            },
            layer_data_1,
        ),
        (
            LayerEntry {
                offset: 55296,
                size: 204800,
                content_hash: seed_crypto::layer_content_hash(&layer_data_2),
                layer_id: layer_id::HNSW_LAYER_A,
                priority: 2,
                required: 0,
                _pad: 0,
            },
            layer_data_2,
        ),
    ]
}

#[test]
fn full_round_trip_with_real_crypto() {
    // 1. Create and compress a fake WASM microkernel.
    let wasm = fake_wasm(5500);
    let compressed = compress::compress(&wasm);
    assert!(
        compressed.len() < wasm.len(),
        "compression failed: {} >= {}",
        compressed.len(),
        wasm.len()
    );

    // 2. Build seed with real signing.
    let host = make_host_entry("https://cdn.example.com/brain.rvf", 0, 1, [0xAA; 16]).unwrap();
    let layers = default_layers_with_real_hashes();

    let mut builder = SeedBuilder::new([0x01; 8], 384, 100_000)
        .with_microkernel(compressed.clone())
        .add_host(host);

    builder.content_hash_full = Some(seed_crypto::full_content_hash(b"full rvf data"));
    builder.total_file_size = Some(260_096);

    for (layer, _data) in &layers {
        builder = builder.add_layer(*layer);
    }

    let (payload, header) = builder.build_and_sign(SIGNING_KEY).unwrap();

    // 3. Verify QR capacity.
    assert!(header.fits_in_qr());
    assert!(payload.len() <= QR_MAX_BYTES);

    // 4. Parse it back.
    let parsed = ParsedSeed::parse(&payload).unwrap();
    assert_eq!(parsed.header.seed_magic, SEED_MAGIC);
    assert!(parsed.header.is_signed());
    assert_eq!(parsed.header.sig_algo, seed_crypto::SIG_ALGO_HMAC_SHA256);

    // 5. Full verification (magic + content hash + signature).
    parsed.verify_all(SIGNING_KEY, &payload).unwrap();

    // 6. Individual verification steps.
    assert!(parsed.verify_content_hash());
    parsed.verify_signature(SIGNING_KEY, &payload).unwrap();

    // 7. Wrong key must fail.
    assert!(parsed
        .verify_signature(b"wrong-key-must-fail-immediately!", &payload)
        .is_err());

    // 8. Decompress microkernel.
    let decompressed = parsed.decompress_microkernel().unwrap();
    assert_eq!(decompressed, wasm);

    // 9. Parse manifest and verify layer hashes.
    let manifest = parsed.parse_manifest().unwrap();
    assert_eq!(manifest.hosts.len(), 1);
    assert_eq!(manifest.layers.len(), 3);

    for (layer, data) in &layers {
        assert!(
            seed_crypto::verify_layer(&layer.content_hash, data),
            "layer {} hash mismatch",
            layer.layer_id
        );
    }

    // 10. Tampered layer data must fail.
    let tampered = vec![0xFF; 4096];
    assert!(!seed_crypto::verify_layer(
        &layers[0].0.content_hash,
        &tampered
    ));
}

#[test]
fn compress_microkernel_method() {
    let wasm = fake_wasm(5500);

    let builder = SeedBuilder::new([0x02; 8], 128, 1000).compress_microkernel(&wasm);

    let (payload, header) = builder.build_and_sign(SIGNING_KEY).unwrap();
    assert!(header.has_microkernel());
    assert!(header.fits_in_qr());

    // Parse and decompress.
    let parsed = ParsedSeed::parse(&payload).unwrap();
    let decompressed = parsed.decompress_microkernel().unwrap();
    assert_eq!(decompressed, wasm);
}

#[test]
fn unsigned_build_still_works() {
    // The original build() method must still work for backward compatibility.
    let builder = SeedBuilder::new([0x03; 8], 128, 1000).with_content_hash([0xAA; 8]);
    let (payload, header) = builder.build().unwrap();
    assert!(!header.is_signed());
    assert_eq!(header.content_hash, [0xAA; 8]);

    let parsed = ParsedSeed::parse(&payload).unwrap();
    assert!(parsed.signature.is_none());
}

#[test]
fn tampered_payload_fails_signature() {
    let builder = SeedBuilder::new([0x04; 8], 128, 1000).compress_microkernel(&fake_wasm(2000));
    let (mut payload, _) = builder.build_and_sign(SIGNING_KEY).unwrap();

    // Tamper with a byte in the microkernel area.
    payload[SEED_HEADER_SIZE + 10] ^= 0xFF;

    let parsed = ParsedSeed::parse(&payload).unwrap();
    assert!(parsed.verify_signature(SIGNING_KEY, &payload).is_err());
}

#[test]
fn tampered_payload_fails_content_hash() {
    let builder = SeedBuilder::new([0x05; 8], 128, 1000).compress_microkernel(&fake_wasm(2000));
    let (mut payload, _) = builder.build_and_sign(SIGNING_KEY).unwrap();

    // Tamper with a byte in the microkernel.
    payload[SEED_HEADER_SIZE + 10] ^= 0xFF;

    let parsed = ParsedSeed::parse(&payload).unwrap();
    assert!(!parsed.verify_content_hash());
}

#[test]
fn verify_all_catches_bad_signature() {
    let builder = SeedBuilder::new([0x06; 8], 128, 1000).compress_microkernel(&fake_wasm(2000));
    let (payload, _) = builder.build_and_sign(SIGNING_KEY).unwrap();

    let parsed = ParsedSeed::parse(&payload).unwrap();
    let err = parsed.verify_all(b"wrong-key-should-definitely-fail", &payload);
    assert!(err.is_err());
}

#[test]
fn bootstrap_progress_with_real_layers() {
    let layers = default_layers_with_real_hashes();
    let manifest = DownloadManifest {
        hosts: vec![],
        content_hash: None,
        total_file_size: Some(260_096),
        layers: layers.iter().map(|(l, _)| *l).collect(),
        session_token: None,
        token_ttl: None,
        cert_pin: None,
    };

    let mut progress = BootstrapProgress::new(&manifest);
    assert!(!progress.query_ready);
    assert_eq!(progress.phase, 0);

    // Download layers progressively.
    progress.record_layer(&layers[0].0);
    assert!(!progress.query_ready); // Level0 alone isn't query-ready.

    progress.record_layer(&layers[1].0);
    assert!(progress.query_ready);
    assert_eq!(progress.phase, 1);
    assert!((progress.estimated_recall - 0.50).abs() < f32::EPSILON);

    progress.record_layer(&layers[2].0);
    assert!((progress.estimated_recall - 0.70).abs() < f32::EPSILON);
    assert_eq!(progress.phase, 2); // All layers downloaded.
}

#[test]
fn compression_ratio_for_wasm() {
    let wasm = fake_wasm(5500);
    let compressed = compress::compress(&wasm);
    let ratio = wasm.len() as f64 / compressed.len() as f64;
    assert!(
        ratio > 1.2,
        "expected compression ratio > 1.2x, got {:.2}x",
        ratio
    );
}

#[test]
fn sha256_produces_correct_hash() {
    // Verify the built-in SHA-256 against a known vector.
    let hash = rvf_types::sha256::sha256(b"abc");
    // NIST test vector for SHA-256("abc").
    assert_eq!(hash[0], 0xba);
    assert_eq!(hash[1], 0x78);
    assert_eq!(hash[2], 0x16);
    assert_eq!(hash[3], 0xbf);
}

#[test]
fn hmac_sha256_produces_correct_mac() {
    // RFC 4231 Test Case 2.
    let key = b"Jefe";
    let data = b"what do ya want for nothing?";
    let mac = rvf_types::sha256::hmac_sha256(key, data);
    assert_eq!(mac[0], 0x5b);
    assert_eq!(mac[1], 0xdc);
    assert_eq!(mac[2], 0xc1);
    assert_eq!(mac[3], 0x46);
}

#[test]
fn maximal_seed_size() {
    // Build the largest possible seed that still fits in QR.
    let wasm = fake_wasm(5500);
    let compressed = compress::compress(&wasm);

    let host1 = make_host_entry("https://cdn.example.com/brain.rvf", 0, 1, [0xAA; 16]).unwrap();
    let host2 = make_host_entry("https://mirror.example.com/brain.rvf", 1, 2, [0xBB; 16]).unwrap();

    let layers = default_layers_with_real_hashes();
    let mut builder = SeedBuilder::new([0x07; 8], 384, 100_000)
        .with_microkernel(compressed)
        .add_host(host1)
        .add_host(host2);

    builder.content_hash_full = Some([0xDD; 32]);
    builder.total_file_size = Some(10_000_000);
    builder.session_token = Some([0xEE; 16]);
    builder.token_ttl = Some(3600);
    builder.cert_pin = Some([0xFF; 32]);
    builder.stream_upgrade = true;

    for (layer, _) in &layers {
        builder = builder.add_layer(*layer);
    }

    let (payload, header) = builder.build_and_sign(SIGNING_KEY).unwrap();
    assert!(
        header.fits_in_qr(),
        "seed size {} exceeds QR max {}",
        header.total_seed_size,
        QR_MAX_BYTES
    );
    assert!(payload.len() <= QR_MAX_BYTES);

    // Full round-trip verification.
    let parsed = ParsedSeed::parse(&payload).unwrap();
    parsed.verify_all(SIGNING_KEY, &payload).unwrap();

    let manifest = parsed.parse_manifest().unwrap();
    assert_eq!(manifest.hosts.len(), 2);
    assert_eq!(manifest.layers.len(), 3);
    assert_eq!(manifest.session_token, Some([0xEE; 16]));
    assert_eq!(manifest.token_ttl, Some(3600));
    assert_eq!(manifest.cert_pin, Some([0xFF; 32]));
}
