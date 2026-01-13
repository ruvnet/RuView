---
name: claude-flow-help
description: Show Claude-Flow commands and usage
---

# Claude-Flow Commands

## 🌊 Claude-Flow: Agent Orchestration Platform

Claude-Flow is the ultimate multi-terminal orchestration platform that revolutionizes how you work with Claude Code.

## Core Commands

### 🚀 System Management
- `./claude-flow start` - Start orchestration system
- `./claude-flow start --ui` - Start with interactive process management UI
- `./claude-flow status` - Check system status
- `./claude-flow monitor` - Real-time monitoring
- `./claude-flow stop` - Stop orchestration

### 🤖 Agent Management
- `./claude-flow agent spawn <type>` - Create new agent
- `./claude-flow agent list` - List active agents
- `./claude-flow agent info <id>` - Agent details
- `./claude-flow agent terminate <id>` - Stop agent

### 📋 Task Management
- `./claude-flow task create <type> "description"` - Create task
- `./claude-flow task list` - List all tasks
- `./claude-flow task status <id>` - Task status
- `./claude-flow task cancel <id>` - Cancel task
- `./claude-flow task workflow <file>` - Execute workflow

### 🧠 Memory Operations
- `./claude-flow memory store "key" "value"` - Store data
- `./claude-flow memory query "search"` - Search memory
- `./claude-flow memory stats` - Memory statistics
- `./claude-flow memory export <file>` - Export memory
- `./claude-flow memory import <file>` - Import memory

### ⚡ SPARC Development
- `./claude-flow sparc "task"` - Run SPARC orchestrator
- `./claude-flow sparc modes` - List all 17+ SPARC modes
- `./claude-flow sparc run <mode> "task"` - Run specific mode
- `./claude-flow sparc tdd "feature"` - TDD workflow
- `./claude-flow sparc info <mode>` - Mode details

### 🐝 Swarm Coordination
- `./claude-flow swarm "task" --strategy <type>` - Start swarm
- `./claude-flow swarm "task" --background` - Long-running swarm
- `./claude-flow swarm "task" --monitor` - With monitoring
- `./claude-flow swarm "task" --ui` - Interactive UI
- `./claude-flow swarm "task" --distributed` - Distributed coordination

### 🌍 MCP Integration
- `./claude-flow mcp status` - MCP server status
- `./claude-flow mcp tools` - List available tools
- `./claude-flow mcp config` - Show configuration
- `./claude-flow mcp logs` - View MCP logs

### 🤖 Claude Integration
- `./claude-flow claude spawn "task"` - Spawn Claude with enhanced guidance
- `./claude-flow claude batch <file>` - Execute workflow configuration

## 🌟 Quick Examples

### Initialize with SPARC:
```bash
npx -y claude-flow@latest init --sparc
```

### Start a development swarm:
```bash
./claude-flow swarm "Build REST API" --strategy development --monitor --review
```

### Run TDD workflow:
```bash
./claude-flow sparc tdd "user authentication"
```

### Store project context:
```bash
./claude-flow memory store "project_requirements" "e-commerce platform specs" --namespace project
```

### Spawn specialized agents:
```bash
./claude-flow agent spawn researcher --name "Senior Researcher" --priority 8
./claude-flow agent spawn developer --name "Lead Developer" --priority 9
```

## 🎯 Best Practices
- Use `./claude-flow` instead of `npx claude-flow` after initialization
- Store important context in memory for cross-session persistence
- Use swarm mode for complex tasks requiring multiple agents
- Enable monitoring for real-time progress tracking
- Use background mode for tasks > 30 minutes

## 📚 Resources
- Documentation: https://github.com/ruvnet/claude-code-flow/docs
- Examples: https://github.com/ruvnet/claude-code-flow/examples
- Issues: https://github.com/ruvnet/claude-code-flow/issues
