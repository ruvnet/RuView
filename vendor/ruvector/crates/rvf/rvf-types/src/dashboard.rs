//! DASHBOARD_SEG (0x11) types for the RVF computational container.
//!
//! Defines the 64-byte `DashboardHeader` and associated constants per ADR-040.
//! The DASHBOARD_SEG embeds a pre-built web dashboard (e.g. Vite + Three.js)
//! that the RVF HTTP server can serve at `/`.

use crate::error::RvfError;

/// Magic number for `DashboardHeader`: "RVDB" in big-endian.
pub const DASHBOARD_MAGIC: u32 = 0x5256_4442;

/// Maximum dashboard bundle size (64 MiB).
pub const DASHBOARD_MAX_SIZE: u64 = 64 * 1024 * 1024;

/// 64-byte header for DASHBOARD_SEG payloads.
///
/// Follows the standard 64-byte `SegmentHeader`. All multi-byte fields are
/// little-endian on the wire.
///
/// Payload layout after header:
/// `[entry_path_bytes | file_table | file_data...]`
///
/// File table: array of `(path_len: u16, data_offset: u64, data_size: u64, path_bytes: [u8])`
/// File data: concatenated raw file contents.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct DashboardHeader {
    /// Magic: `DASHBOARD_MAGIC` (0x52564442, "RVDB").
    pub dashboard_magic: u32,
    /// DashboardHeader format version (currently 1).
    pub header_version: u16,
    /// UI framework: 0=threejs, 1=react, 2=custom.
    pub ui_framework: u8,
    /// Compression: 0=none, 1=gzip, 2=brotli.
    pub compression: u8,
    /// Total uncompressed bundle size in bytes.
    pub bundle_size: u64,
    /// Number of files in the bundle.
    pub file_count: u32,
    /// Length of the entry point path string.
    pub entry_path_len: u16,
    /// Reserved padding.
    pub reserved: u16,
    /// Build timestamp (unix epoch seconds).
    pub build_timestamp: u64,
    /// SHAKE-256-256 of the entire bundle payload.
    pub content_hash: [u8; 32],
}

// Compile-time assertion: DashboardHeader must be exactly 64 bytes.
const _: () = assert!(core::mem::size_of::<DashboardHeader>() == 64);

impl DashboardHeader {
    /// Serialize the header to a 64-byte little-endian array.
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0x00..0x04].copy_from_slice(&self.dashboard_magic.to_le_bytes());
        buf[0x04..0x06].copy_from_slice(&self.header_version.to_le_bytes());
        buf[0x06] = self.ui_framework;
        buf[0x07] = self.compression;
        buf[0x08..0x10].copy_from_slice(&self.bundle_size.to_le_bytes());
        buf[0x10..0x14].copy_from_slice(&self.file_count.to_le_bytes());
        buf[0x14..0x16].copy_from_slice(&self.entry_path_len.to_le_bytes());
        buf[0x16..0x18].copy_from_slice(&self.reserved.to_le_bytes());
        buf[0x18..0x20].copy_from_slice(&self.build_timestamp.to_le_bytes());
        buf[0x20..0x40].copy_from_slice(&self.content_hash);
        buf
    }

    /// Deserialize a `DashboardHeader` from a 64-byte slice.
    pub fn from_bytes(data: &[u8; 64]) -> Result<Self, RvfError> {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != DASHBOARD_MAGIC {
            return Err(RvfError::BadMagic {
                expected: DASHBOARD_MAGIC,
                got: magic,
            });
        }

        Ok(Self {
            dashboard_magic: magic,
            header_version: u16::from_le_bytes([data[0x04], data[0x05]]),
            ui_framework: data[0x06],
            compression: data[0x07],
            bundle_size: u64::from_le_bytes([
                data[0x08], data[0x09], data[0x0A], data[0x0B], data[0x0C], data[0x0D], data[0x0E],
                data[0x0F],
            ]),
            file_count: u32::from_le_bytes([data[0x10], data[0x11], data[0x12], data[0x13]]),
            entry_path_len: u16::from_le_bytes([data[0x14], data[0x15]]),
            reserved: u16::from_le_bytes([data[0x16], data[0x17]]),
            build_timestamp: u64::from_le_bytes([
                data[0x18], data[0x19], data[0x1A], data[0x1B], data[0x1C], data[0x1D], data[0x1E],
                data[0x1F],
            ]),
            content_hash: {
                let mut h = [0u8; 32];
                h.copy_from_slice(&data[0x20..0x40]);
                h
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> DashboardHeader {
        DashboardHeader {
            dashboard_magic: DASHBOARD_MAGIC,
            header_version: 1,
            ui_framework: 0, // threejs
            compression: 0,  // none
            bundle_size: 524288,
            file_count: 12,
            entry_path_len: 10,
            reserved: 0,
            build_timestamp: 1_700_000_000,
            content_hash: [0xAB; 32],
        }
    }

    #[test]
    fn header_size_is_64() {
        assert_eq!(core::mem::size_of::<DashboardHeader>(), 64);
    }

    #[test]
    fn magic_bytes_match_ascii() {
        let bytes_be = DASHBOARD_MAGIC.to_be_bytes();
        assert_eq!(&bytes_be, b"RVDB");
    }

    #[test]
    fn round_trip_serialization() {
        let original = sample_header();
        let bytes = original.to_bytes();
        let decoded = DashboardHeader::from_bytes(&bytes).expect("from_bytes should succeed");

        assert_eq!(decoded.dashboard_magic, DASHBOARD_MAGIC);
        assert_eq!(decoded.header_version, 1);
        assert_eq!(decoded.ui_framework, 0);
        assert_eq!(decoded.compression, 0);
        assert_eq!(decoded.bundle_size, 524288);
        assert_eq!(decoded.file_count, 12);
        assert_eq!(decoded.entry_path_len, 10);
        assert_eq!(decoded.reserved, 0);
        assert_eq!(decoded.build_timestamp, 1_700_000_000);
        assert_eq!(decoded.content_hash, [0xAB; 32]);
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut bytes = sample_header().to_bytes();
        bytes[0] = 0x00; // corrupt magic
        let err = DashboardHeader::from_bytes(&bytes).unwrap_err();
        match err {
            RvfError::BadMagic { expected, .. } => assert_eq!(expected, DASHBOARD_MAGIC),
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn field_offsets() {
        let h = sample_header();
        let base = &h as *const _ as usize;

        assert_eq!(&h.dashboard_magic as *const _ as usize - base, 0x00);
        assert_eq!(&h.header_version as *const _ as usize - base, 0x04);
        assert_eq!(&h.ui_framework as *const _ as usize - base, 0x06);
        assert_eq!(&h.compression as *const _ as usize - base, 0x07);
        assert_eq!(&h.bundle_size as *const _ as usize - base, 0x08);
        assert_eq!(&h.file_count as *const _ as usize - base, 0x10);
        assert_eq!(&h.entry_path_len as *const _ as usize - base, 0x14);
        assert_eq!(&h.reserved as *const _ as usize - base, 0x16);
        assert_eq!(&h.build_timestamp as *const _ as usize - base, 0x18);
        assert_eq!(&h.content_hash as *const _ as usize - base, 0x20);
    }

    #[test]
    fn large_bundle_size_round_trip() {
        let mut h = sample_header();
        h.bundle_size = DASHBOARD_MAX_SIZE;
        h.file_count = 500;
        let bytes = h.to_bytes();
        let decoded = DashboardHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.bundle_size, DASHBOARD_MAX_SIZE);
        assert_eq!(decoded.file_count, 500);
    }
}
