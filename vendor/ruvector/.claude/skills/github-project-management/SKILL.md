---
name: github-project-management
title: GitHub Project Management
version: 2.0.0
category: github
description: Comprehensive GitHub project management with swarm-coordinated issue tracking, project board automation, and sprint planning
author: Claude Code
tags:
  - github
  - project-management
  - issue-tracking
  - project-boards
  - sprint-planning
  - agile
  - swarm-coordination
difficulty: intermediate
prerequisites:
  - GitHub CLI (gh) installed and authenticated
  - ruv-swarm or claude-flow MCP server configured
  - Repository access permissions
tools_required:
  - mcp__github__*
  - mcp__claude-flow__*
  - Bash
  - Read
  - Write
  - TodoWrite
related_skills:
  - github-pr-workflow
  - github-release-management
  - sparc-orchestrator
estimated_time: 30-45 minutes
---

# GitHub Project Management

## Overview

A comprehensive skill for managing GitHub projects using AI swarm coordination. This skill combines intelligent issue management, automated project board synchronization, and swarm-based coordination for efficient project delivery.

## Quick Start

### Basic Issue Creation with Swarm Coordination

```bash
# Create a coordinated issue
gh issue create \
  --title "Feature: Advanced Authentication" \
  --body "Implement OAuth2 with social login..." \
  --label "enhancement,swarm-ready"

# Initialize swarm for issue
npx claude-flow@alpha hooks pre-task --description "Feature implementation"
```

### Project Board Quick Setup

```bash
# Get project ID
PROJECT_ID=$(gh project list --owner @me --format json | \
  jq -r '.projects[0].id')

# Initialize board sync
npx ruv-swarm github board-init \
  --project-id "$PROJECT_ID" \
  --sync-mode "bidirectional"
```

---

## Core Capabilities

### 1. Issue Management & Triage

<details>
<summary><strong>Automated Issue Creation</strong></summary>

#### Single Issue with Swarm Coordination

```javascript
// Initialize issue management swarm
mcp__claude-flow__swarm_init { topology: "star", maxAgents: 3 }
mcp__claude-flow__agent_spawn { type: "coordinator", name: "Issue Coordinator" }
mcp__claude-flow__agent_spawn { type: "researcher", name: "Requirements Analyst" }
mcp__claude-flow__agent_spawn { type: "coder", name: "Implementation Planner" }

// Create comprehensive issue
mcp__github__create_issue {
  owner: "org",
  repo: "repository",
  title: "Integration Review: Complete system integration",
  body: `## 🔄 Integration Review

  ### Overview
  Comprehensive review and integration between components.

  ### Objectives
  - [ ] Verify dependencies and imports
  - [ ] Ensure API integration
  - [ ] Check hook system integration
  - [ ] Validate data systems alignment

  ### Swarm Coordination
  This issue will be managed by coordinated swarm agents for optimal progress tracking.`,
  labels: ["integration", "review", "enhancement"],
  assignees: ["username"]
}

// Set up automated tracking
mcp__claude-flow__task_orchestrate {
  task: "Monitor and coordinate issue progress with automated updates",
  strategy: "adaptive",
  priority: "medium"
}
```

#### Batch Issue Creation

```bash
# Create multiple related issues using gh CLI
gh issue create \
  --title "Feature: Advanced GitHub Integration" \
  --body "Implement comprehensive GitHub workflow automation..." \
  --label "feature,github,high-priority"

gh issue create \
  --title "Bug: Merge conflicts in integration branch" \
  --body "Resolve merge conflicts..." \
  --label "bug,integration,urgent"

gh issue create \
  --title "Documentation: Update integration guides" \
  --body "Update all documentation..." \
  --label "documentation,integration"
```

</details>

<details>
<summary><strong>Issue-to-Swarm Conversion</strong></summary>

#### Transform Issues into Swarm Tasks

```bash
# Get issue details
ISSUE_DATA=$(gh issue view 456 --json title,body,labels,assignees,comments)

# Create swarm from issue
npx ruv-swarm github issue-to-swarm 456 \
  --issue-data "$ISSUE_DATA" \
  --auto-decompose \
  --assign-agents

# Batch process multiple issues
ISSUES=$(gh issue list --label "swarm-ready" --json number,title,body,labels)
npx ruv-swarm github issues-batch \
  --issues "$ISSUES" \
  --parallel

# Update issues with swarm status
echo "$ISSUES" | jq -r '.[].number' | while read -r num; do
  gh issue edit $num --add-label "swarm-processing"
done
```

#### Issue Comment Commands

Execute swarm operations via issue comments:

```markdown
<!-- In issue comment -->
/swarm analyze
/swarm decompose 5
/swarm assign @agent-coder
/swarm estimate
/swarm start
```

</details>

<details>
<summary><strong>Automated Issue Triage</strong></summary>

#### Auto-Label Based on Content

```javascript
// .github/swarm-labels.json
{
  "rules": [
    {
      "keywords": ["bug", "error", "broken"],
      "labels": ["bug", "swarm-debugger"],
      "agents": ["debugger", "tester"]
    },
    {
      "keywords": ["feature", "implement", "add"],
      "labels": ["enhancement", "swarm-feature"],
      "agents": ["architect", "coder", "tester"]
    },
    {
      "keywords": ["slow", "performance", "optimize"],
      "labels": ["performance", "swarm-optimizer"],
      "agents": ["analyst", "optimizer"]
    }
  ]
}
```

#### Automated Triage System

```bash
# Analyze and triage unlabeled issues
npx ruv-swarm github triage \
  --unlabeled \
  --analyze-content \
  --suggest-labels \
  --assign-priority

# Find and link duplicate issues
npx ruv-swarm github find-duplicates \
  --threshold 0.8 \
  --link-related \
  --close-duplicates
```

</details>

<details>
<summary><strong>Task Decomposition & Progress Tracking</strong></summary>

#### Break Down Issues into Subtasks

```bash
# Get issue body
ISSUE_BODY=$(gh issue view 456 --json body --jq '.body')

# Decompose into subtasks
SUBTASKS=$(npx ruv-swarm github issue-decompose 456 \
  --body "$ISSUE_BODY" \
  --max-subtasks 10 \
  --assign-priorities)

# Update issue with checklist
CHECKLIST=$(echo "$SUBTASKS" | jq -r '.tasks[] | "- [ ] " + .description')
UPDATED_BODY="$ISSUE_BODY

## Subtasks
$CHECKLIST"

gh issue edit 456 --body "$UPDATED_BODY"

# Create linked issues for major subtasks
echo "$SUBTASKS" | jq -r '.tasks[] | select(.priority == "high")' | while read -r task; do
  TITLE=$(echo "$task" | jq -r '.title')
  BODY=$(echo "$task" | jq -r '.description')

  gh issue create \
    --title "$TITLE" \
    --body "$BODY

Parent issue: #456" \
    --label "subtask"
done
```

#### Automated Progress Updates

```bash
# Get current issue state
CURRENT=$(gh issue view 456 --json body,labels)

# Get swarm progress
PROGRESS=$(npx ruv-swarm github issue-progress 456)

# Update checklist in issue body
UPDATED_BODY=$(echo "$CURRENT" | jq -r '.body' | \
  npx ruv-swarm github update-checklist --progress "$PROGRESS")

# Edit issue with updated body
gh issue edit 456 --body "$UPDATED_BODY"

# Post progress summary as comment
SUMMARY=$(echo "$PROGRESS" | jq -r '
"## 📊 Progress Update

**Completion**: \(.completion)%
**ETA**: \(.eta)

### Completed Tasks
\(.completed | map("- ✅ " + .) | join("\n"))

### In Progress
\(.in_progress | map("- 🔄 " + .) | join("\n"))

### Remaining
\(.remaining | map("- ⏳ " + .) | join("\n"))

---
🤖 Automated update by swarm agent"')

gh issue comment 456 --body "$SUMMARY"

# Update labels based on progress
if [[ $(echo "$PROGRESS" | jq -r '.completion') -eq 100 ]]; then
  gh issue edit 456 --add-label "ready-for-review" --remove-label "in-progress"
fi
```

</details>

<details>
<summary><strong>Stale Issue Management</strong></summary>

#### Auto-Close Stale Issues with Swarm Analysis

```bash
# Find stale issues
STALE_DATE=$(date -d '30 days ago' --iso-8601)
STALE_ISSUES=$(gh issue list --state open --json number,title,updatedAt,labels \
  --jq ".[] | select(.updatedAt < \"$STALE_DATE\")")

# Analyze each stale issue
echo "$STALE_ISSUES" | jq -r '.number' | while read -r num; do
  # Get full issue context
  ISSUE=$(gh issue view $num --json title,body,comments,labels)

  # Analyze with swarm
  ACTION=$(npx ruv-swarm github analyze-stale \
    --issue "$ISSUE" \
    --suggest-action)

  case "$ACTION" in
    "close")
      gh issue comment $num --body "This issue has been inactive for 30 days and will be closed in 7 days if there's no further activity."
      gh issue edit $num --add-label "stale"
      ;;
    "keep")
      gh issue edit $num --remove-label "stale" 2>/dev/null || true
      ;;
    "needs-info")
      gh issue comment $num --body "This issue needs more information. Please provide additional context or it may be closed as stale."
      gh issue edit $num --add-label "needs-info"
      ;;
  esac
done

# Close issues that have been stale for 37+ days
gh issue list --label stale --state open --json number,updatedAt \
  --jq ".[] | select(.updatedAt < \"$(date -d '37 days ago' --iso-8601)\") | .number" | \
  while read -r num; do
    gh issue close $num --comment "Closing due to inactivity. Feel free to reopen if this is still relevant."
  done
```

</details>

### 2. Project Board Automation

<details>
<summary><strong>Board Initialization & Configuration</strong></summary>

#### Connect Swarm to GitHub Project

```bash
# Get project details
PROJECT_ID=$(gh project list --owner @me --format json | \
  jq -r '.projects[] | select(.title == "Development Board") | .id')

# Initialize swarm with project
npx ruv-swarm github board-init \
  --project-id "$PROJECT_ID" \
  --sync-mode "bidirectional" \
  --create-views "swarm-status,agent-workload,priority"

# Create project fields for swarm tracking
gh project field-create $PROJECT_ID --owner @me \
  --name "Swarm Status" \
  --data-type "SINGLE_SELECT" \
  --single-select-options "pending,in_progress,completed"
```

#### Board Mapping Configuration

```yaml
# .github/board-sync.yml
version: 1
project:
  name: "AI Development Board"
  number: 1

mapping:
  # Map swarm task status to board columns
  status:
    pending: "Backlog"
    assigned: "Ready"
    in_progress: "In Progress"
    review: "Review"
    completed: "Done"
    blocked: "Blocked"

  # Map agent types to labels
  agents:
    coder: "🔧 Development"
    tester: "🧪 Testing"
    analyst: "📊 Analysis"
    designer: "🎨 Design"
    architect: "🏗️ Architecture"

  # Map priority to project fields
  priority:
    critical: "🔴 Critical"
    high: "🟡 High"
    medium: "🟢 Medium"
    low: "⚪ Low"

  # Custom fields
  fields:
    - name: "Agent Count"
      type: number
      source: task.agents.length
    - name: "Complexity"
      type: select
      source: task.complexity
    - name: "ETA"
      type: date
      source: task.estimatedCompletion
```

</details>

<details>
<summary><strong>Task Synchronization</strong></summary>

#### Real-time Board Sync

```bash
# Sync swarm tasks with project cards
npx ruv-swarm github board-sync \
  --map-status '{
    "todo": "To Do",
    "in_progress": "In Progress",
    "review": "Review",
    "done": "Done"
  }' \
  --auto-move-cards \
  --update-metadata

# Enable real-time board updates
npx ruv-swarm github board-realtime \
  --webhook-endpoint "https://api.example.com/github-sync" \
  --update-frequency "immediate" \
  --batch-updates false
```

#### Convert Issues to Project Cards

```bash
# List issues with label
ISSUES=$(gh issue list --label "enhancement" --json number,title,body)

# Add issues to project
echo "$ISSUES" | jq -r '.[].number' | while read -r issue; do
  gh project item-add $PROJECT_ID --owner @me --url "https://github.com/$GITHUB_REPOSITORY/issues/$issue"
done

# Process with swarm
npx ruv-swarm github board-import-issues \
  --issues "$ISSUES" \
  --add-to-column "Backlog" \
  --parse-checklist \
  --assign-agents
```

</details>

<details>
<summary><strong>Smart Card Management</strong></summary>

#### Auto-Assignment

```bash
# Automatically assign cards to agents
npx ruv-swarm github board-auto-assign \
  --strategy "load-balanced" \
  --consider "expertise,workload,availability" \
  --update-cards
```

#### Intelligent Card State Transitions

```bash
# Smart card movement based on rules
npx ruv-swarm github board-smart-move \
  --rules '{
    "auto-progress": "when:all-subtasks-done",
    "auto-review": "when:tests-pass",
    "auto-done": "when:pr-merged"
  }'
```

#### Bulk Operations

```bash
# Bulk card operations
npx ruv-swarm github board-bulk \
  --filter "status:blocked" \
  --action "add-label:needs-attention" \
  --notify-assignees
```

</details>

<details>
<summary><strong>Custom Views & Dashboards</strong></summary>

#### View Configuration

```javascript
// Custom board views
{
  "views": [
    {
      "name": "Swarm Overview",
      "type": "board",
      "groupBy": "status",
      "filters": ["is:open"],
      "sort": "priority:desc"
    },
    {
      "name": "Agent Workload",
      "type": "table",
      "groupBy": "assignedAgent",
      "columns": ["title", "status", "priority", "eta"],
      "sort": "eta:asc"
    },
    {
      "name": "Sprint Progress",
      "type": "roadmap",
      "dateField": "eta",
      "groupBy": "milestone"
    }
  ]
}
```

#### Dashboard Configuration

```javascript
// Dashboard with performance widgets
{
  "dashboard": {
    "widgets": [
      {
        "type": "chart",
        "title": "Task Completion Rate",
        "data": "completed-per-day",
        "visualization": "line"
      },
      {
        "type": "gauge",
        "title": "Sprint Progress",
        "data": "sprint-completion",
        "target": 100
      },
      {
        "type": "heatmap",
        "title": "Agent Activity",
        "data": "agent-tasks-per-day"
      }
    ]
  }
}
```

</details>

### 3. Sprint Planning & Tracking

<details>
<summary><strong>Sprint Management</strong></summary>

#### Initialize Sprint with Swarm Coordination

```bash
# Manage sprints with swarms
npx ruv-swarm github sprint-manage \
  --sprint "Sprint 23" \
  --auto-populate \
  --capacity-planning \
  --track-velocity

# Track milestone progress
npx ruv-swarm github milestone-track \
  --milestone "v2.0 Release" \
  --update-board \
  --show-dependencies \
  --predict-completion
```

#### Agile Development Board Setup

```bash
# Setup agile board
npx ruv-swarm github agile-board \
  --methodology "scrum" \
  --sprint-length "2w" \
  --ceremonies "planning,review,retro" \
  --metrics "velocity,burndown"
```

#### Kanban Flow Board Setup

```bash
# Setup kanban board
npx ruv-swarm github kanban-board \
  --wip-limits '{
    "In Progress": 5,
    "Review": 3
  }' \
  --cycle-time-tracking \
  --continuous-flow
```

</details>

<details>
<summary><strong>Progress Tracking & Analytics</strong></summary>

#### Board Analytics

```bash
# Fetch project data
PROJECT_DATA=$(gh project item-list $PROJECT_ID --owner @me --format json)

# Get issue metrics
ISSUE_METRICS=$(echo "$PROJECT_DATA" | jq -r '.items[] | select(.content.type == "Issue")' | \
  while read -r item; do
    ISSUE_NUM=$(echo "$item" | jq -r '.content.number')
    gh issue view $ISSUE_NUM --json createdAt,closedAt,labels,assignees
  done)

# Generate analytics with swarm
npx ruv-swarm github board-analytics \
  --project-data "$PROJECT_DATA" \
  --issue-metrics "$ISSUE_METRICS" \
  --metrics "throughput,cycle-time,wip" \
  --group-by "agent,priority,type" \
  --time-range "30d" \
  --export "dashboard"
```

#### Performance Reports

```bash
# Track and visualize progress
npx ruv-swarm github board-progress \
  --show "burndown,velocity,cycle-time" \
  --time-period "sprint" \
  --export-metrics

# Generate reports
npx ruv-swarm github board-report \
  --type "sprint-summary" \
  --format "markdown" \
  --include "velocity,burndown,blockers" \
  --distribute "slack,email"
```

#### KPI Tracking

```bash
# Track board performance
npx ruv-swarm github board-kpis \
  --metrics '[
    "average-cycle-time",
    "throughput-per-sprint",
    "blocked-time-percentage",
    "first-time-pass-rate"
  ]' \
  --dashboard-url

# Track team performance
npx ruv-swarm github team-metrics \
  --board "Development" \
  --per-member \
  --include "velocity,quality,collaboration" \
  --anonymous-option
```

</details>

<details>
<summary><strong>Release Planning</strong></summary>

#### Release Coordination

```bash
# Plan releases using board data
npx ruv-swarm github release-plan-board \
  --analyze-velocity \
  --estimate-completion \
  --identify-risks \
  --optimize-scope
```

</details>

### 4. Advanced Coordination

<details>
<summary><strong>Multi-Board Synchronization</strong></summary>

#### Cross-Board Sync

```bash
# Sync across multiple boards
npx ruv-swarm github multi-board-sync \
  --boards "Development,QA,Release" \
  --sync-rules '{
    "Development->QA": "when:ready-for-test",
    "QA->Release": "when:tests-pass"
  }'

# Cross-organization sync
npx ruv-swarm github cross-org-sync \
  --source "org1/Project-A" \
  --target "org2/Project-B" \
  --field-mapping "custom" \
  --conflict-resolution "source-wins"
```

</details>

<details>
<summary><strong>Issue Dependencies & Epic Management</strong></summary>

#### Dependency Resolution

```bash
# Handle issue dependencies
npx ruv-swarm github issue-deps 456 \
  --resolve-order \
  --parallel-safe \
  --update-blocking
```

#### Epic Coordination

```bash
# Coordinate epic-level swarms
npx ruv-swarm github epic-swarm \
  --epic 123 \
  --child-issues "456,457,458" \
  --orchestrate
```

</details>

<details>
<summary><strong>Cross-Repository Coordination</strong></summary>

#### Multi-Repo Issue Management

```bash
# Handle issues across repositories
npx ruv-swarm github cross-repo \
  --issue "org/repo#456" \
  --related "org/other-repo#123" \
  --coordinate
```

</details>

<details>
<summary><strong>Team Collaboration</strong></summary>

#### Work Distribution

```bash
# Distribute work among team
npx ruv-swarm github board-distribute \
  --strategy "skills-based" \
  --balance-workload \
  --respect-preferences \
  --notify-assignments
```

#### Standup Automation

```bash
# Generate standup reports
npx ruv-swarm github standup-report \
  --team "frontend" \
  --include "yesterday,today,blockers" \
  --format "slack" \
  --schedule "daily-9am"
```

#### Review Coordination

```bash
# Coordinate reviews via board
npx ruv-swarm github review-coordinate \
  --board "Code Review" \
  --assign-reviewers \
  --track-feedback \
  --ensure-coverage
```

</details>

---

## Issue Templates

### Integration Issue Template

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

### Bug Report Template

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

### Feature Request Template

```markdown
## ✨ Feature Request

### Feature Description
[Clear description of the proposed feature]

### Use Cases
1. [Use case 1]
2. [Use case 2]
3. [Use case 3]

### Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

### Implementation Approach
#### Design
- [ ] Architecture design
- [ ] API design
- [ ] UI/UX mockups

#### Development
- [ ] Core implementation
- [ ] Integration with existing features
- [ ] Performance optimization

#### Testing
- [ ] Unit tests
- [ ] Integration tests
- [ ] User acceptance testing

### Swarm Coordination
- **Architect**: Design and planning
- **Coder**: Implementation
- **Tester**: Quality assurance
- **Documenter**: Documentation

---
🤖 Generated with Claude Code
```

### Swarm Task Template

```markdown
<!-- .github/ISSUE_TEMPLATE/swarm-task.yml -->
name: Swarm Task
description: Create a task for AI swarm processing
body:
  - type: dropdown
    id: topology
    attributes:
      label: Swarm Topology
      options:
        - mesh
        - hierarchical
        - ring
        - star
  - type: input
    id: agents
    attributes:
      label: Required Agents
      placeholder: "coder, tester, analyst"
  - type: textarea
    id: tasks
    attributes:
      label: Task Breakdown
      placeholder: |
        1. Task one description
        2. Task two description
```

---

## Workflow Integration

### GitHub Actions for Issue Management

```yaml
# .github/workflows/issue-swarm.yml
name: Issue Swarm Handler
on:
  issues:
    types: [opened, labeled, commented]

jobs:
  swarm-process:
    runs-on: ubuntu-latest
    steps:
      - name: Process Issue
        uses: ruvnet/swarm-action@v1
        with:
          command: |
            if [[ "${{ github.event.label.name }}" == "swarm-ready" ]]; then
              npx ruv-swarm github issue-init ${{ github.event.issue.number }}
            fi
```

### Board Integration Workflow

```bash
# Sync with project board
npx ruv-swarm github issue-board-sync \
  --project "Development" \
  --column-mapping '{
    "To Do": "pending",
    "In Progress": "active",
    "Done": "completed"
  }'
```

---

## Specialized Issue Strategies

### Bug Investigation Swarm

```bash
# Specialized bug handling
npx ruv-swarm github bug-swarm 456 \
  --reproduce \
  --isolate \
  --fix \
  --test
```

### Feature Implementation Swarm

```bash
# Feature implementation swarm
npx ruv-swarm github feature-swarm 456 \
  --design \
  --implement \
  --document \
  --demo
```

### Technical Debt Refactoring

```bash
# Refactoring swarm
npx ruv-swarm github debt-swarm 456 \
  --analyze-impact \
  --plan-migration \
  --execute \
  --validate
```

---

## Best Practices

### 1. Swarm-Coordinated Issue Management
- Always initialize swarm for complex issues
- Assign specialized agents based on issue type
- Use memory for progress coordination
- Regular automated progress updates

### 2. Board Organization
- Clear column definitions with consistent naming
- Systematic labeling strategy across repositories
- Regular board grooming and maintenance
- Well-defined automation rules

### 3. Data Integrity
- Bidirectional sync validation
- Conflict resolution strategies
- Comprehensive audit trails
- Regular backups of project data

### 4. Team Adoption
- Comprehensive training materials
- Clear, documented workflows
- Regular team reviews and retrospectives
- Active feedback loops for improvement

### 5. Smart Labeling and Organization
- Consistent labeling strategy across repositories
- Priority-based issue sorting and assignment
- Milestone integration for project coordination
- Agent-type to label mapping

### 6. Automated Progress Tracking
- Regular automated updates with swarm coordination
- Progress metrics and completion tracking
- Cross-issue dependency management
- Real-time status synchronization

---

## Troubleshooting

### Sync Issues

```bash
# Diagnose sync problems
npx ruv-swarm github board-diagnose \
  --check "permissions,webhooks,rate-limits" \
  --test-sync \
  --show-conflicts
```

### Performance Optimization

```bash
# Optimize board performance
npx ruv-swarm github board-optimize \
  --analyze-size \
  --archive-completed \
  --index-fields \
  --cache-views
```

### Data Recovery

```bash
# Recover board data
npx ruv-swarm github board-recover \
  --backup-id "2024-01-15" \
  --restore-cards \
  --preserve-current \
  --merge-conflicts
```

---

## Metrics & Analytics

### Performance Metrics

Automatic tracking of:
- Issue creation and resolution times
- Agent productivity metrics
- Project milestone progress
- Cross-repository coordination efficiency
- Sprint velocity and burndown
- Cycle time and throughput
- Work-in-progress limits

### Reporting Features

- Weekly progress summaries
- Agent performance analytics
- Project health metrics
- Integration success rates
- Team collaboration metrics
- Quality and defect tracking

### Issue Resolution Time

```bash
# Analyze swarm performance
npx ruv-swarm github issue-metrics \
  --issue 456 \
  --metrics "time-to-close,agent-efficiency,subtask-completion"
```

### Swarm Effectiveness

```bash
# Generate effectiveness report
npx ruv-swarm github effectiveness \
  --issues "closed:>2024-01-01" \
  --compare "with-swarm,without-swarm"
```

---

## Security & Permissions

1. **Command Authorization**: Validate user permissions before executing commands
2. **Rate Limiting**: Prevent spam and abuse of issue commands
3. **Audit Logging**: Track all swarm operations on issues and boards
4. **Data Privacy**: Respect private repository settings
5. **Access Control**: Proper GitHub permissions for board operations
6. **Webhook Security**: Secure webhook endpoints for real-time updates

---

## Integration with Other Skills

### Seamless Integration With:
- `github-pr-workflow` - Link issues to pull requests automatically
- `github-release-management` - Coordinate release issues and milestones
- `sparc-orchestrator` - Complex project coordination workflows
- `sparc-tester` - Automated testing workflows for issues

---

## Complete Workflow Example

### Full-Stack Feature Development

```bash
# 1. Create feature issue with swarm coordination
gh issue create \
  --title "Feature: Real-time Collaboration" \
  --body "$(cat <<EOF
## Feature: Real-time Collaboration

### Overview
Implement real-time collaboration features using WebSockets.

### Objectives
- [ ] WebSocket server setup
- [ ] Client-side integration
- [ ] Presence tracking
- [ ] Conflict resolution
- [ ] Testing and documentation

### Swarm Coordination
This feature will use mesh topology for parallel development.
EOF
)" \
  --label "enhancement,swarm-ready,high-priority"

# 2. Initialize swarm and decompose tasks
ISSUE_NUM=$(gh issue list --label "swarm-ready" --limit 1 --json number --jq '.[0].number')
npx ruv-swarm github issue-init $ISSUE_NUM \
  --topology mesh \
  --auto-decompose \
  --assign-agents "architect,coder,tester"

# 3. Add to project board
PROJECT_ID=$(gh project list --owner @me --format json | jq -r '.projects[0].id')
gh project item-add $PROJECT_ID --owner @me \
  --url "https://github.com/$GITHUB_REPOSITORY/issues/$ISSUE_NUM"

# 4. Set up automated tracking
npx ruv-swarm github board-sync \
  --auto-move-cards \
  --update-metadata

# 5. Monitor progress
npx ruv-swarm github issue-progress $ISSUE_NUM \
  --auto-update-comments \
  --notify-on-completion
```

---

## Quick Reference Commands

```bash
# Issue Management
gh issue create --title "..." --body "..." --label "..."
npx ruv-swarm github issue-init <number>
npx ruv-swarm github issue-decompose <number>
npx ruv-swarm github triage --unlabeled

# Project Boards
npx ruv-swarm github board-init --project-id <id>
npx ruv-swarm github board-sync
npx ruv-swarm github board-analytics

# Sprint Management
npx ruv-swarm github sprint-manage --sprint "Sprint X"
npx ruv-swarm github milestone-track --milestone "vX.X"

# Analytics
npx ruv-swarm github issue-metrics --issue <number>
npx ruv-swarm github board-kpis
```

---

## Additional Resources

- [GitHub CLI Documentation](https://cli.github.com/manual/)
- [GitHub Projects Documentation](https://docs.github.com/en/issues/planning-and-tracking-with-projects)
- [Swarm Coordination Guide](https://github.com/ruvnet/ruv-swarm)
- [Claude Flow Documentation](https://github.com/ruvnet/claude-flow)

---

**Last Updated**: 2025-10-19
**Version**: 2.0.0
**Maintainer**: Claude Code
