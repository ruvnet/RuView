---
name: "AgentDB Memory Patterns"
description: "Implement persistent memory patterns for AI agents using AgentDB. Includes session memory, long-term storage, pattern learning, and context management. Use when building stateful agents, chat systems, or intelligent assistants."
---

# AgentDB Memory Patterns

## What This Skill Does

Provides memory management patterns for AI agents using AgentDB's persistent storage and ReasoningBank integration. Enables agents to remember conversations, learn from interactions, and maintain context across sessions.

**Performance**: 150x-12,500x faster than traditional solutions with 100% backward compatibility.

## Prerequisites

- Node.js 18+
- AgentDB v1.0.7+ (via agentic-flow or standalone)
- Understanding of agent architectures

## Quick Start with CLI

### Initialize AgentDB

```bash
# Initialize vector database
npx agentdb@latest init ./agents.db

# Or with custom dimensions
npx agentdb@latest init ./agents.db --dimension 768

# Use preset configurations
npx agentdb@latest init ./agents.db --preset large

# In-memory database for testing
npx agentdb@latest init ./memory.db --in-memory
```

### Start MCP Server for Claude Code

```bash
# Start MCP server (integrates with Claude Code)
npx agentdb@latest mcp

# Add to Claude Code (one-time setup)
claude mcp add agentdb npx agentdb@latest mcp
```

### Create Learning Plugin

```bash
# Interactive plugin wizard
npx agentdb@latest create-plugin

# Use template directly
npx agentdb@latest create-plugin -t decision-transformer -n my-agent

# Available templates:
# - decision-transformer (sequence modeling RL)
# - q-learning (value-based learning)
# - sarsa (on-policy TD learning)
# - actor-critic (policy gradient)
# - curiosity-driven (exploration-based)
```

## Quick Start with API

```typescript
import { createAgentDBAdapter } from 'agentic-flow/reasoningbank';

// Initialize with default configuration
const adapter = await createAgentDBAdapter({
  dbPath: '.agentdb/reasoningbank.db',
  enableLearning: true,      // Enable learning plugins
  enableReasoning: true,      // Enable reasoning agents
  quantizationType: 'scalar', // binary | scalar | product | none
  cacheSize: 1000,            // In-memory cache
});

// Store interaction memory
const patternId = await adapter.insertPattern({
  id: '',
  type: 'pattern',
  domain: 'conversation',
  pattern_data: JSON.stringify({
    embedding: await computeEmbedding('What is the capital of France?'),
    pattern: {
      user: 'What is the capital of France?',
      assistant: 'The capital of France is Paris.',
      timestamp: Date.now()
    }
  }),
  confidence: 0.95,
  usage_count: 1,
  success_count: 1,
  created_at: Date.now(),
  last_used: Date.now(),
});

// Retrieve context with reasoning
const context = await adapter.retrieveWithReasoning(queryEmbedding, {
  domain: 'conversation',
  k: 10,
  useMMR: true,              // Maximal Marginal Relevance
  synthesizeContext: true,    // Generate rich context
});
```

## Memory Patterns

### 1. Session Memory
```typescript
class SessionMemory {
  async storeMessage(role: string, content: string) {
    return await db.storeMemory({
      sessionId: this.sessionId,
      role,
      content,
      timestamp: Date.now()
    });
  }

  async getSessionHistory(limit = 20) {
    return await db.query({
      filters: { sessionId: this.sessionId },
      orderBy: 'timestamp',
      limit
    });
  }
}
```

### 2. Long-Term Memory
```typescript
// Store important facts
await db.storeFact({
  category: 'user_preference',
  key: 'language',
  value: 'English',
  confidence: 1.0,
  source: 'explicit'
});

// Retrieve facts
const prefs = await db.getFacts({
  category: 'user_preference'
});
```

### 3. Pattern Learning
```typescript
// Learn from successful interactions
await db.storePattern({
  trigger: 'user_asks_time',
  response: 'provide_formatted_time',
  success: true,
  context: { timezone: 'UTC' }
});

// Apply learned patterns
const pattern = await db.matchPattern(currentContext);
```

## Advanced Patterns

### Hierarchical Memory
```typescript
// Organize memory in hierarchy
await memory.organize({
  immediate: recentMessages,    // Last 10 messages
  shortTerm: sessionContext,    // Current session
  longTerm: importantFacts,     // Persistent facts
  semantic: embeddedKnowledge   // Vector search
});
```

### Memory Consolidation
```typescript
// Periodically consolidate memories
await memory.consolidate({
  strategy: 'importance',       // Keep important memories
  maxSize: 10000,              // Size limit
  minScore: 0.5                // Relevance threshold
});
```

## CLI Operations

### Query Database

```bash
# Query with vector embedding
npx agentdb@latest query ./agents.db "[0.1,0.2,0.3,...]"

# Top-k results
npx agentdb@latest query ./agents.db "[0.1,0.2,0.3]" -k 10

# With similarity threshold
npx agentdb@latest query ./agents.db "0.1 0.2 0.3" -t 0.75

# JSON output
npx agentdb@latest query ./agents.db "[...]" -f json
```

### Import/Export Data

```bash
# Export vectors to file
npx agentdb@latest export ./agents.db ./backup.json

# Import vectors from file
npx agentdb@latest import ./backup.json

# Get database statistics
npx agentdb@latest stats ./agents.db
```

### Performance Benchmarks

```bash
# Run performance benchmarks
npx agentdb@latest benchmark

# Results show:
# - Pattern Search: 150x faster (100µs vs 15ms)
# - Batch Insert: 500x faster (2ms vs 1s)
# - Large-scale Query: 12,500x faster (8ms vs 100s)
```

## Integration with ReasoningBank

```typescript
import { createAgentDBAdapter, migrateToAgentDB } from 'agentic-flow/reasoningbank';

// Migrate from legacy ReasoningBank
const result = await migrateToAgentDB(
  '.swarm/memory.db',           // Source (legacy)
  '.agentdb/reasoningbank.db'   // Destination (AgentDB)
);

console.log(`✅ Migrated ${result.patternsMigrated} patterns`);

// Train learning model
const adapter = await createAgentDBAdapter({
  enableLearning: true,
});

await adapter.train({
  epochs: 50,
  batchSize: 32,
});

// Get optimal strategy with reasoning
const result = await adapter.retrieveWithReasoning(queryEmbedding, {
  domain: 'task-planning',
  synthesizeContext: true,
  optimizeMemory: true,
});
```

## Learning Plugins

### Available Algorithms (9 Total)

1. **Decision Transformer** - Sequence modeling RL (recommended)
2. **Q-Learning** - Value-based learning
3. **SARSA** - On-policy TD learning
4. **Actor-Critic** - Policy gradient with baseline
5. **Active Learning** - Query selection
6. **Adversarial Training** - Robustness
7. **Curriculum Learning** - Progressive difficulty
8. **Federated Learning** - Distributed learning
9. **Multi-task Learning** - Transfer learning

### List and Manage Plugins

```bash
# List available plugins
npx agentdb@latest list-plugins

# List plugin templates
npx agentdb@latest list-templates

# Get plugin info
npx agentdb@latest plugin-info <name>
```

## Reasoning Agents (4 Modules)

1. **PatternMatcher** - Find similar patterns with HNSW indexing
2. **ContextSynthesizer** - Generate rich context from multiple sources
3. **MemoryOptimizer** - Consolidate similar patterns, prune low-quality
4. **ExperienceCurator** - Quality-based experience filtering

## Best Practices

1. **Enable quantization**: Use scalar/binary for 4-32x memory reduction
2. **Use caching**: 1000 pattern cache for <1ms retrieval
3. **Batch operations**: 500x faster than individual inserts
4. **Train regularly**: Update learning models with new experiences
5. **Enable reasoning**: Automatic context synthesis and optimization
6. **Monitor metrics**: Use `stats` command to track performance

## Troubleshooting

### Issue: Memory growing too large
```bash
# Check database size
npx agentdb@latest stats ./agents.db

# Enable quantization
# Use 'binary' (32x smaller) or 'scalar' (4x smaller)
```

### Issue: Slow search performance
```bash
# Enable HNSW indexing and caching
# Results: <100µs search time
```

### Issue: Migration from legacy ReasoningBank
```bash
# Automatic migration with validation
npx agentdb@latest migrate --source .swarm/memory.db
```

## Performance Characteristics

- **Vector Search**: <100µs (HNSW indexing)
- **Pattern Retrieval**: <1ms (with cache)
- **Batch Insert**: 2ms for 100 patterns
- **Memory Efficiency**: 4-32x reduction with quantization
- **Backward Compatibility**: 100% compatible with ReasoningBank API

## Learn More

- GitHub: https://github.com/ruvnet/agentic-flow/tree/main/packages/agentdb
- Documentation: node_modules/agentic-flow/docs/AGENTDB_INTEGRATION.md
- MCP Integration: `npx agentdb@latest mcp` for Claude Code
- Website: https://agentdb.ruv.io
