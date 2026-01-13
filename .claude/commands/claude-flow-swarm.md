---
name: claude-flow-swarm
description: Coordinate multi-agent swarms for complex tasks
---

# 🐝 Claude-Flow Swarm Coordination

Advanced multi-agent coordination system with timeout-free execution, distributed memory sharing, and intelligent load balancing.

## Basic Usage
```bash
./claude-flow swarm "your complex task" --strategy <type> [options]
```

## 🎯 Swarm Strategies
- **auto** - Automatic strategy selection based on task analysis
- **development** - Code implementation with review and testing
- **research** - Information gathering and synthesis
- **analysis** - Data processing and pattern identification
- **testing** - Comprehensive quality assurance
- **optimization** - Performance tuning and refactoring
- **maintenance** - System updates and bug fixes

## 🤖 Agent Types
- **coordinator** - Plans and delegates tasks to other agents
- **developer** - Writes code and implements solutions
- **researcher** - Gathers and analyzes information
- **analyzer** - Identifies patterns and generates insights
- **tester** - Creates and runs tests for quality assurance
- **reviewer** - Performs code and design reviews
- **documenter** - Creates documentation and guides
- **monitor** - Tracks performance and system health
- **specialist** - Domain-specific expert agents

## 🔄 Coordination Modes
- **centralized** - Single coordinator manages all agents (default)
- **distributed** - Multiple coordinators share management
- **hierarchical** - Tree structure with nested coordination
- **mesh** - Peer-to-peer agent collaboration
- **hybrid** - Mixed coordination strategies

## ⚙️ Common Options
- `--strategy <type>` - Execution strategy
- `--mode <type>` - Coordination mode
- `--max-agents <n>` - Maximum concurrent agents (default: 5)
- `--timeout <minutes>` - Timeout in minutes (default: 60)
- `--background` - Run in background for tasks > 30 minutes
- `--monitor` - Enable real-time monitoring
- `--ui` - Launch terminal UI interface
- `--parallel` - Enable parallel execution
- `--distributed` - Enable distributed coordination
- `--review` - Enable peer review process
- `--testing` - Include automated testing
- `--encryption` - Enable data encryption
- `--verbose` - Detailed logging output
- `--dry-run` - Show configuration without executing

## 🌟 Examples

### Development Swarm with Review
```bash
./claude-flow swarm "Build e-commerce REST API" \
  --strategy development \
  --monitor \
  --review \
  --testing
```

### Long-Running Research Swarm
```bash
./claude-flow swarm "Analyze AI market trends 2024-2025" \
  --strategy research \
  --background \
  --distributed \
  --max-agents 8
```

### Performance Optimization Swarm
```bash
./claude-flow swarm "Optimize database queries and API performance" \
  --strategy optimization \
  --testing \
  --parallel \
  --monitor
```

### Enterprise Development Swarm
```bash
./claude-flow swarm "Implement secure payment processing system" \
  --strategy development \
  --mode distributed \
  --max-agents 10 \
  --parallel \
  --monitor \
  --review \
  --testing \
  --encryption \
  --verbose
```

### Testing and QA Swarm
```bash
./claude-flow swarm "Comprehensive security audit and testing" \
  --strategy testing \
  --review \
  --verbose \
  --max-agents 6
```

## 📊 Monitoring and Control

### Real-time monitoring:
```bash
# Monitor swarm activity
./claude-flow monitor

# Monitor specific component
./claude-flow monitor --focus swarm
```

### Check swarm status:
```bash
# Overall system status
./claude-flow status

# Detailed swarm status
./claude-flow status --verbose
```

### View agent activity:
```bash
# List all agents
./claude-flow agent list

# Agent details
./claude-flow agent info <agent-id>
```

## 💾 Memory Integration

Swarms automatically use distributed memory for collaboration:

```bash
# Store swarm objectives
./claude-flow memory store "swarm_objective" "Build scalable API" --namespace swarm

# Query swarm progress
./claude-flow memory query "swarm_progress" --namespace swarm

# Export swarm memory
./claude-flow memory export swarm-results.json --namespace swarm
```

## 🎯 Key Features

### Timeout-Free Execution
- Background mode for long-running tasks
- State persistence across sessions
- Automatic checkpoint recovery

### Work Stealing & Load Balancing
- Dynamic task redistribution
- Automatic agent scaling
- Resource-aware scheduling

### Circuit Breakers & Fault Tolerance
- Automatic retry with exponential backoff
- Graceful degradation
- Health monitoring and recovery

### Real-Time Collaboration
- Cross-agent communication
- Shared memory access
- Event-driven coordination

### Enterprise Security
- Role-based access control
- Audit logging
- Data encryption
- Input validation

## 🔧 Advanced Configuration

### Dry run to preview:
```bash
./claude-flow swarm "Test task" --dry-run --strategy development
```

### Custom quality thresholds:
```bash
./claude-flow swarm "High quality API" \
  --strategy development \
  --quality-threshold 0.95
```

### Scheduling algorithms:
- FIFO (First In, First Out)
- Priority-based
- Deadline-driven
- Shortest Job First
- Critical Path
- Resource-aware
- Adaptive

For detailed documentation, see: https://github.com/ruvnet/claude-code-flow/docs/swarm-system.md
