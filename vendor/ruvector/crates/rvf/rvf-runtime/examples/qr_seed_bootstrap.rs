//! QR Cognitive Seed — "A World Inside a World"
//!
//! Demonstrates the full zero-dependency pipeline:
//!
//! 1. Compress a WASM microkernel using built-in LZ
//! 2. Build an RVQS payload with hosts, layers, and HMAC-SHA256 signing
//! 3. Verify it fits in a single QR code (<=2,953 bytes)
//! 4. Parse the seed back and verify signature + content hash
//! 5. Decompress the microkernel using built-in LZ
//! 6. Simulate progressive bootstrap from seed to full intelligence
//!
//! Zero external dependencies. Real cryptography.
//!
//! Run: cargo run --example qr_seed_bootstrap -p rvf-runtime

use rvf_runtime::qr_seed::{make_host_entry, BootstrapProgress, ParsedSeed, SeedBuilder};
use rvf_runtime::seed_crypto;
use rvf_types::qr_seed::*;

/// HMAC-SHA256 signing key (in production, load from secure storage).
const SIGNING_KEY: &[u8] = b"example-signing-key-for-demo-ok!";

fn main() {
    println!("=== QR Cognitive Seed: A World Inside a World ===\n");

    // --- Phase 0: Build the seed ---
    println!("[Phase 0] Building RVQS seed with real crypto...");

    // Simulated WASM microkernel (will be compressed by built-in LZ).
    let raw_wasm = fake_wasm(5500);
    println!("  Raw WASM:      {} bytes", raw_wasm.len());

    // Primary CDN host.
    let primary_host = make_host_entry(
        "https://cdn.ruvector.ai/rvf/brain-v1.rvf",
        0, // highest priority
        1, // region: US-East
        [0xAA; 16],
    )
    .expect("primary host");

    // Fallback host.
    let fallback_host = make_host_entry(
        "https://mirror.ruvector.ai/rvf/brain-v1.rvf",
        1, // lower priority
        2, // region: EU-West
        [0xBB; 16],
    )
    .expect("fallback host");

    // Progressive layers with real content hashes.
    let layer_data_0 = vec![0x42u8; 4_096];
    let layer_data_1 = vec![0x43u8; 51_200];
    let layer_data_2 = vec![0x44u8; 204_800];
    let layer_data_3 = vec![0x45u8; 102_400];
    let layer_data_4 = vec![0x46u8; 512_000];

    let layers = vec![
        (
            LayerEntry {
                offset: 0,
                size: 4_096,
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
                offset: 4_096,
                size: 51_200,
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
                offset: 55_296,
                size: 204_800,
                content_hash: seed_crypto::layer_content_hash(&layer_data_2),
                layer_id: layer_id::HNSW_LAYER_A,
                priority: 2,
                required: 0,
                _pad: 0,
            },
            layer_data_2,
        ),
        (
            LayerEntry {
                offset: 260_096,
                size: 102_400,
                content_hash: seed_crypto::layer_content_hash(&layer_data_3),
                layer_id: layer_id::QUANT_DICT,
                priority: 3,
                required: 0,
                _pad: 0,
            },
            layer_data_3,
        ),
        (
            LayerEntry {
                offset: 362_496,
                size: 512_000,
                content_hash: seed_crypto::layer_content_hash(&layer_data_4),
                layer_id: layer_id::HNSW_LAYER_B,
                priority: 4,
                required: 0,
                _pad: 0,
            },
            layer_data_4,
        ),
    ];

    // Build with built-in LZ compression and HMAC-SHA256 signing.
    let mut builder = SeedBuilder::new(
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
        384,
        100_000,
    )
    .compress_microkernel(&raw_wasm)
    .add_host(primary_host)
    .add_host(fallback_host);

    builder.base_dtype = 1; // F16
    builder.profile_id = 2; // Hot profile
    builder.content_hash_full = Some(seed_crypto::full_content_hash(b"full rvf data"));
    builder.total_file_size = Some(874_496);
    builder.stream_upgrade = true;

    for (layer, _data) in &layers {
        builder = builder.add_layer(*layer);
    }

    let (payload, header) = builder.build_and_sign(SIGNING_KEY).expect("seed build");

    println!("  Seed magic:    0x{:08X} (\"RVQS\")", header.seed_magic);
    println!("  Version:       {}", header.seed_version);
    println!("  Flags:         0x{:04X}", header.flags);
    println!("  File ID:       {:02X?}", header.file_id);
    println!("  Vectors:       {}", header.total_vector_count);
    println!("  Dimension:     {}", header.dimension);
    println!(
        "  Microkernel:   {} bytes (LZ compressed)",
        header.microkernel_size
    );
    println!("  Manifest:      {} bytes", header.download_manifest_size);
    println!(
        "  Signature:     {} bytes (HMAC-SHA256, algo={})",
        header.sig_length, header.sig_algo
    );
    println!("  Content hash:  {:02x?}", header.content_hash);
    println!("  Total size:    {} bytes", header.total_seed_size);
    println!("  QR capacity:   {} bytes", QR_MAX_BYTES);
    println!(
        "  Fits in QR:    {} ({} bytes headroom)",
        header.fits_in_qr(),
        QR_MAX_BYTES as u32 - header.total_seed_size
    );
    println!();

    // --- Phase 1: Parse, verify, decompress ---
    println!("[Phase 1] Parsing and verifying seed...");

    let parsed = ParsedSeed::parse(&payload).expect("parse seed");

    println!("  Header valid:  {}", parsed.header.is_valid_magic());
    println!(
        "  Microkernel:   {} ({} bytes compressed)",
        if parsed.microkernel.is_some() {
            "present"
        } else {
            "absent"
        },
        parsed.microkernel.map(|m| m.len()).unwrap_or(0)
    );
    println!(
        "  Manifest:      {} ({} bytes)",
        if parsed.manifest_bytes.is_some() {
            "present"
        } else {
            "absent"
        },
        parsed.manifest_bytes.map(|m| m.len()).unwrap_or(0)
    );
    println!(
        "  Signature:     {} ({} bytes)",
        if parsed.signature.is_some() {
            "present"
        } else {
            "absent"
        },
        parsed.signature.map(|s| s.len()).unwrap_or(0)
    );

    // Full verification: magic + content hash + HMAC-SHA256 signature.
    parsed
        .verify_all(SIGNING_KEY, &payload)
        .expect("verify_all");
    println!("  verify_all:    PASSED (magic + hash + HMAC-SHA256)");

    // Individual checks.
    assert!(parsed.verify_content_hash());
    println!("  content_hash:  PASSED");

    parsed
        .verify_signature(SIGNING_KEY, &payload)
        .expect("sig verify");
    println!("  signature:     PASSED (HMAC-SHA256)");

    // Wrong key must fail.
    assert!(parsed
        .verify_signature(b"wrong-key-should-fail-immediatel", &payload)
        .is_err());
    println!("  wrong key:     REJECTED (as expected)");

    // Decompress microkernel using built-in LZ.
    let decompressed = parsed.decompress_microkernel().expect("decompress");
    assert_eq!(decompressed, raw_wasm);
    let ratio = raw_wasm.len() as f64 / parsed.microkernel.unwrap().len() as f64;
    println!(
        "  Decompressed:  {} bytes (ratio: {:.2}x)",
        decompressed.len(),
        ratio
    );
    println!();

    // --- Phase 2: Parse download manifest ---
    println!("[Phase 2] Parsing download manifest...");

    let manifest = parsed.parse_manifest().expect("parse manifest");

    println!("  Hosts: {}", manifest.hosts.len());
    for (i, host) in manifest.hosts.iter().enumerate() {
        let label = if i == 0 { "Primary" } else { "Fallback" };
        println!(
            "    [{label}] {} (priority={}, region={})",
            host.url_str().unwrap_or("<invalid>"),
            host.priority,
            host.region
        );
    }

    println!(
        "  Content hash:  {:?}",
        manifest.content_hash.map(|h| hex_short(&h))
    );
    println!(
        "  Total size:    {} bytes",
        manifest.total_file_size.unwrap_or(0)
    );
    println!("  Layers: {}", manifest.layers.len());
    for layer in &manifest.layers {
        let name = layer_name(layer.layer_id);
        println!(
            "    [{:>2}] {:<20} offset={:<8} size={:<8} required={} hash={}",
            layer.priority,
            name,
            layer.offset,
            layer.size,
            if layer.required == 1 { "yes" } else { "no " },
            hex_short(&layer.content_hash)
        );
    }

    // Verify layer hashes.
    println!();
    println!("  Layer hash verification:");
    for (layer, data) in &layers {
        let ok = seed_crypto::verify_layer(&layer.content_hash, data);
        println!(
            "    {} {:<20} {}",
            if ok { "PASS" } else { "FAIL" },
            layer_name(layer.layer_id),
            hex_short(&layer.content_hash)
        );
    }
    println!();

    // --- Phase 3: Simulate progressive bootstrap ---
    println!("[Phase 3] Simulating progressive bootstrap...\n");

    let mut progress = BootstrapProgress::new(&manifest);

    println!(
        "  Boot phase: {} | query_ready: {} | recall: {:.0}%",
        phase_name(progress.phase),
        progress.query_ready,
        progress.estimated_recall * 100.0
    );

    for (layer, _data) in &layers {
        progress.record_layer(layer);
        println!(
            "  Downloaded {:<20} | phase: {} | query_ready: {} | recall: {:.0}% | progress: {:.1}%",
            layer_name(layer.layer_id),
            phase_name(progress.phase),
            progress.query_ready,
            progress.estimated_recall * 100.0,
            progress.progress_fraction() * 100.0
        );
    }

    println!("\n=== Seed bootstrapped to full intelligence ===");
    println!(
        "  The AI that lived in printed ink now spans {} bytes.",
        manifest.total_file_size.unwrap_or(0)
    );
    println!("  Zero dependencies. Real cryptography. Scan. Boot. Intelligence.");
}

/// Build a realistic fake WASM module (compressible patterns).
fn fake_wasm(size: usize) -> Vec<u8> {
    let mut wasm = Vec::with_capacity(size);
    wasm.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00]);
    while wasm.len() < size {
        wasm.extend_from_slice(&[0x01, 0x06, 0x01, 0x60, 0x01, 0x7F, 0x01, 0x7F]);
    }
    wasm.truncate(size);
    wasm
}

fn phase_name(phase: u8) -> &'static str {
    match phase {
        0 => "Parse  ",
        1 => "Stream ",
        2 => "Full   ",
        _ => "Unknown",
    }
}

fn layer_name(id: u8) -> &'static str {
    match id {
        0 => "Level 0 manifest",
        1 => "Hot cache",
        2 => "HNSW Layer A",
        3 => "Quant dictionaries",
        4 => "HNSW Layer B",
        5 => "Full vectors",
        6 => "HNSW Layer C",
        _ => "Unknown",
    }
}

fn hex_short(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(4)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
        + ".."
}
