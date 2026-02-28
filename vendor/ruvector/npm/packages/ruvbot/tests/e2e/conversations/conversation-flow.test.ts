/**
 * Conversation Flow - E2E Tests
 *
 * End-to-end tests for complete agent conversation flows
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { createSession, createAgent, createTenant } from '../../factories';
import { createMockSlackApp, type MockSlackBoltApp } from '../../mocks/slack.mock';
import { createMockPool, type MockPool } from '../../mocks/postgres.mock';
import { createMockRuVectorBindings } from '../../mocks/wasm.mock';

// Mock RuvBot for E2E testing
class MockRuvBot {
  private app: MockSlackBoltApp;
  private pool: MockPool;
  private ruvector: ReturnType<typeof createMockRuVectorBindings>;
  private sessions: Map<string, any> = new Map();
  private agents: Map<string, any> = new Map();

  constructor() {
    this.app = createMockSlackApp();
    this.pool = createMockPool();
    this.ruvector = createMockRuVectorBindings();
    this.setupHandlers();
  }

  async start(): Promise<void> {
    await this.pool.connect();
    await this.app.start(3000);
  }

  async stop(): Promise<void> {
    await this.app.stop();
    await this.pool.end();
  }

  getApp(): MockSlackBoltApp {
    return this.app;
  }

  getPool(): MockPool {
    return this.pool;
  }

  getSession(key: string): any {
    return this.sessions.get(key);
  }

  async processMessage(message: {
    text: string;
    channel: string;
    user: string;
    ts: string;
    thread_ts?: string;
  }): Promise<void> {
    await this.app.processMessage(message);
  }

  private setupHandlers(): void {
    // Handle greetings
    this.app.message(/^(hi|hello|hey)/i, async ({ message, say }) => {
      const sessionKey = `${(message as any).channel}:${(message as any).thread_ts || (message as any).ts}`;

      // Create or get session
      if (!this.sessions.has(sessionKey)) {
        this.sessions.set(sessionKey, {
          id: `session-${Date.now()}`,
          channelId: (message as any).channel,
          threadTs: (message as any).thread_ts || (message as any).ts,
          userId: (message as any).user,
          messages: [],
          startedAt: new Date()
        });
      }

      const session = this.sessions.get(sessionKey);
      session.messages.push({ role: 'user', content: (message as any).text, timestamp: new Date() });

      await say({
        channel: (message as any).channel,
        text: 'Hello! I\'m RuvBot. How can I help you today?',
        thread_ts: (message as any).ts
      });

      session.messages.push({ role: 'assistant', content: 'Hello! I\'m RuvBot. How can I help you today?', timestamp: new Date() });
    });

    // Handle code generation requests
    this.app.message(/generate.*code|write.*function/i, async ({ message, say }) => {
      const sessionKey = `${(message as any).channel}:${(message as any).thread_ts || (message as any).ts}`;

      await say({
        channel: (message as any).channel,
        text: 'I\'ll generate that code for you. Give me a moment...',
        thread_ts: (message as any).ts
      });

      // Simulate code generation
      await new Promise(resolve => setTimeout(resolve, 100));

      await say({
        channel: (message as any).channel,
        text: '```javascript\nfunction example() {\n  console.log("Generated code");\n}\n```',
        thread_ts: (message as any).ts
      });

      const session = this.sessions.get(sessionKey);
      if (session) {
        session.messages.push({
          role: 'user',
          content: (message as any).text,
          timestamp: new Date()
        });
        session.messages.push({
          role: 'assistant',
          content: 'Code generated',
          artifact: { type: 'code', language: 'javascript' },
          timestamp: new Date()
        });
      }
    });

    // Handle help requests
    this.app.message(/help|what can you do/i, async ({ message, say }) => {
      await say({
        channel: (message as any).channel,
        text: 'I can help you with:\n- Code generation\n- Code review\n- Testing\n- Documentation\n\nJust ask me what you need!',
        thread_ts: (message as any).ts
      });
    });

    // Handle thank you
    this.app.message(/thanks|thank you/i, async ({ message, say }) => {
      const sessionKey = `${(message as any).channel}:${(message as any).thread_ts || (message as any).ts}`;

      await say({
        channel: (message as any).channel,
        text: 'You\'re welcome! Let me know if you need anything else.',
        thread_ts: (message as any).ts
      });

      // Mark session as potentially complete
      const session = this.sessions.get(sessionKey);
      if (session) {
        session.status = 'satisfied';
      }
    });

    // Handle search requests
    this.app.message(/search|find|look up/i, async ({ message, say }) => {
      await say({
        channel: (message as any).channel,
        text: 'Searching through the knowledge base...',
        thread_ts: (message as any).ts
      });

      // Simulate vector search
      const results = await this.ruvector.search((message as any).text, 3);

      if (results.length > 0) {
        await say({
          channel: (message as any).channel,
          text: `Found ${results.length} relevant results.`,
          thread_ts: (message as any).ts
        });
      } else {
        await say({
          channel: (message as any).channel,
          text: 'No relevant results found.',
          thread_ts: (message as any).ts
        });
      }
    });
  }
}

describe('E2E: Conversation Flow', () => {
  let bot: MockRuvBot;

  beforeEach(async () => {
    bot = new MockRuvBot();
    await bot.start();
  });

  afterEach(async () => {
    await bot.stop();
  });

  describe('Basic Conversation', () => {
    it('should handle greeting and establish session', async () => {
      const channel = 'C12345678';
      const ts = '1234567890.123456';

      await bot.processMessage({
        text: 'Hello!',
        channel,
        user: 'U12345678',
        ts
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages).toHaveLength(1);
      expect(messages[0].text).toContain('RuvBot');

      const session = bot.getSession(`${channel}:${ts}`);
      expect(session).toBeDefined();
      expect(session.messages).toHaveLength(2);
    });

    it('should maintain conversation context in thread', async () => {
      const channel = 'C12345678';
      const parentTs = '1234567890.111111';

      // Start conversation
      await bot.processMessage({
        text: 'Hi there',
        channel,
        user: 'U12345678',
        ts: parentTs
      });

      // Continue in thread
      await bot.processMessage({
        text: 'Help me generate code',
        channel,
        user: 'U12345678',
        ts: '1234567890.222222',
        thread_ts: parentTs
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages.length).toBeGreaterThanOrEqual(2);
    });
  });

  describe('Code Generation Flow', () => {
    it('should generate code on request', async () => {
      await bot.processMessage({
        text: 'Generate code for a hello world function',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages.length).toBeGreaterThanOrEqual(2);

      // Should have progress message and code block
      expect(messages.some(m => m.text?.includes('generating') || m.text?.includes('moment'))).toBe(true);
      expect(messages.some(m => m.text?.includes('```'))).toBe(true);
    });

    it('should handle follow-up questions about generated code', async () => {
      const channel = 'C12345678';
      const parentTs = '1234567890.111111';

      // Request code
      await bot.processMessage({
        text: 'Write a function to sort an array',
        channel,
        user: 'U12345678',
        ts: parentTs
      });

      // Ask for help about the code
      await bot.processMessage({
        text: 'Help me understand this',
        channel,
        user: 'U12345678',
        ts: '1234567890.222222',
        thread_ts: parentTs
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages.length).toBeGreaterThanOrEqual(3);
    });
  });

  describe('Help Flow', () => {
    it('should provide help information', async () => {
      await bot.processMessage({
        text: 'What can you do?',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      const messages = bot.getApp().client.getMessageLog();
      expect(messages).toHaveLength(1);
      expect(messages[0].text).toContain('Code generation');
      expect(messages[0].text).toContain('Code review');
    });
  });

  describe('Multi-turn Conversation', () => {
    it('should handle complete conversation lifecycle', async () => {
      const channel = 'C12345678';
      const parentTs = '1234567890.000001';

      // 1. Greeting
      await bot.processMessage({
        text: 'Hey',
        channel,
        user: 'U12345678',
        ts: parentTs
      });

      // 2. Request
      await bot.processMessage({
        text: 'Generate code for a calculator',
        channel,
        user: 'U12345678',
        ts: '1234567890.000002',
        thread_ts: parentTs
      });

      // 3. Thank you
      await bot.processMessage({
        text: 'Thank you!',
        channel,
        user: 'U12345678',
        ts: '1234567890.000003',
        thread_ts: parentTs
      });

      const session = bot.getSession(`${channel}:${parentTs}`);
      expect(session).toBeDefined();
      expect(session.messages.length).toBeGreaterThan(2);
      expect(session.status).toBe('satisfied');
    });
  });

  describe('Error Recovery', () => {
    it('should handle unknown requests gracefully', async () => {
      await bot.processMessage({
        text: 'asdfghjkl random gibberish',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      // Should not crash
      expect(true).toBe(true);
    });
  });
});

describe('E2E: Multi-user Conversations', () => {
  let bot: MockRuvBot;

  beforeEach(async () => {
    bot = new MockRuvBot();
    await bot.start();
  });

  afterEach(async () => {
    await bot.stop();
  });

  it('should handle multiple concurrent users', async () => {
    const users = ['U11111111', 'U22222222', 'U33333333'];
    const channel = 'C12345678';

    // All users send messages
    for (let i = 0; i < users.length; i++) {
      await bot.processMessage({
        text: 'Hello',
        channel,
        user: users[i],
        ts: `${Date.now()}.${i}`
      });
    }

    const messages = bot.getApp().client.getMessageLog();
    expect(messages).toHaveLength(3); // One response per user
  });

  it('should maintain separate sessions per thread', async () => {
    const channel = 'C12345678';

    // User 1 starts thread
    await bot.processMessage({
      text: 'Hi',
      channel,
      user: 'U11111111',
      ts: '1234567890.111111'
    });

    // User 2 starts different thread
    await bot.processMessage({
      text: 'Hello',
      channel,
      user: 'U22222222',
      ts: '1234567890.222222'
    });

    const session1 = bot.getSession(`${channel}:1234567890.111111`);
    const session2 = bot.getSession(`${channel}:1234567890.222222`);

    expect(session1.userId).toBe('U11111111');
    expect(session2.userId).toBe('U22222222');
  });
});

describe('E2E: Cross-channel Conversations', () => {
  let bot: MockRuvBot;

  beforeEach(async () => {
    bot = new MockRuvBot();
    await bot.start();
  });

  afterEach(async () => {
    await bot.stop();
  });

  it('should handle messages from different channels', async () => {
    const channels = ['C11111111', 'C22222222', 'C33333333'];

    for (const channel of channels) {
      await bot.processMessage({
        text: 'Hello',
        channel,
        user: 'U12345678',
        ts: `${Date.now()}.000000`
      });
    }

    const messages = bot.getApp().client.getMessageLog();
    expect(messages).toHaveLength(3);

    // Each response should be in the correct channel
    const responseChannels = new Set(messages.map(m => m.channel));
    expect(responseChannels.size).toBe(3);
  });
});
