---
name: github-code-review
version: 1.0.0
description: Comprehensive GitHub code review with AI-powered swarm coordination
category: github
tags: [code-review, github, swarm, pr-management, automation]
author: Claude Code Flow
requires:
  - github-cli
  - ruv-swarm
  - claude-flow
capabilities:
  - Multi-agent code review
  - Automated PR management
  - Security and performance analysis
  - Swarm-based review orchestration
  - Intelligent comment generation
  - Quality gate enforcement
---

# GitHub Code Review Skill

> **AI-Powered Code Review**: Deploy specialized review agents to perform comprehensive, intelligent code reviews that go beyond traditional static analysis.

## 🎯 Quick Start

### Simple Review
```bash
# Initialize review swarm for PR
gh pr view 123 --json files,diff | npx ruv-swarm github review-init --pr 123

# Post review status
gh pr comment 123 --body "🔍 Multi-agent code review initiated"
```

### Complete Review Workflow
```bash
# Get PR context with gh CLI
PR_DATA=$(gh pr view 123 --json files,additions,deletions,title,body)
PR_DIFF=$(gh pr diff 123)

# Initialize comprehensive review
npx ruv-swarm github review-init \
  --pr 123 \
  --pr-data "$PR_DATA" \
  --diff "$PR_DIFF" \
  --agents "security,performance,style,architecture,accessibility" \
  --depth comprehensive
```

---

## 📚 Table of Contents

<details>
<summary><strong>Core Features</strong></summary>

- [Multi-Agent Review System](#multi-agent-review-system)
- [Specialized Review Agents](#specialized-review-agents)
- [PR-Based Swarm Management](#pr-based-swarm-management)
- [Automated Workflows](#automated-workflows)
- [Quality Gates & Checks](#quality-gates--checks)

</details>

<details>
<summary><strong>Review Agents</strong></summary>

- [Security Review Agent](#security-review-agent)
- [Performance Review Agent](#performance-review-agent)
- [Architecture Review Agent](#architecture-review-agent)
- [Style & Convention Agent](#style--convention-agent)
- [Accessibility Agent](#accessibility-agent)

</details>

<details>
<summary><strong>Advanced Features</strong></summary>

- [Context-Aware Reviews](#context-aware-reviews)
- [Learning from History](#learning-from-history)
- [Cross-PR Analysis](#cross-pr-analysis)
- [Custom Review Agents](#custom-review-agents)

</details>

<details>
<summary><strong>Integration & Automation</strong></summary>

- [CI/CD Integration](#cicd-integration)
- [Webhook Handlers](#webhook-handlers)
- [PR Comment Commands](#pr-comment-commands)
- [Automated Fixes](#automated-fixes)

</details>

---

## 🚀 Core Features

### Multi-Agent Review System

Deploy specialized AI agents for comprehensive code review:

```bash
# Initialize review swarm with GitHub CLI integration
PR_DATA=$(gh pr view 123 --json files,additions,deletions,title,body)
PR_DIFF=$(gh pr diff 123)

# Start multi-agent review
npx ruv-swarm github review-init \
  --pr 123 \
  --pr-data "$PR_DATA" \
  --diff "$PR_DIFF" \
  --agents "security,performance,style,architecture,accessibility" \
  --depth comprehensive

# Post initial review status
gh pr comment 123 --body "🔍 Multi-agent code review initiated"
```

**Benefits:**
- ✅ Parallel review by specialized agents
- ✅ Comprehensive coverage across multiple domains
- ✅ Faster review cycles with coordinated analysis
- ✅ Consistent quality standards enforcement

---

## 🤖 Specialized Review Agents

### Security Review Agent

**Focus:** Identify security vulnerabilities and suggest fixes

```bash
# Get changed files from PR
CHANGED_FILES=$(gh pr view 123 --json files --jq '.files[].path')

# Run security-focused review
SECURITY_RESULTS=$(npx ruv-swarm github review-security \
  --pr 123 \
  --files "$CHANGED_FILES" \
  --check "owasp,cve,secrets,permissions" \
  --suggest-fixes)

# Post findings based on severity
if echo "$SECURITY_RESULTS" | grep -q "critical"; then
  # Request changes for critical issues
  gh pr review 123 --request-changes --body "$SECURITY_RESULTS"
  gh pr edit 123 --add-label "security-review-required"
else
  # Post as comment for non-critical issues
  gh pr comment 123 --body "$SECURITY_RESULTS"
fi
```

<details>
<summary><strong>Security Checks Performed</strong></summary>

```javascript
{
  "checks": [
    "SQL injection vulnerabilities",
    "XSS attack vectors",
    "Authentication bypasses",
    "Authorization flaws",
    "Cryptographic weaknesses",
    "Dependency vulnerabilities",
    "Secret exposure",
    "CORS misconfigurations"
  ],
  "actions": [
    "Block PR on critical issues",
    "Suggest secure alternatives",
    "Add security test cases",
    "Update security documentation"
  ]
}
```

</details>

<details>
<summary><strong>Comment Template: Security Issue</strong></summary>

```markdown
🔒 **Security Issue: [Type]**

**Severity**: 🔴 Critical / 🟡 High / 🟢 Low

**Description**:
[Clear explanation of the security issue]

**Impact**:
[Potential consequences if not addressed]

**Suggested Fix**:
```language
[Code example of the fix]
```

**References**:
- [OWASP Guide](link)
- [Security Best Practices](link)
```

</details>

---

### Performance Review Agent

**Focus:** Analyze performance impact and optimization opportunities

```bash
# Run performance analysis
npx ruv-swarm github review-performance \
  --pr 123 \
  --profile "cpu,memory,io" \
  --benchmark-against main \
  --suggest-optimizations
```

<details>
<summary><strong>Performance Metrics Analyzed</strong></summary>

```javascript
{
  "metrics": [
    "Algorithm complexity (Big O analysis)",
    "Database query efficiency",
    "Memory allocation patterns",
    "Cache utilization",
    "Network request optimization",
    "Bundle size impact",
    "Render performance"
  ],
  "benchmarks": [
    "Compare with baseline",
    "Load test simulations",
    "Memory leak detection",
    "Bottleneck identification"
  ]
}
```

</details>

---

### Architecture Review Agent

**Focus:** Evaluate design patterns and architectural decisions

```bash
# Architecture review
npx ruv-swarm github review-architecture \
  --pr 123 \
  --check "patterns,coupling,cohesion,solid" \
  --visualize-impact \
  --suggest-refactoring
```

<details>
<summary><strong>Architecture Analysis</strong></summary>

```javascript
{
  "patterns": [
    "Design pattern adherence",
    "SOLID principles",
    "DRY violations",
    "Separation of concerns",
    "Dependency injection",
    "Layer violations",
    "Circular dependencies"
  ],
  "metrics": [
    "Coupling metrics",
    "Cohesion scores",
    "Complexity measures",
    "Maintainability index"
  ]
}
```

</details>

---

### Style & Convention Agent

**Focus:** Enforce coding standards and best practices

```bash
# Style enforcement with auto-fix
npx ruv-swarm github review-style \
  --pr 123 \
  --check "formatting,naming,docs,tests" \
  --auto-fix "formatting,imports,whitespace"
```

<details>
<summary><strong>Style Checks</strong></summary>

```javascript
{
  "checks": [
    "Code formatting",
    "Naming conventions",
    "Documentation standards",
    "Comment quality",
    "Test coverage",
    "Error handling patterns",
    "Logging standards"
  ],
  "auto-fix": [
    "Formatting issues",
    "Import organization",
    "Trailing whitespace",
    "Simple naming issues"
  ]
}
```

</details>

---

## 🔄 PR-Based Swarm Management

### Create Swarm from PR

```bash
# Create swarm from PR description using gh CLI
gh pr view 123 --json body,title,labels,files | npx ruv-swarm swarm create-from-pr

# Auto-spawn agents based on PR labels
gh pr view 123 --json labels | npx ruv-swarm swarm auto-spawn

# Create swarm with full PR context
gh pr view 123 --json body,labels,author,assignees | \
  npx ruv-swarm swarm init --from-pr-data
```

### Label-Based Agent Assignment

Map PR labels to specialized agents:

```json
{
  "label-mapping": {
    "bug": ["debugger", "tester"],
    "feature": ["architect", "coder", "tester"],
    "refactor": ["analyst", "coder"],
    "docs": ["researcher", "writer"],
    "performance": ["analyst", "optimizer"],
    "security": ["security", "authentication", "audit"]
  }
}
```

### Topology Selection by PR Size

```bash
# Automatic topology selection based on PR complexity
# Small PR (< 100 lines): ring topology
# Medium PR (100-500 lines): mesh topology
# Large PR (> 500 lines): hierarchical topology
npx ruv-swarm github pr-topology --pr 123
```

---

## 🎬 PR Comment Commands

Execute swarm commands directly from PR comments:

```markdown
<!-- In PR comment -->
/swarm init mesh 6
/swarm spawn coder "Implement authentication"
/swarm spawn tester "Write unit tests"
/swarm status
/swarm review --agents security,performance
```

<details>
<summary><strong>Webhook Handler for Comment Commands</strong></summary>

```javascript
// webhook-handler.js
const { createServer } = require('http');
const { execSync } = require('child_process');

createServer((req, res) => {
  if (req.url === '/github-webhook') {
    const event = JSON.parse(body);

    if (event.action === 'opened' && event.pull_request) {
      execSync(`npx ruv-swarm github pr-init ${event.pull_request.number}`);
    }

    if (event.comment && event.comment.body.startsWith('/swarm')) {
      const command = event.comment.body;
      execSync(`npx ruv-swarm github handle-comment --pr ${event.issue.number} --command "${command}"`);
    }

    res.writeHead(200);
    res.end('OK');
  }
}).listen(3000);
```

</details>

---

## ⚙️ Review Configuration

### Configuration File

```yaml
# .github/review-swarm.yml
version: 1
review:
  auto-trigger: true
  required-agents:
    - security
    - performance
    - style
  optional-agents:
    - architecture
    - accessibility
    - i18n

  thresholds:
    security: block      # Block merge on security issues
    performance: warn    # Warn on performance issues
    style: suggest       # Suggest style improvements

  rules:
    security:
      - no-eval
      - no-hardcoded-secrets
      - proper-auth-checks
      - validate-input
    performance:
      - no-n-plus-one
      - efficient-queries
      - proper-caching
      - optimize-loops
    architecture:
      - max-coupling: 5
      - min-cohesion: 0.7
      - follow-patterns
      - avoid-circular-deps
```

### Custom Review Triggers

```javascript
{
  "triggers": {
    "high-risk-files": {
      "paths": ["**/auth/**", "**/payment/**", "**/admin/**"],
      "agents": ["security", "architecture"],
      "depth": "comprehensive",
      "require-approval": true
    },
    "performance-critical": {
      "paths": ["**/api/**", "**/database/**", "**/cache/**"],
      "agents": ["performance", "database"],
      "benchmarks": true,
      "regression-threshold": "5%"
    },
    "ui-changes": {
      "paths": ["**/components/**", "**/styles/**", "**/pages/**"],
      "agents": ["accessibility", "style", "i18n"],
      "visual-tests": true,
      "responsive-check": true
    }
  }
}
```

---

## 🤖 Automated Workflows

### Auto-Review on PR Creation

```yaml
# .github/workflows/auto-review.yml
name: Automated Code Review
on:
  pull_request:
    types: [opened, synchronize]
  issue_comment:
    types: [created]

jobs:
  swarm-review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
        with:
          fetch-depth: 0

      - name: Setup GitHub CLI
        run: echo "${{ secrets.GITHUB_TOKEN }}" | gh auth login --with-token

      - name: Run Review Swarm
        run: |
          # Get PR context with gh CLI
          PR_NUM=${{ github.event.pull_request.number }}
          PR_DATA=$(gh pr view $PR_NUM --json files,title,body,labels)
          PR_DIFF=$(gh pr diff $PR_NUM)

          # Run swarm review
          REVIEW_OUTPUT=$(npx ruv-swarm github review-all \
            --pr $PR_NUM \
            --pr-data "$PR_DATA" \
            --diff "$PR_DIFF" \
            --agents "security,performance,style,architecture")

          # Post review results
          echo "$REVIEW_OUTPUT" | gh pr review $PR_NUM --comment -F -

          # Update PR status
          if echo "$REVIEW_OUTPUT" | grep -q "approved"; then
            gh pr review $PR_NUM --approve
          elif echo "$REVIEW_OUTPUT" | grep -q "changes-requested"; then
            gh pr review $PR_NUM --request-changes -b "See review comments above"
          fi

      - name: Update Labels
        run: |
          # Add labels based on review results
          if echo "$REVIEW_OUTPUT" | grep -q "security"; then
            gh pr edit $PR_NUM --add-label "security-review"
          fi
          if echo "$REVIEW_OUTPUT" | grep -q "performance"; then
            gh pr edit $PR_NUM --add-label "performance-review"
          fi
```

---

## 💬 Intelligent Comment Generation

### Generate Contextual Review Comments

```bash
# Get PR diff with context
PR_DIFF=$(gh pr diff 123 --color never)
PR_FILES=$(gh pr view 123 --json files)

# Generate review comments
COMMENTS=$(npx ruv-swarm github review-comment \
  --pr 123 \
  --diff "$PR_DIFF" \
  --files "$PR_FILES" \
  --style "constructive" \
  --include-examples \
  --suggest-fixes)

# Post comments using gh CLI
echo "$COMMENTS" | jq -c '.[]' | while read -r comment; do
  FILE=$(echo "$comment" | jq -r '.path')
  LINE=$(echo "$comment" | jq -r '.line')
  BODY=$(echo "$comment" | jq -r '.body')
  COMMIT_ID=$(gh pr view 123 --json headRefOid -q .headRefOid)

  # Create inline review comments
  gh api \
    --method POST \
    /repos/:owner/:repo/pulls/123/comments \
    -f path="$FILE" \
    -f line="$LINE" \
    -f body="$BODY" \
    -f commit_id="$COMMIT_ID"
done
```

### Batch Comment Management

```bash
# Manage review comments efficiently
npx ruv-swarm github review-comments \
  --pr 123 \
  --group-by "agent,severity" \
  --summarize \
  --resolve-outdated
```

---

## 🚪 Quality Gates & Checks

### Status Checks

```yaml
# Required status checks in branch protection
protection_rules:
  required_status_checks:
    strict: true
    contexts:
      - "review-swarm/security"
      - "review-swarm/performance"
      - "review-swarm/architecture"
      - "review-swarm/tests"
```

### Define Quality Gates

```bash
# Set quality gate thresholds
npx ruv-swarm github quality-gates \
  --define '{
    "security": {"threshold": "no-critical"},
    "performance": {"regression": "<5%"},
    "coverage": {"minimum": "80%"},
    "architecture": {"complexity": "<10"},
    "duplication": {"maximum": "5%"}
  }'
```

### Track Review Metrics

```bash
# Monitor review effectiveness
npx ruv-swarm github review-metrics \
  --period 30d \
  --metrics "issues-found,false-positives,fix-rate,time-to-review" \
  --export-dashboard \
  --format json
```

---

## 🎓 Advanced Features

### Context-Aware Reviews

Analyze PRs with full project context:

```bash
# Review with comprehensive context
npx ruv-swarm github review-context \
  --pr 123 \
  --load-related-prs \
  --analyze-impact \
  --check-breaking-changes \
  --dependency-analysis
```

### Learning from History

Train review agents on your codebase patterns:

```bash
# Learn from past reviews
npx ruv-swarm github review-learn \
  --analyze-past-reviews \
  --identify-patterns \
  --improve-suggestions \
  --reduce-false-positives

# Train on your codebase
npx ruv-swarm github review-train \
  --learn-patterns \
  --adapt-to-style \
  --improve-accuracy
```

### Cross-PR Analysis

Coordinate reviews across related pull requests:

```bash
# Analyze related PRs together
npx ruv-swarm github review-batch \
  --prs "123,124,125" \
  --check-consistency \
  --verify-integration \
  --combined-impact
```

### Multi-PR Swarm Coordination

```bash
# Coordinate swarms across related PRs
npx ruv-swarm github multi-pr \
  --prs "123,124,125" \
  --strategy "parallel" \
  --share-memory
```

---

## 🛠️ Custom Review Agents

### Create Custom Agent

```javascript
// custom-review-agent.js
class CustomReviewAgent {
  constructor(config) {
    this.config = config;
    this.rules = config.rules || [];
  }

  async review(pr) {
    const issues = [];

    // Custom logic: Check for TODO comments in production code
    if (await this.checkTodoComments(pr)) {
      issues.push({
        severity: 'warning',
        file: pr.file,
        line: pr.line,
        message: 'TODO comment found in production code',
        suggestion: 'Resolve TODO or create issue to track it'
      });
    }

    // Custom logic: Verify API versioning
    if (await this.checkApiVersioning(pr)) {
      issues.push({
        severity: 'error',
        file: pr.file,
        line: pr.line,
        message: 'API endpoint missing versioning',
        suggestion: 'Add /v1/, /v2/ prefix to API routes'
      });
    }

    return issues;
  }

  async checkTodoComments(pr) {
    // Implementation
    const todoRegex = /\/\/\s*TODO|\/\*\s*TODO/gi;
    return todoRegex.test(pr.diff);
  }

  async checkApiVersioning(pr) {
    // Implementation
    const apiRegex = /app\.(get|post|put|delete)\(['"]\/api\/(?!v\d+)/;
    return apiRegex.test(pr.diff);
  }
}

module.exports = CustomReviewAgent;
```

### Register Custom Agent

```bash
# Register custom review agent
npx ruv-swarm github register-agent \
  --name "custom-reviewer" \
  --file "./custom-review-agent.js" \
  --category "standards"
```

---

## 🔧 CI/CD Integration

### Integration with Build Pipeline

```yaml
# .github/workflows/build-and-review.yml
name: Build and Review
on: [pull_request]

jobs:
  build-and-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: npm install
      - run: npm test
      - run: npm run build

  swarm-review:
    needs: build-and-test
    runs-on: ubuntu-latest
    steps:
      - name: Run Swarm Review
        run: |
          npx ruv-swarm github review-all \
            --pr ${{ github.event.pull_request.number }} \
            --include-build-results
```

### Automated PR Fixes

```bash
# Auto-fix common issues
npx ruv-swarm github pr-fix 123 \
  --issues "lint,test-failures,formatting" \
  --commit-fixes \
  --push-changes
```

### Progress Updates to PR

```bash
# Post swarm progress to PR using gh CLI
PROGRESS=$(npx ruv-swarm github pr-progress 123 --format markdown)

gh pr comment 123 --body "$PROGRESS"

# Update PR labels based on progress
if [[ $(echo "$PROGRESS" | grep -o '[0-9]\+%' | sed 's/%//') -gt 90 ]]; then
  gh pr edit 123 --add-label "ready-for-review"
fi
```

---

## 📋 Complete Workflow Examples

### Example 1: Security-Critical PR

```bash
# Review authentication system changes
npx ruv-swarm github review-init \
  --pr 456 \
  --agents "security,authentication,audit" \
  --depth "maximum" \
  --require-security-approval \
  --penetration-test
```

### Example 2: Performance-Sensitive PR

```bash
# Review database optimization
npx ruv-swarm github review-init \
  --pr 789 \
  --agents "performance,database,caching" \
  --benchmark \
  --profile \
  --load-test
```

### Example 3: UI Component PR

```bash
# Review new component library
npx ruv-swarm github review-init \
  --pr 321 \
  --agents "accessibility,style,i18n,docs" \
  --visual-regression \
  --component-tests \
  --responsive-check
```

### Example 4: Feature Development PR

```bash
# Review new feature implementation
gh pr view 456 --json body,labels,files | \
  npx ruv-swarm github pr-init 456 \
    --topology hierarchical \
    --agents "architect,coder,tester,security" \
    --auto-assign-tasks
```

### Example 5: Bug Fix PR

```bash
# Review bug fix with debugging focus
npx ruv-swarm github pr-init 789 \
  --topology mesh \
  --agents "debugger,analyst,tester" \
  --priority high \
  --regression-test
```

---

## 📊 Monitoring & Analytics

### Review Dashboard

```bash
# Launch real-time review dashboard
npx ruv-swarm github review-dashboard \
  --real-time \
  --show "agent-activity,issue-trends,fix-rates,coverage"
```

### Generate Review Reports

```bash
# Create comprehensive review report
npx ruv-swarm github review-report \
  --format "markdown" \
  --include "summary,details,trends,recommendations" \
  --email-stakeholders \
  --export-pdf
```

### PR Swarm Analytics

```bash
# Generate PR-specific analytics
npx ruv-swarm github pr-report 123 \
  --metrics "completion-time,agent-efficiency,token-usage,issue-density" \
  --format markdown \
  --compare-baseline
```

### Export to GitHub Insights

```bash
# Export metrics to GitHub Insights
npx ruv-swarm github export-metrics \
  --pr 123 \
  --to-insights \
  --dashboard-url
```

---

## 🔐 Security Considerations

### Best Practices

1. **Token Permissions**: Ensure GitHub tokens have minimal required scopes
2. **Command Validation**: Validate all PR comments before execution
3. **Rate Limiting**: Implement rate limits for PR operations
4. **Audit Trail**: Log all swarm operations for compliance
5. **Secret Management**: Never expose API keys in PR comments or logs

### Security Checklist

- [ ] GitHub token scoped to repository only
- [ ] Webhook signatures verified
- [ ] Command injection protection enabled
- [ ] Rate limiting configured
- [ ] Audit logging enabled
- [ ] Secrets scanning active
- [ ] Branch protection rules enforced

---

## 📚 Best Practices

### 1. Review Configuration
- ✅ Define clear review criteria upfront
- ✅ Set appropriate severity thresholds
- ✅ Configure agent specializations for your stack
- ✅ Establish override procedures for emergencies

### 2. Comment Quality
- ✅ Provide actionable, specific feedback
- ✅ Include code examples with suggestions
- ✅ Reference documentation and best practices
- ✅ Maintain respectful, constructive tone

### 3. Performance Optimization
- ✅ Cache analysis results to avoid redundant work
- ✅ Use incremental reviews for large PRs
- ✅ Enable parallel agent execution
- ✅ Batch comment operations efficiently

### 4. PR Templates

```markdown
<!-- .github/pull_request_template.md -->
## Swarm Configuration
- Topology: [mesh/hierarchical/ring/star]
- Max Agents: [number]
- Auto-spawn: [yes/no]
- Priority: [high/medium/low]

## Tasks for Swarm
- [ ] Task 1 description
- [ ] Task 2 description
- [ ] Task 3 description

## Review Focus Areas
- [ ] Security review
- [ ] Performance analysis
- [ ] Architecture validation
- [ ] Accessibility check
```

### 5. Auto-Merge When Ready

```bash
# Auto-merge when swarm completes and passes checks
SWARM_STATUS=$(npx ruv-swarm github pr-status 123)

if [[ "$SWARM_STATUS" == "complete" ]]; then
  # Check review requirements
  REVIEWS=$(gh pr view 123 --json reviews --jq '.reviews | length')

  if [[ $REVIEWS -ge 2 ]]; then
    # Enable auto-merge
    gh pr merge 123 --auto --squash
  fi
fi
```

---

## 🔗 Integration with Claude Code

### Workflow Pattern

1. **Claude Code** reads PR diff and context
2. **Swarm** coordinates review approach based on PR type
3. **Agents** work in parallel on different review aspects
4. **Progress** updates posted to PR automatically
5. **Final review** performed before marking ready

### Example: Complete PR Management

```javascript
[Single Message - Parallel Execution]:
  // Initialize coordination
  mcp__claude-flow__swarm_init { topology: "hierarchical", maxAgents: 5 }
  mcp__claude-flow__agent_spawn { type: "reviewer", name: "Senior Reviewer" }
  mcp__claude-flow__agent_spawn { type: "tester", name: "QA Engineer" }
  mcp__claude-flow__agent_spawn { type: "coordinator", name: "Merge Coordinator" }

  // Create and manage PR using gh CLI
  Bash("gh pr create --title 'Feature: Add authentication' --base main")
  Bash("gh pr view 54 --json files,diff")
  Bash("gh pr review 54 --approve --body 'LGTM after automated review'")

  // Execute tests and validation
  Bash("npm test")
  Bash("npm run lint")
  Bash("npm run build")

  // Track progress
  TodoWrite { todos: [
    { content: "Complete code review", status: "completed", activeForm: "Completing code review" },
    { content: "Run test suite", status: "completed", activeForm: "Running test suite" },
    { content: "Validate security", status: "completed", activeForm: "Validating security" },
    { content: "Merge when ready", status: "pending", activeForm: "Merging when ready" }
  ]}
```

---

## 🆘 Troubleshooting

### Common Issues

<details>
<summary><strong>Issue: Review agents not spawning</strong></summary>

**Solution:**
```bash
# Check swarm status
npx ruv-swarm swarm-status

# Verify GitHub CLI authentication
gh auth status

# Re-initialize swarm
npx ruv-swarm github review-init --pr 123 --force
```

</details>

<details>
<summary><strong>Issue: Comments not posting to PR</strong></summary>

**Solution:**
```bash
# Verify GitHub token permissions
gh auth status

# Check API rate limits
gh api rate_limit

# Use batch comment posting
npx ruv-swarm github review-comments --pr 123 --batch
```

</details>

<details>
<summary><strong>Issue: Review taking too long</strong></summary>

**Solution:**
```bash
# Use incremental review for large PRs
npx ruv-swarm github review-init --pr 123 --incremental

# Reduce agent count
npx ruv-swarm github review-init --pr 123 --agents "security,style" --max-agents 3

# Enable parallel processing
npx ruv-swarm github review-init --pr 123 --parallel --cache-results
```

</details>

---

## 📖 Additional Resources

### Related Skills
- `github-pr-manager` - Comprehensive PR lifecycle management
- `github-workflow-automation` - Automate GitHub workflows
- `swarm-coordination` - Advanced swarm orchestration

### Documentation
- [GitHub CLI Documentation](https://cli.github.com/manual/)
- [RUV Swarm Guide](https://github.com/ruvnet/ruv-swarm)
- [Claude Flow Integration](https://github.com/ruvnet/claude-flow)

### Support
- GitHub Issues: Report bugs and request features
- Community: Join discussions and share experiences
- Examples: Browse example configurations and workflows

---

## 📄 License

This skill is part of the Claude Code Flow project and is licensed under the MIT License.

---

**Last Updated:** 2025-10-19
**Version:** 1.0.0
**Maintainer:** Claude Code Flow Team
