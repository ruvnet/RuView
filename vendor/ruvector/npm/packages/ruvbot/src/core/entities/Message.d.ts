/**
 * Message entity - represents a single message in a conversation
 */
import type { Message, MessageRole, Attachment, MessageMetadata } from '../types.js';
export interface CreateMessageOptions {
    sessionId: string;
    role: MessageRole;
    content: string;
    attachments?: Attachment[];
    metadata?: MessageMetadata;
}
export declare class MessageEntity implements Message {
    readonly id: string;
    readonly sessionId: string;
    readonly role: MessageRole;
    content: string;
    attachments?: Attachment[];
    embedding?: Float32Array;
    metadata?: MessageMetadata;
    readonly createdAt: Date;
    constructor(options: CreateMessageOptions & {
        id?: string;
    });
    /**
     * Create a user message
     */
    static user(sessionId: string, content: string, attachments?: Attachment[]): MessageEntity;
    /**
     * Create an assistant message
     */
    static assistant(sessionId: string, content: string, metadata?: MessageMetadata): MessageEntity;
    /**
     * Create a system message
     */
    static system(sessionId: string, content: string): MessageEntity;
    /**
     * Create a function result message
     */
    static functionResult(sessionId: string, name: string, result: unknown): MessageEntity;
    /**
     * Set embedding vector
     */
    setEmbedding(embedding: Float32Array): void;
    /**
     * Update metadata
     */
    updateMetadata(updates: Partial<MessageMetadata>): void;
    /**
     * Get token count if available
     */
    getTokenCount(): number | undefined;
    /**
     * Get latency if available
     */
    getLatency(): number | undefined;
    /**
     * Check if message has attachments
     */
    hasAttachments(): boolean;
    /**
     * Get content length
     */
    getContentLength(): number;
    /**
     * Check if message is from user
     */
    isFromUser(): boolean;
    /**
     * Check if message is from assistant
     */
    isFromAssistant(): boolean;
    /**
     * Truncate content to max length
     */
    truncate(maxLength: number): string;
    /**
     * Serialize message to JSON
     */
    toJSON(): Record<string, unknown>;
    /**
     * Create message from JSON
     */
    static fromJSON(data: Record<string, unknown>): MessageEntity;
}
//# sourceMappingURL=Message.d.ts.map