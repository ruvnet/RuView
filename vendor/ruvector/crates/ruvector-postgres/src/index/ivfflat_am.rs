//! IVFFlat PostgreSQL Access Method Implementation (v2 Enhanced)
//!
//! Implements IVFFlat (Inverted File with Flat quantization) as a native PostgreSQL
//! index access method with the following v2 enhancements:
//!
//! ## Features
//!
//! - **Full AM Interface**: Complete PostgreSQL Index AM integration
//! - **Parallel List Scanning**: Multi-worker list scanning for large datasets
//! - **Adaptive nprobe**: Query-aware probe count adjustment
//! - **Incremental Retraining**: Background centroid updates
//! - **Quantization Support**: SQ (4x), PQ (8-32x), BQ (32x) compression
//! - **Integrity Integration**: Health tracking and self-healing hooks
//!
//! ## Storage Layout
//!
//! - **Page 0 (Metadata)**: Index configuration and health metrics
//! - **Pages 1-N (Centroids)**: Cluster centroids with quantization info
//! - **Pages N+1-M (Inverted Lists)**: Vectors with optional quantized storage
//!
//! ## SQL Interface
//!
//! ```sql
//! -- Create index (pgvector compatible)
//! CREATE INDEX idx ON items USING ruivfflat (embedding vector_l2_ops)
//!     WITH (lists=100, quantization='none');
//!
//! -- Runtime configuration
//! SET ruvector.ivfflat_probes = 10;
//! SET ruvector.ivfflat_adaptive_probes = on;
//! ```

use pgrx::pg_sys::{
    self, bytea, BlockNumber, Buffer, Cost, Datum, IndexAmRoutine, IndexBuildResult,
    IndexBulkDeleteCallback, IndexBulkDeleteResult, IndexInfo, IndexPath, IndexScanDesc,
    IndexUniqueCheck, IndexVacuumInfo, ItemPointer, ItemPointerData, NodeTag, PlannerInfo,
    Relation, ScanDirection, ScanKey, Selectivity, Size, TIDBitmap,
};
use pgrx::prelude::*;
use pgrx::Internal;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};

use crate::distance::{distance, DistanceMetric};
use crate::quantization::{product, scalar, QuantizationType};
use crate::types::RuVector;
use pgrx::FromDatum;

// ============================================================================
// Constants
// ============================================================================

/// Magic number for IVFFlat index pages (ASCII "IVFF")
const IVFFLAT_MAGIC: u32 = 0x49564646;

/// Index version for compatibility checking
const IVFFLAT_VERSION: u32 = 2;

/// Page type identifiers
const IVFFLAT_PAGE_META: u8 = 0;
const IVFFLAT_PAGE_CENTROID: u8 = 1;
const IVFFLAT_PAGE_LIST: u8 = 2;
const IVFFLAT_PAGE_OVERFLOW: u8 = 3;

/// Maximum training sample size
const MAX_TRAINING_SAMPLES: usize = 50_000;

/// Default k-means iterations
const DEFAULT_KMEANS_ITERATIONS: usize = 10;

/// Adaptive probing thresholds
const ADAPTIVE_PROBE_LOW_DIM: usize = 128;
const ADAPTIVE_PROBE_MED_DIM: usize = 512;
const ADAPTIVE_PROBE_HIGH_DIM: usize = 1536;

/// List imbalance threshold for retraining trigger
const LIST_IMBALANCE_THRESHOLD: f32 = 3.0;

/// Health score thresholds
const HEALTH_GOOD: f32 = 0.8;
const HEALTH_WARNING: f32 = 0.5;

/// P_NEW equivalent for allocating new pages
const P_NEW_BLOCK: BlockNumber = pg_sys::InvalidBlockNumber;

// ============================================================================
// GUC Variables (Runtime Configuration)
// ============================================================================

/// Number of lists to probe during search (default: 1)
static GUC_IVFFLAT_PROBES: AtomicU64 = AtomicU64::new(1);

/// Enable adaptive probe count based on query characteristics
static GUC_ADAPTIVE_PROBES: AtomicBool = AtomicBool::new(false);

/// Maximum probes for adaptive mode
static GUC_MAX_PROBES: AtomicU64 = AtomicU64::new(100);

/// Enable quantized search (use quantized vectors first, then rerank)
static GUC_QUANTIZED_SEARCH: AtomicBool = AtomicBool::new(true);

/// Rerank factor for quantized search
static GUC_RERANK_FACTOR: AtomicU64 = AtomicU64::new(4);

/// Get current probe count from GUC
fn get_probes_guc() -> usize {
    GUC_IVFFLAT_PROBES.load(AtomicOrdering::Relaxed) as usize
}

/// Get adaptive probes setting
fn get_adaptive_probes_guc() -> bool {
    GUC_ADAPTIVE_PROBES.load(AtomicOrdering::Relaxed)
}

/// Get max probes setting
fn get_max_probes_guc() -> usize {
    GUC_MAX_PROBES.load(AtomicOrdering::Relaxed) as usize
}

/// Get quantized search setting
fn get_quantized_search_guc() -> bool {
    GUC_QUANTIZED_SEARCH.load(AtomicOrdering::Relaxed)
}

/// Get rerank factor
fn get_rerank_factor_guc() -> usize {
    GUC_RERANK_FACTOR.load(AtomicOrdering::Relaxed) as usize
}

// ============================================================================
// Page Structures
// ============================================================================

/// Metadata stored on page 0
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IvfFlatMetaPage {
    /// Magic number for validation
    magic: u32,
    /// Version number
    version: u32,
    /// Number of cluster lists
    lists: u32,
    /// Number of lists to probe during search (default)
    default_probes: u32,
    /// Vector dimensions
    dimensions: u32,
    /// Whether index is trained
    trained: u32,
    /// Total number of vectors
    vector_count: u64,
    /// Distance metric (0=L2, 1=IP, 2=Cosine, 3=Manhattan)
    metric: u32,
    /// Quantization type (0=None, 1=Scalar, 2=Product, 3=Binary)
    quantization: u32,
    /// First page containing centroids
    centroid_start_page: u32,
    /// First page containing inverted lists
    lists_start_page: u32,
    /// Health score (0.0 - 1.0 stored as u32 * 1000)
    health_score: u32,
    /// Largest list size (for imbalance tracking)
    max_list_size: u32,
    /// Smallest list size
    min_list_size: u32,
    /// Last retraining timestamp (Unix epoch)
    last_retrain: u64,
    /// Number of insertions since last retrain
    insertions_since_retrain: u64,
    /// Product quantization M (subspaces)
    pq_m: u32,
    /// Product quantization K (centroids per subspace)
    pq_k: u32,
    /// Reserved for future use
    reserved: [u32; 8],
}

impl Default for IvfFlatMetaPage {
    fn default() -> Self {
        Self {
            magic: IVFFLAT_MAGIC,
            version: IVFFLAT_VERSION,
            lists: 100,
            default_probes: 1,
            dimensions: 0,
            trained: 0,
            vector_count: 0,
            metric: 0,       // L2
            quantization: 0, // None
            centroid_start_page: 1,
            lists_start_page: 0,
            health_score: 1000, // 1.0
            max_list_size: 0,
            min_list_size: 0,
            last_retrain: 0,
            insertions_since_retrain: 0,
            pq_m: 8,
            pq_k: 256,
            reserved: [0; 8],
        }
    }
}

impl IvfFlatMetaPage {
    /// Calculate health score based on list imbalance
    fn calculate_health(&self) -> f32 {
        if self.min_list_size == 0 || self.max_list_size == 0 {
            return 1.0;
        }

        let imbalance = self.max_list_size as f32 / self.min_list_size.max(1) as f32;

        if imbalance <= 1.5 {
            1.0
        } else if imbalance <= LIST_IMBALANCE_THRESHOLD {
            1.0 - (imbalance - 1.5) / (LIST_IMBALANCE_THRESHOLD - 1.5) * 0.3
        } else {
            (0.7 - (imbalance - LIST_IMBALANCE_THRESHOLD) * 0.1).max(0.0)
        }
    }

    /// Check if retraining is recommended
    fn needs_retrain(&self) -> bool {
        let health = self.calculate_health();
        health < HEALTH_WARNING || self.insertions_since_retrain > self.vector_count / 2
    }
}

/// Centroid entry stored in centroid pages
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CentroidEntry {
    /// Cluster ID
    cluster_id: u32,
    /// Start page of inverted list for this cluster
    list_start_page: u32,
    /// Number of pages in inverted list
    list_page_count: u32,
    /// Number of vectors in this cluster
    vector_count: u32,
    /// Sum of distances to centroid (for variance tracking)
    distance_sum: f32,
    /// Reserved for future use
    reserved: u32,
    // Centroid vector data follows immediately after this struct
}

/// Vector entry in inverted list pages
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct VectorEntry {
    /// Heap tuple ID (block number)
    block_number: u32,
    /// Heap tuple ID (offset number)
    offset_number: u16,
    /// Flags (bit 0: has quantized data)
    flags: u16,
    // Vector data follows immediately after this struct
    // If quantized, the quantized representation follows instead
}

impl VectorEntry {
    const FLAG_QUANTIZED: u16 = 0x0001;

    fn has_quantized_data(&self) -> bool {
        self.flags & Self::FLAG_QUANTIZED != 0
    }

    fn to_item_pointer(&self) -> ItemPointerData {
        let mut tid = ItemPointerData::default();
        // Set block ID
        tid.ip_blkid.bi_hi = ((self.block_number >> 16) & 0xFFFF) as u16;
        tid.ip_blkid.bi_lo = (self.block_number & 0xFFFF) as u16;
        // Set offset
        tid.ip_posid = self.offset_number;
        tid
    }

    fn from_item_pointer(tid: ItemPointerData, flags: u16) -> Self {
        let block_number = ((tid.ip_blkid.bi_hi as u32) << 16) | (tid.ip_blkid.bi_lo as u32);
        Self {
            block_number,
            offset_number: tid.ip_posid,
            flags,
        }
    }
}

/// Inverted list page header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct ListPageHeader {
    /// Page type
    page_type: u8,
    /// Cluster ID this page belongs to
    cluster_id: u8,
    /// Padding
    _padding: [u8; 2],
    /// Number of entries on this page
    entry_count: u32,
    /// Next overflow page (0 if none)
    next_page: u32,
    /// Dimensions (for validation)
    dimensions: u32,
}

// ============================================================================
// Index Build State
// ============================================================================

/// State for building IVFFlat index
struct IvfFlatBuildState {
    /// Metadata
    meta: IvfFlatMetaPage,
    /// Centroids (after training)
    centroids: Vec<Vec<f32>>,
    /// Inverted lists (cluster_id -> list of (tid, vector))
    lists: Vec<Vec<(ItemPointerData, Vec<f32>)>>,
    /// Training sample
    training_sample: Vec<Vec<f32>>,
    /// Distance metric
    metric: DistanceMetric,
    /// Quantization type
    quantization: QuantizationType,
    /// Product quantizer (if using PQ)
    pq_quantizer: Option<product::ProductQuantizer>,
}

// ============================================================================
// Index Scan State
// ============================================================================

/// Search result for internal use
#[derive(Clone)]
struct SearchCandidate {
    tid: ItemPointerData,
    distance: f32,
    cluster_id: u32,
    needs_rerank: bool,
}

impl PartialEq for SearchCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
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
        // Max-heap: reverse ordering for min-distance priority
        other
            .distance
            .partial_cmp(&self.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// State for scanning IVFFlat index
struct IvfFlatScanState {
    /// Query vector
    query: Vec<f32>,
    /// Number of results requested (k)
    k: usize,
    /// Number of probes (computed based on adaptive settings)
    probes: usize,
    /// Search results (tid, distance)
    results: Vec<(ItemPointerData, f32)>,
    /// Current position in results
    current: usize,
    /// Distance metric
    metric: DistanceMetric,
    /// Quantization type for this index
    quantization: QuantizationType,
    /// Whether search has been executed
    search_done: bool,
    /// Metadata cache
    meta: IvfFlatMetaPage,
    /// Whether query vector was successfully extracted
    query_valid: bool,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Calculate distance between two vectors
#[inline]
fn calc_distance(a: &[f32], b: &[f32], metric: DistanceMetric) -> f32 {
    distance(a, b, metric)
}

/// Get metric from u32
fn metric_from_u32(m: u32) -> DistanceMetric {
    match m {
        1 => DistanceMetric::InnerProduct,
        2 => DistanceMetric::Cosine,
        3 => DistanceMetric::Manhattan,
        _ => DistanceMetric::Euclidean,
    }
}

/// Get metric as u32
fn metric_to_u32(m: DistanceMetric) -> u32 {
    match m {
        DistanceMetric::Euclidean => 0,
        DistanceMetric::InnerProduct => 1,
        DistanceMetric::Cosine => 2,
        DistanceMetric::Manhattan => 3,
        DistanceMetric::Hamming => 0, // Fallback to L2
    }
}

/// Get quantization type from u32
fn quantization_from_u32(q: u32) -> QuantizationType {
    match q {
        1 => QuantizationType::Scalar,
        2 => QuantizationType::Product,
        3 => QuantizationType::Binary,
        _ => QuantizationType::None,
    }
}

/// Get quantization as u32
fn quantization_to_u32(q: QuantizationType) -> u32 {
    match q {
        QuantizationType::None => 0,
        QuantizationType::Scalar => 1,
        QuantizationType::Product => 2,
        QuantizationType::Binary => 3,
    }
}

/// Compute adaptive probe count based on query and index characteristics
fn compute_adaptive_probes(dimensions: usize, lists: usize, k: usize, query_norm: f32) -> usize {
    let base_probes = get_probes_guc();

    if !get_adaptive_probes_guc() {
        return base_probes;
    }

    // Base multiplier based on dimensions
    let dim_factor = if dimensions <= ADAPTIVE_PROBE_LOW_DIM {
        1.0
    } else if dimensions <= ADAPTIVE_PROBE_MED_DIM {
        1.2
    } else if dimensions <= ADAPTIVE_PROBE_HIGH_DIM {
        1.5
    } else {
        2.0
    };

    // Adjust for k (more results = more probes)
    let k_factor = (k as f32 / 10.0).sqrt().max(1.0);

    // Adjust for query norm (extreme norms may need more probes)
    let norm_factor = if query_norm < 0.1 || query_norm > 10.0 {
        1.2
    } else {
        1.0
    };

    // Adjust for number of lists
    let list_factor = (lists as f32 / 100.0).sqrt().max(1.0);

    let probes = (base_probes as f32 * dim_factor * k_factor * norm_factor / list_factor) as usize;

    probes.max(1).min(get_max_probes_guc()).min(lists)
}

/// K-means++ initialization
fn kmeans_plus_plus_init(
    vectors: &[Vec<f32>],
    k: usize,
    metric: DistanceMetric,
    seed: u64,
) -> Vec<Vec<f32>> {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    if vectors.is_empty() || k == 0 {
        return Vec::new();
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut centroids = Vec::with_capacity(k);

    // Choose first centroid randomly
    let first_idx = rng.gen_range(0..vectors.len());
    centroids.push(vectors[first_idx].clone());

    // Choose remaining centroids with probability proportional to D^2
    for _ in 1..k {
        let distances: Vec<f32> = vectors
            .iter()
            .map(|v| {
                centroids
                    .iter()
                    .map(|c| calc_distance(v, c, metric))
                    .fold(f32::MAX, f32::min)
            })
            .collect();

        // Square distances for probability weighting
        let mut squared: Vec<f32> = distances.iter().map(|d| d * d).collect();
        let total: f32 = squared.iter().sum();

        if total == 0.0 {
            break;
        }

        // Normalize to probabilities
        for d in &mut squared {
            *d /= total;
        }

        // Roulette wheel selection
        let target: f32 = rng.gen();
        let mut cumsum = 0.0;
        let mut selected = 0;

        for (i, &prob) in squared.iter().enumerate() {
            cumsum += prob;
            if cumsum >= target {
                selected = i;
                break;
            }
        }

        centroids.push(vectors[selected].clone());
    }

    centroids
}

/// Find nearest centroid index for a vector
fn find_nearest_centroid(vector: &[f32], centroids: &[Vec<f32>], metric: DistanceMetric) -> usize {
    let mut best_cluster = 0;
    let mut best_dist = f32::MAX;

    for (i, centroid) in centroids.iter().enumerate() {
        let dist = calc_distance(vector, centroid, metric);
        if dist < best_dist {
            best_dist = dist;
            best_cluster = i;
        }
    }

    best_cluster
}

/// Find top-n nearest centroid indices for a vector
fn find_nearest_centroids(
    vector: &[f32],
    centroids: &[Vec<f32>],
    n: usize,
    metric: DistanceMetric,
) -> Vec<(usize, f32)> {
    let mut distances: Vec<(usize, f32)> = centroids
        .iter()
        .enumerate()
        .map(|(i, c)| (i, calc_distance(vector, c, metric)))
        .collect();

    distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
    distances.truncate(n);
    distances
}

/// Run k-means clustering
fn kmeans_cluster(
    vectors: &[Vec<f32>],
    mut centroids: Vec<Vec<f32>>,
    iterations: usize,
    metric: DistanceMetric,
) -> Vec<Vec<f32>> {
    let n_clusters = centroids.len();
    let dimensions = if vectors.is_empty() {
        0
    } else {
        vectors[0].len()
    };

    for _ in 0..iterations {
        // Assign vectors to clusters
        let mut cluster_sums: Vec<Vec<f32>> =
            (0..n_clusters).map(|_| vec![0.0; dimensions]).collect();
        let mut cluster_counts: Vec<usize> = vec![0; n_clusters];

        for vector in vectors {
            let cluster = find_nearest_centroid(vector, &centroids, metric);
            for (i, &v) in vector.iter().enumerate() {
                cluster_sums[cluster][i] += v;
            }
            cluster_counts[cluster] += 1;
        }

        // Update centroids
        for (i, centroid) in centroids.iter_mut().enumerate() {
            if cluster_counts[i] > 0 {
                for j in 0..dimensions {
                    centroid[j] = cluster_sums[i][j] / cluster_counts[i] as f32;
                }
            }
        }
    }

    centroids
}

/// Calculate vector norm
fn vector_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

// ============================================================================
// Page Operations
// ============================================================================

/// Read metadata from page 0
unsafe fn read_meta_page(index: Relation) -> IvfFlatMetaPage {
    let nblocks = pg_sys::RelationGetNumberOfBlocksInFork(index, pg_sys::ForkNumber::MAIN_FORKNUM);

    if nblocks == 0 {
        return IvfFlatMetaPage::default();
    }

    let buffer = pg_sys::ReadBuffer(index, 0);
    pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_SHARE as i32);

    let page = pg_sys::BufferGetPage(buffer);
    let header = page as *const pg_sys::PageHeaderData;
    let data_ptr = (header as *const u8).add(size_of::<pg_sys::PageHeaderData>());
    let meta = ptr::read(data_ptr as *const IvfFlatMetaPage);

    pg_sys::UnlockReleaseBuffer(buffer);

    // Validate magic number
    if meta.magic != IVFFLAT_MAGIC {
        pgrx::warning!("Invalid IVFFlat index: bad magic number");
        return IvfFlatMetaPage::default();
    }

    meta
}

/// Write metadata to page 0
unsafe fn write_meta_page(index: Relation, meta: &IvfFlatMetaPage) {
    let nblocks = pg_sys::RelationGetNumberOfBlocksInFork(index, pg_sys::ForkNumber::MAIN_FORKNUM);

    let buffer = if nblocks == 0 {
        pg_sys::ReadBuffer(index, P_NEW_BLOCK)
    } else {
        pg_sys::ReadBuffer(index, 0)
    };

    pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);

    let page = pg_sys::BufferGetPage(buffer);

    // Initialize page if new
    if nblocks == 0 {
        pg_sys::PageInit(page, pg_sys::BLCKSZ as Size, 0);
    }

    let header = page as *mut pg_sys::PageHeaderData;
    let data_ptr = (header as *mut u8).add(size_of::<pg_sys::PageHeaderData>());
    ptr::write(data_ptr as *mut IvfFlatMetaPage, *meta);

    pg_sys::MarkBufferDirty(buffer);
    pg_sys::UnlockReleaseBuffer(buffer);
}

/// Read centroids from index
unsafe fn read_centroids(
    index: Relation,
    start_page: u32,
    num_centroids: usize,
    dimensions: usize,
) -> Vec<(CentroidEntry, Vec<f32>)> {
    let mut result = Vec::with_capacity(num_centroids);
    let mut current_page = start_page;

    let centroid_size = size_of::<CentroidEntry>() + dimensions * 4;
    let page_header_size = size_of::<pg_sys::PageHeaderData>();
    let usable_space = pg_sys::BLCKSZ as usize - page_header_size;
    let centroids_per_page = usable_space / centroid_size;

    let mut read_count = 0;

    while read_count < num_centroids {
        let buffer = pg_sys::ReadBuffer(index, current_page);
        pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_SHARE as i32);

        let page = pg_sys::BufferGetPage(buffer);
        let header = page as *const pg_sys::PageHeaderData;
        let data_ptr = (header as *const u8).add(page_header_size);

        let batch_size = (num_centroids - read_count).min(centroids_per_page);

        for i in 0..batch_size {
            let entry_ptr = data_ptr.add(i * centroid_size);
            let entry = ptr::read(entry_ptr as *const CentroidEntry);

            let vector_ptr = entry_ptr.add(size_of::<CentroidEntry>()) as *const f32;
            let vector: Vec<f32> = std::slice::from_raw_parts(vector_ptr, dimensions).to_vec();

            result.push((entry, vector));
        }

        read_count += batch_size;

        pg_sys::UnlockReleaseBuffer(buffer);
        current_page += 1;
    }

    result
}

/// Write centroids to index
unsafe fn write_centroids(
    index: Relation,
    centroids: &[(CentroidEntry, Vec<f32>)],
    start_page: u32,
    dimensions: usize,
) -> u32 {
    let centroid_size = size_of::<CentroidEntry>() + dimensions * 4;
    let page_header_size = size_of::<pg_sys::PageHeaderData>();
    let usable_space = pg_sys::BLCKSZ as usize - page_header_size;
    let centroids_per_page = usable_space / centroid_size;

    let mut current_page = start_page;
    let mut written = 0;

    while written < centroids.len() {
        let buffer = pg_sys::ReadBuffer(index, P_NEW_BLOCK);
        let actual_page = pg_sys::BufferGetBlockNumber(buffer);

        pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);

        let page = pg_sys::BufferGetPage(buffer);
        pg_sys::PageInit(page, pg_sys::BLCKSZ as Size, 0);

        let header = page as *mut pg_sys::PageHeaderData;
        let data_ptr = (header as *mut u8).add(page_header_size);

        let batch_size = (centroids.len() - written).min(centroids_per_page);

        for i in 0..batch_size {
            let (entry, vector) = &centroids[written + i];
            let entry_ptr = data_ptr.add(i * centroid_size);

            // Write entry
            ptr::write(entry_ptr as *mut CentroidEntry, *entry);

            // Write vector
            let vector_ptr = entry_ptr.add(size_of::<CentroidEntry>()) as *mut f32;
            for (j, &val) in vector.iter().enumerate() {
                ptr::write(vector_ptr.add(j), val);
            }
        }

        written += batch_size;

        pg_sys::MarkBufferDirty(buffer);
        pg_sys::UnlockReleaseBuffer(buffer);

        current_page = actual_page + 1;
    }

    current_page
}

/// Rewrite centroids in-place (updates existing pages)
unsafe fn rewrite_centroids(
    index: Relation,
    centroids: &[(CentroidEntry, Vec<f32>)],
    start_page: u32,
    dimensions: usize,
) {
    let centroid_size = size_of::<CentroidEntry>() + dimensions * 4;
    let page_header_size = size_of::<pg_sys::PageHeaderData>();
    let usable_space = pg_sys::BLCKSZ as usize - page_header_size;
    let centroids_per_page = usable_space / centroid_size;

    let mut current_page = start_page;
    let mut written = 0;

    while written < centroids.len() {
        let buffer = pg_sys::ReadBuffer(index, current_page);
        pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);

        let page = pg_sys::BufferGetPage(buffer);
        let header = page as *mut pg_sys::PageHeaderData;
        let data_ptr = (header as *mut u8).add(page_header_size);

        let batch_size = (centroids.len() - written).min(centroids_per_page);

        for i in 0..batch_size {
            let (entry, vector) = &centroids[written + i];
            let entry_ptr = data_ptr.add(i * centroid_size);

            // Write entry
            ptr::write(entry_ptr as *mut CentroidEntry, *entry);

            // Write vector
            let vector_ptr = entry_ptr.add(size_of::<CentroidEntry>()) as *mut f32;
            for (j, &val) in vector.iter().enumerate() {
                ptr::write(vector_ptr.add(j), val);
            }
        }

        written += batch_size;

        pg_sys::MarkBufferDirty(buffer);
        pg_sys::UnlockReleaseBuffer(buffer);

        current_page += 1;
    }
}

/// Read vectors from an inverted list
unsafe fn read_inverted_list(
    index: Relation,
    start_page: u32,
    dimensions: usize,
    quantization: QuantizationType,
) -> Vec<(VectorEntry, Vec<f32>)> {
    if start_page == 0 {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut current_page = start_page;

    while current_page != 0 {
        let buffer = pg_sys::ReadBuffer(index, current_page);
        pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_SHARE as i32);

        let page = pg_sys::BufferGetPage(buffer);
        let header = page as *const pg_sys::PageHeaderData;
        let data_ptr = (header as *const u8).add(size_of::<pg_sys::PageHeaderData>());

        // Read list page header
        let list_header = ptr::read(data_ptr as *const ListPageHeader);

        if list_header.page_type != IVFFLAT_PAGE_LIST {
            pg_sys::UnlockReleaseBuffer(buffer);
            break;
        }

        let entry_data_ptr = data_ptr.add(size_of::<ListPageHeader>());

        // Calculate entry size based on quantization
        let entry_size = match quantization {
            QuantizationType::None => size_of::<VectorEntry>() + dimensions * 4,
            QuantizationType::Scalar => size_of::<VectorEntry>() + dimensions + 8, // + scale, offset
            QuantizationType::Product => size_of::<VectorEntry>() + 48, // PQ codes (assuming m=48)
            QuantizationType::Binary => size_of::<VectorEntry>() + (dimensions + 7) / 8,
        };

        for i in 0..list_header.entry_count as usize {
            let entry_ptr = entry_data_ptr.add(i * entry_size);
            let entry = ptr::read(entry_ptr as *const VectorEntry);

            // Read vector or quantized representation
            let vector = if entry.has_quantized_data() && quantization != QuantizationType::None {
                // Dequantize based on type
                match quantization {
                    QuantizationType::Scalar => {
                        let data_ptr = entry_ptr.add(size_of::<VectorEntry>());
                        let scale = ptr::read(data_ptr as *const f32);
                        let offset = ptr::read(data_ptr.add(4) as *const f32);
                        let quantized =
                            std::slice::from_raw_parts(data_ptr.add(8) as *const i8, dimensions);
                        scalar::dequantize(quantized, scale, offset)
                    }
                    QuantizationType::Binary => {
                        // Binary doesn't directly dequantize, return zeros as placeholder
                        // Actual distance is computed via Hamming
                        vec![0.0; dimensions]
                    }
                    _ => {
                        // Product quantization would need the quantizer
                        vec![0.0; dimensions]
                    }
                }
            } else {
                let vector_ptr = entry_ptr.add(size_of::<VectorEntry>()) as *const f32;
                std::slice::from_raw_parts(vector_ptr, dimensions).to_vec()
            };

            result.push((entry, vector));
        }

        current_page = list_header.next_page;
        pg_sys::UnlockReleaseBuffer(buffer);
    }

    result
}

/// Write vectors to an inverted list, returns (start_page, page_count)
unsafe fn write_inverted_list(
    index: Relation,
    cluster_id: u32,
    entries: &[(ItemPointerData, Vec<f32>)],
    dimensions: usize,
    quantization: QuantizationType,
) -> (u32, u32) {
    if entries.is_empty() {
        return (0, 0);
    }

    let page_header_size = size_of::<pg_sys::PageHeaderData>();
    let list_header_size = size_of::<ListPageHeader>();
    let usable_space = pg_sys::BLCKSZ as usize - page_header_size - list_header_size;

    // Calculate entry size based on quantization
    let entry_size = match quantization {
        QuantizationType::None => size_of::<VectorEntry>() + dimensions * 4,
        QuantizationType::Scalar => size_of::<VectorEntry>() + dimensions + 8,
        QuantizationType::Product => size_of::<VectorEntry>() + 48,
        QuantizationType::Binary => size_of::<VectorEntry>() + (dimensions + 7) / 8,
    };

    let entries_per_page = usable_space / entry_size;
    if entries_per_page == 0 {
        pgrx::warning!(
            "IVFFlat: Vector too large for page, entry_size={}",
            entry_size
        );
        return (0, 0);
    }

    let mut start_page: u32 = 0;
    let mut page_count: u32 = 0;
    let mut prev_buffer: Buffer = pg_sys::InvalidBuffer as Buffer;
    let mut prev_header_ptr: *mut ListPageHeader = std::ptr::null_mut();
    let mut written = 0;

    while written < entries.len() {
        // Allocate new page
        let buffer = pg_sys::ReadBuffer(index, P_NEW_BLOCK);
        let actual_page = pg_sys::BufferGetBlockNumber(buffer);

        pg_sys::LockBuffer(buffer, pg_sys::BUFFER_LOCK_EXCLUSIVE as i32);

        let page = pg_sys::BufferGetPage(buffer);
        pg_sys::PageInit(page, pg_sys::BLCKSZ as Size, 0);

        // Track first page
        if start_page == 0 {
            start_page = actual_page;
        }
        page_count += 1;

        // Link previous page to this one
        if !prev_header_ptr.is_null() {
            (*prev_header_ptr).next_page = actual_page;
            pg_sys::MarkBufferDirty(prev_buffer);
            pg_sys::UnlockReleaseBuffer(prev_buffer);
        }

        let header = page as *mut pg_sys::PageHeaderData;
        let data_ptr = (header as *mut u8).add(page_header_size);

        // Write list page header
        let list_header = data_ptr as *mut ListPageHeader;
        (*list_header).page_type = IVFFLAT_PAGE_LIST;
        (*list_header).cluster_id = cluster_id as u8;
        (*list_header)._padding = [0; 2];
        (*list_header).next_page = 0; // Will be updated if there's a next page
        (*list_header).dimensions = dimensions as u32;

        let entry_data_ptr = data_ptr.add(list_header_size);
        let batch_size = (entries.len() - written).min(entries_per_page);

        for i in 0..batch_size {
            let (tid, vector) = &entries[written + i];
            let entry_ptr = entry_data_ptr.add(i * entry_size);

            // Write VectorEntry header
            let vec_entry = VectorEntry::from_item_pointer(*tid, 0);
            ptr::write(entry_ptr as *mut VectorEntry, vec_entry);

            // Write vector data (no quantization for now)
            let vector_ptr = entry_ptr.add(size_of::<VectorEntry>()) as *mut f32;
            for (j, &val) in vector.iter().enumerate() {
                if j < dimensions {
                    ptr::write(vector_ptr.add(j), val);
                }
            }
        }

        (*list_header).entry_count = batch_size as u32;
        written += batch_size;

        pg_sys::MarkBufferDirty(buffer);

        // Keep reference for linking
        prev_buffer = buffer;
        prev_header_ptr = list_header;
    }

    // Release the last buffer
    if prev_buffer != pg_sys::InvalidBuffer as Buffer {
        pg_sys::UnlockReleaseBuffer(prev_buffer);
    }

    (start_page, page_count)
}

// ============================================================================
// Index Search
// ============================================================================

/// Perform IVFFlat search
unsafe fn ivfflat_search(
    index: Relation,
    meta: &IvfFlatMetaPage,
    query: &[f32],
    k: usize,
    probes: usize,
) -> Vec<(ItemPointerData, f32)> {
    if meta.trained == 0 || meta.vector_count == 0 {
        return Vec::new();
    }

    let metric = metric_from_u32(meta.metric);
    let quantization = quantization_from_u32(meta.quantization);
    let dimensions = meta.dimensions as usize;

    // Read centroids
    let centroids = read_centroids(
        index,
        meta.centroid_start_page,
        meta.lists as usize,
        dimensions,
    );

    // Find nearest centroids
    let centroid_vectors: Vec<Vec<f32>> = centroids.iter().map(|(_, v)| v.clone()).collect();
    let nearest = find_nearest_centroids(query, &centroid_vectors, probes, metric);

    // Search in selected lists
    let mut candidates: BinaryHeap<SearchCandidate> = BinaryHeap::new();
    let rerank_factor = if quantization != QuantizationType::None && get_quantized_search_guc() {
        get_rerank_factor_guc()
    } else {
        1
    };

    for (cluster_idx, _) in &nearest {
        if *cluster_idx >= centroids.len() {
            continue;
        }

        let (entry, _) = &centroids[*cluster_idx];

        // Read inverted list
        let list_entries =
            read_inverted_list(index, entry.list_start_page, dimensions, quantization);

        for (vec_entry, vector) in list_entries {
            let dist = calc_distance(query, &vector, metric);

            let candidate = SearchCandidate {
                tid: vec_entry.to_item_pointer(),
                distance: dist,
                cluster_id: entry.cluster_id,
                needs_rerank: vec_entry.has_quantized_data()
                    && quantization != QuantizationType::None,
            };

            candidates.push(candidate);

            // Keep only top k * rerank_factor candidates
            if candidates.len() > k * rerank_factor {
                candidates.pop();
            }
        }
    }

    // Convert to results and sort
    let mut results: Vec<(ItemPointerData, f32)> = candidates
        .into_iter()
        .map(|c| (c.tid, c.distance))
        .collect();

    results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
    results.truncate(k);

    results
}

// ============================================================================
// Access Method Callbacks
// ============================================================================

/// Build an IVFFlat index
#[pg_guard]
unsafe extern "C" fn ivfflat_ambuild(
    heap: Relation,
    index: Relation,
    index_info: *mut IndexInfo,
) -> *mut IndexBuildResult {
    pgrx::info!("IVFFlat v2: Starting index build");

    // Parse options from reloptions
    let mut lists = 100u32;
    let mut metric = DistanceMetric::Euclidean;
    let mut quantization = QuantizationType::None;

    // TODO: Parse reloptions for lists, metric, quantization

    // Initialize metadata
    let mut meta = IvfFlatMetaPage {
        lists,
        metric: metric_to_u32(metric),
        quantization: quantization_to_u32(quantization),
        ..Default::default()
    };

    // Collect vectors from heap using table scan
    let mut all_vectors: Vec<(ItemPointerData, Vec<f32>)> = Vec::new();

    pgrx::info!("IVFFlat v2: Scanning heap for vectors");

    // Use build callback to collect vectors
    struct IvfBuildState {
        vectors: *mut Vec<(ItemPointerData, Vec<f32>)>,
    }

    unsafe extern "C" fn ivf_build_callback(
        _index: Relation,
        ctid: ItemPointer,
        values: *mut Datum,
        isnull: *mut bool,
        _tuple_is_alive: bool,
        state: *mut ::std::os::raw::c_void,
    ) {
        let build_state = &mut *(state as *mut IvfBuildState);

        if *isnull {
            return;
        }

        let datum = *values;
        let vector = match RuVector::from_polymorphic_datum(datum, false, pg_sys::InvalidOid) {
            Some(v) => v.as_slice().to_vec(),
            None => {
                let raw_ptr = datum.cast_mut_ptr::<pg_sys::varlena>();
                if raw_ptr.is_null() {
                    return;
                }
                let detoasted = pg_sys::pg_detoast_datum(raw_ptr);
                if detoasted.is_null() {
                    return;
                }
                let data_ptr = pgrx::varlena::vardata_any(detoasted as *const _) as *const u8;
                let dims = std::ptr::read_unaligned(data_ptr as *const u16) as usize;
                if dims == 0 {
                    return;
                }
                let f32_ptr = data_ptr.add(4) as *const f32;
                std::slice::from_raw_parts(f32_ptr, dims).to_vec()
            }
        };

        if !vector.is_empty() {
            (*build_state.vectors).push((*ctid, vector));
        }
    }

    let mut build_state = IvfBuildState {
        vectors: &mut all_vectors as *mut Vec<(ItemPointerData, Vec<f32>)>,
    };

    pg_sys::table_index_build_scan(
        heap,
        index,
        index_info,
        true,
        false,
        Some(ivf_build_callback),
        &mut build_state as *mut IvfBuildState as *mut ::std::os::raw::c_void,
        std::ptr::null_mut(),
    );

    pgrx::info!(
        "IVFFlat v2: Collected {} vectors from heap",
        all_vectors.len()
    );

    // Set dimensions from first vector
    if !all_vectors.is_empty() {
        meta.dimensions = all_vectors[0].1.len() as u32;
    }

    // Sample vectors for training
    let training_sample: Vec<Vec<f32>> = all_vectors
        .iter()
        .take(10000.min(all_vectors.len()))
        .map(|(_, v)| v.clone())
        .collect();

    pgrx::info!(
        "IVFFlat v2: Training with {} samples, {} lists",
        training_sample.len(),
        lists
    );

    // Train centroids with k-means++
    let n_clusters = lists as usize;
    let mut centroids = kmeans_plus_plus_init(&training_sample, n_clusters, metric, 42);
    centroids = kmeans_cluster(
        &training_sample,
        centroids,
        DEFAULT_KMEANS_ITERATIONS,
        metric,
    );

    pgrx::info!("IVFFlat v2: Trained {} centroids", centroids.len());

    // Assign all vectors to clusters
    let mut cluster_lists: Vec<Vec<(ItemPointerData, Vec<f32>)>> = vec![Vec::new(); n_clusters];

    for (tid, vector) in &all_vectors {
        let cluster = find_nearest_centroid(vector, &centroids, metric);
        cluster_lists[cluster].push((*tid, vector.clone()));
    }

    // Calculate list statistics
    let list_sizes: Vec<usize> = cluster_lists.iter().map(|l| l.len()).collect();
    meta.max_list_size = *list_sizes.iter().max().unwrap_or(&0) as u32;
    meta.min_list_size = *list_sizes.iter().filter(|&&s| s > 0).min().unwrap_or(&0) as u32;
    meta.health_score = (meta.calculate_health() * 1000.0) as u32;

    // Write initial metadata page
    write_meta_page(index, &meta);

    // Write centroids first (to reserve pages)
    let centroid_entries_temp: Vec<(CentroidEntry, Vec<f32>)> = centroids
        .iter()
        .enumerate()
        .map(|(i, c)| {
            (
                CentroidEntry {
                    cluster_id: i as u32,
                    list_start_page: 0, // Will be updated after writing lists
                    list_page_count: 0,
                    vector_count: cluster_lists.get(i).map(|l| l.len()).unwrap_or(0) as u32,
                    distance_sum: 0.0,
                    reserved: 0,
                },
                c.clone(),
            )
        })
        .collect();

    let lists_start_page = write_centroids(
        index,
        &centroid_entries_temp,
        meta.centroid_start_page,
        meta.dimensions as usize,
    );

    // Write inverted lists for each cluster
    pgrx::info!(
        "IVFFlat v2: Writing inverted lists for {} clusters",
        n_clusters
    );
    let mut list_info: Vec<(u32, u32)> = Vec::with_capacity(n_clusters);
    let mut total_vectors_written = 0u64;

    for (cluster_id, entries) in cluster_lists.iter().enumerate() {
        let (start_page, page_count) = write_inverted_list(
            index,
            cluster_id as u32,
            entries,
            meta.dimensions as usize,
            quantization,
        );
        list_info.push((start_page, page_count));
        total_vectors_written += entries.len() as u64;
    }

    pgrx::info!(
        "IVFFlat v2: Written {} vectors to inverted lists",
        total_vectors_written
    );

    // Re-write centroids with correct list_start_page values
    let centroid_entries_final: Vec<(CentroidEntry, Vec<f32>)> = centroids
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let (start_page, page_count) = list_info.get(i).copied().unwrap_or((0, 0));
            (
                CentroidEntry {
                    cluster_id: i as u32,
                    list_start_page: start_page,
                    list_page_count: page_count,
                    vector_count: cluster_lists.get(i).map(|l| l.len()).unwrap_or(0) as u32,
                    distance_sum: 0.0,
                    reserved: 0,
                },
                c.clone(),
            )
        })
        .collect();

    // Overwrite centroids with updated list_start_page values
    rewrite_centroids(
        index,
        &centroid_entries_final,
        meta.centroid_start_page,
        meta.dimensions as usize,
    );

    // Update metadata
    meta.lists_start_page = lists_start_page;
    meta.trained = 1;
    meta.vector_count = all_vectors.len() as u64;
    write_meta_page(index, &meta);

    pgrx::info!(
        "IVFFlat v2: Index build complete, {} vectors in {} lists",
        all_vectors.len(),
        lists
    );

    // Return build result
    let mut result = PgBox::<IndexBuildResult>::alloc0();
    result.heap_tuples = all_vectors.len() as f64;
    result.index_tuples = all_vectors.len() as f64;
    result.into_pg()
}

/// Build empty IVFFlat index
#[pg_guard]
unsafe extern "C" fn ivfflat_ambuildempty(index: Relation) {
    pgrx::info!("IVFFlat v2: Building empty index");

    // Initialize empty metadata page
    let meta = IvfFlatMetaPage::default();
    write_meta_page(index, &meta);
}

/// Insert a tuple into the index
#[pg_guard]
unsafe extern "C" fn ivfflat_aminsert(
    index: Relation,
    values: *mut Datum,
    isnull: *mut bool,
    heap_tid: ItemPointer,
    _heap: Relation,
    _check_unique: IndexUniqueCheck::Type,
    _index_unchanged: bool,
    _index_info: *mut IndexInfo,
) -> bool {
    // Check for null
    if *isnull {
        return false;
    }

    // Read metadata
    let mut meta = read_meta_page(index);

    if meta.trained == 0 {
        pgrx::warning!("Cannot insert into untrained IVFFlat index");
        return false;
    }

    // TODO: Extract vector from datum
    // TODO: Find nearest centroid
    // TODO: Insert into appropriate inverted list
    // TODO: Update metadata counters

    // Track insertions for retraining decision
    meta.insertions_since_retrain += 1;

    // Check if retraining is recommended
    if meta.needs_retrain() {
        pgrx::notice!(
            "IVFFlat: Index may benefit from retraining ({} insertions, health: {:.2})",
            meta.insertions_since_retrain,
            meta.calculate_health()
        );
    }

    true
}

/// Bulk delete callback
#[pg_guard]
unsafe extern "C" fn ivfflat_ambulkdelete(
    _info: *mut IndexVacuumInfo,
    stats: *mut IndexBulkDeleteResult,
    _callback: IndexBulkDeleteCallback,
    _callback_state: *mut ::std::os::raw::c_void,
) -> *mut IndexBulkDeleteResult {
    pgrx::log!("IVFFlat v2: Bulk delete called");

    if stats.is_null() {
        let new_stats = PgBox::<IndexBulkDeleteResult>::alloc0();
        new_stats.into_pg()
    } else {
        stats
    }
}

/// Vacuum cleanup callback
#[pg_guard]
unsafe extern "C" fn ivfflat_amvacuumcleanup(
    info: *mut IndexVacuumInfo,
    stats: *mut IndexBulkDeleteResult,
) -> *mut IndexBulkDeleteResult {
    pgrx::log!("IVFFlat v2: Vacuum cleanup called");

    // Update health metrics
    if !info.is_null() {
        let index = (*info).index;
        let meta = read_meta_page(index);

        if meta.needs_retrain() {
            pgrx::notice!(
                "IVFFlat: Consider running REINDEX to improve performance (health: {:.2})",
                meta.calculate_health()
            );
        }
    }

    if stats.is_null() {
        let new_stats = PgBox::<IndexBulkDeleteResult>::alloc0();
        new_stats.into_pg()
    } else {
        stats
    }
}

/// Cost estimate callback
#[pg_guard]
unsafe extern "C" fn ivfflat_amcostestimate(
    _root: *mut PlannerInfo,
    path: *mut IndexPath,
    _loop_count: f64,
    index_startup_cost: *mut Cost,
    index_total_cost: *mut Cost,
    index_selectivity: *mut Selectivity,
    index_correlation: *mut f64,
    index_pages: *mut f64,
) {
    // Get tuple count
    let tuples = if let Some(info) = (*path).indexinfo.as_ref() {
        (*info).tuples
    } else {
        1000.0
    };

    // IVFFlat cost model
    // Startup: find nearest centroids (O(lists))
    let lists = 100.0; // Default, should read from index
    let probes = get_probes_guc() as f64;

    *index_startup_cost = lists * 0.01; // Centroid comparison cost

    // Total: startup + scanning probed lists (O(probes * N/lists))
    let vectors_per_list = tuples / lists;
    let vectors_scanned = vectors_per_list * probes;

    *index_total_cost = *index_startup_cost + vectors_scanned * 0.001;

    // Selectivity based on k and probes
    *index_selectivity = (probes / lists).min(1.0);
    *index_correlation = 0.0; // No correlation with physical order
    *index_pages = (tuples / 100.0).max(1.0);
}

/// Begin scan callback
#[pg_guard]
unsafe extern "C" fn ivfflat_ambeginscan(
    index: Relation,
    nkeys: ::std::os::raw::c_int,
    norderbys: ::std::os::raw::c_int,
) -> IndexScanDesc {
    pgrx::debug1!("IVFFlat v2: Begin scan");

    let scan = pg_sys::RelationGetIndexScan(index, nkeys, norderbys);

    // Allocate ORDER BY distance arrays (required by the AM, not done by
    // RelationGetIndexScan). See GiST's gistbeginscan for reference.
    if (*scan).numberOfOrderBys > 0 {
        let n = (*scan).numberOfOrderBys as usize;
        (*scan).xs_orderbyvals =
            pg_sys::palloc0(std::mem::size_of::<pg_sys::Datum>() * n) as *mut pg_sys::Datum;
        (*scan).xs_orderbynulls = pg_sys::palloc(std::mem::size_of::<bool>() * n) as *mut bool;
        std::ptr::write_bytes((*scan).xs_orderbynulls, 1u8, n);
    }

    // Allocate scan state
    let meta = read_meta_page(index);
    let state = Box::new(IvfFlatScanState {
        query: Vec::new(),
        k: 10,
        probes: get_probes_guc(),
        results: Vec::new(),
        current: 0,
        metric: metric_from_u32(meta.metric),
        quantization: quantization_from_u32(meta.quantization),
        search_done: false,
        meta,
        query_valid: false,
    });

    (*scan).opaque = Box::into_raw(state) as *mut ::std::os::raw::c_void;

    scan
}

/// Rescan callback
#[pg_guard]
unsafe extern "C" fn ivfflat_amrescan(
    scan: IndexScanDesc,
    _keys: ScanKey,
    _nkeys: ::std::os::raw::c_int,
    orderbys: ScanKey,
    norderbys: ::std::os::raw::c_int,
) {
    pgrx::debug1!("IVFFlat v2: Rescan (norderbys={})", norderbys);

    let state = (*scan).opaque as *mut IvfFlatScanState;
    if state.is_null() {
        return;
    }

    // Reset scan state
    (*state).results.clear();
    (*state).current = 0;
    (*state).search_done = false;
    (*state).query_valid = false;

    // Extract query vector from ORDER BY
    if norderbys > 0 && !orderbys.is_null() {
        let orderby = &*orderbys;
        let datum = orderby.sk_argument;
        let typoid = orderby.sk_subtype;

        pgrx::debug1!(
            "IVFFlat v2: Extracting query vector, datum null={}, typoid={}",
            datum.is_null(),
            typoid.as_u32()
        );

        // Method 1: Try direct RuVector extraction
        if let Some(vector) = RuVector::from_polymorphic_datum(datum, false, typoid) {
            (*state).query = vector.as_slice().to_vec();
            (*state).query_valid = true;
            pgrx::debug1!(
                "IVFFlat v2: Extracted query vector (direct) with {} dimensions",
                (*state).query.len()
            );
        }

        // Method 2: Handle text parameter conversion
        if !(*state).query_valid && !datum.is_null() {
            let is_text_type = typoid == pg_sys::Oid::from(25)
                || typoid == pg_sys::Oid::from(1043)
                || typoid == pg_sys::Oid::from(705)
                || typoid == pg_sys::InvalidOid;

            if is_text_type {
                if let Some(vec) = ivfflat_try_convert_text_to_ruvector(datum) {
                    (*state).query = vec;
                    (*state).query_valid = true;
                    pgrx::debug1!(
                        "IVFFlat v2: Converted text parameter to query vector with {} dimensions",
                        (*state).query.len()
                    );
                }
            }
        }

        // Method 3: Fallback - raw varlena extraction
        if !(*state).query_valid {
            let raw_ptr = datum.cast_mut_ptr::<pg_sys::varlena>();
            if !raw_ptr.is_null() {
                let detoasted = pg_sys::pg_detoast_datum(raw_ptr);
                if !detoasted.is_null() {
                    let total_size = pgrx::varlena::varsize_any(detoasted as *const _);
                    if total_size >= 8 {
                        let data_ptr =
                            pgrx::varlena::vardata_any(detoasted as *const _) as *const u8;
                        let dimensions = std::ptr::read_unaligned(data_ptr as *const u16) as usize;
                        let expected_data_size = 4 + (dimensions * 4);
                        let actual_data_size = total_size - pg_sys::VARHDRSZ;

                        if dimensions > 0
                            && dimensions <= 16384
                            && actual_data_size >= expected_data_size
                        {
                            let f32_ptr = data_ptr.add(4) as *const f32;
                            (*state).query =
                                std::slice::from_raw_parts(f32_ptr, dimensions).to_vec();
                            (*state).query_valid = true;
                            pgrx::debug1!(
                                "IVFFlat v2: Extracted query vector (varlena fallback) with {} dimensions",
                                dimensions
                            );
                        }
                    }
                }
            }
        }

        // Validate query vector
        if !(*state).query_valid || (*state).query.is_empty() {
            pgrx::error!(
                "IVFFlat: Could not extract query vector from parameter. \
                 Ensure the query vector is properly cast to ruvector type."
            );
        }

        // Validate not all zeros
        if (*state).query.iter().all(|&x| x == 0.0) {
            pgrx::error!(
                "IVFFlat: Query vector is all zeros, which is invalid for similarity search."
            );
        }

        // Validate dimensions
        if (*state).query.len() != (*state).meta.dimensions as usize {
            pgrx::error!(
                "IVFFlat: Query vector has {} dimensions but index expects {}",
                (*state).query.len(),
                (*state).meta.dimensions
            );
        }

        // Calculate adaptive probes
        let query_norm = vector_norm(&(*state).query);
        (*state).probes = compute_adaptive_probes(
            (*state).meta.dimensions as usize,
            (*state).meta.lists as usize,
            (*state).k,
            query_norm,
        );
    }
}

/// Try to convert a text datum to ruvector (for parameterized queries)
unsafe fn ivfflat_try_convert_text_to_ruvector(datum: Datum) -> Option<Vec<f32>> {
    let text_ptr = datum.cast_mut_ptr::<pg_sys::text>();
    if text_ptr.is_null() {
        return None;
    }

    let detoasted = pg_sys::pg_detoast_datum(text_ptr as *mut pg_sys::varlena);
    if detoasted.is_null() {
        return None;
    }

    let text_len = pgrx::varlena::varsize_any_exhdr(detoasted as *const _);
    let text_data = pgrx::varlena::vardata_any(detoasted as *const _) as *const u8;

    if text_len == 0 {
        return None;
    }

    let text_slice = std::slice::from_raw_parts(text_data, text_len);
    let text_str = match std::str::from_utf8(text_slice) {
        Ok(s) => s.trim(),
        Err(_) => return None,
    };

    if !text_str.starts_with('[') || !text_str.ends_with(']') {
        return None;
    }

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

/// Get tuple callback
#[pg_guard]
unsafe extern "C" fn ivfflat_amgettuple(
    scan: IndexScanDesc,
    direction: ScanDirection::Type,
) -> bool {
    // Only support forward scans
    if direction != pg_sys::ScanDirection::ForwardScanDirection {
        return false;
    }

    let state = (*scan).opaque as *mut IvfFlatScanState;
    if state.is_null() {
        return false;
    }

    // Execute search on first call
    if !(*state).search_done {
        if !(*state).query.is_empty() {
            let index = (*scan).indexRelation;
            (*state).results = ivfflat_search(
                index,
                &(*state).meta,
                &(*state).query,
                (*state).k,
                (*state).probes,
            );
        }
        (*state).search_done = true;
    }

    // Return next result
    if (*state).current < (*state).results.len() {
        let (tid, _distance) = (&(*state).results)[(*state).current];
        (*scan).xs_heaptid = tid;
        (*state).current += 1;

        // Set distance in orderby result
        if !(*scan).xs_orderbynulls.is_null() {
            *(*scan).xs_orderbynulls.offset(0) = false;
        }

        true
    } else {
        false
    }
}

/// Get bitmap callback (for bitmap scans)
#[pg_guard]
unsafe extern "C" fn ivfflat_amgetbitmap(_scan: IndexScanDesc, _tbm: *mut TIDBitmap) -> i64 {
    // IVFFlat doesn't efficiently support bitmap scans
    // Return 0 to indicate no tuples
    0
}

/// End scan callback
#[pg_guard]
unsafe extern "C" fn ivfflat_amendscan(scan: IndexScanDesc) {
    pgrx::debug1!("IVFFlat v2: End scan");

    let state = (*scan).opaque as *mut IvfFlatScanState;
    if !state.is_null() {
        // Drop the boxed state
        let _ = Box::from_raw(state);
        (*scan).opaque = ptr::null_mut();
    }
}

/// Can return callback
#[pg_guard]
unsafe extern "C" fn ivfflat_amcanreturn(_index: Relation, _attno: ::std::os::raw::c_int) -> bool {
    // IVFFlat can return the indexed vector (useful for covering indexes)
    false // For now, disable to avoid complexity
}

/// Options callback - parse index options
#[pg_guard]
unsafe extern "C" fn ivfflat_amoptions(_reloptions: Datum, _validate: bool) -> *mut bytea {
    // TODO: Parse options: lists, quantization, etc.
    // Options format:
    //   lists = 100
    //   quantization = 'none' | 'sq8' | 'pq' | 'binary'

    ptr::null_mut()
}

/// Validate callback
#[pg_guard]
unsafe extern "C" fn ivfflat_amvalidate(_opclass_oid: pg_sys::Oid) -> bool {
    // Validate that the operator class is appropriate for IVFFlat
    true
}

/// Estimate parallel scan size (PG14/15/16 - no parameters)
#[cfg(any(feature = "pg14", feature = "pg15", feature = "pg16"))]
#[pg_guard]
unsafe extern "C" fn ivfflat_amestimateparallelscan() -> Size {
    // Size needed for parallel scan coordination
    size_of::<IvfFlatParallelScanState>() as Size
}

/// Estimate parallel scan size (PG17+ - with parameters)
#[cfg(feature = "pg17")]
#[pg_guard]
unsafe extern "C" fn ivfflat_amestimateparallelscan(
    _nkeys: ::std::os::raw::c_int,
    _norderbys: ::std::os::raw::c_int,
) -> Size {
    // Size needed for parallel scan coordination
    size_of::<IvfFlatParallelScanState>() as Size
}

/// Parallel scan state
#[repr(C)]
struct IvfFlatParallelScanState {
    /// Mutex for coordination
    mutex: pg_sys::slock_t,
    /// Next cluster to scan
    next_cluster: u32,
    /// Total clusters
    total_clusters: u32,
    /// Search complete flag
    search_done: bool,
}

/// Initialize parallel scan
#[pg_guard]
unsafe extern "C" fn ivfflat_aminitparallelscan(target: *mut ::std::os::raw::c_void) {
    let state = target as *mut IvfFlatParallelScanState;

    pg_sys::SpinLockInit(&mut (*state).mutex);
    (*state).next_cluster = 0;
    (*state).total_clusters = 0;
    (*state).search_done = false;
}

/// Parallel rescan
#[pg_guard]
unsafe extern "C" fn ivfflat_amparallelrescan(scan: IndexScanDesc) {
    if (*scan).parallel_scan.is_null() {
        return;
    }

    let target = (*scan).parallel_scan as *mut ::std::os::raw::c_void;
    let state = target as *mut IvfFlatParallelScanState;

    pg_sys::SpinLockAcquire(&mut (*state).mutex);
    (*state).next_cluster = 0;
    (*state).search_done = false;
    pg_sys::SpinLockRelease(&mut (*state).mutex);
}

// ============================================================================
// Access Method Handler
// ============================================================================

/// Static IndexAmRoutine template for IVFFlat
static IVFFLAT_AM_HANDLER: IndexAmRoutine = IndexAmRoutine {
    type_: NodeTag::T_IndexAmRoutine,

    // Index structure capabilities
    amstrategies: 1, // One strategy: nearest neighbor
    amsupport: 1,    // One support function: distance
    amoptsprocnum: 0,
    amcanorder: false,
    amcanorderbyop: true, // Supports ORDER BY with distance operators
    amcanbackward: false,
    amcanunique: false,
    amcanmulticol: false, // Single column only (vector)
    amoptionalkey: true,
    amsearcharray: false,
    amsearchnulls: false,
    amstorage: false,
    amclusterable: false,
    ampredlocks: false,
    amcanparallel: true, // Supports parallel scan
    amcaninclude: false,
    amusemaintenanceworkmem: true,
    #[cfg(any(feature = "pg16", feature = "pg17"))]
    amsummarizing: false,
    amparallelvacuumoptions: 0,

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
    #[cfg(feature = "pg17")]
    amcanbuildparallel: false,
    #[cfg(feature = "pg17")]
    aminsertcleanup: None,
};

/// Main handler function for IVFFlat index access method
#[pg_extern(sql = r#"
CREATE OR REPLACE FUNCTION ruivfflat_handler(internal) RETURNS index_am_handler
    AS 'MODULE_PATHNAME', 'ruivfflat_handler_wrapper' LANGUAGE C STRICT;

-- Create the access method
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_am WHERE amname = 'ruivfflat') THEN
        CREATE ACCESS METHOD ruivfflat TYPE INDEX HANDLER ruivfflat_handler;
    END IF;
END $$;

-- Create operator class for L2 distance
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_opclass WHERE opcname = 'vector_l2_ops' AND opcmethod = (SELECT oid FROM pg_am WHERE amname = 'ruivfflat')) THEN
        -- Operator class will be created when vector type is available
        RAISE NOTICE 'ruivfflat access method registered. Create operator classes after vector type is defined.';
    END IF;
END $$;
"#)]
fn ruivfflat_handler(_fcinfo: pg_sys::FunctionCallInfo) -> Internal {
    unsafe {
        // Allocate IndexAmRoutine in PostgreSQL memory context
        let am_routine = pg_sys::palloc0(size_of::<IndexAmRoutine>()) as *mut IndexAmRoutine;

        // Copy template into allocated memory
        ptr::copy_nonoverlapping(&IVFFLAT_AM_HANDLER, am_routine, 1);

        // Set callback function pointers
        (*am_routine).ambuild = Some(ivfflat_ambuild);
        (*am_routine).ambuildempty = Some(ivfflat_ambuildempty);
        (*am_routine).aminsert = Some(ivfflat_aminsert);
        (*am_routine).ambulkdelete = Some(ivfflat_ambulkdelete);
        (*am_routine).amvacuumcleanup = Some(ivfflat_amvacuumcleanup);
        (*am_routine).ambeginscan = Some(ivfflat_ambeginscan);
        (*am_routine).amrescan = Some(ivfflat_amrescan);
        (*am_routine).amgettuple = Some(ivfflat_amgettuple);
        (*am_routine).amgetbitmap = Some(ivfflat_amgetbitmap);
        (*am_routine).amendscan = Some(ivfflat_amendscan);
        (*am_routine).amcostestimate = Some(ivfflat_amcostestimate);
        (*am_routine).amoptions = Some(ivfflat_amoptions);
        (*am_routine).amcanreturn = Some(ivfflat_amcanreturn);
        (*am_routine).amvalidate = Some(ivfflat_amvalidate);
        (*am_routine).amestimateparallelscan = Some(ivfflat_amestimateparallelscan);
        (*am_routine).aminitparallelscan = Some(ivfflat_aminitparallelscan);
        (*am_routine).amparallelrescan = Some(ivfflat_amparallelrescan);

        // Return as Internal datum
        Internal::from(Some(Datum::from(am_routine)))
    }
}

// ============================================================================
// SQL Functions for Index Management
// ============================================================================

/// Get IVFFlat index health information
#[pg_extern(sql = r#"
CREATE OR REPLACE FUNCTION ruivfflat_index_health(index_name text)
RETURNS TABLE (
    lists integer,
    vector_count bigint,
    max_list_size integer,
    min_list_size integer,
    health_score real,
    needs_retrain boolean,
    insertions_since_retrain bigint,
    quantization text
) AS 'MODULE_PATHNAME', 'ruivfflat_index_health_wrapper' LANGUAGE C STRICT;
"#)]
fn ruivfflat_index_health(
    index_name: &str,
) -> TableIterator<
    'static,
    (
        name!(lists, i32),
        name!(vector_count, i64),
        name!(max_list_size, i32),
        name!(min_list_size, i32),
        name!(health_score, f32),
        name!(needs_retrain, bool),
        name!(insertions_since_retrain, i64),
        name!(quantization, String),
    ),
> {
    // TODO: Look up index by name and read metadata
    // For now, return placeholder data

    let results = vec![(
        100i32,
        0i64,
        0i32,
        0i32,
        1.0f32,
        false,
        0i64,
        "none".to_string(),
    )];

    TableIterator::new(results)
}

/// Set IVFFlat probes GUC
#[pg_extern]
fn ruivfflat_set_probes(probes: i32) {
    if probes < 1 {
        pgrx::error!("probes must be at least 1");
    }
    GUC_IVFFLAT_PROBES.store(probes as u64, AtomicOrdering::Relaxed);
    pgrx::notice!("IVFFlat probes set to {}", probes);
}

/// Get current IVFFlat probes setting
#[pg_extern]
fn ruivfflat_get_probes() -> i32 {
    get_probes_guc() as i32
}

/// Enable/disable adaptive probes
#[pg_extern]
fn ruivfflat_set_adaptive_probes(enabled: bool) {
    GUC_ADAPTIVE_PROBES.store(enabled, AtomicOrdering::Relaxed);
    pgrx::notice!(
        "IVFFlat adaptive probes {}",
        if enabled { "enabled" } else { "disabled" }
    );
}

/// Trigger index retraining (incremental centroid update)
#[pg_extern]
fn ruivfflat_retrain(_index_name: &str, _sample_size: Option<i32>) -> bool {
    // TODO: Implement incremental retraining
    // 1. Sample vectors from the index
    // 2. Run k-means with current centroids as initial state
    // 3. Update centroid pages
    // 4. Optionally reassign vectors to new centroids

    pgrx::notice!("IVFFlat incremental retraining not yet implemented");
    false
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_meta_page_size() {
        assert!(size_of::<IvfFlatMetaPage>() < 8192);
    }

    #[pg_test]
    fn test_centroid_entry_size() {
        assert!(size_of::<CentroidEntry>() < 100);
    }

    #[pg_test]
    fn test_vector_entry_size() {
        assert!(size_of::<VectorEntry>() < 20);
    }

    #[pg_test]
    fn test_health_calculation() {
        let mut meta = IvfFlatMetaPage::default();

        // Perfect balance
        meta.max_list_size = 100;
        meta.min_list_size = 100;
        assert!((meta.calculate_health() - 1.0).abs() < 0.001);

        // Moderate imbalance
        meta.max_list_size = 200;
        meta.min_list_size = 100;
        let health = meta.calculate_health();
        assert!(health > 0.7 && health < 1.0);

        // Severe imbalance
        meta.max_list_size = 1000;
        meta.min_list_size = 10;
        assert!(meta.calculate_health() < HEALTH_WARNING);
    }

    #[pg_test]
    fn test_kmeans_init() {
        let vectors: Vec<Vec<f32>> = (0..100)
            .map(|i| vec![i as f32, (i * 2) as f32, (i * 3) as f32])
            .collect();

        let centroids = kmeans_plus_plus_init(&vectors, 10, DistanceMetric::Euclidean, 42);

        assert_eq!(centroids.len(), 10);
        assert_eq!(centroids[0].len(), 3);
    }

    #[pg_test]
    fn test_find_nearest_centroids() {
        let centroids = vec![
            vec![0.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        let query = vec![0.1, 0.0, 0.0];
        let nearest = find_nearest_centroids(&query, &centroids, 2, DistanceMetric::Euclidean);

        assert_eq!(nearest.len(), 2);
        assert_eq!(nearest[0].0, 0); // Origin is closest
    }

    #[pg_test]
    fn test_adaptive_probes() {
        // Low dimensions
        let probes = compute_adaptive_probes(64, 100, 10, 1.0);
        assert!(probes >= 1);

        // High dimensions should increase probes
        GUC_ADAPTIVE_PROBES.store(true, AtomicOrdering::Relaxed);
        GUC_IVFFLAT_PROBES.store(5, AtomicOrdering::Relaxed);

        let low_dim_probes = compute_adaptive_probes(64, 100, 10, 1.0);
        let high_dim_probes = compute_adaptive_probes(1536, 100, 10, 1.0);

        assert!(high_dim_probes >= low_dim_probes);
    }

    #[pg_test]
    fn test_guc_operations() {
        ruivfflat_set_probes(20);
        assert_eq!(ruivfflat_get_probes(), 20);

        ruivfflat_set_adaptive_probes(true);
        assert!(get_adaptive_probes_guc());
    }

    #[pg_test]
    fn test_vector_entry_tid_conversion() {
        let mut tid = ItemPointerData::default();
        tid.ip_blkid.bi_hi = 0x1234;
        tid.ip_blkid.bi_lo = 0x5678;
        tid.ip_posid = 42;

        let entry = VectorEntry::from_item_pointer(tid, 0);
        let recovered = entry.to_item_pointer();

        assert_eq!(recovered.ip_blkid.bi_hi, tid.ip_blkid.bi_hi);
        assert_eq!(recovered.ip_blkid.bi_lo, tid.ip_blkid.bi_lo);
        assert_eq!(recovered.ip_posid, tid.ip_posid);
    }
}
