# bottleneck detect

Analyze performance bottlenecks in swarm operations and suggest optimizations.

## Usage

```bash
npx claude-flow bottleneck detect [options]
```

## Options

- `--swarm-id, -s <id>` - Analyze specific swarm (default: current)
- `--time-range, -t <range>` - Analysis period: 1h, 24h, 7d, all (default: 1h)
- `--threshold <percent>` - Bottleneck threshold percentage (default: 20)
- `--export, -e <file>` - Export analysis to file
- `--fix` - Apply automatic optimizations

## Examples

### Basic bottleneck detection

```bash
npx claude-flow bottleneck detect
```

### Analyze specific swarm

```bash
npx claude-flow bottleneck detect --swarm-id swarm-123
```

### Last 24 hours with export

```bash
npx claude-flow bottleneck detect -t 24h -e bottlenecks.json
```

### Auto-fix detected issues

```bash
npx claude-flow bottleneck detect --fix --threshold 15
```

## Metrics Analyzed

### Communication Bottlenecks

- Message queue delays
- Agent response times
- Coordination overhead
- Memory access patterns

### Processing Bottlenecks

- Task completion times
- Agent utilization rates
- Parallel execution efficiency
- Resource contention

### Memory Bottlenecks

- Cache hit rates
- Memory access patterns
- Storage I/O performance
- Neural pattern loading

### Network Bottlenecks

- API call latency
- MCP communication delays
- External service timeouts
- Concurrent request limits

## Output Format

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

## Automatic Fixes

When using `--fix`, the following optimizations may be applied:

1. **Topology Optimization**

   - Switch to more efficient topology
   - Adjust communication patterns
   - Reduce coordination overhead

2. **Caching Enhancement**

   - Enable memory caching
   - Optimize cache strategies
   - Preload common patterns

3. **Concurrency Tuning**

   - Adjust agent counts
   - Optimize parallel execution
   - Balance workload distribution

4. **Priority Adjustment**
   - Reorder task queues
   - Prioritize critical paths
   - Reduce wait times

## Performance Impact

Typical improvements after bottleneck resolution:

- **Communication**: 30-50% faster message delivery
- **Processing**: 20-40% reduced task completion time
- **Memory**: 40-60% fewer cache misses
- **Overall**: 25-45% performance improvement

## Integration with Claude Code

```javascript
// Check for bottlenecks in Claude Code
mcp__claude-flow__bottleneck_detect {
  timeRange: "1h",
  threshold: 20,
  autoFix: false
}
```

## See Also

- `performance report` - Detailed performance analysis
- `token usage` - Token optimization analysis
- `swarm monitor` - Real-time monitoring
- `cache manage` - Cache optimization
