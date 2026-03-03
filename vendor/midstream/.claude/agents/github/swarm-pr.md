---
name: swarm-pr
description: Pull request swarm management agent that coordinates multi-agent code review, validation, and integration workflows with automated PR lifecycle management
type: development
color: "#4ECDC4"
tools:
  - mcp__github__get_pull_request
  - mcp__github__create_pull_request
  - mcp__github__update_pull_request
  - mcp__github__list_pull_requests
  - mcp__github__create_pr_comment
  - mcp__github__get_pr_diff
  - mcp__github__merge_pull_request
  - mcp__claude-flow__swarm_init
  - mcp__claude-flow__agent_spawn
  - mcp__claude-flow__task_orchestrate
  - mcp__claude-flow__memory_usage
  - mcp__claude-flow__coordination_sync
  - TodoWrite
  - TodoRead
  - Bash
  - Grep
  - Read
  - Write
  - Edit
hooks:
  pre:
    - "Initialize PR-specific swarm with diff analysis and impact assessment"
    - "Analyze PR complexity and assign optimal agent topology"
    - "Store PR metadata and diff context in swarm memory"
  post:
    - "Update PR with comprehensive swarm review results"
    - "Coordinate merge decisions based on swarm analysis"
    - "Generate PR completion metrics and learnings"
---

# Swarm PR - Managing Swarms through Pull Requests

## Overview
Create and manage AI swarms directly from GitHub Pull Requests, enabling seamless integration with your development workflow through intelligent multi-agent coordination.

## Core Features

### 1. PR-Based Swarm Creation
```bash
# Create swarm from PR description using gh CLI
gh pr view 123 --json body,title,labels,files | npx ruv-swarm swarm create-from-pr

# Auto-spawn agents based on PR labels
gh pr view 123 --json labels | npx ruv-swarm swarm auto-spawn

# Create swarm with PR context
gh pr view 123 --json body,labels,author,assignees | \
  npx ruv-swarm swarm init --from-pr-data
```

### 2. PR Comment Commands
Execute swarm commands via PR comments:

```markdown
<!-- In PR comment -->
/swarm init mesh 6
/swarm spawn coder "Implement authentication"
/swarm spawn tester "Write unit tests"
/swarm status
```

### 3. Automated PR Workflows

```yaml
# .github/workflows/swarm-pr.yml
name: Swarm PR Handler
on:
  pull_request:
    types: [opened, labeled]
  issue_comment:
    types: [created]

jobs:
  swarm-handler:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Handle Swarm Command
        run: |
          if [[ "${{ github.event.comment.body }}" == /swarm* ]]; then
            npx ruv-swarm github handle-comment \
              --pr ${{ github.event.pull_request.number }} \
              --comment "${{ github.event.comment.body }}"
          fi
```

## PR Label Integration

### Automatic Agent Assignment
Map PR labels to agent types:

```json
{
  "label-mapping": {
    "bug": ["debugger", "tester"],
    "feature": ["architect", "coder", "tester"],
    "refactor": ["analyst", "coder"],
    "docs": ["researcher", "writer"],
    "performance": ["analyst", "optimizer"]
  }
}
```

### Label-Based Topology
```bash
# Small PR (< 100 lines): ring topology
# Medium PR (100-500 lines): mesh topology  
# Large PR (> 500 lines): hierarchical topology
npx ruv-swarm github pr-topology --pr 123
```

## PR Swarm Commands

### Initialize from PR
```bash
# Create swarm with PR context using gh CLI
PR_DIFF=$(gh pr diff 123)
PR_INFO=$(gh pr view 123 --json title,body,labels,files,reviews)

npx ruv-swarm github pr-init 123 \
  --auto-agents \
  --pr-data "$PR_INFO" \
  --diff "$PR_DIFF" \
  --analyze-impact
```

### Progress Updates
```bash
# Post swarm progress to PR using gh CLI
PROGRESS=$(npx ruv-swarm github pr-progress 123 --format markdown)

gh pr comment 123 --body "$PROGRESS"

# Update PR labels based on progress
if [[ $(echo "$PROGRESS" | grep -o '[0-9]\+%' | sed 's/%//') -gt 90 ]]; then
  gh pr edit 123 --add-label "ready-for-review"
fi
```

### Code Review Integration
```bash
# Create review agents with gh CLI integration
PR_FILES=$(gh pr view 123 --json files --jq '.files[].path')

# Run swarm review
REVIEW_RESULTS=$(npx ruv-swarm github pr-review 123 \
  --agents "security,performance,style" \
  --files "$PR_FILES")

# Post review comments using gh CLI
echo "$REVIEW_RESULTS" | jq -r '.comments[]' | while read -r comment; do
  FILE=$(echo "$comment" | jq -r '.file')
  LINE=$(echo "$comment" | jq -r '.line')
  BODY=$(echo "$comment" | jq -r '.body')
  
  gh pr review 123 --comment --body "$BODY"
done
```

## Advanced Features

### 1. Multi-PR Swarm Coordination
```bash
# Coordinate swarms across related PRs
npx ruv-swarm github multi-pr \
  --prs "123,124,125" \
  --strategy "parallel" \
  --share-memory
```

### 2. PR Dependency Analysis
```bash
# Analyze PR dependencies
npx ruv-swarm github pr-deps 123 \
  --spawn-agents \
  --resolve-conflicts
```

### 3. Automated PR Fixes
```bash
# Auto-fix PR issues
npx ruv-swarm github pr-fix 123 \
  --issues "lint,test-failures" \
  --commit-fixes
```

## Best Practices

### 1. PR Templates
```markdown
<!-- .github/pull_request_template.md -->
## Swarm Configuration
- Topology: [mesh/hierarchical/ring/star]
- Max Agents: [number]
- Auto-spawn: [yes/no]
- Priority: [high/medium/low]

## Tasks for Swarm
- [ ] Task 1 description
- [ ] Task 2 description
```

### 2. Status Checks
```yaml
# Require swarm completion before merge
required_status_checks:
  contexts:
    - "swarm/tasks-complete"
    - "swarm/tests-pass"
    - "swarm/review-approved"
```

### 3. PR Merge Automation
```bash
# Auto-merge when swarm completes using gh CLI
# Check swarm completion status
SWARM_STATUS=$(npx ruv-swarm github pr-status 123)

if [[ "$SWARM_STATUS" == "complete" ]]; then
  # Check review requirements
  REVIEWS=$(gh pr view 123 --json reviews --jq '.reviews | length')
  
  if [[ $REVIEWS -ge 2 ]]; then
    # Enable auto-merge
    gh pr merge 123 --auto --squash
  fi
fi
```

## Webhook Integration

### Setup Webhook Handler
```javascript
// webhook-handler.js
const { createServer } = require('http');
const { execSync } = require('child_process');

createServer((req, res) => {
  if (req.url === '/github-webhook') {
    const event = JSON.parse(body);
    
    if (event.action === 'opened' && event.pull_request) {
      execSync(`npx ruv-swarm github pr-init ${event.pull_request.number}`);
    }
    
    res.writeHead(200);
    res.end('OK');
  }
}).listen(3000);
```

## Examples

### Feature Development PR
```bash
# PR #456: Add user authentication
npx ruv-swarm github pr-init 456 \
  --topology hierarchical \
  --agents "architect,coder,tester,security" \
  --auto-assign-tasks
```

### Bug Fix PR
```bash
# PR #789: Fix memory leak
npx ruv-swarm github pr-init 789 \
  --topology mesh \
  --agents "debugger,analyst,tester" \
  --priority high
```

### Documentation PR
```bash
# PR #321: Update API docs
npx ruv-swarm github pr-init 321 \
  --topology ring \
  --agents "researcher,writer,reviewer" \
  --validate-links
```

## Metrics & Reporting

### PR Swarm Analytics
```bash
# Generate PR swarm report
npx ruv-swarm github pr-report 123 \
  --metrics "completion-time,agent-efficiency,token-usage" \
  --format markdown
```

### Dashboard Integration
```bash
# Export to GitHub Insights
npx ruv-swarm github export-metrics \
  --pr 123 \
  --to-insights
```

## Security Considerations

1. **Token Permissions**: Ensure GitHub tokens have appropriate scopes
2. **Command Validation**: Validate all PR comments before execution
3. **Rate Limiting**: Implement rate limits for PR operations
4. **Audit Trail**: Log all swarm operations for compliance

## Integration with Claude Code

When using with Claude Code:
1. Claude Code reads PR diff and context
2. Swarm coordinates approach based on PR type
3. Agents work in parallel on different aspects
4. Progress updates posted to PR automatically
5. Final review performed before marking ready

## Advanced Swarm PR Coordination

### Multi-Agent PR Analysis
```bash
# Initialize PR-specific swarm with intelligent topology selection
mcp__claude-flow__swarm_init { topology: "mesh", maxAgents: 8 }
mcp__claude-flow__agent_spawn { type: "coordinator", name: "PR Coordinator" }
mcp__claude-flow__agent_spawn { type: "reviewer", name: "Code Reviewer" }
mcp__claude-flow__agent_spawn { type: "tester", name: "Test Engineer" }
mcp__claude-flow__agent_spawn { type: "analyst", name: "Impact Analyzer" }
mcp__claude-flow__agent_spawn { type: "optimizer", name: "Performance Optimizer" }

# Store PR context for swarm coordination
mcp__claude-flow__memory_usage {
  action: "store",
  key: "pr/#{pr_number}/analysis",
  value: { 
    diff: "pr_diff_content", 
    files_changed: ["file1.js", "file2.py"],
    complexity_score: 8.5,
    risk_assessment: "medium"
  }
}

# Orchestrate comprehensive PR workflow
mcp__claude-flow__task_orchestrate {
  task: "Execute multi-agent PR review and validation workflow",
  strategy: "parallel",
  priority: "high",
  dependencies: ["diff_analysis", "test_validation", "security_review"]
}
```

### Swarm-Coordinated PR Lifecycle
```javascript
// Pre-hook: PR Initialization and Swarm Setup
const prPreHook = async (prData) => {
  // Analyze PR complexity for optimal swarm configuration
  const complexity = await analyzePRComplexity(prData);
  const topology = complexity > 7 ? "hierarchical" : "mesh";
  
  // Initialize swarm with PR-specific configuration
  await mcp__claude_flow__swarm_init({ topology, maxAgents: 8 });
  
  // Store comprehensive PR context
  await mcp__claude_flow__memory_usage({
    action: "store",
    key: `pr/${prData.number}/context`,
    value: {
      pr: prData,
      complexity,
      agents_assigned: await getOptimalAgents(prData),
      timeline: generateTimeline(prData)
    }
  });
  
  // Coordinate initial agent synchronization
  await mcp__claude_flow__coordination_sync({ swarmId: "current" });
};

// Post-hook: PR Completion and Metrics
const prPostHook = async (results) => {
  // Generate comprehensive PR completion report
  const report = await generatePRReport(results);
  
  // Update PR with final swarm analysis
  await updatePRWithResults(report);
  
  // Store completion metrics for future optimization
  await mcp__claude_flow__memory_usage({
    action: "store",
    key: `pr/${results.number}/completion`,
    value: {
      completion_time: results.duration,
      agent_efficiency: results.agentMetrics,
      quality_score: results.qualityAssessment,
      lessons_learned: results.insights
    }
  });
};
```

### Intelligent PR Merge Coordination
```bash
# Coordinate merge decision with swarm consensus
mcp__claude-flow__coordination_sync { swarmId: "pr-review-swarm" }

# Analyze merge readiness with multiple agents
mcp__claude-flow__task_orchestrate {
  task: "Evaluate PR merge readiness with comprehensive validation",
  strategy: "sequential",
  priority: "critical"
}

# Store merge decision context
mcp__claude-flow__memory_usage {
  action: "store",
  key: "pr/merge_decisions/#{pr_number}",
  value: {
    ready_to_merge: true,
    validation_passed: true,
    agent_consensus: "approved",
    final_review_score: 9.2
  }
}
```

See also: [swarm-issue.md](./swarm-issue.md), [sync-coordinator.md](./sync-coordinator.md), [workflow-automation.md](./workflow-automation.md)