//! `rvf verify-witness` -- Verify all witness events in chain.
//!
//! Scans the RVF file for WITNESS_SEG segments, extracts the payload
//! bytes, and runs `rvf_crypto::verify_witness_chain()` to validate
//! the full SHAKE-256 hash chain.  Reports entry count, chain
//! validity, first/last timestamps, and any chain breaks.

use clap::Args;
use std::io::{BufReader, Read};

use rvf_crypto::witness::{verify_witness_chain, WitnessEntry};
use rvf_types::{SegmentType, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC};

#[derive(Args)]
pub struct VerifyWitnessArgs {
    /// Path to the RVF store
    pub file: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// Result of verifying one witness segment's chain.
struct ChainResult {
    /// Number of entries decoded from this segment.
    entry_count: usize,
    /// Whether the hash chain is intact.
    chain_valid: bool,
    /// Decoded entries (empty when chain_valid == false).
    entries: Vec<WitnessEntry>,
    /// Human-readable error, if any.
    error: Option<String>,
}

/// Extract all WITNESS_SEG payloads from the raw file bytes.
///
/// Returns a vec of `(segment_offset, payload_bytes)`.
fn extract_witness_payloads(raw: &[u8]) -> Vec<(usize, Vec<u8>)> {
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

            if seg_type == SegmentType::Witness as u8 && payload_end <= raw.len() {
                let payload = raw[payload_start..payload_end].to_vec();
                results.push((i, payload));
            }

            // Advance past this segment.
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

/// Verify a single witness payload through the crypto chain.
fn verify_payload(payload: &[u8]) -> ChainResult {
    if payload.is_empty() {
        return ChainResult {
            entry_count: 0,
            chain_valid: true,
            entries: Vec::new(),
            error: None,
        };
    }

    match verify_witness_chain(payload) {
        Ok(entries) => ChainResult {
            entry_count: entries.len(),
            chain_valid: true,
            entries,
            error: None,
        },
        Err(e) => {
            // Try to estimate how many entries were in the payload
            // (73 bytes per entry).
            let estimated = payload.len() / 73;
            ChainResult {
                entry_count: estimated,
                chain_valid: false,
                entries: Vec::new(),
                error: Some(format!("{e}")),
            }
        }
    }
}

/// Format a nanosecond timestamp as a human-readable UTC string.
fn format_timestamp_ns(ns: u64) -> String {
    if ns == 0 {
        return "0 (genesis)".to_string();
    }
    let secs = ns / 1_000_000_000;
    let sub_ns = ns % 1_000_000_000;
    format!("{secs}.{sub_ns:09}s (unix epoch)")
}

/// Map witness_type byte to a name.
fn witness_type_name(wt: u8) -> &'static str {
    match wt {
        0x01 => "PROVENANCE",
        0x02 => "COMPUTATION",
        0x03 => "PLATFORM_ATTESTATION",
        0x04 => "KEY_BINDING",
        0x05 => "DATA_PROVENANCE",
        _ => "UNKNOWN",
    }
}

pub fn run(args: VerifyWitnessArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Read the entire file into memory for segment scanning.
    let file = std::fs::File::open(&args.file)?;
    let mut reader = BufReader::new(file);
    let mut raw_bytes = Vec::new();
    reader.read_to_end(&mut raw_bytes)?;

    let payloads = extract_witness_payloads(&raw_bytes);

    if payloads.is_empty() {
        if args.json {
            crate::output::print_json(&serde_json::json!({
                "status": "no_witnesses",
                "witness_segments": 0,
                "total_entries": 0,
            }));
        } else {
            println!("No witness segments found in file.");
        }
        return Ok(());
    }

    // Verify each witness segment's chain.
    let mut total_entries: usize = 0;
    let mut total_valid_chains: usize = 0;
    let mut all_entries: Vec<WitnessEntry> = Vec::new();
    let mut chain_results: Vec<serde_json::Value> = Vec::new();
    let mut chain_breaks: Vec<String> = Vec::new();

    for (idx, (seg_offset, payload)) in payloads.iter().enumerate() {
        let result = verify_payload(payload);
        total_entries += result.entry_count;

        if result.chain_valid {
            total_valid_chains += 1;
            all_entries.extend(result.entries.iter().cloned());
        } else {
            chain_breaks.push(format!(
                "Segment #{} at offset 0x{:X}: {}",
                idx,
                seg_offset,
                result.error.as_deref().unwrap_or("unknown error"),
            ));
        }

        if args.json {
            let first_ts = result.entries.first().map(|e| e.timestamp_ns).unwrap_or(0);
            let last_ts = result.entries.last().map(|e| e.timestamp_ns).unwrap_or(0);
            chain_results.push(serde_json::json!({
                "segment_index": idx,
                "segment_offset": format!("0x{:X}", seg_offset),
                "entry_count": result.entry_count,
                "chain_valid": result.chain_valid,
                "first_timestamp_ns": first_ts,
                "last_timestamp_ns": last_ts,
                "error": result.error,
            }));
        }
    }

    let first_ts = all_entries.first().map(|e| e.timestamp_ns).unwrap_or(0);
    let last_ts = all_entries.last().map(|e| e.timestamp_ns).unwrap_or(0);
    let all_valid = total_valid_chains == payloads.len();

    if args.json {
        crate::output::print_json(&serde_json::json!({
            "status": if all_valid { "valid" } else { "invalid" },
            "witness_segments": payloads.len(),
            "valid_chains": total_valid_chains,
            "total_entries": total_entries,
            "first_timestamp_ns": first_ts,
            "last_timestamp_ns": last_ts,
            "chain_breaks": chain_breaks,
            "segments": chain_results,
        }));
    } else {
        println!("Witness chain verification (cryptographic):");
        println!();
        crate::output::print_kv("Witness segments:", &payloads.len().to_string());
        crate::output::print_kv(
            "Valid chains:",
            &format!("{}/{}", total_valid_chains, payloads.len()),
        );
        crate::output::print_kv("Total entries:", &total_entries.to_string());

        if !all_entries.is_empty() {
            println!();
            crate::output::print_kv("First timestamp:", &format_timestamp_ns(first_ts));
            crate::output::print_kv("Last timestamp:", &format_timestamp_ns(last_ts));

            // Show witness type distribution.
            let mut type_counts = std::collections::HashMap::new();
            for entry in &all_entries {
                *type_counts.entry(entry.witness_type).or_insert(0u64) += 1;
            }
            println!();
            println!("  Entry types:");
            let mut types: Vec<_> = type_counts.iter().collect();
            types.sort_by_key(|(k, _)| **k);
            for (wt, count) in types {
                println!(
                    "    0x{:02X} ({:20}): {}",
                    wt,
                    witness_type_name(*wt),
                    count
                );
            }
        }

        println!();
        if all_valid {
            println!("  All witness hash chains verified successfully.");
        } else {
            println!(
                "  WARNING: {} chain(s) failed verification:",
                chain_breaks.len()
            );
            for brk in &chain_breaks {
                println!("    - {}", brk);
            }
        }
    }

    Ok(())
}
