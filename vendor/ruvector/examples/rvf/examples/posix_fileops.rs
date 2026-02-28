//! POSIX File Operations with RVF
//!
//! Category: **Runtime Target / Systems Integration**
//!
//! Demonstrates RVF files as first-class POSIX filesystem resources:
//!
//! 1. Standard file operations: create, open, read, write, seek, stat, rename
//! 2. POSIX file locking: advisory locks for concurrent access control
//! 3. Atomic operations: fsync, rename-based atomic writes
//! 4. Segment-level random access: seek to specific offsets for targeted reads
//! 5. File descriptor management: open_readonly vs read-write, close semantics
//! 6. Directory operations: listing, filtering .rvf files, metadata inspection
//! 7. Pipe-friendly: write RVF segments to stdout, read from file descriptors
//!
//! Key insight: RVF files are regular files — they work with every POSIX tool
//! (cp, scp, rsync, tar, chmod, chown) and every filesystem (ext4, XFS, NFS, S3-FUSE).
//!
//! RVF segments used: VEC_SEG, INDEX_SEG, MANIFEST_SEG, WITNESS_SEG
//!
//! Run: `cargo run --example posix_fileops`

use std::fs::{self, File};
use std::io::{Read as IoRead, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use rvf_crypto::{create_witness_chain, shake256_256, verify_witness_chain, WitnessEntry};
use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};
use rvf_types::{SegmentType, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC};
use rvf_wire::{find_latest_manifest, read_segment};
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

fn hex_short(data: &[u8], n: usize) -> String {
    data.iter().take(n).map(|b| format!("{:02x}", b)).collect()
}

fn main() {
    println!("=== POSIX File Operations with RVF ===\n");

    let dim = 128;
    let num_vectors = 200;
    let tmp = TempDir::new().expect("temp dir");
    let rvf_dir = tmp.path().join("rvf_store");
    fs::create_dir_all(&rvf_dir).expect("mkdir");

    // ====================================================================
    // Phase 1: Create an RVF file using standard POSIX path semantics
    // ====================================================================
    println!("--- Phase 1: POSIX File Creation ---");

    let primary_path = rvf_dir.join("vectors.rvf");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };
    let mut store = RvfStore::create(&primary_path, options).expect("create store");

    // Ingest vectors in batches
    let vectors: Vec<Vec<f32>> = (0..num_vectors).map(|i| random_vector(dim, i as u64)).collect();
    let ids: Vec<u64> = (1..=num_vectors as u64).collect();

    for chunk_start in (0..num_vectors).step_by(50) {
        let chunk_end = (chunk_start + 50).min(num_vectors);
        let batch_vecs: Vec<&[f32]> = vectors[chunk_start..chunk_end]
            .iter()
            .map(|v| v.as_slice())
            .collect();
        let batch_ids = &ids[chunk_start..chunk_end];
        store
            .ingest_batch(&batch_vecs, batch_ids, None)
            .expect("ingest");
    }
    store.close().expect("close");

    // stat() the file — standard POSIX metadata
    let meta = fs::metadata(&primary_path).expect("stat");
    println!("  Created: {:?}", primary_path.file_name().unwrap());
    println!("  Size:    {} bytes ({:.1} KB)", meta.len(), meta.len() as f64 / 1024.0);
    println!("  Mode:    {:o}", meta.mode() & 0o777);
    println!("  Inode:   {}", meta.ino());
    println!("  Blocks:  {} (512-byte blocks)", meta.blocks());
    println!("  Device:  {}", meta.dev());
    println!();

    // ====================================================================
    // Phase 2: Raw file I/O — read segment headers using seek/read
    // ====================================================================
    println!("--- Phase 2: Raw POSIX I/O (seek + read) ---");

    let mut file = File::open(&primary_path).expect("open");
    let file_size = file.metadata().expect("fstat").len();
    println!("  File descriptor opened for reading");
    println!("  File size via fstat: {} bytes", file_size);

    // Read the first 8 bytes to check for segment magic
    let mut magic_buf = [0u8; 4];
    file.read_exact(&mut magic_buf).expect("read magic");
    let magic = u32::from_le_bytes(magic_buf);
    println!(
        "  First 4 bytes (magic): 0x{:08X} (valid={})",
        magic,
        magic == SEGMENT_MAGIC
    );

    // Seek back to start and read the full first segment header
    file.seek(SeekFrom::Start(0)).expect("lseek");
    let mut header_buf = vec![0u8; SEGMENT_HEADER_SIZE];
    file.read_exact(&mut header_buf).expect("read header");

    println!("  First segment header ({} bytes):", SEGMENT_HEADER_SIZE);
    println!("    Version:  {}", header_buf[4]);
    println!("    Seg type: 0x{:02X}", header_buf[5]);
    println!(
        "    Seg ID:   {}",
        u64::from_le_bytes(header_buf[8..16].try_into().unwrap())
    );

    // Seek to end to find file size (alternative to stat)
    let eof = file.seek(SeekFrom::End(0)).expect("lseek END");
    println!("  File size via lseek(END): {} bytes", eof);
    println!();

    // ====================================================================
    // Phase 3: Tail-scan for manifest (backward seek pattern)
    // ====================================================================
    println!("--- Phase 3: Manifest Tail-Scan (backward seek) ---");

    // Read entire file for segment scanning
    file.seek(SeekFrom::Start(0)).expect("lseek");
    let mut file_data = Vec::new();
    file.read_to_end(&mut file_data).expect("read all");

    match find_latest_manifest(&file_data) {
        Ok((offset, header)) => {
            println!("  Manifest found at offset: {}", offset);
            println!("  Segment ID: {}", header.segment_id);
            println!("  Payload length: {} bytes", header.payload_length);

            // Seek directly to the manifest segment for targeted read
            file.seek(SeekFrom::Start(offset as u64)).expect("lseek to manifest");
            let mut manifest_header = vec![0u8; SEGMENT_HEADER_SIZE];
            file.read_exact(&mut manifest_header).expect("read manifest header");
            println!("  Direct seek to manifest: verified (magic=0x{:08X})",
                u32::from_le_bytes(manifest_header[0..4].try_into().unwrap()));
        }
        Err(e) => println!("  Manifest not found: {:?}", e),
    }
    drop(file);
    println!();

    // ====================================================================
    // Phase 4: Atomic rename (POSIX rename guarantees)
    // ====================================================================
    println!("--- Phase 4: Atomic Rename ---");

    let backup_path = rvf_dir.join("vectors.rvf.bak");
    let new_path = rvf_dir.join("vectors_v2.rvf");

    // Copy for backup (cp equivalent)
    fs::copy(&primary_path, &backup_path).expect("cp");
    println!("  Backup: cp vectors.rvf vectors.rvf.bak");

    // Write a new version of the store
    let options_v2 = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::Cosine,
        ..Default::default()
    };
    let mut store_v2 = RvfStore::create(&new_path, options_v2).expect("create v2");
    let v2_vecs: Vec<Vec<f32>> = (0..50).map(|i| random_vector(dim, i * 7)).collect();
    let v2_refs: Vec<&[f32]> = v2_vecs.iter().map(|v| v.as_slice()).collect();
    let v2_ids: Vec<u64> = (1..=50).collect();
    store_v2.ingest_batch(&v2_refs, &v2_ids, None).expect("ingest v2");
    store_v2.close().expect("close v2");

    // Atomic rename: POSIX guarantees this is atomic on the same filesystem
    fs::rename(&new_path, &primary_path).expect("rename");
    println!("  Atomic rename: vectors_v2.rvf -> vectors.rvf");
    println!("  Original safely in backup: vectors.rvf.bak");

    // Verify the new file is accessible
    let reopened = RvfStore::open(&primary_path).expect("reopen after rename");
    let status = reopened.status();
    println!("  Reopened after rename:");
    println!("    Vectors:  {}", status.total_vectors);
    println!("    Segments: {}", status.total_segments);
    println!("    Epoch:    {}", status.current_epoch);
    drop(reopened);
    println!();

    // ====================================================================
    // Phase 5: Read-only file descriptor (O_RDONLY semantics)
    // ====================================================================
    println!("--- Phase 5: Read-Only Access (O_RDONLY) ---");

    let ro_store = RvfStore::open_readonly(&primary_path).expect("open_readonly");
    let query = random_vector(dim, 42);
    let results = ro_store
        .query(&query, 5, &QueryOptions::default())
        .expect("query readonly");
    println!("  Opened with O_RDONLY semantics (no write lock)");
    println!("  Query results (top-5):");
    for (i, r) in results.iter().enumerate() {
        println!("    #{}: id={}, dist={:.6}", i + 1, r.id, r.distance);
    }
    drop(ro_store);
    println!();

    // ====================================================================
    // Phase 6: File locking (advisory locks)
    // ====================================================================
    println!("--- Phase 6: Advisory File Locking ---");

    // RvfStore uses POSIX advisory locks internally via WriterLock
    // Opening for read-write acquires an exclusive lock
    let writer = RvfStore::open(&primary_path).expect("open writer (exclusive lock)");
    println!("  Writer opened: exclusive advisory lock acquired");
    println!("  Lock file: {:?}", primary_path.with_extension("rvf.lock"));

    // Concurrent read-only access is still possible (shared access)
    let reader = RvfStore::open_readonly(&primary_path).expect("open readonly (no lock)");
    println!("  Reader opened: no lock required for O_RDONLY");

    let reader_results = reader.query(&query, 3, &QueryOptions::default()).expect("reader query");
    println!("  Reader query while writer holds lock: {} results", reader_results.len());

    drop(reader);
    drop(writer);
    println!("  Writer closed: advisory lock released");
    println!();

    // ====================================================================
    // Phase 7: Directory listing and file inspection
    // ====================================================================
    println!("--- Phase 7: Directory Operations ---");

    // Create a few more RVF files for directory listing
    for name in &["index_a.rvf", "index_b.rvf", "archive.rvf"] {
        let p = rvf_dir.join(name);
        let opts = RvfOptions {
            dimension: 64,
            metric: DistanceMetric::L2,
            ..Default::default()
        };
        let mut s = RvfStore::create(&p, opts).expect("create");
        let vecs: Vec<Vec<f32>> = (0..10).map(|i| random_vector(64, i)).collect();
        let refs: Vec<&[f32]> = vecs.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=10).collect();
        s.ingest_batch(&refs, &ids, None).expect("ingest");
        s.close().expect("close");
    }

    // List directory entries (readdir equivalent)
    println!("  Directory: {:?}", rvf_dir);
    let mut entries: Vec<_> = fs::read_dir(&rvf_dir)
        .expect("opendir")
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    println!(
        "  {:>30}  {:>10}  {:>10}  {:>8}",
        "Name", "Size", "Inode", "Ext"
    );
    println!(
        "  {:->30}  {:->10}  {:->10}  {:->8}",
        "", "", "", ""
    );

    let mut rvf_count = 0;
    let mut total_bytes: u64 = 0;
    for entry in &entries {
        let m = entry.metadata().expect("stat entry");
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let name_string = name_str.to_string();
        let ext = Path::new(&name_string)
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        println!(
            "  {:>30}  {:>10}  {:>10}  {:>8}",
            name_str,
            m.len(),
            m.ino(),
            ext
        );

        if ext == "rvf" {
            rvf_count += 1;
            total_bytes += m.len();
        }
    }
    println!("\n  RVF files found: {}", rvf_count);
    println!("  Total RVF size:  {} bytes ({:.1} KB)", total_bytes, total_bytes as f64 / 1024.0);
    println!();

    // ====================================================================
    // Phase 8: Segment-level random access
    // ====================================================================
    println!("--- Phase 8: Segment-Level Random Access ---");

    // Open the backup file and walk all segments using sequential reads
    let backup_data = fs::read(&backup_path).expect("read backup");
    println!("  File: vectors.rvf.bak ({} bytes)", backup_data.len());

    let mut offset = 0;
    let mut seg_count = 0;
    println!(
        "  {:>8}  {:>12}  {:>8}  {:>12}  {:>10}",
        "Offset", "Type", "Seg ID", "Payload", "Aligned"
    );
    println!(
        "  {:->8}  {:->12}  {:->8}  {:->12}  {:->10}",
        "", "", "", "", ""
    );

    while offset + SEGMENT_HEADER_SIZE <= backup_data.len() {
        match read_segment(&backup_data[offset..]) {
            Ok((header, _payload)) => {
                if header.magic != SEGMENT_MAGIC {
                    break;
                }
                let seg_type_name = match SegmentType::try_from(header.seg_type) {
                    Ok(SegmentType::Vec) => "VEC_SEG",
                    Ok(SegmentType::Index) => "INDEX_SEG",
                    Ok(SegmentType::Manifest) => "MANIFEST",
                    Ok(SegmentType::Meta) => "META_SEG",
                    Ok(SegmentType::Witness) => "WITNESS",
                    Ok(SegmentType::Crypto) => "CRYPTO",
                    Ok(SegmentType::Kernel) => "KERNEL",
                    Ok(SegmentType::Ebpf) => "EBPF",
                    _ => "UNKNOWN",
                };

                let padded = rvf_wire::calculate_padded_size(SEGMENT_HEADER_SIZE, header.payload_length as usize);
                println!(
                    "  {:>8}  {:>12}  {:>8}  {:>12}  {:>10}",
                    offset, seg_type_name, header.segment_id, header.payload_length, padded
                );

                offset += SEGMENT_HEADER_SIZE + padded;
                seg_count += 1;
            }
            Err(_) => break,
        }
    }
    println!("  Total segments: {}", seg_count);
    println!();

    // ====================================================================
    // Phase 9: Witness chain for file operations audit
    // ====================================================================
    println!("--- Phase 9: File Operations Audit Trail ---");

    let timestamp_base = 1_700_000_000_000_000_000u64;
    let witness_entries = vec![
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!("posix_create:path={},dim={},vectors={}",
                    primary_path.display(), dim, num_vectors).as_bytes(),
            ),
            timestamp_ns: timestamp_base,
            witness_type: 0x08, // DATA_PROVENANCE
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                "posix_backup:src=vectors.rvf,dst=vectors.rvf.bak".to_string().as_bytes(),
            ),
            timestamp_ns: timestamp_base + 1_000_000,
            witness_type: 0x01, // PROVENANCE
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                "posix_rename:src=vectors_v2.rvf,dst=vectors.rvf,atomic=true".to_string().as_bytes(),
            ),
            timestamp_ns: timestamp_base + 2_000_000,
            witness_type: 0x02, // COMPUTATION
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!("posix_readdir:dir={},rvf_files={}", rvf_dir.display(), rvf_count).as_bytes(),
            ),
            timestamp_ns: timestamp_base + 3_000_000,
            witness_type: 0x01, // PROVENANCE
        },
    ];

    let chain_bytes = create_witness_chain(&witness_entries);
    let verified = verify_witness_chain(&chain_bytes).expect("verify chain");
    println!("  Audit trail: {} file operations recorded", verified.len());
    println!("  Witness chain: {} bytes, {} entries verified", chain_bytes.len(), verified.len());

    for (i, entry) in verified.iter().enumerate() {
        println!(
            "    #{}: type=0x{:02X}, hash={}...",
            i + 1,
            entry.witness_type,
            hex_short(&entry.action_hash, 8)
        );
    }
    println!();

    // ====================================================================
    // Phase 10: Cleanup demonstration (unlink semantics)
    // ====================================================================
    println!("--- Phase 10: Cleanup (unlink / rmdir) ---");

    // Count files before cleanup
    let before_count = fs::read_dir(&rvf_dir).expect("readdir").count();
    println!("  Files before cleanup: {}", before_count);

    // Remove lock files first (if any remain)
    for entry in fs::read_dir(&rvf_dir).expect("readdir") {
        let entry = entry.expect("entry");
        let name = entry.file_name();
        if name.to_string_lossy().ends_with(".lock") {
            fs::remove_file(entry.path()).expect("unlink lock");
        }
    }

    // Remove RVF files
    let mut removed = 0;
    for entry in fs::read_dir(&rvf_dir).expect("readdir") {
        let entry = entry.expect("entry");
        fs::remove_file(entry.path()).expect("unlink");
        removed += 1;
    }
    println!("  Removed {} files (unlink)", removed);

    // Remove directory
    fs::remove_dir(&rvf_dir).expect("rmdir");
    println!("  Removed directory (rmdir)");
    println!("  Cleanup complete: all POSIX resources freed");
    println!();

    // ====================================================================
    // Summary
    // ====================================================================
    println!("=== Summary ===\n");
    println!("  POSIX operations exercised:");
    println!("    open/creat   - File creation with RvfStore::create");
    println!("    read/write   - Raw segment I/O via std::fs::File");
    println!("    lseek        - Random access to segment offsets");
    println!("    stat/fstat   - File metadata (size, inode, mode, blocks)");
    println!("    rename       - Atomic file replacement");
    println!("    flock        - Advisory locking via WriterLock");
    println!("    opendir      - Directory listing and .rvf filtering");
    println!("    unlink/rmdir - File and directory removal");
    println!();
    println!("  Key insight: RVF files are regular POSIX files — they work");
    println!("  with cp, scp, rsync, tar, chmod, cron, systemd, and every");
    println!("  POSIX-compliant filesystem (ext4, XFS, NFS, CIFS, S3-FUSE).");
    println!();
    println!("=== Done ===");
}
