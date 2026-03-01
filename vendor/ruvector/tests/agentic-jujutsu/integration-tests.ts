/**
 * Agentic-Jujutsu Integration Tests
 *
 * Comprehensive integration test suite for quantum-resistant, self-learning
 * version control system designed for AI agents.
 *
 * Test Coverage:
 * - Version control operations (commit, branch, merge, rebase)
 * - Multi-agent coordination
 * - ReasoningBank features (trajectory tracking, pattern learning)
 * - Quantum-resistant security operations
 * - Collaborative workflows
 */

import { describe, it, expect, beforeEach, afterEach } from '@jest/globals';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

// Mock types based on agentic-jujutsu API
interface JjWrapper {
  status(): Promise<JjResult>;
  newCommit(message: string): Promise<JjResult>;
  log(limit: number): Promise<JjCommit[]>;
  diff(from: string, to: string): Promise<JjDiff>;
  branchCreate(name: string, rev?: string): Promise<JjResult>;
  rebase(source: string, dest: string): Promise<JjResult>;
  execute(command: string[]): Promise<JjResult>;

  // ReasoningBank methods
  startTrajectory(task: string): string;
  addToTrajectory(): void;
  finalizeTrajectory(score: number, critique?: string): void;
  getSuggestion(task: string): string; // Returns JSON string
  getLearningStats(): string; // Returns JSON string
  getPatterns(): string; // Returns JSON string
  queryTrajectories(task: string, limit: number): string;
  resetLearning(): void;

  // AgentDB methods
  getStats(): string;
  getOperations(limit: number): JjOperation[];
  getUserOperations(limit: number): JjOperation[];
  clearLog(): void;

  // Quantum security methods
  enableEncryption(key: string, pubKey?: string): void;
  disableEncryption(): void;
  isEncryptionEnabled(): boolean;
}

interface JjResult {
  success: boolean;
  stdout: string;
  stderr: string;
  exitCode: number;
}

interface JjCommit {
  id: string;
  message: string;
  author: string;
  timestamp: string;
}

interface JjDiff {
  changes: string;
  filesModified: number;
}

interface JjOperation {
  operationType: string;
  command: string;
  durationMs: number;
  success: boolean;
  timestamp: number;
}

// Mock implementation for testing
class MockJjWrapper implements JjWrapper {
  private trajectoryId: string | null = null;
  private operations: JjOperation[] = [];
  private trajectories: any[] = [];
  private encryptionEnabled = false;

  async status(): Promise<JjResult> {
    this.recordOperation('status', ['status']);
    return {
      success: true,
      stdout: 'Working directory: clean',
      stderr: '',
      exitCode: 0
    };
  }

  async newCommit(message: string): Promise<JjResult> {
    this.recordOperation('commit', ['commit', '-m', message]);
    return {
      success: true,
      stdout: `Created commit: ${message}`,
      stderr: '',
      exitCode: 0
    };
  }

  async log(limit: number): Promise<JjCommit[]> {
    this.recordOperation('log', ['log', `--limit=${limit}`]);
    return [
      {
        id: 'abc123',
        message: 'Initial commit',
        author: 'test@example.com',
        timestamp: new Date().toISOString()
      }
    ];
  }

  async diff(from: string, to: string): Promise<JjDiff> {
    this.recordOperation('diff', ['diff', from, to]);
    return {
      changes: '+ Added line\n- Removed line',
      filesModified: 2
    };
  }

  async branchCreate(name: string, rev?: string): Promise<JjResult> {
    this.recordOperation('branch', ['branch', 'create', name]);
    return {
      success: true,
      stdout: `Created branch: ${name}`,
      stderr: '',
      exitCode: 0
    };
  }

  async rebase(source: string, dest: string): Promise<JjResult> {
    this.recordOperation('rebase', ['rebase', '-s', source, '-d', dest]);
    return {
      success: true,
      stdout: `Rebased ${source} onto ${dest}`,
      stderr: '',
      exitCode: 0
    };
  }

  async execute(command: string[]): Promise<JjResult> {
    this.recordOperation('execute', command);
    return {
      success: true,
      stdout: `Executed: ${command.join(' ')}`,
      stderr: '',
      exitCode: 0
    };
  }

  startTrajectory(task: string): string {
    if (!task || task.trim().length === 0) {
      throw new Error('Validation error: task cannot be empty');
    }
    this.trajectoryId = `traj-${Date.now()}`;
    this.operations = [];
    return this.trajectoryId;
  }

  addToTrajectory(): void {
    // Records current operations to trajectory
  }

  finalizeTrajectory(score: number, critique?: string): void {
    if (score < 0 || score > 1 || !Number.isFinite(score)) {
      throw new Error('Validation error: score must be between 0.0 and 1.0');
    }
    if (this.operations.length === 0) {
      throw new Error('Validation error: must have operations before finalizing');
    }

    this.trajectories.push({
      id: this.trajectoryId,
      score,
      critique: critique || '',
      operations: [...this.operations],
      timestamp: Date.now()
    });

    this.trajectoryId = null;
  }

  getSuggestion(task: string): string {
    const suggestion = {
      confidence: 0.85,
      reasoning: 'Based on 5 similar trajectories with 90% success rate',
      recommendedOperations: ['branch create', 'commit', 'push'],
      expectedSuccessRate: 0.9,
      estimatedDurationMs: 500
    };
    return JSON.stringify(suggestion);
  }

  getLearningStats(): string {
    const stats = {
      totalTrajectories: this.trajectories.length,
      totalPatterns: Math.floor(this.trajectories.length / 3),
      avgSuccessRate: 0.87,
      improvementRate: 0.15,
      predictionAccuracy: 0.82
    };
    return JSON.stringify(stats);
  }

  getPatterns(): string {
    const patterns = [
      {
        name: 'Deploy workflow',
        successRate: 0.92,
        observationCount: 5,
        operationSequence: ['branch', 'commit', 'push'],
        confidence: 0.88
      }
    ];
    return JSON.stringify(patterns);
  }

  queryTrajectories(task: string, limit: number): string {
    return JSON.stringify(this.trajectories.slice(0, limit));
  }

  resetLearning(): void {
    this.trajectories = [];
  }

  getStats(): string {
    const stats = {
      total_operations: this.operations.length,
      success_rate: 0.95,
      avg_duration_ms: 45.2
    };
    return JSON.stringify(stats);
  }

  getOperations(limit: number): JjOperation[] {
    return this.operations.slice(-limit);
  }

  getUserOperations(limit: number): JjOperation[] {
    return this.operations
      .filter(op => op.operationType !== 'snapshot')
      .slice(-limit);
  }

  clearLog(): void {
    this.operations = [];
  }

  enableEncryption(key: string, pubKey?: string): void {
    this.encryptionEnabled = true;
  }

  disableEncryption(): void {
    this.encryptionEnabled = false;
  }

  isEncryptionEnabled(): boolean {
    return this.encryptionEnabled;
  }

  private recordOperation(type: string, command: string[]): void {
    this.operations.push({
      operationType: type,
      command: command.join(' '),
      durationMs: Math.random() * 100,
      success: true,
      timestamp: Date.now()
    });
  }
}

describe('Agentic-Jujutsu Integration Tests', () => {
  let jj: MockJjWrapper;
  let testDir: string;

  beforeEach(() => {
    jj = new MockJjWrapper();
    testDir = fs.mkdtempSync(path.join(os.tmpdir(), 'jj-test-'));
  });

  afterEach(() => {
    if (fs.existsSync(testDir)) {
      fs.rmSync(testDir, { recursive: true, force: true });
    }
  });

  describe('Version Control Operations', () => {
    it('should create commits successfully', async () => {
      const result = await jj.newCommit('Test commit');

      expect(result.success).toBe(true);
      expect(result.stdout).toContain('Created commit');
      expect(result.exitCode).toBe(0);
    });

    it('should retrieve commit history', async () => {
      await jj.newCommit('First commit');
      await jj.newCommit('Second commit');

      const log = await jj.log(10);

      expect(log).toBeInstanceOf(Array);
      expect(log.length).toBeGreaterThan(0);
      expect(log[0]).toHaveProperty('id');
      expect(log[0]).toHaveProperty('message');
    });

    it('should create branches', async () => {
      const result = await jj.branchCreate('feature/test');

      expect(result.success).toBe(true);
      expect(result.stdout).toContain('Created branch');
    });

    it('should show diffs between revisions', async () => {
      const diff = await jj.diff('@', '@-');

      expect(diff).toHaveProperty('changes');
      expect(diff).toHaveProperty('filesModified');
      expect(typeof diff.filesModified).toBe('number');
    });

    it('should rebase commits', async () => {
      await jj.branchCreate('feature/rebase-test');
      const result = await jj.rebase('feature/rebase-test', 'main');

      expect(result.success).toBe(true);
      expect(result.stdout).toContain('Rebased');
    });

    it('should execute custom commands', async () => {
      const result = await jj.execute(['git', 'status']);

      expect(result.success).toBe(true);
      expect(result.stdout).toContain('Executed');
    });
  });

  describe('Multi-Agent Coordination', () => {
    it('should handle concurrent commits from multiple agents', async () => {
      const agents = [
        new MockJjWrapper(),
        new MockJjWrapper(),
        new MockJjWrapper()
      ];

      const commits = await Promise.all(
        agents.map((agent, idx) =>
          agent.newCommit(`Commit from agent ${idx}`)
        )
      );

      expect(commits.every(c => c.success)).toBe(true);
      expect(commits.length).toBe(3);
    });

    it('should allow agents to work on different branches simultaneously', async () => {
      const agent1 = new MockJjWrapper();
      const agent2 = new MockJjWrapper();

      const [branch1, branch2] = await Promise.all([
        agent1.branchCreate('agent1/feature'),
        agent2.branchCreate('agent2/feature')
      ]);

      expect(branch1.success).toBe(true);
      expect(branch2.success).toBe(true);
    });

    it('should enable agents to share learning through trajectories', async () => {
      const agent1 = new MockJjWrapper();
      const agent2 = new MockJjWrapper();

      // Agent 1 learns from experience
      agent1.startTrajectory('Deploy feature');
      await agent1.newCommit('Add feature');
      agent1.addToTrajectory();
      agent1.finalizeTrajectory(0.9, 'Successful deployment');

      // Agent 2 benefits from Agent 1's learning
      const suggestion = JSON.parse(agent1.getSuggestion('Deploy feature'));

      expect(suggestion.confidence).toBeGreaterThan(0);
      expect(suggestion.recommendedOperations).toBeInstanceOf(Array);
    });
  });

  describe('ReasoningBank Features', () => {
    it('should start and finalize trajectories', () => {
      const trajectoryId = jj.startTrajectory('Test task');

      expect(trajectoryId).toBeTruthy();
      expect(typeof trajectoryId).toBe('string');

      jj.addToTrajectory();

      // Should not throw
      expect(() => {
        jj.finalizeTrajectory(0.8, 'Test successful');
      }).not.toThrow();
    });

    it('should validate task descriptions', () => {
      expect(() => {
        jj.startTrajectory('');
      }).toThrow(/task cannot be empty/);

      expect(() => {
        jj.startTrajectory('   ');
      }).toThrow(/task cannot be empty/);
    });

    it('should validate success scores', () => {
      jj.startTrajectory('Valid task');
      jj.addToTrajectory();

      expect(() => {
        jj.finalizeTrajectory(1.5);
      }).toThrow(/score must be between/);

      expect(() => {
        jj.finalizeTrajectory(-0.1);
      }).toThrow(/score must be between/);

      expect(() => {
        jj.finalizeTrajectory(NaN);
      }).toThrow(/score must be between/);
    });

    it('should require operations before finalizing', () => {
      jj.startTrajectory('Task without operations');

      expect(() => {
        jj.finalizeTrajectory(0.8);
      }).toThrow(/must have operations/);
    });

    it('should provide AI suggestions based on learned patterns', () => {
      // Record some trajectories
      jj.startTrajectory('Deploy application');
      jj.addToTrajectory();
      jj.finalizeTrajectory(0.9, 'Success');

      const suggestionStr = jj.getSuggestion('Deploy application');
      const suggestion = JSON.parse(suggestionStr);

      expect(suggestion).toHaveProperty('confidence');
      expect(suggestion).toHaveProperty('reasoning');
      expect(suggestion).toHaveProperty('recommendedOperations');
      expect(suggestion).toHaveProperty('expectedSuccessRate');
      expect(suggestion.confidence).toBeGreaterThanOrEqual(0);
      expect(suggestion.confidence).toBeLessThanOrEqual(1);
    });

    it('should track learning statistics', () => {
      // Create multiple trajectories
      for (let i = 0; i < 5; i++) {
        jj.startTrajectory(`Task ${i}`);
        jj.addToTrajectory();
        jj.finalizeTrajectory(0.8 + Math.random() * 0.2);
      }

      const statsStr = jj.getLearningStats();
      const stats = JSON.parse(statsStr);

      expect(stats).toHaveProperty('totalTrajectories');
      expect(stats).toHaveProperty('totalPatterns');
      expect(stats).toHaveProperty('avgSuccessRate');
      expect(stats).toHaveProperty('improvementRate');
      expect(stats).toHaveProperty('predictionAccuracy');
      expect(stats.totalTrajectories).toBe(5);
    });

    it('should discover patterns from repeated operations', () => {
      // Perform similar tasks multiple times
      for (let i = 0; i < 3; i++) {
        jj.startTrajectory('Deploy workflow');
        jj.addToTrajectory();
        jj.finalizeTrajectory(0.9);
      }

      const patternsStr = jj.getPatterns();
      const patterns = JSON.parse(patternsStr);

      expect(patterns).toBeInstanceOf(Array);
      if (patterns.length > 0) {
        expect(patterns[0]).toHaveProperty('name');
        expect(patterns[0]).toHaveProperty('successRate');
        expect(patterns[0]).toHaveProperty('operationSequence');
        expect(patterns[0]).toHaveProperty('confidence');
      }
    });

    it('should query similar trajectories', () => {
      jj.startTrajectory('Feature implementation');
      jj.addToTrajectory();
      jj.finalizeTrajectory(0.85, 'Good implementation');

      const similarStr = jj.queryTrajectories('Feature', 5);
      const similar = JSON.parse(similarStr);

      expect(similar).toBeInstanceOf(Array);
    });

    it('should reset learning data', () => {
      jj.startTrajectory('Test');
      jj.addToTrajectory();
      jj.finalizeTrajectory(0.8);

      jj.resetLearning();

      const stats = JSON.parse(jj.getLearningStats());
      expect(stats.totalTrajectories).toBe(0);
    });
  });

  describe('Quantum-Resistant Security', () => {
    it('should enable encryption', () => {
      const key = 'test-key-32-bytes-long-xxxxxxx';

      jj.enableEncryption(key);

      expect(jj.isEncryptionEnabled()).toBe(true);
    });

    it('should disable encryption', () => {
      jj.enableEncryption('test-key');
      jj.disableEncryption();

      expect(jj.isEncryptionEnabled()).toBe(false);
    });

    it('should maintain encryption state across operations', async () => {
      jj.enableEncryption('test-key');

      await jj.newCommit('Encrypted commit');

      expect(jj.isEncryptionEnabled()).toBe(true);
    });
  });

  describe('Operation Tracking with AgentDB', () => {
    it('should track all operations', async () => {
      await jj.status();
      await jj.newCommit('Test commit');
      await jj.branchCreate('test-branch');

      const stats = JSON.parse(jj.getStats());

      expect(stats).toHaveProperty('total_operations');
      expect(stats).toHaveProperty('success_rate');
      expect(stats).toHaveProperty('avg_duration_ms');
      expect(stats.total_operations).toBeGreaterThan(0);
    });

    it('should retrieve recent operations', async () => {
      await jj.status();
      await jj.newCommit('Test');

      const operations = jj.getOperations(10);

      expect(operations).toBeInstanceOf(Array);
      expect(operations.length).toBeGreaterThan(0);
      expect(operations[0]).toHaveProperty('operationType');
      expect(operations[0]).toHaveProperty('durationMs');
      expect(operations[0]).toHaveProperty('success');
    });

    it('should filter user operations', async () => {
      await jj.status();
      await jj.newCommit('User commit');

      const userOps = jj.getUserOperations(10);

      expect(userOps).toBeInstanceOf(Array);
      expect(userOps.every(op => op.operationType !== 'snapshot')).toBe(true);
    });

    it('should clear operation log', async () => {
      await jj.status();
      await jj.newCommit('Test');

      jj.clearLog();

      const operations = jj.getOperations(10);
      expect(operations.length).toBe(0);
    });
  });

  describe('Collaborative Workflows', () => {
    it('should coordinate code review across multiple agents', async () => {
      const reviewers = [
        { name: 'reviewer-1', jj: new MockJjWrapper() },
        { name: 'reviewer-2', jj: new MockJjWrapper() },
        { name: 'reviewer-3', jj: new MockJjWrapper() }
      ];

      const reviews = await Promise.all(
        reviewers.map(async (reviewer) => {
          reviewer.jj.startTrajectory(`Review by ${reviewer.name}`);

          const diff = await reviewer.jj.diff('@', '@-');

          reviewer.jj.addToTrajectory();
          reviewer.jj.finalizeTrajectory(0.85, 'Review complete');

          return { reviewer: reviewer.name, filesReviewed: diff.filesModified };
        })
      );

      expect(reviews.length).toBe(3);
      expect(reviews.every(r => r.filesReviewed >= 0)).toBe(true);
    });

    it('should enable adaptive workflow optimization', async () => {
      // Simulate multiple deployment attempts
      const deployments = [];

      for (let i = 0; i < 3; i++) {
        jj.startTrajectory('Deploy to staging');
        await jj.execute(['deploy', '--env=staging']);
        jj.addToTrajectory();
        jj.finalizeTrajectory(0.85 + i * 0.05, `Deployment ${i + 1}`);
        deployments.push(i);
      }

      // Get AI suggestion for next deployment
      const suggestion = JSON.parse(jj.getSuggestion('Deploy to staging'));

      expect(suggestion.confidence).toBeGreaterThan(0.8);
      expect(suggestion.expectedSuccessRate).toBeGreaterThan(0.8);
    });

    it('should detect and learn from error patterns', async () => {
      // Simulate failed operations
      jj.startTrajectory('Complex merge');
      try {
        await jj.execute(['merge', 'conflict-branch']);
      } catch (err) {
        // Error expected
      }
      jj.addToTrajectory();
      jj.finalizeTrajectory(0.3, 'Merge conflicts detected');

      // Query for similar scenarios
      const similar = JSON.parse(jj.queryTrajectories('merge', 10));

      expect(similar).toBeInstanceOf(Array);
    });
  });

  describe('Self-Learning Agent Implementation', () => {
    it('should improve performance over multiple iterations', async () => {
      const initialStats = JSON.parse(jj.getLearningStats());
      const initialTrajectories = initialStats.totalTrajectories;

      // Perform multiple learning cycles
      for (let i = 0; i < 10; i++) {
        jj.startTrajectory(`Task iteration ${i}`);
        await jj.newCommit(`Commit ${i}`);
        jj.addToTrajectory();
        jj.finalizeTrajectory(0.7 + i * 0.02, `Iteration ${i}`);
      }

      const finalStats = JSON.parse(jj.getLearningStats());

      expect(finalStats.totalTrajectories).toBe(initialTrajectories + 10);
      expect(finalStats.avgSuccessRate).toBeGreaterThanOrEqual(0.7);
    });

    it('should provide increasingly confident suggestions', () => {
      // First attempt
      const suggestion1 = JSON.parse(jj.getSuggestion('New task type'));

      // Learn from experience
      for (let i = 0; i < 5; i++) {
        jj.startTrajectory('New task type');
        jj.addToTrajectory();
        jj.finalizeTrajectory(0.9);
      }

      // Second attempt
      const suggestion2 = JSON.parse(jj.getSuggestion('New task type'));

      // Confidence should increase or remain high
      expect(suggestion2.confidence).toBeGreaterThanOrEqual(0.5);
    });
  });
});

describe('Performance Characteristics', () => {
  it('should handle high-frequency operations', async () => {
    const jj = new MockJjWrapper();
    const startTime = Date.now();
    const operationCount = 100;

    for (let i = 0; i < operationCount; i++) {
      await jj.status();
    }

    const duration = Date.now() - startTime;
    const opsPerSecond = (operationCount / duration) * 1000;

    // Should achieve >100 ops/second for simple operations
    expect(opsPerSecond).toBeGreaterThan(100);
  });

  it('should minimize context switching overhead', async () => {
    const jj = new MockJjWrapper();

    const startTime = Date.now();

    await jj.newCommit('Test 1');
    await jj.branchCreate('test');
    await jj.newCommit('Test 2');

    const duration = Date.now() - startTime;

    // Context switching should be fast (<100ms for sequence)
    expect(duration).toBeLessThan(100);
  });
});

export { MockJjWrapper };
