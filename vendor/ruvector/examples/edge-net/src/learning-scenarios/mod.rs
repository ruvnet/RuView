//! Learning Scenarios Module
//!
//! This module provides patterns and scenarios for training the
//! RuVector self-learning hooks system, with full Claude Agent SDK
//! and MCP integration support.
//!
//! ## Four Attention Mechanisms
//!
//! | Attention Type | Question Answered | Application |
//! |---------------|-------------------|-------------|
//! | **Neural** | What words matter? | Token/semantic relevance |
//! | **DAG** | What steps matter? | Execution order, dependencies |
//! | **Graph** | What relationships matter? | Code structure, call graphs |
//! | **State Space** | What history still matters? | Context persistence |

pub mod error_recovery;
pub mod file_sequences;
pub mod sdk_integration;
pub mod mcp_tools;
pub mod attention_patterns;

pub use error_recovery::error_patterns::{ErrorLearningTracker, ErrorPattern, RecoveryStrategy};
pub use file_sequences::sequence_tracker::{EditSequence, FileEdit, SequencePattern, SequenceTracker};
pub use sdk_integration::{
    AgentDefinition, HookEventType, HookMatcher, McpServerConfig,
    PermissionMode, QueryOptions, TelemetryConfig, generate_settings_json,
};
pub use mcp_tools::{
    McpToolDef, PropertyDef, ToolCategory, ToolInputSchema,
    get_ruvector_tools, generate_tools_list_json,
};
pub use attention_patterns::{
    NeuralAttention, DagAttention, GraphAttention, StateSpaceAttention,
    AttentionOrchestrator, AttentionAnalysis,
    DagNode, StepType, GraphNode, GraphEdge, NodeType, EdgeType, HistoryEntry,
};

/// Initialize the learning scenarios system
pub fn init() {
    log::info!("🧠 Learning scenarios initialized");
}

/// Learning statistics
#[derive(Debug, Default)]
pub struct LearningStats {
    pub patterns_learned: u32,
    pub errors_recovered: u32,
    pub sequences_detected: u32,
    pub agent_routings: u32,
}

impl LearningStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_pattern(&mut self) {
        self.patterns_learned += 1;
    }

    pub fn record_recovery(&mut self) {
        self.errors_recovered += 1;
    }

    pub fn record_sequence(&mut self) {
        self.sequences_detected += 1;
    }

    pub fn record_routing(&mut self) {
        self.agent_routings += 1;
    }
}
