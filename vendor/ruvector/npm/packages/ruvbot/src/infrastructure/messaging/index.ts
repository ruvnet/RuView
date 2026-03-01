/**
 * Messaging Layer - Event Bus, Queues, Pub/Sub
 */

export interface EventBus {
  publish<T>(event: DomainEvent<T>): Promise<void>;
  subscribe<T>(eventType: string, handler: EventHandler<T>): Subscription;
  replay(from: number, to?: number): AsyncIterator<DomainEvent>;
}

export interface DomainEvent<T = unknown> {
  id: string;
  type: string;
  timestamp: Date;
  tenantId: string;
  payload: T;
  metadata?: Record<string, unknown>;
}

export type EventHandler<T> = (event: DomainEvent<T>) => Promise<void>;

export interface Subscription {
  unsubscribe(): void;
}

export interface QueueManager {
  enqueue<T>(queue: string, job: Job<T>): Promise<string>;
  dequeue<T>(queue: string): Promise<Job<T> | null>;
  schedule<T>(queue: string, job: Job<T>, runAt: Date): Promise<string>;
  cancel(jobId: string): Promise<boolean>;
}

export interface Job<T = unknown> {
  id?: string;
  type: string;
  data: T;
  options?: JobOptions;
}

export interface JobOptions {
  priority?: number;
  delay?: number;
  attempts?: number;
  backoff?: {
    type: 'exponential' | 'fixed';
    delay: number;
  };
  timeout?: number;
}
