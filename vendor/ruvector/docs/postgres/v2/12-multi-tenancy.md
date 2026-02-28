# RuVector Postgres v2 - Multi-Tenancy Model

## Why Multi-Tenancy Matters

Every SaaS application needs tenant isolation. Without native support, teams build:
- Separate databases per tenant (operational nightmare)
- Manual partition schemes (error-prone)
- Application-level filtering (security risk)

RuVector provides **first-class multi-tenancy** with:
- Tenant-isolated search (data never leaks)
- Per-tenant integrity monitoring (one bad tenant doesn't sink others)
- Efficient shared infrastructure (cost-effective)
- Row-level security integration (PostgreSQL-native)

---

## Design Goals

1. **Zero data leakage** — Tenant A never sees Tenant B's vectors
2. **Per-tenant integrity** — Stress in one tenant doesn't affect others
3. **Fair resource allocation** — No noisy neighbor problems
4. **Transparent to queries** — SET tenant, then normal SQL
5. **Efficient storage** — Shared indexes where safe, isolated where needed

---

## Architecture

```
+------------------------------------------------------------------+
|                        Application                                |
|  SET ruvector.tenant_id = 'acme-corp';                           |
|  SELECT * FROM embeddings ORDER BY vec <-> $q LIMIT 10;          |
+------------------------------------------------------------------+
                              |
+------------------------------------------------------------------+
|                    Tenant Context Layer                           |
|  - Validates tenant_id                                            |
|  - Injects tenant filter into all operations                     |
|  - Routes to tenant-specific resources                            |
+------------------------------------------------------------------+
                              |
              +---------------+---------------+
              |                               |
     +--------v--------+            +---------v---------+
     |  Shared Index   |            |  Tenant Indexes   |
     |  (small tenants)|            |  (large tenants)  |
     +--------+--------+            +---------+---------+
              |                               |
              +---------------+---------------+
                              |
+------------------------------------------------------------------+
|                    Per-Tenant Integrity                           |
|  - Separate contracted graphs                                     |
|  - Independent state machines                                     |
|  - Isolated throttling policies                                   |
+------------------------------------------------------------------+
```

---

## SQL Interface

### Setting Tenant Context

```sql
-- Set tenant for session (required before any operation)
SET ruvector.tenant_id = 'acme-corp';

-- Or per-transaction
BEGIN;
SET LOCAL ruvector.tenant_id = 'acme-corp';
-- ... operations ...
COMMIT;

-- Verify current tenant
SELECT current_setting('ruvector.tenant_id');
```

### Tenant-Transparent Operations

```sql
-- Once tenant is set, all operations are automatically scoped
SET ruvector.tenant_id = 'acme-corp';

-- Insert only sees/affects acme-corp data
INSERT INTO embeddings (content, vec) VALUES ('doc', $embedding);

-- Search only returns acme-corp results
SELECT * FROM embeddings ORDER BY vec <-> $query LIMIT 10;

-- Delete only affects acme-corp
DELETE FROM embeddings WHERE id = 123;
```

### Admin Operations (Cross-Tenant)

```sql
-- Superuser can query across tenants
SET ruvector.tenant_id = '*';  -- Wildcard (admin only)

-- View all tenants
SELECT * FROM ruvector_tenants();

-- View tenant stats
SELECT * FROM ruvector_tenant_stats('acme-corp');

-- Migrate tenant to dedicated index
SELECT ruvector_tenant_isolate('acme-corp');
```

---

## Schema Design

### Tenant Registry

```sql
CREATE TABLE ruvector.tenants (
    id              TEXT PRIMARY KEY,
    display_name    TEXT,

    -- Resource limits
    max_vectors     BIGINT DEFAULT 1000000,
    max_collections INTEGER DEFAULT 10,
    max_qps         INTEGER DEFAULT 100,

    -- Isolation level
    isolation_level TEXT DEFAULT 'shared' CHECK (isolation_level IN (
        'shared',      -- Shared index with tenant filter
        'partition',   -- Dedicated partition in shared index
        'dedicated'    -- Separate physical index
    )),

    -- Integrity settings
    integrity_enabled   BOOLEAN DEFAULT true,
    integrity_policy_id INTEGER REFERENCES ruvector.integrity_policies(id),

    -- Metadata
    metadata        JSONB DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    suspended_at    TIMESTAMPTZ,  -- Non-null = suspended

    -- Stats (updated by background worker)
    vector_count    BIGINT DEFAULT 0,
    storage_bytes   BIGINT DEFAULT 0,
    last_access     TIMESTAMPTZ
);

CREATE INDEX idx_tenants_isolation ON ruvector.tenants(isolation_level);
CREATE INDEX idx_tenants_suspended ON ruvector.tenants(suspended_at) WHERE suspended_at IS NOT NULL;
```

### Tenant-Aware Collections

```sql
-- Collections can be tenant-specific or shared
CREATE TABLE ruvector.collections (
    id              SERIAL PRIMARY KEY,
    name            TEXT NOT NULL,
    tenant_id       TEXT REFERENCES ruvector.tenants(id),  -- NULL = shared

    -- ... other columns from 01-sql-schema.md ...

    UNIQUE (name, tenant_id)  -- Same name allowed for different tenants
);

-- Tenant-scoped view
CREATE VIEW ruvector.my_collections AS
SELECT * FROM ruvector.collections
WHERE tenant_id = current_setting('ruvector.tenant_id', true)
   OR tenant_id IS NULL;  -- Shared collections visible to all
```

### Tenant Column in Data Tables

```sql
-- User tables include tenant_id column
CREATE TABLE embeddings (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   TEXT NOT NULL DEFAULT current_setting('ruvector.tenant_id'),
    content     TEXT,
    vec         vector(1536),
    created_at  TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT fk_tenant FOREIGN KEY (tenant_id)
        REFERENCES ruvector.tenants(id) ON DELETE CASCADE
);

-- Partial index per tenant (for dedicated isolation)
CREATE INDEX idx_embeddings_vec_tenant_acme
    ON embeddings USING ruhnsw (vec vector_cosine_ops)
    WHERE tenant_id = 'acme-corp';

-- Or composite index for shared isolation
CREATE INDEX idx_embeddings_vec_shared
    ON embeddings USING ruhnsw (vec vector_cosine_ops);
-- Engine internally filters by tenant_id
```

---

## Row-Level Security Integration

### RLS Policies

```sql
-- Enable RLS on data tables
ALTER TABLE embeddings ENABLE ROW LEVEL SECURITY;

-- Tenant isolation policy
CREATE POLICY tenant_isolation ON embeddings
    USING (tenant_id = current_setting('ruvector.tenant_id', true))
    WITH CHECK (tenant_id = current_setting('ruvector.tenant_id', true));

-- Admin bypass policy
CREATE POLICY admin_access ON embeddings
    FOR ALL
    TO ruvector_admin
    USING (true)
    WITH CHECK (true);
```

### Automatic Policy Creation

```sql
-- Helper function to set up RLS for a table
CREATE FUNCTION ruvector_enable_tenant_rls(
    p_table_name TEXT,
    p_tenant_column TEXT DEFAULT 'tenant_id'
) RETURNS void AS $$
BEGIN
    -- Enable RLS
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', p_table_name);

    -- Create isolation policy
    EXECUTE format(
        'CREATE POLICY tenant_isolation ON %I
         USING (%I = current_setting(''ruvector.tenant_id'', true))
         WITH CHECK (%I = current_setting(''ruvector.tenant_id'', true))',
        p_table_name, p_tenant_column, p_tenant_column
    );

    -- Create admin bypass
    EXECUTE format(
        'CREATE POLICY admin_bypass ON %I FOR ALL TO ruvector_admin USING (true)',
        p_table_name
    );
END;
$$ LANGUAGE plpgsql;

-- Usage
SELECT ruvector_enable_tenant_rls('embeddings');
SELECT ruvector_enable_tenant_rls('documents');
```

---

## Isolation Levels

### Shared (Default)

All tenants share one index. Engine filters by tenant_id.

```
Pros:
  + Most memory-efficient
  + Fastest for small tenants
  + Simple management

Cons:
  - Some cross-tenant cache pollution
  - Shared integrity state

Best for: < 100K vectors per tenant
```

### Partition

Tenants get dedicated partitions within shared index structure.

```
Pros:
  + Better cache isolation
  + Per-partition integrity
  + Easy promotion to dedicated

Cons:
  - Some overhead per partition
  - Still shares top-level structure

Best for: 100K - 10M vectors per tenant
```

### Dedicated

Tenant gets completely separate physical index.

```
Pros:
  + Complete isolation
  + Independent scaling
  + Custom index parameters

Cons:
  - Higher memory overhead
  + More management complexity

Best for: > 10M vectors, enterprise tenants, compliance requirements
```

### Automatic Promotion

```sql
-- Configure auto-promotion thresholds
SELECT ruvector_tenant_set_policy('{
    "auto_promote_to_partition": 100000,   -- vectors
    "auto_promote_to_dedicated": 10000000,
    "check_interval": "1 hour"
}'::jsonb);
```

```rust
// Background worker checks and promotes
pub fn check_tenant_promotion(tenant_id: &str) -> Option<IsolationLevel> {
    let stats = get_tenant_stats(tenant_id)?;
    let policy = get_promotion_policy()?;

    if stats.vector_count > policy.dedicated_threshold {
        Some(IsolationLevel::Dedicated)
    } else if stats.vector_count > policy.partition_threshold {
        Some(IsolationLevel::Partition)
    } else {
        None
    }
}
```

---

## Per-Tenant Integrity

### Separate Contracted Graphs

```sql
-- Each tenant gets its own contracted graph
CREATE TABLE ruvector.tenant_contracted_graph (
    tenant_id       TEXT NOT NULL REFERENCES ruvector.tenants(id),
    collection_id   INTEGER NOT NULL,
    node_type       TEXT NOT NULL,
    node_id         BIGINT NOT NULL,
    -- ... same as contracted_graph ...

    PRIMARY KEY (tenant_id, collection_id, node_type, node_id)
);
```

### Independent State Machines

```rust
// Per-tenant integrity state
pub struct TenantIntegrityState {
    tenant_id: String,
    state: IntegrityState,
    lambda_cut: f32,
    consecutive_samples: u32,
    last_transition: Instant,
    cooldown_until: Option<Instant>,
}

// Tenant stress doesn't affect other tenants
pub fn check_tenant_gate(tenant_id: &str, operation: &str) -> GateResult {
    let state = get_tenant_integrity_state(tenant_id);
    apply_policy(state, operation)
}
```

### Tenant-Specific Policies

```sql
-- Each tenant can have custom thresholds
INSERT INTO ruvector.integrity_policies (tenant_id, name, threshold_high, threshold_low)
VALUES
    ('acme-corp', 'enterprise', 0.6, 0.3),      -- Stricter
    ('startup-xyz', 'standard', 0.4, 0.15);     -- Default
```

---

## Resource Quotas

### Quota Enforcement

```sql
-- Quota table
CREATE TABLE ruvector.tenant_quotas (
    tenant_id       TEXT PRIMARY KEY REFERENCES ruvector.tenants(id),
    max_vectors     BIGINT NOT NULL DEFAULT 1000000,
    max_storage_gb  REAL NOT NULL DEFAULT 10.0,
    max_qps         INTEGER NOT NULL DEFAULT 100,
    max_concurrent  INTEGER NOT NULL DEFAULT 10,

    -- Current usage (updated by triggers/workers)
    current_vectors BIGINT DEFAULT 0,
    current_storage_gb REAL DEFAULT 0,

    -- Rate limiting state
    request_count   INTEGER DEFAULT 0,
    window_start    TIMESTAMPTZ DEFAULT NOW()
);

-- Check quota before insert
CREATE FUNCTION ruvector_check_quota() RETURNS TRIGGER AS $$
DECLARE
    v_quota RECORD;
BEGIN
    SELECT * INTO v_quota
    FROM ruvector.tenant_quotas
    WHERE tenant_id = NEW.tenant_id;

    IF v_quota.current_vectors >= v_quota.max_vectors THEN
        RAISE EXCEPTION 'Tenant % has exceeded vector quota', NEW.tenant_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER check_quota_before_insert
    BEFORE INSERT ON embeddings
    FOR EACH ROW EXECUTE FUNCTION ruvector_check_quota();
```

### Rate Limiting

```rust
// Token bucket rate limiter per tenant
pub struct TenantRateLimiter {
    buckets: DashMap<String, TokenBucket>,
}

impl TenantRateLimiter {
    pub fn check(&self, tenant_id: &str, tokens: u32) -> RateLimitResult {
        let bucket = self.buckets.entry(tenant_id.to_string())
            .or_insert_with(|| TokenBucket::new(
                get_tenant_qps_limit(tenant_id),
            ));

        if bucket.try_acquire(tokens) {
            RateLimitResult::Allowed
        } else {
            RateLimitResult::Limited {
                retry_after_ms: bucket.time_to_refill(tokens),
            }
        }
    }
}
```

### Fair Scheduling

```rust
// Weighted fair queue for search requests
pub struct FairScheduler {
    queues: HashMap<String, VecDeque<SearchRequest>>,
    weights: HashMap<String, f32>,  // Based on tier/quota
}

impl FairScheduler {
    pub fn next(&mut self) -> Option<SearchRequest> {
        // Weighted round-robin across tenants
        // Prevents one tenant from monopolizing resources
        let total_weight: f32 = self.weights.values().sum();

        for (tenant_id, queue) in &mut self.queues {
            let weight = self.weights.get(tenant_id).unwrap_or(&1.0);
            let share = weight / total_weight;

            // Probability of selecting this tenant's request
            if rand::random::<f32>() < share {
                if let Some(req) = queue.pop_front() {
                    return Some(req);
                }
            }
        }

        // Fallback: any available request
        self.queues.values_mut()
            .find_map(|q| q.pop_front())
    }
}
```

---

## Tenant Lifecycle

### Create Tenant

```sql
SELECT ruvector_tenant_create('new-customer', '{
    "display_name": "New Customer Inc.",
    "max_vectors": 5000000,
    "max_qps": 200,
    "isolation_level": "shared",
    "integrity_enabled": true
}'::jsonb);
```

### Suspend Tenant

```sql
-- Suspend (stops all operations, keeps data)
SELECT ruvector_tenant_suspend('bad-actor');

-- Resume
SELECT ruvector_tenant_resume('bad-actor');
```

### Delete Tenant

```sql
-- Soft delete (marks for cleanup)
SELECT ruvector_tenant_delete('churned-customer');

-- Hard delete (immediate, for compliance)
SELECT ruvector_tenant_delete('churned-customer', hard := true);
```

### Migrate Isolation Level

```sql
-- Promote to dedicated (online, no downtime)
SELECT ruvector_tenant_migrate('enterprise-customer', 'dedicated');

-- Status check
SELECT * FROM ruvector_tenant_migration_status('enterprise-customer');
```

---

## Shared Memory Layout

```rust
// Per-tenant state in shared memory
#[repr(C)]
pub struct TenantSharedState {
    tenant_id_hash: u64,           // Fast lookup key
    integrity_state: u8,           // 0=normal, 1=stress, 2=critical
    lambda_cut: f32,               // Current mincut value
    request_count: AtomicU32,      // For rate limiting
    last_request_epoch: AtomicU64, // Rate limit window
    flags: AtomicU32,              // Suspended, migrating, etc.
}

// Tenant lookup table
pub struct TenantRegistry {
    states: [TenantSharedState; MAX_TENANTS],  // Fixed array in shmem
    index: HashMap<String, usize>,              // Heap-based lookup
}
```

---

## Monitoring

### Per-Tenant Metrics

```sql
-- Tenant dashboard
SELECT
    t.id,
    t.display_name,
    t.isolation_level,
    tq.current_vectors,
    tq.max_vectors,
    ROUND(100.0 * tq.current_vectors / tq.max_vectors, 1) AS usage_pct,
    ts.integrity_state,
    ts.lambda_cut,
    ts.avg_search_latency_ms,
    ts.searches_last_hour
FROM ruvector.tenants t
JOIN ruvector.tenant_quotas tq ON t.id = tq.tenant_id
JOIN ruvector.tenant_stats ts ON t.id = ts.tenant_id
ORDER BY tq.current_vectors DESC;
```

### Prometheus Metrics

```
# Per-tenant metrics
ruvector_tenant_vectors{tenant="acme-corp"} 1234567
ruvector_tenant_integrity_state{tenant="acme-corp"} 1
ruvector_tenant_lambda_cut{tenant="acme-corp"} 0.72
ruvector_tenant_search_latency_p99{tenant="acme-corp"} 15.2
ruvector_tenant_qps{tenant="acme-corp"} 45.3
ruvector_tenant_quota_usage{tenant="acme-corp",resource="vectors"} 0.62
```

---

## Security Considerations

### Tenant ID Validation

```rust
// Validate tenant_id before any operation
pub fn validate_tenant_context() -> Result<String, Error> {
    let tenant_id = get_guc("ruvector.tenant_id")?;

    // Check not empty
    if tenant_id.is_empty() {
        return Err(Error::NoTenantContext);
    }

    // Check tenant exists and not suspended
    let tenant = get_tenant(&tenant_id)?;
    if tenant.suspended_at.is_some() {
        return Err(Error::TenantSuspended);
    }

    Ok(tenant_id)
}
```

### Audit Logging

```sql
-- Tenant operations audit log
CREATE TABLE ruvector.tenant_audit_log (
    id              BIGSERIAL PRIMARY KEY,
    tenant_id       TEXT NOT NULL,
    operation       TEXT NOT NULL,  -- search, insert, delete, etc.
    user_id         TEXT,           -- Application user
    details         JSONB,
    ip_address      INET,
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

-- Enabled via GUC
SET ruvector.audit_enabled = true;
```

### Cross-Tenant Prevention

```rust
// Engine-level enforcement (defense in depth)
pub fn execute_search(request: &SearchRequest) -> Result<SearchResults, Error> {
    let context_tenant = validate_tenant_context()?;

    // Double-check request matches context
    if let Some(req_tenant) = &request.tenant_id {
        if req_tenant != &context_tenant {
            // Log security event
            log_security_event("tenant_mismatch", &context_tenant, req_tenant);
            return Err(Error::TenantMismatch);
        }
    }

    // Execute with tenant filter
    execute_search_internal(request, &context_tenant)
}
```

---

## Testing Requirements

### Isolation Tests
- Tenant A cannot see Tenant B's data
- Tenant A's stress doesn't affect Tenant B's operations
- Suspended tenant cannot perform any operations

### Performance Tests
- Shared isolation: < 5% overhead vs single-tenant
- Dedicated isolation: equivalent to single-tenant
- Rate limiting adds < 1ms latency

### Scale Tests
- 1000+ tenants on shared infrastructure
- 100+ tenants with dedicated isolation
- Tenant migration under load

---

## Example: SaaS Application

```python
# Application code
class VectorService:
    def __init__(self, db_pool):
        self.pool = db_pool

    def search(self, tenant_id: str, query_vec: list, k: int = 10):
        with self.pool.connection() as conn:
            # Set tenant context
            conn.execute("SET ruvector.tenant_id = %s", [tenant_id])

            # Search (automatically scoped to tenant)
            results = conn.execute("""
                SELECT id, content, vec <-> %s AS distance
                FROM embeddings
                ORDER BY vec <-> %s
                LIMIT %s
            """, [query_vec, query_vec, k])

            return results.fetchall()

    def insert(self, tenant_id: str, content: str, vec: list):
        with self.pool.connection() as conn:
            conn.execute("SET ruvector.tenant_id = %s", [tenant_id])

            # Insert (tenant_id auto-populated from context)
            conn.execute("""
                INSERT INTO embeddings (content, vec)
                VALUES (%s, %s)
            """, [content, vec])
```
