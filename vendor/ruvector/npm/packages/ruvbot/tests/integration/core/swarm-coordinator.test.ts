/**
 * SwarmCoordinator Integration Tests
 *
 * Tests the multi-agent swarm orchestration system including
 * agent spawning, task dispatch, coordination, and lifecycle management.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  SwarmCoordinator,
  createSwarmCoordinator,
  WORKER_DEFAULTS,
  type SwarmConfig,
  type SwarmAgent,
  type SwarmTask,
  type WorkerType,
} from '../../../src/swarm/SwarmCoordinator.js';

describe('SwarmCoordinator Integration Tests', () => {
  let coordinator: SwarmCoordinator;

  beforeEach(() => {
    coordinator = createSwarmCoordinator({
      topology: 'hierarchical',
      maxAgents: 8,
      strategy: 'specialized',
      consensus: 'raft',
      heartbeatInterval: 1000,
      taskTimeout: 5000,
    });
  });

  afterEach(async () => {
    await coordinator.stop();
  });

  describe('Coordinator Lifecycle', () => {
    it('should start the coordinator', async () => {
      const startedPromise = new Promise<void>(resolve => {
        coordinator.once('started', resolve);
      });

      await coordinator.start();
      await startedPromise;

      // Should be running
      const status = coordinator.getStatus();
      expect(status.topology).toBe('hierarchical');
    });

    it('should stop the coordinator', async () => {
      await coordinator.start();

      const stoppedPromise = new Promise<void>(resolve => {
        coordinator.once('stopped', resolve);
      });

      await coordinator.stop();
      await stoppedPromise;
    });

    it('should handle multiple start calls gracefully', async () => {
      await coordinator.start();
      await coordinator.start(); // Should be idempotent

      const status = coordinator.getStatus();
      expect(status.agentCount).toBe(0);
    });

    it('should handle multiple stop calls gracefully', async () => {
      await coordinator.start();
      await coordinator.stop();
      await coordinator.stop(); // Should be idempotent
    });
  });

  describe('Agent Management', () => {
    beforeEach(async () => {
      await coordinator.start();
    });

    it('should spawn an agent', async () => {
      const spawnedPromise = new Promise<SwarmAgent>(resolve => {
        coordinator.once('agent:spawned', resolve);
      });

      const agent = await coordinator.spawnAgent('coder' as WorkerType);
      const spawnedAgent = await spawnedPromise;

      expect(agent.id).toBeDefined();
      expect(agent.type).toBe('coder');
      expect(agent.status).toBe('idle');
      expect(agent.completedTasks).toBe(0);
      expect(agent.failedTasks).toBe(0);
      expect(spawnedAgent.id).toBe(agent.id);
    });

    it('should spawn multiple agents', async () => {
      const agents: SwarmAgent[] = [];
      agents.push(await coordinator.spawnAgent('optimize'));
      agents.push(await coordinator.spawnAgent('audit'));
      agents.push(await coordinator.spawnAgent('testgaps'));

      const status = coordinator.getStatus();
      expect(status.agentCount).toBe(3);
      expect(status.idleAgents).toBe(3);
    });

    it('should enforce max agents limit', async () => {
      const smallCoordinator = createSwarmCoordinator({ maxAgents: 2 });
      await smallCoordinator.start();

      await smallCoordinator.spawnAgent('optimize');
      await smallCoordinator.spawnAgent('audit');

      await expect(smallCoordinator.spawnAgent('map')).rejects.toThrow('Max agents');

      await smallCoordinator.stop();
    });

    it('should remove an agent', async () => {
      const agent = await coordinator.spawnAgent('optimize');
      expect(coordinator.getStatus().agentCount).toBe(1);

      const removedPromise = new Promise<SwarmAgent>(resolve => {
        coordinator.once('agent:removed', resolve);
      });

      const removed = await coordinator.removeAgent(agent.id);
      const removedAgent = await removedPromise;

      expect(removed).toBe(true);
      expect(removedAgent.id).toBe(agent.id);
      expect(coordinator.getStatus().agentCount).toBe(0);
    });

    it('should return false when removing non-existent agent', async () => {
      const removed = await coordinator.removeAgent('non-existent-id');
      expect(removed).toBe(false);
    });

    it('should get agent by ID', async () => {
      const agent = await coordinator.spawnAgent('optimize');
      const retrieved = coordinator.getAgent(agent.id);

      expect(retrieved).toBeDefined();
      expect(retrieved?.id).toBe(agent.id);
    });

    it('should get all agents', async () => {
      await coordinator.spawnAgent('optimize');
      await coordinator.spawnAgent('audit');
      await coordinator.spawnAgent('map');

      const agents = coordinator.getAgents();
      expect(agents.length).toBe(3);
    });
  });

  describe('Task Dispatch', () => {
    beforeEach(async () => {
      await coordinator.start();
    });

    it('should dispatch a task', async () => {
      const createdPromise = new Promise<SwarmTask>(resolve => {
        coordinator.once('task:created', resolve);
      });

      const task = await coordinator.dispatch({
        worker: 'optimize',
        task: {
          type: 'performance-analysis',
          content: { target: 'api-endpoint' },
        },
      });

      const createdTask = await createdPromise;

      expect(task.id).toBeDefined();
      expect(task.worker).toBe('optimize');
      expect(task.type).toBe('performance-analysis');
      expect(task.status).toBe('pending');
      expect(createdTask.id).toBe(task.id);
    });

    it('should assign task to idle agent of matching type', async () => {
      await coordinator.spawnAgent('optimize');

      const assignedPromise = new Promise<{ task: SwarmTask; agent: SwarmAgent }>(resolve => {
        coordinator.once('task:assigned', resolve);
      });

      const task = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'optimize-query', content: {} },
      });

      const { task: assignedTask, agent } = await assignedPromise;

      expect(assignedTask.id).toBe(task.id);
      expect(assignedTask.status).toBe('running');
      expect(assignedTask.assignedAgent).toBe(agent.id);
      expect(agent.status).toBe('busy');
    });

    it('should queue task when no matching agent available', async () => {
      await coordinator.spawnAgent('audit'); // Wrong type

      const task = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'optimize-query', content: {} },
      });

      expect(task.status).toBe('pending');
      expect(task.assignedAgent).toBeUndefined();
    });

    it('should respect task priority', async () => {
      const task1 = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'low-priority', content: {} },
        priority: 'low',
      });

      const task2 = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'critical', content: {} },
        priority: 'critical',
      });

      expect(task1.priority).toBe('low');
      expect(task2.priority).toBe('critical');
    });

    it('should get task by ID', async () => {
      const task = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'test', content: {} },
      });

      const retrieved = coordinator.getTask(task.id);
      expect(retrieved).toBeDefined();
      expect(retrieved?.id).toBe(task.id);
    });

    it('should get all tasks', async () => {
      await coordinator.dispatch({ worker: 'optimize', task: { type: 'task1', content: {} } });
      await coordinator.dispatch({ worker: 'audit', task: { type: 'task2', content: {} } });
      await coordinator.dispatch({ worker: 'map', task: { type: 'task3', content: {} } });

      const tasks = coordinator.getTasks();
      expect(tasks.length).toBe(3);
    });
  });

  describe('Task Completion', () => {
    beforeEach(async () => {
      await coordinator.start();
    });

    it('should complete a task successfully', async () => {
      const agent = await coordinator.spawnAgent('optimize');

      const task = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'test', content: {} },
      });

      // Wait for assignment
      await new Promise(resolve => setTimeout(resolve, 50));

      const completedPromise = new Promise<SwarmTask>(resolve => {
        coordinator.once('task:completed', resolve);
      });

      coordinator.completeTask(task.id, { result: 'success' });

      const completedTask = await completedPromise;

      expect(completedTask.status).toBe('completed');
      expect(completedTask.result).toEqual({ result: 'success' });
      expect(completedTask.completedAt).toBeDefined();

      // Agent should be idle again
      const updatedAgent = coordinator.getAgent(agent.id);
      expect(updatedAgent?.status).toBe('idle');
      expect(updatedAgent?.completedTasks).toBe(1);
    });

    it('should handle task failure', async () => {
      const agent = await coordinator.spawnAgent('optimize');

      const task = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'test', content: {} },
      });

      await new Promise(resolve => setTimeout(resolve, 50));

      const failedPromise = new Promise<SwarmTask>(resolve => {
        coordinator.once('task:failed', resolve);
      });

      coordinator.completeTask(task.id, undefined, 'Something went wrong');

      const failedTask = await failedPromise;

      expect(failedTask.status).toBe('failed');
      expect(failedTask.error).toBe('Something went wrong');

      const updatedAgent = coordinator.getAgent(agent.id);
      expect(updatedAgent?.failedTasks).toBe(1);
    });

    it('should assign pending task after agent completes', async () => {
      const agent = await coordinator.spawnAgent('optimize');

      // Dispatch first task
      const task1 = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'task1', content: {} },
      });

      await new Promise(resolve => setTimeout(resolve, 50));

      // Dispatch second task (should queue)
      const task2 = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'task2', content: {} },
      });

      expect(coordinator.getTask(task2.id)?.status).toBe('pending');

      // Complete first task
      coordinator.completeTask(task1.id, { done: true });

      await new Promise(resolve => setTimeout(resolve, 50));

      // Second task should now be running
      const updatedTask2 = coordinator.getTask(task2.id);
      expect(updatedTask2?.status).toBe('running');
    });
  });

  describe('Wait for Task', () => {
    beforeEach(async () => {
      await coordinator.start();
    });

    it('should wait for task completion', async () => {
      const agent = await coordinator.spawnAgent('optimize');

      const task = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'async-task', content: {} },
      });

      // Complete after delay
      setTimeout(() => {
        coordinator.completeTask(task.id, { value: 42 });
      }, 100);

      const completedTask = await coordinator.waitForTask(task.id);

      expect(completedTask.status).toBe('completed');
      expect(completedTask.result).toEqual({ value: 42 });
    });

    it('should timeout waiting for task', async () => {
      const task = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'slow-task', content: {} },
      });

      await expect(coordinator.waitForTask(task.id, 100)).rejects.toThrow('timed out');
    });

    it('should reject when task not found', async () => {
      await expect(coordinator.waitForTask('non-existent')).rejects.toThrow('not found');
    });
  });

  describe('Heartbeat Monitoring', () => {
    it('should track agent heartbeats', async () => {
      await coordinator.start();
      const agent = await coordinator.spawnAgent('optimize');

      const initialHeartbeat = agent.lastHeartbeat;

      await new Promise(resolve => setTimeout(resolve, 50));

      coordinator.heartbeat(agent.id);

      const updatedAgent = coordinator.getAgent(agent.id);
      expect(updatedAgent?.lastHeartbeat.getTime()).toBeGreaterThan(initialHeartbeat.getTime());
    });

    it('should mark agent offline after missed heartbeats', async () => {
      const fastCoordinator = createSwarmCoordinator({
        heartbeatInterval: 50, // Very fast for testing
      });
      await fastCoordinator.start();

      const agent = await fastCoordinator.spawnAgent('optimize');

      const offlinePromise = new Promise<SwarmAgent>(resolve => {
        fastCoordinator.once('agent:offline', resolve);
      });

      // Don't send heartbeats, wait for timeout
      const offlineAgent = await offlinePromise;

      expect(offlineAgent.id).toBe(agent.id);
      expect(offlineAgent.status).toBe('offline');

      await fastCoordinator.stop();
    });

    it('should re-queue running task when agent goes offline', async () => {
      const fastCoordinator = createSwarmCoordinator({
        heartbeatInterval: 50,
      });
      await fastCoordinator.start();

      const agent = await fastCoordinator.spawnAgent('optimize');

      const task = await fastCoordinator.dispatch({
        worker: 'optimize',
        task: { type: 'long-running', content: {} },
      });

      await new Promise(resolve => setTimeout(resolve, 50));
      expect(fastCoordinator.getTask(task.id)?.status).toBe('running');

      // Wait for agent to go offline
      await new Promise(resolve => setTimeout(resolve, 200));

      // Task should be re-queued
      const updatedTask = fastCoordinator.getTask(task.id);
      expect(updatedTask?.status).toBe('pending');
      expect(updatedTask?.assignedAgent).toBeUndefined();

      await fastCoordinator.stop();
    });
  });

  describe('Swarm Status', () => {
    beforeEach(async () => {
      await coordinator.start();
    });

    it('should return accurate status', async () => {
      await coordinator.spawnAgent('optimize');
      await coordinator.spawnAgent('audit');

      const task = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'test', content: {} },
      });

      await new Promise(resolve => setTimeout(resolve, 50));

      const status = coordinator.getStatus();

      expect(status.topology).toBe('hierarchical');
      expect(status.consensus).toBe('raft');
      expect(status.agentCount).toBe(2);
      expect(status.maxAgents).toBe(8);
      expect(status.idleAgents).toBe(1);
      expect(status.busyAgents).toBe(1);
      expect(status.runningTasks).toBe(1);
    });

    it('should track completed and failed task counts', async () => {
      const agent = await coordinator.spawnAgent('optimize');

      const task1 = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'success', content: {} },
      });

      await new Promise(resolve => setTimeout(resolve, 50));
      coordinator.completeTask(task1.id, { done: true });
      await new Promise(resolve => setTimeout(resolve, 50));

      const task2 = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'failure', content: {} },
      });

      await new Promise(resolve => setTimeout(resolve, 50));
      coordinator.completeTask(task2.id, undefined, 'error');
      await new Promise(resolve => setTimeout(resolve, 50));

      const status = coordinator.getStatus();
      expect(status.completedTasks).toBe(1);
      expect(status.failedTasks).toBe(1);
    });
  });

  describe('Specialized Strategy', () => {
    it('should only assign tasks to matching agent types', async () => {
      const specializedCoordinator = createSwarmCoordinator({
        strategy: 'specialized',
      });
      await specializedCoordinator.start();

      const optimizeAgent = await specializedCoordinator.spawnAgent('optimize');
      const auditAgent = await specializedCoordinator.spawnAgent('audit');

      const optimizeTask = await specializedCoordinator.dispatch({
        worker: 'optimize',
        task: { type: 'optimize-task', content: {} },
      });

      await new Promise(resolve => setTimeout(resolve, 50));

      expect(optimizeTask.assignedAgent).toBe(optimizeAgent.id);

      await specializedCoordinator.stop();
    });
  });

  describe('Balanced Strategy', () => {
    it('should assign tasks to any available agent', async () => {
      const balancedCoordinator = createSwarmCoordinator({
        strategy: 'balanced',
      });
      await balancedCoordinator.start();

      const auditAgent = await balancedCoordinator.spawnAgent('audit');

      const optimizeTask = await balancedCoordinator.dispatch({
        worker: 'optimize',
        task: { type: 'optimize-task', content: {} },
      });

      await new Promise(resolve => setTimeout(resolve, 50));

      // With balanced strategy, audit agent should take optimize task
      expect(optimizeTask.assignedAgent).toBe(auditAgent.id);

      await balancedCoordinator.stop();
    });
  });

  describe('Priority Queue', () => {
    beforeEach(async () => {
      await coordinator.start();
    });

    it('should process critical tasks before others', async () => {
      // Dispatch tasks in low-to-high priority order
      const lowTask = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'low', content: {} },
        priority: 'low',
      });

      const normalTask = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'normal', content: {} },
        priority: 'normal',
      });

      const criticalTask = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'critical', content: {} },
        priority: 'critical',
      });

      // Now spawn agent - critical should be picked first
      await coordinator.spawnAgent('optimize');

      await new Promise(resolve => setTimeout(resolve, 50));

      expect(coordinator.getTask(criticalTask.id)?.status).toBe('running');
      expect(coordinator.getTask(normalTask.id)?.status).toBe('pending');
      expect(coordinator.getTask(lowTask.id)?.status).toBe('pending');
    });
  });

  describe('Worker Defaults', () => {
    it('should have correct defaults for all worker types', () => {
      const workerTypes: WorkerType[] = [
        'ultralearn', 'optimize', 'consolidate', 'predict', 'audit',
        'map', 'preload', 'deepdive', 'document', 'refactor',
        'benchmark', 'testgaps',
      ];

      for (const type of workerTypes) {
        const config = WORKER_DEFAULTS[type];
        expect(config).toBeDefined();
        expect(config.type).toBe(type);
        expect(config.priority).toBeDefined();
        expect(config.concurrency).toBeGreaterThan(0);
        expect(config.timeout).toBeGreaterThan(0);
        expect(config.retries).toBeGreaterThanOrEqual(0);
        expect(['exponential', 'linear']).toContain(config.backoff);
      }
    });
  });

  describe('Event Emission', () => {
    beforeEach(async () => {
      await coordinator.start();
    });

    it('should emit all expected events', async () => {
      const events: string[] = [];

      coordinator.on('agent:spawned', () => events.push('agent:spawned'));
      coordinator.on('agent:removed', () => events.push('agent:removed'));
      coordinator.on('task:created', () => events.push('task:created'));
      coordinator.on('task:assigned', () => events.push('task:assigned'));
      coordinator.on('task:completed', () => events.push('task:completed'));

      const agent = await coordinator.spawnAgent('optimize');

      const task = await coordinator.dispatch({
        worker: 'optimize',
        task: { type: 'test', content: {} },
      });

      await new Promise(resolve => setTimeout(resolve, 50));

      coordinator.completeTask(task.id, { done: true });

      await new Promise(resolve => setTimeout(resolve, 50));

      await coordinator.removeAgent(agent.id);

      expect(events).toContain('agent:spawned');
      expect(events).toContain('task:created');
      expect(events).toContain('task:assigned');
      expect(events).toContain('task:completed');
      expect(events).toContain('agent:removed');
    });
  });
});
