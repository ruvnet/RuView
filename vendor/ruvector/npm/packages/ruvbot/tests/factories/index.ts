/**
 * Test Factories
 *
 * Factory functions for creating test data with customizable overrides
 */

import { v4 as uuidv4 } from 'uuid';

// Types
export interface Agent {
  id: string;
  name: string;
  type: 'coder' | 'researcher' | 'tester' | 'reviewer' | 'planner';
  status: 'idle' | 'busy' | 'error' | 'terminated';
  capabilities: string[];
  config: AgentConfig;
  metadata: EntityMetadata;
}

export interface AgentConfig {
  model: string;
  temperature: number;
  maxTokens: number;
  systemPrompt?: string;
}

export interface Session {
  id: string;
  tenantId: string;
  userId: string;
  channelId: string;
  threadTs: string;
  status: 'active' | 'paused' | 'completed' | 'error';
  context: SessionContext;
  metadata: SessionMetadata;
}

export interface SessionContext {
  conversationHistory: ConversationMessage[];
  workingDirectory: string;
  activeAgents: string[];
  variables?: Record<string, unknown>;
}

export interface ConversationMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: Date;
  agentId?: string;
}

export interface SessionMetadata {
  createdAt: Date;
  lastActiveAt: Date;
  messageCount: number;
}

export interface Memory {
  id: string;
  sessionId: string | null;
  tenantId: string;
  type: 'short-term' | 'long-term' | 'vector' | 'episodic';
  key: string;
  value: unknown;
  embedding: Float32Array | null;
  metadata: MemoryMetadata;
}

export interface MemoryMetadata {
  createdAt: Date;
  expiresAt: Date | null;
  accessCount: number;
  importance?: number;
}

export interface Skill {
  id: string;
  name: string;
  version: string;
  description: string;
  inputSchema: Record<string, unknown>;
  outputSchema: Record<string, unknown>;
  executor: string;
  timeout: number;
  metadata?: Record<string, unknown>;
}

export interface Tenant {
  id: string;
  name: string;
  slackTeamId: string;
  status: 'active' | 'suspended' | 'trial';
  plan: 'free' | 'pro' | 'enterprise';
  config: TenantConfig;
  metadata: EntityMetadata;
}

export interface TenantConfig {
  maxAgents: number;
  maxSessions: number;
  features: string[];
  customSkills?: string[];
}

export interface EntityMetadata {
  createdAt: Date;
  updatedAt: Date;
  version?: string;
}

// Factory Functions

/**
 * Create an Agent with optional overrides
 */
export function createAgent(overrides: Partial<Agent> = {}): Agent {
  const defaults: Agent = {
    id: `agent-${uuidv4().slice(0, 8)}`,
    name: 'Test Agent',
    type: 'coder',
    status: 'idle',
    capabilities: ['code-generation'],
    config: {
      model: 'claude-sonnet-4',
      temperature: 0.7,
      maxTokens: 4096
    },
    metadata: {
      createdAt: new Date(),
      updatedAt: new Date(),
      version: '1.0.0'
    }
  };

  return {
    ...defaults,
    ...overrides,
    config: { ...defaults.config, ...overrides.config },
    metadata: { ...defaults.metadata, ...overrides.metadata }
  };
}

/**
 * Create multiple agents
 */
export function createAgents(count: number, overrides: Partial<Agent> = {}): Agent[] {
  return Array.from({ length: count }, (_, i) =>
    createAgent({
      ...overrides,
      name: `Agent ${i + 1}`,
      id: `agent-${i + 1}`
    })
  );
}

/**
 * Create a Session with optional overrides
 */
export function createSession(overrides: Partial<Session> = {}): Session {
  const defaults: Session = {
    id: `session-${uuidv4().slice(0, 8)}`,
    tenantId: 'tenant-001',
    userId: 'U12345678',
    channelId: 'C12345678',
    threadTs: `${Date.now()}.000000`,
    status: 'active',
    context: {
      conversationHistory: [],
      workingDirectory: '/workspace',
      activeAgents: []
    },
    metadata: {
      createdAt: new Date(),
      lastActiveAt: new Date(),
      messageCount: 0
    }
  };

  return {
    ...defaults,
    ...overrides,
    context: { ...defaults.context, ...overrides.context },
    metadata: { ...defaults.metadata, ...overrides.metadata }
  };
}

/**
 * Create a Session with conversation history
 */
export function createSessionWithHistory(
  messageCount: number,
  overrides: Partial<Session> = {}
): Session {
  const history: ConversationMessage[] = [];

  for (let i = 0; i < messageCount; i++) {
    history.push({
      role: i % 2 === 0 ? 'user' : 'assistant',
      content: `Message ${i + 1}`,
      timestamp: new Date(Date.now() - (messageCount - i) * 60000)
    });
  }

  return createSession({
    ...overrides,
    context: {
      ...overrides.context,
      conversationHistory: history
    },
    metadata: {
      ...overrides.metadata,
      messageCount
    }
  });
}

/**
 * Create a Memory entry with optional overrides
 */
export function createMemory(overrides: Partial<Memory> = {}): Memory {
  const defaults: Memory = {
    id: `mem-${uuidv4().slice(0, 8)}`,
    sessionId: null,
    tenantId: 'tenant-001',
    type: 'short-term',
    key: `key-${Date.now()}`,
    value: { data: 'test' },
    embedding: null,
    metadata: {
      createdAt: new Date(),
      expiresAt: null,
      accessCount: 0
    }
  };

  return {
    ...defaults,
    ...overrides,
    metadata: { ...defaults.metadata, ...overrides.metadata }
  };
}

/**
 * Create a Memory entry with vector embedding
 */
export function createVectorMemory(
  dimension: number = 384,
  overrides: Partial<Memory> = {}
): Memory {
  return createMemory({
    type: 'vector',
    embedding: new Float32Array(dimension).map(() => Math.random() - 0.5),
    ...overrides
  });
}

/**
 * Create a Skill with optional overrides
 */
export function createSkill(overrides: Partial<Skill> = {}): Skill {
  const defaults: Skill = {
    id: `skill-${uuidv4().slice(0, 8)}`,
    name: 'test-skill',
    version: '1.0.0',
    description: 'A test skill',
    inputSchema: {
      type: 'object',
      properties: {
        input: { type: 'string' }
      },
      required: ['input']
    },
    outputSchema: {
      type: 'object',
      properties: {
        output: { type: 'string' }
      }
    },
    executor: 'native://test',
    timeout: 30000
  };

  return { ...defaults, ...overrides };
}

/**
 * Create a Tenant with optional overrides
 */
export function createTenant(overrides: Partial<Tenant> = {}): Tenant {
  const defaults: Tenant = {
    id: `tenant-${uuidv4().slice(0, 8)}`,
    name: 'Test Tenant',
    slackTeamId: `T${uuidv4().slice(0, 8).toUpperCase()}`,
    status: 'active',
    plan: 'pro',
    config: {
      maxAgents: 10,
      maxSessions: 100,
      features: ['code-generation', 'vector-search']
    },
    metadata: {
      createdAt: new Date(),
      updatedAt: new Date()
    }
  };

  return {
    ...defaults,
    ...overrides,
    config: { ...defaults.config, ...overrides.config },
    metadata: { ...defaults.metadata, ...overrides.metadata }
  };
}

/**
 * Create a Slack message event
 */
export function createSlackMessageEvent(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    type: 'message',
    channel: 'C12345678',
    user: 'U12345678',
    text: 'Test message',
    ts: `${Date.now()}.000000`,
    team: 'T12345678',
    event_ts: `${Date.now()}.000000`,
    ...overrides
  };
}

/**
 * Create a Slack app_mention event
 */
export function createSlackMentionEvent(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    type: 'app_mention',
    channel: 'C12345678',
    user: 'U12345678',
    text: '<@U_BOT> test mention',
    ts: `${Date.now()}.000000`,
    team: 'T12345678',
    event_ts: `${Date.now()}.000000`,
    ...overrides
  };
}

/**
 * Batch factory - create multiple related entities
 */
export function createTestScenario() {
  const tenant = createTenant();
  const session = createSession({ tenantId: tenant.id });
  const agents = createAgents(3, { type: 'coder' });
  const memory = createVectorMemory(384, { tenantId: tenant.id, sessionId: session.id });
  const skill = createSkill();

  return {
    tenant,
    session,
    agents,
    memory,
    skill
  };
}
