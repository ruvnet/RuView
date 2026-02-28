/**
 * Background Workers - Unit Tests
 *
 * Tests for background job processing, scheduling, and lifecycle
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// Worker Types
interface Job {
  id: string;
  type: string;
  payload: unknown;
  priority: 'low' | 'normal' | 'high' | 'critical';
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  attempts: number;
  maxAttempts: number;
  createdAt: Date;
  startedAt?: Date;
  completedAt?: Date;
  error?: string;
  result?: unknown;
}

interface WorkerConfig {
  concurrency: number;
  pollInterval: number;
  maxJobDuration: number;
  retryDelay: number;
}

type JobHandler = (job: Job) => Promise<unknown>;

// Mock Worker Queue for testing
class WorkerQueue {
  private jobs: Map<string, Job> = new Map();
  private handlers: Map<string, JobHandler> = new Map();
  private running: Map<string, Promise<void>> = new Map();
  private config: WorkerConfig;
  private isProcessing: boolean = false;
  private processInterval?: NodeJS.Timeout;
  private eventHandlers: Map<string, Array<(event: unknown) => void>> = new Map();

  constructor(config: Partial<WorkerConfig> = {}) {
    this.config = {
      concurrency: config.concurrency ?? 3,
      pollInterval: config.pollInterval ?? 100,
      maxJobDuration: config.maxJobDuration ?? 30000,
      retryDelay: config.retryDelay ?? 1000
    };
  }

  registerHandler(type: string, handler: JobHandler): void {
    this.handlers.set(type, handler);
  }

  async enqueue(
    type: string,
    payload: unknown,
    options: Partial<Pick<Job, 'priority' | 'maxAttempts'>> = {}
  ): Promise<Job> {
    const job: Job = {
      id: `job-${Date.now()}-${Math.random().toString(36).slice(2)}`,
      type,
      payload,
      priority: options.priority ?? 'normal',
      status: 'pending',
      attempts: 0,
      maxAttempts: options.maxAttempts ?? 3,
      createdAt: new Date()
    };

    this.jobs.set(job.id, job);
    this.emit('enqueued', job);

    return job;
  }

  async getJob(id: string): Promise<Job | null> {
    return this.jobs.get(id) || null;
  }

  async cancelJob(id: string): Promise<boolean> {
    const job = this.jobs.get(id);
    if (!job) return false;

    if (job.status === 'pending') {
      job.status = 'cancelled';
      this.emit('cancelled', job);
      return true;
    }

    return false;
  }

  async retryJob(id: string): Promise<boolean> {
    const job = this.jobs.get(id);
    if (!job) return false;

    if (job.status === 'failed') {
      job.status = 'pending';
      job.attempts = 0;
      job.error = undefined;
      this.emit('retried', job);
      return true;
    }

    return false;
  }

  start(): void {
    if (this.isProcessing) return;

    this.isProcessing = true;
    this.processInterval = setInterval(() => this.processJobs(), this.config.pollInterval);
    this.emit('started', {});
  }

  stop(): void {
    if (!this.isProcessing) return;

    this.isProcessing = false;
    if (this.processInterval) {
      clearInterval(this.processInterval);
      this.processInterval = undefined;
    }
    this.emit('stopped', {});
  }

  async drain(): Promise<void> {
    // Wait for all running jobs to complete
    await Promise.all(this.running.values());
  }

  async flush(): Promise<number> {
    let count = 0;
    for (const [id, job] of this.jobs) {
      if (job.status === 'pending' || job.status === 'failed') {
        this.jobs.delete(id);
        count++;
      }
    }
    return count;
  }

  getStats(): {
    pending: number;
    running: number;
    completed: number;
    failed: number;
    cancelled: number;
  } {
    const stats = { pending: 0, running: 0, completed: 0, failed: 0, cancelled: 0 };

    for (const job of this.jobs.values()) {
      stats[job.status]++;
    }

    return stats;
  }

  getJobsByStatus(status: Job['status']): Job[] {
    return Array.from(this.jobs.values()).filter(j => j.status === status);
  }

  on(event: string, handler: (event: unknown) => void): void {
    const handlers = this.eventHandlers.get(event) || [];
    handlers.push(handler);
    this.eventHandlers.set(event, handlers);
  }

  off(event: string, handler: (event: unknown) => void): void {
    const handlers = this.eventHandlers.get(event) || [];
    this.eventHandlers.set(event, handlers.filter(h => h !== handler));
  }

  private emit(event: string, data: unknown): void {
    const handlers = this.eventHandlers.get(event) || [];
    handlers.forEach(h => h(data));
  }

  private async processJobs(): Promise<void> {
    if (this.running.size >= this.config.concurrency) return;

    const pendingJobs = this.getPendingJobs();
    const slotsAvailable = this.config.concurrency - this.running.size;

    for (let i = 0; i < Math.min(pendingJobs.length, slotsAvailable); i++) {
      const job = pendingJobs[i];
      this.processJob(job);
    }
  }

  private getPendingJobs(): Job[] {
    const priorityOrder = { critical: 0, high: 1, normal: 2, low: 3 };

    return Array.from(this.jobs.values())
      .filter(j => j.status === 'pending')
      .sort((a, b) => {
        // Sort by priority first, then by creation time
        const priorityDiff = priorityOrder[a.priority] - priorityOrder[b.priority];
        if (priorityDiff !== 0) return priorityDiff;
        return a.createdAt.getTime() - b.createdAt.getTime();
      });
  }

  private async processJob(job: Job): Promise<void> {
    const handler = this.handlers.get(job.type);
    if (!handler) {
      job.status = 'failed';
      job.error = `No handler registered for job type: ${job.type}`;
      this.emit('failed', job);
      return;
    }

    job.status = 'running';
    job.startedAt = new Date();
    job.attempts++;
    this.emit('started', job);

    const promise = this.executeJob(job, handler);
    this.running.set(job.id, promise);

    try {
      await promise;
    } finally {
      this.running.delete(job.id);
    }
  }

  private async executeJob(job: Job, handler: JobHandler): Promise<void> {
    try {
      const result = await Promise.race([
        handler(job),
        this.createTimeout(this.config.maxJobDuration)
      ]);

      job.status = 'completed';
      job.completedAt = new Date();
      job.result = result;
      this.emit('completed', job);
    } catch (error) {
      job.error = error instanceof Error ? error.message : 'Unknown error';

      if (job.attempts < job.maxAttempts) {
        job.status = 'pending';
        // Schedule retry after delay
        await new Promise(resolve => setTimeout(resolve, this.config.retryDelay));
      } else {
        job.status = 'failed';
        this.emit('failed', job);
      }
    }
  }

  private async createTimeout(ms: number): Promise<never> {
    return new Promise((_, reject) => {
      setTimeout(() => reject(new Error('Job timed out')), ms);
    });
  }
}

// Scheduled Worker for periodic tasks
class ScheduledWorker {
  private tasks: Map<string, {
    interval: number;
    handler: () => Promise<void>;
    timer?: NodeJS.Timeout;
    lastRun?: Date;
    isRunning: boolean;
  }> = new Map();
  private isActive: boolean = false;

  schedule(
    taskId: string,
    interval: number,
    handler: () => Promise<void>
  ): void {
    this.tasks.set(taskId, {
      interval,
      handler,
      isRunning: false
    });

    if (this.isActive) {
      this.startTask(taskId);
    }
  }

  unschedule(taskId: string): boolean {
    const task = this.tasks.get(taskId);
    if (!task) return false;

    if (task.timer) {
      clearInterval(task.timer);
    }
    return this.tasks.delete(taskId);
  }

  start(): void {
    if (this.isActive) return;
    this.isActive = true;

    for (const taskId of this.tasks.keys()) {
      this.startTask(taskId);
    }
  }

  stop(): void {
    if (!this.isActive) return;
    this.isActive = false;

    for (const [, task] of this.tasks) {
      if (task.timer) {
        clearInterval(task.timer);
        task.timer = undefined;
      }
    }
  }

  async runNow(taskId: string): Promise<void> {
    const task = this.tasks.get(taskId);
    if (!task) throw new Error(`Task ${taskId} not found`);

    if (task.isRunning) {
      throw new Error(`Task ${taskId} is already running`);
    }

    task.isRunning = true;
    try {
      await task.handler();
      task.lastRun = new Date();
    } finally {
      task.isRunning = false;
    }
  }

  getTaskInfo(taskId: string): {
    interval: number;
    lastRun?: Date;
    isRunning: boolean;
  } | null {
    const task = this.tasks.get(taskId);
    if (!task) return null;

    return {
      interval: task.interval,
      lastRun: task.lastRun,
      isRunning: task.isRunning
    };
  }

  listTasks(): string[] {
    return Array.from(this.tasks.keys());
  }

  private startTask(taskId: string): void {
    const task = this.tasks.get(taskId);
    if (!task) return;

    task.timer = setInterval(async () => {
      if (task.isRunning) return;

      task.isRunning = true;
      try {
        await task.handler();
        task.lastRun = new Date();
      } catch (error) {
        // Log error but don't stop the schedule
        console.error(`Scheduled task ${taskId} failed:`, error);
      } finally {
        task.isRunning = false;
      }
    }, task.interval);
  }
}

// Tests
describe('Worker Queue', () => {
  let queue: WorkerQueue;

  beforeEach(() => {
    queue = new WorkerQueue({
      concurrency: 2,
      pollInterval: 10,
      maxJobDuration: 5000,
      retryDelay: 50
    });
  });

  afterEach(() => {
    queue.stop();
  });

  describe('Job Enqueuing', () => {
    it('should enqueue job with default options', async () => {
      const job = await queue.enqueue('test-job', { data: 'test' });

      expect(job.id).toBeDefined();
      expect(job.type).toBe('test-job');
      expect(job.status).toBe('pending');
      expect(job.priority).toBe('normal');
      expect(job.attempts).toBe(0);
      expect(job.maxAttempts).toBe(3);
    });

    it('should enqueue job with custom options', async () => {
      const job = await queue.enqueue('urgent-job', { data: 'urgent' }, {
        priority: 'high',
        maxAttempts: 5
      });

      expect(job.priority).toBe('high');
      expect(job.maxAttempts).toBe(5);
    });

    it('should emit enqueued event', async () => {
      const handler = vi.fn();
      queue.on('enqueued', handler);

      await queue.enqueue('test-job', {});

      expect(handler).toHaveBeenCalled();
    });
  });

  describe('Job Retrieval', () => {
    it('should get job by ID', async () => {
      const created = await queue.enqueue('test', {});
      const retrieved = await queue.getJob(created.id);

      expect(retrieved).not.toBeNull();
      expect(retrieved?.id).toBe(created.id);
    });

    it('should return null for non-existent job', async () => {
      const job = await queue.getJob('non-existent');
      expect(job).toBeNull();
    });
  });

  describe('Job Processing', () => {
    it('should process jobs with registered handler', async () => {
      const handler = vi.fn().mockResolvedValue({ success: true });
      queue.registerHandler('test-job', handler);

      await queue.enqueue('test-job', { data: 'test' });
      queue.start();

      await new Promise(resolve => setTimeout(resolve, 50));

      expect(handler).toHaveBeenCalled();
    });

    it('should mark job as completed on success', async () => {
      queue.registerHandler('test-job', async () => ({ result: 'done' }));

      const job = await queue.enqueue('test-job', {});
      queue.start();

      await new Promise(resolve => setTimeout(resolve, 50));

      const updated = await queue.getJob(job.id);
      expect(updated?.status).toBe('completed');
      expect(updated?.result).toEqual({ result: 'done' });
    });

    it('should mark job as failed when no handler exists', async () => {
      const job = await queue.enqueue('unknown-job', {});
      queue.start();

      await new Promise(resolve => setTimeout(resolve, 50));

      const updated = await queue.getJob(job.id);
      expect(updated?.status).toBe('failed');
      expect(updated?.error).toContain('No handler registered');
    });

    it('should retry failed jobs', async () => {
      let attempts = 0;
      queue.registerHandler('flaky-job', async () => {
        attempts++;
        if (attempts < 2) throw new Error('Temporary failure');
        return { success: true };
      });

      const job = await queue.enqueue('flaky-job', {}, { maxAttempts: 3 });
      queue.start();

      await new Promise(resolve => setTimeout(resolve, 200));

      const updated = await queue.getJob(job.id);
      expect(updated?.status).toBe('completed');
      expect(attempts).toBe(2);
    });

    it('should mark job as failed after max attempts', async () => {
      queue.registerHandler('always-fail', async () => {
        throw new Error('Always fails');
      });

      const job = await queue.enqueue('always-fail', {}, { maxAttempts: 2 });
      queue.start();

      await new Promise(resolve => setTimeout(resolve, 200));

      const updated = await queue.getJob(job.id);
      expect(updated?.status).toBe('failed');
      expect(updated?.attempts).toBe(2);
    });

    it('should respect concurrency limit', async () => {
      let concurrent = 0;
      let maxConcurrent = 0;

      queue.registerHandler('concurrent-job', async () => {
        concurrent++;
        maxConcurrent = Math.max(maxConcurrent, concurrent);
        await new Promise(resolve => setTimeout(resolve, 50));
        concurrent--;
        return {};
      });

      // Enqueue more jobs than concurrency limit
      for (let i = 0; i < 5; i++) {
        await queue.enqueue('concurrent-job', { index: i });
      }

      queue.start();
      await new Promise(resolve => setTimeout(resolve, 300));

      expect(maxConcurrent).toBeLessThanOrEqual(2);
    });

    it('should process high priority jobs first', async () => {
      const processOrder: string[] = [];

      queue.registerHandler('priority-job', async (job) => {
        processOrder.push(job.payload as string);
        return {};
      });

      await queue.enqueue('priority-job', 'low', { priority: 'low' });
      await queue.enqueue('priority-job', 'high', { priority: 'high' });
      await queue.enqueue('priority-job', 'critical', { priority: 'critical' });
      await queue.enqueue('priority-job', 'normal', { priority: 'normal' });

      queue.start();
      await new Promise(resolve => setTimeout(resolve, 100));

      expect(processOrder[0]).toBe('critical');
      expect(processOrder[1]).toBe('high');
    });
  });

  describe('Job Cancellation', () => {
    it('should cancel pending job', async () => {
      const job = await queue.enqueue('test', {});

      const cancelled = await queue.cancelJob(job.id);
      const updated = await queue.getJob(job.id);

      expect(cancelled).toBe(true);
      expect(updated?.status).toBe('cancelled');
    });

    it('should not cancel running job', async () => {
      queue.registerHandler('long-job', async () => {
        await new Promise(resolve => setTimeout(resolve, 1000));
        return {};
      });

      const job = await queue.enqueue('long-job', {});
      queue.start();

      await new Promise(resolve => setTimeout(resolve, 20));

      const cancelled = await queue.cancelJob(job.id);
      expect(cancelled).toBe(false);
    });
  });

  describe('Job Retry', () => {
    it('should retry failed job', async () => {
      queue.registerHandler('retry-job', async () => {
        throw new Error('Fail');
      });

      const job = await queue.enqueue('retry-job', {}, { maxAttempts: 1 });
      queue.start();

      await new Promise(resolve => setTimeout(resolve, 100));

      let updated = await queue.getJob(job.id);
      expect(updated?.status).toBe('failed');

      // Make handler succeed now
      queue.registerHandler('retry-job', async () => ({ success: true }));

      const retried = await queue.retryJob(job.id);
      expect(retried).toBe(true);

      await new Promise(resolve => setTimeout(resolve, 100));

      updated = await queue.getJob(job.id);
      expect(updated?.status).toBe('completed');
    });
  });

  describe('Queue Management', () => {
    it('should start and stop processing', () => {
      const startHandler = vi.fn();
      const stopHandler = vi.fn();

      queue.on('started', startHandler);
      queue.on('stopped', stopHandler);

      queue.start();
      expect(startHandler).toHaveBeenCalled();

      queue.stop();
      expect(stopHandler).toHaveBeenCalled();
    });

    it('should drain running jobs', async () => {
      let completed = 0;
      queue.registerHandler('drain-job', async () => {
        await new Promise(resolve => setTimeout(resolve, 50));
        completed++;
        return {};
      });

      await queue.enqueue('drain-job', {});
      await queue.enqueue('drain-job', {});

      queue.start();
      await new Promise(resolve => setTimeout(resolve, 20));

      await queue.drain();
      expect(completed).toBe(2);
    });

    it('should flush pending and failed jobs', async () => {
      await queue.enqueue('test', {});
      await queue.enqueue('test', {});

      const flushed = await queue.flush();
      expect(flushed).toBe(2);
    });

    it('should get queue stats', async () => {
      queue.registerHandler('stat-job', async () => ({}));

      await queue.enqueue('stat-job', {});
      await queue.enqueue('stat-job', {});

      const stats = queue.getStats();
      expect(stats.pending).toBe(2);
      expect(stats.running).toBe(0);
      expect(stats.completed).toBe(0);
    });

    it('should get jobs by status', async () => {
      queue.registerHandler('status-job', async () => ({}));

      await queue.enqueue('status-job', {});
      await queue.enqueue('status-job', {});

      const pending = queue.getJobsByStatus('pending');
      expect(pending).toHaveLength(2);
    });
  });
});

describe('Scheduled Worker', () => {
  let scheduler: ScheduledWorker;

  beforeEach(() => {
    scheduler = new ScheduledWorker();
  });

  afterEach(() => {
    scheduler.stop();
  });

  describe('Task Scheduling', () => {
    it('should schedule task', () => {
      const handler = vi.fn().mockResolvedValue(undefined);

      scheduler.schedule('task-1', 100, handler);

      const tasks = scheduler.listTasks();
      expect(tasks).toContain('task-1');
    });

    it('should unschedule task', () => {
      scheduler.schedule('task-1', 100, vi.fn());

      const result = scheduler.unschedule('task-1');

      expect(result).toBe(true);
      expect(scheduler.listTasks()).not.toContain('task-1');
    });

    it('should run scheduled task periodically', async () => {
      const handler = vi.fn().mockResolvedValue(undefined);

      scheduler.schedule('periodic', 50, handler);
      scheduler.start();

      await new Promise(resolve => setTimeout(resolve, 120));

      expect(handler).toHaveBeenCalledTimes(2);
    });

    it('should not run task concurrently with itself', async () => {
      let concurrent = 0;
      let maxConcurrent = 0;

      scheduler.schedule('non-concurrent', 10, async () => {
        concurrent++;
        maxConcurrent = Math.max(maxConcurrent, concurrent);
        await new Promise(resolve => setTimeout(resolve, 50));
        concurrent--;
      });

      scheduler.start();
      await new Promise(resolve => setTimeout(resolve, 100));

      expect(maxConcurrent).toBe(1);
    });
  });

  describe('Manual Execution', () => {
    it('should run task immediately', async () => {
      const handler = vi.fn().mockResolvedValue(undefined);
      scheduler.schedule('immediate', 10000, handler);

      await scheduler.runNow('immediate');

      expect(handler).toHaveBeenCalledTimes(1);
    });

    it('should throw when task not found', async () => {
      await expect(scheduler.runNow('non-existent'))
        .rejects.toThrow('not found');
    });

    it('should throw when task is already running', async () => {
      scheduler.schedule('running', 10000, async () => {
        await new Promise(resolve => setTimeout(resolve, 100));
      });

      const promise = scheduler.runNow('running');

      await expect(scheduler.runNow('running'))
        .rejects.toThrow('already running');

      await promise;
    });
  });

  describe('Task Info', () => {
    it('should get task info', () => {
      scheduler.schedule('info-task', 1000, vi.fn());

      const info = scheduler.getTaskInfo('info-task');

      expect(info).not.toBeNull();
      expect(info?.interval).toBe(1000);
      expect(info?.isRunning).toBe(false);
    });

    it('should track last run time', async () => {
      scheduler.schedule('tracked', 10000, vi.fn());

      await scheduler.runNow('tracked');

      const info = scheduler.getTaskInfo('tracked');
      expect(info?.lastRun).toBeInstanceOf(Date);
    });

    it('should return null for non-existent task', () => {
      const info = scheduler.getTaskInfo('non-existent');
      expect(info).toBeNull();
    });
  });

  describe('Lifecycle', () => {
    it('should start all scheduled tasks', () => {
      const handler1 = vi.fn();
      const handler2 = vi.fn();

      scheduler.schedule('task-1', 10000, handler1);
      scheduler.schedule('task-2', 10000, handler2);

      scheduler.start();

      // Tasks are scheduled (not run immediately)
      expect(scheduler.listTasks()).toHaveLength(2);
    });

    it('should stop all scheduled tasks', async () => {
      const handler = vi.fn().mockResolvedValue(undefined);

      scheduler.schedule('stopped', 20, handler);
      scheduler.start();

      await new Promise(resolve => setTimeout(resolve, 50));
      const countBeforeStop = handler.mock.calls.length;

      scheduler.stop();
      await new Promise(resolve => setTimeout(resolve, 50));

      expect(handler.mock.calls.length).toBe(countBeforeStop);
    });
  });
});
