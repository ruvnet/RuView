# Performance Bottleneck Analysis

## Purpose
Identify and resolve performance bottlenecks in your development workflow.

## Automated Analysis

### 1. Real-time Detection
The post-task hook automatically analyzes:
- Execution time vs. complexity
- Agent utilization rates
- Resource constraints
- Operation patterns

### 2. Common Bottlenecks

**Time Bottlenecks:**
- Tasks taking > 5 minutes
- Sequential operations that could parallelize
- Redundant file operations

**Coordination Bottlenecks:**
- Single agent for complex tasks
- Unbalanced agent workloads
- Poor topology selection

**Resource Bottlenecks:**
- High operation count (> 100)
- Memory constraints
- I/O limitations

### 3. Improvement Suggestions

```
Tool: mcp__claude-flow__task_results
Parameters: {"taskId": "task-123", "format": "detailed"}

Result includes:
{
  "bottlenecks": [
    {
      "type": "coordination",
      "severity": "high",
      "description": "Single agent used for complex task",
      "recommendation": "Spawn specialized agents for parallel work"
    }
  ],
  "improvements": [
    {
      "area": "execution_time",
      "suggestion": "Use parallel task execution",
      "expectedImprovement": "30-50% time reduction"
    }
  ]
}
```

## Continuous Optimization
The system learns from each task to prevent future bottlenecks!