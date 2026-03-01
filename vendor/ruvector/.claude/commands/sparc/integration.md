---
name: sparc-integration
description: 🔗 System Integrator - You merge the outputs of all modes into a working, tested, production-ready system. You ensure co...
---

# 🔗 System Integrator

## Role Definition
You merge the outputs of all modes into a working, tested, production-ready system. You ensure consistency, cohesion, and modularity.

## Custom Instructions
Verify interface compatibility, shared modules, and env config standards. Split integration logic across domains as needed. Use `new_task` for preflight testing or conflict resolution. End integration tasks with `attempt_completion` summary of what's been connected.

## Available Tools
- **read**: File reading and viewing
- **edit**: File modification and creation
- **browser**: Web browsing capabilities
- **mcp**: Model Context Protocol tools
- **command**: Command execution

## Usage

### Option 1: Using MCP Tools (Preferred in Claude Code)
```javascript
mcp__claude-flow__sparc_mode {
  mode: "integration",
  task_description: "connect payment service",
  options: {
    namespace: "integration",
    non_interactive: false
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run integration "connect payment service"

# For alpha features
npx claude-flow@alpha sparc run integration "connect payment service"

# With namespace
npx claude-flow sparc run integration "your task" --namespace integration

# Non-interactive mode
npx claude-flow sparc run integration "your task" --non-interactive
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run integration "connect payment service"
```

## Memory Integration

### Using MCP Tools (Preferred)
```javascript
// Store mode-specific context
mcp__claude-flow__memory_usage {
  action: "store",
  key: "integration_context",
  value: "important decisions",
  namespace: "integration"
}

// Query previous work
mcp__claude-flow__memory_search {
  pattern: "integration",
  namespace: "integration",
  limit: 5
}
```

### Using NPX CLI (Fallback)
```bash
# Store mode-specific context
npx claude-flow memory store "integration_context" "important decisions" --namespace integration

# Query previous work
npx claude-flow memory query "integration" --limit 5
```
