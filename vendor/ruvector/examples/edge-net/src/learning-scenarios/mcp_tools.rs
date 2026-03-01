//! Enhanced MCP Tools for RuVector Learning Intelligence
//!
//! Provides MCP tool definitions that integrate with the self-learning
//! hooks system for intelligent code assistance.

use std::collections::HashMap;

/// MCP Tool definition for RuVector intelligence features
#[derive(Debug, Clone)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: ToolInputSchema,
    pub category: ToolCategory,
}

/// Tool input schema
#[derive(Debug, Clone)]
pub struct ToolInputSchema {
    pub required: Vec<String>,
    pub properties: HashMap<String, PropertyDef>,
}

/// Property definition for tool inputs
#[derive(Debug, Clone)]
pub struct PropertyDef {
    pub prop_type: String,
    pub description: String,
    pub default: Option<String>,
    pub enum_values: Option<Vec<String>>,
}

/// Tool categories for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    /// Vector database operations
    VectorDb,
    /// Learning and intelligence
    Learning,
    /// Memory and recall
    Memory,
    /// Swarm coordination
    Swarm,
    /// Telemetry and metrics
    Telemetry,
    /// Agent routing
    AgentRouting,
}

impl ToolCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VectorDb => "vector_db",
            Self::Learning => "learning",
            Self::Memory => "memory",
            Self::Swarm => "swarm",
            Self::Telemetry => "telemetry",
            Self::AgentRouting => "agent_routing",
        }
    }
}

/// Get all RuVector MCP tools
pub fn get_ruvector_tools() -> Vec<McpToolDef> {
    vec![
        // === Learning Intelligence Tools ===
        McpToolDef {
            name: "ruvector_learn_pattern".into(),
            description: "Record a Q-learning pattern for agent routing optimization".into(),
            input_schema: ToolInputSchema {
                required: vec!["state".into(), "action".into()],
                properties: [
                    ("state".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "State identifier (e.g., edit_rs_in_crate)".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("action".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Action taken (e.g., successful-edit, rust-developer)".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("reward".into(), PropertyDef {
                        prop_type: "number".into(),
                        description: "Reward value (-1.0 to 1.0)".into(),
                        default: Some("1.0".into()),
                        enum_values: None,
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Learning,
        },
        McpToolDef {
            name: "ruvector_suggest_agent".into(),
            description: "Get recommended agent for a task based on learned patterns".into(),
            input_schema: ToolInputSchema {
                required: vec!["task".into()],
                properties: [
                    ("task".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Task description".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("file".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "File being worked on".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("crate_name".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Crate/module context".into(),
                        default: None,
                        enum_values: None,
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::AgentRouting,
        },
        McpToolDef {
            name: "ruvector_record_error".into(),
            description: "Record an error pattern for learning recovery strategies".into(),
            input_schema: ToolInputSchema {
                required: vec!["error_code".into(), "message".into()],
                properties: [
                    ("error_code".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Error code (e.g., E0308, TS2322)".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("message".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Error message".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("file".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "File with error".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("fix_applied".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Fix that resolved the error".into(),
                        default: None,
                        enum_values: None,
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Learning,
        },
        McpToolDef {
            name: "ruvector_suggest_fix".into(),
            description: "Get suggested fixes for an error code based on learned patterns".into(),
            input_schema: ToolInputSchema {
                required: vec!["error_code".into()],
                properties: [
                    ("error_code".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Error code to get fixes for".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("context".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Additional context (file type, crate)".into(),
                        default: None,
                        enum_values: None,
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Learning,
        },

        // === Memory Tools ===
        McpToolDef {
            name: "ruvector_remember".into(),
            description: "Store content in semantic vector memory for later recall".into(),
            input_schema: ToolInputSchema {
                required: vec!["content".into(), "memory_type".into()],
                properties: [
                    ("content".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Content to remember".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("memory_type".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Type of memory".into(),
                        default: None,
                        enum_values: Some(vec![
                            "edit".into(), "command".into(), "decision".into(),
                            "pattern".into(), "error".into(), "agent_spawn".into(),
                        ]),
                    }),
                    ("metadata".into(), PropertyDef {
                        prop_type: "object".into(),
                        description: "Additional metadata".into(),
                        default: None,
                        enum_values: None,
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Memory,
        },
        McpToolDef {
            name: "ruvector_recall".into(),
            description: "Search semantic memory for relevant information".into(),
            input_schema: ToolInputSchema {
                required: vec!["query".into()],
                properties: [
                    ("query".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Search query".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("top_k".into(), PropertyDef {
                        prop_type: "integer".into(),
                        description: "Number of results to return".into(),
                        default: Some("5".into()),
                        enum_values: None,
                    }),
                    ("memory_type".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Filter by memory type".into(),
                        default: None,
                        enum_values: None,
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Memory,
        },

        // === Swarm Coordination Tools ===
        McpToolDef {
            name: "ruvector_swarm_register".into(),
            description: "Register an agent in the coordination swarm".into(),
            input_schema: ToolInputSchema {
                required: vec!["agent_id".into(), "agent_type".into()],
                properties: [
                    ("agent_id".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Unique agent identifier".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("agent_type".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Type of agent".into(),
                        default: None,
                        enum_values: Some(vec![
                            "researcher".into(), "coder".into(), "tester".into(),
                            "reviewer".into(), "planner".into(), "coordinator".into(),
                        ]),
                    }),
                    ("capabilities".into(), PropertyDef {
                        prop_type: "array".into(),
                        description: "Agent capabilities".into(),
                        default: None,
                        enum_values: None,
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Swarm,
        },
        McpToolDef {
            name: "ruvector_swarm_coordinate".into(),
            description: "Record coordination between agents for graph learning".into(),
            input_schema: ToolInputSchema {
                required: vec!["source".into(), "target".into()],
                properties: [
                    ("source".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Source agent ID".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("target".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Target agent ID".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("weight".into(), PropertyDef {
                        prop_type: "number".into(),
                        description: "Coordination weight (0.0-1.0)".into(),
                        default: Some("1.0".into()),
                        enum_values: None,
                    }),
                    ("success".into(), PropertyDef {
                        prop_type: "boolean".into(),
                        description: "Whether coordination was successful".into(),
                        default: Some("true".into()),
                        enum_values: None,
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Swarm,
        },
        McpToolDef {
            name: "ruvector_swarm_optimize".into(),
            description: "Get optimal task distribution across swarm agents".into(),
            input_schema: ToolInputSchema {
                required: vec!["tasks".into()],
                properties: [
                    ("tasks".into(), PropertyDef {
                        prop_type: "array".into(),
                        description: "List of tasks to distribute".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("strategy".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Distribution strategy".into(),
                        default: Some("balanced".into()),
                        enum_values: Some(vec![
                            "balanced".into(), "specialized".into(), "adaptive".into(),
                        ]),
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Swarm,
        },

        // === Telemetry Tools ===
        McpToolDef {
            name: "ruvector_telemetry_config".into(),
            description: "Configure telemetry settings".into(),
            input_schema: ToolInputSchema {
                required: vec![],
                properties: [
                    ("disable_telemetry".into(), PropertyDef {
                        prop_type: "boolean".into(),
                        description: "Disable Statsig metrics".into(),
                        default: Some("false".into()),
                        enum_values: None,
                    }),
                    ("disable_error_reporting".into(), PropertyDef {
                        prop_type: "boolean".into(),
                        description: "Disable Sentry error reporting".into(),
                        default: Some("false".into()),
                        enum_values: None,
                    }),
                    ("retention_days".into(), PropertyDef {
                        prop_type: "integer".into(),
                        description: "Data retention period in days".into(),
                        default: Some("30".into()),
                        enum_values: None,
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Telemetry,
        },
        McpToolDef {
            name: "ruvector_intelligence_stats".into(),
            description: "Get intelligence layer statistics".into(),
            input_schema: ToolInputSchema {
                required: vec![],
                properties: [
                    ("detailed".into(), PropertyDef {
                        prop_type: "boolean".into(),
                        description: "Include detailed breakdown".into(),
                        default: Some("false".into()),
                        enum_values: None,
                    }),
                    ("format".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Output format".into(),
                        default: Some("json".into()),
                        enum_values: Some(vec!["json".into(), "text".into(), "markdown".into()]),
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Telemetry,
        },

        // === File Sequence Tools ===
        McpToolDef {
            name: "ruvector_suggest_next_file".into(),
            description: "Suggest next files to edit based on learned patterns".into(),
            input_schema: ToolInputSchema {
                required: vec!["current_file".into()],
                properties: [
                    ("current_file".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Currently edited file".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("count".into(), PropertyDef {
                        prop_type: "integer".into(),
                        description: "Number of suggestions".into(),
                        default: Some("3".into()),
                        enum_values: None,
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Learning,
        },
        McpToolDef {
            name: "ruvector_record_sequence".into(),
            description: "Record file edit sequence for pattern learning".into(),
            input_schema: ToolInputSchema {
                required: vec!["files".into()],
                properties: [
                    ("files".into(), PropertyDef {
                        prop_type: "array".into(),
                        description: "Sequence of files edited".into(),
                        default: None,
                        enum_values: None,
                    }),
                    ("success".into(), PropertyDef {
                        prop_type: "boolean".into(),
                        description: "Whether sequence was successful".into(),
                        default: Some("true".into()),
                        enum_values: None,
                    }),
                    ("pattern_type".into(), PropertyDef {
                        prop_type: "string".into(),
                        description: "Type of editing pattern".into(),
                        default: None,
                        enum_values: Some(vec![
                            "rust_crate_setup".into(),
                            "tdd".into(),
                            "types_first".into(),
                            "refactoring".into(),
                        ]),
                    }),
                ].into_iter().collect(),
            },
            category: ToolCategory::Learning,
        },
    ]
}

/// Generate MCP tools list JSON
pub fn generate_tools_list_json() -> String {
    let tools = get_ruvector_tools();
    let tool_entries: Vec<String> = tools.iter().map(|tool| {
        let props: Vec<String> = tool.input_schema.properties.iter().map(|(name, prop)| {
            let mut prop_json = format!(
                r#"          "{}": {{
            "type": "{}",
            "description": "{}""#,
                name, prop.prop_type, prop.description
            );
            if let Some(default) = &prop.default {
                prop_json.push_str(&format!(r#",
            "default": {}"#, default));
            }
            if let Some(enums) = &prop.enum_values {
                let enum_str: Vec<String> = enums.iter().map(|e| format!("\"{}\"", e)).collect();
                prop_json.push_str(&format!(r#",
            "enum": [{}]"#, enum_str.join(", ")));
            }
            prop_json.push_str("\n          }");
            prop_json
        }).collect();

        let required: Vec<String> = tool.input_schema.required.iter().map(|r| format!("\"{}\"", r)).collect();

        format!(
            r#"    {{
      "name": "{}",
      "description": "{}",
      "inputSchema": {{
        "type": "object",
        "properties": {{
{}
        }},
        "required": [{}]
      }}
    }}"#,
            tool.name, tool.description, props.join(",\n"), required.join(", ")
        )
    }).collect();

    format!(
        r#"{{
  "tools": [
{}
  ]
}}"#,
        tool_entries.join(",\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_ruvector_tools() {
        let tools = get_ruvector_tools();
        assert!(!tools.is_empty());

        // Check we have tools in each category
        let categories: Vec<ToolCategory> = tools.iter().map(|t| t.category).collect();
        assert!(categories.contains(&ToolCategory::Learning));
        assert!(categories.contains(&ToolCategory::Memory));
        assert!(categories.contains(&ToolCategory::Swarm));
    }

    #[test]
    fn test_tool_has_required_properties() {
        let tools = get_ruvector_tools();
        for tool in tools {
            // All required fields should be in properties
            for req in &tool.input_schema.required {
                assert!(
                    tool.input_schema.properties.contains_key(req),
                    "Tool {} missing required property {}", tool.name, req
                );
            }
        }
    }

    #[test]
    fn test_generate_tools_list_json() {
        let json = generate_tools_list_json();
        assert!(json.contains("\"tools\""));
        assert!(json.contains("ruvector_learn_pattern"));
        assert!(json.contains("ruvector_remember"));
    }
}
