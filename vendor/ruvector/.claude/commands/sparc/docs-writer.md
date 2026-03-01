---
name: sparc-docs-writer
description: 📚 Documentation Writer - You write concise, clear, and modular Markdown documentation that explains usage, integration, se...
---

# 📚 Documentation Writer

## Role Definition
You write concise, clear, and modular Markdown documentation that explains usage, integration, setup, and configuration.

## Custom Instructions
Only work in .md files. Use sections, examples, and headings. Keep each file under 500 lines. Do not leak env values. Summarize what you wrote using `attempt_completion`. Delegate large guides with `new_task`.

## Available Tools
- **read**: File reading and viewing
- **edit**: Markdown files only (Files matching: \.md$)

## Usage

### Option 1: Using MCP Tools (Preferred in Claude Code)
```javascript
mcp__claude-flow__sparc_mode {
  mode: "docs-writer",
  task_description: "create API documentation",
  options: {
    namespace: "docs-writer",
    non_interactive: false
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run docs-writer "create API documentation"

# For alpha features
npx claude-flow@alpha sparc run docs-writer "create API documentation"

# With namespace
npx claude-flow sparc run docs-writer "your task" --namespace docs-writer

# Non-interactive mode
npx claude-flow sparc run docs-writer "your task" --non-interactive
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run docs-writer "create API documentation"
```

## Memory Integration

### Using MCP Tools (Preferred)
```javascript
// Store mode-specific context
mcp__claude-flow__memory_usage {
  action: "store",
  key: "docs-writer_context",
  value: "important decisions",
  namespace: "docs-writer"
}

// Query previous work
mcp__claude-flow__memory_search {
  pattern: "docs-writer",
  namespace: "docs-writer",
  limit: 5
}
```

### Using NPX CLI (Fallback)
```bash
# Store mode-specific context
npx claude-flow memory store "docs-writer_context" "important decisions" --namespace docs-writer

# Query previous work
npx claude-flow memory query "docs-writer" --limit 5
```
