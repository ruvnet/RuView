//! Exotic Capability: Self-Booting RVF Microservice
//!
//! Demonstrates embedding a kernel image into an RVF file, turning it
//! into a self-booting cognitive container that contains both data AND runtime.
//!
//! Features:
//!   - Create a store with vectors
//!   - Create a synthetic kernel image (constructed unikernel binary)
//!   - Embed using store.embed_kernel()
//!   - Extract the kernel and verify header fields
//!   - Witness chain for boot sequence audit
//!   - Show that the file contains data + runtime
//!
//! RVF segments used: VEC_SEG, KERNEL_SEG, MANIFEST_SEG, WITNESS_SEG
//!
//! Run: cargo run --example self_booting

use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};
use rvf_runtime::options::DistanceMetric;
use rvf_kernel;
use rvf_types::kernel::{KernelArch, KernelHeader, KernelType, KERNEL_MAGIC};
use rvf_crypto::{create_witness_chain, verify_witness_chain, shake256_256, WitnessEntry};
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
    println!("=== Self-Booting RVF Microservice ===\n");

    let dim = 128;
    let num_vectors = 50;

    // ====================================================================
    // 1. Create store with vectors
    // ====================================================================
    println!("--- 1. Create Store with Vector Data ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("bootable.rvf");

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

    // Verify query works before kernel embedding
    let query = random_vector(dim, 25);
    let results = store
        .query(&query, 5, &QueryOptions::default())
        .expect("query failed");
    println!("  Pre-kernel query: top-5 results OK (nearest ID={})", results[0].id);

    // ====================================================================
    // 2. Create a synthetic kernel image
    // ====================================================================
    println!("\n--- 2. Synthetic Kernel Image ---");

    // Build a real kernel (Docker) or fall back to builtin stub
    let tmpdir = std::env::temp_dir().join("rvf-self-boot-build");
    std::fs::create_dir_all(&tmpdir).ok();
    let built = rvf_kernel::KernelBuilder::new(KernelArch::X86_64)
        .with_initramfs(&["rvf-server"])
        .build(&tmpdir)
        .expect("build kernel");
    let kernel_image = built.bzimage;
    let kernel_label = if kernel_image.len() > 8192 { "real bzImage" } else { "builtin stub" };

    println!("  Kernel image size:  {} bytes ({})", kernel_image.len(), kernel_label);
    println!("  Kernel type:        HermitOS (unikernel)");
    println!("  Target arch:        x86_64");
    println!("  API port:           8080");

    // ====================================================================
    // 3. Embed kernel into the RVF file
    // ====================================================================
    println!("\n--- 3. Embed Kernel (KERNEL_SEG) ---");

    let seg_id = store
        .embed_kernel(
            KernelArch::X86_64 as u8,
            KernelType::Hermit as u8,
            0x0018, // HAS_QUERY_API | HAS_NETWORKING
            &kernel_image,
            8080,
            Some("console=ttyS0 quiet rvf.listen=0.0.0.0:8080"),
        )
        .expect("failed to embed kernel");

    println!("  Kernel embedded as segment ID: {}", seg_id);

    let status = store.status();
    println!("  Store file size: {} bytes", status.file_size);
    println!("  Total segments:  {}", status.total_segments);

    // ====================================================================
    // 4. Extract kernel and verify header
    // ====================================================================
    println!("\n--- 4. Extract and Verify Kernel ---");

    let (header_bytes, image_bytes) = store
        .extract_kernel()
        .expect("extract_kernel failed")
        .expect("no kernel segment found");

    assert_eq!(header_bytes.len(), 128, "kernel header should be 128 bytes");

    // Parse the kernel header
    let header_arr: [u8; 128] = header_bytes.try_into().expect("header size mismatch");
    let kernel_header = KernelHeader::from_bytes(&header_arr).expect("invalid kernel header");

    println!("  Header verification:");
    println!("    Magic:          0x{:08X} (expected: 0x{:08X}) {}",
        kernel_header.kernel_magic,
        KERNEL_MAGIC,
        if kernel_header.kernel_magic == KERNEL_MAGIC { "OK" } else { "FAIL" }
    );
    println!("    Arch:           {} (x86_64=0x{:02X})",
        kernel_header.arch,
        KernelArch::X86_64 as u8
    );
    println!("    Type:           {} (Hermit=0x{:02X})",
        kernel_header.kernel_type,
        KernelType::Hermit as u8
    );
    println!("    API port:       {}", kernel_header.api_port);
    println!("    Image size:     {} bytes", kernel_header.image_size);
    println!("    Image hash:     {}...", hex_string(&kernel_header.image_hash[..16]));
    println!("    Cmdline offset: {}", kernel_header.cmdline_offset);
    println!("    Cmdline length: {}", kernel_header.cmdline_length);

    assert_eq!(kernel_header.kernel_magic, KERNEL_MAGIC);
    assert_eq!(kernel_header.arch, KernelArch::X86_64 as u8);
    assert_eq!(kernel_header.kernel_type, KernelType::Hermit as u8);
    assert_eq!(kernel_header.api_port, 8080);
    assert_eq!(kernel_header.image_size, kernel_image.len() as u64);

    // Verify the image matches
    assert!(
        image_bytes.starts_with(&kernel_image),
        "extracted image does not match original"
    );
    println!("\n  Image verification: extracted image matches original.");

    // ====================================================================
    // 5. Verify queries still work alongside kernel
    // ====================================================================
    println!("\n--- 5. Co-existence Verification ---");

    let results_after = store
        .query(&query, 5, &QueryOptions::default())
        .expect("query failed after kernel embed");

    println!("  Post-kernel query: top-5 results OK (nearest ID={})", results_after[0].id);
    assert_eq!(results[0].id, results_after[0].id, "query results should match");
    println!("  Query results match pre-kernel: VERIFIED");

    // ====================================================================
    // 6. Witness chain for boot sequence
    // ====================================================================
    println!("\n--- 6. Boot Sequence Witness Chain ---");

    let boot_steps = [
        ("vmm_init", 0x01u8),
        ("kernel_load", 0x02),
        ("memory_map", 0x02),
        ("rvf_mount", 0x02),
        ("api_listen", 0x01),
    ];

    let entries: Vec<WitnessEntry> = boot_steps
        .iter()
        .enumerate()
        .map(|(i, (step, wtype))| {
            let action_data = format!("boot:{}:{}", step, i);
            WitnessEntry {
                prev_hash: [0u8; 32],
                action_hash: shake256_256(action_data.as_bytes()),
                timestamp_ns: 1_700_000_000_000_000_000 + i as u64 * 100_000_000,
                witness_type: *wtype,
            }
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified = verify_witness_chain(&chain_bytes).expect("chain verification failed");
    println!("  Boot chain: {} entries, VALID", verified.len());

    println!("\n  Boot sequence:");
    for (i, (step, _)) in boot_steps.iter().enumerate() {
        let wtype_name = match verified[i].witness_type {
            0x01 => "PROV",
            0x02 => "COMP",
            _ => "????",
        };
        println!("    [{}] {} -> {}", wtype_name, i, step);
    }

    // ====================================================================
    // 7. Show what the file contains
    // ====================================================================
    println!("\n--- 7. Self-Booting File Contents ---");
    let final_status = store.status();
    println!("  The single .rvf file now contains:");
    println!("    - {} vectors ({}-dim embeddings)", final_status.total_vectors, dim);
    println!("    - HermitOS x86_64 kernel image ({} bytes)", kernel_image.len());
    println!("    - Query API on port 8080");
    println!("    - {} segments total", final_status.total_segments);
    println!("    - File size: {} bytes", final_status.file_size);
    println!("\n  This file can serve queries WITHOUT any external runtime.");
    println!("  Just boot it: firecracker --kernel bootable.rvf");

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
