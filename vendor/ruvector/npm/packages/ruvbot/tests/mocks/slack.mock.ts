/**
 * Slack API Mock Module
 *
 * Mock implementations for Slack Web API and Events API
 */

import { vi } from 'vitest';

// Types
export interface SlackMessage {
  channel: string;
  text: string;
  thread_ts?: string;
  blocks?: unknown[];
  attachments?: unknown[];
  metadata?: Record<string, unknown>;
}

export interface SlackResponse {
  ok: boolean;
  error?: string;
  ts?: string;
  channel?: string;
  message?: Record<string, unknown>;
}

export interface SlackUser {
  id: string;
  name: string;
  real_name: string;
  is_bot: boolean;
  team_id: string;
}

export interface SlackChannel {
  id: string;
  name: string;
  is_private: boolean;
  is_member: boolean;
  team_id: string;
}

/**
 * Mock Slack Web Client
 */
export class MockSlackWebClient {
  private messageLog: SlackMessage[] = [];
  private _reactionsData: Map<string, string[]> = new Map();
  private _filesData: Map<string, unknown> = new Map();

  // User and channel data
  private _usersData: Map<string, SlackUser> = new Map();
  private _channelsData: Map<string, SlackChannel> = new Map();

  constructor() {
    // Seed default test data
    this.seedDefaultData();
  }

  // Chat API
  chat = {
    postMessage: vi.fn(async (args: SlackMessage): Promise<SlackResponse> => {
      this.messageLog.push(args);
      const ts = `${Date.now()}.${Math.random().toString().slice(2, 8)}`;
      return {
        ok: true,
        ts,
        channel: args.channel,
        message: {
          text: args.text,
          ts,
          user: 'U_BOT',
          type: 'message'
        }
      };
    }),

    update: vi.fn(async (args: { channel: string; ts: string; text?: string; blocks?: unknown[] }): Promise<SlackResponse> => {
      return {
        ok: true,
        ts: args.ts,
        channel: args.channel
      };
    }),

    delete: vi.fn(async (args: { channel: string; ts: string }): Promise<SlackResponse> => {
      return {
        ok: true,
        ts: args.ts,
        channel: args.channel
      };
    }),

    postEphemeral: vi.fn(async (args: SlackMessage & { user: string }): Promise<SlackResponse> => {
      this.messageLog.push(args);
      return {
        ok: true,
        message_ts: `${Date.now()}.${Math.random().toString().slice(2, 8)}`
      } as SlackResponse;
    })
  };

  // Conversations API
  conversations = {
    info: vi.fn(async (args: { channel: string }): Promise<{ ok: boolean; channel?: SlackChannel }> => {
      const channel = this._channelsData.get(args.channel);
      return {
        ok: !!channel,
        channel
      };
    }),

    members: vi.fn(async (args: { channel: string }): Promise<{ ok: boolean; members: string[] }> => {
      return {
        ok: true,
        members: ['U12345678', 'U87654321', 'U_BOT']
      };
    }),

    history: vi.fn(async (args: { channel: string; limit?: number }): Promise<{ ok: boolean; messages: unknown[] }> => {
      return {
        ok: true,
        messages: this.messageLog
          .filter(m => m.channel === args.channel)
          .slice(0, args.limit || 100)
      };
    }),

    replies: vi.fn(async (args: { channel: string; ts: string }): Promise<{ ok: boolean; messages: unknown[] }> => {
      return {
        ok: true,
        messages: this.messageLog
          .filter(m => m.channel === args.channel && m.thread_ts === args.ts)
      };
    }),

    join: vi.fn(async (args: { channel: string }): Promise<SlackResponse> => {
      return { ok: true, channel: args.channel };
    }),

    leave: vi.fn(async (args: { channel: string }): Promise<SlackResponse> => {
      return { ok: true };
    })
  };

  // Users API
  users = {
    info: vi.fn(async (args: { user: string }): Promise<{ ok: boolean; user?: SlackUser }> => {
      const user = this._usersData.get(args.user);
      return {
        ok: !!user,
        user
      };
    }),

    list: vi.fn(async (): Promise<{ ok: boolean; members: SlackUser[] }> => {
      return {
        ok: true,
        members: Array.from(this._usersData.values())
      };
    })
  };

  // Reactions API
  reactions = {
    add: vi.fn(async (args: { channel: string; timestamp: string; name: string }): Promise<SlackResponse> => {
      const key = `${args.channel}:${args.timestamp}`;
      const existing = this._reactionsData.get(key) || [];
      this._reactionsData.set(key, [...existing, args.name]);
      return { ok: true };
    }),

    remove: vi.fn(async (args: { channel: string; timestamp: string; name: string }): Promise<SlackResponse> => {
      const key = `${args.channel}:${args.timestamp}`;
      const existing = this._reactionsData.get(key) || [];
      this._reactionsData.set(key, existing.filter(r => r !== args.name));
      return { ok: true };
    }),

    get: vi.fn(async (args: { channel: string; timestamp: string }): Promise<{ ok: boolean; message: { reactions: unknown[] } }> => {
      const key = `${args.channel}:${args.timestamp}`;
      const reactions = this._reactionsData.get(key) || [];
      return {
        ok: true,
        message: {
          reactions: reactions.map(name => ({ name, count: 1, users: ['U12345678'] }))
        }
      };
    })
  };

  // Files API
  files = {
    upload: vi.fn(async (args: { channels: string; content: string; filename: string }): Promise<{ ok: boolean; file: unknown }> => {
      const fileId = `F${Date.now()}`;
      const file = { id: fileId, name: args.filename, content: args.content };
      this._filesData.set(fileId, file);
      return { ok: true, file };
    }),

    delete: vi.fn(async (args: { file: string }): Promise<SlackResponse> => {
      this._filesData.delete(args.file);
      return { ok: true };
    })
  };

  // Auth API
  auth = {
    test: vi.fn(async (): Promise<{ ok: boolean; user_id: string; team_id: string; bot_id: string }> => {
      return {
        ok: true,
        user_id: 'U_BOT',
        team_id: 'T12345678',
        bot_id: 'B12345678'
      };
    })
  };

  // Test helpers
  getMessageLog(): SlackMessage[] {
    return [...this.messageLog];
  }

  clearMessageLog(): void {
    this.messageLog = [];
  }

  getReactions(channel: string, timestamp: string): string[] {
    return this._reactionsData.get(`${channel}:${timestamp}`) || [];
  }

  addUser(user: SlackUser): void {
    this._usersData.set(user.id, user);
  }

  addChannel(channel: SlackChannel): void {
    this._channelsData.set(channel.id, channel);
  }

  reset(): void {
    this.messageLog = [];
    this._reactionsData.clear();
    this._filesData.clear();
    this.seedDefaultData();

    // Reset all mocks
    vi.clearAllMocks();
  }

  private seedDefaultData(): void {
    // Default users
    this._usersData.set('U12345678', {
      id: 'U12345678',
      name: 'testuser',
      real_name: 'Test User',
      is_bot: false,
      team_id: 'T12345678'
    });

    this._usersData.set('U_BOT', {
      id: 'U_BOT',
      name: 'ruvbot',
      real_name: 'RuvBot',
      is_bot: true,
      team_id: 'T12345678'
    });

    // Default channels
    this._channelsData.set('C12345678', {
      id: 'C12345678',
      name: 'general',
      is_private: false,
      is_member: true,
      team_id: 'T12345678'
    });
  }
}

/**
 * Mock Slack Events Handler
 */
export class MockSlackEventsHandler {
  private eventHandlers: Map<string, Array<(event: unknown) => void>> = new Map();
  private processedEvents: unknown[] = [];

  on(eventType: string, handler: (event: unknown) => void): void {
    const handlers = this.eventHandlers.get(eventType) || [];
    handlers.push(handler);
    this.eventHandlers.set(eventType, handlers);
  }

  off(eventType: string, handler: (event: unknown) => void): void {
    const handlers = this.eventHandlers.get(eventType) || [];
    this.eventHandlers.set(eventType, handlers.filter(h => h !== handler));
  }

  async emit(eventType: string, event: unknown): Promise<void> {
    const handlers = this.eventHandlers.get(eventType) || [];
    this.processedEvents.push({ type: eventType, event, timestamp: new Date() });

    for (const handler of handlers) {
      await handler(event);
    }
  }

  getProcessedEvents(): unknown[] {
    return [...this.processedEvents];
  }

  clearProcessedEvents(): void {
    this.processedEvents = [];
  }

  reset(): void {
    this.eventHandlers.clear();
    this.processedEvents = [];
  }
}

/**
 * Mock Slack Bolt App
 */
export class MockSlackBoltApp {
  client: MockSlackWebClient;
  private eventsHandler: MockSlackEventsHandler;
  private messageHandlers: Array<{ pattern: RegExp | string; handler: Function }> = [];
  private actionHandlers: Map<string, Function> = new Map();
  private commandHandlers: Map<string, Function> = new Map();

  constructor() {
    this.client = new MockSlackWebClient();
    this.eventsHandler = new MockSlackEventsHandler();
  }

  message(pattern: RegExp | string, handler: Function): void {
    this.messageHandlers.push({ pattern, handler });
  }

  action(actionId: string | RegExp, handler: Function): void {
    this.actionHandlers.set(actionId.toString(), handler);
  }

  command(command: string, handler: Function): void {
    this.commandHandlers.set(command, handler);
  }

  event(eventType: string, handler: Function): void {
    this.eventsHandler.on(eventType, handler as (event: unknown) => void);
  }

  async processMessage(message: { text: string; channel: string; user: string; ts: string; thread_ts?: string }): Promise<void> {
    for (const { pattern, handler } of this.messageHandlers) {
      const matches = typeof pattern === 'string'
        ? message.text.includes(pattern)
        : pattern.test(message.text);

      if (matches) {
        const context = {
          say: vi.fn(this.client.chat.postMessage),
          client: this.client,
          message,
          event: message
        };
        await handler(context);
      }
    }
  }

  async processAction(actionId: string, payload: unknown): Promise<void> {
    const handler = this.actionHandlers.get(actionId);
    if (handler) {
      const context = {
        ack: vi.fn(async () => {}),
        respond: vi.fn(async () => {}),
        client: this.client,
        body: payload,
        action: { action_id: actionId }
      };
      await handler(context);
    }
  }

  async processCommand(command: string, payload: unknown): Promise<void> {
    const handler = this.commandHandlers.get(command);
    if (handler) {
      const context = {
        ack: vi.fn(async () => {}),
        respond: vi.fn(async () => {}),
        client: this.client,
        command: payload
      };
      await handler(context);
    }
  }

  async start(port?: number): Promise<void> {
    // No-op for mock
  }

  async stop(): Promise<void> {
    // No-op for mock
  }

  reset(): void {
    this.client.reset();
    this.eventsHandler.reset();
    this.messageHandlers = [];
    this.actionHandlers.clear();
    this.commandHandlers.clear();
  }
}

// Factory functions
export function createMockSlackClient(): MockSlackWebClient {
  return new MockSlackWebClient();
}

export function createMockSlackApp(): MockSlackBoltApp {
  return new MockSlackBoltApp();
}

export default {
  MockSlackWebClient,
  MockSlackEventsHandler,
  MockSlackBoltApp,
  createMockSlackClient,
  createMockSlackApp
};
