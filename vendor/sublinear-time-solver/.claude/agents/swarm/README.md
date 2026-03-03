---
name: Swarm Coordination
type: documentation
category: swarm
description: Specialized swarm coordination agents for claude-code-flow hive-mind system with different topologies
---

# Swarm Coordination Agents

This directory contains specialized swarm coordination agents designed to work with the claude-code-flow hive-mind system. Each agent implements a different coordination topology and strategy.

## Available Agents

### 1. Hierarchical Coordinator (`hierarchical-coordinator.md`)
**Architecture**: Queen-led hierarchy with specialized workers
- **Use Cases**: Complex projects requiring central coordination
- **Strengths**: Clear command structure, efficient resource allocation
- **Best For**: Large-scale development, multi-team coordination

### 2. Mesh Coordinator (`mesh-coordinator.md`) 
**Architecture**: Peer-to-peer distributed network
- **Use Cases**: Fault-tolerant distributed processing
- **Strengths**: High resilience, no single point of failure
- **Best For**: Critical systems, high-availability requirements

### 3. Adaptive Coordinator (`adaptive-coordinator.md`)
**Architecture**: Dynamic topology switching with ML optimization
- **Use Cases**: Variable workloads requiring optimization
- **Strengths**: Self-optimizing, learns from experience
- **Best For**: Production systems, long-running processes

## Coordination Patterns

### Topology Comparison

| Feature | Hierarchical | Mesh | Adaptive |
|---------|-------------|------|----------|
| **Fault Tolerance** | Medium | High | High |
| **Scalability** | High | Medium | High |
| **Coordination Overhead** | Low | High | Variable |
| **Learning Capability** | Low | Low | High |
| **Setup Complexity** | Low | High | Medium |
| **Best Use Case** | Structured projects | Critical systems | Variable workloads |

### Performance Characteristics

```
Hierarchical: ⭐⭐⭐⭐⭐ Coordination Efficiency
              ⭐⭐⭐⭐   Fault Tolerance  
              ⭐⭐⭐⭐⭐ Scalability

Mesh:         ⭐⭐⭐     Coordination Efficiency
              ⭐⭐⭐⭐⭐ Fault Tolerance
              ⭐⭐⭐     Scalability

Adaptive:     ⭐⭐⭐⭐⭐ Coordination Efficiency  
              ⭐⭐⭐⭐⭐ Fault Tolerance
              ⭐⭐⭐⭐⭐ Scalability
```

## MCP Tool Integration

All swarm coordinators leverage the following MCP tools:

### Core Coordination Tools
- `mcp__claude-flow__swarm_init` - Initialize swarm topology
- `mcp__claude-flow__agent_spawn` - Create specialized worker agents  
- `mcp__claude-flow__task_orchestrate` - Coordinate complex workflows
- `mcp__claude-flow__swarm_monitor` - Real-time performance monitoring

### Advanced Features
- `mcp__claude-flow__neural_patterns` - Pattern recognition and learning
- `mcp__claude-flow__daa_consensus` - Distributed decision making
- `mcp__claude-flow__topology_optimize` - Dynamic topology optimization
- `mcp__claude-flow__performance_report` - Comprehensive analytics

## Usage Examples

### Hierarchical Coordination
```bash
# Initialize hierarchical swarm for development project
claude-flow agent spawn hierarchical-coordinator "Build authentication microservice"

# Agents will automatically:
# 1. Decompose project into tasks
# 2. Spawn specialized workers (research, code, test, docs)
# 3. Coordinate execution with central oversight
# 4. Generate comprehensive reports
```

### Mesh Coordination  
```bash
# Initialize mesh network for distributed processing
claude-flow agent spawn mesh-coordinator "Process user analytics data"

# Network will automatically:
# 1. Establish peer-to-peer connections
# 2. Distribute work across available nodes
# 3. Handle node failures gracefully
# 4. Maintain consensus on results
```

### Adaptive Coordination
```bash
# Initialize adaptive swarm for production optimization
claude-flow agent spawn adaptive-coordinator "Optimize system performance"

# System will automatically:
# 1. Analyze current workload patterns
# 2. Select optimal topology (hierarchical/mesh/ring)
# 3. Learn from performance outcomes
# 4. Continuously adapt to changing conditions
```

## Architecture Decision Framework

### When to Use Hierarchical
- ✅ Well-defined project structure
- ✅ Clear resource hierarchy 
- ✅ Need for centralized decision making
- ✅ Large team coordination required
- ❌ High fault tolerance critical
- ❌ Network partitioning likely

### When to Use Mesh
- ✅ High availability requirements
- ✅ Distributed processing needs
- ✅ Network reliability concerns
- ✅ Peer collaboration model
- ❌ Simple coordination sufficient
- ❌ Resource constraints exist

### When to Use Adaptive
- ✅ Variable workload patterns
- ✅ Long-running production systems
- ✅ Performance optimization critical
- ✅ Machine learning acceptable
- ❌ Predictable, stable workloads
- ❌ Simple requirements

## Performance Monitoring

Each coordinator provides comprehensive metrics:

### Key Performance Indicators
- **Task Completion Rate**: Percentage of successful task completion
- **Agent Utilization**: Efficiency of resource usage
- **Coordination Overhead**: Communication and management costs
- **Fault Recovery Time**: Speed of recovery from failures
- **Learning Convergence**: Adaptation effectiveness (adaptive only)

### Monitoring Dashboards
Real-time visibility into:
- Swarm topology and agent status
- Task queues and execution pipelines  
- Performance metrics and trends
- Error rates and failure patterns
- Resource utilization and capacity

## Best Practices

### Design Principles
1. **Start Simple**: Begin with hierarchical for well-understood problems
2. **Scale Gradually**: Add complexity as requirements grow
3. **Monitor Continuously**: Track performance and adapt strategies
4. **Plan for Failure**: Design fault tolerance from the beginning

### Operational Guidelines
1. **Agent Sizing**: Right-size swarms for workload (5-15 agents typical)
2. **Resource Planning**: Ensure adequate compute/memory for coordination overhead
3. **Network Design**: Consider latency and bandwidth for distributed topologies
4. **Security**: Implement proper authentication and authorization

### Troubleshooting
- **Poor Performance**: Check agent capability matching and load distribution
- **Coordination Failures**: Verify network connectivity and consensus thresholds
- **Resource Exhaustion**: Monitor and scale agent pools proactively
- **Learning Issues**: Validate training data quality and model convergence

## Integration with Claude-Flow

These agents integrate seamlessly with the broader claude-flow ecosystem:

- **Memory System**: All coordination state persisted in claude-flow memory bank
- **Terminal Management**: Agents can spawn and manage multiple terminal sessions
- **MCP Integration**: Full access to claude-flow's MCP tool ecosystem
- **Event System**: Real-time coordination through claude-flow event bus
- **Configuration**: Managed through claude-flow configuration system

For implementation details, see individual agent files and the claude-flow documentation.