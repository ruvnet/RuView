//! RVF (RuVector Format) container for WASM sensing modules.
//!
//! Defines the binary format shared between the ESP32 C parser and the
//! Rust builder tool.  The builder (behind `std` feature) packs a `.wasm`
//! binary with a manifest into an `.rvf` file.
//!
//! # Binary Layout
//!
//! ```text
//! [Header: 32 bytes][Manifest: 96 bytes][WASM: N bytes]
//! [Signature: 0|64 bytes][TestVectors: M bytes]
//! ```

/// RVF magic: `"RVF\x01"` as u32 LE = `0x01465652`.
pub const RVF_MAGIC: u32 = 0x0146_5652;

/// Current format version.
pub const RVF_FORMAT_VERSION: u16 = 1;

/// Header size in bytes.
pub const RVF_HEADER_SIZE: usize = 32;

/// Manifest size in bytes.
pub const RVF_MANIFEST_SIZE: usize = 96;

/// Ed25519 signature length.
pub const RVF_SIGNATURE_LEN: usize = 64;

/// Host API version supported by this crate.
pub const RVF_HOST_API_V1: u16 = 1;

// ── Capability flags ─────────────────────────────────────────────────────

pub const CAP_READ_PHASE: u32 = 1 << 0;
pub const CAP_READ_AMPLITUDE: u32 = 1 << 1;
pub const CAP_READ_VARIANCE: u32 = 1 << 2;
pub const CAP_READ_VITALS: u32 = 1 << 3;
pub const CAP_READ_HISTORY: u32 = 1 << 4;
pub const CAP_EMIT_EVENTS: u32 = 1 << 5;
pub const CAP_LOG: u32 = 1 << 6;
pub const CAP_ALL: u32 = 0x7F;

// ── Header flags ─────────────────────────────────────────────────────────

pub const FLAG_HAS_SIGNATURE: u16 = 1 << 0;
pub const FLAG_HAS_TEST_VECTORS: u16 = 1 << 1;

// ── Wire structs (must match C layout exactly) ───────────────────────────

/// RVF header (32 bytes, packed, little-endian).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RvfHeader {
    pub magic: u32,
    pub format_version: u16,
    pub flags: u16,
    pub manifest_len: u32,
    pub wasm_len: u32,
    pub signature_len: u32,
    pub test_vectors_len: u32,
    pub total_len: u32,
    pub reserved: u32,
}

/// RVF manifest (96 bytes, packed, little-endian).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RvfManifest {
    pub module_name: [u8; 32],
    pub required_host_api: u16,
    pub capabilities: u32,
    pub max_frame_us: u32,
    pub max_events_per_sec: u16,
    pub memory_limit_kb: u16,
    pub event_schema_version: u16,
    pub build_hash: [u8; 32],
    pub min_subcarriers: u16,
    pub max_subcarriers: u16,
    pub author: [u8; 10],
    pub _reserved: [u8; 2],
}

// Compile-time size checks.
const _: () = assert!(core::mem::size_of::<RvfHeader>() == RVF_HEADER_SIZE);
const _: () = assert!(core::mem::size_of::<RvfManifest>() == RVF_MANIFEST_SIZE);

// ── Builder (std only) ──────────────────────────────────────────────────

#[cfg(feature = "std")]
pub mod builder {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::io::Write;

    /// Copy a string into a fixed-size null-padded buffer.
    fn copy_to_fixed<const N: usize>(src: &str) -> [u8; N] {
        let mut buf = [0u8; N];
        let len = src.len().min(N - 1); // leave room for null
        buf[..len].copy_from_slice(&src.as_bytes()[..len]);
        buf
    }

    /// Configuration for building an RVF file.
    pub struct RvfConfig {
        pub module_name: String,
        pub author: String,
        pub capabilities: u32,
        pub max_frame_us: u32,
        pub max_events_per_sec: u16,
        pub memory_limit_kb: u16,
        pub event_schema_version: u16,
        pub min_subcarriers: u16,
        pub max_subcarriers: u16,
    }

    impl Default for RvfConfig {
        fn default() -> Self {
            Self {
                module_name: String::from("unnamed"),
                author: String::from("unknown"),
                capabilities: CAP_ALL,
                max_frame_us: 10_000,
                max_events_per_sec: 0,
                memory_limit_kb: 0,
                event_schema_version: 1,
                min_subcarriers: 0,
                max_subcarriers: 0,
            }
        }
    }

    /// Build an RVF container from WASM binary data and a config.
    ///
    /// Returns the complete RVF as a byte vector.
    /// The signature field is zeroed — sign externally and patch bytes
    /// at the signature offset.
    pub fn build_rvf(wasm_data: &[u8], config: &RvfConfig) -> Vec<u8> {
        // Compute SHA-256 of WASM payload.
        let mut hasher = Sha256::new();
        hasher.update(wasm_data);
        let hash: [u8; 32] = hasher.finalize().into();

        // Build manifest.
        let manifest = RvfManifest {
            module_name: copy_to_fixed::<32>(&config.module_name),
            required_host_api: RVF_HOST_API_V1,
            capabilities: config.capabilities,
            max_frame_us: config.max_frame_us,
            max_events_per_sec: config.max_events_per_sec,
            memory_limit_kb: config.memory_limit_kb,
            event_schema_version: config.event_schema_version,
            build_hash: hash,
            min_subcarriers: config.min_subcarriers,
            max_subcarriers: config.max_subcarriers,
            author: copy_to_fixed::<10>(&config.author),
            _reserved: [0; 2],
        };

        let signature_len = RVF_SIGNATURE_LEN as u32;
        let total_len = (RVF_HEADER_SIZE + RVF_MANIFEST_SIZE) as u32
            + wasm_data.len() as u32
            + signature_len;

        // Build header.
        let header = RvfHeader {
            magic: RVF_MAGIC,
            format_version: RVF_FORMAT_VERSION,
            flags: FLAG_HAS_SIGNATURE,
            manifest_len: RVF_MANIFEST_SIZE as u32,
            wasm_len: wasm_data.len() as u32,
            signature_len,
            test_vectors_len: 0,
            total_len,
            reserved: 0,
        };

        // Serialize.
        let mut out = Vec::with_capacity(total_len as usize);

        // SAFETY: header and manifest are packed repr(C) structs with no padding.
        let header_bytes: &[u8] = unsafe {
            core::slice::from_raw_parts(
                &header as *const RvfHeader as *const u8,
                RVF_HEADER_SIZE,
            )
        };
        out.write_all(header_bytes).unwrap();

        let manifest_bytes: &[u8] = unsafe {
            core::slice::from_raw_parts(
                &manifest as *const RvfManifest as *const u8,
                RVF_MANIFEST_SIZE,
            )
        };
        out.write_all(manifest_bytes).unwrap();

        out.write_all(wasm_data).unwrap();

        // Placeholder signature (zeroed — sign externally).
        out.write_all(&[0u8; RVF_SIGNATURE_LEN]).unwrap();

        out
    }

    /// Patch a signature into an existing RVF buffer.
    ///
    /// The signature covers bytes 0 through (header + manifest + wasm - 1).
    pub fn patch_signature(rvf: &mut [u8], signature: &[u8; RVF_SIGNATURE_LEN]) {
        let sig_offset = RVF_HEADER_SIZE + RVF_MANIFEST_SIZE;
        // Read wasm_len from header.
        let wasm_len = u32::from_le_bytes([
            rvf[12], rvf[13], rvf[14], rvf[15],
        ]) as usize;
        let offset = sig_offset + wasm_len;
        rvf[offset..offset + RVF_SIGNATURE_LEN].copy_from_slice(signature);
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_build_rvf_roundtrip() {
            // Minimal valid WASM: magic + version.
            let wasm = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
            let config = RvfConfig {
                module_name: "test-module".into(),
                author: "tester".into(),
                capabilities: CAP_READ_PHASE | CAP_EMIT_EVENTS,
                max_frame_us: 5000,
                ..Default::default()
            };

            let rvf = build_rvf(&wasm, &config);

            // Check magic.
            let magic = u32::from_le_bytes([rvf[0], rvf[1], rvf[2], rvf[3]]);
            assert_eq!(magic, RVF_MAGIC);

            // Check total length.
            let expected_len = RVF_HEADER_SIZE + RVF_MANIFEST_SIZE + wasm.len()
                + RVF_SIGNATURE_LEN;
            assert_eq!(rvf.len(), expected_len);

            // Check WASM payload.
            let wasm_offset = RVF_HEADER_SIZE + RVF_MANIFEST_SIZE;
            assert_eq!(&rvf[wasm_offset..wasm_offset + wasm.len()], &wasm);

            // Check module name in manifest.
            let name_offset = RVF_HEADER_SIZE;
            let name_bytes = &rvf[name_offset..name_offset + 11];
            assert_eq!(&name_bytes[..11], b"test-module");
        }

        #[test]
        fn test_build_hash_integrity() {
            let wasm = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
            let config = RvfConfig::default();
            let rvf = build_rvf(&wasm, &config);

            // Extract build_hash from manifest (offset 48 from manifest start).
            let hash_offset = RVF_HEADER_SIZE + 32 + 2 + 4 + 4 + 2 + 2 + 2;
            let stored_hash = &rvf[hash_offset..hash_offset + 32];

            // Compute expected hash.
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&wasm);
            let expected: [u8; 32] = hasher.finalize().into();

            assert_eq!(stored_hash, &expected);
        }
    }
}
