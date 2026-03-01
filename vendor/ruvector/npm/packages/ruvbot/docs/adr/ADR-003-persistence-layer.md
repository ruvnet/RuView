# ADR-003: Persistence Layer

**Status:** Accepted
**Date:** 2026-01-27
**Decision Makers:** RuVector Architecture Team
**Technical Area:** Data Architecture, Storage

---

## Context and Problem Statement

RuvBot requires a persistence layer that handles diverse data types:

1. **Relational Data**: Users, organizations, sessions, skills (structured, transactional)
2. **Vector Data**: Embeddings for memory recall (high-dimensional, similarity search)
3. **Session State**: Active conversation context (ephemeral, fast access)
4. **Event Streams**: Audit logs, trajectories (append-only, time-series)

The persistence layer must support:

- **Multi-tenancy** with strict isolation
- **High performance** for real-time conversation
- **Durability** for compliance and recovery
- **Scalability** for enterprise deployments

---

## Decision Drivers

### Data Characteristics

| Data Type | Volume | Access Pattern | Consistency | Durability |
|-----------|--------|----------------|-------------|------------|
| User/Org metadata | Low | Read-heavy | Strong | Required |
| Session state | Medium | Read-write balanced | Eventual OK | Nice-to-have |
| Conversation history | High | Append-mostly | Strong | Required |
| Vector embeddings | Very High | Read-heavy | Eventual OK | Required |
| Memory indices | High | Read-heavy | Eventual OK | Nice-to-have |
| Audit logs | Very High | Append-only | Strong | Required |

### Performance Requirements

| Operation | Target Latency | Target Throughput |
|-----------|----------------|-------------------|
| Session lookup | < 5ms p99 | 10K/s |
| Memory recall (HNSW) | < 50ms p99 | 1K/s |
| Conversation insert | < 20ms p99 | 5K/s |
| Full-text search | < 100ms p99 | 500/s |
| Batch embedding insert | < 500ms p99 | 100 batches/s |

---

## Decision Outcome

### Adopt Polyglot Persistence with Unified API

We implement a three-tier storage architecture:

```
+-----------------------------------------------------------------------------+
|                           PERSISTENCE LAYER                                  |
+-----------------------------------------------------------------------------+

                    +--------------------------+
                    |    Persistence Gateway   |
                    |    (Unified API)         |
                    +-------------+------------+
                                  |
          +-----------------------+-----------------------+
          |                       |                       |
+---------v---------+   +---------v---------+   +---------v---------+
|    PostgreSQL     |   |     RuVector      |   |      Redis        |
|    (Primary)      |   |   (Vector Store)  |   |     (Cache)       |
|-------------------|   |-------------------|   |-------------------|
| - User/Org data   |   | - Embeddings      |   | - Session state   |
| - Conversations   |   | - HNSW indices    |   | - Rate limits     |
| - Skills config   |   | - Pattern store   |   | - Pub/Sub         |
| - Audit logs      |   | - Similarity      |   | - Job queues      |
| - RLS isolation   |   | - Learning data   |   | - Leaderboard     |
+-------------------+   +-------------------+   +-------------------+
```

---

## PostgreSQL Schema

### Core Tables

```sql
-- Extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";  -- Full-text search

-- Organizations (tenant root)
CREATE TABLE organizations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR(255) NOT NULL,
  slug VARCHAR(100) NOT NULL UNIQUE,
  plan VARCHAR(50) NOT NULL DEFAULT 'free',
  settings JSONB NOT NULL DEFAULT '{}',
  quotas JSONB NOT NULL DEFAULT '{}',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX organizations_slug_idx ON organizations (slug);

-- Workspaces (project boundary)
CREATE TABLE workspaces (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  name VARCHAR(255) NOT NULL,
  slug VARCHAR(100) NOT NULL,
  settings JSONB NOT NULL DEFAULT '{}',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (org_id, slug)
);

CREATE INDEX workspaces_org_idx ON workspaces (org_id);

-- Users
CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  email VARCHAR(255) NOT NULL,
  password_hash VARCHAR(255),  -- NULL for OAuth users
  display_name VARCHAR(255),
  avatar_url VARCHAR(500),
  roles TEXT[] NOT NULL DEFAULT '{"member"}',
  preferences JSONB NOT NULL DEFAULT '{}',
  email_verified_at TIMESTAMPTZ,
  last_login_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (org_id, email)
);

CREATE INDEX users_org_idx ON users (org_id);
CREATE INDEX users_email_idx ON users (email);

-- Workspace memberships
CREATE TABLE workspace_memberships (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role VARCHAR(50) NOT NULL DEFAULT 'member',
  joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (workspace_id, user_id)
);

CREATE INDEX workspace_memberships_user_idx ON workspace_memberships (user_id);
```

### Session and Conversation Tables

```sql
-- Agents (bot configurations)
CREATE TABLE agents (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name VARCHAR(255) NOT NULL,
  description TEXT,
  persona JSONB NOT NULL DEFAULT '{}',
  skill_ids UUID[] NOT NULL DEFAULT '{}',
  memory_config JSONB NOT NULL DEFAULT '{}',
  status VARCHAR(50) NOT NULL DEFAULT 'active',
  version INTEGER NOT NULL DEFAULT 1,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE agents ENABLE ROW LEVEL SECURITY;
CREATE POLICY agents_isolation ON agents
  FOR ALL USING (org_id = current_tenant_id());

CREATE INDEX agents_org_workspace_idx ON agents (org_id, workspace_id);

-- Sessions (conversation containers)
CREATE TABLE sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL,
  workspace_id UUID NOT NULL,
  agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  channel VARCHAR(50) NOT NULL DEFAULT 'api',  -- api, slack, webhook
  channel_id VARCHAR(255),  -- External channel identifier
  state VARCHAR(50) NOT NULL DEFAULT 'active',
  context_snapshot JSONB,  -- Serialized context for recovery
  turn_count INTEGER NOT NULL DEFAULT 0,
  token_count INTEGER NOT NULL DEFAULT 0,
  started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  last_active_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  expires_at TIMESTAMPTZ NOT NULL,
  ended_at TIMESTAMPTZ
) PARTITION BY LIST (org_id);

ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;
CREATE POLICY sessions_isolation ON sessions
  FOR ALL USING (org_id = current_tenant_id());

CREATE INDEX sessions_user_active_idx ON sessions (user_id, state)
  WHERE state = 'active';
CREATE INDEX sessions_agent_idx ON sessions (agent_id);
CREATE INDEX sessions_expires_idx ON sessions (expires_at)
  WHERE state = 'active';

-- Conversation turns
CREATE TABLE conversation_turns (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL,
  workspace_id UUID NOT NULL,
  session_id UUID NOT NULL,
  user_id UUID NOT NULL,
  role VARCHAR(20) NOT NULL,  -- user, assistant, system, tool
  content TEXT NOT NULL,
  content_type VARCHAR(50) NOT NULL DEFAULT 'text',
  embedding_id UUID,  -- Reference to vector store
  tool_calls JSONB,  -- Function/skill calls
  tool_results JSONB,  -- Function/skill results
  metadata JSONB NOT NULL DEFAULT '{}',
  token_count INTEGER,
  latency_ms INTEGER,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
) PARTITION BY LIST (org_id);

ALTER TABLE conversation_turns ENABLE ROW LEVEL SECURITY;
CREATE POLICY turns_isolation ON conversation_turns
  FOR ALL USING (org_id = current_tenant_id());

-- Composite index for session history queries
CREATE INDEX turns_session_time_idx ON conversation_turns (session_id, created_at DESC);
CREATE INDEX turns_embedding_idx ON conversation_turns (embedding_id)
  WHERE embedding_id IS NOT NULL;
```

### Memory Tables

```sql
-- Memory entries (facts, events stored for recall)
CREATE TABLE memories (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL,
  workspace_id UUID NOT NULL,
  user_id UUID,  -- NULL for workspace-level memories
  memory_type VARCHAR(50) NOT NULL,  -- episodic, semantic, procedural
  content TEXT NOT NULL,
  embedding_id UUID NOT NULL,  -- Reference to vector store
  source_type VARCHAR(50),  -- conversation, import, skill
  source_id UUID,  -- Reference to source entity
  importance FLOAT NOT NULL DEFAULT 0.5,  -- 0-1 importance score
  access_count INTEGER NOT NULL DEFAULT 0,
  last_accessed_at TIMESTAMPTZ,
  is_shared BOOLEAN NOT NULL DEFAULT FALSE,
  expires_at TIMESTAMPTZ,
  metadata JSONB NOT NULL DEFAULT '{}',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
) PARTITION BY LIST (org_id);

ALTER TABLE memories ENABLE ROW LEVEL SECURITY;

-- User-scoped memories
CREATE POLICY memories_user_isolation ON memories
  FOR ALL USING (
    org_id = current_tenant_id()
    AND workspace_id = current_workspace_id()
    AND (user_id = current_user_id() OR user_id IS NULL)
  );

-- Shared memories (read-only across workspace)
CREATE POLICY memories_shared_read ON memories
  FOR SELECT USING (
    org_id = current_tenant_id()
    AND is_shared = TRUE
  );

CREATE INDEX memories_workspace_type_idx ON memories (workspace_id, memory_type);
CREATE INDEX memories_user_type_idx ON memories (user_id, memory_type)
  WHERE user_id IS NOT NULL;
CREATE INDEX memories_embedding_idx ON memories (embedding_id);
CREATE INDEX memories_importance_idx ON memories (importance DESC);
CREATE INDEX memories_access_idx ON memories (last_accessed_at DESC);

-- Memory relationships (for graph traversal)
CREATE TABLE memory_edges (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL,
  source_memory_id UUID NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
  target_memory_id UUID NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
  edge_type VARCHAR(50) NOT NULL,  -- related_to, caused_by, part_of, supersedes
  weight FLOAT NOT NULL DEFAULT 1.0,
  metadata JSONB NOT NULL DEFAULT '{}',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE memory_edges ENABLE ROW LEVEL SECURITY;
CREATE POLICY edges_isolation ON memory_edges
  FOR ALL USING (org_id = current_tenant_id());

CREATE INDEX memory_edges_source_idx ON memory_edges (source_memory_id);
CREATE INDEX memory_edges_target_idx ON memory_edges (target_memory_id);
CREATE INDEX memory_edges_type_idx ON memory_edges (edge_type);
```

### Skills and Learning Tables

```sql
-- Skills (registered capabilities)
CREATE TABLE skills (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL,
  workspace_id UUID,  -- NULL for org-wide skills
  name VARCHAR(255) NOT NULL,
  description TEXT,
  version VARCHAR(50) NOT NULL DEFAULT '1.0.0',
  triggers JSONB NOT NULL DEFAULT '[]',
  parameters JSONB NOT NULL DEFAULT '{}',
  implementation_type VARCHAR(50) NOT NULL,  -- builtin, script, webhook
  implementation JSONB NOT NULL,  -- Type-specific config
  hooks JSONB NOT NULL DEFAULT '{}',
  is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
  usage_count INTEGER NOT NULL DEFAULT 0,
  success_rate FLOAT,
  avg_latency_ms FLOAT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE skills ENABLE ROW LEVEL SECURITY;
CREATE POLICY skills_isolation ON skills
  FOR ALL USING (org_id = current_tenant_id());

CREATE INDEX skills_workspace_idx ON skills (workspace_id);
CREATE INDEX skills_enabled_idx ON skills (is_enabled) WHERE is_enabled = TRUE;

-- Trajectories (learning data)
CREATE TABLE trajectories (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL,
  workspace_id UUID NOT NULL,
  session_id UUID NOT NULL,
  turn_ids UUID[] NOT NULL,
  skill_ids UUID[],
  start_time TIMESTAMPTZ NOT NULL,
  end_time TIMESTAMPTZ NOT NULL,
  verdict VARCHAR(50),  -- positive, negative, neutral, pending
  verdict_reason TEXT,
  metrics JSONB NOT NULL DEFAULT '{}',
  embedding_id UUID,
  is_exported BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE trajectories ENABLE ROW LEVEL SECURITY;
CREATE POLICY trajectories_isolation ON trajectories
  FOR ALL USING (org_id = current_tenant_id());

CREATE INDEX trajectories_session_idx ON trajectories (session_id);
CREATE INDEX trajectories_verdict_idx ON trajectories (verdict)
  WHERE verdict IS NOT NULL;
CREATE INDEX trajectories_export_idx ON trajectories (is_exported)
  WHERE is_exported = FALSE;

-- Learned patterns
CREATE TABLE learned_patterns (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL,
  workspace_id UUID,  -- NULL for org-wide patterns
  pattern_type VARCHAR(50) NOT NULL,  -- response, routing, skill_selection
  embedding_id UUID NOT NULL,
  exemplar_trajectory_ids UUID[] NOT NULL,
  confidence FLOAT NOT NULL,
  usage_count INTEGER NOT NULL DEFAULT 0,
  success_count INTEGER NOT NULL DEFAULT 0,
  is_active BOOLEAN NOT NULL DEFAULT TRUE,
  superseded_by UUID REFERENCES learned_patterns(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE learned_patterns ENABLE ROW LEVEL SECURITY;
CREATE POLICY patterns_isolation ON learned_patterns
  FOR ALL USING (org_id = current_tenant_id());

CREATE INDEX patterns_type_idx ON learned_patterns (pattern_type);
CREATE INDEX patterns_active_idx ON learned_patterns (is_active)
  WHERE is_active = TRUE;
CREATE INDEX patterns_embedding_idx ON learned_patterns (embedding_id);
```

---

## RuVector Integration

### Vector Store Adapter

```typescript
// Unified vector store interface
interface RuVectorAdapter {
  // Index management
  createIndex(config: IndexConfig): Promise<IndexHandle>;
  deleteIndex(handle: IndexHandle): Promise<void>;
  getIndex(namespace: string): Promise<IndexHandle | null>;

  // Vector operations
  insert(handle: IndexHandle, entries: VectorEntry[]): Promise<void>;
  update(handle: IndexHandle, id: string, vector: Float32Array): Promise<void>;
  delete(handle: IndexHandle, ids: string[]): Promise<void>;

  // Search operations
  search(handle: IndexHandle, query: Float32Array, options: SearchOptions): Promise<SearchResult[]>;
  batchSearch(handle: IndexHandle, queries: Float32Array[], options: SearchOptions): Promise<SearchResult[][]>;

  // Index operations
  optimize(handle: IndexHandle): Promise<OptimizationResult>;
  stats(handle: IndexHandle): Promise<IndexStats>;
}

interface IndexConfig {
  namespace: string;
  dimensions: number;
  distanceMetric: 'cosine' | 'euclidean' | 'dot_product';
  hnsw: {
    m: number;
    efConstruction: number;
    efSearch: number;
  };
  quantization?: {
    type: 'scalar' | 'product' | 'binary';
    bits?: number;
  };
}

interface VectorEntry {
  id: string;
  vector: Float32Array;
  metadata?: Record<string, unknown>;
}

interface SearchResult {
  id: string;
  score: number;
  metadata?: Record<string, unknown>;
}
```

### Namespace Schema

```typescript
// Vector namespace organization
const VECTOR_NAMESPACES = {
  // Memory embeddings
  EPISODIC: (orgId: string, workspaceId: string) =>
    `${orgId}/${workspaceId}/memory/episodic`,
  SEMANTIC: (orgId: string, workspaceId: string) =>
    `${orgId}/${workspaceId}/memory/semantic`,
  PROCEDURAL: (orgId: string, workspaceId: string) =>
    `${orgId}/${workspaceId}/memory/procedural`,

  // Conversation embeddings
  CONVERSATIONS: (orgId: string, workspaceId: string) =>
    `${orgId}/${workspaceId}/conversations`,

  // Learning embeddings
  TRAJECTORIES: (orgId: string, workspaceId: string) =>
    `${orgId}/${workspaceId}/learning/trajectories`,
  PATTERNS: (orgId: string, workspaceId: string) =>
    `${orgId}/${workspaceId}/learning/patterns`,

  // Skill embeddings (for intent matching)
  SKILLS: (orgId: string) =>
    `${orgId}/skills`,
};

// Index configuration per namespace type
const INDEX_CONFIGS: Record<string, Partial<IndexConfig>> = {
  'memory/episodic': {
    dimensions: 384,
    distanceMetric: 'cosine',
    hnsw: { m: 16, efConstruction: 100, efSearch: 50 },
  },
  'memory/semantic': {
    dimensions: 384,
    distanceMetric: 'cosine',
    hnsw: { m: 32, efConstruction: 200, efSearch: 100 },
  },
  'conversations': {
    dimensions: 384,
    distanceMetric: 'cosine',
    hnsw: { m: 16, efConstruction: 100, efSearch: 50 },
    quantization: { type: 'scalar' },  // Compress for volume
  },
  'learning/patterns': {
    dimensions: 384,
    distanceMetric: 'cosine',
    hnsw: { m: 32, efConstruction: 200, efSearch: 100 },
  },
};
```

### WASM/Native Detection

```typescript
// Automatic runtime detection
class RuVectorFactory {
  private static instance: RuVectorAdapter | null = null;

  static async create(): Promise<RuVectorAdapter> {
    if (this.instance) return this.instance;

    // Try native first (better performance)
    try {
      const native = await import('@ruvector/core');
      if (native.isNativeAvailable()) {
        console.log('RuVector: Using native NAPI bindings');
        this.instance = new NativeRuVectorAdapter(native);
        return this.instance;
      }
    } catch (e) {
      console.debug('Native bindings not available:', e);
    }

    // Fall back to WASM
    try {
      const wasm = await import('@ruvector/wasm');
      console.log('RuVector: Using WASM runtime');
      this.instance = new WasmRuVectorAdapter(wasm);
      return this.instance;
    } catch (e) {
      throw new Error(`Failed to load RuVector runtime: ${e}`);
    }
  }
}
```

---

## Redis Schema

### Session Cache

```typescript
// Session state keys
const SESSION_KEYS = {
  // Active session state
  state: (sessionId: string) => `session:${sessionId}:state`,

  // Context window (recent turns)
  context: (sessionId: string) => `session:${sessionId}:context`,

  // Session lock (prevent concurrent modifications)
  lock: (sessionId: string) => `session:${sessionId}:lock`,

  // User's active sessions
  userSessions: (userId: string) => `user:${userId}:sessions`,

  // Session expiry sorted set
  expiryIndex: () => 'sessions:expiry',
};

// Session state structure
interface CachedSessionState {
  id: string;
  agentId: string;
  userId: string;
  state: SessionState;
  turnCount: number;
  tokenCount: number;
  lastActiveAt: number;
  expiresAt: number;
}

// Context window structure
interface CachedContextWindow {
  maxTokens: number;
  turns: Array<{
    id: string;
    role: string;
    content: string;
    createdAt: number;
  }>;
  retrievedMemoryIds: string[];
}
```

### Rate Limiting

```typescript
// Rate limit keys
const RATE_LIMIT_KEYS = {
  // Per-tenant rate limits
  tenant: (tenantId: string, action: string, window: string) =>
    `ratelimit:tenant:${tenantId}:${action}:${window}`,

  // Per-user rate limits
  user: (userId: string, action: string, window: string) =>
    `ratelimit:user:${userId}:${action}:${window}`,

  // Global rate limits
  global: (action: string, window: string) =>
    `ratelimit:global:${action}:${window}`,
};

// Rate limit actions
type RateLimitAction =
  | 'api_request'
  | 'llm_call'
  | 'embedding_request'
  | 'memory_write'
  | 'skill_execute'
  | 'webhook_dispatch';
```

### Pub/Sub Channels

```typescript
// Real-time event channels
const PUBSUB_CHANNELS = {
  // Session events
  sessionCreated: (workspaceId: string) =>
    `events:${workspaceId}:session:created`,
  sessionEnded: (workspaceId: string) =>
    `events:${workspaceId}:session:ended`,

  // Conversation events
  turnCreated: (sessionId: string) =>
    `events:session:${sessionId}:turn:created`,

  // Memory events
  memoryCreated: (workspaceId: string) =>
    `events:${workspaceId}:memory:created`,
  memoryUpdated: (workspaceId: string) =>
    `events:${workspaceId}:memory:updated`,

  // Skill events
  skillExecuted: (workspaceId: string) =>
    `events:${workspaceId}:skill:executed`,

  // System events
  quotaWarning: (tenantId: string) =>
    `events:${tenantId}:quota:warning`,
};
```

---

## Data Access Patterns

### Repository Pattern

```typescript
// Base repository with tenant context
abstract class TenantRepository<T> {
  constructor(
    protected db: PostgresAdapter,
    protected tenantContext: TenantContext
  ) {}

  protected async withTenantContext<R>(
    fn: (db: PostgresAdapter) => Promise<R>
  ): Promise<R> {
    // Set tenant context for RLS
    await this.db.query(`
      SELECT set_config('app.current_org_id', $1, true),
             set_config('app.current_workspace_id', $2, true),
             set_config('app.current_user_id', $3, true)
    `, [
      this.tenantContext.orgId,
      this.tenantContext.workspaceId,
      this.tenantContext.userId,
    ]);

    return fn(this.db);
  }

  abstract findById(id: string): Promise<T | null>;
  abstract save(entity: T): Promise<T>;
  abstract delete(id: string): Promise<void>;
}

// Memory repository example
class MemoryRepository extends TenantRepository<Memory> {
  async findById(id: string): Promise<Memory | null> {
    return this.withTenantContext(async (db) => {
      const rows = await db.query<MemoryRow>(
        'SELECT * FROM memories WHERE id = $1',
        [id]
      );
      return rows[0] ? this.toEntity(rows[0]) : null;
    });
  }

  async findByEmbedding(
    embedding: Float32Array,
    options: MemorySearchOptions
  ): Promise<MemoryWithScore[]> {
    // Search vector store first
    const vectorResults = await this.vectorStore.search(
      this.getIndexHandle(),
      embedding,
      { k: options.limit, threshold: options.minScore }
    );

    if (vectorResults.length === 0) return [];

    // Fetch full memory records
    return this.withTenantContext(async (db) => {
      const ids = vectorResults.map(r => r.id);
      const scoreMap = new Map(vectorResults.map(r => [r.id, r.score]));

      const rows = await db.query<MemoryRow>(
        'SELECT * FROM memories WHERE id = ANY($1)',
        [ids]
      );

      return rows
        .map(row => ({
          memory: this.toEntity(row),
          score: scoreMap.get(row.id) ?? 0,
        }))
        .sort((a, b) => b.score - a.score);
    });
  }

  async save(memory: Memory): Promise<Memory> {
    return this.withTenantContext(async (db) => {
      // Generate embedding if not present
      if (!memory.embeddingId) {
        const embedding = await this.embedder.embed(memory.content);
        const embeddingId = crypto.randomUUID();

        await this.vectorStore.insert(this.getIndexHandle(), [{
          id: embeddingId,
          vector: embedding,
          metadata: { memoryId: memory.id },
        }]);

        memory.embeddingId = embeddingId;
      }

      // Upsert to database
      const row = await db.query<MemoryRow>(`
        INSERT INTO memories (
          id, org_id, workspace_id, user_id, memory_type, content,
          embedding_id, source_type, source_id, importance, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        ON CONFLICT (id) DO UPDATE SET
          content = EXCLUDED.content,
          importance = EXCLUDED.importance,
          metadata = EXCLUDED.metadata,
          updated_at = NOW()
        RETURNING *
      `, [
        memory.id,
        this.tenantContext.orgId,
        this.tenantContext.workspaceId,
        memory.userId,
        memory.type,
        memory.content,
        memory.embeddingId,
        memory.sourceType,
        memory.sourceId,
        memory.importance,
        memory.metadata,
      ]);

      return this.toEntity(row[0]);
    });
  }

  private getIndexHandle(): IndexHandle {
    return {
      namespace: VECTOR_NAMESPACES[this.tenantContext.workspaceId]
        ? VECTOR_NAMESPACES.EPISODIC(
            this.tenantContext.orgId,
            this.tenantContext.workspaceId
          )
        : VECTOR_NAMESPACES.SEMANTIC(
            this.tenantContext.orgId,
            this.tenantContext.workspaceId
          ),
    };
  }
}
```

### Unit of Work Pattern

```typescript
// Transaction coordination
class UnitOfWork {
  private operations: Operation[] = [];
  private committed = false;

  constructor(
    private db: PostgresAdapter,
    private vectorStore: RuVectorAdapter,
    private cache: CacheAdapter
  ) {}

  addMemory(memory: Memory): void {
    this.operations.push({
      type: 'memory',
      action: 'upsert',
      entity: memory,
    });
  }

  addTurn(turn: ConversationTurn): void {
    this.operations.push({
      type: 'turn',
      action: 'insert',
      entity: turn,
    });
  }

  async commit(): Promise<void> {
    if (this.committed) throw new Error('Already committed');

    try {
      await this.db.transaction(async (tx) => {
        // Execute database operations
        for (const op of this.operations.filter(o => o.type !== 'cache')) {
          await this.executeDbOperation(tx, op);
        }

        // Execute vector operations (outside transaction, but after DB success)
        for (const op of this.operations.filter(o =>
          o.type === 'memory' || o.type === 'turn'
        )) {
          await this.executeVectorOperation(op);
        }
      });

      // Execute cache operations (best effort)
      for (const op of this.operations.filter(o => o.type === 'cache')) {
        await this.executeCacheOperation(op).catch(console.error);
      }

      this.committed = true;
    } catch (error) {
      // Rollback vector operations on failure
      await this.rollbackVectorOperations();
      throw error;
    }
  }
}
```

---

## Migration Strategy

### Schema Migrations

```typescript
// Migration runner
class MigrationRunner {
  async migrate(direction: 'up' | 'down' = 'up'): Promise<void> {
    const migrations = await this.loadMigrations();
    const applied = await this.getAppliedMigrations();

    if (direction === 'up') {
      const pending = migrations.filter(m => !applied.has(m.version));
      for (const migration of pending) {
        await this.applyMigration(migration);
      }
    } else {
      const toRollback = [...applied].reverse();
      for (const version of toRollback) {
        const migration = migrations.find(m => m.version === version);
        if (migration) {
          await this.rollbackMigration(migration);
        }
      }
    }
  }

  private async applyMigration(migration: Migration): Promise<void> {
    await this.db.transaction(async (tx) => {
      // Run migration SQL
      await tx.query(migration.up);

      // Record migration
      await tx.query(
        'INSERT INTO schema_migrations (version, applied_at) VALUES ($1, NOW())',
        [migration.version]
      );
    });

    console.log(`Applied migration: ${migration.version}`);
  }
}

// Example migration
const MIGRATION_001: Migration = {
  version: '001_initial_schema',
  up: `
    -- Create organizations table
    CREATE TABLE organizations (...);

    -- Create workspaces table
    CREATE TABLE workspaces (...);

    -- ... rest of schema
  `,
  down: `
    DROP TABLE IF EXISTS workspaces;
    DROP TABLE IF EXISTS organizations;
  `,
};
```

---

## Consequences

### Benefits

1. **Strong Isolation**: RLS + namespace isolation at every layer
2. **Performance**: Optimized indices, caching, and partitioning
3. **Flexibility**: Polyglot persistence matches data characteristics
4. **Durability**: PostgreSQL for critical data, redundant vector storage
5. **Scalability**: Horizontal scaling via partitions and Redis cluster

### Trade-offs

| Benefit | Trade-off |
|---------|-----------|
| RLS security | Slight query overhead |
| HNSW speed | Memory consumption |
| Redis caching | Consistency complexity |
| Polyglot persistence | Operational complexity |

---

## Related Decisions

- **ADR-001**: Architecture Overview
- **ADR-002**: Multi-tenancy Design
- **ADR-006**: WASM Integration (vector store runtime)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-27 | RuVector Architecture Team | Initial version |
