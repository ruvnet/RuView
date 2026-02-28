//! WASM_SEG (0x10) types for self-bootstrapping RVF files.
//!
//! Defines the 64-byte `WasmHeader` and associated enums.
//! A WASM_SEG embeds WASM bytecode that enables an RVF file to carry its
//! own execution runtime. When combined with the data segments (VEC_SEG,
//! INDEX_SEG, etc.), this makes the file fully self-bootstrapping:
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │                    .rvf file                             │
//! │                                                         │
//! │  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐  │
//! │  │ WASM_SEG    │  │ WASM_SEG     │  │ VEC_SEG       │  │
//! │  │ role=Interp │  │ role=uKernel │  │ (data)        │  │
//! │  │ ~50 KB      │  │ ~5.5 KB      │  │               │  │
//! │  └──────┬──────┘  └──────┬───────┘  └───────┬───────┘  │
//! │         │                │                   │          │
//! │         │   executes     │    processes      │          │
//! │         └───────────────►└──────────────────►│          │
//! │                                                         │
//! │  Layer 0: Raw bytes                                     │
//! │  Layer 1: Embedded WASM interpreter (native bootstrap)  │
//! │  Layer 2: WASM microkernel (query engine)               │
//! │  Layer 3: RVF data (vectors, indexes, manifests)        │
//! └──────────────────────────────────────────────────────────┘
//! ```
//!
//! The host only needs raw execution capability. RVF becomes
//! self-bootstrapping — "runs anywhere compute exists."

use crate::error::RvfError;

/// Magic number for `WasmHeader`: "RVWM" in big-endian.
pub const WASM_MAGIC: u32 = 0x5256_574D;

/// Role of the embedded WASM module within the bootstrap chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum WasmRole {
    /// RVF microkernel: the query/ingest engine compiled to WASM.
    /// This is the 5.5 KB Cognitum tile runtime with 14+ exports.
    Microkernel = 0x00,
    /// Minimal WASM interpreter: enables self-bootstrapping on hosts
    /// that lack a native WASM runtime. The interpreter runs the
    /// microkernel, which then processes RVF data.
    Interpreter = 0x01,
    /// Combined interpreter + microkernel in a single module.
    /// The interpreter is linked with the microkernel for zero-copy
    /// bootstrap on bare environments.
    Combined = 0x02,
    /// Domain-specific extension module (e.g., custom distance
    /// functions, codon decoder for RVDNA, token scorer for RVText).
    Extension = 0x03,
    /// Control plane module: store management, export, segment
    /// parsing, and file-level operations.
    ControlPlane = 0x04,
}

impl TryFrom<u8> for WasmRole {
    type Error = RvfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Microkernel),
            0x01 => Ok(Self::Interpreter),
            0x02 => Ok(Self::Combined),
            0x03 => Ok(Self::Extension),
            0x04 => Ok(Self::ControlPlane),
            _ => Err(RvfError::InvalidEnumValue {
                type_name: "WasmRole",
                value: value as u64,
            }),
        }
    }
}

/// Target platform hint for the WASM module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum WasmTarget {
    /// Generic wasm32 (runs on any compliant runtime).
    Wasm32 = 0x00,
    /// WASI Preview 1 (requires WASI syscalls).
    WasiP1 = 0x01,
    /// WASI Preview 2 (component model).
    WasiP2 = 0x02,
    /// Browser-optimized (expects Web APIs via imports).
    Browser = 0x03,
    /// Bare-metal tile (no imports beyond host-tile protocol).
    BareTile = 0x04,
}

impl TryFrom<u8> for WasmTarget {
    type Error = RvfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Wasm32),
            0x01 => Ok(Self::WasiP1),
            0x02 => Ok(Self::WasiP2),
            0x03 => Ok(Self::Browser),
            0x04 => Ok(Self::BareTile),
            _ => Err(RvfError::InvalidEnumValue {
                type_name: "WasmTarget",
                value: value as u64,
            }),
        }
    }
}

/// WASM module feature requirements (bitfield).
pub const WASM_FEAT_SIMD: u16 = 1 << 0;
pub const WASM_FEAT_BULK_MEMORY: u16 = 1 << 1;
pub const WASM_FEAT_MULTI_VALUE: u16 = 1 << 2;
pub const WASM_FEAT_REFERENCE_TYPES: u16 = 1 << 3;
pub const WASM_FEAT_THREADS: u16 = 1 << 4;
pub const WASM_FEAT_TAIL_CALL: u16 = 1 << 5;
pub const WASM_FEAT_GC: u16 = 1 << 6;
pub const WASM_FEAT_EXCEPTION_HANDLING: u16 = 1 << 7;

/// 64-byte header for WASM_SEG payloads.
///
/// Follows the standard 64-byte `SegmentHeader`. The WASM bytecode
/// follows immediately after this header within the segment payload.
///
/// For self-bootstrapping files, two WASM_SEGs are present:
/// 1. `role = Interpreter` — a minimal WASM interpreter (~50 KB)
/// 2. `role = Microkernel` — the RVF query engine (~5.5 KB)
///
/// The bootstrap sequence is:
/// 1. Host reads file, finds WASM_SEG with `role = Interpreter`
/// 2. Host loads interpreter bytecode into any available execution engine
/// 3. Interpreter instantiates the microkernel WASM_SEG
/// 4. Microkernel processes VEC_SEG, INDEX_SEG, etc.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct WasmHeader {
    /// Magic: `WASM_MAGIC` (0x5256574D, "RVWM").
    pub wasm_magic: u32,
    /// WasmHeader format version (currently 1).
    pub header_version: u16,
    /// Role in the bootstrap chain (see `WasmRole`).
    pub role: u8,
    /// Target platform (see `WasmTarget`).
    pub target: u8,
    /// Required WASM features bitfield (see `WASM_FEAT_*`).
    pub required_features: u16,
    /// Number of exports in the WASM module.
    pub export_count: u16,
    /// Uncompressed WASM bytecode size (bytes).
    pub bytecode_size: u32,
    /// Compressed bytecode size (0 if uncompressed).
    pub compressed_size: u32,
    /// Compression algorithm (same enum as SegmentHeader).
    pub compression: u8,
    /// Minimum linear memory pages required (64 KB each).
    pub min_memory_pages: u8,
    /// Maximum linear memory pages (0 = no limit).
    pub max_memory_pages: u8,
    /// Number of WASM tables.
    pub table_count: u8,
    /// SHAKE-256-256 hash of uncompressed bytecode.
    pub bytecode_hash: [u8; 32],
    /// Priority order for bootstrap resolution (lower = tried first).
    /// The interpreter with lowest priority is used when multiple are present.
    pub bootstrap_priority: u8,
    /// If role=Interpreter, this is the interpreter type:
    /// 0x00 = generic stack machine, 0x01 = wasm3-compatible,
    /// 0x02 = wamr-compatible, 0x03 = wasmi-compatible.
    pub interpreter_type: u8,
    /// Reserved (must be zero).
    pub reserved: [u8; 6],
}

// Compile-time assertion: WasmHeader must be exactly 64 bytes.
const _: () = assert!(core::mem::size_of::<WasmHeader>() == 64);

impl WasmHeader {
    /// Serialize the header to a 64-byte little-endian array.
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0x00..0x04].copy_from_slice(&self.wasm_magic.to_le_bytes());
        buf[0x04..0x06].copy_from_slice(&self.header_version.to_le_bytes());
        buf[0x06] = self.role;
        buf[0x07] = self.target;
        buf[0x08..0x0A].copy_from_slice(&self.required_features.to_le_bytes());
        buf[0x0A..0x0C].copy_from_slice(&self.export_count.to_le_bytes());
        buf[0x0C..0x10].copy_from_slice(&self.bytecode_size.to_le_bytes());
        buf[0x10..0x14].copy_from_slice(&self.compressed_size.to_le_bytes());
        buf[0x14] = self.compression;
        buf[0x15] = self.min_memory_pages;
        buf[0x16] = self.max_memory_pages;
        buf[0x17] = self.table_count;
        buf[0x18..0x38].copy_from_slice(&self.bytecode_hash);
        buf[0x38] = self.bootstrap_priority;
        buf[0x39] = self.interpreter_type;
        buf[0x3A..0x40].copy_from_slice(&self.reserved);
        buf
    }

    /// Deserialize a `WasmHeader` from a 64-byte slice.
    pub fn from_bytes(data: &[u8; 64]) -> Result<Self, RvfError> {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != WASM_MAGIC {
            return Err(RvfError::BadMagic {
                expected: WASM_MAGIC,
                got: magic,
            });
        }

        Ok(Self {
            wasm_magic: magic,
            header_version: u16::from_le_bytes([data[0x04], data[0x05]]),
            role: data[0x06],
            target: data[0x07],
            required_features: u16::from_le_bytes([data[0x08], data[0x09]]),
            export_count: u16::from_le_bytes([data[0x0A], data[0x0B]]),
            bytecode_size: u32::from_le_bytes([data[0x0C], data[0x0D], data[0x0E], data[0x0F]]),
            compressed_size: u32::from_le_bytes([data[0x10], data[0x11], data[0x12], data[0x13]]),
            compression: data[0x14],
            min_memory_pages: data[0x15],
            max_memory_pages: data[0x16],
            table_count: data[0x17],
            bytecode_hash: {
                let mut h = [0u8; 32];
                h.copy_from_slice(&data[0x18..0x38]);
                h
            },
            bootstrap_priority: data[0x38],
            interpreter_type: data[0x39],
            reserved: {
                let mut r = [0u8; 6];
                r.copy_from_slice(&data[0x3A..0x40]);
                r
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> WasmHeader {
        WasmHeader {
            wasm_magic: WASM_MAGIC,
            header_version: 1,
            role: WasmRole::Microkernel as u8,
            target: WasmTarget::BareTile as u8,
            required_features: WASM_FEAT_SIMD | WASM_FEAT_BULK_MEMORY,
            export_count: 14,
            bytecode_size: 5500,
            compressed_size: 0,
            compression: 0,
            min_memory_pages: 2, // 128 KB
            max_memory_pages: 4, // 256 KB
            table_count: 0,
            bytecode_hash: [0xAB; 32],
            bootstrap_priority: 0,
            interpreter_type: 0,
            reserved: [0; 6],
        }
    }

    #[test]
    fn header_size_is_64() {
        assert_eq!(core::mem::size_of::<WasmHeader>(), 64);
    }

    #[test]
    fn magic_bytes_match_ascii() {
        let bytes_be = WASM_MAGIC.to_be_bytes();
        assert_eq!(&bytes_be, b"RVWM");
    }

    #[test]
    fn round_trip_serialization() {
        let original = sample_header();
        let bytes = original.to_bytes();
        let decoded = WasmHeader::from_bytes(&bytes).expect("from_bytes should succeed");

        assert_eq!(decoded.wasm_magic, WASM_MAGIC);
        assert_eq!(decoded.header_version, 1);
        assert_eq!(decoded.role, WasmRole::Microkernel as u8);
        assert_eq!(decoded.target, WasmTarget::BareTile as u8);
        assert_eq!(
            decoded.required_features,
            WASM_FEAT_SIMD | WASM_FEAT_BULK_MEMORY
        );
        assert_eq!(decoded.export_count, 14);
        assert_eq!(decoded.bytecode_size, 5500);
        assert_eq!(decoded.compressed_size, 0);
        assert_eq!(decoded.compression, 0);
        assert_eq!(decoded.min_memory_pages, 2);
        assert_eq!(decoded.max_memory_pages, 4);
        assert_eq!(decoded.table_count, 0);
        assert_eq!(decoded.bytecode_hash, [0xAB; 32]);
        assert_eq!(decoded.bootstrap_priority, 0);
        assert_eq!(decoded.interpreter_type, 0);
        assert_eq!(decoded.reserved, [0; 6]);
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut bytes = sample_header().to_bytes();
        bytes[0] = 0x00;
        let err = WasmHeader::from_bytes(&bytes).unwrap_err();
        match err {
            RvfError::BadMagic { expected, .. } => assert_eq!(expected, WASM_MAGIC),
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn interpreter_header() {
        let h = WasmHeader {
            wasm_magic: WASM_MAGIC,
            header_version: 1,
            role: WasmRole::Interpreter as u8,
            target: WasmTarget::Wasm32 as u8,
            required_features: 0,
            export_count: 3,
            bytecode_size: 51_200, // ~50 KB interpreter
            compressed_size: 22_000,
            compression: 2,       // ZSTD
            min_memory_pages: 16, // 1 MB
            max_memory_pages: 64, // 4 MB
            table_count: 1,
            bytecode_hash: [0xCD; 32],
            bootstrap_priority: 0,  // highest priority
            interpreter_type: 0x03, // wasmi-compatible
            reserved: [0; 6],
        };
        let bytes = h.to_bytes();
        let decoded = WasmHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.role, WasmRole::Interpreter as u8);
        assert_eq!(decoded.bytecode_size, 51_200);
        assert_eq!(decoded.interpreter_type, 0x03);
    }

    #[test]
    fn combined_bootstrap_header() {
        let h = WasmHeader {
            wasm_magic: WASM_MAGIC,
            header_version: 1,
            role: WasmRole::Combined as u8,
            target: WasmTarget::Wasm32 as u8,
            required_features: WASM_FEAT_SIMD,
            export_count: 17,
            bytecode_size: 56_700, // interpreter + microkernel
            compressed_size: 0,
            compression: 0,
            min_memory_pages: 16,
            max_memory_pages: 64,
            table_count: 1,
            bytecode_hash: [0xEF; 32],
            bootstrap_priority: 0,
            interpreter_type: 0,
            reserved: [0; 6],
        };
        let bytes = h.to_bytes();
        let decoded = WasmHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.role, WasmRole::Combined as u8);
        assert_eq!(decoded.export_count, 17);
    }

    #[test]
    fn wasm_role_try_from() {
        assert_eq!(WasmRole::try_from(0x00), Ok(WasmRole::Microkernel));
        assert_eq!(WasmRole::try_from(0x01), Ok(WasmRole::Interpreter));
        assert_eq!(WasmRole::try_from(0x02), Ok(WasmRole::Combined));
        assert_eq!(WasmRole::try_from(0x03), Ok(WasmRole::Extension));
        assert_eq!(WasmRole::try_from(0x04), Ok(WasmRole::ControlPlane));
        assert!(WasmRole::try_from(0x05).is_err());
        assert!(WasmRole::try_from(0xFF).is_err());
    }

    #[test]
    fn wasm_target_try_from() {
        assert_eq!(WasmTarget::try_from(0x00), Ok(WasmTarget::Wasm32));
        assert_eq!(WasmTarget::try_from(0x01), Ok(WasmTarget::WasiP1));
        assert_eq!(WasmTarget::try_from(0x02), Ok(WasmTarget::WasiP2));
        assert_eq!(WasmTarget::try_from(0x03), Ok(WasmTarget::Browser));
        assert_eq!(WasmTarget::try_from(0x04), Ok(WasmTarget::BareTile));
        assert!(WasmTarget::try_from(0x05).is_err());
        assert!(WasmTarget::try_from(0xFF).is_err());
    }

    #[test]
    fn feature_flags_bit_positions() {
        assert_eq!(WASM_FEAT_SIMD, 0x0001);
        assert_eq!(WASM_FEAT_BULK_MEMORY, 0x0002);
        assert_eq!(WASM_FEAT_MULTI_VALUE, 0x0004);
        assert_eq!(WASM_FEAT_REFERENCE_TYPES, 0x0008);
        assert_eq!(WASM_FEAT_THREADS, 0x0010);
        assert_eq!(WASM_FEAT_TAIL_CALL, 0x0020);
        assert_eq!(WASM_FEAT_GC, 0x0040);
        assert_eq!(WASM_FEAT_EXCEPTION_HANDLING, 0x0080);
    }
}
