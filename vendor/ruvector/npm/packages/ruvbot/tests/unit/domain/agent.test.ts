/**
 * Agent Domain Entity - Unit Tests
 *
 * Tests for Agent lifecycle, state management, and behavior
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { createAgent, createAgents, type Agent, type AgentConfig } from '../../factories';

// Agent Entity Types (would be imported from src/domain/agent.ts)
interface AgentState {
  id: string;
  name: string;
  type: Agent['type'];
  status: Agent['status'];
  capabilities: string[];
  config: AgentConfig;
  currentTask?: string;
  metrics: AgentMetrics;
}

interface AgentMetrics {
  tasksCompleted: number;
  averageLatency: number;
  errorCount: number;
  lastActiveAt: Date | null;
}

// Mock Agent class for testing
class AgentEntity {
  private state: AgentState;
  private eventLog: Array<{ type: string; payload: unknown; timestamp: Date }> = [];

  constructor(initialState: Partial<AgentState>) {
    this.state = {
      id: initialState.id || `agent-${Date.now()}`,
      name: initialState.name || 'Unnamed Agent',
      type: initialState.type || 'coder',
      status: initialState.status || 'idle',
      capabilities: initialState.capabilities || [],
      config: initialState.config || {
        model: 'claude-sonnet-4',
        temperature: 0.7,
        maxTokens: 4096
      },
      currentTask: initialState.currentTask,
      metrics: initialState.metrics || {
        tasksCompleted: 0,
        averageLatency: 0,
        errorCount: 0,
        lastActiveAt: null
      }
    };
  }

  getId(): string {
    return this.state.id;
  }

  getName(): string {
    return this.state.name;
  }

  getType(): Agent['type'] {
    return this.state.type;
  }

  getStatus(): Agent['status'] {
    return this.state.status;
  }

  getCapabilities(): string[] {
    return [...this.state.capabilities];
  }

  getConfig(): AgentConfig {
    return { ...this.state.config };
  }

  getMetrics(): AgentMetrics {
    return { ...this.state.metrics };
  }

  getCurrentTask(): string | undefined {
    return this.state.currentTask;
  }

  isAvailable(): boolean {
    return this.state.status === 'idle';
  }

  hasCapability(capability: string): boolean {
    return this.state.capabilities.includes(capability);
  }

  async assignTask(taskId: string): Promise<void> {
    if (this.state.status !== 'idle') {
      throw new Error(`Agent ${this.state.id} is not available (status: ${this.state.status})`);
    }

    this.state.status = 'busy';
    this.state.currentTask = taskId;
    this.state.metrics.lastActiveAt = new Date();
    this.logEvent('task_assigned', { taskId });
  }

  async completeTask(result: { success: boolean; latency: number }): Promise<void> {
    if (this.state.status !== 'busy') {
      throw new Error(`Agent ${this.state.id} has no active task`);
    }

    const taskId = this.state.currentTask;
    this.state.status = 'idle';
    this.state.currentTask = undefined;
    this.state.metrics.tasksCompleted++;

    // Update average latency
    const totalLatency = this.state.metrics.averageLatency * (this.state.metrics.tasksCompleted - 1);
    this.state.metrics.averageLatency = (totalLatency + result.latency) / this.state.metrics.tasksCompleted;

    if (!result.success) {
      this.state.metrics.errorCount++;
    }

    this.logEvent('task_completed', { taskId, result });
  }

  async failTask(error: Error): Promise<void> {
    if (this.state.status !== 'busy') {
      throw new Error(`Agent ${this.state.id} has no active task`);
    }

    const taskId = this.state.currentTask;
    this.state.status = 'error';
    this.state.currentTask = undefined;
    this.state.metrics.errorCount++;

    this.logEvent('task_failed', { taskId, error: error.message });
  }

  async recover(): Promise<void> {
    if (this.state.status !== 'error') {
      throw new Error(`Agent ${this.state.id} is not in error state`);
    }

    this.state.status = 'idle';
    this.logEvent('recovered', {});
  }

  async terminate(): Promise<void> {
    this.state.status = 'terminated';
    this.state.currentTask = undefined;
    this.logEvent('terminated', {});
  }

  updateConfig(config: Partial<AgentConfig>): void {
    this.state.config = { ...this.state.config, ...config };
    this.logEvent('config_updated', { config });
  }

  addCapability(capability: string): void {
    if (!this.state.capabilities.includes(capability)) {
      this.state.capabilities.push(capability);
      this.logEvent('capability_added', { capability });
    }
  }

  removeCapability(capability: string): void {
    const index = this.state.capabilities.indexOf(capability);
    if (index !== -1) {
      this.state.capabilities.splice(index, 1);
      this.logEvent('capability_removed', { capability });
    }
  }

  getEventLog(): Array<{ type: string; payload: unknown; timestamp: Date }> {
    return [...this.eventLog];
  }

  toJSON(): AgentState {
    return { ...this.state };
  }

  private logEvent(type: string, payload: unknown): void {
    this.eventLog.push({ type, payload, timestamp: new Date() });
  }
}

// Tests
describe('Agent Domain Entity', () => {
  describe('Construction', () => {
    it('should create agent with default values', () => {
      const agent = new AgentEntity({});

      expect(agent.getId()).toBeDefined();
      expect(agent.getName()).toBe('Unnamed Agent');
      expect(agent.getType()).toBe('coder');
      expect(agent.getStatus()).toBe('idle');
      expect(agent.getCapabilities()).toEqual([]);
    });

    it('should create agent with provided values', () => {
      const agent = new AgentEntity({
        id: 'test-agent',
        name: 'Test Agent',
        type: 'researcher',
        capabilities: ['web-search', 'analysis']
      });

      expect(agent.getId()).toBe('test-agent');
      expect(agent.getName()).toBe('Test Agent');
      expect(agent.getType()).toBe('researcher');
      expect(agent.getCapabilities()).toEqual(['web-search', 'analysis']);
    });

    it('should initialize metrics correctly', () => {
      const agent = new AgentEntity({});
      const metrics = agent.getMetrics();

      expect(metrics.tasksCompleted).toBe(0);
      expect(metrics.averageLatency).toBe(0);
      expect(metrics.errorCount).toBe(0);
      expect(metrics.lastActiveAt).toBeNull();
    });
  });

  describe('Availability', () => {
    it('should be available when idle', () => {
      const agent = new AgentEntity({ status: 'idle' });
      expect(agent.isAvailable()).toBe(true);
    });

    it('should not be available when busy', () => {
      const agent = new AgentEntity({ status: 'busy' });
      expect(agent.isAvailable()).toBe(false);
    });

    it('should not be available when in error state', () => {
      const agent = new AgentEntity({ status: 'error' });
      expect(agent.isAvailable()).toBe(false);
    });

    it('should not be available when terminated', () => {
      const agent = new AgentEntity({ status: 'terminated' });
      expect(agent.isAvailable()).toBe(false);
    });
  });

  describe('Capabilities', () => {
    it('should check for capability correctly', () => {
      const agent = new AgentEntity({
        capabilities: ['code-generation', 'code-review']
      });

      expect(agent.hasCapability('code-generation')).toBe(true);
      expect(agent.hasCapability('code-review')).toBe(true);
      expect(agent.hasCapability('unknown')).toBe(false);
    });

    it('should add capability', () => {
      const agent = new AgentEntity({ capabilities: [] });

      agent.addCapability('new-capability');

      expect(agent.hasCapability('new-capability')).toBe(true);
    });

    it('should not duplicate capability', () => {
      const agent = new AgentEntity({ capabilities: ['existing'] });

      agent.addCapability('existing');

      expect(agent.getCapabilities()).toEqual(['existing']);
    });

    it('should remove capability', () => {
      const agent = new AgentEntity({ capabilities: ['to-remove', 'to-keep'] });

      agent.removeCapability('to-remove');

      expect(agent.hasCapability('to-remove')).toBe(false);
      expect(agent.hasCapability('to-keep')).toBe(true);
    });
  });

  describe('Task Lifecycle', () => {
    it('should assign task to idle agent', async () => {
      const agent = new AgentEntity({ status: 'idle' });

      await agent.assignTask('task-001');

      expect(agent.getStatus()).toBe('busy');
      expect(agent.getCurrentTask()).toBe('task-001');
      expect(agent.getMetrics().lastActiveAt).not.toBeNull();
    });

    it('should throw error when assigning task to busy agent', async () => {
      const agent = new AgentEntity({ status: 'busy', currentTask: 'existing-task' });

      await expect(agent.assignTask('new-task')).rejects.toThrow('not available');
    });

    it('should complete task successfully', async () => {
      const agent = new AgentEntity({ status: 'busy', currentTask: 'task-001' });

      await agent.completeTask({ success: true, latency: 100 });

      expect(agent.getStatus()).toBe('idle');
      expect(agent.getCurrentTask()).toBeUndefined();
      expect(agent.getMetrics().tasksCompleted).toBe(1);
      expect(agent.getMetrics().averageLatency).toBe(100);
    });

    it('should track error count on failed completion', async () => {
      const agent = new AgentEntity({ status: 'busy', currentTask: 'task-001' });

      await agent.completeTask({ success: false, latency: 50 });

      expect(agent.getMetrics().errorCount).toBe(1);
      expect(agent.getMetrics().tasksCompleted).toBe(1);
    });

    it('should calculate average latency correctly', async () => {
      const agent = new AgentEntity({ status: 'idle' });

      // First task
      await agent.assignTask('task-1');
      await agent.completeTask({ success: true, latency: 100 });

      // Second task
      await agent.assignTask('task-2');
      await agent.completeTask({ success: true, latency: 200 });

      expect(agent.getMetrics().averageLatency).toBe(150);
    });

    it('should fail task and enter error state', async () => {
      const agent = new AgentEntity({ status: 'busy', currentTask: 'task-001' });

      await agent.failTask(new Error('Task execution failed'));

      expect(agent.getStatus()).toBe('error');
      expect(agent.getCurrentTask()).toBeUndefined();
      expect(agent.getMetrics().errorCount).toBe(1);
    });

    it('should throw error when completing non-existent task', async () => {
      const agent = new AgentEntity({ status: 'idle' });

      await expect(agent.completeTask({ success: true, latency: 100 }))
        .rejects.toThrow('no active task');
    });
  });

  describe('Recovery', () => {
    it('should recover from error state', async () => {
      const agent = new AgentEntity({ status: 'error' });

      await agent.recover();

      expect(agent.getStatus()).toBe('idle');
      expect(agent.isAvailable()).toBe(true);
    });

    it('should throw error when recovering from non-error state', async () => {
      const agent = new AgentEntity({ status: 'idle' });

      await expect(agent.recover()).rejects.toThrow('not in error state');
    });
  });

  describe('Termination', () => {
    it('should terminate agent', async () => {
      const agent = new AgentEntity({ status: 'idle' });

      await agent.terminate();

      expect(agent.getStatus()).toBe('terminated');
      expect(agent.isAvailable()).toBe(false);
    });

    it('should terminate busy agent and clear task', async () => {
      const agent = new AgentEntity({ status: 'busy', currentTask: 'task-001' });

      await agent.terminate();

      expect(agent.getStatus()).toBe('terminated');
      expect(agent.getCurrentTask()).toBeUndefined();
    });
  });

  describe('Configuration', () => {
    it('should update config partially', () => {
      const agent = new AgentEntity({
        config: {
          model: 'claude-sonnet-4',
          temperature: 0.7,
          maxTokens: 4096
        }
      });

      agent.updateConfig({ temperature: 0.5 });

      const config = agent.getConfig();
      expect(config.temperature).toBe(0.5);
      expect(config.model).toBe('claude-sonnet-4');
      expect(config.maxTokens).toBe(4096);
    });
  });

  describe('Event Logging', () => {
    it('should log events during lifecycle', async () => {
      const agent = new AgentEntity({ status: 'idle' });

      await agent.assignTask('task-001');
      await agent.completeTask({ success: true, latency: 100 });

      const events = agent.getEventLog();
      expect(events).toHaveLength(2);
      expect(events[0].type).toBe('task_assigned');
      expect(events[1].type).toBe('task_completed');
    });

    it('should log configuration changes', () => {
      const agent = new AgentEntity({});

      agent.updateConfig({ temperature: 0.5 });
      agent.addCapability('new-cap');

      const events = agent.getEventLog();
      expect(events.some(e => e.type === 'config_updated')).toBe(true);
      expect(events.some(e => e.type === 'capability_added')).toBe(true);
    });
  });

  describe('Serialization', () => {
    it('should serialize to JSON', () => {
      const agent = new AgentEntity({
        id: 'test-agent',
        name: 'Test Agent',
        type: 'coder',
        capabilities: ['code-generation']
      });

      const json = agent.toJSON();

      expect(json.id).toBe('test-agent');
      expect(json.name).toBe('Test Agent');
      expect(json.type).toBe('coder');
      expect(json.capabilities).toEqual(['code-generation']);
    });
  });
});

describe('Agent Factory Integration', () => {
  it('should create agent from factory data', () => {
    const factoryAgent = createAgent({
      name: 'Factory Agent',
      type: 'tester',
      capabilities: ['test-generation']
    });

    const agent = new AgentEntity({
      id: factoryAgent.id,
      name: factoryAgent.name,
      type: factoryAgent.type,
      capabilities: factoryAgent.capabilities,
      config: factoryAgent.config
    });

    expect(agent.getId()).toBe(factoryAgent.id);
    expect(agent.getName()).toBe('Factory Agent');
    expect(agent.getType()).toBe('tester');
  });

  it('should create multiple agents from factory', () => {
    const agents = createAgents(5);

    const agentEntities = agents.map(a => new AgentEntity({
      id: a.id,
      name: a.name,
      type: a.type
    }));

    expect(agentEntities).toHaveLength(5);
    agentEntities.forEach((agent, i) => {
      expect(agent.getName()).toBe(`Agent ${i + 1}`);
    });
  });
});
