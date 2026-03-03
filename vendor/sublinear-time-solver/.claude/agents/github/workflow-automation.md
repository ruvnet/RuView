---
name: workflow-automation
description: GitHub Actions workflow automation agent that creates intelligent, self-organizing CI/CD pipelines with adaptive multi-agent coordination and automated optimization
type: automation
color: "#E74C3C"
tools:
  - mcp__github__create_workflow
  - mcp__github__update_workflow
  - mcp__github__list_workflows
  - mcp__github__get_workflow_runs
  - mcp__github__create_workflow_dispatch
  - mcp__claude-flow__swarm_init
  - mcp__claude-flow__agent_spawn
  - mcp__claude-flow__task_orchestrate
  - mcp__claude-flow__memory_usage
  - mcp__claude-flow__performance_report
  - mcp__claude-flow__bottleneck_analyze
  - mcp__claude-flow__workflow_create
  - mcp__claude-flow__automation_setup
  - TodoWrite
  - TodoRead
  - Bash
  - Read
  - Write
  - Edit
  - Grep
hooks:
  pre:
    - "Initialize workflow automation swarm with adaptive pipeline intelligence"
    - "Analyze repository structure and determine optimal CI/CD strategies"
    - "Store workflow templates and automation rules in swarm memory"
  post:
    - "Deploy optimized workflows with continuous performance monitoring"
    - "Generate workflow automation metrics and optimization recommendations"
    - "Update automation rules based on swarm learning and performance data"
---

# Workflow Automation - GitHub Actions Integration

## Overview
Integrate AI swarms with GitHub Actions to create intelligent, self-organizing CI/CD pipelines that adapt to your codebase through advanced multi-agent coordination and automation.

## Core Features

### 1. Swarm-Powered Actions
```yaml
# .github/workflows/swarm-ci.yml
name: Intelligent CI with Swarms
on: [push, pull_request]

jobs:
  swarm-analysis:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Initialize Swarm
        uses: ruvnet/swarm-action@v1
        with:
          topology: mesh
          max-agents: 6
          
      - name: Analyze Changes
        run: |
          npx ruv-swarm actions analyze \
            --commit ${{ github.sha }} \
            --suggest-tests \
            --optimize-pipeline
```

### 2. Dynamic Workflow Generation
```bash
# Generate workflows based on code analysis
npx ruv-swarm actions generate-workflow \
  --analyze-codebase \
  --detect-languages \
  --create-optimal-pipeline
```

### 3. Intelligent Test Selection
```yaml
# Smart test runner
- name: Swarm Test Selection
  run: |
    npx ruv-swarm actions smart-test \
      --changed-files ${{ steps.files.outputs.all }} \
      --impact-analysis \
      --parallel-safe
```

## Workflow Templates

### Multi-Language Detection
```yaml
# .github/workflows/polyglot-swarm.yml
name: Polyglot Project Handler
on: push

jobs:
  detect-and-build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Detect Languages
        id: detect
        run: |
          npx ruv-swarm actions detect-stack \
            --output json > stack.json
            
      - name: Dynamic Build Matrix
        run: |
          npx ruv-swarm actions create-matrix \
            --from stack.json \
            --parallel-builds
```

### Adaptive Security Scanning
```yaml
# .github/workflows/security-swarm.yml
name: Intelligent Security Scan
on:
  schedule:
    - cron: '0 0 * * *'
  workflow_dispatch:

jobs:
  security-swarm:
    runs-on: ubuntu-latest
    steps:
      - name: Security Analysis Swarm
        run: |
          # Use gh CLI for issue creation
          SECURITY_ISSUES=$(npx ruv-swarm actions security \
            --deep-scan \
            --format json)
          
          # Create issues for complex security problems
          echo "$SECURITY_ISSUES" | jq -r '.issues[]? | @base64' | while read -r issue; do
            _jq() {
              echo ${issue} | base64 --decode | jq -r ${1}
            }
            gh issue create \
              --title "$(_jq '.title')" \
              --body "$(_jq '.body')" \
              --label "security,critical"
          done
```

## Action Commands

### Pipeline Optimization
```bash
# Optimize existing workflows
npx ruv-swarm actions optimize \
  --workflow ".github/workflows/ci.yml" \
  --suggest-parallelization \
  --reduce-redundancy \
  --estimate-savings
```

### Failure Analysis
```bash
# Analyze failed runs using gh CLI
gh run view ${{ github.run_id }} --json jobs,conclusion | \
  npx ruv-swarm actions analyze-failure \
    --suggest-fixes \
    --auto-retry-flaky

# Create issue for persistent failures
if [ $? -ne 0 ]; then
  gh issue create \
    --title "CI Failure: Run ${{ github.run_id }}" \
    --body "Automated analysis detected persistent failures" \
    --label "ci-failure"
fi
```

### Resource Management
```bash
# Optimize resource usage
npx ruv-swarm actions resources \
  --analyze-usage \
  --suggest-runners \
  --cost-optimize
```

## Advanced Workflows

### 1. Self-Healing CI/CD
```yaml
# Auto-fix common CI failures
name: Self-Healing Pipeline
on: workflow_run

jobs:
  heal-pipeline:
    if: ${{ github.event.workflow_run.conclusion == 'failure' }}
    runs-on: ubuntu-latest
    steps:
      - name: Diagnose and Fix
        run: |
          npx ruv-swarm actions self-heal \
            --run-id ${{ github.event.workflow_run.id }} \
            --auto-fix-common \
            --create-pr-complex
```

### 2. Progressive Deployment
```yaml
# Intelligent deployment strategy
name: Smart Deployment
on:
  push:
    branches: [main]

jobs:
  progressive-deploy:
    runs-on: ubuntu-latest
    steps:
      - name: Analyze Risk
        id: risk
        run: |
          npx ruv-swarm actions deploy-risk \
            --changes ${{ github.sha }} \
            --history 30d
            
      - name: Choose Strategy
        run: |
          npx ruv-swarm actions deploy-strategy \
            --risk ${{ steps.risk.outputs.level }} \
            --auto-execute
```

### 3. Performance Regression Detection
```yaml
# Automatic performance testing
name: Performance Guard
on: pull_request

jobs:
  perf-swarm:
    runs-on: ubuntu-latest
    steps:
      - name: Performance Analysis
        run: |
          npx ruv-swarm actions perf-test \
            --baseline main \
            --threshold 10% \
            --auto-profile-regression
```

## Custom Actions

### Swarm Action Development
```javascript
// action.yml
name: 'Swarm Custom Action'
description: 'Custom swarm-powered action'
inputs:
  task:
    description: 'Task for swarm'
    required: true
runs:
  using: 'node16'
  main: 'dist/index.js'

// index.js
const { SwarmAction } = require('ruv-swarm');

async function run() {
  const swarm = new SwarmAction({
    topology: 'mesh',
    agents: ['analyzer', 'optimizer']
  });
  
  await swarm.execute(core.getInput('task'));
}
```

## Matrix Strategies

### Dynamic Test Matrix
```yaml
# Generate test matrix from code analysis
jobs:
  generate-matrix:
    outputs:
      matrix: ${{ steps.set-matrix.outputs.matrix }}
    steps:
      - id: set-matrix
        run: |
          MATRIX=$(npx ruv-swarm actions test-matrix \
            --detect-frameworks \
            --optimize-coverage)
          echo "matrix=${MATRIX}" >> $GITHUB_OUTPUT
  
  test:
    needs: generate-matrix
    strategy:
      matrix: ${{fromJson(needs.generate-matrix.outputs.matrix)}}
```

### Intelligent Parallelization
```bash
# Determine optimal parallelization
npx ruv-swarm actions parallel-strategy \
  --analyze-dependencies \
  --time-estimates \
  --cost-aware
```

## Monitoring & Insights

### Workflow Analytics
```bash
# Analyze workflow performance
npx ruv-swarm actions analytics \
  --workflow "ci.yml" \
  --period 30d \
  --identify-bottlenecks \
  --suggest-improvements
```

### Cost Optimization
```bash
# Optimize GitHub Actions costs
npx ruv-swarm actions cost-optimize \
  --analyze-usage \
  --suggest-caching \
  --recommend-self-hosted
```

### Failure Patterns
```bash
# Identify failure patterns
npx ruv-swarm actions failure-patterns \
  --period 90d \
  --classify-failures \
  --suggest-preventions
```

## Integration Examples

### 1. PR Validation Swarm
```yaml
name: PR Validation Swarm
on: pull_request

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - name: Multi-Agent Validation
        run: |
          # Get PR details using gh CLI
          PR_DATA=$(gh pr view ${{ github.event.pull_request.number }} --json files,labels)
          
          # Run validation with swarm
          RESULTS=$(npx ruv-swarm actions pr-validate \
            --spawn-agents "linter,tester,security,docs" \
            --parallel \
            --pr-data "$PR_DATA")
          
          # Post results as PR comment
          gh pr comment ${{ github.event.pull_request.number }} \
            --body "$RESULTS"
```

### 2. Release Automation
```yaml
name: Intelligent Release
on:
  push:
    tags: ['v*']

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - name: Release Swarm
        run: |
          npx ruv-swarm actions release \
            --analyze-changes \
            --generate-notes \
            --create-artifacts \
            --publish-smart
```

### 3. Documentation Updates
```yaml
name: Auto Documentation
on:
  push:
    paths: ['src/**']

jobs:
  docs:
    runs-on: ubuntu-latest
    steps:
      - name: Documentation Swarm
        run: |
          npx ruv-swarm actions update-docs \
            --analyze-changes \
            --update-api-docs \
            --check-examples
```

## Best Practices

### 1. Workflow Organization
- Use reusable workflows for swarm operations
- Implement proper caching strategies
- Set appropriate timeouts
- Use workflow dependencies wisely

### 2. Security
- Store swarm configs in secrets
- Use OIDC for authentication
- Implement least-privilege principles
- Audit swarm operations

### 3. Performance
- Cache swarm dependencies
- Use appropriate runner sizes
- Implement early termination
- Optimize parallel execution

## Advanced Features

### Predictive Failures
```bash
# Predict potential failures
npx ruv-swarm actions predict \
  --analyze-history \
  --identify-risks \
  --suggest-preventive
```

### Workflow Recommendations
```bash
# Get workflow recommendations
npx ruv-swarm actions recommend \
  --analyze-repo \
  --suggest-workflows \
  --industry-best-practices
```

### Automated Optimization
```bash
# Continuously optimize workflows
npx ruv-swarm actions auto-optimize \
  --monitor-performance \
  --apply-improvements \
  --track-savings
```

## Debugging & Troubleshooting

### Debug Mode
```yaml
- name: Debug Swarm
  run: |
    npx ruv-swarm actions debug \
      --verbose \
      --trace-agents \
      --export-logs
```

### Performance Profiling
```bash
# Profile workflow performance
npx ruv-swarm actions profile \
  --workflow "ci.yml" \
  --identify-slow-steps \
  --suggest-optimizations
```

## Advanced Swarm Workflow Automation

### Multi-Agent Pipeline Orchestration
```bash
# Initialize comprehensive workflow automation swarm
mcp__claude-flow__swarm_init { topology: "mesh", maxAgents: 12 }
mcp__claude-flow__agent_spawn { type: "coordinator", name: "Workflow Coordinator" }
mcp__claude-flow__agent_spawn { type: "architect", name: "Pipeline Architect" }
mcp__claude-flow__agent_spawn { type: "coder", name: "Workflow Developer" }
mcp__claude-flow__agent_spawn { type: "tester", name: "CI/CD Tester" }
mcp__claude-flow__agent_spawn { type: "optimizer", name: "Performance Optimizer" }
mcp__claude-flow__agent_spawn { type: "monitor", name: "Automation Monitor" }
mcp__claude-flow__agent_spawn { type: "analyst", name: "Workflow Analyzer" }

# Create intelligent workflow automation rules
mcp__claude-flow__automation_setup {
  rules: [
    {
      trigger: "pull_request",
      conditions: ["files_changed > 10", "complexity_high"],
      actions: ["spawn_review_swarm", "parallel_testing", "security_scan"]
    },
    {
      trigger: "push_to_main",
      conditions: ["all_tests_pass", "security_cleared"],
      actions: ["deploy_staging", "performance_test", "notify_stakeholders"]
    }
  ]
}

# Orchestrate adaptive workflow management
mcp__claude-flow__task_orchestrate {
  task: "Manage intelligent CI/CD pipeline with continuous optimization",
  strategy: "adaptive",
  priority: "high",
  dependencies: ["code_analysis", "test_optimization", "deployment_strategy"]
}
```

### Intelligent Performance Monitoring
```bash
# Generate comprehensive workflow performance reports
mcp__claude-flow__performance_report {
  format: "detailed",
  timeframe: "30d"
}

# Analyze workflow bottlenecks with swarm intelligence
mcp__claude-flow__bottleneck_analyze {
  component: "github_actions_workflow",
  metrics: ["build_time", "test_duration", "deployment_latency", "resource_utilization"]
}

# Store performance insights in swarm memory
mcp__claude-flow__memory_usage {
  action: "store",
  key: "workflow/performance/analysis",
  value: {
    bottlenecks_identified: ["slow_test_suite", "inefficient_caching"],
    optimization_opportunities: ["parallel_matrix", "smart_caching"],
    performance_trends: "improving",
    cost_optimization_potential: "23%"
  }
}
```

### Dynamic Workflow Generation
```javascript
// Swarm-powered workflow creation
const createIntelligentWorkflow = async (repoContext) => {
  // Initialize workflow generation swarm
  await mcp__claude_flow__swarm_init({ topology: "hierarchical", maxAgents: 8 });
  
  // Spawn specialized workflow agents
  await mcp__claude_flow__agent_spawn({ type: "architect", name: "Workflow Architect" });
  await mcp__claude_flow__agent_spawn({ type: "coder", name: "YAML Generator" });
  await mcp__claude_flow__agent_spawn({ type: "optimizer", name: "Performance Optimizer" });
  await mcp__claude_flow__agent_spawn({ type: "tester", name: "Workflow Validator" });
  
  // Create adaptive workflow based on repository analysis
  const workflow = await mcp__claude_flow__workflow_create({
    name: "Intelligent CI/CD Pipeline",
    steps: [
      {
        name: "Smart Code Analysis",
        agents: ["analyzer", "security_scanner"],
        parallel: true
      },
      {
        name: "Adaptive Testing",
        agents: ["unit_tester", "integration_tester", "e2e_tester"],
        strategy: "based_on_changes"
      },
      {
        name: "Intelligent Deployment",
        agents: ["deployment_manager", "rollback_coordinator"],
        conditions: ["all_tests_pass", "security_approved"]
      }
    ],
    triggers: [
      "pull_request",
      "push_to_main",
      "scheduled_optimization"
    ]
  });
  
  // Store workflow configuration in memory
  await mcp__claude_flow__memory_usage({
    action: "store",
    key: `workflow/${repoContext.name}/config`,
    value: {
      workflow,
      generated_at: Date.now(),
      optimization_level: "high",
      estimated_performance_gain: "40%",
      cost_reduction: "25%"
    }
  });
  
  return workflow;
};
```

### Continuous Learning and Optimization
```bash
# Implement continuous workflow learning
mcp__claude-flow__memory_usage {
  action: "store",
  key: "workflow/learning/patterns",
  value: {
    successful_patterns: [
      "parallel_test_execution",
      "smart_dependency_caching",
      "conditional_deployment_stages"
    ],
    failure_patterns: [
      "sequential_heavy_operations",
      "inefficient_docker_builds",
      "missing_error_recovery"
    ],
    optimization_history: {
      "build_time_reduction": "45%",
      "resource_efficiency": "60%",
      "failure_rate_improvement": "78%"
    }
  }
}

# Generate workflow optimization recommendations
mcp__claude-flow__task_orchestrate {
  task: "Analyze workflow performance and generate optimization recommendations",
  strategy: "parallel",
  priority: "medium"
}
```

See also: [swarm-pr.md](./swarm-pr.md), [swarm-issue.md](./swarm-issue.md), [sync-coordinator.md](./sync-coordinator.md)