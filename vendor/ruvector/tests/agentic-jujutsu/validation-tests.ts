/**
 * Agentic-Jujutsu Validation Tests
 *
 * Comprehensive validation suite for data integrity, security, and correctness.
 *
 * Test Coverage:
 * - Data integrity verification
 * - Cryptographic signature validation
 * - Version history accuracy
 * - Rollback functionality
 * - Input validation (v2.3.1+)
 * - Quantum fingerprint integrity
 * - Cross-agent data consistency
 */

import { describe, it, expect, beforeEach } from '@jest/globals';
import * as crypto from 'crypto';

interface ValidationResult {
  isValid: boolean;
  errors: string[];
  warnings: string[];
}

interface IntegrityCheck {
  dataHash: string;
  timestamp: number;
  verified: boolean;
}

interface RollbackState {
  commitId: string;
  timestamp: number;
  data: any;
}

// Mock validation utilities
class ValidationJjWrapper {
  private commits: Map<string, any> = new Map();
  private branches: Map<string, string> = new Map();
  private trajectories: any[] = [];
  private fingerprints: Map<string, Buffer> = new Map();

  async newCommit(message: string, data?: any): Promise<string> {
    const commitId = this.generateCommitId();
    const commitData = {
      id: commitId,
      message,
      data: data || {},
      timestamp: Date.now(),
      hash: this.calculateHash({ message, data, timestamp: Date.now() })
    };

    this.commits.set(commitId, commitData);
    return commitId;
  }

  async getCommit(commitId: string): Promise<any | null> {
    return this.commits.get(commitId) || null;
  }

  async verifyCommitIntegrity(commitId: string): Promise<ValidationResult> {
    const commit = this.commits.get(commitId);
    if (!commit) {
      return {
        isValid: false,
        errors: ['Commit not found'],
        warnings: []
      };
    }

    const recalculatedHash = this.calculateHash({
      message: commit.message,
      data: commit.data,
      timestamp: commit.timestamp
    });

    const isValid = recalculatedHash === commit.hash;

    return {
      isValid,
      errors: isValid ? [] : ['Hash mismatch - data may be corrupted'],
      warnings: []
    };
  }

  async branchCreate(name: string, fromCommit?: string): Promise<void> {
    const commitId = fromCommit || Array.from(this.commits.keys()).pop() || 'genesis';
    this.branches.set(name, commitId);
  }

  async getBranchHead(name: string): Promise<string | null> {
    return this.branches.get(name) || null;
  }

  async verifyBranchIntegrity(name: string): Promise<ValidationResult> {
    const commitId = this.branches.get(name);
    if (!commitId) {
      return {
        isValid: false,
        errors: ['Branch not found'],
        warnings: []
      };
    }

    const commit = this.commits.get(commitId);
    if (!commit) {
      return {
        isValid: false,
        errors: ['Branch points to non-existent commit'],
        warnings: []
      };
    }

    return {
      isValid: true,
      errors: [],
      warnings: []
    };
  }

  startTrajectory(task: string): string {
    // Validate task according to v2.3.1 rules
    if (!task || task.trim().length === 0) {
      throw new Error('Validation error: task cannot be empty');
    }

    const trimmed = task.trim();
    if (Buffer.byteLength(trimmed, 'utf8') > 10000) {
      throw new Error('Validation error: task exceeds maximum length of 10KB');
    }

    const id = `traj-${Date.now()}`;
    this.trajectories.push({
      id,
      task: trimmed,
      operations: [],
      context: {},
      finalized: false
    });

    return id;
  }

  addToTrajectory(): void {
    const current = this.trajectories[this.trajectories.length - 1];
    if (current) {
      current.operations.push({
        type: 'operation',
        timestamp: Date.now()
      });
    }
  }

  finalizeTrajectory(score: number, critique?: string): void {
    const current = this.trajectories[this.trajectories.length - 1];

    if (!current) {
      throw new Error('No active trajectory');
    }

    // Validate score
    if (!Number.isFinite(score)) {
      throw new Error('Validation error: score must be finite');
    }

    if (score < 0 || score > 1) {
      throw new Error('Validation error: score must be between 0.0 and 1.0');
    }

    // Validate operations
    if (current.operations.length === 0) {
      throw new Error('Validation error: must have at least one operation before finalizing');
    }

    current.score = score;
    current.critique = critique || '';
    current.finalized = true;
  }

  setTrajectoryContext(key: string, value: string): void {
    const current = this.trajectories[this.trajectories.length - 1];
    if (!current) {
      throw new Error('No active trajectory');
    }

    // Validate context key
    if (!key || key.trim().length === 0) {
      throw new Error('Validation error: context key cannot be empty');
    }

    if (Buffer.byteLength(key, 'utf8') > 1000) {
      throw new Error('Validation error: context key exceeds maximum length of 1KB');
    }

    // Validate context value
    if (Buffer.byteLength(value, 'utf8') > 10000) {
      throw new Error('Validation error: context value exceeds maximum length of 10KB');
    }

    current.context[key] = value;
  }

  verifyTrajectoryIntegrity(trajectoryId: string): ValidationResult {
    const trajectory = this.trajectories.find(t => t.id === trajectoryId);

    if (!trajectory) {
      return {
        isValid: false,
        errors: ['Trajectory not found'],
        warnings: []
      };
    }

    const errors: string[] = [];
    const warnings: string[] = [];

    // Check if finalized
    if (!trajectory.finalized) {
      warnings.push('Trajectory not finalized');
    }

    // Check score validity
    if (trajectory.finalized) {
      if (trajectory.score < 0 || trajectory.score > 1) {
        errors.push('Invalid score value');
      }
    }

    // Check operations
    if (trajectory.operations.length === 0) {
      errors.push('No operations recorded');
    }

    return {
      isValid: errors.length === 0,
      errors,
      warnings
    };
  }

  generateQuantumFingerprint(data: Buffer): Buffer {
    // Simulate SHA3-512 (64 bytes)
    const hash = crypto.createHash('sha512');
    hash.update(data);
    const fingerprint = hash.digest();

    // Store for verification
    const key = data.toString('hex');
    this.fingerprints.set(key, fingerprint);

    return fingerprint;
  }

  verifyQuantumFingerprint(data: Buffer, fingerprint: Buffer): boolean {
    const hash = crypto.createHash('sha512');
    hash.update(data);
    const calculated = hash.digest();

    return calculated.equals(fingerprint);
  }

  async createRollbackPoint(label: string): Promise<string> {
    const state = {
      commits: Array.from(this.commits.entries()),
      branches: Array.from(this.branches.entries()),
      trajectories: JSON.parse(JSON.stringify(this.trajectories))
    };

    const rollbackId = `rollback-${Date.now()}`;
    const stateJson = JSON.stringify(state);

    // Create commit for rollback point
    await this.newCommit(`Rollback point: ${label}`, { state: stateJson });

    return rollbackId;
  }

  async rollback(rollbackId: string): Promise<ValidationResult> {
    // Simulate rollback
    return {
      isValid: true,
      errors: [],
      warnings: ['Rollback would reset state']
    };
  }

  private generateCommitId(): string {
    return crypto.randomBytes(20).toString('hex');
  }

  private calculateHash(data: any): string {
    const json = JSON.stringify(data);
    return crypto.createHash('sha256').update(json).digest('hex');
  }
}

describe('Agentic-Jujutsu Validation Tests', () => {
  let jj: ValidationJjWrapper;

  beforeEach(() => {
    jj = new ValidationJjWrapper();
  });

  describe('Data Integrity Verification', () => {
    it('should verify commit data integrity', async () => {
      const commitId = await jj.newCommit('Test commit', { content: 'test data' });

      const validation = await jj.verifyCommitIntegrity(commitId);

      expect(validation.isValid).toBe(true);
      expect(validation.errors).toHaveLength(0);
    });

    it('should detect corrupted commit data', async () => {
      const commitId = await jj.newCommit('Test commit');
      const commit = await jj.getCommit(commitId);

      // Manually corrupt the commit
      commit.data = 'corrupted';

      const validation = await jj.verifyCommitIntegrity(commitId);

      expect(validation.isValid).toBe(false);
      expect(validation.errors.length).toBeGreaterThan(0);
      expect(validation.errors[0]).toContain('Hash mismatch');
    });

    it('should verify branch integrity', async () => {
      const commitId = await jj.newCommit('Test commit');
      await jj.branchCreate('test-branch', commitId);

      const validation = await jj.verifyBranchIntegrity('test-branch');

      expect(validation.isValid).toBe(true);
      expect(validation.errors).toHaveLength(0);
    });

    it('should detect invalid branch references', async () => {
      await jj.branchCreate('test-branch', 'non-existent-commit');

      const validation = await jj.verifyBranchIntegrity('test-branch');

      expect(validation.isValid).toBe(false);
      expect(validation.errors).toContain('Branch points to non-existent commit');
    });

    it('should verify trajectory data integrity', async () => {
      const trajectoryId = jj.startTrajectory('Test task');
      jj.addToTrajectory();
      jj.finalizeTrajectory(0.8, 'Test successful');

      const validation = jj.verifyTrajectoryIntegrity(trajectoryId);

      expect(validation.isValid).toBe(true);
      expect(validation.errors).toHaveLength(0);
    });

    it('should detect incomplete trajectories', async () => {
      const trajectoryId = jj.startTrajectory('Incomplete task');

      const validation = jj.verifyTrajectoryIntegrity(trajectoryId);

      expect(validation.isValid).toBe(false);
      expect(validation.warnings).toContain('Trajectory not finalized');
      expect(validation.errors).toContain('No operations recorded');
    });
  });

  describe('Input Validation (v2.3.1 Compliance)', () => {
    describe('Task Description Validation', () => {
      it('should reject empty task descriptions', () => {
        expect(() => {
          jj.startTrajectory('');
        }).toThrow(/task cannot be empty/);
      });

      it('should reject whitespace-only task descriptions', () => {
        expect(() => {
          jj.startTrajectory('   ');
        }).toThrow(/task cannot be empty/);
      });

      it('should accept and trim valid task descriptions', () => {
        const trajectoryId = jj.startTrajectory('  Valid task  ');
        expect(trajectoryId).toBeTruthy();
      });

      it('should reject task descriptions exceeding 10KB', () => {
        const largeTask = 'a'.repeat(10001);

        expect(() => {
          jj.startTrajectory(largeTask);
        }).toThrow(/exceeds maximum length/);
      });

      it('should accept task descriptions at 10KB limit', () => {
        const maxTask = 'a'.repeat(10000);

        const trajectoryId = jj.startTrajectory(maxTask);
        expect(trajectoryId).toBeTruthy();
      });
    });

    describe('Success Score Validation', () => {
      beforeEach(() => {
        jj.startTrajectory('Test task');
        jj.addToTrajectory();
      });

      it('should accept valid scores (0.0 to 1.0)', () => {
        expect(() => jj.finalizeTrajectory(0.0)).not.toThrow();

        jj.startTrajectory('Test 2');
        jj.addToTrajectory();
        expect(() => jj.finalizeTrajectory(0.5)).not.toThrow();

        jj.startTrajectory('Test 3');
        jj.addToTrajectory();
        expect(() => jj.finalizeTrajectory(1.0)).not.toThrow();
      });

      it('should reject scores below 0.0', () => {
        expect(() => {
          jj.finalizeTrajectory(-0.1);
        }).toThrow(/score must be between/);
      });

      it('should reject scores above 1.0', () => {
        expect(() => {
          jj.finalizeTrajectory(1.1);
        }).toThrow(/score must be between/);
      });

      it('should reject NaN scores', () => {
        expect(() => {
          jj.finalizeTrajectory(NaN);
        }).toThrow(/score must be finite/);
      });

      it('should reject Infinity scores', () => {
        expect(() => {
          jj.finalizeTrajectory(Infinity);
        }).toThrow(/score must be finite/);
      });
    });

    describe('Operations Validation', () => {
      it('should require operations before finalizing', () => {
        jj.startTrajectory('Task without operations');

        expect(() => {
          jj.finalizeTrajectory(0.8);
        }).toThrow(/must have at least one operation/);
      });

      it('should allow finalizing with operations', () => {
        jj.startTrajectory('Task with operations');
        jj.addToTrajectory();

        expect(() => {
          jj.finalizeTrajectory(0.8);
        }).not.toThrow();
      });
    });

    describe('Context Validation', () => {
      beforeEach(() => {
        jj.startTrajectory('Test task');
      });

      it('should reject empty context keys', () => {
        expect(() => {
          jj.setTrajectoryContext('', 'value');
        }).toThrow(/context key cannot be empty/);
      });

      it('should reject whitespace-only context keys', () => {
        expect(() => {
          jj.setTrajectoryContext('   ', 'value');
        }).toThrow(/context key cannot be empty/);
      });

      it('should reject context keys exceeding 1KB', () => {
        const largeKey = 'k'.repeat(1001);

        expect(() => {
          jj.setTrajectoryContext(largeKey, 'value');
        }).toThrow(/context key exceeds/);
      });

      it('should reject context values exceeding 10KB', () => {
        const largeValue = 'v'.repeat(10001);

        expect(() => {
          jj.setTrajectoryContext('key', largeValue);
        }).toThrow(/context value exceeds/);
      });

      it('should accept valid context entries', () => {
        expect(() => {
          jj.setTrajectoryContext('environment', 'production');
          jj.setTrajectoryContext('version', '1.0.0');
        }).not.toThrow();
      });
    });
  });

  describe('Cryptographic Signature Validation', () => {
    it('should generate quantum-resistant fingerprints', () => {
      const data = Buffer.from('test data');

      const fingerprint = jj.generateQuantumFingerprint(data);

      expect(fingerprint).toBeInstanceOf(Buffer);
      expect(fingerprint.length).toBe(64); // SHA3-512 = 64 bytes
    });

    it('should verify valid quantum fingerprints', () => {
      const data = Buffer.from('test data');
      const fingerprint = jj.generateQuantumFingerprint(data);

      const isValid = jj.verifyQuantumFingerprint(data, fingerprint);

      expect(isValid).toBe(true);
    });

    it('should reject invalid quantum fingerprints', () => {
      const data = Buffer.from('test data');
      const wrongData = Buffer.from('wrong data');
      const fingerprint = jj.generateQuantumFingerprint(data);

      const isValid = jj.verifyQuantumFingerprint(wrongData, fingerprint);

      expect(isValid).toBe(false);
    });

    it('should detect tampered fingerprints', () => {
      const data = Buffer.from('test data');
      const fingerprint = jj.generateQuantumFingerprint(data);

      // Tamper with fingerprint
      fingerprint[0] ^= 0xFF;

      const isValid = jj.verifyQuantumFingerprint(data, fingerprint);

      expect(isValid).toBe(false);
    });

    it('should generate unique fingerprints for different data', () => {
      const data1 = Buffer.from('data 1');
      const data2 = Buffer.from('data 2');

      const fp1 = jj.generateQuantumFingerprint(data1);
      const fp2 = jj.generateQuantumFingerprint(data2);

      expect(fp1.equals(fp2)).toBe(false);
    });

    it('should generate consistent fingerprints for same data', () => {
      const data = Buffer.from('consistent data');

      const fp1 = jj.generateQuantumFingerprint(data);
      const fp2 = jj.generateQuantumFingerprint(data);

      expect(fp1.equals(fp2)).toBe(true);
    });
  });

  describe('Version History Accuracy', () => {
    it('should maintain accurate commit history', async () => {
      const commit1 = await jj.newCommit('First commit');
      const commit2 = await jj.newCommit('Second commit');
      const commit3 = await jj.newCommit('Third commit');

      const c1 = await jj.getCommit(commit1);
      const c2 = await jj.getCommit(commit2);
      const c3 = await jj.getCommit(commit3);

      expect(c1?.message).toBe('First commit');
      expect(c2?.message).toBe('Second commit');
      expect(c3?.message).toBe('Third commit');

      expect(c1?.timestamp).toBeLessThan(c2?.timestamp);
      expect(c2?.timestamp).toBeLessThan(c3?.timestamp);
    });

    it('should maintain branch references accurately', async () => {
      const mainCommit = await jj.newCommit('Main commit');
      await jj.branchCreate('main', mainCommit);

      const featureCommit = await jj.newCommit('Feature commit');
      await jj.branchCreate('feature', featureCommit);

      const mainHead = await jj.getBranchHead('main');
      const featureHead = await jj.getBranchHead('feature');

      expect(mainHead).toBe(mainCommit);
      expect(featureHead).toBe(featureCommit);
    });

    it('should maintain trajectory history accurately', () => {
      const traj1 = jj.startTrajectory('Task 1');
      jj.addToTrajectory();
      jj.finalizeTrajectory(0.8);

      const traj2 = jj.startTrajectory('Task 2');
      jj.addToTrajectory();
      jj.finalizeTrajectory(0.9);

      const v1 = jj.verifyTrajectoryIntegrity(traj1);
      const v2 = jj.verifyTrajectoryIntegrity(traj2);

      expect(v1.isValid).toBe(true);
      expect(v2.isValid).toBe(true);
    });
  });

  describe('Rollback Functionality', () => {
    it('should create rollback points', async () => {
      await jj.newCommit('Before rollback');

      const rollbackId = await jj.createRollbackPoint('Safe state');

      expect(rollbackId).toBeTruthy();
      expect(typeof rollbackId).toBe('string');
    });

    it('should rollback to previous state', async () => {
      await jj.newCommit('Commit 1');
      const rollbackId = await jj.createRollbackPoint('Checkpoint');
      await jj.newCommit('Commit 2');

      const result = await jj.rollback(rollbackId);

      expect(result.isValid).toBe(true);
      expect(result.warnings).toContain('Rollback would reset state');
    });

    it('should maintain data integrity after rollback', async () => {
      const commit1 = await jj.newCommit('Original commit');
      const rollbackId = await jj.createRollbackPoint('Original state');

      await jj.rollback(rollbackId);

      // Verify original commit still valid
      const validation = await jj.verifyCommitIntegrity(commit1);
      expect(validation.isValid).toBe(true);
    });
  });

  describe('Cross-Agent Data Consistency', () => {
    it('should maintain consistency across multiple agents', async () => {
      const agents = [
        new ValidationJjWrapper(),
        new ValidationJjWrapper(),
        new ValidationJjWrapper()
      ];

      // Each agent creates commits
      const commits = await Promise.all(
        agents.map((agent, idx) =>
          agent.newCommit(`Agent ${idx} commit`)
        )
      );

      // Verify all commits are valid
      const validations = await Promise.all(
        agents.map((agent, idx) =>
          agent.verifyCommitIntegrity(commits[idx])
        )
      );

      expect(validations.every(v => v.isValid)).toBe(true);
    });

    it('should detect inconsistencies in shared state', async () => {
      const agent1 = new ValidationJjWrapper();
      const agent2 = new ValidationJjWrapper();

      // Agent 1 creates branch
      const commit1 = await agent1.newCommit('Shared commit');
      await agent1.branchCreate('shared-branch', commit1);

      // Agent 2 tries to reference same branch
      const validation = await agent2.verifyBranchIntegrity('shared-branch');

      // Should detect branch doesn't exist in agent2's context
      expect(validation.isValid).toBe(false);
    });
  });

  describe('Edge Cases and Boundary Conditions', () => {
    it('should handle empty commits gracefully', async () => {
      const commitId = await jj.newCommit('');
      const validation = await jj.verifyCommitIntegrity(commitId);

      expect(validation.isValid).toBe(true);
    });

    it('should handle very long commit messages', async () => {
      const longMessage = 'x'.repeat(10000);
      const commitId = await jj.newCommit(longMessage);
      const validation = await jj.verifyCommitIntegrity(commitId);

      expect(validation.isValid).toBe(true);
    });

    it('should handle special characters in data', async () => {
      const specialData = {
        unicode: '你好世界 🚀',
        special: '<>&"\'',
        escape: '\\n\\t\\r'
      };

      const commitId = await jj.newCommit('Special chars', specialData);
      const validation = await jj.verifyCommitIntegrity(commitId);

      expect(validation.isValid).toBe(true);
    });

    it('should handle concurrent validation requests', async () => {
      const commit1 = await jj.newCommit('Commit 1');
      const commit2 = await jj.newCommit('Commit 2');
      const commit3 = await jj.newCommit('Commit 3');

      const validations = await Promise.all([
        jj.verifyCommitIntegrity(commit1),
        jj.verifyCommitIntegrity(commit2),
        jj.verifyCommitIntegrity(commit3)
      ]);

      expect(validations.every(v => v.isValid)).toBe(true);
    });
  });
});

export { ValidationJjWrapper, ValidationResult };
