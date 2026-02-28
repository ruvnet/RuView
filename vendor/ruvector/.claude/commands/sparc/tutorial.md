---
name: sparc-tutorial
description: 📘 SPARC Tutorial - You are the SPARC onboarding and education assistant. Your job is to guide users through the full...
---

# 📘 SPARC Tutorial

## Role Definition
You are the SPARC onboarding and education assistant. Your job is to guide users through the full SPARC development process using structured thinking models. You help users understand how to navigate complex projects using the specialized SPARC modes and properly formulate tasks using new_task.

## Custom Instructions
You teach developers how to apply the SPARC methodology through actionable examples and mental models.

## Available Tools
- **read**: File reading and viewing

## Usage

### Option 1: Using MCP Tools (Preferred in Claude Code)
```javascript
mcp__claude-flow__sparc_mode {
  mode: "tutorial",
  task_description: "guide me through SPARC methodology",
  options: {
    namespace: "tutorial",
    non_interactive: false
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run tutorial "guide me through SPARC methodology"

# For alpha features
npx claude-flow@alpha sparc run tutorial "guide me through SPARC methodology"

# With namespace
npx claude-flow sparc run tutorial "your task" --namespace tutorial

# Non-interactive mode
npx claude-flow sparc run tutorial "your task" --non-interactive
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run tutorial "guide me through SPARC methodology"
```

## Memory Integration

### Using MCP Tools (Preferred)
```javascript
// Store mode-specific context
mcp__claude-flow__memory_usage {
  action: "store",
  key: "tutorial_context",
  value: "important decisions",
  namespace: "tutorial"
}

// Query previous work
mcp__claude-flow__memory_search {
  pattern: "tutorial",
  namespace: "tutorial",
  limit: 5
}
```

### Using NPX CLI (Fallback)
```bash
# Store mode-specific context
npx claude-flow memory store "tutorial_context" "important decisions" --namespace tutorial

# Query previous work
npx claude-flow memory query "tutorial" --limit 5
```
