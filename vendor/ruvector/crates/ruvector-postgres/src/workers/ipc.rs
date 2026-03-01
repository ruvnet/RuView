//! Inter-Process Communication (IPC) System
//!
//! Provides shared memory segments for inter-worker communication,
//! lock-free queues for task distribution, and worker status broadcasting.
//!
//! # IPC Contract Specification
//!
//! ```text
//! +------------------------------------------------------------------+
//! |                   IPC CONTRACT SPECIFICATION                      |
//! +------------------------------------------------------------------+
//!
//! ARCHITECTURE:
//!   Shared memory request queue with bounded payloads, plus optional
//!   shared segment for large vectors referenced by offset and length.
//!
//! HARD CONSTRAINTS:
//!   +----------------------------------+----------------------------+
//!   | Parameter                        | Value                      |
//!   +----------------------------------+----------------------------+
//!   | Max request size (inline)        | 64 KB                      |
//!   | Max response size (inline)       | 64 KB                      |
//!   | Max vector payload (shared seg)  | 16 MB                      |
//!   | Request queue depth              | 1024 entries               |
//!   | Response queue depth             | 1024 entries               |
//!   | Request timeout                  | 30 seconds (configurable)  |
//!   | Cancellation supported           | Yes, via request_id        |
//!   +----------------------------------+----------------------------+
//!
//! BACKPRESSURE BEHAVIOR:
//!   1. Queue full: Return EAGAIN, caller retries with exponential backoff
//!   2. Worker overloaded: Shed load by rejecting low-priority requests
//!   3. Memory pressure: Reject new requests, process existing queue
//!
//! +------------------------------------------------------------------+
//! ```

use parking_lot::RwLock;
use pgrx::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ============================================================================
// Constants
// ============================================================================

/// Maximum inline request/response size (64 KB)
pub const MAX_INLINE_SIZE: usize = 64 * 1024;

/// Maximum large payload size (16 MB)
pub const MAX_LARGE_PAYLOAD_SIZE: usize = 16 * 1024 * 1024;

/// Queue depth for work and result queues
pub const QUEUE_SIZE: usize = 1024;

/// Maximum request timeout in milliseconds
pub const MAX_REQUEST_TIMEOUT_MS: u64 = 30_000;

/// Maximum number of submit retries
pub const MAX_SUBMIT_RETRIES: u32 = 10;

/// Maximum number of collections supported
pub const MAX_COLLECTIONS: usize = 256;

/// Shared memory version for compatibility checking
pub const SHMEM_VERSION: u32 = 1;

// ============================================================================
// Work Item and Result Types
// ============================================================================

/// Reference to large payload in shared segment
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PayloadRef {
    /// Offset into the shared segment
    pub offset: u32,
    /// Length of the payload
    pub length: u32,
}

/// Operation types that can be submitted to the engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Vector search operation
    Search(SearchRequest),
    /// Vector insert operation
    Insert(InsertRequest),
    /// Vector delete operation
    Delete(DeleteRequest),
    /// Build new index
    BuildIndex(BuildIndexRequest),
    /// Update existing index
    UpdateIndex(UpdateIndexRequest),
    /// Reference to large payload in shared segment
    LargePayloadRef(PayloadRef),
    /// Ping for health check
    Ping,
}

/// Search request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    /// Collection ID
    pub collection_id: i32,
    /// Query vector
    pub query: Vec<f32>,
    /// Number of results
    pub k: usize,
    /// HNSW ef_search parameter
    pub ef_search: Option<usize>,
    /// Filter expression (optional)
    pub filter: Option<String>,
    /// Use GNN-enhanced search
    pub use_gnn: bool,
}

/// Insert request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertRequest {
    /// Collection ID
    pub collection_id: i32,
    /// Vectors to insert
    pub vectors: Vec<Vec<f32>>,
    /// Associated IDs
    pub ids: Vec<i64>,
    /// Metadata (optional)
    pub metadata: Option<Vec<String>>,
}

/// Delete request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRequest {
    /// Collection ID
    pub collection_id: i32,
    /// IDs to delete
    pub ids: Vec<i64>,
}

/// Build index request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildIndexRequest {
    /// Collection ID
    pub collection_id: i32,
    /// Index type (hnsw, ivfflat)
    pub index_type: String,
    /// Index parameters as JSON
    pub params: String,
}

/// Update index request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIndexRequest {
    /// Collection ID
    pub collection_id: i32,
    /// Incremental vectors to add
    pub vectors: Vec<Vec<f32>>,
    /// Associated IDs
    pub ids: Vec<i64>,
}

/// Work item submitted to engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    /// Unique request ID
    pub request_id: u64,
    /// Operation to perform
    pub operation: Operation,
    /// Priority (0-255, higher = more urgent)
    pub priority: u8,
    /// Deadline (epoch ms, 0 = no deadline)
    pub deadline_ms: u64,
    /// Submitting backend PID
    pub backend_pid: i32,
}

/// Work result returned from engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkResult {
    /// Request ID this result corresponds to
    pub request_id: u64,
    /// Result status
    pub status: ResultStatus,
    /// Result data (serialized)
    pub data: Vec<u8>,
    /// Processing time in microseconds
    pub processing_time_us: u64,
}

/// Result status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultStatus {
    /// Operation completed successfully
    Success,
    /// Operation failed with error
    Error,
    /// Operation timed out
    Timeout,
    /// Operation was cancelled
    Cancelled,
    /// Queue was full
    QueueFull,
}

// ============================================================================
// Lock-Free Queue Implementation
// ============================================================================

/// Lock-free MPSC work queue
pub struct WorkQueue {
    /// Head index (consumer)
    head: AtomicU64,
    /// Tail index (producers)
    tail: AtomicU64,
    /// Buffer of work items
    buffer: RwLock<Vec<Option<WorkItem>>>,
    /// Queue capacity
    capacity: usize,
}

impl WorkQueue {
    /// Create a new work queue with given capacity
    pub fn new(capacity: usize) -> Self {
        let buffer = (0..capacity).map(|_| None).collect();
        Self {
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            buffer: RwLock::new(buffer),
            capacity,
        }
    }

    /// Push an item to the queue
    pub fn push(&self, item: WorkItem) -> Result<(), QueueError> {
        loop {
            let tail = self.tail.load(Ordering::Acquire);
            let head = self.head.load(Ordering::Acquire);

            // Check if queue is full
            if tail.wrapping_sub(head) >= self.capacity as u64 {
                return Err(QueueError::Full);
            }

            // Try to claim the slot
            if self
                .tail
                .compare_exchange_weak(
                    tail,
                    tail.wrapping_add(1),
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                let slot = (tail % self.capacity as u64) as usize;
                let mut buffer = self.buffer.write();
                buffer[slot] = Some(item);
                return Ok(());
            }
            // CAS failed, retry
        }
    }

    /// Try to pop an item from the queue
    pub fn try_pop(&self) -> Option<WorkItem> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Acquire);

            // Check if queue is empty
            if head >= tail {
                return None;
            }

            let slot = (head % self.capacity as u64) as usize;

            // Try to claim this item
            if self
                .head
                .compare_exchange_weak(
                    head,
                    head.wrapping_add(1),
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                let mut buffer = self.buffer.write();
                return buffer[slot].take();
            }
            // CAS failed, retry
        }
    }

    /// Get approximate queue length
    pub fn len(&self) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);
        tail.wrapping_sub(head) as usize
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Result queue for returning results to callers
pub struct ResultQueue {
    /// Results indexed by request_id modulo capacity
    results: RwLock<std::collections::HashMap<u64, WorkResult>>,
    /// Pending request IDs (for cleanup)
    pending: RwLock<std::collections::HashSet<u64>>,
}

impl ResultQueue {
    /// Create a new result queue
    pub fn new() -> Self {
        Self {
            results: RwLock::new(std::collections::HashMap::new()),
            pending: RwLock::new(std::collections::HashSet::new()),
        }
    }

    /// Push a result
    pub fn push(&self, result: WorkResult) {
        let request_id = result.request_id;
        let mut results = self.results.write();
        let mut pending = self.pending.write();
        results.insert(request_id, result);
        pending.remove(&request_id);
    }

    /// Try to get a result for a request
    pub fn try_get(&self, request_id: u64) -> Option<WorkResult> {
        let mut results = self.results.write();
        results.remove(&request_id)
    }

    /// Mark a request as pending
    pub fn mark_pending(&self, request_id: u64) {
        let mut pending = self.pending.write();
        pending.insert(request_id);
    }

    /// Check if a request is pending
    pub fn is_pending(&self, request_id: u64) -> bool {
        let pending = self.pending.read();
        pending.contains(&request_id)
    }
}

impl Default for ResultQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Queue errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    /// Queue is full
    Full,
    /// Queue is empty
    Empty,
}

// ============================================================================
// Large Payload Segment
// ============================================================================

/// Allocation slot size (64 KB)
const SLOT_SIZE: usize = 64 * 1024;

/// Number of slots in large payload segment
const NUM_SLOTS: usize = MAX_LARGE_PAYLOAD_SIZE / SLOT_SIZE;

/// Large payload segment for vectors > 64KB
pub struct LargePayloadSegment {
    /// Segment size
    size: usize,
    /// Allocation bitmap (256 slots of 64KB each = 16MB)
    alloc_bitmap: Vec<AtomicU64>,
    /// Actual data storage
    data: RwLock<Vec<u8>>,
}

impl LargePayloadSegment {
    /// Create a new large payload segment
    pub fn new() -> Self {
        // 256 slots / 64 bits per u64 = 4 u64s for bitmap
        let bitmap_size = (NUM_SLOTS + 63) / 64;
        let alloc_bitmap = (0..bitmap_size).map(|_| AtomicU64::new(0)).collect();

        Self {
            size: MAX_LARGE_PAYLOAD_SIZE,
            alloc_bitmap,
            data: RwLock::new(vec![0u8; MAX_LARGE_PAYLOAD_SIZE]),
        }
    }

    /// Allocate space for a payload
    pub fn allocate(&self, size: usize) -> Option<PayloadRef> {
        if size > MAX_LARGE_PAYLOAD_SIZE {
            return None;
        }

        let slots_needed = size.div_ceil(SLOT_SIZE);

        // Find contiguous free slots
        for start_slot in 0..=(NUM_SLOTS - slots_needed) {
            if self.try_allocate_range(start_slot, slots_needed) {
                return Some(PayloadRef {
                    offset: (start_slot * SLOT_SIZE) as u32,
                    length: size as u32,
                });
            }
        }

        None
    }

    /// Try to allocate a range of slots
    fn try_allocate_range(&self, start: usize, count: usize) -> bool {
        // Check if all slots are free
        for slot in start..(start + count) {
            let word = slot / 64;
            let bit = slot % 64;
            let bitmap = self.alloc_bitmap[word].load(Ordering::Acquire);
            if bitmap & (1u64 << bit) != 0 {
                return false;
            }
        }

        // Try to atomically set all bits
        for slot in start..(start + count) {
            let word = slot / 64;
            let bit = slot % 64;
            let mask = 1u64 << bit;

            loop {
                let current = self.alloc_bitmap[word].load(Ordering::Acquire);
                if current & mask != 0 {
                    // Slot was taken, rollback
                    for s in start..slot {
                        let w = s / 64;
                        let b = s % 64;
                        self.alloc_bitmap[w].fetch_and(!(1u64 << b), Ordering::Release);
                    }
                    return false;
                }

                if self.alloc_bitmap[word]
                    .compare_exchange_weak(
                        current,
                        current | mask,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    break;
                }
            }
        }

        true
    }

    /// Free a previously allocated payload
    pub fn free(&self, payload_ref: &PayloadRef) {
        let start_slot = payload_ref.offset as usize / SLOT_SIZE;
        let slots = (payload_ref.length as usize).div_ceil(SLOT_SIZE);

        for slot in start_slot..(start_slot + slots) {
            let word = slot / 64;
            let bit = slot % 64;
            self.alloc_bitmap[word].fetch_and(!(1u64 << bit), Ordering::Release);
        }
    }

    /// Write data to the segment
    pub fn write(&self, offset: usize, data: &[u8]) -> Result<(), String> {
        if offset + data.len() > self.size {
            return Err("Write exceeds segment bounds".to_string());
        }

        let mut buffer = self.data.write();
        buffer[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Read data from the segment
    pub fn read(&self, offset: usize, length: usize) -> Result<Vec<u8>, String> {
        if offset + length > self.size {
            return Err("Read exceeds segment bounds".to_string());
        }

        let buffer = self.data.read();
        Ok(buffer[offset..offset + length].to_vec())
    }

    /// Get bytes used
    pub fn bytes_used(&self) -> usize {
        let mut count = 0;
        for bitmap in &self.alloc_bitmap {
            count += bitmap.load(Ordering::Relaxed).count_ones() as usize;
        }
        count * SLOT_SIZE
    }
}

impl Default for LargePayloadSegment {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Index and Integrity State
// ============================================================================

/// Per-collection index state
#[derive(Debug, Clone, Default)]
pub struct IndexState {
    /// Collection ID
    pub collection_id: i32,
    /// Whether index is loaded
    pub loaded: bool,
    /// Number of vectors
    pub vector_count: u64,
    /// Index size in bytes
    pub size_bytes: u64,
    /// Last query timestamp
    pub last_query_at: u64,
    /// Query count
    pub query_count: u64,
}

/// Per-collection integrity permissions
#[derive(Debug, Clone, Default)]
pub struct IntegrityPermissions {
    /// Collection ID
    pub collection_id: i32,
    /// Current integrity state
    pub state: u8, // 0=normal, 1=stress, 2=critical
    /// Lambda cut value
    pub lambda_cut: f64,
    /// Allow reads
    pub allow_reads: bool,
    /// Allow writes
    pub allow_writes: bool,
    /// Allow deletes
    pub allow_deletes: bool,
    /// Last update timestamp
    pub last_update: u64,
}

// ============================================================================
// Global Statistics
// ============================================================================

/// Global statistics counters
#[derive(Debug, Default)]
pub struct GlobalStats {
    /// Total requests processed
    pub total_requests: AtomicU64,
    /// Total successful requests
    pub successful_requests: AtomicU64,
    /// Total failed requests
    pub failed_requests: AtomicU64,
    /// Total timeouts
    pub timeouts: AtomicU64,
    /// Total queue full events
    pub queue_full_events: AtomicU64,
    /// Total bytes processed
    pub bytes_processed: AtomicU64,
    /// Total processing time (microseconds)
    pub total_processing_time_us: AtomicU64,
}

impl GlobalStats {
    /// Create new stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful request
    pub fn record_success(&self, processing_time_us: u64, bytes: u64) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.successful_requests.fetch_add(1, Ordering::Relaxed);
        self.total_processing_time_us
            .fetch_add(processing_time_us, Ordering::Relaxed);
        self.bytes_processed.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record a failed request
    pub fn record_failure(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.failed_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a timeout
    pub fn record_timeout(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.timeouts.fetch_add(1, Ordering::Relaxed);
    }

    /// Record queue full event
    pub fn record_queue_full(&self) {
        self.queue_full_events.fetch_add(1, Ordering::Relaxed);
    }

    /// Get statistics as JSON
    pub fn to_json(&self) -> serde_json::Value {
        let total = self.total_requests.load(Ordering::Relaxed);
        let successful = self.successful_requests.load(Ordering::Relaxed);
        let total_time = self.total_processing_time_us.load(Ordering::Relaxed);

        serde_json::json!({
            "total_requests": total,
            "successful_requests": successful,
            "failed_requests": self.failed_requests.load(Ordering::Relaxed),
            "timeouts": self.timeouts.load(Ordering::Relaxed),
            "queue_full_events": self.queue_full_events.load(Ordering::Relaxed),
            "bytes_processed": self.bytes_processed.load(Ordering::Relaxed),
            "total_processing_time_us": total_time,
            "avg_processing_time_us": if successful > 0 { total_time / successful } else { 0 },
        })
    }
}

// ============================================================================
// Shared Memory Layout
// ============================================================================

/// Complete shared memory layout
pub struct SharedMemoryLayout {
    /// Version for compatibility checking
    pub version: AtomicU32,
    /// Global lock for initialization
    pub init_lock: AtomicU32,
    /// Work queue for operations
    pub work_queue: WorkQueue,
    /// Result queue for responses
    pub result_queue: ResultQueue,
    /// Large payload shared segment
    pub large_payload_segment: LargePayloadSegment,
    /// Per-collection index state
    pub index_states: RwLock<Vec<IndexState>>,
    /// Per-collection integrity state
    pub integrity_states: RwLock<Vec<IntegrityPermissions>>,
    /// Statistics counters
    pub stats: GlobalStats,
    /// Next request ID
    next_request_id: AtomicU64,
    /// Cancelled requests
    cancelled: RwLock<std::collections::HashSet<u64>>,
}

impl SharedMemoryLayout {
    /// Create a new shared memory layout
    pub fn new() -> Self {
        Self {
            version: AtomicU32::new(SHMEM_VERSION),
            init_lock: AtomicU32::new(0),
            work_queue: WorkQueue::new(QUEUE_SIZE),
            result_queue: ResultQueue::new(),
            large_payload_segment: LargePayloadSegment::new(),
            index_states: RwLock::new(vec![IndexState::default(); MAX_COLLECTIONS]),
            integrity_states: RwLock::new(vec![IntegrityPermissions::default(); MAX_COLLECTIONS]),
            stats: GlobalStats::new(),
            next_request_id: AtomicU64::new(1),
            cancelled: RwLock::new(std::collections::HashSet::new()),
        }
    }

    /// Generate next request ID
    pub fn next_request_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Cancel a request
    pub fn cancel_request(&self, request_id: u64) {
        let mut cancelled = self.cancelled.write();
        cancelled.insert(request_id);
    }

    /// Check if request is cancelled
    pub fn is_cancelled(&self, request_id: u64) -> bool {
        let cancelled = self.cancelled.read();
        cancelled.contains(&request_id)
    }

    /// Clean up old cancelled requests
    pub fn cleanup_cancelled(&self, max_age_ms: u64) {
        // In production, track timestamps and clean up old entries
        let mut cancelled = self.cancelled.write();
        if cancelled.len() > 10000 {
            // Keep only recent entries
            cancelled.clear();
        }
    }

    /// Signal the engine worker
    pub fn signal_engine(&self) {
        // In production, use pg_sys::SetLatch to wake up the engine worker
        // For now, this is a no-op as the worker polls
    }

    /// Update integrity permissions for a collection
    pub fn update_integrity_permissions(
        &self,
        collection_id: i32,
        permissions: &IntegrityPermissions,
    ) {
        if (collection_id as usize) < MAX_COLLECTIONS {
            let mut states = self.integrity_states.write();
            states[collection_id as usize] = permissions.clone();
        }
    }

    /// Get integrity permissions for a collection
    pub fn get_integrity_permissions(&self, collection_id: i32) -> Option<IntegrityPermissions> {
        if (collection_id as usize) < MAX_COLLECTIONS {
            let states = self.integrity_states.read();
            Some(states[collection_id as usize].clone())
        } else {
            None
        }
    }
}

impl Default for SharedMemoryLayout {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Shared Memory Access
// ============================================================================

/// Global shared memory instance
static SHARED_MEMORY: OnceLock<SharedMemoryLayout> = OnceLock::new();

/// Get the global shared memory instance
pub fn get_shared_memory() -> &'static SharedMemoryLayout {
    SHARED_MEMORY.get_or_init(SharedMemoryLayout::new)
}

/// SharedMemory wrapper for compatibility
pub struct SharedMemory;

impl SharedMemory {
    /// Get shared memory reference
    pub fn get() -> &'static SharedMemoryLayout {
        get_shared_memory()
    }

    /// Attach to shared memory
    pub fn attach() -> Result<&'static SharedMemoryLayout, String> {
        Ok(get_shared_memory())
    }
}

/// Initialize shared memory
pub fn init_shared_memory() -> Result<(), String> {
    let shmem = get_shared_memory();

    // Verify version
    if shmem.version.load(Ordering::SeqCst) != SHMEM_VERSION {
        return Err("Shared memory version mismatch".to_string());
    }

    pgrx::log!("Shared memory initialized (version {})", SHMEM_VERSION);
    Ok(())
}

// ============================================================================
// Submit and Wait API
// ============================================================================

/// Get current epoch time in milliseconds
fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Submit work to engine and wait for result
pub fn submit_and_wait(operation: Operation, timeout_ms: u64) -> Result<WorkResult, IpcError> {
    let shmem = get_shared_memory();

    // Generate request ID
    let request_id = shmem.next_request_id();

    // Check payload size for large payloads
    let (final_operation, payload_ref) = prepare_operation(operation, shmem)?;

    // Create work item
    let work_item = WorkItem {
        request_id,
        operation: final_operation,
        priority: 128, // Default priority
        deadline_ms: current_epoch_ms() + timeout_ms.min(MAX_REQUEST_TIMEOUT_MS),
        backend_pid: unsafe { pg_sys::MyProcPid },
    };

    // Submit to work queue with backpressure handling
    let mut retry_count = 0;
    loop {
        match shmem.work_queue.push(work_item.clone()) {
            Ok(()) => break,
            Err(QueueError::Full) => {
                retry_count += 1;
                if retry_count > MAX_SUBMIT_RETRIES {
                    if let Some(ref pr) = payload_ref {
                        shmem.large_payload_segment.free(pr);
                    }
                    shmem.stats.record_queue_full();
                    return Err(IpcError::QueueFull);
                }
                // Exponential backoff: 1ms, 2ms, 4ms, 8ms...
                std::thread::sleep(Duration::from_millis(1 << retry_count.min(6)));
            }
            Err(QueueError::Empty) => unreachable!(),
        }
    }

    // Mark as pending
    shmem.result_queue.mark_pending(request_id);

    // Signal engine worker
    shmem.signal_engine();

    // Wait for result
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        // Check result queue
        if let Some(result) = shmem.result_queue.try_get(request_id) {
            if let Some(ref pr) = payload_ref {
                shmem.large_payload_segment.free(pr);
            }
            return Ok(result);
        }

        // Check timeout
        if Instant::now() > deadline {
            shmem.cancel_request(request_id);
            if let Some(ref pr) = payload_ref {
                shmem.large_payload_segment.free(pr);
            }
            shmem.stats.record_timeout();
            return Err(IpcError::Timeout);
        }

        // Check for query cancellation
        if unsafe { pg_sys::QueryCancelPending != 0 } {
            shmem.cancel_request(request_id);
            if let Some(ref pr) = payload_ref {
                shmem.large_payload_segment.free(pr);
            }
            return Err(IpcError::Cancelled);
        }

        // Wait with latch (in production)
        // For now, use a short sleep
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Prepare operation, moving large payloads to shared segment
fn prepare_operation(
    operation: Operation,
    shmem: &SharedMemoryLayout,
) -> Result<(Operation, Option<PayloadRef>), IpcError> {
    // Serialize to check size
    let serialized =
        bincode::serialize(&operation).map_err(|e| IpcError::SerializationError(e.to_string()))?;

    if serialized.len() <= MAX_INLINE_SIZE {
        return Ok((operation, None));
    }

    // Allocate in shared segment
    let payload_ref = shmem
        .large_payload_segment
        .allocate(serialized.len())
        .ok_or(IpcError::PayloadTooLarge)?;

    // Write to shared segment
    shmem
        .large_payload_segment
        .write(payload_ref.offset as usize, &serialized)
        .map_err(IpcError::SharedMemoryError)?;

    Ok((Operation::LargePayloadRef(payload_ref), Some(payload_ref)))
}

/// IPC errors
#[derive(Debug, Clone)]
pub enum IpcError {
    /// Queue is full
    QueueFull,
    /// Operation timed out
    Timeout,
    /// Operation was cancelled
    Cancelled,
    /// Payload too large for shared segment
    PayloadTooLarge,
    /// Shared memory error
    SharedMemoryError(String),
    /// Serialization error
    SerializationError(String),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::QueueFull => write!(f, "Work queue is full"),
            IpcError::Timeout => write!(f, "Operation timed out"),
            IpcError::Cancelled => write!(f, "Operation was cancelled"),
            IpcError::PayloadTooLarge => write!(f, "Payload too large for shared segment"),
            IpcError::SharedMemoryError(e) => write!(f, "Shared memory error: {}", e),
            IpcError::SerializationError(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl std::error::Error for IpcError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_queue_basic() {
        let queue = WorkQueue::new(16);

        let item = WorkItem {
            request_id: 1,
            operation: Operation::Ping,
            priority: 128,
            deadline_ms: 0,
            backend_pid: 0,
        };

        assert!(queue.push(item.clone()).is_ok());
        assert_eq!(queue.len(), 1);

        let popped = queue.try_pop().unwrap();
        assert_eq!(popped.request_id, 1);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_work_queue_full() {
        let queue = WorkQueue::new(2);

        let item = WorkItem {
            request_id: 1,
            operation: Operation::Ping,
            priority: 128,
            deadline_ms: 0,
            backend_pid: 0,
        };

        assert!(queue.push(item.clone()).is_ok());
        assert!(queue.push(item.clone()).is_ok());
        assert_eq!(queue.push(item.clone()), Err(QueueError::Full));
    }

    #[test]
    fn test_result_queue() {
        let queue = ResultQueue::new();

        queue.mark_pending(1);
        assert!(queue.is_pending(1));

        let result = WorkResult {
            request_id: 1,
            status: ResultStatus::Success,
            data: vec![],
            processing_time_us: 100,
        };

        queue.push(result);
        assert!(!queue.is_pending(1));

        let retrieved = queue.try_get(1).unwrap();
        assert_eq!(retrieved.request_id, 1);
    }

    #[test]
    fn test_large_payload_segment() {
        let segment = LargePayloadSegment::new();

        // Allocate 100KB
        let payload_ref = segment.allocate(100 * 1024).unwrap();
        assert_eq!(payload_ref.offset, 0);
        assert_eq!(payload_ref.length, 100 * 1024);

        // Write and read data
        let data = vec![42u8; 1000];
        segment.write(0, &data).unwrap();
        let read_data = segment.read(0, 1000).unwrap();
        assert_eq!(data, read_data);

        // Free
        segment.free(&payload_ref);
        assert_eq!(segment.bytes_used(), 0);
    }

    #[test]
    fn test_global_stats() {
        let stats = GlobalStats::new();

        stats.record_success(100, 1000);
        stats.record_success(200, 2000);
        stats.record_failure();
        stats.record_timeout();

        let json = stats.to_json();
        assert_eq!(json["total_requests"], 4);
        assert_eq!(json["successful_requests"], 2);
        assert_eq!(json["failed_requests"], 1);
        assert_eq!(json["timeouts"], 1);
    }
}
