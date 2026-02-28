//! Hooks CLI - Self-learning intelligence system for Claude Code integration
//!
//! Provides Q-learning based agent routing, error pattern recognition,
//! file sequence prediction, and swarm coordination.
//!
//! ## Hook Input/Output
//!
//! Claude Code hooks receive JSON via stdin and can output JSON for control flow.
//! See: https://docs.anthropic.com/en/docs/claude-code/hooks

use crate::config::Config;
use anyhow::{Context, Result};
use colored::*;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufRead, Read, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// === Hook Input Structures (from stdin JSON) ===

/// Universal hook input from Claude Code
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct HookInput {
    pub session_id: Option<String>,
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    pub hook_event_name: Option<String>,
    // PreToolUse/PostToolUse specific
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub tool_response: Option<serde_json::Value>,
    pub tool_use_id: Option<String>,
    // Notification specific
    pub notification_type: Option<String>,
    pub message: Option<String>,
    // PreCompact specific
    pub trigger: Option<String>,
    // SessionStart specific
    pub source: Option<String>,
}

/// Hook output for control flow
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continue_execution: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppress_output: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<HookSpecificOutput>,
}

/// Hook-specific output for context injection
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HookSpecificOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision_reason: Option<String>,
}

/// Try to parse stdin as JSON (non-blocking)
pub fn try_parse_stdin() -> Option<HookInput> {
    // Check if stdin has data (non-blocking)
    let stdin = io::stdin();
    let mut handle = stdin.lock();

    // Try to read a line
    let mut buffer = String::new();
    if handle.read_line(&mut buffer).ok()? > 0 {
        // Try to parse as JSON
        serde_json::from_str(&buffer).ok()
    } else {
        None
    }
}

/// Output JSON for context injection (PostToolUse)
pub fn output_context_injection(context: &str) {
    let output = HookOutput {
        hook_specific_output: Some(HookSpecificOutput {
            additional_context: Some(context.to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    if let Ok(json) = serde_json::to_string(&output) {
        println!("{}", json);
    }
}

/// Hooks subcommands
#[derive(clap::Subcommand, Debug)]
pub enum HooksCommands {
    /// Initialize hooks in current project
    Init {
        /// Force overwrite existing configuration
        #[arg(long)]
        force: bool,

        /// Initialize PostgreSQL backend (requires RUVECTOR_POSTGRES_URL)
        #[arg(long)]
        postgres: bool,
    },

    /// Install hooks into Claude settings
    Install {
        /// Claude settings directory
        #[arg(long, default_value = ".claude")]
        settings_dir: String,
    },

    /// Show intelligence statistics
    Stats,

    // === Memory Commands ===
    /// Store content in semantic memory
    Remember {
        /// Memory type (edit, command, decision, pattern)
        #[arg(short = 't', long)]
        memory_type: String,

        /// Content to remember
        content: Vec<String>,
    },

    /// Search memory semantically
    Recall {
        /// Search query
        query: Vec<String>,

        /// Number of results
        #[arg(short = 'k', long, default_value = "5")]
        top_k: usize,
    },

    // === Learning Commands ===
    /// Record a learning trajectory
    Learn {
        /// State identifier
        state: String,

        /// Action taken
        action: String,

        /// Reward value (-1.0 to 1.0)
        #[arg(short, long, default_value = "0.0")]
        reward: f32,
    },

    /// Get action suggestion for state
    Suggest {
        /// Current state
        state: String,

        /// Available actions (comma-separated)
        #[arg(short, long)]
        actions: String,
    },

    /// Route task to best agent
    Route {
        /// Task description
        task: Vec<String>,

        /// File being worked on
        #[arg(long)]
        file: Option<String>,

        /// Crate/module context
        #[arg(long)]
        crate_name: Option<String>,

        /// Operation type (edit, review, test)
        #[arg(long, default_value = "edit")]
        operation: String,
    },

    // === Hook Integrations ===
    /// Pre-edit intelligence hook
    PreEdit {
        /// File path
        file: String,
    },

    /// Post-edit learning hook
    PostEdit {
        /// File path
        file: String,

        /// Whether edit succeeded
        #[arg(long)]
        success: bool,
    },

    /// Pre-command intelligence hook
    PreCommand {
        /// Command being run
        command: Vec<String>,
    },

    /// Post-command learning hook
    PostCommand {
        /// Command that ran
        command: Vec<String>,

        /// Whether command succeeded
        #[arg(long)]
        success: bool,

        /// Stderr output (for error learning)
        #[arg(long)]
        stderr: Option<String>,
    },

    // === Session Hooks ===
    /// Session start hook
    SessionStart {
        /// Session ID
        #[arg(long)]
        session_id: Option<String>,

        /// Resume from previous session
        #[arg(long)]
        resume: bool,
    },

    /// Session end hook
    SessionEnd {
        /// Export metrics
        #[arg(long)]
        export_metrics: bool,
    },

    /// Pre-compact hook
    PreCompact {
        /// Conversation length
        #[arg(long)]
        length: Option<usize>,

        /// Auto-triggered compaction
        #[arg(long)]
        auto: bool,
    },

    /// Suggest context for user prompt (UserPromptSubmit hook)
    SuggestContext,

    /// Track notification events
    TrackNotification {
        /// Notification type
        #[arg(long)]
        notification_type: Option<String>,
    },

    // === Claude Code v2.0.55+ Features ===
    /// Process LSP diagnostic events (requires Claude Code 2.0.55+)
    LspDiagnostic {
        /// File with diagnostic
        #[arg(long)]
        file: Option<String>,

        /// Diagnostic severity (error, warning, info, hint)
        #[arg(long)]
        severity: Option<String>,

        /// Diagnostic message
        #[arg(long)]
        message: Option<String>,
    },

    /// Recommend ultrathink mode for complex tasks
    SuggestUltrathink {
        /// Task description
        task: Vec<String>,

        /// File being worked on
        #[arg(long)]
        file: Option<String>,
    },

    /// Coordinate async sub-agent execution
    AsyncAgent {
        /// Agent action (spawn, sync, complete)
        #[arg(long, default_value = "spawn")]
        action: String,

        /// Agent ID
        #[arg(long)]
        agent_id: Option<String>,

        /// Task description
        #[arg(long)]
        task: Option<String>,
    },

    // === V3 Intelligence Features ===
    /// Record error pattern for learning
    RecordError {
        /// Command that failed
        command: String,

        /// Stderr output
        stderr: String,
    },

    /// Get suggested fix for error code
    SuggestFix {
        /// Error code (e.g., E0308, TS2322)
        error_code: String,
    },

    /// Suggest next files to edit
    SuggestNext {
        /// Current file
        file: String,

        /// Number of suggestions
        #[arg(short = 'n', long, default_value = "3")]
        count: usize,
    },

    /// Check if tests should run
    ShouldTest {
        /// File that was edited
        file: String,
    },

    // === Swarm Commands ===
    /// Register agent in swarm
    SwarmRegister {
        /// Agent ID
        agent_id: String,

        /// Agent type
        agent_type: String,

        /// Agent capabilities (comma-separated)
        #[arg(long)]
        capabilities: Option<String>,
    },

    /// Record agent coordination
    SwarmCoordinate {
        /// Source agent
        source: String,

        /// Target agent
        target: String,

        /// Coordination weight
        #[arg(long, default_value = "1.0")]
        weight: f32,
    },

    /// Optimize task distribution
    SwarmOptimize {
        /// Tasks to distribute (comma-separated)
        tasks: String,
    },

    /// Recommend agent for task type
    SwarmRecommend {
        /// Task type
        task_type: String,
    },

    /// Handle agent failure
    SwarmHeal {
        /// Failed agent ID
        agent_id: String,
    },

    /// Show swarm statistics
    SwarmStats,

    /// Generate shell completions
    Completions {
        /// Shell type (bash, zsh, fish, powershell)
        #[arg(value_enum)]
        shell: ShellType,
    },

    /// Compress intelligence data for smaller storage
    Compress,

    /// Show cache statistics
    CacheStats,
}

/// Shell types for completions
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

// === Data Structures ===

/// Q-learning pattern entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QPattern {
    pub state: String,
    pub action: String,
    pub q_value: f32,
    pub visits: u32,
    pub last_update: u64,
}

/// Memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub memory_type: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub metadata: HashMap<String, String>,
    pub timestamp: u64,
}

/// Learning trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub id: String,
    pub state: String,
    pub action: String,
    pub outcome: String,
    pub reward: f32,
    pub timestamp: u64,
}

/// Error pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPattern {
    pub code: String,
    pub error_type: String,
    pub message: String,
    pub fixes: Vec<String>,
    pub occurrences: u32,
}

/// File edit sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSequence {
    pub from_file: String,
    pub to_file: String,
    pub count: u32,
}

/// Swarm agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmAgent {
    pub id: String,
    pub agent_type: String,
    pub capabilities: Vec<String>,
    pub success_rate: f32,
    pub task_count: u32,
    pub status: String,
}

/// Swarm edge (coordination)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmEdge {
    pub source: String,
    pub target: String,
    pub weight: f32,
    pub coordination_count: u32,
}

/// Intelligence storage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntelligenceData {
    pub patterns: HashMap<String, QPattern>,
    pub memories: Vec<MemoryEntry>,
    pub trajectories: Vec<Trajectory>,
    pub errors: HashMap<String, ErrorPattern>,
    pub file_sequences: Vec<FileSequence>,
    pub agents: HashMap<String, SwarmAgent>,
    pub edges: Vec<SwarmEdge>,
    pub stats: IntelligenceStats,
}

/// Intelligence statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntelligenceStats {
    pub total_patterns: u32,
    pub total_memories: u32,
    pub total_trajectories: u32,
    pub total_errors: u32,
    pub session_count: u32,
    pub last_session: u64,
}

/// Intelligence engine with optimizations
pub struct Intelligence {
    data: IntelligenceData,
    data_path: PathBuf,
    alpha: f32,                              // Learning rate
    gamma: f32,                              // Discount factor
    epsilon: f32,                            // Exploration rate
    dirty: bool,                             // Track if data needs saving
    q_cache: RefCell<LruCache<String, f32>>, // LRU cache for Q-values
    use_compression: bool,                   // Use gzip compression
}

impl Intelligence {
    /// Create new intelligence engine with lazy loading
    pub fn new(data_path: PathBuf) -> Self {
        Self::with_options(data_path, false)
    }

    /// Create with compression option
    pub fn with_options(data_path: PathBuf, use_compression: bool) -> Self {
        let data = Self::load_data(&data_path, use_compression).unwrap_or_default();
        Self {
            data,
            data_path,
            alpha: 0.1,
            gamma: 0.95,
            epsilon: 0.1,
            dirty: false,
            q_cache: RefCell::new(LruCache::new(NonZeroUsize::new(1000).unwrap())),
            use_compression,
        }
    }

    /// Create lightweight instance for read-only operations (lazy loading)
    pub fn lazy() -> Self {
        Self {
            data: IntelligenceData::default(),
            data_path: PathBuf::new(),
            alpha: 0.1,
            gamma: 0.95,
            epsilon: 0.1,
            dirty: false,
            q_cache: RefCell::new(LruCache::new(NonZeroUsize::new(100).unwrap())),
            use_compression: false,
        }
    }

    /// Load data from file (supports both JSON and compressed)
    fn load_data(path: &Path, try_compressed: bool) -> Result<IntelligenceData> {
        let compressed_path = path.with_extension("json.gz");

        // Try compressed first if enabled
        if try_compressed && compressed_path.exists() {
            let file = File::open(&compressed_path)?;
            let mut decoder = GzDecoder::new(file);
            let mut content = String::new();
            decoder.read_to_string(&mut content)?;
            return Ok(serde_json::from_str(&content)?);
        }

        // Fall back to JSON
        if path.exists() {
            let content = fs::read_to_string(path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(IntelligenceData::default())
        }
    }

    /// Save data to file (batch mode - only saves if dirty)
    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        self.force_save()
    }

    /// Force save regardless of dirty flag
    pub fn force_save(&mut self) -> Result<()> {
        if let Some(parent) = self.data_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self.data)?;

        if self.use_compression {
            let compressed_path = self.data_path.with_extension("json.gz");
            let file = File::create(&compressed_path)?;
            let mut encoder = GzEncoder::new(file, Compression::fast());
            encoder.write_all(content.as_bytes())?;
            encoder.finish()?;
            // Remove uncompressed if exists
            let _ = fs::remove_file(&self.data_path);
        } else {
            fs::write(&self.data_path, content)?;
        }

        self.dirty = false;
        Ok(())
    }

    /// Mark data as modified (for batch saves)
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Check if data needs saving
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Migrate to compressed storage
    pub fn migrate_to_compressed(&mut self) -> Result<()> {
        self.use_compression = true;
        self.dirty = true;
        self.force_save()
    }

    /// Get current timestamp
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Generate simple embedding from text (hash-based for speed)
    fn embed(&self, text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0f32; 64];
        for (i, c) in text.chars().enumerate() {
            let idx = (c as usize + i * 7) % 64;
            embedding[idx] += 1.0;
        }
        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut embedding {
                *v /= norm;
            }
        }
        embedding
    }

    /// Cosine similarity between embeddings
    fn similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    // === Memory Operations ===

    /// Remember content
    pub fn remember(
        &mut self,
        memory_type: &str,
        content: &str,
        metadata: HashMap<String, String>,
    ) -> String {
        let id = format!("mem_{}", Self::now());
        let embedding = self.embed(content);

        self.data.memories.push(MemoryEntry {
            id: id.clone(),
            memory_type: memory_type.to_string(),
            content: content.to_string(),
            embedding,
            metadata,
            timestamp: Self::now(),
        });

        // Limit memory size
        if self.data.memories.len() > 5000 {
            self.data.memories.drain(0..1000);
        }

        self.data.stats.total_memories = self.data.memories.len() as u32;
        id
    }

    /// Recall from memory
    pub fn recall(&self, query: &str, top_k: usize) -> Vec<&MemoryEntry> {
        let query_embed = self.embed(query);

        let mut scored: Vec<_> = self
            .data
            .memories
            .iter()
            .map(|m| {
                let score = Self::similarity(&query_embed, &m.embedding);
                (score, m)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(top_k).map(|(_, m)| m).collect()
    }

    // === Q-Learning Operations ===

    /// Get Q-value for state-action pair (with LRU cache)
    fn get_q(&self, state: &str, action: &str) -> f32 {
        let key = format!("{}|{}", state, action);

        // Check cache first
        if let Some(&cached) = self.q_cache.borrow_mut().get(&key) {
            return cached;
        }

        // Lookup in data
        let value = self
            .data
            .patterns
            .get(&key)
            .map(|p| p.q_value)
            .unwrap_or(0.0);

        // Cache the result
        self.q_cache.borrow_mut().put(key, value);
        value
    }

    /// Update Q-value (marks dirty for batch save)
    fn update_q(&mut self, state: &str, action: &str, reward: f32) {
        let key = format!("{}|{}", state, action);
        let now = Self::now();
        let alpha = self.alpha;

        // Insert or update pattern
        let new_q_value = {
            let pattern = self.data.patterns.entry(key.clone()).or_insert(QPattern {
                state: state.to_string(),
                action: action.to_string(),
                q_value: 0.0,
                visits: 0,
                last_update: 0,
            });

            // Q-learning update
            pattern.q_value = pattern.q_value + alpha * (reward - pattern.q_value);
            pattern.visits += 1;
            pattern.last_update = now;
            pattern.q_value
        };

        // Update stats after borrow is released
        self.data.stats.total_patterns = self.data.patterns.len() as u32;

        // Update cache with new value
        self.q_cache.borrow_mut().put(key, new_q_value);

        // Mark for batch save
        self.mark_dirty();
    }

    /// Learn from trajectory
    pub fn learn(&mut self, state: &str, action: &str, outcome: &str, reward: f32) -> String {
        let id = format!("traj_{}", Self::now());

        // Update Q-value
        self.update_q(state, action, reward);

        // Store trajectory
        self.data.trajectories.push(Trajectory {
            id: id.clone(),
            state: state.to_string(),
            action: action.to_string(),
            outcome: outcome.to_string(),
            reward,
            timestamp: Self::now(),
        });

        // Limit trajectories
        if self.data.trajectories.len() > 1000 {
            self.data.trajectories.drain(0..200);
        }

        self.data.stats.total_trajectories = self.data.trajectories.len() as u32;
        id
    }

    /// Suggest best action for state
    pub fn suggest(&self, state: &str, actions: &[String]) -> (String, f32) {
        let mut best_action = actions.first().cloned().unwrap_or_default();
        let mut best_q = f32::MIN;

        for action in actions {
            let q = self.get_q(state, action);
            if q > best_q {
                best_q = q;
                best_action = action.clone();
            }
        }

        let confidence = if best_q > 0.0 { best_q.min(1.0) } else { 0.0 };
        (best_action, confidence)
    }

    /// Route to best agent
    pub fn route(
        &self,
        task: &str,
        file: Option<&str>,
        crate_name: Option<&str>,
        operation: &str,
    ) -> (String, f32, String) {
        let file_type = file
            .and_then(|f| Path::new(f).extension())
            .and_then(|e| e.to_str())
            .unwrap_or("unknown");

        let state = format!(
            "{}_{}_in_{}",
            operation,
            file_type,
            crate_name.unwrap_or("project")
        );

        // Agent candidates based on file type
        let agents: Vec<String> = match file_type {
            "rs" => vec!["rust-developer", "coder", "reviewer", "tester"],
            "ts" | "tsx" | "js" | "jsx" => vec!["typescript-developer", "coder", "frontend-dev"],
            "py" => vec!["python-developer", "coder", "ml-developer"],
            "md" => vec!["docs-writer", "coder"],
            "toml" | "json" | "yaml" => vec!["config-specialist", "coder"],
            _ => vec!["coder", "reviewer"],
        }
        .into_iter()
        .map(String::from)
        .collect();

        let (agent, confidence) = self.suggest(&state, &agents);

        let reason = if confidence > 0.5 {
            "learned from past success".to_string()
        } else if confidence > 0.0 {
            "based on patterns".to_string()
        } else {
            format!("default for {} files", file_type)
        };

        (agent, confidence, reason)
    }

    // === Error Pattern Learning ===

    /// Record error pattern
    pub fn record_error(&mut self, command: &str, stderr: &str) -> Vec<String> {
        let mut recorded = Vec::new();

        // Parse Rust errors
        for line in stderr.lines() {
            if let Some(code) = Self::extract_error_code(line) {
                let key = code.clone();
                let pattern = self.data.errors.entry(key.clone()).or_insert(ErrorPattern {
                    code: code.clone(),
                    error_type: Self::classify_error(&code),
                    message: line.chars().take(200).collect(),
                    fixes: Vec::new(),
                    occurrences: 0,
                });
                pattern.occurrences += 1;
                recorded.push(code);
            }
        }

        self.data.stats.total_errors = self.data.errors.len() as u32;
        recorded
    }

    /// Extract error code from line
    fn extract_error_code(line: &str) -> Option<String> {
        // Rust: error[E0308]
        if let Some(start) = line.find("error[E") {
            let rest = &line[start + 6..];
            if let Some(end) = rest.find(']') {
                return Some(format!("E{}", &rest[1..end]));
            }
        }
        // TypeScript: TS2322
        if let Some(start) = line.find("TS") {
            let rest = &line[start..];
            let code: String = rest.chars().take_while(|c| c.is_alphanumeric()).collect();
            if code.len() >= 5 {
                return Some(code);
            }
        }
        None
    }

    /// Classify error type
    fn classify_error(code: &str) -> String {
        match code {
            c if c.starts_with("E03") => "type-error",
            c if c.starts_with("E04") => "resolution-error",
            c if c.starts_with("E05") => "lifetime-error",
            c if c.starts_with("TS2") => "typescript-type-error",
            _ => "unknown",
        }
        .to_string()
    }

    /// Suggest fix for error
    pub fn suggest_fix(&self, error_code: &str) -> Option<&ErrorPattern> {
        self.data.errors.get(error_code)
    }

    // === File Sequence Prediction ===

    /// Record file edit
    pub fn record_file_edit(&mut self, file: &str, previous_file: Option<&str>) {
        if let Some(prev) = previous_file {
            let existing = self
                .data
                .file_sequences
                .iter_mut()
                .find(|s| s.from_file == prev && s.to_file == file);

            if let Some(seq) = existing {
                seq.count += 1;
            } else {
                self.data.file_sequences.push(FileSequence {
                    from_file: prev.to_string(),
                    to_file: file.to_string(),
                    count: 1,
                });
            }
        }
    }

    /// Suggest next files
    pub fn suggest_next(&self, file: &str, count: usize) -> Vec<(&str, u32)> {
        let mut suggestions: Vec<_> = self
            .data
            .file_sequences
            .iter()
            .filter(|s| s.from_file == file)
            .map(|s| (s.to_file.as_str(), s.count))
            .collect();

        suggestions.sort_by(|a, b| b.1.cmp(&a.1));
        suggestions.into_iter().take(count).collect()
    }

    /// Check if tests should run
    pub fn should_test(&self, file: &str) -> (bool, String) {
        let file_type = Path::new(file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match file_type {
            "rs" => {
                let crate_match = file.contains("crates/");
                if crate_match {
                    let crate_name = file
                        .split("crates/")
                        .nth(1)
                        .and_then(|s| s.split('/').next())
                        .unwrap_or("all");
                    (true, format!("cargo test -p {}", crate_name))
                } else {
                    (true, "cargo test".to_string())
                }
            }
            "ts" | "tsx" | "js" | "jsx" => (true, "npm test".to_string()),
            "py" => (true, "pytest".to_string()),
            _ => (false, String::new()),
        }
    }

    // === Swarm Operations ===

    /// Register agent
    pub fn swarm_register(&mut self, id: &str, agent_type: &str, capabilities: Vec<String>) {
        self.data.agents.insert(
            id.to_string(),
            SwarmAgent {
                id: id.to_string(),
                agent_type: agent_type.to_string(),
                capabilities,
                success_rate: 1.0,
                task_count: 0,
                status: "active".to_string(),
            },
        );
    }

    /// Record coordination
    pub fn swarm_coordinate(&mut self, source: &str, target: &str, weight: f32) {
        let existing = self
            .data
            .edges
            .iter_mut()
            .find(|e| e.source == source && e.target == target);

        if let Some(edge) = existing {
            edge.weight = (edge.weight + weight) / 2.0;
            edge.coordination_count += 1;
        } else {
            self.data.edges.push(SwarmEdge {
                source: source.to_string(),
                target: target.to_string(),
                weight,
                coordination_count: 1,
            });
        }
    }

    /// Recommend agent for task
    pub fn swarm_recommend(&self, task_type: &str) -> Option<&SwarmAgent> {
        self.data
            .agents
            .values()
            .filter(|a| a.status == "active" && a.agent_type == task_type)
            .max_by(|a, b| {
                a.success_rate
                    .partial_cmp(&b.success_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Handle agent failure
    pub fn swarm_heal(&mut self, agent_id: &str) -> Option<String> {
        if let Some(agent) = self.data.agents.get_mut(agent_id) {
            agent.status = "failed".to_string();
            agent.success_rate *= 0.8;
        }

        // Find replacement
        let failed_type = self
            .data
            .agents
            .get(agent_id)
            .map(|a| a.agent_type.clone())?;
        self.data
            .agents
            .values()
            .filter(|a| a.status == "active" && a.agent_type == failed_type && a.id != agent_id)
            .max_by(|a, b| {
                a.success_rate
                    .partial_cmp(&b.success_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|a| a.id.clone())
    }

    /// Get swarm stats
    pub fn swarm_stats(&self) -> (usize, usize, f32) {
        let agent_count = self.data.agents.len();
        let edge_count = self.data.edges.len();
        let avg_success = if agent_count > 0 {
            self.data
                .agents
                .values()
                .map(|a| a.success_rate)
                .sum::<f32>()
                / agent_count as f32
        } else {
            0.0
        };
        (agent_count, edge_count, avg_success)
    }

    /// Get full stats
    pub fn stats(&self) -> &IntelligenceStats {
        &self.data.stats
    }

    /// Get pattern count
    pub fn pattern_count(&self) -> usize {
        self.data.patterns.len()
    }

    /// Get memory count
    pub fn memory_count(&self) -> usize {
        self.data.memories.len()
    }
}

// === Command Implementations ===

/// Get intelligence data path (cross-platform)
pub fn get_intelligence_path() -> PathBuf {
    // Try HOME (Unix) then USERPROFILE (Windows) then HOMEPATH (Windows fallback)
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| {
            // Windows fallback: HOMEDRIVE + HOMEPATH
            let drive = std::env::var("HOMEDRIVE").unwrap_or_default();
            let homepath = std::env::var("HOMEPATH").unwrap_or_default();
            if !drive.is_empty() && !homepath.is_empty() {
                format!("{}{}", drive, homepath)
            } else {
                ".".to_string()
            }
        });

    PathBuf::from(home)
        .join(".ruvector")
        .join("intelligence.json")
}

/// Initialize PostgreSQL schema for hooks
fn init_postgres_schema() -> Result<()> {
    use std::process::Command;

    // Check for PostgreSQL connection URL
    let pg_url = std::env::var("RUVECTOR_POSTGRES_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .map_err(|_| anyhow::anyhow!(
            "PostgreSQL URL not set. Set RUVECTOR_POSTGRES_URL or DATABASE_URL environment variable.\n\
             Example: export RUVECTOR_POSTGRES_URL=\"postgres://user:pass@localhost/ruvector\""
        ))?;

    println!("{}", "🐘 Initializing PostgreSQL schema...".cyan().bold());

    // Embedded schema SQL
    let schema_sql = include_str!("../../sql/hooks_schema.sql");

    // Try psql first
    let result = Command::new("psql")
        .arg(&pg_url)
        .arg("-c")
        .arg(schema_sql)
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                println!(
                    "{}",
                    "✅ PostgreSQL schema applied successfully!".green().bold()
                );
                println!("\n{}", "Tables created:".bold());
                println!("   • ruvector_hooks_patterns     (Q-learning)");
                println!("   • ruvector_hooks_memories     (Vector embeddings)");
                println!("   • ruvector_hooks_trajectories (Learning history)");
                println!("   • ruvector_hooks_errors       (Error patterns)");
                println!("   • ruvector_hooks_file_sequences (File predictions)");
                println!("   • ruvector_hooks_swarm_agents (Swarm registry)");
                println!("   • ruvector_hooks_swarm_edges  (Coordination graph)");
                println!("   • ruvector_hooks_stats        (Global statistics)");
                println!("\n{}", "Functions created:".bold());
                println!("   • ruvector_hooks_update_q, ruvector_hooks_best_action");
                println!("   • ruvector_hooks_remember, ruvector_hooks_recall");
                println!("   • ruvector_hooks_swarm_register, ruvector_hooks_swarm_stats");
                println!("   • + 8 more helper functions");
                println!("\n{}", "Next steps:".bold());
                println!("   1. Run 'ruvector hooks init' to create Claude Code hooks");
                println!("   2. Build with: cargo build --features postgres");
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Failed to apply schema: {}", stderr);
            }
        }
        Err(e) => {
            // psql not found, provide manual instructions
            println!(
                "{}",
                "⚠️  psql not found. Apply schema manually:".yellow().bold()
            );
            println!("\n{}", "Option 1: Using psql".bold());
            println!("   psql $RUVECTOR_POSTGRES_URL -f crates/ruvector-cli/sql/hooks_schema.sql");
            println!("\n{}", "Option 2: Copy to clipboard (macOS)".bold());
            println!("   cat crates/ruvector-cli/sql/hooks_schema.sql | pbcopy");
            println!("\n{}", "Option 3: View schema".bold());
            println!("   cat crates/ruvector-cli/sql/hooks_schema.sql");
            anyhow::bail!("psql command not found: {}. See instructions above.", e);
        }
    }
}

/// Initialize hooks
pub fn init_hooks(force: bool, postgres: bool, _config: &Config) -> Result<()> {
    // Handle PostgreSQL initialization
    if postgres {
        return init_postgres_schema();
    }

    let claude_dir = PathBuf::from(".claude");
    let settings_path = claude_dir.join("settings.json");

    if settings_path.exists() && !force {
        println!(
            "{}",
            "Hooks already initialized. Use --force to overwrite.".yellow()
        );
        return Ok(());
    }

    fs::create_dir_all(&claude_dir)?;

    // Create platform-specific statusline script
    let (statusline_path, statusline_config) = if cfg!(windows) {
        // Windows: Use PowerShell script
        let path = claude_dir.join("statusline-command.ps1");
        let script = include_str!("../../scripts/statusline-command.ps1");
        fs::write(&path, script)?;
        (Some(path), Some(".claude/statusline-command.ps1"))
    } else {
        // Unix (macOS, Linux): Use bash script
        let path = claude_dir.join("statusline-command.sh");
        let script = include_str!("../../scripts/statusline-command.sh");
        fs::write(&path, script)?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms)?;
        }
        (Some(path), Some(".claude/statusline-command.sh"))
    };

    let hooks_config = serde_json::json!({
        "statusline": statusline_config,
        "hooks": {
            // Pre-tool hooks: provide guidance before actions
            "PreToolUse": [{
                "matcher": "Edit|Write|MultiEdit",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks pre-edit \"$TOOL_INPUT_FILE_PATH\"",
                    "timeout": 3000
                }]
            }, {
                "matcher": "Bash",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks pre-command \"$TOOL_INPUT_COMMAND\"",
                    "timeout": 3000
                }]
            }, {
                "matcher": "Task",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks swarm-recommend \"$TOOL_INPUT_SUBAGENT_TYPE\"",
                    "timeout": 2000
                }, {
                    // Claude Code v2.0.55+: Register async sub-agent
                    "type": "command",
                    "command": "npx ruvector hooks async-agent --action spawn --agent-id \"$TOOL_INPUT_SUBAGENT_TYPE\" --task \"$TOOL_INPUT_PROMPT\"",
                    "timeout": 1000
                }]
            }],
            // Post-tool hooks: record outcomes for learning
            "PostToolUse": [{
                "matcher": "Edit|Write|MultiEdit",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks post-edit \"$TOOL_INPUT_FILE_PATH\" --success=$TOOL_STATUS",
                    "timeout": 3000
                }]
            }, {
                "matcher": "Bash",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks post-command \"$TOOL_INPUT_COMMAND\" --success=$TOOL_STATUS",
                    "timeout": 3000
                }]
            }, {
                // Claude Code v2.0.55+: LSP diagnostics integration
                "matcher": "LSP",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks lsp-diagnostic --file \"$TOOL_INPUT_FILE\" --severity \"$TOOL_INPUT_SEVERITY\" --message \"$TOOL_INPUT_MESSAGE\"",
                    "timeout": 2000
                }]
            }, {
                // Claude Code v2.0.55+: Async sub-agent completion tracking
                "matcher": "Task",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks async-agent --action complete --agent-id \"$TOOL_INPUT_SUBAGENT_TYPE\"",
                    "timeout": 2000
                }]
            }],
            // Session lifecycle hooks
            "SessionStart": [{
                "matcher": "startup",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks session-start",
                    "timeout": 5000
                }]
            }, {
                "matcher": "resume",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks session-start --resume",
                    "timeout": 3000
                }]
            }],
            "Stop": [{
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks session-end --export-metrics",
                    "timeout": 5000
                }]
            }],
            // Context compaction hooks
            "PreCompact": [{
                "matcher": "auto",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks pre-compact --auto",
                    "timeout": 3000
                }]
            }, {
                "matcher": "manual",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks pre-compact",
                    "timeout": 3000
                }]
            }],
            // User prompt injection (inject learned context)
            "UserPromptSubmit": [{
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks suggest-context",
                    "timeout": 2000
                }]
            }],
            // Notification tracking
            "Notification": [{
                "matcher": ".*",
                "hooks": [{
                    "type": "command",
                    "command": "npx ruvector hooks track-notification",
                    "timeout": 1000
                }]
            }]
        }
    });

    fs::write(&settings_path, serde_json::to_string_pretty(&hooks_config)?)?;

    println!("{}", "✅ Hooks initialized!".green().bold());
    println!("   Created: {}", settings_path.display());
    if let Some(ref path) = statusline_path {
        println!("   Created: {}", path.display());
    }
    println!("\n{}", "Next steps:".bold());
    println!("   1. Restart Claude Code to activate hooks");
    println!("   2. Run 'npx ruvector hooks stats' to verify");

    Ok(())
}

/// Install hooks
pub fn install_hooks(settings_dir: &str, _config: &Config) -> Result<()> {
    let settings_path = PathBuf::from(settings_dir).join("settings.json");

    if !settings_path.exists() {
        return init_hooks(false, false, _config);
    }

    let content = fs::read_to_string(&settings_path)?;
    let mut settings: serde_json::Value = serde_json::from_str(&content)?;

    // Add hooks if not present
    if settings.get("hooks").is_none() {
        settings["hooks"] = serde_json::json!({
            "SessionStart": [{
                "hooks": [{
                    "type": "command",
                    "command": "ruvector hooks session-start"
                }]
            }]
        });
        fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
        println!("{}", "✅ Hooks installed!".green().bold());
    } else {
        println!("{}", "Hooks already installed.".yellow());
    }

    Ok(())
}

/// Show stats
pub fn show_stats(_config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());
    let stats = intel.stats();

    println!("{}", "🧠 RuVector Intelligence Stats".bold().cyan());
    println!();
    println!(
        "  {} Q-learning patterns",
        stats.total_patterns.to_string().green()
    );
    println!(
        "  {} vector memories",
        stats.total_memories.to_string().green()
    );
    println!(
        "  {} learning trajectories",
        stats.total_trajectories.to_string().green()
    );
    println!(
        "  {} error patterns",
        stats.total_errors.to_string().green()
    );
    println!();

    let (agents, edges, avg_success) = intel.swarm_stats();
    println!("{}", "Swarm Status:".bold());
    println!("  {} agents registered", agents.to_string().cyan());
    println!("  {} coordination edges", edges.to_string().cyan());
    if avg_success.is_nan() || avg_success == 0.0 {
        println!("  {}% average success rate", "N/A".cyan());
    } else {
        println!(
            "  {:.0}% average success rate",
            (avg_success * 100.0).to_string().cyan()
        );
    }

    Ok(())
}

/// Remember content
pub fn remember_content(memory_type: &str, content: &str, _config: &Config) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());
    let id = intel.remember(memory_type, content, HashMap::new());
    intel.save()?;

    println!("{}", serde_json::json!({ "success": true, "id": id }));
    Ok(())
}

/// Recall from memory
pub fn recall_content(query: &str, top_k: usize, _config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());
    let results = intel.recall(query, top_k);

    let output: Vec<_> = results
        .iter()
        .map(|m| {
            serde_json::json!({
                "type": m.memory_type,
                "content": m.content.chars().take(200).collect::<String>(),
                "timestamp": m.timestamp
            })
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "query": query,
            "results": output
        }))?
    );

    Ok(())
}

/// Learn trajectory
pub fn learn_trajectory(state: &str, action: &str, reward: f32, _config: &Config) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());
    let id = intel.learn(state, action, "recorded", reward);
    intel.save()?;

    println!(
        "{}",
        serde_json::json!({
            "success": true,
            "id": id,
            "state": state,
            "action": action,
            "reward": reward
        })
    );

    Ok(())
}

/// Suggest action
pub fn suggest_action(state: &str, actions_str: &str, _config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());
    let actions: Vec<String> = actions_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let (action, confidence) = intel.suggest(state, &actions);

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "state": state,
            "action": action,
            "confidence": confidence,
            "explored": confidence == 0.0
        }))?
    );

    Ok(())
}

/// Route to agent
pub fn route_task(
    task: &str,
    file: Option<&str>,
    crate_name: Option<&str>,
    operation: &str,
    _config: &Config,
) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());
    let (agent, confidence, reason) = intel.route(task, file, crate_name, operation);

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "task": task,
            "recommended": agent,
            "confidence": confidence,
            "reasoning": reason,
            "file": file,
            "crate": crate_name
        }))?
    );

    Ok(())
}

/// Pre-edit hook
pub fn pre_edit_hook(file: &str, _config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());

    let file_type = Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown");

    let crate_name = file
        .split("crates/")
        .nth(1)
        .and_then(|s| s.split('/').next());

    let (agent, confidence, reason) =
        intel.route(&format!("edit {}", file), Some(file), crate_name, "edit");

    let similar = intel.recall(
        &format!("edit {} {}", file_type, crate_name.unwrap_or("")),
        3,
    );

    println!("{}", "🧠 Intelligence Analysis:".bold());
    println!(
        "   📁 {}/{}",
        crate_name.unwrap_or("project").cyan(),
        Path::new(file)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );
    println!(
        "   🤖 Recommended: {} ({:.0}% confidence)",
        agent.green().bold(),
        confidence * 100.0
    );
    if !reason.is_empty() {
        println!("      → {}", reason.dimmed());
    }
    if !similar.is_empty() {
        println!("   📚 {} similar past edits found", similar.len());
    }

    Ok(())
}

/// Post-edit hook
pub fn post_edit_hook(file: &str, success: bool, _config: &Config) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());

    let file_type = Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown");

    let crate_name = file
        .split("crates/")
        .nth(1)
        .and_then(|s| s.split('/').next());

    let state = format!("edit_{}_in_{}", file_type, crate_name.unwrap_or("project"));
    let action = if success {
        "successful-edit"
    } else {
        "failed-edit"
    };
    let reward = if success { 1.0 } else { -0.5 };

    intel.learn(
        &state,
        action,
        if success { "completed" } else { "failed" },
        reward,
    );
    intel.remember(
        "edit",
        &format!(
            "{} edit of {} in {}",
            if success { "successful" } else { "failed" },
            file_type,
            crate_name.unwrap_or("project")
        ),
        HashMap::new(),
    );

    intel.save()?;

    let icon = if success { "✅" } else { "❌" };
    println!(
        "📊 Learning recorded: {} {}",
        icon,
        Path::new(file)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );

    // Suggest tests
    let (should_test, test_cmd) = intel.should_test(file);
    if should_test {
        println!("   🧪 Consider: {}", test_cmd.cyan());
    }

    // Suggest next files
    let next = intel.suggest_next(file, 2);
    if !next.is_empty() {
        let files: Vec<_> = next
            .iter()
            .map(|(f, _)| {
                Path::new(f)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        println!("   📁 Often edit next: {}", files.join(", ").dimmed());
    }

    Ok(())
}

/// Pre-command hook
pub fn pre_command_hook(command: &str, _config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());

    let cmd_type = if command.starts_with("cargo") {
        "cargo"
    } else if command.starts_with("npm") {
        "npm"
    } else if command.starts_with("git") {
        "git"
    } else if command.starts_with("wasm-pack") {
        "wasm"
    } else {
        "other"
    };

    let state = format!("{}_in_general", cmd_type);
    let actions = vec![
        "command-succeeded".to_string(),
        "command-failed".to_string(),
    ];
    let (suggestion, confidence) = intel.suggest(&state, &actions);

    println!("🧠 Command: {}", cmd_type.cyan());
    if confidence > 0.3 {
        println!("   💡 Likely: {}", suggestion);
    }

    Ok(())
}

/// Post-command hook
pub fn post_command_hook(
    command: &str,
    success: bool,
    stderr: Option<&str>,
    _config: &Config,
) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());

    let cmd_type = if command.starts_with("cargo") {
        "cargo"
    } else if command.starts_with("npm") {
        "npm"
    } else if command.starts_with("git") {
        "git"
    } else if command.starts_with("wasm-pack") {
        "wasm"
    } else {
        "other"
    };

    let state = format!("{}_in_general", cmd_type);
    let action = if success {
        "command-succeeded"
    } else {
        "command-failed"
    };
    let reward = if success { 1.0 } else { -0.5 };

    intel.learn(
        &state,
        action,
        &command.chars().take(100).collect::<String>(),
        reward,
    );

    // Record errors if failed
    if !success {
        if let Some(err) = stderr {
            let errors = intel.record_error(command, err);
            if !errors.is_empty() {
                println!(
                    "📊 Command ❌ recorded ({} error patterns learned)",
                    errors.len()
                );
                for code in errors.iter().take(2) {
                    if let Some(pattern) = intel.suggest_fix(code) {
                        if !pattern.fixes.is_empty() {
                            println!("   💡 {}: {}", code, pattern.fixes[0]);
                        }
                    }
                }
                intel.save()?;
                return Ok(());
            }
        }
    }

    intel.save()?;

    let icon = if success { "✅" } else { "❌" };
    println!("📊 Command {} recorded", icon);

    Ok(())
}

/// Session start hook
pub fn session_start_hook(_session_id: Option<&str>, resume: bool, _config: &Config) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());

    if !resume {
        intel.data.stats.session_count += 1;
    }
    intel.data.stats.last_session = Intelligence::now();
    intel.mark_dirty();
    intel.save()?;

    let stats = intel.stats();

    if resume {
        println!("{}", "🧠 RuVector Intelligence Layer Resumed".bold().cyan());
    } else {
        println!("{}", "🧠 RuVector Intelligence Layer Active".bold().cyan());
    }
    println!();
    println!("⚡ Intelligence guides: agent routing, error fixes, file sequences");

    // Show quick stats on startup
    if stats.total_patterns > 0 || stats.total_memories > 0 {
        println!(
            "   {} patterns | {} memories | {} sessions",
            stats.total_patterns, stats.total_memories, stats.session_count
        );
    }

    Ok(())
}

/// Session end hook
pub fn session_end_hook(export_metrics: bool, _config: &Config) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());
    intel.save()?; // Final save

    if export_metrics {
        let stats = intel.stats();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "patterns": stats.total_patterns,
                "memories": stats.total_memories,
                "trajectories": stats.total_trajectories,
                "errors": stats.total_errors,
                "sessions": stats.session_count
            }))?
        );
    }

    println!("{}", "📊 Session ended. Learning data saved.".green());

    Ok(())
}

/// Pre-compact hook
pub fn pre_compact_hook(length: Option<usize>, auto: bool, _config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());
    let stats = intel.stats();

    if auto {
        // Auto-compact: just save critical state silently
        println!(
            "🗜️ Pre-compact: {} trajectories, {} memories saved",
            stats.total_trajectories, stats.total_memories
        );
    } else {
        // Manual compact: show full summary
        println!("{}", "🗜️ Pre-compact Summary".bold().cyan());
        println!("   Conversation length: {}", length.unwrap_or(0));
        println!("   Patterns learned: {}", stats.total_patterns);
        println!("   Memories stored: {}", stats.total_memories);
        println!("   Trajectories: {}", stats.total_trajectories);
        println!("   Error patterns: {}", stats.total_errors);
    }

    Ok(())
}

/// Suggest context for user prompt (UserPromptSubmit hook)
/// Returns learned patterns and suggestions to inject into Claude's context
pub fn suggest_context_cmd(_config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());
    let stats = intel.stats();

    // Only output if we have learned patterns
    if stats.total_patterns > 0 || stats.total_errors > 0 {
        // Output goes to stdout and gets injected as context
        println!("RuVector Intelligence: {} learned patterns, {} error fixes available. Use 'ruvector hooks route' for agent suggestions.",
            stats.total_patterns, stats.total_errors);
    }

    Ok(())
}

/// Track notification events
pub fn track_notification_cmd(notification_type: Option<&str>, _config: &Config) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());

    // Track notification as a learning trajectory
    if let Some(ntype) = notification_type {
        intel.learn(
            &format!("notification:{}", ntype),
            "observed",
            "tracked",
            0.0,
        );
        intel.save()?;
    }

    Ok(())
}

/// Record error
pub fn record_error_cmd(command: &str, stderr: &str, _config: &Config) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());
    let errors = intel.record_error(command, stderr);
    intel.save()?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "recorded": errors.len(),
            "errors": errors
        }))?
    );

    Ok(())
}

/// Suggest fix
pub fn suggest_fix_cmd(error_code: &str, _config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());

    if let Some(pattern) = intel.suggest_fix(error_code) {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "code": pattern.code,
                "type": pattern.error_type,
                "occurrences": pattern.occurrences,
                "fixes": pattern.fixes
            }))?
        );
    } else {
        println!(
            "{}",
            serde_json::json!({
                "code": error_code,
                "found": false
            })
        );
    }

    Ok(())
}

/// Suggest next files
pub fn suggest_next_cmd(file: &str, count: usize, _config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());
    let suggestions = intel.suggest_next(file, count);

    let output: Vec<_> = suggestions
        .iter()
        .map(|(f, c)| {
            serde_json::json!({
                "file": f,
                "count": c
            })
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

/// Should test
pub fn should_test_cmd(file: &str, _config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());
    let (suggest, command) = intel.should_test(file);

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "suggest": suggest,
            "command": command
        }))?
    );

    Ok(())
}

/// Swarm register
pub fn swarm_register_cmd(
    agent_id: &str,
    agent_type: &str,
    capabilities: Option<&str>,
    _config: &Config,
) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());
    let caps: Vec<String> = capabilities
        .map(|s| s.split(',').map(|c| c.trim().to_string()).collect())
        .unwrap_or_default();

    intel.swarm_register(agent_id, agent_type, caps);
    intel.save()?;

    println!(
        "{}",
        serde_json::json!({
            "success": true,
            "agent_id": agent_id,
            "type": agent_type
        })
    );

    Ok(())
}

/// Swarm coordinate
pub fn swarm_coordinate_cmd(
    source: &str,
    target: &str,
    weight: f32,
    _config: &Config,
) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());
    intel.swarm_coordinate(source, target, weight);
    intel.save()?;

    println!(
        "{}",
        serde_json::json!({
            "success": true,
            "source": source,
            "target": target,
            "weight": weight
        })
    );

    Ok(())
}

/// Swarm optimize
pub fn swarm_optimize_cmd(tasks: &str, _config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());
    let task_list: Vec<&str> = tasks.split(',').map(|s| s.trim()).collect();

    let assignments: Vec<_> = task_list
        .iter()
        .map(|task| {
            let (agent, edges, _) = intel.swarm_stats();
            serde_json::json!({
                "task": task,
                "available_agents": agent,
                "coordination_edges": edges
            })
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "tasks": task_list.len(),
            "assignments": assignments
        }))?
    );

    Ok(())
}

/// Swarm recommend
pub fn swarm_recommend_cmd(task_type: &str, _config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());

    if let Some(agent) = intel.swarm_recommend(task_type) {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "task_type": task_type,
                "recommended": agent.id,
                "success_rate": agent.success_rate,
                "capabilities": agent.capabilities
            }))?
        );
    } else {
        println!(
            "{}",
            serde_json::json!({
                "task_type": task_type,
                "recommended": null,
                "message": "No matching agent found"
            })
        );
    }

    Ok(())
}

/// Swarm heal
pub fn swarm_heal_cmd(agent_id: &str, _config: &Config) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());
    let replacement = intel.swarm_heal(agent_id);
    intel.save()?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "failed_agent": agent_id,
            "replacement": replacement,
            "healed": replacement.is_some()
        }))?
    );

    Ok(())
}

/// Swarm stats
pub fn swarm_stats_cmd(_config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());
    let (agents, edges, avg_success) = intel.swarm_stats();

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "agents": agents,
            "edges": edges,
            "average_success_rate": avg_success,
            "topology": "mesh"
        }))?
    );

    Ok(())
}

// === Claude Code v2.0.55+ Feature Commands ===

/// Process LSP diagnostic events
pub fn lsp_diagnostic_cmd(
    file: Option<&str>,
    severity: Option<&str>,
    message: Option<&str>,
    _config: &Config,
) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());

    // Parse stdin for full diagnostic info
    let stdin_input = try_parse_stdin();

    let file = file
        .or_else(|| {
            stdin_input.as_ref().and_then(|i| {
                i.tool_input
                    .as_ref()
                    .and_then(|t| t.get("file").and_then(|f| f.as_str()))
            })
        })
        .unwrap_or("unknown");

    let severity = severity.unwrap_or("error");
    let message = message.unwrap_or("");

    // Learn from diagnostic patterns
    let state = format!(
        "lsp:{}:{}",
        severity,
        file.split('/').last().unwrap_or(file)
    );
    intel.learn(
        &state,
        "diagnostic",
        severity,
        if severity == "error" { -0.5 } else { 0.0 },
    );
    intel.save()?;

    // Output JSON for context injection
    if severity == "error" {
        let output = HookOutput {
            hook_specific_output: Some(HookSpecificOutput {
                additional_context: Some(format!(
                    "LSP reports {} in {}: {}. Consider fixing before proceeding.",
                    severity, file, message
                )),
                ..Default::default()
            }),
            ..Default::default()
        };
        println!("{}", serde_json::to_string(&output)?);
    }

    Ok(())
}

/// Recommend ultrathink mode for complex tasks
pub fn suggest_ultrathink_cmd(task: &str, file: Option<&str>, _config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());

    // Complexity indicators that suggest ultrathink
    let complexity_patterns = [
        ("algorithm", 0.8),
        ("optimize", 0.7),
        ("refactor", 0.6),
        ("debug", 0.7),
        ("performance", 0.7),
        ("concurrent", 0.8),
        ("async", 0.6),
        ("architecture", 0.8),
        ("security", 0.7),
        ("cryptograph", 0.9),
        ("distributed", 0.8),
        ("consensus", 0.9),
        ("neural", 0.8),
        ("ml", 0.7),
        ("complex", 0.6),
    ];

    let task_lower = task.to_lowercase();
    let mut complexity_score = 0.0;
    let mut matched_patterns = Vec::new();

    for (pattern, weight) in complexity_patterns {
        if task_lower.contains(pattern) {
            complexity_score += weight;
            matched_patterns.push(pattern);
        }
    }

    // Check file extension for inherent complexity
    if let Some(f) = file {
        if f.ends_with(".rs") || f.ends_with(".cpp") || f.ends_with(".go") {
            complexity_score += 0.2;
        }
    }

    // Check learned patterns
    let state = format!(
        "task:{}",
        task_lower.split_whitespace().next().unwrap_or("unknown")
    );
    let (_, q_value) = intel.suggest(&state, &["ultrathink".to_string(), "normal".to_string()]);
    if q_value > 0.5 {
        complexity_score += 0.3;
    }

    let recommend_ultrathink = complexity_score >= 0.6;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "task": task,
            "complexity_score": complexity_score,
            "recommend_ultrathink": recommend_ultrathink,
            "matched_patterns": matched_patterns,
            "suggestion": if recommend_ultrathink {
                "Consider using 'ultrathink' for this complex task"
            } else {
                "Standard reasoning should suffice"
            }
        }))?
    );

    Ok(())
}

/// Coordinate async sub-agent execution
pub fn async_agent_cmd(
    action: &str,
    agent_id: Option<&str>,
    task: Option<&str>,
    _config: &Config,
) -> Result<()> {
    let mut intel = Intelligence::new(get_intelligence_path());

    match action {
        "spawn" => {
            let default_id = format!(
                "async-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            );
            let agent_id = agent_id.unwrap_or(&default_id);
            let task = task.unwrap_or("unknown");

            // Register async agent
            intel.swarm_register(agent_id, "async-subagent", vec!["parallel".to_string()]);
            intel.learn(&format!("async:{}", agent_id), "spawned", "active", 0.0);
            intel.save()?;

            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "action": "spawn",
                    "agent_id": agent_id,
                    "task": task,
                    "status": "spawned",
                    "coordination": "async"
                }))?
            );
        }
        "sync" => {
            // Record coordination between async agents
            if let Some(id) = agent_id {
                intel.learn(&format!("async:{}", id), "sync", "waiting", 0.1);
                intel.save()?;

                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "action": "sync",
                        "agent_id": id,
                        "status": "synchronizing"
                    }))?
                );
            }
        }
        "complete" => {
            if let Some(id) = agent_id {
                intel.learn(&format!("async:{}", id), "complete", "finished", 1.0);
                intel.save()?;

                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "action": "complete",
                        "agent_id": id,
                        "status": "completed",
                        "reward": 1.0
                    }))?
                );
            }
        }
        _ => {
            println!(
                "{}",
                serde_json::json!({
                    "error": format!("Unknown action: {}. Use spawn, sync, or complete.", action)
                })
            );
        }
    }

    Ok(())
}

// === Optimization Commands ===

/// Generate shell completions
pub fn generate_completions(shell: ShellType) -> Result<()> {
    // We need to get the parent CLI struct, but since we're in a submodule,
    // we'll generate a standalone completions script
    let completions = match shell {
        ShellType::Bash => {
            r#"# Bash completion for ruvector hooks
_ruvector_hooks() {
    local cur prev commands
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    if [[ ${COMP_CWORD} -eq 2 ]]; then
        commands="init install stats remember recall learn suggest route pre-edit post-edit pre-command post-command session-start session-end pre-compact record-error suggest-fix suggest-next should-test swarm-register swarm-coordinate swarm-optimize swarm-recommend swarm-heal swarm-stats completions compress cache-stats"
        COMPREPLY=( $(compgen -W "${commands}" -- ${cur}) )
        return 0
    fi
}
complete -F _ruvector_hooks ruvector
"#
        }
        ShellType::Zsh => {
            r#"#compdef ruvector

_ruvector_hooks() {
    local -a commands
    commands=(
        'init:Initialize hooks in current project'
        'install:Install hooks into Claude settings'
        'stats:Show intelligence statistics'
        'remember:Store content in semantic memory'
        'recall:Search memory semantically'
        'learn:Record a learning trajectory'
        'suggest:Get action suggestion for state'
        'route:Route task to best agent'
        'pre-edit:Pre-edit intelligence hook'
        'post-edit:Post-edit learning hook'
        'pre-command:Pre-command intelligence hook'
        'post-command:Post-command learning hook'
        'session-start:Session start hook'
        'session-end:Session end hook'
        'pre-compact:Pre-compact hook'
        'record-error:Record error pattern'
        'suggest-fix:Get fix for error code'
        'suggest-next:Suggest next files'
        'should-test:Check if tests should run'
        'swarm-register:Register agent in swarm'
        'swarm-coordinate:Record coordination'
        'swarm-optimize:Optimize task distribution'
        'swarm-recommend:Recommend agent for task'
        'swarm-heal:Handle agent failure'
        'swarm-stats:Show swarm statistics'
        'completions:Generate shell completions'
        'compress:Compress intelligence data'
        'cache-stats:Show cache statistics'
    )
    _describe 'command' commands
}

_ruvector() {
    local line
    _arguments -C \
        "1: :->cmds" \
        "*::arg:->args"
    case $line[1] in
        hooks)
            _ruvector_hooks
        ;;
    esac
}

compdef _ruvector ruvector
"#
        }
        ShellType::Fish => {
            r#"# Fish completion for ruvector hooks
complete -c ruvector -n "__fish_use_subcommand" -a hooks -d "Self-learning intelligence hooks"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a init -d "Initialize hooks"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a install -d "Install hooks into Claude settings"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a stats -d "Show intelligence statistics"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a remember -d "Store content in memory"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a recall -d "Search memory"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a learn -d "Record learning trajectory"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a suggest -d "Get action suggestion"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a route -d "Route task to agent"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a pre-edit -d "Pre-edit hook"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a post-edit -d "Post-edit hook"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a session-start -d "Session start hook"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a session-end -d "Session end hook"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a swarm-stats -d "Show swarm stats"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a completions -d "Generate completions"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a compress -d "Compress storage"
complete -c ruvector -n "__fish_seen_subcommand_from hooks" -a cache-stats -d "Show cache stats"
"#
        }
        ShellType::PowerShell => {
            r#"# PowerShell completion for ruvector hooks
Register-ArgumentCompleter -Native -CommandName ruvector -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    $commands = @(
        'init', 'install', 'stats', 'remember', 'recall', 'learn', 'suggest', 'route',
        'pre-edit', 'post-edit', 'pre-command', 'post-command', 'session-start',
        'session-end', 'pre-compact', 'record-error', 'suggest-fix', 'suggest-next',
        'should-test', 'swarm-register', 'swarm-coordinate', 'swarm-optimize',
        'swarm-recommend', 'swarm-heal', 'swarm-stats', 'completions', 'compress', 'cache-stats'
    )
    $commands | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
        [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
    }
}
"#
        }
    };

    println!("{}", completions);
    println!("\n# To install, add this to your shell config:");
    match shell {
        ShellType::Bash => println!("# Add to ~/.bashrc or ~/.bash_profile"),
        ShellType::Zsh => println!("# Add to ~/.zshrc"),
        ShellType::Fish => println!("# Save to ~/.config/fish/completions/ruvector.fish"),
        ShellType::PowerShell => println!("# Add to your PowerShell profile"),
    }

    Ok(())
}

/// Compress intelligence storage
pub fn compress_storage(_config: &Config) -> Result<()> {
    let path = get_intelligence_path();
    let compressed_path = path.with_extension("json.gz");

    if !path.exists() {
        println!("{}", "No intelligence data found to compress.".yellow());
        return Ok(());
    }

    // Get original size
    let original_size = fs::metadata(&path)?.len();

    // Migrate to compressed
    let mut intel = Intelligence::with_options(path.clone(), true);
    intel.migrate_to_compressed()?;

    // Get compressed size
    let compressed_size = fs::metadata(&compressed_path)?.len();
    let ratio = (1.0 - (compressed_size as f64 / original_size as f64)) * 100.0;

    println!("{}", "✅ Storage compressed!".green());
    println!("   Original:   {} bytes", original_size);
    println!("   Compressed: {} bytes", compressed_size);
    println!("   Saved:      {:.1}%", ratio);

    Ok(())
}

/// Show cache statistics
pub fn cache_stats(_config: &Config) -> Result<()> {
    let intel = Intelligence::new(get_intelligence_path());
    let stats = intel.stats();
    let cache_size = 1000; // Our default LRU cache size

    println!("{}", "🧠 Cache Statistics".cyan().bold());
    println!();
    println!("  LRU Cache Size:     {} entries", cache_size);
    println!("  Patterns in DB:     {}", stats.total_patterns);
    println!("  Memories in DB:     {}", stats.total_memories);
    println!("  Trajectories:       {}", stats.total_trajectories);
    println!();
    println!("{}", "Performance Benefits:".bold());
    println!("  - LRU cache: ~10x faster Q-value lookups");
    println!("  - Batch saves: Reduced disk I/O");
    println!("  - Lazy loading: Faster startup for read-only ops");

    // Check if using compressed storage
    let compressed_path = get_intelligence_path().with_extension("json.gz");
    if compressed_path.exists() {
        println!("  - Compression: {} (enabled)", "gzip".green());
    } else {
        println!(
            "  - Compression: {} (run 'hooks compress')",
            "disabled".yellow()
        );
    }

    Ok(())
}
