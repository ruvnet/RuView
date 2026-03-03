# 📊 Truth Command

View truth scores and reliability metrics for your codebase and agent tasks.

## Overview

The `truth` command provides comprehensive insights into code quality, agent performance, and verification metrics.

## Usage

```bash
claude-flow truth [options]
```

## Options

- `--format <type>` - Output format: table (default), json, csv, html
- `--period <time>` - Time period: 1h, 24h, 7d, 30d
- `--agent <name>` - Filter by specific agent
- `--threshold <0-1>` - Show only scores below threshold
- `--export <file>` - Export metrics to file
- `--watch` - Real-time monitoring mode

## Metrics Displayed

### Truth Scores
- **Overall Score**: Aggregate truth score (0.0-1.0)
- **File Scores**: Individual file truth ratings
- **Agent Scores**: Per-agent reliability metrics
- **Task Scores**: Task completion quality

### Trends
- **Improvement Rate**: Quality trend over time
- **Regression Detection**: Identifies declining scores
- **Agent Learning**: Shows agent improvement curves

### Statistics
- **Mean Score**: Average truth score
- **Median Score**: Middle value of scores
- **Standard Deviation**: Score consistency
- **Confidence Interval**: Statistical reliability

## Examples

### Basic Usage
```bash
# View current truth scores
claude-flow truth

# View scores for last 7 days
claude-flow truth --period 7d

# Export to HTML report
claude-flow truth --export report.html --format html
```

### Advanced Analysis
```bash
# Monitor real-time scores
claude-flow truth --watch

# Find problematic files
claude-flow truth --threshold 0.8

# Agent-specific metrics
claude-flow truth --agent coder --period 24h

# JSON for processing
claude-flow truth --format json | jq '.overall_score'
```

## Dashboard View

```
📊 Truth Metrics Dashboard
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Overall Truth Score: 0.947 ✅
Trend: ↗️ +2.3% (7d)

Top Performers:
  verification-agent   0.982 ⭐
  code-analyzer       0.971 ⭐
  test-generator      0.958 ✅

Needs Attention:
  refactor-agent      0.821 ⚠️
  docs-generator      0.794 ⚠️

Recent Tasks:
  task-456  0.991 ✅  "Implement auth"
  task-455  0.967 ✅  "Add tests"
  task-454  0.743 ❌  "Refactor API"
```

## Integration

### With CI/CD
```yaml
# GitHub Actions example
- name: Check Truth Scores
  run: |
    claude-flow truth --format json > truth.json
    score=$(jq '.overall_score' truth.json)
    if (( $(echo "$score < 0.95" | bc -l) )); then
      echo "Truth score too low: $score"
      exit 1
    fi
```

### With Monitoring
```bash
# Send to monitoring system
claude-flow truth --format json | \
  curl -X POST https://metrics.example.com/api/truth \
  -H "Content-Type: application/json" \
  -d @-
```

## Configuration

Set truth display preferences in `.claude-flow/config.json`:

```json
{
  "truth": {
    "defaultFormat": "table",
    "defaultPeriod": "24h",
    "warningThreshold": 0.85,
    "criticalThreshold": 0.75,
    "autoExport": {
      "enabled": true,
      "path": ".claude-flow/metrics/truth-daily.json"
    }
  }
}
```

## Related Commands

- `verify` - Run verification checks
- `pair` - Collaborative development with truth tracking
- `report` - Generate detailed reports