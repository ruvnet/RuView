# ADR-006: Unified Memory Pool and Paging Strategy

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-01-18 |
| **Authors** | Architecture Team |
| **Reviewers** | Performance Engineering, ML Infrastructure |
| **Supersedes** | None |
| **Related** | ADR-003 (KV Cache), ADR-005 (LoRA Adapter Loading) |

**Note**: The memory pool and paging strategy described here is complemented by ADR-029. The RVF segment model provides memory management through append-only segments with temperature-tiered quantization.

## 1. Context and Problem Statement

Modern LLM inference systems face significant memory management challenges when serving multiple concurrent requests with varying adapter configurations. The S-LoRA paper demonstrated that a unified memory pool approach can dramatically improve throughput and reduce fragmentation compared to traditional per-request allocation.

### Current Challenges

1. **Memory Fragmentation**: Traditional allocators suffer from fragmentation when managing:
   - Variable-length KV cache sequences
   - Multiple LoRA adapter weights of different ranks
   - Temporary computation buffers

2. **Multi-Tenant Requirements**: Production systems must support:
   - Thousands of concurrent LoRA adapters
   - Heterogeneous batch sizes and sequence lengths
   - Dynamic adapter hot-swapping without service interruption

3. **Performance Constraints**:
   - GPU memory bandwidth is the primary bottleneck
   - Allocation latency must be sub-microsecond for inference paths
   - Memory utilization must exceed 90% to be cost-effective

### Key Insights from S-LoRA

S-LoRA's unified memory pool architecture demonstrated:
- 30x throughput improvement over naive per-adapter allocation
- Near-zero fragmentation through page-based management
- Efficient heterogeneous batching across adapter variants

## 2. Decision Drivers

- **DR-1**: Maximize GPU memory utilization (target: >95%)
- **DR-2**: Support 10,000+ concurrent LoRA adapters
- **DR-3**: Sub-microsecond allocation latency for hot paths
- **DR-4**: Zero-copy semantics where possible
- **DR-5**: Graceful degradation under memory pressure
- **DR-6**: Support heterogeneous tensor sizes without fragmentation

## 3. Considered Options

### Option A: Traditional Per-Request Allocator
- Standard cudaMalloc/cudaFree per request
- Simple implementation
- **Rejected**: Severe fragmentation, high allocation latency

### Option B: Slab Allocator with Fixed Size Classes
- Pre-defined size buckets (power-of-2)
- Low fragmentation within classes
- **Rejected**: Poor fit for variable-length KV caches

### Option C: Unified Paged Memory Pool (Selected)
- Single arena for all tensor types
- Page-granular allocation
- Reference-counted pinning
- LRU eviction with hysteresis

### Option D: Virtual Memory with Demand Paging
- Leverage CUDA virtual memory APIs
- Over-commit with page faults
- **Rejected**: Page fault latency incompatible with inference SLOs

## 4. Decision

We adopt **Option C: Unified Paged Memory Pool** with the following specifications.

### 4.1 Page Size Configuration

```
Default Page Size: 2 MB
Configurable Range: 512 KB - 4 MB
Page Alignment: 256 bytes (GPU cache line)
```

**Rationale for 2MB default**:
- Matches CUDA large page size for optimal TLB usage
- Balances internal fragmentation vs. metadata overhead
- Sufficient granularity for typical LoRA adapter sizes (rank 8-64)

### 4.2 Unified Pool Architecture

```
+------------------------------------------------------------------+
|                    UNIFIED MEMORY POOL                            |
+------------------------------------------------------------------+
|  Page 0   |  Page 1   |  Page 2   |   ...   |  Page N-1  |       |
|  [KV-A]   |  [KV-A]   |  [LoRA-1] |         |  [Temp]    |       |
|  pinned   |  pinned   |  pinned   |  free   |  unpinned  |       |
+------------------------------------------------------------------+
                              |
                              v
+------------------------------------------------------------------+
|                    PAGE METADATA TABLE                            |
+------------------------------------------------------------------+
| Page ID | Status   | Content Type | Ref Count | Last Access | ... |
|---------|----------|--------------|-----------|-------------|-----|
| 0       | PINNED   | KV_CACHE     | 3         | T+0         |     |
| 1       | PINNED   | KV_CACHE     | 3         | T+0         |     |
| 2       | PINNED   | LORA_WEIGHT  | 1         | T-100ms     |     |
| 3       | FREE     | -            | 0         | -           |     |
| N-1     | UNPINNED | TEMP_BUFFER  | 0         | T-500ms     |     |
+------------------------------------------------------------------+
```

### 4.3 Content Types

| Type | Description | Typical Size | Pin Duration |
|------|-------------|--------------|--------------|
| `KV_CACHE` | Key-value cache for attention | 1-100+ pages | Request lifetime |
| `LORA_WEIGHT` | LoRA adapter A/B matrices | 1-8 pages | Variable (hot/cold) |
| `TEMP_BUFFER` | Scratch space for computation | 1-4 pages | Kernel duration |
| `ACTIVATION` | Intermediate activations | 2-16 pages | Layer duration |
| `GRADIENT` | Gradient buffers (training) | Varies | Backward pass |

## 5. Allocation Strategy

### 5.1 Allocation Algorithm

```python
def allocate_pages(num_pages: int, content_type: ContentType) -> PageRange:
    """
    Allocate contiguous page range using best-fit strategy.

    Algorithm:
    1. Try thread-local free cache (fast path)
    2. Search global free list for best-fit range
    3. If insufficient free pages, trigger eviction
    4. Return contiguous PageRange or raise OOM
    """

    # Fast path: thread-local cache
    if thread_cache.has_contiguous(num_pages):
        return thread_cache.pop(num_pages)

    # Global free list with best-fit
    with global_freelist.try_lock():
        range = global_freelist.best_fit(num_pages)
        if range:
            return range

    # Eviction required
    evicted = eviction_policy.evict_until_free(num_pages)
    return global_freelist.allocate_after_eviction(num_pages)
```

### 5.2 Best-Fit vs First-Fit Analysis

| Strategy | Fragmentation | Search Time | Use Case |
|----------|---------------|-------------|----------|
| First-Fit | Higher | O(1) amortized | High-throughput, uniform sizes |
| Best-Fit | Lower | O(log N) | Variable sizes, long-running |

**Decision**: Use **best-fit** as default due to heterogeneous tensor sizes. Provide first-fit option for latency-critical paths.

### 5.3 Lock-Free Free List

```rust
struct LockFreePageList {
    head: AtomicPtr<PageNode>,
    size: AtomicUsize,
}

impl LockFreePageList {
    fn push(&self, page: PageId) {
        loop {
            let old_head = self.head.load(Ordering::Acquire);
            let new_node = PageNode { page, next: old_head };
            if self.head.compare_exchange_weak(
                old_head,
                &new_node,
                Ordering::Release,
                Ordering::Relaxed
            ).is_ok() {
                self.size.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
    }

    fn pop(&self) -> Option<PageId> {
        loop {
            let old_head = self.head.load(Ordering::Acquire);
            if old_head.is_null() {
                return None;
            }
            let next = unsafe { (*old_head).next };
            if self.head.compare_exchange_weak(
                old_head,
                next,
                Ordering::Release,
                Ordering::Relaxed
            ).is_ok() {
                self.size.fetch_sub(1, Ordering::Relaxed);
                return Some(unsafe { (*old_head).page });
            }
        }
    }
}
```

## 6. Pinning Rules

### 6.1 Pin States

```
         +----------+
         |  FREE    |
         +----+-----+
              |
              | allocate()
              v
         +----------+
    +--->| UNPINNED |<---+
    |    +----+-----+    |
    |         |          |
    | unpin() | pin()    | evict()
    |         v          |
    |    +----------+    |
    +----| PINNED   |----+
         +----------+
```

### 6.2 Reference Counting

```rust
struct PageMetadata {
    status: AtomicU8,           // FREE, UNPINNED, PINNED
    content_type: ContentType,
    ref_count: AtomicU32,       // Pin reference count
    last_access: AtomicU64,     // Timestamp for LRU
    owner_id: u64,              // Request/adapter ID
}

impl PageMetadata {
    fn pin(&self) -> Result<(), PinError> {
        loop {
            let count = self.ref_count.load(Ordering::Acquire);
            if self.status.load(Ordering::Acquire) == Status::FREE {
                return Err(PinError::PageFreed);
            }
            if self.ref_count.compare_exchange_weak(
                count,
                count + 1,
                Ordering::Release,
                Ordering::Relaxed
            ).is_ok() {
                self.status.store(Status::PINNED, Ordering::Release);
                return Ok(());
            }
        }
    }

    fn unpin(&self) {
        let prev = self.ref_count.fetch_sub(1, Ordering::Release);
        if prev == 1 {
            self.status.store(Status::UNPINNED, Ordering::Release);
        }
    }
}
```

### 6.3 Pinning Rules by Content Type

| Content Type | Auto-Pin Duration | Manual Unpin Required |
|--------------|-------------------|----------------------|
| KV_CACHE | Request lifetime | No (RAII handle) |
| LORA_WEIGHT | While in active batch | Yes |
| TEMP_BUFFER | Kernel execution | No (RAII handle) |
| ACTIVATION | Forward/backward pass | No (RAII handle) |

## 7. Eviction Policy

### 7.1 LRU with Size-Awareness

```python
class EvictionPolicy:
    def __init__(self, hysteresis_factor: float = 0.1):
        self.hysteresis = hysteresis_factor
        self.eviction_queue = PriorityQueue()  # Min-heap by score

    def compute_score(self, page: PageMetadata) -> float:
        """
        Eviction score: lower = more likely to evict

        Score = recency_weight * (1 / time_since_access)
              + size_weight * (pages_in_block / total_pages)
              + priority_weight * content_type_priority
        """
        recency = 1.0 / (current_time - page.last_access + 1)
        size_factor = page.block_size / self.total_pages
        priority = CONTENT_PRIORITY[page.content_type]

        return (0.6 * recency + 0.2 * size_factor + 0.2 * priority)

    def evict_until_free(self, required_pages: int) -> List[PageRange]:
        """
        Evict pages until required_pages are free.
        Uses hysteresis to prevent thrashing.
        """
        target = required_pages * (1 + self.hysteresis)
        evicted = []

        while self.free_pages < target:
            candidate = self.eviction_queue.pop_min()
            if candidate.ref_count > 0:
                continue  # Skip pinned pages

            # Evict the page
            self.free_page(candidate)
            evicted.append(candidate)

        return evicted
```

### 7.2 Content Type Priorities

| Priority | Content Type | Eviction Preference |
|----------|--------------|---------------------|
| 1 (lowest) | TEMP_BUFFER | Evict first |
| 2 | ACTIVATION | Evict second |
| 3 | LORA_WEIGHT (cold) | Evict third |
| 4 | LORA_WEIGHT (warm) | Prefer to keep |
| 5 (highest) | KV_CACHE | Evict last |

### 7.3 Hysteresis Mechanism

```
Memory Pressure vs. Eviction Rate

Eviction |                    ____________________
Rate     |                   /
         |                  /
         |                 /
         |           _____/
         |          /
         |_________/
         +------------------------------------------------
              Low        Medium        High      Critical
                       Memory Pressure

Hysteresis Band: Prevents oscillation between evict/allocate cycles
- Start eviction at 90% utilization
- Continue until 80% utilization
- Resume eviction only when pressure returns to 90%
```

## 8. Concurrency Model

### 8.1 Lock Hierarchy

```
Level 1 (Global):     [Eviction Mutex]
                            |
Level 2 (Per-Region): [Region Lock 0] [Region Lock 1] ... [Region Lock N]
                            |
Level 3 (Per-Thread): [Thread Cache 0] [Thread Cache 1] ... [Thread Cache M]
```

### 8.2 Lightweight Eviction Mutex

```rust
struct EvictionCoordinator {
    mutex: Mutex<()>,
    in_progress: AtomicBool,
    waiting_threads: AtomicUsize,
}

impl EvictionCoordinator {
    fn maybe_evict(&self, required: usize) -> bool {
        // Fast path: no eviction needed
        if self.free_pages() >= required {
            return true;
        }

        // Check if eviction already in progress
        if self.in_progress.load(Ordering::Acquire) {
            self.waiting_threads.fetch_add(1, Ordering::Relaxed);
            while self.in_progress.load(Ordering::Acquire) {
                std::hint::spin_loop();
            }
            self.waiting_threads.fetch_sub(1, Ordering::Relaxed);
            return self.free_pages() >= required;
        }

        // Acquire eviction lock
        let _guard = self.mutex.lock();
        self.in_progress.store(true, Ordering::Release);

        // Perform eviction
        self.evict_pages(required);

        self.in_progress.store(false, Ordering::Release);
        true
    }
}
```

### 8.3 Per-Thread Free Page Cache

```rust
thread_local! {
    static PAGE_CACHE: RefCell<ThreadPageCache> = RefCell::new(
        ThreadPageCache::new(THREAD_CACHE_SIZE)
    );
}

struct ThreadPageCache {
    pages: Vec<PageId>,
    max_size: usize,
}

impl ThreadPageCache {
    fn allocate(&mut self, count: usize) -> Option<Vec<PageId>> {
        if self.pages.len() >= count {
            Some(self.pages.drain(..count).collect())
        } else {
            None
        }
    }

    fn return_pages(&mut self, pages: Vec<PageId>) {
        let space = self.max_size - self.pages.len();
        let to_cache = pages.len().min(space);
        self.pages.extend(pages.into_iter().take(to_cache));

        // Return excess to global pool
        if pages.len() > to_cache {
            global_pool.return_pages(&pages[to_cache..]);
        }
    }
}
```

### 8.4 Two-Phase Kernel Activation

For GPU kernel updates that depend on page mappings:

```rust
enum ActivationPhase {
    Prepare,    // Acquire pages, update metadata
    Commit,     // Make visible to GPU kernels
    Rollback,   // On failure, release pages
}

impl PageAllocator {
    fn two_phase_allocate(&self, request: AllocationRequest) -> TwoPhaseHandle {
        // Phase 1: Prepare
        let pages = self.allocate_internal(request.size)?;
        let handle = TwoPhaseHandle::new(pages, ActivationPhase::Prepare);

        handle
    }

    fn commit(&self, handle: &mut TwoPhaseHandle) {
        // Phase 2: Commit - atomic visibility update
        memory_fence();
        for page in &handle.pages {
            self.page_table.make_visible(page);
        }
        handle.phase = ActivationPhase::Commit;
    }

    fn rollback(&self, handle: TwoPhaseHandle) {
        // Rollback - return pages to free list
        for page in handle.pages {
            self.free_page(page);
        }
    }
}
```

## 9. Multi-Tenant Adapter Serving

### 9.1 Adapter Residency Tiers

```
+------------------+     +-----------------+     +------------------+
|   HOT TIER       |     |   WARM TIER     |     |   COLD TIER      |
|   (GPU Memory)   |     |   (CPU Memory)  |     |   (Disk/NVMe)    |
+------------------+     +-----------------+     +------------------+
|  fp16 weights    |     |  int8 weights   |     |  Compressed      |
|  Instant access  |     |  ~1ms load time |     |  ~10ms load time |
|  Top 100 adapters|     |  Next 1000      |     |  Remaining       |
+------------------+     +-----------------+     +------------------+
        ^                       ^                       ^
        |                       |                       |
        +-------[Promotion]-----+-------[Promotion]-----+
        |                       |                       |
        +------[Demotion]-------+------[Demotion]-------+
```

### 9.2 Residency Rules

```python
class AdapterResidencyManager:
    def __init__(self):
        self.hot_budget = 100     # Max adapters in GPU
        self.warm_budget = 1000   # Max adapters in CPU
        self.access_window = 60   # seconds

    def compute_residency(self, adapter: Adapter) -> Tier:
        """
        Determine optimal residency tier based on usage patterns.
        """
        recent_accesses = adapter.accesses_in_window(self.access_window)

        if recent_accesses >= 10:
            return Tier.HOT
        elif recent_accesses >= 1:
            return Tier.WARM
        else:
            return Tier.COLD

    def rebalance(self):
        """
        Periodic rebalancing of adapters across tiers.
        """
        all_adapters = sorted(
            self.adapters,
            key=lambda a: a.access_frequency,
            reverse=True
        )

        # Assign to tiers
        for i, adapter in enumerate(all_adapters):
            if i < self.hot_budget:
                self.promote_to_hot(adapter)
            elif i < self.hot_budget + self.warm_budget:
                self.move_to_warm(adapter)
            else:
                self.demote_to_cold(adapter)
```

### 9.3 Heterogeneous Batching (S-LoRA Style)

```python
class HeterogeneousBatcher:
    """
    Batch requests with different LoRA adapters together.
    Uses BGMV (Batched Gather Matrix-Vector) for efficiency.
    """

    def __init__(self, max_batch_size: int = 256):
        self.max_batch = max_batch_size
        self.pending_requests = defaultdict(list)

    def add_request(self, request: InferenceRequest):
        adapter_id = request.adapter_id or "base"
        self.pending_requests[adapter_id].append(request)

    def form_batch(self) -> HeterogeneousBatch:
        """
        Form a batch that may contain multiple adapters.
        """
        batch = HeterogeneousBatch()

        # Sort adapters by pending request count
        adapters = sorted(
            self.pending_requests.items(),
            key=lambda x: len(x[1]),
            reverse=True
        )

        for adapter_id, requests in adapters:
            available_slots = self.max_batch - len(batch)
            if available_slots <= 0:
                break

            # Add requests from this adapter
            to_add = requests[:available_slots]
            batch.add_adapter_requests(adapter_id, to_add)

            # Update pending
            self.pending_requests[adapter_id] = requests[available_slots:]

        return batch
```

### 9.4 Adapter Compression

```rust
struct AdapterCompressor {
    compression_threshold: Duration,  // Compress after idle for this long
}

impl AdapterCompressor {
    fn maybe_compress(&self, adapter: &mut Adapter) -> bool {
        if adapter.last_access.elapsed() < self.compression_threshold {
            return false;
        }

        match adapter.precision {
            Precision::FP16 => {
                // Compress to INT8 for warm tier
                adapter.weights = quantize_to_int8(&adapter.weights);
                adapter.precision = Precision::INT8;
                true
            }
            Precision::INT8 => {
                // Already compressed
                false
            }
        }
    }

    fn decompress_for_use(&self, adapter: &mut Adapter) {
        if adapter.precision == Precision::INT8 {
            adapter.weights = dequantize_to_fp16(&adapter.weights);
            adapter.precision = Precision::FP16;
        }
    }
}
```

## 10. API Design

### 10.1 Core Interfaces

```rust
pub trait MemoryPool {
    /// Allocate contiguous pages
    fn allocate(&self, pages: usize, content_type: ContentType) -> Result<PageRange, AllocError>;

    /// Free pages back to pool
    fn free(&self, range: PageRange);

    /// Pin pages (prevent eviction)
    fn pin(&self, range: &PageRange) -> PinGuard;

    /// Get pool statistics
    fn stats(&self) -> PoolStats;
}

pub trait EvictionPolicy {
    /// Select pages for eviction
    fn select_victims(&self, required: usize) -> Vec<PageId>;

    /// Notify of page access (for LRU tracking)
    fn touch(&self, page: PageId);

    /// Update eviction parameters
    fn configure(&mut self, config: EvictionConfig);
}

pub trait AdapterManager {
    /// Load adapter into appropriate tier
    fn load(&self, adapter_id: &str) -> Result<AdapterHandle, LoadError>;

    /// Unload adapter (may stay cached)
    fn unload(&self, handle: AdapterHandle);

    /// Get adapter for inference (promotes if needed)
    fn acquire(&self, adapter_id: &str) -> Result<ActiveAdapter, AcquireError>;

    /// Release adapter after inference
    fn release(&self, adapter: ActiveAdapter);
}
```

### 10.2 RAII Handles

```rust
/// RAII guard that automatically unpins on drop
pub struct PinGuard<'a> {
    pool: &'a MemoryPool,
    range: PageRange,
}

impl<'a> Drop for PinGuard<'a> {
    fn drop(&mut self) {
        self.pool.unpin(&self.range);
    }
}

/// RAII handle for allocated pages
pub struct AllocationHandle {
    pool: Arc<MemoryPool>,
    range: PageRange,
    pin_guard: Option<PinGuard>,
}

impl Drop for AllocationHandle {
    fn drop(&mut self) {
        self.pin_guard.take(); // Unpin first
        self.pool.free(self.range.clone());
    }
}
```

## 11. Metrics and Observability

### 11.1 Key Metrics

| Metric | Description | Target |
|--------|-------------|--------|
| `pool_utilization` | Percentage of pages in use | >95% |
| `allocation_latency_p99` | 99th percentile allocation time | <1us |
| `eviction_rate` | Pages evicted per second | Minimize |
| `fragmentation_ratio` | Largest free block / total free | >0.8 |
| `pin_contention` | Pin operation retries | <0.1% |
| `adapter_hit_rate` | Hot tier hit rate | >90% |

### 11.2 Prometheus Metrics

```rust
lazy_static! {
    static ref POOL_UTILIZATION: Gauge = register_gauge!(
        "ruvector_memory_pool_utilization",
        "Percentage of memory pool in use"
    ).unwrap();

    static ref ALLOCATION_LATENCY: Histogram = register_histogram!(
        "ruvector_allocation_latency_seconds",
        "Time to allocate pages",
        vec![0.0000001, 0.000001, 0.00001, 0.0001, 0.001]
    ).unwrap();

    static ref EVICTION_TOTAL: Counter = register_counter!(
        "ruvector_pages_evicted_total",
        "Total pages evicted"
    ).unwrap();
}
```

## 12. Configuration

```yaml
memory_pool:
  # Page configuration
  page_size: "2MB"              # 512KB, 1MB, 2MB, 4MB
  total_pages: 4096             # Total pool size = page_size * total_pages
  alignment: 256                # Bytes

  # Allocation strategy
  allocation_strategy: "best_fit"  # first_fit, best_fit
  thread_cache_size: 16            # Pages per thread cache

  # Eviction policy
  eviction:
    policy: "lru_size_aware"
    hysteresis: 0.1              # 10% hysteresis band
    high_watermark: 0.90         # Start eviction at 90%
    low_watermark: 0.80          # Stop eviction at 80%

  # Pinning
  pinning:
    max_pin_duration: "30s"      # Auto-unpin after this
    pin_timeout: "100ms"         # Timeout for pin acquisition

  # Adapter serving
  adapters:
    hot_tier_budget: 100
    warm_tier_budget: 1000
    compression_threshold: "60s"
    promotion_threshold: 10      # Accesses to promote
```

## 13. Consequences

### Positive

- **High Utilization**: Unified pool achieves >95% memory utilization
- **Low Fragmentation**: Page-based allocation eliminates external fragmentation
- **Scalable Multi-Tenancy**: Supports 10,000+ adapters with tiered residency
- **Predictable Latency**: Lock-free fast paths maintain sub-microsecond allocation
- **Graceful Degradation**: Hysteresis prevents thrashing under pressure

### Negative

- **Internal Fragmentation**: Fixed page size wastes space for small allocations
- **Complexity**: Reference counting and eviction add implementation complexity
- **Tuning Required**: Optimal performance requires workload-specific configuration

### Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Page size mismatch | Medium | Medium | Configurable page sizes |
| Eviction storms | Low | High | Hysteresis + priorities |
| Pin leaks | Medium | Medium | RAII + timeout enforcement |
| Adapter thrashing | Medium | Medium | Promotion/demotion thresholds |

## 14. Implementation Plan

### Phase 1: Core Pool (Week 1-2)
- [ ] Page allocator with metadata table
- [ ] Best-fit allocation algorithm
- [ ] Basic LRU eviction
- [ ] Unit tests for allocation/free

### Phase 2: Concurrency (Week 3-4)
- [ ] Lock-free free list
- [ ] Thread-local caching
- [ ] Two-phase activation
- [ ] Stress tests for concurrency

### Phase 3: Adapter Serving (Week 5-6)
- [ ] Residency tier management
- [ ] Heterogeneous batching
- [ ] Adapter compression
- [ ] Integration tests

### Phase 4: Observability (Week 7)
- [ ] Prometheus metrics
- [ ] Grafana dashboards
- [ ] Alerting rules
- [ ] Performance benchmarks

## 15. References

1. S-LoRA: Serving Thousands of Concurrent LoRA Adapters (arXiv:2311.03285)
2. vLLM: Easy, Fast, and Cheap LLM Serving with PagedAttention
3. CUDA Best Practices Guide: Memory Management
4. The Slab Allocator: An Object-Caching Kernel Memory Allocator (Bonwick, 1994)
5. Lock-Free Data Structures (Herlihy & Shavit)

## 16. Appendix

### A. Page State Machine

```
                    allocate()
        +-------------------------------+
        |                               |
        v                               |
    +-------+     pin()      +--------+ |
    | FREE  |--------------->| PINNED |--+
    +-------+                +--------+
        ^                        |
        |                        | unpin() && ref_count == 0
        |                        v
        |     evict()       +----------+
        +-------------------| UNPINNED |
                            +----------+
```

### B. Memory Layout Example

```
GPU Memory (8GB total, 4096 x 2MB pages):

Pages 0-99:     KV Cache Pool (hot)
Pages 100-199:  LoRA Adapter Pool (hot tier, 100 adapters)
Pages 200-299:  Temporary Buffers
Pages 300-3999: Dynamic allocation zone
Pages 4000-4095: Reserved for system

CPU Memory (host staging):
- Warm tier adapters (int8 compressed)
- Prefetch buffers
- Eviction targets
```

### C. Benchmark Targets

| Operation | Target Latency | Throughput |
|-----------|----------------|------------|
| Allocate 1 page | <100ns | >10M/s |
| Allocate 100 pages | <1us | >1M/s |
| Pin page | <50ns | >20M/s |
| Unpin page | <50ns | >20M/s |
| Evict 1 page | <10us | >100K/s |
| Load adapter (hot) | <100us | >10K/s |
| Load adapter (warm) | <1ms | >1K/s |
| Load adapter (cold) | <10ms | >100/s |

---

## Related Decisions

- **ADR-001**: Ruvector Core Architecture
- **ADR-002**: RuvLLM Integration
- **ADR-004**: KV Cache Management
- **ADR-007**: Security Review & Technical Debt

---

## Security Status (v2.1)

| Component | Status | Notes |
|-----------|--------|-------|
| PooledBuffer | ✅ Secure | Double-free prevention documented |
| PageAllocator | ✅ Secure | RAII handles prevent leaks |
| AdapterManager | ✅ Secure | Access control enforced |

**Fixes Applied:**
- Documented safety invariants in `PooledBuffer::Drop` implementation
- Added empty buffer check in `return_buffer()` to prevent double-free

See ADR-007 for full security audit trail.

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-18 | RuVector Architecture Team | Initial version |
| 1.1 | 2026-01-19 | Security Review Agent | Added security status, related decisions |
