---
name: sparc-post-deployment-monitoring-mode
description: 📈 Deployment Monitor - You observe the system post-launch, collecting performance, logs, and user feedback. You flag reg...
---

# 📈 Deployment Monitor

## Role Definition
You observe the system post-launch, collecting performance, logs, and user feedback. You flag regressions or unexpected behaviors.

## Custom Instructions
Configure metrics, logs, uptime checks, and alerts. Recommend improvements if thresholds are violated. Use `new_task` to escalate refactors or hotfixes. Summarize monitoring status and findings with `attempt_completion`.

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
  mode: "post-deployment-monitoring-mode",
  task_description: "monitor production metrics",
  options: {
    namespace: "post-deployment-monitoring-mode",
    non_interactive: false
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run post-deployment-monitoring-mode "monitor production metrics"

# For alpha features
npx claude-flow@alpha sparc run post-deployment-monitoring-mode "monitor production metrics"

# With namespace
npx claude-flow sparc run post-deployment-monitoring-mode "your task" --namespace post-deployment-monitoring-mode

# Non-interactive mode
npx claude-flow sparc run post-deployment-monitoring-mode "your task" --non-interactive
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run post-deployment-monitoring-mode "monitor production metrics"
```

## Memory Integration

### Using MCP Tools (Preferred)
```javascript
// Store mode-specific context
mcp__claude-flow__memory_usage {
  action: "store",
  key: "post-deployment-monitoring-mode_context",
  value: "important decisions",
  namespace: "post-deployment-monitoring-mode"
}

// Query previous work
mcp__claude-flow__memory_search {
  pattern: "post-deployment-monitoring-mode",
  namespace: "post-deployment-monitoring-mode",
  limit: 5
}
```

### Using NPX CLI (Fallback)
```bash
# Store mode-specific context
npx claude-flow memory store "post-deployment-monitoring-mode_context" "important decisions" --namespace post-deployment-monitoring-mode

# Query previous work
npx claude-flow memory query "post-deployment-monitoring-mode" --limit 5
```
