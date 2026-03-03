---
name: performance-analysis
version: 1.0.0
description: Comprehensive performance analysis, bottleneck detection, and optimization recommendations for Claude Flow swarms
category: monitoring
tags: [performance, bottleneck, optimization, profiling, metrics, analysis]
author: Claude Flow Team
---

# Performance Analysis Skill

Comprehensive performance analysis suite for identifying bottlenecks, profiling swarm operations, generating detailed reports, and providing actionable optimization recommendations.

## Overview

This skill consolidates all performance analysis capabilities:
- **Bottleneck Detection**: Identify performance bottlenecks across communication, processing, memory, and network
- **Performance Profiling**: Real-time monitoring and historical analysis of swarm operations
- **Report Generation**: Create comprehensive performance reports in multiple formats
- **Optimization Recommendations**: AI-powered suggestions for improving performance

## Quick Start

### Basic Bottleneck Detection
```bash
npx claude-flow bottleneck detect
```

### Generate Performance Report
```bash
npx claude-flow analysis performance-report --format html --include-metrics
```

### Analyze and Auto-Fix
```bash
npx claude-flow bottleneck detect --fix --threshold 15
```

## Core Capabilities

### 1. Bottleneck Detection

#### Command Syntax
```bash
npx claude-flow bottleneck detect [options]
```

#### Options
- `--swarm-id, -s <id>` - Analyze specific swarm (default: current)
- `--time-range, -t <range>` - Analysis period: 1h, 24h, 7d, all (default: 1h)
- `--threshold <percent>` - Bottleneck threshold percentage (default: 20)
- `--export, -e <file>` - Export analysis to file
- `--fix` - Apply automatic optimizations

#### Usage Examples
```bash
# Basic detection for current swarm
npx claude-flow bottleneck detect

# Analyze specific swarm over 24 hours
npx claude-flow bottleneck detect --swarm-id swarm-123 -t 24h

# Export detailed analysis
npx claude-flow bottleneck detect -t 24h -e bottlenecks.json

# Auto-fix detected issues
npx claude-flow bottleneck detect --fix --threshold 15

# Low threshold for sensitive detection
npx claude-flow bottleneck detect --threshold 10 --export critical-issues.json
```

#### Metrics Analyzed

**Communication Bottlenecks:**
- Message queue delays
- Agent response times
- Coordination overhead
- Memory access patterns
- Inter-agent communication latency

**Processing Bottlenecks:**
- Task completion times
- Agent utilization rates
- Parallel execution efficiency
- Resource contention
- CPU/memory usage patterns

**Memory Bottlenecks:**
- Cache hit rates
- Memory access patterns
- Storage I/O performance
- Neural pattern loading times
- Memory allocation efficiency

**Network Bottlenecks:**
- API call latency
- MCP communication delays
- External service timeouts
- Concurrent request limits
- Network throughput issues

#### Output Format
```
🔍 Bottleneck Analysis Report
━━━━━━━━━━━━━━━━━━━━━━━━━━━

📊 Summary
├── Time Range: Last 1 hour
├── Agents Analyzed: 6
├── Tasks Processed: 42
└── Critical Issues: 2

🚨 Critical Bottlenecks
1. Agent Communication (35% impact)
   └── coordinator → coder-1 messages delayed by 2.3s avg

2. Memory Access (28% impact)
   └── Neural pattern loading taking 1.8s per access

⚠️ Warning Bottlenecks
1. Task Queue (18% impact)
   └── 5 tasks waiting > 10s for assignment

💡 Recommendations
1. Switch to hierarchical topology (est. 40% improvement)
2. Enable memory caching (est. 25% improvement)
3. Increase agent concurrency to 8 (est. 20% improvement)

✅ Quick Fixes Available
Run with --fix to apply:
- Enable smart caching
- Optimize message routing
- Adjust agent priorities
```

### 2. Performance Profiling

#### Real-time Detection
Automatic analysis during task execution:
- Execution time vs. complexity
- Agent utilization rates
- Resource constraints
- Operation patterns

#### Common Bottleneck Patterns

**Time Bottlenecks:**
- Tasks taking > 5 minutes
- Sequential operations that could parallelize
- Redundant file operations
- Inefficient algorithm implementations

**Coordination Bottlenecks:**
- Single agent for complex tasks
- Unbalanced agent workloads
- Poor topology selection
- Excessive synchronization points

**Resource Bottlenecks:**
- High operation count (> 100)
- Memory constraints
- I/O limitations
- Thread pool saturation

#### MCP Integration
```javascript
// Check for bottlenecks in Claude Code
mcp__claude-flow__bottleneck_detect({
  timeRange: "1h",
  threshold: 20,
  autoFix: false
})

// Get detailed task results with bottleneck analysis
mcp__claude-flow__task_results({
  taskId: "task-123",
  format: "detailed"
})
```

**Result Format:**
```json
{
  "bottlenecks": [
    {
      "type": "coordination",
      "severity": "high",
      "description": "Single agent used for complex task",
      "recommendation": "Spawn specialized agents for parallel work",
      "impact": "35%",
      "affectedComponents": ["coordinator", "coder-1"]
    }
  ],
  "improvements": [
    {
      "area": "execution_time",
      "suggestion": "Use parallel task execution",
      "expectedImprovement": "30-50% time reduction",
      "implementationSteps": [
        "Split task into smaller units",
        "Spawn 3-4 specialized agents",
        "Use mesh topology for coordination"
      ]
    }
  ],
  "metrics": {
    "avgExecutionTime": "142s",
    "agentUtilization": "67%",
    "cacheHitRate": "82%",
    "parallelizationFactor": 1.2
  }
}
```

### 3. Report Generation

#### Command Syntax
```bash
npx claude-flow analysis performance-report [options]
```

#### Options
- `--format <type>` - Report format: json, html, markdown (default: markdown)
- `--include-metrics` - Include detailed metrics and charts
- `--compare <id>` - Compare with previous swarm
- `--time-range <range>` - Analysis period: 1h, 24h, 7d, 30d, all
- `--output <file>` - Output file path
- `--sections <list>` - Comma-separated sections to include

#### Report Sections
1. **Executive Summary**
   - Overall performance score
   - Key metrics overview
   - Critical findings

2. **Swarm Overview**
   - Topology configuration
   - Agent distribution
   - Task statistics

3. **Performance Metrics**
   - Execution times
   - Throughput analysis
   - Resource utilization
   - Latency breakdown

4. **Bottleneck Analysis**
   - Identified bottlenecks
   - Impact assessment
   - Optimization priorities

5. **Comparative Analysis** (when --compare used)
   - Performance trends
   - Improvement metrics
   - Regression detection

6. **Recommendations**
   - Prioritized action items
   - Expected improvements
   - Implementation guidance

#### Usage Examples
```bash
# Generate HTML report with all metrics
npx claude-flow analysis performance-report --format html --include-metrics

# Compare current swarm with previous
npx claude-flow analysis performance-report --compare swarm-123 --format markdown

# Custom output with specific sections
npx claude-flow analysis performance-report \
  --sections summary,metrics,recommendations \
  --output reports/perf-analysis.html \
  --format html

# Weekly performance report
npx claude-flow analysis performance-report \
  --time-range 7d \
  --include-metrics \
  --format markdown \
  --output docs/weekly-performance.md

# JSON format for CI/CD integration
npx claude-flow analysis performance-report \
  --format json \
  --output build/performance.json
```

#### Sample Markdown Report
```markdown
# Performance Analysis Report

## Executive Summary
- **Overall Score**: 87/100
- **Analysis Period**: Last 24 hours
- **Swarms Analyzed**: 3
- **Critical Issues**: 1

## Key Metrics
| Metric | Value | Trend | Target |
|--------|-------|-------|--------|
| Avg Task Time | 42s | ↓ 12% | 35s |
| Agent Utilization | 78% | ↑ 5% | 85% |
| Cache Hit Rate | 91% | → | 90% |
| Parallel Efficiency | 2.3x | ↑ 0.4x | 2.5x |

## Bottleneck Analysis
### Critical
1. **Agent Communication Delay** (Impact: 35%)
   - Coordinator → Coder messages delayed by 2.3s avg
   - **Fix**: Switch to hierarchical topology

### Warnings
1. **Memory Access Pattern** (Impact: 18%)
   - Neural pattern loading: 1.8s per access
   - **Fix**: Enable memory caching

## Recommendations
1. **High Priority**: Switch to hierarchical topology (40% improvement)
2. **Medium Priority**: Enable memory caching (25% improvement)
3. **Low Priority**: Increase agent concurrency to 8 (20% improvement)
```

### 4. Optimization Recommendations

#### Automatic Fixes
When using `--fix`, the following optimizations may be applied:

**1. Topology Optimization**
- Switch to more efficient topology (mesh → hierarchical)
- Adjust communication patterns
- Reduce coordination overhead
- Optimize message routing

**2. Caching Enhancement**
- Enable memory caching
- Optimize cache strategies
- Preload common patterns
- Implement cache warming

**3. Concurrency Tuning**
- Adjust agent counts
- Optimize parallel execution
- Balance workload distribution
- Implement load balancing

**4. Priority Adjustment**
- Reorder task queues
- Prioritize critical paths
- Reduce wait times
- Implement fair scheduling

**5. Resource Optimization**
- Optimize memory usage
- Reduce I/O operations
- Batch API calls
- Implement connection pooling

#### Performance Impact
Typical improvements after bottleneck resolution:

- **Communication**: 30-50% faster message delivery
- **Processing**: 20-40% reduced task completion time
- **Memory**: 40-60% fewer cache misses
- **Network**: 25-45% reduced API latency
- **Overall**: 25-45% total performance improvement

## Advanced Usage

### Continuous Monitoring
```bash
# Monitor performance in real-time
npx claude-flow swarm monitor --interval 5

# Generate hourly reports
while true; do
  npx claude-flow analysis performance-report \
    --format json \
    --output logs/perf-$(date +%Y%m%d-%H%M).json
  sleep 3600
done
```

### CI/CD Integration
```yaml
# .github/workflows/performance.yml
name: Performance Analysis
on: [push, pull_request]

jobs:
  analyze:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run Performance Analysis
        run: |
          npx claude-flow analysis performance-report \
            --format json \
            --output performance.json
      - name: Check Performance Thresholds
        run: |
          npx claude-flow bottleneck detect \
            --threshold 15 \
            --export bottlenecks.json
      - name: Upload Reports
        uses: actions/upload-artifact@v2
        with:
          name: performance-reports
          path: |
            performance.json
            bottlenecks.json
```

### Custom Analysis Scripts
```javascript
// scripts/analyze-performance.js
const { exec } = require('child_process');
const fs = require('fs');

async function analyzePerformance() {
  // Run bottleneck detection
  const bottlenecks = await runCommand(
    'npx claude-flow bottleneck detect --format json'
  );

  // Generate performance report
  const report = await runCommand(
    'npx claude-flow analysis performance-report --format json'
  );

  // Analyze results
  const analysis = {
    bottlenecks: JSON.parse(bottlenecks),
    performance: JSON.parse(report),
    timestamp: new Date().toISOString()
  };

  // Save combined analysis
  fs.writeFileSync(
    'analysis/combined-report.json',
    JSON.stringify(analysis, null, 2)
  );

  // Generate alerts if needed
  if (analysis.bottlenecks.critical.length > 0) {
    console.error('CRITICAL: Performance bottlenecks detected!');
    process.exit(1);
  }
}

function runCommand(cmd) {
  return new Promise((resolve, reject) => {
    exec(cmd, (error, stdout, stderr) => {
      if (error) reject(error);
      else resolve(stdout);
    });
  });
}

analyzePerformance().catch(console.error);
```

## Best Practices

### 1. Regular Analysis
- Run bottleneck detection after major changes
- Generate weekly performance reports
- Monitor trends over time
- Set up automated alerts

### 2. Threshold Tuning
- Start with default threshold (20%)
- Lower for production systems (10-15%)
- Higher for development (25-30%)
- Adjust based on requirements

### 3. Fix Strategy
- Always review before applying --fix
- Test fixes in development first
- Apply fixes incrementally
- Monitor impact after changes

### 4. Report Integration
- Include in documentation
- Share with team regularly
- Track improvements over time
- Use for capacity planning

### 5. Continuous Optimization
- Learn from each analysis
- Build performance budgets
- Establish baselines
- Set improvement goals

## Troubleshooting

### Common Issues

**High Memory Usage**
```bash
# Analyze memory bottlenecks
npx claude-flow bottleneck detect --threshold 10

# Check cache performance
npx claude-flow cache manage --action stats

# Review memory metrics
npx claude-flow memory usage
```

**Slow Task Execution**
```bash
# Identify slow tasks
npx claude-flow task status --detailed

# Analyze coordination overhead
npx claude-flow bottleneck detect --time-range 1h

# Check agent utilization
npx claude-flow agent metrics
```

**Poor Cache Performance**
```bash
# Analyze cache hit rates
npx claude-flow analysis performance-report --sections metrics

# Review cache strategy
npx claude-flow cache manage --action analyze

# Enable cache warming
npx claude-flow bottleneck detect --fix
```

## Integration with Other Skills

- **swarm-orchestration**: Use performance data to optimize topology
- **memory-management**: Improve cache strategies based on analysis
- **task-coordination**: Adjust scheduling based on bottlenecks
- **neural-training**: Train patterns from performance data

## Related Commands

- `npx claude-flow swarm monitor` - Real-time monitoring
- `npx claude-flow token usage` - Token optimization analysis
- `npx claude-flow cache manage` - Cache optimization
- `npx claude-flow agent metrics` - Agent performance metrics
- `npx claude-flow task status` - Task execution analysis

## See Also

- [Bottleneck Detection Guide](/workspaces/claude-code-flow/.claude/commands/analysis/bottleneck-detect.md)
- [Performance Report Guide](/workspaces/claude-code-flow/.claude/commands/analysis/performance-report.md)
- [Performance Bottlenecks Overview](/workspaces/claude-code-flow/.claude/commands/analysis/performance-bottlenecks.md)
- [Swarm Monitoring Documentation](../swarm-orchestration/SKILL.md)
- [Memory Management Documentation](../memory-management/SKILL.md)

---

**Version**: 1.0.0
**Last Updated**: 2025-10-19
**Maintainer**: Claude Flow Team
