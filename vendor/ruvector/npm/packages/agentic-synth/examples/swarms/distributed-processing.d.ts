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
/**
 * Generate map-reduce job execution data for distributed processing
 */
export declare function mapReduceJobData(): Promise<import("../../dist/index.js").GenerationResult<unknown>>;
/**
 * Generate worker pool execution data
 */
export declare function workerPoolSimulation(): Promise<{
    poolStates: import("../../dist/index.js").GenerationResult<unknown>;
    workerMetrics: import("../../dist/index.js").GenerationResult<unknown>;
    taskExecutions: import("../../dist/index.js").GenerationResult<unknown>;
}>;
/**
 * Generate message queue data (RabbitMQ, SQS, etc.)
 */
export declare function messageQueueScenarios(): Promise<{
    queueMetrics: import("../../dist/index.js").GenerationResult<unknown>;
    messages: import("../../dist/index.js").GenerationResult<unknown>;
    consumers: import("../../dist/index.js").GenerationResult<unknown>;
}>;
/**
 * Generate event-driven architecture data (Kafka, EventBridge)
 */
export declare function eventDrivenArchitecture(): Promise<{
    events: import("../../dist/index.js").GenerationResult<unknown>;
    handlers: import("../../dist/index.js").GenerationResult<unknown>;
    projections: import("../../dist/index.js").GenerationResult<unknown>;
}>;
/**
 * Generate saga pattern distributed transaction data
 */
export declare function sagaPatternTransactions(): Promise<{
    sagas: import("../../dist/index.js").GenerationResult<unknown>;
    sagaEvents: import("../../dist/index.js").GenerationResult<unknown>;
}>;
/**
 * Generate stream processing pipeline data (Kafka Streams, Flink)
 */
export declare function streamProcessingPipeline(): Promise<{
    pipeline: import("../../dist/index.js").GenerationResult<unknown>;
    metrics: import("../../dist/index.js").GenerationResult<unknown>;
    aggregations: import("../../dist/index.js").GenerationResult<unknown>;
}>;
export declare function runAllDistributedProcessingExamples(): Promise<void>;
//# sourceMappingURL=distributed-processing.d.ts.map