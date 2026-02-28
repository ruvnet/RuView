/// Example 09: MCP Tools - Tool registry and JSON schema generation
///
/// Demonstrates:
/// - Creating a RoboticsToolRegistry with built-in tools
/// - Listing all registered tools
/// - Filtering tools by category
/// - Looking up a specific tool and inspecting its parameters
/// - Registering a custom tool
/// - Generating MCP-compatible JSON schema

use ruvector_robotics::mcp::{
    ParamType, RoboticsToolRegistry, ToolCategory, ToolDefinition, ToolParameter,
};

fn main() {
    println!("=== Example 09: MCP Tools ===");
    println!();

    // -- Step 1: Create registry --
    let mut registry = RoboticsToolRegistry::new();
    println!("[1] Tool registry created: {} built-in tools", registry.list_tools().len());

    // -- Step 2: List all tools --
    println!();
    println!("[2] All registered tools:");
    let mut tools: Vec<_> = registry.list_tools().iter().map(|t| &t.name).collect();
    tools.sort();
    for name in &tools {
        let tool = registry.get_tool(name).unwrap();
        println!(
            "    {:.<30} {:?} ({} params)",
            tool.name, tool.category, tool.parameters.len()
        );
    }

    // -- Step 3: Filter by category --
    println!();
    let categories = [
        ToolCategory::Perception,
        ToolCategory::Navigation,
        ToolCategory::Cognition,
        ToolCategory::Memory,
        ToolCategory::Planning,
        ToolCategory::Swarm,
    ];
    println!("[3] Tools by category:");
    for cat in &categories {
        let cat_tools = registry.list_by_category(*cat);
        println!("    {:?}: {} tools", cat, cat_tools.len());
        for tool in &cat_tools {
            println!("      - {}", tool.name);
        }
    }

    // -- Step 4: Inspect a specific tool --
    println!();
    if let Some(tool) = registry.get_tool("detect_obstacles") {
        println!("[4] Tool details for 'detect_obstacles':");
        println!("    Description: {}", tool.description);
        println!("    Category:    {:?}", tool.category);
        println!("    Parameters:");
        for param in &tool.parameters {
            println!(
                "      - {} ({:?}, required={}): {}",
                param.name, param.param_type, param.required, param.description
            );
        }
    }

    // -- Step 5: Register a custom tool --
    println!();
    let custom = ToolDefinition::new(
        "custom_slam",
        "Run SLAM algorithm on sensor data",
        vec![
            ToolParameter::new("point_cloud_json", "JSON-encoded point cloud", ParamType::String, true),
            ToolParameter::new("odometry_json", "JSON-encoded odometry data", ParamType::String, false),
            ToolParameter::new("resolution", "Map resolution in meters", ParamType::Number, false),
        ],
        ToolCategory::Perception,
    );
    registry.register_tool(custom);
    println!("[5] Registered custom tool 'custom_slam'. Total: {} tools", registry.list_tools().len());

    // -- Step 6: Generate MCP schema --
    println!();
    let schema = registry.to_mcp_schema();
    let json = serde_json::to_string_pretty(&schema).unwrap();
    println!("[6] Full MCP schema ({} bytes):", json.len());
    for (i, line) in json.lines().enumerate() {
        if i > 14 {
            println!("    ... ({} more lines)", json.lines().count() - i);
            break;
        }
        println!("    {}", line);
    }

    println!();
    println!("[done] MCP tools example complete.");
}
