/**
 * Long-running Tasks - E2E Tests
 *
 * End-to-end tests for long-running task completion
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { createAgent, createSession } from '../../factories';
import { createMockSlackApp, type MockSlackBoltApp } from '../../mocks/slack.mock';

// Task types
interface Task {
  id: string;
  type: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  progress: number;
  result?: unknown;
  error?: string;
  startedAt?: Date;
  completedAt?: Date;
}

// Mock Task Manager
class MockTaskManager {
  private tasks: Map<string, Task> = new Map();
  private eventHandlers: Map<string, Array<(task: Task) => void>> = new Map();

  async createTask(type: string, payload: unknown): Promise<Task> {
    const task: Task = {
      id: `task-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      type,
      status: 'pending',
      progress: 0
    };

    this.tasks.set(task.id, task);
    this.emit('created', task);

    return task;
  }

  async startTask(taskId: string): Promise<void> {
    const task = this.tasks.get(taskId);
    if (!task) throw new Error(`Task ${taskId} not found`);

    task.status = 'running';
    task.startedAt = new Date();
    this.emit('started', task);
  }

  async updateProgress(taskId: string, progress: number): Promise<void> {
    const task = this.tasks.get(taskId);
    if (!task) throw new Error(`Task ${taskId} not found`);

    task.progress = progress;
    this.emit('progress', task);
  }

  async completeTask(taskId: string, result: unknown): Promise<void> {
    const task = this.tasks.get(taskId);
    if (!task) throw new Error(`Task ${taskId} not found`);

    task.status = 'completed';
    task.progress = 100;
    task.result = result;
    task.completedAt = new Date();
    this.emit('completed', task);
  }

  async failTask(taskId: string, error: string): Promise<void> {
    const task = this.tasks.get(taskId);
    if (!task) throw new Error(`Task ${taskId} not found`);

    task.status = 'failed';
    task.error = error;
    task.completedAt = new Date();
    this.emit('failed', task);
  }

  getTask(taskId: string): Task | undefined {
    return this.tasks.get(taskId);
  }

  on(event: string, handler: (task: Task) => void): void {
    const handlers = this.eventHandlers.get(event) || [];
    handlers.push(handler);
    this.eventHandlers.set(event, handlers);
  }

  private emit(event: string, task: Task): void {
    const handlers = this.eventHandlers.get(event) || [];
    handlers.forEach(h => h(task));
  }

  // Simulate long-running task execution
  async executeTask(taskId: string, duration: number, steps: number): Promise<void> {
    await this.startTask(taskId);

    const stepDuration = duration / steps;

    for (let i = 1; i <= steps; i++) {
      await new Promise(resolve => setTimeout(resolve, stepDuration));
      await this.updateProgress(taskId, (i / steps) * 100);
    }

    await this.completeTask(taskId, { message: 'Task completed successfully' });
  }
}

// Mock Orchestrator for E2E testing
class MockTaskOrchestrator {
  private app: MockSlackBoltApp;
  private taskManager: MockTaskManager;
  private activeTasks: Map<string, { channel: string; threadTs: string }> = new Map();

  constructor() {
    this.app = createMockSlackApp();
    this.taskManager = new MockTaskManager();
    this.setupHandlers();
    this.setupTaskEvents();
  }

  getApp(): MockSlackBoltApp {
    return this.app;
  }

  getTaskManager(): MockTaskManager {
    return this.taskManager;
  }

  async processMessage(message: {
    text: string;
    channel: string;
    user: string;
    ts: string;
    thread_ts?: string;
  }): Promise<void> {
    await this.app.processMessage(message);
  }

  private setupHandlers(): void {
    // Handle long task requests
    this.app.message(/run.*long.*task|execute.*batch/i, async ({ message, say }) => {
      const task = await this.taskManager.createTask('long-running', {
        request: (message as any).text
      });

      this.activeTasks.set(task.id, {
        channel: (message as any).channel,
        threadTs: (message as any).ts
      });

      await say({
        channel: (message as any).channel,
        text: `Starting task ${task.id}. I'll update you on progress...`,
        thread_ts: (message as any).ts
      });

      // Execute task in background
      this.taskManager.executeTask(task.id, 500, 5);
    });

    // Handle analysis requests
    this.app.message(/analyze|process.*data/i, async ({ message, say }) => {
      const task = await this.taskManager.createTask('analysis', {
        request: (message as any).text
      });

      this.activeTasks.set(task.id, {
        channel: (message as any).channel,
        threadTs: (message as any).ts
      });

      await say({
        channel: (message as any).channel,
        text: `Beginning analysis (Task: ${task.id})...`,
        thread_ts: (message as any).ts
      });

      // Execute analysis task
      this.taskManager.executeTask(task.id, 300, 3);
    });

    // Handle code refactoring requests
    this.app.message(/refactor.*code|rewrite/i, async ({ message, say }) => {
      const task = await this.taskManager.createTask('refactoring', {
        request: (message as any).text
      });

      this.activeTasks.set(task.id, {
        channel: (message as any).channel,
        threadTs: (message as any).ts
      });

      await say({
        channel: (message as any).channel,
        text: `Starting code refactoring (Task: ${task.id}). This may take a while...`,
        thread_ts: (message as any).ts
      });

      // Execute refactoring task
      this.taskManager.executeTask(task.id, 800, 8);
    });
  }

  private setupTaskEvents(): void {
    this.taskManager.on('progress', async (task) => {
      const context = this.activeTasks.get(task.id);
      if (!context) return;

      // Only send updates at 25%, 50%, 75%
      if ([25, 50, 75].includes(task.progress)) {
        await this.app.client.chat.postMessage({
          channel: context.channel,
          text: `Task ${task.id} progress: ${task.progress}%`,
          thread_ts: context.threadTs
        });
      }
    });

    this.taskManager.on('completed', async (task) => {
      const context = this.activeTasks.get(task.id);
      if (!context) return;

      const duration = task.completedAt!.getTime() - task.startedAt!.getTime();

      await this.app.client.chat.postMessage({
        channel: context.channel,
        text: `Task ${task.id} completed successfully in ${duration}ms!`,
        thread_ts: context.threadTs
      });

      this.activeTasks.delete(task.id);
    });

    this.taskManager.on('failed', async (task) => {
      const context = this.activeTasks.get(task.id);
      if (!context) return;

      await this.app.client.chat.postMessage({
        channel: context.channel,
        text: `Task ${task.id} failed: ${task.error}`,
        thread_ts: context.threadTs
      });

      this.activeTasks.delete(task.id);
    });
  }
}

describe('E2E: Long-running Tasks', () => {
  let orchestrator: MockTaskOrchestrator;

  beforeEach(() => {
    orchestrator = new MockTaskOrchestrator();
  });

  describe('Task Execution', () => {
    it('should start and complete long-running task', async () => {
      await orchestrator.processMessage({
        text: 'Run a long task for me',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      // Wait for task to complete
      await new Promise(resolve => setTimeout(resolve, 600));

      const messages = orchestrator.getApp().client.getMessageLog();

      // Should have start message, progress updates, and completion
      expect(messages.some(m => m.text?.includes('Starting task'))).toBe(true);
      expect(messages.some(m => m.text?.includes('completed'))).toBe(true);
    });

    it('should send progress updates', async () => {
      await orchestrator.processMessage({
        text: 'Run a long task',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      // Wait for task to complete
      await new Promise(resolve => setTimeout(resolve, 600));

      const messages = orchestrator.getApp().client.getMessageLog();
      const progressMessages = messages.filter(m => m.text?.includes('progress'));

      // Should have multiple progress updates
      expect(progressMessages.length).toBeGreaterThan(0);
    });

    it('should report completion time', async () => {
      await orchestrator.processMessage({
        text: 'Execute batch process',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      await new Promise(resolve => setTimeout(resolve, 600));

      const messages = orchestrator.getApp().client.getMessageLog();
      const completionMessage = messages.find(m => m.text?.includes('completed'));

      expect(completionMessage?.text).toMatch(/\d+ms/);
    });
  });

  describe('Multiple Concurrent Tasks', () => {
    it('should handle multiple tasks concurrently', async () => {
      // Start multiple tasks
      await orchestrator.processMessage({
        text: 'Run a long task',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.111111'
      });

      await orchestrator.processMessage({
        text: 'Analyze this data',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.222222'
      });

      // Wait for both to complete
      await new Promise(resolve => setTimeout(resolve, 700));

      const messages = orchestrator.getApp().client.getMessageLog();
      const completedMessages = messages.filter(m => m.text?.includes('completed'));

      expect(completedMessages.length).toBe(2);
    });

    it('should track tasks independently', async () => {
      await orchestrator.processMessage({
        text: 'Run a long task',
        channel: 'C11111111',
        user: 'U12345678',
        ts: '1234567890.111111'
      });

      await orchestrator.processMessage({
        text: 'Process data',
        channel: 'C22222222',
        user: 'U12345678',
        ts: '1234567890.222222'
      });

      await new Promise(resolve => setTimeout(resolve, 700));

      const messages = orchestrator.getApp().client.getMessageLog();

      // Each channel should have its own completion message
      const channel1Completed = messages.some(
        m => m.channel === 'C11111111' && m.text?.includes('completed')
      );
      const channel2Completed = messages.some(
        m => m.channel === 'C22222222' && m.text?.includes('completed')
      );

      expect(channel1Completed).toBe(true);
      expect(channel2Completed).toBe(true);
    });
  });

  describe('Task Types', () => {
    it('should handle analysis tasks', async () => {
      await orchestrator.processMessage({
        text: 'Analyze the codebase',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      await new Promise(resolve => setTimeout(resolve, 400));

      const messages = orchestrator.getApp().client.getMessageLog();
      expect(messages.some(m => m.text?.includes('analysis'))).toBe(true);
    });

    it('should handle refactoring tasks', async () => {
      await orchestrator.processMessage({
        text: 'Refactor the code in src/main.ts',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      await new Promise(resolve => setTimeout(resolve, 900));

      const messages = orchestrator.getApp().client.getMessageLog();
      expect(messages.some(m => m.text?.includes('refactoring'))).toBe(true);
    });
  });
});

describe('E2E: Task Manager', () => {
  let taskManager: MockTaskManager;

  beforeEach(() => {
    taskManager = new MockTaskManager();
  });

  describe('Task Lifecycle', () => {
    it('should create task in pending state', async () => {
      const task = await taskManager.createTask('test', {});

      expect(task.status).toBe('pending');
      expect(task.progress).toBe(0);
    });

    it('should transition through states correctly', async () => {
      const states: string[] = [];

      taskManager.on('created', (t) => states.push(t.status));
      taskManager.on('started', (t) => states.push(t.status));
      taskManager.on('completed', (t) => states.push(t.status));

      const task = await taskManager.createTask('test', {});
      await taskManager.executeTask(task.id, 100, 2);

      expect(states).toEqual(['pending', 'running', 'completed']);
    });

    it('should track progress correctly', async () => {
      const progressValues: number[] = [];

      taskManager.on('progress', (t) => progressValues.push(t.progress));

      const task = await taskManager.createTask('test', {});
      await taskManager.executeTask(task.id, 100, 4);

      expect(progressValues).toEqual([25, 50, 75, 100]);
    });

    it('should record timing information', async () => {
      const task = await taskManager.createTask('test', {});
      await taskManager.executeTask(task.id, 100, 2);

      const completed = taskManager.getTask(task.id)!;

      expect(completed.startedAt).toBeDefined();
      expect(completed.completedAt).toBeDefined();
      expect(completed.completedAt!.getTime()).toBeGreaterThan(completed.startedAt!.getTime());
    });
  });

  describe('Task Failure', () => {
    it('should handle task failure', async () => {
      const task = await taskManager.createTask('test', {});
      await taskManager.startTask(task.id);
      await taskManager.failTask(task.id, 'Something went wrong');

      const failed = taskManager.getTask(task.id)!;

      expect(failed.status).toBe('failed');
      expect(failed.error).toBe('Something went wrong');
    });
  });
});
