---
name: "V3 Memory Unification"
description: "Unify 6+ memory systems into AgentDB with HNSW indexing for 150x-12,500x search improvements. Implements ADR-006 (Unified Memory Service) and ADR-009 (Hybrid Memory Backend)."
---

# V3 Memory Unification

## What This Skill Does

Consolidates disparate memory systems into unified AgentDB backend with HNSW vector search, achieving 150x-12,500x search performance improvements while maintaining backward compatibility.

## Quick Start

```bash
# Initialize memory unification
Task("Memory architecture", "Design AgentDB unification strategy", "v3-memory-specialist")

# AgentDB integration
Task("AgentDB setup", "Configure HNSW indexing and vector search", "v3-memory-specialist")

# Data migration
Task("Memory migration", "Migrate SQLite/Markdown to AgentDB", "v3-memory-specialist")
```

## Systems to Unify

### Legacy Systems → AgentDB
```
┌─────────────────────────────────────────┐
│  • MemoryManager (basic operations)     │
│  • DistributedMemorySystem (clustering) │
│  • SwarmMemory (agent-specific)         │
│  • AdvancedMemoryManager (features)     │
│  • SQLiteBackend (structured)           │
│  • MarkdownBackend (file-based)         │
│  • HybridBackend (combination)          │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│       🚀 AgentDB with HNSW             │
│  • 150x-12,500x faster search          │
│  • Unified query interface             │
│  • Cross-agent memory sharing          │
│  • SONA learning integration           │
└─────────────────────────────────────────┘
```

## Implementation Architecture

### Unified Memory Service
```typescript
class UnifiedMemoryService implements IMemoryBackend {
  constructor(
    private agentdb: AgentDBAdapter,
    private indexer: HNSWIndexer,
    private migrator: DataMigrator
  ) {}

  async store(entry: MemoryEntry): Promise<void> {
    await this.agentdb.store(entry);
    await this.indexer.index(entry);
  }

  async query(query: MemoryQuery): Promise<MemoryEntry[]> {
    if (query.semantic) {
      return this.indexer.search(query); // 150x-12,500x faster
    }
    return this.agentdb.query(query);
  }
}
```

### HNSW Vector Search
```typescript
class HNSWIndexer {
  constructor(dimensions: number = 1536) {
    this.index = new HNSWIndex({
      dimensions,
      efConstruction: 200,
      M: 16,
      speedupTarget: '150x-12500x'
    });
  }

  async search(query: MemoryQuery): Promise<MemoryEntry[]> {
    const embedding = await this.embedContent(query.content);
    const results = this.index.search(embedding, query.limit || 10);
    return this.retrieveEntries(results);
  }
}
```

## Migration Strategy

### Phase 1: Foundation
```typescript
// AgentDB adapter setup
const agentdb = new AgentDBAdapter({
  dimensions: 1536,
  indexType: 'HNSW',
  speedupTarget: '150x-12500x'
});
```

### Phase 2: Data Migration
```typescript
// SQLite → AgentDB
const migrateFromSQLite = async () => {
  const entries = await sqlite.getAll();
  for (const entry of entries) {
    const embedding = await generateEmbedding(entry.content);
    await agentdb.store({ ...entry, embedding });
  }
};

// Markdown → AgentDB
const migrateFromMarkdown = async () => {
  const files = await glob('**/*.md');
  for (const file of files) {
    const content = await fs.readFile(file, 'utf-8');
    await agentdb.store({
      id: generateId(),
      content,
      embedding: await generateEmbedding(content),
      metadata: { originalFile: file }
    });
  }
};
```

## SONA Integration

### Learning Pattern Storage
```typescript
class SONAMemoryIntegration {
  async storePattern(pattern: LearningPattern): Promise<void> {
    await this.memory.store({
      id: pattern.id,
      content: pattern.data,
      metadata: {
        sonaMode: pattern.mode,
        reward: pattern.reward,
        adaptationTime: pattern.adaptationTime
      },
      embedding: await this.generateEmbedding(pattern.data)
    });
  }

  async retrieveSimilarPatterns(query: string): Promise<LearningPattern[]> {
    return this.memory.query({
      type: 'semantic',
      content: query,
      filters: { type: 'learning_pattern' }
    });
  }
}
```

## Performance Targets

- **Search Speed**: 150x-12,500x improvement via HNSW
- **Memory Usage**: 50-75% reduction through optimization
- **Query Latency**: <100ms for 1M+ entries
- **Cross-Agent Sharing**: Real-time memory synchronization
- **SONA Integration**: <0.05ms adaptation time

## Success Metrics

- [ ] All 7 legacy memory systems migrated to AgentDB
- [ ] 150x-12,500x search performance validated
- [ ] 50-75% memory usage reduction achieved
- [ ] Backward compatibility maintained
- [ ] SONA learning patterns integrated
- [ ] Cross-agent memory sharing operational