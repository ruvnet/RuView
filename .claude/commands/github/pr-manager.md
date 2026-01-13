# GitHub PR Manager

## Purpose
Comprehensive pull request management with ruv-swarm coordination for automated reviews, testing, and merge workflows.

## Capabilities
- **Multi-reviewer coordination** with swarm agents
- **Automated conflict resolution** and merge strategies
- **Comprehensive testing** integration and validation
- **Real-time progress tracking** with GitHub issue coordination
- **Intelligent branch management** and synchronization

## Tools Available
- `mcp__github__create_pull_request`
- `mcp__github__get_pull_request`
- `mcp__github__list_pull_requests`
- `mcp__github__create_pull_request_review`
- `mcp__github__merge_pull_request`
- `mcp__github__get_pull_request_files`
- `mcp__github__get_pull_request_status`
- `mcp__github__update_pull_request_branch`
- `mcp__github__get_pull_request_comments`
- `mcp__github__get_pull_request_reviews`
- `mcp__claude-flow__*` (all swarm coordination tools)
- `TodoWrite`, `TodoRead`, `Task`, `Bash`, `Read`, `Write`

## Usage Patterns

### 1. Create and Manage PR with Swarm Coordination
```javascript
// Initialize review swarm
mcp__claude-flow__swarm_init { topology: "mesh", maxAgents: 4 }
mcp__claude-flow__agent_spawn { type: "reviewer", name: "Code Quality Reviewer" }
mcp__claude-flow__agent_spawn { type: "tester", name: "Testing Agent" }
mcp__claude-flow__agent_spawn { type: "coordinator", name: "PR Coordinator" }

// Create PR and orchestrate review
mcp__github__create_pull_request {
  owner: "ruvnet",
  repo: "ruv-FANN",
  title: "Integration: claude-code-flow and ruv-swarm",
  head: "integration/claude-code-flow-ruv-swarm",
  base: "main",
  body: "Comprehensive integration between packages..."
}

// Orchestrate review process
mcp__claude-flow__task_orchestrate {
  task: "Complete PR review with testing and validation",
  strategy: "parallel",
  priority: "high"
}
```

### 2. Automated Multi-File Review
```javascript
// Get PR files and create parallel review tasks
mcp__github__get_pull_request_files { owner: "ruvnet", repo: "ruv-FANN", pull_number: 54 }

// Create coordinated reviews
mcp__github__create_pull_request_review {
  owner: "ruvnet",
  repo: "ruv-FANN", 
  pull_number: 54,
  body: "Automated swarm review with comprehensive analysis",
  event: "APPROVE",
  comments: [
    { path: "package.json", line: 78, body: "Dependency integration verified" },
    { path: "src/index.js", line: 45, body: "Import structure optimized" }
  ]
}
```

### 3. Merge Coordination with Testing
```javascript
// Validate PR status and merge when ready
mcp__github__get_pull_request_status { owner: "ruvnet", repo: "ruv-FANN", pull_number: 54 }

// Merge with coordination
mcp__github__merge_pull_request {
  owner: "ruvnet",
  repo: "ruv-FANN",
  pull_number: 54,
  merge_method: "squash",
  commit_title: "feat: Complete claude-code-flow and ruv-swarm integration",
  commit_message: "Comprehensive integration with swarm coordination"
}

// Post-merge coordination
mcp__claude-flow__memory_usage {
  action: "store",
  key: "pr/54/merged",
  value: { timestamp: Date.now(), status: "success" }
}
```

## Batch Operations Example

### Complete PR Lifecycle in Parallel:
```javascript
[Single Message - Complete PR Management]:
  // Initialize coordination
  mcp__claude-flow__swarm_init { topology: "hierarchical", maxAgents: 5 }
  mcp__claude-flow__agent_spawn { type: "reviewer", name: "Senior Reviewer" }
  mcp__claude-flow__agent_spawn { type: "tester", name: "QA Engineer" }
  mcp__claude-flow__agent_spawn { type: "coordinator", name: "Merge Coordinator" }
  
  // Create and manage PR using gh CLI
  Bash("gh pr create --repo :owner/:repo --title '...' --head '...' --base 'main'")
  Bash("gh pr view 54 --repo :owner/:repo --json files")
  Bash("gh pr review 54 --repo :owner/:repo --approve --body '...'")
  
  
  // Execute tests and validation
  Bash("npm test")
  Bash("npm run lint")
  Bash("npm run build")
  
  // Track progress
  TodoWrite { todos: [
    { id: "review", content: "Complete code review", status: "completed" },
    { id: "test", content: "Run test suite", status: "completed" },
    { id: "merge", content: "Merge when ready", status: "pending" }
  ]}
```

## Best Practices

### 1. **Always Use Swarm Coordination**
- Initialize swarm before complex PR operations
- Assign specialized agents for different review aspects
- Use memory for cross-agent coordination

### 2. **Batch PR Operations**
- Combine multiple GitHub API calls in single messages
- Parallel file operations for large PRs
- Coordinate testing and validation simultaneously

### 3. **Intelligent Review Strategy**
- Automated conflict detection and resolution
- Multi-agent review for comprehensive coverage
- Performance and security validation integration

### 4. **Progress Tracking**
- Use TodoWrite for PR milestone tracking
- GitHub issue integration for project coordination
- Real-time status updates through swarm memory

## Integration with Other Modes

### Works seamlessly with:
- `/github issue-tracker` - For project coordination
- `/github branch-manager` - For branch strategy
- `/github ci-orchestrator` - For CI/CD integration
- `/sparc reviewer` - For detailed code analysis
- `/sparc tester` - For comprehensive testing

## Error Handling

### Automatic retry logic for:
- Network failures during GitHub API calls
- Merge conflicts with intelligent resolution
- Test failures with automatic re-runs
- Review bottlenecks with load balancing

### Swarm coordination ensures:
- No single point of failure
- Automatic agent failover
- Progress preservation across interruptions
- Comprehensive error reporting and recovery