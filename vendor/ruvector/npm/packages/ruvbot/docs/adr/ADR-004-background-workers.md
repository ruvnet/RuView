# ADR-004: Background Workers

**Status:** Accepted
**Date:** 2026-01-27
**Decision Makers:** RuVector Architecture Team
**Technical Area:** Infrastructure, Async Processing

---

## Context and Problem Statement

RuvBot requires background processing for tasks that:

1. **Take too long** for synchronous request handling (> 30s)
2. **Should be deferred** to reduce response latency
3. **Need retry logic** for unreliable operations
4. **Run on schedules** for maintenance and optimization
5. **Process in batches** for efficiency

Key use cases:

- **Memory consolidation**: Merge episodic memories into semantic knowledge
- **Embedding generation**: Batch process text to vectors
- **Pattern learning**: Train on trajectories during off-peak hours
- **Session cleanup**: Expire and archive old sessions
- **Index optimization**: Rebalance HNSW indices
- **Webhook dispatch**: Reliable delivery with retries

---

## Decision Drivers

### Reliability Requirements

| Requirement | Description |
|-------------|-------------|
| At-least-once delivery | Jobs must execute even after worker crashes |
| Idempotency | Safe to retry without side effects |
| Visibility | Monitor job status, progress, failures |
| Dead letter handling | Capture failed jobs for analysis |
| Graceful shutdown | Complete in-progress jobs before stopping |

### Performance Requirements

| Metric | Target |
|--------|--------|
| Job pickup latency | < 100ms |
| Throughput | 1000 jobs/second |
| Concurrent workers | 50 per instance |
| Job timeout | Configurable, default 5 minutes |
| Retry delay | Exponential backoff, max 1 hour |

---

## Decision Outcome

### Adopt BullMQ with Custom Worker Framework

We implement a worker framework on top of BullMQ (Redis-backed) with integration to the agentic-flow worker pattern.

```
+-----------------------------------------------------------------------------+
|                           WORKER ARCHITECTURE                                |
+-----------------------------------------------------------------------------+

                    +---------------------------+
                    |      Job Dispatcher       |
                    |   (Application Layer)     |
                    +-------------+-------------+
                                  |
                    +-------------v-------------+
                    |        BullMQ Queue       |
                    |   (Redis-backed storage)  |
                    +-------------+-------------+
                                  |
          +-----------------------+-----------------------+
          |                       |                       |
+---------v---------+   +---------v---------+   +---------v---------+
|   Worker Pool     |   |   Worker Pool     |   |   Worker Pool     |
|   (Instance 1)    |   |   (Instance 2)    |   |   (Instance N)    |
|-------------------|   |-------------------|   |-------------------|
| - memory-consol   |   | - embedding-batch |   | - webhook-disp    |
| - pattern-learn   |   | - session-cleanup |   | - index-optimize  |
+-------------------+   +-------------------+   +-------------------+
          |                       |                       |
          +-----------------------+-----------------------+
                                  |
                    +-------------v-------------+
                    |       Worker Monitor      |
                    | (Metrics, Alerts, Admin)  |
                    +---------------------------+
```

---

## Job Types

### Core Worker Definitions

```typescript
// Job type registry
const JOB_TYPES = {
  // Memory workers
  MEMORY_CONSOLIDATION: 'memory-consolidation',
  MEMORY_CLEANUP: 'memory-cleanup',
  MEMORY_IMPORTANCE_DECAY: 'memory-importance-decay',

  // Embedding workers
  EMBEDDING_BATCH: 'embedding-batch',
  EMBEDDING_REINDEX: 'embedding-reindex',

  // Learning workers
  TRAJECTORY_PROCESS: 'trajectory-process',
  PATTERN_TRAINING: 'pattern-training',
  EWC_CONSOLIDATION: 'ewc-consolidation',

  // Session workers
  SESSION_CLEANUP: 'session-cleanup',
  SESSION_ARCHIVE: 'session-archive',

  // Index workers
  INDEX_OPTIMIZATION: 'index-optimization',
  INDEX_REBALANCE: 'index-rebalance',

  // Integration workers
  WEBHOOK_DISPATCH: 'webhook-dispatch',
  SLACK_SYNC: 'slack-sync',

  // Maintenance workers
  QUOTA_CHECK: 'quota-check',
  AUDIT_EXPORT: 'audit-export',
  DATA_EXPORT: 'data-export',
  DATA_DELETION: 'data-deletion',
} as const;

type JobType = typeof JOB_TYPES[keyof typeof JOB_TYPES];
```

### Job Configuration

```typescript
// Job configuration with defaults
interface JobConfig<T> {
  type: JobType;
  data: T;
  options?: {
    priority?: number;        // Higher = more urgent (default: 0)
    delay?: number;           // ms to wait before processing
    attempts?: number;        // Max retry attempts (default: 3)
    backoff?: {
      type: 'exponential' | 'fixed';
      delay: number;          // Base delay in ms
    };
    timeout?: number;         // Max execution time in ms
    removeOnComplete?: boolean | number;  // Keep N completed jobs
    removeOnFail?: boolean | number;      // Keep N failed jobs
  };
  tenant?: {
    orgId: string;
    workspaceId?: string;
  };
}

// Default configurations per job type
const JOB_DEFAULTS: Record<JobType, Partial<JobConfig<unknown>['options']>> = {
  'memory-consolidation': {
    priority: 5,
    attempts: 3,
    backoff: { type: 'exponential', delay: 5000 },
    timeout: 300000,  // 5 minutes
  },
  'embedding-batch': {
    priority: 10,
    attempts: 5,
    backoff: { type: 'exponential', delay: 2000 },
    timeout: 60000,   // 1 minute
  },
  'webhook-dispatch': {
    priority: 15,
    attempts: 10,
    backoff: { type: 'exponential', delay: 1000 },
    timeout: 30000,   // 30 seconds
  },
  'pattern-training': {
    priority: 1,
    attempts: 2,
    backoff: { type: 'fixed', delay: 60000 },
    timeout: 3600000, // 1 hour
  },
  'session-cleanup': {
    priority: 3,
    attempts: 3,
    backoff: { type: 'exponential', delay: 10000 },
    timeout: 120000,  // 2 minutes
  },
  // ... other defaults
};
```

---

## Worker Implementation

### Worker Base Class

```typescript
// Abstract worker with tenant context and lifecycle hooks
abstract class BaseWorker<TData, TResult> {
  protected logger: Logger;
  protected metrics: MetricsCollector;
  protected tenantContext?: TenantContext;

  constructor(
    protected readonly type: JobType,
    protected readonly config: WorkerConfig
  ) {
    this.logger = createLogger(`worker:${type}`);
    this.metrics = createMetricsCollector(`worker.${type}`);
  }

  // Main processing method - implement in subclasses
  abstract process(data: TData, job: Job): Promise<TResult>;

  // Lifecycle hooks
  async onStart(job: Job): Promise<void> {
    this.metrics.increment('started');
    this.logger.info(`Starting job ${job.id}`, { data: job.data });

    // Set tenant context if provided
    if (job.data.tenantContext) {
      this.tenantContext = job.data.tenantContext;
    }
  }

  async onComplete(job: Job, result: TResult): Promise<void> {
    this.metrics.increment('completed');
    this.metrics.timing('duration', Date.now() - job.processedOn!);
    this.logger.info(`Completed job ${job.id}`, { result });
  }

  async onFailed(job: Job, error: Error): Promise<void> {
    this.metrics.increment('failed');
    this.logger.error(`Failed job ${job.id}`, { error: error.message });

    // Check if should alert
    if (job.attemptsMade >= (job.opts.attempts ?? 3) - 1) {
      await this.alertOnFinalFailure(job, error);
    }
  }

  async onProgress(job: Job, progress: number | object): Promise<void> {
    this.logger.debug(`Progress ${job.id}`, { progress });
  }

  // Error classification
  isRetryable(error: Error): boolean {
    // Network errors, rate limits, temporary failures
    return (
      error.name === 'NetworkError' ||
      error.name === 'TimeoutError' ||
      (error as any).code === 'ECONNRESET' ||
      (error as any).status === 429 ||
      (error as any).status >= 500
    );
  }

  private async alertOnFinalFailure(job: Job, error: Error): Promise<void> {
    // Send to alerting system
    await this.metrics.alert({
      severity: 'warning',
      title: `Worker ${this.type} final failure`,
      message: `Job ${job.id} failed after ${job.attemptsMade} attempts: ${error.message}`,
      tags: {
        jobType: this.type,
        jobId: job.id,
        tenantId: this.tenantContext?.orgId,
      },
    });
  }
}
```

### Memory Consolidation Worker

```typescript
// Consolidate episodic memories into semantic knowledge
class MemoryConsolidationWorker extends BaseWorker<
  MemoryConsolidationData,
  MemoryConsolidationResult
> {
  constructor(
    private memoryRepo: MemoryRepository,
    private vectorStore: RuVectorAdapter,
    private llm: LLMProvider
  ) {
    super(JOB_TYPES.MEMORY_CONSOLIDATION, {
      concurrency: 5,
      limiter: { max: 10, duration: 1000 },
    });
  }

  async process(data: MemoryConsolidationData, job: Job): Promise<MemoryConsolidationResult> {
    const { workspaceId, userId, memoryIds, consolidationType } = data;

    // Load memories to consolidate
    const memories = await this.memoryRepo.findByIds(memoryIds);

    if (memories.length < 2) {
      return { consolidated: false, reason: 'insufficient_memories' };
    }

    await job.updateProgress({ phase: 'analyzing', percent: 10 });

    // Analyze memories for consolidation
    const clusters = await this.clusterMemories(memories);

    await job.updateProgress({ phase: 'generating', percent: 40 });

    // Generate consolidated knowledge for each cluster
    const consolidatedMemories: Memory[] = [];

    for (const cluster of clusters) {
      if (cluster.memories.length < 2) continue;

      // Use LLM to synthesize cluster into semantic knowledge
      const synthesis = await this.synthesizeCluster(cluster);

      // Create new semantic memory
      const semanticMemory = await this.memoryRepo.save({
        id: crypto.randomUUID(),
        type: 'semantic',
        content: synthesis.content,
        importance: synthesis.importance,
        sourceType: 'consolidation',
        sourceId: job.id,
        metadata: {
          sourceMemoryIds: cluster.memories.map(m => m.id),
          consolidationType,
          synthesisConfidence: synthesis.confidence,
        },
      });

      consolidatedMemories.push(semanticMemory);

      // Optionally reduce importance of source memories
      if (consolidationType === 'merge') {
        await Promise.all(
          cluster.memories.map(m =>
            this.memoryRepo.updateImportance(m.id, m.importance * 0.5)
          )
        );
      }
    }

    await job.updateProgress({ phase: 'indexing', percent: 80 });

    // Update vector indices
    await this.reindexMemories(consolidatedMemories);

    await job.updateProgress({ phase: 'complete', percent: 100 });

    return {
      consolidated: true,
      clustersProcessed: clusters.length,
      memoriesCreated: consolidatedMemories.length,
      sourceMemoriesProcessed: memories.length,
    };
  }

  private async clusterMemories(memories: Memory[]): Promise<MemoryCluster[]> {
    // Get embeddings
    const embeddings = await Promise.all(
      memories.map(m => this.vectorStore.getById(m.embeddingId!))
    );

    // Simple clustering by cosine similarity threshold
    const clusters: MemoryCluster[] = [];
    const assigned = new Set<string>();

    for (let i = 0; i < memories.length; i++) {
      if (assigned.has(memories[i].id)) continue;

      const cluster: MemoryCluster = {
        memories: [memories[i]],
        centroid: embeddings[i]!,
      };
      assigned.add(memories[i].id);

      for (let j = i + 1; j < memories.length; j++) {
        if (assigned.has(memories[j].id)) continue;

        const similarity = cosineSimilarity(embeddings[i]!, embeddings[j]!);
        if (similarity > 0.8) {
          cluster.memories.push(memories[j]);
          assigned.add(memories[j].id);
        }
      }

      clusters.push(cluster);
    }

    return clusters;
  }

  private async synthesizeCluster(cluster: MemoryCluster): Promise<Synthesis> {
    const prompt = `Synthesize the following related memories into a single coherent fact or concept:

${cluster.memories.map((m, i) => `${i + 1}. ${m.content}`).join('\n')}

Provide a concise synthesis that captures the key information.`;

    const response = await this.llm.complete(prompt, {
      maxTokens: 200,
      temperature: 0.3,
    });

    return {
      content: response.text.trim(),
      importance: Math.max(...cluster.memories.map(m => m.importance)),
      confidence: cluster.memories.length >= 3 ? 0.9 : 0.7,
    };
  }
}
```

### Embedding Batch Worker

```typescript
// Batch process embeddings for efficiency
class EmbeddingBatchWorker extends BaseWorker<
  EmbeddingBatchData,
  EmbeddingBatchResult
> {
  constructor(
    private embedder: EmbeddingService,
    private vectorStore: RuVectorAdapter,
    private db: PostgresAdapter
  ) {
    super(JOB_TYPES.EMBEDDING_BATCH, {
      concurrency: 3,
      limiter: { max: 5, duration: 1000 },
    });
  }

  async process(data: EmbeddingBatchData, job: Job): Promise<EmbeddingBatchResult> {
    const { items, namespace, updateTable, updateColumn } = data;

    const results: EmbeddingResult[] = [];
    const batchSize = 32;
    const totalBatches = Math.ceil(items.length / batchSize);

    for (let i = 0; i < items.length; i += batchSize) {
      const batch = items.slice(i, i + batchSize);
      const batchNum = Math.floor(i / batchSize) + 1;

      await job.updateProgress({
        phase: 'embedding',
        batch: batchNum,
        totalBatches,
        percent: (batchNum / totalBatches) * 80,
      });

      try {
        // Generate embeddings
        const embeddings = await this.embedder.embedBatch(
          batch.map(item => item.text)
        );

        // Prepare vector entries
        const entries: VectorEntry[] = batch.map((item, idx) => ({
          id: item.embeddingId || crypto.randomUUID(),
          vector: embeddings[idx],
          metadata: item.metadata,
        }));

        // Insert into vector store
        await this.vectorStore.upsert(namespace, entries);

        // Update source records if needed
        if (updateTable && updateColumn) {
          await this.updateSourceRecords(
            updateTable,
            updateColumn,
            batch.map((item, idx) => ({
              id: item.sourceId,
              embeddingId: entries[idx].id,
            }))
          );
        }

        results.push(
          ...batch.map((item, idx) => ({
            sourceId: item.sourceId,
            embeddingId: entries[idx].id,
            success: true,
          }))
        );
      } catch (error) {
        // Record failures but continue with next batch
        this.logger.warn(`Batch ${batchNum} failed`, { error });
        results.push(
          ...batch.map(item => ({
            sourceId: item.sourceId,
            embeddingId: null,
            success: false,
            error: (error as Error).message,
          }))
        );
      }
    }

    await job.updateProgress({ phase: 'complete', percent: 100 });

    return {
      total: items.length,
      succeeded: results.filter(r => r.success).length,
      failed: results.filter(r => !r.success).length,
      results,
    };
  }

  private async updateSourceRecords(
    table: string,
    column: string,
    updates: Array<{ id: string; embeddingId: string }>
  ): Promise<void> {
    // Batch update using UNNEST
    await this.db.query(`
      UPDATE ${table} AS t
      SET ${column} = u.embedding_id
      FROM UNNEST($1::uuid[], $2::uuid[]) AS u(id, embedding_id)
      WHERE t.id = u.id
    `, [
      updates.map(u => u.id),
      updates.map(u => u.embeddingId),
    ]);
  }
}
```

### Webhook Dispatch Worker

```typescript
// Reliable webhook delivery with retries
class WebhookDispatchWorker extends BaseWorker<
  WebhookDispatchData,
  WebhookDispatchResult
> {
  constructor(
    private http: HttpClient,
    private webhookRepo: WebhookRepository
  ) {
    super(JOB_TYPES.WEBHOOK_DISPATCH, {
      concurrency: 20,
      limiter: { max: 100, duration: 1000 },
    });
  }

  async process(data: WebhookDispatchData, job: Job): Promise<WebhookDispatchResult> {
    const { webhookId, payload, headers, signature } = data;

    // Load webhook configuration
    const webhook = await this.webhookRepo.findById(webhookId);
    if (!webhook || !webhook.isEnabled) {
      return { success: false, reason: 'webhook_disabled' };
    }

    // Prepare request
    const requestHeaders: Record<string, string> = {
      'Content-Type': 'application/json',
      'User-Agent': 'RuvBot-Webhook/1.0',
      'X-Webhook-Id': webhookId,
      'X-Delivery-Id': job.id,
      'X-Signature': signature,
      ...headers,
    };

    const startTime = Date.now();

    try {
      const response = await this.http.post(webhook.url, {
        body: JSON.stringify(payload),
        headers: requestHeaders,
        timeout: 10000,
      });

      const latency = Date.now() - startTime;

      // Record delivery
      await this.webhookRepo.recordDelivery({
        webhookId,
        jobId: job.id,
        status: 'success',
        statusCode: response.status,
        latencyMs: latency,
        responseBody: response.data?.slice(0, 1000),
      });

      // Update webhook stats
      await this.webhookRepo.updateStats(webhookId, {
        lastDeliveredAt: new Date(),
        successCount: { increment: 1 },
        avgLatencyMs: latency,
      });

      return {
        success: true,
        statusCode: response.status,
        latencyMs: latency,
      };
    } catch (error) {
      const latency = Date.now() - startTime;
      const httpError = error as HttpError;

      // Record failure
      await this.webhookRepo.recordDelivery({
        webhookId,
        jobId: job.id,
        status: 'failed',
        statusCode: httpError.status,
        latencyMs: latency,
        errorMessage: httpError.message,
      });

      // Update failure stats
      await this.webhookRepo.updateStats(webhookId, {
        failureCount: { increment: 1 },
      });

      // Check if should disable webhook
      const recentFailures = await this.webhookRepo.countRecentFailures(
        webhookId,
        24 * 60 * 60 * 1000 // 24 hours
      );

      if (recentFailures >= webhook.maxFailures) {
        await this.webhookRepo.disable(webhookId, 'max_failures_exceeded');
        this.metrics.increment('webhook_disabled');
      }

      // Determine if retryable
      if (!this.isRetryable(error as Error)) {
        return {
          success: false,
          reason: 'non_retryable_error',
          error: httpError.message,
        };
      }

      throw error; // Let BullMQ handle retry
    }
  }
}
```

---

## Scheduling

### Cron-Based Scheduling

```typescript
// Scheduled job configuration
interface ScheduledJob {
  name: string;
  cron: string;
  jobType: JobType;
  jobData: Record<string, unknown>;
  options?: {
    timezone?: string;
    tz?: string;
    startDate?: Date;
    endDate?: Date;
    limit?: number;
  };
}

// Default scheduled jobs
const SCHEDULED_JOBS: ScheduledJob[] = [
  {
    name: 'session-cleanup',
    cron: '0 */15 * * * *',  // Every 15 minutes
    jobType: JOB_TYPES.SESSION_CLEANUP,
    jobData: { maxAge: 24 * 60 * 60 * 1000 },  // 24 hours
  },
  {
    name: 'memory-importance-decay',
    cron: '0 0 */6 * * *',  // Every 6 hours
    jobType: JOB_TYPES.MEMORY_IMPORTANCE_DECAY,
    jobData: { decayFactor: 0.99 },
  },
  {
    name: 'index-optimization',
    cron: '0 0 3 * * *',  // 3 AM daily
    jobType: JOB_TYPES.INDEX_OPTIMIZATION,
    jobData: { threshold: 0.8 },
    options: { timezone: 'UTC' },
  },
  {
    name: 'pattern-training-daily',
    cron: '0 0 4 * * *',  // 4 AM daily
    jobType: JOB_TYPES.PATTERN_TRAINING,
    jobData: { batchSize: 1000, epochs: 5 },
    options: { timezone: 'UTC' },
  },
  {
    name: 'quota-check',
    cron: '0 0 * * * *',  // Every hour
    jobType: JOB_TYPES.QUOTA_CHECK,
    jobData: {},
  },
];

// Scheduler class
class WorkerScheduler {
  private schedulers: Map<string, RepeatableJob> = new Map();

  constructor(private queue: Queue) {}

  async start(): Promise<void> {
    for (const job of SCHEDULED_JOBS) {
      const repeatable = await this.queue.add(
        job.jobType,
        job.jobData,
        {
          repeat: {
            pattern: job.cron,
            ...job.options,
          },
        }
      );
      this.schedulers.set(job.name, repeatable);
      console.log(`Scheduled ${job.name}: ${job.cron}`);
    }
  }

  async stop(): Promise<void> {
    for (const [name, job] of this.schedulers) {
      await job.remove();
      console.log(`Unscheduled ${name}`);
    }
    this.schedulers.clear();
  }

  async reschedule(name: string, cron: string): Promise<void> {
    const existing = this.schedulers.get(name);
    if (existing) {
      await existing.remove();
    }

    const job = SCHEDULED_JOBS.find(j => j.name === name);
    if (!job) throw new Error(`Unknown scheduled job: ${name}`);

    const repeatable = await this.queue.add(
      job.jobType,
      job.jobData,
      { repeat: { pattern: cron, ...job.options } }
    );
    this.schedulers.set(name, repeatable);
  }
}
```

---

## Monitoring and Observability

### Metrics Collection

```typescript
// Worker metrics
interface WorkerMetrics {
  // Counters
  jobsStarted: Counter;
  jobsCompleted: Counter;
  jobsFailed: Counter;
  jobsRetried: Counter;

  // Gauges
  activeJobs: Gauge;
  waitingJobs: Gauge;
  delayedJobs: Gauge;
  workerInstances: Gauge;

  // Histograms
  jobDuration: Histogram;
  jobAttempts: Histogram;
  queueWaitTime: Histogram;
}

// Metrics exporter
class WorkerMetricsExporter {
  private registry: MetricRegistry;

  constructor() {
    this.registry = new MetricRegistry();
    this.initializeMetrics();
  }

  private initializeMetrics(): void {
    // Job lifecycle metrics
    this.registry.registerCounter('ruvbot_worker_jobs_total', {
      help: 'Total jobs processed',
      labels: ['type', 'status'],
    });

    this.registry.registerGauge('ruvbot_worker_jobs_active', {
      help: 'Currently active jobs',
      labels: ['type'],
    });

    this.registry.registerHistogram('ruvbot_worker_job_duration_seconds', {
      help: 'Job processing duration',
      labels: ['type'],
      buckets: [0.1, 0.5, 1, 5, 10, 30, 60, 300],
    });

    // Queue metrics
    this.registry.registerGauge('ruvbot_worker_queue_depth', {
      help: 'Number of jobs in queue',
      labels: ['type', 'state'],
    });
  }

  async collectMetrics(): Promise<string> {
    // Collect from all queues
    const queues = await this.getQueueStats();

    for (const [queueName, stats] of Object.entries(queues)) {
      this.registry.setGauge('ruvbot_worker_queue_depth', stats.waiting, {
        type: queueName,
        state: 'waiting',
      });
      this.registry.setGauge('ruvbot_worker_queue_depth', stats.active, {
        type: queueName,
        state: 'active',
      });
      this.registry.setGauge('ruvbot_worker_queue_depth', stats.delayed, {
        type: queueName,
        state: 'delayed',
      });
    }

    return this.registry.export();
  }
}
```

### Health Checks

```typescript
// Worker health check
interface WorkerHealthCheck {
  status: 'healthy' | 'degraded' | 'unhealthy';
  checks: {
    redis: HealthStatus;
    workers: HealthStatus;
    queues: HealthStatus;
    deadLetters: HealthStatus;
  };
  details: {
    activeWorkers: number;
    totalQueues: number;
    oldestWaitingJob: number | null;
    deadLetterCount: number;
  };
}

class WorkerHealthChecker {
  async check(): Promise<WorkerHealthCheck> {
    const [redis, workers, queues, deadLetters] = await Promise.all([
      this.checkRedis(),
      this.checkWorkers(),
      this.checkQueues(),
      this.checkDeadLetters(),
    ]);

    const status = this.aggregateStatus([redis, workers, queues, deadLetters]);

    return {
      status,
      checks: { redis, workers, queues, deadLetters },
      details: {
        activeWorkers: workers.details?.count ?? 0,
        totalQueues: queues.details?.count ?? 0,
        oldestWaitingJob: queues.details?.oldestWaiting ?? null,
        deadLetterCount: deadLetters.details?.count ?? 0,
      },
    };
  }

  private async checkRedis(): Promise<HealthStatus> {
    try {
      const start = Date.now();
      await this.redis.ping();
      const latency = Date.now() - start;

      return {
        status: latency < 100 ? 'healthy' : 'degraded',
        latency,
      };
    } catch (error) {
      return { status: 'unhealthy', error: (error as Error).message };
    }
  }

  private async checkWorkers(): Promise<HealthStatus> {
    const workers = await this.getActiveWorkers();
    const minWorkers = this.config.minWorkers ?? 1;

    if (workers.length >= minWorkers) {
      return { status: 'healthy', details: { count: workers.length } };
    } else if (workers.length > 0) {
      return { status: 'degraded', details: { count: workers.length } };
    } else {
      return { status: 'unhealthy', details: { count: 0 } };
    }
  }

  private async checkDeadLetters(): Promise<HealthStatus> {
    const count = await this.getDeadLetterCount();
    const threshold = this.config.deadLetterThreshold ?? 100;

    if (count === 0) {
      return { status: 'healthy', details: { count } };
    } else if (count < threshold) {
      return { status: 'degraded', details: { count } };
    } else {
      return { status: 'unhealthy', details: { count } };
    }
  }
}
```

---

## Error Handling and Dead Letters

### Dead Letter Queue

```typescript
// Dead letter handling
interface DeadLetterEntry {
  id: string;
  originalJobId: string;
  jobType: JobType;
  jobData: unknown;
  error: {
    message: string;
    stack?: string;
    code?: string;
  };
  attempts: number;
  failedAt: Date;
  tenant?: {
    orgId: string;
    workspaceId?: string;
  };
}

class DeadLetterManager {
  constructor(
    private queue: Queue,
    private storage: DeadLetterStorage,
    private alerter: Alerter
  ) {}

  async moveToDeadLetter(job: Job, error: Error): Promise<void> {
    const entry: DeadLetterEntry = {
      id: crypto.randomUUID(),
      originalJobId: job.id,
      jobType: job.name as JobType,
      jobData: job.data,
      error: {
        message: error.message,
        stack: error.stack,
        code: (error as any).code,
      },
      attempts: job.attemptsMade,
      failedAt: new Date(),
      tenant: job.data.tenantContext,
    };

    await this.storage.save(entry);

    // Alert on critical job types
    if (this.isCriticalJobType(job.name as JobType)) {
      await this.alerter.send({
        severity: 'high',
        title: `Critical job moved to dead letter: ${job.name}`,
        message: `Job ${job.id} failed after ${job.attemptsMade} attempts: ${error.message}`,
        metadata: {
          jobType: job.name,
          jobId: job.id,
          tenant: entry.tenant,
        },
      });
    }
  }

  async retry(deadLetterId: string): Promise<Job> {
    const entry = await this.storage.findById(deadLetterId);
    if (!entry) throw new Error(`Dead letter not found: ${deadLetterId}`);

    // Re-enqueue with fresh attempts
    const job = await this.queue.add(entry.jobType, entry.jobData, {
      attempts: 3,
      removeOnComplete: true,
    });

    // Mark as retried
    await this.storage.markRetried(deadLetterId, job.id);

    return job;
  }

  async purge(filter: DeadLetterFilter): Promise<number> {
    const entries = await this.storage.find(filter);
    await this.storage.deleteMany(entries.map(e => e.id));
    return entries.length;
  }

  private isCriticalJobType(type: JobType): boolean {
    return [
      JOB_TYPES.DATA_DELETION,
      JOB_TYPES.WEBHOOK_DISPATCH,
      JOB_TYPES.AUDIT_EXPORT,
    ].includes(type);
  }
}
```

---

## Consequences

### Benefits

1. **Reliability**: At-least-once delivery with persistent storage
2. **Observability**: Comprehensive metrics and health checks
3. **Flexibility**: Extensible worker framework
4. **Efficiency**: Batch processing and rate limiting
5. **Maintainability**: Clean separation of concerns

### Trade-offs

| Benefit | Trade-off |
|---------|-----------|
| Persistence | Redis memory usage |
| Concurrency | Complexity in ordering |
| Retries | Potential duplicates (need idempotency) |
| Monitoring | Observability overhead |

---

## Related Decisions

- **ADR-001**: Architecture Overview
- **ADR-003**: Persistence Layer (Redis integration)
- **ADR-007**: Learning System (training workers)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-27 | RuVector Architecture Team | Initial version |
