# RuVector Postgres v2 - Phase 2: Tiered Storage & Compression

## Overview

Phase 2 implements automatic tiered storage with compression, enabling efficient management of vectors across hot/warm/cool/cold tiers based on access patterns.

---

## Objectives

### Primary Goals
1. Automatic tier classification based on access frequency
2. Transparent compression for cold tiers
3. Background tier management without blocking operations
4. SQL API for tier configuration and monitoring

### Success Criteria
- 4-32x storage reduction for cold vectors
- < 5% query latency overhead for hot tier
- Automatic promotion on access
- Zero-downtime tier migrations

---

## Tier Exactness and Correctness

### Visibility and Correctness per Tier

When vectors move tiers or change representation (SQ8, PQ), distances become approximate. Users must understand the tradeoffs.

```
+------------------------------------------------------------------+
|              TIER EXACTNESS SPECIFICATION                         |
+------------------------------------------------------------------+

HOT TIER:
  • Storage: float32 (4 bytes per dimension)
  • Distance: EXACT
  • Recall: > 98%
  • Use: Frequently accessed, latency-critical

WARM TIER:
  • Storage: float16 or SQ8 (1-2 bytes per dimension)
  • Distance: EXACT or SCALED (configurable)
  • Recall: > 96%
  • Note: float16 is exact; SQ8 introduces quantization error

COOL TIER:
  • Storage: PQ16 (0.125 bytes per dimension per subquantizer)
  • Distance: APPROXIMATE
  • Recall: > 94%
  • Rerank: REQUIRED for exact final top-k
  • Strategy: Search cool tier, fetch hot/warm for rerank

COLD TIER:
  • Storage: PQ32/64 (high compression)
  • Distance: APPROXIMATE ONLY
  • Recall: > 90%
  • Rerank: OPTIONAL (offline or batch only)
  • Note: May not be in memory, disk fetch required

+------------------------------------------------------------------+
```

### Exactness Mode Configuration

```sql
-- Session-level GUC for search exactness
-- SET ruvector.search_exactness = 'exact' | 'balanced' | 'fast';

-- GUC registration in extension
```

```rust
/// Register the exactness GUC
static SEARCH_EXACTNESS: GucSetting<ExactnessMode> = GucSetting::new(ExactnessMode::Balanced);

#[pg_guard]
pub extern "C" fn _PG_init() {
    GucRegistry::define_enum_guc(
        "ruvector.search_exactness",
        "Controls distance calculation accuracy vs speed tradeoff",
        "exact: always use original vectors; balanced: rerank approximate results; fast: use tier representation as-is",
        &SEARCH_EXACTNESS,
        &[
            ("exact", ExactnessMode::Exact as i32),
            ("balanced", ExactnessMode::Balanced as i32),
            ("fast", ExactnessMode::Fast as i32),
        ],
        GucContext::Userset,
        GucFlags::default(),
    );
}
```

```sql
-- Usage examples:
SET ruvector.search_exactness = 'fast';      -- Fastest, lowest recall
SET ruvector.search_exactness = 'balanced';  -- Default, good tradeoff
SET ruvector.search_exactness = 'exact';     -- Highest recall, slowest

-- Per-collection default (overridden by session GUC)
ALTER TABLE ruvector.collections ADD COLUMN IF NOT EXISTS
    default_exactness TEXT DEFAULT 'balanced'
    CHECK (default_exactness IN ('exact', 'balanced', 'fast'));
```

### Search Function with Exactness

```sql
-- Extended search function with exactness mode
CREATE FUNCTION ruvector_search(
    p_collection_name TEXT,
    p_query vector,
    p_k INTEGER,
    p_exactness ruvector.exactness_mode DEFAULT NULL,  -- NULL = use collection default
    p_ef_search INTEGER DEFAULT NULL
) RETURNS TABLE (
    id BIGINT,
    distance REAL,
    tier TEXT,
    is_exact BOOLEAN
) AS 'MODULE_PATHNAME', 'ruvector_search_exactness' LANGUAGE C;
```

### Rust Implementation

```rust
/// Search with tier-aware exactness handling
pub fn search_with_exactness(
    collection_id: i32,
    query: &[f32],
    k: usize,
    exactness: ExactnessMode,
) -> Vec<SearchResult> {
    match exactness {
        ExactnessMode::Fast => {
            // Search all tiers, return as-is
            search_all_tiers(collection_id, query, k)
        }
        ExactnessMode::Balanced => {
            // Search all tiers with over-fetch
            let candidates = search_all_tiers(collection_id, query, k * 2);

            // Rerank cool/cold tier results using hot/warm vectors
            let mut results = Vec::with_capacity(k);
            for candidate in candidates {
                if candidate.tier.is_exact() {
                    results.push(candidate);
                } else {
                    // Fetch original vector for rerank
                    if let Some(original) = fetch_original_vector(candidate.id) {
                        let exact_distance = distance(query, &original);
                        results.push(SearchResult {
                            distance: exact_distance,
                            is_exact: true,
                            ..candidate
                        });
                    }
                }
                if results.len() >= k {
                    break;
                }
            }

            // Re-sort by exact distance
            results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
            results.truncate(k);
            results
        }
        ExactnessMode::Exact => {
            // Always fetch original vectors
            let candidates = search_all_tiers(collection_id, query, k * 3);

            let mut results: Vec<SearchResult> = candidates.into_iter()
                .filter_map(|c| {
                    let original = fetch_original_vector(c.id)?;
                    let exact_distance = distance(query, &original);
                    Some(SearchResult {
                        id: c.id,
                        distance: exact_distance,
                        tier: c.tier,
                        is_exact: true,
                    })
                })
                .collect();

            results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
            results.truncate(k);
            results
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExactnessMode {
    Fast,
    Balanced,
    Exact,
}

impl Tier {
    fn is_exact(&self) -> bool {
        matches!(self, Tier::Hot | Tier::Warm)
    }
}
```

### User-Facing Documentation

```sql
-- Help users understand tier behavior
COMMENT ON TYPE ruvector.exactness_mode IS '
Search exactness mode controls distance calculation accuracy:

fast:
  - Uses compressed representation directly
  - Fastest queries, lowest recall
  - Best for: exploratory search, suggestions

balanced (DEFAULT):
  - Approximate search, then rerank top candidates
  - Good balance of speed and accuracy
  - Best for: most production workloads

exact:
  - Always uses original float32 vectors for scoring
  - Highest recall, slowest queries
  - Best for: precision-critical applications
';
```

---

## Architecture

### Tier Hierarchy

```
+------------------------------------------------------------------+
|                           HOT TIER                                |
|  - Full precision (f32)                                          |
|  - In-memory index                                                |
|  - Last access < 24 hours (configurable)                         |
+------------------------------------------------------------------+
                              |
                              | Demote after threshold
                              v
+------------------------------------------------------------------+
|                          WARM TIER                                |
|  - Scalar quantization (SQ8 - int8)                              |
|  - 4x compression                                                 |
|  - Last access 24h - 7 days                                      |
+------------------------------------------------------------------+
                              |
                              v
+------------------------------------------------------------------+
|                          COOL TIER                                |
|  - Product quantization (PQ16)                                   |
|  - 16x compression                                                |
|  - Last access 7 - 30 days                                       |
+------------------------------------------------------------------+
                              |
                              v
+------------------------------------------------------------------+
|                          COLD TIER                                |
|  - Product quantization (PQ32/64)                                |
|  - 32-64x compression                                            |
|  - Last access > 30 days                                         |
+------------------------------------------------------------------+
```

### Data Flow

```
                    +---------------+
                    |   Query/Insert|
                    +-------+-------+
                            |
                            v
                    +-------+-------+
                    | Access Counter|  <-- Increment on every access
                    |    Update     |
                    +-------+-------+
                            |
              +-------------+-------------+
              |                           |
              v                           v
    +---------+---------+       +---------+---------+
    |   Hot Tier Search |       |  Warm/Cool/Cold   |
    |   (Full Precision)|       |  Decompress+Search|
    +-------------------+       +-------------------+
                            |
                            v (if accessed)
                    +-------+-------+
                    |   Promote to  |  <-- Background async
                    |   Hotter Tier |
                    +---------------+

    Background Worker (periodic):
    +-----------------------------------------------------------+
    | 1. Scan access counters                                    |
    | 2. Identify vectors for demotion (low access, hot tier)   |
    | 3. Compress and move to appropriate tier                   |
    | 4. Update tier statistics                                  |
    +-----------------------------------------------------------+
```

---

## Deliverables

### 1. Access Counter Infrastructure

```rust
// src/tiering/access_counter.rs

use pgrx::prelude::*;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use dashmap::DashMap;

/// Per-collection access counters in shared memory
pub struct AccessCounterStore {
    /// Collection ID -> Counter map
    counters: DashMap<(i32, TupleId), AccessCounter>,
    /// Configuration
    config: TierConfig,
}

#[derive(Debug, Default)]
pub struct AccessCounter {
    /// Total access count
    pub count: AtomicU32,
    /// Last access timestamp (epoch seconds)
    pub last_access: AtomicU64,
    /// Current tier
    pub tier: AtomicU8,
}

impl AccessCounter {
    /// Record an access
    pub fn record_access(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_access.store(now, Ordering::Relaxed);
    }

    /// Get hours since last access
    pub fn hours_since_access(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let last = self.last_access.load(Ordering::Relaxed);
        (now - last) / 3600
    }

    /// Get current tier
    pub fn current_tier(&self) -> Tier {
        match self.tier.load(Ordering::Relaxed) {
            0 => Tier::Hot,
            1 => Tier::Warm,
            2 => Tier::Cool,
            _ => Tier::Cold,
        }
    }
}

impl AccessCounterStore {
    /// Create new store with configuration
    pub fn new(config: TierConfig) -> Self {
        Self {
            counters: DashMap::new(),
            config,
        }
    }

    /// Record access for a vector
    pub fn record_access(&self, collection_id: i32, tid: TupleId) {
        let key = (collection_id, tid);
        self.counters
            .entry(key)
            .or_default()
            .record_access();
    }

    /// Get vectors needing demotion
    pub fn get_demotion_candidates(&self, collection_id: i32) -> Vec<DemotionCandidate> {
        let mut candidates = Vec::new();

        for entry in self.counters.iter() {
            let ((coll_id, tid), counter) = entry.pair();

            if *coll_id != collection_id {
                continue;
            }

            let hours = counter.hours_since_access();
            let current_tier = counter.current_tier();
            let target_tier = self.config.tier_for_age(hours);

            if target_tier > current_tier {
                candidates.push(DemotionCandidate {
                    tid: *tid,
                    current_tier,
                    target_tier,
                    hours_since_access: hours,
                    access_count: counter.count.load(Ordering::Relaxed),
                });
            }
        }

        candidates
    }

    /// Get vectors needing promotion (just accessed in cold tier)
    pub fn get_promotion_candidates(&self, collection_id: i32) -> Vec<PromotionCandidate> {
        let mut candidates = Vec::new();

        for entry in self.counters.iter() {
            let ((coll_id, tid), counter) = entry.pair();

            if *coll_id != collection_id {
                continue;
            }

            let hours = counter.hours_since_access();
            let current_tier = counter.current_tier();

            // Recently accessed but in cold tier
            if current_tier != Tier::Hot && hours < self.config.promotion_threshold_hours {
                candidates.push(PromotionCandidate {
                    tid: *tid,
                    current_tier,
                    target_tier: Tier::Hot,
                    recent_access_count: counter.count.load(Ordering::Relaxed),
                });
            }
        }

        candidates
    }

    /// Persist counters to database
    pub fn persist(&self, collection_id: i32) -> Result<(), Error> {
        Spi::run(|client| {
            for entry in self.counters.iter() {
                let ((coll_id, tid), counter) = entry.pair();

                if *coll_id != collection_id {
                    continue;
                }

                client.update(
                    "INSERT INTO ruvector.access_counters
                     (collection_id, vector_tid, access_count, last_access, current_tier)
                     VALUES ($1, $2, $3, to_timestamp($4), $5)
                     ON CONFLICT (collection_id, vector_tid) DO UPDATE SET
                         access_count = EXCLUDED.access_count,
                         last_access = EXCLUDED.last_access,
                         current_tier = EXCLUDED.current_tier",
                    None,
                    &[
                        (*coll_id).into(),
                        format!("({},{})", tid.block, tid.offset).into(),
                        counter.count.load(Ordering::Relaxed).into(),
                        counter.last_access.load(Ordering::Relaxed).into(),
                        counter.tier.load(Ordering::Relaxed).to_string().into(),
                    ],
                )?;
            }
            Ok(())
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Hot = 0,
    Warm = 1,
    Cool = 2,
    Cold = 3,
}

#[derive(Debug, Clone)]
pub struct TierConfig {
    pub hot_threshold_hours: u64,    // 0 (always start hot)
    pub warm_threshold_hours: u64,   // 24
    pub cool_threshold_hours: u64,   // 168 (7 days)
    pub cold_threshold_hours: u64,   // 720 (30 days)
    pub promotion_threshold_hours: u64, // 1 (promote if accessed within 1 hour)
}

impl Default for TierConfig {
    fn default() -> Self {
        Self {
            hot_threshold_hours: 0,
            warm_threshold_hours: 24,
            cool_threshold_hours: 168,
            cold_threshold_hours: 720,
            promotion_threshold_hours: 1,
        }
    }
}

impl TierConfig {
    pub fn tier_for_age(&self, hours: u64) -> Tier {
        if hours < self.warm_threshold_hours {
            Tier::Hot
        } else if hours < self.cool_threshold_hours {
            Tier::Warm
        } else if hours < self.cold_threshold_hours {
            Tier::Cool
        } else {
            Tier::Cold
        }
    }
}
```

### 2. Compression Module

```rust
// src/tiering/compression.rs

/// Scalar Quantization (SQ8)
/// Compresses f32 to i8: 4x compression
pub struct ScalarQuantizer {
    /// Minimum value for scaling
    min_val: f32,
    /// Maximum value for scaling
    max_val: f32,
    /// Scale factor: (max - min) / 255
    scale: f32,
}

impl ScalarQuantizer {
    /// Learn quantization parameters from data
    pub fn fit(vectors: &[&[f32]]) -> Self {
        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;

        for vec in vectors {
            for &val in *vec {
                min_val = min_val.min(val);
                max_val = max_val.max(val);
            }
        }

        let range = max_val - min_val;
        let scale = if range > 0.0 { range / 255.0 } else { 1.0 };

        Self { min_val, max_val, scale }
    }

    /// Quantize a vector to i8
    pub fn quantize(&self, vector: &[f32]) -> Vec<i8> {
        vector.iter()
            .map(|&val| {
                let normalized = (val - self.min_val) / self.scale;
                (normalized.clamp(0.0, 255.0) as i8).wrapping_sub(128)
            })
            .collect()
    }

    /// Dequantize back to f32
    pub fn dequantize(&self, quantized: &[i8]) -> Vec<f32> {
        quantized.iter()
            .map(|&val| {
                let normalized = (val.wrapping_add(128)) as f32;
                normalized * self.scale + self.min_val
            })
            .collect()
    }

    /// Compute approximate L2 distance using quantized vectors
    pub fn distance_sq8(&self, a: &[i8], b: &[i8]) -> f32 {
        let sum: i32 = a.iter().zip(b.iter())
            .map(|(&x, &y)| {
                let diff = (x as i32) - (y as i32);
                diff * diff
            })
            .sum();

        (sum as f32 * self.scale * self.scale).sqrt()
    }

    /// Serialize for storage
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12);
        bytes.extend_from_slice(&self.min_val.to_le_bytes());
        bytes.extend_from_slice(&self.max_val.to_le_bytes());
        bytes.extend_from_slice(&self.scale.to_le_bytes());
        bytes
    }

    /// Deserialize from storage
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let min_val = f32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let max_val = f32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let scale = f32::from_le_bytes(bytes[8..12].try_into().unwrap());
        Self { min_val, max_val, scale }
    }
}

/// Product Quantization (PQ)
/// Compresses vectors by dividing into subspaces
pub struct ProductQuantizer {
    /// Number of subspaces
    m: usize,
    /// Bits per subspace (typically 8)
    bits: usize,
    /// Codebooks: m codebooks, each with 2^bits centroids
    codebooks: Vec<Vec<Vec<f32>>>,
    /// Original dimensions
    dims: usize,
    /// Dimensions per subspace
    dims_per_subspace: usize,
}

impl ProductQuantizer {
    /// Create new PQ with m subspaces
    pub fn new(dims: usize, m: usize, bits: usize) -> Self {
        assert!(dims % m == 0, "Dimensions must be divisible by m");

        Self {
            m,
            bits,
            codebooks: Vec::new(),
            dims,
            dims_per_subspace: dims / m,
        }
    }

    /// Train PQ codebooks using k-means
    pub fn fit(&mut self, vectors: &[&[f32]], iterations: usize) {
        let k = 1 << self.bits;  // Number of centroids per subspace
        self.codebooks.clear();

        for subspace in 0..self.m {
            let start = subspace * self.dims_per_subspace;
            let end = start + self.dims_per_subspace;

            // Extract subvectors
            let subvectors: Vec<Vec<f32>> = vectors.iter()
                .map(|v| v[start..end].to_vec())
                .collect();

            // Run k-means
            let centroids = kmeans(&subvectors, k, iterations);
            self.codebooks.push(centroids);
        }
    }

    /// Encode a vector to PQ codes
    pub fn encode(&self, vector: &[f32]) -> Vec<u8> {
        let mut codes = Vec::with_capacity(self.m);

        for (subspace, codebook) in self.codebooks.iter().enumerate() {
            let start = subspace * self.dims_per_subspace;
            let end = start + self.dims_per_subspace;
            let subvec = &vector[start..end];

            // Find nearest centroid
            let mut best_idx = 0;
            let mut best_dist = f32::MAX;

            for (idx, centroid) in codebook.iter().enumerate() {
                let dist = l2_distance_squared(subvec, centroid);
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = idx;
                }
            }

            codes.push(best_idx as u8);
        }

        codes
    }

    /// Decode PQ codes back to approximate vector
    pub fn decode(&self, codes: &[u8]) -> Vec<f32> {
        let mut vector = Vec::with_capacity(self.dims);

        for (subspace, &code) in codes.iter().enumerate() {
            let centroid = &self.codebooks[subspace][code as usize];
            vector.extend_from_slice(centroid);
        }

        vector
    }

    /// Compute asymmetric distance (query vs PQ code)
    /// This is more accurate than symmetric distance
    pub fn asymmetric_distance(&self, query: &[f32], codes: &[u8]) -> f32 {
        let mut dist = 0.0;

        for (subspace, &code) in codes.iter().enumerate() {
            let start = subspace * self.dims_per_subspace;
            let end = start + self.dims_per_subspace;
            let query_sub = &query[start..end];
            let centroid = &self.codebooks[subspace][code as usize];

            dist += l2_distance_squared(query_sub, centroid);
        }

        dist.sqrt()
    }

    /// Precompute distance table for faster search
    pub fn compute_distance_table(&self, query: &[f32]) -> Vec<Vec<f32>> {
        let k = 1 << self.bits;
        let mut table = vec![vec![0.0; k]; self.m];

        for (subspace, codebook) in self.codebooks.iter().enumerate() {
            let start = subspace * self.dims_per_subspace;
            let end = start + self.dims_per_subspace;
            let query_sub = &query[start..end];

            for (idx, centroid) in codebook.iter().enumerate() {
                table[subspace][idx] = l2_distance_squared(query_sub, centroid);
            }
        }

        table
    }

    /// Fast distance lookup using precomputed table
    pub fn table_distance(&self, table: &[Vec<f32>], codes: &[u8]) -> f32 {
        let mut dist = 0.0;
        for (subspace, &code) in codes.iter().enumerate() {
            dist += table[subspace][code as usize];
        }
        dist.sqrt()
    }

    /// Compression ratio
    pub fn compression_ratio(&self) -> f32 {
        let original_bytes = self.dims * 4;  // f32
        let compressed_bytes = self.m;  // u8 per subspace
        original_bytes as f32 / compressed_bytes as f32
    }

    /// Serialize for storage
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Header
        bytes.extend_from_slice(&(self.m as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.bits as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.dims as u32).to_le_bytes());

        // Codebooks
        for codebook in &self.codebooks {
            for centroid in codebook {
                for &val in centroid {
                    bytes.extend_from_slice(&val.to_le_bytes());
                }
            }
        }

        bytes
    }

    /// Deserialize from storage
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let m = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let bits = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let dims = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let dims_per_subspace = dims / m;
        let k = 1 << bits;

        let mut codebooks = Vec::with_capacity(m);
        let mut offset = 12;

        for _ in 0..m {
            let mut codebook = Vec::with_capacity(k);
            for _ in 0..k {
                let mut centroid = Vec::with_capacity(dims_per_subspace);
                for _ in 0..dims_per_subspace {
                    let val = f32::from_le_bytes(
                        bytes[offset..offset+4].try_into().unwrap()
                    );
                    centroid.push(val);
                    offset += 4;
                }
                codebook.push(centroid);
            }
            codebooks.push(codebook);
        }

        Self { m, bits, codebooks, dims, dims_per_subspace }
    }
}

fn l2_distance_squared(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter())
        .map(|(x, y)| {
            let diff = x - y;
            diff * diff
        })
        .sum()
}

fn kmeans(data: &[Vec<f32>], k: usize, iterations: usize) -> Vec<Vec<f32>> {
    if data.is_empty() {
        return Vec::new();
    }

    let dims = data[0].len();
    let mut rng = rand::thread_rng();

    // Initialize centroids randomly
    let mut centroids: Vec<Vec<f32>> = data
        .choose_multiple(&mut rng, k.min(data.len()))
        .cloned()
        .collect();

    while centroids.len() < k {
        centroids.push(vec![0.0; dims]);
    }

    for _ in 0..iterations {
        // Assign points to clusters
        let mut assignments = vec![Vec::new(); k];
        for point in data {
            let mut best_cluster = 0;
            let mut best_dist = f32::MAX;

            for (idx, centroid) in centroids.iter().enumerate() {
                let dist = l2_distance_squared(point, centroid);
                if dist < best_dist {
                    best_dist = dist;
                    best_cluster = idx;
                }
            }

            assignments[best_cluster].push(point.clone());
        }

        // Update centroids
        for (idx, cluster) in assignments.iter().enumerate() {
            if cluster.is_empty() {
                continue;
            }

            let mut new_centroid = vec![0.0; dims];
            for point in cluster {
                for (i, &val) in point.iter().enumerate() {
                    new_centroid[i] += val;
                }
            }
            for val in &mut new_centroid {
                *val /= cluster.len() as f32;
            }
            centroids[idx] = new_centroid;
        }
    }

    centroids
}
```

### 3. Tier Manager

```rust
// src/tiering/manager.rs

/// Tier manager coordinates tier operations
pub struct TierManager {
    /// Access counter store
    access_store: Arc<AccessCounterStore>,
    /// Compression codebooks per collection
    codebooks: DashMap<i32, CompressionCodebooks>,
    /// Configuration
    config: TierManagerConfig,
}

#[derive(Debug, Clone)]
pub struct TierManagerConfig {
    /// Maximum vectors to demote per cycle
    pub max_demotions_per_cycle: usize,
    /// Maximum vectors to promote per cycle
    pub max_promotions_per_cycle: usize,
    /// Batch size for compression operations
    pub compression_batch_size: usize,
    /// Enable async promotion (background)
    pub async_promotion: bool,
}

impl Default for TierManagerConfig {
    fn default() -> Self {
        Self {
            max_demotions_per_cycle: 10000,
            max_promotions_per_cycle: 1000,
            compression_batch_size: 100,
            async_promotion: true,
        }
    }
}

#[derive(Debug, Clone)]
struct CompressionCodebooks {
    sq8: Option<ScalarQuantizer>,
    pq16: Option<ProductQuantizer>,
    pq32: Option<ProductQuantizer>,
}

impl TierManager {
    /// Create new tier manager
    pub fn new(
        access_store: Arc<AccessCounterStore>,
        config: TierManagerConfig,
    ) -> Self {
        Self {
            access_store,
            codebooks: DashMap::new(),
            config,
        }
    }

    /// Process tier management for a collection
    pub fn process_collection(&self, collection_id: i32) -> TierReport {
        let mut report = TierReport::default();

        // Get demotion candidates
        let demotions = self.access_store.get_demotion_candidates(collection_id);
        let demotions = demotions.into_iter()
            .take(self.config.max_demotions_per_cycle)
            .collect::<Vec<_>>();

        report.demotion_candidates = demotions.len();

        // Process demotions in batches
        for batch in demotions.chunks(self.config.compression_batch_size) {
            match self.process_demotions(collection_id, batch) {
                Ok(count) => report.demotions_completed += count,
                Err(e) => report.errors.push(format!("Demotion error: {}", e)),
            }
        }

        // Get promotion candidates
        let promotions = self.access_store.get_promotion_candidates(collection_id);
        let promotions = promotions.into_iter()
            .take(self.config.max_promotions_per_cycle)
            .collect::<Vec<_>>();

        report.promotion_candidates = promotions.len();

        // Process promotions
        for batch in promotions.chunks(self.config.compression_batch_size) {
            match self.process_promotions(collection_id, batch) {
                Ok(count) => report.promotions_completed += count,
                Err(e) => report.errors.push(format!("Promotion error: {}", e)),
            }
        }

        // Update statistics
        if let Err(e) = self.update_tier_stats(collection_id) {
            report.errors.push(format!("Stats update error: {}", e));
        }

        report
    }

    /// Process demotion batch
    fn process_demotions(
        &self,
        collection_id: i32,
        candidates: &[DemotionCandidate],
    ) -> Result<usize, Error> {
        let mut completed = 0;

        // Group by target tier
        let mut by_tier: HashMap<Tier, Vec<_>> = HashMap::new();
        for candidate in candidates {
            by_tier.entry(candidate.target_tier)
                .or_default()
                .push(candidate);
        }

        // Get or create codebooks
        let codebooks = self.get_or_create_codebooks(collection_id)?;

        for (tier, tier_candidates) in by_tier {
            // Fetch vectors
            let vectors = self.fetch_vectors(collection_id, &tier_candidates)?;

            // Compress based on tier
            let compressed = match tier {
                Tier::Warm => {
                    let sq8 = codebooks.sq8.as_ref()
                        .ok_or_else(|| Error::MissingCodebook("sq8"))?;
                    vectors.iter()
                        .map(|(tid, vec)| (*tid, CompressedVector::Sq8(sq8.quantize(vec))))
                        .collect()
                }
                Tier::Cool => {
                    let pq16 = codebooks.pq16.as_ref()
                        .ok_or_else(|| Error::MissingCodebook("pq16"))?;
                    vectors.iter()
                        .map(|(tid, vec)| (*tid, CompressedVector::Pq16(pq16.encode(vec))))
                        .collect()
                }
                Tier::Cold => {
                    let pq32 = codebooks.pq32.as_ref()
                        .ok_or_else(|| Error::MissingCodebook("pq32"))?;
                    vectors.iter()
                        .map(|(tid, vec)| (*tid, CompressedVector::Pq32(pq32.encode(vec))))
                        .collect()
                }
                _ => continue,
            };

            // Store compressed vectors
            self.store_compressed(collection_id, tier, &compressed)?;

            // Update access counters
            for (tid, _) in &compressed {
                self.access_store.update_tier(collection_id, *tid, tier);
            }

            completed += compressed.len();
        }

        Ok(completed)
    }

    /// Process promotion batch (decompress and move to hot tier)
    fn process_promotions(
        &self,
        collection_id: i32,
        candidates: &[PromotionCandidate],
    ) -> Result<usize, Error> {
        let codebooks = self.get_or_create_codebooks(collection_id)?;
        let mut completed = 0;

        for candidate in candidates {
            // Fetch compressed vector
            let compressed = self.fetch_compressed(collection_id, candidate.tid)?;

            // Decompress
            let vector = match compressed {
                CompressedVector::Sq8(data) => {
                    let sq8 = codebooks.sq8.as_ref()
                        .ok_or_else(|| Error::MissingCodebook("sq8"))?;
                    sq8.dequantize(&data)
                }
                CompressedVector::Pq16(codes) => {
                    let pq16 = codebooks.pq16.as_ref()
                        .ok_or_else(|| Error::MissingCodebook("pq16"))?;
                    pq16.decode(&codes)
                }
                CompressedVector::Pq32(codes) => {
                    let pq32 = codebooks.pq32.as_ref()
                        .ok_or_else(|| Error::MissingCodebook("pq32"))?;
                    pq32.decode(&codes)
                }
                CompressedVector::Full(_) => continue,  // Already hot
            };

            // Store in hot tier
            self.store_hot(collection_id, candidate.tid, &vector)?;

            // Update access counter
            self.access_store.update_tier(collection_id, candidate.tid, Tier::Hot);

            completed += 1;
        }

        Ok(completed)
    }

    /// Get or create compression codebooks for a collection
    fn get_or_create_codebooks(&self, collection_id: i32) -> Result<CompressionCodebooks, Error> {
        if let Some(cb) = self.codebooks.get(&collection_id) {
            return Ok(cb.clone());
        }

        // Try to load from database
        if let Some(cb) = self.load_codebooks(collection_id)? {
            self.codebooks.insert(collection_id, cb.clone());
            return Ok(cb);
        }

        // Train new codebooks
        let cb = self.train_codebooks(collection_id)?;
        self.save_codebooks(collection_id, &cb)?;
        self.codebooks.insert(collection_id, cb.clone());

        Ok(cb)
    }

    /// Train compression codebooks from collection data
    fn train_codebooks(&self, collection_id: i32) -> Result<CompressionCodebooks, Error> {
        // Sample vectors for training
        let sample_size = 10000;
        let vectors = self.sample_vectors(collection_id, sample_size)?;

        if vectors.is_empty() {
            return Err(Error::EmptyCollection);
        }

        let dims = vectors[0].len();
        let vector_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();

        // Train SQ8
        let sq8 = ScalarQuantizer::fit(&vector_refs);

        // Train PQ16 (16 subspaces)
        let m16 = if dims >= 16 { 16 } else { dims };
        let mut pq16 = ProductQuantizer::new(dims, m16, 8);
        pq16.fit(&vector_refs, 20);

        // Train PQ32 (32 subspaces)
        let m32 = if dims >= 32 { 32 } else { dims };
        let mut pq32 = ProductQuantizer::new(dims, m32, 8);
        pq32.fit(&vector_refs, 20);

        Ok(CompressionCodebooks {
            sq8: Some(sq8),
            pq16: Some(pq16),
            pq32: Some(pq32),
        })
    }

    /// Update tier statistics
    fn update_tier_stats(&self, collection_id: i32) -> Result<(), Error> {
        Spi::run(|client| {
            // Count vectors per tier
            for tier in [Tier::Hot, Tier::Warm, Tier::Cool, Tier::Cold] {
                let tier_name = tier.to_string();

                client.update(
                    "INSERT INTO ruvector.tier_stats
                     (collection_id, tier_name, vector_count, size_bytes, snapshot_time)
                     SELECT
                         $1,
                         $2,
                         COUNT(*),
                         SUM(pg_column_size(vector_tid)),
                         NOW()
                     FROM ruvector.access_counters
                     WHERE collection_id = $1 AND current_tier = $2
                     ON CONFLICT (collection_id, tier_name, snapshot_time)
                     DO UPDATE SET
                         vector_count = EXCLUDED.vector_count,
                         size_bytes = EXCLUDED.size_bytes",
                    None,
                    &[collection_id.into(), tier_name.into()],
                )?;
            }
            Ok(())
        })
    }
}

#[derive(Debug, Default)]
pub struct TierReport {
    pub demotion_candidates: usize,
    pub demotions_completed: usize,
    pub promotion_candidates: usize,
    pub promotions_completed: usize,
    pub errors: Vec<String>,
}

enum CompressedVector {
    Full(Vec<f32>),
    Sq8(Vec<i8>),
    Pq16(Vec<u8>),
    Pq32(Vec<u8>),
}
```

### 4. Background Compactor Worker

```rust
// src/tiering/compactor.rs

/// Background worker for tier compaction
#[pg_guard]
pub extern "C" fn ruvector_compactor_worker_main(_arg: pg_sys::Datum) {
    pgrx::log!("RuVector compactor worker starting");

    let config = CompactorConfig::default();
    let tier_manager = TierManager::new(
        get_access_store(),
        TierManagerConfig::default(),
    );

    loop {
        if unsafe { pg_sys::ShutdownRequestPending } {
            break;
        }

        // Get collections needing tier management
        let collections = match get_collections_for_tiering() {
            Ok(c) => c,
            Err(e) => {
                pgrx::warning!("Failed to get collections: {}", e);
                sleep_interruptible(config.interval_secs);
                continue;
            }
        };

        for collection in collections {
            // Check integrity gate
            let gate = check_integrity_gate(collection.id, "compression");
            if !gate.allowed {
                pgrx::debug1!(
                    "Skipping tier management for {}: {}",
                    collection.name,
                    gate.reason.unwrap_or_default()
                );
                continue;
            }

            // Process collection
            let report = tier_manager.process_collection(collection.id);

            pgrx::log!(
                "Tier management for {}: {} demotions, {} promotions, {} errors",
                collection.name,
                report.demotions_completed,
                report.promotions_completed,
                report.errors.len()
            );

            for error in &report.errors {
                pgrx::warning!("Tier error: {}", error);
            }
        }

        sleep_interruptible(config.interval_secs);
    }

    pgrx::log!("RuVector compactor worker stopped");
}

#[derive(Debug, Clone)]
struct CompactorConfig {
    /// Interval between compaction cycles
    interval_secs: u64,
}

impl Default for CompactorConfig {
    fn default() -> Self {
        Self {
            interval_secs: 3600,  // 1 hour
        }
    }
}

fn get_collections_for_tiering() -> Result<Vec<CollectionInfo>, Error> {
    Spi::connect(|client| {
        client.select(
            "SELECT c.id, c.name
             FROM ruvector.collections c
             JOIN ruvector.tier_policies tp ON c.id = tp.collection_id
             WHERE tp.enabled = true
             GROUP BY c.id",
            None,
            &[],
        )?.map(|row| {
            Ok(CollectionInfo {
                id: row.get::<i32>(1)?,
                name: row.get::<String>(2)?,
            })
        }).collect()
    })
}
```

### 5. SQL Functions

```sql
-- Configure tier thresholds
CREATE FUNCTION ruvector_set_tiers(
    p_collection_name TEXT,
    p_warm_hours INTEGER DEFAULT 24,
    p_cool_hours INTEGER DEFAULT 168,
    p_cold_hours INTEGER DEFAULT 720
) RETURNS BOOLEAN AS $$
DECLARE
    v_collection_id INTEGER;
BEGIN
    SELECT id INTO v_collection_id
    FROM ruvector.collections WHERE name = p_collection_name;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Collection not found: %', p_collection_name;
    END IF;

    -- Update tier policies
    INSERT INTO ruvector.tier_policies
        (collection_id, tier_name, threshold_hours, enabled)
    VALUES
        (v_collection_id, 'warm', p_warm_hours, true),
        (v_collection_id, 'cool', p_cool_hours, true),
        (v_collection_id, 'cold', p_cold_hours, true)
    ON CONFLICT (collection_id, tier_name) DO UPDATE SET
        threshold_hours = EXCLUDED.threshold_hours,
        enabled = true;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Set compression method per tier
CREATE FUNCTION ruvector_set_compression(
    p_collection_name TEXT,
    p_tier TEXT,
    p_compression TEXT
) RETURNS BOOLEAN AS $$
BEGIN
    UPDATE ruvector.tier_policies
    SET compression = p_compression
    WHERE collection_id = (SELECT id FROM ruvector.collections WHERE name = p_collection_name)
      AND tier_name = p_tier;

    RETURN FOUND;
END;
$$ LANGUAGE plpgsql;

-- Trigger manual compaction
CREATE FUNCTION ruvector_compact(p_collection_name TEXT)
RETURNS JSONB AS 'MODULE_PATHNAME', 'ruvector_compact' LANGUAGE C;

-- Get tier report
CREATE FUNCTION ruvector_tier_report(p_collection_name TEXT)
RETURNS TABLE (
    tier_name TEXT,
    vector_count BIGINT,
    size_mb REAL,
    compression TEXT,
    avg_age_hours REAL,
    compression_ratio REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        tp.tier_name,
        COALESCE(ts.vector_count, 0),
        COALESCE(ts.size_bytes::real / 1024 / 1024, 0),
        tp.compression,
        EXTRACT(EPOCH FROM NOW() - MAX(ac.last_access))::real / 3600,
        CASE tp.compression
            WHEN 'sq8' THEN 4.0
            WHEN 'pq16' THEN 16.0
            WHEN 'pq32' THEN 32.0
            ELSE 1.0
        END
    FROM ruvector.tier_policies tp
    JOIN ruvector.collections c ON tp.collection_id = c.id
    LEFT JOIN ruvector.tier_stats ts ON tp.collection_id = ts.collection_id
                                    AND tp.tier_name = ts.tier_name
    LEFT JOIN ruvector.access_counters ac ON tp.collection_id = ac.collection_id
                                         AND ac.current_tier = tp.tier_name
    WHERE c.name = p_collection_name
    GROUP BY tp.tier_name, ts.vector_count, ts.size_bytes, tp.compression
    ORDER BY CASE tp.tier_name
        WHEN 'hot' THEN 1
        WHEN 'warm' THEN 2
        WHEN 'cool' THEN 3
        WHEN 'cold' THEN 4
    END;
END;
$$ LANGUAGE plpgsql;

-- Get detailed tier statistics
CREATE FUNCTION ruvector_tier_stats(p_collection_name TEXT)
RETURNS JSONB AS $$
DECLARE
    v_result JSONB;
BEGIN
    SELECT jsonb_build_object(
        'collection', p_collection_name,
        'total_vectors', SUM(ts.vector_count),
        'total_size_mb', SUM(ts.size_bytes)::real / 1024 / 1024,
        'tiers', jsonb_agg(jsonb_build_object(
            'name', tp.tier_name,
            'vector_count', COALESCE(ts.vector_count, 0),
            'size_mb', COALESCE(ts.size_bytes::real / 1024 / 1024, 0),
            'compression', tp.compression,
            'threshold_hours', tp.threshold_hours,
            'enabled', tp.enabled
        ) ORDER BY CASE tp.tier_name
            WHEN 'hot' THEN 1 WHEN 'warm' THEN 2
            WHEN 'cool' THEN 3 WHEN 'cold' THEN 4
        END)
    ) INTO v_result
    FROM ruvector.tier_policies tp
    JOIN ruvector.collections c ON tp.collection_id = c.id
    LEFT JOIN ruvector.tier_stats ts ON tp.collection_id = ts.collection_id
                                    AND tp.tier_name = ts.tier_name
    WHERE c.name = p_collection_name
    GROUP BY c.name;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

-- Force promote a vector to hot tier
CREATE FUNCTION ruvector_promote_vector(
    p_collection_name TEXT,
    p_vector_id TEXT
) RETURNS BOOLEAN AS 'MODULE_PATHNAME', 'ruvector_promote_vector' LANGUAGE C;

-- Retrain compression codebooks
CREATE FUNCTION ruvector_retrain_compression(
    p_collection_name TEXT,
    p_sample_size INTEGER DEFAULT 10000
) RETURNS JSONB AS 'MODULE_PATHNAME', 'ruvector_retrain_compression' LANGUAGE C;
```

---

## Storage Schema

```sql
-- Compressed vector storage
CREATE TABLE ruvector.compressed_vectors (
    collection_id   INTEGER NOT NULL,
    vector_tid      TID NOT NULL,
    tier            TEXT NOT NULL,
    compression     TEXT NOT NULL,
    data            BYTEA NOT NULL,
    original_dims   INTEGER NOT NULL,

    PRIMARY KEY (collection_id, vector_tid)
) PARTITION BY LIST (collection_id);

-- Compression codebooks
CREATE TABLE ruvector.compression_codebooks (
    collection_id   INTEGER PRIMARY KEY REFERENCES ruvector.collections(id),
    sq8_params      BYTEA,
    pq16_codebook   BYTEA,
    pq32_codebook   BYTEA,
    trained_on      INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

---

## Testing Requirements

### Unit Tests
- SQ8 quantization accuracy
- PQ encode/decode roundtrip
- Distance approximation error bounds
- Access counter operations

### Integration Tests
- Full demotion cycle
- Full promotion cycle
- Compressed search accuracy
- Background worker behavior

### Performance Tests
- Compression throughput
- Decompression latency
- Storage savings measurement
- Query latency by tier

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| rand | Random sampling |
| dashmap | Concurrent hash map |
| parking_lot | Synchronization |

---

## Timeline

| Week | Deliverable |
|------|-------------|
| 5 | Access counter infrastructure |
| 6 | SQ8 and PQ compression |
| 7 | Tier manager and background worker |
| 8 | SQL functions and testing |
