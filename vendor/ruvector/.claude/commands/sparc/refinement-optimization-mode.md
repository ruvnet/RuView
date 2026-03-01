---
name: sparc-refinement-optimization-mode
description: 🧹 Optimizer - You refactor, modularize, and improve system performance. You enforce file size limits, dependenc...
---

# 🧹 Optimizer

## Role Definition
You refactor, modularize, and improve system performance. You enforce file size limits, dependency decoupling, and configuration hygiene.

## Custom Instructions
Audit files for clarity, modularity, and size. Break large components (>500 lines) into smaller ones. Move inline configs to env files. Optimize performance or structure. Use `new_task` to delegate changes and finalize with `attempt_completion`.

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
  mode: "refinement-optimization-mode",
  task_description: "optimize database queries",
  options: {
    namespace: "refinement-optimization-mode",
    non_interactive: false
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run refinement-optimization-mode "optimize database queries"

# For alpha features
npx claude-flow@alpha sparc run refinement-optimization-mode "optimize database queries"

# With namespace
npx claude-flow sparc run refinement-optimization-mode "your task" --namespace refinement-optimization-mode

# Non-interactive mode
npx claude-flow sparc run refinement-optimization-mode "your task" --non-interactive
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run refinement-optimization-mode "optimize database queries"
```

## Memory Integration

### Using MCP Tools (Preferred)
```javascript
// Store mode-specific context
mcp__claude-flow__memory_usage {
  action: "store",
  key: "refinement-optimization-mode_context",
  value: "important decisions",
  namespace: "refinement-optimization-mode"
}

// Query previous work
mcp__claude-flow__memory_search {
  pattern: "refinement-optimization-mode",
  namespace: "refinement-optimization-mode",
  limit: 5
}
```

### Using NPX CLI (Fallback)
```bash
# Store mode-specific context
npx claude-flow memory store "refinement-optimization-mode_context" "important decisions" --namespace refinement-optimization-mode

# Query previous work
npx claude-flow memory query "refinement-optimization-mode" --limit 5
```
