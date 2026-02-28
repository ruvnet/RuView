//! Browser-Side Vector Search (WASM Target)
//!
//! Category: Runtime Targets
//!
//! Demonstrates what a browser-side WASM deployment looks like from the
//! Rust side. This example exercises the minimal API surface needed for
//! browser use: small footprint store (50 vectors, 64 dims), insert,
//! query, and wire-format serialization for WASM transfer.
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG (via RvfStore), raw wire
//! segments (via rvf_wire::write_segment) for WASM transfer demonstration.
//!
//! In real deployment, the rvf-wasm crate compiles to wasm32-unknown-unknown
//! and achieves a 5.5 KB WASM binary size. This example runs natively but
//! exercises the same code paths the WASM build would use.
//!
//! Run with:
//!   cargo run --example browser_wasm

use rvf_runtime::{QueryOptions, RvfOptions, RvfStore, SearchResult};
use rvf_runtime::options::DistanceMetric;
use rvf_types::{SegmentFlags, SegmentType, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC};
use rvf_wire::{write_segment, read_segment, validate_segment, calculate_padded_size};
use tempfile::TempDir;

/// Simple pseudo-random number generator (LCG) for deterministic results.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

fn main() {
    println!("=== RVF Browser WASM Vector Search ===\n");

    let dim = 64;
    let num_vectors = 50;
    let k = 3;

    // ====================================================================
    // 1. Create a minimal-footprint store (browser constraints)
    // ====================================================================
    println!("--- 1. Create Minimal Browser Store ---");
    println!("  Target: wasm32-unknown-unknown (5.5 KB binary)");
    println!("  Vectors: {} x {} dims (fp32)", num_vectors, dim);
    println!("  Memory budget: small (no HNSW, brute-force scan)");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("browser.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // ====================================================================
    // 2. Insert vectors (representing browser-side embedding cache)
    // ====================================================================
    println!("\n--- 2. Insert Vectors (Browser Embedding Cache) ---");

    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| random_vector(dim, i as u64))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_vectors as u64).collect();

    let ingest = store
        .ingest_batch(&vec_refs, &ids, None)
        .expect("failed to ingest");
    println!(
        "  Ingested {} vectors (rejected: {}, epoch: {})",
        ingest.accepted, ingest.rejected, ingest.epoch
    );

    let raw_bytes = (num_vectors * dim * 4) as f64;
    println!(
        "  Raw vector data: {:.1} KB ({} vectors x {} dims x 4 bytes)",
        raw_bytes / 1024.0,
        num_vectors,
        dim
    );

    // ====================================================================
    // 3. Query top-3 nearest neighbors (browser-side search)
    // ====================================================================
    println!("\n--- 3. Browser-Side Query (top-{}) ---", k);

    let query = random_vector(dim, 42);
    let results = store
        .query(&query, k, &QueryOptions::default())
        .expect("query failed");

    println!("  Query seed: 42");
    print_results(&results);

    // ====================================================================
    // 4. Show store status (file size = what gets transferred to browser)
    // ====================================================================
    println!("\n--- 4. Store Footprint ---");

    let status = store.status();
    println!("  Total vectors:  {}", status.total_vectors);
    println!("  File size:      {} bytes ({:.1} KB)", status.file_size, status.file_size as f64 / 1024.0);
    println!("  Segments:       {}", status.total_segments);
    println!("  Epoch:          {}", status.current_epoch);

    // ====================================================================
    // 5. Wire format: build raw segments for WASM transfer
    // ====================================================================
    println!("\n--- 5. Wire Format Segments (WASM Transfer Demonstration) ---");
    println!("  Building raw segments that WASM runtime would process...\n");

    // Build a VEC_SEG payload manually (same format as RvfStore uses internally)
    let mut vec_payload = Vec::new();
    vec_payload.extend_from_slice(&(dim as u16).to_le_bytes());
    vec_payload.extend_from_slice(&(num_vectors as u32).to_le_bytes());
    for (i, vec) in vectors.iter().enumerate() {
        vec_payload.extend_from_slice(&(i as u64).to_le_bytes());
        for &val in vec {
            vec_payload.extend_from_slice(&val.to_le_bytes());
        }
    }

    let vec_seg = write_segment(
        SegmentType::Vec as u8,
        &vec_payload,
        SegmentFlags::empty(),
        1,
    );

    let vec_padded = calculate_padded_size(SEGMENT_HEADER_SIZE, vec_payload.len());
    println!("  VEC_SEG:");
    println!("    Segment ID:     1");
    println!("    Type:           0x{:02X} (VEC_SEG)", SegmentType::Vec as u8);
    println!("    Payload:        {} bytes", vec_payload.len());
    println!("    Total (padded): {} bytes (64-byte aligned)", vec_padded);

    // Build a small MANIFEST_SEG payload
    let mut manifest_payload = Vec::new();
    let epoch: u32 = 1;
    let total: u64 = num_vectors as u64;
    manifest_payload.extend_from_slice(&epoch.to_le_bytes());
    manifest_payload.extend_from_slice(&(dim as u16).to_le_bytes());
    manifest_payload.extend_from_slice(&total.to_le_bytes());
    // Segment directory: 1 entry
    manifest_payload.extend_from_slice(&1u32.to_le_bytes());
    manifest_payload.extend_from_slice(&1u64.to_le_bytes()); // seg_id
    manifest_payload.extend_from_slice(&0u64.to_le_bytes()); // offset
    manifest_payload.extend_from_slice(&(vec_payload.len() as u64).to_le_bytes());
    manifest_payload.push(SegmentType::Vec as u8);

    let manifest_seg = write_segment(
        SegmentType::Manifest as u8,
        &manifest_payload,
        SegmentFlags::empty(),
        2,
    );

    let manifest_padded = calculate_padded_size(SEGMENT_HEADER_SIZE, manifest_payload.len());
    println!("\n  MANIFEST_SEG:");
    println!("    Segment ID:     2");
    println!("    Type:           0x{:02X} (MANIFEST_SEG)", SegmentType::Manifest as u8);
    println!("    Payload:        {} bytes", manifest_payload.len());
    println!("    Total (padded): {} bytes (64-byte aligned)", manifest_padded);

    // Combine into a constructed WASM transfer buffer
    let mut wasm_buffer: Vec<u8> = Vec::new();
    wasm_buffer.extend_from_slice(&vec_seg);
    let manifest_offset = wasm_buffer.len();
    wasm_buffer.extend_from_slice(&manifest_seg);

    println!("\n  Combined WASM transfer buffer:");
    println!("    Total size:     {} bytes ({:.1} KB)", wasm_buffer.len(), wasm_buffer.len() as f64 / 1024.0);
    println!("    VEC_SEG:        offset 0, {} bytes", vec_seg.len());
    println!("    MANIFEST_SEG:   offset {}, {} bytes", manifest_offset, manifest_seg.len());

    // ====================================================================
    // 6. Validate wire format integrity (what WASM runtime does on receive)
    // ====================================================================
    println!("\n--- 6. Wire Format Validation (WASM Receive Path) ---");

    // Validate VEC_SEG
    let (vec_header, vec_data) = read_segment(&wasm_buffer[0..]).expect("failed to read VEC_SEG");
    let magic_valid = vec_header.magic == SEGMENT_MAGIC;
    println!("  VEC_SEG at offset 0:");
    println!("    Magic:       0x{:08X} (valid={})", vec_header.magic, magic_valid);
    println!("    Version:     {}", vec_header.version);
    println!("    Seg ID:      {}", vec_header.segment_id);
    println!("    Payload len: {} bytes", vec_header.payload_length);
    match validate_segment(&vec_header, vec_data) {
        Ok(()) => println!("    Hash:        VALID"),
        Err(e) => println!("    Hash:        INVALID ({:?})", e),
    }

    // Validate MANIFEST_SEG
    let (mfst_header, mfst_data) = read_segment(&wasm_buffer[manifest_offset..])
        .expect("failed to read MANIFEST_SEG");
    println!("\n  MANIFEST_SEG at offset {}:", manifest_offset);
    println!("    Magic:       0x{:08X} (valid={})", mfst_header.magic, mfst_header.magic == SEGMENT_MAGIC);
    println!("    Version:     {}", mfst_header.version);
    println!("    Seg ID:      {}", mfst_header.segment_id);
    println!("    Payload len: {} bytes", mfst_header.payload_length);
    match validate_segment(&mfst_header, mfst_data) {
        Ok(()) => println!("    Hash:        VALID"),
        Err(e) => println!("    Hash:        INVALID ({:?})", e),
    }

    // ====================================================================
    // 7. Segment header layout detail (for WASM implementors)
    // ====================================================================
    println!("\n--- 7. Segment Header Layout (64 bytes) ---");
    println!("  {:>8}  {:>6}  Field", "Offset", "Size");
    println!("  {:->8}  {:->6}  {:->30}", "", "", "");
    println!("  {:>8}  {:>6}  magic (0x52564653 = \"RVFS\")", "0x00", "4");
    println!("  {:>8}  {:>6}  version (1)", "0x04", "1");
    println!("  {:>8}  {:>6}  seg_type", "0x05", "1");
    println!("  {:>8}  {:>6}  flags", "0x06", "2");
    println!("  {:>8}  {:>6}  segment_id", "0x08", "8");
    println!("  {:>8}  {:>6}  payload_length", "0x10", "8");
    println!("  {:>8}  {:>6}  timestamp_ns", "0x18", "8");
    println!("  {:>8}  {:>6}  checksum_algo", "0x20", "1");
    println!("  {:>8}  {:>6}  compression", "0x21", "1");
    println!("  {:>8}  {:>6}  reserved", "0x22", "6");
    println!("  {:>8}  {:>6}  content_hash", "0x28", "16");
    println!("  {:>8}  {:>6}  uncompressed_len", "0x38", "4");
    println!("  {:>8}  {:>6}  alignment_pad", "0x3C", "4");

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Browser WASM Summary ===\n");
    println!("  {:>24}  {:>12}", "Metric", "Value");
    println!("  {:->24}  {:->12}", "", "");
    println!("  {:>24}  {:>12}", "Vectors", num_vectors);
    println!("  {:>24}  {:>12}", "Dimensions", dim);
    println!("  {:>24}  {:>10.1} KB", "Raw vector data", raw_bytes / 1024.0);
    println!("  {:>24}  {:>10.1} KB", "RVF file size", status.file_size as f64 / 1024.0);
    println!("  {:>24}  {:>10.1} KB", "Wire transfer size", wasm_buffer.len() as f64 / 1024.0);
    println!("  {:>24}  {:>12}", "WASM binary target", "5.5 KB");
    println!("  {:>24}  {:>12}", "Query results (k)", k);
    println!("  {:>24}  {:>12}", "Segments", 2);

    store.close().expect("failed to close store");

    println!("\nDone.");
}

fn print_results(results: &[SearchResult]) {
    println!("  {:>6}  {:>12}", "ID", "Distance");
    println!("  {:->6}  {:->12}", "", "");
    for r in results {
        println!("  {:>6}  {:>12.6}", r.id, r.distance);
    }
}
