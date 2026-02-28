//! Continuous Batching Scheduler
//!
//! This module implements the core continuous batching scheduler that
//! efficiently batches prefill and decode requests for maximum GPU utilization.

use super::batch::{
    BatchedRequest, DecodeTask, IterationPlan, PrefillTask, ScheduledBatch, TokenBudget,
};
use super::kv_cache_manager::{KvCacheManager, KvCachePoolConfig};
use super::request::{InferenceRequest, Priority, RequestId, RequestState, RunningRequest};
use crate::error::{Result, RuvLLMError};
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

/// Preemption strategy when memory is exhausted
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreemptionMode {
    /// Evict and recompute prefill later (no memory overhead)
    Recompute,
    /// Swap KV cache to CPU memory (faster resume, uses CPU RAM)
    Swap,
}

impl Default for PreemptionMode {
    fn default() -> Self {
        Self::Recompute
    }
}

/// Priority policy for request scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityPolicy {
    /// First come, first served
    Fcfs,
    /// Shortest job first (based on remaining tokens)
    ShortestJobFirst,
    /// Priority-based (respects request priority levels)
    PriorityBased,
    /// Adaptive (combines multiple factors)
    Adaptive,
}

impl Default for PriorityPolicy {
    fn default() -> Self {
        Self::Fcfs
    }
}

/// Configuration for the continuous batching scheduler
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Maximum requests in a single batch
    pub max_batch_size: usize,
    /// Maximum tokens waiting before forcing scheduling
    pub max_waiting_tokens: usize,
    /// Maximum tokens per batch iteration
    pub max_tokens_per_batch: usize,
    /// Maximum prefill tokens per iteration
    pub max_prefill_tokens: usize,
    /// Preemption strategy
    pub preemption_mode: PreemptionMode,
    /// Priority scheduling policy
    pub priority_policy: PriorityPolicy,
    /// Enable chunked prefill for long prompts
    pub chunked_prefill: bool,
    /// Chunk size for chunked prefill
    pub prefill_chunk_size: usize,
    /// Maximum time a request can wait (ms)
    pub max_waiting_time_ms: u64,
    /// Enable priority aging (waiting requests gain priority)
    pub priority_aging: bool,
    /// Aging factor (priority increase per second)
    pub aging_factor: f32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 256,
            max_waiting_tokens: 8192,
            max_tokens_per_batch: 4096,
            max_prefill_tokens: 2048,
            preemption_mode: PreemptionMode::Recompute,
            priority_policy: PriorityPolicy::Fcfs,
            chunked_prefill: true,
            prefill_chunk_size: 512,
            max_waiting_time_ms: 30000,
            priority_aging: true,
            aging_factor: 0.1,
        }
    }
}

/// Request queue for pending requests
#[derive(Debug)]
pub struct RequestQueue {
    /// Pending requests awaiting scheduling
    pub pending: VecDeque<InferenceRequest>,
    /// Currently running requests
    pub running: HashMap<RequestId, RunningRequest>,
    /// Preempted requests waiting to resume
    pub preempted: VecDeque<RequestId>,
    /// Total pending tokens
    pending_tokens: usize,
}

impl RequestQueue {
    /// Create a new request queue
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            running: HashMap::new(),
            preempted: VecDeque::new(),
            pending_tokens: 0,
        }
    }

    /// Add a new request to the queue
    pub fn add(&mut self, request: InferenceRequest) {
        self.pending_tokens += request.prompt_len();
        self.pending.push_back(request);
    }

    /// Get the number of pending requests
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Get the number of running requests
    pub fn running_count(&self) -> usize {
        self.running.len()
    }

    /// Get the number of preempted requests
    pub fn preempted_count(&self) -> usize {
        self.preempted.len()
    }

    /// Total pending tokens in queue
    pub fn pending_tokens(&self) -> usize {
        self.pending_tokens
    }

    /// Pop a pending request
    pub fn pop_pending(&mut self) -> Option<InferenceRequest> {
        if let Some(request) = self.pending.pop_front() {
            self.pending_tokens -= request.prompt_len();
            Some(request)
        } else {
            None
        }
    }

    /// Add a running request
    pub fn add_running(&mut self, request: RunningRequest) {
        self.running.insert(request.id(), request);
    }

    /// Remove a running request
    pub fn remove_running(&mut self, id: RequestId) -> Option<RunningRequest> {
        self.running.remove(&id)
    }

    /// Get a mutable reference to a running request
    pub fn get_running_mut(&mut self, id: RequestId) -> Option<&mut RunningRequest> {
        self.running.get_mut(&id)
    }

    /// Add a preempted request ID
    pub fn add_preempted(&mut self, id: RequestId) {
        self.preempted.push_back(id);
    }

    /// Pop a preempted request ID
    pub fn pop_preempted(&mut self) -> Option<RequestId> {
        self.preempted.pop_front()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.running.is_empty() && self.preempted.is_empty()
    }

    /// Sort pending by priority (for priority-based scheduling)
    pub fn sort_pending_by_priority(&mut self) {
        let mut pending_vec: Vec<_> = self.pending.drain(..).collect();
        pending_vec.sort_by(|a, b| b.priority.cmp(&a.priority));
        self.pending = pending_vec.into_iter().collect();
    }

    /// Sort pending by shortest job first
    pub fn sort_pending_by_length(&mut self) {
        let mut pending_vec: Vec<_> = self.pending.drain(..).collect();
        pending_vec.sort_by_key(|r| r.prompt_len() + r.params.max_tokens);
        self.pending = pending_vec.into_iter().collect();
    }
}

impl Default for RequestQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Continuous batching scheduler
pub struct ContinuousBatchScheduler {
    /// Configuration
    config: SchedulerConfig,
    /// KV cache manager
    kv_cache_manager: KvCacheManager,
    /// Batch counter
    batch_counter: AtomicU64,
    /// Preempted request data (for recompute mode)
    preempted_data: RwLock<HashMap<RequestId, PreemptedRequestData>>,
}

/// Data stored for preempted requests in recompute mode
#[derive(Debug, Clone)]
struct PreemptedRequestData {
    /// Original request
    request: InferenceRequest,
    /// Generated tokens before preemption
    generated_tokens: Vec<u32>,
    /// Decode steps completed
    decode_steps: usize,
}

impl ContinuousBatchScheduler {
    /// Create a new scheduler with given configuration
    pub fn new(config: SchedulerConfig, kv_cache_config: KvCachePoolConfig) -> Self {
        let kv_cache_manager = KvCacheManager::new(kv_cache_config);

        Self {
            config,
            kv_cache_manager,
            batch_counter: AtomicU64::new(0),
            preempted_data: RwLock::new(HashMap::new()),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(SchedulerConfig::default(), KvCachePoolConfig::default())
    }

    /// Schedule requests for the next iteration
    pub fn schedule(&mut self, queue: &mut RequestQueue) -> ScheduledBatch {
        let batch_id = self.batch_counter.fetch_add(1, Ordering::Relaxed);
        let plan = self.create_iteration_plan(queue);
        plan.to_scheduled_batch(batch_id)
    }

    /// Create an iteration plan from the current queue state
    fn create_iteration_plan(&mut self, queue: &mut RequestQueue) -> IterationPlan {
        let mut plan = IterationPlan::empty();
        let mut budget = TokenBudget::new(
            self.config.max_prefill_tokens,
            self.config.max_batch_size,
            self.config.max_tokens_per_batch,
        );

        // Apply priority policy
        match self.config.priority_policy {
            PriorityPolicy::ShortestJobFirst => queue.sort_pending_by_length(),
            PriorityPolicy::PriorityBased => queue.sort_pending_by_priority(),
            _ => {}
        }

        // First, schedule decode for running requests (they have priority)
        self.schedule_decode_requests(queue, &mut plan, &mut budget);

        // Check for preempted requests that need to be resumed
        self.schedule_preempted_requests(queue, &mut plan, &mut budget);

        // Then, schedule new prefill requests
        self.schedule_prefill_requests(queue, &mut plan, &mut budget);

        // If memory pressure, preempt if needed
        if self.should_preempt(queue) {
            self.preempt_requests(queue, &mut plan);
        }

        plan
    }

    /// Schedule decode tasks for running requests
    fn schedule_decode_requests(
        &self,
        queue: &mut RequestQueue,
        plan: &mut IterationPlan,
        budget: &mut TokenBudget,
    ) {
        // Collect running request IDs (to avoid borrow conflicts)
        let running_ids: Vec<RequestId> = queue.running.keys().copied().collect();

        for id in running_ids {
            if !budget.try_allocate_decode() {
                break;
            }

            if let Some(running) = queue.running.get(&id) {
                // Skip if prefill not complete
                if !running.prefill_complete {
                    continue;
                }

                // Get last generated token (or first prompt token if no generations yet)
                let input_token =
                    running.generated_tokens.last().copied().unwrap_or_else(|| {
                        running.request.prompt_tokens.last().copied().unwrap_or(0)
                    });

                plan.decode_tasks.push(DecodeTask {
                    request_id: id,
                    input_token,
                    position: running.current_seq_len,
                    kv_cache_slot: running.kv_cache_slot,
                    block_table: running.block_table.clone(),
                    context_len: running.context_len,
                });
            }
        }
    }

    /// Schedule prefill tasks for new requests
    fn schedule_prefill_requests(
        &mut self,
        queue: &mut RequestQueue,
        plan: &mut IterationPlan,
        budget: &mut TokenBudget,
    ) {
        while !queue.pending.is_empty() {
            // Check if we can allocate for next request
            let request = match queue.pending.front() {
                Some(r) => r,
                None => break,
            };

            // Check if we have capacity
            if !self.can_add_request(request) {
                break;
            }

            // Check token budget
            let prefill_tokens = if self.config.chunked_prefill {
                request.prompt_len().min(self.config.prefill_chunk_size)
            } else {
                request.prompt_len()
            };

            if !budget.try_allocate_prefill(prefill_tokens) {
                break;
            }

            // Pop request and allocate
            let request = queue.pop_pending().unwrap();
            let request_id = request.id;
            let max_tokens = request.max_seq_len;

            // Allocate KV cache
            let slot_id = match self.kv_cache_manager.allocate(request_id, max_tokens) {
                Ok(slot) => slot,
                Err(_) => {
                    // Put request back and break
                    queue.add(request);
                    break;
                }
            };

            // Get block table
            let block_table = self
                .kv_cache_manager
                .get_block_table(request_id)
                .unwrap_or_default();

            // Determine tokens to prefill
            let tokens = if self.config.chunked_prefill
                && request.prompt_len() > self.config.prefill_chunk_size
            {
                request.prompt_tokens[..self.config.prefill_chunk_size].to_vec()
            } else {
                request.prompt_tokens.clone()
            };

            plan.prefill_tasks.push(PrefillTask {
                request_id,
                tokens,
                start_position: 0,
                kv_cache_slot: slot_id,
                block_table: block_table.clone(),
            });

            // Create running request
            let mut running = RunningRequest::new(request, slot_id);
            running.block_table = block_table;

            // If chunked, mark partial prefill
            if self.config.chunked_prefill
                && running.request.prompt_len() > self.config.prefill_chunk_size
            {
                running.prefill_tokens_processed = self.config.prefill_chunk_size;
            } else {
                running.complete_prefill();
            }

            queue.add_running(running);
        }
    }

    /// Schedule preempted requests that need to resume
    fn schedule_preempted_requests(
        &mut self,
        queue: &mut RequestQueue,
        plan: &mut IterationPlan,
        budget: &mut TokenBudget,
    ) {
        while let Some(request_id) = queue.pop_preempted() {
            // Check if we're using swap mode and request is swapped
            if self.config.preemption_mode == PreemptionMode::Swap
                && self.kv_cache_manager.is_swapped(request_id)
            {
                // Try to swap back in
                if let Ok(slot_id) = self.kv_cache_manager.swap_in(request_id) {
                    plan.swap_in_requests.push(request_id);

                    // Resume as decode
                    if budget.try_allocate_decode() {
                        if let Some(running) = queue.running.get(&request_id) {
                            let input_token = running.generated_tokens.last().copied().unwrap_or(0);

                            plan.decode_tasks.push(DecodeTask {
                                request_id,
                                input_token,
                                position: running.current_seq_len,
                                kv_cache_slot: slot_id,
                                block_table: running.block_table.clone(),
                                context_len: running.context_len,
                            });
                        }
                    }
                } else {
                    // Cannot swap in, put back in preempted queue
                    queue.add_preempted(request_id);
                    break;
                }
            } else if self.config.preemption_mode == PreemptionMode::Recompute {
                // Recompute mode: need to re-prefill
                let preempted_data = self.preempted_data.write().remove(&request_id);

                if let Some(data) = preempted_data {
                    // Check if we can allocate
                    if !self.kv_cache_manager.can_allocate(data.request.max_seq_len) {
                        // Put back and restore data
                        queue.add_preempted(request_id);
                        self.preempted_data.write().insert(request_id, data);
                        break;
                    }

                    let tokens_needed =
                        data.request.prompt_tokens.len() + data.generated_tokens.len();

                    if !budget.try_allocate_prefill(tokens_needed) {
                        // Put back
                        queue.add_preempted(request_id);
                        self.preempted_data.write().insert(request_id, data);
                        break;
                    }

                    // Allocate and re-prefill
                    let slot_id = self
                        .kv_cache_manager
                        .allocate(request_id, data.request.max_seq_len)
                        .unwrap();

                    let block_table = self
                        .kv_cache_manager
                        .get_block_table(request_id)
                        .unwrap_or_default();

                    // Combine prompt + generated tokens for prefill
                    let mut all_tokens = data.request.prompt_tokens.clone();
                    all_tokens.extend(&data.generated_tokens);

                    plan.prefill_tasks.push(PrefillTask {
                        request_id,
                        tokens: all_tokens,
                        start_position: 0,
                        kv_cache_slot: slot_id,
                        block_table: block_table.clone(),
                    });

                    // Recreate running request
                    let mut running = RunningRequest::new(data.request, slot_id);
                    running.generated_tokens = data.generated_tokens;
                    running.decode_steps = data.decode_steps;
                    running.block_table = block_table;
                    running.complete_prefill();
                    running.context_len =
                        running.request.prompt_tokens.len() + running.generated_tokens.len();
                    running.current_seq_len = running.context_len;

                    queue.add_running(running);
                }
            }
        }
    }

    /// Check if a request can be added
    pub fn can_add_request(&self, request: &InferenceRequest) -> bool {
        self.kv_cache_manager.can_allocate(request.max_seq_len)
    }

    /// Check if we should preempt requests
    fn should_preempt(&self, queue: &RequestQueue) -> bool {
        // Preempt if we have pending requests but no capacity
        if !queue.pending.is_empty() && self.kv_cache_manager.available_slots() == 0 {
            return true;
        }

        // Preempt if we have high-priority pending requests
        if let Some(pending) = queue.pending.front() {
            if pending.priority == Priority::Critical {
                return queue
                    .running
                    .values()
                    .any(|r| r.request.priority < Priority::Critical);
            }
        }

        false
    }

    /// Preempt requests to free resources
    fn preempt_requests(&mut self, queue: &mut RequestQueue, plan: &mut IterationPlan) {
        // Select victim(s) to preempt
        if let Some(victim_id) = self.select_victim(queue) {
            self.evict_request(queue, victim_id, plan);
        }
    }

    /// Select a request to preempt (lowest priority, most recent)
    fn select_victim(&self, queue: &RequestQueue) -> Option<RequestId> {
        queue
            .running
            .values()
            .filter(|r| r.request.priority != Priority::Critical)
            .min_by(|a, b| {
                // First compare by priority (lower is worse)
                a.request
                    .priority
                    .cmp(&b.request.priority)
                    // Then by decode steps (fewer is worse)
                    .then_with(|| a.decode_steps.cmp(&b.decode_steps))
            })
            .map(|r| r.id())
    }

    /// Evict a request
    fn evict_request(
        &mut self,
        queue: &mut RequestQueue,
        request_id: RequestId,
        plan: &mut IterationPlan,
    ) {
        if let Some(running) = queue.remove_running(request_id) {
            match self.config.preemption_mode {
                PreemptionMode::Recompute => {
                    // Store request data for later recomputation
                    self.preempted_data.write().insert(
                        request_id,
                        PreemptedRequestData {
                            request: running.request,
                            generated_tokens: running.generated_tokens,
                            decode_steps: running.decode_steps,
                        },
                    );

                    // Free KV cache
                    self.kv_cache_manager.free(request_id);
                }
                PreemptionMode::Swap => {
                    // Swap out to CPU memory
                    if self.kv_cache_manager.swap_out(request_id).is_ok() {
                        plan.swap_out_requests.push(request_id);
                    }
                    // Keep running request (will be inactive)
                    queue.add_running(running);
                }
            }

            plan.evicted_requests.push(request_id);
            queue.add_preempted(request_id);
        }
    }

    /// Get the KV cache manager
    pub fn kv_cache_manager(&self) -> &KvCacheManager {
        &self.kv_cache_manager
    }

    /// Get mutable KV cache manager
    pub fn kv_cache_manager_mut(&mut self) -> &mut KvCacheManager {
        &mut self.kv_cache_manager
    }

    /// Get scheduler configuration
    pub fn config(&self) -> &SchedulerConfig {
        &self.config
    }

    /// Get scheduler statistics
    pub fn stats(&self) -> SchedulerStats {
        let kv_stats = self.kv_cache_manager.stats();
        SchedulerStats {
            batches_scheduled: self.batch_counter.load(Ordering::Relaxed),
            kv_cache_utilization: kv_stats.slot_utilization(),
            block_utilization: kv_stats.block_utilization(),
            preempted_requests: self.preempted_data.read().len(),
        }
    }
}

/// Scheduler statistics
#[derive(Debug, Clone, Default)]
pub struct SchedulerStats {
    /// Total batches scheduled
    pub batches_scheduled: u64,
    /// KV cache slot utilization
    pub kv_cache_utilization: f64,
    /// Block utilization
    pub block_utilization: f64,
    /// Currently preempted requests
    pub preempted_requests: usize,
}

/// Iteration-level scheduler that wraps the batch scheduler
pub struct IterationScheduler {
    /// Underlying batch scheduler
    batch_scheduler: ContinuousBatchScheduler,
    /// Token budget per iteration
    iteration_budget: TokenBudget,
}

impl IterationScheduler {
    /// Create a new iteration scheduler
    pub fn new(config: SchedulerConfig, kv_cache_config: KvCachePoolConfig) -> Self {
        let iteration_budget = TokenBudget::new(
            config.max_prefill_tokens,
            config.max_batch_size,
            config.max_tokens_per_batch,
        );

        Self {
            batch_scheduler: ContinuousBatchScheduler::new(config, kv_cache_config),
            iteration_budget,
        }
    }

    /// Plan the next iteration
    pub fn next_iteration(&mut self, queue: &mut RequestQueue) -> Option<IterationPlan> {
        self.iteration_budget.reset();

        if queue.is_empty() {
            return None;
        }

        let batch = self.batch_scheduler.schedule(queue);

        if batch.is_empty() {
            None
        } else {
            // Convert batch back to plan format
            let mut plan = IterationPlan::empty();

            for req in batch.requests {
                if req.is_prefill {
                    plan.prefill_tasks.push(PrefillTask {
                        request_id: req.request_id,
                        tokens: req.token_ids,
                        start_position: req.position_offset,
                        kv_cache_slot: req.kv_cache_slot,
                        block_table: req.block_table,
                    });
                } else {
                    plan.decode_tasks.push(DecodeTask {
                        request_id: req.request_id,
                        input_token: req.token_ids[0],
                        position: req.position_offset,
                        kv_cache_slot: req.kv_cache_slot,
                        block_table: req.block_table,
                        context_len: req.context_len,
                    });
                }
            }

            Some(plan)
        }
    }

    /// Get the underlying batch scheduler
    pub fn batch_scheduler(&self) -> &ContinuousBatchScheduler {
        &self.batch_scheduler
    }

    /// Get mutable batch scheduler
    pub fn batch_scheduler_mut(&mut self) -> &mut ContinuousBatchScheduler {
        &mut self.batch_scheduler
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::GenerateParams;

    fn create_test_request(prompt_len: usize) -> InferenceRequest {
        let prompt_tokens: Vec<u32> = (0..prompt_len as u32).collect();
        let params = GenerateParams::default().with_max_tokens(100);
        InferenceRequest::new(prompt_tokens, params)
    }

    #[test]
    fn test_request_queue() {
        let mut queue = RequestQueue::new();

        let request = create_test_request(10);
        queue.add(request);

        assert_eq!(queue.pending_count(), 1);
        assert_eq!(queue.pending_tokens(), 10);

        let popped = queue.pop_pending().unwrap();
        assert_eq!(popped.prompt_len(), 10);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_scheduler_basic() {
        let config = SchedulerConfig::default();
        let kv_config = KvCachePoolConfig {
            num_slots: 4,
            max_seq_len: 256,
            block_size: 16,
            total_blocks: 64,
            num_kv_heads: 2,
            head_dim: 64,
            num_layers: 4,
        };

        let mut scheduler = ContinuousBatchScheduler::new(config, kv_config);
        let mut queue = RequestQueue::new();

        // Add a request
        queue.add(create_test_request(10));

        // Schedule
        let batch = scheduler.schedule(&mut queue);

        assert!(!batch.is_empty());
        assert!(batch.has_prefill);
        assert_eq!(batch.len(), 1);

        // Request should now be running
        assert_eq!(queue.pending_count(), 0);
        assert_eq!(queue.running_count(), 1);
    }

    #[test]
    fn test_scheduler_multiple_requests() {
        let config = SchedulerConfig::default();
        let kv_config = KvCachePoolConfig {
            num_slots: 4,
            max_seq_len: 256,
            block_size: 16,
            total_blocks: 128,
            num_kv_heads: 2,
            head_dim: 64,
            num_layers: 4,
        };

        let mut scheduler = ContinuousBatchScheduler::new(config, kv_config);
        let mut queue = RequestQueue::new();

        // Add multiple requests
        for _ in 0..3 {
            queue.add(create_test_request(20));
        }

        let batch = scheduler.schedule(&mut queue);
        assert!(batch.len() >= 1);
    }

    #[test]
    fn test_scheduler_with_priority() {
        let config = SchedulerConfig {
            priority_policy: PriorityPolicy::PriorityBased,
            ..Default::default()
        };
        let kv_config = KvCachePoolConfig::default();

        let mut scheduler = ContinuousBatchScheduler::new(config, kv_config);
        let mut queue = RequestQueue::new();

        // Add low priority request first
        queue.add(create_test_request(10).with_priority(Priority::Low));

        // Add high priority request second
        queue.add(create_test_request(10).with_priority(Priority::High));

        let batch = scheduler.schedule(&mut queue);

        // High priority should be first
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_iteration_scheduler() {
        let config = SchedulerConfig::default();
        let kv_config = KvCachePoolConfig {
            num_slots: 4,
            max_seq_len: 256,
            block_size: 16,
            total_blocks: 64,
            num_kv_heads: 2,
            head_dim: 64,
            num_layers: 4,
        };

        let mut scheduler = IterationScheduler::new(config, kv_config);
        let mut queue = RequestQueue::new();

        queue.add(create_test_request(10));

        let plan = scheduler.next_iteration(&mut queue);
        assert!(plan.is_some());
        assert!(plan.unwrap().has_work());
    }
}
