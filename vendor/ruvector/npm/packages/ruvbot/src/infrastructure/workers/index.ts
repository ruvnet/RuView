/**
 * Workers Layer - Background Jobs, Scheduled Tasks
 */

export interface WorkerPool {
  register<T>(jobType: string, handler: JobHandler<T>): void;
  start(): Promise<void>;
  stop(graceful?: boolean): Promise<void>;
  status(): WorkerPoolStatus;
}

export type JobHandler<T> = (job: WorkerJob<T>, context: WorkerContext) => Promise<JobResult>;

export interface WorkerJob<T = unknown> {
  id: string;
  type: string;
  data: T;
  attemptsMade: number;
  options: WorkerJobOptions;
}

export interface WorkerJobOptions {
  attempts: number;
  timeout: number;
}

export interface WorkerContext {
  updateProgress(progress: number | object): Promise<void>;
  log(level: 'debug' | 'info' | 'warn' | 'error', message: string): void;
}

export interface JobResult {
  success: boolean;
  output?: unknown;
  error?: string;
}

export interface WorkerPoolStatus {
  running: boolean;
  activeWorkers: number;
  pendingJobs: number;
  processedJobs: number;
  failedJobs: number;
}

// Built-in worker types
export const WORKER_TYPES = {
  MEMORY_CONSOLIDATION: 'memory-consolidation',
  EMBEDDING_BATCH: 'embedding-batch',
  PATTERN_TRAINING: 'pattern-training',
  SESSION_CLEANUP: 'session-cleanup',
  INDEX_OPTIMIZATION: 'index-optimization',
  WEBHOOK_DISPATCH: 'webhook-dispatch',
} as const;

export type WorkerType = typeof WORKER_TYPES[keyof typeof WORKER_TYPES];
