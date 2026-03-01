//! KERNEL_SEG (0x0E) types for the RVF computational container.
//!
//! Defines the 128-byte `KernelHeader` and associated enums per ADR-030.
//! The KERNEL_SEG embeds a unikernel image that can self-boot an RVF file
//! as a standalone query-serving microservice.

use crate::error::RvfError;

/// Magic number for `KernelHeader`: "RVKN" in big-endian.
pub const KERNEL_MAGIC: u32 = 0x5256_4B4E;

/// Kernel flags: kernel image is cryptographically signed.
pub const KERNEL_FLAG_SIGNED: u32 = 1 << 8;
/// Kernel flags: kernel image is compressed per the `compression` field.
pub const KERNEL_FLAG_COMPRESSED: u32 = 1 << 10;
/// Kernel flags: kernel must run inside a TEE enclave.
pub const KERNEL_FLAG_REQUIRES_TEE: u32 = 1 << 0;
/// Kernel flags: kernel measurement stored in WITNESS_SEG.
pub const KERNEL_FLAG_MEASURED: u32 = 1 << 9;
/// Kernel flags: kernel requires KVM (hardware virtualization).
pub const KERNEL_FLAG_REQUIRES_KVM: u32 = 1 << 1;
/// Kernel flags: kernel requires UEFI boot.
pub const KERNEL_FLAG_REQUIRES_UEFI: u32 = 1 << 2;
/// Kernel flags: kernel includes network stack.
pub const KERNEL_FLAG_HAS_NETWORKING: u32 = 1 << 3;
/// Kernel flags: kernel exposes RVF query API on api_port.
pub const KERNEL_FLAG_HAS_QUERY_API: u32 = 1 << 4;
/// Kernel flags: kernel exposes RVF ingest API.
pub const KERNEL_FLAG_HAS_INGEST_API: u32 = 1 << 5;
/// Kernel flags: kernel exposes health/metrics API.
pub const KERNEL_FLAG_HAS_ADMIN_API: u32 = 1 << 6;
/// Kernel flags: kernel can generate TEE attestation quotes.
pub const KERNEL_FLAG_ATTESTATION_READY: u32 = 1 << 7;
/// Kernel flags: kernel is position-independent.
pub const KERNEL_FLAG_RELOCATABLE: u32 = 1 << 11;
/// Kernel flags: kernel includes VirtIO network driver.
pub const KERNEL_FLAG_HAS_VIRTIO_NET: u32 = 1 << 12;
/// Kernel flags: kernel includes VirtIO block driver.
pub const KERNEL_FLAG_HAS_VIRTIO_BLK: u32 = 1 << 13;
/// Kernel flags: kernel includes VSOCK for host communication.
pub const KERNEL_FLAG_HAS_VSOCK: u32 = 1 << 14;

/// Target CPU architecture for the kernel image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum KernelArch {
    /// AMD64 / Intel 64.
    X86_64 = 0x00,
    /// ARM 64-bit (ARMv8-A and later).
    Aarch64 = 0x01,
    /// RISC-V 64-bit (RV64GC).
    Riscv64 = 0x02,
    /// Architecture-independent (e.g., interpreted).
    Universal = 0xFE,
    /// Reserved / unspecified.
    Unknown = 0xFF,
}

impl TryFrom<u8> for KernelArch {
    type Error = RvfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::X86_64),
            0x01 => Ok(Self::Aarch64),
            0x02 => Ok(Self::Riscv64),
            0xFE => Ok(Self::Universal),
            0xFF => Ok(Self::Unknown),
            _ => Err(RvfError::InvalidEnumValue {
                type_name: "KernelArch",
                value: value as u64,
            }),
        }
    }
}

/// Kernel type / runtime model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum KernelType {
    /// Hermit OS unikernel (Rust-native).
    Hermit = 0x00,
    /// Minimal Linux kernel (bzImage compatible).
    MicroLinux = 0x01,
    /// Asterinas framekernel (Linux ABI compatible).
    Asterinas = 0x02,
    /// WASI Preview 2 component (alternative to WASM_SEG).
    WasiPreview2 = 0x03,
    /// Custom kernel (requires external VMM knowledge).
    Custom = 0x04,
    /// Test stub for CI (boots, reports health, exits).
    TestStub = 0xFE,
}

impl TryFrom<u8> for KernelType {
    type Error = RvfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Hermit),
            0x01 => Ok(Self::MicroLinux),
            0x02 => Ok(Self::Asterinas),
            0x03 => Ok(Self::WasiPreview2),
            0x04 => Ok(Self::Custom),
            0xFE => Ok(Self::TestStub),
            _ => Err(RvfError::InvalidEnumValue {
                type_name: "KernelType",
                value: value as u64,
            }),
        }
    }
}

/// Transport mechanism for the kernel's query API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ApiTransport {
    /// HTTP/1.1 over TCP (default).
    TcpHttp = 0x00,
    /// gRPC over TCP (HTTP/2).
    TcpGrpc = 0x01,
    /// VirtIO socket (Firecracker host<->guest).
    Vsock = 0x02,
    /// Shared memory region (for same-host co-location).
    SharedMem = 0x03,
    /// No network API (batch mode only).
    None = 0xFF,
}

impl TryFrom<u8> for ApiTransport {
    type Error = RvfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::TcpHttp),
            0x01 => Ok(Self::TcpGrpc),
            0x02 => Ok(Self::Vsock),
            0x03 => Ok(Self::SharedMem),
            0xFF => Ok(Self::None),
            _ => Err(RvfError::InvalidEnumValue {
                type_name: "ApiTransport",
                value: value as u64,
            }),
        }
    }
}

/// 128-byte header for KERNEL_SEG payloads.
///
/// Follows the standard 64-byte `SegmentHeader`. All multi-byte fields are
/// little-endian on the wire except `api_port` which is network byte order
/// (big-endian) per ADR-030.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct KernelHeader {
    /// Magic: `KERNEL_MAGIC` (0x52564B4E, "RVKN").
    pub kernel_magic: u32,
    /// KernelHeader format version (currently 1).
    pub header_version: u16,
    /// Target architecture (see `KernelArch`).
    pub arch: u8,
    /// Kernel type (see `KernelType`).
    pub kernel_type: u8,
    /// Bitfield flags (see `KERNEL_FLAG_*` constants).
    pub kernel_flags: u32,
    /// Minimum RAM required (MiB).
    pub min_memory_mb: u32,
    /// Virtual address of kernel entry point.
    pub entry_point: u64,
    /// Uncompressed kernel image size (bytes).
    pub image_size: u64,
    /// Compressed kernel image size (bytes).
    pub compressed_size: u64,
    /// Compression algorithm (same enum as `SegmentHeader.compression`).
    pub compression: u8,
    /// API transport (see `ApiTransport`).
    pub api_transport: u8,
    /// Default API port (network byte order).
    pub api_port: u16,
    /// Supported RVF query API version.
    pub api_version: u32,
    /// SHAKE-256-256 of uncompressed kernel image.
    pub image_hash: [u8; 32],
    /// Unique build identifier (UUID v7).
    pub build_id: [u8; 16],
    /// Build time (nanosecond UNIX timestamp).
    pub build_timestamp: u64,
    /// Recommended vCPU count (0 = single).
    pub vcpu_count: u32,
    /// Reserved (must be zero).
    pub reserved_0: u32,
    /// Offset to kernel command line within payload.
    pub cmdline_offset: u64,
    /// Length of kernel command line (bytes).
    pub cmdline_length: u32,
    /// Reserved (must be zero).
    pub reserved_1: u32,
}

// Compile-time assertion: KernelHeader must be exactly 128 bytes.
const _: () = assert!(core::mem::size_of::<KernelHeader>() == 128);

impl KernelHeader {
    /// Serialize the header to a 128-byte little-endian array.
    pub fn to_bytes(&self) -> [u8; 128] {
        let mut buf = [0u8; 128];
        buf[0x00..0x04].copy_from_slice(&self.kernel_magic.to_le_bytes());
        buf[0x04..0x06].copy_from_slice(&self.header_version.to_le_bytes());
        buf[0x06] = self.arch;
        buf[0x07] = self.kernel_type;
        buf[0x08..0x0C].copy_from_slice(&self.kernel_flags.to_le_bytes());
        buf[0x0C..0x10].copy_from_slice(&self.min_memory_mb.to_le_bytes());
        buf[0x10..0x18].copy_from_slice(&self.entry_point.to_le_bytes());
        buf[0x18..0x20].copy_from_slice(&self.image_size.to_le_bytes());
        buf[0x20..0x28].copy_from_slice(&self.compressed_size.to_le_bytes());
        buf[0x28] = self.compression;
        buf[0x29] = self.api_transport;
        buf[0x2A..0x2C].copy_from_slice(&self.api_port.to_be_bytes());
        buf[0x2C..0x30].copy_from_slice(&self.api_version.to_le_bytes());
        buf[0x30..0x50].copy_from_slice(&self.image_hash);
        buf[0x50..0x60].copy_from_slice(&self.build_id);
        buf[0x60..0x68].copy_from_slice(&self.build_timestamp.to_le_bytes());
        buf[0x68..0x6C].copy_from_slice(&self.vcpu_count.to_le_bytes());
        buf[0x6C..0x70].copy_from_slice(&self.reserved_0.to_le_bytes());
        buf[0x70..0x78].copy_from_slice(&self.cmdline_offset.to_le_bytes());
        buf[0x78..0x7C].copy_from_slice(&self.cmdline_length.to_le_bytes());
        buf[0x7C..0x80].copy_from_slice(&self.reserved_1.to_le_bytes());
        buf
    }

    /// Deserialize a `KernelHeader` from a 128-byte slice.
    pub fn from_bytes(data: &[u8; 128]) -> Result<Self, RvfError> {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != KERNEL_MAGIC {
            return Err(RvfError::BadMagic {
                expected: KERNEL_MAGIC,
                got: magic,
            });
        }

        Ok(Self {
            kernel_magic: magic,
            header_version: u16::from_le_bytes([data[0x04], data[0x05]]),
            arch: data[0x06],
            kernel_type: data[0x07],
            kernel_flags: u32::from_le_bytes([data[0x08], data[0x09], data[0x0A], data[0x0B]]),
            min_memory_mb: u32::from_le_bytes([data[0x0C], data[0x0D], data[0x0E], data[0x0F]]),
            entry_point: u64::from_le_bytes([
                data[0x10], data[0x11], data[0x12], data[0x13], data[0x14], data[0x15], data[0x16],
                data[0x17],
            ]),
            image_size: u64::from_le_bytes([
                data[0x18], data[0x19], data[0x1A], data[0x1B], data[0x1C], data[0x1D], data[0x1E],
                data[0x1F],
            ]),
            compressed_size: u64::from_le_bytes([
                data[0x20], data[0x21], data[0x22], data[0x23], data[0x24], data[0x25], data[0x26],
                data[0x27],
            ]),
            compression: data[0x28],
            api_transport: data[0x29],
            api_port: u16::from_be_bytes([data[0x2A], data[0x2B]]),
            api_version: u32::from_le_bytes([data[0x2C], data[0x2D], data[0x2E], data[0x2F]]),
            image_hash: {
                let mut h = [0u8; 32];
                h.copy_from_slice(&data[0x30..0x50]);
                h
            },
            build_id: {
                let mut id = [0u8; 16];
                id.copy_from_slice(&data[0x50..0x60]);
                id
            },
            build_timestamp: u64::from_le_bytes([
                data[0x60], data[0x61], data[0x62], data[0x63], data[0x64], data[0x65], data[0x66],
                data[0x67],
            ]),
            vcpu_count: u32::from_le_bytes([data[0x68], data[0x69], data[0x6A], data[0x6B]]),
            reserved_0: u32::from_le_bytes([data[0x6C], data[0x6D], data[0x6E], data[0x6F]]),
            cmdline_offset: u64::from_le_bytes([
                data[0x70], data[0x71], data[0x72], data[0x73], data[0x74], data[0x75], data[0x76],
                data[0x77],
            ]),
            cmdline_length: u32::from_le_bytes([data[0x78], data[0x79], data[0x7A], data[0x7B]]),
            reserved_1: u32::from_le_bytes([data[0x7C], data[0x7D], data[0x7E], data[0x7F]]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> KernelHeader {
        KernelHeader {
            kernel_magic: KERNEL_MAGIC,
            header_version: 1,
            arch: KernelArch::X86_64 as u8,
            kernel_type: KernelType::Hermit as u8,
            kernel_flags: KERNEL_FLAG_HAS_QUERY_API | KERNEL_FLAG_COMPRESSED,
            min_memory_mb: 32,
            entry_point: 0x0020_0000,
            image_size: 400_000,
            compressed_size: 180_000,
            compression: 2, // ZSTD
            api_transport: ApiTransport::TcpHttp as u8,
            api_port: 8080,
            api_version: 1,
            image_hash: [0xAB; 32],
            build_id: [0xCD; 16],
            build_timestamp: 1_700_000_000_000_000_000,
            vcpu_count: 1,
            reserved_0: 0,
            cmdline_offset: 128,
            cmdline_length: 64,
            reserved_1: 0,
        }
    }

    #[test]
    fn header_size_is_128() {
        assert_eq!(core::mem::size_of::<KernelHeader>(), 128);
    }

    #[test]
    fn magic_bytes_match_ascii() {
        let bytes_be = KERNEL_MAGIC.to_be_bytes();
        assert_eq!(&bytes_be, b"RVKN");
    }

    #[test]
    fn round_trip_serialization() {
        let original = sample_header();
        let bytes = original.to_bytes();
        let decoded = KernelHeader::from_bytes(&bytes).expect("from_bytes should succeed");

        assert_eq!(decoded.kernel_magic, KERNEL_MAGIC);
        assert_eq!(decoded.header_version, 1);
        assert_eq!(decoded.arch, KernelArch::X86_64 as u8);
        assert_eq!(decoded.kernel_type, KernelType::Hermit as u8);
        assert_eq!(
            decoded.kernel_flags,
            KERNEL_FLAG_HAS_QUERY_API | KERNEL_FLAG_COMPRESSED
        );
        assert_eq!(decoded.min_memory_mb, 32);
        assert_eq!(decoded.entry_point, 0x0020_0000);
        assert_eq!(decoded.image_size, 400_000);
        assert_eq!(decoded.compressed_size, 180_000);
        assert_eq!(decoded.compression, 2);
        assert_eq!(decoded.api_transport, ApiTransport::TcpHttp as u8);
        assert_eq!(decoded.api_port, 8080);
        assert_eq!(decoded.api_version, 1);
        assert_eq!(decoded.image_hash, [0xAB; 32]);
        assert_eq!(decoded.build_id, [0xCD; 16]);
        assert_eq!(decoded.build_timestamp, 1_700_000_000_000_000_000);
        assert_eq!(decoded.vcpu_count, 1);
        assert_eq!(decoded.reserved_0, 0);
        assert_eq!(decoded.cmdline_offset, 128);
        assert_eq!(decoded.cmdline_length, 64);
        assert_eq!(decoded.reserved_1, 0);
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut bytes = sample_header().to_bytes();
        bytes[0] = 0x00; // corrupt magic
        let err = KernelHeader::from_bytes(&bytes).unwrap_err();
        match err {
            RvfError::BadMagic { expected, .. } => assert_eq!(expected, KERNEL_MAGIC),
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn field_offsets() {
        let h = sample_header();
        let base = &h as *const _ as usize;

        assert_eq!(&h.kernel_magic as *const _ as usize - base, 0x00);
        assert_eq!(&h.header_version as *const _ as usize - base, 0x04);
        assert_eq!(&h.arch as *const _ as usize - base, 0x06);
        assert_eq!(&h.kernel_type as *const _ as usize - base, 0x07);
        assert_eq!(&h.kernel_flags as *const _ as usize - base, 0x08);
        assert_eq!(&h.min_memory_mb as *const _ as usize - base, 0x0C);
        assert_eq!(&h.entry_point as *const _ as usize - base, 0x10);
        assert_eq!(&h.image_size as *const _ as usize - base, 0x18);
        assert_eq!(&h.compressed_size as *const _ as usize - base, 0x20);
        assert_eq!(&h.compression as *const _ as usize - base, 0x28);
        assert_eq!(&h.api_transport as *const _ as usize - base, 0x29);
        assert_eq!(&h.api_port as *const _ as usize - base, 0x2A);
        assert_eq!(&h.api_version as *const _ as usize - base, 0x2C);
        assert_eq!(&h.image_hash as *const _ as usize - base, 0x30);
        assert_eq!(&h.build_id as *const _ as usize - base, 0x50);
        assert_eq!(&h.build_timestamp as *const _ as usize - base, 0x60);
        assert_eq!(&h.vcpu_count as *const _ as usize - base, 0x68);
        assert_eq!(&h.reserved_0 as *const _ as usize - base, 0x6C);
        assert_eq!(&h.cmdline_offset as *const _ as usize - base, 0x70);
        assert_eq!(&h.cmdline_length as *const _ as usize - base, 0x78);
        assert_eq!(&h.reserved_1 as *const _ as usize - base, 0x7C);
    }

    #[test]
    fn kernel_arch_try_from() {
        assert_eq!(KernelArch::try_from(0x00), Ok(KernelArch::X86_64));
        assert_eq!(KernelArch::try_from(0x01), Ok(KernelArch::Aarch64));
        assert_eq!(KernelArch::try_from(0x02), Ok(KernelArch::Riscv64));
        assert_eq!(KernelArch::try_from(0xFE), Ok(KernelArch::Universal));
        assert_eq!(KernelArch::try_from(0xFF), Ok(KernelArch::Unknown));
        assert!(KernelArch::try_from(0x03).is_err());
        assert!(KernelArch::try_from(0x80).is_err());
    }

    #[test]
    fn kernel_type_try_from() {
        assert_eq!(KernelType::try_from(0x00), Ok(KernelType::Hermit));
        assert_eq!(KernelType::try_from(0x01), Ok(KernelType::MicroLinux));
        assert_eq!(KernelType::try_from(0x02), Ok(KernelType::Asterinas));
        assert_eq!(KernelType::try_from(0x03), Ok(KernelType::WasiPreview2));
        assert_eq!(KernelType::try_from(0x04), Ok(KernelType::Custom));
        assert_eq!(KernelType::try_from(0xFE), Ok(KernelType::TestStub));
        assert!(KernelType::try_from(0x05).is_err());
        assert!(KernelType::try_from(0xFF).is_err());
    }

    #[test]
    fn api_transport_try_from() {
        assert_eq!(ApiTransport::try_from(0x00), Ok(ApiTransport::TcpHttp));
        assert_eq!(ApiTransport::try_from(0x01), Ok(ApiTransport::TcpGrpc));
        assert_eq!(ApiTransport::try_from(0x02), Ok(ApiTransport::Vsock));
        assert_eq!(ApiTransport::try_from(0x03), Ok(ApiTransport::SharedMem));
        assert_eq!(ApiTransport::try_from(0xFF), Ok(ApiTransport::None));
        assert!(ApiTransport::try_from(0x04).is_err());
        assert!(ApiTransport::try_from(0x80).is_err());
    }

    #[test]
    fn kernel_flags_bit_positions() {
        assert_eq!(KERNEL_FLAG_REQUIRES_TEE, 0x0001);
        assert_eq!(KERNEL_FLAG_REQUIRES_KVM, 0x0002);
        assert_eq!(KERNEL_FLAG_REQUIRES_UEFI, 0x0004);
        assert_eq!(KERNEL_FLAG_HAS_NETWORKING, 0x0008);
        assert_eq!(KERNEL_FLAG_HAS_QUERY_API, 0x0010);
        assert_eq!(KERNEL_FLAG_HAS_INGEST_API, 0x0020);
        assert_eq!(KERNEL_FLAG_HAS_ADMIN_API, 0x0040);
        assert_eq!(KERNEL_FLAG_ATTESTATION_READY, 0x0080);
        assert_eq!(KERNEL_FLAG_SIGNED, 0x0100);
        assert_eq!(KERNEL_FLAG_MEASURED, 0x0200);
        assert_eq!(KERNEL_FLAG_COMPRESSED, 0x0400);
        assert_eq!(KERNEL_FLAG_RELOCATABLE, 0x0800);
        assert_eq!(KERNEL_FLAG_HAS_VIRTIO_NET, 0x1000);
        assert_eq!(KERNEL_FLAG_HAS_VIRTIO_BLK, 0x2000);
        assert_eq!(KERNEL_FLAG_HAS_VSOCK, 0x4000);
    }

    #[test]
    fn api_port_network_byte_order() {
        let mut h = sample_header();
        h.api_port = 0x1F90; // 8080
        let bytes = h.to_bytes();
        // api_port at offset 0x2A, big-endian
        assert_eq!(bytes[0x2A], 0x1F);
        assert_eq!(bytes[0x2B], 0x90);
        let decoded = KernelHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.api_port, 0x1F90);
    }

    #[test]
    fn zero_filled_reserved_fields() {
        let h = sample_header();
        let bytes = h.to_bytes();
        // reserved_0 at 0x6C..0x70 should be zero
        assert_eq!(&bytes[0x6C..0x70], &[0, 0, 0, 0]);
        // reserved_1 at 0x7C..0x80 should be zero
        assert_eq!(&bytes[0x7C..0x80], &[0, 0, 0, 0]);
    }
}
