-- RuVector Hooks Intelligence Schema
-- PostgreSQL schema for self-learning hooks with pgvector support
-- Requires: ruvector extension (CREATE EXTENSION ruvector CASCADE)

-- ============================================================================
-- Q-Learning Patterns Table
-- Stores state-action pairs with Q-values for agent routing decisions
-- ============================================================================
CREATE TABLE IF NOT EXISTS ruvector_hooks_patterns (
    id SERIAL PRIMARY KEY,
    state TEXT NOT NULL,
    action TEXT NOT NULL,
    q_value REAL DEFAULT 0.0,
    visits INTEGER DEFAULT 0,
    last_update TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(state, action)
);

CREATE INDEX IF NOT EXISTS idx_patterns_state ON ruvector_hooks_patterns(state);
CREATE INDEX IF NOT EXISTS idx_patterns_q_value ON ruvector_hooks_patterns(q_value DESC);

-- ============================================================================
-- Vector Memory Table
-- Semantic memory with pgvector embeddings for context retrieval
-- ============================================================================
CREATE TABLE IF NOT EXISTS ruvector_hooks_memories (
    id SERIAL PRIMARY KEY,
    memory_type TEXT NOT NULL,
    content TEXT NOT NULL,
    embedding ruvector,  -- Uses native ruvector type
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_memories_type ON ruvector_hooks_memories(memory_type);
CREATE INDEX IF NOT EXISTS idx_memories_created ON ruvector_hooks_memories(created_at DESC);
-- Note: HNSW index on embedding created after extension is ready
-- CREATE INDEX IF NOT EXISTS idx_memories_embedding ON ruvector_hooks_memories
--     USING hnsw (embedding ruvector_cosine_ops) WITH (m = 16, ef_construction = 64);

-- ============================================================================
-- Learning Trajectories Table
-- Records of state-action-reward sequences for reinforcement learning
-- ============================================================================
CREATE TABLE IF NOT EXISTS ruvector_hooks_trajectories (
    id SERIAL PRIMARY KEY,
    state TEXT NOT NULL,
    action TEXT NOT NULL,
    outcome TEXT,
    reward REAL DEFAULT 0.0,
    context JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_trajectories_state ON ruvector_hooks_trajectories(state);
CREATE INDEX IF NOT EXISTS idx_trajectories_reward ON ruvector_hooks_trajectories(reward DESC);
CREATE INDEX IF NOT EXISTS idx_trajectories_created ON ruvector_hooks_trajectories(created_at DESC);

-- ============================================================================
-- Error Patterns Table
-- Learned error patterns with suggested fixes
-- ============================================================================
CREATE TABLE IF NOT EXISTS ruvector_hooks_errors (
    id SERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    error_type TEXT NOT NULL,
    message TEXT,
    fixes TEXT[] DEFAULT '{}',
    occurrences INTEGER DEFAULT 1,
    last_seen TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_errors_code ON ruvector_hooks_errors(code);
CREATE INDEX IF NOT EXISTS idx_errors_type ON ruvector_hooks_errors(error_type);

-- ============================================================================
-- File Sequences Table
-- Tracks file edit sequences for predicting next files
-- ============================================================================
CREATE TABLE IF NOT EXISTS ruvector_hooks_file_sequences (
    id SERIAL PRIMARY KEY,
    from_file TEXT NOT NULL,
    to_file TEXT NOT NULL,
    count INTEGER DEFAULT 1,
    last_seen TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(from_file, to_file)
);

CREATE INDEX IF NOT EXISTS idx_sequences_from ON ruvector_hooks_file_sequences(from_file);
CREATE INDEX IF NOT EXISTS idx_sequences_count ON ruvector_hooks_file_sequences(count DESC);

-- ============================================================================
-- Swarm Agents Table
-- Registered agents in the swarm with performance metrics
-- ============================================================================
CREATE TABLE IF NOT EXISTS ruvector_hooks_swarm_agents (
    id TEXT PRIMARY KEY,
    agent_type TEXT NOT NULL,
    capabilities TEXT[] DEFAULT '{}',
    success_rate REAL DEFAULT 1.0,
    task_count INTEGER DEFAULT 0,
    status TEXT DEFAULT 'active',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_agents_type ON ruvector_hooks_swarm_agents(agent_type);
CREATE INDEX IF NOT EXISTS idx_agents_status ON ruvector_hooks_swarm_agents(status);

-- ============================================================================
-- Swarm Edges Table
-- Coordination edges between agents
-- ============================================================================
CREATE TABLE IF NOT EXISTS ruvector_hooks_swarm_edges (
    id SERIAL PRIMARY KEY,
    source_agent TEXT NOT NULL REFERENCES ruvector_hooks_swarm_agents(id) ON DELETE CASCADE,
    target_agent TEXT NOT NULL REFERENCES ruvector_hooks_swarm_agents(id) ON DELETE CASCADE,
    weight REAL DEFAULT 1.0,
    coordination_count INTEGER DEFAULT 1,
    last_coordination TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(source_agent, target_agent)
);

CREATE INDEX IF NOT EXISTS idx_edges_source ON ruvector_hooks_swarm_edges(source_agent);
CREATE INDEX IF NOT EXISTS idx_edges_target ON ruvector_hooks_swarm_edges(target_agent);

-- ============================================================================
-- Session Stats Table
-- Global statistics for the intelligence layer
-- ============================================================================
CREATE TABLE IF NOT EXISTS ruvector_hooks_stats (
    id INTEGER PRIMARY KEY DEFAULT 1,
    session_count INTEGER DEFAULT 0,
    last_session TIMESTAMPTZ DEFAULT NOW(),
    total_edits INTEGER DEFAULT 0,
    total_commands INTEGER DEFAULT 0,
    total_errors_learned INTEGER DEFAULT 0,
    CHECK (id = 1)  -- Single row table
);

INSERT INTO ruvector_hooks_stats (id) VALUES (1) ON CONFLICT (id) DO NOTHING;

-- ============================================================================
-- Helper Functions
-- ============================================================================

-- Update Q-value using Q-learning formula
CREATE OR REPLACE FUNCTION ruvector_hooks_update_q(
    p_state TEXT,
    p_action TEXT,
    p_reward REAL,
    p_alpha REAL DEFAULT 0.1
) RETURNS VOID AS $$
BEGIN
    INSERT INTO ruvector_hooks_patterns (state, action, q_value, visits, last_update)
    VALUES (p_state, p_action, p_reward * p_alpha, 1, NOW())
    ON CONFLICT (state, action) DO UPDATE SET
        q_value = ruvector_hooks_patterns.q_value + p_alpha * (p_reward - ruvector_hooks_patterns.q_value),
        visits = ruvector_hooks_patterns.visits + 1,
        last_update = NOW();
END;
$$ LANGUAGE plpgsql;

-- Get best action for state
CREATE OR REPLACE FUNCTION ruvector_hooks_best_action(
    p_state TEXT,
    p_actions TEXT[]
) RETURNS TABLE(action TEXT, q_value REAL, confidence REAL) AS $$
BEGIN
    RETURN QUERY
    SELECT
        p.action,
        p.q_value,
        CASE WHEN p.q_value > 0 THEN LEAST(p.q_value, 1.0) ELSE 0.0 END as confidence
    FROM ruvector_hooks_patterns p
    WHERE p.state = p_state
      AND p.action = ANY(p_actions)
    ORDER BY p.q_value DESC
    LIMIT 1;

    -- If no match found, return first action with 0 confidence
    IF NOT FOUND THEN
        RETURN QUERY SELECT p_actions[1], 0.0::REAL, 0.0::REAL;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Remember content with embedding
CREATE OR REPLACE FUNCTION ruvector_hooks_remember(
    p_type TEXT,
    p_content TEXT,
    p_embedding REAL[] DEFAULT NULL,
    p_metadata JSONB DEFAULT '{}'
) RETURNS INTEGER AS $$
DECLARE
    v_id INTEGER;
BEGIN
    INSERT INTO ruvector_hooks_memories (memory_type, content, embedding, metadata)
    VALUES (p_type, p_content,
            CASE WHEN p_embedding IS NOT NULL THEN p_embedding::TEXT::ruvector ELSE NULL END,
            p_metadata)
    RETURNING id INTO v_id;

    -- Cleanup old memories (keep last 5000)
    DELETE FROM ruvector_hooks_memories
    WHERE id IN (
        SELECT id FROM ruvector_hooks_memories
        ORDER BY created_at ASC
        OFFSET 5000
    );

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

-- Recall from memory using semantic search
CREATE OR REPLACE FUNCTION ruvector_hooks_recall(
    p_query_embedding REAL[],
    p_limit INTEGER DEFAULT 5
) RETURNS TABLE(
    id INTEGER,
    memory_type TEXT,
    content TEXT,
    metadata JSONB,
    similarity REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        m.id,
        m.memory_type,
        m.content,
        m.metadata,
        1.0 - (m.embedding <=> p_query_embedding::TEXT::ruvector) as similarity
    FROM ruvector_hooks_memories m
    WHERE m.embedding IS NOT NULL
    ORDER BY m.embedding <=> p_query_embedding::TEXT::ruvector
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

-- Record file sequence
CREATE OR REPLACE FUNCTION ruvector_hooks_record_sequence(
    p_from_file TEXT,
    p_to_file TEXT
) RETURNS VOID AS $$
BEGIN
    INSERT INTO ruvector_hooks_file_sequences (from_file, to_file, count, last_seen)
    VALUES (p_from_file, p_to_file, 1, NOW())
    ON CONFLICT (from_file, to_file) DO UPDATE SET
        count = ruvector_hooks_file_sequences.count + 1,
        last_seen = NOW();
END;
$$ LANGUAGE plpgsql;

-- Get suggested next files
CREATE OR REPLACE FUNCTION ruvector_hooks_suggest_next(
    p_file TEXT,
    p_limit INTEGER DEFAULT 3
) RETURNS TABLE(to_file TEXT, count INTEGER) AS $$
BEGIN
    RETURN QUERY
    SELECT fs.to_file, fs.count
    FROM ruvector_hooks_file_sequences fs
    WHERE fs.from_file = p_file
    ORDER BY fs.count DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

-- Record error pattern
CREATE OR REPLACE FUNCTION ruvector_hooks_record_error(
    p_code TEXT,
    p_type TEXT,
    p_message TEXT DEFAULT NULL
) RETURNS VOID AS $$
BEGIN
    INSERT INTO ruvector_hooks_errors (code, error_type, message, occurrences, last_seen)
    VALUES (p_code, p_type, p_message, 1, NOW())
    ON CONFLICT (code) DO UPDATE SET
        occurrences = ruvector_hooks_errors.occurrences + 1,
        last_seen = NOW(),
        message = COALESCE(p_message, ruvector_hooks_errors.message);
END;
$$ LANGUAGE plpgsql;

-- Register swarm agent
CREATE OR REPLACE FUNCTION ruvector_hooks_swarm_register(
    p_id TEXT,
    p_type TEXT,
    p_capabilities TEXT[] DEFAULT '{}'
) RETURNS VOID AS $$
BEGIN
    INSERT INTO ruvector_hooks_swarm_agents (id, agent_type, capabilities)
    VALUES (p_id, p_type, p_capabilities)
    ON CONFLICT (id) DO UPDATE SET
        agent_type = p_type,
        capabilities = p_capabilities,
        updated_at = NOW();
END;
$$ LANGUAGE plpgsql;

-- Record swarm coordination
CREATE OR REPLACE FUNCTION ruvector_hooks_swarm_coordinate(
    p_source TEXT,
    p_target TEXT,
    p_weight REAL DEFAULT 1.0
) RETURNS VOID AS $$
BEGIN
    INSERT INTO ruvector_hooks_swarm_edges (source_agent, target_agent, weight, coordination_count)
    VALUES (p_source, p_target, p_weight, 1)
    ON CONFLICT (source_agent, target_agent) DO UPDATE SET
        weight = (ruvector_hooks_swarm_edges.weight + p_weight) / 2,
        coordination_count = ruvector_hooks_swarm_edges.coordination_count + 1,
        last_coordination = NOW();
END;
$$ LANGUAGE plpgsql;

-- Get swarm stats
CREATE OR REPLACE FUNCTION ruvector_hooks_swarm_stats()
RETURNS TABLE(
    agent_count INTEGER,
    edge_count INTEGER,
    avg_success_rate REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_swarm_agents WHERE status = 'active'),
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_swarm_edges),
        (SELECT COALESCE(AVG(success_rate), 0.0)::REAL FROM ruvector_hooks_swarm_agents WHERE status = 'active');
END;
$$ LANGUAGE plpgsql;

-- Increment session count
CREATE OR REPLACE FUNCTION ruvector_hooks_session_start()
RETURNS VOID AS $$
BEGIN
    UPDATE ruvector_hooks_stats
    SET session_count = session_count + 1,
        last_session = NOW()
    WHERE id = 1;
END;
$$ LANGUAGE plpgsql;

-- Get full stats
CREATE OR REPLACE FUNCTION ruvector_hooks_get_stats()
RETURNS TABLE(
    patterns INTEGER,
    memories INTEGER,
    trajectories INTEGER,
    errors INTEGER,
    sessions INTEGER,
    agents INTEGER,
    edges INTEGER
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_patterns),
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_memories),
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_trajectories),
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_errors),
        (SELECT session_count FROM ruvector_hooks_stats WHERE id = 1),
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_swarm_agents),
        (SELECT COUNT(*)::INTEGER FROM ruvector_hooks_swarm_edges);
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- Comments
-- ============================================================================
COMMENT ON TABLE ruvector_hooks_patterns IS 'Q-learning patterns for agent routing decisions';
COMMENT ON TABLE ruvector_hooks_memories IS 'Semantic memory with vector embeddings';
COMMENT ON TABLE ruvector_hooks_trajectories IS 'Reinforcement learning trajectories';
COMMENT ON TABLE ruvector_hooks_errors IS 'Learned error patterns and fixes';
COMMENT ON TABLE ruvector_hooks_file_sequences IS 'File edit sequence predictions';
COMMENT ON TABLE ruvector_hooks_swarm_agents IS 'Registered swarm agents';
COMMENT ON TABLE ruvector_hooks_swarm_edges IS 'Agent coordination graph';
COMMENT ON TABLE ruvector_hooks_stats IS 'Global intelligence statistics';
