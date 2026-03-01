# Tiny Dancer Routing Module

Neural-powered dynamic agent routing with FastGRNN for intelligent AI agent selection.

## Overview

The Tiny Dancer routing module provides intelligent routing of requests to AI agents based on multiple optimization criteria including cost, latency, quality, and balanced performance. It uses a FastGRNN (Fast Gated Recurrent Neural Network) for adaptive decision-making.

## Architecture

### Components

1. **FastGRNN** (`fastgrnn.rs`)
   - Lightweight gated recurrent neural network
   - Real-time routing decisions with minimal compute
   - Adaptive learning from routing patterns

2. **Agent Registry** (`agents.rs`)
   - Thread-safe agent storage with DashMap
   - Capability-based agent discovery
   - Performance metrics tracking

3. **Router** (`router.rs`)
   - Multi-objective optimization
   - Constraint-based filtering
   - Neural-enhanced confidence scoring

4. **PostgreSQL Operators** (`operators.rs`)
   - SQL functions for agent management
   - Routing query interface
   - Statistics and monitoring

## PostgreSQL Functions

### Agent Registration

```sql
-- Register a simple agent
SELECT ruvector_register_agent(
    'gpt-4',                    -- Agent name
    'llm',                      -- Agent type
    ARRAY['code_generation', 'reasoning'],  -- Capabilities
    0.03,                       -- Cost per request ($)
    500.0,                      -- Average latency (ms)
    0.95                        -- Quality score (0-1)
);

-- Register with full configuration
SELECT ruvector_register_agent_full('{
    "name": "claude-3-opus",
    "agent_type": "llm",
    "capabilities": ["coding", "reasoning", "writing"],
    "cost_model": {
        "per_request": 0.025,
        "per_token": 0.00005
    },
    "performance": {
        "avg_latency_ms": 400.0,
        "quality_score": 0.93,
        "success_rate": 0.99,
        "p95_latency_ms": 600.0,
        "p99_latency_ms": 1000.0
    },
    "is_active": true
}'::jsonb);
```

### Routing Requests

```sql
-- Basic routing (optimize for balanced performance)
SELECT ruvector_route(
    embedding_vector,           -- Request embedding (384-dim)
    'balanced',                 -- Optimization target
    NULL                        -- No constraints
)
FROM requests
WHERE id = 123;

-- Cost-optimized routing with constraints
SELECT ruvector_route(
    embedding_vector,
    'cost',
    '{"max_latency_ms": 1000.0, "min_quality": 0.8}'::jsonb
)
FROM requests
WHERE id = 456;

-- Quality-optimized with capability requirements
SELECT ruvector_route(
    embedding_vector,
    'quality',
    '{
        "max_cost": 0.1,
        "required_capabilities": ["code_generation", "debugging"],
        "excluded_agents": ["slow-agent"]
    }'::jsonb
);

-- Latency-optimized routing
SELECT ruvector_route(
    embedding_vector,
    'latency',
    '{"max_latency_ms": 500.0}'::jsonb
);
```

### Agent Management

```sql
-- List all agents
SELECT * FROM ruvector_list_agents();

-- Get specific agent details
SELECT ruvector_get_agent('gpt-4');

-- Find agents by capability
SELECT * FROM ruvector_find_agents_by_capability('code_generation', 5);

-- Update agent performance metrics
SELECT ruvector_update_agent_metrics(
    'gpt-4',                    -- Agent name
    450.0,                      -- Observed latency (ms)
    true,                       -- Success
    0.92                        -- Quality score (optional)
);

-- Deactivate an agent
SELECT ruvector_set_agent_active('gpt-4', false);

-- Remove an agent
SELECT ruvector_remove_agent('old-agent');

-- Get routing statistics
SELECT ruvector_routing_stats();
```

## Usage Examples

### Example 1: Multi-Model Routing System

```sql
-- Register various AI models
SELECT ruvector_register_agent('gpt-4', 'llm',
    ARRAY['coding', 'reasoning', 'math'], 0.03, 500.0, 0.95);
SELECT ruvector_register_agent('gpt-3.5-turbo', 'llm',
    ARRAY['general', 'fast'], 0.002, 200.0, 0.75);
SELECT ruvector_register_agent('claude-3-opus', 'llm',
    ARRAY['coding', 'writing', 'analysis'], 0.025, 400.0, 0.93);
SELECT ruvector_register_agent('llama-2-70b', 'llm',
    ARRAY['local', 'private'], 0.0, 800.0, 0.72);

-- Create routing view
CREATE VIEW intelligent_routing AS
SELECT
    r.id,
    r.query_text,
    r.embedding,
    route.agent_name,
    route.confidence,
    route.estimated_cost,
    route.estimated_latency_ms,
    route.expected_quality,
    route.reasoning
FROM requests r,
LATERAL (
    SELECT (ruvector_route(
        r.embedding,
        'balanced',
        NULL
    ))::jsonb AS route_data
) route_query,
LATERAL jsonb_to_record(route_query.route_data) AS route(
    agent_name text,
    confidence float4,
    estimated_cost float4,
    estimated_latency_ms float4,
    expected_quality float4,
    similarity_score float4,
    reasoning text
);

-- Query with automatic routing
SELECT * FROM intelligent_routing WHERE id = 123;
```

### Example 2: Cost-Aware Batch Processing

```sql
-- Process batch with cost constraints
CREATE TEMP TABLE batch_results AS
SELECT
    r.id,
    r.query_text,
    routing.agent_name,
    routing.estimated_cost,
    routing.expected_quality
FROM requests r
CROSS JOIN LATERAL (
    SELECT (ruvector_route(
        r.embedding,
        'cost',
        '{"max_cost": 0.01, "min_quality": 0.7}'::jsonb
    ))::jsonb->'agent_name' AS agent_name,
    (ruvector_route(
        r.embedding,
        'cost',
        '{"max_cost": 0.01, "min_quality": 0.7}'::jsonb
    ))::jsonb->'estimated_cost' AS estimated_cost,
    (ruvector_route(
        r.embedding,
        'cost',
        '{"max_cost": 0.01, "min_quality": 0.7}'::jsonb
    ))::jsonb->'expected_quality' AS expected_quality
) routing
WHERE r.processed = false
LIMIT 1000;

-- Calculate total estimated cost
SELECT
    SUM((estimated_cost)::float) AS total_cost,
    AVG((expected_quality)::float) AS avg_quality,
    COUNT(*) AS total_requests
FROM batch_results;
```

### Example 3: Quality-First Routing

```sql
-- Route critical requests to highest quality agents
CREATE FUNCTION route_critical_request(
    request_embedding float4[],
    min_quality float4 DEFAULT 0.9
) RETURNS jsonb AS $$
    SELECT ruvector_route(
        request_embedding,
        'quality',
        jsonb_build_object(
            'min_quality', min_quality,
            'max_latency_ms', 2000.0,
            'required_capabilities', ARRAY['reasoning', 'analysis']
        )
    );
$$ LANGUAGE SQL;

-- Use the function
SELECT route_critical_request(embedding_vector, 0.95)
FROM critical_requests
WHERE priority = 'high';
```

### Example 4: Real-time Performance Tracking

```sql
-- Update metrics after each request
CREATE FUNCTION record_agent_performance(
    agent_name text,
    actual_latency_ms float4,
    success boolean,
    quality_score float4
) RETURNS void AS $$
BEGIN
    PERFORM ruvector_update_agent_metrics(
        agent_name,
        actual_latency_ms,
        success,
        quality_score
    );
END;
$$ LANGUAGE plpgsql;

-- Trigger to auto-update metrics
CREATE TRIGGER update_agent_metrics_trigger
AFTER INSERT ON request_completions
FOR EACH ROW
EXECUTE FUNCTION record_agent_performance(
    NEW.agent_name,
    NEW.latency_ms,
    NEW.success,
    NEW.quality_score
);
```

### Example 5: Capability-Based Routing

```sql
-- Create specialized routing functions
CREATE FUNCTION route_code_request(emb float4[]) RETURNS text AS $$
    SELECT (ruvector_route(
        emb,
        'quality',
        '{"required_capabilities": ["coding", "debugging"]}'::jsonb
    ))::jsonb->>'agent_name';
$$ LANGUAGE SQL;

CREATE FUNCTION route_writing_request(emb float4[]) RETURNS text AS $$
    SELECT (ruvector_route(
        emb,
        'quality',
        '{"required_capabilities": ["writing", "editing"]}'::jsonb
    ))::jsonb->>'agent_name';
$$ LANGUAGE SQL;

-- Use in application logic
SELECT
    CASE
        WHEN task_type = 'code' THEN route_code_request(embedding)
        WHEN task_type = 'write' THEN route_writing_request(embedding)
        ELSE (ruvector_route(embedding, 'balanced', NULL))::jsonb->>'agent_name'
    END AS selected_agent
FROM tasks;
```

## Optimization Targets

### Cost
- Minimizes cost per request
- Considers both per-request and per-token costs
- Ideal for high-volume, cost-sensitive workloads

### Latency
- Minimizes response time
- Uses average latency metrics
- Best for real-time applications

### Quality
- Maximizes quality score
- Based on historical performance
- Recommended for critical tasks

### Balanced
- Multi-objective optimization
- Balances cost, latency, quality, and similarity
- Default for general-purpose routing

## Constraints

### max_cost
Maximum acceptable cost per request (in dollars)

### max_latency_ms
Maximum acceptable latency in milliseconds

### min_quality
Minimum required quality score (0-1 scale)

### required_capabilities
Array of required agent capabilities

### excluded_agents
Array of agent names to exclude from selection

## Performance Considerations

1. **Agent Registry**: Thread-safe with DashMap for concurrent access
2. **Embedding Similarity**: Uses fast cosine similarity for request matching
3. **FastGRNN**: Lightweight neural network for real-time inference
4. **Caching**: Consider caching routing decisions for identical requests

## Monitoring

```sql
-- View agent statistics
SELECT name, total_requests, avg_latency_ms, quality_score, success_rate
FROM ruvector_list_agents()
ORDER BY total_requests DESC;

-- Get overall routing statistics
SELECT ruvector_routing_stats();

-- Find underperforming agents
SELECT name, success_rate, quality_score
FROM ruvector_list_agents()
WHERE success_rate < 0.95
   OR quality_score < 0.7;
```

## Best Practices

1. **Register Accurate Metrics**: Keep agent performance metrics up-to-date
2. **Use Constraints**: Always set appropriate constraints for production
3. **Monitor Performance**: Track actual vs. estimated metrics
4. **Update Regularly**: Use `ruvector_update_agent_metrics` after each request
5. **Capability Matching**: Ensure agents have accurate capability tags
6. **Cost Tracking**: Monitor total routing costs with statistics queries

## Integration with Other Modules

The routing module integrates seamlessly with:
- **Vector Search**: Use query embeddings for semantic routing
- **GNN**: Enhance routing with graph neural networks
- **Quantization**: Reduce embedding storage costs
- **HNSW Index**: Fast similarity search for agent selection

## Future Enhancements

- [ ] A/B testing framework for agent comparison
- [ ] Multi-armed bandit algorithms for exploration
- [ ] Reinforcement learning for adaptive routing
- [ ] Cost prediction models
- [ ] Load balancing across agent instances
- [ ] Geo-distributed agent routing
