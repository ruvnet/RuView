# Tiny Dancer Routing - Quick Reference

## One-Minute Setup

```sql
-- Register your first agent
SELECT ruvector_register_agent(
    'gpt-4',                    -- name
    'llm',                      -- type
    ARRAY['coding'],            -- capabilities
    0.03,                       -- cost per request
    500.0,                      -- latency (ms)
    0.95                        -- quality (0-1)
);

-- Route a request
SELECT ruvector_route(
    embedding_vector,           -- your 384-dim embedding
    'balanced',                 -- optimize for: cost|latency|quality|balanced
    NULL                        -- constraints (optional)
);
```

## Common Commands

### Register Agents

```sql
-- Simple registration
SELECT ruvector_register_agent(name, type, capabilities, cost, latency, quality);

-- Full configuration
SELECT ruvector_register_agent_full('{
    "name": "claude-3",
    "agent_type": "llm",
    "capabilities": ["coding", "writing"],
    "cost_model": {"per_request": 0.025},
    "performance": {"avg_latency_ms": 400, "quality_score": 0.93}
}'::jsonb);
```

### Route Requests

```sql
-- Cost-optimized
SELECT ruvector_route(emb, 'cost', NULL);

-- Quality-optimized
SELECT ruvector_route(emb, 'quality', NULL);

-- Latency-optimized
SELECT ruvector_route(emb, 'latency', NULL);

-- Balanced (default)
SELECT ruvector_route(emb, 'balanced', NULL);
```

### Add Constraints

```sql
-- Max cost
SELECT ruvector_route(emb, 'quality', '{"max_cost": 0.01}'::jsonb);

-- Max latency
SELECT ruvector_route(emb, 'balanced', '{"max_latency_ms": 500}'::jsonb);

-- Min quality
SELECT ruvector_route(emb, 'cost', '{"min_quality": 0.8}'::jsonb);

-- Required capability
SELECT ruvector_route(emb, 'balanced',
    '{"required_capabilities": ["coding"]}'::jsonb);

-- Multiple constraints
SELECT ruvector_route(emb, 'balanced', '{
    "max_cost": 0.05,
    "max_latency_ms": 1000,
    "min_quality": 0.85,
    "required_capabilities": ["coding", "analysis"],
    "excluded_agents": ["slow-agent"]
}'::jsonb);
```

### Manage Agents

```sql
-- List all
SELECT * FROM ruvector_list_agents();

-- Get specific agent
SELECT ruvector_get_agent('gpt-4');

-- Find by capability
SELECT * FROM ruvector_find_agents_by_capability('coding', 5);

-- Update metrics
SELECT ruvector_update_agent_metrics('gpt-4', 450.0, true, 0.92);

-- Deactivate
SELECT ruvector_set_agent_active('gpt-4', false);

-- Remove
SELECT ruvector_remove_agent('old-agent');

-- Statistics
SELECT ruvector_routing_stats();
```

## Response Format

```json
{
  "agent_name": "gpt-4",
  "confidence": 0.87,
  "estimated_cost": 0.03,
  "estimated_latency_ms": 500.0,
  "expected_quality": 0.95,
  "similarity_score": 0.82,
  "reasoning": "Selected gpt-4 for highest quality...",
  "alternatives": [
    {
      "name": "claude-3",
      "score": 0.85,
      "reason": "0.02 lower quality"
    }
  ]
}
```

## Extract Specific Fields

```sql
-- Get agent name
SELECT (ruvector_route(emb, 'balanced', NULL))::jsonb->>'agent_name';

-- Get cost
SELECT (ruvector_route(emb, 'cost', NULL))::jsonb->>'estimated_cost';

-- Get full decision
SELECT
    (route)::jsonb->>'agent_name' AS agent,
    ((route)::jsonb->>'confidence')::float AS confidence,
    ((route)::jsonb->>'estimated_cost')::float AS cost
FROM (
    SELECT ruvector_route(emb, 'balanced', NULL) AS route
    FROM requests WHERE id = 1
) r;
```

## Common Patterns

### Smart Routing by Priority

```sql
SELECT ruvector_route(
    embedding,
    CASE priority
        WHEN 'critical' THEN 'quality'
        WHEN 'low' THEN 'cost'
        ELSE 'balanced'
    END,
    CASE priority
        WHEN 'critical' THEN '{"min_quality": 0.95}'::jsonb
        ELSE NULL
    END
) FROM requests;
```

### Batch Processing

```sql
SELECT
    id,
    (ruvector_route(embedding, 'cost', '{"max_cost": 0.01}'::jsonb))::jsonb->>'agent_name' AS agent
FROM requests
WHERE processed = false
LIMIT 1000;
```

### With Capability Filter

```sql
SELECT ruvector_route(
    embedding,
    'quality',
    jsonb_build_object(
        'required_capabilities',
        CASE task_type
            WHEN 'coding' THEN ARRAY['coding']
            WHEN 'writing' THEN ARRAY['writing']
            ELSE ARRAY[]::text[]
        END
    )
) FROM requests;
```

### Cost Tracking

```sql
-- Daily costs
SELECT
    DATE(completed_at),
    agent_name,
    COUNT(*) AS requests,
    SUM(cost) AS total_cost
FROM request_completions
GROUP BY 1, 2
ORDER BY 1 DESC, total_cost DESC;
```

## Agent Types

- `llm` - Language models
- `embedding` - Embedding models
- `specialized` - Task-specific
- `vision` - Vision models
- `audio` - Audio models
- `multimodal` - Multi-modal
- `custom` - User-defined

## Optimization Targets

| Target | Optimizes | Use Case |
|--------|-----------|----------|
| `cost` | Minimize cost | High-volume, budget-constrained |
| `latency` | Minimize response time | Real-time applications |
| `quality` | Maximize quality | Critical tasks |
| `balanced` | Balance all factors | General purpose |

## Constraints Reference

| Constraint | Type | Description |
|------------|------|-------------|
| `max_cost` | float | Maximum cost per request |
| `max_latency_ms` | float | Maximum latency in ms |
| `min_quality` | float | Minimum quality (0-1) |
| `required_capabilities` | array | Required capabilities |
| `excluded_agents` | array | Agents to exclude |

## Performance Metrics

| Metric | Description | Updated By |
|--------|-------------|------------|
| `avg_latency_ms` | Average response time | `update_agent_metrics` |
| `quality_score` | Quality rating (0-1) | `update_agent_metrics` |
| `success_rate` | Success ratio (0-1) | `update_agent_metrics` |
| `total_requests` | Total processed | Auto-incremented |
| `p95_latency_ms` | 95th percentile | Auto-calculated |
| `p99_latency_ms` | 99th percentile | Auto-calculated |

## Troubleshooting

### No agents match constraints

```sql
-- Check available agents
SELECT * FROM ruvector_list_agents() WHERE is_active = true;

-- Relax constraints
SELECT ruvector_route(emb, 'balanced', '{"max_cost": 1.0}'::jsonb);
```

### Unexpected routing decisions

```sql
-- Check reasoning
SELECT (ruvector_route(emb, 'balanced', NULL))::jsonb->>'reasoning';

-- View alternatives
SELECT (ruvector_route(emb, 'balanced', NULL))::jsonb->'alternatives';
```

### Agent not appearing

```sql
-- Verify registration
SELECT ruvector_get_agent('agent-name');

-- Check active status
SELECT is_active FROM ruvector_list_agents() WHERE name = 'agent-name';

-- Reactivate
SELECT ruvector_set_agent_active('agent-name', true);
```

## Best Practices

1. **Always set constraints in production**
   ```sql
   SELECT ruvector_route(emb, 'balanced', '{"max_cost": 0.1}'::jsonb);
   ```

2. **Update metrics after each request**
   ```sql
   SELECT ruvector_update_agent_metrics(agent, latency, success, quality);
   ```

3. **Monitor agent health**
   ```sql
   SELECT * FROM ruvector_list_agents()
   WHERE success_rate < 0.9 OR avg_latency_ms > 1000;
   ```

4. **Use capability filters**
   ```sql
   SELECT ruvector_route(emb, 'quality',
       '{"required_capabilities": ["coding"]}'::jsonb);
   ```

5. **Track costs**
   ```sql
   SELECT SUM(cost) FROM request_completions
   WHERE completed_at > NOW() - INTERVAL '1 day';
   ```

## Examples by Use Case

### High-Volume Processing (Cost-Optimized)
```sql
SELECT ruvector_route(emb, 'cost', '{"max_cost": 0.005}'::jsonb);
```

### Real-Time Chat (Latency-Optimized)
```sql
SELECT ruvector_route(emb, 'latency', '{"max_latency_ms": 200}'::jsonb);
```

### Critical Analysis (Quality-Optimized)
```sql
SELECT ruvector_route(emb, 'quality', '{"min_quality": 0.95}'::jsonb);
```

### Production Workload (Balanced)
```sql
SELECT ruvector_route(emb, 'balanced', '{
    "max_cost": 0.05,
    "max_latency_ms": 1000,
    "min_quality": 0.85
}'::jsonb);
```

### Code Generation
```sql
SELECT ruvector_route(emb, 'quality',
    '{"required_capabilities": ["coding", "debugging"]}'::jsonb);
```

## Quick Debugging

```sql
-- Check if routing is working
SELECT ruvector_routing_stats();

-- List active agents
SELECT name, capabilities FROM ruvector_list_agents() WHERE is_active;

-- Test simple route
SELECT ruvector_route(ARRAY[0.1]::float4[] || ARRAY(SELECT 0::float4 FROM generate_series(1,383)), 'balanced', NULL);

-- View agent details
SELECT jsonb_pretty(ruvector_get_agent('gpt-4'));

-- Clear and restart (testing only)
-- SELECT ruvector_clear_agents();
```

## Integration Example

```sql
-- Complete workflow
CREATE TABLE my_requests (
    id SERIAL PRIMARY KEY,
    query TEXT,
    embedding vector(384)
);

-- Route and execute
WITH routing AS (
    SELECT
        r.id,
        r.query,
        (ruvector_route(
            r.embedding::float4[],
            'balanced',
            '{"max_cost": 0.05}'::jsonb
        ))::jsonb AS decision
    FROM my_requests r
    WHERE id = 1
)
SELECT
    id,
    decision->>'agent_name' AS agent,
    decision->>'reasoning' AS why,
    ((decision->>'confidence')::float * 100)::int AS confidence_pct
FROM routing;
```
