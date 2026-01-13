---
name: github-workflow-automation
version: 1.0.0
category: github
description: Advanced GitHub Actions workflow automation with AI swarm coordination, intelligent CI/CD pipelines, and comprehensive repository management
tags:
  - github
  - github-actions
  - ci-cd
  - workflow-automation
  - swarm-coordination
  - deployment
  - security
authors:
  - claude-flow
requires:
  - gh (GitHub CLI)
  - git
  - claude-flow@alpha
  - node (v16+)
priority: high
progressive_disclosure: true
---

# GitHub Workflow Automation Skill

## Overview

This skill provides comprehensive GitHub Actions automation with AI swarm coordination. It integrates intelligent CI/CD pipelines, workflow orchestration, and repository management to create self-organizing, adaptive GitHub workflows.

## Quick Start

<details>
<summary>💡 Basic Usage - Click to expand</summary>

### Initialize GitHub Workflow Automation
```bash
# Start with a simple workflow
npx ruv-swarm actions generate-workflow \
  --analyze-codebase \
  --detect-languages \
  --create-optimal-pipeline
```

### Common Commands
```bash
# Optimize existing workflow
npx ruv-swarm actions optimize \
  --workflow ".github/workflows/ci.yml" \
  --suggest-parallelization

# Analyze failed runs
gh run view <run-id> --json jobs,conclusion | \
  npx ruv-swarm actions analyze-failure \
    --suggest-fixes
```

</details>

## Core Capabilities

### 🤖 Swarm-Powered GitHub Modes

<details>
<summary>Available GitHub Integration Modes</summary>

#### 1. gh-coordinator
**GitHub workflow orchestration and coordination**
- **Coordination Mode**: Hierarchical
- **Max Parallel Operations**: 10
- **Batch Optimized**: Yes
- **Best For**: Complex GitHub workflows, multi-repo coordination

```bash
# Usage example
npx claude-flow@alpha github gh-coordinator \
  "Coordinate multi-repo release across 5 repositories"
```

#### 2. pr-manager
**Pull request management and review coordination**
- **Review Mode**: Automated
- **Multi-reviewer**: Yes
- **Conflict Resolution**: Intelligent

```bash
# Create PR with automated review
gh pr create --title "Feature: New capability" \
  --body "Automated PR with swarm review" | \
  npx ruv-swarm actions pr-validate \
    --spawn-agents "linter,tester,security,docs"
```

#### 3. issue-tracker
**Issue management and project coordination**
- **Issue Workflow**: Automated
- **Label Management**: Smart
- **Progress Tracking**: Real-time

```bash
# Create coordinated issue workflow
npx claude-flow@alpha github issue-tracker \
  "Manage sprint issues with automated tracking"
```

#### 4. release-manager
**Release coordination and deployment**
- **Release Pipeline**: Automated
- **Versioning**: Semantic
- **Deployment**: Multi-stage

```bash
# Automated release management
npx claude-flow@alpha github release-manager \
  "Create v2.0.0 release with changelog and deployment"
```

#### 5. repo-architect
**Repository structure and organization**
- **Structure Optimization**: Yes
- **Multi-repo Support**: Yes
- **Template Management**: Advanced

```bash
# Optimize repository structure
npx claude-flow@alpha github repo-architect \
  "Restructure monorepo with optimal organization"
```

#### 6. code-reviewer
**Automated code review and quality assurance**
- **Review Quality**: Deep
- **Security Analysis**: Yes
- **Performance Check**: Automated

```bash
# Automated code review
gh pr view 123 --json files | \
  npx ruv-swarm actions pr-validate \
    --deep-review \
    --security-scan
```

#### 7. ci-orchestrator
**CI/CD pipeline coordination**
- **Pipeline Management**: Advanced
- **Test Coordination**: Parallel
- **Deployment**: Automated

```bash
# Orchestrate CI/CD pipeline
npx claude-flow@alpha github ci-orchestrator \
  "Setup parallel test execution with smart caching"
```

#### 8. security-guardian
**Security and compliance management**
- **Security Scan**: Automated
- **Compliance Check**: Continuous
- **Vulnerability Management**: Proactive

```bash
# Security audit
npx ruv-swarm actions security \
  --deep-scan \
  --compliance-check \
  --create-issues
```

</details>

### 🔧 Workflow Templates

<details>
<summary>Production-Ready GitHub Actions Templates</summary>

#### 1. Intelligent CI with Swarms
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

#### 2. Multi-Language Detection
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

#### 3. Adaptive Security Scanning
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
          SECURITY_ISSUES=$(npx ruv-swarm actions security \
            --deep-scan \
            --format json)

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

#### 4. Self-Healing Pipeline
```yaml
# .github/workflows/self-healing.yml
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

#### 5. Progressive Deployment
```yaml
# .github/workflows/smart-deployment.yml
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

#### 6. Performance Regression Detection
```yaml
# .github/workflows/performance-guard.yml
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

#### 7. PR Validation Swarm
```yaml
# .github/workflows/pr-validation.yml
name: PR Validation Swarm
on: pull_request

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - name: Multi-Agent Validation
        run: |
          PR_DATA=$(gh pr view ${{ github.event.pull_request.number }} --json files,labels)

          RESULTS=$(npx ruv-swarm actions pr-validate \
            --spawn-agents "linter,tester,security,docs" \
            --parallel \
            --pr-data "$PR_DATA")

          gh pr comment ${{ github.event.pull_request.number }} \
            --body "$RESULTS"
```

#### 8. Intelligent Release
```yaml
# .github/workflows/intelligent-release.yml
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

</details>

### 📊 Monitoring & Analytics

<details>
<summary>Workflow Analysis & Optimization</summary>

#### Workflow Analytics
```bash
# Analyze workflow performance
npx ruv-swarm actions analytics \
  --workflow "ci.yml" \
  --period 30d \
  --identify-bottlenecks \
  --suggest-improvements
```

#### Cost Optimization
```bash
# Optimize GitHub Actions costs
npx ruv-swarm actions cost-optimize \
  --analyze-usage \
  --suggest-caching \
  --recommend-self-hosted
```

#### Failure Pattern Analysis
```bash
# Identify failure patterns
npx ruv-swarm actions failure-patterns \
  --period 90d \
  --classify-failures \
  --suggest-preventions
```

#### Resource Management
```bash
# Optimize resource usage
npx ruv-swarm actions resources \
  --analyze-usage \
  --suggest-runners \
  --cost-optimize
```

</details>

## Advanced Features

### 🧪 Dynamic Test Strategies

<details>
<summary>Intelligent Test Selection & Execution</summary>

#### Smart Test Selection
```yaml
# Automatically select relevant tests
- name: Swarm Test Selection
  run: |
    npx ruv-swarm actions smart-test \
      --changed-files ${{ steps.files.outputs.all }} \
      --impact-analysis \
      --parallel-safe
```

#### Dynamic Test Matrix
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

#### Intelligent Parallelization
```bash
# Determine optimal parallelization
npx ruv-swarm actions parallel-strategy \
  --analyze-dependencies \
  --time-estimates \
  --cost-aware
```

</details>

### 🔮 Predictive Analysis

<details>
<summary>AI-Powered Workflow Predictions</summary>

#### Predictive Failures
```bash
# Predict potential failures
npx ruv-swarm actions predict \
  --analyze-history \
  --identify-risks \
  --suggest-preventive
```

#### Workflow Recommendations
```bash
# Get workflow recommendations
npx ruv-swarm actions recommend \
  --analyze-repo \
  --suggest-workflows \
  --industry-best-practices
```

#### Automated Optimization
```bash
# Continuously optimize workflows
npx ruv-swarm actions auto-optimize \
  --monitor-performance \
  --apply-improvements \
  --track-savings
```

</details>

### 🎯 Custom Actions Development

<details>
<summary>Build Your Own Swarm Actions</summary>

#### Custom Swarm Action Template
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

run().catch(error => core.setFailed(error.message));
```

</details>

## Integration with Claude-Flow

### 🔄 Swarm Coordination Patterns

<details>
<summary>MCP-Based GitHub Workflow Coordination</summary>

#### Initialize GitHub Swarm
```javascript
// Step 1: Initialize swarm coordination
mcp__claude-flow__swarm_init {
  topology: "hierarchical",
  maxAgents: 8
}

// Step 2: Spawn specialized agents
mcp__claude-flow__agent_spawn { type: "coordinator", name: "GitHub Coordinator" }
mcp__claude-flow__agent_spawn { type: "reviewer", name: "Code Reviewer" }
mcp__claude-flow__agent_spawn { type: "tester", name: "QA Agent" }
mcp__claude-flow__agent_spawn { type: "analyst", name: "Security Analyst" }

// Step 3: Orchestrate GitHub workflow
mcp__claude-flow__task_orchestrate {
  task: "Complete PR review and merge workflow",
  strategy: "parallel",
  priority: "high"
}
```

#### GitHub Hooks Integration
```bash
# Pre-task: Setup GitHub context
npx claude-flow@alpha hooks pre-task \
  --description "PR review workflow" \
  --context "pr-123"

# During task: Track progress
npx claude-flow@alpha hooks notify \
  --message "Completed security scan" \
  --type "github-action"

# Post-task: Export results
npx claude-flow@alpha hooks post-task \
  --task-id "pr-review-123" \
  --export-github-summary
```

</details>

### 📦 Batch Operations

<details>
<summary>Concurrent GitHub Operations</summary>

#### Parallel GitHub CLI Commands
```javascript
// Single message with all GitHub operations
[Concurrent Execution]:
  Bash("gh issue create --title 'Feature A' --body 'Description A' --label 'enhancement'")
  Bash("gh issue create --title 'Feature B' --body 'Description B' --label 'enhancement'")
  Bash("gh pr create --title 'PR 1' --head 'feature-a' --base 'main'")
  Bash("gh pr create --title 'PR 2' --head 'feature-b' --base 'main'")
  Bash("gh pr checks 123 --watch")
  TodoWrite { todos: [
    {content: "Review security scan results", status: "pending"},
    {content: "Merge approved PRs", status: "pending"},
    {content: "Update changelog", status: "pending"}
  ]}
```

</details>

## Best Practices

### 🏗️ Workflow Organization

<details>
<summary>Structure Your GitHub Workflows</summary>

#### 1. Use Reusable Workflows
```yaml
# .github/workflows/reusable-swarm.yml
name: Reusable Swarm Workflow
on:
  workflow_call:
    inputs:
      topology:
        required: true
        type: string

jobs:
  swarm-task:
    runs-on: ubuntu-latest
    steps:
      - name: Initialize Swarm
        run: |
          npx ruv-swarm init --topology ${{ inputs.topology }}
```

#### 2. Implement Proper Caching
```yaml
- name: Cache Swarm Dependencies
  uses: actions/cache@v3
  with:
    path: ~/.npm
    key: ${{ runner.os }}-swarm-${{ hashFiles('**/package-lock.json') }}
```

#### 3. Set Appropriate Timeouts
```yaml
jobs:
  swarm-task:
    timeout-minutes: 30
    steps:
      - name: Swarm Operation
        timeout-minutes: 10
```

#### 4. Use Workflow Dependencies
```yaml
jobs:
  setup:
    runs-on: ubuntu-latest

  test:
    needs: setup
    runs-on: ubuntu-latest

  deploy:
    needs: [setup, test]
    runs-on: ubuntu-latest
```

</details>

### 🔒 Security Best Practices

<details>
<summary>Secure Your GitHub Workflows</summary>

#### 1. Store Configurations Securely
```yaml
- name: Setup Swarm
  env:
    SWARM_CONFIG: ${{ secrets.SWARM_CONFIG }}
    API_KEY: ${{ secrets.API_KEY }}
  run: |
    npx ruv-swarm init --config "$SWARM_CONFIG"
```

#### 2. Use OIDC Authentication
```yaml
permissions:
  id-token: write
  contents: read

- name: Configure AWS Credentials
  uses: aws-actions/configure-aws-credentials@v2
  with:
    role-to-assume: arn:aws:iam::123456789012:role/GitHubAction
    aws-region: us-east-1
```

#### 3. Implement Least-Privilege
```yaml
permissions:
  contents: read
  pull-requests: write
  issues: write
```

#### 4. Audit Swarm Operations
```yaml
- name: Audit Swarm Actions
  run: |
    npx ruv-swarm actions audit \
      --export-logs \
      --compliance-report
```

</details>

### ⚡ Performance Optimization

<details>
<summary>Maximize Workflow Performance</summary>

#### 1. Cache Swarm Dependencies
```yaml
- uses: actions/cache@v3
  with:
    path: |
      ~/.npm
      node_modules
    key: ${{ runner.os }}-swarm-${{ hashFiles('**/package-lock.json') }}
```

#### 2. Use Appropriate Runner Sizes
```yaml
jobs:
  heavy-task:
    runs-on: ubuntu-latest-4-cores
    steps:
      - name: Intensive Swarm Operation
```

#### 3. Implement Early Termination
```yaml
- name: Quick Fail Check
  run: |
    if ! npx ruv-swarm actions pre-check; then
      echo "Pre-check failed, terminating early"
      exit 1
    fi
```

#### 4. Optimize Parallel Execution
```yaml
strategy:
  matrix:
    include:
      - runner: ubuntu-latest
        task: test
      - runner: ubuntu-latest
        task: lint
      - runner: ubuntu-latest
        task: security
  max-parallel: 3
```

</details>

## Debugging & Troubleshooting

### 🐛 Debug Tools

<details>
<summary>Debug GitHub Workflow Issues</summary>

#### Debug Mode
```yaml
- name: Debug Swarm
  run: |
    npx ruv-swarm actions debug \
      --verbose \
      --trace-agents \
      --export-logs
  env:
    ACTIONS_STEP_DEBUG: true
```

#### Performance Profiling
```bash
# Profile workflow performance
npx ruv-swarm actions profile \
  --workflow "ci.yml" \
  --identify-slow-steps \
  --suggest-optimizations
```

#### Failure Analysis
```bash
# Analyze failed runs
gh run view <run-id> --json jobs,conclusion | \
  npx ruv-swarm actions analyze-failure \
    --suggest-fixes \
    --auto-retry-flaky
```

#### Log Analysis
```bash
# Download and analyze logs
gh run download <run-id>
npx ruv-swarm actions analyze-logs \
  --directory ./logs \
  --identify-errors
```

</details>

## Real-World Examples

### 🚀 Complete Workflows

<details>
<summary>Production-Ready Integration Examples</summary>

#### Example 1: Full-Stack Application CI/CD
```yaml
name: Full-Stack CI/CD with Swarms
on:
  push:
    branches: [main, develop]
  pull_request:

jobs:
  initialize:
    runs-on: ubuntu-latest
    outputs:
      swarm-id: ${{ steps.init.outputs.swarm-id }}
    steps:
      - id: init
        run: |
          SWARM_ID=$(npx ruv-swarm init --topology mesh --output json | jq -r '.id')
          echo "swarm-id=${SWARM_ID}" >> $GITHUB_OUTPUT

  backend:
    needs: initialize
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Backend Tests
        run: |
          npx ruv-swarm agents spawn --type tester \
            --task "Run backend test suite" \
            --swarm-id ${{ needs.initialize.outputs.swarm-id }}

  frontend:
    needs: initialize
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Frontend Tests
        run: |
          npx ruv-swarm agents spawn --type tester \
            --task "Run frontend test suite" \
            --swarm-id ${{ needs.initialize.outputs.swarm-id }}

  security:
    needs: initialize
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Security Scan
        run: |
          npx ruv-swarm agents spawn --type security \
            --task "Security audit" \
            --swarm-id ${{ needs.initialize.outputs.swarm-id }}

  deploy:
    needs: [backend, frontend, security]
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    steps:
      - name: Deploy
        run: |
          npx ruv-swarm actions deploy \
            --strategy progressive \
            --swarm-id ${{ needs.initialize.outputs.swarm-id }}
```

#### Example 2: Monorepo Management
```yaml
name: Monorepo Coordination
on: push

jobs:
  detect-changes:
    runs-on: ubuntu-latest
    outputs:
      packages: ${{ steps.detect.outputs.packages }}
    steps:
      - uses: actions/checkout@v3
        with:
          fetch-depth: 0

      - id: detect
        run: |
          PACKAGES=$(npx ruv-swarm actions detect-changes \
            --monorepo \
            --output json)
          echo "packages=${PACKAGES}" >> $GITHUB_OUTPUT

  build-packages:
    needs: detect-changes
    runs-on: ubuntu-latest
    strategy:
      matrix:
        package: ${{ fromJson(needs.detect-changes.outputs.packages) }}
    steps:
      - name: Build Package
        run: |
          npx ruv-swarm actions build \
            --package ${{ matrix.package }} \
            --parallel-deps
```

#### Example 3: Multi-Repo Synchronization
```bash
# Synchronize multiple repositories
npx claude-flow@alpha github sync-coordinator \
  "Synchronize version updates across:
   - github.com/org/repo-a
   - github.com/org/repo-b
   - github.com/org/repo-c

   Update dependencies, align versions, create PRs"
```

</details>

## Command Reference

### 📚 Quick Command Guide

<details>
<summary>All Available Commands</summary>

#### Workflow Generation
```bash
npx ruv-swarm actions generate-workflow [options]
  --analyze-codebase       Analyze repository structure
  --detect-languages       Detect programming languages
  --create-optimal-pipeline Generate optimized workflow
```

#### Optimization
```bash
npx ruv-swarm actions optimize [options]
  --workflow <path>        Path to workflow file
  --suggest-parallelization Suggest parallel execution
  --reduce-redundancy      Remove redundant steps
  --estimate-savings       Estimate time/cost savings
```

#### Analysis
```bash
npx ruv-swarm actions analyze [options]
  --commit <sha>           Analyze specific commit
  --suggest-tests          Suggest test improvements
  --optimize-pipeline      Optimize pipeline structure
```

#### Testing
```bash
npx ruv-swarm actions smart-test [options]
  --changed-files <files>  Files that changed
  --impact-analysis        Analyze test impact
  --parallel-safe          Only parallel-safe tests
```

#### Security
```bash
npx ruv-swarm actions security [options]
  --deep-scan             Deep security analysis
  --format <format>       Output format (json/text)
  --create-issues         Auto-create GitHub issues
```

#### Deployment
```bash
npx ruv-swarm actions deploy [options]
  --strategy <type>       Deployment strategy
  --risk <level>          Risk assessment level
  --auto-execute          Execute automatically
```

#### Monitoring
```bash
npx ruv-swarm actions analytics [options]
  --workflow <name>       Workflow to analyze
  --period <duration>     Analysis period
  --identify-bottlenecks  Find bottlenecks
  --suggest-improvements  Improvement suggestions
```

</details>

## Integration Checklist

### ✅ Setup Verification

<details>
<summary>Verify Your Setup</summary>

- [ ] GitHub CLI (`gh`) installed and authenticated
- [ ] Git configured with user credentials
- [ ] Node.js v16+ installed
- [ ] `claude-flow@alpha` package available
- [ ] Repository has `.github/workflows` directory
- [ ] GitHub Actions enabled on repository
- [ ] Necessary secrets configured
- [ ] Runner permissions verified

#### Quick Setup Script
```bash
#!/bin/bash
# setup-github-automation.sh

# Install dependencies
npm install -g claude-flow@alpha

# Verify GitHub CLI
gh auth status || gh auth login

# Create workflow directory
mkdir -p .github/workflows

# Generate initial workflow
npx ruv-swarm actions generate-workflow \
  --analyze-codebase \
  --create-optimal-pipeline > .github/workflows/ci.yml

echo "✅ GitHub workflow automation setup complete"
```

</details>

## Related Skills

- `github-pr-enhancement` - Advanced PR management
- `release-coordination` - Release automation
- `swarm-coordination` - Multi-agent orchestration
- `ci-cd-optimization` - Pipeline optimization

## Support & Documentation

- **GitHub CLI Docs**: https://cli.github.com/manual/
- **GitHub Actions**: https://docs.github.com/en/actions
- **Claude-Flow**: https://github.com/ruvnet/claude-flow
- **Ruv-Swarm**: https://github.com/ruvnet/ruv-swarm

## Version History

- **v1.0.0** (2025-01-19): Initial skill consolidation
  - Merged workflow-automation.md (441 lines)
  - Merged github-modes.md (146 lines)
  - Added progressive disclosure
  - Enhanced with swarm coordination patterns
  - Added comprehensive examples and best practices

---

**Skill Status**: ✅ Production Ready
**Last Updated**: 2025-01-19
**Maintainer**: claude-flow team
