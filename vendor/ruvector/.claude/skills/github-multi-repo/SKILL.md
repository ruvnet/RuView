---
name: github-multi-repo
version: 1.0.0
description: Multi-repository coordination, synchronization, and architecture management with AI swarm orchestration
category: github-integration
tags: [multi-repo, synchronization, architecture, coordination, github]
author: Claude Flow Team
requires:
  - ruv-swarm@^1.0.11
  - gh-cli@^2.0.0
capabilities:
  - cross-repository coordination
  - package synchronization
  - architecture optimization
  - template management
  - distributed workflows
---

# GitHub Multi-Repository Coordination Skill

## Overview

Advanced multi-repository coordination system that combines swarm intelligence, package synchronization, and repository architecture optimization. This skill enables organization-wide automation, cross-project collaboration, and scalable repository management.

## Core Capabilities

### 🔄 Multi-Repository Swarm Coordination
Cross-repository AI swarm orchestration for distributed development workflows.

### 📦 Package Synchronization
Intelligent dependency resolution and version alignment across multiple packages.

### 🏗️ Repository Architecture
Structure optimization and template management for scalable projects.

### 🔗 Integration Management
Cross-package integration testing and deployment coordination.

## Quick Start

### Initialize Multi-Repo Coordination
```bash
# Basic swarm initialization
npx claude-flow skill run github-multi-repo init \
  --repos "org/frontend,org/backend,org/shared" \
  --topology hierarchical

# Advanced initialization with synchronization
npx claude-flow skill run github-multi-repo init \
  --repos "org/frontend,org/backend,org/shared" \
  --topology mesh \
  --shared-memory \
  --sync-strategy eventual
```

### Synchronize Packages
```bash
# Synchronize package versions and dependencies
npx claude-flow skill run github-multi-repo sync \
  --packages "claude-code-flow,ruv-swarm" \
  --align-versions \
  --update-docs
```

### Optimize Architecture
```bash
# Analyze and optimize repository structure
npx claude-flow skill run github-multi-repo optimize \
  --analyze-structure \
  --suggest-improvements \
  --create-templates
```

## Features

### 1. Cross-Repository Swarm Orchestration

#### Repository Discovery
```javascript
// Auto-discover related repositories with gh CLI
const REPOS = Bash(`gh repo list my-organization --limit 100 \
  --json name,description,languages,topics \
  --jq '.[] | select(.languages | keys | contains(["TypeScript"]))'`)

// Analyze repository dependencies
const DEPS = Bash(`gh repo list my-organization --json name | \
  jq -r '.[].name' | while read -r repo; do
    gh api repos/my-organization/$repo/contents/package.json \
      --jq '.content' 2>/dev/null | base64 -d | jq '{name, dependencies}'
  done | jq -s '.'`)

// Initialize swarm with discovered repositories
mcp__claude-flow__swarm_init({
  topology: "hierarchical",
  maxAgents: 8,
  metadata: { repos: REPOS, dependencies: DEPS }
})
```

#### Synchronized Operations
```javascript
// Execute synchronized changes across repositories
[Parallel Multi-Repo Operations]:
  // Spawn coordination agents
  Task("Repository Coordinator", "Coordinate changes across all repositories", "coordinator")
  Task("Dependency Analyzer", "Analyze cross-repo dependencies", "analyst")
  Task("Integration Tester", "Validate cross-repo changes", "tester")

  // Get matching repositories
  Bash(`gh repo list org --limit 100 --json name \
    --jq '.[] | select(.name | test("-service$")) | .name' > /tmp/repos.txt`)

  // Execute task across repositories
  Bash(`cat /tmp/repos.txt | while read -r repo; do
    gh repo clone org/$repo /tmp/$repo -- --depth=1
    cd /tmp/$repo

    # Apply changes
    npm update
    npm test

    # Create PR if successful
    if [ $? -eq 0 ]; then
      git checkout -b update-dependencies-$(date +%Y%m%d)
      git add -A
      git commit -m "chore: Update dependencies"
      git push origin HEAD
      gh pr create --title "Update dependencies" --body "Automated update" --label "dependencies"
    fi
  done`)

  // Track all operations
  TodoWrite { todos: [
    { id: "discover", content: "Discover all service repositories", status: "completed" },
    { id: "update", content: "Update dependencies", status: "completed" },
    { id: "test", content: "Run integration tests", status: "in_progress" },
    { id: "pr", content: "Create pull requests", status: "pending" }
  ]}
```

### 2. Package Synchronization

#### Version Alignment
```javascript
// Synchronize package dependencies and versions
[Complete Package Sync]:
  // Initialize sync swarm
  mcp__claude-flow__swarm_init({ topology: "mesh", maxAgents: 5 })

  // Spawn sync agents
  Task("Sync Coordinator", "Coordinate version alignment", "coordinator")
  Task("Dependency Analyzer", "Analyze dependencies", "analyst")
  Task("Integration Tester", "Validate synchronization", "tester")

  // Read package states
  Read("/workspaces/ruv-FANN/claude-code-flow/claude-code-flow/package.json")
  Read("/workspaces/ruv-FANN/ruv-swarm/npm/package.json")

  // Align versions using gh CLI
  Bash(`gh api repos/:owner/:repo/git/refs \
    -f ref='refs/heads/sync/package-alignment' \
    -f sha=$(gh api repos/:owner/:repo/git/refs/heads/main --jq '.object.sha')`)

  // Update package.json files
  Bash(`gh api repos/:owner/:repo/contents/package.json \
    --method PUT \
    -f message="feat: Align Node.js version requirements" \
    -f branch="sync/package-alignment" \
    -f content="$(cat aligned-package.json | base64)"`)

  // Store sync state
  mcp__claude-flow__memory_usage({
    action: "store",
    key: "sync/packages/status",
    value: {
      timestamp: Date.now(),
      packages_synced: ["claude-code-flow", "ruv-swarm"],
      status: "synchronized"
    }
  })
```

#### Documentation Synchronization
```javascript
// Synchronize CLAUDE.md files across packages
[Documentation Sync]:
  // Get source documentation
  Bash(`gh api repos/:owner/:repo/contents/ruv-swarm/docs/CLAUDE.md \
    --jq '.content' | base64 -d > /tmp/claude-source.md`)

  // Update target documentation
  Bash(`gh api repos/:owner/:repo/contents/claude-code-flow/CLAUDE.md \
    --method PUT \
    -f message="docs: Synchronize CLAUDE.md" \
    -f branch="sync/documentation" \
    -f content="$(cat /tmp/claude-source.md | base64)"`)

  // Track sync status
  mcp__claude-flow__memory_usage({
    action: "store",
    key: "sync/documentation/status",
    value: { status: "synchronized", files: ["CLAUDE.md"] }
  })
```

#### Cross-Package Integration
```javascript
// Coordinate feature implementation across packages
[Cross-Package Feature]:
  // Push changes to all packages
  mcp__github__push_files({
    branch: "feature/github-integration",
    files: [
      {
        path: "claude-code-flow/.claude/commands/github/github-modes.md",
        content: "[GitHub modes documentation]"
      },
      {
        path: "ruv-swarm/src/github-coordinator/hooks.js",
        content: "[GitHub coordination hooks]"
      }
    ],
    message: "feat: Add GitHub workflow integration"
  })

  // Create coordinated PR
  Bash(`gh pr create \
    --title "Feature: GitHub Workflow Integration" \
    --body "## 🚀 GitHub Integration

### Features
- ✅ Multi-repo coordination
- ✅ Package synchronization
- ✅ Architecture optimization

### Testing
- [x] Package dependency verification
- [x] Integration tests
- [x] Cross-package compatibility"`)
```

### 3. Repository Architecture

#### Structure Analysis
```javascript
// Analyze and optimize repository structure
[Architecture Analysis]:
  // Initialize architecture swarm
  mcp__claude-flow__swarm_init({ topology: "hierarchical", maxAgents: 6 })

  // Spawn architecture agents
  Task("Senior Architect", "Analyze repository structure", "architect")
  Task("Structure Analyst", "Identify optimization opportunities", "analyst")
  Task("Performance Optimizer", "Optimize structure for scalability", "optimizer")
  Task("Best Practices Researcher", "Research architecture patterns", "researcher")

  // Analyze current structures
  LS("/workspaces/ruv-FANN/claude-code-flow/claude-code-flow")
  LS("/workspaces/ruv-FANN/ruv-swarm/npm")

  // Search for best practices
  Bash(`gh search repos "language:javascript template architecture" \
    --limit 10 \
    --json fullName,description,stargazersCount \
    --sort stars \
    --order desc`)

  // Store analysis results
  mcp__claude-flow__memory_usage({
    action: "store",
    key: "architecture/analysis/results",
    value: {
      repositories_analyzed: ["claude-code-flow", "ruv-swarm"],
      optimization_areas: ["structure", "workflows", "templates"],
      recommendations: ["standardize_structure", "improve_workflows"]
    }
  })
```

#### Template Creation
```javascript
// Create standardized repository template
[Template Creation]:
  // Create template repository
  mcp__github__create_repository({
    name: "claude-project-template",
    description: "Standardized template for Claude Code projects",
    private: false,
    autoInit: true
  })

  // Push template structure
  mcp__github__push_files({
    repo: "claude-project-template",
    files: [
      {
        path: ".claude/commands/github/github-modes.md",
        content: "[GitHub modes template]"
      },
      {
        path: ".claude/config.json",
        content: JSON.stringify({
          version: "1.0",
          mcp_servers: {
            "ruv-swarm": {
              command: "npx",
              args: ["ruv-swarm", "mcp", "start"]
            }
          }
        })
      },
      {
        path: "CLAUDE.md",
        content: "[Standardized CLAUDE.md]"
      },
      {
        path: "package.json",
        content: JSON.stringify({
          name: "claude-project-template",
          engines: { node: ">=20.0.0" },
          dependencies: { "ruv-swarm": "^1.0.11" }
        })
      }
    ],
    message: "feat: Create standardized template"
  })
```

#### Cross-Repository Standardization
```javascript
// Synchronize structure across repositories
[Structure Standardization]:
  const repositories = ["claude-code-flow", "ruv-swarm", "claude-extensions"]

  // Update common files across all repositories
  repositories.forEach(repo => {
    mcp__github__create_or_update_file({
      repo: "ruv-FANN",
      path: `${repo}/.github/workflows/integration.yml`,
      content: `name: Integration Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with: { node-version: '20' }
      - run: npm install && npm test`,
      message: "ci: Standardize integration workflow",
      branch: "structure/standardization"
    })
  })
```

### 4. Orchestration Workflows

#### Dependency Management
```javascript
// Update dependencies across all repositories
[Organization-Wide Dependency Update]:
  // Create tracking issue
  TRACKING_ISSUE=$(Bash(`gh issue create \
    --title "Dependency Update: typescript@5.0.0" \
    --body "Tracking TypeScript update across all repositories" \
    --label "dependencies,tracking" \
    --json number -q .number`))

  // Find all TypeScript repositories
  TS_REPOS=$(Bash(`gh repo list org --limit 100 --json name | \
    jq -r '.[].name' | while read -r repo; do
      if gh api repos/org/$repo/contents/package.json 2>/dev/null | \
         jq -r '.content' | base64 -d | grep -q '"typescript"'; then
        echo "$repo"
      fi
    done`))

  // Update each repository
  Bash(`echo "$TS_REPOS" | while read -r repo; do
    gh repo clone org/$repo /tmp/$repo -- --depth=1
    cd /tmp/$repo

    npm install --save-dev typescript@5.0.0

    if npm test; then
      git checkout -b update-typescript-5
      git add package.json package-lock.json
      git commit -m "chore: Update TypeScript to 5.0.0

Part of #$TRACKING_ISSUE"

      git push origin HEAD
      gh pr create \
        --title "Update TypeScript to 5.0.0" \
        --body "Updates TypeScript\n\nTracking: #$TRACKING_ISSUE" \
        --label "dependencies"
    else
      gh issue comment $TRACKING_ISSUE \
        --body "❌ Failed to update $repo - tests failing"
    fi
  done`)
```

#### Refactoring Operations
```javascript
// Coordinate large-scale refactoring
[Cross-Repo Refactoring]:
  // Initialize refactoring swarm
  mcp__claude-flow__swarm_init({ topology: "mesh", maxAgents: 8 })

  // Spawn specialized agents
  Task("Refactoring Coordinator", "Coordinate refactoring across repos", "coordinator")
  Task("Impact Analyzer", "Analyze refactoring impact", "analyst")
  Task("Code Transformer", "Apply refactoring changes", "coder")
  Task("Migration Guide Creator", "Create migration documentation", "documenter")
  Task("Integration Tester", "Validate refactored code", "tester")

  // Execute refactoring
  mcp__claude-flow__task_orchestrate({
    task: "Rename OldAPI to NewAPI across all repositories",
    strategy: "sequential",
    priority: "high"
  })
```

#### Security Updates
```javascript
// Coordinate security patches
[Security Patch Deployment]:
  // Scan all repositories
  Bash(`gh repo list org --limit 100 --json name | jq -r '.[].name' | \
    while read -r repo; do
      gh repo clone org/$repo /tmp/$repo -- --depth=1
      cd /tmp/$repo
      npm audit --json > /tmp/audit-$repo.json
    done`)

  // Apply patches
  Bash(`for repo in /tmp/audit-*.json; do
    if [ $(jq '.vulnerabilities | length' $repo) -gt 0 ]; then
      cd /tmp/$(basename $repo .json | sed 's/audit-//')
      npm audit fix

      if npm test; then
        git checkout -b security/patch-$(date +%Y%m%d)
        git add -A
        git commit -m "security: Apply security patches"
        git push origin HEAD
        gh pr create --title "Security patches" --label "security"
      fi
    fi
  done`)
```

## Configuration

### Multi-Repo Config File
```yaml
# .swarm/multi-repo.yml
version: 1
organization: my-org

repositories:
  - name: frontend
    url: github.com/my-org/frontend
    role: ui
    agents: [coder, designer, tester]

  - name: backend
    url: github.com/my-org/backend
    role: api
    agents: [architect, coder, tester]

  - name: shared
    url: github.com/my-org/shared
    role: library
    agents: [analyst, coder]

coordination:
  topology: hierarchical
  communication: webhook
  memory: redis://shared-memory

dependencies:
  - from: frontend
    to: [backend, shared]
  - from: backend
    to: [shared]
```

### Repository Roles
```javascript
{
  "roles": {
    "ui": {
      "responsibilities": ["user-interface", "ux", "accessibility"],
      "default-agents": ["designer", "coder", "tester"]
    },
    "api": {
      "responsibilities": ["endpoints", "business-logic", "data"],
      "default-agents": ["architect", "coder", "security"]
    },
    "library": {
      "responsibilities": ["shared-code", "utilities", "types"],
      "default-agents": ["analyst", "coder", "documenter"]
    }
  }
}
```

## Communication Strategies

### 1. Webhook-Based Coordination
```javascript
const { MultiRepoSwarm } = require('ruv-swarm');

const swarm = new MultiRepoSwarm({
  webhook: {
    url: 'https://swarm-coordinator.example.com',
    secret: process.env.WEBHOOK_SECRET
  }
});

swarm.on('repo:update', async (event) => {
  await swarm.propagate(event, {
    to: event.dependencies,
    strategy: 'eventual-consistency'
  });
});
```

### 2. Event Streaming
```yaml
# Kafka configuration for real-time coordination
kafka:
  brokers: ['kafka1:9092', 'kafka2:9092']
  topics:
    swarm-events:
      partitions: 10
      replication: 3
    swarm-memory:
      partitions: 5
      replication: 3
```

## Synchronization Patterns

### 1. Eventually Consistent
```javascript
{
  "sync": {
    "strategy": "eventual",
    "max-lag": "5m",
    "retry": {
      "attempts": 3,
      "backoff": "exponential"
    }
  }
}
```

### 2. Strong Consistency
```javascript
{
  "sync": {
    "strategy": "strong",
    "consensus": "raft",
    "quorum": 0.51,
    "timeout": "30s"
  }
}
```

### 3. Hybrid Approach
```javascript
{
  "sync": {
    "default": "eventual",
    "overrides": {
      "security-updates": "strong",
      "dependency-updates": "strong",
      "documentation": "eventual"
    }
  }
}
```

## Use Cases

### 1. Microservices Coordination
```bash
npx claude-flow skill run github-multi-repo microservices \
  --services "auth,users,orders,payments" \
  --ensure-compatibility \
  --sync-contracts \
  --integration-tests
```

### 2. Library Updates
```bash
npx claude-flow skill run github-multi-repo lib-update \
  --library "org/shared-lib" \
  --version "2.0.0" \
  --find-consumers \
  --update-imports \
  --run-tests
```

### 3. Organization-Wide Changes
```bash
npx claude-flow skill run github-multi-repo org-policy \
  --policy "add-security-headers" \
  --repos "org/*" \
  --validate-compliance \
  --create-reports
```

## Architecture Patterns

### Monorepo Structure
```
ruv-FANN/
├── packages/
│   ├── claude-code-flow/
│   │   ├── src/
│   │   ├── .claude/
│   │   └── package.json
│   ├── ruv-swarm/
│   │   ├── src/
│   │   ├── wasm/
│   │   └── package.json
│   └── shared/
│       ├── types/
│       ├── utils/
│       └── config/
├── tools/
│   ├── build/
│   ├── test/
│   └── deploy/
├── docs/
│   ├── architecture/
│   ├── integration/
│   └── examples/
└── .github/
    ├── workflows/
    ├── templates/
    └── actions/
```

### Command Structure
```
.claude/
├── commands/
│   ├── github/
│   │   ├── github-modes.md
│   │   ├── pr-manager.md
│   │   ├── issue-tracker.md
│   │   └── sync-coordinator.md
│   ├── sparc/
│   │   ├── sparc-modes.md
│   │   ├── coder.md
│   │   └── tester.md
│   └── swarm/
│       ├── coordination.md
│       └── orchestration.md
├── templates/
│   ├── issue.md
│   ├── pr.md
│   └── project.md
└── config.json
```

## Monitoring & Visualization

### Multi-Repo Dashboard
```bash
npx claude-flow skill run github-multi-repo dashboard \
  --port 3000 \
  --metrics "agent-activity,task-progress,memory-usage" \
  --real-time
```

### Dependency Graph
```bash
npx claude-flow skill run github-multi-repo dep-graph \
  --format mermaid \
  --include-agents \
  --show-data-flow
```

### Health Monitoring
```bash
npx claude-flow skill run github-multi-repo health-check \
  --repos "org/*" \
  --check "connectivity,memory,agents" \
  --alert-on-issues
```

## Best Practices

### 1. Repository Organization
- Clear repository roles and boundaries
- Consistent naming conventions
- Documented dependencies
- Shared configuration standards

### 2. Communication
- Use appropriate sync strategies
- Implement circuit breakers
- Monitor latency and failures
- Clear error propagation

### 3. Security
- Secure cross-repo authentication
- Encrypted communication channels
- Audit trail for all operations
- Principle of least privilege

### 4. Version Management
- Semantic versioning alignment
- Dependency compatibility validation
- Automated version bump coordination

### 5. Testing Integration
- Cross-package test validation
- Integration test automation
- Performance regression detection

## Performance Optimization

### Caching Strategy
```bash
npx claude-flow skill run github-multi-repo cache-strategy \
  --analyze-patterns \
  --suggest-cache-layers \
  --implement-invalidation
```

### Parallel Execution
```bash
npx claude-flow skill run github-multi-repo parallel-optimize \
  --analyze-dependencies \
  --identify-parallelizable \
  --execute-optimal
```

### Resource Pooling
```bash
npx claude-flow skill run github-multi-repo resource-pool \
  --share-agents \
  --distribute-load \
  --monitor-usage
```

## Troubleshooting

### Connectivity Issues
```bash
npx claude-flow skill run github-multi-repo diagnose-connectivity \
  --test-all-repos \
  --check-permissions \
  --verify-webhooks
```

### Memory Synchronization
```bash
npx claude-flow skill run github-multi-repo debug-memory \
  --check-consistency \
  --identify-conflicts \
  --repair-state
```

### Performance Bottlenecks
```bash
npx claude-flow skill run github-multi-repo perf-analysis \
  --profile-operations \
  --identify-bottlenecks \
  --suggest-optimizations
```

## Advanced Features

### 1. Distributed Task Queue
```bash
npx claude-flow skill run github-multi-repo queue \
  --backend redis \
  --workers 10 \
  --priority-routing \
  --dead-letter-queue
```

### 2. Cross-Repo Testing
```bash
npx claude-flow skill run github-multi-repo test \
  --setup-test-env \
  --link-services \
  --run-e2e \
  --tear-down
```

### 3. Monorepo Migration
```bash
npx claude-flow skill run github-multi-repo to-monorepo \
  --analyze-repos \
  --suggest-structure \
  --preserve-history \
  --create-migration-prs
```

## Examples

### Full-Stack Application Update
```bash
npx claude-flow skill run github-multi-repo fullstack-update \
  --frontend "org/web-app" \
  --backend "org/api-server" \
  --database "org/db-migrations" \
  --coordinate-deployment
```

### Cross-Team Collaboration
```bash
npx claude-flow skill run github-multi-repo cross-team \
  --teams "frontend,backend,devops" \
  --task "implement-feature-x" \
  --assign-by-expertise \
  --track-progress
```

## Metrics and Reporting

### Sync Quality Metrics
- Package version alignment percentage
- Documentation consistency score
- Integration test success rate
- Synchronization completion time

### Architecture Health Metrics
- Repository structure consistency score
- Documentation coverage percentage
- Cross-repository integration success rate
- Template adoption and usage statistics

### Automated Reporting
- Weekly sync status reports
- Dependency drift detection
- Documentation divergence alerts
- Integration health monitoring

## Integration Points

### Related Skills
- `github-workflow` - GitHub workflow automation
- `github-pr` - Pull request management
- `sparc-architect` - Architecture design
- `sparc-optimizer` - Performance optimization

### Related Commands
- `/github sync-coordinator` - Cross-repo synchronization
- `/github release-manager` - Coordinated releases
- `/github repo-architect` - Repository optimization
- `/sparc architect` - Detailed architecture design

## Support and Resources

- Documentation: https://github.com/ruvnet/claude-flow
- Issues: https://github.com/ruvnet/claude-flow/issues
- Examples: `.claude/examples/github-multi-repo/`

---

**Version:** 1.0.0
**Last Updated:** 2025-10-19
**Maintainer:** Claude Flow Team
