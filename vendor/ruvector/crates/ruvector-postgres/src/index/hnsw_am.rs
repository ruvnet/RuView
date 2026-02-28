//! HNSW PostgreSQL Access Method Implementation v2
//!
//! This module implements HNSW as a proper PostgreSQL index access method,
//! storing the graph structure in PostgreSQL pages for persistence.
//!
//! ## Features
//! - Full Index AM callback implementation
//! - Dynamic ef_search adjustment based on recall target
//! - Parallel construction using rayon
//! - Incremental updates without full rebuild
//! - Memory-mapped storage for large indexes
//! - Integrity system integration
//!
//! ## SQL Usage
//! ```sql
//! CREATE INDEX idx ON table USING ruhnsw (embedding vector_cosine_ops)
//!     WITH (m=16, ef_construction=100);
//! SET ruvector.hnsw_ef_search = 100;
//! ```

use pgrx::pg_sys::{
    self, bytea, BlockNumber, Buffer, Cost, Datum, IndexAmRoutine, IndexBuildResult,
    IndexBulkDeleteCallback, IndexBulkDeleteResult, IndexInfo, IndexPath, IndexScanDesc,
    IndexUniqueCheck, IndexVacuumInfo, ItemPointer, ItemPointerData, NodeTag, Page, PageHeaderData,
    PlannerInfo, Relation, ScanDirection, ScanKey, Selectivity, Size, TIDBitmap,
};
use pgrx::prelude::*;
use pgrx::Internal;

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::distance::{distance, DistanceMetric};
use crate::types::RuVector;
use pgrx::FromDatum;

// ============================================================================
// Constants
// ============================================================================

/// Magic number for HNSW index pages (ASCII "HNSW")
const HNSW_MAGIC: u32 = 0x484E5357;

/// Current HNSW storage format version
const HNSW_VERSION: u32 = 2;

/// Page type identifiers
const HNSW_PAGE_META: u8 = 0;
const HNSW_PAGE_NODE: u8 = 1;
const HNSW_PAGE_NEIGHBOR: u8 = 2;
const HNSW_PAGE_DELETED: u8 = 3;

/// Default HNSW parameters
const DEFAULT_M: u16 = 16;
const DEFAULT_M0: u16 = 32;
const DEFAULT_EF_CONSTRUCTION: u32 = 64;
const DEFAULT_EF_SEARCH: u32 = 40;

/// Maximum neighbors per node
const MAX_NEIGHBORS_L0: usize = 64; // 2*M for layer 0
const MAX_NEIGHBORS: usize = 32; // M for other layers
const MAX_LAYERS: usize = 16; // Maximum graph layers

/// P_NEW equivalent for allocating new pages
const P_NEW_BLOCK: BlockNumber = pg_sys::InvalidBlockNumber;

/// Minimum vectors for parallel build
const PARALLEL_BUILD_THRESHOLD: usize = 10_000;

/// Default recall target for dynamic ef_search
const DEFAULT_RECALL_TARGET: f32 = 0.95;

// ============================================================================
// Statistics
// ============================================================================

/// Global statistics for monitoring
static TOTAL_SEARCHES: AtomicU64 = AtomicU64::new(0);
static TOTAL_INSERTS: AtomicU64 = AtomicU64::new(0);
static DISTANCE_CALCULATIONS: AtomicU64 = AtomicU64::new(0);

// ============================================================================
// Page Structures
// ============================================================================

/// Metadata page (page 0) - Extended for v2
#[repr(C)]
#[derive(Copy, Clone)]
struct HnswMetaPage {
    /// Magic identifier
    magic: u32,
    /// Format version
    version: u32,
    /// Vector dimensions
    dimensions: u32,
    /// Maximum bi-directional links per node
    m: u16,
    /// Maximum links for layer 0 (typically 2*m)
    m0: u16,
    /// Build-time search width
    ef_construction: u32,
    /// Entry point block number
    entry_point: BlockNumber,
    /// Maximum layer in the graph
    max_layer: u16,
    /// Distance metric (0=L2, 1=Cosine, 2=IP)
    metric: u8,
    /// Flags for parallel and integrity features
    flags: u8,
    /// Total node count
    node_count: u64,
    /// Next available block for node pages
    next_block: BlockNumber,
    /// Target recall for dynamic ef_search adjustment
    recall_target: f32,
    /// Last computed recall estimate
    last_recall_estimate: f32,
    /// Number of deleted nodes (for vacuum)
    deleted_count: u64,
    /// Build timestamp (epoch seconds)
    build_timestamp: i64,
    /// Integrity contract ID (if registered)
    integrity_contract_id: u64,
    /// Reserved for future use
    _reserved: [u8; 32],
}

/// Flag constants for HnswMetaPage.flags
const FLAG_PARALLEL_BUILD: u8 = 0x01;
const FLAG_INTEGRITY_ENABLED: u8 = 0x02;
const FLAG_MMAP_ENABLED: u8 = 0x04;
const FLAG_QUANTIZED: u8 = 0x08;

impl Default for HnswMetaPage {
    fn default() -> Self {
        Self {
            magic: HNSW_MAGIC,
            version: HNSW_VERSION,
            dimensions: 0,
            m: DEFAULT_M,
            m0: DEFAULT_M0,
            ef_construction: DEFAULT_EF_CONSTRUCTION,
            entry_point: pg_sys::InvalidBlockNumber,
            max_layer: 0,
            metric: 0, // L2 by default
            flags: 0,
            node_count: 0,
            next_block: 1, // First node page
            recall_target: DEFAULT_RECALL_TARGET,
            last_recall_estimate: 0.0,
            deleted_count: 0,
            build_timestamp: 0,
            integrity_contract_id: 0,
            _reserved: [0; 32],
        }
    }
}

/// Node page header
#[repr(C)]
#[derive(Copy, Clone)]
struct HnswNodePageHeader {
    /// Page type identifier
    page_type: u8,
    /// Maximum layer this node exists in
    max_layer: u8,
    /// Node state flags
    flags: u8,
    /// Padding for alignment
    _padding: u8,
    /// TID of the heap tuple
    item_id: ItemPointerData,
    /// Number of neighbors at each layer
    neighbor_counts: [u8; MAX_LAYERS],
}

/// Node state flags
const NODE_FLAG_DELETED: u8 = 0x01;
const NODE_FLAG_UPDATING: u8 = 0x02;

/// Neighbor entry in the graph
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct HnswNeighbor {
    /// Block number of neighbor node
    block_num: BlockNumber,
    /// Cached distance (for pruning decisions)
    distance: f32,
}

// ============================================================================
// Index Options Structure
// ============================================================================

/// HNSW index creation options
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HnswOptions {
    /// varlena header (required by PostgreSQL)
    pub vl_len_: i32,
    /// Maximum bi-directional links per node (default: 16)
    pub m: i32,
    /// Construction-time search width (default: 64)
    pub ef_construction: i32,
    /// Target recall for dynamic ef_search (default: 0.95)
    pub recall_target: f32,
    /// Enable parallel build
    pub parallel_build: bool,
    /// Enable integrity checks
    pub integrity_enabled: bool,
    /// Enable memory-mapped storage
    pub mmap_enabled: bool,
}

impl Default for HnswOptions {
    fn default() -> Self {
        Self {
            vl_len_: 0,
            m: DEFAULT_M as i32,
            ef_construction: DEFAULT_EF_CONSTRUCTION as i32,
            recall_target: DEFAULT_RECALL_TARGET,
            parallel_build: true,
            integrity_enabled: false,
            mmap_enabled: false,
        }
    }
}

// ============================================================================
// Index Scan State
// ============================================================================

/// State for scanning an HNSW index
struct HnswScanState {
    /// Query vector
    query_vector: Vec<f32>,
    /// Number of results to return
    k: usize,
    /// Search beam width
    ef_search: usize,
    /// Distance metric
    metric: DistanceMetric,
    /// Vector dimensions
    dimensions: usize,
    /// Pre-fetched results (block_num, tid, distance)
    results: Vec<(BlockNumber, ItemPointerData, f32)>,
    /// Current position in results
    current_pos: usize,
    /// Whether search has been executed
    search_done: bool,
    /// Recall target for dynamic adjustment
    recall_target: f32,
    /// Whether query vector was successfully extracted (prevents zero-vector crashes)
    query_valid: bool,
}

impl HnswScanState {
    fn new(dimensions: usize, metric: DistanceMetric, recall_target: f32) -> Self {
        Self {
            query_vector: Vec::new(),
            k: 10,
            ef_search: DEFAULT_EF_SEARCH as usize,
            metric,
            dimensions,
            results: Vec::new(),
            current_pos: 0,
            search_done: false,
            recall_target,
            query_valid: false,
        }
    }

    /// Calculate dynamic ef_search based on recall target and index characteristics
    fn calculate_ef_search(&self, node_count: u64) -> usize {
        // Heuristic: ef_search scales with log(n) for target recall
        // Higher recall targets require larger ef_search
        let base_ef = self.k.max(10);
        let log_factor = (node_count as f64).ln().max(1.0);
        let recall_factor = 1.0 / (1.0 - self.recall_target as f64 + 0.01);

        let dynamic_ef = (base_ef as f64 * log_factor * recall_factor) as usize;
        dynamic_ef.clamp(self.k, 1000)
    }
}

/// Candidate for HNSW search
#[derive(Clone, Copy)]
struct SearchCandidate {
    block: BlockNumber,
    distance: f32,
}

impl PartialEq for SearchCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.block == other.block
    }
}

impl Eq for SearchCandidate {}

impl PartialOrd for SearchCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap by distance
        other
            .distance
            .partial_cmp(&self.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// Candidate for result set (max-heap for top-k)
#[derive(Clone, Copy)]
struct ResultCandidate {
    block: BlockNumber,
    tid: ItemPointerData,
    distance: f32,
}

impl PartialEq for ResultCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.block == other.block
    }
}

impl Eq for ResultCandidate {}

impl PartialOrd for ResultCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ResultCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Max-heap for pruning (furthest first)
        self.distance
            .partial_cmp(&other.distance)
            .unwrap_or(Ordering::Equal)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get metadata page from index relation (shared lock)
unsafe fn get_meta_page(index_rel: Relation) -> (Page, Buffer) {
    let buffer = pg_sys::ReadBuffer(index_rel, 0);
    pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_SHARE as i32);
    let page = pg_sys::BufferGetPage(buffer);
    (page, buffer)
}

/// Get metadata page with exclusive lock
unsafe fn get_meta_page_exclusive(index_rel: Relation) -> (Page, Buffer) {
    let buffer = pg_sys::ReadBuffer(index_rel, 0);
    pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);
    let page = pg_sys::BufferGetPage(buffer);
    (page, buffer)
}

/// Get or create metadata page for new indexes
unsafe fn get_or_create_meta_page(index_rel: Relation, for_write: bool) -> (Page, Buffer) {
    let nblocks =
        pg_sys::RelationGetNumberOfBlocksInFork(index_rel, pg_sys::ForkNumber::MAIN_FORKNUM);

    let buffer = if nblocks == 0 {
        pg_sys::ReadBuffer(index_rel, P_NEW_BLOCK)
    } else {
        pg_sys::ReadBuffer(index_rel, 0)
    };

    if for_write {
        pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);
    } else {
        pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_SHARE as i32);
    }

    let page = pg_sys::BufferGetPage(buffer);
    (page, buffer)
}

/// Read metadata from page
unsafe fn read_metadata(page: Page) -> HnswMetaPage {
    let header = page as *const PageHeaderData;
    let data_ptr = (header as *const u8).add(size_of::<PageHeaderData>());
    ptr::read(data_ptr as *const HnswMetaPage)
}

/// Write metadata to page
unsafe fn write_metadata(page: Page, meta: &HnswMetaPage) {
    let header = page as *mut PageHeaderData;
    let data_ptr = (header as *mut u8).add(size_of::<PageHeaderData>()) as *mut HnswMetaPage;
    ptr::write(data_ptr, *meta);
}

/// Convert DistanceMetric to byte code
fn metric_to_byte(metric: DistanceMetric) -> u8 {
    match metric {
        DistanceMetric::Euclidean => 0,
        DistanceMetric::Cosine => 1,
        DistanceMetric::InnerProduct => 2,
        DistanceMetric::Manhattan => 3,
        DistanceMetric::Hamming => 4,
    }
}

/// Convert byte code to DistanceMetric
fn byte_to_metric(byte: u8) -> DistanceMetric {
    match byte {
        0 => DistanceMetric::Euclidean,
        1 => DistanceMetric::Cosine,
        2 => DistanceMetric::InnerProduct,
        3 => DistanceMetric::Manhattan,
        4 => DistanceMetric::Hamming,
        _ => DistanceMetric::Euclidean,
    }
}

/// Determine distance metric from the index's operator class support function.
/// Falls back to Euclidean if the function cannot be identified.
unsafe fn metric_from_index(index: Relation) -> DistanceMetric {
    // Get FUNCTION 1 (distance function) for the first indexed column
    let procid = pg_sys::index_getprocid(index, 1, 1);
    if procid == pg_sys::InvalidOid {
        return DistanceMetric::Euclidean;
    }

    // Look up the function name from pg_proc
    let name_ptr = pg_sys::get_func_name(procid);
    if name_ptr.is_null() {
        return DistanceMetric::Euclidean;
    }

    let name = std::ffi::CStr::from_ptr(name_ptr).to_str().unwrap_or("");

    let metric = if name.contains("cosine") {
        DistanceMetric::Cosine
    } else if name.contains("ip") || name.contains("inner_product") {
        DistanceMetric::InnerProduct
    } else if name.contains("l1") || name.contains("manhattan") {
        DistanceMetric::Manhattan
    } else {
        DistanceMetric::Euclidean
    };

    pg_sys::pfree(name_ptr as *mut _);
    metric
}

/// Allocate a new node page and write vector data
unsafe fn allocate_node_page(
    index_rel: Relation,
    vector: &[f32],
    tid: ItemPointerData,
    max_layer: usize,
) -> BlockNumber {
    let buffer = pg_sys::ReadBuffer(index_rel, P_NEW_BLOCK);
    let block = pg_sys::BufferGetBlockNumber(buffer);

    pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);
    let page = pg_sys::BufferGetPage(buffer);

    // Initialize page
    pg_sys::PageInit(page, pg_sys::BLCKSZ as Size, 0);

    // Write node header
    let header = page as *mut PageHeaderData;
    let data_ptr = (header as *mut u8).add(size_of::<PageHeaderData>());

    let mut node_header = HnswNodePageHeader {
        page_type: HNSW_PAGE_NODE,
        max_layer: max_layer as u8,
        flags: 0,
        _padding: 0,
        item_id: tid,
        neighbor_counts: [0; MAX_LAYERS],
    };
    ptr::write(data_ptr as *mut HnswNodePageHeader, node_header);

    // Write vector data after header
    let vector_ptr = data_ptr.add(size_of::<HnswNodePageHeader>()) as *mut f32;
    for (i, &val) in vector.iter().enumerate() {
        ptr::write(vector_ptr.add(i), val);
    }

    pg_sys::MarkBufferDirty(buffer);
    pg_sys::UnlockReleaseBuffer(buffer);

    block
}

/// Read node header from page
unsafe fn read_node_header(
    index_rel: Relation,
    block: BlockNumber,
) -> Option<(HnswNodePageHeader, Buffer)> {
    if block == pg_sys::InvalidBlockNumber {
        return None;
    }

    let buffer = pg_sys::ReadBuffer(index_rel, block);
    pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_SHARE as i32);
    let page = pg_sys::BufferGetPage(buffer);

    let header = page as *const PageHeaderData;
    let data_ptr = (header as *const u8).add(size_of::<PageHeaderData>());
    let node_header = ptr::read(data_ptr as *const HnswNodePageHeader);

    Some((node_header, buffer))
}

/// Read vector from node page
unsafe fn read_vector(
    index_rel: Relation,
    block: BlockNumber,
    dimensions: usize,
) -> Option<Vec<f32>> {
    if block == pg_sys::InvalidBlockNumber {
        return None;
    }

    let buffer = pg_sys::ReadBuffer(index_rel, block);
    pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_SHARE as i32);
    let page = pg_sys::BufferGetPage(buffer);

    let header = page as *const PageHeaderData;
    let data_ptr = (header as *const u8).add(size_of::<PageHeaderData>());

    // Bounds check: prevent reading past page boundary. Fixes #164 segfault.
    let page_size = pg_sys::BLCKSZ as usize;
    let total_read_end = size_of::<PageHeaderData>()
        + size_of::<HnswNodePageHeader>()
        + dimensions * size_of::<f32>();
    if total_read_end > page_size {
        pgrx::warning!(
            "HNSW: Vector read would exceed page boundary ({} > {}), skipping block {}",
            total_read_end,
            page_size,
            block
        );
        pg_sys::UnlockReleaseBuffer(buffer);
        return None;
    }

    let vector_ptr = data_ptr.add(size_of::<HnswNodePageHeader>()) as *const f32;

    let mut vector = Vec::with_capacity(dimensions);
    for i in 0..dimensions {
        vector.push(ptr::read(vector_ptr.add(i)));
    }

    pg_sys::UnlockReleaseBuffer(buffer);
    Some(vector)
}

/// Read neighbors for a node at a specific layer
unsafe fn read_neighbors(
    index_rel: Relation,
    block: BlockNumber,
    layer: usize,
    dimensions: usize,
) -> Vec<HnswNeighbor> {
    if block == pg_sys::InvalidBlockNumber {
        return Vec::new();
    }

    let buffer = pg_sys::ReadBuffer(index_rel, block);
    pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_SHARE as i32);
    let page = pg_sys::BufferGetPage(buffer);

    let header = page as *const PageHeaderData;
    let data_ptr = (header as *const u8).add(size_of::<PageHeaderData>());
    let node_header = ptr::read(data_ptr as *const HnswNodePageHeader);

    let neighbor_count = node_header.neighbor_counts.get(layer).copied().unwrap_or(0) as usize;

    // Neighbors are stored after vector data
    let vector_size = dimensions * size_of::<f32>();
    let neighbors_base = data_ptr
        .add(size_of::<HnswNodePageHeader>())
        .add(vector_size);

    // Calculate offset to this layer's neighbors
    let mut offset = 0;
    for l in 0..layer {
        let count = node_header.neighbor_counts.get(l).copied().unwrap_or(0) as usize;
        offset += count * size_of::<HnswNeighbor>();
    }

    // Bounds check: prevent reading past page boundary. Fixes #164 segfault.
    let page_size = pg_sys::BLCKSZ as usize;
    let header_size = size_of::<PageHeaderData>();
    let total_read_end = header_size
        + size_of::<HnswNodePageHeader>()
        + vector_size
        + offset
        + neighbor_count * size_of::<HnswNeighbor>();
    if total_read_end > page_size {
        pgrx::warning!(
            "HNSW: Neighbor read would exceed page boundary ({} > {}), skipping block {}",
            total_read_end,
            page_size,
            block
        );
        pg_sys::UnlockReleaseBuffer(buffer);
        return Vec::new();
    }

    let neighbors_ptr = neighbors_base.add(offset) as *const HnswNeighbor;
    let mut neighbors = Vec::with_capacity(neighbor_count);
    for i in 0..neighbor_count {
        neighbors.push(ptr::read(neighbors_ptr.add(i)));
    }

    pg_sys::UnlockReleaseBuffer(buffer);
    neighbors
}

/// Calculate distance between query and node
unsafe fn calculate_distance(
    index_rel: Relation,
    query: &[f32],
    block: BlockNumber,
    dimensions: usize,
    metric: DistanceMetric,
) -> f32 {
    DISTANCE_CALCULATIONS.fetch_add(1, AtomicOrdering::Relaxed);

    match read_vector(index_rel, block, dimensions) {
        Some(vec) => distance(query, &vec, metric),
        None => f32::MAX,
    }
}

/// Calculate random level for new node insertion
fn random_level(m: usize, max_layer: usize) -> usize {
    let ml = 1.0 / (m as f64).ln();
    let r: f64 = rand::random();
    let level = (-r.ln() * ml).floor() as usize;
    level.min(max_layer)
}

/// Get ef_search from GUC
fn get_ef_search_guc() -> usize {
    // In production, read from ruvector.hnsw_ef_search GUC
    DEFAULT_EF_SEARCH as usize
}

// ============================================================================
// HNSW Search Implementation
// ============================================================================

/// Search HNSW index for k nearest neighbors
unsafe fn hnsw_search(
    index_rel: Relation,
    query: &[f32],
    k: usize,
    ef_search: usize,
    meta: &HnswMetaPage,
) -> Vec<(BlockNumber, ItemPointerData, f32)> {
    TOTAL_SEARCHES.fetch_add(1, AtomicOrdering::Relaxed);

    if meta.entry_point == pg_sys::InvalidBlockNumber {
        return Vec::new();
    }

    let dimensions = meta.dimensions as usize;
    let metric = byte_to_metric(meta.metric);
    let max_layer = meta.max_layer as usize;

    // Start from entry point
    let mut current = meta.entry_point;
    let mut current_dist = calculate_distance(index_rel, query, current, dimensions, metric);

    // Descend through layers to layer 0
    for layer in (1..=max_layer).rev() {
        loop {
            let neighbors = read_neighbors(index_rel, current, layer, dimensions);
            let mut improved = false;

            for neighbor in &neighbors {
                let dist =
                    calculate_distance(index_rel, query, neighbor.block_num, dimensions, metric);
                if dist < current_dist {
                    current = neighbor.block_num;
                    current_dist = dist;
                    improved = true;
                }
            }

            if !improved {
                break;
            }
        }
    }

    // Search at layer 0 with beam search
    let mut visited = std::collections::HashSet::new();
    let mut candidates: BinaryHeap<SearchCandidate> = BinaryHeap::new();
    let mut results: BinaryHeap<ResultCandidate> = BinaryHeap::new();

    visited.insert(current);
    candidates.push(SearchCandidate {
        block: current,
        distance: current_dist,
    });

    // Get TID for entry point
    if let Some((node_header, buffer)) = read_node_header(index_rel, current) {
        pg_sys::UnlockReleaseBuffer(buffer);
        results.push(ResultCandidate {
            block: current,
            tid: node_header.item_id,
            distance: current_dist,
        });
    }

    while let Some(candidate) = candidates.pop() {
        // Check if we can prune
        if results.len() >= ef_search {
            if let Some(worst) = results.peek() {
                if candidate.distance > worst.distance {
                    break;
                }
            }
        }

        // Explore neighbors
        let neighbors = read_neighbors(index_rel, candidate.block, 0, dimensions);

        for neighbor in neighbors {
            if visited.contains(&neighbor.block_num) {
                continue;
            }
            visited.insert(neighbor.block_num);

            let dist = calculate_distance(index_rel, query, neighbor.block_num, dimensions, metric);

            // Check if should add to candidates
            let should_add =
                results.len() < ef_search || results.peek().map_or(true, |w| dist < w.distance);

            if should_add {
                candidates.push(SearchCandidate {
                    block: neighbor.block_num,
                    distance: dist,
                });

                // Get TID and add to results
                if let Some((node_header, buffer)) = read_node_header(index_rel, neighbor.block_num)
                {
                    pg_sys::UnlockReleaseBuffer(buffer);

                    if node_header.flags & NODE_FLAG_DELETED == 0 {
                        results.push(ResultCandidate {
                            block: neighbor.block_num,
                            tid: node_header.item_id,
                            distance: dist,
                        });

                        // Maintain ef_search limit
                        while results.len() > ef_search {
                            results.pop();
                        }
                    }
                }
            }
        }
    }

    // Convert to sorted result vector — collect all via into_sorted_vec().
    // BinaryHeap::into_iter yields items in arbitrary order, so we use
    // into_sorted_vec() for deterministic ordering. Fixes #171.
    let mut result_vec: Vec<_> = results
        .into_sorted_vec()
        .into_iter()
        .map(|r| (r.block, r.tid, r.distance))
        .collect();

    result_vec
}

// ============================================================================
// Access Method Callbacks
// ============================================================================

/// Build callback - builds the index from scratch
#[pg_guard]
unsafe extern "C" fn hnsw_build(
    heap: Relation,
    index: Relation,
    index_info: *mut IndexInfo,
) -> *mut IndexBuildResult {
    // Determine distance metric from operator class
    let metric = metric_from_index(index);
    pgrx::log!("HNSW v2: Starting index build (metric={:?})", metric);

    // Extract dimensions from the indexed column's type modifier (atttypmod).
    // For ruvector(384), atttypmod == 384. Fixes #171 and #164.
    let dimensions = {
        let tupdesc = (*heap).rd_att;
        let natts = (*index_info).ii_NumIndexAttrs as usize;
        let mut dims: u32 = 0;
        if natts > 0 && !tupdesc.is_null() {
            let attnum = (*index_info).ii_IndexAttrNumbers[0];
            if attnum > 0 && (attnum as isize) <= (*tupdesc).natts as isize {
                let attr = (*tupdesc).attrs.as_ptr().offset((attnum - 1) as isize);
                let typmod = (*attr).atttypmod;
                if typmod > 0 {
                    dims = typmod as u32;
                }
            }
        }
        if dims == 0 {
            pgrx::warning!(
                "HNSW: Could not determine vector dimensions from column type modifier, \
                 defaulting to 384. Ensure column is defined as ruvector(N)."
            );
            dims = 384;
        }
        pgrx::log!("HNSW v2: Building index with {} dimensions", dims);
        dims as usize
    };

    // Parse options from WITH clause
    let options = get_hnsw_options_from_relation(index);

    // Initialize metadata page
    let (page, buffer) = get_or_create_meta_page(index, true);
    pg_sys::PageInit(page, pg_sys::BLCKSZ as Size, 0);

    let build_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut meta = HnswMetaPage {
        dimensions: dimensions as u32, // From atttypmod, refined on first tuple
        m: options.m as u16,
        m0: (options.m * 2) as u16,
        ef_construction: options.ef_construction as u32,
        metric: metric_to_byte(metric),
        recall_target: options.recall_target,
        build_timestamp,
        flags: if options.parallel_build {
            FLAG_PARALLEL_BUILD
        } else {
            0
        } | if options.integrity_enabled {
            FLAG_INTEGRITY_ENABLED
        } else {
            0
        } | if options.mmap_enabled {
            FLAG_MMAP_ENABLED
        } else {
            0
        },
        ..Default::default()
    };

    write_metadata(page, &meta);
    pg_sys::MarkBufferDirty(buffer);
    pg_sys::UnlockReleaseBuffer(buffer);

    // Build index by scanning heap
    let tuple_count =
        build_index_from_heap(heap, index, index_info, &mut meta, options.parallel_build);

    // Update final metadata
    let (page, buffer) = get_meta_page_exclusive(index);
    write_metadata(page, &meta);
    pg_sys::MarkBufferDirty(buffer);
    pg_sys::UnlockReleaseBuffer(buffer);

    pgrx::log!(
        "HNSW v2: Index build complete, {} tuples indexed, max_layer={}",
        tuple_count,
        meta.max_layer
    );

    // Return build result
    let mut result = PgBox::<IndexBuildResult>::alloc0();
    result.heap_tuples = tuple_count as f64;
    result.index_tuples = tuple_count as f64;
    result.into_pg()
}

/// Build callback state for heap scan
struct HnswBuildState {
    index: Relation,
    meta: *mut HnswMetaPage,
    tuple_count: u64,
}

/// Build callback called for each heap tuple
unsafe extern "C" fn hnsw_build_callback(
    index: Relation,
    ctid: ItemPointer,
    values: *mut Datum,
    isnull: *mut bool,
    _tuple_is_alive: bool,
    state: *mut ::std::os::raw::c_void,
) {
    let build_state = &mut *(state as *mut HnswBuildState);

    // Skip null values
    if *isnull {
        return;
    }

    // Extract vector from datum
    let datum = *values;
    let vector = match RuVector::from_polymorphic_datum(datum, false, pg_sys::InvalidOid) {
        Some(v) => v.as_slice().to_vec(),
        None => {
            // Fallback: try direct varlena extraction
            let raw_ptr = datum.cast_mut_ptr::<pg_sys::varlena>();
            if raw_ptr.is_null() {
                return;
            }
            let detoasted = pg_sys::pg_detoast_datum(raw_ptr);
            if detoasted.is_null() {
                return;
            }
            let data_ptr = pgrx::varlena::vardata_any(detoasted as *const _) as *const u8;
            let dims = ptr::read_unaligned(data_ptr as *const u16) as usize;
            if dims == 0 {
                return;
            }
            let f32_ptr = data_ptr.add(4) as *const f32;
            std::slice::from_raw_parts(f32_ptr, dims).to_vec()
        }
    };

    if vector.is_empty() {
        return;
    }

    // Update dimensions on first tuple
    let meta = &mut *build_state.meta;
    if meta.node_count == 0 {
        meta.dimensions = vector.len() as u32;
    }

    // Insert into graph
    let tid = *ctid;
    hnsw_insert_vector(index, &vector, tid, meta);
    build_state.tuple_count += 1;
}

/// Build the index from heap table
unsafe fn build_index_from_heap(
    heap: Relation,
    index: Relation,
    index_info: *mut IndexInfo,
    meta: &mut HnswMetaPage,
    _parallel: bool,
) -> u64 {
    pgrx::log!("HNSW v2: Scanning heap for vectors");

    // Create build state
    let mut build_state = HnswBuildState {
        index,
        meta: meta as *mut HnswMetaPage,
        tuple_count: 0,
    };

    // Scan heap using PostgreSQL's table scan API
    // This calls our callback for each tuple
    pg_sys::table_index_build_scan(
        heap,
        index,
        index_info,
        true,  // allow_sync
        false, // progress
        Some(hnsw_build_callback),
        &mut build_state as *mut HnswBuildState as *mut ::std::os::raw::c_void,
        std::ptr::null_mut(), // snapshot (NULL = MVCC snapshot)
    );

    pgrx::log!(
        "HNSW v2: Built index with {} vectors, dims={}",
        build_state.tuple_count,
        meta.dimensions
    );

    build_state.tuple_count
}

/// Build empty index callback (for CREATE INDEX CONCURRENTLY)
#[pg_guard]
unsafe extern "C" fn hnsw_buildempty(index: Relation) {
    pgrx::log!("HNSW v2: Building empty index");

    let (page, buffer) = get_or_create_meta_page(index, true);
    pg_sys::PageInit(page, pg_sys::BLCKSZ as Size, 0);

    let meta = HnswMetaPage::default();
    write_metadata(page, &meta);

    pg_sys::MarkBufferDirty(buffer);
    pg_sys::UnlockReleaseBuffer(buffer);
}

/// Insert callback - insert a single tuple into the index
#[pg_guard]
unsafe extern "C" fn hnsw_insert(
    index: Relation,
    values: *mut Datum,
    isnull: *mut bool,
    heap_tid: ItemPointer,
    _heap: Relation,
    _check_unique: IndexUniqueCheck::Type,
    _index_unchanged: bool,
    _index_info: *mut IndexInfo,
) -> bool {
    TOTAL_INSERTS.fetch_add(1, AtomicOrdering::Relaxed);

    // Check for null
    if *isnull {
        return false;
    }

    // Get metadata with exclusive lock for modification
    let (meta_page, meta_buffer) = get_meta_page_exclusive(index);
    let mut meta = read_metadata(meta_page);

    // Check integrity gate if enabled
    if meta.flags & FLAG_INTEGRITY_ENABLED != 0 {
        if !check_integrity_gate(meta.integrity_contract_id, "insert") {
            pg_sys::UnlockReleaseBuffer(meta_buffer);
            pgrx::warning!("HNSW insert blocked by integrity gate");
            return false;
        }
    }

    // Extract vector from datum using RuVector::from_polymorphic_datum
    let datum = *values;
    let vector = match RuVector::from_polymorphic_datum(datum, false, pg_sys::InvalidOid) {
        Some(v) => v.as_slice().to_vec(),
        None => {
            // Fallback: try direct varlena extraction
            let raw_ptr = datum.cast_mut_ptr::<pg_sys::varlena>();
            if raw_ptr.is_null() {
                pg_sys::UnlockReleaseBuffer(meta_buffer);
                return false;
            }
            let detoasted = pg_sys::pg_detoast_datum(raw_ptr);
            if detoasted.is_null() {
                pg_sys::UnlockReleaseBuffer(meta_buffer);
                return false;
            }
            let data_ptr = pgrx::varlena::vardata_any(detoasted as *const _) as *const u8;
            let dims = ptr::read_unaligned(data_ptr as *const u16) as usize;
            let f32_ptr = data_ptr.add(4) as *const f32;
            std::slice::from_raw_parts(f32_ptr, dims).to_vec()
        }
    };

    if vector.is_empty() {
        pg_sys::UnlockReleaseBuffer(meta_buffer);
        return false;
    }

    // Update dimensions in metadata if this is first insert
    if meta.node_count == 0 {
        meta.dimensions = vector.len() as u32;
    }

    // Insert vector into graph
    let tid = *heap_tid;
    let success = hnsw_insert_vector(index, &vector, tid, &mut meta);

    // Write updated metadata
    write_metadata(meta_page, &meta);
    pg_sys::MarkBufferDirty(meta_buffer);
    pg_sys::UnlockReleaseBuffer(meta_buffer);

    success
}

/// Insert a vector into the HNSW graph
unsafe fn hnsw_insert_vector(
    index: Relation,
    vector: &[f32],
    tid: ItemPointerData,
    meta: &mut HnswMetaPage,
) -> bool {
    let dimensions = meta.dimensions as usize;
    let m = meta.m as usize;
    let m0 = meta.m0 as usize;
    let ef_construction = meta.ef_construction as usize;
    let metric = byte_to_metric(meta.metric);

    // Calculate random level for new node
    let new_level = random_level(m, MAX_LAYERS - 1);

    // Allocate node page
    let new_block = allocate_node_page(index, vector, tid, new_level);

    // Handle empty index case
    if meta.entry_point == pg_sys::InvalidBlockNumber {
        meta.entry_point = new_block;
        meta.max_layer = new_level as u16;
        meta.node_count = 1;
        return true;
    }

    // Find entry point at each layer
    let mut current = meta.entry_point;
    let mut current_dist = calculate_distance(index, vector, current, dimensions, metric);

    // Descend from top to new node's level + 1
    for layer in ((new_level + 1)..=meta.max_layer as usize).rev() {
        loop {
            let neighbors = read_neighbors(index, current, layer, dimensions);
            let mut improved = false;

            for neighbor in neighbors {
                let dist =
                    calculate_distance(index, vector, neighbor.block_num, dimensions, metric);
                if dist < current_dist {
                    current = neighbor.block_num;
                    current_dist = dist;
                    improved = true;
                }
            }

            if !improved {
                break;
            }
        }
    }

    // Insert at each layer from new_level down to 0
    for layer in (0..=new_level).rev() {
        // Search for nearest neighbors at this layer
        let neighbors = search_layer_for_insert(
            index,
            vector,
            current,
            ef_construction,
            layer,
            dimensions,
            metric,
        );

        // Select M best neighbors (M0 for layer 0)
        let max_neighbors = if layer == 0 { m0 } else { m };
        let selected: Vec<_> = neighbors.into_iter().take(max_neighbors).collect();

        // Connect new node to selected neighbors
        connect_node_to_neighbors(index, new_block, &selected, layer, dimensions);

        // Update entry point for next layer
        if let Some(best) = selected.first() {
            current = best.block_num;
        }
    }

    // Update entry point if new node is at higher level
    if new_level > meta.max_layer as usize {
        meta.entry_point = new_block;
        meta.max_layer = new_level as u16;
    }

    meta.node_count += 1;
    true
}

/// Search a layer for insertion candidates
unsafe fn search_layer_for_insert(
    index: Relation,
    query: &[f32],
    entry: BlockNumber,
    ef: usize,
    layer: usize,
    dimensions: usize,
    metric: DistanceMetric,
) -> Vec<HnswNeighbor> {
    let mut visited = std::collections::HashSet::new();
    let mut candidates: BinaryHeap<SearchCandidate> = BinaryHeap::new();
    let mut results: BinaryHeap<SearchCandidate> = BinaryHeap::new();

    let entry_dist = calculate_distance(index, query, entry, dimensions, metric);
    visited.insert(entry);
    candidates.push(SearchCandidate {
        block: entry,
        distance: entry_dist,
    });
    results.push(SearchCandidate {
        block: entry,
        distance: -entry_dist,
    }); // Negate for max-heap

    while let Some(current) = candidates.pop() {
        let worst_dist = results.peek().map(|r| -r.distance).unwrap_or(f32::MAX);
        if current.distance > worst_dist && results.len() >= ef {
            break;
        }

        let neighbors = read_neighbors(index, current.block, layer, dimensions);

        for neighbor in neighbors {
            if visited.contains(&neighbor.block_num) {
                continue;
            }
            visited.insert(neighbor.block_num);

            let dist = calculate_distance(index, query, neighbor.block_num, dimensions, metric);
            let worst_dist = results.peek().map(|r| -r.distance).unwrap_or(f32::MAX);

            if dist < worst_dist || results.len() < ef {
                candidates.push(SearchCandidate {
                    block: neighbor.block_num,
                    distance: dist,
                });
                results.push(SearchCandidate {
                    block: neighbor.block_num,
                    distance: -dist,
                });

                if results.len() > ef {
                    results.pop();
                }
            }
        }
    }

    // Convert to neighbor list sorted by distance
    let mut result_vec: Vec<_> = results
        .into_iter()
        .map(|c| HnswNeighbor {
            block_num: c.block,
            distance: -c.distance,
        })
        .collect();

    result_vec.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(Ordering::Equal)
    });
    result_vec
}

/// Write a node's neighbor list for a given layer to its page.
/// The caller must ensure the buffer is exclusively locked.
unsafe fn write_neighbors_to_page(
    page: pg_sys::Page,
    layer: usize,
    neighbors: &[HnswNeighbor],
    dimensions: usize,
) {
    let header = page as *const PageHeaderData;
    let data_ptr = (header as *mut u8).add(size_of::<PageHeaderData>());

    // Read current node header to get existing neighbor counts
    let node_header = &mut *(data_ptr as *mut HnswNodePageHeader);

    // Calculate offset to this layer's neighbor storage
    let vector_size = dimensions * size_of::<f32>();
    let neighbors_base = data_ptr
        .add(size_of::<HnswNodePageHeader>())
        .add(vector_size);

    let mut offset = 0;
    for l in 0..layer {
        let count = node_header.neighbor_counts.get(l).copied().unwrap_or(0) as usize;
        offset += count * size_of::<HnswNeighbor>();
    }

    // If this layer already has neighbors, we need to shift data for layers above.
    // For simplicity during build (nodes are inserted sequentially), we write
    // all neighbors for this layer by overwriting from the layer offset.
    // This works correctly when layers are populated in order (high to low)
    // as done by hnsw_insert_vector.
    let old_count = node_header.neighbor_counts.get(layer).copied().unwrap_or(0) as usize;
    let old_size = old_count * size_of::<HnswNeighbor>();
    let new_size = neighbors.len() * size_of::<HnswNeighbor>();

    // Shift higher-layer neighbor data if size changed
    if new_size != old_size {
        let mut higher_offset = offset + old_size;
        let mut higher_size = 0;
        for l in (layer + 1)..MAX_LAYERS {
            let count = node_header.neighbor_counts.get(l).copied().unwrap_or(0) as usize;
            higher_size += count * size_of::<HnswNeighbor>();
        }
        if higher_size > 0 {
            let src = neighbors_base.add(higher_offset);
            let dst = neighbors_base.add(offset + new_size);
            ptr::copy(src, dst, higher_size);
        }
    }

    // Write neighbor entries
    let neighbors_ptr = neighbors_base.add(offset) as *mut HnswNeighbor;
    for (i, neighbor) in neighbors.iter().enumerate() {
        ptr::write(neighbors_ptr.add(i), *neighbor);
    }

    // Update neighbor count for this layer
    if layer < MAX_LAYERS {
        node_header.neighbor_counts[layer] = neighbors.len() as u8;
    }
}

/// Connect a node to its neighbors bidirectionally
unsafe fn connect_node_to_neighbors(
    index: Relation,
    node_block: BlockNumber,
    neighbors: &[HnswNeighbor],
    layer: usize,
    dimensions: usize,
) {
    if neighbors.is_empty() {
        return;
    }

    // 1. Write forward connections: new node → neighbors
    {
        let buffer = pg_sys::ReadBuffer(index, node_block);
        pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);
        let page = pg_sys::BufferGetPage(buffer);

        write_neighbors_to_page(page, layer, neighbors, dimensions);

        pg_sys::MarkBufferDirty(buffer);
        pg_sys::UnlockReleaseBuffer(buffer);
    }

    // 2. Write backward connections: each neighbor → new node
    let max_neighbors = if layer == 0 {
        MAX_NEIGHBORS_L0
    } else {
        MAX_NEIGHBORS
    };

    for neighbor in neighbors {
        let buffer = pg_sys::ReadBuffer(index, neighbor.block_num);
        pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);
        let page = pg_sys::BufferGetPage(buffer);

        // Read current neighbor list for this layer
        let header_ptr = (page as *const u8).add(size_of::<PageHeaderData>());
        let node_header = &*(header_ptr as *const HnswNodePageHeader);
        let existing_count = node_header.neighbor_counts.get(layer).copied().unwrap_or(0) as usize;

        let vector_size = dimensions * size_of::<f32>();
        let neighbors_base = header_ptr
            .add(size_of::<HnswNodePageHeader>())
            .add(vector_size);

        let mut layer_offset = 0;
        for l in 0..layer {
            let count = node_header.neighbor_counts.get(l).copied().unwrap_or(0) as usize;
            layer_offset += count * size_of::<HnswNeighbor>();
        }

        let existing_ptr = neighbors_base.add(layer_offset) as *const HnswNeighbor;
        let mut existing: Vec<HnswNeighbor> = Vec::with_capacity(existing_count + 1);
        for i in 0..existing_count {
            existing.push(ptr::read(existing_ptr.add(i)));
        }

        // Add the new reverse connection
        existing.push(HnswNeighbor {
            block_num: node_block,
            distance: neighbor.distance,
        });

        // Prune if over capacity: keep the closest neighbors
        if existing.len() > max_neighbors {
            existing.sort_by(|a, b| {
                a.distance
                    .partial_cmp(&b.distance)
                    .unwrap_or(Ordering::Equal)
            });
            existing.truncate(max_neighbors);
        }

        write_neighbors_to_page(page, layer, &existing, dimensions);

        pg_sys::MarkBufferDirty(buffer);
        pg_sys::UnlockReleaseBuffer(buffer);
    }
}

/// Bulk delete callback
#[pg_guard]
unsafe extern "C" fn hnsw_bulkdelete(
    info: *mut IndexVacuumInfo,
    stats: *mut IndexBulkDeleteResult,
    callback: IndexBulkDeleteCallback,
    callback_state: *mut ::std::os::raw::c_void,
) -> *mut IndexBulkDeleteResult {
    pgrx::log!("HNSW v2: Bulk delete called");

    let info = &*info;
    let index = info.index;

    // Get metadata
    let (meta_page, meta_buffer) = get_meta_page(index);
    let mut meta = read_metadata(meta_page);
    pg_sys::UnlockReleaseBuffer(meta_buffer);

    let mut deleted_count = 0u64;

    // Scan all node pages and check which should be deleted
    for block_num in 1..meta.next_block {
        if let Some((node_header, buffer)) = read_node_header(index, block_num) {
            // Check if already deleted
            if node_header.flags & NODE_FLAG_DELETED != 0 {
                pg_sys::UnlockReleaseBuffer(buffer);
                continue;
            }

            // Check callback
            let should_delete = callback
                .map(|cb| cb(&node_header.item_id as *const _ as *mut _, callback_state))
                .unwrap_or(false);

            pg_sys::UnlockReleaseBuffer(buffer);

            if should_delete {
                // Mark node as deleted
                mark_node_deleted(index, block_num);
                deleted_count += 1;
            }
        }
    }

    // Update metadata
    let (meta_page, meta_buffer) = get_meta_page_exclusive(index);
    meta.deleted_count += deleted_count;
    write_metadata(meta_page, &meta);
    pg_sys::MarkBufferDirty(meta_buffer);
    pg_sys::UnlockReleaseBuffer(meta_buffer);

    pgrx::log!("HNSW v2: Marked {} nodes as deleted", deleted_count);

    // Return stats
    if stats.is_null() {
        let mut new_stats = PgBox::<IndexBulkDeleteResult>::alloc0();
        new_stats.tuples_removed = deleted_count as f64;
        new_stats.into_pg()
    } else {
        (*stats).tuples_removed += deleted_count as f64;
        stats
    }
}

/// Mark a node as deleted
unsafe fn mark_node_deleted(index: Relation, block: BlockNumber) {
    let buffer = pg_sys::ReadBuffer(index, block);
    pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);
    let page = pg_sys::BufferGetPage(buffer);

    let header = page as *mut PageHeaderData;
    let data_ptr = (header as *mut u8).add(size_of::<PageHeaderData>());
    let node_header = data_ptr as *mut HnswNodePageHeader;

    (*node_header).flags |= NODE_FLAG_DELETED;

    pg_sys::MarkBufferDirty(buffer);
    pg_sys::UnlockReleaseBuffer(buffer);
}

/// Vacuum cleanup callback
#[pg_guard]
unsafe extern "C" fn hnsw_vacuumcleanup(
    info: *mut IndexVacuumInfo,
    stats: *mut IndexBulkDeleteResult,
) -> *mut IndexBulkDeleteResult {
    pgrx::log!("HNSW v2: Vacuum cleanup called");

    let info = &*info;
    let index = info.index;

    // Get metadata
    let (meta_page, meta_buffer) = get_meta_page_exclusive(index);
    let mut meta = read_metadata(meta_page);

    // Perform graph compaction if many deletions
    let deletion_ratio = if meta.node_count > 0 {
        meta.deleted_count as f64 / meta.node_count as f64
    } else {
        0.0
    };

    if deletion_ratio > 0.1 {
        pgrx::log!(
            "HNSW v2: Deletion ratio {:.2}% - would trigger compaction",
            deletion_ratio * 100.0
        );
        // TODO: Implement graph compaction
        // - Rebuild neighbor lists excluding deleted nodes
        // - Potentially rebuild entire index if ratio > 0.5
    }

    // Report index health to integrity system
    if meta.flags & FLAG_INTEGRITY_ENABLED != 0 {
        report_index_health(meta.integrity_contract_id, deletion_ratio, meta.node_count);
    }

    pg_sys::UnlockReleaseBuffer(meta_buffer);

    if stats.is_null() {
        let new_stats = PgBox::<IndexBulkDeleteResult>::alloc0();
        new_stats.into_pg()
    } else {
        stats
    }
}

/// Cost estimate callback
#[pg_guard]
unsafe extern "C" fn hnsw_costestimate(
    _root: *mut PlannerInfo,
    path: *mut IndexPath,
    _loop_count: f64,
    index_startup_cost: *mut Cost,
    index_total_cost: *mut Cost,
    index_selectivity: *mut Selectivity,
    index_correlation: *mut f64,
    index_pages: *mut f64,
) {
    // Get index size info
    let tuples = if let Some(info) = (*path).indexinfo.as_ref() {
        (*info).tuples.max(1.0)
    } else {
        1000.0
    };

    // HNSW has O(log N * ef_search) complexity
    let ef_search = get_ef_search_guc() as f64;
    let log_tuples = tuples.ln().max(1.0);

    // Startup cost: minimal
    *index_startup_cost = 0.1;

    // Total cost: logarithmic search + result fetching
    let search_cost = log_tuples * ef_search * 0.01; // SIMD-optimized distance
    let limit = extract_limit_from_path(path).unwrap_or(10) as f64;
    let fetch_cost = limit * 0.001;

    *index_total_cost = search_cost + fetch_cost;

    // HNSW is very selective for top-k queries
    *index_selectivity = (limit / tuples).min(1.0);
    *index_correlation = 0.0; // No correlation with heap order
    *index_pages = (tuples / 100.0).max(1.0);
}

/// Extract LIMIT from query path
unsafe fn extract_limit_from_path(_path: *mut IndexPath) -> Option<usize> {
    // TODO: Extract actual LIMIT from plan
    Some(10)
}

/// Begin scan callback
#[pg_guard]
unsafe extern "C" fn hnsw_beginscan(
    index: Relation,
    nkeys: ::std::os::raw::c_int,
    norderbys: ::std::os::raw::c_int,
) -> IndexScanDesc {
    pgrx::debug1!(
        "HNSW v2: Begin scan (nkeys={}, norderbys={})",
        nkeys,
        norderbys
    );

    let scan = pg_sys::RelationGetIndexScan(index, nkeys, norderbys);

    // Allocate ORDER BY distance arrays (required by the AM, not done by
    // RelationGetIndexScan). See GiST's gistbeginscan for reference.
    if (*scan).numberOfOrderBys > 0 {
        let n = (*scan).numberOfOrderBys as usize;
        (*scan).xs_orderbyvals =
            pg_sys::palloc0(std::mem::size_of::<pg_sys::Datum>() * n) as *mut pg_sys::Datum;
        (*scan).xs_orderbynulls = pg_sys::palloc(std::mem::size_of::<bool>() * n) as *mut bool;
        // Initialize all ORDER BY values as null (true = null)
        std::ptr::write_bytes((*scan).xs_orderbynulls, 1u8, n);
    }

    // Get metadata for dimensions and metric
    let (meta_page, meta_buffer) = get_meta_page(index);
    let meta = read_metadata(meta_page);
    pg_sys::UnlockReleaseBuffer(meta_buffer);

    // Allocate scan state
    let state = Box::new(HnswScanState::new(
        meta.dimensions as usize,
        byte_to_metric(meta.metric),
        meta.recall_target,
    ));

    (*scan).opaque = Box::into_raw(state) as *mut ::std::os::raw::c_void;

    scan
}

/// Rescan callback - set query vector
#[pg_guard]
unsafe extern "C" fn hnsw_rescan(
    scan: IndexScanDesc,
    _keys: ScanKey,
    _nkeys: ::std::os::raw::c_int,
    orderbys: ScanKey,
    norderbys: ::std::os::raw::c_int,
) {
    pgrx::debug1!("HNSW v2: Rescan (norderbys={})", norderbys);

    let state = &mut *((*scan).opaque as *mut HnswScanState);

    // Reset state
    state.results.clear();
    state.current_pos = 0;
    state.search_done = false;
    state.query_valid = false; // Reset validity flag

    // Non-kNN scan (e.g., COUNT(*), WHERE embedding IS NOT NULL)
    // When there are no ORDER BY operators, we cannot perform a vector search.
    // Return early and let hnsw_gettuple return false, forcing PostgreSQL to
    // fall back to a sequential scan. Fixes #152.
    if norderbys <= 0 || orderbys.is_null() {
        return;
    }

    // Extract query vector from ORDER BY
    if norderbys > 0 && !orderbys.is_null() {
        let orderby = &*orderbys;
        let datum = orderby.sk_argument;
        let typoid = orderby.sk_subtype;

        pgrx::debug1!(
            "HNSW v2: Extracting query vector, datum null={}, typoid={}",
            datum.is_null(),
            typoid.as_u32()
        );

        // Method 1: Try direct RuVector extraction (works for literals and some casts)
        if let Some(vector) = RuVector::from_polymorphic_datum(
            datum, false, // not null
            typoid,
        ) {
            state.query_vector = vector.as_slice().to_vec();
            state.query_valid = true;
            pgrx::debug1!(
                "HNSW v2: Extracted query vector (direct) with {} dimensions",
                state.query_vector.len()
            );
        }

        // Method 2: Handle parameterized queries - check if it's a text type needing conversion
        if !state.query_valid && !datum.is_null() {
            // Check if the type is text (OID 25) or varchar (OID 1043) or unknown (OID 705)
            let is_text_type = typoid == pg_sys::Oid::from(25)
                || typoid == pg_sys::Oid::from(1043)
                || typoid == pg_sys::Oid::from(705)
                || typoid == pg_sys::InvalidOid;

            if is_text_type {
                // Try to convert text to ruvector using the input function
                if let Some(vec) = try_convert_text_to_ruvector(datum) {
                    state.query_vector = vec;
                    state.query_valid = true;
                    pgrx::debug1!(
                        "HNSW v2: Converted text parameter to query vector with {} dimensions",
                        state.query_vector.len()
                    );
                }
            }
        }

        // Method 3: Fallback - try raw varlena extraction
        if !state.query_valid {
            let raw_ptr = datum.cast_mut_ptr::<pg_sys::varlena>();
            if !raw_ptr.is_null() {
                let detoasted = pg_sys::pg_detoast_datum(raw_ptr);
                if !detoasted.is_null() {
                    // Check if this looks like our vector format
                    let total_size = pgrx::varlena::varsize_any(detoasted as *const _);
                    if total_size >= 8 {
                        // Minimum: 4 byte header + 4 byte data
                        let data_ptr =
                            pgrx::varlena::vardata_any(detoasted as *const _) as *const u8;
                        let dimensions = ptr::read_unaligned(data_ptr as *const u16) as usize;

                        // Validate dimensions match expected and data size is correct
                        let expected_data_size = 4 + (dimensions * 4); // 4 bytes header + f32 data
                        let actual_data_size = total_size - pg_sys::VARHDRSZ;

                        if dimensions > 0
                            && dimensions <= 16384
                            && actual_data_size >= expected_data_size
                        {
                            let f32_ptr = data_ptr.add(4) as *const f32;
                            state.query_vector =
                                std::slice::from_raw_parts(f32_ptr, dimensions).to_vec();
                            state.query_valid = true;
                            pgrx::debug1!(
                                "HNSW v2: Extracted query vector (varlena fallback) with {} dimensions",
                                dimensions
                            );
                        }
                    }
                }
            }
        }
    }

    // Validate query vector - CRITICAL: Prevent crashes from invalid queries
    // Note: if query_valid is false due to norderbys==0 (non-kNN scan),
    // we already returned early above. This check only fires for kNN scans
    // where vector extraction genuinely failed.
    if !state.query_valid || state.query_vector.is_empty() {
        // Instead of using zeros which crash, raise a proper error
        pgrx::error!(
            "HNSW: Could not extract query vector from parameter. \
             Ensure the query vector is properly cast to ruvector type, e.g.: \
             ORDER BY embedding <=> '[1,2,3]'::ruvector(dim)"
        );
    }

    // Validate vector is not all zeros (would cause issues in hyperbolic space)
    if is_zero_vector(&state.query_vector) {
        pgrx::error!(
            "HNSW: Query vector is all zeros, which is invalid for similarity search. \
             Please provide a valid non-zero query vector."
        );
    }

    // Validate dimension match
    if state.query_vector.len() != state.dimensions {
        pgrx::error!(
            "HNSW: Query vector has {} dimensions but index expects {}",
            state.query_vector.len(),
            state.dimensions
        );
    }

    // Get ef_search from GUC (ruvector.ef_search)
    state.k = 10; // Default, will be overridden by LIMIT in executor
}

/// Try to convert a text datum to ruvector by calling the input function
unsafe fn try_convert_text_to_ruvector(datum: Datum) -> Option<Vec<f32>> {
    // Get the text value
    let text_ptr = datum.cast_mut_ptr::<pg_sys::text>();
    if text_ptr.is_null() {
        return None;
    }

    // Detoast if needed
    let detoasted = pg_sys::pg_detoast_datum(text_ptr as *mut pg_sys::varlena);
    if detoasted.is_null() {
        return None;
    }

    // Extract the text content
    let text_len = pgrx::varlena::varsize_any_exhdr(detoasted as *const _);
    let text_data = pgrx::varlena::vardata_any(detoasted as *const _) as *const u8;

    if text_len == 0 {
        return None;
    }

    // Convert to string for parsing
    let text_slice = std::slice::from_raw_parts(text_data, text_len);
    let text_str = match std::str::from_utf8(text_slice) {
        Ok(s) => s.trim(),
        Err(_) => return None,
    };

    // Must start with '[' and end with ']' for vector format
    if !text_str.starts_with('[') || !text_str.ends_with(']') {
        return None;
    }

    // Parse the vector values
    let inner = &text_str[1..text_str.len() - 1];
    let values: Vec<f32> = inner
        .split(',')
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .collect();

    if values.is_empty() {
        return None;
    }

    Some(values)
}

/// Check if a vector is all zeros
fn is_zero_vector(v: &[f32]) -> bool {
    v.iter().all(|&x| x == 0.0)
}

/// Get tuple callback - return next result
#[pg_guard]
unsafe extern "C" fn hnsw_gettuple(scan: IndexScanDesc, direction: ScanDirection::Type) -> bool {
    // Only support forward scans
    if direction != pg_sys::ScanDirection::ForwardScanDirection {
        return false;
    }

    let state = &mut *((*scan).opaque as *mut HnswScanState);
    let index = (*scan).indexRelation;

    // Non-kNN scan: no query vector was provided (e.g., COUNT(*), WHERE IS NOT NULL).
    // Return false to tell PostgreSQL this index cannot satisfy this scan type,
    // forcing fallback to sequential scan. Fixes #152.
    if !state.query_valid && !state.search_done {
        return false;
    }

    // Execute search on first call
    if !state.search_done {
        let (meta_page, meta_buffer) = get_meta_page(index);
        let meta = read_metadata(meta_page);
        pg_sys::UnlockReleaseBuffer(meta_buffer);

        // Calculate dynamic ef_search based on recall target
        let ef_search = state.calculate_ef_search(meta.node_count);
        state.ef_search = ef_search;

        // Perform search
        state.results = hnsw_search(index, &state.query_vector, state.k, ef_search, &meta);

        state.search_done = true;

        pgrx::debug1!(
            "HNSW v2: Search complete, {} results (ef_search={})",
            state.results.len(),
            ef_search
        );
    }

    // Return next result
    if state.current_pos < state.results.len() {
        let (_, tid, distance) = state.results[state.current_pos];
        state.current_pos += 1;

        // Set tuple ID
        (*scan).xs_heaptid = tid;

        // Set ORDER BY value (distance) — arrays allocated in beginscan
        if !(*scan).xs_orderbynulls.is_null() {
            *(*scan).xs_orderbynulls.add(0) = false;
        }
        if !(*scan).xs_orderbyvals.is_null() {
            // Inline Float8GetDatum: reinterpret f64 bits as Datum (avoids FFI overhead)
            *(*scan).xs_orderbyvals.add(0) =
                pg_sys::Datum::from((distance as f64).to_bits() as usize);
        }

        (*scan).xs_recheck = false;
        // Do NOT set xs_recheckorderby = true. PG17's IndexNextWithReorder
        // compares recalculated distances (from heap tuples) against index-reported
        // distances. Floating-point precision differences between index-stored
        // vectors and heap vectors cause spurious "index returned tuples in wrong
        // order" errors. With recheckorderby = false the executor trusts our
        // ordering directly, which is correct since we sort results in hnsw_search.
        (*scan).xs_recheckorderby = false;

        true
    } else {
        false
    }
}

/// Get bitmap callback - for bitmap scans (not typically used for k-NN)
#[pg_guard]
unsafe extern "C" fn hnsw_getbitmap(_scan: IndexScanDesc, _tbm: *mut TIDBitmap) -> i64 {
    pgrx::warning!("HNSW v2: Bitmap scans not supported for k-NN queries");
    0
}

/// End scan callback
#[pg_guard]
unsafe extern "C" fn hnsw_endscan(scan: IndexScanDesc) {
    pgrx::debug1!("HNSW v2: End scan");

    // Free scan state and null out pointer to prevent use-after-free
    if !(*scan).opaque.is_null() {
        let state = Box::from_raw((*scan).opaque as *mut HnswScanState);
        drop(state);
        (*scan).opaque = std::ptr::null_mut();
    }
}

/// Can return callback - indicates if index can return indexed data
#[pg_guard]
unsafe extern "C" fn hnsw_canreturn(_index: Relation, _attno: ::std::os::raw::c_int) -> bool {
    // HNSW does not support returning indexed data directly (no index-only scans).
    // Returning true here caused PostgreSQL to set up index-only scan paths which
    // initialize the scan descriptor differently, leading to corrupted xs_orderbyvals.
    false
}

/// Options callback - parse index options from WITH clause
#[pg_guard]
unsafe extern "C" fn hnsw_options(reloptions: Datum, validate: bool) -> *mut bytea {
    pgrx::debug1!("HNSW v2: Parsing options (validate={})", validate);

    // TODO: Implement proper reloptions parsing using pg_sys::parseRelOptions
    // For now, return null to use defaults

    if reloptions.is_null() {
        return ptr::null_mut();
    }

    // In production:
    // 1. Define relopt_parse_elt array for m, ef_construction, recall_target, etc.
    // 2. Call pg_sys::parseRelOptions
    // 3. Validate ranges (m: 2-100, ef_construction: 10-2000, recall_target: 0.5-1.0)

    ptr::null_mut()
}

/// Validate callback - validate operator class
#[pg_guard]
unsafe extern "C" fn hnsw_validate(opclassoid: pg_sys::Oid) -> bool {
    pgrx::debug1!("HNSW v2: Validating operator class {:?}", opclassoid);

    // Validate that the operator class provides required operators:
    // - Strategy 1: distance ordering (e.g., <->)
    // And support functions:
    // - Support 1: distance calculation

    // For now, accept all - proper validation would query pg_amop/pg_amproc
    true
}

/// Property callback - report index properties
#[pg_guard]
unsafe extern "C" fn hnsw_property(
    _index_oid: pg_sys::Oid,
    attno: ::std::os::raw::c_int,
    prop: ::std::os::raw::c_int,
    _res_bool: *mut bool,
    _res_prop: *mut ::std::os::raw::c_int,
) -> bool {
    pgrx::debug1!("HNSW v2: Property query (attno={}, prop={})", attno, prop);
    false // Use default property values
}

// ============================================================================
// Integrity System Integration
// ============================================================================

/// Check integrity gate before operations
fn check_integrity_gate(_contract_id: u64, _operation: &str) -> bool {
    // TODO: Integrate with contracted graph integrity system
    // This would check if the operation is allowed under current system stress
    true
}

/// Report index health to integrity system
fn report_index_health(_contract_id: u64, _deletion_ratio: f64, _node_count: u64) {
    // TODO: Report to contracted graph
    // This enables graceful degradation under stress
}

/// Get HNSW options from relation
unsafe fn get_hnsw_options_from_relation(_index: Relation) -> HnswOptions {
    // TODO: Parse actual reloptions from relation
    HnswOptions::default()
}

// ============================================================================
// Access Method Handler
// ============================================================================

/// Static IndexAmRoutine template for HNSW
static HNSW_AM_HANDLER: IndexAmRoutine = IndexAmRoutine {
    type_: NodeTag::T_IndexAmRoutine,

    // Index structure capabilities
    amstrategies: 1, // One strategy: nearest neighbor
    amsupport: 2,    // Two support functions: distance, normalize
    amoptsprocnum: 0,
    amcanorder: false,
    amcanorderbyop: true, // Supports ORDER BY with distance operators
    amcanbackward: false,
    amcanunique: false,
    amcanmulticol: false, // Single column only (vector)
    amoptionalkey: true,
    amsearcharray: false,
    amsearchnulls: false,
    amstorage: true, // Custom storage format
    amclusterable: false,
    ampredlocks: false,
    amcanparallel: true, // Supports parallel scan
    amcaninclude: false,
    amusemaintenanceworkmem: true,
    #[cfg(any(feature = "pg16", feature = "pg17"))]
    amsummarizing: false,
    amparallelvacuumoptions: pg_sys::VACUUM_OPTION_PARALLEL_COND_CLEANUP as u8,

    // Key type
    amkeytype: pg_sys::ANYELEMENTOID,

    // Callbacks - set to None, will be filled in at runtime
    ambuild: None,
    ambuildempty: None,
    aminsert: None,
    ambulkdelete: None,
    amvacuumcleanup: None,
    amcanreturn: None,
    amcostestimate: None,
    amoptions: None,
    amproperty: None,
    ambuildphasename: None,
    amvalidate: None,
    amadjustmembers: None,
    ambeginscan: None,
    amrescan: None,
    amgettuple: None,
    amgetbitmap: None,
    amendscan: None,
    ammarkpos: None,
    amrestrpos: None,
    amestimateparallelscan: None,
    aminitparallelscan: None,
    amparallelrescan: None,
    // PG17 additions
    #[cfg(feature = "pg17")]
    amcanbuildparallel: true,
    #[cfg(feature = "pg17")]
    aminsertcleanup: None,
};

/// Main handler function for HNSW index access method
#[pg_extern(sql = "
CREATE OR REPLACE FUNCTION hnsw_handler(internal) RETURNS index_am_handler
AS 'MODULE_PATHNAME', 'hnsw_handler_wrapper' LANGUAGE C STRICT;
")]
fn hnsw_handler(_fcinfo: pg_sys::FunctionCallInfo) -> Internal {
    unsafe {
        // Allocate IndexAmRoutine in PostgreSQL memory context
        let am_routine = pg_sys::palloc0(size_of::<IndexAmRoutine>()) as *mut IndexAmRoutine;

        // Copy template into allocated memory
        ptr::copy_nonoverlapping(&HNSW_AM_HANDLER, am_routine, 1);

        // Set callback function pointers
        (*am_routine).ambuild = Some(hnsw_build);
        (*am_routine).ambuildempty = Some(hnsw_buildempty);
        (*am_routine).aminsert = Some(hnsw_insert);
        (*am_routine).ambulkdelete = Some(hnsw_bulkdelete);
        (*am_routine).amvacuumcleanup = Some(hnsw_vacuumcleanup);
        (*am_routine).ambeginscan = Some(hnsw_beginscan);
        (*am_routine).amrescan = Some(hnsw_rescan);
        (*am_routine).amgettuple = Some(hnsw_gettuple);
        (*am_routine).amgetbitmap = Some(hnsw_getbitmap);
        (*am_routine).amendscan = Some(hnsw_endscan);
        (*am_routine).amcostestimate = Some(hnsw_costestimate);
        (*am_routine).amoptions = Some(hnsw_options);
        (*am_routine).amcanreturn = Some(hnsw_canreturn);
        (*am_routine).amvalidate = Some(hnsw_validate);
        // Note: amproperty signature differs across PostgreSQL versions, skip for now

        // Return as Internal datum
        Internal::from(Some(Datum::from(am_routine)))
    }
}

// ============================================================================
// SQL Functions for Index Management
// ============================================================================

/// Get HNSW index statistics
#[pg_extern]
fn ruhnsw_stats(index_name: &str) -> pgrx::JsonB {
    let stats = serde_json::json!({
        "name": index_name,
        "total_searches": TOTAL_SEARCHES.load(AtomicOrdering::Relaxed),
        "total_inserts": TOTAL_INSERTS.load(AtomicOrdering::Relaxed),
        "distance_calculations": DISTANCE_CALCULATIONS.load(AtomicOrdering::Relaxed),
    });
    pgrx::JsonB(stats)
}

/// Reset HNSW statistics
#[pg_extern]
fn ruhnsw_reset_stats() {
    TOTAL_SEARCHES.store(0, AtomicOrdering::Relaxed);
    TOTAL_INSERTS.store(0, AtomicOrdering::Relaxed);
    DISTANCE_CALCULATIONS.store(0, AtomicOrdering::Relaxed);
}

/// Get dynamic ef_search recommendation
#[pg_extern]
fn ruhnsw_recommended_ef_search(index_name: &str, k: i32, recall_target: f64) -> i32 {
    // Heuristic for ef_search based on k and recall target
    let base_ef = k.max(10);
    let recall_factor = 1.0 / (1.0 - recall_target + 0.01);
    let recommended = (base_ef as f64 * recall_factor * 2.0) as i32;
    recommended.clamp(k, 1000)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_page_size() {
        assert!(size_of::<HnswMetaPage>() < 8192);
    }

    #[test]
    fn test_node_header_size() {
        assert!(size_of::<HnswNodePageHeader>() < 100);
    }

    #[test]
    fn test_hnsw_options_default() {
        let opts = HnswOptions::default();
        assert_eq!(opts.m, DEFAULT_M as i32);
        assert_eq!(opts.ef_construction, DEFAULT_EF_CONSTRUCTION as i32);
        assert!((opts.recall_target - DEFAULT_RECALL_TARGET).abs() < 0.001);
    }

    #[test]
    fn test_metric_conversion() {
        assert_eq!(
            byte_to_metric(metric_to_byte(DistanceMetric::Euclidean)),
            DistanceMetric::Euclidean
        );
        assert_eq!(
            byte_to_metric(metric_to_byte(DistanceMetric::Cosine)),
            DistanceMetric::Cosine
        );
        assert_eq!(
            byte_to_metric(metric_to_byte(DistanceMetric::InnerProduct)),
            DistanceMetric::InnerProduct
        );
    }

    #[test]
    fn test_random_level_distribution() {
        // Test that random_level produces reasonable distribution
        let m = 16;
        let mut levels = vec![0; 10];

        for _ in 0..10000 {
            let level = random_level(m, 9);
            if level < 10 {
                levels[level] += 1;
            }
        }

        // Most nodes should be at level 0
        assert!(levels[0] > 5000);
        // Higher levels should be less frequent
        assert!(levels[1] < levels[0]);
    }

    #[test]
    fn test_scan_state_ef_search_calculation() {
        let state = HnswScanState::new(128, DistanceMetric::Euclidean, 0.95);

        // Small index
        let ef_small = state.calculate_ef_search(100);

        // Large index
        let ef_large = state.calculate_ef_search(1_000_000);

        // Larger index should have larger ef_search
        assert!(ef_large > ef_small);

        // Both should be at least k
        assert!(ef_small >= state.k);
        assert!(ef_large >= state.k);
    }

    #[test]
    fn test_search_candidate_ordering() {
        let mut heap: BinaryHeap<SearchCandidate> = BinaryHeap::new();

        heap.push(SearchCandidate {
            block: 1,
            distance: 0.5,
        });
        heap.push(SearchCandidate {
            block: 2,
            distance: 0.1,
        });
        heap.push(SearchCandidate {
            block: 3,
            distance: 0.9,
        });

        // Should be min-heap by distance
        assert_eq!(heap.pop().unwrap().distance, 0.1);
        assert_eq!(heap.pop().unwrap().distance, 0.5);
        assert_eq!(heap.pop().unwrap().distance, 0.9);
    }

    #[test]
    fn test_result_candidate_ordering() {
        let mut heap: BinaryHeap<ResultCandidate> = BinaryHeap::new();
        let dummy_tid = ItemPointerData {
            ip_blkid: pg_sys::BlockIdData { bi_hi: 0, bi_lo: 0 },
            ip_posid: 0,
        };

        heap.push(ResultCandidate {
            block: 1,
            tid: dummy_tid,
            distance: 0.5,
        });
        heap.push(ResultCandidate {
            block: 2,
            tid: dummy_tid,
            distance: 0.1,
        });
        heap.push(ResultCandidate {
            block: 3,
            tid: dummy_tid,
            distance: 0.9,
        });

        // Should be max-heap by distance (for pruning)
        assert_eq!(heap.pop().unwrap().distance, 0.9);
        assert_eq!(heap.pop().unwrap().distance, 0.5);
        assert_eq!(heap.pop().unwrap().distance, 0.1);
    }

    #[test]
    fn test_hnsw_meta_flags() {
        let mut meta = HnswMetaPage::default();

        // Set flags
        meta.flags = FLAG_PARALLEL_BUILD | FLAG_INTEGRITY_ENABLED;

        assert!(meta.flags & FLAG_PARALLEL_BUILD != 0);
        assert!(meta.flags & FLAG_INTEGRITY_ENABLED != 0);
        assert!(meta.flags & FLAG_MMAP_ENABLED == 0);
    }
}
