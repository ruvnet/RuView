# Agentic-Jujutsu Examples

This directory contains comprehensive examples demonstrating the capabilities of agentic-jujutsu, a quantum-resistant, self-learning version control system designed for AI agents.

## Examples Overview

### 1. Basic Usage (`basic-usage.ts`)
Fundamental operations for getting started:
- Repository status checks
- Creating commits
- Branch management
- Viewing commit history and diffs

**Run:** `npx ts-node basic-usage.ts`

### 2. Learning Workflow (`learning-workflow.ts`)
Demonstrates ReasoningBank self-learning capabilities:
- Starting and tracking learning trajectories
- Recording operations and outcomes
- Getting AI-powered suggestions
- Viewing learning statistics and discovered patterns

**Run:** `npx ts-node learning-workflow.ts`

### 3. Multi-Agent Coordination (`multi-agent-coordination.ts`)
Shows how multiple AI agents work simultaneously:
- Concurrent commits without locks (23x faster than Git)
- Shared learning across agents
- Collaborative code review workflows
- Conflict-free coordination

**Run:** `npx ts-node multi-agent-coordination.ts`

### 4. Quantum Security (`quantum-security.ts`)
Demonstrates quantum-resistant security features:
- SHA3-512 quantum fingerprints (<1ms)
- HQC-128 encryption
- Data integrity verification
- Secure trajectory storage

**Run:** `npx ts-node quantum-security.ts`

## Key Features Demonstrated

### Performance Benefits
- **23x faster** concurrent commits (350 ops/s vs Git's 15 ops/s)
- **10x faster** context switching (<100ms vs Git's 500-1000ms)
- **87% automatic** conflict resolution
- **Zero** lock waiting time

### Self-Learning Capabilities
- Trajectory tracking for continuous improvement
- Pattern discovery from successful operations
- AI-powered suggestions with confidence scores
- Learning statistics and improvement metrics

### Quantum-Resistant Security
- SHA3-512 fingerprints (NIST FIPS 202)
- HQC-128 post-quantum encryption
- <1ms verification performance
- Future-proof against quantum computers

### Multi-Agent Features
- Lock-free concurrent operations
- Shared learning between agents
- Collaborative workflows
- Cross-agent pattern recognition

## Prerequisites

```bash
# Install agentic-jujutsu
npm install agentic-jujutsu

# Or run directly
npx agentic-jujutsu
```

## Running the Examples

### Individual Examples
```bash
# Basic usage
npx ts-node examples/agentic-jujutsu/basic-usage.ts

# Learning workflow
npx ts-node examples/agentic-jujutsu/learning-workflow.ts

# Multi-agent coordination
npx ts-node examples/agentic-jujutsu/multi-agent-coordination.ts

# Quantum security
npx ts-node examples/agentic-jujutsu/quantum-security.ts
```

### Run All Examples
```bash
cd examples/agentic-jujutsu
for file in *.ts; do
  echo "Running $file..."
  npx ts-node "$file"
  echo ""
done
```

## Testing

Comprehensive test suites are available in `/tests/agentic-jujutsu/`:

```bash
# Run all tests
./tests/agentic-jujutsu/run-all-tests.sh

# Run with coverage
./tests/agentic-jujutsu/run-all-tests.sh --coverage

# Run with verbose output
./tests/agentic-jujutsu/run-all-tests.sh --verbose

# Stop on first failure
./tests/agentic-jujutsu/run-all-tests.sh --bail
```

## Integration with Ruvector

Agentic-jujutsu can be integrated with Ruvector for:
- Versioning vector embeddings
- Tracking AI model experiments
- Managing agent memory evolution
- Collaborative AI development

Example integration:
```typescript
import { VectorDB } from 'ruvector';
import { JjWrapper } from 'agentic-jujutsu';

const db = new VectorDB();
const jj = new JjWrapper();

// Track vector database changes
jj.startTrajectory('Update embeddings');
await db.insert('doc1', [0.1, 0.2, 0.3]);
await jj.newCommit('Add new embeddings');
jj.addToTrajectory();
jj.finalizeTrajectory(0.9, 'Embeddings updated successfully');
```

## Best Practices

### 1. Trajectory Management
- Use meaningful task descriptions
- Record honest success scores (0.0-1.0)
- Always finalize trajectories
- Add detailed critiques for learning

### 2. Multi-Agent Coordination
- Let agents work concurrently (no manual locks)
- Share learning through trajectories
- Use suggestions for informed decisions
- Monitor improvement rates

### 3. Security
- Enable encryption for sensitive operations
- Verify fingerprints regularly
- Use quantum-resistant features for long-term data
- Keep encryption keys secure

### 4. Performance
- Batch operations when possible
- Use async operations for I/O
- Monitor operation statistics
- Optimize based on learning patterns

## Documentation

For complete API documentation and guides:
- **Skill Documentation**: `.claude/skills/agentic-jujutsu/SKILL.md`
- **NPM Package**: https://npmjs.com/package/agentic-jujutsu
- **GitHub**: https://github.com/ruvnet/agentic-flow/tree/main/packages/agentic-jujutsu

## Version

Examples compatible with agentic-jujutsu v2.3.2+

## License

MIT License - See project LICENSE file
