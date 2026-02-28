# RuVector Postgres v2 - Index Access Methods Specification

## Overview

This document specifies the PostgreSQL Index Access Method (AM) implementation for RuVector v2, including HNSW and IVFFlat indexes. The AM layer provides the bridge between PostgreSQL's query executor and the RuVector engine.

---

## Index AM Architecture

### Component Diagram

```
+------------------------------------------------------------------+
|                        PostgreSQL Planner                         |
|  - Recognizes ORDER BY distance LIMIT k pattern                   |
|  - Selects index scan based on cost estimation                    |
+----------------------------------+-------------------------------+
                                   |
                                   v
+------------------------------------------------------------------+
|                        Index AM Handler                           |
|  - hnsw_handler() / ivfflat_handler()                            |
|  - Returns IndexAmRoutine with callbacks                          |
+----------------------------------+-------------------------------+
                                   |
     +-----------------------------+-----------------------------+
     |                             |                             |
     v                             v                             v
+---------+                 +-----------+                 +---------+
| ambuild |                 | amgettuple|                 | aminsert|
| Build   |                 | Scan      |                 | Insert  |
+---------+                 +-----------+                 +---------+
     |                             |                             |
     +-----------------------------+-----------------------------+
                                   |
                                   v
+------------------------------------------------------------------+
|                     Shared Memory Queue                           |
|  - Submits work items to Engine Worker                           |
|  - Receives results via result queue                             |
+------------------------------------------------------------------+
                                   |
                                   v
+------------------------------------------------------------------+
|                     RuVector Engine Worker                        |
|  - Executes actual HNSW/IVFFlat operations                       |
|  - Returns TIDs and distances                                     |
+------------------------------------------------------------------+
```

---

## 1. Index AM Handler

### HNSW Handler

```rust
/// HNSW index access method handler
#[pg_extern]
fn hnsw_handler(_index_oid: pg_sys::Oid) -> pg_sys::IndexAmRoutine {
    let mut amroutine = pg_sys::IndexAmRoutine::default();

    // Identification
    amroutine.type_ = pg_sys::NodeTag::T_IndexAmRoutine;
    amroutine.amstrategies = 1;  // One strategy (distance ordering)
    amroutine.amsupport = 1;     // One support function (distance)
    amroutine.amoptsprocnum = 0; // No optimizer support function

    // AM capabilities
    amroutine.amcanorder = true;           // Support ORDER BY
    amroutine.amcanorderbyop = true;       // Support ORDER BY operator
    amroutine.amcanbackward = false;       // No backward scans
    amroutine.amcanunique = false;         // Not for unique constraints
    amroutine.amcanmulticol = false;       // Single column only
    amroutine.amoptionalkey = true;        // Query doesn't require key
    amroutine.amsearcharray = false;       // No array search
    amroutine.amsearchnulls = false;       // No null search
    amroutine.amstorage = true;            // Custom storage format
    amroutine.amclusterable = false;       // Not clusterable
    amroutine.ampredlocks = false;         // No predicate locking
    amroutine.amcanparallel = true;        // Supports parallel scan
    amroutine.amcaninclude = false;        // No INCLUDE columns
    amroutine.amusemaintenanceworkmem = true; // Use maintenance_work_mem

    // Parallel settings
    amroutine.amparallelvacuumoptions =
        pg_sys::VACUUM_OPTION_PARALLEL_COND_CLEANUP;

    // Callbacks
    amroutine.ambuild = Some(hnsw_build);
    amroutine.ambuildempty = Some(hnsw_buildempty);
    amroutine.aminsert = Some(hnsw_insert);
    amroutine.ambulkdelete = Some(hnsw_bulkdelete);
    amroutine.amvacuumcleanup = Some(hnsw_vacuumcleanup);
    amroutine.amcanreturn = None;
    amroutine.amcostestimate = Some(hnsw_costestimate);
    amroutine.amoptions = Some(hnsw_options);
    amroutine.amproperty = Some(hnsw_property);
    amroutine.ambuildphasename = None;
    amroutine.amvalidate = Some(hnsw_validate);
    amroutine.amadjustmembers = None;
    amroutine.ambeginscan = Some(hnsw_beginscan);
    amroutine.amrescan = Some(hnsw_rescan);
    amroutine.amgettuple = Some(hnsw_gettuple);
    amroutine.amgetbitmap = None;
    amroutine.amendscan = Some(hnsw_endscan);
    amroutine.ammarkpos = None;
    amroutine.amrestrpos = None;
    amroutine.amestimateparallelscan = Some(hnsw_estimateparallelscan);
    amroutine.aminitparallelscan = Some(hnsw_initparallelscan);
    amroutine.amparallelrescan = Some(hnsw_parallelrescan);

    amroutine
}
```

### IVFFlat Handler

```rust
/// IVFFlat index access method handler
#[pg_extern]
fn ivfflat_handler(_index_oid: pg_sys::Oid) -> pg_sys::IndexAmRoutine {
    let mut amroutine = pg_sys::IndexAmRoutine::default();

    // Similar to HNSW but with IVFFlat-specific options
    amroutine.type_ = pg_sys::NodeTag::T_IndexAmRoutine;
    amroutine.amstrategies = 1;
    amroutine.amsupport = 1;

    // Capabilities (same as HNSW)
    amroutine.amcanorder = true;
    amroutine.amcanorderbyop = true;
    amroutine.amcanbackward = false;
    amroutine.amcanunique = false;
    amroutine.amcanmulticol = false;
    amroutine.amstorage = true;
    amroutine.amcanparallel = true;
    amroutine.amusemaintenanceworkmem = true;

    // IVFFlat-specific callbacks
    amroutine.ambuild = Some(ivfflat_build);
    amroutine.ambuildempty = Some(ivfflat_buildempty);
    amroutine.aminsert = Some(ivfflat_insert);
    amroutine.ambulkdelete = Some(ivfflat_bulkdelete);
    amroutine.amvacuumcleanup = Some(ivfflat_vacuumcleanup);
    amroutine.amcostestimate = Some(ivfflat_costestimate);
    amroutine.amoptions = Some(ivfflat_options);
    amroutine.amproperty = Some(ivfflat_property);
    amroutine.amvalidate = Some(ivfflat_validate);
    amroutine.ambeginscan = Some(ivfflat_beginscan);
    amroutine.amrescan = Some(ivfflat_rescan);
    amroutine.amgettuple = Some(ivfflat_gettuple);
    amroutine.amendscan = Some(ivfflat_endscan);
    amroutine.amestimateparallelscan = Some(ivfflat_estimateparallelscan);
    amroutine.aminitparallelscan = Some(ivfflat_initparallelscan);
    amroutine.amparallelrescan = Some(ivfflat_parallelrescan);

    amroutine
}
```

---

## 2. Index Options

### HNSW Options

```rust
/// HNSW index creation options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct HnswOptions {
    /// varlena header (required)
    pub vl_len_: i32,

    /// Maximum bi-directional links per node (default: 16)
    pub m: i32,

    /// Construction-time search width (default: 64)
    pub ef_construction: i32,
}

impl Default for HnswOptions {
    fn default() -> Self {
        Self {
            vl_len_: 0,  // Will be set by PostgreSQL
            m: 16,
            ef_construction: 64,
        }
    }
}

/// Parse HNSW options from CREATE INDEX
#[pg_guard]
extern "C" fn hnsw_options(
    reloptions: pg_sys::Datum,
    validate: bool,
) -> *mut pg_sys::bytea {
    // Define option spec
    static OPTIONS: &[pg_sys::relopt_parse_elt] = &[
        pg_sys::relopt_parse_elt {
            optname: c"m".as_ptr(),
            opttype: pg_sys::relopt_type::RELOPT_TYPE_INT,
            offset: offset_of!(HnswOptions, m) as i32,
        },
        pg_sys::relopt_parse_elt {
            optname: c"ef_construction".as_ptr(),
            opttype: pg_sys::relopt_type::RELOPT_TYPE_INT,
            offset: offset_of!(HnswOptions, ef_construction) as i32,
        },
    ];

    unsafe {
        pg_sys::parseRelOptions(
            reloptions,
            validate,
            pg_sys::RELOPT_KIND_INDEX as pg_sys::relopt_kind,
            &HnswOptions::default() as *const _ as *mut _,
            size_of::<HnswOptions>() as i32,
            OPTIONS.as_ptr(),
            OPTIONS.len() as i32,
        ) as *mut pg_sys::bytea
    }
}
```

### IVFFlat Options

```rust
/// IVFFlat index creation options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct IvfFlatOptions {
    pub vl_len_: i32,

    /// Number of inverted lists (default: 100)
    pub lists: i32,
}

impl Default for IvfFlatOptions {
    fn default() -> Self {
        Self {
            vl_len_: 0,
            lists: 100,
        }
    }
}

#[pg_guard]
extern "C" fn ivfflat_options(
    reloptions: pg_sys::Datum,
    validate: bool,
) -> *mut pg_sys::bytea {
    static OPTIONS: &[pg_sys::relopt_parse_elt] = &[
        pg_sys::relopt_parse_elt {
            optname: c"lists".as_ptr(),
            opttype: pg_sys::relopt_type::RELOPT_TYPE_INT,
            offset: offset_of!(IvfFlatOptions, lists) as i32,
        },
    ];

    unsafe {
        pg_sys::parseRelOptions(
            reloptions,
            validate,
            pg_sys::RELOPT_KIND_INDEX as pg_sys::relopt_kind,
            &IvfFlatOptions::default() as *const _ as *mut _,
            size_of::<IvfFlatOptions>() as i32,
            OPTIONS.as_ptr(),
            OPTIONS.len() as i32,
        ) as *mut pg_sys::bytea
    }
}
```

---

## 3. Index Build

### HNSW Build

```rust
/// Build HNSW index
#[pg_guard]
extern "C" fn hnsw_build(
    heap: pg_sys::Relation,
    index: pg_sys::Relation,
    index_info: *mut pg_sys::IndexInfo,
) -> *mut pg_sys::IndexBuildResult {
    let heap = unsafe { PgRelation::from_raw(heap) };
    let index = unsafe { PgRelation::from_raw(index) };
    let index_info = unsafe { &*index_info };

    // Get options
    let options = get_hnsw_options(&index);

    pgrx::log!(
        "Building HNSW index: m={}, ef_construction={}",
        options.m, options.ef_construction
    );

    // Prepare build request
    let build_request = BuildIndexRequest {
        index_oid: index.oid().as_u32(),
        heap_oid: heap.oid().as_u32(),
        column_no: index_info.ii_IndexAttrNumbers[0] as usize,
        index_type: "hnsw".to_string(),
        params: serde_json::json!({
            "m": options.m,
            "ef_construction": options.ef_construction,
        }),
    };

    // Check integrity gate
    let gate_result = check_integrity_gate(
        build_request.heap_oid,
        "index_build"
    );
    if !gate_result.allowed {
        pgrx::error!(
            "Index build blocked by integrity gate: {}",
            gate_result.reason.unwrap_or_default()
        );
    }

    // Submit build to engine
    let result = match submit_and_wait(
        Operation::BuildIndex(build_request),
        600_000, // 10 minute timeout
    ) {
        Ok(result) => result,
        Err(e) => pgrx::error!("HNSW build failed: {}", e),
    };

    // Create result
    let build_result = unsafe {
        let result = pg_sys::palloc0(size_of::<pg_sys::IndexBuildResult>())
            as *mut pg_sys::IndexBuildResult;
        (*result).heap_tuples = result.tuple_count as f64;
        (*result).index_tuples = result.indexed_count as f64;
        result
    };

    // Update collection metadata
    update_collection_index_oid(
        heap.oid().as_u32(),
        index.oid().as_u32()
    );

    pgrx::log!(
        "HNSW index built: {} vectors indexed",
        result.indexed_count
    );

    build_result
}

/// Build empty HNSW index (for CREATE INDEX CONCURRENTLY)
#[pg_guard]
extern "C" fn hnsw_buildempty(index: pg_sys::Relation) {
    let index = unsafe { PgRelation::from_raw(index) };
    let options = get_hnsw_options(&index);

    // Initialize empty index structure in engine
    let request = BuildIndexRequest {
        index_oid: index.oid().as_u32(),
        heap_oid: 0,  // Will be set later
        column_no: 0,
        index_type: "hnsw".to_string(),
        params: serde_json::json!({
            "m": options.m,
            "ef_construction": options.ef_construction,
            "empty": true,
        }),
    };

    if let Err(e) = submit_and_wait(Operation::BuildIndex(request), 30_000) {
        pgrx::error!("Failed to initialize empty HNSW index: {}", e);
    }
}
```

### IVFFlat Build

```rust
/// Build IVFFlat index
#[pg_guard]
extern "C" fn ivfflat_build(
    heap: pg_sys::Relation,
    index: pg_sys::Relation,
    index_info: *mut pg_sys::IndexInfo,
) -> *mut pg_sys::IndexBuildResult {
    let heap = unsafe { PgRelation::from_raw(heap) };
    let index = unsafe { PgRelation::from_raw(index) };
    let index_info = unsafe { &*index_info };

    let options = get_ivfflat_options(&index);

    pgrx::log!("Building IVFFlat index: lists={}", options.lists);

    // Step 1: Sample for k-means
    let sample_request = SampleVectorsRequest {
        heap_oid: heap.oid().as_u32(),
        column_no: index_info.ii_IndexAttrNumbers[0] as usize,
        sample_size: (options.lists * 50).min(100_000) as usize,
    };

    let sample_result = match submit_and_wait(
        Operation::SampleVectors(sample_request),
        60_000,
    ) {
        Ok(r) => r,
        Err(e) => pgrx::error!("Sampling failed: {}", e),
    };

    // Step 2: Run k-means for centroids
    let kmeans_request = KMeansRequest {
        vectors: sample_result.vectors,
        k: options.lists as usize,
        max_iterations: 100,
    };

    let kmeans_result = match submit_and_wait(
        Operation::KMeans(kmeans_request),
        300_000,
    ) {
        Ok(r) => r,
        Err(e) => pgrx::error!("K-means failed: {}", e),
    };

    // Step 3: Build index with centroids
    let build_request = BuildIndexRequest {
        index_oid: index.oid().as_u32(),
        heap_oid: heap.oid().as_u32(),
        column_no: index_info.ii_IndexAttrNumbers[0] as usize,
        index_type: "ivfflat".to_string(),
        params: serde_json::json!({
            "lists": options.lists,
            "centroids": kmeans_result.centroids,
        }),
    };

    let result = match submit_and_wait(
        Operation::BuildIndex(build_request),
        600_000,
    ) {
        Ok(r) => r,
        Err(e) => pgrx::error!("IVFFlat build failed: {}", e),
    };

    let build_result = unsafe {
        let result_ptr = pg_sys::palloc0(size_of::<pg_sys::IndexBuildResult>())
            as *mut pg_sys::IndexBuildResult;
        (*result_ptr).heap_tuples = result.tuple_count as f64;
        (*result_ptr).index_tuples = result.indexed_count as f64;
        result_ptr
    };

    pgrx::log!(
        "IVFFlat index built: {} vectors in {} lists",
        result.indexed_count, options.lists
    );

    build_result
}
```

---

## 4. Index Insert

### HNSW Insert

```rust
/// Insert tuple into HNSW index
#[pg_guard]
extern "C" fn hnsw_insert(
    index: pg_sys::Relation,
    values: *mut pg_sys::Datum,
    isnull: *mut bool,
    heap_tid: pg_sys::ItemPointer,
    heap: pg_sys::Relation,
    check_unique: pg_sys::IndexUniqueCheck,
    _insert_state: *mut pg_sys::IndexInsertState,
    _index_info: *mut pg_sys::IndexInfo,
) -> bool {
    let index = unsafe { PgRelation::from_raw(index) };

    // Check null
    if unsafe { *isnull } {
        return false;  // Don't index nulls
    }

    // Extract vector
    let datum = unsafe { *values };
    let vector = unsafe { RuVector::from_datum(datum) };

    // Check dimensions
    let expected_dims = get_index_dimensions(&index);
    if vector.dimensions() != expected_dims {
        pgrx::error!(
            "Vector has {} dimensions, index expects {}",
            vector.dimensions(), expected_dims
        );
    }

    // Check integrity gate
    let gate_result = check_integrity_gate(
        index.oid().as_u32(),
        "insert"
    );

    if !gate_result.allowed {
        if gate_result.throttle_pct > 0 {
            // Apply throttling
            apply_throttle(gate_result.throttle_pct);
        } else {
            pgrx::error!("Insert blocked by integrity gate");
        }
    }

    // Prepare insert request
    let request = InsertRequest {
        index_oid: index.oid().as_u32(),
        vector: vector.as_slice().to_vec(),
        tid: TupleId::from(unsafe { *heap_tid }),
    };

    // Submit to engine
    match submit_and_wait(Operation::Insert(request), 5_000) {
        Ok(_) => true,
        Err(Error::Timeout) => {
            pgrx::warning!("HNSW insert timeout, retry recommended");
            false
        }
        Err(e) => {
            pgrx::warning!("HNSW insert failed: {}", e);
            false
        }
    }
}
```

### Batch Insert Optimization

```rust
/// Insert callback for IndexBuildCallback
/// Called for each tuple during index build
struct HnswBuildState {
    index_oid: u32,
    batch: Vec<(Vec<f32>, TupleId)>,
    batch_size: usize,
    total_inserted: u64,
}

#[pg_guard]
extern "C" fn hnsw_build_callback(
    index: pg_sys::Relation,
    heap_tid: pg_sys::ItemPointer,
    values: *mut pg_sys::Datum,
    isnull: *mut bool,
    _tuple_is_alive: bool,
    state: *mut std::ffi::c_void,
) {
    let state = unsafe { &mut *(state as *mut HnswBuildState) };

    if unsafe { *isnull } {
        return;
    }

    let datum = unsafe { *values };
    let vector = unsafe { RuVector::from_datum(datum) };
    let tid = TupleId::from(unsafe { *heap_tid });

    // Add to batch
    state.batch.push((vector.as_slice().to_vec(), tid));

    // Flush if batch full
    if state.batch.len() >= state.batch_size {
        flush_batch(state);
    }
}

fn flush_batch(state: &mut HnswBuildState) {
    if state.batch.is_empty() {
        return;
    }

    let request = BatchInsertRequest {
        index_oid: state.index_oid,
        vectors: state.batch.drain(..).collect(),
    };

    match submit_and_wait(Operation::BatchInsert(request), 30_000) {
        Ok(result) => {
            state.total_inserted += result.inserted_count as u64;
        }
        Err(e) => {
            pgrx::warning!("Batch insert failed: {}", e);
        }
    }
}
```

---

## 5. Index Scan

### Scan State

```rust
/// HNSW scan state
#[repr(C)]
pub struct HnswScanState {
    /// Scan is initialized
    pub initialized: bool,

    /// Query vector
    pub query: Option<Vec<f32>>,

    /// Number of results to return
    pub limit: usize,

    /// ef_search parameter (runtime)
    pub ef_search: usize,

    /// Pre-fetched results from engine
    pub results: Vec<(TupleId, f32)>,

    /// Current position in results
    pub current_pos: usize,

    /// Request ID for async operation
    pub request_id: u64,

    /// Filter expression (if any)
    pub filter: Option<FilterExpr>,
}
```

### Begin Scan

```rust
/// Begin HNSW index scan
#[pg_guard]
extern "C" fn hnsw_beginscan(
    index: pg_sys::Relation,
    nkeys: std::ffi::c_int,
    norderbys: std::ffi::c_int,
) -> pg_sys::IndexScanDesc {
    let index = unsafe { PgRelation::from_raw(index) };

    // Allocate scan descriptor
    let scan = unsafe {
        pg_sys::RelationGetIndexScan(
            index.as_ptr(),
            nkeys,
            norderbys,
        )
    };

    // Allocate scan state
    let state = Box::new(HnswScanState {
        initialized: false,
        query: None,
        limit: 0,
        ef_search: get_ef_search_guc(),
        results: Vec::new(),
        current_pos: 0,
        request_id: 0,
        filter: None,
    });

    // Store state in opaque
    unsafe {
        (*scan).opaque = Box::into_raw(state) as *mut std::ffi::c_void;
    }

    scan
}
```

### Rescan (Set Query)

```rust
/// Reset HNSW scan with new query
#[pg_guard]
extern "C" fn hnsw_rescan(
    scan: pg_sys::IndexScanDesc,
    keys: pg_sys::ScanKey,
    _nkeys: std::ffi::c_int,
    orderbys: pg_sys::ScanKey,
    norderbys: std::ffi::c_int,
) {
    let state = unsafe {
        &mut *((*scan).opaque as *mut HnswScanState)
    };

    // Reset state
    state.initialized = false;
    state.results.clear();
    state.current_pos = 0;

    // Extract query vector from ORDER BY
    if norderbys > 0 && !orderbys.is_null() {
        let orderby = unsafe { &*orderbys };
        let datum = orderby.sk_argument;

        let query_vector = unsafe { RuVector::from_datum(datum) };
        state.query = Some(query_vector.as_slice().to_vec());

        pgrx::debug1!(
            "HNSW scan: query vector with {} dimensions",
            query_vector.dimensions()
        );
    }

    // Extract LIMIT from scan descriptor
    // (PostgreSQL sets this based on query)
    state.limit = get_scan_limit(scan);
}
```

### Get Tuple

```rust
/// Get next tuple from HNSW scan
#[pg_guard]
extern "C" fn hnsw_gettuple(
    scan: pg_sys::IndexScanDesc,
    direction: pg_sys::ScanDirection,
) -> bool {
    // We only support forward scans
    if direction != pg_sys::ScanDirection::ForwardScanDirection {
        return false;
    }

    let state = unsafe {
        &mut *((*scan).opaque as *mut HnswScanState)
    };

    // Initialize search if not done
    if !state.initialized {
        if let Some(ref query) = state.query {
            // Submit search to engine
            let request = SearchRequest {
                index_oid: unsafe { (*(*scan).indexRelation).rd_id },
                query: query.clone(),
                k: state.limit.max(10),  // Fetch at least 10
                ef_search: Some(state.ef_search),
                filter: state.filter.clone(),
                use_gnn: false,  // Can be controlled via GUC
            };

            match submit_and_wait(Operation::Search(request), 10_000) {
                Ok(result) => {
                    state.results = result.results
                        .into_iter()
                        .map(|(tid, dist)| (tid, dist))
                        .collect();
                }
                Err(e) => {
                    pgrx::warning!("HNSW search failed: {}", e);
                    return false;
                }
            }
        }

        state.initialized = true;
    }

    // Return next result
    if state.current_pos < state.results.len() {
        let (tid, distance) = state.results[state.current_pos];
        state.current_pos += 1;

        // Set tuple ID
        unsafe {
            (*scan).xs_heaptid = tid.into();

            // Set ORDER BY value (distance)
            if !(*scan).xs_orderbynulls.is_null() {
                *(*scan).xs_orderbynulls = false;
            }

            // Set distance as recheck value
            let datum = pg_sys::Float8GetDatum(distance as f64);
            if !(*scan).xs_orderbyvals.is_null() {
                *(*scan).xs_orderbyvals = datum;
            }
        }

        true
    } else {
        false
    }
}
```

### End Scan

```rust
/// End HNSW index scan
#[pg_guard]
extern "C" fn hnsw_endscan(scan: pg_sys::IndexScanDesc) {
    // Free scan state
    let state = unsafe {
        Box::from_raw((*scan).opaque as *mut HnswScanState)
    };
    drop(state);
}
```

---

## 6. Cost Estimation

### HNSW Cost Model

```rust
/// Estimate HNSW scan cost
#[pg_guard]
extern "C" fn hnsw_costestimate(
    _root: *mut pg_sys::PlannerInfo,
    path: *mut pg_sys::IndexPath,
    loop_count: f64,
    index_startup_cost: *mut pg_sys::Cost,
    index_total_cost: *mut pg_sys::Cost,
    index_selectivity: *mut pg_sys::Selectivity,
    index_correlation: *mut f64,
    index_pages: *mut f64,
) {
    let index_path = unsafe { &*path };
    let index = unsafe { &*(*index_path.indexinfo).indexrelid };

    // Get index size
    let num_pages = unsafe { (*index).rd_rel.relpages } as f64;
    let num_tuples = unsafe { (*index).rd_rel.reltuples } as f64;

    // HNSW complexity: O(log N * ef_search)
    let ef_search = get_ef_search_guc() as f64;
    let search_complexity = num_tuples.log2() * ef_search;

    // Cost per tuple comparison (SIMD optimized)
    let cpu_tuple_cost = 0.001;  // Lower than default due to SIMD

    // Startup cost: graph traversal overhead
    let startup_cost = search_complexity * cpu_tuple_cost * 10.0;

    // Total cost: startup + results fetching
    let limit = get_path_limit(path);
    let total_cost = startup_cost + (limit as f64 * cpu_tuple_cost);

    unsafe {
        *index_startup_cost = startup_cost;
        *index_total_cost = total_cost * loop_count;
        *index_selectivity = (limit as f64 / num_tuples).min(1.0);
        *index_correlation = 0.0;  // No correlation with heap order
        *index_pages = num_pages;
    }
}
```

### IVFFlat Cost Model

```rust
/// Estimate IVFFlat scan cost
#[pg_guard]
extern "C" fn ivfflat_costestimate(
    _root: *mut pg_sys::PlannerInfo,
    path: *mut pg_sys::IndexPath,
    loop_count: f64,
    index_startup_cost: *mut pg_sys::Cost,
    index_total_cost: *mut pg_sys::Cost,
    index_selectivity: *mut pg_sys::Selectivity,
    index_correlation: *mut f64,
    index_pages: *mut f64,
) {
    let index_path = unsafe { &*path };
    let index = unsafe { &*(*index_path.indexinfo).indexrelid };

    let num_tuples = unsafe { (*index).rd_rel.reltuples } as f64;
    let num_pages = unsafe { (*index).rd_rel.relpages } as f64;

    let options = get_ivfflat_options_from_index(index);
    let lists = options.lists as f64;
    let probes = get_probes_guc() as f64;

    // IVFFlat: probe (probes) lists, each with ~(N/lists) vectors
    let vectors_per_list = num_tuples / lists;
    let vectors_scanned = vectors_per_list * probes;

    let cpu_tuple_cost = 0.001;

    // Startup: find nearest centroids
    let startup_cost = lists * cpu_tuple_cost;

    // Total: scan probed lists
    let total_cost = startup_cost + vectors_scanned * cpu_tuple_cost;

    let limit = get_path_limit(path);

    unsafe {
        *index_startup_cost = startup_cost;
        *index_total_cost = total_cost * loop_count;
        *index_selectivity = (limit as f64 / num_tuples).min(1.0);
        *index_correlation = 0.0;
        *index_pages = num_pages;
    }
}
```

---

## 7. Vacuum and Delete

### Bulk Delete

```rust
/// Bulk delete from HNSW index
#[pg_guard]
extern "C" fn hnsw_bulkdelete(
    info: *mut pg_sys::IndexVacuumInfo,
    stats: *mut pg_sys::IndexBulkDeleteResult,
    callback: pg_sys::IndexBulkDeleteCallback,
    callback_state: *mut std::ffi::c_void,
) -> *mut pg_sys::IndexBulkDeleteResult {
    let info = unsafe { &*info };
    let index_oid = unsafe { (*info.index).rd_id };

    // Get all TIDs in the index
    let all_tids = get_all_index_tids(index_oid);

    // Check which should be deleted
    let mut tids_to_delete = Vec::new();
    for tid in all_tids {
        let item_ptr: pg_sys::ItemPointerData = tid.into();
        let should_delete = unsafe {
            callback.unwrap()(&item_ptr as *const _, callback_state)
        };
        if should_delete {
            tids_to_delete.push(tid);
        }
    }

    if !tids_to_delete.is_empty() {
        // Submit delete request to engine
        let request = BulkDeleteRequest {
            index_oid,
            tids: tids_to_delete.clone(),
        };

        match submit_and_wait(Operation::BulkDelete(request), 60_000) {
            Ok(_) => {
                pgrx::log!("Deleted {} vectors from HNSW index", tids_to_delete.len());
            }
            Err(e) => {
                pgrx::warning!("Bulk delete failed: {}", e);
            }
        }
    }

    // Update stats
    let result = if stats.is_null() {
        unsafe {
            pg_sys::palloc0(size_of::<pg_sys::IndexBulkDeleteResult>())
                as *mut pg_sys::IndexBulkDeleteResult
        }
    } else {
        stats
    };

    unsafe {
        (*result).num_index_tuples -= tids_to_delete.len() as f64;
        (*result).tuples_removed += tids_to_delete.len() as f64;
    }

    result
}
```

### Vacuum Cleanup

```rust
/// HNSW vacuum cleanup
#[pg_guard]
extern "C" fn hnsw_vacuumcleanup(
    info: *mut pg_sys::IndexVacuumInfo,
    stats: *mut pg_sys::IndexBulkDeleteResult,
) -> *mut pg_sys::IndexBulkDeleteResult {
    let info = unsafe { &*info };
    let index_oid = unsafe { (*info.index).rd_id };

    // Request index compaction from engine
    let request = CompactIndexRequest {
        index_oid,
        rebuild_edges: true,
    };

    if let Err(e) = submit_and_wait(Operation::CompactIndex(request), 300_000) {
        pgrx::warning!("HNSW compaction failed: {}", e);
    }

    // Return or create stats
    if stats.is_null() {
        unsafe {
            pg_sys::palloc0(size_of::<pg_sys::IndexBulkDeleteResult>())
                as *mut pg_sys::IndexBulkDeleteResult
        }
    } else {
        stats
    }
}
```

---

## 8. Parallel Scan Support

### Estimate Parallel Scan Size

```rust
/// Estimate shared memory needed for parallel HNSW scan
#[pg_guard]
extern "C" fn hnsw_estimateparallelscan() -> pg_sys::Size {
    // Space for:
    // - mutex
    // - shared results buffer
    // - worker coordination
    size_of::<HnswParallelScanState>() as pg_sys::Size
}

/// Shared state for parallel scan
#[repr(C)]
pub struct HnswParallelScanState {
    /// Mutex for coordination
    pub mutex: pg_sys::slock_t,

    /// Search has been executed
    pub search_done: bool,

    /// Number of workers that have fetched results
    pub workers_finished: i32,

    /// Total workers
    pub total_workers: i32,

    /// Results offset for each worker
    pub next_result_offset: usize,

    /// Total results count
    pub total_results: usize,
}
```

### Initialize Parallel Scan

```rust
/// Initialize parallel HNSW scan
#[pg_guard]
extern "C" fn hnsw_initparallelscan(
    target: *mut std::ffi::c_void,
) {
    let state = target as *mut HnswParallelScanState;

    unsafe {
        pg_sys::SpinLockInit(&mut (*state).mutex);
        (*state).search_done = false;
        (*state).workers_finished = 0;
        (*state).total_workers = 0;
        (*state).next_result_offset = 0;
        (*state).total_results = 0;
    }
}
```

---

## 9. SQL for Index Creation

```sql
-- Create HNSW index (default L2)
CREATE INDEX idx_items_embedding ON items
USING hnsw (embedding vector_l2_ops)
WITH (m = 16, ef_construction = 64);

-- Create HNSW index with cosine distance
CREATE INDEX idx_items_embedding_cosine ON items
USING hnsw (embedding vector_cosine_ops)
WITH (m = 32, ef_construction = 128);

-- Create IVFFlat index
CREATE INDEX idx_items_embedding_ivf ON items
USING ivfflat (embedding vector_l2_ops)
WITH (lists = 100);

-- Search using index
SELECT id, title
FROM items
ORDER BY embedding <-> '[0.1, 0.2, ...]'
LIMIT 10;

-- Set runtime parameters
SET ruvector.ef_search = 100;
SET ruvector.probes = 10;
```

---

## Testing Requirements

### Unit Tests
- Index AM handler returns correct callbacks
- Options parsing (m, ef_construction, lists)
- Scan state management

### Integration Tests
- Full build/insert/search cycle
- Parallel build performance
- Vacuum and delete correctness

### Recall Tests
- Measure recall@k vs brute force
- Varying ef_search / probes
- Different dataset sizes

### Performance Tests
- Build time vs dataset size
- Query latency distribution
- Parallel scan scaling

---

## Dependencies

| Component | Purpose |
|-----------|---------|
| pgrx | PostgreSQL extension framework |
| Index AM API | PostgreSQL 14+ required |
| Shared memory | Communication with engine |
