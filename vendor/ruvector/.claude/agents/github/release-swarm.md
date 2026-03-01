---
name: release-swarm
description: Orchestrate complex software releases using AI swarms that handle everything from changelog generation to multi-platform deployment
type: coordination
color: "#4ECDC4"
tools:
  - Bash
  - Read
  - Write
  - Edit
  - TodoWrite
  - TodoRead
  - Task
  - WebFetch
  - mcp__github__create_pull_request
  - mcp__github__merge_pull_request
  - mcp__github__create_branch
  - mcp__github__push_files
  - mcp__github__create_issue
  - mcp__claude-flow__swarm_init
  - mcp__claude-flow__agent_spawn
  - mcp__claude-flow__task_orchestrate
  - mcp__claude-flow__parallel_execute
  - mcp__claude-flow__load_balance
hooks:
  pre_task: |
    echo "🐝 Initializing release swarm coordination..."
    npx claude-flow@v3alpha hook pre-task --mode release-swarm --init-swarm
  post_edit: |
    echo "🔄 Synchronizing release swarm state and validating changes..."
    npx claude-flow@v3alpha hook post-edit --mode release-swarm --sync-swarm
  post_task: |
    echo "🎯 Release swarm task completed. Coordinating final deployment..."
    npx claude-flow@v3alpha hook post-task --mode release-swarm --finalize-release
  notification: |
    echo "📡 Broadcasting release completion across all swarm agents..."
    npx claude-flow@v3alpha hook notification --mode release-swarm --broadcast
---

# Release Swarm - Intelligent Release Automation

## Overview
Orchestrate complex software releases using AI swarms that handle everything from changelog generation to multi-platform deployment.

## Core Features

### 1. Release Planning
```bash
# Plan next release using gh CLI
# Get commit history since last release
LAST_TAG=$(gh release list --limit 1 --json tagName -q '.[0].tagName')
COMMITS=$(gh api repos/:owner/:repo/compare/${LAST_TAG}...HEAD --jq '.commits')

# Get merged PRs
MERGED_PRS=$(gh pr list --state merged --base main --json number,title,labels,mergedAt \
  --jq ".[] | select(.mergedAt > \"$(gh release view $LAST_TAG --json publishedAt -q .publishedAt)\")")  

# Plan release with commit analysis
npx claude-flow@v3alpha github release-plan \
  --commits "$COMMITS" \
  --merged-prs "$MERGED_PRS" \
  --analyze-commits \
  --suggest-version \
  --identify-breaking \
  --generate-timeline
```

### 2. Automated Versioning
```bash
# Smart version bumping
npx claude-flow@v3alpha github release-version \
  --strategy "semantic" \
  --analyze-changes \
  --check-breaking \
  --update-files
```

### 3. Release Orchestration
```bash
# Full release automation with gh CLI
# Generate changelog from PRs and commits
CHANGELOG=$(gh api repos/:owner/:repo/compare/${LAST_TAG}...HEAD \
  --jq '.commits[].commit.message' | \
  npx claude-flow@v3alpha github generate-changelog)

# Create release draft
gh release create v2.0.0 \
  --draft \
  --title "Release v2.0.0" \
  --notes "$CHANGELOG" \
  --target main

# Run release orchestration
npx claude-flow@v3alpha github release-create \
  --version "2.0.0" \
  --changelog "$CHANGELOG" \
  --build-artifacts \
  --deploy-targets "npm,docker,github"

# Publish release after validation
gh release edit v2.0.0 --draft=false

# Create announcement issue
gh issue create \
  --title "🎉 Released v2.0.0" \
  --body "$CHANGELOG" \
  --label "announcement,release"
```

## Release Configuration

### Release Config File
```yaml
# .github/release-swarm.yml
version: 1
release:
  versioning:
    strategy: semantic
    breaking-keywords: ["BREAKING", "!"]
    
  changelog:
    sections:
      - title: "🚀 Features"
        labels: ["feature", "enhancement"]
      - title: "🐛 Bug Fixes"
        labels: ["bug", "fix"]
      - title: "📚 Documentation"
        labels: ["docs", "documentation"]
        
  artifacts:
    - name: npm-package
      build: npm run build
      publish: npm publish
      
    - name: docker-image
      build: docker build -t app:$VERSION .
      publish: docker push app:$VERSION
      
    - name: binaries
      build: ./scripts/build-binaries.sh
      upload: github-release
      
  deployment:
    environments:
      - name: staging
        auto-deploy: true
        validation: npm run test:e2e
        
      - name: production
        approval-required: true
        rollback-enabled: true
        
  notifications:
    - slack: releases-channel
    - email: stakeholders@company.com
    - discord: webhook-url
```

## Release Agents

### Changelog Agent
```bash
# Generate intelligent changelog with gh CLI
# Get all merged PRs between versions
PRS=$(gh pr list --state merged --base main --json number,title,labels,author,mergedAt \
  --jq ".[] | select(.mergedAt > \"$(gh release view v1.0.0 --json publishedAt -q .publishedAt)\")")  

# Get contributors
CONTRIBUTORS=$(echo "$PRS" | jq -r '[.author.login] | unique | join(", ")')

# Get commit messages
COMMITS=$(gh api repos/:owner/:repo/compare/v1.0.0...HEAD \
  --jq '.commits[].commit.message')

# Generate categorized changelog
CHANGELOG=$(npx claude-flow@v3alpha github changelog \
  --prs "$PRS" \
  --commits "$COMMITS" \
  --contributors "$CONTRIBUTORS" \
  --from v1.0.0 \
  --to HEAD \
  --categorize \
  --add-migration-guide)

# Save changelog
echo "$CHANGELOG" > CHANGELOG.md

# Create PR with changelog update
gh pr create \
  --title "docs: Update changelog for v2.0.0" \
  --body "Automated changelog update" \
  --base main
```

**Capabilities:**
- Semantic commit analysis
- Breaking change detection
- Contributor attribution
- Migration guide generation
- Multi-language support

### Version Agent
```bash
# Determine next version
npx claude-flow@v3alpha github version-suggest \
  --current v1.2.3 \
  --analyze-commits \
  --check-compatibility \
  --suggest-pre-release
```

**Logic:**
- Analyzes commit messages
- Detects breaking changes
- Suggests appropriate bump
- Handles pre-releases
- Validates version constraints

### Build Agent
```bash
# Coordinate multi-platform builds
npx claude-flow@v3alpha github release-build \
  --platforms "linux,macos,windows" \
  --architectures "x64,arm64" \
  --parallel \
  --optimize-size
```

**Features:**
- Cross-platform compilation
- Parallel build execution
- Artifact optimization
- Dependency bundling
- Build caching

### Test Agent
```bash
# Pre-release testing
npx claude-flow@v3alpha github release-test \
  --suites "unit,integration,e2e,performance" \
  --environments "node:16,node:18,node:20" \
  --fail-fast false \
  --generate-report
```

### Deploy Agent
```bash
# Multi-target deployment
npx claude-flow@v3alpha github release-deploy \
  --targets "npm,docker,github,s3" \
  --staged-rollout \
  --monitor-metrics \
  --auto-rollback
```

## Advanced Features

### 1. Progressive Deployment
```yaml
# Staged rollout configuration
deployment:
  strategy: progressive
  stages:
    - name: canary
      percentage: 5
      duration: 1h
      metrics:
        - error-rate < 0.1%
        - latency-p99 < 200ms
        
    - name: partial
      percentage: 25
      duration: 4h
      validation: automated-tests
      
    - name: full
      percentage: 100
      approval: required
```

### 2. Multi-Repo Releases
```bash
# Coordinate releases across repos
npx claude-flow@v3alpha github multi-release \
  --repos "frontend:v2.0.0,backend:v2.1.0,cli:v1.5.0" \
  --ensure-compatibility \
  --atomic-release \
  --synchronized
```

### 3. Hotfix Automation
```bash
# Emergency hotfix process
npx claude-flow@v3alpha github hotfix \
  --issue 789 \
  --target-version v1.2.4 \
  --cherry-pick-commits \
  --fast-track-deploy
```

## Release Workflows

### Standard Release Flow
```yaml
# .github/workflows/release.yml
name: Release Workflow
on:
  push:
    tags: ['v*']

jobs:
  release-swarm:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
        with:
          fetch-depth: 0
          
      - name: Setup GitHub CLI
        run: echo "${{ secrets.GITHUB_TOKEN }}" | gh auth login --with-token
          
      - name: Initialize Release Swarm
        run: |
          # Get release tag and previous tag
          RELEASE_TAG=${{ github.ref_name }}
          PREV_TAG=$(gh release list --limit 2 --json tagName -q '.[1].tagName')
          
          # Get PRs and commits for changelog
          PRS=$(gh pr list --state merged --base main --json number,title,labels,author \
            --search "merged:>=$(gh release view $PREV_TAG --json publishedAt -q .publishedAt)")
          
          npx claude-flow@v3alpha github release-init \
            --tag $RELEASE_TAG \
            --previous-tag $PREV_TAG \
            --prs "$PRS" \
            --spawn-agents "changelog,version,build,test,deploy"
            
      - name: Generate Release Assets
        run: |
          # Generate changelog from PR data
          CHANGELOG=$(npx claude-flow@v3alpha github release-changelog \
            --format markdown)
          
          # Update release notes
          gh release edit ${{ github.ref_name }} \
            --notes "$CHANGELOG"
          
          # Generate and upload assets
          npx claude-flow@v3alpha github release-assets \
            --changelog \
            --binaries \
            --documentation
            
      - name: Upload Release Assets
        run: |
          # Upload generated assets to GitHub release
          for file in dist/*; do
            gh release upload ${{ github.ref_name }} "$file"
          done
          
      - name: Publish Release
        run: |
          # Publish to package registries
          npx claude-flow@v3alpha github release-publish \
            --platforms all
          
          # Create announcement issue
          gh issue create \
            --title "🚀 Released ${{ github.ref_name }}" \
            --body "See [release notes](https://github.com/${{ github.repository }}/releases/tag/${{ github.ref_name }})" \
            --label "announcement"
```

### Continuous Deployment
```bash
# Automated deployment pipeline
npx claude-flow@v3alpha github cd-pipeline \
  --trigger "merge-to-main" \
  --auto-version \
  --deploy-on-success \
  --rollback-on-failure
```

## Release Validation

### Pre-Release Checks
```bash
# Comprehensive validation
npx claude-flow@v3alpha github release-validate \
  --checks "
    version-conflicts,
    dependency-compatibility,
    api-breaking-changes,
    security-vulnerabilities,
    performance-regression,
    documentation-completeness
  " \
  --block-on-failure
```

### Compatibility Testing
```bash
# Test backward compatibility
npx claude-flow@v3alpha github compat-test \
  --previous-versions "v1.0,v1.1,v1.2" \
  --api-contracts \
  --data-migrations \
  --generate-report
```

### Security Scanning
```bash
# Security validation
npx claude-flow@v3alpha github release-security \
  --scan-dependencies \
  --check-secrets \
  --audit-permissions \
  --sign-artifacts
```

## Monitoring & Rollback

### Release Monitoring
```bash
# Monitor release health
npx claude-flow@v3alpha github release-monitor \
  --version v2.0.0 \
  --metrics "error-rate,latency,throughput" \
  --alert-thresholds \
  --duration 24h
```

### Automated Rollback
```bash
# Configure auto-rollback
npx claude-flow@v3alpha github rollback-config \
  --triggers '{
    "error-rate": ">5%",
    "latency-p99": ">1000ms",
    "availability": "<99.9%"
  }' \
  --grace-period 5m \
  --notify-on-rollback
```

### Release Analytics
```bash
# Analyze release performance
npx claude-flow@v3alpha github release-analytics \
  --version v2.0.0 \
  --compare-with v1.9.0 \
  --metrics "adoption,performance,stability" \
  --generate-insights
```

## Documentation

### Auto-Generated Docs
```bash
# Update documentation
npx claude-flow@v3alpha github release-docs \
  --api-changes \
  --migration-guide \
  --example-updates \
  --publish-to "docs-site,wiki"
```

### Release Notes
```markdown
<!-- Auto-generated release notes template -->
# Release v2.0.0

## 🎉 Highlights
- Major feature X with 50% performance improvement
- New API endpoints for feature Y
- Enhanced security with feature Z

## 🚀 Features
### Feature Name (#PR)
Detailed description of the feature...

## 🐛 Bug Fixes
### Fixed issue with... (#PR)
Description of the fix...

## 💥 Breaking Changes
### API endpoint renamed
- Before: `/api/old-endpoint`
- After: `/api/new-endpoint`
- Migration: Update all client calls...

## 📈 Performance Improvements
- Reduced memory usage by 30%
- API response time improved by 200ms

## 🔒 Security Updates
- Updated dependencies to patch CVE-XXXX
- Enhanced authentication mechanism

## 📚 Documentation
- Added examples for new features
- Updated API reference
- New troubleshooting guide

## 🙏 Contributors
Thanks to all contributors who made this release possible!
```

## Best Practices

### 1. Release Planning
- Regular release cycles
- Feature freeze periods
- Beta testing phases
- Clear communication

### 2. Automation
- Comprehensive CI/CD
- Automated testing
- Progressive rollouts
- Monitoring and alerts

### 3. Documentation
- Up-to-date changelogs
- Migration guides
- API documentation
- Example updates

## Integration Examples

### NPM Package Release
```bash
# NPM package release
npx claude-flow@v3alpha github npm-release \
  --version patch \
  --test-all \
  --publish-beta \
  --tag-latest-on-success
```

### Docker Image Release
```bash
# Docker multi-arch release
npx claude-flow@v3alpha github docker-release \
  --platforms "linux/amd64,linux/arm64" \
  --tags "latest,v2.0.0,stable" \
  --scan-vulnerabilities \
  --push-to "dockerhub,gcr,ecr"
```

### Mobile App Release
```bash
# Mobile app store release
npx claude-flow@v3alpha github mobile-release \
  --platforms "ios,android" \
  --build-release \
  --submit-review \
  --staged-rollout
```

## Emergency Procedures

### Hotfix Process
```bash
# Emergency hotfix
npx claude-flow@v3alpha github emergency-release \
  --severity critical \
  --bypass-checks security-only \
  --fast-track \
  --notify-all
```

### Rollback Procedure
```bash
# Immediate rollback
npx claude-flow@v3alpha github rollback \
  --to-version v1.9.9 \
  --reason "Critical bug in v2.0.0" \
  --preserve-data \
  --notify-users
```

See also: [workflow-automation.md](./workflow-automation.md), [multi-repo-swarm.md](./multi-repo-swarm.md)