# Multi-Agent Swarm Coordination Examples

Comprehensive examples demonstrating synthetic data generation for multi-agent systems, distributed processing, collective intelligence, and agent lifecycle management using agentic-synth.

## Overview

This directory contains production-ready examples for generating realistic test data for:

- **Agent Coordination**: Communication patterns, task distribution, consensus building, load balancing, and fault tolerance
- **Distributed Processing**: Map-reduce jobs, worker pools, message queues, event-driven architectures, and saga transactions
- **Collective Intelligence**: Collaborative problem-solving, knowledge sharing, emergent behaviors, voting systems, and reputation tracking
- **Agent Lifecycle**: Spawning/termination, state synchronization, health checks, recovery patterns, and version migrations

## Quick Start

### Installation

```bash
# Install agentic-synth
npm install @ruvector/agentic-synth

# Optional: Install integration packages
npm install claude-flow@alpha      # For swarm orchestration
npm install ruv-swarm             # For enhanced coordination
npm install flow-nexus@latest     # For cloud features
```

### Basic Usage

```typescript
import { createSynth } from '@ruvector/agentic-synth';

const synth = createSynth({
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY,
  cacheStrategy: 'memory',
});

// Generate agent communication data
const messages = await synth.generateEvents({
  count: 500,
  eventTypes: ['direct_message', 'broadcast', 'request_reply'],
  schema: {
    sender_agent_id: 'agent-{1-20}',
    receiver_agent_id: 'agent-{1-20}',
    payload: { action: 'string', data: 'object' },
    timestamp: 'ISO timestamp',
  },
});
```

## Examples

### 1. Agent Coordination (`agent-coordination.ts`)

Generate data for multi-agent communication and coordination patterns.

**Examples:**
- Agent communication patterns (direct messages, broadcasts, pub/sub)
- Task distribution scenarios with load balancing
- Consensus building (Raft, Paxos, Byzantine)
- Load balancing metrics and patterns
- Fault tolerance and failure recovery
- Hierarchical swarm coordination

**Run:**
```bash
npx tsx examples/swarms/agent-coordination.ts
```

**Integration with claude-flow:**
```bash
# Initialize swarm coordination
npx claude-flow@alpha hooks pre-task --description "Agent coordination"
npx claude-flow@alpha hooks notify --message "Coordination data generated"
npx claude-flow@alpha hooks post-edit --memory-key "swarm/coordinator/messages"
```

**Key Features:**
- 500+ communication events with realistic latency
- 300 task distribution scenarios with dependencies
- 50 consensus rounds with multiple protocols
- 100 agent metrics for load balancing analysis
- 100 failure scenarios with recovery actions
- Hierarchical topology with sub-coordinators

### 2. Distributed Processing (`distributed-processing.ts`)

Generate data for distributed computation and processing systems.

**Examples:**
- Map-reduce job execution data
- Worker pool simulation and metrics
- Message queue scenarios (RabbitMQ, SQS)
- Event-driven architecture (Kafka, EventBridge)
- Saga pattern distributed transactions
- Stream processing pipelines (Kafka Streams, Flink)

**Run:**
```bash
npx tsx examples/swarms/distributed-processing.ts
```

**Integration with Message Queues:**
```typescript
// RabbitMQ
const channel = await connection.createChannel();
await channel.assertQueue('tasks', { durable: true });
channel.sendToQueue('tasks', Buffer.from(JSON.stringify(taskData)));

// Kafka
await producer.send({
  topic: 'events',
  messages: generatedEvents.map(e => ({ value: JSON.stringify(e) })),
});
```

**Key Features:**
- 20 map-reduce jobs with mapper/reducer task tracking
- 200 worker pool states with utilization metrics
- 1,000 message queue messages with priority handling
- 2,000 event-driven architecture events
- 100 saga pattern transactions with compensation
- Stream processing with windowed aggregations

### 3. Collective Intelligence (`collective-intelligence.ts`)

Generate data for swarm intelligence and collaborative systems.

**Examples:**
- Collaborative problem-solving sessions
- Knowledge sharing and transfer patterns
- Emergent behavior simulation
- Voting and consensus mechanisms
- Reputation and trust systems

**Run:**
```bash
npx tsx examples/swarms/collective-intelligence.ts
```

**Integration with AgenticDB:**
```typescript
import AgenticDB from 'agenticdb';

const db = new AgenticDB();

// Store knowledge embeddings
await db.storeVector({
  text: knowledge.content,
  metadata: { category: knowledge.category, rating: knowledge.quality_rating },
});

// Semantic search for similar knowledge
const results = await db.search({ query: 'distributed consensus', topK: 10 });
```

**Integration with Neural Training:**
```bash
# Train patterns from successful collaborations
npx claude-flow@alpha hooks neural-train --pattern "collaboration"
npx claude-flow@alpha hooks session-end --export-metrics true
```

**Key Features:**
- 30 collaborative problem-solving sessions
- 200 knowledge base entries with quality ratings
- 1,000 agent interactions showing emergent patterns
- 50 voting sessions with multiple voting methods
- 100 reputation profiles with trust relationships

### 4. Agent Lifecycle (`agent-lifecycle.ts`)

Generate data for agent lifecycle management and orchestration.

**Examples:**
- Agent spawning and termination events
- State synchronization across distributed agents
- Health check scenarios and monitoring
- Recovery patterns and failure handling
- Version migration and deployment strategies

**Run:**
```bash
npx tsx examples/swarms/agent-lifecycle.ts
```

**Integration with Kubernetes:**
```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: agent-swarm
spec:
  replicas: 10
  template:
    spec:
      containers:
      - name: agent
        image: agent:v2.0
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
```

**Integration with claude-flow:**
```bash
# Spawn agents dynamically
npx claude-flow@alpha hooks pre-task --spawn-agents 5
npx claude-flow@alpha mcp start

# Use MCP tools for lifecycle management
# - agent_spawn: Create new agents
# - swarm_status: Monitor agent health
# - agent_metrics: Track performance
```

**Key Features:**
- 500 lifecycle events (spawn, ready, terminate, failed)
- 500 state snapshots with synchronization events
- 1,000 health checks with auto-healing actions
- 100 failure scenarios with recovery strategies
- 50 version migrations with canary deployments

## Integration Guides

### Claude-Flow Integration

Claude-Flow provides swarm orchestration and neural pattern learning.

**Setup:**
```bash
npm install claude-flow@alpha
npx claude-flow@alpha mcp start
```

**Usage:**
```bash
# Initialize swarm with topology
npx claude-flow@alpha hooks pre-task --description "Initialize mesh swarm"

# Store coordination data in memory
npx claude-flow@alpha hooks post-edit \
  --file "coordination.json" \
  --memory-key "swarm/coordinator/state"

# Train neural patterns from successful runs
npx claude-flow@alpha hooks neural-train --pattern "distributed-consensus"

# Export session metrics
npx claude-flow@alpha hooks session-end --export-metrics true
```

**MCP Tools:**
- `swarm_init`: Initialize swarm topology
- `agent_spawn`: Spawn new agents
- `task_orchestrate`: Orchestrate distributed tasks
- `swarm_status`: Monitor swarm health
- `neural_patterns`: Analyze learned patterns
- `memory_usage`: Track coordination memory

### Ruv-Swarm Integration

Ruv-Swarm provides enhanced multi-agent coordination.

**Setup:**
```bash
npm install ruv-swarm
npx ruv-swarm mcp start
```

**Usage:**
```typescript
// Access via MCP tools
// - swarm_init: Initialize coordination patterns
// - agent_metrics: Real-time agent performance
// - neural_status: Neural pattern analysis
```

### Flow-Nexus Cloud Integration

Flow-Nexus provides cloud-based agent management and sandboxed execution.

**Setup:**
```bash
npm install flow-nexus@latest
npx flow-nexus@latest register
npx flow-nexus@latest login
```

**MCP Tools (70+ available):**
```bash
# Create cloud sandbox for agent execution
# mcp__flow-nexus__sandbox_create

# Deploy swarm to cloud
# mcp__flow-nexus__swarm_init

# Scale agents dynamically
# mcp__flow-nexus__swarm_scale

# Real-time monitoring
# mcp__flow-nexus__execution_stream_subscribe
```

### Message Queue Integration

#### RabbitMQ

```typescript
import amqp from 'amqplib';

const connection = await amqp.connect('amqp://localhost');
const channel = await connection.createChannel();

await channel.assertQueue('tasks', { durable: true });

// Publish generated task data
for (const task of generatedTasks.data) {
  channel.sendToQueue('tasks', Buffer.from(JSON.stringify(task)), {
    persistent: true,
    priority: task.priority,
  });
}
```

#### Apache Kafka

```typescript
import { Kafka } from 'kafkajs';

const kafka = new Kafka({ clientId: 'agentic-synth', brokers: ['localhost:9092'] });
const producer = kafka.producer();

await producer.connect();
await producer.send({
  topic: 'agent-events',
  messages: generatedEvents.data.map(event => ({
    key: event.agent_id,
    value: JSON.stringify(event),
    partition: hash(event.partition_key) % partitionCount,
  })),
});
```

### Database Integration

#### AgenticDB (Vector Database)

```typescript
import AgenticDB from 'agenticdb';

const db = new AgenticDB({ persist: true, path: './agent-knowledge' });

// Store agent knowledge with embeddings
for (const entry of knowledgeBase.data) {
  await db.storeVector({
    text: entry.content,
    metadata: {
      category: entry.category,
      author: entry.author_agent_id,
      rating: entry.quality_rating,
      tags: entry.tags,
    },
  });
}

// Semantic search
const results = await db.search({
  query: 'consensus algorithm implementation',
  topK: 10,
  filter: { category: 'best_practice' },
});
```

#### Redis (State Synchronization)

```typescript
import Redis from 'ioredis';

const redis = new Redis();

// Store agent state
await redis.set(
  `agent:${agentId}:state`,
  JSON.stringify(stateSnapshot),
  'EX',
  3600 // TTL in seconds
);

// Get agent state
const state = await redis.get(`agent:${agentId}:state`);
```

## Use Cases

### 1. Testing Distributed Systems

Generate realistic test data for distributed agent systems:

```typescript
import { createSynth } from '@ruvector/agentic-synth';

const synth = createSynth();

// Generate test data for 100 agents coordinating
const testData = await synth.generateStructured({
  count: 100,
  schema: {
    agent_id: 'agent-{1-100}',
    tasks_assigned: 'number (0-50)',
    messages_sent: 'number (0-200)',
    coordination_events: ['array of coordination events'],
  },
});

// Use in integration tests
describe('Agent Swarm Coordination', () => {
  it('should handle 100 concurrent agents', async () => {
    const swarm = new AgentSwarm();
    await swarm.initialize(testData);
    expect(swarm.activeAgents).toBe(100);
  });
});
```

### 2. Load Testing and Benchmarking

Generate high-volume data for performance testing:

```typescript
// Generate 10,000 concurrent events
const loadTestData = await synth.generateEvents({
  count: 10000,
  eventTypes: ['task_request', 'task_complete', 'heartbeat'],
  distribution: 'poisson',
  timeRange: { start: new Date(), end: new Date(Date.now() + 3600000) },
});

// Replay events in load test
for (const event of loadTestData.data) {
  await testHarness.sendEvent(event);
}
```

### 3. Machine Learning Training

Generate training data for ML models:

```typescript
// Generate agent behavior data for ML training
const trainingData = await synth.generateStructured({
  count: 5000,
  schema: {
    // Features
    agent_load: 'number (0-100)',
    queue_depth: 'number (0-1000)',
    error_rate: 'number (0-100)',
    response_time_ms: 'number (10-5000)',
    // Label
    health_score: 'number (0-100, based on features)',
  },
});

// Train predictive model
const features = trainingData.data.map(d => [
  d.agent_load,
  d.queue_depth,
  d.error_rate,
  d.response_time_ms,
]);
const labels = trainingData.data.map(d => d.health_score);

await model.fit(features, labels);
```

### 4. Monitoring and Alerting

Generate test data for monitoring systems:

```typescript
// Generate various failure scenarios
const monitoringData = await synth.generateEvents({
  count: 200,
  eventTypes: ['agent_crash', 'high_latency', 'resource_exhaustion'],
  schema: {
    severity: 'critical | warning | info',
    affected_agents: ['array of agent ids'],
    metrics: { cpu: 'number', memory: 'number', latency: 'number' },
  },
});

// Test alerting rules
for (const event of monitoringData.data) {
  if (event.severity === 'critical') {
    expect(alertingSystem.shouldAlert(event)).toBe(true);
  }
}
```

## Performance Considerations

### Caching

Enable caching to speed up repeated data generation:

```typescript
const synth = createSynth({
  cacheStrategy: 'memory', // or 'disk'
  cacheTTL: 3600, // 1 hour
});

// First call - generates data
const data1 = await synth.generate('structured', options);

// Second call - returns cached result (much faster)
const data2 = await synth.generate('structured', options);
```

### Batch Generation

Generate multiple datasets in parallel:

```typescript
const batches = [
  { count: 100, schema: agentCoordinationSchema },
  { count: 200, schema: taskDistributionSchema },
  { count: 150, schema: healthCheckSchema },
];

const results = await synth.generateBatch('structured', batches, 3); // 3 concurrent
```

### Streaming

Generate large datasets with streaming:

```typescript
for await (const agent of synth.generateStream('structured', {
  count: 10000,
  schema: agentSchema,
})) {
  await processAgent(agent);
  // Process one at a time to avoid memory issues
}
```

## Best Practices

1. **Schema Design**: Create reusable schemas for consistency
2. **Constraints**: Use constraints to ensure data validity
3. **Caching**: Enable caching for development/testing
4. **Error Handling**: Always handle generation errors gracefully
5. **Validation**: Validate generated data before use
6. **Integration**: Use hooks for seamless integration with coordination frameworks

## Troubleshooting

### Common Issues

**Issue: API rate limits**
```typescript
// Solution: Enable caching and batch requests
const synth = createSynth({
  cacheStrategy: 'memory',
  maxRetries: 3,
  timeout: 30000,
});
```

**Issue: Memory usage with large datasets**
```typescript
// Solution: Use streaming instead of batch generation
for await (const item of synth.generateStream('structured', options)) {
  await processItem(item);
}
```

**Issue: Inconsistent data across runs**
```typescript
// Solution: Use constraints for consistency
const result = await synth.generateStructured({
  count: 100,
  schema: {...},
  constraints: [
    'IDs should be unique',
    'Timestamps should be in chronological order',
    'References should point to valid entities',
  ],
});
```

## API Reference

### Main Functions

- `createSynth(config)`: Create agentic-synth instance
- `generateStructured(options)`: Generate structured data
- `generateEvents(options)`: Generate event streams
- `generateTimeSeries(options)`: Generate time-series data
- `generateStream(type, options)`: Stream generation
- `generateBatch(type, batches, concurrency)`: Batch generation

### Configuration Options

```typescript
interface SynthConfig {
  provider: 'gemini' | 'openrouter';
  apiKey?: string;
  model?: string;
  cacheStrategy?: 'memory' | 'disk' | 'none';
  cacheTTL?: number; // seconds
  maxRetries?: number;
  timeout?: number; // milliseconds
  streaming?: boolean;
}
```

## Contributing

Contributions are welcome! Please submit examples that demonstrate:

- Real-world multi-agent patterns
- Integration with popular frameworks
- Performance optimizations
- Novel coordination strategies

## Resources

- [agentic-synth Documentation](https://github.com/ruvnet/ruvector/tree/main/packages/agentic-synth)
- [claude-flow Documentation](https://github.com/ruvnet/claude-flow)
- [AgenticDB Documentation](https://github.com/ruvnet/ruvector)
- [Flow-Nexus Platform](https://flow-nexus.ruv.io)

## License

MIT License - See LICENSE file for details

## Support

- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- Discussions: https://github.com/ruvnet/ruvector/discussions
- Discord: [Join our community](#)

---

**Note**: All examples use environment variables for API keys. Set `GEMINI_API_KEY` or `OPENROUTER_API_KEY` before running examples.
