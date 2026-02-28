//! Batch management for continuous batching
//!
//! This module provides structures for organizing requests into
//! efficient batches that can be processed together by the model.

use super::request::{RequestId, RunningRequest};
use std::collections::HashMap;

/// A request that has been prepared for batch processing
#[derive(Debug, Clone)]
pub struct BatchedRequest {
    /// Request identifier
    pub request_id: RequestId,
    /// Token IDs to process in this batch iteration
    pub token_ids: Vec<u32>,
    /// Position offset for this request's tokens
    pub position_offset: usize,
    /// KV cache slot assignment
    pub kv_cache_slot: usize,
    /// Block table for paged attention
    pub block_table: Vec<usize>,
    /// Whether this is a prefill (true) or decode (false) request
    pub is_prefill: bool,
    /// Sequence length including new tokens
    pub seq_len: usize,
    /// Context length (tokens already in cache)
    pub context_len: usize,
}

impl BatchedRequest {
    /// Create a prefill batch request
    pub fn prefill(
        request_id: RequestId,
        token_ids: Vec<u32>,
        kv_cache_slot: usize,
        block_table: Vec<usize>,
    ) -> Self {
        let seq_len = token_ids.len();
        Self {
            request_id,
            token_ids,
            position_offset: 0,
            kv_cache_slot,
            block_table,
            is_prefill: true,
            seq_len,
            context_len: 0,
        }
    }

    /// Create a decode batch request
    pub fn decode(
        request_id: RequestId,
        token_id: u32,
        position_offset: usize,
        kv_cache_slot: usize,
        block_table: Vec<usize>,
        context_len: usize,
    ) -> Self {
        Self {
            request_id,
            token_ids: vec![token_id],
            position_offset,
            kv_cache_slot,
            block_table,
            is_prefill: false,
            seq_len: context_len + 1,
            context_len,
        }
    }

    /// Get the number of tokens in this batch request
    pub fn num_tokens(&self) -> usize {
        self.token_ids.len()
    }
}

/// A scheduled batch ready for model execution
#[derive(Debug)]
pub struct ScheduledBatch {
    /// Batched requests
    pub requests: Vec<BatchedRequest>,
    /// Total tokens in this batch
    pub total_tokens: usize,
    /// Whether this batch contains any prefill operations
    pub has_prefill: bool,
    /// Whether this batch contains any decode operations
    pub has_decode: bool,
    /// Maximum sequence length in the batch
    pub max_seq_len: usize,
    /// Batch ID for tracking
    pub batch_id: u64,
}

impl ScheduledBatch {
    /// Create an empty batch
    pub fn new(batch_id: u64) -> Self {
        Self {
            requests: Vec::new(),
            total_tokens: 0,
            has_prefill: false,
            has_decode: false,
            max_seq_len: 0,
            batch_id,
        }
    }

    /// Add a batched request
    pub fn add(&mut self, request: BatchedRequest) {
        self.total_tokens += request.num_tokens();
        self.has_prefill |= request.is_prefill;
        self.has_decode |= !request.is_prefill;
        self.max_seq_len = self.max_seq_len.max(request.seq_len);
        self.requests.push(request);
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Get the number of requests in the batch
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Get request IDs in the batch
    pub fn request_ids(&self) -> Vec<RequestId> {
        self.requests.iter().map(|r| r.request_id).collect()
    }

    /// Merge prefill and decode requests into a single batch
    ///
    /// This is key for continuous batching efficiency - we can process
    /// both prefill and decode requests in a single forward pass.
    pub fn merge_prefill_decode(
        prefill: Vec<BatchedRequest>,
        decode: Vec<BatchedRequest>,
        batch_id: u64,
    ) -> Self {
        let mut batch = Self::new(batch_id);

        // Add all prefill requests first
        for req in prefill {
            batch.add(req);
        }

        // Then add decode requests
        for req in decode {
            batch.add(req);
        }

        batch
    }

    /// Get the batch as separated prefill and decode requests
    pub fn split_by_type(&self) -> (Vec<&BatchedRequest>, Vec<&BatchedRequest>) {
        let prefill: Vec<_> = self.requests.iter().filter(|r| r.is_prefill).collect();
        let decode: Vec<_> = self.requests.iter().filter(|r| !r.is_prefill).collect();
        (prefill, decode)
    }

    /// Collect all input token IDs (padded for batch processing)
    pub fn collect_input_ids(&self) -> Vec<Vec<u32>> {
        self.requests.iter().map(|r| r.token_ids.clone()).collect()
    }

    /// Collect position offsets
    pub fn collect_positions(&self) -> Vec<usize> {
        self.requests.iter().map(|r| r.position_offset).collect()
    }

    /// Collect KV cache slots
    pub fn collect_kv_slots(&self) -> Vec<usize> {
        self.requests.iter().map(|r| r.kv_cache_slot).collect()
    }

    /// Calculate batch statistics
    pub fn stats(&self) -> BatchStats {
        let prefill_count = self.requests.iter().filter(|r| r.is_prefill).count();
        let decode_count = self.requests.len() - prefill_count;

        let prefill_tokens: usize = self
            .requests
            .iter()
            .filter(|r| r.is_prefill)
            .map(|r| r.num_tokens())
            .sum();

        BatchStats {
            batch_id: self.batch_id,
            total_requests: self.requests.len(),
            prefill_requests: prefill_count,
            decode_requests: decode_count,
            total_tokens: self.total_tokens,
            prefill_tokens,
            decode_tokens: self.total_tokens - prefill_tokens,
            max_seq_len: self.max_seq_len,
        }
    }
}

/// Statistics for a scheduled batch
#[derive(Debug, Clone, Default)]
pub struct BatchStats {
    /// Batch identifier
    pub batch_id: u64,
    /// Total number of requests
    pub total_requests: usize,
    /// Number of prefill requests
    pub prefill_requests: usize,
    /// Number of decode requests
    pub decode_requests: usize,
    /// Total tokens in batch
    pub total_tokens: usize,
    /// Tokens from prefill operations
    pub prefill_tokens: usize,
    /// Tokens from decode operations
    pub decode_tokens: usize,
    /// Maximum sequence length
    pub max_seq_len: usize,
}

/// Prefill task for iteration scheduling
#[derive(Debug, Clone)]
pub struct PrefillTask {
    /// Request ID
    pub request_id: RequestId,
    /// Tokens to prefill
    pub tokens: Vec<u32>,
    /// Starting position
    pub start_position: usize,
    /// KV cache slot
    pub kv_cache_slot: usize,
    /// Block table
    pub block_table: Vec<usize>,
}

/// Decode task for iteration scheduling
#[derive(Debug, Clone)]
pub struct DecodeTask {
    /// Request ID
    pub request_id: RequestId,
    /// Token to decode from
    pub input_token: u32,
    /// Position offset
    pub position: usize,
    /// KV cache slot
    pub kv_cache_slot: usize,
    /// Block table
    pub block_table: Vec<usize>,
    /// Context length
    pub context_len: usize,
}

/// Plan for a single iteration of the serving loop
#[derive(Debug)]
pub struct IterationPlan {
    /// Prefill tasks to execute
    pub prefill_tasks: Vec<PrefillTask>,
    /// Decode tasks to execute
    pub decode_tasks: Vec<DecodeTask>,
    /// Requests that were evicted due to preemption
    pub evicted_requests: Vec<RequestId>,
    /// Requests that should be swapped out
    pub swap_out_requests: Vec<RequestId>,
    /// Requests that should be swapped in
    pub swap_in_requests: Vec<RequestId>,
}

impl IterationPlan {
    /// Create an empty iteration plan
    pub fn empty() -> Self {
        Self {
            prefill_tasks: Vec::new(),
            decode_tasks: Vec::new(),
            evicted_requests: Vec::new(),
            swap_out_requests: Vec::new(),
            swap_in_requests: Vec::new(),
        }
    }

    /// Check if there's work to do
    pub fn has_work(&self) -> bool {
        !self.prefill_tasks.is_empty() || !self.decode_tasks.is_empty()
    }

    /// Total number of requests to process
    pub fn total_requests(&self) -> usize {
        self.prefill_tasks.len() + self.decode_tasks.len()
    }

    /// Total tokens to process
    pub fn total_tokens(&self) -> usize {
        let prefill_tokens: usize = self.prefill_tasks.iter().map(|t| t.tokens.len()).sum();
        let decode_tokens = self.decode_tasks.len(); // Each decode is 1 token
        prefill_tokens + decode_tokens
    }

    /// Convert to a scheduled batch
    pub fn to_scheduled_batch(&self, batch_id: u64) -> ScheduledBatch {
        let prefill: Vec<BatchedRequest> = self
            .prefill_tasks
            .iter()
            .map(|t| {
                BatchedRequest::prefill(
                    t.request_id,
                    t.tokens.clone(),
                    t.kv_cache_slot,
                    t.block_table.clone(),
                )
            })
            .collect();

        let decode: Vec<BatchedRequest> = self
            .decode_tasks
            .iter()
            .map(|t| {
                BatchedRequest::decode(
                    t.request_id,
                    t.input_token,
                    t.position,
                    t.kv_cache_slot,
                    t.block_table.clone(),
                    t.context_len,
                )
            })
            .collect();

        ScheduledBatch::merge_prefill_decode(prefill, decode, batch_id)
    }
}

/// Token budget for iteration scheduling
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Maximum tokens for prefill operations
    pub max_prefill_tokens: usize,
    /// Maximum tokens for decode operations (usually = max_batch_size)
    pub max_decode_tokens: usize,
    /// Maximum total tokens per iteration
    pub max_total_tokens: usize,
    /// Current prefill tokens allocated
    pub prefill_tokens: usize,
    /// Current decode tokens allocated
    pub decode_tokens: usize,
}

impl TokenBudget {
    /// Create a new token budget
    pub fn new(max_prefill: usize, max_decode: usize, max_total: usize) -> Self {
        Self {
            max_prefill_tokens: max_prefill,
            max_decode_tokens: max_decode,
            max_total_tokens: max_total,
            prefill_tokens: 0,
            decode_tokens: 0,
        }
    }

    /// Reset the budget for a new iteration
    pub fn reset(&mut self) {
        self.prefill_tokens = 0;
        self.decode_tokens = 0;
    }

    /// Total tokens currently allocated
    pub fn total_tokens(&self) -> usize {
        self.prefill_tokens + self.decode_tokens
    }

    /// Remaining capacity for prefill tokens
    pub fn remaining_prefill(&self) -> usize {
        let from_prefill_limit = self.max_prefill_tokens.saturating_sub(self.prefill_tokens);
        let from_total_limit = self.max_total_tokens.saturating_sub(self.total_tokens());
        from_prefill_limit.min(from_total_limit)
    }

    /// Remaining capacity for decode tokens
    pub fn remaining_decode(&self) -> usize {
        let from_decode_limit = self.max_decode_tokens.saturating_sub(self.decode_tokens);
        let from_total_limit = self.max_total_tokens.saturating_sub(self.total_tokens());
        from_decode_limit.min(from_total_limit)
    }

    /// Try to allocate prefill tokens
    pub fn try_allocate_prefill(&mut self, tokens: usize) -> bool {
        if tokens <= self.remaining_prefill() {
            self.prefill_tokens += tokens;
            true
        } else {
            false
        }
    }

    /// Try to allocate a decode token
    pub fn try_allocate_decode(&mut self) -> bool {
        if self.remaining_decode() > 0 {
            self.decode_tokens += 1;
            true
        } else {
            false
        }
    }

    /// Check if budget is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.total_tokens() >= self.max_total_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batched_request() {
        let prefill = BatchedRequest::prefill(RequestId::new(), vec![1, 2, 3, 4], 0, vec![0, 1]);
        assert!(prefill.is_prefill);
        assert_eq!(prefill.num_tokens(), 4);
        assert_eq!(prefill.seq_len, 4);

        let decode = BatchedRequest::decode(RequestId::new(), 5, 10, 1, vec![0, 1, 2], 10);
        assert!(!decode.is_prefill);
        assert_eq!(decode.num_tokens(), 1);
        assert_eq!(decode.context_len, 10);
    }

    #[test]
    fn test_scheduled_batch() {
        let mut batch = ScheduledBatch::new(1);

        batch.add(BatchedRequest::prefill(
            RequestId::new(),
            vec![1, 2, 3],
            0,
            vec![],
        ));
        batch.add(BatchedRequest::decode(RequestId::new(), 4, 5, 1, vec![], 5));

        assert_eq!(batch.len(), 2);
        assert!(batch.has_prefill);
        assert!(batch.has_decode);
        assert_eq!(batch.total_tokens, 4); // 3 prefill + 1 decode

        let (prefill, decode) = batch.split_by_type();
        assert_eq!(prefill.len(), 1);
        assert_eq!(decode.len(), 1);
    }

    #[test]
    fn test_token_budget() {
        let mut budget = TokenBudget::new(100, 32, 128);

        assert!(budget.try_allocate_prefill(50));
        assert_eq!(budget.prefill_tokens, 50);
        assert_eq!(budget.remaining_prefill(), 50);

        assert!(budget.try_allocate_decode());
        assert_eq!(budget.decode_tokens, 1);

        // Should fail - exceeds prefill limit
        assert!(!budget.try_allocate_prefill(60));

        budget.reset();
        assert_eq!(budget.total_tokens(), 0);
    }

    #[test]
    fn test_iteration_plan() {
        let plan = IterationPlan {
            prefill_tasks: vec![PrefillTask {
                request_id: RequestId::new(),
                tokens: vec![1, 2, 3, 4, 5],
                start_position: 0,
                kv_cache_slot: 0,
                block_table: vec![],
            }],
            decode_tasks: vec![DecodeTask {
                request_id: RequestId::new(),
                input_token: 6,
                position: 10,
                kv_cache_slot: 1,
                block_table: vec![],
                context_len: 10,
            }],
            evicted_requests: vec![],
            swap_out_requests: vec![],
            swap_in_requests: vec![],
        };

        assert!(plan.has_work());
        assert_eq!(plan.total_requests(), 2);
        assert_eq!(plan.total_tokens(), 6); // 5 prefill + 1 decode

        let batch = plan.to_scheduled_batch(42);
        assert_eq!(batch.batch_id, 42);
        assert_eq!(batch.len(), 2);
    }
}
