//! Request types for the continuous batching serving engine
//!
//! This module defines the core request structures used throughout
//! the serving system, including inference requests, running requests,
//! and completed requests.

use crate::backends::GenerateParams;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use uuid::Uuid;

/// Unique identifier for a request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub Uuid);

impl RequestId {
    /// Create a new random request ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a request ID from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Priority level for request scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    /// Lowest priority - background tasks
    Low = 0,
    /// Normal priority - default
    Normal = 1,
    /// High priority - interactive requests
    High = 2,
    /// Critical priority - system requests
    Critical = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

impl Priority {
    /// Get numeric value for comparison
    pub fn value(&self) -> u8 {
        *self as u8
    }
}

/// State of a request in the serving pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RequestState {
    /// Request is waiting to be scheduled
    Pending,
    /// Request is being processed (prefill or decode)
    Running,
    /// Request has been preempted and is waiting to resume
    Preempted,
    /// Request has completed successfully
    Completed,
    /// Request failed with an error
    Failed,
    /// Request was cancelled
    Cancelled,
}

/// An incoming inference request
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    /// Unique request identifier
    pub id: RequestId,
    /// Tokenized prompt
    pub prompt_tokens: Vec<u32>,
    /// Generation parameters
    pub params: GenerateParams,
    /// Request arrival time
    pub arrival_time: Instant,
    /// Request priority
    pub priority: Priority,
    /// Optional session ID for multi-turn conversations
    pub session_id: Option<String>,
    /// Maximum sequence length (prompt + generation)
    pub max_seq_len: usize,
    /// User-provided metadata
    pub metadata: Option<serde_json::Value>,
}

impl InferenceRequest {
    /// Create a new inference request
    pub fn new(prompt_tokens: Vec<u32>, params: GenerateParams) -> Self {
        let max_seq_len = prompt_tokens.len() + params.max_tokens;
        Self {
            id: RequestId::new(),
            prompt_tokens,
            params,
            arrival_time: Instant::now(),
            priority: Priority::Normal,
            session_id: None,
            max_seq_len,
            metadata: None,
        }
    }

    /// Set the priority
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Set the session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Get the number of prompt tokens
    pub fn prompt_len(&self) -> usize {
        self.prompt_tokens.len()
    }

    /// Get the maximum tokens to generate
    pub fn max_new_tokens(&self) -> usize {
        self.params.max_tokens
    }

    /// Time since request arrival
    pub fn waiting_time(&self) -> std::time::Duration {
        self.arrival_time.elapsed()
    }
}

/// A request that is currently being processed
#[derive(Debug)]
pub struct RunningRequest {
    /// Original request
    pub request: InferenceRequest,
    /// Generated tokens so far
    pub generated_tokens: Vec<u32>,
    /// KV cache slot assignment
    pub kv_cache_slot: usize,
    /// Current sequence length (prompt + generated)
    pub current_seq_len: usize,
    /// Number of prefill tokens processed
    pub prefill_tokens_processed: usize,
    /// Whether prefill is complete
    pub prefill_complete: bool,
    /// Start time of processing
    pub start_time: Instant,
    /// Last decode step time
    pub last_step_time: Instant,
    /// Number of decode steps completed
    pub decode_steps: usize,
    /// Current state
    pub state: RequestState,
    /// Block table for paged attention
    pub block_table: Vec<usize>,
    /// Number of context tokens in cache
    pub context_len: usize,
}

impl RunningRequest {
    /// Create a new running request from an inference request
    pub fn new(request: InferenceRequest, kv_cache_slot: usize) -> Self {
        let now = Instant::now();
        let prompt_len = request.prompt_tokens.len();
        Self {
            request,
            generated_tokens: Vec::new(),
            kv_cache_slot,
            current_seq_len: prompt_len,
            prefill_tokens_processed: 0,
            prefill_complete: false,
            start_time: now,
            last_step_time: now,
            decode_steps: 0,
            state: RequestState::Running,
            block_table: Vec::new(),
            context_len: 0,
        }
    }

    /// Get the request ID
    pub fn id(&self) -> RequestId {
        self.request.id
    }

    /// Add a generated token
    pub fn add_token(&mut self, token: u32) {
        self.generated_tokens.push(token);
        self.current_seq_len += 1;
        self.decode_steps += 1;
        self.last_step_time = Instant::now();
    }

    /// Check if generation is complete
    pub fn is_complete(&self) -> bool {
        // Check max tokens
        if self.generated_tokens.len() >= self.request.params.max_tokens {
            return true;
        }
        // Check for EOS token (if we had tokenizer info, we'd check here)
        false
    }

    /// Check if we should stop based on stop sequences
    pub fn should_stop(&self, _decoded_text: &str) -> bool {
        // Would check against stop_sequences in params
        // For now, just check token count
        self.is_complete()
    }

    /// Get total tokens (prompt + generated)
    pub fn total_tokens(&self) -> usize {
        self.current_seq_len
    }

    /// Get remaining tokens to generate
    pub fn remaining_tokens(&self) -> usize {
        self.request
            .params
            .max_tokens
            .saturating_sub(self.generated_tokens.len())
    }

    /// Get the position for the next token
    pub fn next_position(&self) -> usize {
        self.current_seq_len
    }

    /// Time since processing started
    pub fn processing_time(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Time since last decode step
    pub fn time_since_last_step(&self) -> std::time::Duration {
        self.last_step_time.elapsed()
    }

    /// Calculate tokens per second
    pub fn tokens_per_second(&self) -> f64 {
        let elapsed = self.processing_time().as_secs_f64();
        if elapsed > 0.0 && self.decode_steps > 0 {
            self.decode_steps as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Mark prefill as complete
    pub fn complete_prefill(&mut self) {
        self.prefill_complete = true;
        self.prefill_tokens_processed = self.request.prompt_tokens.len();
        self.context_len = self.prefill_tokens_processed;
    }

    /// Get tokens that need prefill processing
    pub fn get_prefill_tokens(&self) -> &[u32] {
        &self.request.prompt_tokens[self.prefill_tokens_processed..]
    }

    /// Mark some prefill tokens as processed
    pub fn advance_prefill(&mut self, count: usize) {
        self.prefill_tokens_processed += count;
        self.context_len = self.prefill_tokens_processed;
        if self.prefill_tokens_processed >= self.request.prompt_tokens.len() {
            self.prefill_complete = true;
        }
    }
}

/// Result of a completed request
#[derive(Debug, Clone)]
pub struct CompletedRequest {
    /// Request ID
    pub id: RequestId,
    /// Original prompt tokens
    pub prompt_tokens: Vec<u32>,
    /// Generated tokens
    pub generated_tokens: Vec<u32>,
    /// Final state
    pub state: RequestState,
    /// Total processing time
    pub processing_time_ms: u64,
    /// Time spent waiting
    pub waiting_time_ms: u64,
    /// Prefill time
    pub prefill_time_ms: u64,
    /// Decode time
    pub decode_time_ms: u64,
    /// Number of decode steps
    pub decode_steps: usize,
    /// Tokens per second during decode
    pub tokens_per_second: f64,
    /// Error message if failed
    pub error: Option<String>,
    /// Finish reason
    pub finish_reason: FinishReason,
}

/// Reason for request completion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FinishReason {
    /// Reached max tokens
    Length,
    /// Hit a stop sequence
    Stop,
    /// Hit EOS token
    EndOfSequence,
    /// Request was cancelled
    Cancelled,
    /// Request failed
    Error,
}

impl CompletedRequest {
    /// Create a successful completion
    pub fn success(running: &RunningRequest, prefill_time_ms: u64) -> Self {
        let processing_time = running.processing_time();
        let decode_time_ms = processing_time.as_millis() as u64 - prefill_time_ms;

        Self {
            id: running.id(),
            prompt_tokens: running.request.prompt_tokens.clone(),
            generated_tokens: running.generated_tokens.clone(),
            state: RequestState::Completed,
            processing_time_ms: processing_time.as_millis() as u64,
            waiting_time_ms: running.request.waiting_time().as_millis() as u64,
            prefill_time_ms,
            decode_time_ms,
            decode_steps: running.decode_steps,
            tokens_per_second: running.tokens_per_second(),
            error: None,
            finish_reason: if running.generated_tokens.len() >= running.request.params.max_tokens {
                FinishReason::Length
            } else {
                FinishReason::EndOfSequence
            },
        }
    }

    /// Create a failed completion
    pub fn failure(running: &RunningRequest, error: impl Into<String>) -> Self {
        Self {
            id: running.id(),
            prompt_tokens: running.request.prompt_tokens.clone(),
            generated_tokens: running.generated_tokens.clone(),
            state: RequestState::Failed,
            processing_time_ms: running.processing_time().as_millis() as u64,
            waiting_time_ms: running.request.waiting_time().as_millis() as u64,
            prefill_time_ms: 0,
            decode_time_ms: 0,
            decode_steps: running.decode_steps,
            tokens_per_second: running.tokens_per_second(),
            error: Some(error.into()),
            finish_reason: FinishReason::Error,
        }
    }

    /// Create a cancelled completion
    pub fn cancelled(running: &RunningRequest) -> Self {
        Self {
            id: running.id(),
            prompt_tokens: running.request.prompt_tokens.clone(),
            generated_tokens: running.generated_tokens.clone(),
            state: RequestState::Cancelled,
            processing_time_ms: running.processing_time().as_millis() as u64,
            waiting_time_ms: running.request.waiting_time().as_millis() as u64,
            prefill_time_ms: 0,
            decode_time_ms: 0,
            decode_steps: running.decode_steps,
            tokens_per_second: running.tokens_per_second(),
            error: None,
            finish_reason: FinishReason::Cancelled,
        }
    }

    /// Get total token count
    pub fn total_tokens(&self) -> usize {
        self.prompt_tokens.len() + self.generated_tokens.len()
    }
}

/// Output from a single token generation step
#[derive(Debug, Clone)]
pub struct TokenOutput {
    /// Request ID
    pub request_id: RequestId,
    /// Generated token ID
    pub token_id: u32,
    /// Token text (if decoded)
    pub token_text: Option<String>,
    /// Log probability
    pub logprob: Option<f32>,
    /// Whether this is the final token
    pub is_final: bool,
    /// Finish reason (if final)
    pub finish_reason: Option<FinishReason>,
    /// Current sequence length
    pub seq_len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id() {
        let id1 = RequestId::new();
        let id2 = RequestId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Low < Priority::Normal);
        assert!(Priority::Normal < Priority::High);
        assert!(Priority::High < Priority::Critical);
    }

    #[test]
    fn test_inference_request() {
        let params = GenerateParams::default();
        let request = InferenceRequest::new(vec![1, 2, 3], params)
            .with_priority(Priority::High)
            .with_session("session-123");

        assert_eq!(request.prompt_len(), 3);
        assert_eq!(request.priority, Priority::High);
        assert_eq!(request.session_id, Some("session-123".to_string()));
    }

    #[test]
    fn test_running_request() {
        let params = GenerateParams::default().with_max_tokens(10);
        let request = InferenceRequest::new(vec![1, 2, 3], params);
        let mut running = RunningRequest::new(request, 0);

        assert!(!running.is_complete());
        assert!(!running.prefill_complete);

        running.complete_prefill();
        assert!(running.prefill_complete);

        for i in 0..10 {
            running.add_token(i);
        }
        assert!(running.is_complete());
    }
}
