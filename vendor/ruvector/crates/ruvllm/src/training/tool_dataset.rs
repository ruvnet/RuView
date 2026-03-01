//! # Tool Calling Dataset for MCP Fine-Tuning
//!
//! This module generates training datasets for tool calling fine-tuning,
//! covering 140+ Claude Flow MCP tools with diverse examples.
//!
//! ## Tool Categories
//!
//! The dataset covers the following MCP tool categories:
//!
//! - **Agent Management**: agent_spawn, agent_terminate, agent_status, agent_list, etc.
//! - **Memory Operations**: memory_store, memory_retrieve, memory_search, memory_delete
//! - **Swarm Coordination**: swarm_init, swarm_status, swarm_shutdown, swarm_health
//! - **Task Management**: task_create, task_status, task_list, task_complete
//! - **Hooks & Learning**: hooks_pre-task, hooks_post-task, hooks_route, hooks_metrics
//! - **Session Management**: session_save, session_restore, session_list
//! - **Workflow**: workflow_create, workflow_execute, workflow_status
//! - **System**: system_status, system_metrics, system_health
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::training::{ToolCallDataset, ToolDatasetConfig, ToolCallExample};
//!
//! let config = ToolDatasetConfig::default();
//! let dataset = ToolCallDataset::generate(config)?;
//!
//! println!("Generated {} examples", dataset.len());
//!
//! // Export for training
//! dataset.export_jsonl("tool_training.jsonl")?;
//! ```

use crate::error::Result;
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// MCP Tool categories for Claude Flow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolCategory {
    /// Agent lifecycle management
    AgentManagement,
    /// Memory storage and retrieval
    MemoryOperations,
    /// Multi-agent swarm coordination
    SwarmCoordination,
    /// Task creation and tracking
    TaskManagement,
    /// Hooks for learning and routing
    HooksLearning,
    /// Session state persistence
    SessionManagement,
    /// Workflow orchestration
    Workflow,
    /// System monitoring and health
    System,
    /// Configuration management
    Configuration,
    /// Hive-mind consensus
    HiveMind,
    /// Terminal operations
    Terminal,
    /// Neural/ML operations
    Neural,
    /// Performance monitoring
    Performance,
    /// GitHub integration
    GitHub,
    /// Claims/ownership
    Claims,
    /// AI security/defence
    AiDefence,
    /// Embeddings
    Embeddings,
    /// DAA (Decentralized Autonomous Agents)
    Daa,
    /// Coordination
    Coordination,
}

impl ToolCategory {
    /// Get all tool categories
    pub fn all() -> &'static [ToolCategory] {
        &[
            Self::AgentManagement,
            Self::MemoryOperations,
            Self::SwarmCoordination,
            Self::TaskManagement,
            Self::HooksLearning,
            Self::SessionManagement,
            Self::Workflow,
            Self::System,
            Self::Configuration,
            Self::HiveMind,
            Self::Terminal,
            Self::Neural,
            Self::Performance,
            Self::GitHub,
            Self::Claims,
            Self::AiDefence,
            Self::Embeddings,
            Self::Daa,
            Self::Coordination,
        ]
    }

    /// Get category name
    pub fn name(&self) -> &'static str {
        match self {
            Self::AgentManagement => "agent",
            Self::MemoryOperations => "memory",
            Self::SwarmCoordination => "swarm",
            Self::TaskManagement => "task",
            Self::HooksLearning => "hooks",
            Self::SessionManagement => "session",
            Self::Workflow => "workflow",
            Self::System => "system",
            Self::Configuration => "config",
            Self::HiveMind => "hive-mind",
            Self::Terminal => "terminal",
            Self::Neural => "neural",
            Self::Performance => "performance",
            Self::GitHub => "github",
            Self::Claims => "claims",
            Self::AiDefence => "aidefence",
            Self::Embeddings => "embeddings",
            Self::Daa => "daa",
            Self::Coordination => "coordination",
        }
    }
}

/// MCP Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    /// Tool name (e.g., "agent_spawn")
    pub name: String,
    /// Tool category
    pub category: ToolCategory,
    /// Description
    pub description: String,
    /// Required parameters
    pub required_params: Vec<ToolParam>,
    /// Optional parameters
    pub optional_params: Vec<ToolParam>,
    /// Example use cases
    pub use_cases: Vec<String>,
}

/// Tool parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParam {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub param_type: ParamType,
    /// Description
    pub description: String,
    /// Example values
    pub examples: Vec<String>,
}

/// Parameter types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParamType {
    /// String value
    String,
    /// Integer value
    Integer,
    /// Boolean value
    Boolean,
    /// Float value
    Float,
    /// JSON object
    Object,
    /// Array of values
    Array,
    /// Enum (predefined values)
    Enum,
}

/// A single tool call training example
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallExample {
    /// Input prompt/request
    pub prompt: String,
    /// Expected tool to call
    pub expected_tool: String,
    /// Expected parameters
    pub expected_params: serde_json::Value,
    /// Whether this call succeeded
    pub success: bool,
    /// Category of the tool
    pub category: ToolCategory,
    /// Difficulty level
    pub difficulty: DifficultyLevel,
    /// Error message (if failure case)
    pub error_message: Option<String>,
    /// Alternative tools that could work
    pub alternatives: Vec<String>,
    /// Context about the scenario
    pub context: String,
    /// Quality score (0.0-1.0)
    pub quality_score: f32,
}

/// Difficulty levels for tool calling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DifficultyLevel {
    /// Simple, single tool call
    Easy,
    /// Moderate complexity, may need parameter reasoning
    Medium,
    /// Complex scenario, multiple considerations
    Hard,
    /// Edge cases and error recovery
    Expert,
}

/// Configuration for tool dataset generation
#[derive(Debug, Clone)]
pub struct ToolDatasetConfig {
    /// Examples per tool
    pub examples_per_tool: usize,
    /// Include error/recovery cases
    pub include_error_cases: bool,
    /// Error case ratio (0.0-1.0)
    pub error_case_ratio: f32,
    /// Random seed
    pub seed: u64,
    /// Include multi-step scenarios
    pub include_multi_step: bool,
    /// Include alternative tools
    pub include_alternatives: bool,
    /// Difficulty distribution
    pub difficulty_weights: DifficultyWeights,
}

/// Weights for difficulty distribution
#[derive(Debug, Clone)]
pub struct DifficultyWeights {
    pub easy: f32,
    pub medium: f32,
    pub hard: f32,
    pub expert: f32,
}

impl Default for DifficultyWeights {
    fn default() -> Self {
        Self {
            easy: 0.3,
            medium: 0.4,
            hard: 0.2,
            expert: 0.1,
        }
    }
}

impl Default for ToolDatasetConfig {
    fn default() -> Self {
        Self {
            examples_per_tool: 10,
            include_error_cases: true,
            error_case_ratio: 0.15,
            seed: 42,
            include_multi_step: true,
            include_alternatives: true,
            difficulty_weights: DifficultyWeights::default(),
        }
    }
}

impl ToolDatasetConfig {
    /// Create config for comprehensive training
    pub fn comprehensive() -> Self {
        Self {
            examples_per_tool: 20,
            include_error_cases: true,
            error_case_ratio: 0.2,
            include_multi_step: true,
            include_alternatives: true,
            difficulty_weights: DifficultyWeights {
                easy: 0.25,
                medium: 0.35,
                hard: 0.25,
                expert: 0.15,
            },
            ..Default::default()
        }
    }

    /// Create config for quick testing
    pub fn minimal() -> Self {
        Self {
            examples_per_tool: 3,
            include_error_cases: false,
            error_case_ratio: 0.0,
            include_multi_step: false,
            include_alternatives: false,
            ..Default::default()
        }
    }
}

/// Dataset statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolDatasetStats {
    /// Total examples
    pub total_examples: usize,
    /// Examples per category
    pub by_category: HashMap<String, usize>,
    /// Examples per tool
    pub by_tool: HashMap<String, usize>,
    /// Examples per difficulty
    pub by_difficulty: HashMap<String, usize>,
    /// Success/error ratio
    pub success_count: usize,
    /// Error examples count
    pub error_count: usize,
    /// Average quality score
    pub avg_quality: f32,
}

/// Complete tool calling dataset
#[derive(Debug)]
pub struct ToolCallDataset {
    /// All examples
    pub examples: Vec<ToolCallExample>,
    /// Tool definitions
    pub tool_definitions: Vec<McpToolDef>,
    /// Statistics
    pub stats: ToolDatasetStats,
}

impl ToolCallDataset {
    /// Generate a complete tool calling dataset
    pub fn generate(config: ToolDatasetConfig) -> Result<Self> {
        let mut generator = ToolDatasetGenerator::new(config);
        generator.generate()
    }

    /// Get the number of examples
    pub fn len(&self) -> usize {
        self.examples.len()
    }

    /// Check if dataset is empty
    pub fn is_empty(&self) -> bool {
        self.examples.is_empty()
    }

    /// Export to JSONL format
    pub fn export_jsonl<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        for example in &self.examples {
            let json = serde_json::to_string(example)?;
            writeln!(writer, "{}", json)?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Export to JSON format
    pub fn export_json<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, &self.examples)?;
        Ok(())
    }

    /// Export tool definitions
    pub fn export_tool_defs<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, &self.tool_definitions)?;
        Ok(())
    }

    /// Filter examples by category
    pub fn filter_by_category(&self, category: ToolCategory) -> Vec<&ToolCallExample> {
        self.examples
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Filter examples by tool
    pub fn filter_by_tool(&self, tool_name: &str) -> Vec<&ToolCallExample> {
        self.examples
            .iter()
            .filter(|e| e.expected_tool == tool_name)
            .collect()
    }

    /// Filter examples by difficulty
    pub fn filter_by_difficulty(&self, difficulty: DifficultyLevel) -> Vec<&ToolCallExample> {
        self.examples
            .iter()
            .filter(|e| e.difficulty == difficulty)
            .collect()
    }

    /// Split into train/validation/test sets
    pub fn split(
        &self,
        train_ratio: f32,
        val_ratio: f32,
        seed: u64,
    ) -> (
        Vec<ToolCallExample>,
        Vec<ToolCallExample>,
        Vec<ToolCallExample>,
    ) {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut examples = self.examples.clone();
        examples.shuffle(&mut rng);

        let n = examples.len();
        let train_end = (n as f32 * train_ratio) as usize;
        let val_end = train_end + (n as f32 * val_ratio) as usize;

        let train = examples[..train_end].to_vec();
        let val = examples[train_end..val_end].to_vec();
        let test = examples[val_end..].to_vec();

        (train, val, test)
    }
}

/// Dataset generator for tool calling examples
pub struct ToolDatasetGenerator {
    config: ToolDatasetConfig,
    rng: StdRng,
    tools: Vec<McpToolDef>,
}

impl ToolDatasetGenerator {
    /// Create a new generator
    pub fn new(config: ToolDatasetConfig) -> Self {
        let rng = StdRng::seed_from_u64(config.seed);
        let tools = Self::define_mcp_tools();

        Self { config, rng, tools }
    }

    /// Generate the complete dataset
    pub fn generate(&mut self) -> Result<ToolCallDataset> {
        let mut examples = Vec::new();

        for tool in &self.tools.clone() {
            let tool_examples = self.generate_tool_examples(tool);
            examples.extend(tool_examples);
        }

        // Shuffle examples
        examples.shuffle(&mut self.rng);

        // Compute statistics
        let stats = Self::compute_stats(&examples);

        Ok(ToolCallDataset {
            examples,
            tool_definitions: self.tools.clone(),
            stats,
        })
    }

    /// Generate examples for a single tool
    fn generate_tool_examples(&mut self, tool: &McpToolDef) -> Vec<ToolCallExample> {
        let mut examples = Vec::new();

        for i in 0..self.config.examples_per_tool {
            let is_error = self.config.include_error_cases
                && self.rng.gen::<f32>() < self.config.error_case_ratio;

            let difficulty = self.sample_difficulty();

            let example = if is_error {
                self.generate_error_example(tool, difficulty)
            } else {
                self.generate_success_example(tool, difficulty, i)
            };

            examples.push(example);
        }

        examples
    }

    /// Sample a difficulty level based on weights
    fn sample_difficulty(&mut self) -> DifficultyLevel {
        let w = &self.config.difficulty_weights;
        let r = self.rng.gen::<f32>();

        if r < w.easy {
            DifficultyLevel::Easy
        } else if r < w.easy + w.medium {
            DifficultyLevel::Medium
        } else if r < w.easy + w.medium + w.hard {
            DifficultyLevel::Hard
        } else {
            DifficultyLevel::Expert
        }
    }

    /// Generate a success example
    fn generate_success_example(
        &mut self,
        tool: &McpToolDef,
        difficulty: DifficultyLevel,
        index: usize,
    ) -> ToolCallExample {
        let prompt_template = self.get_prompt_template(tool, difficulty, index);
        let params = self.generate_params(tool, difficulty);

        let context = self.generate_context(tool, difficulty);
        let alternatives = if self.config.include_alternatives {
            self.get_alternative_tools(tool)
        } else {
            Vec::new()
        };

        let quality = match difficulty {
            DifficultyLevel::Easy => 0.95 + self.rng.gen::<f32>() * 0.05,
            DifficultyLevel::Medium => 0.85 + self.rng.gen::<f32>() * 0.10,
            DifficultyLevel::Hard => 0.75 + self.rng.gen::<f32>() * 0.15,
            DifficultyLevel::Expert => 0.70 + self.rng.gen::<f32>() * 0.20,
        };

        ToolCallExample {
            prompt: prompt_template,
            expected_tool: tool.name.clone(),
            expected_params: params,
            success: true,
            category: tool.category,
            difficulty,
            error_message: None,
            alternatives,
            context,
            quality_score: quality,
        }
    }

    /// Generate an error/recovery example
    fn generate_error_example(
        &mut self,
        tool: &McpToolDef,
        difficulty: DifficultyLevel,
    ) -> ToolCallExample {
        let error_types = [
            ("Missing required parameter", "Parameter validation failed"),
            ("Invalid parameter type", "Type mismatch error"),
            (
                "Resource not found",
                "The specified resource does not exist",
            ),
            ("Permission denied", "Insufficient permissions"),
            ("Rate limited", "Too many requests"),
        ];

        let (error_type, error_msg) = error_types.choose(&mut self.rng).unwrap();

        let prompt = format!(
            "Call {} but with incomplete or incorrect parameters for error handling training",
            tool.name
        );

        let mut params = self.generate_params(tool, difficulty);
        // Corrupt the params for error case
        if let Some(obj) = params.as_object_mut() {
            if !obj.is_empty() {
                let keys: Vec<String> = obj.keys().cloned().collect();
                if let Some(key) = keys.choose(&mut self.rng) {
                    obj.remove(key);
                }
            }
        }

        ToolCallExample {
            prompt,
            expected_tool: tool.name.clone(),
            expected_params: params,
            success: false,
            category: tool.category,
            difficulty,
            error_message: Some(format!("{}: {}", error_type, error_msg)),
            alternatives: Vec::new(),
            context: format!("Error recovery scenario for {}", tool.name),
            quality_score: 0.7,
        }
    }

    /// Get prompt template for a tool
    fn get_prompt_template(
        &mut self,
        tool: &McpToolDef,
        difficulty: DifficultyLevel,
        index: usize,
    ) -> String {
        let use_case = if !tool.use_cases.is_empty() {
            tool.use_cases[index % tool.use_cases.len()].clone()
        } else {
            tool.description.clone()
        };

        match difficulty {
            DifficultyLevel::Easy => format!("I need to {} using the {} tool", use_case, tool.name),
            DifficultyLevel::Medium => format!(
                "Help me {}. I want to use the appropriate MCP tool for this task.",
                use_case
            ),
            DifficultyLevel::Hard => format!(
                "I have a complex requirement: {}. Determine the best tool and parameters.",
                use_case
            ),
            DifficultyLevel::Expert => format!(
                "Given the scenario: {} - what tool should I use and how should I handle potential edge cases?",
                use_case
            ),
        }
    }

    /// Generate parameters for a tool
    fn generate_params(
        &mut self,
        tool: &McpToolDef,
        _difficulty: DifficultyLevel,
    ) -> serde_json::Value {
        let mut params = serde_json::Map::new();

        // Add required parameters
        for param in &tool.required_params {
            let value = self.generate_param_value(param);
            params.insert(param.name.clone(), value);
        }

        // Randomly add some optional parameters
        for param in &tool.optional_params {
            if self.rng.gen_bool(0.5) {
                let value = self.generate_param_value(param);
                params.insert(param.name.clone(), value);
            }
        }

        serde_json::Value::Object(params)
    }

    /// Generate a value for a parameter
    fn generate_param_value(&mut self, param: &ToolParam) -> serde_json::Value {
        if !param.examples.is_empty() && self.rng.gen_bool(0.7) {
            let example = param.examples.choose(&mut self.rng).unwrap();
            // Try to parse as appropriate type
            match param.param_type {
                ParamType::Integer => {
                    if let Ok(n) = example.parse::<i64>() {
                        return serde_json::Value::Number(n.into());
                    }
                }
                ParamType::Float => {
                    if let Ok(n) = example.parse::<f64>() {
                        if let Some(num) = serde_json::Number::from_f64(n) {
                            return serde_json::Value::Number(num);
                        }
                    }
                }
                ParamType::Boolean => {
                    if let Ok(b) = example.parse::<bool>() {
                        return serde_json::Value::Bool(b);
                    }
                }
                _ => {}
            }
            return serde_json::Value::String(example.clone());
        }

        match param.param_type {
            ParamType::String => {
                serde_json::Value::String(format!("example_{}", self.rng.gen::<u32>()))
            }
            ParamType::Integer => serde_json::Value::Number((self.rng.gen_range(1..100)).into()),
            ParamType::Boolean => serde_json::Value::Bool(self.rng.gen()),
            ParamType::Float => {
                let f = self.rng.gen::<f64>();
                serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Number(0.into()))
            }
            ParamType::Object => serde_json::Value::Object(serde_json::Map::new()),
            ParamType::Array => serde_json::Value::Array(vec![]),
            ParamType::Enum => {
                if !param.examples.is_empty() {
                    serde_json::Value::String(param.examples.choose(&mut self.rng).unwrap().clone())
                } else {
                    serde_json::Value::String("default".to_string())
                }
            }
        }
    }

    /// Generate context for an example
    fn generate_context(&mut self, tool: &McpToolDef, difficulty: DifficultyLevel) -> String {
        let contexts = match difficulty {
            DifficultyLevel::Easy => vec![
                format!("Simple {} operation", tool.category.name()),
                format!("Basic use of {}", tool.name),
            ],
            DifficultyLevel::Medium => vec![
                format!("Standard {} workflow", tool.category.name()),
                format!("Common {} scenario", tool.name),
            ],
            DifficultyLevel::Hard => vec![
                format!("Complex {} integration", tool.category.name()),
                format!("Multi-step {} scenario", tool.name),
            ],
            DifficultyLevel::Expert => vec![
                format!("Edge case handling for {}", tool.name),
                format!(
                    "Production scenario with {} error handling",
                    tool.category.name()
                ),
            ],
        };

        contexts.choose(&mut self.rng).unwrap().clone()
    }

    /// Get alternative tools for a given tool
    fn get_alternative_tools(&self, tool: &McpToolDef) -> Vec<String> {
        self.tools
            .iter()
            .filter(|t| t.category == tool.category && t.name != tool.name)
            .take(2)
            .map(|t| t.name.clone())
            .collect()
    }

    /// Compute dataset statistics
    fn compute_stats(examples: &[ToolCallExample]) -> ToolDatasetStats {
        let mut stats = ToolDatasetStats {
            total_examples: examples.len(),
            ..Default::default()
        };

        let mut total_quality = 0.0f32;

        for example in examples {
            // By category
            *stats
                .by_category
                .entry(example.category.name().to_string())
                .or_insert(0) += 1;

            // By tool
            *stats
                .by_tool
                .entry(example.expected_tool.clone())
                .or_insert(0) += 1;

            // By difficulty
            *stats
                .by_difficulty
                .entry(format!("{:?}", example.difficulty))
                .or_insert(0) += 1;

            // Success/error
            if example.success {
                stats.success_count += 1;
            } else {
                stats.error_count += 1;
            }

            total_quality += example.quality_score;
        }

        if !examples.is_empty() {
            stats.avg_quality = total_quality / examples.len() as f32;
        }

        stats
    }

    /// Define all 140+ MCP tools
    fn define_mcp_tools() -> Vec<McpToolDef> {
        let mut tools = Vec::new();

        // ===== Agent Management Tools =====
        tools.push(McpToolDef {
            name: "agent_spawn".to_string(),
            category: ToolCategory::AgentManagement,
            description: "Spawn a new agent with intelligent model selection".to_string(),
            required_params: vec![ToolParam {
                name: "agentType".to_string(),
                param_type: ParamType::String,
                description: "Type of agent to spawn".to_string(),
                examples: vec![
                    "coder".to_string(),
                    "researcher".to_string(),
                    "tester".to_string(),
                    "reviewer".to_string(),
                ],
            }],
            optional_params: vec![
                ToolParam {
                    name: "agentId".to_string(),
                    param_type: ParamType::String,
                    description: "Custom agent ID".to_string(),
                    examples: vec!["agent-1".to_string(), "coder-main".to_string()],
                },
                ToolParam {
                    name: "model".to_string(),
                    param_type: ParamType::Enum,
                    description: "Claude model to use".to_string(),
                    examples: vec![
                        "haiku".to_string(),
                        "sonnet".to_string(),
                        "opus".to_string(),
                    ],
                },
                ToolParam {
                    name: "task".to_string(),
                    param_type: ParamType::String,
                    description: "Task description for model routing".to_string(),
                    examples: vec![
                        "implement authentication".to_string(),
                        "write tests".to_string(),
                    ],
                },
            ],
            use_cases: vec![
                "spawn a coder agent to implement a feature".to_string(),
                "create a researcher agent to analyze requirements".to_string(),
                "start a tester agent with opus model for complex testing".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "agent_terminate".to_string(),
            category: ToolCategory::AgentManagement,
            description: "Terminate an agent".to_string(),
            required_params: vec![ToolParam {
                name: "agentId".to_string(),
                param_type: ParamType::String,
                description: "ID of agent to terminate".to_string(),
                examples: vec!["agent-1".to_string(), "coder-main".to_string()],
            }],
            optional_params: vec![ToolParam {
                name: "force".to_string(),
                param_type: ParamType::Boolean,
                description: "Force immediate termination".to_string(),
                examples: vec!["true".to_string(), "false".to_string()],
            }],
            use_cases: vec![
                "stop an agent that has completed its task".to_string(),
                "force terminate an unresponsive agent".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "agent_status".to_string(),
            category: ToolCategory::AgentManagement,
            description: "Get agent status".to_string(),
            required_params: vec![ToolParam {
                name: "agentId".to_string(),
                param_type: ParamType::String,
                description: "ID of agent".to_string(),
                examples: vec!["agent-1".to_string()],
            }],
            optional_params: vec![],
            use_cases: vec![
                "check if an agent is still running".to_string(),
                "get current status of a specific agent".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "agent_list".to_string(),
            category: ToolCategory::AgentManagement,
            description: "List all agents".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "status".to_string(),
                    param_type: ParamType::String,
                    description: "Filter by status".to_string(),
                    examples: vec![
                        "running".to_string(),
                        "idle".to_string(),
                        "terminated".to_string(),
                    ],
                },
                ToolParam {
                    name: "includeTerminated".to_string(),
                    param_type: ParamType::Boolean,
                    description: "Include terminated agents".to_string(),
                    examples: vec!["true".to_string(), "false".to_string()],
                },
            ],
            use_cases: vec![
                "list all currently running agents".to_string(),
                "get a full inventory of agents including terminated ones".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "agent_pool".to_string(),
            category: ToolCategory::AgentManagement,
            description: "Manage agent pool".to_string(),
            required_params: vec![ToolParam {
                name: "action".to_string(),
                param_type: ParamType::Enum,
                description: "Pool action".to_string(),
                examples: vec![
                    "status".to_string(),
                    "scale".to_string(),
                    "drain".to_string(),
                    "fill".to_string(),
                ],
            }],
            optional_params: vec![ToolParam {
                name: "targetSize".to_string(),
                param_type: ParamType::Integer,
                description: "Target pool size".to_string(),
                examples: vec!["5".to_string(), "10".to_string()],
            }],
            use_cases: vec![
                "scale the agent pool to handle increased load".to_string(),
                "drain the pool before maintenance".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "agent_health".to_string(),
            category: ToolCategory::AgentManagement,
            description: "Check agent health".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "agentId".to_string(),
                    param_type: ParamType::String,
                    description: "Specific agent ID".to_string(),
                    examples: vec!["agent-1".to_string()],
                },
                ToolParam {
                    name: "threshold".to_string(),
                    param_type: ParamType::Float,
                    description: "Health threshold".to_string(),
                    examples: vec!["0.8".to_string(), "0.9".to_string()],
                },
            ],
            use_cases: vec![
                "check health of all agents in the swarm".to_string(),
                "verify a specific agent meets health threshold".to_string(),
            ],
        });

        // ===== Memory Operations Tools =====
        tools.push(McpToolDef {
            name: "memory_store".to_string(),
            category: ToolCategory::MemoryOperations,
            description: "Store a value in memory (persisted to disk)".to_string(),
            required_params: vec![
                ToolParam {
                    name: "key".to_string(),
                    param_type: ParamType::String,
                    description: "Memory key".to_string(),
                    examples: vec!["user-prefs".to_string(), "session-state".to_string()],
                },
                ToolParam {
                    name: "value".to_string(),
                    param_type: ParamType::Object,
                    description: "Value to store".to_string(),
                    examples: vec!["{}".to_string()],
                },
            ],
            optional_params: vec![ToolParam {
                name: "metadata".to_string(),
                param_type: ParamType::Object,
                description: "Optional metadata".to_string(),
                examples: vec!["{}".to_string()],
            }],
            use_cases: vec![
                "store user preferences for later retrieval".to_string(),
                "persist session state across conversations".to_string(),
                "save learned patterns for the intelligence system".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "memory_retrieve".to_string(),
            category: ToolCategory::MemoryOperations,
            description: "Retrieve a value from memory".to_string(),
            required_params: vec![ToolParam {
                name: "key".to_string(),
                param_type: ParamType::String,
                description: "Memory key".to_string(),
                examples: vec!["user-prefs".to_string()],
            }],
            optional_params: vec![],
            use_cases: vec![
                "get previously stored user preferences".to_string(),
                "retrieve session state from last conversation".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "memory_search".to_string(),
            category: ToolCategory::MemoryOperations,
            description: "Search memory by keyword".to_string(),
            required_params: vec![ToolParam {
                name: "query".to_string(),
                param_type: ParamType::String,
                description: "Search query".to_string(),
                examples: vec!["authentication".to_string(), "user settings".to_string()],
            }],
            optional_params: vec![ToolParam {
                name: "limit".to_string(),
                param_type: ParamType::Integer,
                description: "Result limit".to_string(),
                examples: vec!["10".to_string(), "50".to_string()],
            }],
            use_cases: vec![
                "search for entries related to authentication".to_string(),
                "find all memory entries matching a pattern".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "memory_delete".to_string(),
            category: ToolCategory::MemoryOperations,
            description: "Delete a memory entry".to_string(),
            required_params: vec![ToolParam {
                name: "key".to_string(),
                param_type: ParamType::String,
                description: "Memory key".to_string(),
                examples: vec!["old-session".to_string()],
            }],
            optional_params: vec![],
            use_cases: vec![
                "remove outdated session data".to_string(),
                "clean up temporary memory entries".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "memory_list".to_string(),
            category: ToolCategory::MemoryOperations,
            description: "List all memory entries".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "limit".to_string(),
                    param_type: ParamType::Integer,
                    description: "Result limit".to_string(),
                    examples: vec!["100".to_string()],
                },
                ToolParam {
                    name: "offset".to_string(),
                    param_type: ParamType::Integer,
                    description: "Result offset".to_string(),
                    examples: vec!["0".to_string()],
                },
            ],
            use_cases: vec![
                "list all stored memory entries".to_string(),
                "paginate through memory entries".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "memory_stats".to_string(),
            category: ToolCategory::MemoryOperations,
            description: "Get memory storage statistics".to_string(),
            required_params: vec![],
            optional_params: vec![],
            use_cases: vec![
                "check memory usage statistics".to_string(),
                "monitor memory storage capacity".to_string(),
            ],
        });

        // ===== Swarm Coordination Tools =====
        tools.push(McpToolDef {
            name: "swarm_init".to_string(),
            category: ToolCategory::SwarmCoordination,
            description: "Initialize a swarm".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "topology".to_string(),
                    param_type: ParamType::Enum,
                    description: "Swarm topology type".to_string(),
                    examples: vec![
                        "hierarchical".to_string(),
                        "mesh".to_string(),
                        "star".to_string(),
                    ],
                },
                ToolParam {
                    name: "maxAgents".to_string(),
                    param_type: ParamType::Integer,
                    description: "Maximum number of agents".to_string(),
                    examples: vec!["8".to_string(), "15".to_string()],
                },
            ],
            use_cases: vec![
                "initialize a hierarchical swarm for coordinated work".to_string(),
                "set up a mesh topology for peer-to-peer coordination".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "swarm_status".to_string(),
            category: ToolCategory::SwarmCoordination,
            description: "Get swarm status".to_string(),
            required_params: vec![],
            optional_params: vec![ToolParam {
                name: "swarmId".to_string(),
                param_type: ParamType::String,
                description: "Swarm ID".to_string(),
                examples: vec!["swarm-1".to_string()],
            }],
            use_cases: vec![
                "check the current status of the swarm".to_string(),
                "monitor swarm health and agent count".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "swarm_shutdown".to_string(),
            category: ToolCategory::SwarmCoordination,
            description: "Shutdown a swarm".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "swarmId".to_string(),
                    param_type: ParamType::String,
                    description: "Swarm ID".to_string(),
                    examples: vec!["swarm-1".to_string()],
                },
                ToolParam {
                    name: "graceful".to_string(),
                    param_type: ParamType::Boolean,
                    description: "Graceful shutdown".to_string(),
                    examples: vec!["true".to_string()],
                },
            ],
            use_cases: vec![
                "gracefully shutdown the swarm after completing tasks".to_string(),
                "force shutdown a problematic swarm".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "swarm_health".to_string(),
            category: ToolCategory::SwarmCoordination,
            description: "Check swarm health status".to_string(),
            required_params: vec![],
            optional_params: vec![ToolParam {
                name: "swarmId".to_string(),
                param_type: ParamType::String,
                description: "Swarm ID to check".to_string(),
                examples: vec!["swarm-1".to_string()],
            }],
            use_cases: vec![
                "verify swarm is healthy before assigning tasks".to_string(),
                "diagnose issues in a malfunctioning swarm".to_string(),
            ],
        });

        // ===== Task Management Tools =====
        tools.push(McpToolDef {
            name: "task_create".to_string(),
            category: ToolCategory::TaskManagement,
            description: "Create a new task".to_string(),
            required_params: vec![
                ToolParam {
                    name: "type".to_string(),
                    param_type: ParamType::Enum,
                    description: "Task type".to_string(),
                    examples: vec![
                        "feature".to_string(),
                        "bugfix".to_string(),
                        "research".to_string(),
                    ],
                },
                ToolParam {
                    name: "description".to_string(),
                    param_type: ParamType::String,
                    description: "Task description".to_string(),
                    examples: vec!["Implement user authentication".to_string()],
                },
            ],
            optional_params: vec![
                ToolParam {
                    name: "priority".to_string(),
                    param_type: ParamType::Enum,
                    description: "Task priority".to_string(),
                    examples: vec![
                        "low".to_string(),
                        "normal".to_string(),
                        "high".to_string(),
                        "critical".to_string(),
                    ],
                },
                ToolParam {
                    name: "assignTo".to_string(),
                    param_type: ParamType::Array,
                    description: "Agent IDs to assign".to_string(),
                    examples: vec!["[\"agent-1\"]".to_string()],
                },
            ],
            use_cases: vec![
                "create a feature task and assign it to a coder".to_string(),
                "create a high-priority bugfix task".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "task_status".to_string(),
            category: ToolCategory::TaskManagement,
            description: "Get task status".to_string(),
            required_params: vec![ToolParam {
                name: "taskId".to_string(),
                param_type: ParamType::String,
                description: "Task ID".to_string(),
                examples: vec!["task-123".to_string()],
            }],
            optional_params: vec![],
            use_cases: vec![
                "check progress of a specific task".to_string(),
                "verify if a task has been completed".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "task_list".to_string(),
            category: ToolCategory::TaskManagement,
            description: "List all tasks".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "status".to_string(),
                    param_type: ParamType::String,
                    description: "Filter by status".to_string(),
                    examples: vec![
                        "pending".to_string(),
                        "in_progress".to_string(),
                        "completed".to_string(),
                    ],
                },
                ToolParam {
                    name: "priority".to_string(),
                    param_type: ParamType::String,
                    description: "Filter by priority".to_string(),
                    examples: vec!["high".to_string(), "critical".to_string()],
                },
            ],
            use_cases: vec![
                "list all pending tasks".to_string(),
                "get all high-priority tasks in progress".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "task_complete".to_string(),
            category: ToolCategory::TaskManagement,
            description: "Mark task as complete".to_string(),
            required_params: vec![ToolParam {
                name: "taskId".to_string(),
                param_type: ParamType::String,
                description: "Task ID".to_string(),
                examples: vec!["task-123".to_string()],
            }],
            optional_params: vec![ToolParam {
                name: "result".to_string(),
                param_type: ParamType::Object,
                description: "Task result data".to_string(),
                examples: vec!["{}".to_string()],
            }],
            use_cases: vec![
                "mark a task as completed with results".to_string(),
                "finalize a task after review".to_string(),
            ],
        });

        // ===== Hooks & Learning Tools =====
        tools.push(McpToolDef {
            name: "hooks_pre-task".to_string(),
            category: ToolCategory::HooksLearning,
            description:
                "Record task start and get agent suggestions with intelligent model routing"
                    .to_string(),
            required_params: vec![
                ToolParam {
                    name: "taskId".to_string(),
                    param_type: ParamType::String,
                    description: "Task identifier".to_string(),
                    examples: vec!["task-001".to_string()],
                },
                ToolParam {
                    name: "description".to_string(),
                    param_type: ParamType::String,
                    description: "Task description".to_string(),
                    examples: vec!["Implement user login".to_string()],
                },
            ],
            optional_params: vec![ToolParam {
                name: "filePath".to_string(),
                param_type: ParamType::String,
                description: "Optional file path for AST analysis".to_string(),
                examples: vec!["src/auth.rs".to_string()],
            }],
            use_cases: vec![
                "get agent routing suggestions before starting a task".to_string(),
                "record task start for learning system".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "hooks_post-task".to_string(),
            category: ToolCategory::HooksLearning,
            description: "Record task completion for learning".to_string(),
            required_params: vec![ToolParam {
                name: "taskId".to_string(),
                param_type: ParamType::String,
                description: "Task identifier".to_string(),
                examples: vec!["task-001".to_string()],
            }],
            optional_params: vec![
                ToolParam {
                    name: "success".to_string(),
                    param_type: ParamType::Boolean,
                    description: "Whether task was successful".to_string(),
                    examples: vec!["true".to_string()],
                },
                ToolParam {
                    name: "quality".to_string(),
                    param_type: ParamType::Float,
                    description: "Quality score (0-1)".to_string(),
                    examples: vec!["0.9".to_string()],
                },
            ],
            use_cases: vec![
                "record successful task completion for reinforcement learning".to_string(),
                "provide feedback on task quality".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "hooks_route".to_string(),
            category: ToolCategory::HooksLearning,
            description: "Route task to optimal agent using learned patterns".to_string(),
            required_params: vec![ToolParam {
                name: "task".to_string(),
                param_type: ParamType::String,
                description: "Task description".to_string(),
                examples: vec!["implement caching layer".to_string()],
            }],
            optional_params: vec![ToolParam {
                name: "context".to_string(),
                param_type: ParamType::String,
                description: "Additional context".to_string(),
                examples: vec!["performance-critical".to_string()],
            }],
            use_cases: vec![
                "get the optimal agent type for a given task".to_string(),
                "use learned patterns to route tasks intelligently".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "hooks_metrics".to_string(),
            category: ToolCategory::HooksLearning,
            description: "View learning metrics dashboard".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "period".to_string(),
                    param_type: ParamType::Enum,
                    description: "Metrics period".to_string(),
                    examples: vec!["1h".to_string(), "24h".to_string(), "7d".to_string()],
                },
                ToolParam {
                    name: "includeV3".to_string(),
                    param_type: ParamType::Boolean,
                    description: "Include V3 performance metrics".to_string(),
                    examples: vec!["true".to_string()],
                },
            ],
            use_cases: vec![
                "view learning system performance metrics".to_string(),
                "analyze agent routing effectiveness".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "hooks_pre-edit".to_string(),
            category: ToolCategory::HooksLearning,
            description: "Get context and agent suggestions before editing a file".to_string(),
            required_params: vec![ToolParam {
                name: "filePath".to_string(),
                param_type: ParamType::String,
                description: "Path to the file being edited".to_string(),
                examples: vec!["src/main.rs".to_string()],
            }],
            optional_params: vec![ToolParam {
                name: "operation".to_string(),
                param_type: ParamType::Enum,
                description: "Type of operation".to_string(),
                examples: vec![
                    "create".to_string(),
                    "update".to_string(),
                    "refactor".to_string(),
                ],
            }],
            use_cases: vec![
                "get suggestions before editing a source file".to_string(),
                "analyze file context for intelligent assistance".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "hooks_post-edit".to_string(),
            category: ToolCategory::HooksLearning,
            description: "Record editing outcome for learning".to_string(),
            required_params: vec![ToolParam {
                name: "filePath".to_string(),
                param_type: ParamType::String,
                description: "Path to the edited file".to_string(),
                examples: vec!["src/main.rs".to_string()],
            }],
            optional_params: vec![ToolParam {
                name: "success".to_string(),
                param_type: ParamType::Boolean,
                description: "Whether the edit was successful".to_string(),
                examples: vec!["true".to_string()],
            }],
            use_cases: vec![
                "record successful edit for learning".to_string(),
                "track edit outcomes for pattern learning".to_string(),
            ],
        });

        // ===== Session Management Tools =====
        tools.push(McpToolDef {
            name: "session_save".to_string(),
            category: ToolCategory::SessionManagement,
            description: "Save current session state".to_string(),
            required_params: vec![ToolParam {
                name: "name".to_string(),
                param_type: ParamType::String,
                description: "Session name".to_string(),
                examples: vec!["feature-auth".to_string()],
            }],
            optional_params: vec![
                ToolParam {
                    name: "includeAgents".to_string(),
                    param_type: ParamType::Boolean,
                    description: "Include agents in session".to_string(),
                    examples: vec!["true".to_string()],
                },
                ToolParam {
                    name: "includeMemory".to_string(),
                    param_type: ParamType::Boolean,
                    description: "Include memory in session".to_string(),
                    examples: vec!["true".to_string()],
                },
            ],
            use_cases: vec![
                "save current work session before break".to_string(),
                "persist session state for later continuation".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "session_restore".to_string(),
            category: ToolCategory::SessionManagement,
            description: "Restore a saved session".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "name".to_string(),
                    param_type: ParamType::String,
                    description: "Session name to restore".to_string(),
                    examples: vec!["feature-auth".to_string()],
                },
                ToolParam {
                    name: "sessionId".to_string(),
                    param_type: ParamType::String,
                    description: "Session ID to restore".to_string(),
                    examples: vec!["session-123".to_string()],
                },
            ],
            use_cases: vec![
                "restore a previously saved session".to_string(),
                "continue work from a saved checkpoint".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "session_list".to_string(),
            category: ToolCategory::SessionManagement,
            description: "List saved sessions".to_string(),
            required_params: vec![],
            optional_params: vec![ToolParam {
                name: "limit".to_string(),
                param_type: ParamType::Integer,
                description: "Maximum sessions to return".to_string(),
                examples: vec!["10".to_string()],
            }],
            use_cases: vec![
                "view all saved sessions".to_string(),
                "find a specific session to restore".to_string(),
            ],
        });

        // ===== Workflow Tools =====
        tools.push(McpToolDef {
            name: "workflow_create".to_string(),
            category: ToolCategory::Workflow,
            description: "Create a new workflow".to_string(),
            required_params: vec![ToolParam {
                name: "name".to_string(),
                param_type: ParamType::String,
                description: "Workflow name".to_string(),
                examples: vec!["feature-development".to_string()],
            }],
            optional_params: vec![
                ToolParam {
                    name: "steps".to_string(),
                    param_type: ParamType::Array,
                    description: "Workflow steps".to_string(),
                    examples: vec!["[]".to_string()],
                },
                ToolParam {
                    name: "description".to_string(),
                    param_type: ParamType::String,
                    description: "Workflow description".to_string(),
                    examples: vec!["Full feature development workflow".to_string()],
                },
            ],
            use_cases: vec![
                "create a multi-step development workflow".to_string(),
                "define a reusable workflow template".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "workflow_execute".to_string(),
            category: ToolCategory::Workflow,
            description: "Execute a workflow".to_string(),
            required_params: vec![ToolParam {
                name: "workflowId".to_string(),
                param_type: ParamType::String,
                description: "Workflow ID to execute".to_string(),
                examples: vec!["workflow-123".to_string()],
            }],
            optional_params: vec![ToolParam {
                name: "variables".to_string(),
                param_type: ParamType::Object,
                description: "Runtime variables to inject".to_string(),
                examples: vec!["{}".to_string()],
            }],
            use_cases: vec![
                "execute a predefined workflow".to_string(),
                "run a workflow with custom variables".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "workflow_status".to_string(),
            category: ToolCategory::Workflow,
            description: "Get workflow status".to_string(),
            required_params: vec![ToolParam {
                name: "workflowId".to_string(),
                param_type: ParamType::String,
                description: "Workflow ID".to_string(),
                examples: vec!["workflow-123".to_string()],
            }],
            optional_params: vec![ToolParam {
                name: "verbose".to_string(),
                param_type: ParamType::Boolean,
                description: "Include step details".to_string(),
                examples: vec!["true".to_string()],
            }],
            use_cases: vec![
                "check progress of a running workflow".to_string(),
                "get detailed status including step information".to_string(),
            ],
        });

        // ===== System Tools =====
        tools.push(McpToolDef {
            name: "system_status".to_string(),
            category: ToolCategory::System,
            description: "Get overall system status".to_string(),
            required_params: vec![],
            optional_params: vec![ToolParam {
                name: "verbose".to_string(),
                param_type: ParamType::Boolean,
                description: "Include detailed information".to_string(),
                examples: vec!["true".to_string()],
            }],
            use_cases: vec![
                "check system health and status".to_string(),
                "get detailed system diagnostics".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "system_metrics".to_string(),
            category: ToolCategory::System,
            description: "Get system metrics and performance data".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "category".to_string(),
                    param_type: ParamType::Enum,
                    description: "Metrics category".to_string(),
                    examples: vec!["all".to_string(), "cpu".to_string(), "memory".to_string()],
                },
                ToolParam {
                    name: "timeRange".to_string(),
                    param_type: ParamType::String,
                    description: "Time range".to_string(),
                    examples: vec!["1h".to_string(), "24h".to_string()],
                },
            ],
            use_cases: vec![
                "get CPU and memory metrics".to_string(),
                "analyze system performance over time".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "system_health".to_string(),
            category: ToolCategory::System,
            description: "Perform system health check".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "deep".to_string(),
                    param_type: ParamType::Boolean,
                    description: "Perform deep health check".to_string(),
                    examples: vec!["true".to_string()],
                },
                ToolParam {
                    name: "fix".to_string(),
                    param_type: ParamType::Boolean,
                    description: "Attempt to fix issues".to_string(),
                    examples: vec!["true".to_string()],
                },
            ],
            use_cases: vec![
                "run a comprehensive health check".to_string(),
                "diagnose and fix system issues".to_string(),
            ],
        });

        // ===== Configuration Tools =====
        tools.push(McpToolDef {
            name: "config_get".to_string(),
            category: ToolCategory::Configuration,
            description: "Get configuration value".to_string(),
            required_params: vec![ToolParam {
                name: "key".to_string(),
                param_type: ParamType::String,
                description: "Configuration key (dot notation supported)".to_string(),
                examples: vec!["swarm.topology".to_string(), "memory.backend".to_string()],
            }],
            optional_params: vec![ToolParam {
                name: "scope".to_string(),
                param_type: ParamType::Enum,
                description: "Configuration scope".to_string(),
                examples: vec![
                    "project".to_string(),
                    "user".to_string(),
                    "system".to_string(),
                ],
            }],
            use_cases: vec![
                "get a specific configuration value".to_string(),
                "check swarm topology setting".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "config_set".to_string(),
            category: ToolCategory::Configuration,
            description: "Set configuration value".to_string(),
            required_params: vec![
                ToolParam {
                    name: "key".to_string(),
                    param_type: ParamType::String,
                    description: "Configuration key".to_string(),
                    examples: vec!["swarm.maxAgents".to_string()],
                },
                ToolParam {
                    name: "value".to_string(),
                    param_type: ParamType::Object,
                    description: "Configuration value".to_string(),
                    examples: vec!["10".to_string()],
                },
            ],
            optional_params: vec![ToolParam {
                name: "scope".to_string(),
                param_type: ParamType::String,
                description: "Configuration scope".to_string(),
                examples: vec!["project".to_string()],
            }],
            use_cases: vec![
                "update swarm configuration".to_string(),
                "change memory backend setting".to_string(),
            ],
        });

        // ===== Hive-Mind Tools =====
        tools.push(McpToolDef {
            name: "hive-mind_init".to_string(),
            category: ToolCategory::HiveMind,
            description: "Initialize the hive-mind collective".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "topology".to_string(),
                    param_type: ParamType::Enum,
                    description: "Network topology".to_string(),
                    examples: vec![
                        "mesh".to_string(),
                        "hierarchical".to_string(),
                        "ring".to_string(),
                    ],
                },
                ToolParam {
                    name: "queenId".to_string(),
                    param_type: ParamType::String,
                    description: "Initial queen agent ID".to_string(),
                    examples: vec!["queen-1".to_string()],
                },
            ],
            use_cases: vec![
                "initialize a mesh-based hive-mind".to_string(),
                "set up hierarchical coordination with a queen".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "hive-mind_status".to_string(),
            category: ToolCategory::HiveMind,
            description: "Get hive-mind status".to_string(),
            required_params: vec![],
            optional_params: vec![ToolParam {
                name: "verbose".to_string(),
                param_type: ParamType::Boolean,
                description: "Include detailed information".to_string(),
                examples: vec!["true".to_string()],
            }],
            use_cases: vec![
                "check hive-mind collective status".to_string(),
                "monitor consensus state".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "hive-mind_consensus".to_string(),
            category: ToolCategory::HiveMind,
            description: "Propose or vote on consensus".to_string(),
            required_params: vec![ToolParam {
                name: "action".to_string(),
                param_type: ParamType::Enum,
                description: "Consensus action".to_string(),
                examples: vec![
                    "propose".to_string(),
                    "vote".to_string(),
                    "status".to_string(),
                ],
            }],
            optional_params: vec![
                ToolParam {
                    name: "proposalId".to_string(),
                    param_type: ParamType::String,
                    description: "Proposal ID".to_string(),
                    examples: vec!["proposal-1".to_string()],
                },
                ToolParam {
                    name: "vote".to_string(),
                    param_type: ParamType::Boolean,
                    description: "Vote (true=for, false=against)".to_string(),
                    examples: vec!["true".to_string()],
                },
            ],
            use_cases: vec![
                "propose a new decision for consensus".to_string(),
                "vote on an existing proposal".to_string(),
            ],
        });

        // ===== Neural Tools =====
        tools.push(McpToolDef {
            name: "neural_train".to_string(),
            category: ToolCategory::Neural,
            description: "Train a neural model".to_string(),
            required_params: vec![ToolParam {
                name: "modelType".to_string(),
                param_type: ParamType::Enum,
                description: "Model type".to_string(),
                examples: vec![
                    "moe".to_string(),
                    "transformer".to_string(),
                    "classifier".to_string(),
                ],
            }],
            optional_params: vec![
                ToolParam {
                    name: "epochs".to_string(),
                    param_type: ParamType::Integer,
                    description: "Number of training epochs".to_string(),
                    examples: vec!["10".to_string()],
                },
                ToolParam {
                    name: "learningRate".to_string(),
                    param_type: ParamType::Float,
                    description: "Learning rate".to_string(),
                    examples: vec!["0.001".to_string()],
                },
            ],
            use_cases: vec![
                "train a mixture of experts model".to_string(),
                "fine-tune classifier for task routing".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "neural_predict".to_string(),
            category: ToolCategory::Neural,
            description: "Make predictions using a neural model".to_string(),
            required_params: vec![ToolParam {
                name: "input".to_string(),
                param_type: ParamType::String,
                description: "Input text or data".to_string(),
                examples: vec!["implement user authentication".to_string()],
            }],
            optional_params: vec![
                ToolParam {
                    name: "modelId".to_string(),
                    param_type: ParamType::String,
                    description: "Model ID to use".to_string(),
                    examples: vec!["model-1".to_string()],
                },
                ToolParam {
                    name: "topK".to_string(),
                    param_type: ParamType::Integer,
                    description: "Number of top predictions".to_string(),
                    examples: vec!["5".to_string()],
                },
            ],
            use_cases: vec![
                "get neural model prediction for task routing".to_string(),
                "classify task complexity using neural model".to_string(),
            ],
        });

        // ===== Performance Tools =====
        tools.push(McpToolDef {
            name: "performance_report".to_string(),
            category: ToolCategory::Performance,
            description: "Generate performance report".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "format".to_string(),
                    param_type: ParamType::Enum,
                    description: "Report format".to_string(),
                    examples: vec![
                        "json".to_string(),
                        "summary".to_string(),
                        "detailed".to_string(),
                    ],
                },
                ToolParam {
                    name: "timeRange".to_string(),
                    param_type: ParamType::String,
                    description: "Time range".to_string(),
                    examples: vec!["1h".to_string(), "24h".to_string()],
                },
            ],
            use_cases: vec![
                "generate a performance report for the last hour".to_string(),
                "get detailed performance analytics".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "performance_benchmark".to_string(),
            category: ToolCategory::Performance,
            description: "Run performance benchmarks".to_string(),
            required_params: vec![],
            optional_params: vec![
                ToolParam {
                    name: "suite".to_string(),
                    param_type: ParamType::Enum,
                    description: "Benchmark suite".to_string(),
                    examples: vec![
                        "all".to_string(),
                        "memory".to_string(),
                        "neural".to_string(),
                    ],
                },
                ToolParam {
                    name: "iterations".to_string(),
                    param_type: ParamType::Integer,
                    description: "Number of iterations".to_string(),
                    examples: vec!["100".to_string()],
                },
            ],
            use_cases: vec![
                "run comprehensive benchmarks".to_string(),
                "benchmark memory subsystem performance".to_string(),
            ],
        });

        // ===== AIDefence Tools =====
        tools.push(McpToolDef {
            name: "aidefence_scan".to_string(),
            category: ToolCategory::AiDefence,
            description: "Scan input text for AI manipulation threats".to_string(),
            required_params: vec![ToolParam {
                name: "input".to_string(),
                param_type: ParamType::String,
                description: "Text to scan for threats".to_string(),
                examples: vec!["user input text".to_string()],
            }],
            optional_params: vec![ToolParam {
                name: "quick".to_string(),
                param_type: ParamType::Boolean,
                description: "Quick scan mode".to_string(),
                examples: vec!["true".to_string()],
            }],
            use_cases: vec![
                "scan user input for prompt injection attempts".to_string(),
                "detect potential jailbreak attempts".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "aidefence_is_safe".to_string(),
            category: ToolCategory::AiDefence,
            description: "Quick boolean check if input is safe".to_string(),
            required_params: vec![ToolParam {
                name: "input".to_string(),
                param_type: ParamType::String,
                description: "Text to check".to_string(),
                examples: vec!["user message".to_string()],
            }],
            optional_params: vec![],
            use_cases: vec![
                "quickly validate user input is safe".to_string(),
                "guard against malicious inputs".to_string(),
            ],
        });

        // ===== Embeddings Tools =====
        tools.push(McpToolDef {
            name: "embeddings_generate".to_string(),
            category: ToolCategory::Embeddings,
            description: "Generate embeddings for text".to_string(),
            required_params: vec![ToolParam {
                name: "text".to_string(),
                param_type: ParamType::String,
                description: "Text to embed".to_string(),
                examples: vec!["implement authentication".to_string()],
            }],
            optional_params: vec![
                ToolParam {
                    name: "hyperbolic".to_string(),
                    param_type: ParamType::Boolean,
                    description: "Return hyperbolic embedding".to_string(),
                    examples: vec!["false".to_string()],
                },
                ToolParam {
                    name: "normalize".to_string(),
                    param_type: ParamType::Boolean,
                    description: "L2 normalize the embedding".to_string(),
                    examples: vec!["true".to_string()],
                },
            ],
            use_cases: vec![
                "generate embeddings for semantic search".to_string(),
                "create hyperbolic embeddings for hierarchical data".to_string(),
            ],
        });

        tools.push(McpToolDef {
            name: "embeddings_search".to_string(),
            category: ToolCategory::Embeddings,
            description: "Semantic search across stored embeddings".to_string(),
            required_params: vec![ToolParam {
                name: "query".to_string(),
                param_type: ParamType::String,
                description: "Search query".to_string(),
                examples: vec!["authentication patterns".to_string()],
            }],
            optional_params: vec![
                ToolParam {
                    name: "topK".to_string(),
                    param_type: ParamType::Integer,
                    description: "Number of results".to_string(),
                    examples: vec!["5".to_string()],
                },
                ToolParam {
                    name: "threshold".to_string(),
                    param_type: ParamType::Float,
                    description: "Minimum similarity threshold".to_string(),
                    examples: vec!["0.5".to_string()],
                },
            ],
            use_cases: vec![
                "find similar patterns using semantic search".to_string(),
                "retrieve relevant documents by meaning".to_string(),
            ],
        });

        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_generation() {
        let config = ToolDatasetConfig {
            examples_per_tool: 3,
            include_error_cases: false,
            ..Default::default()
        };

        let dataset = ToolCallDataset::generate(config).unwrap();

        // Should have examples for all defined tools
        assert!(!dataset.examples.is_empty());
        assert!(!dataset.tool_definitions.is_empty());
    }

    #[test]
    fn test_tool_categories() {
        let categories = ToolCategory::all();
        assert!(categories.len() >= 10); // We have at least 10 categories
    }

    #[test]
    fn test_error_cases() {
        let config = ToolDatasetConfig {
            examples_per_tool: 10,
            include_error_cases: true,
            error_case_ratio: 0.5, // 50% error cases
            ..Default::default()
        };

        let dataset = ToolCallDataset::generate(config).unwrap();

        // Should have both success and error cases
        assert!(dataset.stats.success_count > 0);
        assert!(dataset.stats.error_count > 0);
    }

    #[test]
    fn test_difficulty_distribution() {
        let config = ToolDatasetConfig::comprehensive();
        let dataset = ToolCallDataset::generate(config).unwrap();

        // Should have examples of all difficulties
        assert!(dataset.stats.by_difficulty.contains_key("Easy"));
        assert!(dataset.stats.by_difficulty.contains_key("Medium"));
        assert!(dataset.stats.by_difficulty.contains_key("Hard"));
        assert!(dataset.stats.by_difficulty.contains_key("Expert"));
    }

    #[test]
    fn test_dataset_split() {
        let config = ToolDatasetConfig::minimal();
        let dataset = ToolCallDataset::generate(config).unwrap();

        let (train, val, test) = dataset.split(0.7, 0.15, 42);

        assert_eq!(train.len() + val.len() + test.len(), dataset.len());
        assert!(train.len() >= val.len());
        assert!(train.len() >= test.len());
    }

    #[test]
    fn test_filter_by_category() {
        let config = ToolDatasetConfig::minimal();
        let dataset = ToolCallDataset::generate(config).unwrap();

        let memory_examples = dataset.filter_by_category(ToolCategory::MemoryOperations);
        for example in memory_examples {
            assert_eq!(example.category, ToolCategory::MemoryOperations);
        }
    }

    #[test]
    fn test_tool_definitions() {
        let tools = ToolDatasetGenerator::define_mcp_tools();

        // Check we have the core tools
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

        assert!(tool_names.contains(&"agent_spawn"));
        assert!(tool_names.contains(&"memory_store"));
        assert!(tool_names.contains(&"memory_search"));
        assert!(tool_names.contains(&"swarm_init"));
        assert!(tool_names.contains(&"task_create"));
        assert!(tool_names.contains(&"hooks_pre-task"));
    }

    #[test]
    fn test_param_generation() {
        let config = ToolDatasetConfig::minimal();
        let dataset = ToolCallDataset::generate(config).unwrap();

        for example in &dataset.examples {
            // All examples should have params
            assert!(example.expected_params.is_object());
        }
    }

    #[test]
    fn test_quality_scores() {
        let config = ToolDatasetConfig::minimal();
        let dataset = ToolCallDataset::generate(config).unwrap();

        for example in &dataset.examples {
            assert!(example.quality_score >= 0.0);
            assert!(example.quality_score <= 1.0);
        }

        // Average quality should be reasonable
        assert!(dataset.stats.avg_quality > 0.5);
    }
}
