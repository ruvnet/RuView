//! RVF WASM Microkernel for Cognitum tiles.
//!
//! All 14 exports as `#[no_mangle] pub extern "C" fn`.
//! No allocator — all memory is statically laid out in WASM linear memory.
//! Target: wasm32-unknown-unknown, < 8 KB after wasm-opt.

#![no_std]

extern crate alloc;

mod alloc_setup;
pub mod bootstrap;
mod distance;
mod memory;
mod segment;
mod store;
mod topk;

use memory::*;

// =====================================================================
// Core Query Path
// =====================================================================

/// Initialize tile with configuration from data memory.
/// config_ptr: pointer to 64-byte tile config.
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn rvf_init(config_ptr: i32) -> i32 {
    let ptr = config_ptr as usize;
    if ptr + TILE_CONFIG_SIZE > DATA_MEMORY_SIZE {
        return -1;
    }
    unsafe {
        let src = config_ptr as *const u8;
        let dst = DATA_MEMORY.as_mut_ptr();
        core::ptr::copy_nonoverlapping(src, dst, TILE_CONFIG_SIZE);
    }
    topk::heap_reset();
    0
}

/// Load query vector into query scratch area.
/// query_ptr: pointer to fp16 vector in data memory.
/// dim: vector dimensionality.
/// Returns 0 on success.
#[no_mangle]
pub extern "C" fn rvf_load_query(query_ptr: i32, dim: i32) -> i32 {
    let dim = dim as usize;
    let byte_len = dim * 2; // fp16 = 2 bytes per element
    if byte_len > QUERY_SCRATCH_SIZE {
        return -1;
    }
    unsafe {
        let src = query_ptr as *const u8;
        let dst = DATA_MEMORY.as_mut_ptr().add(QUERY_SCRATCH_OFFSET);
        core::ptr::copy_nonoverlapping(src, dst, byte_len);
        let dim_ptr = DATA_MEMORY.as_mut_ptr().add(TILE_CONFIG_DIM_OFFSET) as *mut u32;
        *dim_ptr = dim as u32;
    }
    0
}

/// Load a block of vectors into SIMD scratch.
/// block_ptr: source pointer, count: number of vectors, dtype: data type.
/// Returns 0 on success.
#[no_mangle]
pub extern "C" fn rvf_load_block(block_ptr: i32, count: i32, dtype: i32) -> i32 {
    let count = count as usize;
    let dim = unsafe {
        let dim_ptr = DATA_MEMORY.as_ptr().add(TILE_CONFIG_DIM_OFFSET) as *const u32;
        *dim_ptr as usize
    };
    let elem_size = match dtype {
        0 => 2, // fp16
        1 => 1, // i8
        2 => 4, // f32
        _ => return -1,
    };
    let total_bytes = count * dim * elem_size;
    if total_bytes > SIMD_BLOCK_SIZE {
        return -1;
    }
    unsafe {
        let src = block_ptr as *const u8;
        let dst = SIMD_SCRATCH.as_mut_ptr();
        core::ptr::copy_nonoverlapping(src, dst, total_bytes);
        let count_ptr = DATA_MEMORY.as_mut_ptr().add(TILE_CONFIG_COUNT_OFFSET) as *mut u32;
        *count_ptr = count as u32;
        let dtype_ptr = DATA_MEMORY.as_mut_ptr().add(TILE_CONFIG_DTYPE_OFFSET) as *mut u32;
        *dtype_ptr = dtype as u32;
    }
    0
}

/// Compute distances between query and loaded block.
/// metric: 0=L2, 1=IP, 2=cosine, 3=hamming.
/// result_ptr: pointer to write f32 distance results.
/// Returns number of distances computed, or negative on error.
#[no_mangle]
pub extern "C" fn rvf_distances(metric: i32, result_ptr: i32) -> i32 {
    let (dim, count, dtype) = unsafe {
        let dim = *(DATA_MEMORY.as_ptr().add(TILE_CONFIG_DIM_OFFSET) as *const u32) as usize;
        let count =
            *(DATA_MEMORY.as_ptr().add(TILE_CONFIG_COUNT_OFFSET) as *const u32) as usize;
        let dtype = *(DATA_MEMORY.as_ptr().add(TILE_CONFIG_DTYPE_OFFSET) as *const u32);
        (dim, count, dtype)
    };

    if dim == 0 || count == 0 {
        return -1;
    }

    let query_ptr = unsafe { DATA_MEMORY.as_ptr().add(QUERY_SCRATCH_OFFSET) };
    let block_ptr = unsafe { SIMD_SCRATCH.as_ptr() };
    let out_ptr = result_ptr as *mut f32;

    for i in 0..count {
        let dist = match dtype {
            0 => {
                let vec_offset = i * dim * 2;
                let vec_ptr = unsafe { block_ptr.add(vec_offset) };
                match metric {
                    0 => distance::l2_fp16(query_ptr, vec_ptr, dim),
                    1 => distance::ip_fp16(query_ptr, vec_ptr, dim),
                    2 => distance::cosine_fp16(query_ptr, vec_ptr, dim),
                    3 => distance::hamming(query_ptr, vec_ptr, dim * 2),
                    _ => return -1,
                }
            }
            1 => {
                let vec_offset = i * dim;
                let vec_ptr = unsafe { block_ptr.add(vec_offset) };
                match metric {
                    0 => distance::l2_i8(query_ptr, vec_ptr, dim),
                    3 => distance::hamming(query_ptr, vec_ptr, dim),
                    _ => return -1,
                }
            }
            _ => return -1,
        };
        unsafe {
            *out_ptr.add(i) = dist;
        }
    }

    count as i32
}

/// Merge distances into top-K heap.
/// Returns 0 on success.
#[no_mangle]
pub extern "C" fn rvf_topk_merge(dist_ptr: i32, id_ptr: i32, count: i32, k: i32) -> i32 {
    let k = k as usize;
    let count = count as usize;
    if k > topk::MAX_K {
        return -1;
    }

    for i in 0..count {
        let dist = unsafe { *(dist_ptr as *const f32).add(i) };
        let id = unsafe { *(id_ptr as *const u64).add(i) };
        topk::heap_insert(dist, id, k);
    }

    0
}

/// Read current top-K results into output buffer.
/// out_ptr: pointer to write (id: u64, dist: f32) pairs.
/// Returns number of results written.
#[no_mangle]
pub extern "C" fn rvf_topk_read(out_ptr: i32) -> i32 {
    topk::heap_read_sorted(out_ptr as *mut u8)
}

// =====================================================================
// Quantization
// =====================================================================

/// Load scalar quantization parameters (min/max per dimension).
/// params_ptr: pointer to f32 pairs [min0, max0, min1, max1, ...].
/// dim: number of dimensions.
/// Returns 0 on success.
#[no_mangle]
pub extern "C" fn rvf_load_sq_params(params_ptr: i32, dim: i32) -> i32 {
    let byte_len = dim as usize * 8; // 2 f32 per dim
    if byte_len > DECODE_WORKSPACE_SIZE {
        return -1;
    }
    unsafe {
        let src = params_ptr as *const u8;
        let dst = DATA_MEMORY.as_mut_ptr().add(DECODE_WORKSPACE_OFFSET);
        core::ptr::copy_nonoverlapping(src, dst, byte_len);
    }
    0
}

/// Dequantize int8 block to fp16 in SIMD scratch.
/// src_ptr: source i8 data, dst_ptr: destination fp16 data, count: total values.
/// Returns 0 on success.
#[no_mangle]
pub extern "C" fn rvf_dequant_i8(src_ptr: i32, dst_ptr: i32, count: i32) -> i32 {
    let dim = unsafe {
        *(DATA_MEMORY.as_ptr().add(TILE_CONFIG_DIM_OFFSET) as *const u32) as usize
    };
    if dim == 0 {
        return -1;
    }

    let params = unsafe { DATA_MEMORY.as_ptr().add(DECODE_WORKSPACE_OFFSET) as *const f32 };

    for i in 0..(count as usize) {
        let dim_idx = i % dim;
        let min_val = unsafe { *params.add(dim_idx * 2) };
        let max_val = unsafe { *params.add(dim_idx * 2 + 1) };
        let raw = unsafe { *(src_ptr as *const i8).add(i) } as f32;
        let normalized = (raw + 128.0) / 255.0;
        let val = min_val + normalized * (max_val - min_val);
        let fp16_bits = f32_to_f16(val);
        unsafe {
            *(dst_ptr as *mut u16).add(i) = fp16_bits;
        }
    }

    0
}

/// Load PQ codebook subset into SIMD scratch distance accumulator area.
/// codebook_ptr: source data, m: number of subspaces, k: centroids per subspace.
/// Returns 0 on success.
#[no_mangle]
pub extern "C" fn rvf_load_pq_codebook(codebook_ptr: i32, m: i32, k: i32) -> i32 {
    let dim = unsafe {
        *(DATA_MEMORY.as_ptr().add(TILE_CONFIG_DIM_OFFSET) as *const u32) as usize
    };
    let m_usize = m as usize;
    if m_usize == 0 {
        return -1;
    }
    let sub_dim = dim / m_usize;
    let total_bytes = m_usize * k as usize * sub_dim * 2;
    if total_bytes > SIMD_PQ_TABLE_SIZE {
        return -1;
    }
    unsafe {
        let src = codebook_ptr as *const u8;
        let dst = SIMD_SCRATCH.as_mut_ptr().add(SIMD_PQ_TABLE_OFFSET);
        core::ptr::copy_nonoverlapping(src, dst, total_bytes);
        let m_ptr = DATA_MEMORY.as_mut_ptr().add(TILE_CONFIG_PQ_M_OFFSET) as *mut u32;
        *m_ptr = m as u32;
        let k_ptr = DATA_MEMORY.as_mut_ptr().add(TILE_CONFIG_PQ_K_OFFSET) as *mut u32;
        *k_ptr = k as u32;
    }
    0
}

/// Compute PQ asymmetric distances.
/// codes_ptr: PQ codes (m bytes per vector), count: number of vectors.
/// result_ptr: output f32 distances.
/// Returns number of distances computed.
#[no_mangle]
pub extern "C" fn rvf_pq_distances(codes_ptr: i32, count: i32, result_ptr: i32) -> i32 {
    let (dim, m, k) = unsafe {
        let dim = *(DATA_MEMORY.as_ptr().add(TILE_CONFIG_DIM_OFFSET) as *const u32) as usize;
        let m = *(DATA_MEMORY.as_ptr().add(TILE_CONFIG_PQ_M_OFFSET) as *const u32) as usize;
        let k = *(DATA_MEMORY.as_ptr().add(TILE_CONFIG_PQ_K_OFFSET) as *const u32) as usize;
        (dim, m, k)
    };
    if m == 0 || k == 0 || dim == 0 {
        return -1;
    }
    let sub_dim = dim / m;
    let query_ptr = unsafe { DATA_MEMORY.as_ptr().add(QUERY_SCRATCH_OFFSET) };
    let codebook_ptr = unsafe { SIMD_SCRATCH.as_ptr().add(SIMD_PQ_TABLE_OFFSET) };

    // Precompute query-centroid distance lookup table
    let dlt_ptr = unsafe { SIMD_SCRATCH.as_mut_ptr().add(SIMD_HOT_CACHE_OFFSET) as *mut f32 };
    for sub in 0..m {
        let q_offset = sub * sub_dim * 2;
        for c in 0..k {
            let cb_offset = (sub * k + c) * sub_dim * 2;
            let dist = distance::l2_fp16(
                unsafe { query_ptr.add(q_offset) },
                unsafe { codebook_ptr.add(cb_offset) },
                sub_dim,
            );
            unsafe {
                *dlt_ptr.add(sub * k + c) = dist;
            }
        }
    }

    for i in 0..(count as usize) {
        let mut total_dist: f32 = 0.0;
        for sub in 0..m {
            let code = unsafe { *(codes_ptr as *const u8).add(i * m + sub) } as usize;
            if code < k {
                total_dist += unsafe { *dlt_ptr.add(sub * k + code) };
            }
        }
        unsafe {
            *(result_ptr as *mut f32).add(i) = total_dist;
        }
    }

    count
}

// =====================================================================
// HNSW Navigation
// =====================================================================

/// Load neighbor list for a node into the neighbor cache.
/// Returns number of neighbors loaded.
#[no_mangle]
pub extern "C" fn rvf_load_neighbors(node_id: i64, layer: i32, out_ptr: i32) -> i32 {
    let _ = node_id;
    let _ = layer;
    unsafe {
        let cache_ptr = DATA_MEMORY.as_mut_ptr().add(NEIGHBOR_CACHE_OFFSET) as *mut i32;
        *cache_ptr = out_ptr;
    }
    0
}

/// Greedy search step: from current_id at a given layer, find nearest neighbor.
/// Returns the ID of the nearest unvisited neighbor, or -1 if none.
#[no_mangle]
pub extern "C" fn rvf_greedy_step(current_id: i64, layer: i32) -> i64 {
    let _ = layer;
    let neighbor_ptr = unsafe {
        *(DATA_MEMORY.as_ptr().add(NEIGHBOR_CACHE_OFFSET) as *const i32)
    };
    if neighbor_ptr == 0 {
        return -1;
    }

    // Neighbor list format: [count: u32, (id: u64, dist: f32)*]
    let count = unsafe { *(neighbor_ptr as *const u32) } as usize;
    if count == 0 {
        return -1;
    }

    let mut best_id: i64 = -1;
    let mut best_dist: f32 = f32::MAX;

    let entries_ptr = unsafe { (neighbor_ptr as *const u8).add(4) };
    for i in 0..count {
        let offset = i * 12; // 8 bytes id + 4 bytes dist
        let id = unsafe { *(entries_ptr.add(offset) as *const u64) } as i64;
        let dist = unsafe { *(entries_ptr.add(offset + 8) as *const f32) };
        if id != current_id && dist < best_dist {
            best_dist = dist;
            best_id = id;
        }
    }

    best_id
}

// =====================================================================
// Segment Verification
// =====================================================================

/// Verify segment header magic and version.
/// Returns 0 if valid, non-zero error code otherwise.
#[no_mangle]
pub extern "C" fn rvf_verify_header(header_ptr: i32) -> i32 {
    let ptr = header_ptr as *const u8;
    let magic = unsafe {
        let b = core::slice::from_raw_parts(ptr, 4);
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    };
    if magic != 0x5256_4653 {
        return 1;
    }
    let version = unsafe { *ptr.add(4) };
    if version != 1 {
        return 2;
    }
    0
}

/// Compute CRC32C of a data region.
/// Returns the 32-bit CRC value.
#[no_mangle]
pub extern "C" fn rvf_crc32c(data_ptr: i32, len: i32) -> i32 {
    let ptr = data_ptr as *const u8;
    let data = unsafe { core::slice::from_raw_parts(ptr, len as usize) };
    crc32c_compute(data) as i32
}

// =====================================================================
// Helpers
// =====================================================================

/// Software CRC32C (Castagnoli) implementation.
fn crc32c_compute(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0x82F6_3B78;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^ 0xFFFF_FFFF
}

/// Convert f32 to IEEE 754 half-precision (f16) bit pattern.
fn f32_to_f16(val: f32) -> u16 {
    let bits = val.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let mantissa = bits & 0x007F_FFFF;

    if exp == 0xFF {
        return sign | 0x7C00 | if mantissa != 0 { 0x0200 } else { 0 };
    }

    let new_exp = exp - 127 + 15;
    if new_exp >= 31 {
        return sign | 0x7C00;
    }
    if new_exp <= 0 {
        if new_exp < -10 {
            return sign;
        }
        let mant = (mantissa | 0x0080_0000) >> (1 - new_exp + 13);
        return sign | mant as u16;
    }
    sign | ((new_exp as u16) << 10) | ((mantissa >> 13) as u16)
}

// =====================================================================
// Control Plane — In-Memory Store
// =====================================================================

/// Create an in-memory store. Returns a handle (>0) or negative on error.
#[no_mangle]
pub extern "C" fn rvf_store_create(dim: i32, metric: i32) -> i32 {
    if dim <= 0 {
        return -1;
    }
    store::registry().create(dim as u32, metric as u8)
}

/// Open a .rvf file from raw bytes. Returns a store handle.
#[no_mangle]
pub extern "C" fn rvf_store_open(buf_ptr: i32, buf_len: i32) -> i32 {
    if buf_len <= 0 {
        return -1;
    }
    let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, buf_len as usize) };
    let segments = segment::parse_segments(buf);

    let reg = store::registry();
    let mut dim: u32 = 0;
    let mut entries: alloc::vec::Vec<(u64, alloc::vec::Vec<f32>)> = alloc::vec::Vec::new();

    for seg in &segments {
        // SegmentType::Vec = 0x01
        if seg.seg_type == 0x01 {
            let payload_start = seg.offset + rvf_types::constants::SEGMENT_HEADER_SIZE;
            let payload_end = payload_start + seg.payload_length as usize;
            if payload_end > buf.len() || seg.payload_length < 6 {
                continue;
            }
            let payload = &buf[payload_start..payload_end];

            let count = u16::from_le_bytes([payload[0], payload[1]]) as usize;
            let seg_dim = u32::from_le_bytes([payload[2], payload[3], payload[4], payload[5]]);
            if dim == 0 {
                dim = seg_dim;
            }

            let mut offset = 6;
            for _ in 0..count {
                if offset + 8 > payload.len() {
                    break;
                }
                let id = u64::from_le_bytes([
                    payload[offset],
                    payload[offset + 1],
                    payload[offset + 2],
                    payload[offset + 3],
                    payload[offset + 4],
                    payload[offset + 5],
                    payload[offset + 6],
                    payload[offset + 7],
                ]);
                offset += 8;
                let vec_bytes = (seg_dim as usize) * 4;
                if offset + vec_bytes > payload.len() {
                    break;
                }
                let mut vec_data = alloc::vec::Vec::with_capacity(seg_dim as usize);
                for d in 0..seg_dim as usize {
                    let f = f32::from_le_bytes([
                        payload[offset + d * 4],
                        payload[offset + d * 4 + 1],
                        payload[offset + d * 4 + 2],
                        payload[offset + d * 4 + 3],
                    ]);
                    vec_data.push(f);
                }
                offset += vec_bytes;
                entries.push((id, vec_data));
            }
        }
    }

    if dim == 0 {
        dim = 1;
    }
    let handle = reg.create(dim, 0);
    if handle <= 0 {
        return handle;
    }

    if let Some(s) = reg.get_mut(handle) {
        for (id, data) in entries {
            s.entries.push(store::VecEntry {
                id,
                data,
                deleted: false,
            });
        }
    }

    handle
}

/// Ingest vectors into a store. Returns count ingested or negative on error.
#[no_mangle]
pub extern "C" fn rvf_store_ingest(handle: i32, vecs_ptr: i32, ids_ptr: i32, count: i32) -> i32 {
    if count <= 0 {
        return 0;
    }
    match store::registry().get_mut(handle) {
        Some(s) => s.ingest(vecs_ptr as *const f32, ids_ptr as *const u64, count as u32),
        None => -1,
    }
}

/// Query a store for k nearest neighbors.
/// Results written to out_ptr as (id: u64, dist: f32) pairs.
/// Returns number of results.
#[no_mangle]
pub extern "C" fn rvf_store_query(
    handle: i32,
    query_ptr: i32,
    k: i32,
    metric: i32,
    out_ptr: i32,
) -> i32 {
    if k <= 0 {
        return 0;
    }
    match store::registry().get(handle) {
        Some(s) => s.query(query_ptr as *const f32, k as u32, metric, out_ptr as *mut u8),
        None => -1,
    }
}

/// Delete vectors by ID. Returns count deleted.
#[no_mangle]
pub extern "C" fn rvf_store_delete(handle: i32, ids_ptr: i32, count: i32) -> i32 {
    if count <= 0 {
        return 0;
    }
    match store::registry().get_mut(handle) {
        Some(s) => s.delete(ids_ptr as *const u64, count as u32),
        None => -1,
    }
}

/// Get live vector count.
#[no_mangle]
pub extern "C" fn rvf_store_count(handle: i32) -> i32 {
    match store::registry().get(handle) {
        Some(s) => s.count() as i32,
        None => -1,
    }
}

/// Get store dimension.
#[no_mangle]
pub extern "C" fn rvf_store_dimension(handle: i32) -> i32 {
    match store::registry().get(handle) {
        Some(s) => s.dimension() as i32,
        None => -1,
    }
}

/// Write store status to output buffer (20 bytes).
#[no_mangle]
pub extern "C" fn rvf_store_status(handle: i32, out_ptr: i32) -> i32 {
    match store::registry().get(handle) {
        Some(s) => s.status(out_ptr as *mut u8),
        None => -1,
    }
}

/// Export store as .rvf bytes. Returns bytes written or negative if buffer too small.
#[no_mangle]
pub extern "C" fn rvf_store_export(handle: i32, out_ptr: i32, out_len: i32) -> i32 {
    match store::registry().get(handle) {
        Some(s) => s.export(out_ptr as *mut u8, out_len as u32),
        None => -1,
    }
}

/// Close and free a store.
#[no_mangle]
pub extern "C" fn rvf_store_close(handle: i32) -> i32 {
    store::registry().close(handle)
}

// =====================================================================
// Segment Parsing & Inspection
// =====================================================================

/// Parse a segment header from raw bytes.
/// Writes 24 bytes to out_ptr.
#[no_mangle]
pub extern "C" fn rvf_parse_header(buf_ptr: i32, buf_len: i32, out_ptr: i32) -> i32 {
    if buf_len < 64 {
        return -1;
    }
    let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, buf_len as usize) };
    segment::parse_header_to_buf(buf, out_ptr as *mut u8)
}

/// Count segments in a .rvf buffer.
#[no_mangle]
pub extern "C" fn rvf_segment_count(buf_ptr: i32, buf_len: i32) -> i32 {
    if buf_len <= 0 {
        return 0;
    }
    let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, buf_len as usize) };
    segment::parse_segments(buf).len() as i32
}

/// Get info for segment at index `idx` in the buffer.
/// Writes to out_ptr: [seg_id: u64, type: u8, padding: 3 bytes, payload_len: u64, offset: u64] = 28 bytes
#[no_mangle]
pub extern "C" fn rvf_segment_info(buf_ptr: i32, buf_len: i32, idx: i32, out_ptr: i32) -> i32 {
    if buf_len <= 0 || idx < 0 {
        return -1;
    }
    let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, buf_len as usize) };
    let segments = segment::parse_segments(buf);
    let i = idx as usize;
    if i >= segments.len() {
        return -1;
    }

    let seg = &segments[i];
    let out = out_ptr as *mut u8;
    unsafe {
        let id_bytes = seg.seg_id.to_le_bytes();
        for b in 0..8 {
            *out.add(b) = id_bytes[b];
        }
        *out.add(8) = seg.seg_type;
        *out.add(9) = 0;
        *out.add(10) = 0;
        *out.add(11) = 0; // padding
        let pl_bytes = seg.payload_length.to_le_bytes();
        for b in 0..8 {
            *out.add(12 + b) = pl_bytes[b];
        }
        let off_bytes = (seg.offset as u64).to_le_bytes();
        for b in 0..8 {
            *out.add(20 + b) = off_bytes[b];
        }
    }
    0
}

/// Verify checksum of a data region.
/// The last 4 bytes of the buffer are treated as the expected CRC32C.
/// Returns 1 if CRC32C matches, 0 if not.
#[no_mangle]
pub extern "C" fn rvf_verify_checksum(buf_ptr: i32, buf_len: i32) -> i32 {
    if buf_len < 4 {
        return -1;
    }
    let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, buf_len as usize) };
    let data = &buf[..buf.len() - 4];
    let expected = u32::from_le_bytes([
        buf[buf.len() - 4],
        buf[buf.len() - 3],
        buf[buf.len() - 2],
        buf[buf.len() - 1],
    ]);
    let computed = crc32c_compute(data);
    if computed == expected {
        1
    } else {
        0
    }
}

// =====================================================================
// Witness Chain Verification
// =====================================================================

/// Verify a SHAKE-256 witness chain in memory.
///
/// `chain_ptr`: pointer to serialized witness chain (73 bytes per entry).
/// `chain_len`: total byte length of the chain.
///
/// Returns the number of verified entries on success, or a negative error code:
///   -1: invalid pointer/length
///   -2: truncated chain (not a multiple of 73 bytes)
///   -3: chain integrity failure (prev_hash mismatch)
#[no_mangle]
pub extern "C" fn rvf_witness_verify(chain_ptr: i32, chain_len: i32) -> i32 {
    if chain_len < 0 {
        return -1;
    }
    let len = chain_len as usize;
    if len == 0 {
        return 0;
    }
    let data = unsafe { core::slice::from_raw_parts(chain_ptr as *const u8, len) };
    match rvf_crypto::verify_witness_chain(data) {
        Ok(entries) => entries.len() as i32,
        Err(e) => {
            use rvf_types::RvfError;
            match e {
                RvfError::Code(rvf_types::ErrorCode::TruncatedSegment) => -2,
                RvfError::Code(rvf_types::ErrorCode::InvalidChecksum) => -3,
                _ => -1,
            }
        }
    }
}

/// Count witness entries in a chain without full verification.
///
/// Returns the entry count (chain_len / 73), or -1 if not aligned.
#[no_mangle]
pub extern "C" fn rvf_witness_count(chain_len: i32) -> i32 {
    if chain_len < 0 {
        return -1;
    }
    let len = chain_len as usize;
    if len == 0 {
        return 0;
    }
    if len % 73 != 0 {
        return -1;
    }
    (len / 73) as i32
}

// =====================================================================
// Memory Management
// =====================================================================

// rvf_alloc and rvf_free are exported from alloc_setup module.

/// Panic handler for no_std WASM.
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
