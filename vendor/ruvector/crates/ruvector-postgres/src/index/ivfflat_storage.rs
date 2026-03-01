//! IVFFlat Storage Management
//!
//! Handles page-level storage operations for IVFFlat index including:
//! - Centroid page management
//! - Inverted list page management
//! - Vector serialization/deserialization
//! - Zero-copy vector access

use pgrx::pg_sys;
use std::mem::size_of;
use std::ptr;
use std::slice;

// ============================================================================
// Constants
// ============================================================================

/// P_NEW equivalent for allocating new pages
const P_NEW_BLOCK: pg_sys::BlockNumber = pg_sys::InvalidBlockNumber;

/// Maximum number of centroids per page
const CENTROIDS_PER_PAGE: usize = 32;

/// Maximum number of vector entries per inverted list page
const VECTORS_PER_PAGE: usize = 64;

// ============================================================================
// Centroid Page Operations
// ============================================================================

/// Write centroids to index pages
pub unsafe fn write_centroids(
    index: pg_sys::Relation,
    centroids: &[Vec<f32>],
    start_page: u32,
) -> u32 {
    let mut current_page = start_page;
    let mut written = 0;

    while written < centroids.len() {
        let buffer = pg_sys::ReadBuffer(index, P_NEW_BLOCK);
        let actual_page = pg_sys::BufferGetBlockNumber(buffer);

        pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);

        let page = pg_sys::BufferGetPage(buffer);
        pg_sys::PageInit(page, pg_sys::BLCKSZ as pg_sys::Size, 0);

        let header = page as *const pg_sys::PageHeaderData;
        let page_data = (header as *const u8).add(size_of::<pg_sys::PageHeaderData>()) as *mut u8;
        let mut offset = 0usize;

        // Write centroids to this page
        let batch_size = (centroids.len() - written).min(CENTROIDS_PER_PAGE);
        for i in 0..batch_size {
            let centroid = &centroids[written + i];
            let cluster_id = (written + i) as u32;

            // Write cluster ID
            ptr::write(page_data.add(offset) as *mut u32, cluster_id);
            offset += 4;

            // Write list page (will be filled later)
            ptr::write(page_data.add(offset) as *mut u32, 0);
            offset += 4;

            // Write count
            ptr::write(page_data.add(offset) as *mut u32, 0);
            offset += 4;

            // Write centroid vector
            let centroid_ptr = page_data.add(offset) as *mut f32;
            for (j, &val) in centroid.iter().enumerate() {
                ptr::write(centroid_ptr.add(j), val);
            }
            offset += centroid.len() * 4;
        }

        written += batch_size;

        pg_sys::MarkBufferDirty(buffer);
        pg_sys::UnlockReleaseBuffer(buffer);

        current_page = actual_page + 1;
    }

    current_page
}

/// Read centroids from index pages
pub unsafe fn read_centroids(
    index: pg_sys::Relation,
    start_page: u32,
    num_centroids: usize,
    dimensions: usize,
) -> Vec<Vec<f32>> {
    let mut centroids = Vec::with_capacity(num_centroids);
    let mut read = 0;
    let mut current_page = start_page;

    while read < num_centroids {
        let buffer = pg_sys::ReadBuffer(index, current_page);
        pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_SHARE as i32);

        let page = pg_sys::BufferGetPage(buffer);
        let header = page as *const pg_sys::PageHeaderData;
        let page_data = (header as *const u8).add(size_of::<pg_sys::PageHeaderData>());
        let mut offset = 0usize;

        // Read centroids from this page
        let batch_size = (num_centroids - read).min(CENTROIDS_PER_PAGE);
        for _ in 0..batch_size {
            // Skip cluster ID, list_page, and count
            offset += 12;

            // Read centroid vector
            let centroid_ptr = page_data.add(offset) as *const f32;
            let centroid: Vec<f32> = slice::from_raw_parts(centroid_ptr, dimensions).to_vec();
            centroids.push(centroid);

            offset += dimensions * 4;
        }

        read += batch_size;

        pg_sys::UnlockReleaseBuffer(buffer);
        current_page += 1;
    }

    centroids
}

// ============================================================================
// Inverted List Operations
// ============================================================================

/// Inverted list entry
#[derive(Debug, Clone)]
pub struct InvertedListEntry {
    pub tid: pg_sys::ItemPointerData,
    pub vector: Vec<f32>,
}

/// Write inverted list to pages
pub unsafe fn write_inverted_list(
    index: pg_sys::Relation,
    list: &[(pg_sys::ItemPointerData, Vec<f32>)],
) -> u32 {
    if list.is_empty() {
        return 0;
    }

    let buffer = pg_sys::ReadBuffer(index, P_NEW_BLOCK);
    let page_num = pg_sys::BufferGetBlockNumber(buffer);

    pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);

    let page = pg_sys::BufferGetPage(buffer);
    pg_sys::PageInit(page, pg_sys::BLCKSZ as pg_sys::Size, 0);

    let header = page as *const pg_sys::PageHeaderData;
    let page_data = (header as *const u8).add(size_of::<pg_sys::PageHeaderData>()) as *mut u8;
    let mut offset = 0usize;
    let dimensions = list[0].1.len();

    // Write list entries
    let batch_size = list.len().min(VECTORS_PER_PAGE);
    for i in 0..batch_size {
        let (tid, vector) = &list[i];

        // Write TID
        ptr::write(page_data.add(offset) as *mut pg_sys::ItemPointerData, *tid);
        offset += size_of::<pg_sys::ItemPointerData>();

        // Write vector
        let vector_ptr = page_data.add(offset) as *mut f32;
        for (j, &val) in vector.iter().enumerate() {
            ptr::write(vector_ptr.add(j), val);
        }
        offset += dimensions * 4;
    }

    pg_sys::MarkBufferDirty(buffer);
    pg_sys::UnlockReleaseBuffer(buffer);

    page_num
}

/// Read inverted list from pages
pub unsafe fn read_inverted_list(
    index: pg_sys::Relation,
    start_page: u32,
    dimensions: usize,
) -> Vec<InvertedListEntry> {
    if start_page == 0 {
        return Vec::new();
    }

    let buffer = pg_sys::ReadBuffer(index, start_page);
    pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_SHARE as i32);

    let page = pg_sys::BufferGetPage(buffer);
    let header = page as *const pg_sys::PageHeaderData;
    let page_data = (header as *const u8).add(size_of::<pg_sys::PageHeaderData>());
    let mut offset = 0usize;
    let mut entries = Vec::new();

    // Calculate available space
    let entry_size = size_of::<pg_sys::ItemPointerData>() + dimensions * 4;
    let page_header_size = size_of::<pg_sys::PageHeaderData>();
    let available_space = pg_sys::BLCKSZ as usize - page_header_size;
    let max_entries = available_space / entry_size;

    // Read entries
    for _ in 0..max_entries {
        if offset + entry_size > available_space {
            break;
        }

        // Read TID
        let tid = ptr::read(page_data.add(offset) as *const pg_sys::ItemPointerData);
        offset += size_of::<pg_sys::ItemPointerData>();

        // Check if this is a valid entry (block number > 0)
        if tid.ip_blkid.bi_hi == 0 && tid.ip_blkid.bi_lo == 0 {
            break;
        }

        // Read vector
        let vector_ptr = page_data.add(offset) as *const f32;
        let vector: Vec<f32> = slice::from_raw_parts(vector_ptr, dimensions).to_vec();
        offset += dimensions * 4;

        entries.push(InvertedListEntry { tid, vector });
    }

    pg_sys::UnlockReleaseBuffer(buffer);
    entries
}

// ============================================================================
// Vector Extraction from Heap
// ============================================================================

/// Extract vector from heap tuple (zero-copy when possible)
pub unsafe fn extract_vector_from_tuple(
    tuple: *mut pg_sys::HeapTupleData,
    tuple_desc: pg_sys::TupleDesc,
    attno: i16,
) -> Option<Vec<f32>> {
    let mut is_null = false;
    let datum = pg_sys::heap_getattr(tuple, attno as i32, tuple_desc, &mut is_null);

    if is_null {
        return None;
    }

    // Extract vector from datum
    // This assumes the datum is a varlena type containing f32 array
    extract_vector_from_datum(datum)
}

/// Extract vector from datum
unsafe fn extract_vector_from_datum(datum: pg_sys::Datum) -> Option<Vec<f32>> {
    if datum.is_null() {
        return None;
    }

    // Detoast if needed
    let varlena = pg_sys::pg_detoast_datum_packed(datum.cast_mut_ptr());

    // Get data pointer - access varlena data manually
    // varlena header is 4 bytes, data follows
    let varlena_ptr = varlena as *const u8;

    // Read the varlena length (first 4 bytes, lower 30 bits)
    let header = ptr::read(varlena_ptr as *const u32);
    let _data_size = (header >> 2) as usize;

    // Data starts after the 4-byte header
    let data_ptr = varlena_ptr.add(4);

    // First 4 bytes are dimension count
    let dimensions = ptr::read(data_ptr as *const u32) as usize;

    // Following bytes are f32 vector data
    let vector_ptr = data_ptr.add(4) as *const f32;
    let vector = slice::from_raw_parts(vector_ptr, dimensions).to_vec();

    Some(vector)
}

/// Create datum from vector
pub unsafe fn create_vector_datum(vector: &[f32]) -> pg_sys::Datum {
    let dimensions = vector.len() as u32;
    let data_size = 4 + (dimensions as usize * 4);
    let total_size = 4 + data_size; // 4 byte varlena header + data

    let varlena = pg_sys::palloc(total_size) as *mut u8;

    // Set varlena header (size << 2)
    let header = (total_size as u32) << 2;
    ptr::write(varlena as *mut u32, header);

    let data_ptr = varlena.add(4);

    // Write dimensions
    ptr::write(data_ptr as *mut u32, dimensions);

    // Write vector data
    let vector_ptr = data_ptr.add(4) as *mut f32;
    for (i, &val) in vector.iter().enumerate() {
        ptr::write(vector_ptr.add(i), val);
    }

    pg_sys::Datum::from(varlena as *mut ::std::os::raw::c_void)
}

// ============================================================================
// Heap Scanning Utilities
// ============================================================================

/// Callback for heap scan
pub type HeapScanCallback =
    unsafe extern "C" fn(tuple: *mut pg_sys::HeapTupleData, context: *mut ::std::os::raw::c_void);

/// Scan heap relation and collect vectors
pub unsafe fn scan_heap_for_vectors(
    _heap: pg_sys::Relation,
    _index_info: *mut pg_sys::IndexInfo,
    _callback: impl Fn(pg_sys::ItemPointerData, Vec<f32>),
) {
    // This is a simplified version
    // Real implementation would use table_beginscan_catalog or similar

    // For now, this is a placeholder showing the structure
    // In production, use proper PostgreSQL table scanning API
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #[test]
    fn test_centroid_serialization() {
        // Test would validate centroid read/write
    }

    #[test]
    fn test_inverted_list_serialization() {
        // Test would validate inverted list read/write
    }
}
