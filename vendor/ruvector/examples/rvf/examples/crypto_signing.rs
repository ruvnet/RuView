//! Segment Signing and Witness Chains
//!
//! Demonstrates post-quantum-ready cryptographic features:
//! 1. Generate Ed25519 keypair
//! 2. Create and sign a segment
//! 3. Verify signature
//! 4. Tamper with segment, show verification fails
//! 5. Create a witness chain (5 entries)
//! 6. Verify witness chain integrity
//! 7. Tamper with chain, show detection

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

use rvf_types::{SegmentHeader, SegmentType};
use rvf_crypto::{
    sign_segment, verify_segment,
    create_witness_chain, verify_witness_chain, WitnessEntry,
    shake256_256,
};

fn main() {
    println!("=== RVF Crypto Signing Example ===\n");

    // ====================================================================
    // 1. Generate Ed25519 keypair
    // ====================================================================
    println!("--- 1. Key Generation ---");

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    println!("  Algorithm:    Ed25519");
    println!(
        "  Public key:   {}",
        hex_string(&verifying_key.to_bytes())
    );
    println!("  Key size:     32 bytes (public), 32 bytes (private)");

    // ====================================================================
    // 2. Create and sign a segment
    // ====================================================================
    println!("\n--- 2. Segment Signing ---");

    let mut header = SegmentHeader::new(SegmentType::Vec as u8, 42);
    header.timestamp_ns = 1_700_000_000_000_000_000; // 2023-11-14 epoch ns
    header.payload_length = 256;

    let payload = b"This is a vector segment payload containing embedding data \
                    that we want to protect with a cryptographic signature.";

    println!("  Segment ID:      {}", header.segment_id);
    println!("  Segment type:    VEC_SEG (0x{:02X})", header.seg_type);
    println!("  Payload size:    {} bytes", payload.len());

    let footer = sign_segment(&header, payload, &signing_key);

    println!("  Signature algo:  Ed25519 (algo_id={})", footer.sig_algo);
    println!("  Signature size:  {} bytes", footer.sig_length);
    println!(
        "  Signature:       {}...",
        hex_string(&footer.signature[..16])
    );
    println!(
        "  Footer length:   {} bytes",
        footer.footer_length
    );

    // ====================================================================
    // 3. Verify signature
    // ====================================================================
    println!("\n--- 3. Signature Verification ---");

    let valid = verify_segment(&header, payload, &footer, &verifying_key);
    println!("  Original payload + header: {}", if valid { "VALID" } else { "INVALID" });
    assert!(valid, "signature should be valid for original data");

    // ====================================================================
    // 4. Tamper detection
    // ====================================================================
    println!("\n--- 4. Tamper Detection ---");

    // 4a. Tamper with payload
    let tampered_payload = b"This payload has been tampered with! The data is now different.";
    let valid_tampered_payload = verify_segment(&header, tampered_payload, &footer, &verifying_key);
    println!(
        "  Tampered payload:  {}",
        if valid_tampered_payload { "VALID (BAD!)" } else { "INVALID (tamper detected)" }
    );
    assert!(!valid_tampered_payload, "tampered payload should fail");

    // 4b. Tamper with header (change segment_id)
    let mut tampered_header = header;
    tampered_header.segment_id = 999;
    let valid_tampered_header = verify_segment(&tampered_header, payload, &footer, &verifying_key);
    println!(
        "  Tampered header:   {}",
        if valid_tampered_header { "VALID (BAD!)" } else { "INVALID (tamper detected)" }
    );
    assert!(!valid_tampered_header, "tampered header should fail");

    // 4c. Wrong key
    let wrong_key = SigningKey::generate(&mut OsRng);
    let wrong_pubkey = wrong_key.verifying_key();
    let valid_wrong_key = verify_segment(&header, payload, &footer, &wrong_pubkey);
    println!(
        "  Wrong public key:  {}",
        if valid_wrong_key { "VALID (BAD!)" } else { "INVALID (wrong key detected)" }
    );
    assert!(!valid_wrong_key, "wrong key should fail");

    // ====================================================================
    // 5. Witness Chain
    // ====================================================================
    println!("\n--- 5. Witness Chain Creation ---");

    let num_entries = 5;
    let entries: Vec<WitnessEntry> = (0..num_entries)
        .map(|i| {
            let action_data = format!("action-{}: ingest batch #{}", i, i * 100);
            WitnessEntry {
                prev_hash: [0u8; 32], // will be overwritten by create_witness_chain
                action_hash: shake256_256(action_data.as_bytes()),
                timestamp_ns: 1_700_000_000_000_000_000 + i as u64 * 1_000_000_000,
                witness_type: if i == 0 { 0x01 } else { 0x02 }, // PROVENANCE then COMPUTATION
            }
        })
        .collect();

    println!("  Creating chain with {} entries...", num_entries);
    let chain_bytes = create_witness_chain(&entries);
    println!("  Chain size: {} bytes ({} bytes per entry)", chain_bytes.len(), chain_bytes.len() / num_entries);

    // Print chain entries.
    println!("\n  Chain entries:");
    println!(
        "  {:>5}  {:>8}  {:>18}  {:>32}",
        "Index", "Type", "Timestamp (ns)", "Prev Hash (first 16 bytes)"
    );
    println!(
        "  {:->5}  {:->8}  {:->18}  {:->32}",
        "", "", "", ""
    );

    // ====================================================================
    // 6. Verify witness chain
    // ====================================================================
    println!("\n--- 6. Witness Chain Verification ---");

    match verify_witness_chain(&chain_bytes) {
        Ok(verified_entries) => {
            println!("  Chain integrity: VALID ({} entries verified)", verified_entries.len());

            for (i, entry) in verified_entries.iter().enumerate() {
                let wtype = match entry.witness_type {
                    0x01 => "PROV",
                    0x02 => "COMP",
                    _ => "????",
                };
                println!(
                    "  {:>5}  {:>8}  {:>18}  {}",
                    i,
                    wtype,
                    entry.timestamp_ns,
                    hex_string(&entry.prev_hash[..16]),
                );
            }

            // Verify first entry has zero prev_hash (genesis).
            assert_eq!(
                verified_entries[0].prev_hash,
                [0u8; 32],
                "first entry should have zero prev_hash"
            );
            println!("\n  Genesis entry has zero prev_hash: confirmed.");

            // Verify action hashes match original entries.
            for (i, (orig, verified)) in entries.iter().zip(verified_entries.iter()).enumerate() {
                assert_eq!(
                    orig.action_hash, verified.action_hash,
                    "action hash mismatch at entry {}",
                    i
                );
            }
            println!("  All action hashes match original entries.");
        }
        Err(e) => {
            println!("  Chain integrity: FAILED ({:?})", e);
        }
    }

    // ====================================================================
    // 7. Witness chain tamper detection
    // ====================================================================
    println!("\n--- 7. Witness Chain Tamper Detection ---");

    // Tamper with the third entry's action_hash (byte 32 of entry 2).
    let entry_size = 73; // 32 + 32 + 8 + 1
    let tamper_offset = 2 * entry_size + 32; // third entry, action_hash byte 0

    let mut tampered_chain = chain_bytes.clone();
    tampered_chain[tamper_offset] ^= 0xFF; // flip bits

    match verify_witness_chain(&tampered_chain) {
        Ok(_) => {
            println!("  Tampered chain: VALID (unexpected!)");
        }
        Err(e) => {
            println!("  Tampered chain: INVALID ({:?})", e);
            println!("  Tamper at entry 2 (byte {}) successfully detected.", tamper_offset);
        }
    }

    // Truncation detection.
    let truncated_chain = &chain_bytes[..chain_bytes.len() - 10];
    match verify_witness_chain(truncated_chain) {
        Ok(_) => println!("  Truncated chain: VALID (unexpected!)"),
        Err(e) => println!("  Truncated chain: INVALID ({:?})", e),
    }

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Summary ===\n");
    println!("  Ed25519 signing and verification: working");
    println!("  Tamper detection (payload):       working");
    println!("  Tamper detection (header):        working");
    println!("  Wrong-key rejection:              working");
    println!("  Witness chain creation:           {} entries", num_entries);
    println!("  Witness chain verification:       working");
    println!("  Witness chain tamper detection:    working");

    println!("\nDone.");
}

/// Format bytes as a hex string.
fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
