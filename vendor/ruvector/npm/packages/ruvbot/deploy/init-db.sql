-- =============================================================================
-- RuvBot - Database Initialization Script
-- =============================================================================
-- PostgreSQL schema for multi-tenant RuvBot deployment
-- Supports: Sessions, Memory, Skills, Events, Metrics
-- =============================================================================

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";  -- For text search

-- =============================================================================
-- Tenants (Multi-tenancy support)
-- =============================================================================

CREATE TABLE IF NOT EXISTS tenants (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(100) UNIQUE NOT NULL,
    settings JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Enable Row-Level Security
ALTER TABLE tenants ENABLE ROW LEVEL SECURITY;

-- =============================================================================
-- Users
-- =============================================================================

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    external_id VARCHAR(255),  -- Slack user ID, Discord ID, etc.
    username VARCHAR(255),
    email VARCHAR(255),
    avatar_url TEXT,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(tenant_id, external_id)
);

CREATE INDEX idx_users_tenant ON users(tenant_id);
CREATE INDEX idx_users_external ON users(external_id);

ALTER TABLE users ENABLE ROW LEVEL SECURITY;

-- =============================================================================
-- Agents
-- =============================================================================

CREATE TABLE IF NOT EXISTS agents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    type VARCHAR(50) DEFAULT 'assistant',
    model VARCHAR(100) DEFAULT 'claude-3-5-sonnet',
    system_prompt TEXT,
    config JSONB DEFAULT '{}',
    status VARCHAR(20) DEFAULT 'active',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_agents_tenant ON agents(tenant_id);

ALTER TABLE agents ENABLE ROW LEVEL SECURITY;

-- =============================================================================
-- Sessions
-- =============================================================================

CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    agent_id UUID REFERENCES agents(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    channel_type VARCHAR(50),  -- slack, discord, telegram, web
    channel_id VARCHAR(255),   -- Channel/room ID
    thread_id VARCHAR(255),    -- Thread ID if applicable
    status VARCHAR(20) DEFAULT 'active',
    metadata JSONB DEFAULT '{}',
    started_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    ended_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_sessions_tenant ON sessions(tenant_id);
CREATE INDEX idx_sessions_agent ON sessions(agent_id);
CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_channel ON sessions(channel_type, channel_id);
CREATE INDEX idx_sessions_status ON sessions(status) WHERE status = 'active';

ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;

-- =============================================================================
-- Turns (Conversation Messages)
-- =============================================================================

CREATE TABLE IF NOT EXISTS turns (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    session_id UUID REFERENCES sessions(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL,  -- user, assistant, system
    content TEXT NOT NULL,
    tokens_input INTEGER DEFAULT 0,
    tokens_output INTEGER DEFAULT 0,
    latency_ms INTEGER,
    tool_calls JSONB,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_turns_session ON turns(session_id);
CREATE INDEX idx_turns_created ON turns(created_at DESC);

-- =============================================================================
-- Memory (Long-term storage with vector search)
-- =============================================================================

CREATE TABLE IF NOT EXISTS memories (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    type VARCHAR(50) NOT NULL,  -- fact, preference, episode, semantic
    content TEXT NOT NULL,
    embedding FLOAT8[],  -- Vector embedding (1536 dimensions for OpenAI, 768 for local)
    importance FLOAT DEFAULT 0.5,
    access_count INTEGER DEFAULT 0,
    last_accessed TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_memories_tenant ON memories(tenant_id);
CREATE INDEX idx_memories_user ON memories(user_id);
CREATE INDEX idx_memories_type ON memories(type);
CREATE INDEX idx_memories_importance ON memories(importance DESC);

-- Full-text search index
CREATE INDEX idx_memories_content_trgm ON memories USING gin(content gin_trgm_ops);

ALTER TABLE memories ENABLE ROW LEVEL SECURITY;

-- =============================================================================
-- Skills
-- =============================================================================

CREATE TABLE IF NOT EXISTS skills (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    category VARCHAR(100),
    version VARCHAR(20) DEFAULT '1.0.0',
    schema JSONB NOT NULL,  -- Input/output schema
    implementation JSONB,   -- Skill configuration
    enabled BOOLEAN DEFAULT true,
    usage_count INTEGER DEFAULT 0,
    success_rate FLOAT DEFAULT 1.0,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(tenant_id, name, version)
);

CREATE INDEX idx_skills_tenant ON skills(tenant_id);
CREATE INDEX idx_skills_category ON skills(category);
CREATE INDEX idx_skills_enabled ON skills(enabled) WHERE enabled = true;

ALTER TABLE skills ENABLE ROW LEVEL SECURITY;

-- =============================================================================
-- Events (Event Sourcing)
-- =============================================================================

CREATE TABLE IF NOT EXISTS events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    aggregate_type VARCHAR(100) NOT NULL,
    aggregate_id UUID NOT NULL,
    event_type VARCHAR(100) NOT NULL,
    event_version INTEGER DEFAULT 1,
    payload JSONB NOT NULL,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_events_aggregate ON events(aggregate_type, aggregate_id);
CREATE INDEX idx_events_type ON events(event_type);
CREATE INDEX idx_events_created ON events(created_at DESC);

-- =============================================================================
-- Metrics (Usage tracking)
-- =============================================================================

CREATE TABLE IF NOT EXISTS metrics (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    metric_name VARCHAR(100) NOT NULL,
    metric_value FLOAT NOT NULL,
    dimensions JSONB DEFAULT '{}',
    recorded_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_metrics_tenant ON metrics(tenant_id);
CREATE INDEX idx_metrics_name ON metrics(metric_name);
CREATE INDEX idx_metrics_recorded ON metrics(recorded_at DESC);

-- Partition by time for better performance
-- (In production, consider using TimescaleDB or partitioning)

-- =============================================================================
-- Patterns (Learning patterns from interactions)
-- =============================================================================

CREATE TABLE IF NOT EXISTS patterns (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID REFERENCES tenants(id) ON DELETE CASCADE,
    pattern_type VARCHAR(50) NOT NULL,  -- intent, response, workflow
    pattern_key VARCHAR(255) NOT NULL,
    pattern_value JSONB NOT NULL,
    confidence FLOAT DEFAULT 0.5,
    usage_count INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(tenant_id, pattern_type, pattern_key)
);

CREATE INDEX idx_patterns_tenant ON patterns(tenant_id);
CREATE INDEX idx_patterns_type ON patterns(pattern_type);
CREATE INDEX idx_patterns_confidence ON patterns(confidence DESC);

ALTER TABLE patterns ENABLE ROW LEVEL SECURITY;

-- =============================================================================
-- Functions
-- =============================================================================

-- Update timestamp trigger
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply to all tables with updated_at
DO $$
DECLARE
    t text;
BEGIN
    FOR t IN
        SELECT table_name
        FROM information_schema.columns
        WHERE column_name = 'updated_at'
        AND table_schema = 'public'
    LOOP
        EXECUTE format('
            DROP TRIGGER IF EXISTS update_%I_updated_at ON %I;
            CREATE TRIGGER update_%I_updated_at
            BEFORE UPDATE ON %I
            FOR EACH ROW
            EXECUTE FUNCTION update_updated_at();
        ', t, t, t, t);
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- Default Data
-- =============================================================================

-- Default tenant for development
INSERT INTO tenants (id, name, slug, settings)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'Default Tenant',
    'default',
    '{"plan": "free", "features": ["basic_chat", "memory", "skills"]}'
)
ON CONFLICT (slug) DO NOTHING;

-- Default agent
INSERT INTO agents (id, tenant_id, name, type, model, system_prompt, config)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    '00000000-0000-0000-0000-000000000001',
    'RuvBot',
    'assistant',
    'claude-3-5-sonnet',
    'You are RuvBot, a helpful AI assistant with long-term memory and learning capabilities.',
    '{"temperature": 0.7, "maxTokens": 4096}'
)
ON CONFLICT DO NOTHING;

-- =============================================================================
-- Grants (for application user)
-- =============================================================================

-- Note: Run these after creating the application user
-- GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO ruvbot;
-- GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO ruvbot;
-- GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO ruvbot;
