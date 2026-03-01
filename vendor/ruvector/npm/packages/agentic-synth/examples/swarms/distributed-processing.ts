/**
 * Distributed Processing Examples
 *
 * Demonstrates distributed computation patterns including map-reduce,
 * worker pools, message queues, event-driven architectures, and
 * saga pattern transactions for multi-agent systems.
 *
 * Integrates with:
 * - claude-flow: Distributed workflow orchestration
 * - Apache Kafka: Event streaming
 * - RabbitMQ: Message queuing
 * - Redis: Distributed caching
 */

import { AgenticSynth, createSynth } from '../../dist/index.js';
import type { GenerationResult } from '../../src/types.js';

// ============================================================================
// Example 1: Map-Reduce Job Data
// ============================================================================

/**
 * Generate map-reduce job execution data for distributed processing
 */
export async function mapReduceJobData() {
  console.log('\n🗺️  Example 1: Map-Reduce Job Execution\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Generate map-reduce jobs
  const jobs = await synth.generateStructured({
    count: 20,
    schema: {
      job_id: 'UUID',
      job_name: 'descriptive job name (e.g., "Word Count Analysis")',
      input_size_mb: 'number (100-10000)',
      map_phase: {
        mapper_count: 'number (10-100)',
        tasks: [
          {
            task_id: 'UUID',
            mapper_id: 'mapper-{1-100}',
            input_split: 'split-{id}',
            input_size_mb: 'number (proportional to job input)',
            output_records: 'number (1000-100000)',
            execution_time_ms: 'number (1000-30000)',
            status: 'completed | failed | running',
          },
        ],
        start_time: 'ISO timestamp',
        end_time: 'ISO timestamp',
        duration_ms: 'number (sum of task times)',
      },
      shuffle_phase: {
        data_transferred_mb: 'number (input_size_mb * 0.8-1.2)',
        partitions: 'number (10-50)',
        transfer_time_ms: 'number (5000-60000)',
      },
      reduce_phase: {
        reducer_count: 'number (5-30)',
        tasks: [
          {
            task_id: 'UUID',
            reducer_id: 'reducer-{1-30}',
            partition_id: 'number',
            input_records: 'number (10000-500000)',
            output_records: 'number (100-10000)',
            execution_time_ms: 'number (2000-40000)',
            status: 'completed | failed | running',
          },
        ],
        start_time: 'ISO timestamp',
        end_time: 'ISO timestamp',
        duration_ms: 'number',
      },
      overall_status: 'completed | failed | running',
      total_duration_ms: 'number',
      efficiency_score: 'number (0.5-1.0)',
    },
    constraints: [
      'Map tasks should run in parallel',
      'Reduce phase starts after map phase completes',
      '95% of tasks should be completed',
      'Efficiency should correlate with parallelism',
    ],
  });

  // Analyze map-reduce performance
  const completedJobs = jobs.data.filter((j: any) => j.overall_status === 'completed');
  const avgEfficiency = completedJobs.reduce((sum: number, j: any) => sum + j.efficiency_score, 0) / completedJobs.length;

  console.log('Map-Reduce Analysis:');
  console.log(`- Total jobs: ${jobs.data.length}`);
  console.log(`- Completed: ${completedJobs.length}`);
  console.log(`- Average efficiency: ${(avgEfficiency * 100).toFixed(1)}%`);
  console.log(`- Total data processed: ${jobs.data.reduce((sum: number, j: any) => sum + j.input_size_mb, 0).toFixed(0)} MB`);

  // Calculate parallelism metrics
  const avgMappers = jobs.data.reduce((sum: number, j: any) => sum + j.map_phase.mapper_count, 0) / jobs.data.length;
  const avgReducers = jobs.data.reduce((sum: number, j: any) => sum + j.reduce_phase.reducer_count, 0) / jobs.data.length;

  console.log(`\nParallelism Metrics:`);
  console.log(`- Average mappers: ${avgMappers.toFixed(1)}`);
  console.log(`- Average reducers: ${avgReducers.toFixed(1)}`);

  // Integration with claude-flow
  console.log('\nClaude-Flow Integration:');
  console.log('npx claude-flow@alpha hooks pre-task --description "Map-Reduce job execution"');
  console.log('// Store results in coordination memory for analysis');

  return jobs;
}

// ============================================================================
// Example 2: Worker Pool Simulation
// ============================================================================

/**
 * Generate worker pool execution data
 */
export async function workerPoolSimulation() {
  console.log('\n👷 Example 2: Worker Pool Simulation\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate worker pool state over time
  const poolStates = await synth.generateTimeSeries({
    count: 100,
    interval: '1m',
    metrics: [
      'active_workers',
      'idle_workers',
      'queue_size',
      'tasks_per_minute',
      'avg_task_duration_ms',
      'worker_utilization',
    ],
    trend: 'mixed',
    seasonality: true,
  });

  // Generate individual worker performance
  const workerMetrics = await synth.generateStructured({
    count: 200,
    schema: {
      timestamp: 'ISO timestamp',
      worker_id: 'worker-{1-20}',
      state: 'idle | busy | initializing | terminating',
      current_task_id: 'UUID or null',
      tasks_completed: 'number (0-1000)',
      tasks_failed: 'number (0-50)',
      avg_execution_time_ms: 'number (100-5000)',
      cpu_usage: 'number (0-100)',
      memory_mb: 'number (100-2000)',
      uptime_seconds: 'number (0-86400)',
      last_heartbeat: 'ISO timestamp',
    },
    constraints: [
      'Busy workers should have current_task_id',
      'Idle workers should have null current_task_id',
      'High task count should correlate with low avg execution time',
    ],
  });

  // Generate task execution history
  const taskExecutions = await synth.generateEvents({
    count: 500,
    eventTypes: ['task_queued', 'task_assigned', 'task_started', 'task_completed', 'task_failed'],
    schema: {
      event_id: 'UUID',
      task_id: 'UUID',
      event_type: 'one of eventTypes',
      worker_id: 'worker-{1-20} or null',
      queue_wait_time_ms: 'number (0-10000)',
      execution_time_ms: 'number (100-5000)',
      priority: 'number (1-10)',
      timestamp: 'ISO timestamp',
    },
    distribution: 'poisson',
  });

  console.log('Worker Pool Analysis:');
  console.log(`- Time series points: ${poolStates.data.length}`);
  console.log(`- Worker metrics: ${workerMetrics.data.length}`);
  console.log(`- Task executions: ${taskExecutions.data.length}`);

  // Calculate pool efficiency
  const avgUtilization = poolStates.data.reduce(
    (sum: number, p: any) => sum + p.worker_utilization,
    0
  ) / poolStates.data.length;

  console.log(`\nPool Efficiency:`);
  console.log(`- Average utilization: ${avgUtilization.toFixed(1)}%`);
  console.log(`- Active workers: ${workerMetrics.data.filter((w: any) => w.state === 'busy').length}`);
  console.log(`- Idle workers: ${workerMetrics.data.filter((w: any) => w.state === 'idle').length}`);

  // Integration patterns
  console.log('\nIntegration Pattern:');
  console.log('// Bull Queue (Redis-based)');
  console.log('const queue = new Queue("tasks", { redis: redisConfig });');
  console.log('// Process with worker pool');
  console.log('queue.process(concurrency, async (job) => { /* ... */ });');

  return { poolStates, workerMetrics, taskExecutions };
}

// ============================================================================
// Example 3: Message Queue Scenarios
// ============================================================================

/**
 * Generate message queue data (RabbitMQ, SQS, etc.)
 */
export async function messageQueueScenarios() {
  console.log('\n📬 Example 3: Message Queue Scenarios\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate queue metrics
  const queueMetrics = await synth.generateTimeSeries({
    count: 150,
    interval: '30s',
    metrics: [
      'messages_published',
      'messages_consumed',
      'queue_depth',
      'consumer_count',
      'publish_rate',
      'consume_rate',
      'avg_latency_ms',
    ],
    trend: 'up',
    seasonality: false,
  });

  // Generate message data
  const messages = await synth.generateStructured({
    count: 1000,
    schema: {
      message_id: 'UUID',
      queue_name: 'tasks | events | notifications | dead_letter',
      priority: 'number (0-9)',
      payload_size_bytes: 'number (100-10000)',
      content_type: 'application/json | text/plain | application/octet-stream',
      routing_key: 'routing key pattern',
      headers: {
        correlation_id: 'UUID',
        reply_to: 'queue name or null',
        timestamp: 'ISO timestamp',
        retry_count: 'number (0-5)',
      },
      published_at: 'ISO timestamp',
      consumed_at: 'ISO timestamp or null',
      acknowledged_at: 'ISO timestamp or null',
      status: 'pending | consumed | acknowledged | dead_letter | expired',
      time_in_queue_ms: 'number (0-60000)',
      consumer_id: 'consumer-{1-15} or null',
    },
    constraints: [
      'Consumed messages should have consumed_at timestamp',
      'Acknowledged messages should have acknowledged_at after consumed_at',
      '5% of messages should be in dead_letter queue',
      'Higher priority messages should have lower time_in_queue_ms',
    ],
  });

  // Generate consumer performance
  const consumers = await synth.generateStructured({
    count: 15,
    schema: {
      consumer_id: 'consumer-{1-15}',
      queue_subscriptions: ['array of 1-3 queue names'],
      messages_processed: 'number (0-200)',
      processing_rate_per_second: 'number (1-50)',
      error_count: 'number (0-20)',
      avg_processing_time_ms: 'number (100-2000)',
      prefetch_count: 'number (1-100)',
      status: 'active | idle | error | disconnected',
      last_activity: 'ISO timestamp',
    },
  });

  console.log('Message Queue Analysis:');
  console.log(`- Total messages: ${messages.data.length}`);
  console.log(`- Pending: ${messages.data.filter((m: any) => m.status === 'pending').length}`);
  console.log(`- Consumed: ${messages.data.filter((m: any) => m.status === 'consumed').length}`);
  console.log(`- Dead letter: ${messages.data.filter((m: any) => m.status === 'dead_letter').length}`);

  // Calculate throughput
  const totalProcessed = consumers.data.reduce(
    (sum: number, c: any) => sum + c.messages_processed,
    0
  );
  const avgLatency = messages.data
    .filter((m: any) => m.time_in_queue_ms)
    .reduce((sum: number, m: any) => sum + m.time_in_queue_ms, 0) / messages.data.length;

  console.log(`\nThroughput Metrics:`);
  console.log(`- Total processed: ${totalProcessed}`);
  console.log(`- Active consumers: ${consumers.data.filter((c: any) => c.status === 'active').length}`);
  console.log(`- Average latency: ${avgLatency.toFixed(0)}ms`);

  // RabbitMQ integration example
  console.log('\nRabbitMQ Integration:');
  console.log('// Connect to RabbitMQ');
  console.log('const channel = await connection.createChannel();');
  console.log('await channel.assertQueue("tasks", { durable: true });');
  console.log('// Publish with agentic-synth data');

  return { queueMetrics, messages, consumers };
}

// ============================================================================
// Example 4: Event-Driven Architecture
// ============================================================================

/**
 * Generate event-driven architecture data (Kafka, EventBridge)
 */
export async function eventDrivenArchitecture() {
  console.log('\n⚡ Example 4: Event-Driven Architecture\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate event streams
  const events = await synth.generateEvents({
    count: 2000,
    eventTypes: [
      'user.created',
      'user.updated',
      'order.placed',
      'order.confirmed',
      'order.shipped',
      'payment.processed',
      'inventory.updated',
      'notification.sent',
    ],
    schema: {
      event_id: 'UUID',
      event_type: 'one of eventTypes',
      event_version: 'v1 | v2',
      aggregate_id: 'UUID',
      aggregate_type: 'user | order | payment | inventory',
      payload: 'JSON object with event data',
      metadata: {
        causation_id: 'UUID (triggering event)',
        correlation_id: 'UUID (workflow id)',
        timestamp: 'ISO timestamp',
        source_service: 'service name',
        user_id: 'UUID or null',
      },
      partition_key: 'aggregate_id',
      sequence_number: 'number',
      published_at: 'ISO timestamp',
    },
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 3600000),
      end: new Date(),
    },
  });

  // Generate event handlers (subscribers)
  const handlers = await synth.generateStructured({
    count: 30,
    schema: {
      handler_id: 'UUID',
      handler_name: 'descriptive name',
      subscribed_events: ['array of 1-5 event types'],
      service_name: 'service name',
      processing_mode: 'sync | async | batch',
      events_processed: 'number (0-500)',
      events_failed: 'number (0-50)',
      avg_processing_time_ms: 'number (50-2000)',
      retry_policy: {
        max_retries: 'number (3-10)',
        backoff_strategy: 'exponential | linear | constant',
        dead_letter_queue: 'boolean',
      },
      status: 'active | degraded | failing | disabled',
    },
  });

  // Generate event projections (read models)
  const projections = await synth.generateStructured({
    count: 100,
    schema: {
      projection_id: 'UUID',
      projection_type: 'user_profile | order_summary | inventory_view',
      aggregate_id: 'UUID',
      version: 'number (1-100)',
      data: 'JSON object representing current state',
      last_event_id: 'UUID (from events)',
      last_updated: 'ISO timestamp',
      consistency_lag_ms: 'number (0-5000)',
    },
  });

  console.log('Event-Driven Architecture Analysis:');
  console.log(`- Total events: ${events.data.length}`);
  console.log(`- Event handlers: ${handlers.data.length}`);
  console.log(`- Projections: ${projections.data.length}`);

  // Event type distribution
  const eventTypes = new Map<string, number>();
  events.data.forEach((e: any) => {
    eventTypes.set(e.event_type, (eventTypes.get(e.event_type) || 0) + 1);
  });

  console.log('\nEvent Distribution:');
  eventTypes.forEach((count, type) => {
    console.log(`- ${type}: ${count}`);
  });

  // Calculate average consistency lag
  const avgLag = projections.data.reduce(
    (sum: number, p: any) => sum + p.consistency_lag_ms,
    0
  ) / projections.data.length;

  console.log(`\nConsistency Metrics:`);
  console.log(`- Average lag: ${avgLag.toFixed(0)}ms`);
  console.log(`- Active handlers: ${handlers.data.filter((h: any) => h.status === 'active').length}`);

  // Kafka integration
  console.log('\nKafka Integration:');
  console.log('// Produce events');
  console.log('await producer.send({ topic: "events", messages: [...] });');
  console.log('// Consume with handler coordination');
  console.log('await consumer.run({ eachMessage: async ({ message }) => { /* ... */ } });');

  return { events, handlers, projections };
}

// ============================================================================
// Example 5: Saga Pattern Transactions
// ============================================================================

/**
 * Generate saga pattern distributed transaction data
 */
export async function sagaPatternTransactions() {
  console.log('\n🔄 Example 5: Saga Pattern Transactions\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate saga executions
  const sagas = await synth.generateStructured({
    count: 100,
    schema: {
      saga_id: 'UUID',
      saga_type: 'order_fulfillment | payment_processing | user_registration | booking_reservation',
      orchestration_type: 'orchestration | choreography',
      initiator_service: 'service name',
      steps: [
        {
          step_id: 'UUID',
          step_name: 'descriptive step name',
          service: 'service name',
          operation: 'operation name',
          compensation_operation: 'compensation operation name',
          status: 'pending | executing | completed | failed | compensating | compensated',
          started_at: 'ISO timestamp or null',
          completed_at: 'ISO timestamp or null',
          duration_ms: 'number (100-5000)',
          retry_count: 'number (0-3)',
        },
      ],
      overall_status: 'in_progress | completed | failed | compensated',
      started_at: 'ISO timestamp',
      completed_at: 'ISO timestamp or null',
      total_duration_ms: 'number',
      compensation_triggered: 'boolean',
      compensation_reason: 'error description or null',
    },
    constraints: [
      'Sagas should have 3-8 steps',
      'Failed sagas should have compensation_triggered = true',
      'Compensated steps should execute in reverse order',
      '80% of sagas should complete successfully',
    ],
  });

  // Generate saga events
  const sagaEvents = await synth.generateEvents({
    count: 500,
    eventTypes: [
      'saga_started',
      'step_started',
      'step_completed',
      'step_failed',
      'compensation_started',
      'compensation_completed',
      'saga_completed',
      'saga_failed',
    ],
    schema: {
      event_id: 'UUID',
      saga_id: 'UUID (from sagas)',
      step_id: 'UUID or null',
      event_type: 'one of eventTypes',
      service: 'service name',
      payload: 'JSON object',
      timestamp: 'ISO timestamp',
    },
  });

  // Analyze saga patterns
  const successfulSagas = sagas.data.filter((s: any) => s.overall_status === 'completed');
  const failedSagas = sagas.data.filter((s: any) => s.overall_status === 'failed');
  const compensatedSagas = sagas.data.filter((s: any) => s.overall_status === 'compensated');

  console.log('Saga Pattern Analysis:');
  console.log(`- Total sagas: ${sagas.data.length}`);
  console.log(`- Successful: ${successfulSagas.length} (${((successfulSagas.length / sagas.data.length) * 100).toFixed(1)}%)`);
  console.log(`- Failed: ${failedSagas.length}`);
  console.log(`- Compensated: ${compensatedSagas.length}`);

  // Calculate average steps and duration
  const avgSteps = sagas.data.reduce((sum: number, s: any) => sum + s.steps.length, 0) / sagas.data.length;
  const avgDuration = sagas.data
    .filter((s: any) => s.completed_at)
    .reduce((sum: number, s: any) => sum + s.total_duration_ms, 0) / successfulSagas.length;

  console.log(`\nTransaction Metrics:`);
  console.log(`- Average steps per saga: ${avgSteps.toFixed(1)}`);
  console.log(`- Average duration: ${avgDuration.toFixed(0)}ms`);
  console.log(`- Compensation rate: ${((compensatedSagas.length / sagas.data.length) * 100).toFixed(1)}%`);

  // Orchestration vs Choreography
  const orchestrated = sagas.data.filter((s: any) => s.orchestration_type === 'orchestration').length;
  const choreographed = sagas.data.filter((s: any) => s.orchestration_type === 'choreography').length;

  console.log(`\nOrchestration Style:`);
  console.log(`- Orchestration: ${orchestrated}`);
  console.log(`- Choreography: ${choreographed}`);

  // Integration with coordination frameworks
  console.log('\nIntegration Pattern:');
  console.log('// MassTransit Saga State Machine');
  console.log('class OrderSaga : MassTransitStateMachine<OrderState> { /* ... */ }');
  console.log('// Or custom orchestrator with agentic-synth test data');

  return { sagas, sagaEvents };
}

// ============================================================================
// Example 6: Stream Processing Pipeline
// ============================================================================

/**
 * Generate stream processing pipeline data (Kafka Streams, Flink)
 */
export async function streamProcessingPipeline() {
  console.log('\n🌊 Example 6: Stream Processing Pipeline\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate pipeline topology
  const pipeline = await synth.generateStructured({
    count: 1,
    schema: {
      pipeline_id: 'UUID',
      pipeline_name: 'descriptive name',
      stages: [
        {
          stage_id: 'UUID',
          stage_type: 'source | transform | aggregate | filter | sink',
          stage_name: 'stage name',
          parallelism: 'number (1-20)',
          input_topics: ['array of topic names'],
          output_topics: ['array of topic names'],
          transformation: 'description of transformation',
          windowing: {
            type: 'tumbling | sliding | session | global',
            size_ms: 'number (1000-300000)',
            slide_ms: 'number or null',
          },
          state_store: 'store name or null',
        },
      ],
    },
  });

  // Generate processing metrics
  const metrics = await synth.generateTimeSeries({
    count: 200,
    interval: '30s',
    metrics: [
      'records_in',
      'records_out',
      'records_filtered',
      'processing_latency_ms',
      'watermark_lag_ms',
      'cpu_usage',
      'memory_mb',
    ],
    trend: 'mixed',
  });

  // Generate windowed aggregations
  const aggregations = await synth.generateStructured({
    count: 150,
    schema: {
      window_id: 'UUID',
      stage_id: 'UUID (from pipeline)',
      window_start: 'ISO timestamp',
      window_end: 'ISO timestamp',
      record_count: 'number (100-10000)',
      aggregate_values: {
        sum: 'number',
        avg: 'number',
        min: 'number',
        max: 'number',
        count: 'number',
      },
      emitted_at: 'ISO timestamp',
      late_arrivals: 'number (0-100)',
    },
  });

  console.log('Stream Processing Analysis:');
  console.log(`- Pipeline stages: ${pipeline.data[0].stages.length}`);
  console.log(`- Metric points: ${metrics.data.length}`);
  console.log(`- Window aggregations: ${aggregations.data.length}`);

  // Calculate throughput
  const avgRecordsIn = metrics.data.reduce((sum: number, m: any) => sum + m.records_in, 0) / metrics.data.length;
  const avgRecordsOut = metrics.data.reduce((sum: number, m: any) => sum + m.records_out, 0) / metrics.data.length;

  console.log(`\nThroughput:`);
  console.log(`- Input: ${avgRecordsIn.toFixed(0)} records/interval`);
  console.log(`- Output: ${avgRecordsOut.toFixed(0)} records/interval`);
  console.log(`- Filter rate: ${(((avgRecordsIn - avgRecordsOut) / avgRecordsIn) * 100).toFixed(1)}%`);

  console.log('\nApache Flink Integration:');
  console.log('// Define stream processing job');
  console.log('env.addSource(kafkaSource).keyBy(...).window(...).aggregate(...).addSink(kafkaSink);');

  return { pipeline, metrics, aggregations };
}

// ============================================================================
// Run All Examples
// ============================================================================

export async function runAllDistributedProcessingExamples() {
  console.log('🚀 Running All Distributed Processing Examples\n');
  console.log('='.repeat(70));

  try {
    await mapReduceJobData();
    console.log('='.repeat(70));

    await workerPoolSimulation();
    console.log('='.repeat(70));

    await messageQueueScenarios();
    console.log('='.repeat(70));

    await eventDrivenArchitecture();
    console.log('='.repeat(70));

    await sagaPatternTransactions();
    console.log('='.repeat(70));

    await streamProcessingPipeline();
    console.log('='.repeat(70));

    console.log('\n✅ All distributed processing examples completed!\n');
  } catch (error: any) {
    console.error('❌ Error running examples:', error.message);
    throw error;
  }
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  runAllDistributedProcessingExamples().catch(console.error);
}
