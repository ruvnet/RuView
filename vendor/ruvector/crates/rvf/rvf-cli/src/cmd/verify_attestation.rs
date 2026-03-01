//! `rvf verify-attestation` -- Verify KernelBinding and attestation.
//!
//! Validates the KERNEL_SEG header magic, computes the SHAKE-256-256
//! hash of the kernel image and compares it against the hash stored
//! in the header, inspects the KernelBinding, and scans for any
//! WITNESS_SEG payloads that contain attestation witness chains.

use clap::Args;
use std::io::{BufReader, Read};
use std::path::Path;

use rvf_crypto::{shake256_256, verify_attestation_witness_payload};
use rvf_runtime::RvfStore;
use rvf_types::kernel::KERNEL_MAGIC;
use rvf_types::{SegmentType, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC};

use super::map_rvf_err;

#[derive(Args)]
pub struct VerifyAttestationArgs {
    /// Path to the RVF store
    pub file: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// Scan raw file bytes for WITNESS_SEG payloads that look like attestation
/// witness payloads (first 4 bytes decode to a chain_entry_count > 0).
fn find_attestation_witness_payloads(raw: &[u8]) -> Vec<Vec<u8>> {
    let magic_bytes = SEGMENT_MAGIC.to_le_bytes();
    let mut results = Vec::new();
    let mut i = 0usize;

    while i + SEGMENT_HEADER_SIZE <= raw.len() {
        if raw[i..i + 4] == magic_bytes {
            let seg_type = raw[i + 5];
            let payload_len = u64::from_le_bytes([
                raw[i + 0x10],
                raw[i + 0x11],
                raw[i + 0x12],
                raw[i + 0x13],
                raw[i + 0x14],
                raw[i + 0x15],
                raw[i + 0x16],
                raw[i + 0x17],
            ]) as usize;

            let payload_start = i + SEGMENT_HEADER_SIZE;
            let payload_end = payload_start + payload_len;

            if seg_type == SegmentType::Witness as u8
                && payload_end <= raw.len()
                && payload_len >= 4
            {
                let payload = &raw[payload_start..payload_end];
                // Attestation witness payloads start with a u32 count + offset
                // table.  A plain witness chain (raw entries) would have bytes
                // that decode to a much larger count value, so this heuristic
                // is reasonable.  We attempt full verification below anyway.
                let count =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
                // A plausible attestation payload: count fits in the payload
                // with offset table + chain entries + at least some records.
                let min_size = 4 + count * 8 + count * 73;
                if count > 0 && count < 10_000 && payload_len >= min_size {
                    results.push(payload.to_vec());
                }
            }

            let advance = SEGMENT_HEADER_SIZE + payload_len;
            if advance > 0 && i.checked_add(advance).is_some() {
                i += advance;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    results
}

pub fn run(args: VerifyAttestationArgs) -> Result<(), Box<dyn std::error::Error>> {
    let store = RvfStore::open_readonly(Path::new(&args.file)).map_err(map_rvf_err)?;

    let kernel_data = store.extract_kernel().map_err(map_rvf_err)?;

    // Also scan for attestation witness payloads in the file.
    let raw_bytes = {
        let file = std::fs::File::open(&args.file)?;
        let mut reader = BufReader::new(file);
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        buf
    };
    let att_payloads = find_attestation_witness_payloads(&raw_bytes);

    match kernel_data {
        None => {
            if args.json {
                crate::output::print_json(&serde_json::json!({
                    "status": "no_kernel",
                    "message": "No KERNEL_SEG found in file",
                    "attestation_witnesses": att_payloads.len(),
                }));
            } else {
                println!("No KERNEL_SEG found in file.");
                if !att_payloads.is_empty() {
                    println!();
                    println!(
                        "  Found {} attestation witness payload(s) -- see verify-witness.",
                        att_payloads.len()
                    );
                }
            }
        }
        Some((header_bytes, image_bytes)) => {
            // -- 1. Verify kernel header magic -----------------------------------
            let magic = u32::from_le_bytes([
                header_bytes[0],
                header_bytes[1],
                header_bytes[2],
                header_bytes[3],
            ]);
            let magic_valid = magic == KERNEL_MAGIC;

            // -- 2. Verify image hash --------------------------------------------
            // The header stores the SHAKE-256-256 hash of the image at offset
            // 0x30..0x50 (32 bytes).
            let stored_image_hash = &header_bytes[0x30..0x50];
            let computed_image_hash = shake256_256(&image_bytes);
            let image_hash_valid = stored_image_hash == computed_image_hash.as_slice();

            let stored_hash_hex = crate::output::hex(stored_image_hash);
            let computed_hash_hex = crate::output::hex(&computed_image_hash);

            // -- 3. Check KernelBinding (128 bytes after 128-byte header) --------
            let has_binding = image_bytes.len() >= 128;

            let mut binding_valid = false;
            let mut manifest_hash_hex = String::new();
            let mut policy_hash_hex = String::new();

            if has_binding {
                let binding_bytes = &image_bytes[..128];
                manifest_hash_hex = crate::output::hex(&binding_bytes[0..32]);
                policy_hash_hex = crate::output::hex(&binding_bytes[32..64]);

                let binding_version = u16::from_le_bytes([binding_bytes[64], binding_bytes[65]]);

                binding_valid = binding_version > 0;
            }

            // -- 4. Verify arch --------------------------------------------------
            let arch = header_bytes[0x06];
            let arch_name = match arch {
                1 => "x86_64",
                2 => "aarch64",
                3 => "riscv64",
                _ => "unknown",
            };

            // -- 5. Verify attestation witness payloads --------------------------
            let mut att_verified: usize = 0;
            let mut att_entries_total: usize = 0;
            let mut att_errors: Vec<String> = Vec::new();

            for (idx, payload) in att_payloads.iter().enumerate() {
                match verify_attestation_witness_payload(payload) {
                    Ok(entries) => {
                        att_verified += 1;
                        att_entries_total += entries.len();
                    }
                    Err(e) => {
                        att_errors.push(format!("Attestation witness #{}: {}", idx, e));
                    }
                }
            }

            // -- 6. Overall status -----------------------------------------------
            let overall_valid = magic_valid && image_hash_valid && att_errors.is_empty();

            if args.json {
                crate::output::print_json(&serde_json::json!({
                    "status": if overall_valid { "valid" } else { "invalid" },
                    "magic_valid": magic_valid,
                    "arch": arch_name,
                    "image_hash_valid": image_hash_valid,
                    "stored_image_hash": stored_hash_hex,
                    "computed_image_hash": computed_hash_hex,
                    "has_kernel_binding": binding_valid,
                    "manifest_root_hash": if binding_valid { &manifest_hash_hex } else { "" },
                    "policy_hash": if binding_valid { &policy_hash_hex } else { "" },
                    "image_size": image_bytes.len(),
                    "attestation_witnesses": att_payloads.len(),
                    "attestation_verified": att_verified,
                    "attestation_entries": att_entries_total,
                    "attestation_errors": att_errors,
                }));
            } else {
                println!("Attestation verification:");
                crate::output::print_kv("Magic valid:", &magic_valid.to_string());
                crate::output::print_kv("Architecture:", arch_name);
                crate::output::print_kv("Image size:", &format!("{} bytes", image_bytes.len()));
                println!();

                // Image hash verification output.
                crate::output::print_kv("Stored image hash:", &stored_hash_hex);
                crate::output::print_kv("Computed image hash:", &computed_hash_hex);
                if image_hash_valid {
                    println!("  Image hash: MATCH");
                } else {
                    println!("  Image hash: MISMATCH -- image may be tampered!");
                }

                if binding_valid {
                    println!();
                    println!("  KernelBinding present:");
                    crate::output::print_kv("Manifest hash:", &manifest_hash_hex);
                    crate::output::print_kv("Policy hash:", &policy_hash_hex);
                } else {
                    println!();
                    println!("  No KernelBinding found (legacy format or unsigned stub).");
                }

                if !att_payloads.is_empty() {
                    println!();
                    crate::output::print_kv(
                        "Attestation witnesses:",
                        &format!(
                            "{} payload(s), {} verified, {} entries",
                            att_payloads.len(),
                            att_verified,
                            att_entries_total
                        ),
                    );
                    if !att_errors.is_empty() {
                        println!("  WARNING: attestation witness errors:");
                        for err in &att_errors {
                            println!("    - {}", err);
                        }
                    }
                }

                println!();
                if overall_valid {
                    println!("  Attestation verification PASSED.");
                } else {
                    let mut reasons = Vec::new();
                    if !magic_valid {
                        reasons.push("invalid magic");
                    }
                    if !image_hash_valid {
                        reasons.push("image hash mismatch");
                    }
                    if !att_errors.is_empty() {
                        reasons.push("attestation witness error(s)");
                    }
                    println!("  Attestation verification FAILED: {}", reasons.join(", "));
                }
            }
        }
    }
    Ok(())
}
