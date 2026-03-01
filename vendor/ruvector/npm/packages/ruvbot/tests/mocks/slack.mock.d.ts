/**
 * Slack API Mock Module
 *
 * Mock implementations for Slack Web API and Events API
 */
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
export declare class MockSlackWebClient {
    private messageLog;
    private _reactionsData;
    private _filesData;
    private _usersData;
    private _channelsData;
    constructor();
    chat: {
        postMessage: import("vitest").Mock<[args: SlackMessage], Promise<SlackResponse>>;
        update: import("vitest").Mock<[args: {
            channel: string;
            ts: string;
            text?: string;
            blocks?: unknown[];
        }], Promise<SlackResponse>>;
        delete: import("vitest").Mock<[args: {
            channel: string;
            ts: string;
        }], Promise<SlackResponse>>;
        postEphemeral: import("vitest").Mock<[args: SlackMessage & {
            user: string;
        }], Promise<SlackResponse>>;
    };
    conversations: {
        info: import("vitest").Mock<[args: {
            channel: string;
        }], Promise<{
            ok: boolean;
            channel?: SlackChannel;
        }>>;
        members: import("vitest").Mock<[args: {
            channel: string;
        }], Promise<{
            ok: boolean;
            members: string[];
        }>>;
        history: import("vitest").Mock<[args: {
            channel: string;
            limit?: number;
        }], Promise<{
            ok: boolean;
            messages: unknown[];
        }>>;
        replies: import("vitest").Mock<[args: {
            channel: string;
            ts: string;
        }], Promise<{
            ok: boolean;
            messages: unknown[];
        }>>;
        join: import("vitest").Mock<[args: {
            channel: string;
        }], Promise<SlackResponse>>;
        leave: import("vitest").Mock<[args: {
            channel: string;
        }], Promise<SlackResponse>>;
    };
    users: {
        info: import("vitest").Mock<[args: {
            user: string;
        }], Promise<{
            ok: boolean;
            user?: SlackUser;
        }>>;
        list: import("vitest").Mock<[], Promise<{
            ok: boolean;
            members: SlackUser[];
        }>>;
    };
    reactions: {
        add: import("vitest").Mock<[args: {
            channel: string;
            timestamp: string;
            name: string;
        }], Promise<SlackResponse>>;
        remove: import("vitest").Mock<[args: {
            channel: string;
            timestamp: string;
            name: string;
        }], Promise<SlackResponse>>;
        get: import("vitest").Mock<[args: {
            channel: string;
            timestamp: string;
        }], Promise<{
            ok: boolean;
            message: {
                reactions: unknown[];
            };
        }>>;
    };
    files: {
        upload: import("vitest").Mock<[args: {
            channels: string;
            content: string;
            filename: string;
        }], Promise<{
            ok: boolean;
            file: unknown;
        }>>;
        delete: import("vitest").Mock<[args: {
            file: string;
        }], Promise<SlackResponse>>;
    };
    auth: {
        test: import("vitest").Mock<[], Promise<{
            ok: boolean;
            user_id: string;
            team_id: string;
            bot_id: string;
        }>>;
    };
    getMessageLog(): SlackMessage[];
    clearMessageLog(): void;
    getReactions(channel: string, timestamp: string): string[];
    addUser(user: SlackUser): void;
    addChannel(channel: SlackChannel): void;
    reset(): void;
    private seedDefaultData;
}
/**
 * Mock Slack Events Handler
 */
export declare class MockSlackEventsHandler {
    private eventHandlers;
    private processedEvents;
    on(eventType: string, handler: (event: unknown) => void): void;
    off(eventType: string, handler: (event: unknown) => void): void;
    emit(eventType: string, event: unknown): Promise<void>;
    getProcessedEvents(): unknown[];
    clearProcessedEvents(): void;
    reset(): void;
}
/**
 * Mock Slack Bolt App
 */
export declare class MockSlackBoltApp {
    client: MockSlackWebClient;
    private eventsHandler;
    private messageHandlers;
    private actionHandlers;
    private commandHandlers;
    constructor();
    message(pattern: RegExp | string, handler: Function): void;
    action(actionId: string | RegExp, handler: Function): void;
    command(command: string, handler: Function): void;
    event(eventType: string, handler: Function): void;
    processMessage(message: {
        text: string;
        channel: string;
        user: string;
        ts: string;
        thread_ts?: string;
    }): Promise<void>;
    processAction(actionId: string, payload: unknown): Promise<void>;
    processCommand(command: string, payload: unknown): Promise<void>;
    start(port?: number): Promise<void>;
    stop(): Promise<void>;
    reset(): void;
}
export declare function createMockSlackClient(): MockSlackWebClient;
export declare function createMockSlackApp(): MockSlackBoltApp;
declare const _default: {
    MockSlackWebClient: typeof MockSlackWebClient;
    MockSlackEventsHandler: typeof MockSlackEventsHandler;
    MockSlackBoltApp: typeof MockSlackBoltApp;
    createMockSlackClient: typeof createMockSlackClient;
    createMockSlackApp: typeof createMockSlackApp;
};
export default _default;
//# sourceMappingURL=slack.mock.d.ts.map