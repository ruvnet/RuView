# RuVector Postgres v2 - Consistency and Replication Model

## Overview

This document specifies the consistency contract between PostgreSQL heap tuples and the RuVector engine, MVCC interaction, WAL and logical decoding strategy, crash recovery, replay order, and idempotency guarantees.

---

## Core Consistency Contract

### Authoritative Source of Truth

```
+------------------------------------------------------------------+
|                    CONSISTENCY HIERARCHY                          |
+------------------------------------------------------------------+
|                                                                   |
|  1. PostgreSQL Heap is AUTHORITATIVE for:                        |
|     - Row existence                                               |
|     - Visibility rules (MVCC xmin/xmax)                          |
|     - Transaction commit status                                   |
|     - Data integrity constraints                                  |
|                                                                   |
|  2. RuVector Engine Index is EVENTUALLY CONSISTENT:              |
|     - Bounded lag window (configurable, default 100ms)           |
|     - Reconciled on demand                                        |
|     - Never returns invisible tuples                              |
|     - Never resurrects deleted embeddings                         |
|                                                                   |
+------------------------------------------------------------------+
```

### Consistency Guarantees

| Property | Guarantee | Enforcement |
|----------|-----------|-------------|
| **No phantom reads** | Index never returns invisible tuples | Heap visibility check on every result |
| **No zombie vectors** | Deleted vectors never return | Delete markers + tombstone cleanup |
| **No stale updates** | Updated vectors show new values | Version-aware index entries |
| **Bounded staleness** | Max lag from commit to searchable | Configurable, default 100ms |
| **Crash consistency** | Recoverable to last WAL checkpoint | WAL-based recovery |

---

## Consistency Mechanisms

### Option A: Synchronous Index Maintenance

```
INSERT/UPDATE Transaction:
+------------------------------------------------------------------+
|                                                                   |
|  1. BEGIN                                                         |
|  2. Write heap tuple                                              |
|  3. Call engine (synchronous)                                     |
|     └─ If engine rejects → ROLLBACK                              |
|  4. Append to WAL                                                 |
|  5. COMMIT                                                        |
|                                                                   |
+------------------------------------------------------------------+

Pros:
- Strongest consistency
- Simple mental model
- No reconciliation needed

Cons:
- Higher latency per operation
- Engine failure blocks writes
- Reduces write throughput
```

### Option B: Asynchronous Maintenance with Reconciliation

```
INSERT/UPDATE Transaction:
+------------------------------------------------------------------+
|                                                                   |
|  1. BEGIN                                                         |
|  2. Write heap tuple                                              |
|  3. Write to change log table OR trigger logical decoding         |
|  4. Append to WAL                                                 |
|  5. COMMIT                                                        |
|                                                                   |
|  Background (continuous):                                         |
|  6. Engine reads change log / logical replication stream          |
|  7. Applies changes to index                                      |
|  8. Index scan checks heap visibility for every result            |
|                                                                   |
+------------------------------------------------------------------+

Pros:
- Lower write latency
- Engine failure doesn't block writes
- Higher throughput

Cons:
- Bounded staleness window
- Requires visibility rechecks
- More complex recovery
```

### v2 Hybrid Model (Recommended)

```
+------------------------------------------------------------------+
|                   v2 HYBRID CONSISTENCY MODEL                     |
+------------------------------------------------------------------+
|                                                                   |
|  SYNCHRONOUS (Hot Tier):                                          |
|  - Primary HNSW index mutations                                   |
|  - Hot tier inserts/updates                                       |
|  - Visibility-critical operations                                 |
|                                                                   |
|  ASYNCHRONOUS (Background):                                       |
|  - Compaction and tier moves                                      |
|  - Graph edge maintenance                                         |
|  - GNN training data capture                                      |
|  - Cold tier updates                                              |
|  - Index optimization/rewiring                                    |
|                                                                   |
+------------------------------------------------------------------+
```

---

## Implementation Details

### Visibility Check Protocol

```rust
/// Check heap visibility for index results
pub fn check_visibility(
    snapshot: &Snapshot,
    results: &[IndexResult],
) -> Vec<IndexResult> {
    results.iter()
        .filter(|r| {
            // Fetch heap tuple header
            let htup = heap_fetch_tuple_header(r.tid);

            // Check MVCC visibility
            htup.map_or(false, |h| {
                heap_tuple_satisfies_snapshot(h, snapshot)
            })
        })
        .cloned()
        .collect()
}

/// Index scan must always recheck heap
impl IndexScan {
    fn next(&mut self) -> Option<HeapTuple> {
        loop {
            // Get next candidate from index
            let candidate = self.index.next()?;

            // CRITICAL: Always verify against heap
            if let Some(tuple) = self.heap_fetch_visible(candidate.tid) {
                return Some(tuple);
            }
            // Invisible tuple, try next
        }
    }
}
```

### Incremental Candidate Paging API

The engine must support incremental candidate paging so the executor can skip MVCC-invisible rows and request more until k visible results are produced.

```rust
/// Search request with cursor support for incremental paging
#[derive(Debug)]
pub struct SearchRequest {
    pub collection_id: i32,
    pub query: Vec<f32>,
    pub want_k: usize,           // Desired visible results
    pub cursor: Option<Cursor>,  // Resume from previous batch
    pub max_candidates: usize,   // Max to return per batch (default: want_k * 2)
}

/// Search response with cursor for pagination
#[derive(Debug)]
pub struct SearchResponse {
    pub candidates: Vec<Candidate>,
    pub cursor: Option<Cursor>,  // None if exhausted
    pub total_scanned: usize,
}

/// Cursor token for resuming search
#[derive(Debug, Clone)]
pub struct Cursor {
    pub ef_search_position: usize,
    pub last_distance: f32,
    pub visited_count: usize,
}

/// Engine returns batches with cursor tokens
impl Engine {
    pub fn search_batch(&self, req: SearchRequest) -> SearchResponse {
        let start_pos = req.cursor.map(|c| c.ef_search_position).unwrap_or(0);

        // Continue HNSW search from cursor position
        let (candidates, next_pos, exhausted) = self.hnsw.search_continue(
            &req.query,
            req.max_candidates,
            start_pos,
        );

        SearchResponse {
            candidates,
            cursor: if exhausted {
                None
            } else {
                Some(Cursor {
                    ef_search_position: next_pos,
                    last_distance: candidates.last().map(|c| c.distance).unwrap_or(f32::MAX),
                    visited_count: start_pos + candidates.len(),
                })
            },
            total_scanned: start_pos + candidates.len(),
        }
    }
}

/// Executor uses incremental paging
fn execute_vector_search(query: &[f32], k: usize, snapshot: &Snapshot) -> Vec<HeapTuple> {
    let mut results = Vec::with_capacity(k);
    let mut cursor = None;

    loop {
        // Request batch from engine
        let response = engine.search_batch(SearchRequest {
            collection_id,
            query: query.to_vec(),
            want_k: k - results.len(),
            cursor,
            max_candidates: (k - results.len()) * 2,  // Over-fetch
        });

        // Check visibility and collect visible tuples
        for candidate in response.candidates {
            if let Some(tuple) = heap_fetch_visible(candidate.tid, snapshot) {
                results.push(tuple);
                if results.len() >= k {
                    return results;
                }
            }
        }

        // Check if exhausted
        match response.cursor {
            Some(c) => cursor = Some(c),
            None => break,  // No more candidates
        }
    }

    results
}
```

### Change Log Table (Async Mode)

```sql
-- Change log for async reconciliation
CREATE TABLE ruvector._change_log (
    id              BIGSERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL,
    operation       CHAR(1) NOT NULL CHECK (operation IN ('I', 'U', 'D')),
    tuple_tid       TID NOT NULL,
    vector_data     BYTEA,  -- NULL for deletes
    xmin            XID NOT NULL,
    committed       BOOLEAN DEFAULT FALSE,
    applied         BOOLEAN DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX idx_change_log_pending
    ON ruvector._change_log(collection_id, id)
    WHERE NOT applied;

-- Trigger to capture changes
CREATE FUNCTION ruvector._log_change() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO ruvector._change_log (collection_id, operation, tuple_tid, vector_data, xmin)
        SELECT collection_id, 'I', NEW.ctid, NEW.embedding, txid_current()
        FROM ruvector.collections WHERE table_name = TG_TABLE_NAME;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO ruvector._change_log (collection_id, operation, tuple_tid, vector_data, xmin)
        SELECT collection_id, 'U', NEW.ctid, NEW.embedding, txid_current()
        FROM ruvector.collections WHERE table_name = TG_TABLE_NAME;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO ruvector._change_log (collection_id, operation, tuple_tid, vector_data, xmin)
        SELECT collection_id, 'D', OLD.ctid, NULL, txid_current()
        FROM ruvector.collections WHERE table_name = TG_TABLE_NAME;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;
```

### Logical Decoding (Alternative)

```rust
/// Logical decoding output plugin for RuVector
pub struct RuVectorOutputPlugin;

impl OutputPlugin for RuVectorOutputPlugin {
    fn begin_txn(&mut self, xid: TransactionId) {
        self.current_xid = Some(xid);
        self.changes.clear();
    }

    fn change(&mut self, relation: &Relation, change: &Change) {
        // Only process tables with vector columns
        if !self.is_vector_table(relation) {
            return;
        }

        match change {
            Change::Insert(new) => {
                self.changes.push(VectorChange::Insert {
                    tid: new.tid,
                    vector: extract_vector(new),
                });
            }
            Change::Update(old, new) => {
                self.changes.push(VectorChange::Update {
                    old_tid: old.tid,
                    new_tid: new.tid,
                    vector: extract_vector(new),
                });
            }
            Change::Delete(old) => {
                self.changes.push(VectorChange::Delete {
                    tid: old.tid,
                });
            }
        }
    }

    fn commit_txn(&mut self, xid: TransactionId, commit_lsn: XLogRecPtr) {
        // Apply all changes atomically
        self.engine.apply_changes(&self.changes, commit_lsn);
    }
}
```

---

## MVCC Interaction

### Transaction Visibility Rules

```rust
/// Snapshot-aware index search
pub fn search_with_snapshot(
    collection_id: i32,
    query: &[f32],
    k: usize,
    snapshot: &Snapshot,
) -> Vec<SearchResult> {
    // Get more candidates than k to account for invisible tuples
    let over_fetch_factor = 2.0;
    let candidates = engine.search(
        collection_id,
        query,
        (k as f32 * over_fetch_factor) as usize,
    );

    // Filter by visibility
    let visible: Vec<_> = candidates.into_iter()
        .filter(|c| is_visible(c.tid, snapshot))
        .take(k)
        .collect();

    // If we don't have enough, fetch more
    if visible.len() < k {
        // Recursive fetch with larger over_fetch
        return search_with_larger_pool(...);
    }

    visible
}

/// Check tuple visibility against snapshot
fn is_visible(tid: TupleId, snapshot: &Snapshot) -> bool {
    let htup = unsafe { heap_fetch_tuple(tid) };

    match htup {
        Some(tuple) => {
            // HeapTupleSatisfiesVisibility equivalent
            let xmin = tuple.t_xmin;
            let xmax = tuple.t_xmax;

            // Inserted by committed transaction visible to us
            let xmin_visible = snapshot.xmin <= xmin &&
                              !snapshot.xip.contains(&xmin) &&
                              pg_xact_status(xmin) == XACT_STATUS_COMMITTED;

            // Not deleted, or deleted by transaction not visible to us
            let not_deleted = xmax == InvalidTransactionId ||
                             snapshot.xmax <= xmax ||
                             snapshot.xip.contains(&xmax) ||
                             pg_xact_status(xmax) != XACT_STATUS_COMMITTED;

            xmin_visible && not_deleted
        }
        None => false,  // Tuple vacuumed away
    }
}
```

### HOT Update Handling

```rust
/// Handle Heap-Only Tuple updates
pub fn handle_hot_update(old_tid: TupleId, new_tid: TupleId, new_vector: &[f32]) {
    // HOT updates may change ctid without changing embedding
    if vectors_equal(get_vector(old_tid), new_vector) {
        // Only ctid changed, update TID mapping
        engine.update_tid_mapping(old_tid, new_tid);
    } else {
        // Vector changed, full update needed
        engine.delete(old_tid);
        engine.insert(new_tid, new_vector);
    }
}
```

---

## WAL and Recovery

### WAL Record Types

```rust
/// Custom WAL record types for RuVector
#[repr(u8)]
pub enum RuVectorWalRecord {
    /// Vector inserted into index
    IndexInsert = 0x10,
    /// Vector deleted from index
    IndexDelete = 0x11,
    /// Index page split
    IndexSplit = 0x12,
    /// HNSW edge added
    HnswEdgeAdd = 0x20,
    /// HNSW edge removed
    HnswEdgeRemove = 0x21,
    /// Tier change
    TierChange = 0x30,
    /// Integrity state change
    IntegrityChange = 0x40,
}

impl RuVectorWalRecord {
    /// Write WAL record
    pub fn write(&self, data: &[u8]) -> XLogRecPtr {
        unsafe {
            let rdata = XLogRecData {
                data: data.as_ptr() as *mut c_char,
                len: data.len() as u32,
                next: std::ptr::null_mut(),
            };

            XLogInsert(RM_RUVECTOR_ID, self.to_u8(), &rdata)
        }
    }
}
```

### Crash Recovery

```rust
/// Redo function for crash recovery
pub extern "C" fn ruvector_redo(record: *mut XLogReaderState) {
    let info = unsafe { (*record).decoded_record.as_ref() };

    match RuVectorWalRecord::from_u8(info.xl_info) {
        Some(RuVectorWalRecord::IndexInsert) => {
            let insert_data: IndexInsertData = deserialize(info.data);
            engine.redo_insert(insert_data);
        }
        Some(RuVectorWalRecord::IndexDelete) => {
            let delete_data: IndexDeleteData = deserialize(info.data);
            engine.redo_delete(delete_data);
        }
        Some(RuVectorWalRecord::HnswEdgeAdd) => {
            let edge_data: HnswEdgeData = deserialize(info.data);
            engine.redo_edge_add(edge_data);
        }
        // ... other record types
        _ => {
            pgrx::warning!("Unknown RuVector WAL record type");
        }
    }
}

/// Startup recovery sequence
pub fn startup_recovery() {
    pgrx::log!("RuVector: Starting crash recovery");

    // 1. Load last consistent checkpoint
    let checkpoint = load_checkpoint();

    // 2. Rebuild in-memory structures
    engine.load_from_checkpoint(&checkpoint);

    // 3. Replay WAL from checkpoint
    let wal_reader = WalReader::from_lsn(checkpoint.redo_lsn);
    for record in wal_reader {
        ruvector_redo(&record);
    }

    // 4. Reconcile with heap if needed
    if checkpoint.needs_reconciliation {
        reconcile_with_heap();
    }

    pgrx::log!("RuVector: Recovery complete");
}
```

### Replay Order Guarantees

```
WAL Replay Order Contract:
+------------------------------------------------------------------+
|                                                                   |
|  1. WAL records replayed in LSN order (guaranteed by PostgreSQL) |
|                                                                   |
|  2. Within a transaction:                                         |
|     - Heap insert before index insert                            |
|     - Index delete before heap delete (for visibility)           |
|                                                                   |
|  3. Cross-transaction:                                            |
|     - Commit order preserved                                      |
|     - Visibility respects commit timestamps                       |
|                                                                   |
|  4. Recovery invariant:                                           |
|     - After recovery, index matches committed heap state          |
|     - No uncommitted changes in index                             |
|                                                                   |
+------------------------------------------------------------------+
```

---

## Idempotency and Ordering Rules

**CRITICAL**: If WAL is truth, these invariants prevent "eventual corruption".

### Explicit Replay Rules

```
+------------------------------------------------------------------+
|              ENGINE REPLAY INVARIANTS                             |
+------------------------------------------------------------------+

RULE 1: Apply operations in LSN order
  - Each operation carries its source LSN
  - Engine rejects out-of-order operations
  - Crash recovery replays from last checkpoint LSN

RULE 2: Store last applied LSN per collection
  - Persisted in ruvector.collection_state.last_applied_lsn
  - Updated atomically after each operation
  - Skip operations with LSN <= last_applied_lsn

RULE 3: Delete wins over insert for same TID
  - If TID inserted then deleted, final state is deleted
  - Replay order handles this naturally if LSN-ordered
  - Edge case: TID reuse after VACUUM requires checking xmin

RULE 4: Update = Delete + Insert
  - Updates decompose to delete old, insert new
  - Both carry same transaction LSN
  - Applied atomically

RULE 5: Rollback handling
  - Uncommitted operations not in WAL (crash safe)
  - For explicit ROLLBACK during runtime:
    - Synchronous mode: engine notified, reverts in-memory state
    - Async mode: change log entry marked rollback, skipped on apply

+------------------------------------------------------------------+
```

### Conflict Resolution

```rust
/// Handle conflicts during replay
pub fn apply_with_conflict_resolution(
    &mut self,
    op: WalOperation,
) -> Result<(), ReplayError> {
    // Check LSN ordering
    let last_lsn = self.lsn_tracker.get(op.collection_id);
    if op.lsn <= last_lsn {
        // Already applied, skip (idempotent)
        return Ok(());
    }

    match op.kind {
        OpKind::Insert { tid, vector } => {
            if self.index.contains_tid(tid) {
                // TID exists - check if this is TID reuse after VACUUM
                let existing_lsn = self.index.get_lsn(tid);
                if op.lsn > existing_lsn {
                    // Newer insert wins - delete old, insert new
                    self.index.delete(tid);
                    self.index.insert(tid, &vector, op.lsn);
                }
                // else: stale insert, skip
            } else {
                self.index.insert(tid, &vector, op.lsn);
            }
        }
        OpKind::Delete { tid } => {
            // Delete always wins if LSN is newer
            if self.index.contains_tid(tid) {
                let existing_lsn = self.index.get_lsn(tid);
                if op.lsn > existing_lsn {
                    self.index.delete(tid);
                }
            }
            // If not present, already deleted - idempotent
        }
        OpKind::Update { old_tid, new_tid, vector } => {
            // Atomic delete + insert
            self.index.delete(old_tid);
            self.index.insert(new_tid, &vector, op.lsn);
        }
    }

    self.lsn_tracker.update(op.collection_id, op.lsn);
    Ok(())
}
```

### Idempotent Operations

```rust
/// All engine operations must be idempotent for safe replay
impl Engine {
    /// Idempotent insert - safe to replay
    pub fn redo_insert(&mut self, data: IndexInsertData) {
        // Check if already exists
        if self.index.contains_tid(data.tid) {
            // Already inserted, skip
            return;
        }

        // Insert with LSN tracking
        self.index.insert_with_lsn(data.tid, &data.vector, data.lsn);
    }

    /// Idempotent delete - safe to replay
    pub fn redo_delete(&mut self, data: IndexDeleteData) {
        // Check if already deleted
        if !self.index.contains_tid(data.tid) {
            // Already deleted, skip
            return;
        }

        // Delete with tombstone
        self.index.delete_with_lsn(data.tid, data.lsn);
    }

    /// Idempotent edge add - safe to replay
    pub fn redo_edge_add(&mut self, data: HnswEdgeData) {
        // HNSW edges are idempotent by nature
        self.hnsw.add_edge(data.from, data.to, data.lsn);
    }
}
```

### LSN-Based Deduplication

```rust
/// Track applied LSN per collection
pub struct LsnTracker {
    applied_lsn: HashMap<i32, XLogRecPtr>,
}

impl LsnTracker {
    /// Check if operation should be applied
    pub fn should_apply(&self, collection_id: i32, lsn: XLogRecPtr) -> bool {
        match self.applied_lsn.get(&collection_id) {
            Some(&last_lsn) => lsn > last_lsn,
            None => true,
        }
    }

    /// Mark operation as applied
    pub fn mark_applied(&mut self, collection_id: i32, lsn: XLogRecPtr) {
        self.applied_lsn.insert(collection_id, lsn);
    }
}
```

---

## Replication Strategies

### Physical Replication (Streaming)

```
Primary → Standby streaming with RuVector:

Primary:
1. Write heap + index changes
2. Generate WAL records
3. Stream to standby

Standby:
1. Receive WAL stream
2. Apply heap changes (PostgreSQL)
3. Apply index changes (RuVector redo)
4. Engine state matches primary
```

### Logical Replication

```
Publisher → Subscriber with RuVector:

Publisher:
1. Changes captured via logical decoding
2. RuVector output plugin extracts vector changes
3. Publishes to replication slot

Subscriber:
1. Receives logical changes
2. Applies to local heap
3. Local RuVector engine indexes changes
4. Independent index structures
```

---

## Configuration

```sql
-- Consistency configuration
ALTER SYSTEM SET ruvector.consistency_mode = 'hybrid';  -- 'sync', 'async', 'hybrid'
ALTER SYSTEM SET ruvector.max_lag_ms = 100;             -- Max staleness window
ALTER SYSTEM SET ruvector.visibility_recheck = true;    -- Always recheck heap
ALTER SYSTEM SET ruvector.wal_level = 'logical';        -- For logical replication

-- Recovery configuration
ALTER SYSTEM SET ruvector.checkpoint_interval = 300;    -- Checkpoint every 5 min
ALTER SYSTEM SET ruvector.wal_buffer_size = '64MB';     -- WAL buffer
ALTER SYSTEM SET ruvector.recovery_target_timeline = 'latest';
```

---

## Monitoring

```sql
-- Consistency lag monitoring
SELECT
    c.name AS collection,
    s.last_heap_lsn,
    s.last_index_lsn,
    pg_wal_lsn_diff(s.last_heap_lsn, s.last_index_lsn) AS lag_bytes,
    s.lag_ms,
    s.pending_changes
FROM ruvector.consistency_status s
JOIN ruvector.collections c ON s.collection_id = c.id;

-- Visibility recheck statistics
SELECT
    collection_name,
    total_searches,
    visibility_rechecks,
    invisible_filtered,
    (invisible_filtered::float / NULLIF(visibility_rechecks, 0) * 100)::numeric(5,2) AS invisible_pct
FROM ruvector.visibility_stats
ORDER BY invisible_pct DESC;

-- WAL replay status
SELECT
    pg_last_wal_receive_lsn() AS receive_lsn,
    pg_last_wal_replay_lsn() AS replay_lsn,
    ruvector_last_applied_lsn() AS ruvector_lsn,
    pg_wal_lsn_diff(pg_last_wal_replay_lsn(), ruvector_last_applied_lsn()) AS ruvector_lag_bytes;
```

---

## Testing Requirements

### Unit Tests
- Visibility check correctness
- Idempotent operation replay
- LSN tracking accuracy
- MVCC snapshot handling

### Integration Tests
- Crash recovery scenarios
- Concurrent transaction visibility
- Replication lag handling
- HOT update handling

### Chaos Tests
- Primary failover
- Network partition during replication
- Partial WAL replay
- Checkpoint corruption recovery

---

## Summary

The v2 consistency model ensures:

1. **Heap is authoritative** - All visibility decisions defer to PostgreSQL heap
2. **Bounded staleness** - Index catches up within configurable lag window
3. **Crash safe** - WAL-based recovery with idempotent replay
4. **Replication compatible** - Works with streaming and logical replication
5. **MVCC aware** - Respects transaction isolation guarantees
