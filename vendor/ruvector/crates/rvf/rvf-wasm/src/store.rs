//! In-memory WasmStore for browser-side RVF operations.
//!
//! Handle-based API: each store gets an integer handle.
//! Supports create, ingest, query, delete, export.

extern crate alloc;

use alloc::vec::Vec;

/// Distance metric enum matching rvf-types.
#[derive(Clone, Copy)]
pub enum Metric {
    L2 = 0,
    InnerProduct = 1,
    Cosine = 2,
}

/// A single vector entry in the store.
pub struct VecEntry {
    pub id: u64,
    pub data: Vec<f32>,
    pub deleted: bool,
}

/// An in-memory RVF store.
pub struct WasmStore {
    dimension: u32,
    metric: Metric,
    pub entries: Vec<VecEntry>,
}

impl WasmStore {
    pub fn new(dimension: u32, metric: u8) -> Self {
        let m = match metric {
            1 => Metric::InnerProduct,
            2 => Metric::Cosine,
            _ => Metric::L2,
        };
        Self {
            dimension,
            metric: m,
            entries: Vec::new(),
        }
    }

    pub fn dimension(&self) -> u32 {
        self.dimension
    }

    pub fn count(&self) -> u32 {
        self.entries.iter().filter(|e| !e.deleted).count() as u32
    }

    pub fn ingest(&mut self, vecs_ptr: *const f32, ids_ptr: *const u64, count: u32) -> i32 {
        let dim = self.dimension as usize;
        let mut accepted = 0i32;
        for i in 0..count as usize {
            let id = unsafe { *ids_ptr.add(i) };
            let mut data = Vec::with_capacity(dim);
            for d in 0..dim {
                data.push(unsafe { *vecs_ptr.add(i * dim + d) });
            }
            self.entries.push(VecEntry {
                id,
                data,
                deleted: false,
            });
            accepted += 1;
        }
        accepted
    }

    pub fn query(
        &self,
        query_ptr: *const f32,
        k: u32,
        metric_override: i32,
        out_ptr: *mut u8,
    ) -> i32 {
        let dim = self.dimension as usize;
        let metric = if metric_override >= 0 {
            match metric_override as u8 {
                1 => Metric::InnerProduct,
                2 => Metric::Cosine,
                _ => Metric::L2,
            }
        } else {
            self.metric
        };

        let query: Vec<f32> = (0..dim).map(|i| unsafe { *query_ptr.add(i) }).collect();

        // Collect (distance, id) for all live entries
        let mut candidates: Vec<(f32, u64)> = Vec::new();
        for entry in &self.entries {
            if entry.deleted {
                continue;
            }
            let dist = compute_distance(&query, &entry.data, metric);
            candidates.push((dist, entry.id));
        }

        // Sort by distance ascending
        candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal));

        let result_count = (k as usize).min(candidates.len());
        // Write results: (id: u64, dist: f32) pairs = 12 bytes each
        for i in 0..result_count {
            let (dist, id) = candidates[i];
            let offset = i * 12;
            let id_bytes = id.to_le_bytes();
            let dist_bytes = dist.to_le_bytes();
            unsafe {
                for b in 0..8 {
                    *out_ptr.add(offset + b) = id_bytes[b];
                }
                for b in 0..4 {
                    *out_ptr.add(offset + 8 + b) = dist_bytes[b];
                }
            }
        }

        result_count as i32
    }

    pub fn delete(&mut self, ids_ptr: *const u64, count: u32) -> i32 {
        let mut deleted = 0i32;
        for i in 0..count as usize {
            let target_id = unsafe { *ids_ptr.add(i) };
            for entry in self.entries.iter_mut() {
                if entry.id == target_id && !entry.deleted {
                    entry.deleted = true;
                    deleted += 1;
                }
            }
        }
        deleted
    }

    /// Write status to output buffer.
    /// Format: [count: u32, dimension: u32, metric: u32, total_entries: u32, deleted: u32]
    pub fn status(&self, out_ptr: *mut u8) -> i32 {
        let live = self.count();
        let total = self.entries.len() as u32;
        let deleted = total - live;
        let metric_val = self.metric as u32;

        unsafe {
            write_u32(out_ptr, 0, live);
            write_u32(out_ptr, 4, self.dimension);
            write_u32(out_ptr, 8, metric_val);
            write_u32(out_ptr, 12, total);
            write_u32(out_ptr, 16, deleted);
        }
        20 // bytes written
    }

    /// Export the store as .rvf bytes into a pre-allocated buffer.
    /// Returns bytes written, or negative if buffer too small.
    pub fn export(&self, out_ptr: *mut u8, out_len: u32) -> i32 {
        use rvf_types::constants::{SEGMENT_HEADER_SIZE, SEGMENT_MAGIC, SEGMENT_VERSION};
        use rvf_types::SegmentType;

        let dim = self.dimension as usize;
        let live_entries: Vec<&VecEntry> = self.entries.iter().filter(|e| !e.deleted).collect();
        let n = live_entries.len();

        // Vec segment payload: [count: u16, dim: u32, (id: u64, vec: f32*dim)*]
        let vec_payload_len = 2 + 4 + n * (8 + dim * 4);
        // Manifest segment payload: [epoch: u32, dim: u16, total_vecs: u64, profile: u8,
        //   seg_count: u32, (seg_id: u64, offset: u64, payload_len: u64, type: u8)*]
        let manifest_payload_len = 4 + 2 + 8 + 1 + 4 + 1 * 25; // 1 segment entry

        let total_size =
            SEGMENT_HEADER_SIZE + vec_payload_len + SEGMENT_HEADER_SIZE + manifest_payload_len;

        if (out_len as usize) < total_size {
            return -(total_size as i32);
        }

        let mut offset = 0usize;

        // -- Vec segment header --
        unsafe {
            write_u32(out_ptr, offset, SEGMENT_MAGIC);
            *out_ptr.add(offset + 4) = SEGMENT_VERSION;
            *out_ptr.add(offset + 5) = SegmentType::Vec as u8;
            write_u16_at(out_ptr, offset + 6, 0); // flags
            write_u64(out_ptr, offset + 8, 1); // seg_id = 1
            write_u64(out_ptr, offset + 16, vec_payload_len as u64);
            // rest of header zeros (timestamp, checksum, etc.)
            for i in 24..SEGMENT_HEADER_SIZE {
                *out_ptr.add(offset + i) = 0;
            }
        }
        offset += SEGMENT_HEADER_SIZE;

        // -- Vec segment payload --
        unsafe {
            write_u16_at(out_ptr, offset, n as u16);
            offset += 2;
            write_u32(out_ptr, offset, self.dimension);
            offset += 4;
            for entry in &live_entries {
                write_u64(out_ptr, offset, entry.id);
                offset += 8;
                for d in 0..dim {
                    write_f32(out_ptr, offset, entry.data[d]);
                    offset += 4;
                }
            }
        }

        // -- Manifest segment header --
        unsafe {
            write_u32(out_ptr, offset, SEGMENT_MAGIC);
            *out_ptr.add(offset + 4) = SEGMENT_VERSION;
            *out_ptr.add(offset + 5) = SegmentType::Manifest as u8;
            write_u16_at(out_ptr, offset + 6, 0);
            write_u64(out_ptr, offset + 8, 2); // seg_id = 2
            write_u64(out_ptr, offset + 16, manifest_payload_len as u64);
            for i in 24..SEGMENT_HEADER_SIZE {
                *out_ptr.add(offset + i) = 0;
            }
        }
        offset += SEGMENT_HEADER_SIZE;

        // -- Manifest payload --
        unsafe {
            write_u32(out_ptr, offset, 1); // epoch
            offset += 4;
            write_u16_at(out_ptr, offset, self.dimension as u16);
            offset += 2;
            write_u64(out_ptr, offset, n as u64); // total_vectors
            offset += 8;
            *out_ptr.add(offset) = 0; // profile
            offset += 1;
            write_u32(out_ptr, offset, 1); // seg_count = 1
            offset += 4;
            // segment entry
            write_u64(out_ptr, offset, 1); // seg_id
            offset += 8;
            write_u64(out_ptr, offset, 0); // offset (start of file)
            offset += 8;
            write_u64(out_ptr, offset, vec_payload_len as u64);
            offset += 8;
            *out_ptr.add(offset) = SegmentType::Vec as u8;
            offset += 1;
        }

        offset as i32
    }
}

fn compute_distance(a: &[f32], b: &[f32], metric: Metric) -> f32 {
    match metric {
        Metric::L2 => {
            let mut sum = 0.0f32;
            for i in 0..a.len() {
                let d = a[i] - b[i];
                sum += d * d;
            }
            sum
        }
        Metric::InnerProduct => {
            let mut dot = 0.0f32;
            for i in 0..a.len() {
                dot += a[i] * b[i];
            }
            -dot
        }
        Metric::Cosine => {
            let mut dot = 0.0f32;
            let mut na = 0.0f32;
            let mut nb = 0.0f32;
            for i in 0..a.len() {
                dot += a[i] * b[i];
                na += a[i] * a[i];
                nb += b[i] * b[i];
            }
            let denom = sqrt_approx(na) * sqrt_approx(nb);
            if denom < 1e-10 {
                1.0
            } else {
                1.0 - dot / denom
            }
        }
    }
}

fn sqrt_approx(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut bits = x.to_bits();
    bits = 0x1FBD_1DF5 + (bits >> 1);
    let mut y = f32::from_bits(bits);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y
}

unsafe fn write_u32(ptr: *mut u8, offset: usize, val: u32) {
    let bytes = val.to_le_bytes();
    for i in 0..4 {
        *ptr.add(offset + i) = bytes[i];
    }
}

unsafe fn write_u64(ptr: *mut u8, offset: usize, val: u64) {
    let bytes = val.to_le_bytes();
    for i in 0..8 {
        *ptr.add(offset + i) = bytes[i];
    }
}

unsafe fn write_u16_at(ptr: *mut u8, offset: usize, val: u16) {
    let bytes = val.to_le_bytes();
    *ptr.add(offset) = bytes[0];
    *ptr.add(offset + 1) = bytes[1];
}

unsafe fn write_f32(ptr: *mut u8, offset: usize, val: f32) {
    let bytes = val.to_le_bytes();
    for i in 0..4 {
        *ptr.add(offset + i) = bytes[i];
    }
}

// -- Global store registry --

pub(crate) struct StoreRegistry {
    stores: Vec<Option<WasmStore>>,
}

impl StoreRegistry {
    fn new() -> Self {
        Self {
            stores: Vec::new(),
        }
    }

    pub(crate) fn create(&mut self, dim: u32, metric: u8) -> i32 {
        let store = WasmStore::new(dim, metric);
        // Find an empty slot or push new
        for (i, slot) in self.stores.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(store);
                return (i + 1) as i32; // 1-based handles
            }
        }
        self.stores.push(Some(store));
        self.stores.len() as i32
    }

    pub(crate) fn get(&self, handle: i32) -> Option<&WasmStore> {
        if handle <= 0 {
            return None;
        }
        self.stores
            .get((handle - 1) as usize)
            .and_then(|s| s.as_ref())
    }

    pub(crate) fn get_mut(&mut self, handle: i32) -> Option<&mut WasmStore> {
        if handle <= 0 {
            return None;
        }
        self.stores
            .get_mut((handle - 1) as usize)
            .and_then(|s| s.as_mut())
    }

    pub(crate) fn close(&mut self, handle: i32) -> i32 {
        if handle <= 0 {
            return -1;
        }
        let idx = (handle - 1) as usize;
        if idx < self.stores.len() && self.stores[idx].is_some() {
            self.stores[idx] = None;
            0
        } else {
            -1
        }
    }
}

// Safety: WASM is single-threaded.
// We use Option to lazily initialize, since Vec::new() is not const in all editions.
static mut REGISTRY: Option<StoreRegistry> = None;

pub fn registry() -> &'static mut StoreRegistry {
    unsafe {
        if REGISTRY.is_none() {
            REGISTRY = Some(StoreRegistry::new());
        }
        REGISTRY.as_mut().unwrap()
    }
}
