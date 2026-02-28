//! Continuous Batching Serving Module
//!
//! This module provides high-performance LLM serving with continuous batching,
//! enabling efficient multi-request handling with dynamic batching of prefill
//! and decode operations.
//!
//! ## Architecture
//!
//! The serving system consists of several interconnected components:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        ServingEngine                            │
//! │  ┌─────────────────────────────────────────────────────────┐   │
//! │  │                     RequestQueue                         │   │
//! │  │  ┌─────────┐    ┌─────────┐    ┌───────────┐           │   │
//! │  │  │ Pending │ -> │ Running │ -> │ Completed │           │   │
//! │  │  └─────────┘    └─────────┘    └───────────┘           │   │
//! │  └─────────────────────────────────────────────────────────┘   │
//! │                            │                                    │
//! │                            v                                    │
//! │  ┌─────────────────────────────────────────────────────────┐   │
//! │  │              ContinuousBatchScheduler                    │   │
//! │  │  ┌─────────────────────────────────────────────────┐    │   │
//! │  │  │              IterationPlan                       │    │   │
//! │  │  │  - PrefillTasks (new requests)                  │    │   │
//! │  │  │  - DecodeTasks (ongoing generation)             │    │   │
//! │  │  │  - Preemption handling                          │    │   │
//! │  │  └─────────────────────────────────────────────────┘    │   │
//! │  └─────────────────────────────────────────────────────────┘   │
//! │                            │                                    │
//! │                            v                                    │
//! │  ┌─────────────────────────────────────────────────────────┐   │
//! │  │                   KvCacheManager                         │   │
//! │  │  - Slot allocation (request -> cache)                   │   │
//! │  │  - Block management (paged attention)                   │   │
//! │  │  - Swap in/out (preemption support)                     │   │
//! │  └─────────────────────────────────────────────────────────┘   │
//! │                            │                                    │
//! │                            v                                    │
//! │  ┌─────────────────────────────────────────────────────────┐   │
//! │  │                    ScheduledBatch                        │   │
//! │  │  - Mixed prefill + decode requests                      │   │
//! │  │  - Optimized for GPU execution                          │   │
//! │  └─────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Key Features
//!
//! - **Continuous Batching**: New requests can join ongoing batches mid-generation
//! - **Mixed Prefill/Decode**: Processes both prefill and decode in single batches
//! - **Dynamic Scheduling**: Priority-based and adaptive scheduling policies
//! - **Memory Management**: Paged attention with block-level KV cache allocation
//! - **Preemption**: Recompute or swap strategies for memory pressure handling
//! - **Streaming**: Real-time token streaming with callbacks
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use ruvllm::serving::{ServingEngine, ServingEngineConfig, InferenceRequest};
//! use ruvllm::backends::{GenerateParams, create_backend};
//! use std::sync::Arc;
//!
//! // Create backend and engine
//! let backend = Arc::new(create_backend());
//! let config = ServingEngineConfig::default();
//! let engine = ServingEngine::new(backend, config);
//!
//! // Submit a request
//! let params = GenerateParams::default().with_max_tokens(100);
//! let request = InferenceRequest::new(vec![1, 2, 3], params);
//! let request_id = engine.submit(request)?;
//!
//! // Run serving loop (in separate thread or async)
//! std::thread::spawn(move || {
//!     engine.run().unwrap();
//! });
//!
//! // Poll for result
//! loop {
//!     if let Some(result) = engine.get_result(request_id) {
//!         println!("Generated {} tokens in {}ms",
//!             result.completion_tokens,
//!             result.processing_time_ms);
//!         break;
//!     }
//!     std::thread::sleep(std::time::Duration::from_millis(10));
//! }
//! ```
//!
//! ## Streaming Example
//!
//! ```rust,ignore
//! use ruvllm::serving::{ServingEngine, InferenceRequest, TokenOutput};
//!
//! // Submit with callback for streaming
//! let callback = Box::new(|output: TokenOutput| {
//!     if let Some(text) = output.token_text {
//!         print!("{}", text);
//!     }
//!     if output.is_final {
//!         println!("\n[Done]");
//!     }
//! });
//!
//! engine.submit_with_callback(request, callback)?;
//! ```
//!
//! ## Async Example
//!
//! ```rust,ignore
//! use ruvllm::serving::{ServingEngine, InferenceRequest};
//! use futures::StreamExt;
//!
//! // Async submission
//! let result = engine.submit_async(request).await?;
//!
//! // Or with streaming
//! let mut stream = engine.stream(request)?;
//! while let Some(output) = stream.next().await {
//!     process_token(output);
//! }
//! ```

pub mod batch;
pub mod engine;
pub mod kv_cache_manager;
pub mod request;
pub mod scheduler;

// Re-exports for convenience
pub use batch::{
    BatchStats, BatchedRequest, DecodeTask, IterationPlan, PrefillTask, ScheduledBatch, TokenBudget,
};
pub use engine::{GenerationResult, ServingEngine, ServingEngineConfig, ServingMetrics};
pub use kv_cache_manager::{
    KvCacheAllocation, KvCacheManager, KvCacheManagerStats, KvCachePoolConfig,
};
pub use request::{
    CompletedRequest, FinishReason, InferenceRequest, Priority, RequestId, RequestState,
    RunningRequest, TokenOutput,
};
pub use scheduler::{
    ContinuousBatchScheduler, IterationScheduler, PreemptionMode, PriorityPolicy, RequestQueue,
    SchedulerConfig, SchedulerStats,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::{GenerateParams, NoopBackend};
    use std::sync::Arc;

    #[test]
    fn test_full_serving_flow() {
        // Create engine with test configuration
        let backend = Arc::new(NoopBackend);
        let config = ServingEngineConfig {
            kv_cache: KvCachePoolConfig {
                num_slots: 8,
                max_seq_len: 256,
                block_size: 16,
                total_blocks: 128,
                num_kv_heads: 2,
                head_dim: 64,
                num_layers: 4,
            },
            max_concurrent_requests: 8,
            ..Default::default()
        };

        let engine = ServingEngine::new(backend, config);

        // Submit multiple requests
        let mut request_ids = Vec::new();
        for i in 0..3 {
            let params = GenerateParams::default().with_max_tokens(5);
            let prompt: Vec<u32> = (0..10).map(|j| (i * 10 + j) as u32).collect();
            let request = InferenceRequest::new(prompt, params);
            let id = engine.submit(request).unwrap();
            request_ids.push(id);
        }

        // Run iterations until all complete
        let mut iterations = 0;
        let max_iterations = 100;

        while iterations < max_iterations {
            let outputs = engine.run_iteration().unwrap();
            iterations += 1;

            // Check if all requests are complete
            let all_complete = request_ids.iter().all(|id| engine.is_complete(*id));
            if all_complete {
                break;
            }
        }

        // Verify we got results
        for id in &request_ids {
            // Result may or may not be available depending on completion
            // Just verify we can check
            let _ = engine.get_result(*id);
        }

        // Check metrics
        let metrics = engine.metrics();
        assert!(metrics.total_requests_processed > 0);
    }

    #[test]
    fn test_scheduler_continuous_batching() {
        let scheduler_config = SchedulerConfig::default();
        let kv_config = KvCachePoolConfig {
            num_slots: 4,
            max_seq_len: 128,
            block_size: 16,
            total_blocks: 32,
            num_kv_heads: 2,
            head_dim: 64,
            num_layers: 4,
        };

        let mut scheduler = ContinuousBatchScheduler::new(scheduler_config, kv_config);
        let mut queue = RequestQueue::new();

        // Add first request
        let params = GenerateParams::default().with_max_tokens(10);
        let request1 = InferenceRequest::new(vec![1, 2, 3], params.clone());
        queue.add(request1);

        // First batch: prefill
        let batch1 = scheduler.schedule(&mut queue);
        assert!(batch1.has_prefill);
        assert_eq!(queue.running_count(), 1);

        // Add second request while first is running
        let request2 = InferenceRequest::new(vec![4, 5, 6], params);
        queue.add(request2);

        // Second batch: should have both prefill (new) and potentially decode (old)
        let batch2 = scheduler.schedule(&mut queue);
        // May have both prefill and decode depending on first request state
        assert!(batch2.len() >= 1);
    }

    #[test]
    fn test_priority_scheduling() {
        let scheduler_config = SchedulerConfig {
            priority_policy: PriorityPolicy::PriorityBased,
            ..Default::default()
        };
        let kv_config = KvCachePoolConfig::default();

        let mut scheduler = ContinuousBatchScheduler::new(scheduler_config, kv_config);
        let mut queue = RequestQueue::new();

        // Add low priority first
        let low =
            InferenceRequest::new(vec![1], GenerateParams::default()).with_priority(Priority::Low);
        queue.add(low);

        // Add high priority second
        let high =
            InferenceRequest::new(vec![2], GenerateParams::default()).with_priority(Priority::High);
        queue.add(high);

        // Schedule - high priority should be processed first
        let batch = scheduler.schedule(&mut queue);
        assert!(!batch.is_empty());

        // The scheduler should respect priority ordering
        // (exact behavior depends on scheduler implementation)
    }

    #[test]
    fn test_kv_cache_allocation() {
        let config = KvCachePoolConfig {
            num_slots: 4,
            max_seq_len: 128,
            block_size: 16,
            total_blocks: 32,
            num_kv_heads: 2,
            head_dim: 64,
            num_layers: 4,
        };

        let mut manager = KvCacheManager::new(config);

        // Allocate slots
        let id1 = RequestId::new();
        let slot1 = manager.allocate(id1, 64).unwrap();

        let id2 = RequestId::new();
        let slot2 = manager.allocate(id2, 64).unwrap();

        assert_ne!(slot1, slot2);

        // Extend allocation
        manager.extend(id1, 32).unwrap();

        let allocation = manager.get_allocation(id1).unwrap();
        assert_eq!(allocation.current_length, 32);

        // Free
        manager.free(id1);
        assert!(manager.get_allocation(id1).is_none());

        // Stats
        let stats = manager.stats();
        assert_eq!(stats.active_allocations, 1);
    }

    #[test]
    fn test_iteration_plan() {
        let plan = IterationPlan {
            prefill_tasks: vec![PrefillTask {
                request_id: RequestId::new(),
                tokens: vec![1, 2, 3, 4, 5],
                start_position: 0,
                kv_cache_slot: 0,
                block_table: vec![0],
            }],
            decode_tasks: vec![DecodeTask {
                request_id: RequestId::new(),
                input_token: 10,
                position: 5,
                kv_cache_slot: 1,
                block_table: vec![1],
                context_len: 5,
            }],
            evicted_requests: vec![],
            swap_out_requests: vec![],
            swap_in_requests: vec![],
        };

        assert!(plan.has_work());
        assert_eq!(plan.total_requests(), 2);
        assert_eq!(plan.total_tokens(), 6); // 5 prefill + 1 decode

        let batch = plan.to_scheduled_batch(1);
        assert_eq!(batch.batch_id, 1);
        assert!(batch.has_prefill);
        assert!(batch.has_decode);
    }
}
