/**
 * BaseAdapter - Abstract Channel Adapter
 *
 * Base class for all channel adapters providing a unified interface
 * for multi-channel messaging support.
 */
import type { EventEmitter } from 'events';
export type ChannelType = 'slack' | 'discord' | 'telegram' | 'signal' | 'whatsapp' | 'line' | 'imessage' | 'web' | 'api' | 'cli';
export interface Attachment {
    id: string;
    type: 'image' | 'file' | 'audio' | 'video' | 'link';
    url?: string;
    data?: Buffer;
    mimeType?: string;
    filename?: string;
    size?: number;
}
export interface UnifiedMessage {
    id: string;
    channelId: string;
    channelType: ChannelType;
    tenantId: string;
    userId: string;
    username?: string;
    content: string;
    attachments?: Attachment[];
    threadId?: string;
    replyTo?: string;
    timestamp: Date;
    metadata: Record<string, unknown>;
}
export interface SendOptions {
    threadId?: string;
    replyTo?: string;
    attachments?: Attachment[];
    metadata?: Record<string, unknown>;
}
export interface ChannelCredentials {
    token?: string;
    apiKey?: string;
    webhookUrl?: string;
    clientId?: string;
    clientSecret?: string;
    botId?: string;
    [key: string]: unknown;
}
export interface AdapterConfig {
    type: ChannelType;
    tenantId: string;
    credentials: ChannelCredentials;
    enabled?: boolean;
    rateLimit?: {
        requests: number;
        windowMs: number;
    };
}
export interface AdapterStatus {
    connected: boolean;
    lastActivity?: Date;
    errorCount: number;
    messageCount: number;
}
export type MessageHandler = (message: UnifiedMessage) => Promise<void>;
export declare abstract class BaseAdapter {
    protected readonly config: AdapterConfig;
    protected status: AdapterStatus;
    protected messageHandlers: MessageHandler[];
    protected eventEmitter?: EventEmitter;
    constructor(config: AdapterConfig);
    /**
     * Get channel type
     */
    get type(): ChannelType;
    /**
     * Get tenant ID
     */
    get tenantId(): string;
    /**
     * Check if adapter is enabled
     */
    get enabled(): boolean;
    /**
     * Get adapter status
     */
    getStatus(): AdapterStatus;
    /**
     * Register a message handler
     */
    onMessage(handler: MessageHandler): void;
    /**
     * Remove a message handler
     */
    offMessage(handler: MessageHandler): void;
    /**
     * Emit a received message to all handlers
     */
    protected emitMessage(message: UnifiedMessage): Promise<void>;
    /**
     * Create a unified message from raw input
     */
    protected createUnifiedMessage(content: string, userId: string, channelId: string, extra?: Partial<UnifiedMessage>): UnifiedMessage;
    /**
     * Connect to the channel
     */
    abstract connect(): Promise<void>;
    /**
     * Disconnect from the channel
     */
    abstract disconnect(): Promise<void>;
    /**
     * Send a message to the channel
     */
    abstract send(channelId: string, content: string, options?: SendOptions): Promise<string>;
    /**
     * Reply to a message
     */
    abstract reply(message: UnifiedMessage, content: string, options?: SendOptions): Promise<string>;
}
export default BaseAdapter;
//# sourceMappingURL=BaseAdapter.d.ts.map