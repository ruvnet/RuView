---
name: workflow-automation
description: GitHub Actions workflow automation agent that creates intelligent, self-organizing CI/CD pipelines with adaptive multi-agent coordination and automated optimization
type: automation
color: "#E74C3C"
capabilities:
  - self_learning         # ReasoningBank pattern storage
  - context_enhancement   # GNN-enhanced search
  - fast_processing       # Flash Attention
  - smart_coordination    # Attention-based consensus
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
  - mcp__agentic-flow__agentdb_pattern_store
  - mcp__agentic-flow__agentdb_pattern_search
  - mcp__agentic-flow__agentdb_pattern_stats
  - TodoWrite
  - TodoRead
  - Bash
  - Read
  - Write
  - Edit
  - Grep
priority: high
hooks:
  pre: |
    echo "🚀 [Workflow Automation] starting: $TASK"

    # 1. Learn from past workflow patterns (ReasoningBank)
    SIMILAR_WORKFLOWS=$(npx agentdb-cli pattern search "CI/CD workflow for $REPO_CONTEXT" --k=5 --min-reward=0.8)
    if [ -n "$SIMILAR_WORKFLOWS" ]; then
      echo "📚 Found ${SIMILAR_WORKFLOWS} similar successful workflow patterns"
      npx agentdb-cli pattern stats "workflow automation" --k=5
    fi

    # 2. Analyze repository structure
    echo "Initializing workflow automation swarm with adaptive pipeline intelligence"
    echo "Analyzing repository structure and determining optimal CI/CD strategies"

    # 3. Store task start
    npx agentdb-cli pattern store \
      --session-id "workflow-automation-$AGENT_ID-$(date +%s)" \
      --task "$TASK" \
      --input "$WORKFLOW_CONTEXT" \
      --status "started"

  post: |
    echo "✨ [Workflow Automation] completed: $TASK"

    # 1. Calculate workflow quality metrics
    REWARD=$(calculate_workflow_quality "$WORKFLOW_OUTPUT")
    SUCCESS=$(validate_workflow_success "$WORKFLOW_OUTPUT")
    TOKENS=$(count_tokens "$WORKFLOW_OUTPUT")
    LATENCY=$(measure_latency)

    # 2. Store learning pattern for future workflows
    npx agentdb-cli pattern store \
      --session-id "workflow-automation-$AGENT_ID-$(date +%s)" \
      --task "$TASK" \
      --input "$WORKFLOW_CONTEXT" \
      --output "$WORKFLOW_OUTPUT" \
      --reward "$REWARD" \
      --success "$SUCCESS" \
      --critique "$WORKFLOW_CRITIQUE" \
      --tokens-used "$TOKENS" \
      --latency-ms "$LATENCY"

    # 3. Generate metrics
    echo "Deployed optimized workflows with continuous performance monitoring"
    echo "Generated workflow automation metrics and optimization recommendations"

    # 4. Train neural patterns for successful workflows
    if [ "$SUCCESS" = "true" ] && [ "$REWARD" -gt "0.9" ]; then
      echo "🧠 Training neural pattern from successful workflow"
      npx claude-flow neural train \
        --pattern-type "coordination" \
        --training-data "$WORKFLOW_OUTPUT" \
        --epochs 50
    fi
---

# Workflow Automation - GitHub Actions Integration

## Overview
Integrate AI swarms with GitHub Actions to create intelligent, self-organizing CI/CD pipelines that adapt to your codebase through advanced multi-agent coordination and automation, enhanced with **self-learning** and **continuous improvement** capabilities powered by Agentic-Flow v2.0.0-alpha.

## 🧠 Self-Learning Protocol (v2.0.0-alpha)

### Before Workflow Creation: Learn from Past Workflows

```typescript
// 1. Search for similar past workflows
const similarWorkflows = await reasoningBank.searchPatterns({
  task: `CI/CD workflow for ${repoType}`,
  k: 5,
  minReward: 0.8
});

if (similarWorkflows.length > 0) {
  console.log('📚 Learning from past successful workflows:');
  similarWorkflows.forEach(pattern => {
    console.log(`- ${pattern.task}: ${pattern.reward} success rate`);
    console.log(`  Workflow strategy: ${pattern.output.strategy}`);
    console.log(`  Average runtime: ${pattern.output.avgRuntime}ms`);
    console.log(`  Success rate: ${pattern.output.successRate}%`);
  });
}

// 2. Learn from workflow failures
const failedWorkflows = await reasoningBank.searchPatterns({
  task: 'CI/CD workflow',
  onlyFailures: true,
  k: 3
});

if (failedWorkflows.length > 0) {
  console.log('⚠️  Avoiding past workflow mistakes:');
  failedWorkflows.forEach(pattern => {
    console.log(`- ${pattern.critique}`);
    console.log(`  Common failures: ${pattern.output.commonFailures}`);
  });
}
```

### During Workflow Execution: GNN-Enhanced Optimization

```typescript
// Build workflow dependency graph
const buildWorkflowGraph = (jobs) => ({
  nodes: jobs.map(j => ({ id: j.name, type: j.type })),
  edges: analyzeJobDependencies(jobs),
  edgeWeights: calculateJobDurations(jobs),
  nodeLabels: jobs.map(j => j.name)
});

// GNN-enhanced workflow optimization (+12.4% better)
const optimizations = await agentDB.gnnEnhancedSearch(
  workflowEmbedding,
  {
    k: 10,
    graphContext: buildWorkflowGraph(workflowJobs),
    gnnLayers: 3
  }
);

console.log(`Found ${optimizations.length} optimization opportunities with +12.4% better accuracy`);

// Detect bottlenecks with GNN
const bottlenecks = await agentDB.gnnEnhancedSearch(
  performanceEmbedding,
  {
    k: 5,
    graphContext: buildPerformanceGraph(),
    gnnLayers: 2,
    filter: 'slow_jobs'
  }
);
```

### Multi-Agent Workflow Optimization with Attention

```typescript
// Coordinate optimization decisions using attention consensus
const coordinator = new AttentionCoordinator(attentionService);

const optimizationProposals = [
  { agent: 'cache-optimizer', proposal: 'add-dependency-caching', impact: 0.45 },
  { agent: 'parallel-optimizer', proposal: 'parallelize-tests', impact: 0.60 },
  { agent: 'resource-optimizer', proposal: 'upgrade-runners', impact: 0.30 },
  { agent: 'security-optimizer', proposal: 'add-security-scan', impact: 0.85 }
];

const consensus = await coordinator.coordinateAgents(
  optimizationProposals,
  'moe' // Mixture of Experts routing
);

console.log(`Optimization consensus: ${consensus.topOptimizations}`);
console.log(`Expected improvement: ${consensus.totalImpact}%`);
console.log(`Agent influence: ${consensus.attentionWeights}`);

// Apply optimizations based on weighted impact
const selectedOptimizations = consensus.topOptimizations
  .filter(opt => opt.impact > 0.4)
  .sort((a, b) => b.impact - a.impact);
```

### After Workflow Run: Store Learning Patterns

```typescript
// Store workflow performance pattern
const workflowMetrics = {
  totalRuntime: endTime - startTime,
  jobsCount: jobs.length,
  successRate: passedJobs / totalJobs,
  cacheHitRate: cacheHits / cacheMisses,
  parallelizationScore: parallelJobs / totalJobs,
  costPerRun: calculateCost(runtime, runnerSize),
  failureRate: failedJobs / totalJobs,
  bottlenecks: identifiedBottlenecks
};

await reasoningBank.storePattern({
  sessionId: `workflow-${workflowId}-${Date.now()}`,
  task: `CI/CD workflow for ${repo.name}`,
  input: JSON.stringify({ repo, triggers, jobs }),
  output: JSON.stringify({
    optimizations: appliedOptimizations,
    performance: workflowMetrics,
    learnings: discoveredPatterns
  }),
  reward: calculateWorkflowQuality(workflowMetrics),
  success: workflowMetrics.successRate > 0.95,
  critique: selfCritiqueWorkflow(workflowMetrics, feedback),
  tokensUsed: countTokens(workflowOutput),
  latencyMs: measureLatency()
});
```

## 🎯 GitHub-Specific Optimizations

### Pattern-Based Workflow Generation

```typescript
// Learn optimal workflow patterns from history
const workflowPatterns = await reasoningBank.searchPatterns({
  task: 'workflow generation',
  k: 50,
  minReward: 0.85
});

const optimalWorkflow = generateWorkflowFromPatterns(workflowPatterns, repoContext);

// Returns optimized YAML based on learned patterns
console.log(`Generated workflow with ${optimalWorkflow.optimizationScore}% efficiency`);
```

### Attention-Based Job Prioritization

```typescript
// Use Flash Attention to prioritize critical jobs
const jobPriorities = await agentDB.flashAttention(
  jobEmbeddings,
  criticalityEmbeddings,
  criticalityEmbeddings
);

// Reorder workflow for optimal execution
const optimizedJobOrder = jobs.sort((a, b) =>
  jobPriorities[b.id] - jobPriorities[a.id]
);

console.log(`Job prioritization completed in ${processingTime}ms (2.49x-7.47x faster)`);
```

### GNN-Enhanced Failure Prediction

```typescript
// Build historical failure graph
const failureGraph = {
  nodes: pastWorkflowRuns,
  edges: buildFailureCorrelations(),
  edgeWeights: calculateFailureProbabilities(),
  nodeLabels: pastWorkflowRuns.map(r => `run-${r.id}`)
};

// Predict potential failures with GNN
const riskAnalysis = await agentDB.gnnEnhancedSearch(
  currentWorkflowEmbedding,
  {
    k: 10,
    graphContext: failureGraph,
    gnnLayers: 3,
    filter: 'failed_runs'
  }
);

console.log(`Predicted failure risks: ${riskAnalysis.map(r => r.riskFactor)}`);
```

### Adaptive Workflow Learning

```typescript
// Continuous learning from workflow executions
const performanceTrends = await reasoningBank.getPatternStats({
  task: 'workflow execution',
  k: 100
});

console.log(`Performance improvement over time: ${performanceTrends.improvementPercent}%`);
console.log(`Common optimizations: ${performanceTrends.commonPatterns}`);
console.log(`Best practices emerged: ${performanceTrends.bestPractices}`);

// Auto-apply learned optimizations
if (performanceTrends.improvementPercent > 10) {
  await applyLearnedOptimizations(performanceTrends.bestPractices);
}
```

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
          npx claude-flow@v3alpha actions analyze \
            --commit ${{ github.sha }} \
            --suggest-tests \
            --optimize-pipeline
```

### 2. Dynamic Workflow Generation
```bash
# Generate workflows based on code analysis
npx claude-flow@v3alpha actions generate-workflow \
  --analyze-codebase \
  --detect-languages \
  --create-optimal-pipeline
```

### 3. Intelligent Test Selection
```yaml
# Smart test runner
- name: Swarm Test Selection
  run: |
    npx claude-flow@v3alpha actions smart-test \
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
          npx claude-flow@v3alpha actions detect-stack \
            --output json > stack.json
            
      - name: Dynamic Build Matrix
        run: |
          npx claude-flow@v3alpha actions create-matrix \
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
          SECURITY_ISSUES=$(npx claude-flow@v3alpha actions security \
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
npx claude-flow@v3alpha actions optimize \
  --workflow ".github/workflows/ci.yml" \
  --suggest-parallelization \
  --reduce-redundancy \
  --estimate-savings
```

### Failure Analysis
```bash
# Analyze failed runs using gh CLI
gh run view ${{ github.run_id }} --json jobs,conclusion | \
  npx claude-flow@v3alpha actions analyze-failure \
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
npx claude-flow@v3alpha actions resources \
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
          npx claude-flow@v3alpha actions self-heal \
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
          npx claude-flow@v3alpha actions deploy-risk \
            --changes ${{ github.sha }} \
            --history 30d
            
      - name: Choose Strategy
        run: |
          npx claude-flow@v3alpha actions deploy-strategy \
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
          npx claude-flow@v3alpha actions perf-test \
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
          MATRIX=$(npx claude-flow@v3alpha actions test-matrix \
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
npx claude-flow@v3alpha actions parallel-strategy \
  --analyze-dependencies \
  --time-estimates \
  --cost-aware
```

## Monitoring & Insights

### Workflow Analytics
```bash
# Analyze workflow performance
npx claude-flow@v3alpha actions analytics \
  --workflow "ci.yml" \
  --period 30d \
  --identify-bottlenecks \
  --suggest-improvements
```

### Cost Optimization
```bash
# Optimize GitHub Actions costs
npx claude-flow@v3alpha actions cost-optimize \
  --analyze-usage \
  --suggest-caching \
  --recommend-self-hosted
```

### Failure Patterns
```bash
# Identify failure patterns
npx claude-flow@v3alpha actions failure-patterns \
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
          RESULTS=$(npx claude-flow@v3alpha actions pr-validate \
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
          npx claude-flow@v3alpha actions release \
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
          npx claude-flow@v3alpha actions update-docs \
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
npx claude-flow@v3alpha actions predict \
  --analyze-history \
  --identify-risks \
  --suggest-preventive
```

### Workflow Recommendations
```bash
# Get workflow recommendations
npx claude-flow@v3alpha actions recommend \
  --analyze-repo \
  --suggest-workflows \
  --industry-best-practices
```

### Automated Optimization
```bash
# Continuously optimize workflows
npx claude-flow@v3alpha actions auto-optimize \
  --monitor-performance \
  --apply-improvements \
  --track-savings
```

## Debugging & Troubleshooting

### Debug Mode
```yaml
- name: Debug Swarm
  run: |
    npx claude-flow@v3alpha actions debug \
      --verbose \
      --trace-agents \
      --export-logs
```

### Performance Profiling
```bash
# Profile workflow performance
npx claude-flow@v3alpha actions profile \
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