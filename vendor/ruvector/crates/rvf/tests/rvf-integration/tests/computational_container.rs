//! Integration tests for the RVF computational container segments:
//! KERNEL_SEG (0x0E) and EBPF_SEG (0x0F).
//!
//! These tests exercise the raw binary format for embedded kernel images and
//! eBPF programs within RVF files. Because the high-level kernel/eBPF APIs
//! may not exist yet (other agents may be creating them), all tests construct
//! segment headers and payloads via raw byte manipulation. This ensures the
//! tests work regardless of whether typed wrappers are available.
//!
//! Wire format references:
//! - KERNEL_SEG segment type: 0x0E (SegmentType::Kernel)
//! - EBPF_SEG segment type:   0x0F (SegmentType::Ebpf)
//! - Segment header:          64 bytes (SEGMENT_HEADER_SIZE)
//! - KernelHeader payload:    128 bytes (magic 0x52564B4E = "RVKN")
//! - EbpfHeader payload:      64 bytes  (magic 0x52564250 = "RVBP")

use rvf_runtime::options::{DistanceMetric, RvfOptions};
use rvf_runtime::RvfStore;
use rvf_types::{SegmentFlags, SegmentType, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC, SEGMENT_VERSION};
use rvf_wire::{read_segment, validate_segment, write_segment};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Constants for the computational container sub-headers
// ---------------------------------------------------------------------------

/// KernelHeader magic: "RVKN" as big-endian u32 => 0x52564B4E.
const KERNEL_MAGIC: u32 = 0x5256_4B4E;

/// EbpfHeader magic: "RVBP" as big-endian u32 => 0x52564250.
const EBPF_MAGIC: u32 = 0x5256_4250;

/// Size of the KernelHeader in bytes.
const KERNEL_HEADER_SIZE: usize = 128;

/// Size of the EbpfHeader in bytes.
const EBPF_HEADER_SIZE: usize = 64;

/// Architecture discriminants for KernelHeader.arch field.
const ARCH_X86_64: u8 = 0x00;
const ARCH_AARCH64: u8 = 0x01;

/// Kernel type discriminants for KernelHeader.kernel_type field.
const KERNEL_TYPE_UNIKERNEL: u8 = 0x00;
const KERNEL_TYPE_TEST_STUB: u8 = 0xFD;

/// Kernel flags (stored in a u32 at offset 8 of the KernelHeader).
const KERNEL_FLAG_SIGNED: u32 = 0x0000_0001;
const KERNEL_FLAG_REQUIRES_TEE: u32 = 0x0000_0002;
const KERNEL_FLAG_READ_ONLY: u32 = 0x0000_0004;
const KERNEL_FLAG_INGEST_ENABLED: u32 = 0x0000_0008;

// ---------------------------------------------------------------------------
// Helper: construct a 128-byte KernelHeader payload
// ---------------------------------------------------------------------------

/// Build a 128-byte KernelHeader with the given parameters.
///
/// Layout (all little-endian):
///   [0..4]    magic:          u32 = 0x52564B4E
///   [4..6]    version:        u16
///   [6]       arch:           u8
///   [7]       kernel_type:    u8
///   [8..12]   flags:          u32
///   [12..16]  entry_point:    u32
///   [16..24]  image_size:     u64
///   [24..28]  bss_size:       u32
///   [28..30]  stack_pages:    u16
///   [30..32]  max_dimension:  u16
///   [32..64]  image_hash:     [u8; 32] (SHAKE-256-256 of the image bytes)
///   [64..80]  reserved_0:     [u8; 16]
///   [80..128] reserved_1:     [u8; 48]
fn make_kernel_header(
    arch: u8,
    kernel_type: u8,
    flags: u32,
    entry_point: u32,
    image_size: u64,
    bss_size: u32,
    stack_pages: u16,
    max_dimension: u16,
    image_hash: [u8; 32],
) -> [u8; KERNEL_HEADER_SIZE] {
    let mut buf = [0u8; KERNEL_HEADER_SIZE];

    // magic
    buf[0..4].copy_from_slice(&KERNEL_MAGIC.to_le_bytes());
    // version
    buf[4..6].copy_from_slice(&1u16.to_le_bytes());
    // arch
    buf[6] = arch;
    // kernel_type
    buf[7] = kernel_type;
    // flags
    buf[8..12].copy_from_slice(&flags.to_le_bytes());
    // entry_point
    buf[12..16].copy_from_slice(&entry_point.to_le_bytes());
    // image_size
    buf[16..24].copy_from_slice(&image_size.to_le_bytes());
    // bss_size
    buf[24..28].copy_from_slice(&bss_size.to_le_bytes());
    // stack_pages
    buf[28..30].copy_from_slice(&stack_pages.to_le_bytes());
    // max_dimension
    buf[30..32].copy_from_slice(&max_dimension.to_le_bytes());
    // image_hash
    buf[32..64].copy_from_slice(&image_hash);
    // reserved fields stay zeroed

    buf
}

// ---------------------------------------------------------------------------
// Helper: construct a 64-byte EbpfHeader payload
// ---------------------------------------------------------------------------

/// Build a 64-byte EbpfHeader with the given parameters.
///
/// Layout (all little-endian):
///   [0..4]    magic:          u32 = 0x52564250
///   [4..6]    version:        u16
///   [6]       program_type:   u8
///   [7]       attach_point:   u8
///   [8..12]   flags:          u32
///   [12..16]  insn_count:     u32
///   [16..20]  map_count:      u32
///   [20..22]  max_dimension:  u16
///   [22..24]  reserved_0:     u16
///   [24..32]  program_hash:   [u8; 8] (truncated hash of bytecode)
///   [32..64]  reserved_1:     [u8; 32]
fn make_ebpf_header(
    program_type: u8,
    attach_point: u8,
    flags: u32,
    insn_count: u32,
    map_count: u32,
    max_dimension: u16,
    program_hash: [u8; 8],
) -> [u8; EBPF_HEADER_SIZE] {
    let mut buf = [0u8; EBPF_HEADER_SIZE];

    // magic
    buf[0..4].copy_from_slice(&EBPF_MAGIC.to_le_bytes());
    // version
    buf[4..6].copy_from_slice(&1u16.to_le_bytes());
    // program_type
    buf[6] = program_type;
    // attach_point
    buf[7] = attach_point;
    // flags
    buf[8..12].copy_from_slice(&flags.to_le_bytes());
    // insn_count
    buf[12..16].copy_from_slice(&insn_count.to_le_bytes());
    // map_count
    buf[16..20].copy_from_slice(&map_count.to_le_bytes());
    // max_dimension
    buf[20..22].copy_from_slice(&max_dimension.to_le_bytes());
    // reserved_0
    buf[22..24].copy_from_slice(&0u16.to_le_bytes());
    // program_hash
    buf[24..32].copy_from_slice(&program_hash);
    // reserved_1 stays zeroed

    buf
}

// ---------------------------------------------------------------------------
// Helper: build a raw 64-byte RVF segment header
// ---------------------------------------------------------------------------

fn build_raw_segment_header(
    seg_type: u8,
    seg_id: u64,
    payload_len: u64,
) -> [u8; SEGMENT_HEADER_SIZE] {
    let mut buf = [0u8; SEGMENT_HEADER_SIZE];
    buf[0x00..0x04].copy_from_slice(&SEGMENT_MAGIC.to_le_bytes());
    buf[0x04] = SEGMENT_VERSION;
    buf[0x05] = seg_type;
    // flags at 0x06..0x08 stay zero
    buf[0x08..0x10].copy_from_slice(&seg_id.to_le_bytes());
    buf[0x10..0x18].copy_from_slice(&payload_len.to_le_bytes());
    buf
}

// ---------------------------------------------------------------------------
// Helper: simple hash for testing (non-cryptographic)
// ---------------------------------------------------------------------------

/// A simple deterministic hash for testing purposes. Produces a 32-byte digest.
fn simple_test_hash(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, &b) in data.iter().enumerate() {
        out[i % 32] = out[i % 32].wrapping_add(b);
        let j = (i + 13) % 32;
        out[j] = out[j].wrapping_add(out[i % 32].rotate_left(3));
    }
    out
}

// ---------------------------------------------------------------------------
// Helper: read entire file into bytes
// ---------------------------------------------------------------------------

fn read_file_bytes(path: &std::path::Path) -> Vec<u8> {
    let mut file = OpenOptions::new().read(true).open(path).unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    buf
}

// ---------------------------------------------------------------------------
// Helper: scan file for segment headers, return (offset, type, id, payload_len)
// ---------------------------------------------------------------------------

fn scan_segments(file_bytes: &[u8]) -> Vec<(usize, u8, u64, u64)> {
    let magic_bytes = SEGMENT_MAGIC.to_le_bytes();
    let mut segments = Vec::new();

    if file_bytes.len() < SEGMENT_HEADER_SIZE {
        return segments;
    }

    let last_possible = file_bytes.len() - SEGMENT_HEADER_SIZE;
    for i in 0..=last_possible {
        if file_bytes[i..i + 4] == magic_bytes {
            let seg_type = file_bytes[i + 5];
            let seg_id = u64::from_le_bytes(file_bytes[i + 0x08..i + 0x10].try_into().unwrap());
            let payload_len =
                u64::from_le_bytes(file_bytes[i + 0x10..i + 0x18].try_into().unwrap());
            segments.push((i, seg_type, seg_id, payload_len));
        }
    }

    segments
}

// ---------------------------------------------------------------------------
// Helper: make RvfStore options
// ---------------------------------------------------------------------------

fn make_options(dim: u16) -> RvfOptions {
    RvfOptions {
        dimension: dim,
        metric: DistanceMetric::L2,
        ..Default::default()
    }
}

// ===========================================================================
// TEST 1: kernel_header_round_trip
// ===========================================================================

/// Construct a 128-byte KernelHeader, wrap it in a KERNEL_SEG (type 0x0E)
/// using the rvf-wire writer, read it back, and verify all fields match.
#[test]
fn kernel_header_round_trip() {
    let image_hash = simple_test_hash(b"test kernel image bytes");
    let kernel_hdr = make_kernel_header(
        ARCH_X86_64,                                // arch
        KERNEL_TYPE_UNIKERNEL,                      // kernel_type
        KERNEL_FLAG_SIGNED | KERNEL_FLAG_READ_ONLY, // flags
        0x0000_1000,                                // entry_point
        4096,                                       // image_size
        512,                                        // bss_size
        4,                                          // stack_pages
        256,                                        // max_dimension
        image_hash,
    );

    // Write as a KERNEL_SEG using rvf-wire
    let seg_flags = SegmentFlags::empty();
    let encoded = write_segment(
        SegmentType::Kernel as u8,
        &kernel_hdr,
        seg_flags,
        100, // segment_id
    );

    // Read back the RVF segment
    let (header, payload) = read_segment(&encoded).unwrap();

    // Verify outer segment header
    assert_eq!(header.magic, SEGMENT_MAGIC, "segment magic mismatch");
    assert_eq!(header.version, SEGMENT_VERSION, "segment version mismatch");
    assert_eq!(
        header.seg_type,
        SegmentType::Kernel as u8,
        "segment type should be Kernel (0x0E)"
    );
    assert_eq!(header.segment_id, 100, "segment_id mismatch");
    assert_eq!(
        header.payload_length, KERNEL_HEADER_SIZE as u64,
        "payload length mismatch"
    );

    // Validate content hash
    validate_segment(&header, payload).expect("content hash validation should pass");

    // Verify inner KernelHeader fields
    assert_eq!(
        payload.len(),
        KERNEL_HEADER_SIZE,
        "kernel header payload size"
    );

    let magic = u32::from_le_bytes(payload[0..4].try_into().unwrap());
    assert_eq!(magic, KERNEL_MAGIC, "kernel magic mismatch");

    let version = u16::from_le_bytes(payload[4..6].try_into().unwrap());
    assert_eq!(version, 1, "kernel version mismatch");

    assert_eq!(payload[6], ARCH_X86_64, "arch mismatch");
    assert_eq!(payload[7], KERNEL_TYPE_UNIKERNEL, "kernel_type mismatch");

    let flags = u32::from_le_bytes(payload[8..12].try_into().unwrap());
    assert_eq!(
        flags,
        KERNEL_FLAG_SIGNED | KERNEL_FLAG_READ_ONLY,
        "kernel flags mismatch"
    );

    let entry_point = u32::from_le_bytes(payload[12..16].try_into().unwrap());
    assert_eq!(entry_point, 0x0000_1000, "entry_point mismatch");

    let image_size = u64::from_le_bytes(payload[16..24].try_into().unwrap());
    assert_eq!(image_size, 4096, "image_size mismatch");

    let bss_size = u32::from_le_bytes(payload[24..28].try_into().unwrap());
    assert_eq!(bss_size, 512, "bss_size mismatch");

    let stack_pages = u16::from_le_bytes(payload[28..30].try_into().unwrap());
    assert_eq!(stack_pages, 4, "stack_pages mismatch");

    let max_dimension = u16::from_le_bytes(payload[30..32].try_into().unwrap());
    assert_eq!(max_dimension, 256, "max_dimension mismatch");

    let mut read_hash = [0u8; 32];
    read_hash.copy_from_slice(&payload[32..64]);
    assert_eq!(read_hash, image_hash, "image_hash mismatch");

    println!("PASS: kernel_header_round_trip -- all fields verified");
}

// ===========================================================================
// TEST 2: ebpf_header_round_trip
// ===========================================================================

/// Construct a 64-byte EbpfHeader, wrap it in an EBPF_SEG (type 0x0F)
/// using the rvf-wire writer, read it back, and verify all fields match.
#[test]
fn ebpf_header_round_trip() {
    let program_hash: [u8; 8] = [0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
    let ebpf_hdr = make_ebpf_header(
        0x01,   // program_type: filter
        0x02,   // attach_point: ingress
        0x0003, // flags
        256,    // insn_count
        4,      // map_count
        128,    // max_dimension
        program_hash,
    );

    // Write as an EBPF_SEG
    let encoded = write_segment(
        SegmentType::Ebpf as u8,
        &ebpf_hdr,
        SegmentFlags::empty(),
        200,
    );

    // Read back
    let (header, payload) = read_segment(&encoded).unwrap();

    // Verify outer segment header
    assert_eq!(
        header.seg_type,
        SegmentType::Ebpf as u8,
        "segment type should be Ebpf (0x0F)"
    );
    assert_eq!(header.segment_id, 200);
    assert_eq!(header.payload_length, EBPF_HEADER_SIZE as u64);

    // Validate hash
    validate_segment(&header, payload).expect("ebpf content hash should validate");

    // Verify inner EbpfHeader fields
    assert_eq!(payload.len(), EBPF_HEADER_SIZE);

    let magic = u32::from_le_bytes(payload[0..4].try_into().unwrap());
    assert_eq!(magic, EBPF_MAGIC, "ebpf magic mismatch");

    let version = u16::from_le_bytes(payload[4..6].try_into().unwrap());
    assert_eq!(version, 1);

    assert_eq!(payload[6], 0x01, "program_type mismatch");
    assert_eq!(payload[7], 0x02, "attach_point mismatch");

    let flags = u32::from_le_bytes(payload[8..12].try_into().unwrap());
    assert_eq!(flags, 0x0003, "ebpf flags mismatch");

    let insn_count = u32::from_le_bytes(payload[12..16].try_into().unwrap());
    assert_eq!(insn_count, 256, "insn_count mismatch");

    let map_count = u32::from_le_bytes(payload[16..20].try_into().unwrap());
    assert_eq!(map_count, 4, "map_count mismatch");

    let max_dim = u16::from_le_bytes(payload[20..22].try_into().unwrap());
    assert_eq!(max_dim, 128, "max_dimension mismatch");

    let mut read_hash = [0u8; 8];
    read_hash.copy_from_slice(&payload[24..32]);
    assert_eq!(read_hash, program_hash, "program_hash mismatch");

    println!("PASS: ebpf_header_round_trip -- all fields verified");
}

// ===========================================================================
// TEST 3: kernel_segment_survives_store_reopen
// ===========================================================================

/// Create an RVF store, add vectors, manually append a fake KERNEL_SEG
/// (type 0x0E) to the file, close and reopen the store, then verify the
/// kernel segment is still present when scanning the raw file bytes.
#[test]
fn kernel_segment_survives_store_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("kernel_reopen.rvf");
    let dim: u16 = 4;

    // Step 1: Create a store with some vectors
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let vectors: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; dim as usize]).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=10).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    // Step 2: Manually append a KERNEL_SEG
    let kernel_payload = make_kernel_header(
        ARCH_X86_64,
        KERNEL_TYPE_TEST_STUB,
        KERNEL_FLAG_INGEST_ENABLED,
        0x0000_2000,
        8192,
        1024,
        8,
        512,
        [0xAA; 32],
    );
    let kernel_seg_id: u64 = 5000;
    {
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        let seg_header = build_raw_segment_header(
            SegmentType::Kernel as u8,
            kernel_seg_id,
            kernel_payload.len() as u64,
        );
        file.write_all(&seg_header).unwrap();
        file.write_all(&kernel_payload).unwrap();
        file.sync_all().unwrap();
    }

    // Step 3: Verify the kernel segment is in the file
    let bytes_before = read_file_bytes(&path);
    let segs_before = scan_segments(&bytes_before);
    let kernel_segs_before: Vec<_> = segs_before
        .iter()
        .filter(|s| s.1 == SegmentType::Kernel as u8)
        .collect();
    assert_eq!(
        kernel_segs_before.len(),
        1,
        "expected 1 KERNEL_SEG before reopen, found {}",
        kernel_segs_before.len()
    );
    assert_eq!(
        kernel_segs_before[0].2, kernel_seg_id,
        "segment ID mismatch before reopen"
    );

    // Step 4: Reopen the store (readonly) -- should not panic
    let store = RvfStore::open_readonly(&path).unwrap();
    assert_eq!(
        store.status().total_vectors,
        10,
        "store should still report 10 vectors after reopen with kernel segment"
    );

    // Step 5: Verify the kernel segment is still present in the raw file
    let bytes_after = read_file_bytes(&path);
    let segs_after = scan_segments(&bytes_after);
    let kernel_segs_after: Vec<_> = segs_after
        .iter()
        .filter(|s| s.1 == SegmentType::Kernel as u8)
        .collect();
    assert_eq!(
        kernel_segs_after.len(),
        1,
        "KERNEL_SEG should still be present after store reopen, found {}",
        kernel_segs_after.len()
    );
    assert_eq!(
        kernel_segs_after[0].2, kernel_seg_id,
        "segment ID mismatch after reopen"
    );

    // Verify the payload is intact
    let offset = kernel_segs_after[0].0;
    let payload_start = offset + SEGMENT_HEADER_SIZE;
    let payload_end = payload_start + KERNEL_HEADER_SIZE;
    assert!(
        bytes_after.len() >= payload_end,
        "file too short to contain kernel payload"
    );
    assert_eq!(
        &bytes_after[payload_start..payload_end],
        &kernel_payload[..],
        "kernel payload bytes should be preserved after reopen"
    );

    println!("PASS: kernel_segment_survives_store_reopen");
}

// ===========================================================================
// TEST 4: multi_arch_kernel_segments
// ===========================================================================

/// Create an RVF file with two KERNEL_SEGs: one for x86_64 (arch=0) and
/// one for aarch64 (arch=1). Verify both are present and distinguishable.
#[test]
fn multi_arch_kernel_segments() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("multi_arch.rvf");
    let dim: u16 = 4;

    // Create a store with some vectors
    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        let vectors: Vec<Vec<f32>> = (0..5).map(|i| vec![i as f32; dim as usize]).collect();
        let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (1..=5).collect();
        store.ingest_batch(&refs, &ids, None).unwrap();
        store.close().unwrap();
    }

    // Append two KERNEL_SEGs with different architectures
    let x86_kernel = make_kernel_header(
        ARCH_X86_64,
        KERNEL_TYPE_UNIKERNEL,
        0,
        0x1000,
        4096,
        256,
        2,
        128,
        [0x11; 32],
    );
    let arm_kernel = make_kernel_header(
        ARCH_AARCH64,
        KERNEL_TYPE_UNIKERNEL,
        0,
        0x2000,
        8192,
        512,
        4,
        256,
        [0x22; 32],
    );

    {
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();

        // x86_64 kernel
        let h1 = build_raw_segment_header(SegmentType::Kernel as u8, 6001, x86_kernel.len() as u64);
        file.write_all(&h1).unwrap();
        file.write_all(&x86_kernel).unwrap();

        // aarch64 kernel
        let h2 = build_raw_segment_header(SegmentType::Kernel as u8, 6002, arm_kernel.len() as u64);
        file.write_all(&h2).unwrap();
        file.write_all(&arm_kernel).unwrap();

        file.sync_all().unwrap();
    }

    // Scan the file for KERNEL_SEGs
    let bytes = read_file_bytes(&path);
    let segs = scan_segments(&bytes);
    let kernel_segs: Vec<_> = segs
        .iter()
        .filter(|s| s.1 == SegmentType::Kernel as u8)
        .collect();

    assert_eq!(
        kernel_segs.len(),
        2,
        "expected 2 KERNEL_SEGs (x86_64 + aarch64), found {}",
        kernel_segs.len()
    );

    // Extract and verify architectures
    let mut archs = Vec::new();
    for &(offset, _, seg_id, _) in &kernel_segs {
        let payload_start = offset + SEGMENT_HEADER_SIZE;
        let arch_byte = bytes[payload_start + 6]; // arch is at offset 6 in KernelHeader
        archs.push((seg_id, arch_byte));
        println!("  KERNEL_SEG id={} arch=0x{:02X}", seg_id, arch_byte);
    }

    // One should be x86_64 (0x00), the other aarch64 (0x01)
    let has_x86 = archs.iter().any(|&(_, a)| a == ARCH_X86_64);
    let has_arm = archs.iter().any(|&(_, a)| a == ARCH_AARCH64);
    assert!(has_x86, "should have an x86_64 KERNEL_SEG");
    assert!(has_arm, "should have an aarch64 KERNEL_SEG");

    // Verify entry points are different
    let x86_entry = {
        let &(off, _, _, _) = kernel_segs
            .iter()
            .find(|s| bytes[s.0 + SEGMENT_HEADER_SIZE + 6] == ARCH_X86_64)
            .unwrap();
        u32::from_le_bytes(
            bytes[off + SEGMENT_HEADER_SIZE + 12..off + SEGMENT_HEADER_SIZE + 16]
                .try_into()
                .unwrap(),
        )
    };
    let arm_entry = {
        let &(off, _, _, _) = kernel_segs
            .iter()
            .find(|s| bytes[s.0 + SEGMENT_HEADER_SIZE + 6] == ARCH_AARCH64)
            .unwrap();
        u32::from_le_bytes(
            bytes[off + SEGMENT_HEADER_SIZE + 12..off + SEGMENT_HEADER_SIZE + 16]
                .try_into()
                .unwrap(),
        )
    };
    assert_eq!(x86_entry, 0x1000, "x86_64 entry_point mismatch");
    assert_eq!(arm_entry, 0x2000, "aarch64 entry_point mismatch");

    println!("PASS: multi_arch_kernel_segments -- both architectures found and distinguishable");
}

// ===========================================================================
// TEST 5: kernel_image_hash_verification
// ===========================================================================

/// Embed a kernel with a known hash, read it back, compute the hash of the
/// image bytes, and verify it matches the image_hash field in the header.
#[test]
fn kernel_image_hash_verification() {
    // Fake kernel image data
    let image_data: Vec<u8> = (0..256u16).map(|i| (i & 0xFF) as u8).collect();
    let expected_hash = simple_test_hash(&image_data);

    // Build a KernelHeader with the image hash and the image as payload
    let kernel_hdr = make_kernel_header(
        ARCH_X86_64,
        KERNEL_TYPE_UNIKERNEL,
        0,
        0x0000_1000,
        image_data.len() as u64,
        0,
        2,
        64,
        expected_hash,
    );

    // Construct a full payload: KernelHeader + image_data
    let mut full_payload = Vec::with_capacity(KERNEL_HEADER_SIZE + image_data.len());
    full_payload.extend_from_slice(&kernel_hdr);
    full_payload.extend_from_slice(&image_data);

    // Write as a KERNEL_SEG
    let encoded = write_segment(
        SegmentType::Kernel as u8,
        &full_payload,
        SegmentFlags::empty(),
        300,
    );

    // Read back
    let (header, payload) = read_segment(&encoded).unwrap();
    validate_segment(&header, payload).expect("segment hash should validate");

    // Extract the KernelHeader from the payload
    assert!(payload.len() >= KERNEL_HEADER_SIZE + image_data.len());

    // Read image_hash from offset 32..64 of the KernelHeader
    let mut stored_hash = [0u8; 32];
    stored_hash.copy_from_slice(&payload[32..64]);

    // Read image_size from offset 16..24
    let stored_image_size = u64::from_le_bytes(payload[16..24].try_into().unwrap());
    assert_eq!(
        stored_image_size,
        image_data.len() as u64,
        "image_size should match"
    );

    // Extract image bytes from after the KernelHeader
    let image_start = KERNEL_HEADER_SIZE;
    let image_end = image_start + stored_image_size as usize;
    let extracted_image = &payload[image_start..image_end];

    // Compute hash of extracted image
    let computed_hash = simple_test_hash(extracted_image);

    // Verify hash match
    assert_eq!(
        stored_hash, computed_hash,
        "image_hash in KernelHeader should match computed hash of image bytes"
    );
    assert_eq!(
        stored_hash, expected_hash,
        "image_hash should match the original expected hash"
    );

    println!("PASS: kernel_image_hash_verification -- hash verified successfully");
}

// ===========================================================================
// TEST 6: kernel_flags_validation
// ===========================================================================

/// Test that SIGNED, REQUIRES_TEE, READ_ONLY, and INGEST_ENABLED flags
/// are preserved through a write/read cycle.
#[test]
fn kernel_flags_validation() {
    // Test each flag individually
    let flag_tests: Vec<(u32, &str)> = vec![
        (KERNEL_FLAG_SIGNED, "SIGNED"),
        (KERNEL_FLAG_REQUIRES_TEE, "REQUIRES_TEE"),
        (KERNEL_FLAG_READ_ONLY, "READ_ONLY"),
        (KERNEL_FLAG_INGEST_ENABLED, "INGEST_ENABLED"),
    ];

    for (flag, name) in &flag_tests {
        let kernel_hdr = make_kernel_header(
            ARCH_X86_64,
            KERNEL_TYPE_UNIKERNEL,
            *flag,
            0,
            0,
            0,
            0,
            0,
            [0u8; 32],
        );

        let encoded = write_segment(
            SegmentType::Kernel as u8,
            &kernel_hdr,
            SegmentFlags::empty(),
            400,
        );

        let (_header, payload) = read_segment(&encoded).unwrap();
        let read_flags = u32::from_le_bytes(payload[8..12].try_into().unwrap());

        assert_eq!(
            read_flags, *flag,
            "flag {name} (0x{flag:08X}) not preserved: got 0x{read_flags:08X}"
        );
        assert!(read_flags & *flag != 0, "flag {name} bit should be set");

        println!("  flag {name} (0x{flag:08X}): OK");
    }

    // Test all flags combined
    let all_flags = KERNEL_FLAG_SIGNED
        | KERNEL_FLAG_REQUIRES_TEE
        | KERNEL_FLAG_READ_ONLY
        | KERNEL_FLAG_INGEST_ENABLED;

    let kernel_hdr = make_kernel_header(
        ARCH_X86_64,
        KERNEL_TYPE_UNIKERNEL,
        all_flags,
        0,
        0,
        0,
        0,
        0,
        [0u8; 32],
    );

    let encoded = write_segment(
        SegmentType::Kernel as u8,
        &kernel_hdr,
        SegmentFlags::empty(),
        401,
    );

    let (_header, payload) = read_segment(&encoded).unwrap();
    let read_flags = u32::from_le_bytes(payload[8..12].try_into().unwrap());

    assert_eq!(
        read_flags, all_flags,
        "all kernel flags combined (0x{all_flags:08X}) not preserved: got 0x{read_flags:08X}"
    );
    assert!(
        read_flags & KERNEL_FLAG_SIGNED != 0,
        "SIGNED bit missing from combined"
    );
    assert!(
        read_flags & KERNEL_FLAG_REQUIRES_TEE != 0,
        "REQUIRES_TEE bit missing from combined"
    );
    assert!(
        read_flags & KERNEL_FLAG_READ_ONLY != 0,
        "READ_ONLY bit missing from combined"
    );
    assert!(
        read_flags & KERNEL_FLAG_INGEST_ENABLED != 0,
        "INGEST_ENABLED bit missing from combined"
    );

    println!("PASS: kernel_flags_validation -- all flag bits preserved");
}

// ===========================================================================
// TEST 7: ebpf_max_dimension_check
// ===========================================================================

/// Create an EBPF_SEG with max_dimension=128 and verify the field is
/// correctly stored and retrieved through a write/read cycle.
#[test]
fn ebpf_max_dimension_check() {
    let test_cases: &[(u16, &str)] = &[
        (0, "zero"),
        (1, "minimum"),
        (128, "typical"),
        (256, "larger"),
        (1024, "large"),
        (u16::MAX, "max u16"),
    ];

    for &(max_dim, label) in test_cases {
        let ebpf_hdr = make_ebpf_header(0x01, 0x00, 0, 100, 2, max_dim, [0u8; 8]);

        let encoded = write_segment(
            SegmentType::Ebpf as u8,
            &ebpf_hdr,
            SegmentFlags::empty(),
            500,
        );

        let (_header, payload) = read_segment(&encoded).unwrap();
        let read_max_dim = u16::from_le_bytes(payload[20..22].try_into().unwrap());

        assert_eq!(
            read_max_dim, max_dim,
            "max_dimension for case '{label}': expected {max_dim}, got {read_max_dim}"
        );

        println!("  max_dimension={max_dim} ({label}): OK");
    }

    println!("PASS: ebpf_max_dimension_check -- all dimension values preserved");
}

// ===========================================================================
// TEST 8: test_stub_kernel_type
// ===========================================================================

/// Create a KERNEL_SEG with kernel_type=0xFD (TestStub). This is the first
/// end-to-end demo target per implementation priorities. Verifies the
/// kernel_type field round-trips correctly and the segment is readable.
#[test]
fn test_stub_kernel_type() {
    let test_stub_image = b"#!/bin/test_stub\x00RVF_TEST_KERNEL_V1\x00";
    let image_hash = simple_test_hash(test_stub_image);

    let kernel_hdr = make_kernel_header(
        ARCH_X86_64,
        KERNEL_TYPE_TEST_STUB, // 0xFD
        KERNEL_FLAG_INGEST_ENABLED,
        0x0000_0000, // entry_point: 0 for test stubs
        test_stub_image.len() as u64,
        0,  // bss_size: none
        1,  // stack_pages: minimal
        64, // max_dimension
        image_hash,
    );

    // Full payload: KernelHeader + test stub image
    let mut full_payload = Vec::with_capacity(KERNEL_HEADER_SIZE + test_stub_image.len());
    full_payload.extend_from_slice(&kernel_hdr);
    full_payload.extend_from_slice(test_stub_image);

    // Write as KERNEL_SEG
    let encoded = write_segment(
        SegmentType::Kernel as u8,
        &full_payload,
        SegmentFlags::empty(),
        600,
    );

    // Read back
    let (header, payload) = read_segment(&encoded).unwrap();

    // Verify outer segment
    assert_eq!(header.seg_type, SegmentType::Kernel as u8);
    assert_eq!(header.segment_id, 600);
    validate_segment(&header, payload).expect("test stub content hash should validate");

    // Verify kernel_type is TestStub (0xFD)
    assert_eq!(
        payload[7], KERNEL_TYPE_TEST_STUB,
        "kernel_type should be TestStub (0xFD), got 0x{:02X}",
        payload[7]
    );

    // Verify the test stub image is intact
    let image_start = KERNEL_HEADER_SIZE;
    let image_size = u64::from_le_bytes(payload[16..24].try_into().unwrap()) as usize;
    assert_eq!(image_size, test_stub_image.len(), "image_size mismatch");

    let extracted = &payload[image_start..image_start + image_size];
    assert_eq!(extracted, test_stub_image, "test stub image data mismatch");

    // Verify hash
    let mut stored_hash = [0u8; 32];
    stored_hash.copy_from_slice(&payload[32..64]);
    let computed = simple_test_hash(extracted);
    assert_eq!(stored_hash, computed, "test stub image hash mismatch");

    // Verify this can also be written to a file and survive a store reopen
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test_stub.rvf");

    {
        let mut store = RvfStore::create(&path, make_options(4)).unwrap();
        let v = vec![1.0f32; 4];
        store.ingest_batch(&[v.as_slice()], &[1], None).unwrap();
        store.close().unwrap();
    }

    // Append the test stub segment
    {
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        let seg_header =
            build_raw_segment_header(SegmentType::Kernel as u8, 600, full_payload.len() as u64);
        file.write_all(&seg_header).unwrap();
        file.write_all(&full_payload).unwrap();
        file.sync_all().unwrap();
    }

    // Reopen and verify store is not broken
    let store = RvfStore::open_readonly(&path).unwrap();
    assert_eq!(
        store.status().total_vectors,
        1,
        "store should still work with test stub segment"
    );

    // Verify test stub is in the file
    let bytes = read_file_bytes(&path);
    let segs = scan_segments(&bytes);
    let kernel_segs: Vec<_> = segs
        .iter()
        .filter(|s| s.1 == SegmentType::Kernel as u8)
        .collect();
    assert_eq!(
        kernel_segs.len(),
        1,
        "should find one KERNEL_SEG (TestStub)"
    );

    let kernel_offset = kernel_segs[0].0;
    let kt = bytes[kernel_offset + SEGMENT_HEADER_SIZE + 7];
    assert_eq!(
        kt, KERNEL_TYPE_TEST_STUB,
        "kernel_type in file should be TestStub (0xFD), got 0x{:02X}",
        kt
    );

    println!("PASS: test_stub_kernel_type -- TestStub (0xFD) end-to-end verified");
}
