-- Tiny Dancer Routing Module - SQL Examples
--
-- Complete examples for agent registration, routing, and monitoring

-- ============================================================================
-- Setup: Create supporting tables
-- ============================================================================

-- Table for storing requests with embeddings
CREATE TABLE ai_requests (
    id BIGSERIAL PRIMARY KEY,
    query_text TEXT NOT NULL,
    embedding vector(384),  -- Request embedding
    task_type TEXT,         -- 'coding', 'writing', 'analysis', etc.
    priority TEXT,          -- 'low', 'medium', 'high', 'critical'
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Table for tracking request completions
CREATE TABLE request_completions (
    id BIGSERIAL PRIMARY KEY,
    request_id BIGINT REFERENCES ai_requests(id),
    agent_name TEXT NOT NULL,
    latency_ms FLOAT NOT NULL,
    cost FLOAT NOT NULL,
    quality_score FLOAT,
    success BOOLEAN DEFAULT true,
    error_message TEXT,
    completed_at TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================================================
-- Agent Registration
-- ============================================================================

-- Register OpenAI models
SELECT ruvector_register_agent(
    'gpt-4',
    'llm',
    ARRAY['coding', 'reasoning', 'math', 'writing', 'analysis'],
    0.03,    -- $0.03 per request
    500.0,   -- 500ms average latency
    0.95     -- 0.95 quality score
);

SELECT ruvector_register_agent(
    'gpt-4-turbo',
    'llm',
    ARRAY['coding', 'reasoning', 'fast', 'multimodal'],
    0.02,
    300.0,
    0.93
);

SELECT ruvector_register_agent(
    'gpt-3.5-turbo',
    'llm',
    ARRAY['general', 'fast', 'chat'],
    0.002,
    150.0,
    0.75
);

-- Register Anthropic models
SELECT ruvector_register_agent(
    'claude-3-opus',
    'llm',
    ARRAY['coding', 'reasoning', 'analysis', 'writing'],
    0.025,
    400.0,
    0.93
);

SELECT ruvector_register_agent(
    'claude-3-sonnet',
    'llm',
    ARRAY['coding', 'balanced', 'analysis'],
    0.01,
    250.0,
    0.88
);

SELECT ruvector_register_agent(
    'claude-3-haiku',
    'llm',
    ARRAY['fast', 'general', 'chat'],
    0.003,
    100.0,
    0.80
);

-- Register open-source models
SELECT ruvector_register_agent(
    'llama-2-70b',
    'llm',
    ARRAY['local', 'private', 'coding', 'general'],
    0.0,     -- Free (self-hosted)
    800.0,
    0.72
);

SELECT ruvector_register_agent(
    'mixtral-8x7b',
    'llm',
    ARRAY['local', 'private', 'fast', 'coding'],
    0.0,
    600.0,
    0.78
);

-- Register specialized models
SELECT ruvector_register_agent(
    'codellama-34b',
    'specialized',
    ARRAY['coding', 'local', 'specialized'],
    0.0,
    700.0,
    0.82
);

SELECT ruvector_register_agent(
    'deepseek-coder',
    'specialized',
    ARRAY['coding', 'specialized', 'fast'],
    0.005,
    200.0,
    0.85
);

-- ============================================================================
-- Basic Routing Examples
-- ============================================================================

-- Example 1: Balanced routing (default)
SELECT ruvector_route(
    (SELECT embedding FROM ai_requests WHERE id = 1),
    'balanced',
    NULL
) AS routing_decision;

-- Example 2: Cost-optimized routing
SELECT ruvector_route(
    (SELECT embedding FROM ai_requests WHERE id = 2),
    'cost',
    NULL
) AS routing_decision;

-- Example 3: Quality-optimized routing
SELECT ruvector_route(
    (SELECT embedding FROM ai_requests WHERE id = 3),
    'quality',
    '{"min_quality": 0.9}'::jsonb
) AS routing_decision;

-- Example 4: Latency-optimized routing
SELECT ruvector_route(
    (SELECT embedding FROM ai_requests WHERE id = 4),
    'latency',
    '{"max_latency_ms": 300.0}'::jsonb
) AS routing_decision;

-- ============================================================================
-- Constraint-Based Routing
-- ============================================================================

-- Example 5: Routing with cost constraint
SELECT
    r.id,
    r.query_text,
    (ruvector_route(
        r.embedding,
        'quality',
        '{"max_cost": 0.01}'::jsonb
    ))::jsonb->>'agent_name' AS selected_agent,
    (ruvector_route(
        r.embedding,
        'quality',
        '{"max_cost": 0.01}'::jsonb
    ))::jsonb->>'estimated_cost' AS estimated_cost
FROM ai_requests r
WHERE r.id = 5;

-- Example 6: Routing with multiple constraints
SELECT ruvector_route(
    (SELECT embedding FROM ai_requests WHERE id = 6),
    'balanced',
    '{
        "max_cost": 0.02,
        "max_latency_ms": 500.0,
        "min_quality": 0.85,
        "required_capabilities": ["coding", "analysis"]
    }'::jsonb
) AS routing_decision;

-- Example 7: Exclude specific agents
SELECT ruvector_route(
    (SELECT embedding FROM ai_requests WHERE id = 7),
    'quality',
    '{
        "excluded_agents": ["gpt-3.5-turbo", "llama-2-70b"],
        "min_quality": 0.9
    }'::jsonb
) AS routing_decision;

-- ============================================================================
-- Capability-Based Routing
-- ============================================================================

-- Example 8: Route coding tasks
SELECT
    r.id,
    r.query_text,
    (ruvector_route(
        r.embedding,
        'quality',
        '{"required_capabilities": ["coding"]}'::jsonb
    ))::jsonb AS routing
FROM ai_requests r
WHERE r.task_type = 'coding'
LIMIT 10;

-- Example 9: Route with multiple required capabilities
SELECT ruvector_route(
    (SELECT embedding FROM ai_requests WHERE task_type = 'complex_analysis' LIMIT 1),
    'balanced',
    '{
        "required_capabilities": ["coding", "reasoning", "analysis"],
        "min_quality": 0.85
    }'::jsonb
) AS routing_decision;

-- ============================================================================
-- Batch Routing
-- ============================================================================

-- Example 10: Process batch of requests
CREATE TEMP TABLE batch_routing_results AS
SELECT
    r.id,
    r.query_text,
    r.task_type,
    r.priority,
    (ruvector_route(
        r.embedding,
        CASE
            WHEN r.priority = 'critical' THEN 'quality'
            WHEN r.priority = 'high' THEN 'balanced'
            ELSE 'cost'
        END,
        CASE
            WHEN r.priority = 'critical' THEN '{"min_quality": 0.95}'::jsonb
            WHEN r.priority = 'high' THEN '{"min_quality": 0.85, "max_latency_ms": 500.0}'::jsonb
            ELSE '{"max_cost": 0.005}'::jsonb
        END
    ))::jsonb AS routing_decision
FROM ai_requests r
WHERE created_at > NOW() - INTERVAL '1 hour'
  AND r.id NOT IN (SELECT request_id FROM request_completions);

-- View batch results
SELECT
    id,
    task_type,
    priority,
    routing_decision->>'agent_name' AS agent,
    (routing_decision->>'confidence')::float AS confidence,
    (routing_decision->>'estimated_cost')::float AS cost,
    (routing_decision->>'estimated_latency_ms')::float AS latency_ms,
    routing_decision->>'reasoning' AS reasoning
FROM batch_routing_results
ORDER BY priority DESC, id;

-- Calculate batch statistics
SELECT
    task_type,
    routing_decision->>'agent_name' AS agent,
    COUNT(*) AS requests,
    AVG((routing_decision->>'estimated_cost')::float) AS avg_cost,
    AVG((routing_decision->>'estimated_latency_ms')::float) AS avg_latency,
    AVG((routing_decision->>'confidence')::float) AS avg_confidence
FROM batch_routing_results
GROUP BY task_type, routing_decision->>'agent_name'
ORDER BY requests DESC;

-- ============================================================================
-- Performance Tracking
-- ============================================================================

-- Example 11: Record request completion
INSERT INTO request_completions (request_id, agent_name, latency_ms, cost, quality_score, success)
VALUES (1, 'gpt-4', 450.0, 0.03, 0.92, true);

-- Update agent metrics after completion
SELECT ruvector_update_agent_metrics(
    'gpt-4',
    450.0,
    true,
    0.92
);

-- Example 12: Track performance over time
SELECT
    agent_name,
    DATE_TRUNC('hour', completed_at) AS hour,
    COUNT(*) AS requests,
    AVG(latency_ms) AS avg_latency,
    AVG(cost) AS avg_cost,
    AVG(quality_score) AS avg_quality,
    SUM(CASE WHEN success THEN 1 ELSE 0 END)::float / COUNT(*) AS success_rate
FROM request_completions
WHERE completed_at > NOW() - INTERVAL '24 hours'
GROUP BY agent_name, DATE_TRUNC('hour', completed_at)
ORDER BY hour DESC, requests DESC;

-- ============================================================================
-- Agent Management
-- ============================================================================

-- Example 13: List all agents with statistics
SELECT
    name,
    agent_type,
    capabilities,
    cost_per_request,
    avg_latency_ms,
    quality_score,
    success_rate,
    total_requests,
    is_active
FROM ruvector_list_agents()
ORDER BY total_requests DESC;

-- Example 14: Find best agents by capability
SELECT * FROM ruvector_find_agents_by_capability('coding', 5);
SELECT * FROM ruvector_find_agents_by_capability('writing', 5);
SELECT * FROM ruvector_find_agents_by_capability('fast', 5);

-- Example 15: Get detailed agent information
SELECT ruvector_get_agent('gpt-4') AS agent_details;
SELECT ruvector_get_agent('claude-3-opus') AS agent_details;

-- Example 16: View routing statistics
SELECT ruvector_routing_stats() AS stats;

-- ============================================================================
-- Advanced Routing Patterns
-- ============================================================================

-- Example 17: Create smart routing function
CREATE OR REPLACE FUNCTION smart_route(
    request_embedding vector,
    task_type TEXT,
    priority TEXT DEFAULT 'medium',
    max_budget FLOAT DEFAULT NULL
) RETURNS jsonb AS $$
DECLARE
    optimization_target TEXT;
    constraints jsonb;
BEGIN
    -- Determine optimization strategy
    optimization_target := CASE
        WHEN priority = 'critical' THEN 'quality'
        WHEN priority = 'high' THEN 'balanced'
        WHEN priority = 'low' THEN 'cost'
        ELSE 'balanced'
    END;

    -- Build constraints
    constraints := jsonb_build_object(
        'max_cost', COALESCE(max_budget, 1.0),
        'min_quality', CASE
            WHEN priority = 'critical' THEN 0.95
            WHEN priority = 'high' THEN 0.85
            ELSE 0.70
        END,
        'required_capabilities', CASE
            WHEN task_type = 'coding' THEN ARRAY['coding']
            WHEN task_type = 'writing' THEN ARRAY['writing']
            WHEN task_type = 'analysis' THEN ARRAY['analysis', 'reasoning']
            ELSE ARRAY[]::text[]
        END
    );

    RETURN ruvector_route(
        request_embedding::float4[],
        optimization_target,
        constraints
    );
END;
$$ LANGUAGE plpgsql;

-- Use smart routing
SELECT smart_route(
    (SELECT embedding FROM ai_requests WHERE id = 100),
    'coding',
    'high',
    0.05
) AS routing_decision;

-- Example 18: Cost-aware view with fallback
CREATE VIEW cost_optimized_routing AS
SELECT
    r.id,
    r.query_text,
    r.task_type,
    r.priority,
    -- Try cost-optimized first
    COALESCE(
        (SELECT ruvector_route(r.embedding, 'cost', '{"max_cost": 0.01, "min_quality": 0.8}'::jsonb)),
        -- Fallback to balanced if no cheap option
        ruvector_route(r.embedding, 'balanced', '{"max_cost": 0.05}'::jsonb)
    ) AS routing_decision
FROM ai_requests r;

-- Example 19: A/B testing framework
CREATE TABLE routing_experiments (
    id BIGSERIAL PRIMARY KEY,
    request_id BIGINT REFERENCES ai_requests(id),
    agent_a TEXT,
    agent_b TEXT,
    selected_agent TEXT,
    a_score FLOAT,
    b_score FLOAT,
    actual_quality FLOAT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Run A/B test
INSERT INTO routing_experiments (request_id, agent_a, agent_b, selected_agent, a_score, b_score)
SELECT
    r.id,
    'gpt-4' AS agent_a,
    'claude-3-opus' AS agent_b,
    CASE WHEN random() < 0.5 THEN 'gpt-4' ELSE 'claude-3-opus' END AS selected_agent,
    (ruvector_route(r.embedding, 'quality', '{"excluded_agents": ["claude-3-opus"]}'::jsonb))::jsonb->>'expected_quality' AS a_score,
    (ruvector_route(r.embedding, 'quality', '{"excluded_agents": ["gpt-4"]}'::jsonb))::jsonb->>'expected_quality' AS b_score
FROM ai_requests r
WHERE created_at > NOW() - INTERVAL '1 hour'
LIMIT 100;

-- ============================================================================
-- Monitoring and Alerts
-- ============================================================================

-- Example 20: Monitor agent health
CREATE VIEW agent_health AS
SELECT
    name,
    avg_latency_ms,
    quality_score,
    success_rate,
    total_requests,
    CASE
        WHEN NOT is_active THEN 'inactive'
        WHEN success_rate < 0.90 THEN 'critical'
        WHEN avg_latency_ms > 1000 THEN 'slow'
        WHEN quality_score < 0.75 THEN 'low_quality'
        ELSE 'healthy'
    END AS health_status
FROM ruvector_list_agents();

-- Find unhealthy agents
SELECT * FROM agent_health WHERE health_status != 'healthy';

-- Example 21: Cost tracking
CREATE VIEW daily_routing_costs AS
SELECT
    DATE_TRUNC('day', completed_at) AS day,
    agent_name,
    COUNT(*) AS requests,
    SUM(cost) AS total_cost,
    AVG(cost) AS avg_cost_per_request,
    AVG(quality_score) AS avg_quality
FROM request_completions
WHERE completed_at > NOW() - INTERVAL '30 days'
GROUP BY DATE_TRUNC('day', completed_at), agent_name
ORDER BY day DESC, total_cost DESC;

-- ============================================================================
-- Cleanup
-- ============================================================================

-- Example 22: Deactivate underperforming agents
UPDATE ruvector_list_agents()
SET is_active = false
WHERE success_rate < 0.80;

-- Example 23: Remove inactive agents
SELECT ruvector_remove_agent(name)
FROM ruvector_list_agents()
WHERE NOT is_active
  AND total_requests = 0;

-- Example 24: Clear all agents (testing only)
-- SELECT ruvector_clear_agents();
