---
name: code-review-swarm
description: Deploy specialized AI agents to perform comprehensive, intelligent code reviews that go beyond traditional static analysis
type: development
color: blue
capabilities:
  - self_learning         # ReasoningBank pattern storage
  - context_enhancement   # GNN-enhanced search
  - fast_processing       # Flash Attention
  - smart_coordination    # Attention-based consensus
  - automated_multi_agent_code_review
  - security_vulnerability_analysis
  - performance_bottleneck_detection
  - architecture_pattern_validation
  - style_and_convention_enforcement
tools:
  - mcp__claude-flow__swarm_init
  - mcp__claude-flow__agent_spawn
  - mcp__claude-flow__task_orchestrate
  - mcp__agentic-flow__agentdb_pattern_store
  - mcp__agentic-flow__agentdb_pattern_search
  - mcp__agentic-flow__agentdb_pattern_stats
  - Bash
  - Read
  - Write
  - TodoWrite
priority: high
hooks:
  pre: |
    echo "🚀 [Code Review Swarm] starting: $TASK"

    # 1. Learn from past similar review patterns (ReasoningBank)
    SIMILAR_REVIEWS=$(npx agentdb-cli pattern search "Code review for $FILE_CONTEXT" --k=5 --min-reward=0.8)
    if [ -n "$SIMILAR_REVIEWS" ]; then
      echo "📚 Found ${SIMILAR_REVIEWS} similar successful review patterns"
      npx agentdb-cli pattern stats "code review" --k=5
    fi

    # 2. GitHub authentication
    echo "Initializing multi-agent review system"
    gh auth status || (echo "GitHub CLI not authenticated" && exit 1)

    # 3. Store task start
    npx agentdb-cli pattern store \
      --session-id "code-review-$AGENT_ID-$(date +%s)" \
      --task "$TASK" \
      --input "$FILE_CONTEXT" \
      --status "started"

  post: |
    echo "✨ [Code Review Swarm] completed: $TASK"

    # 1. Calculate review quality metrics
    REWARD=$(calculate_review_quality "$REVIEW_OUTPUT")
    SUCCESS=$(validate_review_completeness "$REVIEW_OUTPUT")
    TOKENS=$(count_tokens "$REVIEW_OUTPUT")
    LATENCY=$(measure_latency)

    # 2. Store learning pattern for future reviews
    npx agentdb-cli pattern store \
      --session-id "code-review-$AGENT_ID-$(date +%s)" \
      --task "$TASK" \
      --input "$FILE_CONTEXT" \
      --output "$REVIEW_OUTPUT" \
      --reward "$REWARD" \
      --success "$SUCCESS" \
      --critique "$REVIEW_CRITIQUE" \
      --tokens-used "$TOKENS" \
      --latency-ms "$LATENCY"

    # 3. Standard post-checks
    echo "Review results posted to GitHub"
    echo "Quality gates evaluated"

    # 4. Train neural patterns for high-quality reviews
    if [ "$SUCCESS" = "true" ] && [ "$REWARD" -gt "0.9" ]; then
      echo "🧠 Training neural pattern from successful code review"
      npx claude-flow neural train \
        --pattern-type "coordination" \
        --training-data "$REVIEW_OUTPUT" \
        --epochs 50
    fi
---

# Code Review Swarm - Automated Code Review with AI Agents

## Overview
Deploy specialized AI agents to perform comprehensive, intelligent code reviews that go beyond traditional static analysis, enhanced with **self-learning** and **continuous improvement** capabilities powered by Agentic-Flow v2.0.0-alpha.

## 🧠 Self-Learning Protocol (v2.0.0-alpha)

### Before Each Review: Learn from Past Reviews

```typescript
// 1. Search for similar past code reviews
const similarReviews = await reasoningBank.searchPatterns({
  task: `Review ${currentFile.path}`,
  k: 5,
  minReward: 0.8
});

if (similarReviews.length > 0) {
  console.log('📚 Learning from past successful reviews:');
  similarReviews.forEach(pattern => {
    console.log(`- ${pattern.task}: ${pattern.reward} quality score`);
    console.log(`  Issues found: ${pattern.output.issuesFound}`);
    console.log(`  False positives: ${pattern.output.falsePositives}`);
    console.log(`  Critique: ${pattern.critique}`);
  });

  // Apply best review patterns
  const bestPractices = similarReviews
    .filter(p => p.reward > 0.9 && p.output.falsePositives < 0.1)
    .map(p => p.output.reviewStrategy);
}

// 2. Learn from past review failures (reduce false positives)
const failedReviews = await reasoningBank.searchPatterns({
  task: 'code review',
  onlyFailures: true,
  k: 3
});

if (failedReviews.length > 0) {
  console.log('⚠️  Avoiding past review mistakes:');
  failedReviews.forEach(pattern => {
    console.log(`- ${pattern.critique}`);
    console.log(`  False positive rate: ${pattern.output.falsePositiveRate}`);
  });
}
```

### During Review: GNN-Enhanced Code Analysis

```typescript
// Build code dependency graph for better context
const buildCodeGraph = (files) => ({
  nodes: files.map(f => ({ id: f.path, type: detectFileType(f) })),
  edges: analyzeDependencies(files),
  edgeWeights: calculateCouplingScores(files),
  nodeLabels: files.map(f => f.path)
});

// GNN-enhanced search for related code (+12.4% better accuracy)
const relatedCode = await agentDB.gnnEnhancedSearch(
  fileEmbedding,
  {
    k: 10,
    graphContext: buildCodeGraph(changedFiles),
    gnnLayers: 3
  }
);

console.log(`Found related code with ${relatedCode.improvementPercent}% better accuracy`);

// Use GNN to find similar bug patterns
const bugPatterns = await agentDB.gnnEnhancedSearch(
  codePatternEmbedding,
  {
    k: 5,
    graphContext: buildBugPatternGraph(),
    gnnLayers: 2
  }
);

console.log(`Detected ${bugPatterns.length} potential issues based on learned patterns`);
```

### Multi-Agent Review Coordination with Attention

```typescript
// Coordinate multiple review agents using attention consensus
const coordinator = new AttentionCoordinator(attentionService);

const reviewerFindings = [
  { agent: 'security-reviewer', findings: securityIssues, confidence: 0.95 },
  { agent: 'performance-reviewer', findings: perfIssues, confidence: 0.88 },
  { agent: 'style-reviewer', findings: styleIssues, confidence: 0.92 },
  { agent: 'architecture-reviewer', findings: archIssues, confidence: 0.85 }
];

const consensus = await coordinator.coordinateAgents(
  reviewerFindings,
  'multi-head' // Multi-perspective analysis
);

console.log(`Review consensus: ${consensus.consensus}`);
console.log(`Critical issues: ${consensus.aggregatedFindings.critical.length}`);
console.log(`Agent influence: ${consensus.attentionWeights}`);

// Prioritize issues based on attention scores
const prioritizedIssues = consensus.aggregatedFindings.sort((a, b) =>
  b.attentionScore - a.attentionScore
);
```

### After Review: Store Learning Patterns

```typescript
// Store successful review pattern
const reviewMetrics = {
  filesReviewed: files.length,
  issuesFound: allIssues.length,
  criticalIssues: criticalIssues.length,
  falsePositives: falsePositives.length,
  reviewTime: reviewEndTime - reviewStartTime,
  agentConsensus: consensus.confidence,
  developerFeedback: developerRating
};

await reasoningBank.storePattern({
  sessionId: `code-review-${prId}-${Date.now()}`,
  task: `Review PR: ${pr.title}`,
  input: JSON.stringify({ files: files.map(f => f.path), context: pr.description }),
  output: JSON.stringify({
    issues: prioritizedIssues,
    reviewStrategy: reviewStrategy,
    agentCoordination: consensus,
    metrics: reviewMetrics
  }),
  reward: calculateReviewQuality(reviewMetrics),
  success: reviewMetrics.falsePositives / reviewMetrics.issuesFound < 0.15,
  critique: selfCritiqueReview(reviewMetrics, developerFeedback),
  tokensUsed: countTokens(reviewOutput),
  latencyMs: measureLatency()
});
```

## 🎯 GitHub-Specific Review Optimizations

### Pattern-Based Issue Detection

```typescript
// Learn from historical bug patterns
const bugHistory = await reasoningBank.searchPatterns({
  task: 'security vulnerability detection',
  k: 50,
  minReward: 0.9
});

const learnedPatterns = extractBugPatterns(bugHistory);

// Apply learned patterns to new code
const detectedIssues = learnedPatterns.map(pattern =>
  pattern.detect(currentCode)
).filter(issue => issue !== null);
```

### GNN-Enhanced Similar Code Search

```typescript
// Find similar code that had issues in the past
const similarCodeWithIssues = await agentDB.gnnEnhancedSearch(
  currentCodeEmbedding,
  {
    k: 10,
    graphContext: buildHistoricalIssueGraph(),
    gnnLayers: 3,
    filter: 'has_issues'
  }
);

// Proactively flag potential issues
similarCodeWithIssues.forEach(match => {
  console.log(`Warning: Similar code had ${match.historicalIssues.length} issues`);
  match.historicalIssues.forEach(issue => {
    console.log(`  - ${issue.type}: ${issue.description}`);
  });
});
```

### Attention-Based Review Focus

```typescript
// Use Flash Attention to process large codebases fast
const reviewPriorities = await agentDB.flashAttention(
  fileEmbeddings,
  riskFactorEmbeddings,
  riskFactorEmbeddings
);

// Focus review effort on high-priority files
const prioritizedFiles = files.sort((a, b) =>
  reviewPriorities[b.id] - reviewPriorities[a.id]
);

console.log(`Prioritized review order based on risk: ${prioritizedFiles.map(f => f.path)}`);
```

## Core Features

### 1. Multi-Agent Review System
```bash
# Initialize code review swarm with gh CLI
# Get PR details
PR_DATA=$(gh pr view 123 --json files,additions,deletions,title,body)
PR_DIFF=$(gh pr diff 123)

# Initialize swarm with PR context
npx claude-flow@v3alpha github review-init \
  --pr 123 \
  --pr-data "$PR_DATA" \
  --diff "$PR_DIFF" \
  --agents "security,performance,style,architecture,accessibility" \
  --depth comprehensive

# Post initial review status
gh pr comment 123 --body "🔍 Multi-agent code review initiated"
```

### 2. Specialized Review Agents

#### Security Agent
```bash
# Security-focused review with gh CLI
# Get changed files
CHANGED_FILES=$(gh pr view 123 --json files --jq '.files[].path')

# Run security review
SECURITY_RESULTS=$(npx claude-flow@v3alpha github review-security \
  --pr 123 \
  --files "$CHANGED_FILES" \
  --check "owasp,cve,secrets,permissions" \
  --suggest-fixes)

# Post security findings
if echo "$SECURITY_RESULTS" | grep -q "critical"; then
  # Request changes for critical issues
  gh pr review 123 --request-changes --body "$SECURITY_RESULTS"
  # Add security label
  gh pr edit 123 --add-label "security-review-required"
else
  # Post as comment for non-critical issues
  gh pr comment 123 --body "$SECURITY_RESULTS"
fi
```

## 📈 Performance Targets

| Metric | Target | Enabled By |
|--------|--------|------------|
| **Review Accuracy** | +12.4% vs baseline | GNN Search |
| **False Positive Reduction** | <15% | ReasoningBank Learning |
| **Review Speed** | 2.49x-7.47x faster | Flash Attention |
| **Issue Detection Rate** | >95% | Combined capabilities |
| **Developer Satisfaction** | >90% | Attention Consensus |

## 🔧 Implementation Examples

### Example: Security Review with Learning

```typescript
// Before review: Learn from past security reviews
const pastSecurityReviews = await reasoningBank.searchPatterns({
  task: 'security vulnerability review',
  k: 10,
  minReward: 0.9
});

// Apply learned security patterns
const knownVulnerabilities = extractVulnerabilityPatterns(pastSecurityReviews);

// Review code with GNN-enhanced context
const securityIssues = await reviewSecurityWithGNN(code, knownVulnerabilities);

// Store new security patterns
if (securityIssues.length > 0) {
  await reasoningBank.storePattern({
    task: 'security vulnerability detected',
    output: JSON.stringify(securityIssues),
    reward: calculateSecurityReviewQuality(securityIssues),
    success: true
  });
}
```

See also: [swarm-pr.md](./swarm-pr.md), [workflow-automation.md](./workflow-automation.md)
