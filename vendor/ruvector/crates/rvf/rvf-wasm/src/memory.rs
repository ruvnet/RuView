//! Static memory layout matching the Cognitum tile spec.
//!
//! No allocator. All buffers are statically allocated as mutable byte arrays.

/// Total data memory: 8 KB
pub const DATA_MEMORY_SIZE: usize = 8 * 1024;

/// Total SIMD scratch memory: 64 KB
pub const SIMD_SCRATCH_SIZE: usize = 64 * 1024;

// === Data Memory Layout ===

/// Tile configuration: 64 bytes at offset 0x0000
pub const TILE_CONFIG_SIZE: usize = 64;

/// Offset within tile config for stored dimension (u32)
pub const TILE_CONFIG_DIM_OFFSET: usize = 0x04;
/// Offset within tile config for stored vector count (u32)
pub const TILE_CONFIG_COUNT_OFFSET: usize = 0x08;
/// Offset within tile config for stored dtype (u32)
pub const TILE_CONFIG_DTYPE_OFFSET: usize = 0x0C;
/// Offset within tile config for PQ M parameter (u32)
pub const TILE_CONFIG_PQ_M_OFFSET: usize = 0x10;
/// Offset within tile config for PQ K parameter (u32)
pub const TILE_CONFIG_PQ_K_OFFSET: usize = 0x14;

/// Query scratch: 192 bytes at offset 0x0040
pub const QUERY_SCRATCH_OFFSET: usize = 0x0040;
pub const QUERY_SCRATCH_SIZE: usize = 192;

/// Result buffer: 256 bytes at offset 0x0100
pub const RESULT_BUFFER_OFFSET: usize = 0x0100;
pub const RESULT_BUFFER_SIZE: usize = 256;

/// Routing table: 512 bytes at offset 0x0200
pub const ROUTING_TABLE_OFFSET: usize = 0x0200;
pub const ROUTING_TABLE_SIZE: usize = 512;

/// Decode workspace: 1 KB at offset 0x0400
pub const DECODE_WORKSPACE_OFFSET: usize = 0x0400;
pub const DECODE_WORKSPACE_SIZE: usize = 1024;

/// Message I/O buffer: 2 KB at offset 0x0800
pub const MESSAGE_IO_OFFSET: usize = 0x0800;
pub const MESSAGE_IO_SIZE: usize = 2048;

/// Neighbor list cache: 4 KB at offset 0x1000
pub const NEIGHBOR_CACHE_OFFSET: usize = 0x1000;
pub const NEIGHBOR_CACHE_SIZE: usize = 4096;

// === SIMD Scratch Layout ===

/// Vector block area: 32 KB at offset 0x0000
pub const SIMD_BLOCK_SIZE: usize = 32 * 1024;

/// PQ distance table: 16 KB at offset 0x8000
pub const SIMD_PQ_TABLE_OFFSET: usize = 0x8000;
pub const SIMD_PQ_TABLE_SIZE: usize = 16 * 1024;

/// Hot cache: 12 KB at offset 0xC000
pub const SIMD_HOT_CACHE_OFFSET: usize = 0xC000;

// === Static Buffers ===

/// Main data memory (8 KB).
pub static mut DATA_MEMORY: [u8; DATA_MEMORY_SIZE] = [0u8; DATA_MEMORY_SIZE];

/// SIMD scratch memory (64 KB).
pub static mut SIMD_SCRATCH: [u8; SIMD_SCRATCH_SIZE] = [0u8; SIMD_SCRATCH_SIZE];
