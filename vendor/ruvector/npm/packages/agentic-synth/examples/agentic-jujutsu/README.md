# Agentic-Jujutsu Integration Examples

This directory contains comprehensive examples demonstrating the integration of **agentic-jujutsu** (quantum-resistant, self-learning version control) with **agentic-synth** (synthetic data generation).

## 🎯 Overview

Agentic-jujutsu brings advanced version control capabilities to synthetic data generation:

- **Version Control**: Track data generation history with full provenance
- **Multi-Agent Coordination**: Multiple agents generating different data types
- **ReasoningBank Intelligence**: Self-learning and adaptive generation
- **Quantum-Resistant Security**: Cryptographic integrity and immutable history
- **Collaborative Workflows**: Team-based data generation with review processes

## 📋 Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Examples](#examples)
  - [Version Control Integration](#1-version-control-integration)
  - [Multi-Agent Data Generation](#2-multi-agent-data-generation)
  - [ReasoningBank Learning](#3-reasoningbank-learning)
  - [Quantum-Resistant Data](#4-quantum-resistant-data)
  - [Collaborative Workflows](#5-collaborative-workflows)
- [Testing](#testing)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)
- [API Reference](#api-reference)

## 🚀 Installation

### Prerequisites

- Node.js 18+ or Bun runtime
- Git (for jujutsu compatibility)
- Agentic-synth installed

### Install Agentic-Jujutsu

```bash
# Install globally for CLI access
npm install -g agentic-jujutsu@latest

# Or use via npx (no installation required)
npx agentic-jujutsu@latest --version
```

### Install Dependencies

```bash
cd packages/agentic-synth
npm install
```

## ⚡ Quick Start

### Basic Version-Controlled Data Generation

```typescript
import { VersionControlledDataGenerator } from './examples/agentic-jujutsu/version-control-integration';

const generator = new VersionControlledDataGenerator('./my-data-repo');

// Initialize repository
await generator.initializeRepository();

// Generate and commit data
const schema = {
  name: 'string',
  email: 'email',
  age: 'number'
};

const commit = await generator.generateAndCommit(
  schema,
  1000,
  'Initial user dataset'
);

console.log(`Generated ${commit.metadata.recordCount} records`);
console.log(`Quality: ${(commit.metadata.quality * 100).toFixed(1)}%`);
```

### Running with npx

```bash
# Initialize a jujutsu repository
npx agentic-jujutsu@latest init

# Check status
npx agentic-jujutsu@latest status

# View history
npx agentic-jujutsu@latest log

# Create branches for experimentation
npx agentic-jujutsu@latest branch create experiment-1
```

## 📚 Examples

### 1. Version Control Integration

**File**: `version-control-integration.ts`

Demonstrates version controlling synthetic data with branching, merging, and rollback capabilities.

**Key Features**:
- Repository initialization
- Data generation with metadata tracking
- Branch management for different strategies
- Dataset comparison between versions
- Rollback to previous generations
- Version tagging

**Run Example**:
```bash
npx tsx examples/agentic-jujutsu/version-control-integration.ts
```

**Key Commands**:
```typescript
// Initialize repository
await generator.initializeRepository();

// Generate and commit
const commit = await generator.generateAndCommit(schema, 1000, 'Message');

// Create experimental branch
await generator.createGenerationBranch('experiment-1', 'Testing new approach');

// Compare datasets
const comparison = await generator.compareDatasets(commit1.hash, commit2.hash);

// Tag stable version
await generator.tagVersion('v1.0', 'Production baseline');

// Rollback if needed
await generator.rollbackToVersion(previousCommit);
```

**Real-World Use Cases**:
- A/B testing different generation strategies
- Maintaining production vs. experimental datasets
- Rolling back to known-good generations
- Tracking data quality over time

---

### 2. Multi-Agent Data Generation

**File**: `multi-agent-data-generation.ts`

Coordinates multiple agents generating different types of synthetic data with automatic conflict resolution.

**Key Features**:
- Agent registration with dedicated branches
- Parallel data generation
- Contribution merging (sequential/octopus)
- Conflict detection and resolution
- Agent synchronization
- Activity tracking

**Run Example**:
```bash
npx tsx examples/agentic-jujutsu/multi-agent-data-generation.ts
```

**Key Commands**:
```typescript
// Initialize multi-agent environment
await coordinator.initialize();

// Register agents
const userAgent = await coordinator.registerAgent(
  'agent-001',
  'User Generator',
  'users',
  { name: 'string', email: 'email' }
);

// Parallel generation
const contributions = await coordinator.coordinateParallelGeneration([
  { agentId: 'agent-001', count: 1000, description: 'Users' },
  { agentId: 'agent-002', count: 500, description: 'Products' }
]);

// Merge contributions
await coordinator.mergeContributions(['agent-001', 'agent-002']);

// Synchronize agents
await coordinator.synchronizeAgents();
```

**Real-World Use Cases**:
- Large-scale data generation with specialized agents
- Distributed team generating different data types
- Parallel processing for faster generation
- Coordinating microservices generating test data

---

### 3. ReasoningBank Learning

**File**: `reasoning-bank-learning.ts`

Self-learning data generation that improves quality over time using ReasoningBank intelligence.

**Key Features**:
- Trajectory tracking for each generation
- Pattern recognition from successful generations
- Adaptive schema evolution
- Continuous quality improvement
- Memory distillation
- Self-optimization

**Run Example**:
```bash
npx tsx examples/agentic-jujutsu/reasoning-bank-learning.ts
```

**Key Commands**:
```typescript
// Initialize ReasoningBank
await generator.initialize();

// Generate with learning
const { data, trajectory } = await generator.generateWithLearning(
  schema,
  { count: 1000 },
  'Learning generation'
);

console.log(`Quality: ${trajectory.quality}`);
console.log(`Lessons learned: ${trajectory.lessons.length}`);

// Evolve schema based on learning
const evolved = await generator.evolveSchema(schema, 0.95, 10);

// Continuous improvement
const improvement = await generator.continuousImprovement(5);
console.log(`Quality improved by ${improvement.qualityImprovement}%`);

// Recognize patterns
const patterns = await generator.recognizePatterns();
```

**Real-World Use Cases**:
- Optimizing data quality automatically
- Learning from production feedback
- Adapting schemas to new requirements
- Self-improving test data generation

---

### 4. Quantum-Resistant Data

**File**: `quantum-resistant-data.ts`

Secure data generation with cryptographic signatures and quantum-resistant integrity verification.

**Key Features**:
- Quantum-resistant key generation
- Cryptographic data signing
- Integrity verification
- Merkle tree proofs
- Audit trail generation
- Tampering detection

**Run Example**:
```bash
npx tsx examples/agentic-jujutsu/quantum-resistant-data.ts
```

**Key Commands**:
```typescript
// Initialize quantum-resistant repo
await generator.initialize();

// Generate secure data
const generation = await generator.generateSecureData(
  schema,
  1000,
  'Secure generation'
);

console.log(`Hash: ${generation.dataHash}`);
console.log(`Signature: ${generation.signature}`);

// Verify integrity
const verified = await generator.verifyIntegrity(generation.id);

// Create proof
const proof = await generator.createIntegrityProof(generation.id);

// Generate audit trail
const audit = await generator.generateAuditTrail(generation.id);

// Detect tampering
const tampered = await generator.detectTampering();
```

**Real-World Use Cases**:
- Financial data generation with audit requirements
- Healthcare data with HIPAA compliance
- Blockchain and cryptocurrency test data
- Secure supply chain data
- Regulated industry compliance

---

### 5. Collaborative Workflows

**File**: `collaborative-workflows.ts`

Team-based data generation with review processes, quality gates, and approval workflows.

**Key Features**:
- Team creation with permissions
- Team-specific workspaces
- Review request system
- Quality gate automation
- Comment and approval system
- Collaborative schema design
- Team statistics and reporting

**Run Example**:
```bash
npx tsx examples/agentic-jujutsu/collaborative-workflows.ts
```

**Key Commands**:
```typescript
// Initialize workspace
await workflow.initialize();

// Create teams
const dataTeam = await workflow.createTeam(
  'data-team',
  'Data Engineering',
  ['alice', 'bob', 'charlie']
);

// Team generates data
await workflow.teamGenerate(
  'data-team',
  'alice',
  schema,
  1000,
  'User dataset'
);

// Create review request
const review = await workflow.createReviewRequest(
  'data-team',
  'alice',
  'Add user dataset',
  'Generated 1000 users',
  ['dave', 'eve']
);

// Add comments
await workflow.addComment(review.id, 'dave', 'Looks good!');

// Approve and merge
await workflow.approveReview(review.id, 'dave');
await workflow.mergeReview(review.id);

// Design collaborative schema
await workflow.designCollaborativeSchema(
  'user-schema',
  ['alice', 'dave'],
  baseSchema
);
```

**Real-World Use Cases**:
- Enterprise data generation with governance
- Multi-team development environments
- Quality assurance workflows
- Production data approval processes
- Regulated data generation pipelines

---

## 🧪 Testing

### Run the Comprehensive Test Suite

```bash
# Run all tests
npm test examples/agentic-jujutsu/test-suite.ts

# Run with coverage
npm run test:coverage examples/agentic-jujutsu/test-suite.ts

# Run specific test suite
npm test examples/agentic-jujutsu/test-suite.ts -t "Version Control"
```

### Test Categories

The test suite includes:

1. **Version Control Integration Tests**
   - Repository initialization
   - Data generation and commits
   - Branch management
   - Dataset comparison
   - History retrieval

2. **Multi-Agent Coordination Tests**
   - Agent registration
   - Parallel generation
   - Contribution merging
   - Activity tracking

3. **ReasoningBank Learning Tests**
   - Learning-enabled generation
   - Pattern recognition
   - Schema evolution
   - Continuous improvement

4. **Quantum-Resistant Tests**
   - Secure data generation
   - Integrity verification
   - Proof creation and validation
   - Audit trail generation
   - Tampering detection

5. **Collaborative Workflow Tests**
   - Team creation
   - Review requests
   - Quality gates
   - Schema collaboration

6. **Performance Benchmarks**
   - Operation timing
   - Scalability tests
   - Resource usage

7. **Error Handling Tests**
   - Invalid inputs
   - Edge cases
   - Graceful failures

## 📖 Best Practices

### 1. Repository Organization

```
my-data-repo/
├── .jj/                    # Jujutsu metadata
├── data/
│   ├── users/             # Organized by type
│   ├── products/
│   └── transactions/
├── schemas/
│   └── shared/            # Collaborative schemas
└── reviews/               # Review requests
```

### 2. Commit Messages

Use descriptive commit messages with metadata:

```typescript
await generator.generateAndCommit(
  schema,
  count,
  `Generate ${count} records for ${purpose}

Quality: ${quality}
Schema: ${schemaVersion}
Generator: ${generatorName}`
);
```

### 3. Branch Naming

Follow consistent branch naming:

- `agent/{agent-id}/{data-type}` - Agent branches
- `team/{team-id}/{team-name}` - Team branches
- `experiment/{description}` - Experimental branches
- `schema/{schema-name}` - Schema design branches

### 4. Quality Gates

Always define quality gates for production:

```typescript
const qualityGates = [
  { name: 'Data Completeness', required: true },
  { name: 'Schema Validation', required: true },
  { name: 'Quality Threshold', required: true },
  { name: 'Security Scan', required: false }
];
```

### 5. Security

For sensitive data:

- Always use quantum-resistant features
- Enable integrity verification
- Generate audit trails
- Regular tampering scans
- Secure key management

### 6. Learning Optimization

Maximize ReasoningBank benefits:

- Track all generations as trajectories
- Regularly recognize patterns
- Use adaptive schema evolution
- Implement continuous improvement
- Analyze quality trends

## 🔧 Troubleshooting

### Common Issues

#### 1. Jujutsu Not Found

```bash
# Error: jujutsu command not found

# Solution: Install jujutsu
npm install -g agentic-jujutsu@latest

# Or use npx
npx agentic-jujutsu@latest init
```

#### 2. Merge Conflicts

```bash
# Error: Merge conflicts detected

# Solution: Use conflict resolution
await coordinator.resolveConflicts(conflictFiles, 'ours');
# or
await coordinator.resolveConflicts(conflictFiles, 'theirs');
```

#### 3. Integrity Verification Failed

```typescript
// Error: Signature verification failed

// Solution: Check keys and regenerate if needed
await generator.initialize(); // Regenerates keys
const verified = await generator.verifyIntegrity(generationId);
```

#### 4. Quality Gates Failing

```typescript
// Error: Quality gate threshold not met

// Solution: Use adaptive learning to improve
const evolved = await generator.evolveSchema(schema, targetQuality);
```

#### 5. Permission Denied

```bash
# Error: Permission denied on team operations

# Solution: Verify team membership
const team = await workflow.teams.get(teamId);
if (!team.members.includes(author)) {
  // Add member to team
  team.members.push(author);
}
```

### Debug Mode

Enable debug logging:

```typescript
// Set environment variable
process.env.DEBUG = 'agentic-jujutsu:*';

// Or enable in code
import { setLogLevel } from 'agentic-synth';
setLogLevel('debug');
```

## 📚 API Reference

### VersionControlledDataGenerator

```typescript
class VersionControlledDataGenerator {
  constructor(repoPath: string);

  async initializeRepository(): Promise<void>;
  async generateAndCommit(schema: any, count: number, message: string): Promise<JujutsuCommit>;
  async createGenerationBranch(branchName: string, description: string): Promise<void>;
  async compareDatasets(ref1: string, ref2: string): Promise<any>;
  async mergeBranches(source: string, target: string): Promise<void>;
  async rollbackToVersion(commitHash: string): Promise<void>;
  async getHistory(limit?: number): Promise<any[]>;
  async tagVersion(tag: string, message: string): Promise<void>;
}
```

### MultiAgentDataCoordinator

```typescript
class MultiAgentDataCoordinator {
  constructor(repoPath: string);

  async initialize(): Promise<void>;
  async registerAgent(id: string, name: string, dataType: string, schema: any): Promise<Agent>;
  async agentGenerate(agentId: string, count: number, description: string): Promise<AgentContribution>;
  async coordinateParallelGeneration(tasks: Task[]): Promise<AgentContribution[]>;
  async mergeContributions(agentIds: string[], strategy?: 'sequential' | 'octopus'): Promise<any>;
  async resolveConflicts(files: string[], strategy: 'ours' | 'theirs' | 'manual'): Promise<void>;
  async synchronizeAgents(agentIds?: string[]): Promise<void>;
  async getAgentActivity(agentId: string): Promise<any>;
}
```

### ReasoningBankDataGenerator

```typescript
class ReasoningBankDataGenerator {
  constructor(repoPath: string);

  async initialize(): Promise<void>;
  async generateWithLearning(schema: any, parameters: any, description: string): Promise<{ data: any[]; trajectory: GenerationTrajectory }>;
  async evolveSchema(baseSchema: any, targetQuality?: number, maxGenerations?: number): Promise<AdaptiveSchema>;
  async recognizePatterns(): Promise<LearningPattern[]>;
  async continuousImprovement(iterations?: number): Promise<any>;
}
```

### QuantumResistantDataGenerator

```typescript
class QuantumResistantDataGenerator {
  constructor(repoPath: string);

  async initialize(): Promise<void>;
  async generateSecureData(schema: any, count: number, description: string): Promise<SecureDataGeneration>;
  async verifyIntegrity(generationId: string): Promise<boolean>;
  async createIntegrityProof(generationId: string): Promise<IntegrityProof>;
  async verifyIntegrityProof(generationId: string): Promise<boolean>;
  async generateAuditTrail(generationId: string): Promise<AuditTrail>;
  async detectTampering(): Promise<string[]>;
}
```

### CollaborativeDataWorkflow

```typescript
class CollaborativeDataWorkflow {
  constructor(repoPath: string);

  async initialize(): Promise<void>;
  async createTeam(id: string, name: string, members: string[], permissions?: string[]): Promise<Team>;
  async teamGenerate(teamId: string, author: string, schema: any, count: number, description: string): Promise<Contribution>;
  async createReviewRequest(teamId: string, author: string, title: string, description: string, reviewers: string[]): Promise<ReviewRequest>;
  async addComment(requestId: string, author: string, text: string): Promise<void>;
  async approveReview(requestId: string, reviewer: string): Promise<void>;
  async mergeReview(requestId: string): Promise<void>;
  async designCollaborativeSchema(name: string, contributors: string[], baseSchema: any): Promise<any>;
  async getTeamStatistics(teamId: string): Promise<any>;
}
```

## 🔗 Related Resources

- [Agentic-Jujutsu Repository](https://github.com/ruvnet/agentic-jujutsu)
- [Agentic-Synth Documentation](../../README.md)
- [Jujutsu VCS Documentation](https://github.com/martinvonz/jj)
- [ReasoningBank Paper](https://arxiv.org/abs/example)

## 🤝 Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Add tests for new features
4. Submit a pull request

## 📄 License

MIT License - see LICENSE file for details

## 💬 Support

- Issues: [GitHub Issues](https://github.com/ruvnet/ruvector/issues)
- Discussions: [GitHub Discussions](https://github.com/ruvnet/ruvector/discussions)
- Email: support@ruv.io

---

**Built with ❤️ by the RUV Team**
