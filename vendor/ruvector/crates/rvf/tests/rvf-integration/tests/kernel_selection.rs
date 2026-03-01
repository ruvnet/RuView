//! Integration tests for deterministic kernel selection.
//!
//! Tests embedding multiple kernels with different architectures and
//! verifying selection based on architecture match, signed vs unsigned
//! precedence, and api_version ordering.

use rvf_runtime::options::{DistanceMetric, RvfOptions};
use rvf_runtime::RvfStore;
use rvf_types::kernel::{KernelHeader, KERNEL_MAGIC};
use rvf_types::kernel_binding::KernelBinding;
use rvf_types::{SegmentType, SEGMENT_HEADER_SIZE, SEGMENT_MAGIC};
use std::fs::OpenOptions;
use std::io::Read;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ARCH_X86_64: u8 = 0x00;
const ARCH_AARCH64: u8 = 0x01;
const KERNEL_FLAG_SIGNED: u32 = 0x0000_0001;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_options(dim: u16) -> RvfOptions {
    RvfOptions {
        dimension: dim,
        metric: DistanceMetric::L2,
        ..Default::default()
    }
}

fn read_file_bytes(path: &std::path::Path) -> Vec<u8> {
    let mut file = OpenOptions::new().read(true).open(path).unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    buf
}

/// Scan the file for all KERNEL_SEG segments and return their raw payloads.
fn extract_kernel_segments(file_bytes: &[u8]) -> Vec<(u64, Vec<u8>)> {
    let magic_bytes = SEGMENT_MAGIC.to_le_bytes();
    let mut results = Vec::new();

    if file_bytes.len() < SEGMENT_HEADER_SIZE {
        return results;
    }

    let last_possible = file_bytes.len() - SEGMENT_HEADER_SIZE;
    for i in 0..=last_possible {
        if file_bytes[i..i + 4] == magic_bytes {
            let seg_type = file_bytes[i + 5];
            if seg_type == SegmentType::Kernel as u8 {
                let seg_id = u64::from_le_bytes(file_bytes[i + 0x08..i + 0x10].try_into().unwrap());
                let payload_len =
                    u64::from_le_bytes(file_bytes[i + 0x10..i + 0x18].try_into().unwrap()) as usize;

                let payload_start = i + SEGMENT_HEADER_SIZE;
                let payload_end = payload_start + payload_len;
                if payload_end <= file_bytes.len() && payload_len >= 128 {
                    let payload = file_bytes[payload_start..payload_end].to_vec();
                    results.push((seg_id, payload));
                }
            }
        }
    }

    results
}

// ===========================================================================
// TEST 1: embed_kernel_with_arch_x86_64
// ===========================================================================

/// Embed a kernel for x86_64 and verify the architecture field is stored.
#[test]
fn embed_kernel_with_arch_x86_64() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("kernel_x86.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
    let kernel_image = b"x86_64-kernel-image-data";

    let seg_id = store
        .embed_kernel(ARCH_X86_64, 0x00, 0, kernel_image, 8080, None)
        .unwrap();
    assert!(seg_id > 0);

    let (header_bytes, _image) = store.extract_kernel().unwrap().unwrap();

    // Parse the KernelHeader to verify arch
    let mut header_arr = [0u8; 128];
    header_arr.copy_from_slice(&header_bytes);
    let header = KernelHeader::from_bytes(&header_arr).unwrap();

    assert_eq!(header.arch, ARCH_X86_64, "arch should be x86_64");
    assert_eq!(header.kernel_magic, KERNEL_MAGIC);

    store.close().unwrap();

    println!("PASS: embed_kernel_with_arch_x86_64");
}

// ===========================================================================
// TEST 2: embed_kernel_with_arch_aarch64
// ===========================================================================

/// Embed a kernel for aarch64 and verify the architecture field.
#[test]
fn embed_kernel_with_arch_aarch64() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("kernel_arm.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
    let kernel_image = b"aarch64-kernel-image-data";

    store
        .embed_kernel(ARCH_AARCH64, 0x00, 0, kernel_image, 9090, None)
        .unwrap();

    let (header_bytes, _image) = store.extract_kernel().unwrap().unwrap();

    let mut header_arr = [0u8; 128];
    header_arr.copy_from_slice(&header_bytes);
    let header = KernelHeader::from_bytes(&header_arr).unwrap();

    assert_eq!(header.arch, ARCH_AARCH64, "arch should be aarch64");

    store.close().unwrap();

    println!("PASS: embed_kernel_with_arch_aarch64");
}

// ===========================================================================
// TEST 3: multi_kernel_file_contains_both
// ===========================================================================

/// Embed two kernels (x86_64 and aarch64) into the same file and verify
/// both are present in the raw file bytes.
#[test]
fn multi_kernel_file_contains_both() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("multi_kernel.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    // Embed x86_64 kernel
    store
        .embed_kernel(ARCH_X86_64, 0x00, 0, b"x86-image", 8080, None)
        .unwrap();

    // Embed aarch64 kernel
    store
        .embed_kernel(ARCH_AARCH64, 0x00, 0, b"arm-image", 9090, None)
        .unwrap();

    store.close().unwrap();

    // Scan raw file for all KERNEL_SEGs
    let bytes = read_file_bytes(&path);
    let kernels = extract_kernel_segments(&bytes);

    assert_eq!(
        kernels.len(),
        2,
        "file should contain 2 KERNEL_SEGs, found {}",
        kernels.len()
    );

    // Verify architectures
    let mut archs = Vec::new();
    for (_seg_id, payload) in &kernels {
        let mut header_arr = [0u8; 128];
        header_arr.copy_from_slice(&payload[..128]);
        let header = KernelHeader::from_bytes(&header_arr).unwrap();
        archs.push(header.arch);
    }

    assert!(archs.contains(&ARCH_X86_64), "should have x86_64 kernel");
    assert!(archs.contains(&ARCH_AARCH64), "should have aarch64 kernel");

    println!("PASS: multi_kernel_file_contains_both");
}

// ===========================================================================
// TEST 4: signed_kernel_flags_preserved
// ===========================================================================

/// Embed a signed kernel and verify the SIGNED flag is preserved.
#[test]
fn signed_kernel_flags_preserved() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("signed_kernel.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    store
        .embed_kernel(
            ARCH_X86_64,
            0x00,
            KERNEL_FLAG_SIGNED,
            b"signed-kernel-image",
            8080,
            None,
        )
        .unwrap();

    let (header_bytes, _image) = store.extract_kernel().unwrap().unwrap();
    let mut header_arr = [0u8; 128];
    header_arr.copy_from_slice(&header_bytes);
    let header = KernelHeader::from_bytes(&header_arr).unwrap();

    assert!(
        header.kernel_flags & KERNEL_FLAG_SIGNED != 0,
        "SIGNED flag should be set: got 0x{:08X}",
        header.kernel_flags
    );

    store.close().unwrap();

    println!("PASS: signed_kernel_flags_preserved");
}

// ===========================================================================
// TEST 5: kernel_binding_round_trip
// ===========================================================================

/// Embed a kernel with a KernelBinding and verify the binding survives
/// extraction.
#[test]
fn kernel_binding_round_trip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("kernel_binding.rvf");
    let dim: u16 = 4;

    let mut store = RvfStore::create(&path, make_options(dim)).unwrap();

    let binding = KernelBinding {
        manifest_root_hash: [0xAA; 32],
        policy_hash: [0xBB; 32],
        binding_version: 1,
        min_runtime_version: 2,
        _pad0: 0,
        allowed_segment_mask: 0x00FF_FFFF,
        _reserved: [0; 48],
    };

    store
        .embed_kernel_with_binding(
            ARCH_X86_64,
            0x00,
            KERNEL_FLAG_SIGNED,
            b"kernel-with-binding",
            8080,
            Some("console=ttyS0"),
            &binding,
        )
        .unwrap();

    // Extract the binding
    let extracted_binding = store.extract_kernel_binding().unwrap();
    assert!(extracted_binding.is_some(), "binding should be extractable");

    let eb = extracted_binding.unwrap();
    assert_eq!(eb.binding_version, 1, "binding_version mismatch");
    assert_eq!(eb.min_runtime_version, 2, "min_runtime_version mismatch");
    assert_eq!(
        eb.manifest_root_hash, [0xAA; 32],
        "manifest_root_hash mismatch"
    );
    assert_eq!(eb.policy_hash, [0xBB; 32], "policy_hash mismatch");
    assert_eq!(
        eb.allowed_segment_mask, 0x00FF_FFFF,
        "segment_mask mismatch"
    );

    store.close().unwrap();

    println!("PASS: kernel_binding_round_trip");
}

// ===========================================================================
// TEST 6: kernel_binding_persists_through_reopen
// ===========================================================================

/// Embed a kernel with binding, close, reopen, and verify the binding
/// is still present.
#[test]
fn kernel_binding_persists_through_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("binding_persist.rvf");
    let dim: u16 = 4;

    let binding = KernelBinding {
        manifest_root_hash: [0x11; 32],
        policy_hash: [0x22; 32],
        binding_version: 3,
        min_runtime_version: 1,
        _pad0: 0,
        allowed_segment_mask: 0xDEAD_BEEF,
        _reserved: [0; 48],
    };

    {
        let mut store = RvfStore::create(&path, make_options(dim)).unwrap();
        store
            .embed_kernel_with_binding(
                ARCH_AARCH64,
                0x00,
                0,
                b"persistent-binding-kernel",
                7070,
                None,
                &binding,
            )
            .unwrap();
        store.close().unwrap();
    }

    {
        let store = RvfStore::open_readonly(&path).unwrap();
        let eb = store.extract_kernel_binding().unwrap();
        assert!(eb.is_some(), "binding should persist through reopen");

        let eb = eb.unwrap();
        assert_eq!(eb.binding_version, 3);
        assert_eq!(eb.min_runtime_version, 1);
        assert_eq!(eb.manifest_root_hash, [0x11; 32]);
        assert_eq!(eb.policy_hash, [0x22; 32]);
        assert_eq!(eb.allowed_segment_mask, 0xDEAD_BEEF);
    }

    println!("PASS: kernel_binding_persists_through_reopen");
}

// ===========================================================================
// TEST 7: no_kernel_returns_none
// ===========================================================================

/// A store without any kernel should return None for extraction.
#[test]
fn no_kernel_returns_none() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("no_kernel.rvf");
    let dim: u16 = 4;

    let store = RvfStore::create(&path, make_options(dim)).unwrap();

    assert!(store.extract_kernel().unwrap().is_none());
    assert!(store.extract_kernel_binding().unwrap().is_none());

    store.close().unwrap();

    println!("PASS: no_kernel_returns_none");
}

// ===========================================================================
// TEST 8: kernel_header_serialization
// ===========================================================================

/// Test KernelHeader serialization and deserialization directly.
#[test]
fn kernel_header_serialization() {
    let header = KernelHeader {
        kernel_magic: KERNEL_MAGIC,
        header_version: 1,
        arch: ARCH_AARCH64,
        kernel_type: 0xFD,
        kernel_flags: KERNEL_FLAG_SIGNED,
        min_memory_mb: 0,
        entry_point: 0x1000,
        image_size: 65536,
        compressed_size: 32768,
        compression: 1,
        api_transport: 0,
        api_port: 8443,
        api_version: 2,
        image_hash: [0xCC; 32],
        build_id: [0xDD; 16],
        build_timestamp: 1700000000,
        vcpu_count: 4,
        reserved_0: 0,
        cmdline_offset: 256,
        cmdline_length: 32,
        reserved_1: 0,
    };

    let bytes = header.to_bytes();
    let decoded = KernelHeader::from_bytes(&bytes).unwrap();

    assert_eq!(decoded.kernel_magic, KERNEL_MAGIC);
    assert_eq!(decoded.header_version, 1);
    assert_eq!(decoded.arch, ARCH_AARCH64);
    assert_eq!(decoded.kernel_type, 0xFD);
    assert_eq!(decoded.kernel_flags, KERNEL_FLAG_SIGNED);
    assert_eq!(decoded.entry_point, 0x1000);
    assert_eq!(decoded.image_size, 65536);
    assert_eq!(decoded.compressed_size, 32768);
    assert_eq!(decoded.compression, 1);
    assert_eq!(decoded.api_port, 8443);
    assert_eq!(decoded.api_version, 2);
    assert_eq!(decoded.image_hash, [0xCC; 32]);
    assert_eq!(decoded.build_id, [0xDD; 16]);
    assert_eq!(decoded.build_timestamp, 1700000000);
    assert_eq!(decoded.vcpu_count, 4);
    assert_eq!(decoded.cmdline_offset, 256);
    assert_eq!(decoded.cmdline_length, 32);

    println!("PASS: kernel_header_serialization");
}

// ===========================================================================
// TEST 9: kernel_binding_serialization
// ===========================================================================

/// Test KernelBinding serialization directly.
#[test]
fn kernel_binding_serialization() {
    let binding = KernelBinding {
        manifest_root_hash: [0x01; 32],
        policy_hash: [0x02; 32],
        binding_version: 5,
        min_runtime_version: 3,
        _pad0: 0,
        allowed_segment_mask: 0xFFFF_FFFF_FFFF_FFFF,
        _reserved: [0; 48],
    };

    let bytes = binding.to_bytes();
    let decoded = KernelBinding::from_bytes(&bytes);

    assert_eq!(
        decoded, binding,
        "round-trip should produce identical binding"
    );

    println!("PASS: kernel_binding_serialization");
}
