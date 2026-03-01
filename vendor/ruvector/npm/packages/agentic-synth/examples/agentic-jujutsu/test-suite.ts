/**
 * Comprehensive Test Suite for Agentic-Jujutsu Integration
 *
 * Tests all features of agentic-jujutsu integration with agentic-synth:
 * - Version control
 * - Multi-agent coordination
 * - ReasoningBank learning
 * - Quantum-resistant features
 * - Collaborative workflows
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';
import { execSync } from 'child_process';
import { VersionControlledDataGenerator } from './version-control-integration';
import { MultiAgentDataCoordinator } from './multi-agent-data-generation';
import { ReasoningBankDataGenerator } from './reasoning-bank-learning';
import { QuantumResistantDataGenerator } from './quantum-resistant-data';
import { CollaborativeDataWorkflow } from './collaborative-workflows';

const TEST_ROOT = path.join(process.cwd(), 'test-repos');

// Test utilities
function cleanupTestRepos() {
  if (fs.existsSync(TEST_ROOT)) {
    fs.rmSync(TEST_ROOT, { recursive: true, force: true });
  }
}

function createTestRepo(name: string): string {
  const repoPath = path.join(TEST_ROOT, name);
  fs.mkdirSync(repoPath, { recursive: true });
  return repoPath;
}

describe('Version Control Integration', () => {
  let repoPath: string;
  let generator: VersionControlledDataGenerator;

  beforeAll(() => {
    cleanupTestRepos();
    repoPath = createTestRepo('version-control-test');
    generator = new VersionControlledDataGenerator(repoPath);
  });

  afterAll(() => {
    cleanupTestRepos();
  });

  it('should initialize jujutsu repository', async () => {
    await generator.initializeRepository();
    expect(fs.existsSync(path.join(repoPath, '.jj'))).toBe(true);
    expect(fs.existsSync(path.join(repoPath, 'data'))).toBe(true);
  });

  it('should generate and commit data with metadata', async () => {
    const schema = {
      name: 'string',
      email: 'email',
      age: 'number'
    };

    const commit = await generator.generateAndCommit(
      schema,
      100,
      'Test data generation'
    );

    expect(commit).toBeDefined();
    expect(commit.hash).toBeTruthy();
    expect(commit.metadata.recordCount).toBe(100);
    expect(commit.metadata.quality).toBeGreaterThan(0);
  });

  it('should create and manage branches', async () => {
    await generator.createGenerationBranch(
      'experiment-1',
      'Testing branch creation'
    );

    const branchFile = path.join(repoPath, '.jj', 'branches', 'experiment-1.desc');
    expect(fs.existsSync(branchFile)).toBe(true);
  });

  it('should compare datasets between commits', async () => {
    const schema = { name: 'string', value: 'number' };

    const commit1 = await generator.generateAndCommit(schema, 50, 'Dataset 1');
    const commit2 = await generator.generateAndCommit(schema, 75, 'Dataset 2');

    const comparison = await generator.compareDatasets(commit1.hash, commit2.hash);

    expect(comparison).toBeDefined();
    expect(comparison.ref1).toBe(commit1.hash);
    expect(comparison.ref2).toBe(commit2.hash);
  });

  it('should tag versions', async () => {
    await generator.tagVersion('v1.0.0', 'First stable version');
    // Tag creation is tested by not throwing
    expect(true).toBe(true);
  });

  it('should retrieve generation history', async () => {
    const history = await generator.getHistory(5);
    expect(Array.isArray(history)).toBe(true);
    expect(history.length).toBeGreaterThan(0);
  });
});

describe('Multi-Agent Data Generation', () => {
  let repoPath: string;
  let coordinator: MultiAgentDataCoordinator;

  beforeAll(() => {
    repoPath = createTestRepo('multi-agent-test');
    coordinator = new MultiAgentDataCoordinator(repoPath);
  });

  it('should initialize multi-agent environment', async () => {
    await coordinator.initialize();
    expect(fs.existsSync(path.join(repoPath, '.jj'))).toBe(true);
    expect(fs.existsSync(path.join(repoPath, 'data', 'users'))).toBe(true);
  });

  it('should register agents', async () => {
    const agent = await coordinator.registerAgent(
      'test-agent-1',
      'Test Agent',
      'users',
      { name: 'string', email: 'email' }
    );

    expect(agent.id).toBe('test-agent-1');
    expect(agent.branch).toContain('agent/test-agent-1');
  });

  it('should generate data for specific agent', async () => {
    await coordinator.registerAgent(
      'test-agent-2',
      'Agent 2',
      'products',
      { name: 'string', price: 'number' }
    );

    const contribution = await coordinator.agentGenerate(
      'test-agent-2',
      50,
      'Test generation'
    );

    expect(contribution.agentId).toBe('test-agent-2');
    expect(contribution.recordCount).toBe(50);
    expect(contribution.quality).toBeGreaterThan(0);
  });

  it('should coordinate parallel generation', async () => {
    await coordinator.registerAgent('agent-a', 'Agent A', 'typeA', { id: 'string' });
    await coordinator.registerAgent('agent-b', 'Agent B', 'typeB', { id: 'string' });

    const contributions = await coordinator.coordinateParallelGeneration([
      { agentId: 'agent-a', count: 25, description: 'Task A' },
      { agentId: 'agent-b', count: 30, description: 'Task B' }
    ]);

    expect(contributions.length).toBe(2);
    expect(contributions[0].recordCount).toBe(25);
    expect(contributions[1].recordCount).toBe(30);
  });

  it('should get agent activity', async () => {
    const activity = await coordinator.getAgentActivity('agent-a');
    expect(activity).toBeDefined();
    expect(activity.agent).toBe('Agent A');
  });
});

describe('ReasoningBank Learning', () => {
  let repoPath: string;
  let generator: ReasoningBankDataGenerator;

  beforeAll(() => {
    repoPath = createTestRepo('reasoning-bank-test');
    generator = new ReasoningBankDataGenerator(repoPath);
  });

  it('should initialize ReasoningBank system', async () => {
    await generator.initialize();
    expect(fs.existsSync(path.join(repoPath, 'data', 'trajectories'))).toBe(true);
    expect(fs.existsSync(path.join(repoPath, 'data', 'patterns'))).toBe(true);
  });

  it('should generate with learning enabled', async () => {
    const schema = { name: 'string', value: 'number' };
    const result = await generator.generateWithLearning(
      schema,
      { count: 100 },
      'Learning test'
    );

    expect(result.data.length).toBe(100);
    expect(result.trajectory).toBeDefined();
    expect(result.trajectory.quality).toBeGreaterThan(0);
    expect(result.trajectory.verdict).toBeTruthy();
  });

  it('should recognize patterns from trajectories', async () => {
    // Generate multiple trajectories
    const schema = { id: 'string', score: 'number' };

    await generator.generateWithLearning(schema, { count: 50 }, 'Pattern test 1');
    await generator.generateWithLearning(schema, { count: 50 }, 'Pattern test 2');

    const patterns = await generator.recognizePatterns();
    expect(Array.isArray(patterns)).toBe(true);
  });

  it('should perform continuous improvement', async () => {
    const improvement = await generator.continuousImprovement(2);

    expect(improvement).toBeDefined();
    expect(improvement.iterations.length).toBe(2);
    expect(improvement.qualityTrend.length).toBe(2);
    expect(improvement.bestQuality).toBeGreaterThan(0);
  });
});

describe('Quantum-Resistant Features', () => {
  let repoPath: string;
  let generator: QuantumResistantDataGenerator;

  beforeAll(() => {
    repoPath = createTestRepo('quantum-resistant-test');
    generator = new QuantumResistantDataGenerator(repoPath);
  });

  it('should initialize quantum-resistant repository', async () => {
    await generator.initialize();
    expect(fs.existsSync(path.join(repoPath, '.jj', 'quantum-keys'))).toBe(true);
    expect(fs.existsSync(path.join(repoPath, 'data', 'secure'))).toBe(true);
  });

  it('should generate secure data with signatures', async () => {
    const schema = { userId: 'string', data: 'string' };
    const generation = await generator.generateSecureData(
      schema,
      50,
      'Secure generation test'
    );

    expect(generation.id).toBeTruthy();
    expect(generation.dataHash).toBeTruthy();
    expect(generation.signature).toBeTruthy();
    expect(generation.quantumResistant).toBe(true);
  });

  it('should verify data integrity', async () => {
    const schema = { id: 'string' };
    const generation = await generator.generateSecureData(schema, 25, 'Test');

    const verified = await generator.verifyIntegrity(generation.id);
    expect(verified).toBe(true);
  });

  it('should create integrity proofs', async () => {
    const schema = { value: 'number' };
    const generation = await generator.generateSecureData(schema, 30, 'Proof test');

    const proof = await generator.createIntegrityProof(generation.id);
    expect(proof).toBeDefined();
    expect(proof.dataHash).toBeTruthy();
    expect(proof.merkleRoot).toBeTruthy();
    expect(proof.quantumSafe).toBe(true);
  });

  it('should verify integrity proofs', async () => {
    const schema = { name: 'string' };
    const generation = await generator.generateSecureData(schema, 20, 'Verify test');

    await generator.createIntegrityProof(generation.id);
    const verified = await generator.verifyIntegrityProof(generation.id);

    expect(verified).toBe(true);
  });

  it('should generate audit trails', async () => {
    const schema = { id: 'string' };
    const generation = await generator.generateSecureData(schema, 15, 'Audit test');

    const audit = await generator.generateAuditTrail(generation.id);
    expect(audit).toBeDefined();
    expect(audit.generation).toBe(generation.id);
    expect(audit.integrityScore).toBeGreaterThanOrEqual(0);
  });

  it('should detect tampering', async () => {
    const tampered = await generator.detectTampering();
    expect(Array.isArray(tampered)).toBe(true);
    // Should be empty if no tampering
    expect(tampered.length).toBe(0);
  });
});

describe('Collaborative Workflows', () => {
  let repoPath: string;
  let workflow: CollaborativeDataWorkflow;

  beforeAll(() => {
    repoPath = createTestRepo('collaborative-test');
    workflow = new CollaborativeDataWorkflow(repoPath);
  });

  it('should initialize collaborative workspace', async () => {
    await workflow.initialize();
    expect(fs.existsSync(path.join(repoPath, 'data', 'shared'))).toBe(true);
    expect(fs.existsSync(path.join(repoPath, 'reviews'))).toBe(true);
  });

  it('should create teams', async () => {
    const team = await workflow.createTeam(
      'test-team',
      'Test Team',
      ['alice', 'bob']
    );

    expect(team.id).toBe('test-team');
    expect(team.name).toBe('Test Team');
    expect(team.members.length).toBe(2);
  });

  it('should allow team to generate data', async () => {
    await workflow.createTeam('gen-team', 'Generation Team', ['charlie']);

    const contribution = await workflow.teamGenerate(
      'gen-team',
      'charlie',
      { name: 'string', value: 'number' },
      50,
      'Team generation test'
    );

    expect(contribution.author).toBe('charlie');
    expect(contribution.team).toBe('Generation Team');
  });

  it('should create review requests', async () => {
    await workflow.createTeam('review-team', 'Review Team', ['dave']);
    await workflow.teamGenerate(
      'review-team',
      'dave',
      { id: 'string' },
      25,
      'Review test'
    );

    const review = await workflow.createReviewRequest(
      'review-team',
      'dave',
      'Test Review',
      'Testing review process',
      ['alice']
    );

    expect(review.title).toBe('Test Review');
    expect(review.status).toBe('pending');
    expect(review.qualityGates.length).toBeGreaterThan(0);
  });

  it('should add comments to reviews', async () => {
    const review = await workflow.createReviewRequest(
      'review-team',
      'dave',
      'Comment Test',
      'Testing comments',
      ['alice']
    );

    await workflow.addComment(review.id, 'alice', 'Looks good!');
    // Comment addition is tested by not throwing
    expect(true).toBe(true);
  });

  it('should design collaborative schemas', async () => {
    const schema = await workflow.designCollaborativeSchema(
      'test-schema',
      ['alice', 'bob'],
      { field1: 'string', field2: 'number' }
    );

    expect(schema.name).toBe('test-schema');
    expect(schema.contributors.length).toBe(2);
  });

  it('should get team statistics', async () => {
    const stats = await workflow.getTeamStatistics('review-team');
    expect(stats).toBeDefined();
    expect(stats.team).toBe('Review Team');
  });
});

describe('Performance Benchmarks', () => {
  it('should benchmark version control operations', async () => {
    const repoPath = createTestRepo('perf-version-control');
    const generator = new VersionControlledDataGenerator(repoPath);

    await generator.initializeRepository();

    const start = Date.now();
    const schema = { name: 'string', value: 'number' };

    for (let i = 0; i < 5; i++) {
      await generator.generateAndCommit(schema, 100, `Perf test ${i}`);
    }

    const duration = Date.now() - start;
    console.log(`Version control benchmark: 5 commits in ${duration}ms`);

    expect(duration).toBeLessThan(30000); // Should complete within 30 seconds
  });

  it('should benchmark multi-agent coordination', async () => {
    const repoPath = createTestRepo('perf-multi-agent');
    const coordinator = new MultiAgentDataCoordinator(repoPath);

    await coordinator.initialize();

    // Register agents
    for (let i = 0; i < 3; i++) {
      await coordinator.registerAgent(
        `perf-agent-${i}`,
        `Agent ${i}`,
        `type${i}`,
        { id: 'string' }
      );
    }

    const start = Date.now();
    await coordinator.coordinateParallelGeneration([
      { agentId: 'perf-agent-0', count: 100, description: 'Task 1' },
      { agentId: 'perf-agent-1', count: 100, description: 'Task 2' },
      { agentId: 'perf-agent-2', count: 100, description: 'Task 3' }
    ]);

    const duration = Date.now() - start;
    console.log(`Multi-agent benchmark: 3 agents, 300 records in ${duration}ms`);

    expect(duration).toBeLessThan(20000); // Should complete within 20 seconds
  });
});

describe('Error Handling', () => {
  it('should handle invalid repository paths', async () => {
    const generator = new VersionControlledDataGenerator('/invalid/path/that/does/not/exist');

    await expect(async () => {
      await generator.generateAndCommit({}, 10, 'Test');
    }).rejects.toThrow();
  });

  it('should handle invalid agent operations', async () => {
    const repoPath = createTestRepo('error-handling');
    const coordinator = new MultiAgentDataCoordinator(repoPath);
    await coordinator.initialize();

    await expect(async () => {
      await coordinator.agentGenerate('non-existent-agent', 10, 'Test');
    }).rejects.toThrow('not found');
  });

  it('should handle verification failures gracefully', async () => {
    const repoPath = createTestRepo('error-verification');
    const generator = new QuantumResistantDataGenerator(repoPath);
    await generator.initialize();

    const verified = await generator.verifyIntegrity('non-existent-id');
    expect(verified).toBe(false);
  });
});

// Run all tests
console.log('🧪 Running comprehensive test suite for agentic-jujutsu integration...\n');
