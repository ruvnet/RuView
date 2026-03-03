# Coordinate Task Execution

## 🎯 Key Principle
**This tool coordinates Claude Code's actions. It does NOT write code or create content.**

## MCP Tool Usage in Claude Code

**Tool:** `mcp__claude-flow__task_orchestrate`

## Parameters
```json
{"task": "Implement authentication system", "strategy": "parallel", "priority": "high"}
```

## Description
Break down and coordinate complex tasks for systematic execution by Claude Code

## Details
Orchestration strategies:
- **parallel**: Claude Code works on independent components simultaneously
- **sequential**: Step-by-step execution for dependent tasks
- **adaptive**: Dynamically adjusts based on task complexity

The orchestrator creates a plan that Claude Code follows using its native tools.

## Example Usage

**In Claude Code:**
1. Use the tool: `mcp__claude-flow__task_orchestrate`
2. With parameters: `{"task": "Implement authentication system", "strategy": "parallel", "priority": "high"}`
3. Claude Code then executes the coordinated plan using its native tools

## Important Reminders
- ✅ This tool provides coordination and structure
- ✅ Claude Code performs all actual implementation
- ❌ The tool does NOT write code
- ❌ The tool does NOT access files directly
- ❌ The tool does NOT execute commands

## See Also
- Main documentation: /claude.md
- Other commands in this category
- Workflow examples in /workflows/
