/**
 * Test Fixtures Index
 *
 * Centralized fixture exports for RuvBot tests
 */

// Agent Fixtures
export const agentFixtures = {
  basicAgent: {
    id: 'agent-001',
    name: 'Test Agent',
    type: 'coder' as const,
    status: 'idle' as const,
    capabilities: ['code-generation', 'code-review'],
    config: {
      model: 'claude-sonnet-4',
      temperature: 0.7,
      maxTokens: 4096
    },
    metadata: {
      createdAt: new Date('2024-01-01'),
      updatedAt: new Date('2024-01-01'),
      version: '1.0.0'
    }
  },

  researcherAgent: {
    id: 'agent-002',
    name: 'Research Agent',
    type: 'researcher' as const,
    status: 'idle' as const,
    capabilities: ['web-search', 'document-analysis', 'summarization'],
    config: {
      model: 'claude-sonnet-4',
      temperature: 0.5,
      maxTokens: 8192
    },
    metadata: {
      createdAt: new Date('2024-01-01'),
      updatedAt: new Date('2024-01-01'),
      version: '1.0.0'
    }
  },

  testerAgent: {
    id: 'agent-003',
    name: 'Tester Agent',
    type: 'tester' as const,
    status: 'idle' as const,
    capabilities: ['test-generation', 'test-execution', 'coverage-analysis'],
    config: {
      model: 'claude-haiku-3',
      temperature: 0.3,
      maxTokens: 4096
    },
    metadata: {
      createdAt: new Date('2024-01-01'),
      updatedAt: new Date('2024-01-01'),
      version: '1.0.0'
    }
  }
};

// Session Fixtures
export const sessionFixtures = {
  basicSession: {
    id: 'session-001',
    tenantId: 'tenant-001',
    userId: 'user-001',
    channelId: 'C12345678',
    threadTs: '1234567890.123456',
    status: 'active' as const,
    context: {
      conversationHistory: [],
      workingDirectory: '/workspace',
      activeAgents: []
    },
    metadata: {
      createdAt: new Date('2024-01-01'),
      lastActiveAt: new Date('2024-01-01'),
      messageCount: 0
    }
  },

  activeSession: {
    id: 'session-002',
    tenantId: 'tenant-001',
    userId: 'user-001',
    channelId: 'C12345678',
    threadTs: '1234567890.654321',
    status: 'active' as const,
    context: {
      conversationHistory: [
        { role: 'user', content: 'Hello', timestamp: new Date() },
        { role: 'assistant', content: 'Hi there!', timestamp: new Date() }
      ],
      workingDirectory: '/workspace/project',
      activeAgents: ['agent-001']
    },
    metadata: {
      createdAt: new Date('2024-01-01'),
      lastActiveAt: new Date('2024-01-02'),
      messageCount: 2
    }
  }
};

// Memory Fixtures
export const memoryFixtures = {
  shortTermMemory: {
    id: 'mem-001',
    sessionId: 'session-001',
    type: 'short-term' as const,
    key: 'current-task',
    value: { task: 'implement feature', priority: 'high' },
    embedding: new Float32Array(384).fill(0.1),
    metadata: {
      createdAt: new Date(),
      expiresAt: new Date(Date.now() + 3600000),
      accessCount: 1
    }
  },

  longTermMemory: {
    id: 'mem-002',
    sessionId: null,
    tenantId: 'tenant-001',
    type: 'long-term' as const,
    key: 'coding-pattern-react',
    value: { pattern: 'functional-components', examples: [] },
    embedding: new Float32Array(384).fill(0.2),
    metadata: {
      createdAt: new Date('2024-01-01'),
      expiresAt: null,
      accessCount: 42
    }
  },

  vectorMemory: {
    id: 'mem-003',
    tenantId: 'tenant-001',
    type: 'vector' as const,
    key: 'codebase-embeddings',
    value: { path: '/src/index.ts', summary: 'Main entry point' },
    embedding: new Float32Array(384).map(() => Math.random() - 0.5),
    metadata: {
      createdAt: new Date(),
      expiresAt: null,
      accessCount: 10
    }
  }
};

// Skill Fixtures
export const skillFixtures = {
  codeGenerationSkill: {
    id: 'skill-001',
    name: 'code-generation',
    version: '1.0.0',
    description: 'Generate code based on natural language descriptions',
    inputSchema: {
      type: 'object',
      properties: {
        language: { type: 'string' },
        description: { type: 'string' },
        context: { type: 'object' }
      },
      required: ['language', 'description']
    },
    outputSchema: {
      type: 'object',
      properties: {
        code: { type: 'string' },
        explanation: { type: 'string' }
      }
    },
    executor: 'wasm://skills/code-generation',
    timeout: 30000
  },

  testGenerationSkill: {
    id: 'skill-002',
    name: 'test-generation',
    version: '1.0.0',
    description: 'Generate tests for given code',
    inputSchema: {
      type: 'object',
      properties: {
        code: { type: 'string' },
        framework: { type: 'string' },
        coverage: { type: 'string' }
      },
      required: ['code']
    },
    outputSchema: {
      type: 'object',
      properties: {
        tests: { type: 'string' },
        coverage: { type: 'number' }
      }
    },
    executor: 'wasm://skills/test-generation',
    timeout: 60000
  },

  vectorSearchSkill: {
    id: 'skill-003',
    name: 'vector-search',
    version: '1.0.0',
    description: 'Search vector embeddings using RuVector',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string' },
        topK: { type: 'number' },
        threshold: { type: 'number' }
      },
      required: ['query']
    },
    outputSchema: {
      type: 'object',
      properties: {
        results: { type: 'array' },
        scores: { type: 'array' }
      }
    },
    executor: 'native://ruvector/search',
    timeout: 5000
  }
};

// Slack Event Fixtures
export const slackFixtures = {
  messageEvent: {
    type: 'message',
    channel: 'C12345678',
    user: 'U12345678',
    text: 'Hello, bot!',
    ts: '1234567890.123456',
    team: 'T12345678',
    event_ts: '1234567890.123456'
  },

  appMentionEvent: {
    type: 'app_mention',
    channel: 'C12345678',
    user: 'U12345678',
    text: '<@U_BOT> help me with this code',
    ts: '1234567890.123456',
    team: 'T12345678',
    event_ts: '1234567890.123456'
  },

  threadReplyEvent: {
    type: 'message',
    channel: 'C12345678',
    user: 'U12345678',
    text: 'This is a reply',
    ts: '1234567890.654321',
    thread_ts: '1234567890.123456',
    team: 'T12345678',
    event_ts: '1234567890.654321'
  }
};

// Tenant Fixtures
export const tenantFixtures = {
  basicTenant: {
    id: 'tenant-001',
    name: 'Acme Corp',
    slackTeamId: 'T12345678',
    status: 'active' as const,
    plan: 'pro',
    config: {
      maxAgents: 10,
      maxSessions: 100,
      features: ['code-generation', 'vector-search']
    },
    metadata: {
      createdAt: new Date('2024-01-01'),
      updatedAt: new Date('2024-01-01')
    }
  },

  enterpriseTenant: {
    id: 'tenant-002',
    name: 'Enterprise Inc',
    slackTeamId: 'T87654321',
    status: 'active' as const,
    plan: 'enterprise',
    config: {
      maxAgents: 100,
      maxSessions: 1000,
      features: ['code-generation', 'vector-search', 'custom-skills', 'sso']
    },
    metadata: {
      createdAt: new Date('2024-01-01'),
      updatedAt: new Date('2024-01-01')
    }
  }
};
