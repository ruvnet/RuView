//! Level 0 root manifest and hotset pointer types.
//!
//! The root manifest is always the last 4096 bytes of the most recent
//! MANIFEST_SEG. Its fixed size enables instant location via `seek(EOF - 4096)`.

use crate::constants::ROOT_MANIFEST_MAGIC;

/// Inline hotset pointer for HNSW entry points.
///
/// Offset 0x038 in Level0Root.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct EntrypointPtr {
    /// Byte offset to the segment containing HNSW entry points.
    pub seg_offset: u64,
    /// Block offset within that segment.
    pub block_offset: u32,
    /// Number of entry points.
    pub count: u32,
}

/// Inline hotset pointer for top-layer adjacency.
///
/// Offset 0x048 in Level0Root.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct TopLayerPtr {
    /// Byte offset to the segment with top-layer adjacency.
    pub seg_offset: u64,
    /// Block offset within the segment.
    pub block_offset: u32,
    /// Number of nodes in the top layer.
    pub node_count: u32,
}

/// Inline hotset pointer for cluster centroids / pivots.
///
/// Offset 0x058 in Level0Root.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct CentroidPtr {
    /// Byte offset to the segment with cluster centroids.
    pub seg_offset: u64,
    /// Block offset within the segment.
    pub block_offset: u32,
    /// Number of centroids.
    pub count: u32,
}

/// Inline hotset pointer for quantization dictionary.
///
/// Offset 0x068 in Level0Root.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct QuantDictPtr {
    /// Byte offset to the quantization dictionary segment.
    pub seg_offset: u64,
    /// Block offset within the segment.
    pub block_offset: u32,
    /// Dictionary size in bytes.
    pub size: u32,
}

/// Inline hotset pointer for the hot vector cache (HOT_SEG).
///
/// Offset 0x078 in Level0Root.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct HotCachePtr {
    /// Byte offset to the HOT_SEG with interleaved hot vectors.
    pub seg_offset: u64,
    /// Block offset within the segment.
    pub block_offset: u32,
    /// Number of vectors in the hot cache.
    pub vector_count: u32,
}

/// Inline hotset pointer for prefetch hint table.
///
/// Offset 0x088 in Level0Root.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct PrefetchMapPtr {
    /// Byte offset to the prefetch hint table.
    pub offset: u64,
    /// Number of prefetch entries.
    pub entries: u32,
    /// Padding to align to 16 bytes (matches other hotset pointers).
    pub _pad: u32,
}

/// The Level 0 root manifest (exactly 4096 bytes).
///
/// Always located at the last 4096 bytes of the most recent MANIFEST_SEG.
/// Its fixed size enables instant boot: `seek(EOF - 4096)`.
///
/// ## Binary layout
///
/// | Offset | Size | Field |
/// |--------|------|-------|
/// | 0x000  | 4    | magic (0x52564D30 "RVM0") |
/// | 0x004  | 2    | version |
/// | 0x006  | 2    | flags |
/// | 0x008  | 8    | l1_manifest_offset |
/// | 0x010  | 8    | l1_manifest_length |
/// | 0x018  | 8    | total_vector_count |
/// | 0x020  | 2    | dimension |
/// | 0x022  | 1    | base_dtype |
/// | 0x023  | 1    | profile_id |
/// | 0x024  | 4    | epoch |
/// | 0x028  | 8    | created_ns |
/// | 0x030  | 8    | modified_ns |
/// | 0x038  | 16   | entrypoint_ptr |
/// | 0x048  | 16   | toplayer_ptr |
/// | 0x058  | 16   | centroid_ptr |
/// | 0x068  | 16   | quantdict_ptr |
/// | 0x078  | 16   | hot_cache_ptr |
/// | 0x088  | 16   | prefetch_map_ptr (includes 4B padding) |
/// | 0x098  | 2    | sig_algo |
/// | 0x09A  | 2    | sig_length |
/// | 0x09C  | 3684 | signature_buf |
/// | 0xF00  | 252  | reserved |
/// | 0xFFC  | 4    | root_checksum |
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct Level0Root {
    // ---- Basic header (0x000 - 0x037) ----
    /// Magic number: must be `0x52564D30` ("RVM0").
    pub magic: u32,
    /// Root manifest version.
    pub version: u16,
    /// Root manifest flags.
    pub flags: u16,
    /// Byte offset to the Level 1 manifest segment.
    pub l1_manifest_offset: u64,
    /// Byte length of the Level 1 manifest segment.
    pub l1_manifest_length: u64,
    /// Total vectors across all segments.
    pub total_vector_count: u64,
    /// Vector dimensionality.
    pub dimension: u16,
    /// Base data type enum (see `DataType`).
    pub base_dtype: u8,
    /// Domain profile id.
    pub profile_id: u8,
    /// Current overlay epoch number.
    pub epoch: u32,
    /// File creation timestamp (nanoseconds).
    pub created_ns: u64,
    /// Last modification timestamp (nanoseconds).
    pub modified_ns: u64,

    // ---- Hotset pointers (0x038 - 0x093) ----
    /// HNSW entry points.
    pub entrypoint: EntrypointPtr,
    /// Top-layer adjacency.
    pub toplayer: TopLayerPtr,
    /// Cluster centroids / pivots.
    pub centroid: CentroidPtr,
    /// Quantization dictionary.
    pub quantdict: QuantDictPtr,
    /// Hot vector cache (HOT_SEG).
    pub hot_cache: HotCachePtr,
    /// Prefetch hint table.
    pub prefetch_map: PrefetchMapPtr,

    // ---- Crypto (0x094 - 0x097 + signature) ----
    /// Manifest signature algorithm.
    pub sig_algo: u16,
    /// Signature byte length.
    pub sig_length: u16,
    /// Signature bytes (up to 3688 bytes; only first `sig_length` are meaningful).
    pub signature_buf: [u8; Self::SIG_BUF_SIZE],

    // ---- Reserved + checksum (0xF00 - 0xFFF) ----
    /// Reserved / zero-padded area.
    pub reserved: [u8; 252],
    /// CRC32C of bytes 0x000 through 0xFFB.
    pub root_checksum: u32,
}

// Compile-time assertion: Level0Root must be exactly 4096 bytes.
const _: () = assert!(core::mem::size_of::<Level0Root>() == 4096);

impl Level0Root {
    /// Size of the signature buffer within the root manifest.
    /// From offset 0x09C to 0xEFF inclusive = 3684 bytes.
    pub const SIG_BUF_SIZE: usize = 3684;

    /// Create a zeroed root manifest with only the magic set.
    pub const fn zeroed() -> Self {
        Self {
            magic: ROOT_MANIFEST_MAGIC,
            version: 0,
            flags: 0,
            l1_manifest_offset: 0,
            l1_manifest_length: 0,
            total_vector_count: 0,
            dimension: 0,
            base_dtype: 0,
            profile_id: 0,
            epoch: 0,
            created_ns: 0,
            modified_ns: 0,
            entrypoint: EntrypointPtr {
                seg_offset: 0,
                block_offset: 0,
                count: 0,
            },
            toplayer: TopLayerPtr {
                seg_offset: 0,
                block_offset: 0,
                node_count: 0,
            },
            centroid: CentroidPtr {
                seg_offset: 0,
                block_offset: 0,
                count: 0,
            },
            quantdict: QuantDictPtr {
                seg_offset: 0,
                block_offset: 0,
                size: 0,
            },
            hot_cache: HotCachePtr {
                seg_offset: 0,
                block_offset: 0,
                vector_count: 0,
            },
            prefetch_map: PrefetchMapPtr {
                offset: 0,
                entries: 0,
                _pad: 0,
            },
            sig_algo: 0,
            sig_length: 0,
            signature_buf: [0u8; Self::SIG_BUF_SIZE],
            reserved: [0u8; 252],
            root_checksum: 0,
        }
    }

    /// Check whether the magic field matches the expected value.
    #[inline]
    pub const fn is_valid_magic(&self) -> bool {
        self.magic == ROOT_MANIFEST_MAGIC
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level0_root_size_is_4096() {
        assert_eq!(core::mem::size_of::<Level0Root>(), 4096);
    }

    #[test]
    fn zeroed_has_valid_magic() {
        let root = Level0Root::zeroed();
        assert!(root.is_valid_magic());
    }

    #[test]
    fn field_offsets() {
        let root = Level0Root::zeroed();
        let base = core::ptr::addr_of!(root) as usize;

        // Use addr_of! for packed struct fields to avoid UB.
        let magic_off = core::ptr::addr_of!(root.magic) as usize - base;
        let version_off = core::ptr::addr_of!(root.version) as usize - base;
        let flags_off = core::ptr::addr_of!(root.flags) as usize - base;
        let l1_offset_off = core::ptr::addr_of!(root.l1_manifest_offset) as usize - base;
        let l1_length_off = core::ptr::addr_of!(root.l1_manifest_length) as usize - base;
        let total_vec_off = core::ptr::addr_of!(root.total_vector_count) as usize - base;
        let dim_off = core::ptr::addr_of!(root.dimension) as usize - base;
        let dtype_off = core::ptr::addr_of!(root.base_dtype) as usize - base;
        let profile_off = core::ptr::addr_of!(root.profile_id) as usize - base;
        let epoch_off = core::ptr::addr_of!(root.epoch) as usize - base;
        let created_off = core::ptr::addr_of!(root.created_ns) as usize - base;
        let modified_off = core::ptr::addr_of!(root.modified_ns) as usize - base;
        let entry_off = core::ptr::addr_of!(root.entrypoint) as usize - base;
        let toplayer_off = core::ptr::addr_of!(root.toplayer) as usize - base;
        let centroid_off = core::ptr::addr_of!(root.centroid) as usize - base;
        let quantdict_off = core::ptr::addr_of!(root.quantdict) as usize - base;
        let hot_cache_off = core::ptr::addr_of!(root.hot_cache) as usize - base;
        let prefetch_off = core::ptr::addr_of!(root.prefetch_map) as usize - base;
        let sig_algo_off = core::ptr::addr_of!(root.sig_algo) as usize - base;
        let sig_len_off = core::ptr::addr_of!(root.sig_length) as usize - base;
        let sig_buf_off = core::ptr::addr_of!(root.signature_buf) as usize - base;
        let reserved_off = core::ptr::addr_of!(root.reserved) as usize - base;
        let checksum_off = core::ptr::addr_of!(root.root_checksum) as usize - base;

        assert_eq!(magic_off, 0x000);
        assert_eq!(version_off, 0x004);
        assert_eq!(flags_off, 0x006);
        assert_eq!(l1_offset_off, 0x008);
        assert_eq!(l1_length_off, 0x010);
        assert_eq!(total_vec_off, 0x018);
        assert_eq!(dim_off, 0x020);
        assert_eq!(dtype_off, 0x022);
        assert_eq!(profile_off, 0x023);
        assert_eq!(epoch_off, 0x024);
        assert_eq!(created_off, 0x028);
        assert_eq!(modified_off, 0x030);
        assert_eq!(entry_off, 0x038);
        assert_eq!(toplayer_off, 0x048);
        assert_eq!(centroid_off, 0x058);
        assert_eq!(quantdict_off, 0x068);
        assert_eq!(hot_cache_off, 0x078);
        assert_eq!(prefetch_off, 0x088);
        assert_eq!(sig_algo_off, 0x098);
        assert_eq!(sig_len_off, 0x09A);
        assert_eq!(sig_buf_off, 0x09C);
        assert_eq!(reserved_off, 0xF00);
        assert_eq!(checksum_off, 0xFFC);
    }

    #[test]
    fn hotset_pointer_sizes() {
        assert_eq!(core::mem::size_of::<EntrypointPtr>(), 16);
        assert_eq!(core::mem::size_of::<TopLayerPtr>(), 16);
        assert_eq!(core::mem::size_of::<CentroidPtr>(), 16);
        assert_eq!(core::mem::size_of::<QuantDictPtr>(), 16);
        assert_eq!(core::mem::size_of::<HotCachePtr>(), 16);
        assert_eq!(core::mem::size_of::<PrefetchMapPtr>(), 16);
    }
}
