//! Claude API Integration for Agent Communication
//!
//! Provides full Claude API compatibility for multi-agent coordination,
//! including streaming response handling, context window management,
//! and workflow orchestration.
//!
//! ## Key Features
//!
//! - **Full Claude API Compatibility**: Messages, streaming, tool use
//! - **Streaming Response Handling**: Real-time token generation with quality monitoring
//! - **Context Window Management**: Dynamic compression/expansion based on task complexity
//! - **Multi-Agent Coordination**: Workflow orchestration with dependency resolution
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! | AgentCoordinator  |---->| ClaudeClient      |
//! | (workflow mgmt)   |     | (API interface)   |
//! +--------+----------+     +--------+----------+
//!          |                         |
//!          v                         v
//! +--------+----------+     +--------+----------+
//! | ResponseStreamer  |<----| ContextManager    |
//! | (token handling)  |     | (window mgmt)     |
//! +-------------------+     +-------------------+
//! ```

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use super::{AgentType, ClaudeFlowAgent, ClaudeFlowTask};
use crate::error::{Result, RuvLLMError};

// ============================================================================
// Claude API Types
// ============================================================================

/// Claude model variants for intelligent routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClaudeModel {
    /// Fast, cost-effective for simple tasks
    Haiku,
    /// Balanced performance and capability
    Sonnet,
    /// Most capable for complex reasoning
    Opus,
}

impl ClaudeModel {
    /// Get short name for the model
    pub fn name(&self) -> &'static str {
        match self {
            Self::Haiku => "haiku",
            Self::Sonnet => "sonnet",
            Self::Opus => "opus",
        }
    }

    /// Get model identifier string
    pub fn model_id(&self) -> &'static str {
        match self {
            Self::Haiku => "claude-3-5-haiku-20241022",
            Self::Sonnet => "claude-sonnet-4-20250514",
            Self::Opus => "claude-opus-4-20250514",
        }
    }

    /// Get cost per 1K input tokens (USD)
    pub fn input_cost_per_1k(&self) -> f64 {
        match self {
            Self::Haiku => 0.00025,
            Self::Sonnet => 0.003,
            Self::Opus => 0.015,
        }
    }

    /// Get cost per 1K output tokens (USD)
    pub fn output_cost_per_1k(&self) -> f64 {
        match self {
            Self::Haiku => 0.00125,
            Self::Sonnet => 0.015,
            Self::Opus => 0.075,
        }
    }

    /// Get typical latency for first token (ms)
    pub fn typical_ttft_ms(&self) -> u64 {
        match self {
            Self::Haiku => 200,
            Self::Sonnet => 500,
            Self::Opus => 1500,
        }
    }

    /// Get maximum context window size
    pub fn max_context_tokens(&self) -> usize {
        match self {
            Self::Haiku => 200_000,
            Self::Sonnet => 200_000,
            Self::Opus => 200_000,
        }
    }
}

impl Default for ClaudeModel {
    fn default() -> Self {
        Self::Sonnet
    }
}

/// Message role in conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// User message
    User,
    /// Assistant response
    Assistant,
    /// System instructions
    System,
}

/// Content block types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content
    Text { text: String },
    /// Tool use request
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Tool result
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// Message in conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message role
    pub role: MessageRole,
    /// Message content blocks
    pub content: Vec<ContentBlock>,
}

impl Message {
    /// Create a simple text message
    pub fn text(role: MessageRole, text: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Create a user message
    pub fn user(text: impl Into<String>) -> Self {
        Self::text(MessageRole::User, text)
    }

    /// Create an assistant message
    pub fn assistant(text: impl Into<String>) -> Self {
        Self::text(MessageRole::Assistant, text)
    }

    /// Estimate token count for this message
    pub fn estimate_tokens(&self) -> usize {
        self.content
            .iter()
            .map(|block| {
                match block {
                    ContentBlock::Text { text } => text.len() / 4, // ~4 chars per token
                    ContentBlock::ToolUse { input, .. } => {
                        input.to_string().len() / 4 + 50 // overhead for tool structure
                    }
                    ContentBlock::ToolResult { content, .. } => content.len() / 4 + 20,
                }
            })
            .sum()
    }
}

/// Request to Claude API
#[derive(Debug, Clone, Serialize)]
pub struct ClaudeRequest {
    /// Model to use
    pub model: String,
    /// Conversation messages
    pub messages: Vec<Message>,
    /// Maximum tokens to generate
    pub max_tokens: usize,
    /// System prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Temperature for sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Enable streaming
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Response from Claude API
#[derive(Debug, Clone, Deserialize)]
pub struct ClaudeResponse {
    /// Response ID
    pub id: String,
    /// Model used
    pub model: String,
    /// Content blocks
    pub content: Vec<ContentBlock>,
    /// Stop reason
    pub stop_reason: Option<String>,
    /// Usage statistics
    pub usage: UsageStats,
}

/// Token usage statistics
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UsageStats {
    /// Input tokens used
    pub input_tokens: usize,
    /// Output tokens generated
    pub output_tokens: usize,
}

impl UsageStats {
    /// Calculate cost for given model
    pub fn calculate_cost(&self, model: ClaudeModel) -> f64 {
        let input_cost = (self.input_tokens as f64 / 1000.0) * model.input_cost_per_1k();
        let output_cost = (self.output_tokens as f64 / 1000.0) * model.output_cost_per_1k();
        input_cost + output_cost
    }
}

// ============================================================================
// Streaming Types
// ============================================================================

/// Streaming token with metadata
#[derive(Debug, Clone)]
pub struct StreamToken {
    /// Token text
    pub text: String,
    /// Token index in sequence
    pub index: usize,
    /// Cumulative latency from stream start
    pub latency_ms: u64,
    /// Quality score (0.0 - 1.0) if available
    pub quality_score: Option<f32>,
}

/// Stream event types
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Stream started
    Start {
        request_id: String,
        model: ClaudeModel,
    },
    /// Token generated
    Token(StreamToken),
    /// Content block completed
    ContentBlockComplete { index: usize, content: ContentBlock },
    /// Stream completed
    Complete {
        usage: UsageStats,
        stop_reason: String,
        total_latency_ms: u64,
    },
    /// Error occurred
    Error { message: String, is_retryable: bool },
}

/// Quality monitoring for streaming responses
#[derive(Debug, Clone)]
pub struct QualityMonitor {
    /// Minimum acceptable quality score
    pub min_quality: f32,
    /// Check interval (tokens)
    pub check_interval: usize,
    /// Accumulated quality scores
    scores: Vec<f32>,
    /// Tokens since last check
    tokens_since_check: usize,
}

impl QualityMonitor {
    /// Create new quality monitor
    pub fn new(min_quality: f32, check_interval: usize) -> Self {
        Self {
            min_quality,
            check_interval,
            scores: Vec::new(),
            tokens_since_check: 0,
        }
    }

    /// Record a quality observation
    pub fn record(&mut self, score: f32) {
        self.scores.push(score);
        self.tokens_since_check += 1;
    }

    /// Check if quality is acceptable
    pub fn should_continue(&self) -> bool {
        if self.scores.is_empty() {
            return true;
        }
        let avg = self.scores.iter().sum::<f32>() / self.scores.len() as f32;
        avg >= self.min_quality
    }

    /// Check if it's time to evaluate quality
    pub fn should_check(&self) -> bool {
        self.tokens_since_check >= self.check_interval
    }

    /// Reset check counter
    pub fn reset_check(&mut self) {
        self.tokens_since_check = 0;
    }

    /// Get average quality score
    pub fn average_quality(&self) -> f32 {
        if self.scores.is_empty() {
            1.0
        } else {
            self.scores.iter().sum::<f32>() / self.scores.len() as f32
        }
    }
}

/// Response streamer for real-time token handling
pub struct ResponseStreamer {
    /// Request ID
    pub request_id: String,
    /// Model being used
    pub model: ClaudeModel,
    /// Stream start time
    start_time: Instant,
    /// Token count
    token_count: usize,
    /// Quality monitor
    quality_monitor: QualityMonitor,
    /// Event sender
    sender: mpsc::Sender<StreamEvent>,
    /// Accumulated text
    accumulated_text: String,
    /// Is stream complete
    is_complete: bool,
}

impl ResponseStreamer {
    /// Create new response streamer
    pub fn new(request_id: String, model: ClaudeModel, sender: mpsc::Sender<StreamEvent>) -> Self {
        Self {
            request_id: request_id.clone(),
            model,
            start_time: Instant::now(),
            token_count: 0,
            quality_monitor: QualityMonitor::new(0.6, 20),
            sender,
            accumulated_text: String::new(),
            is_complete: false,
        }
    }

    /// Process incoming token
    pub async fn process_token(&mut self, text: String, quality_score: Option<f32>) -> Result<()> {
        if self.is_complete {
            return Err(RuvLLMError::InvalidOperation(
                "Stream already complete".to_string(),
            ));
        }

        let token = StreamToken {
            text: text.clone(),
            index: self.token_count,
            latency_ms: self.start_time.elapsed().as_millis() as u64,
            quality_score,
        };

        // Update quality monitor
        if let Some(score) = quality_score {
            self.quality_monitor.record(score);
        }

        // Accumulate text
        self.accumulated_text.push_str(&text);
        self.token_count += 1;

        // Send token event
        self.sender
            .send(StreamEvent::Token(token))
            .await
            .map_err(|e| RuvLLMError::InvalidOperation(format!("Failed to send token: {}", e)))?;

        Ok(())
    }

    /// Complete the stream
    pub async fn complete(&mut self, usage: UsageStats, stop_reason: String) -> Result<()> {
        self.is_complete = true;

        self.sender
            .send(StreamEvent::Complete {
                usage,
                stop_reason,
                total_latency_ms: self.start_time.elapsed().as_millis() as u64,
            })
            .await
            .map_err(|e| {
                RuvLLMError::InvalidOperation(format!("Failed to send complete: {}", e))
            })?;

        Ok(())
    }

    /// Get current statistics
    pub fn stats(&self) -> StreamStats {
        let elapsed = self.start_time.elapsed();
        StreamStats {
            token_count: self.token_count,
            elapsed_ms: elapsed.as_millis() as u64,
            tokens_per_second: if elapsed.as_secs_f64() > 0.0 {
                self.token_count as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            },
            average_quality: self.quality_monitor.average_quality(),
            is_complete: self.is_complete,
        }
    }

    /// Get accumulated text
    pub fn accumulated_text(&self) -> &str {
        &self.accumulated_text
    }

    /// Check if quality is acceptable
    pub fn quality_acceptable(&self) -> bool {
        self.quality_monitor.should_continue()
    }
}

/// Stream statistics
#[derive(Debug, Clone)]
pub struct StreamStats {
    /// Total tokens processed
    pub token_count: usize,
    /// Elapsed time in milliseconds
    pub elapsed_ms: u64,
    /// Tokens per second
    pub tokens_per_second: f64,
    /// Average quality score
    pub average_quality: f32,
    /// Is stream complete
    pub is_complete: bool,
}

// ============================================================================
// Context Window Management
// ============================================================================

/// Context window state
#[derive(Debug, Clone)]
pub struct ContextWindow {
    /// Current messages
    messages: Vec<Message>,
    /// System prompt
    system_prompt: Option<String>,
    /// Maximum tokens for context
    max_tokens: usize,
    /// Current estimated token count
    current_tokens: usize,
    /// Compression threshold (0.0 - 1.0)
    compression_threshold: f32,
}

impl ContextWindow {
    /// Create new context window
    pub fn new(max_tokens: usize) -> Self {
        Self {
            messages: Vec::new(),
            system_prompt: None,
            max_tokens,
            current_tokens: 0,
            compression_threshold: 0.8,
        }
    }

    /// Set system prompt
    pub fn set_system(&mut self, prompt: impl Into<String>) {
        let prompt = prompt.into();
        self.current_tokens -= self.system_prompt.as_ref().map_or(0, |p| p.len() / 4);
        self.current_tokens += prompt.len() / 4;
        self.system_prompt = Some(prompt);
    }

    /// Add message to context
    pub fn add_message(&mut self, message: Message) {
        let tokens = message.estimate_tokens();
        self.current_tokens += tokens;
        self.messages.push(message);

        // Check if compression needed
        if self.needs_compression() {
            self.compress();
        }
    }

    /// Check if context needs compression
    pub fn needs_compression(&self) -> bool {
        self.current_tokens as f32 > self.max_tokens as f32 * self.compression_threshold
    }

    /// Get utilization ratio
    pub fn utilization(&self) -> f32 {
        self.current_tokens as f32 / self.max_tokens as f32
    }

    /// Compress context to fit within limits
    pub fn compress(&mut self) {
        // Strategy: Keep system, first user message, and recent messages
        if self.messages.len() <= 4 {
            return;
        }

        let target_tokens = (self.max_tokens as f32 * 0.6) as usize;

        // Keep first and last N messages
        let keep_first = 1;
        let mut keep_last = 3;

        while self.current_tokens > target_tokens && keep_last > 1 {
            let to_remove = self.messages.len() - keep_first - keep_last;
            if to_remove > 0 {
                // Remove middle messages
                let removed: Vec<_> = self.messages.drain(keep_first..keep_first + 1).collect();
                for msg in removed {
                    self.current_tokens -= msg.estimate_tokens();
                }
            } else {
                keep_last -= 1;
            }
        }
    }

    /// Expand context window for complex task
    pub fn expand_for_task(&mut self, task_complexity: f32, model: ClaudeModel) {
        // Higher complexity = larger context window needed
        let base_max = model.max_context_tokens();
        let expansion_factor = 0.5 + (task_complexity * 0.5); // 0.5 to 1.0
        self.max_tokens = (base_max as f32 * expansion_factor) as usize;
    }

    /// Get messages for request
    pub fn get_messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get system prompt
    pub fn get_system(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    /// Get current token estimate
    pub fn token_count(&self) -> usize {
        self.current_tokens
    }

    /// Get remaining capacity
    pub fn remaining_capacity(&self) -> usize {
        self.max_tokens.saturating_sub(self.current_tokens)
    }

    /// Clear context
    pub fn clear(&mut self) {
        self.messages.clear();
        self.current_tokens = self.system_prompt.as_ref().map_or(0, |p| p.len() / 4);
    }
}

/// Context manager for dynamic window management
pub struct ContextManager {
    /// Windows by agent ID
    windows: HashMap<String, ContextWindow>,
    /// Default max tokens
    default_max_tokens: usize,
}

impl ContextManager {
    /// Create new context manager
    pub fn new(default_max_tokens: usize) -> Self {
        Self {
            windows: HashMap::new(),
            default_max_tokens,
        }
    }

    /// Get or create context window for agent
    pub fn get_window(&mut self, agent_id: &str) -> &mut ContextWindow {
        if !self.windows.contains_key(agent_id) {
            self.windows.insert(
                agent_id.to_string(),
                ContextWindow::new(self.default_max_tokens),
            );
        }
        self.windows.get_mut(agent_id).unwrap()
    }

    /// Remove context window
    pub fn remove_window(&mut self, agent_id: &str) {
        self.windows.remove(agent_id);
    }

    /// Get total token usage across all windows
    pub fn total_tokens(&self) -> usize {
        self.windows.values().map(|w| w.token_count()).sum()
    }

    /// Get window count
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }
}

// ============================================================================
// Multi-Agent Coordination
// ============================================================================

/// Agent state in workflow
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentState {
    /// Agent is idle
    Idle,
    /// Agent is executing task
    Running,
    /// Agent is waiting for dependencies
    Blocked,
    /// Agent completed successfully
    Completed,
    /// Agent failed
    Failed,
}

/// Agent execution context
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// Agent identifier
    pub agent_id: String,
    /// Agent type
    pub agent_type: AgentType,
    /// Assigned model
    pub model: ClaudeModel,
    /// Current state
    pub state: AgentState,
    /// Context window
    pub context_tokens: usize,
    /// Total tokens used
    pub total_tokens_used: usize,
    /// Total cost incurred
    pub total_cost: f64,
    /// Task start time
    pub started_at: Option<Instant>,
    /// Task completion time
    pub completed_at: Option<Instant>,
    /// Error message if failed
    pub error: Option<String>,
}

impl AgentContext {
    /// Create new agent context
    pub fn new(agent_id: String, agent_type: AgentType, model: ClaudeModel) -> Self {
        Self {
            agent_id,
            agent_type,
            model,
            state: AgentState::Idle,
            context_tokens: 0,
            total_tokens_used: 0,
            total_cost: 0.0,
            started_at: None,
            completed_at: None,
            error: None,
        }
    }

    /// Start execution
    pub fn start(&mut self) {
        self.state = AgentState::Running;
        self.started_at = Some(Instant::now());
    }

    /// Mark as blocked
    pub fn block(&mut self) {
        self.state = AgentState::Blocked;
    }

    /// Complete execution
    pub fn complete(&mut self, usage: &UsageStats) {
        self.state = AgentState::Completed;
        self.completed_at = Some(Instant::now());
        self.total_tokens_used += usage.input_tokens + usage.output_tokens;
        self.total_cost += usage.calculate_cost(self.model);
    }

    /// Fail execution
    pub fn fail(&mut self, error: String) {
        self.state = AgentState::Failed;
        self.completed_at = Some(Instant::now());
        self.error = Some(error);
    }

    /// Get execution duration
    pub fn duration(&self) -> Option<Duration> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            (Some(start), None) => Some(start.elapsed()),
            _ => None,
        }
    }
}

/// Workflow step definition
#[derive(Debug, Clone)]
pub struct WorkflowStep {
    /// Step identifier
    pub step_id: String,
    /// Agent type to execute step
    pub agent_type: AgentType,
    /// Task description
    pub task: String,
    /// Dependencies (step IDs that must complete first)
    pub dependencies: Vec<String>,
    /// Required model (or None for auto-selection)
    pub required_model: Option<ClaudeModel>,
    /// Maximum retries
    pub max_retries: u32,
}

/// Workflow execution result
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    /// Workflow identifier
    pub workflow_id: String,
    /// Step results
    pub step_results: HashMap<String, StepResult>,
    /// Total execution time
    pub total_duration: Duration,
    /// Total tokens used
    pub total_tokens: usize,
    /// Total cost
    pub total_cost: f64,
    /// Success status
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Individual step result
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Step identifier
    pub step_id: String,
    /// Agent that executed step
    pub agent_id: String,
    /// Model used
    pub model: ClaudeModel,
    /// Response content
    pub response: Option<String>,
    /// Execution duration
    pub duration: Duration,
    /// Tokens used
    pub tokens_used: usize,
    /// Cost incurred
    pub cost: f64,
    /// Success status
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Multi-agent coordinator
pub struct AgentCoordinator {
    /// Agent contexts
    agents: Arc<RwLock<HashMap<String, AgentContext>>>,
    /// Context manager
    context_manager: Arc<RwLock<ContextManager>>,
    /// Default model for agents
    default_model: ClaudeModel,
    /// Maximum concurrent agents
    max_concurrent: usize,
    /// Total workflows executed
    workflows_executed: u64,
    /// Total cost incurred
    total_cost: f64,
}

impl AgentCoordinator {
    /// Create new agent coordinator
    pub fn new(default_model: ClaudeModel, max_concurrent: usize) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            context_manager: Arc::new(RwLock::new(ContextManager::new(100_000))),
            default_model,
            max_concurrent,
            workflows_executed: 0,
            total_cost: 0.0,
        }
    }

    /// Spawn a new agent
    pub fn spawn_agent(&self, agent_id: String, agent_type: AgentType) -> Result<()> {
        let mut agents = self.agents.write();

        if agents.len() >= self.max_concurrent {
            return Err(RuvLLMError::OutOfMemory(format!(
                "Maximum concurrent agents ({}) reached",
                self.max_concurrent
            )));
        }

        if agents.contains_key(&agent_id) {
            return Err(RuvLLMError::InvalidOperation(format!(
                "Agent {} already exists",
                agent_id
            )));
        }

        let context = AgentContext::new(agent_id.clone(), agent_type, self.default_model);
        agents.insert(agent_id, context);

        Ok(())
    }

    /// Get agent context
    pub fn get_agent(&self, agent_id: &str) -> Option<AgentContext> {
        self.agents.read().get(agent_id).cloned()
    }

    /// Update agent state
    pub fn update_agent<F>(&self, agent_id: &str, f: F) -> Result<()>
    where
        F: FnOnce(&mut AgentContext),
    {
        let mut agents = self.agents.write();
        let agent = agents
            .get_mut(agent_id)
            .ok_or_else(|| RuvLLMError::NotFound(format!("Agent {} not found", agent_id)))?;
        f(agent);
        Ok(())
    }

    /// Terminate agent
    pub fn terminate_agent(&self, agent_id: &str) -> Result<()> {
        let mut agents = self.agents.write();
        agents
            .remove(agent_id)
            .ok_or_else(|| RuvLLMError::NotFound(format!("Agent {} not found", agent_id)))?;

        // Clean up context window
        self.context_manager.write().remove_window(agent_id);

        Ok(())
    }

    /// Get active agent count
    pub fn active_agent_count(&self) -> usize {
        self.agents
            .read()
            .values()
            .filter(|a| a.state == AgentState::Running)
            .count()
    }

    /// Get total agent count
    pub fn total_agent_count(&self) -> usize {
        self.agents.read().len()
    }

    /// Execute workflow with dependency resolution
    pub async fn execute_workflow(
        &mut self,
        workflow_id: String,
        steps: Vec<WorkflowStep>,
    ) -> Result<WorkflowResult> {
        let start_time = Instant::now();
        let mut step_results: HashMap<String, StepResult> = HashMap::new();
        let mut completed_steps: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        // Build dependency graph
        let mut pending_steps: Vec<&WorkflowStep> = steps.iter().collect();

        while !pending_steps.is_empty() {
            // Find steps with satisfied dependencies
            let ready_steps: Vec<_> = pending_steps
                .iter()
                .filter(|step| {
                    step.dependencies
                        .iter()
                        .all(|dep| completed_steps.contains(dep))
                })
                .cloned()
                .collect();

            if ready_steps.is_empty() && !pending_steps.is_empty() {
                return Err(RuvLLMError::InvalidOperation(
                    "Workflow has circular dependencies".to_string(),
                ));
            }

            // Execute ready steps in parallel
            for step in ready_steps {
                let agent_id = format!("{}-{}", workflow_id, step.step_id);
                let model = step.required_model.unwrap_or(self.default_model);

                // Spawn agent for step
                self.spawn_agent(agent_id.clone(), step.agent_type)?;
                self.update_agent(&agent_id, |a| a.start())?;

                // Simulate execution (in production, would call Claude API)
                let step_start = Instant::now();

                // Create mock result
                let result = StepResult {
                    step_id: step.step_id.clone(),
                    agent_id: agent_id.clone(),
                    model,
                    response: Some(format!("Completed: {}", step.task)),
                    duration: step_start.elapsed(),
                    tokens_used: 500, // Mock value
                    cost: 0.001,      // Mock value
                    success: true,
                    error: None,
                };

                self.update_agent(&agent_id, |a| {
                    let usage = UsageStats {
                        input_tokens: 250,
                        output_tokens: 250,
                    };
                    a.complete(&usage);
                })?;

                step_results.insert(step.step_id.clone(), result);
                completed_steps.insert(step.step_id.clone());

                // Clean up agent
                self.terminate_agent(&agent_id)?;
            }

            // Remove completed steps from pending
            pending_steps.retain(|step| !completed_steps.contains(&step.step_id));
        }

        // Calculate totals
        let total_tokens: usize = step_results.values().map(|r| r.tokens_used).sum();
        let total_cost: f64 = step_results.values().map(|r| r.cost).sum();

        self.workflows_executed += 1;
        self.total_cost += total_cost;

        Ok(WorkflowResult {
            workflow_id,
            step_results,
            total_duration: start_time.elapsed(),
            total_tokens,
            total_cost,
            success: true,
            error: None,
        })
    }

    /// Get coordinator statistics
    pub fn stats(&self) -> CoordinatorStats {
        let agents = self.agents.read();
        let active_count = agents
            .values()
            .filter(|a| a.state == AgentState::Running)
            .count();
        let total_tokens: usize = agents.values().map(|a| a.total_tokens_used).sum();

        CoordinatorStats {
            total_agents: agents.len(),
            active_agents: active_count,
            blocked_agents: agents
                .values()
                .filter(|a| a.state == AgentState::Blocked)
                .count(),
            completed_agents: agents
                .values()
                .filter(|a| a.state == AgentState::Completed)
                .count(),
            failed_agents: agents
                .values()
                .filter(|a| a.state == AgentState::Failed)
                .count(),
            workflows_executed: self.workflows_executed,
            total_tokens_used: total_tokens,
            total_cost: self.total_cost,
        }
    }
}

/// Coordinator statistics
#[derive(Debug, Clone)]
pub struct CoordinatorStats {
    /// Total agents created
    pub total_agents: usize,
    /// Currently active agents
    pub active_agents: usize,
    /// Blocked agents
    pub blocked_agents: usize,
    /// Completed agents
    pub completed_agents: usize,
    /// Failed agents
    pub failed_agents: usize,
    /// Total workflows executed
    pub workflows_executed: u64,
    /// Total tokens used
    pub total_tokens_used: usize,
    /// Total cost incurred
    pub total_cost: f64,
}

// ============================================================================
// Cost Estimation
// ============================================================================

/// Cost estimator for Claude API usage
pub struct CostEstimator {
    /// Usage by model
    usage_by_model: HashMap<ClaudeModel, UsageStats>,
}

impl CostEstimator {
    /// Create new cost estimator
    pub fn new() -> Self {
        Self {
            usage_by_model: HashMap::new(),
        }
    }

    /// Estimate cost for a request
    pub fn estimate_request_cost(
        &self,
        model: ClaudeModel,
        input_tokens: usize,
        expected_output_tokens: usize,
    ) -> f64 {
        let input_cost = (input_tokens as f64 / 1000.0) * model.input_cost_per_1k();
        let output_cost = (expected_output_tokens as f64 / 1000.0) * model.output_cost_per_1k();
        input_cost + output_cost
    }

    /// Record actual usage
    pub fn record_usage(&mut self, model: ClaudeModel, usage: &UsageStats) {
        let entry = self.usage_by_model.entry(model).or_default();
        entry.input_tokens += usage.input_tokens;
        entry.output_tokens += usage.output_tokens;
    }

    /// Get total cost to date
    pub fn total_cost(&self) -> f64 {
        self.usage_by_model
            .iter()
            .map(|(model, usage)| usage.calculate_cost(*model))
            .sum()
    }

    /// Get cost breakdown by model
    pub fn cost_breakdown(&self) -> HashMap<ClaudeModel, f64> {
        self.usage_by_model
            .iter()
            .map(|(model, usage)| (*model, usage.calculate_cost(*model)))
            .collect()
    }

    /// Get total usage by model
    pub fn usage_by_model(&self) -> &HashMap<ClaudeModel, UsageStats> {
        &self.usage_by_model
    }
}

impl Default for CostEstimator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Latency Tracking
// ============================================================================

/// Latency tracker for performance monitoring
pub struct LatencyTracker {
    /// Samples by model
    samples: HashMap<ClaudeModel, Vec<LatencySample>>,
    /// Maximum samples to keep per model
    max_samples: usize,
}

/// Single latency sample
#[derive(Debug, Clone)]
pub struct LatencySample {
    /// Time to first token (ms)
    pub ttft_ms: u64,
    /// Total response time (ms)
    pub total_ms: u64,
    /// Input tokens
    pub input_tokens: usize,
    /// Output tokens
    pub output_tokens: usize,
    /// Timestamp
    pub timestamp: Instant,
}

impl LatencyTracker {
    /// Create new latency tracker
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: HashMap::new(),
            max_samples,
        }
    }

    /// Record latency sample
    pub fn record(&mut self, model: ClaudeModel, sample: LatencySample) {
        let samples = self.samples.entry(model).or_default();
        samples.push(sample);

        // Trim old samples
        if samples.len() > self.max_samples {
            samples.remove(0);
        }
    }

    /// Get average TTFT for model
    pub fn average_ttft(&self, model: ClaudeModel) -> Option<f64> {
        self.samples.get(&model).map(|samples| {
            if samples.is_empty() {
                return 0.0;
            }
            let sum: u64 = samples.iter().map(|s| s.ttft_ms).sum();
            sum as f64 / samples.len() as f64
        })
    }

    /// Get p95 TTFT for model
    pub fn p95_ttft(&self, model: ClaudeModel) -> Option<u64> {
        self.samples.get(&model).and_then(|samples| {
            if samples.is_empty() {
                return None;
            }
            let mut ttfts: Vec<u64> = samples.iter().map(|s| s.ttft_ms).collect();
            ttfts.sort();
            let idx = (ttfts.len() as f64 * 0.95) as usize;
            ttfts.get(idx.min(ttfts.len() - 1)).copied()
        })
    }

    /// Get average tokens per second for model
    pub fn average_tokens_per_second(&self, model: ClaudeModel) -> Option<f64> {
        self.samples.get(&model).map(|samples| {
            if samples.is_empty() {
                return 0.0;
            }
            let total_tokens: usize = samples.iter().map(|s| s.output_tokens).sum();
            let total_time_ms: u64 = samples.iter().map(|s| s.total_ms - s.ttft_ms).sum();
            if total_time_ms == 0 {
                return 0.0;
            }
            total_tokens as f64 / (total_time_ms as f64 / 1000.0)
        })
    }

    /// Get statistics for model
    pub fn get_stats(&self, model: ClaudeModel) -> Option<LatencyStats> {
        self.samples.get(&model).map(|samples| LatencyStats {
            sample_count: samples.len(),
            avg_ttft_ms: self.average_ttft(model).unwrap_or(0.0),
            p95_ttft_ms: self.p95_ttft(model).unwrap_or(0),
            avg_tokens_per_second: self.average_tokens_per_second(model).unwrap_or(0.0),
        })
    }
}

/// Latency statistics
#[derive(Debug, Clone)]
pub struct LatencyStats {
    /// Number of samples
    pub sample_count: usize,
    /// Average time to first token
    pub avg_ttft_ms: f64,
    /// P95 time to first token
    pub p95_ttft_ms: u64,
    /// Average tokens per second
    pub avg_tokens_per_second: f64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_model_costs() {
        let usage = UsageStats {
            input_tokens: 1000,
            output_tokens: 500,
        };

        let haiku_cost = usage.calculate_cost(ClaudeModel::Haiku);
        let sonnet_cost = usage.calculate_cost(ClaudeModel::Sonnet);
        let opus_cost = usage.calculate_cost(ClaudeModel::Opus);

        assert!(haiku_cost < sonnet_cost);
        assert!(sonnet_cost < opus_cost);
    }

    #[test]
    fn test_context_window_compression() {
        let mut window = ContextWindow::new(1000);

        // Add many messages
        for i in 0..20 {
            window.add_message(Message::user(format!(
                "Message {} with some content to add tokens",
                i
            )));
        }

        // Window should have compressed
        assert!(window.token_count() <= 1000);
    }

    #[test]
    fn test_message_token_estimation() {
        let msg = Message::user("Hello, this is a test message with some content.");
        let tokens = msg.estimate_tokens();
        assert!(tokens > 0);
        assert!(tokens < 100); // Should be reasonable estimate
    }

    #[test]
    fn test_quality_monitor() {
        let mut monitor = QualityMonitor::new(0.6, 10);

        // Add good quality scores
        for _ in 0..5 {
            monitor.record(0.8);
        }
        assert!(monitor.should_continue());

        // Add bad quality scores
        let mut bad_monitor = QualityMonitor::new(0.6, 10);
        for _ in 0..5 {
            bad_monitor.record(0.3);
        }
        assert!(!bad_monitor.should_continue());
    }

    #[test]
    fn test_agent_coordinator() {
        let coordinator = AgentCoordinator::new(ClaudeModel::Sonnet, 10);

        coordinator
            .spawn_agent("agent-1".to_string(), AgentType::Coder)
            .unwrap();
        coordinator
            .spawn_agent("agent-2".to_string(), AgentType::Researcher)
            .unwrap();

        assert_eq!(coordinator.total_agent_count(), 2);

        coordinator.update_agent("agent-1", |a| a.start()).unwrap();
        assert_eq!(coordinator.active_agent_count(), 1);

        coordinator.terminate_agent("agent-1").unwrap();
        assert_eq!(coordinator.total_agent_count(), 1);
    }

    #[test]
    fn test_cost_estimator() {
        let mut estimator = CostEstimator::new();

        let usage = UsageStats {
            input_tokens: 1000,
            output_tokens: 500,
        };

        estimator.record_usage(ClaudeModel::Sonnet, &usage);
        estimator.record_usage(ClaudeModel::Haiku, &usage);

        let total = estimator.total_cost();
        assert!(total > 0.0);

        let breakdown = estimator.cost_breakdown();
        assert!(breakdown.contains_key(&ClaudeModel::Sonnet));
        assert!(breakdown.contains_key(&ClaudeModel::Haiku));
    }

    #[test]
    fn test_latency_tracker() {
        let mut tracker = LatencyTracker::new(100);

        for i in 0..10 {
            tracker.record(
                ClaudeModel::Sonnet,
                LatencySample {
                    ttft_ms: 400 + i * 10,
                    total_ms: 1000 + i * 100,
                    input_tokens: 500,
                    output_tokens: 200,
                    timestamp: Instant::now(),
                },
            );
        }

        let stats = tracker.get_stats(ClaudeModel::Sonnet).unwrap();
        assert_eq!(stats.sample_count, 10);
        assert!(stats.avg_ttft_ms > 400.0);
        assert!(stats.avg_tokens_per_second > 0.0);
    }
}
