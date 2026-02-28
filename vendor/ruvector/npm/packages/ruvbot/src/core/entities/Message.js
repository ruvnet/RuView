"use strict";
/**
 * Message entity - represents a single message in a conversation
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.MessageEntity = void 0;
const uuid_1 = require("uuid");
class MessageEntity {
    constructor(options) {
        this.id = options.id || (0, uuid_1.v4)();
        this.sessionId = options.sessionId;
        this.role = options.role;
        this.content = options.content;
        this.attachments = options.attachments;
        this.metadata = options.metadata;
        this.createdAt = new Date();
    }
    /**
     * Create a user message
     */
    static user(sessionId, content, attachments) {
        return new MessageEntity({
            sessionId,
            role: 'user',
            content,
            attachments,
        });
    }
    /**
     * Create an assistant message
     */
    static assistant(sessionId, content, metadata) {
        return new MessageEntity({
            sessionId,
            role: 'assistant',
            content,
            metadata,
        });
    }
    /**
     * Create a system message
     */
    static system(sessionId, content) {
        return new MessageEntity({
            sessionId,
            role: 'system',
            content,
        });
    }
    /**
     * Create a function result message
     */
    static functionResult(sessionId, name, result) {
        return new MessageEntity({
            sessionId,
            role: 'function',
            content: JSON.stringify({ name, result }),
        });
    }
    /**
     * Set embedding vector
     */
    setEmbedding(embedding) {
        this.embedding = embedding;
    }
    /**
     * Update metadata
     */
    updateMetadata(updates) {
        this.metadata = {
            ...this.metadata,
            ...updates,
        };
    }
    /**
     * Get token count if available
     */
    getTokenCount() {
        return this.metadata?.tokens;
    }
    /**
     * Get latency if available
     */
    getLatency() {
        return this.metadata?.latency;
    }
    /**
     * Check if message has attachments
     */
    hasAttachments() {
        return (this.attachments?.length ?? 0) > 0;
    }
    /**
     * Get content length
     */
    getContentLength() {
        return this.content.length;
    }
    /**
     * Check if message is from user
     */
    isFromUser() {
        return this.role === 'user';
    }
    /**
     * Check if message is from assistant
     */
    isFromAssistant() {
        return this.role === 'assistant';
    }
    /**
     * Truncate content to max length
     */
    truncate(maxLength) {
        if (this.content.length <= maxLength) {
            return this.content;
        }
        return this.content.slice(0, maxLength - 3) + '...';
    }
    /**
     * Serialize message to JSON
     */
    toJSON() {
        return {
            id: this.id,
            sessionId: this.sessionId,
            role: this.role,
            content: this.content,
            attachments: this.attachments,
            metadata: this.metadata,
            createdAt: this.createdAt.toISOString(),
        };
    }
    /**
     * Create message from JSON
     */
    static fromJSON(data) {
        const message = new MessageEntity({
            id: data.id,
            sessionId: data.sessionId,
            role: data.role,
            content: data.content,
            attachments: data.attachments,
            metadata: data.metadata,
        });
        return message;
    }
}
exports.MessageEntity = MessageEntity;
//# sourceMappingURL=Message.js.map