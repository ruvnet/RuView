//! Exotic Capability: Sealed Cognitive Engine (Capstone)
//!
//! Demonstrates RVF as a complete, self-contained cognitive unit by
//! combining every major RVF capability into a single sealed file.
//!
//! Components assembled:
//!   - 500 vectors:       the "knowledge base"
//!   - Metadata:          the "context"
//!   - Kernel image:      the "runtime" (KERNEL_SEG)
//!   - eBPF bytecode:     the "accelerator" (EBPF_SEG)
//!   - Witness chain:     the "trust chain" (10 entries)
//!   - Ed25519 signature: the "attestation"
//!   - Derived snapshot:  the "versioned release"
//!
//! RVF segments used: VEC_SEG, KERNEL_SEG, EBPF_SEG, MANIFEST_SEG,
//!                    WITNESS_SEG, CRYPTO_SEG
//!
//! Run: cargo run --example sealed_engine

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

use rvf_runtime::{
    MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore,
};
use rvf_runtime::options::DistanceMetric;
use rvf_types::kernel::{KernelArch, KernelHeader, KernelType, KERNEL_MAGIC};
use rvf_types::ebpf::{EbpfAttachType, EbpfHeader, EbpfProgramType, EBPF_MAGIC};
use rvf_types::{DerivationType, SegmentHeader, SegmentType};
use rvf_crypto::{
    sign_segment, verify_segment,
    create_witness_chain, verify_witness_chain, shake256_256, WitnessEntry,
};
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
    println!("=== Sealed Cognitive Engine (Capstone) ===\n");

    let dim = 256;
    let num_vectors = 500;

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("sealed_engine.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // ====================================================================
    // 1. Insert 500 vectors: the "knowledge base"
    // ====================================================================
    println!("--- 1. Knowledge Base (500 Vectors) ---");

    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| random_vector(dim, i as u64))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_vectors as u64).collect();

    let categories = ["perception", "reasoning", "memory", "action", "learning"];

    // Metadata: category (0), importance (1)
    let mut metadata = Vec::with_capacity(num_vectors * 2);
    for i in 0..num_vectors {
        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(categories[i % categories.len()].to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(((i * 11 + 7) % 101) as u64),
        });
    }

    let ingest = store
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("ingest failed");
    println!("  Vectors:    {} ({} dims)", ingest.accepted, dim);
    println!("  Categories: {:?}", categories);
    println!("  Metadata:   category + importance per vector");

    // Verify query
    let query = random_vector(dim, 250);
    let results = store
        .query(&query, 5, &QueryOptions::default())
        .expect("query failed");
    println!("  Query test: top-5 OK (nearest ID={}, dist={:.6})", results[0].id, results[0].distance);

    // ====================================================================
    // 2. Embed kernel: the "runtime"
    // ====================================================================
    println!("\n--- 2. Runtime (Kernel Image) ---");

    let mut kernel_image = Vec::with_capacity(8192);
    // Build a synthetic kernel binary
    kernel_image.extend_from_slice(&[0x7F, b'E', b'L', b'F']); // ELF magic
    kernel_image.extend_from_slice(b"RVF-COGNITIVE-ENGINE-v1.0");
    for i in 29..8192u32 {
        kernel_image.push((i.wrapping_mul(0xDEAD) >> 8) as u8);
    }

    let kernel_seg_id = store
        .embed_kernel(
            KernelArch::X86_64 as u8,
            KernelType::Hermit as u8,
            0x0078, // HAS_QUERY | HAS_INGEST | HAS_ADMIN | HAS_NETWORKING
            &kernel_image,
            9090,
            Some("rvf.mode=sealed rvf.readonly=true"),
        )
        .expect("failed to embed kernel");

    println!("  Kernel embedded:  segment ID {}", kernel_seg_id);
    println!("  Arch:             x86_64 (HermitOS)");
    println!("  Image size:       {} bytes", kernel_image.len());
    println!("  API port:         9090");
    println!("  Flags:            QUERY | INGEST | ADMIN | NETWORKING");

    // ====================================================================
    // 3. Embed eBPF: the "accelerator"
    // ====================================================================
    println!("\n--- 3. Accelerator (eBPF Program) ---");

    let num_insns = 64;
    let mut ebpf_bytecode = Vec::with_capacity(num_insns * 8);
    for i in 0..num_insns {
        let insn: u64 = 0xB700_0000_0000_0000 | ((i as u64) << 16);
        ebpf_bytecode.extend_from_slice(&insn.to_le_bytes());
    }

    let mut btf_section = Vec::with_capacity(512);
    btf_section.extend_from_slice(&0x9FEB_u16.to_le_bytes());
    btf_section.resize(512, 0);

    let ebpf_seg_id = store
        .embed_ebpf(
            EbpfProgramType::XdpDistance as u8,
            EbpfAttachType::XdpIngress as u8,
            dim as u16,
            &ebpf_bytecode,
            Some(&btf_section),
        )
        .expect("failed to embed eBPF");

    println!("  eBPF embedded:    segment ID {}", ebpf_seg_id);
    println!("  Program type:     XDP Distance Computation");
    println!("  Instructions:     {}", num_insns);
    println!("  BTF section:      {} bytes", btf_section.len());

    // ====================================================================
    // 4. Witness chain: the "trust chain" (10 entries)
    // ====================================================================
    println!("\n--- 4. Trust Chain (10-Entry Witness Chain) ---");

    let chain_steps = [
        ("genesis", 0x01u8),
        ("model_load", 0x02),
        ("data_ingest", 0x08),
        ("index_build", 0x02),
        ("kernel_embed", 0x02),
        ("ebpf_embed", 0x02),
        ("metadata_seal", 0x08),
        ("attestation", 0x05),
        ("verification", 0x02),
        ("release_seal", 0x01),
    ];

    let entries: Vec<WitnessEntry> = chain_steps
        .iter()
        .enumerate()
        .map(|(i, (step, wtype))| {
            let action_data = format!("sealed_engine:{}:step_{}", step, i);
            WitnessEntry {
                prev_hash: [0u8; 32],
                action_hash: shake256_256(action_data.as_bytes()),
                timestamp_ns: 1_700_000_000_000_000_000 + i as u64 * 1_000_000_000,
                witness_type: *wtype,
            }
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified_chain = verify_witness_chain(&chain_bytes).expect("chain verification failed");

    println!("  Chain entries:  {}", verified_chain.len());
    println!("  Chain size:     {} bytes", chain_bytes.len());
    println!("  Integrity:      VALID");

    println!("\n  Trust chain:");
    for (i, (step, _)) in chain_steps.iter().enumerate() {
        let wtype_name = match verified_chain[i].witness_type {
            0x01 => "PROV",
            0x02 => "COMP",
            0x05 => "ATTS",
            0x08 => "DATA",
            _ => "????",
        };
        println!("    [{:>4}] {} -> {}", wtype_name, i, step);
    }

    // ====================================================================
    // 5. Ed25519 signature: the "attestation"
    // ====================================================================
    println!("\n--- 5. Attestation (Ed25519 Signature) ---");

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    let mut header = SegmentHeader::new(SegmentType::Vec as u8, 1);
    header.timestamp_ns = 1_700_000_000_000_000_000;
    header.payload_length = 4096;

    let attestation_payload = b"Sealed cognitive engine: knowledge + runtime + accelerator";

    let footer = sign_segment(&header, attestation_payload, &signing_key);
    let sig_valid = verify_segment(&header, attestation_payload, &footer, &verifying_key);

    println!("  Signer:     {}...", hex_string(&verifying_key.to_bytes()[..16]));
    println!("  Signature:  {}...", hex_string(&footer.signature[..16]));
    println!("  Valid:       {}", sig_valid);
    assert!(sig_valid, "signature should be valid");

    // ====================================================================
    // 6. Derive snapshot: the "versioned release"
    // ====================================================================
    println!("\n--- 6. Versioned Release (Snapshot Derivation) ---");

    let snapshot_path = tmp_dir.path().join("engine_v1.0.rvf");
    let snapshot = store
        .derive(&snapshot_path, DerivationType::Snapshot, None)
        .expect("failed to derive snapshot");

    let parent_id = store.file_id();
    let snap_parent_id = snapshot.parent_id();

    println!("  Parent ID:      {}", hex_string(parent_id));
    println!("  Snapshot ID:    {}", hex_string(snapshot.file_id()));
    println!("  Parent match:   {}", parent_id == snap_parent_id);
    println!("  Lineage depth:  {}", snapshot.lineage_depth());

    assert_eq!(parent_id, snap_parent_id);
    assert_eq!(snapshot.lineage_depth(), 1);

    snapshot.close().expect("failed to close snapshot");

    // ====================================================================
    // 7. Verification of all components
    // ====================================================================
    println!("\n--- 7. Component Verification ---");

    // Verify kernel
    let (kh_bytes, ki_bytes) = store
        .extract_kernel()
        .expect("extract_kernel failed")
        .expect("no kernel found");
    let kh_arr: [u8; 128] = kh_bytes.try_into().unwrap();
    let kh = KernelHeader::from_bytes(&kh_arr).expect("invalid kernel header");
    assert_eq!(kh.kernel_magic, KERNEL_MAGIC);
    assert_eq!(kh.arch, KernelArch::X86_64 as u8);
    assert_eq!(kh.api_port, 9090);
    assert!(ki_bytes.starts_with(&kernel_image));
    println!("  Kernel:     VALID (magic={:#010X}, arch=x86_64, port=9090)", kh.kernel_magic);

    // Verify eBPF
    let (eh_bytes, ep_bytes) = store
        .extract_ebpf()
        .expect("extract_ebpf failed")
        .expect("no eBPF found");
    let eh_arr: [u8; 64] = eh_bytes.try_into().unwrap();
    let eh = EbpfHeader::from_bytes(&eh_arr).expect("invalid eBPF header");
    assert_eq!(eh.ebpf_magic, EBPF_MAGIC);
    assert_eq!(eh.program_type, EbpfProgramType::XdpDistance as u8);
    assert_eq!(eh.max_dimension, dim as u16);
    assert_eq!(&ep_bytes[..ebpf_bytecode.len()], ebpf_bytecode.as_slice());
    println!("  eBPF:       VALID (magic={:#010X}, type=XDP, dim={})", eh.ebpf_magic, eh.max_dimension);

    // Verify witness chain
    let re_verified = verify_witness_chain(&chain_bytes).expect("re-verify failed");
    assert_eq!(re_verified.len(), 10);
    println!("  Witness:    VALID ({} entries)", re_verified.len());

    // Verify signature
    assert!(sig_valid);
    println!("  Signature:  VALID (Ed25519)");

    // Verify lineage
    assert_eq!(store.lineage_depth(), 0);
    println!("  Lineage:    ROOT (depth=0)");

    // Verify queries still work
    let final_results = store
        .query(&query, 5, &QueryOptions::default())
        .expect("final query failed");
    assert_eq!(final_results[0].id, results[0].id);
    println!("  Queries:    VALID (results consistent)");

    // ====================================================================
    // 8. Sealed Engine Manifest
    // ====================================================================
    println!("\n--- 8. Sealed Engine Manifest ---");

    let final_status = store.status();

    println!("  +-------------------------------------------------+");
    println!("  |          SEALED COGNITIVE ENGINE v1.0             |");
    println!("  +-------------------------------------------------+");
    println!("  | Component        | Details                       |");
    println!("  |------------------|-------------------------------|");
    println!("  | Knowledge Base   | {} vectors x {} dims       |", final_status.total_vectors, dim);
    println!("  | Context          | {} categories, importance    |", categories.len());
    println!("  | Runtime          | HermitOS x86_64 ({} KB)     |", kernel_image.len() / 1024);
    println!("  | Accelerator      | XDP eBPF ({} insns)         |", num_insns);
    println!("  | Trust Chain      | {} witness entries           |", chain_steps.len());
    println!("  | Attestation      | Ed25519 signature            |");
    println!("  | Version          | Snapshot (depth=1)           |");
    println!("  | Total Segments   | {}                           |", final_status.total_segments);
    println!("  | File Size        | {} bytes                  |", final_status.file_size);
    println!("  | API Port         | 9090                         |");
    println!("  +-------------------------------------------------+");
    println!();
    println!("  This single .rvf file is a complete cognitive unit:");
    println!("    - Data:    embeddings + metadata for semantic search");
    println!("    - Runtime: boots as a microservice on port 9090");
    println!("    - Accel:   eBPF for kernel-level distance compute");
    println!("    - Trust:   cryptographic audit trail + signature");
    println!("    - Version: derived snapshot for reproducibility");

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
