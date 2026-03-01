//! MCP tool registrations for agentic robotics.
//!
//! Provides a registry of robotics tools that can be exposed via MCP servers.
//! This is a lightweight, dependency-free implementation that models tool
//! definitions, categories, and JSON schema generation without pulling in an
//! external MCP SDK.

pub mod executor;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Parameter types
// ---------------------------------------------------------------------------

/// JSON Schema type for a tool parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    String,
    Number,
    Integer,
    Boolean,
    Array,
    Object,
}

impl ParamType {
    fn as_schema_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::Array => "array",
            Self::Object => "object",
        }
    }
}

/// A single parameter accepted by a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub param_type: ParamType,
    pub required: bool,
}

impl ToolParameter {
    pub fn new(name: &str, description: &str, param_type: ParamType, required: bool) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            param_type,
            required,
        }
    }
}

// ---------------------------------------------------------------------------
// Tool categories
// ---------------------------------------------------------------------------

/// High-level category that a tool belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolCategory {
    Perception,
    Navigation,
    Cognition,
    Swarm,
    Memory,
    Planning,
}

// ---------------------------------------------------------------------------
// Tool definition
// ---------------------------------------------------------------------------

/// Complete definition of a single MCP-exposed tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
    pub category: ToolCategory,
}

impl ToolDefinition {
    pub fn new(
        name: &str,
        description: &str,
        parameters: Vec<ToolParameter>,
        category: ToolCategory,
    ) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
            category,
        }
    }

    /// Convert this tool definition into its JSON Schema representation.
    fn to_schema(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required: Vec<serde_json::Value> = Vec::new();

        for param in &self.parameters {
            let mut prop = serde_json::Map::new();
            prop.insert(
                "type".to_string(),
                serde_json::Value::String(param.param_type.as_schema_str().to_string()),
            );
            prop.insert(
                "description".to_string(),
                serde_json::Value::String(param.description.clone()),
            );
            properties.insert(param.name.clone(), serde_json::Value::Object(prop));

            if param.required {
                required.push(serde_json::Value::String(param.name.clone()));
            }
        }

        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": {
                "type": "object",
                "properties": properties,
                "required": required,
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Request / Response
// ---------------------------------------------------------------------------

/// A request to invoke a tool by name with JSON arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    pub tool_name: String,
    pub arguments: HashMap<String, serde_json::Value>,
}

/// The result of a tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub success: bool,
    pub result: serde_json::Value,
    pub error: Option<String>,
    pub latency_us: u64,
}

impl ToolResponse {
    /// Convenience constructor for a successful response.
    pub fn ok(result: serde_json::Value, latency_us: u64) -> Self {
        Self { success: true, result, error: None, latency_us }
    }

    /// Convenience constructor for a failed response.
    pub fn err(message: impl Into<String>, latency_us: u64) -> Self {
        Self {
            success: false,
            result: serde_json::Value::Null,
            error: Some(message.into()),
            latency_us,
        }
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Registry of robotics tools exposed to MCP clients.
///
/// Call [`RoboticsToolRegistry::new`] to get a registry pre-populated with all
/// built-in tools, or start from [`RoboticsToolRegistry::empty`] and register
/// tools manually.
#[derive(Debug, Clone)]
pub struct RoboticsToolRegistry {
    tools: HashMap<String, ToolDefinition>,
}

impl Default for RoboticsToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RoboticsToolRegistry {
    /// Create a registry pre-populated with all built-in robotics tools.
    pub fn new() -> Self {
        let mut registry = Self { tools: HashMap::new() };
        registry.register_defaults();
        registry
    }

    /// Create an empty registry with no tools registered.
    pub fn empty() -> Self {
        Self { tools: HashMap::new() }
    }

    /// Register a single tool. Overwrites any existing tool with the same name.
    pub fn register_tool(&mut self, tool: ToolDefinition) {
        self.tools.insert(tool.name.clone(), tool);
    }

    /// List every registered tool (unordered).
    pub fn list_tools(&self) -> Vec<&ToolDefinition> {
        self.tools.values().collect()
    }

    /// Look up a tool by its exact name.
    pub fn get_tool(&self, name: &str) -> Option<&ToolDefinition> {
        self.tools.get(name)
    }

    /// Return all tools belonging to the given category.
    pub fn list_by_category(&self, category: ToolCategory) -> Vec<&ToolDefinition> {
        self.tools.values().filter(|t| t.category == category).collect()
    }

    /// Produce a full MCP-compatible JSON schema describing every tool.
    pub fn to_mcp_schema(&self) -> serde_json::Value {
        let mut tools: Vec<serde_json::Value> =
            self.tools.values().map(|t| t.to_schema()).collect();
        // Sort by name for deterministic output.
        tools.sort_by(|a, b| {
            let na = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let nb = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
            na.cmp(nb)
        });
        serde_json::json!({ "tools": tools })
    }

    // -- default tool registration ------------------------------------------

    fn register_defaults(&mut self) {
        self.register_tool(ToolDefinition::new(
            "detect_obstacles",
            "Detect obstacles in a point cloud relative to the robot position",
            vec![
                ToolParameter::new(
                    "point_cloud_json", "JSON-encoded point cloud", ParamType::String, true,
                ),
                ToolParameter::new(
                    "robot_position", "Robot [x,y,z] position", ParamType::Array, true,
                ),
                ToolParameter::new(
                    "max_distance", "Maximum detection distance in meters", ParamType::Number, false,
                ),
            ],
            ToolCategory::Perception,
        ));

        self.register_tool(ToolDefinition::new(
            "build_scene_graph",
            "Build a scene graph from detected objects with spatial edges",
            vec![
                ToolParameter::new(
                    "objects_json", "JSON array of scene objects", ParamType::String, true,
                ),
                ToolParameter::new(
                    "max_edge_distance", "Maximum edge distance in meters", ParamType::Number, false,
                ),
            ],
            ToolCategory::Perception,
        ));

        self.register_tool(ToolDefinition::new(
            "predict_trajectory",
            "Predict future trajectory from current position and velocity",
            vec![
                ToolParameter::new("position", "Current [x,y,z] position", ParamType::Array, true),
                ToolParameter::new("velocity", "Current [vx,vy,vz] velocity", ParamType::Array, true),
                ToolParameter::new("steps", "Number of prediction steps", ParamType::Integer, true),
                ToolParameter::new("dt", "Time step in seconds", ParamType::Number, false),
            ],
            ToolCategory::Navigation,
        ));

        self.register_tool(ToolDefinition::new(
            "focus_attention",
            "Extract a region of interest from a point cloud by center and radius",
            vec![
                ToolParameter::new(
                    "point_cloud_json", "JSON-encoded point cloud", ParamType::String, true,
                ),
                ToolParameter::new("center", "Focus center [x,y,z]", ParamType::Array, true),
                ToolParameter::new("radius", "Attention radius in meters", ParamType::Number, true),
            ],
            ToolCategory::Perception,
        ));

        self.register_tool(ToolDefinition::new(
            "detect_anomalies",
            "Detect anomalous points in a point cloud using statistical analysis",
            vec![
                ToolParameter::new(
                    "point_cloud_json", "JSON-encoded point cloud", ParamType::String, true,
                ),
            ],
            ToolCategory::Perception,
        ));

        self.register_tool(ToolDefinition::new(
            "spatial_search",
            "Search for nearest neighbours in the spatial index",
            vec![
                ToolParameter::new("query", "Query vector [x,y,z]", ParamType::Array, true),
                ToolParameter::new("k", "Number of neighbours to return", ParamType::Integer, true),
            ],
            ToolCategory::Perception,
        ));

        self.register_tool(ToolDefinition::new(
            "insert_points",
            "Insert points into the spatial index for later retrieval",
            vec![
                ToolParameter::new(
                    "points_json", "JSON array of [x,y,z] points", ParamType::String, true,
                ),
            ],
            ToolCategory::Perception,
        ));

        self.register_tool(ToolDefinition::new(
            "store_memory",
            "Store a vector in episodic memory with an importance score",
            vec![
                ToolParameter::new("key", "Unique memory key", ParamType::String, true),
                ToolParameter::new("data", "Data vector to store", ParamType::Array, true),
                ToolParameter::new(
                    "importance", "Importance weight 0.0-1.0", ParamType::Number, false,
                ),
            ],
            ToolCategory::Memory,
        ));

        self.register_tool(ToolDefinition::new(
            "recall_memory",
            "Recall the k most similar memories to a query vector",
            vec![
                ToolParameter::new(
                    "query", "Query vector for similarity search", ParamType::Array, true,
                ),
                ToolParameter::new("k", "Number of memories to recall", ParamType::Integer, true),
            ],
            ToolCategory::Memory,
        ));

        self.register_tool(ToolDefinition::new(
            "learn_skill",
            "Learn a new skill from demonstration trajectories",
            vec![
                ToolParameter::new("name", "Skill name identifier", ParamType::String, true),
                ToolParameter::new(
                    "demonstrations_json",
                    "JSON array of demonstration trajectories",
                    ParamType::String,
                    true,
                ),
            ],
            ToolCategory::Cognition,
        ));

        self.register_tool(ToolDefinition::new(
            "execute_skill",
            "Execute a previously learned skill by name",
            vec![
                ToolParameter::new("name", "Name of the skill to execute", ParamType::String, true),
            ],
            ToolCategory::Cognition,
        ));

        self.register_tool(ToolDefinition::new(
            "plan_behavior",
            "Generate a behavior tree plan for a given goal and preconditions",
            vec![
                ToolParameter::new("goal", "Goal description", ParamType::String, true),
                ToolParameter::new(
                    "conditions_json",
                    "JSON object of current conditions",
                    ParamType::String,
                    false,
                ),
            ],
            ToolCategory::Planning,
        ));

        self.register_tool(ToolDefinition::new(
            "coordinate_swarm",
            "Coordinate a multi-robot swarm for a given task",
            vec![
                ToolParameter::new(
                    "task_json", "JSON-encoded task specification", ParamType::String, true,
                ),
            ],
            ToolCategory::Swarm,
        ));

        self.register_tool(ToolDefinition::new(
            "update_world_model",
            "Update the internal world model with a new or changed object",
            vec![
                ToolParameter::new(
                    "object_json", "JSON-encoded object to upsert", ParamType::String, true,
                ),
            ],
            ToolCategory::Cognition,
        ));

        self.register_tool(ToolDefinition::new(
            "get_world_state",
            "Retrieve the current world model state, optionally filtered by object id",
            vec![
                ToolParameter::new(
                    "object_id", "Optional object id to filter", ParamType::Integer, false,
                ),
            ],
            ToolCategory::Cognition,
        ));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_15_default_tools() {
        let registry = RoboticsToolRegistry::new();
        assert_eq!(registry.list_tools().len(), 15);
    }

    #[test]
    fn test_list_tools_returns_all() {
        let registry = RoboticsToolRegistry::new();
        let tools = registry.list_tools();
        let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        names.sort();

        let expected = vec![
            "build_scene_graph",
            "coordinate_swarm",
            "detect_anomalies",
            "detect_obstacles",
            "execute_skill",
            "focus_attention",
            "get_world_state",
            "insert_points",
            "learn_skill",
            "plan_behavior",
            "predict_trajectory",
            "recall_memory",
            "spatial_search",
            "store_memory",
            "update_world_model",
        ];
        assert_eq!(names, expected);
    }

    #[test]
    fn test_get_tool_by_name() {
        let registry = RoboticsToolRegistry::new();

        let tool = registry.get_tool("detect_obstacles").unwrap();
        assert_eq!(tool.category, ToolCategory::Perception);
        assert_eq!(tool.parameters.len(), 3);
        assert!(tool.parameters.iter().any(|p| p.name == "point_cloud_json" && p.required));

        let tool = registry.get_tool("predict_trajectory").unwrap();
        assert_eq!(tool.category, ToolCategory::Navigation);
        assert_eq!(tool.parameters.len(), 4);

        assert!(registry.get_tool("nonexistent").is_none());
    }

    #[test]
    fn test_list_by_category_perception() {
        let registry = RoboticsToolRegistry::new();
        let perception = registry.list_by_category(ToolCategory::Perception);
        assert_eq!(perception.len(), 6);
        for tool in &perception {
            assert_eq!(tool.category, ToolCategory::Perception);
        }
    }

    #[test]
    fn test_list_by_category_counts() {
        let registry = RoboticsToolRegistry::new();
        assert_eq!(registry.list_by_category(ToolCategory::Perception).len(), 6);
        assert_eq!(registry.list_by_category(ToolCategory::Navigation).len(), 1);
        assert_eq!(registry.list_by_category(ToolCategory::Cognition).len(), 4);
        assert_eq!(registry.list_by_category(ToolCategory::Memory).len(), 2);
        assert_eq!(registry.list_by_category(ToolCategory::Planning).len(), 1);
        assert_eq!(registry.list_by_category(ToolCategory::Swarm).len(), 1);
    }

    #[test]
    fn test_to_mcp_schema_valid_json() {
        let registry = RoboticsToolRegistry::new();
        let schema = registry.to_mcp_schema();

        let tools = schema.get("tools").unwrap().as_array().unwrap();
        assert_eq!(tools.len(), 15);

        // Tools are sorted by name.
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t.get("name").unwrap().as_str().unwrap())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);

        // Each tool has the expected schema shape.
        for tool in tools {
            assert!(tool.get("name").unwrap().is_string());
            assert!(tool.get("description").unwrap().is_string());
            let input = tool.get("inputSchema").unwrap();
            assert_eq!(input.get("type").unwrap().as_str().unwrap(), "object");
            assert!(input.get("properties").unwrap().is_object());
            assert!(input.get("required").unwrap().is_array());
        }
    }

    #[test]
    fn test_schema_required_fields() {
        let registry = RoboticsToolRegistry::new();
        let schema = registry.to_mcp_schema();
        let tools = schema["tools"].as_array().unwrap();

        let obs = tools.iter().find(|t| t["name"] == "detect_obstacles").unwrap();
        let required = obs["inputSchema"]["required"].as_array().unwrap();
        let req_names: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(req_names.contains(&"point_cloud_json"));
        assert!(req_names.contains(&"robot_position"));
        assert!(!req_names.contains(&"max_distance"));
    }

    #[test]
    fn test_tool_request_serialization() {
        let mut args = HashMap::new();
        args.insert("k".to_string(), serde_json::json!(5));
        args.insert("query".to_string(), serde_json::json!([1.0, 2.0, 3.0]));

        let req = ToolRequest { tool_name: "spatial_search".to_string(), arguments: args };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: ToolRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tool_name, "spatial_search");
        assert_eq!(deserialized.arguments["k"], serde_json::json!(5));
    }

    #[test]
    fn test_tool_response_ok() {
        let resp = ToolResponse::ok(serde_json::json!({"obstacles": 3}), 420);
        assert!(resp.success);
        assert!(resp.error.is_none());
        assert_eq!(resp.latency_us, 420);
        assert_eq!(resp.result["obstacles"], 3);

        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: ToolResponse = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
    }

    #[test]
    fn test_tool_response_err() {
        let resp = ToolResponse::err("something went wrong", 100);
        assert!(!resp.success);
        assert_eq!(resp.error.as_deref(), Some("something went wrong"));
        assert!(resp.result.is_null());
    }

    #[test]
    fn test_register_custom_tool() {
        let mut registry = RoboticsToolRegistry::new();
        assert_eq!(registry.list_tools().len(), 15);

        let custom = ToolDefinition::new(
            "my_custom_tool",
            "A custom tool for testing",
            vec![ToolParameter::new("input", "The input data", ParamType::String, true)],
            ToolCategory::Cognition,
        );
        registry.register_tool(custom);
        assert_eq!(registry.list_tools().len(), 16);

        let tool = registry.get_tool("my_custom_tool").unwrap();
        assert_eq!(tool.description, "A custom tool for testing");
        assert_eq!(tool.parameters.len(), 1);
    }

    #[test]
    fn test_register_overwrites_existing() {
        let mut registry = RoboticsToolRegistry::new();
        let replacement = ToolDefinition::new(
            "detect_obstacles",
            "Replaced description",
            vec![],
            ToolCategory::Perception,
        );
        registry.register_tool(replacement);
        assert_eq!(registry.list_tools().len(), 15);
        let tool = registry.get_tool("detect_obstacles").unwrap();
        assert_eq!(tool.description, "Replaced description");
        assert!(tool.parameters.is_empty());
    }

    #[test]
    fn test_empty_registry() {
        let registry = RoboticsToolRegistry::empty();
        assert_eq!(registry.list_tools().len(), 0);
        assert!(registry.get_tool("detect_obstacles").is_none());
    }

    #[test]
    fn test_param_type_serde_roundtrip() {
        let types = vec![
            ParamType::String,
            ParamType::Number,
            ParamType::Integer,
            ParamType::Boolean,
            ParamType::Array,
            ParamType::Object,
        ];
        for pt in types {
            let json = serde_json::to_string(&pt).unwrap();
            let deserialized: ParamType = serde_json::from_str(&json).unwrap();
            assert_eq!(pt, deserialized);
        }
    }

    #[test]
    fn test_tool_category_serde_roundtrip() {
        let categories = vec![
            ToolCategory::Perception,
            ToolCategory::Navigation,
            ToolCategory::Cognition,
            ToolCategory::Swarm,
            ToolCategory::Memory,
            ToolCategory::Planning,
        ];
        for cat in categories {
            let json = serde_json::to_string(&cat).unwrap();
            let deserialized: ToolCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(cat, deserialized);
        }
    }

    #[test]
    fn test_tool_definition_serde_roundtrip() {
        let tool = ToolDefinition::new(
            "test_tool",
            "A tool for testing",
            vec![
                ToolParameter::new("a", "param a", ParamType::String, true),
                ToolParameter::new("b", "param b", ParamType::Number, false),
            ],
            ToolCategory::Navigation,
        );
        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: ToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(tool, deserialized);
    }

    #[test]
    fn test_default_trait() {
        let registry = RoboticsToolRegistry::default();
        assert_eq!(registry.list_tools().len(), 15);
    }
}
