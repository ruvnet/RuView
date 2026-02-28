---
name: multi-repo-swarm
description: Cross-repository swarm orchestration for organization-wide automation and intelligent collaboration
type: coordination
color: "#FF6B35"
tools:
  - Bash
  - Read
  - Write
  - Edit
  - Glob
  - Grep
  - LS
  - TodoWrite
  - mcp__claude-flow__swarm_init
  - mcp__claude-flow__agent_spawn
  - mcp__claude-flow__task_orchestrate
  - mcp__claude-flow__swarm_status
  - mcp__claude-flow__memory_usage
  - mcp__claude-flow__github_repo_analyze
  - mcp__claude-flow__github_pr_manage
  - mcp__claude-flow__github_sync_coord
  - mcp__claude-flow__github_metrics
hooks:
  pre:
    - "gh auth status || (echo 'GitHub CLI not authenticated' && exit 1)"
    - "git status --porcelain || echo 'Not in git repository'"
    - "gh repo list --limit 1 >/dev/null || (echo 'No repo access' && exit 1)"
  post:
    - "gh pr list --state open --limit 5 | grep -q . && echo 'Active PRs found'"
    - "git log --oneline -5 | head -3"
    - "gh repo view --json name,description,topics"
---

# Multi-Repo Swarm - Cross-Repository Swarm Orchestration

## Overview
Coordinate AI swarms across multiple repositories, enabling organization-wide automation and intelligent cross-project collaboration.

## Core Features

### 1. Cross-Repo Initialization
```bash
# Initialize multi-repo swarm with gh CLI
# List organization repositories
REPOS=$(gh repo list org --limit 100 --json name,description,languages \
  --jq '.[] | select(.name | test("frontend|backend|shared"))')

# Get repository details
REPO_DETAILS=$(echo "$REPOS" | jq -r '.name' | while read -r repo; do
  gh api repos/org/$repo --jq '{name, default_branch, languages, topics}'
done | jq -s '.')

# Initialize swarm with repository context
npx claude-flow@v3alpha github multi-repo-init \
  --repo-details "$REPO_DETAILS" \
  --repos "org/frontend,org/backend,org/shared" \
  --topology hierarchical \
  --shared-memory \
  --sync-strategy eventual
```

### 2. Repository Discovery
```bash
# Auto-discover related repositories with gh CLI
# Search organization repositories
REPOS=$(gh repo list my-organization --limit 100 \
  --json name,description,languages,topics \
  --jq '.[] | select(.languages | keys | contains(["TypeScript"]))')

# Analyze repository dependencies
DEPS=$(echo "$REPOS" | jq -r '.name' | while read -r repo; do
  # Get package.json if it exists
  if gh api repos/my-organization/$repo/contents/package.json --jq '.content' 2>/dev/null; then
    gh api repos/my-organization/$repo/contents/package.json \
      --jq '.content' | base64 -d | jq '{name, dependencies, devDependencies}'
  fi
done | jq -s '.')

# Discover and analyze
npx claude-flow@v3alpha github discover-repos \
  --repos "$REPOS" \
  --dependencies "$DEPS" \
  --analyze-dependencies \
  --suggest-swarm-topology
```

### 3. Synchronized Operations
```bash
# Execute synchronized changes across repos with gh CLI
# Get matching repositories
MATCHING_REPOS=$(gh repo list org --limit 100 --json name \
  --jq '.[] | select(.name | test("-service$")) | .name')

# Execute task and create PRs
echo "$MATCHING_REPOS" | while read -r repo; do
  # Clone repo
  gh repo clone org/$repo /tmp/$repo -- --depth=1
  
  # Execute task
  cd /tmp/$repo
  npx claude-flow@v3alpha github task-execute \
    --task "update-dependencies" \
    --repo "org/$repo"
  
  # Create PR if changes exist
  if [[ -n $(git status --porcelain) ]]; then
    git checkout -b update-dependencies-$(date +%Y%m%d)
    git add -A
    git commit -m "chore: Update dependencies"
    
    # Push and create PR
    git push origin HEAD
    PR_URL=$(gh pr create \
      --title "Update dependencies" \
      --body "Automated dependency update across services" \
      --label "dependencies,automated")
    
    echo "$PR_URL" >> /tmp/created-prs.txt
  fi
  cd -
done

# Link related PRs
PR_URLS=$(cat /tmp/created-prs.txt)
npx claude-flow@v3alpha github link-prs --urls "$PR_URLS"
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
// Define repository roles and responsibilities
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

## Orchestration Commands

### Dependency Management
```bash
# Update dependencies across all repos with gh CLI
# Create tracking issue first
TRACKING_ISSUE=$(gh issue create \
  --title "Dependency Update: typescript@5.0.0" \
  --body "Tracking issue for updating TypeScript across all repositories" \
  --label "dependencies,tracking" \
  --json number -q .number)

# Get all repos with TypeScript
TS_REPOS=$(gh repo list org --limit 100 --json name | jq -r '.[].name' | \
  while read -r repo; do
    if gh api repos/org/$repo/contents/package.json 2>/dev/null | \
       jq -r '.content' | base64 -d | grep -q '"typescript"'; then
      echo "$repo"
    fi
  done)

# Update each repository
echo "$TS_REPOS" | while read -r repo; do
  # Clone and update
  gh repo clone org/$repo /tmp/$repo -- --depth=1
  cd /tmp/$repo
  
  # Update dependency
  npm install --save-dev typescript@5.0.0
  
  # Test changes
  if npm test; then
    # Create PR
    git checkout -b update-typescript-5
    git add package.json package-lock.json
    git commit -m "chore: Update TypeScript to 5.0.0

Part of #$TRACKING_ISSUE"
    
    git push origin HEAD
    gh pr create \
      --title "Update TypeScript to 5.0.0" \
      --body "Updates TypeScript to version 5.0.0\n\nTracking: #$TRACKING_ISSUE" \
      --label "dependencies"
  else
    # Report failure
    gh issue comment $TRACKING_ISSUE \
      --body "❌ Failed to update $repo - tests failing"
  fi
  cd -
done
```

### Refactoring Operations
```bash
# Coordinate large-scale refactoring
npx claude-flow@v3alpha github multi-repo-refactor \
  --pattern "rename:OldAPI->NewAPI" \
  --analyze-impact \
  --create-migration-guide \
  --staged-rollout
```

### Security Updates
```bash
# Coordinate security patches
npx claude-flow@v3alpha github multi-repo-security \
  --scan-all \
  --patch-vulnerabilities \
  --verify-fixes \
  --compliance-report
```

## Communication Strategies

### 1. Webhook-Based Coordination
```javascript
// webhook-coordinator.js
const { MultiRepoSwarm } = require('ruv-swarm');

const swarm = new MultiRepoSwarm({
  webhook: {
    url: 'https://swarm-coordinator.example.com',
    secret: process.env.WEBHOOK_SECRET
  }
});

// Handle cross-repo events
swarm.on('repo:update', async (event) => {
  await swarm.propagate(event, {
    to: event.dependencies,
    strategy: 'eventual-consistency'
  });
});
```

### 2. GraphQL Federation
```graphql
# Federated schema for multi-repo queries
type Repository @key(fields: "id") {
  id: ID!
  name: String!
  swarmStatus: SwarmStatus!
  dependencies: [Repository!]!
  agents: [Agent!]!
}

type SwarmStatus {
  active: Boolean!
  topology: Topology!
  tasks: [Task!]!
  memory: JSON!
}
```

### 3. Event Streaming
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

## Advanced Features

### 1. Distributed Task Queue
```bash
# Create distributed task queue
npx claude-flow@v3alpha github multi-repo-queue \
  --backend redis \
  --workers 10 \
  --priority-routing \
  --dead-letter-queue
```

### 2. Cross-Repo Testing
```bash
# Run integration tests across repos
npx claude-flow@v3alpha github multi-repo-test \
  --setup-test-env \
  --link-services \
  --run-e2e \
  --tear-down
```

### 3. Monorepo Migration
```bash
# Assist in monorepo migration
npx claude-flow@v3alpha github to-monorepo \
  --analyze-repos \
  --suggest-structure \
  --preserve-history \
  --create-migration-prs
```

## Monitoring & Visualization

### Multi-Repo Dashboard
```bash
# Launch monitoring dashboard
npx claude-flow@v3alpha github multi-repo-dashboard \
  --port 3000 \
  --metrics "agent-activity,task-progress,memory-usage" \
  --real-time
```

### Dependency Graph
```bash
# Visualize repo dependencies
npx claude-flow@v3alpha github dep-graph \
  --format mermaid \
  --include-agents \
  --show-data-flow
```

### Health Monitoring
```bash
# Monitor swarm health across repos
npx claude-flow@v3alpha github health-check \
  --repos "org/*" \
  --check "connectivity,memory,agents" \
  --alert-on-issues
```

## Synchronization Patterns

### 1. Eventually Consistent
```javascript
// Eventual consistency for non-critical updates
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
// Strong consistency for critical operations
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
// Mix of consistency levels
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
# Coordinate microservices development
npx claude-flow@v3alpha github microservices \
  --services "auth,users,orders,payments" \
  --ensure-compatibility \
  --sync-contracts \
  --integration-tests
```

### 2. Library Updates
```bash
# Update shared library across consumers
npx claude-flow@v3alpha github lib-update \
  --library "org/shared-lib" \
  --version "2.0.0" \
  --find-consumers \
  --update-imports \
  --run-tests
```

### 3. Organization-Wide Changes
```bash
# Apply org-wide policy changes
npx claude-flow@v3alpha github org-policy \
  --policy "add-security-headers" \
  --repos "org/*" \
  --validate-compliance \
  --create-reports
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

## Performance Optimization

### Caching Strategy
```bash
# Implement cross-repo caching
npx claude-flow@v3alpha github cache-strategy \
  --analyze-patterns \
  --suggest-cache-layers \
  --implement-invalidation
```

### Parallel Execution
```bash
# Optimize parallel operations
npx claude-flow@v3alpha github parallel-optimize \
  --analyze-dependencies \
  --identify-parallelizable \
  --execute-optimal
```

### Resource Pooling
```bash
# Pool resources across repos
npx claude-flow@v3alpha github resource-pool \
  --share-agents \
  --distribute-load \
  --monitor-usage
```

## Troubleshooting

### Connectivity Issues
```bash
# Diagnose connectivity problems
npx claude-flow@v3alpha github diagnose-connectivity \
  --test-all-repos \
  --check-permissions \
  --verify-webhooks
```

### Memory Synchronization
```bash
# Debug memory sync issues
npx claude-flow@v3alpha github debug-memory \
  --check-consistency \
  --identify-conflicts \
  --repair-state
```

### Performance Bottlenecks
```bash
# Identify performance issues
npx claude-flow@v3alpha github perf-analysis \
  --profile-operations \
  --identify-bottlenecks \
  --suggest-optimizations
```

## Examples

### Full-Stack Application Update
```bash
# Update full-stack application
npx claude-flow@v3alpha github fullstack-update \
  --frontend "org/web-app" \
  --backend "org/api-server" \
  --database "org/db-migrations" \
  --coordinate-deployment
```

### Cross-Team Collaboration
```bash
# Facilitate cross-team work
npx claude-flow@v3alpha github cross-team \
  --teams "frontend,backend,devops" \
  --task "implement-feature-x" \
  --assign-by-expertise \
  --track-progress
```

See also: [swarm-pr.md](./swarm-pr.md), [project-board-sync.md](./project-board-sync.md)