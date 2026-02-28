/**
 * Skill Domain Entity - Unit Tests
 *
 * Tests for Skill registration, execution, and validation
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { createSkill, type Skill } from '../../factories';

// Skill Types
interface SkillDefinition {
  id: string;
  name: string;
  version: string;
  description: string;
  inputSchema: JSONSchema;
  outputSchema: JSONSchema;
  executor: string;
  timeout: number;
  retries: number;
  metadata: SkillMetadata;
}

interface JSONSchema {
  type: string;
  properties?: Record<string, unknown>;
  required?: string[];
  additionalProperties?: boolean;
}

interface SkillMetadata {
  author: string;
  createdAt: Date;
  updatedAt: Date;
  usageCount: number;
  averageLatency: number;
  successRate: number;
  tags: string[];
}

interface SkillExecutionContext {
  tenantId: string;
  sessionId: string;
  agentId: string;
  timeout?: number;
}

interface SkillExecutionResult {
  success: boolean;
  output: unknown;
  error?: string;
  latency: number;
  tokensUsed?: number;
}

// Mock Skill Registry class for testing
class SkillRegistry {
  private skills: Map<string, SkillDefinition> = new Map();
  private executors: Map<string, (input: unknown, context: SkillExecutionContext) => Promise<unknown>> = new Map();

  async register(skill: Omit<SkillDefinition, 'metadata'> & { metadata?: Partial<SkillMetadata> }): Promise<SkillDefinition> {
    if (this.skills.has(skill.id)) {
      throw new Error(`Skill ${skill.id} is already registered`);
    }

    this.validateSchema(skill.inputSchema);
    this.validateSchema(skill.outputSchema);

    const fullSkill: SkillDefinition = {
      ...skill,
      metadata: {
        author: skill.metadata?.author || 'unknown',
        createdAt: skill.metadata?.createdAt || new Date(),
        updatedAt: new Date(),
        usageCount: skill.metadata?.usageCount || 0,
        averageLatency: skill.metadata?.averageLatency || 0,
        successRate: skill.metadata?.successRate || 1,
        tags: skill.metadata?.tags || []
      }
    };

    this.skills.set(skill.id, fullSkill);
    return fullSkill;
  }

  async unregister(skillId: string): Promise<boolean> {
    return this.skills.delete(skillId);
  }

  async get(skillId: string): Promise<SkillDefinition | null> {
    return this.skills.get(skillId) || null;
  }

  async getByName(name: string, version?: string): Promise<SkillDefinition | null> {
    for (const skill of this.skills.values()) {
      if (skill.name === name) {
        if (!version || skill.version === version) {
          return skill;
        }
      }
    }
    return null;
  }

  async list(tags?: string[]): Promise<SkillDefinition[]> {
    let skills = Array.from(this.skills.values());

    if (tags && tags.length > 0) {
      skills = skills.filter(s =>
        tags.some(tag => s.metadata.tags.includes(tag))
      );
    }

    return skills;
  }

  async listByExecutorType(type: string): Promise<SkillDefinition[]> {
    return Array.from(this.skills.values()).filter(s =>
      s.executor.startsWith(type)
    );
  }

  registerExecutor(
    pattern: string,
    executor: (input: unknown, context: SkillExecutionContext) => Promise<unknown>
  ): void {
    this.executors.set(pattern, executor);
  }

  async execute(
    skillId: string,
    input: unknown,
    context: SkillExecutionContext
  ): Promise<SkillExecutionResult> {
    const skill = await this.get(skillId);
    if (!skill) {
      return {
        success: false,
        output: null,
        error: `Skill ${skillId} not found`,
        latency: 0
      };
    }

    // Validate input
    const validationError = this.validateInput(input, skill.inputSchema);
    if (validationError) {
      return {
        success: false,
        output: null,
        error: validationError,
        latency: 0
      };
    }

    // Find executor
    const executor = this.findExecutor(skill.executor);
    if (!executor) {
      return {
        success: false,
        output: null,
        error: `No executor found for ${skill.executor}`,
        latency: 0
      };
    }

    // Execute with timeout
    const startTime = performance.now();
    const timeout = context.timeout || skill.timeout;

    try {
      const result = await Promise.race([
        executor(input, context),
        this.createTimeout(timeout)
      ]);

      // Use performance.now() for sub-millisecond precision, ensure minimum 0.001ms
      const latency = Math.max(performance.now() - startTime, 0.001);

      // Update metrics
      this.updateMetrics(skill, true, latency);

      return {
        success: true,
        output: result,
        latency
      };
    } catch (error) {
      const latency = Math.max(performance.now() - startTime, 0.001);

      // Update metrics
      this.updateMetrics(skill, false, latency);

      return {
        success: false,
        output: null,
        error: error instanceof Error ? error.message : 'Unknown error',
        latency
      };
    }
  }

  async executeWithRetry(
    skillId: string,
    input: unknown,
    context: SkillExecutionContext,
    maxRetries?: number
  ): Promise<SkillExecutionResult> {
    const skill = await this.get(skillId);
    const retries = maxRetries ?? skill?.retries ?? 0;

    let lastResult: SkillExecutionResult | null = null;

    for (let attempt = 0; attempt <= retries; attempt++) {
      const result = await this.execute(skillId, input, context);

      if (result.success) {
        return result;
      }

      lastResult = result;

      // Exponential backoff
      if (attempt < retries) {
        await new Promise(resolve => setTimeout(resolve, Math.pow(2, attempt) * 100));
      }
    }

    return lastResult!;
  }

  size(): number {
    return this.skills.size;
  }

  clear(): void {
    this.skills.clear();
  }

  private validateSchema(schema: JSONSchema): void {
    if (!schema.type) {
      throw new Error('Schema must have a type');
    }

    const validTypes = ['object', 'array', 'string', 'number', 'boolean', 'null'];
    if (!validTypes.includes(schema.type)) {
      throw new Error(`Invalid schema type: ${schema.type}`);
    }
  }

  private validateInput(input: unknown, schema: JSONSchema): string | null {
    if (schema.type === 'object') {
      if (typeof input !== 'object' || input === null) {
        return 'Input must be an object';
      }

      const inputObj = input as Record<string, unknown>;

      // Check required fields
      if (schema.required) {
        for (const field of schema.required) {
          if (!(field in inputObj)) {
            return `Missing required field: ${field}`;
          }
        }
      }

      // Validate property types if defined
      if (schema.properties) {
        for (const [key, propSchema] of Object.entries(schema.properties)) {
          if (key in inputObj) {
            const propError = this.validateProperty(inputObj[key], propSchema as JSONSchema);
            if (propError) {
              return `Invalid ${key}: ${propError}`;
            }
          }
        }
      }
    }

    return null;
  }

  private validateProperty(value: unknown, schema: JSONSchema): string | null {
    const type = schema.type;

    switch (type) {
      case 'string':
        if (typeof value !== 'string') return 'must be a string';
        break;
      case 'number':
        if (typeof value !== 'number') return 'must be a number';
        break;
      case 'boolean':
        if (typeof value !== 'boolean') return 'must be a boolean';
        break;
      case 'array':
        if (!Array.isArray(value)) return 'must be an array';
        break;
      case 'object':
        if (typeof value !== 'object' || value === null) return 'must be an object';
        break;
    }

    return null;
  }

  private findExecutor(
    executorUri: string
  ): ((input: unknown, context: SkillExecutionContext) => Promise<unknown>) | null {
    for (const [pattern, executor] of this.executors) {
      if (executorUri.startsWith(pattern)) {
        return executor;
      }
    }
    return null;
  }

  private async createTimeout(ms: number): Promise<never> {
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Skill execution timed out')), ms);
    });
  }

  private updateMetrics(skill: SkillDefinition, success: boolean, latency: number): void {
    const previousCount = skill.metadata.usageCount;
    const totalExecutions = previousCount + 1;
    const totalLatency = skill.metadata.averageLatency * previousCount + latency;

    // Calculate success count from previous executions
    const previousSuccessCount = skill.metadata.successRate * previousCount;
    const newSuccessCount = success ? previousSuccessCount + 1 : previousSuccessCount;

    skill.metadata.usageCount = totalExecutions;
    skill.metadata.averageLatency = totalLatency / totalExecutions;
    skill.metadata.successRate = newSuccessCount / totalExecutions;
    skill.metadata.updatedAt = new Date();
  }
}

// Tests
describe('Skill Registry', () => {
  let registry: SkillRegistry;

  beforeEach(() => {
    registry = new SkillRegistry();
  });

  describe('Registration', () => {
    it('should register a skill', async () => {
      const skill = await registry.register({
        id: 'skill-001',
        name: 'test-skill',
        version: '1.0.0',
        description: 'A test skill',
        inputSchema: { type: 'object', properties: { input: { type: 'string' } } },
        outputSchema: { type: 'object', properties: { output: { type: 'string' } } },
        executor: 'native://test',
        timeout: 30000,
        retries: 3
      });

      expect(skill.id).toBe('skill-001');
      expect(skill.name).toBe('test-skill');
      expect(skill.metadata.usageCount).toBe(0);
    });

    it('should throw error when registering duplicate skill', async () => {
      await registry.register({
        id: 'skill-001',
        name: 'test',
        version: '1.0.0',
        description: 'Test',
        inputSchema: { type: 'object' },
        outputSchema: { type: 'object' },
        executor: 'native://test',
        timeout: 30000,
        retries: 0
      });

      await expect(registry.register({
        id: 'skill-001',
        name: 'duplicate',
        version: '1.0.0',
        description: 'Duplicate',
        inputSchema: { type: 'object' },
        outputSchema: { type: 'object' },
        executor: 'native://test',
        timeout: 30000,
        retries: 0
      })).rejects.toThrow('already registered');
    });

    it('should throw error for invalid schema type', async () => {
      await expect(registry.register({
        id: 'skill-001',
        name: 'test',
        version: '1.0.0',
        description: 'Test',
        inputSchema: { type: 'invalid' as any },
        outputSchema: { type: 'object' },
        executor: 'native://test',
        timeout: 30000,
        retries: 0
      })).rejects.toThrow('Invalid schema type');
    });

    it('should unregister skill', async () => {
      await registry.register({
        id: 'skill-001',
        name: 'test',
        version: '1.0.0',
        description: 'Test',
        inputSchema: { type: 'object' },
        outputSchema: { type: 'object' },
        executor: 'native://test',
        timeout: 30000,
        retries: 0
      });

      const result = await registry.unregister('skill-001');
      const skill = await registry.get('skill-001');

      expect(result).toBe(true);
      expect(skill).toBeNull();
    });
  });

  describe('Retrieval', () => {
    beforeEach(async () => {
      await registry.register({
        id: 'skill-001',
        name: 'code-gen',
        version: '1.0.0',
        description: 'Generate code',
        inputSchema: { type: 'object' },
        outputSchema: { type: 'object' },
        executor: 'wasm://code-gen',
        timeout: 30000,
        retries: 0,
        metadata: { tags: ['code', 'generation'] }
      });

      await registry.register({
        id: 'skill-002',
        name: 'code-gen',
        version: '2.0.0',
        description: 'Generate code v2',
        inputSchema: { type: 'object' },
        outputSchema: { type: 'object' },
        executor: 'wasm://code-gen-v2',
        timeout: 30000,
        retries: 0,
        metadata: { tags: ['code', 'generation', 'v2'] }
      });

      await registry.register({
        id: 'skill-003',
        name: 'test-gen',
        version: '1.0.0',
        description: 'Generate tests',
        inputSchema: { type: 'object' },
        outputSchema: { type: 'object' },
        executor: 'native://test-gen',
        timeout: 60000,
        retries: 2,
        metadata: { tags: ['testing', 'generation'] }
      });
    });

    it('should get skill by ID', async () => {
      const skill = await registry.get('skill-001');
      expect(skill?.name).toBe('code-gen');
    });

    it('should get skill by name', async () => {
      const skill = await registry.getByName('code-gen');
      expect(skill).not.toBeNull();
      expect(skill?.name).toBe('code-gen');
    });

    it('should get skill by name and version', async () => {
      const skill = await registry.getByName('code-gen', '2.0.0');
      expect(skill?.id).toBe('skill-002');
    });

    it('should list all skills', async () => {
      const skills = await registry.list();
      expect(skills).toHaveLength(3);
    });

    it('should list skills by tag', async () => {
      const skills = await registry.list(['testing']);
      expect(skills).toHaveLength(1);
      expect(skills[0].name).toBe('test-gen');
    });

    it('should list skills by executor type', async () => {
      const wasmSkills = await registry.listByExecutorType('wasm://');
      const nativeSkills = await registry.listByExecutorType('native://');

      expect(wasmSkills).toHaveLength(2);
      expect(nativeSkills).toHaveLength(1);
    });
  });

  describe('Execution', () => {
    const context: SkillExecutionContext = {
      tenantId: 'tenant-001',
      sessionId: 'session-001',
      agentId: 'agent-001'
    };

    beforeEach(async () => {
      await registry.register({
        id: 'skill-001',
        name: 'echo',
        version: '1.0.0',
        description: 'Echo input',
        inputSchema: {
          type: 'object',
          properties: { message: { type: 'string' } },
          required: ['message']
        },
        outputSchema: { type: 'object' },
        executor: 'native://echo',
        timeout: 5000,
        retries: 2
      });

      registry.registerExecutor('native://echo', async (input) => {
        return { echoed: (input as any).message };
      });
    });

    it('should execute skill successfully', async () => {
      const result = await registry.execute(
        'skill-001',
        { message: 'Hello' },
        context
      );

      expect(result.success).toBe(true);
      expect(result.output).toEqual({ echoed: 'Hello' });
      expect(result.latency).toBeGreaterThan(0);
    });

    it('should fail for non-existent skill', async () => {
      const result = await registry.execute(
        'non-existent',
        {},
        context
      );

      expect(result.success).toBe(false);
      expect(result.error).toContain('not found');
    });

    it('should validate required input fields', async () => {
      const result = await registry.execute(
        'skill-001',
        {},
        context
      );

      expect(result.success).toBe(false);
      expect(result.error).toContain('Missing required field');
    });

    it('should fail without executor', async () => {
      await registry.register({
        id: 'skill-no-executor',
        name: 'no-executor',
        version: '1.0.0',
        description: 'No executor',
        inputSchema: { type: 'object' },
        outputSchema: { type: 'object' },
        executor: 'unknown://test',
        timeout: 5000,
        retries: 0
      });

      const result = await registry.execute(
        'skill-no-executor',
        {},
        context
      );

      expect(result.success).toBe(false);
      expect(result.error).toContain('No executor found');
    });

    it('should handle execution errors', async () => {
      await registry.register({
        id: 'skill-error',
        name: 'error',
        version: '1.0.0',
        description: 'Throws error',
        inputSchema: { type: 'object' },
        outputSchema: { type: 'object' },
        executor: 'native://error',
        timeout: 5000,
        retries: 0
      });

      registry.registerExecutor('native://error', async () => {
        throw new Error('Execution failed');
      });

      const result = await registry.execute(
        'skill-error',
        {},
        context
      );

      expect(result.success).toBe(false);
      expect(result.error).toBe('Execution failed');
    });

    it('should update metrics after execution', async () => {
      await registry.execute(
        'skill-001',
        { message: 'test' },
        context
      );

      const skill = await registry.get('skill-001');
      expect(skill?.metadata.usageCount).toBe(1);
      expect(skill?.metadata.averageLatency).toBeGreaterThan(0);
      expect(skill?.metadata.successRate).toBe(1);
    });

    it('should update success rate on failure', async () => {
      await registry.register({
        id: 'skill-flaky',
        name: 'flaky',
        version: '1.0.0',
        description: 'Flaky skill',
        inputSchema: { type: 'object' },
        outputSchema: { type: 'object' },
        executor: 'native://flaky',
        timeout: 5000,
        retries: 0
      });

      let callCount = 0;
      registry.registerExecutor('native://flaky', async () => {
        callCount++;
        if (callCount === 1) {
          throw new Error('First call fails');
        }
        return { success: true };
      });

      // First call fails
      await registry.execute('skill-flaky', {}, context);

      // Second call succeeds
      await registry.execute('skill-flaky', {}, context);

      const skill = await registry.get('skill-flaky');
      expect(skill?.metadata.usageCount).toBe(2);
      expect(skill?.metadata.successRate).toBe(0.5);
    });
  });

  describe('Retry Mechanism', () => {
    const context: SkillExecutionContext = {
      tenantId: 'tenant-001',
      sessionId: 'session-001',
      agentId: 'agent-001'
    };

    it('should retry failed executions', async () => {
      await registry.register({
        id: 'skill-retry',
        name: 'retry',
        version: '1.0.0',
        description: 'Retry skill',
        inputSchema: { type: 'object' },
        outputSchema: { type: 'object' },
        executor: 'native://retry',
        timeout: 5000,
        retries: 2
      });

      let attempts = 0;
      registry.registerExecutor('native://retry', async () => {
        attempts++;
        if (attempts < 3) {
          throw new Error(`Attempt ${attempts} failed`);
        }
        return { success: true };
      });

      const result = await registry.executeWithRetry(
        'skill-retry',
        {},
        context
      );

      expect(result.success).toBe(true);
      expect(attempts).toBe(3);
    });

    it('should fail after max retries', async () => {
      await registry.register({
        id: 'skill-always-fail',
        name: 'always-fail',
        version: '1.0.0',
        description: 'Always fails',
        inputSchema: { type: 'object' },
        outputSchema: { type: 'object' },
        executor: 'native://always-fail',
        timeout: 5000,
        retries: 2
      });

      registry.registerExecutor('native://always-fail', async () => {
        throw new Error('Always fails');
      });

      const result = await registry.executeWithRetry(
        'skill-always-fail',
        {},
        context
      );

      expect(result.success).toBe(false);
    });
  });
});

describe('Skill Factory Integration', () => {
  let registry: SkillRegistry;

  beforeEach(() => {
    registry = new SkillRegistry();
  });

  it('should register skill from factory data', async () => {
    const factorySkill = createSkill({
      name: 'factory-skill',
      description: 'Created from factory'
    });

    const skill = await registry.register({
      id: factorySkill.id,
      name: factorySkill.name,
      version: factorySkill.version,
      description: factorySkill.description,
      inputSchema: factorySkill.inputSchema as any,
      outputSchema: factorySkill.outputSchema as any,
      executor: factorySkill.executor,
      timeout: factorySkill.timeout,
      retries: 0
    });

    expect(skill.name).toBe('factory-skill');
    expect(skill.description).toBe('Created from factory');
  });
});
