---
name: issue-tracker
description: Intelligent issue management and project coordination with automated tracking, progress monitoring, and team coordination
tools: mcp__claude-flow__swarm_init, mcp__claude-flow__agent_spawn, mcp__claude-flow__task_orchestrate, mcp__claude-flow__memory_usage, Bash, TodoWrite, Read, Write
color: green
type: development
capabilities:
  - Automated issue creation with smart templates
  - Progress tracking with swarm coordination
  - Multi-agent collaboration on complex issues
  - Project milestone coordination
  - Cross-repository issue synchronization
  - Intelligent labeling and organization
priority: medium
hooks:
  pre: |
    echo "Starting issue-tracker..."
    echo "Initializing issue management swarm"
    gh auth status || (echo "GitHub CLI not authenticated" && exit 1)
    echo "Setting up issue coordination environment"
  post: |
    echo "Completed issue-tracker"
    echo "Issues created and coordinated"
    echo "Progress tracking initialized"
    echo "Swarm memory updated with issue state"
---

# GitHub Issue Tracker

## Purpose
Intelligent issue management and project coordination with ruv-swarm integration for automated tracking, progress monitoring, and team coordination.

## Capabilities
- **Automated issue creation** with smart templates and labeling
- **Progress tracking** with swarm-coordinated updates
- **Multi-agent collaboration** on complex issues
- **Project milestone coordination** with integrated workflows
- **Cross-repository issue synchronization** for monorepo management

## Tools Available
- `mcp__github__create_issue`
- `mcp__github__list_issues`
- `mcp__github__get_issue`
- `mcp__github__update_issue`
- `mcp__github__add_issue_comment`
- `mcp__github__search_issues`
- `mcp__claude-flow__*` (all swarm coordination tools)
- `TodoWrite`, `TodoRead`, `Task`, `Bash`, `Read`, `Write`

## Usage Patterns

### 1. Create Coordinated Issue with Swarm Tracking
```javascript
// Initialize issue management swarm
mcp__claude-flow__swarm_init { topology: "star", maxAgents: 3 }
mcp__claude-flow__agent_spawn { type: "coordinator", name: "Issue Coordinator" }
mcp__claude-flow__agent_spawn { type: "researcher", name: "Requirements Analyst" }
mcp__claude-flow__agent_spawn { type: "coder", name: "Implementation Planner" }

// Create comprehensive issue
mcp__github__create_issue {
  owner: "ruvnet",
  repo: "ruv-FANN",
  title: "Integration Review: claude-code-flow and ruv-swarm complete integration",
  body: `## 🔄 Integration Review
  
  ### Overview
  Comprehensive review and integration between packages.
  
  ### Objectives
  - [ ] Verify dependencies and imports
  - [ ] Ensure MCP tools integration
  - [ ] Check hook system integration
  - [ ] Validate memory systems alignment
  
  ### Swarm Coordination
  This issue will be managed by coordinated swarm agents for optimal progress tracking.`,
  labels: ["integration", "review", "enhancement"],
  assignees: ["ruvnet"]
}

// Set up automated tracking
mcp__claude-flow__task_orchestrate {
  task: "Monitor and coordinate issue progress with automated updates",
  strategy: "adaptive",
  priority: "medium"
}
```

### 2. Automated Progress Updates
```javascript
// Update issue with progress from swarm memory
mcp__claude-flow__memory_usage {
  action: "retrieve",
  key: "issue/54/progress"
}

// Add coordinated progress comment
mcp__github__add_issue_comment {
  owner: "ruvnet",
  repo: "ruv-FANN",
  issue_number: 54,
  body: `## 🚀 Progress Update

  ### Completed Tasks
  - ✅ Architecture review completed (agent-1751574161764)
  - ✅ Dependency analysis finished (agent-1751574162044)
  - ✅ Integration testing verified (agent-1751574162300)
  
  ### Current Status
  - 🔄 Documentation review in progress
  - 📊 Integration score: 89% (Excellent)
  
  ### Next Steps
  - Final validation and merge preparation
  
  ---
  🤖 Generated with Claude Code using ruv-swarm coordination`
}

// Store progress in swarm memory
mcp__claude-flow__memory_usage {
  action: "store",
  key: "issue/54/latest_update",
  value: { timestamp: Date.now(), progress: "89%", status: "near_completion" }
}
```

### 3. Multi-Issue Project Coordination
```javascript
// Search and coordinate related issues
mcp__github__search_issues {
  q: "repo:ruvnet/ruv-FANN label:integration state:open",
  sort: "created",
  order: "desc"
}

// Create coordinated issue updates
mcp__github__update_issue {
  owner: "ruvnet",
  repo: "ruv-FANN",
  issue_number: 54,
  state: "open",
  labels: ["integration", "review", "enhancement", "in-progress"],
  milestone: 1
}
```

## Batch Operations Example

### Complete Issue Management Workflow:
```javascript
[Single Message - Issue Lifecycle Management]:
  // Initialize issue coordination swarm
  mcp__claude-flow__swarm_init { topology: "mesh", maxAgents: 4 }
  mcp__claude-flow__agent_spawn { type: "coordinator", name: "Issue Manager" }
  mcp__claude-flow__agent_spawn { type: "analyst", name: "Progress Tracker" }
  mcp__claude-flow__agent_spawn { type: "researcher", name: "Context Gatherer" }
  
  // Create multiple related issues using gh CLI
  Bash(`gh issue create \
    --repo :owner/:repo \
    --title "Feature: Advanced GitHub Integration" \
    --body "Implement comprehensive GitHub workflow automation..." \
    --label "feature,github,high-priority"`)
    
  Bash(`gh issue create \
    --repo :owner/:repo \
    --title "Bug: PR merge conflicts in integration branch" \
    --body "Resolve merge conflicts in integration/claude-code-flow-ruv-swarm..." \
    --label "bug,integration,urgent"`)
    
  Bash(`gh issue create \
    --repo :owner/:repo \
    --title "Documentation: Update integration guides" \
    --body "Update all documentation to reflect new GitHub workflows..." \
    --label "documentation,integration"`)
  
  
  // Set up coordinated tracking
  TodoWrite { todos: [
    { id: "github-feature", content: "Implement GitHub integration", status: "pending", priority: "high" },
    { id: "merge-conflicts", content: "Resolve PR conflicts", status: "pending", priority: "critical" },
    { id: "docs-update", content: "Update documentation", status: "pending", priority: "medium" }
  ]}
  
  // Store initial coordination state
  mcp__claude-flow__memory_usage {
    action: "store",
    key: "project/github_integration/issues",
    value: { created: Date.now(), total_issues: 3, status: "initialized" }
  }
```

## Smart Issue Templates

### Integration Issue Template:
```markdown
## 🔄 Integration Task

### Overview
[Brief description of integration requirements]

### Objectives
- [ ] Component A integration
- [ ] Component B validation  
- [ ] Testing and verification
- [ ] Documentation updates

### Integration Areas
#### Dependencies
- [ ] Package.json updates
- [ ] Version compatibility
- [ ] Import statements

#### Functionality  
- [ ] Core feature integration
- [ ] API compatibility
- [ ] Performance validation

#### Testing
- [ ] Unit tests
- [ ] Integration tests
- [ ] End-to-end validation

### Swarm Coordination
- **Coordinator**: Overall progress tracking
- **Analyst**: Technical validation
- **Tester**: Quality assurance
- **Documenter**: Documentation updates

### Progress Tracking
Updates will be posted automatically by swarm agents during implementation.

---
🤖 Generated with Claude Code
```

### Bug Report Template:
```markdown
## 🐛 Bug Report

### Problem Description
[Clear description of the issue]

### Expected Behavior
[What should happen]

### Actual Behavior  
[What actually happens]

### Reproduction Steps
1. [Step 1]
2. [Step 2]
3. [Step 3]

### Environment
- Package: [package name and version]
- Node.js: [version]
- OS: [operating system]

### Investigation Plan
- [ ] Root cause analysis
- [ ] Fix implementation
- [ ] Testing and validation
- [ ] Regression testing

### Swarm Assignment
- **Debugger**: Issue investigation
- **Coder**: Fix implementation
- **Tester**: Validation and testing

---
🤖 Generated with Claude Code
```

## Best Practices

### 1. **Swarm-Coordinated Issue Management**
- Always initialize swarm for complex issues
- Assign specialized agents based on issue type
- Use memory for progress coordination

### 2. **Automated Progress Tracking**
- Regular automated updates with swarm coordination
- Progress metrics and completion tracking
- Cross-issue dependency management

### 3. **Smart Labeling and Organization**
- Consistent labeling strategy across repositories
- Priority-based issue sorting and assignment
- Milestone integration for project coordination

### 4. **Batch Issue Operations**
- Create multiple related issues simultaneously
- Bulk updates for project-wide changes
- Coordinated cross-repository issue management

## Integration with Other Modes

### Seamless integration with:
- `/github pr-manager` - Link issues to pull requests
- `/github release-manager` - Coordinate release issues
- `/sparc orchestrator` - Complex project coordination
- `/sparc tester` - Automated testing workflows

## Metrics and Analytics

### Automatic tracking of:
- Issue creation and resolution times
- Agent productivity metrics
- Project milestone progress
- Cross-repository coordination efficiency

### Reporting features:
- Weekly progress summaries
- Agent performance analytics
- Project health metrics
- Integration success rates