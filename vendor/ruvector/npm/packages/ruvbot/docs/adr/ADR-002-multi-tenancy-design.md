# ADR-002: Multi-tenancy Design

**Status:** Accepted
**Date:** 2026-01-27
**Decision Makers:** RuVector Architecture Team
**Technical Area:** Security, Data Architecture

---

## Context and Problem Statement

RuvBot must serve multiple organizations (tenants) and users within each organization while maintaining strict data isolation. A breach of tenant boundaries would:

1. Violate privacy and compliance requirements (GDPR, SOC2, HIPAA)
2. Expose sensitive business information
3. Destroy trust in the platform
4. Create legal liability

The multi-tenancy design must address:

- **Data Isolation**: No cross-tenant data access
- **Authentication**: Identity verification at multiple levels
- **Authorization**: Fine-grained permission control
- **Resource Limits**: Fair usage and cost allocation
- **Audit Trails**: Complete visibility into access patterns

---

## Decision Drivers

### Security Requirements

| Requirement | Criticality | Description |
|-------------|-------------|-------------|
| Zero cross-tenant leakage | Critical | No tenant can access another tenant's data |
| Row-level security | Critical | Database enforces isolation, not just application |
| Token-based auth | High | Stateless, revocable authentication |
| RBAC + ABAC | High | Role and attribute-based access control |
| Audit logging | High | All data access logged with tenant context |

### Operational Requirements

| Requirement | Target | Description |
|-------------|--------|-------------|
| Tenant provisioning | < 30s | New tenant setup time |
| User provisioning | < 5s | New user creation time |
| Quota enforcement | Real-time | Immediate limit enforcement |
| Data export | < 1h for 1GB | GDPR data portability |
| Data deletion | < 24h | GDPR right to erasure |

---

## Decision Outcome

### Adopt Hierarchical Multi-tenancy with RLS and JWT Claims

We implement a three-level hierarchy with PostgreSQL Row-Level Security (RLS) as the primary isolation mechanism.

```
+---------------------------+
|        ORGANIZATION       |  Billing entity, security boundary
|---------------------------|
| id: UUID                  |
| name: string              |
| plan: Plan                |
| settings: OrgSettings     |
| quotas: ResourceQuotas    |
+-------------+-------------+
              |
              | 1:N
              v
+---------------------------+
|         WORKSPACE         |  Project/team boundary
|---------------------------|
| id: UUID                  |
| orgId: UUID (FK)          |
| name: string              |
| settings: WorkspaceSettings|
+-------------+-------------+
              |
              | 1:N
              v
+---------------------------+
|           USER            |  Individual identity
|---------------------------|
| id: UUID                  |
| workspaceId: UUID (FK)    |
| email: string             |
| roles: Role[]             |
| preferences: Preferences  |
+---------------------------+
```

---

## Tenant Isolation Layers

### Layer 1: Network Isolation

```
Internet
    |
    v
+---+---+
|  WAF  |  Rate limiting, DDoS protection
+---+---+
    |
    v
+---+---+
| LB/TLS|  TLS termination, tenant routing
+---+---+
    |
    +--------+--------+--------+
    |        |        |        |
+---v---+ +---v---+ +---v---+ +---v---+
| Org A | | Org B | | Org C | | Org D |  Virtual host routing
+-------+ +-------+ +-------+ +-------+
```

### Layer 2: Authentication & Authorization

```typescript
// JWT token structure with tenant claims
interface RuvBotToken {
  // Standard claims
  sub: string;          // User ID
  iat: number;          // Issued at
  exp: number;          // Expiration

  // Tenant claims (always present)
  org_id: string;       // Organization ID
  workspace_id: string; // Workspace ID

  // Permission claims
  roles: Role[];        // User roles
  permissions: string[];// Explicit permissions

  // Resource claims
  quotas: {
    sessions: number;
    messages_per_day: number;
    memory_mb: number;
  };
}

// Role hierarchy
enum Role {
  ORG_OWNER = 'org:owner',
  ORG_ADMIN = 'org:admin',
  WORKSPACE_ADMIN = 'workspace:admin',
  MEMBER = 'member',
  VIEWER = 'viewer',
  API_KEY = 'api_key',
}

// Permission matrix
const PERMISSIONS: Record<Role, string[]> = {
  'org:owner': ['*'],
  'org:admin': ['org:read', 'org:write', 'workspace:*', 'user:*', 'billing:read'],
  'workspace:admin': ['workspace:read', 'workspace:write', 'user:read', 'user:invite'],
  'member': ['session:*', 'memory:read', 'memory:write', 'skill:execute'],
  'viewer': ['session:read', 'memory:read'],
  'api_key': ['session:create', 'session:read'],
};
```

### Layer 3: Database Row-Level Security

```sql
-- Enable RLS on all tenant-scoped tables
ALTER TABLE conversations ENABLE ROW LEVEL SECURITY;
ALTER TABLE memories ENABLE ROW LEVEL SECURITY;
ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE skills ENABLE ROW LEVEL SECURITY;
ALTER TABLE trajectories ENABLE ROW LEVEL SECURITY;

-- Create tenant context function
CREATE OR REPLACE FUNCTION current_tenant_id()
RETURNS UUID AS $$
BEGIN
  RETURN current_setting('app.current_org_id', true)::UUID;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

CREATE OR REPLACE FUNCTION current_workspace_id()
RETURNS UUID AS $$
BEGIN
  RETURN current_setting('app.current_workspace_id', true)::UUID;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- RLS policies for conversations
CREATE POLICY conversations_isolation ON conversations
  FOR ALL
  USING (org_id = current_tenant_id())
  WITH CHECK (org_id = current_tenant_id());

-- RLS policies for memories (workspace-level)
CREATE POLICY memories_isolation ON memories
  FOR ALL
  USING (
    org_id = current_tenant_id()
    AND workspace_id = current_workspace_id()
  );

-- Read-only policy for cross-workspace memory sharing
CREATE POLICY memories_shared_read ON memories
  FOR SELECT
  USING (
    org_id = current_tenant_id()
    AND is_shared = true
  );
```

### Layer 4: Vector Store Isolation

```typescript
// Namespace isolation in RuVector
interface VectorNamespace {
  // Namespace format: {org_id}/{workspace_id}/{collection}
  // Example: "550e8400-e29b/.../episodic"

  encode(orgId: string, workspaceId: string, collection: string): string;
  decode(namespace: string): { orgId: string; workspaceId: string; collection: string };
  validate(namespace: string, token: RuvBotToken): boolean;
}

// Vector store with tenant isolation
class TenantIsolatedVectorStore {
  constructor(
    private store: RuVectorAdapter,
    private tenantContext: TenantContext
  ) {}

  async search(query: Float32Array, options: SearchOptions): Promise<SearchResult[]> {
    const namespace = this.getNamespace(options.collection);

    // Validate namespace matches token claims
    if (!this.validateNamespace(namespace)) {
      throw new TenantIsolationError('Namespace mismatch');
    }

    return this.store.search(query, { ...options, namespace });
  }

  private getNamespace(collection: string): string {
    return `${this.tenantContext.orgId}/${this.tenantContext.workspaceId}/${collection}`;
  }

  private validateNamespace(namespace: string): boolean {
    const { orgId, workspaceId } = VectorNamespace.decode(namespace);
    return (
      orgId === this.tenantContext.orgId &&
      workspaceId === this.tenantContext.workspaceId
    );
  }
}
```

---

## Data Partitioning Strategy

### PostgreSQL Partitioning

```sql
-- Partition conversations by org_id for isolation and performance
CREATE TABLE conversations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id UUID NOT NULL,
  workspace_id UUID NOT NULL,
  session_id UUID NOT NULL,
  user_id UUID NOT NULL,
  content TEXT NOT NULL,
  role VARCHAR(20) NOT NULL,
  embedding_id UUID,
  metadata JSONB,
  created_at TIMESTAMPTZ DEFAULT NOW()
) PARTITION BY LIST (org_id);

-- Create partition per organization
CREATE OR REPLACE FUNCTION create_org_partition(org_id UUID)
RETURNS void AS $$
DECLARE
  partition_name TEXT;
BEGIN
  partition_name := 'conversations_' || replace(org_id::text, '-', '_');
  EXECUTE format(
    'CREATE TABLE IF NOT EXISTS %I PARTITION OF conversations FOR VALUES IN (%L)',
    partition_name,
    org_id
  );
END;
$$ LANGUAGE plpgsql;

-- Indexes per partition
CREATE INDEX CONCURRENTLY conversations_session_idx
  ON conversations (session_id, created_at DESC);
CREATE INDEX CONCURRENTLY conversations_user_idx
  ON conversations (user_id, created_at DESC);
CREATE INDEX CONCURRENTLY conversations_embedding_idx
  ON conversations (embedding_id) WHERE embedding_id IS NOT NULL;
```

### Vector Store Partitioning

```typescript
// HNSW index per tenant for isolation and independent scaling
interface TenantVectorIndex {
  orgId: string;
  workspaceId: string;
  collection: 'episodic' | 'semantic' | 'skills';

  // Index configuration (can vary per tenant plan)
  config: {
    dimensions: number;     // 384 for MiniLM, 1536 for larger models
    m: number;              // HNSW connections (16-32)
    efConstruction: number; // Build quality (100-200)
    efSearch: number;       // Query quality (50-100)
  };

  // Usage metrics
  metrics: {
    vectorCount: number;
    memoryUsageMB: number;
    avgSearchLatencyMs: number;
    lastOptimized: Date;
  };
}

// Index lifecycle management
class TenantIndexManager {
  async provisionTenant(orgId: string): Promise<void> {
    // Create default workspaces indices
    await this.createIndex(orgId, 'default', 'episodic');
    await this.createIndex(orgId, 'default', 'semantic');
    await this.createIndex(orgId, 'default', 'skills');
  }

  async deleteTenant(orgId: string): Promise<void> {
    // Delete all indices for org (GDPR deletion)
    const indices = await this.listIndices(orgId);
    await Promise.all(indices.map(idx => this.deleteIndex(idx.id)));

    // Log deletion for audit
    await this.auditLog.record({
      action: 'tenant_deletion',
      orgId,
      indexCount: indices.length,
      timestamp: new Date(),
    });
  }

  async optimizeIndex(indexId: string): Promise<OptimizationResult> {
    // Background optimization with tenant resource limits
    const index = await this.getIndex(indexId);
    const quota = await this.getQuota(index.orgId);

    if (index.metrics.memoryUsageMB > quota.maxVectorMemoryMB) {
      // Apply quantization to reduce memory
      return this.compressIndex(indexId, 'product_quantization');
    }

    return this.rebalanceIndex(indexId);
  }
}
```

---

## Authentication Flows

### OAuth2/OIDC Flow

```
+--------+                               +--------+
|  User  |                               |  IdP   |
+---+----+                               +---+----+
    |                                        |
    |  1. Login request                      |
    +--------------------------------------->|
    |                                        |
    |  2. Redirect to IdP                    |
    |<---------------------------------------+
    |                                        |
    |  3. Authenticate + consent             |
    +--------------------------------------->|
    |                                        |
    |  4. Auth code redirect                 |
    |<---------------------------------------+
    |                                        |
    |                   +--------+           |
    |  5. Auth code     | RuvBot |           |
    +------------------>|  Auth  |           |
    |                   +---+----+           |
    |                       |                |
    |  6. Exchange code     |                |
    |                       +--------------->|
    |                       |                |
    |  7. ID + Access token |                |
    |                       |<---------------+
    |                       |                |
    |  8. Create session,   |
    |     issue RuvBot JWT  |
    |<----------------------+
    |                       |
    |  9. Authenticated     |
    +<----------------------+
```

### API Key Authentication

```typescript
// API key structure
interface APIKey {
  id: string;
  keyHash: string;            // SHA-256 hash of actual key
  prefix: string;             // First 8 chars for identification
  orgId: string;
  workspaceId: string;
  name: string;
  permissions: string[];
  rateLimit: RateLimitConfig;
  expiresAt: Date | null;
  lastUsedAt: Date | null;
  createdBy: string;
  createdAt: Date;
}

// API key validation middleware
async function validateAPIKey(req: Request): Promise<TenantContext> {
  const authHeader = req.headers.authorization;
  if (!authHeader?.startsWith('Bearer ')) {
    throw new AuthenticationError('Missing authorization header');
  }

  const key = authHeader.slice(7);
  const prefix = key.slice(0, 8);
  const keyHash = crypto.createHash('sha256').update(key).digest('hex');

  // Lookup by prefix, then verify hash (timing-safe)
  const apiKey = await db.apiKeys.findByPrefix(prefix);
  if (!apiKey || !crypto.timingSafeEqual(
    Buffer.from(apiKey.keyHash),
    Buffer.from(keyHash)
  )) {
    throw new AuthenticationError('Invalid API key');
  }

  // Check expiration
  if (apiKey.expiresAt && apiKey.expiresAt < new Date()) {
    throw new AuthenticationError('API key expired');
  }

  // Update last used (async, don't block)
  db.apiKeys.updateLastUsed(apiKey.id).catch(console.error);

  return {
    orgId: apiKey.orgId,
    workspaceId: apiKey.workspaceId,
    userId: apiKey.createdBy,
    roles: [Role.API_KEY],
    permissions: apiKey.permissions,
  };
}
```

---

## Resource Quotas and Rate Limiting

### Quota Configuration

```typescript
// Plan-based quota tiers
interface ResourceQuotas {
  // Session limits
  maxConcurrentSessions: number;
  maxSessionDurationMinutes: number;
  maxTurnsPerSession: number;

  // Memory limits
  maxMemoriesPerWorkspace: number;
  maxVectorStorageMB: number;
  maxEmbeddingsPerDay: number;

  // Compute limits
  maxLLMTokensPerDay: number;
  maxSkillExecutionsPerDay: number;
  maxBackgroundJobsPerHour: number;

  // Rate limits
  requestsPerMinute: number;
  requestsPerHour: number;
  burstLimit: number;
}

const PLAN_QUOTAS: Record<Plan, ResourceQuotas> = {
  free: {
    maxConcurrentSessions: 2,
    maxSessionDurationMinutes: 30,
    maxTurnsPerSession: 50,
    maxMemoriesPerWorkspace: 1000,
    maxVectorStorageMB: 50,
    maxEmbeddingsPerDay: 500,
    maxLLMTokensPerDay: 10000,
    maxSkillExecutionsPerDay: 100,
    maxBackgroundJobsPerHour: 10,
    requestsPerMinute: 20,
    requestsPerHour: 500,
    burstLimit: 5,
  },
  pro: {
    maxConcurrentSessions: 10,
    maxSessionDurationMinutes: 120,
    maxTurnsPerSession: 500,
    maxMemoriesPerWorkspace: 50000,
    maxVectorStorageMB: 1000,
    maxEmbeddingsPerDay: 10000,
    maxLLMTokensPerDay: 500000,
    maxSkillExecutionsPerDay: 5000,
    maxBackgroundJobsPerHour: 200,
    requestsPerMinute: 100,
    requestsPerHour: 5000,
    burstLimit: 20,
  },
  enterprise: {
    maxConcurrentSessions: -1, // Unlimited
    maxSessionDurationMinutes: -1,
    maxTurnsPerSession: -1,
    maxMemoriesPerWorkspace: -1,
    maxVectorStorageMB: -1,
    maxEmbeddingsPerDay: -1,
    maxLLMTokensPerDay: -1,
    maxSkillExecutionsPerDay: -1,
    maxBackgroundJobsPerHour: -1,
    requestsPerMinute: 500,
    requestsPerHour: 20000,
    burstLimit: 50,
  },
};
```

### Rate Limiter Implementation

```typescript
// Token bucket rate limiter with Redis backend
class TenantRateLimiter {
  constructor(private redis: Redis) {}

  async checkLimit(
    tenantId: string,
    action: string,
    config: RateLimitConfig
  ): Promise<RateLimitResult> {
    const key = `ratelimit:${tenantId}:${action}`;
    const now = Date.now();
    const windowMs = config.windowMs || 60000;

    // Lua script for atomic rate limit check
    const result = await this.redis.eval(`
      local key = KEYS[1]
      local now = tonumber(ARGV[1])
      local window = tonumber(ARGV[2])
      local limit = tonumber(ARGV[3])
      local burst = tonumber(ARGV[4])

      -- Remove expired entries
      redis.call('ZREMRANGEBYSCORE', key, 0, now - window)

      -- Count current requests
      local count = redis.call('ZCARD', key)

      -- Check burst limit (recent 1s)
      local burstCount = redis.call('ZCOUNT', key, now - 1000, now)

      if burstCount >= burst then
        return {0, limit - count, burst - burstCount, now + 1000}
      end

      if count >= limit then
        local oldest = redis.call('ZRANGE', key, 0, 0, 'WITHSCORES')
        local retryAfter = oldest[2] + window - now
        return {0, 0, burst - burstCount, retryAfter}
      end

      -- Add current request
      redis.call('ZADD', key, now, now .. ':' .. math.random())
      redis.call('PEXPIRE', key, window)

      return {1, limit - count - 1, burst - burstCount - 1, 0}
    `, 1, key, now, windowMs, config.limit, config.burstLimit);

    const [allowed, remaining, burstRemaining, retryAfter] = result as number[];

    return {
      allowed: allowed === 1,
      remaining,
      burstRemaining,
      retryAfter: retryAfter > 0 ? Math.ceil(retryAfter / 1000) : 0,
      limit: config.limit,
    };
  }
}
```

---

## Audit Logging

```typescript
// Comprehensive audit trail
interface AuditEvent {
  id: string;
  timestamp: Date;

  // Tenant context
  orgId: string;
  workspaceId: string;
  userId: string;

  // Event details
  action: AuditAction;
  resource: AuditResource;
  resourceId: string;

  // Request context
  requestId: string;
  ipAddress: string;
  userAgent: string;

  // Change tracking
  before?: Record<string, unknown>;
  after?: Record<string, unknown>;

  // Outcome
  status: 'success' | 'failure' | 'denied';
  errorCode?: string;
  errorMessage?: string;
}

type AuditAction =
  | 'create' | 'read' | 'update' | 'delete'
  | 'login' | 'logout' | 'token_refresh'
  | 'export' | 'import'
  | 'share' | 'unshare'
  | 'invite' | 'remove'
  | 'skill_execute' | 'memory_recall'
  | 'quota_exceeded' | 'rate_limited';

type AuditResource =
  | 'user' | 'session' | 'conversation'
  | 'memory' | 'skill' | 'agent'
  | 'workspace' | 'organization'
  | 'api_key' | 'webhook';

// Audit logger with async persistence
class AuditLogger {
  private buffer: AuditEvent[] = [];
  private flushInterval: NodeJS.Timeout;

  constructor(
    private storage: AuditStorage,
    private config: { batchSize: number; flushMs: number }
  ) {
    this.flushInterval = setInterval(() => this.flush(), config.flushMs);
  }

  async log(event: Omit<AuditEvent, 'id' | 'timestamp'>): Promise<void> {
    const fullEvent: AuditEvent = {
      ...event,
      id: crypto.randomUUID(),
      timestamp: new Date(),
    };

    this.buffer.push(fullEvent);

    if (this.buffer.length >= this.config.batchSize) {
      await this.flush();
    }
  }

  private async flush(): Promise<void> {
    if (this.buffer.length === 0) return;

    const events = this.buffer.splice(0, this.buffer.length);
    await this.storage.batchInsert(events);
  }

  async query(filter: AuditFilter): Promise<AuditEvent[]> {
    // Ensure tenant isolation in queries
    if (!filter.orgId) {
      throw new Error('orgId required for audit queries');
    }
    return this.storage.query(filter);
  }
}
```

---

## GDPR Compliance

### Data Export

```typescript
// Personal data export for GDPR Article 15
class DataExporter {
  async exportUserData(
    orgId: string,
    userId: string
  ): Promise<DataExportResult> {
    const export = {
      metadata: {
        userId,
        orgId,
        exportedAt: new Date(),
        format: 'json',
        version: '1.0',
      },
      data: {} as Record<string, unknown>,
    };

    // Collect all user data across contexts
    const [
      profile,
      sessions,
      conversations,
      memories,
      preferences,
      auditLogs,
    ] = await Promise.all([
      this.exportProfile(userId),
      this.exportSessions(userId),
      this.exportConversations(userId),
      this.exportMemories(userId),
      this.exportPreferences(userId),
      this.exportAuditLogs(userId),
    ]);

    export.data = {
      profile,
      sessions,
      conversations,
      memories,
      preferences,
      auditLogs,
    };

    // Generate downloadable archive
    const archivePath = await this.createArchive(export);

    // Log export for audit
    await this.auditLogger.log({
      orgId,
      workspaceId: '*',
      userId,
      action: 'export',
      resource: 'user',
      resourceId: userId,
      status: 'success',
    });

    return {
      downloadUrl: await this.generateSignedUrl(archivePath),
      expiresAt: new Date(Date.now() + 24 * 60 * 60 * 1000), // 24h
      sizeBytes: await this.getFileSize(archivePath),
    };
  }
}
```

### Data Deletion

```typescript
// Right to erasure (GDPR Article 17)
class DataDeleter {
  async deleteUserData(
    orgId: string,
    userId: string,
    options: DeletionOptions = {}
  ): Promise<DeletionResult> {
    const jobId = crypto.randomUUID();

    // Start deletion job (may take time for large datasets)
    await this.jobQueue.enqueue('data-deletion', {
      jobId,
      orgId,
      userId,
      options,
    });

    return {
      jobId,
      status: 'pending',
      estimatedCompletionTime: await this.estimateCompletionTime(userId),
    };
  }

  async executeDeletion(job: DeletionJob): Promise<void> {
    const { orgId, userId, options } = job.data;

    // Order matters: delete dependent data first
    const steps = [
      { name: 'sessions', fn: () => this.deleteSessions(userId) },
      { name: 'conversations', fn: () => this.deleteConversations(userId) },
      { name: 'memories', fn: () => this.deleteMemories(userId, options.preserveShared) },
      { name: 'embeddings', fn: () => this.deleteEmbeddings(userId) },
      { name: 'trajectories', fn: () => this.deleteTrajectories(userId) },
      { name: 'preferences', fn: () => this.deletePreferences(userId) },
      { name: 'audit_logs', fn: () => this.anonymizeAuditLogs(userId) }, // Anonymize, not delete
      { name: 'profile', fn: () => this.deleteProfile(userId) },
    ];

    for (const step of steps) {
      try {
        const result = await step.fn();
        await this.updateProgress(job.id, step.name, 'completed', result);
      } catch (error) {
        await this.updateProgress(job.id, step.name, 'failed', error);
        throw error; // Fail job, require manual intervention
      }
    }

    // Final audit entry (anonymized user reference)
    await this.auditLogger.log({
      orgId,
      workspaceId: '*',
      userId: 'DELETED_USER',
      action: 'delete',
      resource: 'user',
      resourceId: userId.slice(0, 8) + '...',
      status: 'success',
    });
  }
}
```

---

## Consequences

### Benefits

1. **Strong Isolation**: RLS + namespace isolation prevents cross-tenant access
2. **Compliance Ready**: GDPR, SOC2, HIPAA requirements addressed
3. **Scalable Quotas**: Per-tenant resource limits enable fair usage
4. **Audit Trail**: Complete visibility for security and compliance
5. **Flexible Auth**: OAuth2 + API keys support various use cases

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| RLS bypass via SQL injection | Low | Critical | Parameterized queries, ORM only |
| Token theft | Medium | High | Short expiry, refresh rotation |
| Quota gaming (multiple accounts) | Medium | Medium | Device fingerprinting, email verification |
| Audit log tampering | Low | High | Append-only storage, checksums |

---

## Related Decisions

- **ADR-001**: Architecture Overview
- **ADR-003**: Persistence Layer (RLS implementation details)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-27 | RuVector Architecture Team | Initial version |
