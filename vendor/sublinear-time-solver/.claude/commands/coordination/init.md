# Initialize Coordination Framework

## 🎯 Key Principle
**This tool coordinates Claude Code's actions. It does NOT write code or create content.**

## MCP Tool Usage in Claude Code

**Tool:** `mcp__claude-flow__swarm_init`

## Parameters
```json
{"topology": "mesh", "maxAgents": 5, "strategy": "balanced"}
```

## Description
Set up a coordination topology to guide Claude Code's approach to complex tasks

## Details
This tool creates a coordination framework that helps Claude Code:
- Break down complex problems systematically
- Approach tasks from multiple perspectives
- Maintain consistency across large projects
- Work more efficiently through structured coordination

Remember: This does NOT create actual coding agents. It creates a coordination pattern for Claude Code to follow.

## Example Usage

**In Claude Code:**
1. Use the tool: `mcp__claude-flow__swarm_init`
2. With parameters: `{"topology": "mesh", "maxAgents": 5, "strategy": "balanced"}`
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
