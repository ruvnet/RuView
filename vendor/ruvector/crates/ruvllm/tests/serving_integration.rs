#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! Continuous Batching and Serving Integration Tests for v2.1
//!
//! Tests continuous batching scheduler, KV cache management, request queuing,
//! and preemption handling for LLM serving.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ============================================================================
// Request Types
// ============================================================================

/// Unique identifier for inference requests
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(pub u64);

impl RequestId {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        RequestId(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

/// Request priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Realtime = 3,
}

impl Default for RequestPriority {
    fn default() -> Self {
        RequestPriority::Normal
    }
}

/// Generation parameters for a request
#[derive(Debug, Clone)]
pub struct GenerateParams {
    pub max_tokens: usize,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: usize,
    pub stop_sequences: Vec<String>,
}

impl Default for GenerateParams {
    fn default() -> Self {
        Self {
            max_tokens: 256,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            stop_sequences: Vec::new(),
        }
    }
}

/// Request state in the serving pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestState {
    /// Waiting in queue
    Queued,
    /// Prefill phase (processing prompt)
    Prefill,
    /// Decode phase (generating tokens)
    Decode,
    /// Temporarily paused (preempted)
    Paused,
    /// Successfully completed
    Completed,
    /// Cancelled or errored
    Aborted,
}

/// Inference request
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    pub id: RequestId,
    pub prompt_tokens: Vec<u32>,
    pub params: GenerateParams,
    pub priority: RequestPriority,
    pub state: RequestState,
    pub generated_tokens: Vec<u32>,
    pub kv_cache_slot: Option<usize>,
    pub created_at: Instant,
    pub started_at: Option<Instant>,
    pub completed_at: Option<Instant>,
}

impl InferenceRequest {
    pub fn new(prompt_tokens: Vec<u32>, params: GenerateParams) -> Self {
        Self {
            id: RequestId::new(),
            prompt_tokens,
            params,
            priority: RequestPriority::Normal,
            state: RequestState::Queued,
            generated_tokens: Vec::new(),
            kv_cache_slot: None,
            created_at: Instant::now(),
            started_at: None,
            completed_at: None,
        }
    }

    pub fn with_priority(mut self, priority: RequestPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Total sequence length (prompt + generated)
    pub fn seq_len(&self) -> usize {
        self.prompt_tokens.len() + self.generated_tokens.len()
    }

    /// Check if request is complete
    pub fn is_complete(&self) -> bool {
        self.state == RequestState::Completed || self.state == RequestState::Aborted
    }

    /// Check if max tokens reached
    pub fn max_tokens_reached(&self) -> bool {
        self.generated_tokens.len() >= self.params.max_tokens
    }
}

// ============================================================================
// Request Queue
// ============================================================================

/// Priority-aware request queue
#[derive(Debug)]
pub struct RequestQueue {
    /// Queued requests by priority
    queues: HashMap<RequestPriority, VecDeque<InferenceRequest>>,
    /// Total count
    count: usize,
}

impl RequestQueue {
    pub fn new() -> Self {
        let mut queues = HashMap::new();
        queues.insert(RequestPriority::Realtime, VecDeque::new());
        queues.insert(RequestPriority::High, VecDeque::new());
        queues.insert(RequestPriority::Normal, VecDeque::new());
        queues.insert(RequestPriority::Low, VecDeque::new());

        Self { queues, count: 0 }
    }

    /// Submit a new request
    pub fn submit(&mut self, request: InferenceRequest) {
        self.queues
            .get_mut(&request.priority)
            .unwrap()
            .push_back(request);
        self.count += 1;
    }

    /// Pop highest priority request
    pub fn pop(&mut self) -> Option<InferenceRequest> {
        for priority in [
            RequestPriority::Realtime,
            RequestPriority::High,
            RequestPriority::Normal,
            RequestPriority::Low,
        ] {
            if let Some(queue) = self.queues.get_mut(&priority) {
                if let Some(request) = queue.pop_front() {
                    self.count -= 1;
                    return Some(request);
                }
            }
        }
        None
    }

    /// Peek at next request without removing
    pub fn peek(&self) -> Option<&InferenceRequest> {
        for priority in [
            RequestPriority::Realtime,
            RequestPriority::High,
            RequestPriority::Normal,
            RequestPriority::Low,
        ] {
            if let Some(queue) = self.queues.get(&priority) {
                if let Some(request) = queue.front() {
                    return Some(request);
                }
            }
        }
        None
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get total count
    pub fn len(&self) -> usize {
        self.count
    }

    /// Get count by priority
    pub fn count_by_priority(&self, priority: RequestPriority) -> usize {
        self.queues.get(&priority).map(|q| q.len()).unwrap_or(0)
    }
}

impl Default for RequestQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// KV Cache Management
// ============================================================================

/// KV cache slot allocation
#[derive(Debug, Clone)]
pub struct KvCacheSlot {
    pub slot_id: usize,
    pub request_id: Option<RequestId>,
    pub allocated_tokens: usize,
    pub max_tokens: usize,
}

/// KV cache manager with slot allocation
#[derive(Debug)]
pub struct KvCacheManager {
    slots: Vec<KvCacheSlot>,
    free_slots: VecDeque<usize>,
    request_to_slot: HashMap<RequestId, usize>,
    max_tokens_per_slot: usize,
}

impl KvCacheManager {
    pub fn new(num_slots: usize, max_tokens_per_slot: usize) -> Self {
        let mut slots = Vec::with_capacity(num_slots);
        let mut free_slots = VecDeque::with_capacity(num_slots);

        for i in 0..num_slots {
            slots.push(KvCacheSlot {
                slot_id: i,
                request_id: None,
                allocated_tokens: 0,
                max_tokens: max_tokens_per_slot,
            });
            free_slots.push_back(i);
        }

        Self {
            slots,
            free_slots,
            request_to_slot: HashMap::new(),
            max_tokens_per_slot,
        }
    }

    /// Allocate a slot for a request
    pub fn allocate(&mut self, request_id: RequestId, initial_tokens: usize) -> Option<usize> {
        if initial_tokens > self.max_tokens_per_slot {
            return None;
        }

        let slot_id = self.free_slots.pop_front()?;
        let slot = &mut self.slots[slot_id];
        slot.request_id = Some(request_id);
        slot.allocated_tokens = initial_tokens;

        self.request_to_slot.insert(request_id, slot_id);
        Some(slot_id)
    }

    /// Free a slot
    pub fn free(&mut self, request_id: RequestId) {
        if let Some(slot_id) = self.request_to_slot.remove(&request_id) {
            let slot = &mut self.slots[slot_id];
            slot.request_id = None;
            slot.allocated_tokens = 0;
            self.free_slots.push_back(slot_id);
        }
    }

    /// Extend allocation for a request
    pub fn extend(&mut self, request_id: RequestId, additional_tokens: usize) -> bool {
        if let Some(&slot_id) = self.request_to_slot.get(&request_id) {
            let slot = &mut self.slots[slot_id];
            if slot.allocated_tokens + additional_tokens <= slot.max_tokens {
                slot.allocated_tokens += additional_tokens;
                return true;
            }
        }
        false
    }

    /// Get slot for a request
    pub fn get_slot(&self, request_id: RequestId) -> Option<&KvCacheSlot> {
        self.request_to_slot
            .get(&request_id)
            .map(|&id| &self.slots[id])
    }

    /// Check available slots
    pub fn available_slots(&self) -> usize {
        self.free_slots.len()
    }

    /// Total slots
    pub fn total_slots(&self) -> usize {
        self.slots.len()
    }

    /// Check if a request has allocation
    pub fn has_allocation(&self, request_id: RequestId) -> bool {
        self.request_to_slot.contains_key(&request_id)
    }
}

// ============================================================================
// Continuous Batching Scheduler
// ============================================================================

/// Preemption modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreemptionMode {
    /// Recompute KV cache from scratch
    Recompute,
    /// Swap KV cache to CPU memory
    Swap,
}

impl Default for PreemptionMode {
    fn default() -> Self {
        PreemptionMode::Recompute
    }
}

/// Scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Maximum tokens per batch
    pub max_batch_tokens: usize,
    /// Preemption mode
    pub preemption_mode: PreemptionMode,
    /// Enable priority scheduling
    pub enable_priority: bool,
    /// Maximum waiting time before preemption (ms)
    pub max_wait_time_ms: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 32,
            max_batch_tokens: 4096,
            preemption_mode: PreemptionMode::Recompute,
            enable_priority: true,
            max_wait_time_ms: 1000,
        }
    }
}

/// Scheduled batch for execution
#[derive(Debug)]
pub struct ScheduledBatch {
    pub requests: Vec<InferenceRequest>,
    pub is_prefill: bool,
    pub total_tokens: usize,
}

/// Continuous batching scheduler
#[derive(Debug)]
pub struct ContinuousBatchScheduler {
    pub config: SchedulerConfig,
    /// Currently running requests
    running: Vec<InferenceRequest>,
    /// Paused requests (preempted)
    paused: Vec<InferenceRequest>,
    /// KV cache manager
    kv_cache: KvCacheManager,
}

impl ContinuousBatchScheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        // Create KV cache with slots matching max batch size
        let kv_cache = KvCacheManager::new(config.max_batch_size * 2, config.max_batch_tokens);

        Self {
            config,
            running: Vec::new(),
            paused: Vec::new(),
            kv_cache,
        }
    }

    /// Schedule next batch from queue
    pub fn schedule(&mut self, queue: &mut RequestQueue) -> ScheduledBatch {
        let mut batch = Vec::new();
        let mut total_tokens = 0;
        let mut is_prefill = false;

        // First, check paused requests (they have priority)
        while !self.paused.is_empty() && batch.len() < self.config.max_batch_size {
            if let Some(request) = self.paused.pop() {
                let tokens = request.seq_len();
                if total_tokens + tokens <= self.config.max_batch_tokens {
                    total_tokens += tokens;
                    batch.push(request);
                } else {
                    self.paused.push(request);
                    break;
                }
            }
        }

        // Then add new requests from queue
        while batch.len() < self.config.max_batch_size && !queue.is_empty() {
            if let Some(request) = queue.peek() {
                let tokens = request.prompt_tokens.len();
                if total_tokens + tokens <= self.config.max_batch_tokens {
                    let mut request = queue.pop().unwrap();

                    // Try to allocate KV cache
                    if let Some(slot) = self.kv_cache.allocate(request.id, tokens) {
                        request.kv_cache_slot = Some(slot);
                        request.state = RequestState::Prefill;
                        is_prefill = true;
                        total_tokens += tokens;
                        batch.push(request);
                    } else {
                        // No cache available, check preemption
                        if self.should_preempt(&request) {
                            self.preempt_lowest_priority();
                            // Re-queue request for retry
                            queue.submit(request);
                        } else {
                            queue.submit(request);
                            break;
                        }
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        ScheduledBatch {
            requests: batch,
            is_prefill,
            total_tokens,
        }
    }

    /// Check if preemption should occur
    fn should_preempt(&self, new_request: &InferenceRequest) -> bool {
        if !self.running.is_empty() {
            // Check if new request has higher priority
            if let Some(lowest) = self
                .running
                .iter()
                .filter(|r| r.state == RequestState::Decode)
                .min_by_key(|r| r.priority)
            {
                return new_request.priority > lowest.priority;
            }
        }
        false
    }

    /// Preempt lowest priority running request
    fn preempt_lowest_priority(&mut self) {
        if let Some(idx) = self
            .running
            .iter()
            .enumerate()
            .filter(|(_, r)| r.state == RequestState::Decode)
            .min_by_key(|(_, r)| r.priority)
            .map(|(i, _)| i)
        {
            let mut request = self.running.remove(idx);
            request.state = RequestState::Paused;

            // Free KV cache based on preemption mode
            if self.config.preemption_mode == PreemptionMode::Recompute {
                self.kv_cache.free(request.id);
                request.kv_cache_slot = None;
            }

            self.paused.push(request);
        }
    }

    /// Mark request as complete
    pub fn complete(&mut self, request_id: RequestId) {
        if let Some(idx) = self.running.iter().position(|r| r.id == request_id) {
            let mut request = self.running.remove(idx);
            request.state = RequestState::Completed;
            request.completed_at = Some(Instant::now());
            self.kv_cache.free(request_id);
        }
    }

    /// Abort a request
    pub fn abort(&mut self, request_id: RequestId) {
        // Check running
        if let Some(idx) = self.running.iter().position(|r| r.id == request_id) {
            let mut request = self.running.remove(idx);
            request.state = RequestState::Aborted;
            self.kv_cache.free(request_id);
            return;
        }

        // Check paused
        if let Some(idx) = self.paused.iter().position(|r| r.id == request_id) {
            let mut request = self.paused.remove(idx);
            request.state = RequestState::Aborted;
            self.kv_cache.free(request_id);
        }
    }

    /// Get statistics
    pub fn stats(&self) -> SchedulerStats {
        SchedulerStats {
            running_requests: self.running.len(),
            paused_requests: self.paused.len(),
            available_kv_slots: self.kv_cache.available_slots(),
            total_kv_slots: self.kv_cache.total_slots(),
        }
    }
}

/// Scheduler statistics
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub running_requests: usize,
    pub paused_requests: usize,
    pub available_kv_slots: usize,
    pub total_kv_slots: usize,
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn test_request_queue_basic() {
    let mut queue = RequestQueue::new();

    // Add requests
    for _ in 0..5 {
        queue.submit(InferenceRequest::new(
            vec![1, 2, 3],
            GenerateParams::default(),
        ));
    }

    assert_eq!(queue.len(), 5);
    assert!(!queue.is_empty());

    // Pop all
    for _ in 0..5 {
        assert!(queue.pop().is_some());
    }

    assert!(queue.is_empty());
    assert!(queue.pop().is_none());
}

#[test]
fn test_request_queue_priority() {
    let mut queue = RequestQueue::new();

    // Add low priority first
    queue.submit(
        InferenceRequest::new(vec![1], GenerateParams::default())
            .with_priority(RequestPriority::Low),
    );

    // Add high priority second
    queue.submit(
        InferenceRequest::new(vec![2], GenerateParams::default())
            .with_priority(RequestPriority::High),
    );

    // Add normal priority third
    queue.submit(
        InferenceRequest::new(vec![3], GenerateParams::default())
            .with_priority(RequestPriority::Normal),
    );

    // Should get high first
    let req = queue.pop().unwrap();
    assert_eq!(req.priority, RequestPriority::High);
    assert_eq!(req.prompt_tokens, vec![2]);

    // Then normal
    let req = queue.pop().unwrap();
    assert_eq!(req.priority, RequestPriority::Normal);
    assert_eq!(req.prompt_tokens, vec![3]);

    // Then low
    let req = queue.pop().unwrap();
    assert_eq!(req.priority, RequestPriority::Low);
    assert_eq!(req.prompt_tokens, vec![1]);
}

#[test]
fn test_continuous_batching_basic() {
    let scheduler = ContinuousBatchScheduler::new(SchedulerConfig::default());
    let mut queue = RequestQueue::new();

    // Add requests
    for i in 0..5 {
        queue.submit(InferenceRequest::new(
            vec![1, 2, 3], // prompt tokens
            GenerateParams::default(),
        ));
    }

    assert_eq!(queue.len(), 5);
}

#[test]
fn test_continuous_batching_schedule() {
    let mut scheduler = ContinuousBatchScheduler::new(SchedulerConfig {
        max_batch_size: 4,
        max_batch_tokens: 100,
        ..Default::default()
    });
    let mut queue = RequestQueue::new();

    // Add 6 requests
    for _ in 0..6 {
        queue.submit(InferenceRequest::new(
            vec![1, 2, 3, 4, 5], // 5 tokens each
            GenerateParams::default(),
        ));
    }

    // First batch should take 4 (max batch size)
    let batch = scheduler.schedule(&mut queue);
    assert!(batch.requests.len() <= 4);
    assert!(batch.is_prefill);
}

#[test]
fn test_kv_cache_allocation() {
    let mut manager = KvCacheManager::new(4, 1024);

    let slot1 = manager.allocate(RequestId(1), 512).unwrap();
    let slot2 = manager.allocate(RequestId(2), 512).unwrap();

    assert_ne!(slot1, slot2);
    assert_eq!(manager.available_slots(), 2);

    // Free first slot
    manager.free(RequestId(1));
    assert_eq!(manager.available_slots(), 3);

    // Should be able to allocate again (slot may be reused via FIFO queue)
    let slot3 = manager.allocate(RequestId(3), 256).unwrap();
    // Note: Due to FIFO queue, slot3 may not be slot1 - just verify allocation works
    assert!(slot3 < 4, "Slot should be valid");
    assert_eq!(manager.available_slots(), 2);
}

#[test]
fn test_kv_cache_extend() {
    let mut manager = KvCacheManager::new(2, 100);

    // Allocate with initial tokens
    manager.allocate(RequestId(1), 50).unwrap();

    // Should be able to extend
    assert!(manager.extend(RequestId(1), 30));

    // Get slot and verify
    let slot = manager.get_slot(RequestId(1)).unwrap();
    assert_eq!(slot.allocated_tokens, 80);

    // Should fail to extend beyond max
    assert!(!manager.extend(RequestId(1), 50));
}

#[test]
fn test_kv_cache_full() {
    let mut manager = KvCacheManager::new(2, 100);

    // Fill all slots
    assert!(manager.allocate(RequestId(1), 50).is_some());
    assert!(manager.allocate(RequestId(2), 50).is_some());

    // Third should fail
    assert!(manager.allocate(RequestId(3), 50).is_none());
    assert_eq!(manager.available_slots(), 0);
}

#[test]
fn test_preemption_recompute() {
    let mut scheduler = ContinuousBatchScheduler::new(SchedulerConfig {
        max_batch_size: 2,
        preemption_mode: PreemptionMode::Recompute,
        ..Default::default()
    });

    // Stats should show empty
    let stats = scheduler.stats();
    assert_eq!(stats.running_requests, 0);
    assert_eq!(stats.paused_requests, 0);
}

#[test]
fn test_request_lifecycle() {
    let mut request = InferenceRequest::new(vec![1, 2, 3], GenerateParams::default());

    assert_eq!(request.state, RequestState::Queued);
    assert!(!request.is_complete());
    assert!(!request.max_tokens_reached());

    // Simulate prefill
    request.state = RequestState::Prefill;
    request.started_at = Some(Instant::now());

    // Simulate decode
    request.state = RequestState::Decode;
    for i in 0..10 {
        request.generated_tokens.push(100 + i);
    }

    assert_eq!(request.seq_len(), 13); // 3 prompt + 10 generated

    // Complete
    request.state = RequestState::Completed;
    request.completed_at = Some(Instant::now());

    assert!(request.is_complete());
}

#[test]
fn test_request_max_tokens() {
    let mut request = InferenceRequest::new(
        vec![1, 2, 3],
        GenerateParams {
            max_tokens: 5,
            ..Default::default()
        },
    );

    assert!(!request.max_tokens_reached());

    for i in 0..5 {
        request.generated_tokens.push(100 + i);
    }

    assert!(request.max_tokens_reached());
}

#[test]
fn test_scheduler_stats() {
    let scheduler = ContinuousBatchScheduler::new(SchedulerConfig {
        max_batch_size: 8,
        ..Default::default()
    });

    let stats = scheduler.stats();
    assert_eq!(stats.running_requests, 0);
    assert_eq!(stats.paused_requests, 0);
    assert!(stats.available_kv_slots > 0);
    assert!(stats.total_kv_slots > 0);
}

#[test]
fn test_batch_token_limit() {
    let mut scheduler = ContinuousBatchScheduler::new(SchedulerConfig {
        max_batch_size: 10,
        max_batch_tokens: 20, // Very small
        ..Default::default()
    });
    let mut queue = RequestQueue::new();

    // Add requests with 10 tokens each
    for _ in 0..5 {
        queue.submit(InferenceRequest::new(
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10], // 10 tokens
            GenerateParams::default(),
        ));
    }

    // Should only fit 2 requests (20 / 10 = 2)
    let batch = scheduler.schedule(&mut queue);
    assert!(batch.total_tokens <= 20);
    assert!(batch.requests.len() <= 2);
}

#[test]
fn test_realtime_priority() {
    let mut queue = RequestQueue::new();

    // Add normal requests
    for _ in 0..3 {
        queue.submit(
            InferenceRequest::new(vec![1], GenerateParams::default())
                .with_priority(RequestPriority::Normal),
        );
    }

    // Add realtime request last
    queue.submit(
        InferenceRequest::new(vec![9], GenerateParams::default())
            .with_priority(RequestPriority::Realtime),
    );

    // Realtime should be first despite being added last
    let req = queue.pop().unwrap();
    assert_eq!(req.priority, RequestPriority::Realtime);
    assert_eq!(req.prompt_tokens, vec![9]);
}

#[test]
fn test_scheduler_config_default() {
    let config = SchedulerConfig::default();

    assert!(config.max_batch_size > 0);
    assert!(config.max_batch_tokens > 0);
    assert!(config.enable_priority);
}

#[test]
fn test_generate_params_default() {
    let params = GenerateParams::default();

    assert!(params.max_tokens > 0);
    assert!(params.temperature > 0.0);
    assert!(params.top_p > 0.0 && params.top_p <= 1.0);
    assert!(params.top_k > 0);
}

// ============================================================================
// Async Integration Tests
// ============================================================================

#[cfg(feature = "async-runtime")]
mod async_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// Simulated token generation for testing
    async fn simulate_generation(
        _request: &mut InferenceRequest,
        tokens_to_generate: usize,
    ) -> Vec<u32> {
        let mut tokens = Vec::with_capacity(tokens_to_generate);
        for i in 0..tokens_to_generate {
            // Simulate latency
            tokio::time::sleep(Duration::from_micros(100)).await;
            tokens.push(1000 + i as u32);
        }
        tokens
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let request_count = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let count = Arc::clone(&request_count);
                tokio::spawn(async move {
                    let mut request = InferenceRequest::new(
                        vec![i as u32],
                        GenerateParams {
                            max_tokens: 5,
                            ..Default::default()
                        },
                    );

                    let tokens = simulate_generation(&mut request, 5).await;
                    request.generated_tokens = tokens;

                    count.fetch_add(1, Ordering::SeqCst);
                    request
                })
            })
            .collect();

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        assert_eq!(results.len(), 10);
        assert_eq!(request_count.load(Ordering::SeqCst), 10);

        for request in results {
            assert_eq!(request.generated_tokens.len(), 5);
        }
    }

    #[tokio::test]
    async fn test_batch_processing_simulation() {
        let mut scheduler = ContinuousBatchScheduler::new(SchedulerConfig {
            max_batch_size: 4,
            max_batch_tokens: 100,
            ..Default::default()
        });
        let queue = Arc::new(Mutex::new(RequestQueue::new()));

        // Submit requests
        {
            let mut q = queue.lock().unwrap();
            for _ in 0..8 {
                q.submit(InferenceRequest::new(
                    vec![1, 2, 3, 4, 5],
                    GenerateParams::default(),
                ));
            }
        }

        // Process in batches
        let mut processed = 0;
        while processed < 8 {
            let batch = {
                let mut q = queue.lock().unwrap();
                scheduler.schedule(&mut q)
            };

            if batch.requests.is_empty() {
                break;
            }

            // Simulate batch processing
            tokio::time::sleep(Duration::from_millis(10)).await;
            processed += batch.requests.len();

            // Mark as complete
            for request in batch.requests {
                scheduler.complete(request.id);
            }
        }

        assert_eq!(processed, 8);
    }
}

// ============================================================================
// Stress Tests
// ============================================================================

#[test]
fn test_high_throughput_queue() {
    let mut queue = RequestQueue::new();

    // Add many requests
    for i in 0..1000 {
        let priority = match i % 4 {
            0 => RequestPriority::Low,
            1 => RequestPriority::Normal,
            2 => RequestPriority::High,
            _ => RequestPriority::Realtime,
        };

        queue.submit(
            InferenceRequest::new(vec![i as u32], GenerateParams::default())
                .with_priority(priority),
        );
    }

    assert_eq!(queue.len(), 1000);

    // Verify priority ordering during removal
    let mut last_priority = RequestPriority::Realtime;
    while let Some(req) = queue.pop() {
        assert!(req.priority <= last_priority || req.priority == last_priority);
        if req.priority < last_priority {
            last_priority = req.priority;
        }
    }
}

#[test]
fn test_kv_cache_churn() {
    let mut manager = KvCacheManager::new(10, 1024);
    let mut active_requests: Vec<RequestId> = Vec::new();

    // Simulate rapid allocation/deallocation
    for i in 0..100 {
        let request_id = RequestId(i);

        if let Some(_slot) = manager.allocate(request_id, 100) {
            // Extend a few times
            for _ in 0..3 {
                manager.extend(request_id, 50);
            }

            // Free every other one
            if i % 2 == 0 {
                manager.free(request_id);
            } else {
                active_requests.push(request_id);
            }
        }
    }

    // Free remaining active requests to test cleanup
    for request_id in &active_requests {
        manager.free(*request_id);
    }

    // After freeing all, should have all slots available
    assert_eq!(
        manager.available_slots(),
        10,
        "All slots should be free after cleanup"
    );
}

#[test]
fn test_request_id_uniqueness() {
    let mut ids = std::collections::HashSet::new();

    for _ in 0..1000 {
        let req = InferenceRequest::new(vec![1], GenerateParams::default());
        assert!(!ids.contains(&req.id.0), "Duplicate request ID");
        ids.insert(req.id.0);
    }
}
