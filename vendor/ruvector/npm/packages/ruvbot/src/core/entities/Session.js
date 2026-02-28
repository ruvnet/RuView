"use strict";
/**
 * Session entity - represents a conversation session
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.SessionEntity = void 0;
const uuid_1 = require("uuid");
class SessionEntity {
    constructor(config) {
        this.id = config.id || (0, uuid_1.v4)();
        this.agentId = config.agentId;
        this.userId = config.userId;
        this.channelId = config.channelId;
        this.platform = config.platform ?? 'api';
        this.messages = [];
        this.context = {
            topics: [],
            entities: [],
        };
        this.metadata = config.metadata ?? {};
        this.createdAt = new Date();
        this.updatedAt = new Date();
        if (config.ttl) {
            this.expiresAt = new Date(Date.now() + config.ttl);
        }
    }
    /**
     * Create a new session
     */
    static create(agentId, options) {
        return new SessionEntity({
            agentId,
            ...options,
        });
    }
    /**
     * Add a message to the session
     */
    addMessage(message) {
        this.messages.push(message);
        this.updatedAt = new Date();
    }
    /**
     * Get recent messages
     */
    getRecentMessages(count) {
        return this.messages.slice(-count);
    }
    /**
     * Get messages by role
     */
    getMessagesByRole(role) {
        return this.messages.filter((m) => m.role === role);
    }
    /**
     * Update session context
     */
    updateContext(updates) {
        this.context = {
            ...this.context,
            ...updates,
        };
        this.updatedAt = new Date();
    }
    /**
     * Add entity to context
     */
    addEntity(entity) {
        // Avoid duplicates
        const exists = this.context.entities.some((e) => e.type === entity.type && e.value === entity.value);
        if (!exists) {
            this.context.entities.push(entity);
        }
        this.updatedAt = new Date();
    }
    /**
     * Add topic to context
     */
    addTopic(topic) {
        if (!this.context.topics.includes(topic)) {
            this.context.topics.push(topic);
        }
        this.updatedAt = new Date();
    }
    /**
     * Set relevant memories from search
     */
    setRelevantMemories(memories) {
        this.context.relevantMemories = memories;
        this.updatedAt = new Date();
    }
    /**
     * Check if session is expired
     */
    isExpired() {
        if (!this.expiresAt)
            return false;
        return Date.now() > this.expiresAt.getTime();
    }
    /**
     * Extend session expiration
     */
    extend(ttl) {
        this.expiresAt = new Date(Date.now() + ttl);
        this.updatedAt = new Date();
    }
    /**
     * Get session duration in milliseconds
     */
    getDuration() {
        return Date.now() - this.createdAt.getTime();
    }
    /**
     * Get message count
     */
    getMessageCount() {
        return this.messages.length;
    }
    /**
     * Get conversation turns (user + assistant pairs)
     */
    getTurnCount() {
        return Math.floor(this.messages.filter((m) => m.role === 'user').length);
    }
    /**
     * Build conversation history for LLM context
     */
    buildConversationHistory(maxMessages) {
        const messages = maxMessages ? this.getRecentMessages(maxMessages) : this.messages;
        return messages.map((m) => ({
            role: m.role,
            content: m.content,
        }));
    }
    /**
     * Serialize session to JSON
     */
    toJSON() {
        return {
            id: this.id,
            agentId: this.agentId,
            userId: this.userId,
            channelId: this.channelId,
            platform: this.platform,
            messages: this.messages,
            context: {
                summary: this.context.summary,
                topics: this.context.topics,
                entities: this.context.entities,
            },
            metadata: this.metadata,
            createdAt: this.createdAt.toISOString(),
            updatedAt: this.updatedAt.toISOString(),
            expiresAt: this.expiresAt?.toISOString(),
        };
    }
    /**
     * Create session from JSON
     */
    static fromJSON(data) {
        const session = new SessionEntity({
            id: data.id,
            agentId: data.agentId,
            userId: data.userId,
            channelId: data.channelId,
            platform: data.platform,
        });
        session.messages = data.messages;
        session.context = data.context;
        session.metadata = data.metadata;
        return session;
    }
}
exports.SessionEntity = SessionEntity;
//# sourceMappingURL=Session.js.map