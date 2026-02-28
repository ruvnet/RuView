//! QR Cognitive Seed — Encode to QR Code
//!
//! Builds an RVQS seed payload and renders it as an SVG QR code.
//!
//! Run: cargo run --example qr_seed_encode -p rvf-runtime --features qr

use rvf_runtime::qr_encode::{EcLevel, QrEncoder};
use rvf_runtime::qr_seed::SeedBuilder;

fn main() {
    println!("=== QR Seed Encoder ===\n");

    // Build a minimal RVQS seed payload.
    let builder = SeedBuilder::new(
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
        384,     // dimension
        100_000, // total vectors
    );

    let (payload, header) = builder.build().expect("seed build");

    println!("Seed payload: {} bytes", payload.len());
    println!("  Magic:    0x{:08X}", header.seed_magic);
    println!("  Version:  {}", header.seed_version);
    println!("  Vectors:  {}", header.total_vector_count);
    println!("  Dim:      {}", header.dimension);
    println!();

    // Encode as QR code. The 64-byte header fits easily in Version 2 with EC-M.
    let code = QrEncoder::encode(&payload, EcLevel::M).expect("QR encode");

    println!("QR Code:");
    println!("  Version:  {}", code.version);
    println!("  Size:     {}x{} modules", code.size, code.size);
    println!();

    // Render as ASCII for terminal display.
    let ascii = QrEncoder::to_ascii(&code);
    println!("{ascii}");
    println!();

    // Render as SVG.
    let svg = QrEncoder::to_svg(&code);
    println!("SVG output: {} bytes", svg.len());
    println!("  Starts with: {}", &svg[..60]);
    println!();

    println!("=== Done ===");
}
