---
name: agentic-jujutsu
version: 2.3.2
description: Quantum-resistant, self-learning version control for AI agents with ReasoningBank intelligence and multi-agent coordination
hooks:
  pre: |
    echo "🧠 Agentic Jujutsu activated"
    if [ -d "/workspaces/ruvector/.claude/intelligence" ]; then
      cd /workspaces/ruvector/.claude/intelligence
      INTELLIGENCE_MODE=treatment node cli.js pre-edit "$FILE" 2>/dev/null || true
    fi
  post: |
    echo "✅ Agentic Jujutsu complete"
    if [ -d "/workspaces/ruvector/.claude/intelligence" ]; then
      cd /workspaces/ruvector/.claude/intelligence
      INTELLIGENCE_MODE=treatment node cli.js post-edit "$FILE" "true" 2>/dev/null || true
    fi
---

# Agentic Jujutsu - AI Agent Version Control

> Quantum-ready, self-learning version control designed for multiple AI agents working simultaneously without conflicts.

## 🧠 Self-Learning Intelligence
Integrates with RuVector's Q-learning and vector memory for improved performance.
CLI: `node .claude/intelligence/cli.js stats`

## When to Use This Skill

Use **agentic-jujutsu** when you need:
- ✅ Multiple AI agents modifying code simultaneously
- ✅ Lock-free version control (23x faster than Git)
- ✅ Self-learning AI that improves from experience
- ✅ Quantum-resistant security for future-proof protection
- ✅ Automatic conflict resolution (87% success rate)
- ✅ Pattern recognition and intelligent suggestions
- ✅ Multi-agent coordination without blocking

## Quick Start

### Installation

```bash
npx agentic-jujutsu
```

### Basic Usage

```javascript
const { JjWrapper } = require('agentic-jujutsu');

const jj = new JjWrapper();

// Basic operations
await jj.status();
await jj.newCommit('Add feature');
await jj.log(10);

// Self-learning trajectory
const id = jj.startTrajectory('Implement authentication');
await jj.branchCreate('feature/auth');
await jj.newCommit('Add auth');
jj.addToTrajectory();
jj.finalizeTrajectory(0.9, 'Clean implementation');

// Get AI suggestions
const suggestion = JSON.parse(jj.getSuggestion('Add logout feature'));
console.log(`Confidence: ${suggestion.confidence}`);
```

## Core Capabilities

### 1. Self-Learning with ReasoningBank

Track operations, learn patterns, and get intelligent suggestions:

```javascript
// Start learning trajectory
const trajectoryId = jj.startTrajectory('Deploy to production');

// Perform operations (automatically tracked)
await jj.execute(['git', 'push', 'origin', 'main']);
await jj.branchCreate('release/v1.0');
await jj.newCommit('Release v1.0');

// Record operations to trajectory
jj.addToTrajectory();

// Finalize with success score (0.0-1.0) and critique
jj.finalizeTrajectory(0.95, 'Deployment successful, no issues');

// Later: Get AI-powered suggestions for similar tasks
const suggestion = JSON.parse(jj.getSuggestion('Deploy to staging'));
console.log('AI Recommendation:', suggestion.reasoning);
console.log('Confidence:', (suggestion.confidence * 100).toFixed(1) + '%');
console.log('Expected Success:', (suggestion.expectedSuccessRate * 100).toFixed(1) + '%');
```

**Validation (v2.3.1)**:
- ✅ Tasks must be non-empty (max 10KB)
- ✅ Success scores must be 0.0-1.0
- ✅ Must have operations before finalizing
- ✅ Contexts cannot be empty

### 2. Pattern Discovery

Automatically identify successful operation sequences:

```javascript
// Get discovered patterns
const patterns = JSON.parse(jj.getPatterns());

patterns.forEach(pattern => {
    console.log(`Pattern: ${pattern.name}`);
    console.log(`  Success rate: ${(pattern.successRate * 100).toFixed(1)}%`);
    console.log(`  Used ${pattern.observationCount} times`);
    console.log(`  Operations: ${pattern.operationSequence.join(' → ')}`);
    console.log(`  Confidence: ${(pattern.confidence * 100).toFixed(1)}%`);
});
```

### 3. Learning Statistics

Track improvement over time:

```javascript
const stats = JSON.parse(jj.getLearningStats());

console.log('Learning Progress:');
console.log(`  Total trajectories: ${stats.totalTrajectories}`);
console.log(`  Patterns discovered: ${stats.totalPatterns}`);
console.log(`  Average success: ${(stats.avgSuccessRate * 100).toFixed(1)}%`);
console.log(`  Improvement rate: ${(stats.improvementRate * 100).toFixed(1)}%`);
console.log(`  Prediction accuracy: ${(stats.predictionAccuracy * 100).toFixed(1)}%`);
```

### 4. Multi-Agent Coordination

Multiple agents work concurrently without conflicts:

```javascript
// Agent 1: Developer
const dev = new JjWrapper();
dev.startTrajectory('Implement feature');
await dev.newCommit('Add feature X');
dev.addToTrajectory();
dev.finalizeTrajectory(0.85);

// Agent 2: Reviewer (learns from Agent 1)
const reviewer = new JjWrapper();
const suggestion = JSON.parse(reviewer.getSuggestion('Review feature X'));

if (suggestion.confidence > 0.7) {
    console.log('High confidence approach:', suggestion.reasoning);
}

// Agent 3: Tester (benefits from both)
const tester = new JjWrapper();
const similar = JSON.parse(tester.queryTrajectories('test feature', 5));
console.log(`Found ${similar.length} similar test approaches`);
```

### 5. Quantum-Resistant Security (v2.3.0+)

Fast integrity verification with quantum-resistant cryptography:

```javascript
const { generateQuantumFingerprint, verifyQuantumFingerprint } = require('agentic-jujutsu');

// Generate SHA3-512 fingerprint (NIST FIPS 202)
const data = Buffer.from('commit-data');
const fingerprint = generateQuantumFingerprint(data);
console.log('Fingerprint:', fingerprint.toString('hex'));

// Verify integrity (<1ms)
const isValid = verifyQuantumFingerprint(data, fingerprint);
console.log('Valid:', isValid);

// HQC-128 encryption for trajectories
const crypto = require('crypto');
const key = crypto.randomBytes(32).toString('base64');
jj.enableEncryption(key);
```

### 6. Operation Tracking with AgentDB

Automatic tracking of all operations:

```javascript
// Operations are tracked automatically
await jj.status();
await jj.newCommit('Fix bug');
await jj.rebase('main');

// Get operation statistics
const stats = JSON.parse(jj.getStats());
console.log(`Total operations: ${stats.total_operations}`);
console.log(`Success rate: ${(stats.success_rate * 100).toFixed(1)}%`);
console.log(`Avg duration: ${stats.avg_duration_ms.toFixed(2)}ms`);

// Query recent operations
const ops = jj.getOperations(10);
ops.forEach(op => {
    console.log(`${op.operationType}: ${op.command}`);
    console.log(`  Duration: ${op.durationMs}ms, Success: ${op.success}`);
});

// Get user operations (excludes snapshots)
const userOps = jj.getUserOperations(20);
```

## Advanced Use Cases

### Use Case 1: Adaptive Workflow Optimization

Learn and improve deployment workflows:

```javascript
async function adaptiveDeployment(jj, environment) {
    // Get AI suggestion based on past deployments
    const suggestion = JSON.parse(jj.getSuggestion(`Deploy to ${environment}`));
    
    console.log(`Deploying with ${(suggestion.confidence * 100).toFixed(0)}% confidence`);
    console.log(`Expected duration: ${suggestion.estimatedDurationMs}ms`);
    
    // Start tracking
    jj.startTrajectory(`Deploy to ${environment}`);
    
    // Execute recommended operations
    for (const op of suggestion.recommendedOperations) {
        console.log(`Executing: ${op}`);
        await executeOperation(op);
    }
    
    jj.addToTrajectory();
    
    // Record outcome
    const success = await verifyDeployment();
    jj.finalizeTrajectory(
        success ? 0.95 : 0.5,
        success ? 'Deployment successful' : 'Issues detected'
    );
}
```

### Use Case 2: Multi-Agent Code Review

Coordinate review across multiple agents:

```javascript
async function coordinatedReview(agents) {
    const reviews = await Promise.all(agents.map(async (agent) => {
        const jj = new JjWrapper();
        
        // Start review trajectory
        jj.startTrajectory(`Review by ${agent.name}`);
        
        // Get AI suggestion for review approach
        const suggestion = JSON.parse(jj.getSuggestion('Code review'));
        
        // Perform review
        const diff = await jj.diff('@', '@-');
        const issues = await agent.analyze(diff);
        
        jj.addToTrajectory();
        jj.finalizeTrajectory(
            issues.length === 0 ? 0.9 : 0.6,
            `Found ${issues.length} issues`
        );
        
        return { agent: agent.name, issues, suggestion };
    }));
    
    // Aggregate learning from all agents
    return reviews;
}
```

### Use Case 3: Error Pattern Detection

Learn from failures to prevent future issues:

```javascript
async function smartMerge(jj, branch) {
    // Query similar merge attempts
    const similar = JSON.parse(jj.queryTrajectories(`merge ${branch}`, 10));
    
    // Analyze past failures
    const failures = similar.filter(t => t.successScore < 0.5);
    
    if (failures.length > 0) {
        console.log('⚠️ Similar merges failed in the past:');
        failures.forEach(f => {
            if (f.critique) {
                console.log(`  - ${f.critique}`);
            }
        });
    }
    
    // Get AI recommendation
    const suggestion = JSON.parse(jj.getSuggestion(`merge ${branch}`));
    
    if (suggestion.confidence < 0.7) {
        console.log('⚠️ Low confidence. Recommended steps:');
        suggestion.recommendedOperations.forEach(op => console.log(`  - ${op}`));
    }
    
    // Execute merge with tracking
    jj.startTrajectory(`Merge ${branch}`);
    try {
        await jj.execute(['merge', branch]);
        jj.addToTrajectory();
        jj.finalizeTrajectory(0.9, 'Merge successful');
    } catch (err) {
        jj.addToTrajectory();
        jj.finalizeTrajectory(0.3, `Merge failed: ${err.message}`);
        throw err;
    }
}
```

### Use Case 4: Continuous Learning Loop

Implement a self-improving agent:

```javascript
class SelfImprovingAgent {
    constructor() {
        this.jj = new JjWrapper();
    }
    
    async performTask(taskDescription) {
        // Get AI suggestion
        const suggestion = JSON.parse(this.jj.getSuggestion(taskDescription));
        
        console.log(`Task: ${taskDescription}`);
        console.log(`AI Confidence: ${(suggestion.confidence * 100).toFixed(1)}%`);
        console.log(`Expected Success: ${(suggestion.expectedSuccessRate * 100).toFixed(1)}%`);
        
        // Start trajectory
        this.jj.startTrajectory(taskDescription);
        
        // Execute with recommended approach
        const startTime = Date.now();
        let success = false;
        
        try {
            for (const op of suggestion.recommendedOperations) {
                await this.execute(op);
            }
            success = true;
        } catch (err) {
            console.error('Task failed:', err.message);
        }
        
        const duration = Date.now() - startTime;
        
        // Record learning
        this.jj.addToTrajectory();
        this.jj.finalizeTrajectory(
            success ? 0.9 : 0.4,
            success 
                ? `Completed in ${duration}ms using ${suggestion.recommendedOperations.length} operations`
                : `Failed after ${duration}ms`
        );
        
        // Check improvement
        const stats = JSON.parse(this.jj.getLearningStats());
        console.log(`Improvement rate: ${(stats.improvementRate * 100).toFixed(1)}%`);
        
        return success;
    }
    
    async execute(operation) {
        // Execute operation logic
    }
}

// Usage
const agent = new SelfImprovingAgent();

// Agent improves over time
for (let i = 1; i <= 10; i++) {
    console.log(`\n--- Attempt ${i} ---`);
    await agent.performTask('Deploy application');
}
```

## API Reference

### Core Methods

| Method | Description | Returns |
|--------|-------------|---------|
| `new JjWrapper()` | Create wrapper instance | JjWrapper |
| `status()` | Get repository status | Promise<JjResult> |
| `newCommit(msg)` | Create new commit | Promise<JjResult> |
| `log(limit)` | Show commit history | Promise<JjCommit[]> |
| `diff(from, to)` | Show differences | Promise<JjDiff> |
| `branchCreate(name, rev?)` | Create branch | Promise<JjResult> |
| `rebase(source, dest)` | Rebase commits | Promise<JjResult> |

### ReasoningBank Methods

| Method | Description | Returns |
|--------|-------------|---------|
| `startTrajectory(task)` | Begin learning trajectory | string (trajectory ID) |
| `addToTrajectory()` | Add recent operations | void |
| `finalizeTrajectory(score, critique?)` | Complete trajectory (score: 0.0-1.0) | void |
| `getSuggestion(task)` | Get AI recommendation | JSON: DecisionSuggestion |
| `getLearningStats()` | Get learning metrics | JSON: LearningStats |
| `getPatterns()` | Get discovered patterns | JSON: Pattern[] |
| `queryTrajectories(task, limit)` | Find similar trajectories | JSON: Trajectory[] |
| `resetLearning()` | Clear learned data | void |

### AgentDB Methods

| Method | Description | Returns |
|--------|-------------|---------|
| `getStats()` | Get operation statistics | JSON: Stats |
| `getOperations(limit)` | Get recent operations | JjOperation[] |
| `getUserOperations(limit)` | Get user operations only | JjOperation[] |
| `clearLog()` | Clear operation log | void |

### Quantum Security Methods (v2.3.0+)

| Method | Description | Returns |
|--------|-------------|---------|
| `generateQuantumFingerprint(data)` | Generate SHA3-512 fingerprint | Buffer (64 bytes) |
| `verifyQuantumFingerprint(data, fp)` | Verify fingerprint | boolean |
| `enableEncryption(key, pubKey?)` | Enable HQC-128 encryption | void |
| `disableEncryption()` | Disable encryption | void |
| `isEncryptionEnabled()` | Check encryption status | boolean |

## Performance Characteristics

| Metric | Git | Agentic Jujutsu |
|--------|-----|-----------------|
| Concurrent commits | 15 ops/s | 350 ops/s (23x) |
| Context switching | 500-1000ms | 50-100ms (10x) |
| Conflict resolution | 30-40% auto | 87% auto (2.5x) |
| Lock waiting | 50 min/day | 0 min (∞) |
| Quantum fingerprints | N/A | <1ms |

## Best Practices

### 1. Trajectory Management

```javascript
// ✅ Good: Meaningful task descriptions
jj.startTrajectory('Implement user authentication with JWT');

// ❌ Bad: Vague descriptions
jj.startTrajectory('fix stuff');

// ✅ Good: Honest success scores
jj.finalizeTrajectory(0.7, 'Works but needs refactoring');

// ❌ Bad: Always 1.0
jj.finalizeTrajectory(1.0, 'Perfect!'); // Prevents learning
```

### 2. Pattern Recognition

```javascript
// ✅ Good: Let patterns emerge naturally
for (let i = 0; i < 10; i++) {
    jj.startTrajectory('Deploy feature');
    await deploy();
    jj.addToTrajectory();
    jj.finalizeTrajectory(wasSuccessful ? 0.9 : 0.5);
}

// ❌ Bad: Not recording outcomes
await deploy(); // No learning
```

### 3. Multi-Agent Coordination

```javascript
// ✅ Good: Concurrent operations
const agents = ['agent1', 'agent2', 'agent3'];
await Promise.all(agents.map(async (agent) => {
    const jj = new JjWrapper();
    // Each agent works independently
    await jj.newCommit(`Changes by ${agent}`);
}));

// ❌ Bad: Sequential with locks
for (const agent of agents) {
    await agent.waitForLock(); // Not needed!
    await agent.commit();
}
```

### 4. Error Handling

```javascript
// ✅ Good: Record failures with details
try {
    await jj.execute(['complex-operation']);
    jj.finalizeTrajectory(0.9);
} catch (err) {
    jj.finalizeTrajectory(0.3, `Failed: ${err.message}. Root cause: ...`);
}

// ❌ Bad: Silent failures
try {
    await jj.execute(['operation']);
} catch (err) {
    // No learning from failure
}
```

## Validation Rules (v2.3.1+)

### Task Description
- ✅ Cannot be empty or whitespace-only
- ✅ Maximum length: 10,000 bytes
- ✅ Automatically trimmed

### Success Score
- ✅ Must be finite (not NaN or Infinity)
- ✅ Must be between 0.0 and 1.0 (inclusive)

### Operations
- ✅ Must have at least one operation before finalizing

### Context
- ✅ Cannot be empty
- ✅ Keys cannot be empty or whitespace-only
- ✅ Keys max 1,000 bytes, values max 10,000 bytes

## Troubleshooting

### Issue: Low Confidence Suggestions

```javascript
const suggestion = JSON.parse(jj.getSuggestion('new task'));

if (suggestion.confidence < 0.5) {
    // Not enough data - check learning stats
    const stats = JSON.parse(jj.getLearningStats());
    console.log(`Need more data. Current trajectories: ${stats.totalTrajectories}`);
    
    // Recommend: Record 5-10 trajectories first
}
```

### Issue: Validation Errors

```javascript
try {
    jj.startTrajectory(''); // Empty task
} catch (err) {
    if (err.message.includes('Validation error')) {
        console.log('Invalid input:', err.message);
        // Use non-empty, meaningful task description
    }
}

try {
    jj.finalizeTrajectory(1.5); // Score > 1.0
} catch (err) {
    // Use score between 0.0 and 1.0
    jj.finalizeTrajectory(Math.max(0, Math.min(1, score)));
}
```

### Issue: No Patterns Discovered

```javascript
const patterns = JSON.parse(jj.getPatterns());

if (patterns.length === 0) {
    // Need more trajectories with >70% success
    // Record at least 3-5 successful trajectories
}
```

## Examples

### Example 1: Simple Learning Workflow

```javascript
const { JjWrapper } = require('agentic-jujutsu');

async function learnFromWork() {
    const jj = new JjWrapper();
    
    // Start tracking
    jj.startTrajectory('Add user profile feature');
    
    // Do work
    await jj.branchCreate('feature/user-profile');
    await jj.newCommit('Add user profile model');
    await jj.newCommit('Add profile API endpoints');
    await jj.newCommit('Add profile UI');
    
    // Record operations
    jj.addToTrajectory();
    
    // Finalize with result
    jj.finalizeTrajectory(0.85, 'Feature complete, minor styling issues remain');
    
    // Next time, get suggestions
    const suggestion = JSON.parse(jj.getSuggestion('Add settings page'));
    console.log('AI suggests:', suggestion.reasoning);
}
```

### Example 2: Multi-Agent Swarm

```javascript
async function agentSwarm(taskList) {
    const agents = taskList.map((task, i) => ({
        name: `agent-${i}`,
        jj: new JjWrapper(),
        task
    }));
    
    // All agents work concurrently (no conflicts!)
    const results = await Promise.all(agents.map(async (agent) => {
        agent.jj.startTrajectory(agent.task);
        
        // Get AI suggestion
        const suggestion = JSON.parse(agent.jj.getSuggestion(agent.task));
        
        // Execute task
        const success = await executeTask(agent, suggestion);
        
        agent.jj.addToTrajectory();
        agent.jj.finalizeTrajectory(success ? 0.9 : 0.5);
        
        return { agent: agent.name, success };
    }));
    
    console.log('Results:', results);
}
```

## Related Documentation

- **NPM Package**: https://npmjs.com/package/agentic-jujutsu
- **GitHub**: https://github.com/ruvnet/agentic-flow/tree/main/packages/agentic-jujutsu
- **Full README**: See package README.md
- **Validation Guide**: docs/VALIDATION_FIXES_v2.3.1.md
- **AgentDB Guide**: docs/AGENTDB_GUIDE.md

## Version History

- **v2.3.2** - Documentation updates
- **v2.3.1** - Validation fixes for ReasoningBank
- **v2.3.0** - Quantum-resistant security with @qudag/napi-core
- **v2.1.0** - Self-learning AI with ReasoningBank
- **v2.0.0** - Zero-dependency installation with embedded jj binary

---

**Status**: ✅ Production Ready
**License**: MIT
**Maintained**: Active
