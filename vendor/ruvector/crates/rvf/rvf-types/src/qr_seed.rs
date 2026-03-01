//! QR Cognitive Seed types for ADR-034.
//!
//! Defines the RVQS (RuVector QR Seed) binary format — a compact
//! self-bootstrapping cognitive payload that fits in a single QR code.
//! Scan and mount a portable brain.

/// RVQS magic: "RVQS" in ASCII = 0x52565153.
pub const SEED_MAGIC: u32 = 0x5256_5153;

/// Maximum payload that fits in QR Version 40, Low EC.
pub const QR_MAX_BYTES: usize = 2_953;

// ---- Seed Flags (bit positions) ----

/// Embedded WASM microkernel present.
pub const SEED_HAS_MICROKERNEL: u16 = 0x0001;
/// Progressive download manifest present.
pub const SEED_HAS_DOWNLOAD: u16 = 0x0002;
/// Payload is signed.
pub const SEED_SIGNED: u16 = 0x0004;
/// Seed is useful without network access.
pub const SEED_OFFLINE_CAPABLE: u16 = 0x0008;
/// Payload is encrypted.
pub const SEED_ENCRYPTED: u16 = 0x0010;
/// Microkernel is Brotli-compressed.
pub const SEED_COMPRESSED: u16 = 0x0020;
/// Seed contains inline vector data.
pub const SEED_HAS_VECTORS: u16 = 0x0040;
/// Seed can upgrade itself via streaming.
pub const SEED_STREAM_UPGRADE: u16 = 0x0080;

/// Header size in bytes (fixed).
pub const SEED_HEADER_SIZE: usize = 64;

/// RVQS header — the first 64 bytes of any QR Cognitive Seed.
///
/// Contains everything needed to verify and bootstrap the seed
/// before any network access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct SeedHeader {
    /// Magic number: must be SEED_MAGIC.
    pub seed_magic: u32,
    /// Seed format version.
    pub seed_version: u16,
    /// Seed flags bitfield.
    pub flags: u16,
    /// Unique identifier for this seed.
    pub file_id: [u8; 8],
    /// Expected total vectors when fully loaded.
    pub total_vector_count: u32,
    /// Vector dimensionality.
    pub dimension: u16,
    /// Base data type (DataType enum).
    pub base_dtype: u8,
    /// Domain profile id.
    pub profile_id: u8,
    /// Seed creation timestamp (nanoseconds since epoch).
    pub created_ns: u64,
    /// Offset to WASM microkernel data within the seed payload.
    pub microkernel_offset: u32,
    /// Compressed microkernel size in bytes.
    pub microkernel_size: u32,
    /// Offset to download manifest within the seed payload.
    pub download_manifest_offset: u32,
    /// Download manifest size in bytes.
    pub download_manifest_size: u32,
    /// Signature algorithm (0=Ed25519, 1=ML-DSA-65).
    pub sig_algo: u16,
    /// Signature byte length.
    pub sig_length: u16,
    /// Total seed payload size in bytes.
    pub total_seed_size: u32,
    /// SHAKE-256-64 of the complete expanded RVF file.
    pub content_hash: [u8; 8],
}

const _: () = assert!(core::mem::size_of::<SeedHeader>() == SEED_HEADER_SIZE);

impl SeedHeader {
    /// Check if the magic field is valid.
    pub const fn is_valid_magic(&self) -> bool {
        self.seed_magic == SEED_MAGIC
    }

    /// Check if the seed has an embedded microkernel.
    pub const fn has_microkernel(&self) -> bool {
        self.flags & SEED_HAS_MICROKERNEL != 0
    }

    /// Check if the seed has a download manifest.
    pub const fn has_download_manifest(&self) -> bool {
        self.flags & SEED_HAS_DOWNLOAD != 0
    }

    /// Check if the seed is signed.
    pub const fn is_signed(&self) -> bool {
        self.flags & SEED_SIGNED != 0
    }

    /// Check if the seed is offline-capable.
    pub const fn is_offline_capable(&self) -> bool {
        self.flags & SEED_OFFLINE_CAPABLE != 0
    }

    /// Check if the seed fits in a single QR code.
    pub const fn fits_in_qr(&self) -> bool {
        (self.total_seed_size as usize) <= QR_MAX_BYTES
    }

    /// Serialize the header to 64 bytes (little-endian).
    pub fn to_bytes(&self) -> [u8; SEED_HEADER_SIZE] {
        let mut buf = [0u8; SEED_HEADER_SIZE];
        buf[0x00..0x04].copy_from_slice(&self.seed_magic.to_le_bytes());
        buf[0x04..0x06].copy_from_slice(&self.seed_version.to_le_bytes());
        buf[0x06..0x08].copy_from_slice(&self.flags.to_le_bytes());
        buf[0x08..0x10].copy_from_slice(&self.file_id);
        buf[0x10..0x14].copy_from_slice(&self.total_vector_count.to_le_bytes());
        buf[0x14..0x16].copy_from_slice(&self.dimension.to_le_bytes());
        buf[0x16] = self.base_dtype;
        buf[0x17] = self.profile_id;
        buf[0x18..0x20].copy_from_slice(&self.created_ns.to_le_bytes());
        buf[0x20..0x24].copy_from_slice(&self.microkernel_offset.to_le_bytes());
        buf[0x24..0x28].copy_from_slice(&self.microkernel_size.to_le_bytes());
        buf[0x28..0x2C].copy_from_slice(&self.download_manifest_offset.to_le_bytes());
        buf[0x2C..0x30].copy_from_slice(&self.download_manifest_size.to_le_bytes());
        buf[0x30..0x32].copy_from_slice(&self.sig_algo.to_le_bytes());
        buf[0x32..0x34].copy_from_slice(&self.sig_length.to_le_bytes());
        buf[0x34..0x38].copy_from_slice(&self.total_seed_size.to_le_bytes());
        buf[0x38..0x40].copy_from_slice(&self.content_hash);
        buf
    }

    /// Deserialize from 64 bytes (little-endian).
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::error::RvfError> {
        if buf.len() < SEED_HEADER_SIZE {
            return Err(crate::error::RvfError::SizeMismatch {
                expected: SEED_HEADER_SIZE,
                got: buf.len(),
            });
        }

        let seed_magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if seed_magic != SEED_MAGIC {
            return Err(crate::error::RvfError::BadMagic {
                expected: SEED_MAGIC,
                got: seed_magic,
            });
        }

        let mut file_id = [0u8; 8];
        file_id.copy_from_slice(&buf[0x08..0x10]);
        let mut content_hash = [0u8; 8];
        content_hash.copy_from_slice(&buf[0x38..0x40]);

        Ok(Self {
            seed_magic,
            seed_version: u16::from_le_bytes([buf[0x04], buf[0x05]]),
            flags: u16::from_le_bytes([buf[0x06], buf[0x07]]),
            file_id,
            total_vector_count: u32::from_le_bytes([buf[0x10], buf[0x11], buf[0x12], buf[0x13]]),
            dimension: u16::from_le_bytes([buf[0x14], buf[0x15]]),
            base_dtype: buf[0x16],
            profile_id: buf[0x17],
            created_ns: u64::from_le_bytes([
                buf[0x18], buf[0x19], buf[0x1A], buf[0x1B], buf[0x1C], buf[0x1D], buf[0x1E],
                buf[0x1F],
            ]),
            microkernel_offset: u32::from_le_bytes([buf[0x20], buf[0x21], buf[0x22], buf[0x23]]),
            microkernel_size: u32::from_le_bytes([buf[0x24], buf[0x25], buf[0x26], buf[0x27]]),
            download_manifest_offset: u32::from_le_bytes([
                buf[0x28], buf[0x29], buf[0x2A], buf[0x2B],
            ]),
            download_manifest_size: u32::from_le_bytes([
                buf[0x2C], buf[0x2D], buf[0x2E], buf[0x2F],
            ]),
            sig_algo: u16::from_le_bytes([buf[0x30], buf[0x31]]),
            sig_length: u16::from_le_bytes([buf[0x32], buf[0x33]]),
            total_seed_size: u32::from_le_bytes([buf[0x34], buf[0x35], buf[0x36], buf[0x37]]),
            content_hash,
        })
    }
}

// ---- Download Manifest TLV Tags ----

/// Primary download host.
pub const DL_TAG_HOST_PRIMARY: u16 = 0x0001;
/// Fallback download host.
pub const DL_TAG_HOST_FALLBACK: u16 = 0x0002;
/// SHAKE-256-256 hash of the full RVF file.
pub const DL_TAG_CONTENT_HASH: u16 = 0x0003;
/// Expected total file size.
pub const DL_TAG_TOTAL_SIZE: u16 = 0x0004;
/// Progressive layer manifest.
pub const DL_TAG_LAYER_MANIFEST: u16 = 0x0005;
/// Ephemeral session token.
pub const DL_TAG_SESSION_TOKEN: u16 = 0x0006;
/// Token TTL in seconds.
pub const DL_TAG_TTL: u16 = 0x0007;
/// TLS certificate pin (SHA-256 of SPKI).
pub const DL_TAG_CERT_PIN: u16 = 0x0008;

/// A single host entry in the download manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HostEntry {
    /// Download URL (HTTPS).
    pub url: [u8; 128],
    /// Actual URL length within the buffer.
    pub url_length: u16,
    /// Priority (lower = preferred).
    pub priority: u16,
    /// Geographic region hint.
    pub region: u16,
    /// SHAKE-256-128 of host's public key.
    pub host_key_hash: [u8; 16],
}

impl HostEntry {
    /// Get the URL as a string slice.
    pub fn url_str(&self) -> Option<&str> {
        core::str::from_utf8(&self.url[..self.url_length as usize]).ok()
    }

    /// Encode to bytes.
    pub fn to_bytes(&self) -> [u8; 150] {
        let mut buf = [0u8; 150];
        buf[0..2].copy_from_slice(&self.url_length.to_le_bytes());
        buf[2..130].copy_from_slice(&self.url);
        buf[130..132].copy_from_slice(&self.priority.to_le_bytes());
        buf[132..134].copy_from_slice(&self.region.to_le_bytes());
        buf[134..150].copy_from_slice(&self.host_key_hash);
        buf
    }
}

/// A single layer entry in the progressive download manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct LayerEntry {
    /// Byte offset in the full RVF file.
    pub offset: u32,
    /// Layer size in bytes.
    pub size: u32,
    /// SHAKE-256-128 content hash.
    pub content_hash: [u8; 16],
    /// Layer identifier.
    pub layer_id: u8,
    /// Download priority (0 = immediate).
    pub priority: u8,
    /// 1 = required before first query.
    pub required: u8,
    /// Padding.
    pub _pad: u8,
}

const _: () = assert!(core::mem::size_of::<LayerEntry>() == 28);

/// Well-known layer identifiers.
pub mod layer_id {
    /// Level 0 manifest (4 KB).
    pub const LEVEL0: u8 = 0;
    /// Hot cache (centroids + entry points).
    pub const HOT_CACHE: u8 = 1;
    /// HNSW Layer A (recall >= 0.70).
    pub const HNSW_LAYER_A: u8 = 2;
    /// Quantization dictionaries.
    pub const QUANT_DICT: u8 = 3;
    /// HNSW Layer B (recall >= 0.85).
    pub const HNSW_LAYER_B: u8 = 4;
    /// Full vectors (warm tier).
    pub const FULL_VECTORS: u8 = 5;
    /// HNSW Layer C (recall >= 0.95).
    pub const HNSW_LAYER_C: u8 = 6;
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;

    fn test_header() -> SeedHeader {
        SeedHeader {
            seed_magic: SEED_MAGIC,
            seed_version: 1,
            flags: SEED_HAS_MICROKERNEL
                | SEED_HAS_DOWNLOAD
                | SEED_SIGNED
                | SEED_COMPRESSED
                | SEED_STREAM_UPGRADE,
            file_id: [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
            total_vector_count: 100_000,
            dimension: 384,
            base_dtype: 1, // F16
            profile_id: 2, // Hot
            created_ns: 1_700_000_000_000_000_000,
            microkernel_offset: 64,
            microkernel_size: 2100,
            download_manifest_offset: 2164,
            download_manifest_size: 512,
            sig_algo: 0, // Ed25519
            sig_length: 64,
            total_seed_size: 2740,
            content_hash: [0xAB; 8],
        }
    }

    #[test]
    fn header_size_is_64() {
        assert_eq!(core::mem::size_of::<SeedHeader>(), 64);
    }

    #[test]
    fn layer_entry_size_is_28() {
        assert_eq!(core::mem::size_of::<LayerEntry>(), 28);
    }

    #[test]
    fn header_round_trip() {
        let header = test_header();
        let bytes = header.to_bytes();
        let decoded = SeedHeader::from_bytes(&bytes).unwrap();
        assert_eq!(header, decoded);
    }

    #[test]
    fn header_invalid_magic() {
        let mut bytes = test_header().to_bytes();
        bytes[0] = 0xFF; // Corrupt magic.
        assert!(SeedHeader::from_bytes(&bytes).is_err());
    }

    #[test]
    fn header_too_short() {
        let bytes = [0u8; 32];
        assert!(SeedHeader::from_bytes(&bytes).is_err());
    }

    #[test]
    fn header_flags() {
        let header = test_header();
        assert!(header.has_microkernel());
        assert!(header.has_download_manifest());
        assert!(header.is_signed());
        assert!(!header.is_offline_capable());
        assert!(header.fits_in_qr());
    }

    #[test]
    fn header_fits_in_qr() {
        let mut header = test_header();
        header.total_seed_size = 2953; // Max QR capacity.
        assert!(header.fits_in_qr());
        header.total_seed_size = 2954;
        assert!(!header.fits_in_qr());
    }

    #[test]
    fn seed_magic_is_rvqs() {
        let bytes = SEED_MAGIC.to_be_bytes();
        assert_eq!(&bytes, b"RVQS");
    }

    #[test]
    fn flag_bit_positions() {
        assert_eq!(SEED_HAS_MICROKERNEL, 1 << 0);
        assert_eq!(SEED_HAS_DOWNLOAD, 1 << 1);
        assert_eq!(SEED_SIGNED, 1 << 2);
        assert_eq!(SEED_OFFLINE_CAPABLE, 1 << 3);
        assert_eq!(SEED_ENCRYPTED, 1 << 4);
        assert_eq!(SEED_COMPRESSED, 1 << 5);
        assert_eq!(SEED_HAS_VECTORS, 1 << 6);
        assert_eq!(SEED_STREAM_UPGRADE, 1 << 7);
    }
}
