"use strict";
/**
 * Slack API Mock Module
 *
 * Mock implementations for Slack Web API and Events API
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.MockSlackBoltApp = exports.MockSlackEventsHandler = exports.MockSlackWebClient = void 0;
exports.createMockSlackClient = createMockSlackClient;
exports.createMockSlackApp = createMockSlackApp;
const vitest_1 = require("vitest");
/**
 * Mock Slack Web Client
 */
class MockSlackWebClient {
    constructor() {
        this.messageLog = [];
        this._reactionsData = new Map();
        this._filesData = new Map();
        // User and channel data
        this._usersData = new Map();
        this._channelsData = new Map();
        // Chat API
        this.chat = {
            postMessage: vitest_1.vi.fn(async (args) => {
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
            update: vitest_1.vi.fn(async (args) => {
                return {
                    ok: true,
                    ts: args.ts,
                    channel: args.channel
                };
            }),
            delete: vitest_1.vi.fn(async (args) => {
                return {
                    ok: true,
                    ts: args.ts,
                    channel: args.channel
                };
            }),
            postEphemeral: vitest_1.vi.fn(async (args) => {
                this.messageLog.push(args);
                return {
                    ok: true,
                    message_ts: `${Date.now()}.${Math.random().toString().slice(2, 8)}`
                };
            })
        };
        // Conversations API
        this.conversations = {
            info: vitest_1.vi.fn(async (args) => {
                const channel = this._channelsData.get(args.channel);
                return {
                    ok: !!channel,
                    channel
                };
            }),
            members: vitest_1.vi.fn(async (args) => {
                return {
                    ok: true,
                    members: ['U12345678', 'U87654321', 'U_BOT']
                };
            }),
            history: vitest_1.vi.fn(async (args) => {
                return {
                    ok: true,
                    messages: this.messageLog
                        .filter(m => m.channel === args.channel)
                        .slice(0, args.limit || 100)
                };
            }),
            replies: vitest_1.vi.fn(async (args) => {
                return {
                    ok: true,
                    messages: this.messageLog
                        .filter(m => m.channel === args.channel && m.thread_ts === args.ts)
                };
            }),
            join: vitest_1.vi.fn(async (args) => {
                return { ok: true, channel: args.channel };
            }),
            leave: vitest_1.vi.fn(async (args) => {
                return { ok: true };
            })
        };
        // Users API
        this.users = {
            info: vitest_1.vi.fn(async (args) => {
                const user = this._usersData.get(args.user);
                return {
                    ok: !!user,
                    user
                };
            }),
            list: vitest_1.vi.fn(async () => {
                return {
                    ok: true,
                    members: Array.from(this._usersData.values())
                };
            })
        };
        // Reactions API
        this.reactions = {
            add: vitest_1.vi.fn(async (args) => {
                const key = `${args.channel}:${args.timestamp}`;
                const existing = this._reactionsData.get(key) || [];
                this._reactionsData.set(key, [...existing, args.name]);
                return { ok: true };
            }),
            remove: vitest_1.vi.fn(async (args) => {
                const key = `${args.channel}:${args.timestamp}`;
                const existing = this._reactionsData.get(key) || [];
                this._reactionsData.set(key, existing.filter(r => r !== args.name));
                return { ok: true };
            }),
            get: vitest_1.vi.fn(async (args) => {
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
        this.files = {
            upload: vitest_1.vi.fn(async (args) => {
                const fileId = `F${Date.now()}`;
                const file = { id: fileId, name: args.filename, content: args.content };
                this._filesData.set(fileId, file);
                return { ok: true, file };
            }),
            delete: vitest_1.vi.fn(async (args) => {
                this._filesData.delete(args.file);
                return { ok: true };
            })
        };
        // Auth API
        this.auth = {
            test: vitest_1.vi.fn(async () => {
                return {
                    ok: true,
                    user_id: 'U_BOT',
                    team_id: 'T12345678',
                    bot_id: 'B12345678'
                };
            })
        };
        // Seed default test data
        this.seedDefaultData();
    }
    // Test helpers
    getMessageLog() {
        return [...this.messageLog];
    }
    clearMessageLog() {
        this.messageLog = [];
    }
    getReactions(channel, timestamp) {
        return this._reactionsData.get(`${channel}:${timestamp}`) || [];
    }
    addUser(user) {
        this._usersData.set(user.id, user);
    }
    addChannel(channel) {
        this._channelsData.set(channel.id, channel);
    }
    reset() {
        this.messageLog = [];
        this._reactionsData.clear();
        this._filesData.clear();
        this.seedDefaultData();
        // Reset all mocks
        vitest_1.vi.clearAllMocks();
    }
    seedDefaultData() {
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
exports.MockSlackWebClient = MockSlackWebClient;
/**
 * Mock Slack Events Handler
 */
class MockSlackEventsHandler {
    constructor() {
        this.eventHandlers = new Map();
        this.processedEvents = [];
    }
    on(eventType, handler) {
        const handlers = this.eventHandlers.get(eventType) || [];
        handlers.push(handler);
        this.eventHandlers.set(eventType, handlers);
    }
    off(eventType, handler) {
        const handlers = this.eventHandlers.get(eventType) || [];
        this.eventHandlers.set(eventType, handlers.filter(h => h !== handler));
    }
    async emit(eventType, event) {
        const handlers = this.eventHandlers.get(eventType) || [];
        this.processedEvents.push({ type: eventType, event, timestamp: new Date() });
        for (const handler of handlers) {
            await handler(event);
        }
    }
    getProcessedEvents() {
        return [...this.processedEvents];
    }
    clearProcessedEvents() {
        this.processedEvents = [];
    }
    reset() {
        this.eventHandlers.clear();
        this.processedEvents = [];
    }
}
exports.MockSlackEventsHandler = MockSlackEventsHandler;
/**
 * Mock Slack Bolt App
 */
class MockSlackBoltApp {
    constructor() {
        this.messageHandlers = [];
        this.actionHandlers = new Map();
        this.commandHandlers = new Map();
        this.client = new MockSlackWebClient();
        this.eventsHandler = new MockSlackEventsHandler();
    }
    message(pattern, handler) {
        this.messageHandlers.push({ pattern, handler });
    }
    action(actionId, handler) {
        this.actionHandlers.set(actionId.toString(), handler);
    }
    command(command, handler) {
        this.commandHandlers.set(command, handler);
    }
    event(eventType, handler) {
        this.eventsHandler.on(eventType, handler);
    }
    async processMessage(message) {
        for (const { pattern, handler } of this.messageHandlers) {
            const matches = typeof pattern === 'string'
                ? message.text.includes(pattern)
                : pattern.test(message.text);
            if (matches) {
                const context = {
                    say: vitest_1.vi.fn(this.client.chat.postMessage),
                    client: this.client,
                    message,
                    event: message
                };
                await handler(context);
            }
        }
    }
    async processAction(actionId, payload) {
        const handler = this.actionHandlers.get(actionId);
        if (handler) {
            const context = {
                ack: vitest_1.vi.fn(async () => { }),
                respond: vitest_1.vi.fn(async () => { }),
                client: this.client,
                body: payload,
                action: { action_id: actionId }
            };
            await handler(context);
        }
    }
    async processCommand(command, payload) {
        const handler = this.commandHandlers.get(command);
        if (handler) {
            const context = {
                ack: vitest_1.vi.fn(async () => { }),
                respond: vitest_1.vi.fn(async () => { }),
                client: this.client,
                command: payload
            };
            await handler(context);
        }
    }
    async start(port) {
        // No-op for mock
    }
    async stop() {
        // No-op for mock
    }
    reset() {
        this.client.reset();
        this.eventsHandler.reset();
        this.messageHandlers = [];
        this.actionHandlers.clear();
        this.commandHandlers.clear();
    }
}
exports.MockSlackBoltApp = MockSlackBoltApp;
// Factory functions
function createMockSlackClient() {
    return new MockSlackWebClient();
}
function createMockSlackApp() {
    return new MockSlackBoltApp();
}
exports.default = {
    MockSlackWebClient,
    MockSlackEventsHandler,
    MockSlackBoltApp,
    createMockSlackClient,
    createMockSlackApp
};
//# sourceMappingURL=slack.mock.js.map