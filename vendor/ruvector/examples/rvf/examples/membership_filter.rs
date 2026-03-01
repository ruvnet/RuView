//! Membership Filter — Shared HNSW Index Views
//!
//! Demonstrates the MembershipFilter (MEMBERSHIP_SEG, 0x22) per ADR-031:
//! 1. Create a MembershipFilter in include mode
//! 2. Add/remove vector IDs
//! 3. Check membership (visibility)
//! 4. Serialize and deserialize (round-trip)
//! 5. Show exclude mode (inverted logic)
//! 6. Build a MembershipHeader for wire serialization
//!
//! The membership filter enables shared HNSW index traversal:
//! - Parent and child share the same HNSW graph
//! - The filter controls which results are returned
//! - Excluded nodes still serve as routing waypoints in the graph
//!
//! RVF types used: MembershipHeader (96B), MembershipFilter, FilterMode
//!
//! Run with:
//!   cargo run --example membership_filter

use rvf_runtime::membership::MembershipFilter;
use rvf_types::membership::{FilterMode, MembershipHeader, MEMBERSHIP_MAGIC};

fn hex(data: &[u8], n: usize) -> String {
    data.iter().take(n).map(|b| format!("{:02x}", b)).collect()
}

fn main() {
    println!("=== RVF Membership Filter Example ===\n");

    let total_vectors: u64 = 1000;

    // ================================================================
    // Phase 1: Include mode — only listed vectors are visible
    // ================================================================
    println!("--- Phase 1: Include Mode ---\n");

    let mut include_filter = MembershipFilter::new_include(total_vectors);

    println!("  Total vectors:   {}", include_filter.vector_count());
    println!("  Mode:            {:?} (visible iff bit is set)", include_filter.mode());
    println!("  Members:         {} (empty = empty view, fail-safe)", include_filter.member_count());
    println!();

    // In include mode, empty filter = nothing visible
    println!("  Visibility before adding members:");
    println!("    Vector 0:      {}", include_filter.contains(0));
    println!("    Vector 500:    {}", include_filter.contains(500));
    println!("    Vector 999:    {}", include_filter.contains(999));
    println!();

    // Add some vectors to the include set
    let included_ids: Vec<u64> = (0..500).collect(); // first 500 vectors
    for &id in &included_ids {
        include_filter.add(id);
    }

    println!("  After adding vectors 0-499:");
    println!("    Members:       {}", include_filter.member_count());
    println!("    Vector 0:      {} (included)", include_filter.contains(0));
    println!("    Vector 250:    {} (included)", include_filter.contains(250));
    println!("    Vector 499:    {} (included)", include_filter.contains(499));
    println!("    Vector 500:    {} (excluded)", include_filter.contains(500));
    println!("    Vector 999:    {} (excluded)", include_filter.contains(999));
    println!();

    // Remove a vector
    include_filter.remove(250);
    println!("  After removing vector 250:");
    println!("    Members:       {}", include_filter.member_count());
    println!("    Vector 250:    {} (removed)", include_filter.contains(250));
    println!("    Vector 249:    {} (still included)", include_filter.contains(249));
    println!();

    // ================================================================
    // Phase 2: Exclude mode — listed vectors are hidden
    // ================================================================
    println!("--- Phase 2: Exclude Mode ---\n");

    let mut exclude_filter = MembershipFilter::new_exclude(total_vectors);

    println!("  Mode:            {:?} (visible iff bit is NOT set)", exclude_filter.mode());
    println!("  Members:         {} (empty = full view)", exclude_filter.member_count());
    println!();

    // In exclude mode, empty filter = everything visible
    println!("  Visibility before adding exclusions:");
    println!("    Vector 0:      {} (all visible)", exclude_filter.contains(0));
    println!("    Vector 999:    {} (all visible)", exclude_filter.contains(999));
    println!();

    // Add vectors to the exclude set (these become hidden)
    for id in 900..1000 {
        exclude_filter.add(id);
    }

    println!("  After excluding vectors 900-999:");
    println!("    Members:       {} (excluded count)", exclude_filter.member_count());
    println!("    Vector 0:      {} (still visible)", exclude_filter.contains(0));
    println!("    Vector 899:    {} (still visible)", exclude_filter.contains(899));
    println!("    Vector 900:    {} (now hidden)", exclude_filter.contains(900));
    println!("    Vector 999:    {} (now hidden)", exclude_filter.contains(999));
    println!();

    // ================================================================
    // Phase 3: Serialization round-trip
    // ================================================================
    println!("--- Phase 3: Serialize / Deserialize ---\n");

    // Serialize the include filter
    let header = include_filter.to_header();
    let bitmap_data = include_filter.serialize();

    println!("  MembershipHeader (96 bytes):");
    println!("    Magic:         0x{:08X} (\"RVMB\")", header.magic);
    println!("    Version:       {}", header.version);
    println!("    Filter type:   {} (Bitmap)", header.filter_type);
    println!("    Filter mode:   {} (Include)", header.filter_mode);
    println!("    Vector count:  {}", header.vector_count);
    println!("    Member count:  {}", header.member_count);
    println!("    Filter size:   {} bytes", header.filter_size);
    println!("    Generation ID: {}", header.generation_id);
    println!("    Filter hash:   {}...", hex(&header.filter_hash, 8));
    println!();

    // Deserialize
    let restored = MembershipFilter::deserialize(&bitmap_data, &header)
        .expect("deserialize should succeed");

    println!("  Round-trip verification:");
    println!("    Vector count:  {} (was {})", restored.vector_count(), include_filter.vector_count());
    println!("    Member count:  {} (was {})", restored.member_count(), include_filter.member_count());
    println!("    Mode:          {:?} (was {:?})", restored.mode(), include_filter.mode());
    println!("    Vector 0:      {} (was {})", restored.contains(0), include_filter.contains(0));
    println!("    Vector 250:    {} (was {})", restored.contains(250), include_filter.contains(250));
    println!("    Vector 500:    {} (was {})", restored.contains(500), include_filter.contains(500));
    println!();

    // ================================================================
    // Phase 4: Generation tracking
    // ================================================================
    println!("--- Phase 4: Generation Counter ---\n");

    let mut filter = MembershipFilter::new_include(100);
    println!("  Initial generation: {}", filter.generation_id());

    filter.add(10);
    filter.bump_generation();
    println!("  After bump:         {}", filter.generation_id());

    filter.add(20);
    filter.bump_generation();
    println!("  After second bump:  {}", filter.generation_id());
    println!();
    println!("  The generation counter prevents stale-filter replay attacks.");
    println!("  On open, the runtime rejects filters with generation_id lower");
    println!("  than the manifest's membership_generation.");
    println!();

    // ================================================================
    // Phase 5: Wire format details
    // ================================================================
    println!("--- Phase 5: Wire Format ---\n");

    let header_bytes = header.to_bytes();
    println!("  MembershipHeader on wire: {} bytes", header_bytes.len());
    println!("  Bitmap payload:           {} bytes", bitmap_data.len());
    println!("  Total MEMBERSHIP_SEG:     {} bytes (header + bitmap)",
        header_bytes.len() + bitmap_data.len());
    println!();

    // Verify magic from raw bytes
    let magic = u32::from_le_bytes([
        header_bytes[0], header_bytes[1], header_bytes[2], header_bytes[3],
    ]);
    assert_eq!(magic, MEMBERSHIP_MAGIC);
    println!("  Magic verified:           0x{:08X} == MEMBERSHIP_MAGIC", magic);
    println!();

    // Verify round-trip of header bytes
    let mut raw = [0u8; 96];
    raw.copy_from_slice(&header_bytes);
    let decoded = MembershipHeader::from_bytes(&raw).expect("from_bytes should succeed");
    assert_eq!(decoded.vector_count, total_vectors);
    println!("  Header round-trip:        OK");
    println!();

    // ================================================================
    // Summary
    // ================================================================
    println!("=== Membership Filter Summary ===\n");
    println!("  Include mode:    vector visible iff filter.contains(id)");
    println!("  Exclude mode:    vector visible iff !filter.contains(id)");
    println!("  Fail-safe:       empty include filter = empty view");
    println!("  HNSW traversal:  excluded nodes serve as routing waypoints");
    println!("                   but are never returned in results");
    println!("  Segment type:    MEMBERSHIP_SEG (0x22)");
    println!("  Header size:     96 bytes (MembershipHeader)");
    println!("  Filter type:     Dense bitmap (1 bit per vector)");
    println!("  Anti-replay:     monotonic generation_id");
    println!();

    println!("Done.");
}
