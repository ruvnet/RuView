//! Low-Level Wire Format
//!
//! Demonstrates reading and writing raw RVF segments using rvf-wire:
//! 1. Write a VEC_SEG with vector data
//! 2. Write an INDEX_SEG with adjacency data
//! 3. Write a MANIFEST_SEG
//! 4. Read segments back and validate hashes
//! 5. Print segment info
//! 6. Show tail-scan for latest manifest

use rvf_types::{SegmentFlags, SegmentType, SEGMENT_MAGIC};
use rvf_wire::{write_segment, read_segment, validate_segment};
use rvf_wire::tail_scan::find_latest_manifest;

fn main() {
    println!("=== RVF Wire Format Example ===\n");

    // We will build an RVF file in memory by concatenating segments.
    let mut file_data: Vec<u8> = Vec::new();

    // ====================================================================
    // 1. Write a VEC_SEG (vector data segment)
    // ====================================================================
    println!("--- 1. Writing VEC_SEG ---");

    // Build a VEC_SEG payload: dimension (u16) + count (u32) + vector data.
    let dim: u16 = 4;
    let count: u32 = 3;
    let vectors: Vec<[f32; 4]> = vec![
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    let ids: Vec<u64> = vec![100, 200, 300];

    // Build payload: dim (2 bytes) + count (4 bytes) + entries (id + vector each).
    let mut vec_payload = Vec::new();
    vec_payload.extend_from_slice(&dim.to_le_bytes());
    vec_payload.extend_from_slice(&count.to_le_bytes());
    for (i, vec_data) in vectors.iter().enumerate() {
        vec_payload.extend_from_slice(&ids[i].to_le_bytes());
        for &val in vec_data {
            vec_payload.extend_from_slice(&val.to_le_bytes());
        }
    }

    let vec_seg = write_segment(
        SegmentType::Vec as u8,
        &vec_payload,
        SegmentFlags::empty(),
        1, // segment_id
    );

    println!("  Segment ID:      1");
    println!("  Segment type:    VEC_SEG (0x{:02X})", SegmentType::Vec as u8);
    println!("  Payload size:    {} bytes", vec_payload.len());
    println!("  Total seg size:  {} bytes (64-byte aligned)", vec_seg.len());

    let vec_seg_offset = file_data.len();
    file_data.extend_from_slice(&vec_seg);

    // ====================================================================
    // 2. Write an INDEX_SEG (adjacency data)
    // ====================================================================
    println!("\n--- 2. Writing INDEX_SEG ---");

    // Build an index payload: a simple adjacency list.
    // Format: num_nodes (u32) + entries (node_id: u64 + neighbor_count: u16 + neighbors: u64[])
    let mut index_payload = Vec::new();
    let num_nodes: u32 = 3;
    index_payload.extend_from_slice(&num_nodes.to_le_bytes());

    let adjacency: Vec<(u64, Vec<u64>)> = vec![
        (100, vec![200, 300]),
        (200, vec![100, 300]),
        (300, vec![100, 200]),
    ];

    for (node_id, neighbors) in &adjacency {
        index_payload.extend_from_slice(&node_id.to_le_bytes());
        index_payload.extend_from_slice(&(neighbors.len() as u16).to_le_bytes());
        for &nid in neighbors {
            index_payload.extend_from_slice(&nid.to_le_bytes());
        }
    }

    let index_seg = write_segment(
        SegmentType::Index as u8,
        &index_payload,
        SegmentFlags::empty().with(SegmentFlags::SEALED),
        2,
    );

    println!("  Segment ID:      2");
    println!("  Segment type:    INDEX_SEG (0x{:02X})", SegmentType::Index as u8);
    println!("  Flags:           SEALED");
    println!("  Payload size:    {} bytes", index_payload.len());
    println!("  Total seg size:  {} bytes", index_seg.len());

    let index_seg_offset = file_data.len();
    file_data.extend_from_slice(&index_seg);

    // ====================================================================
    // 3. Write a MANIFEST_SEG
    // ====================================================================
    println!("\n--- 3. Writing MANIFEST_SEG ---");

    // Build a manifest payload: epoch + dimension + vector_count + segment_dir
    let mut manifest_payload = Vec::new();
    let epoch: u32 = 1;
    let total_vectors: u64 = 3;
    manifest_payload.extend_from_slice(&epoch.to_le_bytes());
    manifest_payload.extend_from_slice(&dim.to_le_bytes());
    manifest_payload.extend_from_slice(&total_vectors.to_le_bytes());
    // Segment directory: 2 entries (vec_seg, index_seg)
    let dir_count: u32 = 2;
    manifest_payload.extend_from_slice(&dir_count.to_le_bytes());
    // Entry 1: vec_seg
    manifest_payload.extend_from_slice(&1u64.to_le_bytes()); // seg_id
    manifest_payload.extend_from_slice(&(vec_seg_offset as u64).to_le_bytes()); // offset
    manifest_payload.extend_from_slice(&(vec_payload.len() as u64).to_le_bytes()); // payload_len
    manifest_payload.push(SegmentType::Vec as u8); // seg_type
    // Entry 2: index_seg
    manifest_payload.extend_from_slice(&2u64.to_le_bytes());
    manifest_payload.extend_from_slice(&(index_seg_offset as u64).to_le_bytes());
    manifest_payload.extend_from_slice(&(index_payload.len() as u64).to_le_bytes());
    manifest_payload.push(SegmentType::Index as u8);

    let manifest_seg = write_segment(
        SegmentType::Manifest as u8,
        &manifest_payload,
        SegmentFlags::empty(),
        3,
    );

    println!("  Segment ID:      3");
    println!("  Segment type:    MANIFEST_SEG (0x{:02X})", SegmentType::Manifest as u8);
    println!("  Payload size:    {} bytes", manifest_payload.len());
    println!("  Total seg size:  {} bytes", manifest_seg.len());

    let manifest_seg_offset = file_data.len();
    file_data.extend_from_slice(&manifest_seg);

    // ====================================================================
    // 4. Read back and validate segments
    // ====================================================================
    println!("\n--- 4. Reading Segments Back ---\n");

    let segments = [
        ("VEC_SEG", vec_seg_offset),
        ("INDEX_SEG", index_seg_offset),
        ("MANIFEST_SEG", manifest_seg_offset),
    ];

    for (name, offset) in &segments {
        let data = &file_data[*offset..];
        let (header, payload) = read_segment(data).expect("failed to read segment");

        println!("  {} at offset {}:", name, offset);
        println!("    Magic:         0x{:08X} (valid={})", header.magic, header.magic == SEGMENT_MAGIC);
        println!("    Version:       {}", header.version);
        println!("    Segment ID:    {}", header.segment_id);
        println!("    Type:          0x{:02X}", header.seg_type);
        println!("    Flags:         0x{:04X}", header.flags);
        println!("    Payload len:   {} bytes", header.payload_length);
        println!("    Checksum algo: {}", header.checksum_algo);
        println!(
            "    Content hash:  {}",
            header.content_hash.iter().map(|b| format!("{:02x}", b)).collect::<String>()
        );

        // Validate the content hash.
        match validate_segment(&header, payload) {
            Ok(()) => println!("    Hash valid:    YES"),
            Err(e) => println!("    Hash valid:    NO ({:?})", e),
        }
        println!();
    }

    // ====================================================================
    // 5. Demonstrate corruption detection
    // ====================================================================
    println!("--- 5. Corruption Detection ---");

    let (header, _payload) = read_segment(&file_data[vec_seg_offset..]).unwrap();
    let corrupted_payload = b"this is definitely not the original payload";
    match validate_segment(&header, corrupted_payload) {
        Ok(()) => println!("  Corrupted payload: hash matched (unexpected!)"),
        Err(e) => println!("  Corrupted payload: hash mismatch detected ({:?})", e),
    }

    // ====================================================================
    // 6. Tail-scan for latest manifest
    // ====================================================================
    println!("\n--- 6. Tail-Scan for Latest Manifest ---");

    match find_latest_manifest(&file_data) {
        Ok((offset, header)) => {
            println!("  Found manifest at offset {}", offset);
            println!("  Segment ID: {}", header.segment_id);
            println!("  Payload length: {} bytes", header.payload_length);
        }
        Err(e) => {
            println!("  Manifest not found: {:?}", e);
        }
    }

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== File Layout Summary ===\n");
    println!("  Total file size: {} bytes", file_data.len());
    println!("  {:>6}  {:>12}  {:>10}  {:>10}", "Offset", "Type", "Seg ID", "Size");
    println!("  {:->6}  {:->12}  {:->10}  {:->10}", "", "", "", "");
    println!(
        "  {:>6}  {:>12}  {:>10}  {:>10}",
        vec_seg_offset, "VEC_SEG", 1, vec_seg.len()
    );
    println!(
        "  {:>6}  {:>12}  {:>10}  {:>10}",
        index_seg_offset, "INDEX_SEG", 2, index_seg.len()
    );
    println!(
        "  {:>6}  {:>12}  {:>10}  {:>10}",
        manifest_seg_offset, "MANIFEST_SEG", 3, manifest_seg.len()
    );

    println!("\nDone.");
}
