//! Exotic Capability: eBPF Hot-Path Acceleration
//!
//! Demonstrates embedding an eBPF program into an RVF file for
//! kernel-level fast-path vector distance computation. The EBPF_SEG
//! co-exists with regular vector data in the same file.
//!
//! Features:
//!   - Create a store with vectors
//!   - Create synthetic eBPF bytecode and BTF sections
//!   - Embed using store.embed_ebpf()
//!   - Extract and verify header fields
//!   - Verify normal queries work alongside eBPF segments
//!
//! RVF segments used: VEC_SEG, EBPF_SEG, MANIFEST_SEG
//!
//! Run: cargo run --example ebpf_accelerator

use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};
use rvf_runtime::options::DistanceMetric;
use rvf_types::ebpf::{EbpfAttachType, EbpfHeader, EbpfProgramType, EBPF_MAGIC};
use tempfile::TempDir;

/// Simple LCG-based pseudo-random vector generator for deterministic results.
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
    println!("=== eBPF Hot-Path Acceleration ===\n");

    let dim = 384;
    let num_vectors = 100;

    // ====================================================================
    // 1. Create store with vectors
    // ====================================================================
    println!("--- 1. Create Store with Vector Data ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("accelerated.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| random_vector(dim, i as u64))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_vectors as u64).collect();

    let ingest = store
        .ingest_batch(&vec_refs, &ids, None)
        .expect("ingest failed");
    println!("  Ingested {} vectors ({} dims)", ingest.accepted, dim);

    // Baseline query
    let query = random_vector(dim, 42);
    let baseline_results = store
        .query(&query, 5, &QueryOptions::default())
        .expect("query failed");
    println!("  Baseline query: top-5 nearest (ID={}, dist={:.6})",
        baseline_results[0].id, baseline_results[0].distance
    );

    // ====================================================================
    // 2. Create synthetic eBPF bytecode
    // ====================================================================
    println!("\n--- 2. Synthetic eBPF Program ---");

    // Build XDP distance computation program
    // Each BPF instruction is 8 bytes
    let num_instructions = 48;
    let mut bytecode = Vec::with_capacity(num_instructions * 8);
    for i in 0..num_instructions {
        // Generate BPF instructions: opcode + regs + off + imm
        let insn: u64 = 0xB700_0000_0000_0000 | ((i as u64) << 32); // MOV64 pattern
        bytecode.extend_from_slice(&insn.to_le_bytes());
    }

    // Build BTF (BPF Type Format) section
    let mut btf_data = Vec::with_capacity(256);
    // BTF header magic
    btf_data.extend_from_slice(&0x9FEB_u16.to_le_bytes()); // BTF magic
    btf_data.extend_from_slice(&1u8.to_le_bytes());        // version
    btf_data.push(0);                                       // flags
    btf_data.extend_from_slice(&24u32.to_le_bytes());       // hdr_len
    // Pad to 256 bytes
    btf_data.resize(256, 0);

    println!("  Program type:   XDP Distance Computation");
    println!("  Attach point:   XDP Ingress");
    println!("  Instructions:   {}", num_instructions);
    println!("  Bytecode size:  {} bytes", bytecode.len());
    println!("  BTF size:       {} bytes", btf_data.len());
    println!("  Max dimension:  {}", dim);

    // ====================================================================
    // 3. Embed eBPF program
    // ====================================================================
    println!("\n--- 3. Embed eBPF (EBPF_SEG) ---");

    let seg_id = store
        .embed_ebpf(
            EbpfProgramType::XdpDistance as u8,
            EbpfAttachType::XdpIngress as u8,
            dim as u16,
            &bytecode,
            Some(&btf_data),
        )
        .expect("failed to embed eBPF");

    println!("  eBPF embedded as segment ID: {}", seg_id);

    let status = store.status();
    println!("  Store file size: {} bytes", status.file_size);
    println!("  Total segments:  {}", status.total_segments);

    // ====================================================================
    // 4. Extract and verify eBPF header
    // ====================================================================
    println!("\n--- 4. Extract and Verify eBPF ---");

    let (header_bytes, payload_bytes) = store
        .extract_ebpf()
        .expect("extract_ebpf failed")
        .expect("no eBPF segment found");

    assert_eq!(header_bytes.len(), 64, "eBPF header should be 64 bytes");

    // Parse the eBPF header
    let header_arr: [u8; 64] = header_bytes.try_into().expect("header size mismatch");
    let ebpf_header = EbpfHeader::from_bytes(&header_arr).expect("invalid eBPF header");

    println!("  Header verification:");
    println!("    Magic:          0x{:08X} (expected: 0x{:08X}) {}",
        ebpf_header.ebpf_magic,
        EBPF_MAGIC,
        if ebpf_header.ebpf_magic == EBPF_MAGIC { "OK" } else { "FAIL" }
    );
    println!("    Program type:   0x{:02X} (XdpDistance=0x{:02X})",
        ebpf_header.program_type,
        EbpfProgramType::XdpDistance as u8
    );
    println!("    Attach type:    0x{:02X} (XdpIngress=0x{:02X})",
        ebpf_header.attach_type,
        EbpfAttachType::XdpIngress as u8
    );
    println!("    Insn count:     {}", ebpf_header.insn_count);
    println!("    Max dimension:  {}", ebpf_header.max_dimension);
    println!("    Program size:   {} bytes", ebpf_header.program_size);
    println!("    Map count:      {}", ebpf_header.map_count);
    println!("    BTF size:       {} bytes", ebpf_header.btf_size);
    println!("    Program hash:   {}...", hex_string(&ebpf_header.program_hash[..16]));

    // Verify header fields
    assert_eq!(ebpf_header.ebpf_magic, EBPF_MAGIC);
    assert_eq!(ebpf_header.program_type, EbpfProgramType::XdpDistance as u8);
    assert_eq!(ebpf_header.attach_type, EbpfAttachType::XdpIngress as u8);
    assert_eq!(ebpf_header.max_dimension, dim as u16);
    assert_eq!(ebpf_header.program_size, bytecode.len() as u64);
    assert_eq!(ebpf_header.btf_size, btf_data.len() as u32);

    println!("\n  All header fields verified.");

    // Verify payload contains bytecode + BTF
    assert_eq!(payload_bytes.len(), bytecode.len() + btf_data.len());
    assert_eq!(&payload_bytes[..bytecode.len()], bytecode.as_slice());
    assert_eq!(&payload_bytes[bytecode.len()..], btf_data.as_slice());
    println!("  Payload verified: bytecode + BTF match original.");

    // ====================================================================
    // 5. Verify queries work alongside eBPF
    // ====================================================================
    println!("\n--- 5. Co-existence Verification ---");

    let results_after = store
        .query(&query, 5, &QueryOptions::default())
        .expect("query failed after eBPF embed");

    println!("  Post-eBPF query: top-5 nearest (ID={}, dist={:.6})",
        results_after[0].id, results_after[0].distance
    );

    assert_eq!(
        baseline_results[0].id, results_after[0].id,
        "query results should be identical"
    );
    assert!(
        (baseline_results[0].distance - results_after[0].distance).abs() < 1e-6,
        "distances should match"
    );
    println!("  Results match baseline: VERIFIED");

    // ====================================================================
    // 6. Show file layout
    // ====================================================================
    println!("\n--- 6. File Layout ---");
    let final_status = store.status();
    println!("  The single .rvf file now contains:");
    println!("    - VEC_SEG:    {} vectors x {} dims", final_status.total_vectors, dim);
    println!("    - EBPF_SEG:   XDP program ({} instructions)", num_instructions);
    println!("    - BTF data:   {} bytes (type info for verifier)", btf_data.len());
    println!("    - MANIFEST:   segment directory + metadata");
    println!("    - Total:      {} bytes, {} segments",
        final_status.file_size, final_status.total_segments
    );
    println!("\n  At query time, the kernel can:");
    println!("    1. Load the XDP program from EBPF_SEG");
    println!("    2. Attach to NIC ingress for wire-speed distance");
    println!("    3. Fall back to userspace for complex queries");

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
