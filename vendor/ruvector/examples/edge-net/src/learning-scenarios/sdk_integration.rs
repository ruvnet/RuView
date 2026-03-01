//! Claude Agent SDK Integration for RuVector
//!
//! Provides patterns and utilities for integrating RuVector's self-learning
//! intelligence with the Claude Agent SDK.

use std::collections::HashMap;

/// Permission modes matching Claude Code's permission system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    /// Default mode - requires approval for most operations
    Default,
    /// Accept edits mode - auto-approves file edits
    AcceptEdits,
    /// Bypass permissions - runs without prompts (CI/CD)
    BypassPermissions,
    /// Plan mode - safe analysis without execution
    Plan,
}

impl PermissionMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "acceptedits" | "accept-edits" | "accept_edits" => Self::AcceptEdits,
            "bypasspermissions" | "bypass-permissions" | "bypass" => Self::BypassPermissions,
            "plan" => Self::Plan,
            _ => Self::Default,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::BypassPermissions => "bypassPermissions",
            Self::Plan => "plan",
        }
    }

    /// Check if this mode allows a specific operation
    pub fn allows(&self, operation: &str) -> bool {
        match self {
            Self::BypassPermissions => true,
            Self::AcceptEdits => matches!(operation, "read" | "edit" | "write" | "glob" | "grep"),
            Self::Plan => matches!(operation, "read" | "glob" | "grep"),
            Self::Default => false, // Requires explicit approval
        }
    }
}

/// Telemetry configuration matching Claude Code's telemetry options
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Disable Statsig metrics collection
    pub disable_telemetry: bool,
    /// Disable Sentry error reporting
    pub disable_error_reporting: bool,
    /// Disable /bug command
    pub disable_bug_command: bool,
    /// Disable all non-essential network traffic
    pub disable_nonessential_traffic: bool,
    /// Custom telemetry endpoint
    pub custom_endpoint: Option<String>,
    /// Data retention days (consumer: 5 years or 30 days, commercial: 30 days)
    pub retention_days: u32,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            disable_telemetry: false,
            disable_error_reporting: false,
            disable_bug_command: false,
            disable_nonessential_traffic: false,
            custom_endpoint: None,
            retention_days: 30,
        }
    }
}

impl TelemetryConfig {
    /// Create config from environment variables
    pub fn from_env() -> Self {
        Self {
            disable_telemetry: std::env::var("DISABLE_TELEMETRY").is_ok(),
            disable_error_reporting: std::env::var("DISABLE_ERROR_REPORTING").is_ok(),
            disable_bug_command: std::env::var("DISABLE_BUG_COMMAND").is_ok(),
            disable_nonessential_traffic: std::env::var("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC").is_ok(),
            custom_endpoint: std::env::var("RUVECTOR_TELEMETRY_ENDPOINT").ok(),
            retention_days: std::env::var("RUVECTOR_RETENTION_DAYS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
        }
    }

    /// Check if telemetry is enabled
    pub fn is_enabled(&self) -> bool {
        !self.disable_telemetry && !self.disable_nonessential_traffic
    }

    /// Export as environment variables
    pub fn to_env_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        if self.disable_telemetry {
            vars.insert("DISABLE_TELEMETRY".into(), "1".into());
        }
        if self.disable_error_reporting {
            vars.insert("DISABLE_ERROR_REPORTING".into(), "1".into());
        }
        if self.disable_bug_command {
            vars.insert("DISABLE_BUG_COMMAND".into(), "1".into());
        }
        if self.disable_nonessential_traffic {
            vars.insert("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".into(), "1".into());
        }
        if let Some(endpoint) = &self.custom_endpoint {
            vars.insert("RUVECTOR_TELEMETRY_ENDPOINT".into(), endpoint.clone());
        }
        vars.insert("RUVECTOR_RETENTION_DAYS".into(), self.retention_days.to_string());
        vars
    }
}

/// Agent SDK query options
#[derive(Debug, Clone)]
pub struct QueryOptions {
    /// Allowed tools for this query
    pub allowed_tools: Vec<String>,
    /// Permission mode
    pub permission_mode: PermissionMode,
    /// System prompt override
    pub system_prompt: Option<String>,
    /// Model to use (sonnet, opus, haiku)
    pub model: String,
    /// Session ID to resume
    pub resume_session: Option<String>,
    /// Maximum agentic turns
    pub max_turns: Option<u32>,
    /// Output format (text, json, stream-json)
    pub output_format: String,
    /// Custom agents/subagents
    pub agents: HashMap<String, AgentDefinition>,
    /// MCP servers to enable
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            allowed_tools: vec![
                "Read".into(),
                "Edit".into(),
                "Write".into(),
                "Bash".into(),
                "Glob".into(),
                "Grep".into(),
            ],
            permission_mode: PermissionMode::Default,
            system_prompt: None,
            model: "claude-sonnet-4-5-20250929".into(),
            resume_session: None,
            max_turns: None,
            output_format: "text".into(),
            agents: HashMap::new(),
            mcp_servers: HashMap::new(),
        }
    }
}

/// Agent definition for custom subagents
#[derive(Debug, Clone)]
pub struct AgentDefinition {
    pub description: String,
    pub prompt: String,
    pub tools: Vec<String>,
}

/// MCP server configuration
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

/// Hook event types matching Claude Code's hook system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEventType {
    /// Before a tool is executed
    PreToolUse,
    /// After a tool execution completes
    PostToolUse,
    /// When a notification is received
    Notification,
    /// Before context compaction
    PreCompact,
    /// When a session starts
    SessionStart,
    /// When execution stops
    Stop,
    /// When user submits a prompt
    UserPromptSubmit,
}

impl HookEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::Notification => "Notification",
            Self::PreCompact => "PreCompact",
            Self::SessionStart => "SessionStart",
            Self::Stop => "Stop",
            Self::UserPromptSubmit => "UserPromptSubmit",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "PreToolUse" => Some(Self::PreToolUse),
            "PostToolUse" => Some(Self::PostToolUse),
            "Notification" => Some(Self::Notification),
            "PreCompact" => Some(Self::PreCompact),
            "SessionStart" => Some(Self::SessionStart),
            "Stop" => Some(Self::Stop),
            "UserPromptSubmit" => Some(Self::UserPromptSubmit),
            _ => None,
        }
    }
}

/// Hook matcher configuration
#[derive(Debug, Clone)]
pub struct HookMatcher {
    pub event_type: HookEventType,
    pub matcher: String, // Regex pattern for tool matching
    pub command: String,
    pub timeout_ms: u32,
}

impl HookMatcher {
    pub fn new(event_type: HookEventType, matcher: &str, command: &str) -> Self {
        Self {
            event_type,
            matcher: matcher.into(),
            command: command.into(),
            timeout_ms: 5000,
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u32) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

/// Generate Claude Code settings JSON for RuVector integration
pub fn generate_settings_json(telemetry: &TelemetryConfig) -> String {
    let env_vars = telemetry.to_env_vars();
    let env_json: Vec<String> = env_vars
        .iter()
        .map(|(k, v)| format!("    \"{}\": \"{}\"", k, v))
        .collect();

    format!(
        r#"{{
  "env": {{
    "RUVECTOR_INTELLIGENCE_ENABLED": "true",
    "RUVECTOR_LEARNING_RATE": "0.1",
    "RUVECTOR_MEMORY_BACKEND": "rvlite",
    "INTELLIGENCE_MODE": "treatment",
{}
  }},
  "permissions": {{
    "allow": [
      "Bash(ruvector:*)",
      "Bash(ruvector-cli:*)",
      "Bash(npx ruvector:*)",
      "Bash(cargo test:*)",
      "Bash(git:*)"
    ],
    "deny": [
      "Bash(rm -rf /)"
    ]
  }},
  "hooks": {{
    "PreToolUse": [
      {{
        "matcher": "Edit|Write|MultiEdit",
        "hooks": [{{
          "type": "command",
          "command": "ruvector-cli hooks pre-edit \"$TOOL_INPUT_file_path\""
        }}]
      }},
      {{
        "matcher": "Bash",
        "hooks": [{{
          "type": "command",
          "command": "ruvector-cli hooks pre-command \"$TOOL_INPUT_command\""
        }}]
      }},
      {{
        "matcher": "Task",
        "hooks": [{{
          "type": "command",
          "timeout": 1000,
          "command": "ruvector-cli hooks remember \"Agent: $TOOL_INPUT_subagent_type\" -t agent_spawn"
        }}]
      }}
    ],
    "PostToolUse": [
      {{
        "matcher": "Edit|Write|MultiEdit",
        "hooks": [{{
          "type": "command",
          "command": "ruvector-cli hooks post-edit \"$TOOL_INPUT_file_path\" --success"
        }}]
      }},
      {{
        "matcher": "Bash",
        "hooks": [{{
          "type": "command",
          "command": "ruvector-cli hooks post-command \"$TOOL_INPUT_command\" --success"
        }}]
      }}
    ],
    "SessionStart": [{{
      "hooks": [{{
        "type": "command",
        "command": "ruvector-cli hooks session-start"
      }}]
    }}],
    "Stop": [{{
      "hooks": [{{
        "type": "command",
        "command": "ruvector-cli hooks session-end"
      }}]
    }}],
    "UserPromptSubmit": [{{
      "hooks": [{{
        "type": "command",
        "timeout": 2000,
        "command": "ruvector-cli hooks suggest-context"
      }}]
    }}]
  }}
}}"#,
        env_json.join(",\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_mode_from_str() {
        assert_eq!(PermissionMode::from_str("acceptEdits"), PermissionMode::AcceptEdits);
        assert_eq!(PermissionMode::from_str("bypass"), PermissionMode::BypassPermissions);
        assert_eq!(PermissionMode::from_str("plan"), PermissionMode::Plan);
        assert_eq!(PermissionMode::from_str("unknown"), PermissionMode::Default);
    }

    #[test]
    fn test_permission_mode_allows() {
        assert!(PermissionMode::BypassPermissions.allows("edit"));
        assert!(PermissionMode::AcceptEdits.allows("read"));
        assert!(!PermissionMode::AcceptEdits.allows("bash"));
        assert!(PermissionMode::Plan.allows("grep"));
        assert!(!PermissionMode::Plan.allows("edit"));
    }

    #[test]
    fn test_telemetry_config_from_env() {
        // Default should have telemetry enabled
        let config = TelemetryConfig::default();
        assert!(config.is_enabled());
        assert!(!config.disable_telemetry);
    }

    #[test]
    fn test_hook_event_type_roundtrip() {
        for event in [
            HookEventType::PreToolUse,
            HookEventType::PostToolUse,
            HookEventType::SessionStart,
        ] {
            let s = event.as_str();
            assert_eq!(HookEventType::from_str(s), Some(event));
        }
    }

    #[test]
    fn test_generate_settings_json() {
        let config = TelemetryConfig::default();
        let json = generate_settings_json(&config);
        assert!(json.contains("RUVECTOR_INTELLIGENCE_ENABLED"));
        assert!(json.contains("PreToolUse"));
        assert!(json.contains("PostToolUse"));
    }
}
