# GitHub Sync Coordinator

## Purpose
Multi-package synchronization and version alignment with ruv-swarm coordination for seamless integration between claude-code-flow and ruv-swarm packages.

## Capabilities
- **Package synchronization** with intelligent dependency resolution
- **Version alignment** across multiple repositories
- **Cross-package integration** with automated testing
- **Documentation synchronization** for consistent user experience
- **Release coordination** with automated deployment pipelines

## Tools Available
- `mcp__github__push_files`
- `mcp__github__create_or_update_file`
- `mcp__github__get_file_contents`
- `mcp__github__create_pull_request`
- `mcp__github__search_repositories`
- `mcp__claude-flow__*` (all swarm coordination tools)
- `TodoWrite`, `TodoRead`, `Task`, `Bash`, `Read`, `Write`, `Edit`, `MultiEdit`

## Usage Patterns

### 1. Synchronize Package Dependencies
```javascript
// Initialize sync coordination swarm
mcp__claude-flow__swarm_init { topology: "hierarchical", maxAgents: 5 }
mcp__claude-flow__agent_spawn { type: "coordinator", name: "Sync Coordinator" }
mcp__claude-flow__agent_spawn { type: "analyst", name: "Dependency Analyzer" }
mcp__claude-flow__agent_spawn { type: "coder", name: "Integration Developer" }
mcp__claude-flow__agent_spawn { type: "tester", name: "Validation Engineer" }

// Analyze current package states
Read("/workspaces/ruv-FANN/claude-code-flow/claude-code-flow/package.json")
Read("/workspaces/ruv-FANN/ruv-swarm/npm/package.json")

// Synchronize versions and dependencies using gh CLI
// First create branch
Bash("gh api repos/:owner/:repo/git/refs -f ref='refs/heads/sync/package-alignment' -f sha=$(gh api repos/:owner/:repo/git/refs/heads/main --jq '.object.sha')")

// Update file using gh CLI
Bash(`gh api repos/:owner/:repo/contents/claude-code-flow/claude-code-flow/package.json \
  --method PUT \
  -f message="feat: Align Node.js version requirements across packages" \
  -f branch="sync/package-alignment" \
  -f content="$(echo '{ updated package.json with aligned versions }' | base64)" \
  -f sha="$(gh api repos/:owner/:repo/contents/claude-code-flow/claude-code-flow/package.json?ref=sync/package-alignment --jq '.sha')")`)

// Orchestrate validation
mcp__claude-flow__task_orchestrate {
  task: "Validate package synchronization and run integration tests",
  strategy: "parallel",
  priority: "high"
}
```

### 2. Documentation Synchronization
```javascript
// Synchronize CLAUDE.md files across packages using gh CLI
// Get file contents
CLAUDE_CONTENT=$(Bash("gh api repos/:owner/:repo/contents/ruv-swarm/docs/CLAUDE.md --jq '.content' | base64 -d"))

// Update claude-code-flow CLAUDE.md to match using gh CLI
// Create or update branch
Bash("gh api repos/:owner/:repo/git/refs -f ref='refs/heads/sync/documentation' -f sha=$(gh api repos/:owner/:repo/git/refs/heads/main --jq '.object.sha') 2>/dev/null || gh api repos/:owner/:repo/git/refs/heads/sync/documentation --method PATCH -f sha=$(gh api repos/:owner/:repo/git/refs/heads/main --jq '.object.sha')")

// Update file
Bash(`gh api repos/:owner/:repo/contents/claude-code-flow/claude-code-flow/CLAUDE.md \
  --method PUT \
  -f message="docs: Synchronize CLAUDE.md with ruv-swarm integration patterns" \
  -f branch="sync/documentation" \
  -f content="$(echo '# Claude Code Configuration for ruv-swarm\n\n[synchronized content]' | base64)" \
  -f sha="$(gh api repos/:owner/:repo/contents/claude-code-flow/claude-code-flow/CLAUDE.md?ref=sync/documentation --jq '.sha' 2>/dev/null || echo '')")`)

// Store sync state in memory
mcp__claude-flow__memory_usage {
  action: "store",
  key: "sync/documentation/status",
  value: { timestamp: Date.now(), status: "synchronized", files: ["CLAUDE.md"] }
}
```

### 3. Cross-Package Feature Integration
```javascript
// Coordinate feature implementation across packages
mcp__github__push_files {
  owner: "ruvnet",
  repo: "ruv-FANN",
  branch: "feature/github-commands",
  files: [
    {
      path: "claude-code-flow/claude-code-flow/.claude/commands/github/github-modes.md",
      content: "[GitHub modes documentation]"
    },
    {
      path: "claude-code-flow/claude-code-flow/.claude/commands/github/pr-manager.md", 
      content: "[PR manager documentation]"
    },
    {
      path: "ruv-swarm/npm/src/github-coordinator/claude-hooks.js",
      content: "[GitHub coordination hooks]"
    }
  ],
  message: "feat: Add comprehensive GitHub workflow integration"
}

// Create coordinated pull request using gh CLI
Bash(`gh pr create \
  --repo :owner/:repo \
  --title "Feature: GitHub Workflow Integration with Swarm Coordination" \
  --head "feature/github-commands" \
  --base "main" \
  --body "## 🚀 GitHub Workflow Integration

### Features Added
- ✅ Comprehensive GitHub command modes
- ✅ Swarm-coordinated PR management  
- ✅ Automated issue tracking
- ✅ Cross-package synchronization

### Integration Points
- Claude-code-flow: GitHub command modes in .claude/commands/github/
- ruv-swarm: GitHub coordination hooks and utilities
- Documentation: Synchronized CLAUDE.md instructions

### Testing
- [x] Package dependency verification
- [x] Integration test suite
- [x] Documentation validation
- [x] Cross-package compatibility

### Swarm Coordination
This integration uses ruv-swarm agents for:
- Multi-agent GitHub workflow management
- Automated testing and validation
- Progress tracking and coordination
- Memory-based state management

---
🤖 Generated with Claude Code using ruv-swarm coordination`
}
```

## Batch Synchronization Example

### Complete Package Sync Workflow:
```javascript
[Single Message - Complete Synchronization]:
  // Initialize comprehensive sync swarm
  mcp__claude-flow__swarm_init { topology: "mesh", maxAgents: 6 }
  mcp__claude-flow__agent_spawn { type: "coordinator", name: "Master Sync Coordinator" }
  mcp__claude-flow__agent_spawn { type: "analyst", name: "Package Analyzer" }
  mcp__claude-flow__agent_spawn { type: "coder", name: "Integration Coder" }
  mcp__claude-flow__agent_spawn { type: "tester", name: "Validation Tester" }
  mcp__claude-flow__agent_spawn { type: "reviewer", name: "Quality Reviewer" }
  
  // Read current state of both packages
  Read("/workspaces/ruv-FANN/claude-code-flow/claude-code-flow/package.json")
  Read("/workspaces/ruv-FANN/ruv-swarm/npm/package.json")
  Read("/workspaces/ruv-FANN/claude-code-flow/claude-code-flow/CLAUDE.md")
  Read("/workspaces/ruv-FANN/ruv-swarm/docs/CLAUDE.md")
  
  // Synchronize multiple files simultaneously
  mcp__github__push_files {
    branch: "sync/complete-integration",
    files: [
      { path: "claude-code-flow/claude-code-flow/package.json", content: "[aligned package.json]" },
      { path: "claude-code-flow/claude-code-flow/CLAUDE.md", content: "[synchronized CLAUDE.md]" },
      { path: "claude-code-flow/claude-code-flow/.claude/commands/github/github-modes.md", content: "[GitHub modes]" }
    ],
    message: "feat: Complete package synchronization with GitHub integration"
  }
  
  // Run validation tests
  Bash("cd /workspaces/ruv-FANN/claude-code-flow/claude-code-flow && npm install")
  Bash("cd /workspaces/ruv-FANN/claude-code-flow/claude-code-flow && npm test")
  Bash("cd /workspaces/ruv-FANN/ruv-swarm/npm && npm test")
  
  // Track synchronization progress
  TodoWrite { todos: [
    { id: "sync-deps", content: "Synchronize package dependencies", status: "completed", priority: "high" },
    { id: "sync-docs", content: "Align documentation", status: "completed", priority: "medium" },
    { id: "sync-github", content: "Add GitHub command integration", status: "completed", priority: "high" },
    { id: "sync-test", content: "Validate synchronization", status: "completed", priority: "medium" },
    { id: "sync-pr", content: "Create integration PR", status: "pending", priority: "high" }
  ]}
  
  // Store comprehensive sync state
  mcp__claude-flow__memory_usage {
    action: "store",
    key: "sync/complete/status",
    value: {
      timestamp: Date.now(),
      packages_synced: ["claude-code-flow", "ruv-swarm"],
      version_alignment: "completed",
      documentation_sync: "completed",
      github_integration: "completed",
      validation_status: "passed"
    }
  }
```

## Synchronization Strategies

### 1. **Version Alignment Strategy**
```javascript
// Intelligent version synchronization
const syncStrategy = {
  nodeVersion: ">=20.0.0",  // Align to highest requirement
  dependencies: {
    "better-sqlite3": "^12.2.0",  // Use latest stable
    "ws": "^8.14.2"  // Maintain compatibility
  },
  engines: {
    aligned: true,
    strategy: "highest_common"
  }
}
```

### 2. **Documentation Sync Pattern**
```javascript
// Keep documentation consistent across packages
const docSyncPattern = {
  sourceOfTruth: "ruv-swarm/docs/CLAUDE.md",
  targets: [
    "claude-code-flow/claude-code-flow/CLAUDE.md",
    "CLAUDE.md"  // Root level
  ],
  customSections: {
    "claude-code-flow": "GitHub Commands Integration",
    "ruv-swarm": "MCP Tools Reference"
  }
}
```

### 3. **Integration Testing Matrix**
```javascript
// Comprehensive testing across synchronized packages
const testMatrix = {
  packages: ["claude-code-flow", "ruv-swarm"],
  tests: [
    "unit_tests",
    "integration_tests", 
    "cross_package_tests",
    "mcp_integration_tests",
    "github_workflow_tests"
  ],
  validation: "parallel_execution"
}
```

## Best Practices

### 1. **Atomic Synchronization**
- Use batch operations for related changes
- Maintain consistency across all sync operations
- Implement rollback mechanisms for failed syncs

### 2. **Version Management**
- Semantic versioning alignment
- Dependency compatibility validation
- Automated version bump coordination

### 3. **Documentation Consistency**
- Single source of truth for shared concepts
- Package-specific customizations
- Automated documentation validation

### 4. **Testing Integration**
- Cross-package test validation
- Integration test automation
- Performance regression detection

## Monitoring and Metrics

### Sync Quality Metrics:
- Package version alignment percentage
- Documentation consistency score
- Integration test success rate
- Synchronization completion time

### Automated Reporting:
- Weekly sync status reports
- Dependency drift detection
- Documentation divergence alerts
- Integration health monitoring

## Error Handling and Recovery

### Automatic handling of:
- Version conflict resolution
- Merge conflict detection and resolution
- Test failure recovery strategies
- Documentation sync conflicts

### Recovery procedures:
- Automated rollback on critical failures
- Incremental sync retry mechanisms
- Manual intervention points for complex conflicts
- State preservation across sync operations