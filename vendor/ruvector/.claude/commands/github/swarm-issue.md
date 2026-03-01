# Swarm Issue - Issue-Based Swarm Coordination

## Overview
Transform GitHub Issues into intelligent swarm tasks, enabling automatic task decomposition and agent coordination.

## Core Features

### 1. Issue-to-Swarm Conversion
```bash
# Create swarm from issue using gh CLI
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

### 2. Issue Comment Commands
Execute swarm operations via issue comments:

```markdown
<!-- In issue comment -->
/swarm analyze
/swarm decompose 5
/swarm assign @agent-coder
/swarm estimate
/swarm start
```

### 3. Issue Templates for Swarms

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

## Issue Label Automation

### Auto-Label Based on Content
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

### Dynamic Agent Assignment
```bash
# Assign agents based on issue content
npx ruv-swarm github issue-analyze 456 \
  --suggest-agents \
  --estimate-complexity \
  --create-subtasks
```

## Issue Swarm Commands

### Initialize from Issue
```bash
# Create swarm with full issue context using gh CLI
# Get complete issue data
ISSUE=$(gh issue view 456 --json title,body,labels,assignees,comments,projectItems)

# Get referenced issues and PRs
REFERENCES=$(gh issue view 456 --json body --jq '.body' | \
  grep -oE '#[0-9]+' | while read -r ref; do
    NUM=${ref#\#}
    gh issue view $NUM --json number,title,state 2>/dev/null || \
    gh pr view $NUM --json number,title,state 2>/dev/null
  done | jq -s '.')

# Initialize swarm
npx ruv-swarm github issue-init 456 \
  --issue-data "$ISSUE" \
  --references "$REFERENCES" \
  --load-comments \
  --analyze-references \
  --auto-topology

# Add swarm initialization comment
gh issue comment 456 --body "🐝 Swarm initialized for this issue"
```

### Task Decomposition
```bash
# Break down issue into subtasks with gh CLI
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

### Progress Tracking
```bash
# Update issue with swarm progress using gh CLI
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

## Advanced Features

### 1. Issue Dependencies
```bash
# Handle issue dependencies
npx ruv-swarm github issue-deps 456 \
  --resolve-order \
  --parallel-safe \
  --update-blocking
```

### 2. Epic Management
```bash
# Coordinate epic-level swarms
npx ruv-swarm github epic-swarm \
  --epic 123 \
  --child-issues "456,457,458" \
  --orchestrate
```

### 3. Issue Templates
```bash
# Generate issue from swarm analysis
npx ruv-swarm github create-issues \
  --from-analysis \
  --template "bug-report" \
  --auto-assign
```

## Workflow Integration

### GitHub Actions for Issues
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

### Issue Board Integration
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

## Issue Types & Strategies

### Bug Reports
```bash
# Specialized bug handling
npx ruv-swarm github bug-swarm 456 \
  --reproduce \
  --isolate \
  --fix \
  --test
```

### Feature Requests
```bash
# Feature implementation swarm
npx ruv-swarm github feature-swarm 456 \
  --design \
  --implement \
  --document \
  --demo
```

### Technical Debt
```bash
# Refactoring swarm
npx ruv-swarm github debt-swarm 456 \
  --analyze-impact \
  --plan-migration \
  --execute \
  --validate
```

## Automation Examples

### Auto-Close Stale Issues
```bash
# Process stale issues with swarm using gh CLI
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
      # Add stale label and warning comment
      gh issue comment $num --body "This issue has been inactive for 30 days and will be closed in 7 days if there's no further activity."
      gh issue edit $num --add-label "stale"
      ;;
    "keep")
      # Remove stale label if present
      gh issue edit $num --remove-label "stale" 2>/dev/null || true
      ;;
    "needs-info")
      # Request more information
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

### Issue Triage
```bash
# Automated triage system
npx ruv-swarm github triage \
  --unlabeled \
  --analyze-content \
  --suggest-labels \
  --assign-priority
```

### Duplicate Detection
```bash
# Find duplicate issues
npx ruv-swarm github find-duplicates \
  --threshold 0.8 \
  --link-related \
  --close-duplicates
```

## Integration Patterns

### 1. Issue-PR Linking
```bash
# Link issues to PRs automatically
npx ruv-swarm github link-pr \
  --issue 456 \
  --pr 789 \
  --update-both
```

### 2. Milestone Coordination
```bash
# Coordinate milestone swarms
npx ruv-swarm github milestone-swarm \
  --milestone "v2.0" \
  --parallel-issues \
  --track-progress
```

### 3. Cross-Repo Issues
```bash
# Handle issues across repositories
npx ruv-swarm github cross-repo \
  --issue "org/repo#456" \
  --related "org/other-repo#123" \
  --coordinate
```

## Metrics & Analytics

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

## Best Practices

### 1. Issue Templates
- Include swarm configuration options
- Provide task breakdown structure
- Set clear acceptance criteria
- Include complexity estimates

### 2. Label Strategy
- Use consistent swarm-related labels
- Map labels to agent types
- Priority indicators for swarm
- Status tracking labels

### 3. Comment Etiquette
- Clear command syntax
- Progress updates in threads
- Summary comments for decisions
- Link to relevant PRs

## Security & Permissions

1. **Command Authorization**: Validate user permissions before executing commands
2. **Rate Limiting**: Prevent spam and abuse of issue commands
3. **Audit Logging**: Track all swarm operations on issues
4. **Data Privacy**: Respect private repository settings

## Examples

### Complex Bug Investigation
```bash
# Issue #789: Memory leak in production
npx ruv-swarm github issue-init 789 \
  --topology hierarchical \
  --agents "debugger,analyst,tester,monitor" \
  --priority critical \
  --reproduce-steps
```

### Feature Implementation
```bash
# Issue #234: Add OAuth integration
npx ruv-swarm github issue-init 234 \
  --topology mesh \
  --agents "architect,coder,security,tester" \
  --create-design-doc \
  --estimate-effort
```

### Documentation Update
```bash
# Issue #567: Update API documentation
npx ruv-swarm github issue-init 567 \
  --topology ring \
  --agents "researcher,writer,reviewer" \
  --check-links \
  --validate-examples
```

See also: [swarm-pr.md](./swarm-pr.md), [project-board-sync.md](./project-board-sync.md)