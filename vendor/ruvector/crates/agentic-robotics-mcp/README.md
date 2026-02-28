# agentic-robotics-mcp

[![Crates.io](https://img.shields.io/crates/v/agentic-robotics-mcp.svg)](https://crates.io/crates/agentic-robotics-mcp)
[![Documentation](https://docs.rs/agentic-robotics-mcp/badge.svg)](https://docs.rs/agentic-robotics-mcp)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](../../LICENSE)
[![MCP 2025-11](https://img.shields.io/badge/MCP-2025--11-green.svg)](https://modelcontextprotocol.io)

**Control robots with AI assistants using the Model Context Protocol**

Give Claude, GPT, or any AI assistant the ability to control your robots through natural language. Part of the [Agentic Robotics](https://github.com/ruvnet/vibecast) framework.

---

## 🎯 What is This?

**Problem:** You have a robot. You want to control it with natural language using an AI assistant like Claude.

**Solution:** This crate implements the [Model Context Protocol (MCP)](https://modelcontextprotocol.io), which lets AI assistants discover and use your robot's capabilities as "tools".

**Example conversation:**

```
You: "Claude, move the robot to the kitchen"
Claude: *calls move_robot tool with location="kitchen"*
Robot: *navigates to kitchen*
Claude: "I've moved the robot to the kitchen"
```

---

## 🚀 Quick Start (5 minutes)

### Step 1: Add to your project

```toml
[dependencies]
agentic-robotics-mcp = "0.1"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

### Step 2: Create a simple MCP server

```rust
use agentic_robotics_mcp::*;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create MCP server
    let server = McpServer::new("my-robot", "1.0.0");

    // Register a "move_robot" tool
    let move_tool = McpTool {
        name: "move_robot".to_string(),
        description: "Move the robot to a location".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "Where to move (kitchen, bedroom, etc.)"
                }
            },
            "required": ["location"]
        }),
    };

    server.register_tool(move_tool, server::tool(|args| {
        let location = args["location"].as_str().unwrap();
        println!("🤖 Moving robot to: {}", location);

        // Your robot movement code here
        // move_robot_hardware(location);

        Ok(server::text_response(format!(
            "Robot moved to {}",
            location
        )))
    })).await?;

    // Run stdio transport (for Claude Desktop, IDEs, etc.)
    let transport = transport::StdioTransport::new(server);
    transport.run().await?;

    Ok(())
}
```

### Step 3: Connect from Claude Desktop

Add to your Claude Desktop config:

**Mac:** `~/Library/Application Support/Claude/claude_desktop_config.json`  
**Windows:** `%APPDATA%\Claude\claude_desktop_config.json`  
**Linux:** `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "my-robot": {
      "command": "/path/to/your/robot-mcp-server"
    }
  }
}
```

**That's it!** Claude can now control your robot 🎉

---

## 📖 Complete Documentation

This README provides everything you need to know. Jump to:

- [Why Use MCP](#-why-use-mcp-for-robots)
- [Complete Tutorial](#-complete-tutorial)
- [Real-World Examples](#-real-world-use-cases)
- [Advanced Features](#-advanced-features)
- [Troubleshooting](#-troubleshooting)

**Or view the full docs at [docs.rs/agentic-robotics-mcp](https://docs.rs/agentic-robotics-mcp)**

---

## 🤖 Why Use MCP for Robots?

Traditional robot control requires writing code for every possible command. With MCP, you describe what your robot can do, and AI figures out how to use those capabilities.

### Before MCP
```rust
// You write code for hundreds of commands
match command {
    "move forward" => robot.forward(),
    "turn left" => robot.left(),
    "go to kitchen" => robot.navigate("kitchen"),
    // ... 100+ more commands
}
```

### With MCP
```rust
// Just describe capabilities - AI does the rest
server.register_tool(move_tool, handler);
server.register_tool(grab_tool, handler);
server.register_tool(scan_tool, handler);

// AI: "go to kitchen and grab the cup"
// -> Automatically calls: move_robot("kitchen"), grab_object("cup")
```

**Benefits:**
- ✅ **Natural language** - Control robots by talking naturally
- ✅ **Flexible** - AI combines tools in creative, unexpected ways
- ✅ **Simple** - Just describe capabilities, don't write parsers
- ✅ **Standard** - Works with Claude, GPT, and all MCP-compatible AIs
- ✅ **Discoverable** - AI learns what your robot can do automatically

---

## 🎓 Complete Tutorial

Let's build complete robot control systems step by step.

### Example 1: Navigation Robot (Beginner)

```rust
use agentic_robotics_mcp::*;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = McpServer::new("navigation-robot", "1.0.0");

    // Tool 1: Move to location
    server.register_tool(
        McpTool {
            name: "move_to".to_string(),
            description: "Move robot to a named location".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "kitchen, bedroom, living room, etc."
                    }
                },
                "required": ["location"]
            }),
        },
        server::tool(|args| {
            let location = args["location"].as_str().unwrap();
            Ok(server::text_response(format!("Moving to {}", location)))
        })
    ).await?;

    // Tool 2: Get current status
    server.register_tool(
        McpTool {
            name: "get_status".to_string(),
            description: "Get robot position, battery level, and state".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        server::tool(|_| {
            Ok(server::text_response(
                "Position: (5.2, 3.1)\nBattery: 87%\nState: Idle"
            ))
        })
    ).await?;

    // Tool 3: Emergency stop
    server.register_tool(
        McpTool {
            name: "emergency_stop".to_string(),
            description: "EMERGENCY: Stop all robot movement immediately".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        server::tool(|_| {
            println!("🛑 EMERGENCY STOP");
            Ok(server::text_response("Robot stopped"))
        })
    ).await?;

    // Start MCP server with stdio transport
    let transport = transport::StdioTransport::new(server);
    transport.run().await?;

    Ok(())
}
```

**What Claude can do:**
- "Move to the kitchen" → `move_to(location="kitchen")`
- "Where are you?" → `get_status()`
- "Stop immediately!" → `emergency_stop()`

### Example 2: Vision Robot with Images (Intermediate)

```rust
use agentic_robotics_mcp::*;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = McpServer::new("vision-robot", "1.0.0");

    // Tool: Detect objects in view
    server.register_tool(
        McpTool {
            name: "detect_objects".to_string(),
            description: "Detect all objects visible to camera".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        server::tool(|_| {
            // Your vision code here
            let objects = vec!["cup", "book", "phone"];

            Ok(server::text_response(format!(
                "Detected:\n{}",
                objects.iter().map(|o| format!("- {}", o)).collect::<Vec<_>>().join("\n")
            )))
        })
    ).await?;

    // Tool: Take photo and return image
    server.register_tool(
        McpTool {
            name: "take_photo".to_string(),
            description: "Capture photo from robot camera".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        server::tool(|_| {
            // Capture and encode image
            // let image_base64 = capture_camera_base64();

            Ok(ToolResult {
                content: vec![
                    ContentItem::Text {
                        text: "Photo captured".to_string()
                    },
                    ContentItem::Image {
                        data: "iVBORw0KGgoAAAANS...".to_string(),  // base64
                        mimeType: "image/jpeg".to_string(),
                    }
                ],
                is_error: None,
            })
        })
    ).await?;

    let transport = transport::StdioTransport::new(server);
    transport.run().await?;

    Ok(())
}
```

**What Claude can do:**
- "What do you see?" → Shows detected objects
- "Take a picture" → Returns photo to Claude (shown to user)
- "Is there a cup nearby?" → Combines detection + reasoning

### Example 3: Robotic Arm (Advanced)

```rust
use agentic_robotics_mcp::*;
use agentic_robotics_core::Node;  // Connect to your robot
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect to robot control system
    let mut node = Node::new("mcp_arm_controller")?;
    let cmd_pub = node.publish("/arm/commands")?;

    let server = McpServer::new("robotic-arm", "1.0.0");

    // Tool: Pick up object
    server.register_tool(
        McpTool {
            name: "pick_object".to_string(),
            description: "Pick up an object at specified position".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "object": { "type": "string" },
                    "x": { "type": "number" },
                    "y": { "type": "number" },
                    "z": { "type": "number" }
                },
                "required": ["object", "x", "y", "z"]
            }),
        },
        server::tool(move |args| {
            let obj = args["object"].as_str().unwrap();
            let x = args["x"].as_f64().unwrap();
            let y = args["y"].as_f64().unwrap();
            let z = args["z"].as_f64().unwrap();

            // Send command to robot
            // cmd_pub.publish(&PickCommand { object: obj, position: (x,y,z) }).await?;

            Ok(server::text_response(format!(
                "Picked up {} at ({}, {}, {})",
                obj, x, y, z
            )))
        })
    ).await?;

    // Tool: Place object
    server.register_tool(
        McpTool {
            name: "place_object".to_string(),
            description: "Place held object at location".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "location": { "type": "string", "description": "table, shelf, etc." }
                },
                "required": ["location"]
            }),
        },
        server::tool(|args| {
            let loc = args["location"].as_str().unwrap();
            Ok(server::text_response(format!("Placed object at {}", loc)))
        })
    ).await?;

    let transport = transport::StdioTransport::new(server);
    transport.run().await?;

    Ok(())
}
```

**What Claude can do:**
- "Pick up the red block at position (0.5, 0.3, 0.1)" → Precise control
- "Place it on the table" → Predefined locations
- "Move the cup from the counter to the shelf" → Multi-step tasks

---

## 🌟 Real-World Use Cases

### Use Case 1: Warehouse Robot

```rust
// Tools: navigate_to, scan_barcode, pick_item, place_item, get_battery

// Claude conversation:
// "Go to aisle 5, scan the items, and bring any with low stock to the depot"
// -> Robot autonomously: navigates, scans, identifies low stock, picks, delivers
```

### Use Case 2: Home Assistant Robot

```rust
// Tools: navigate, detect_objects, vacuum_area, water_plants, take_photo

// Claude:
// "Clean the living room and water any plants that look dry"
// -> Navigates, identifies plants, checks moisture, waters as needed
```

### Use Case 3: Research Laboratory Robot

```rust
// Tools: move_to_station, pipette_liquid, centrifuge, analyze_sample

// Claude:
// "Prepare 10 samples for PCR analysis"
// -> Executes lab protocol automatically
```

### Use Case 4: Security Patrol Robot

```rust
// Tools: patrol_route, detect_anomalies, take_photo, sound_alarm

// Claude:
// "Patrol the building and alert me if you see anything unusual"
// -> Autonomous patrol with AI-powered anomaly detection
```

---

## 🔧 Advanced Features

### Returning Images

```rust
server::tool(|_| {
    let image_data = capture_camera();  // Your camera code
    let base64 = base64::encode(image_data);

    Ok(ToolResult {
        content: vec![
            ContentItem::Image {
                data: base64,
                mimeType: "image/jpeg".to_string(),
            }
        ],
        is_error: None,
    })
})
```

### Multiple Content Items

```rust
server::tool(|_| {
    Ok(ToolResult {
        content: vec![
            ContentItem::Text { text: "Scan complete".to_string() },
            ContentItem::Image { data: photo_base64, mimeType: "image/jpeg".to_string() },
            ContentItem::Resource {
                uri: "file:///robot/scans/scan001.pcd".to_string(),
                mimeType: "application/octet-stream".to_string(),
                data: point_cloud_base64,
            }
        ],
        is_error: None,
    })
})
```

### Error Handling

```rust
server::tool(|args| {
    let location = args["location"].as_str().unwrap();

    if location == "restricted_area" {
        return Ok(server::error_response(
            "Access denied: Cannot enter restricted area"
        ));
    }

    Ok(server::text_response("Moving..."))
})
```

### Async Operations

```rust
server::tool(|args| {
    // Tool handlers are sync, but you can use tokio::task::block_in_place
    // for async work if needed
    Ok(server::text_response("Done"))
})
```

---

## 🔌 Supported Transports

### STDIO (Local AI Assistants)

For Claude Desktop, VS Code extensions, command-line tools:

```rust
let transport = transport::StdioTransport::new(server);
transport.run().await?;
```

### SSE (Remote Web Access)

For web dashboards, mobile apps, remote control:

```rust
// Coming soon
use agentic_robotics_mcp::transport::sse;
sse::run_sse_server(server, "0.0.0.0:8080").await?;
```

---

## 🛠️ Configuration Examples

### Claude Desktop Config

**Mac:** `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "warehouse-robot": {
      "command": "/opt/robots/warehouse-mcp",
      "env": {
        "ROBOT_ID": "WH-001",
        "ROBOT_HOST": "192.168.1.100"
      }
    },
    "home-assistant": {
      "command": "/usr/local/bin/home-robot-mcp"
    }
  }
}
```

### Reading Environment Variables

```rust
use std::env;

let robot_id = env::var("ROBOT_ID").unwrap_or("default".to_string());
let robot_host = env::var("ROBOT_HOST").unwrap_or("localhost".to_string());
```

---

## 🐛 Troubleshooting

### Server doesn't appear in Claude

**Check:**
1. Config file path is correct for your OS
2. Binary is executable: `chmod +x /path/to/mcp-server`
3. Binary runs standalone: `./mcp-server` (should wait for input)
4. Check Claude logs:
   - Mac: `~/Library/Logs/Claude/mcp-server-*.log`
   - Windows: `%APPDATA%\Claude\logs\`
   - Linux: `~/.local/state/Claude/logs/`

### Tools aren't being called

**Solutions:**
1. Make tool descriptions very clear and specific
2. Verify `input_schema` matches what AI sends
3. Add logging: `eprintln!("Tool {} called with: {:?}", name, args);`
4. Test with simple tools first

### Connection errors

```rust
// Add error handling
match transport.run().await {
    Ok(_) => println!("Server stopped gracefully"),
    Err(e) => {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}
```

### Debug Mode

```rust
// Enable debug output
env_logger::init();

// Or manual logging
eprintln!("MCP Server started");
eprintln!("Registered tools: {:?}", tool_names);
```

---

## 📚 Examples

Complete working examples in the [repository](https://github.com/ruvnet/vibecast/tree/main/examples):

- `mcp-navigation.rs` - Navigation robot with MCP
- `mcp-vision.rs` - Computer vision integration
- `mcp-arm.rs` - Robotic arm control
- `mcp-swarm.rs` - Multi-robot coordination

Run them:
```bash
cargo run --example mcp-navigation
```

---

## 🧪 Testing

```rust
#[tokio::test]
async fn test_move_tool() {
    let server = McpServer::new("test", "1.0.0");

    server.register_tool(move_tool, move_handler).await.unwrap();

    let request = McpRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "move_to",
            "arguments": { "location": "kitchen" }
        })),
    };

    let response = server.handle_request(request).await;
    assert!(response.result.is_some());
}
```

---

## 🔗 Links

- **MCP Spec**: [modelcontextprotocol.io](https://modelcontextprotocol.io)
- **Claude Desktop**: [claude.ai/download](https://claude.ai/download)
- **Homepage**: [ruv.io](https://ruv.io)
- **Docs**: [docs.rs/agentic-robotics-mcp](https://docs.rs/agentic-robotics-mcp)
- **Repository**: [github.com/ruvnet/vibecast](https://github.com/ruvnet/vibecast)
- **Examples**: [github.com/ruvnet/vibecast/tree/main/examples](https://github.com/ruvnet/vibecast/tree/main/examples)

---

## 🤝 Contributing

Ideas for contributions:
- [ ] More example robots
- [ ] WebSocket transport
- [ ] Async tool handlers
- [ ] Tool composition
- [ ] Better error messages
- [ ] Performance optimizations

---

## 📄 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](../../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

---

<div align="center">

**Make robots accessible through natural language** 🤖

*Part of the Agentic Robotics framework - Making robotics faster, safer, and more accessible*

[Quick Start](#-quick-start-5-minutes) · [Tutorial](#-complete-tutorial) · [Examples](#-real-world-use-cases) · [Troubleshooting](#-troubleshooting)

**MCP 2025-11 Compliant** • **STDIO & SSE Transport** • **Production Ready**

</div>
