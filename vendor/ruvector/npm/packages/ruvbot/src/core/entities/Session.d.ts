/**
 * Session entity - represents a conversation session
 */
import type { Session, SessionConfig, SessionContext, Message, Platform, Entity, MemoryEntry } from '../types.js';
export declare class SessionEntity implements Session {
    readonly id: string;
    readonly agentId: string;
    readonly userId?: string;
    readonly channelId?: string;
    readonly platform: Platform;
    messages: Message[];
    context: SessionContext;
    metadata: Record<string, unknown>;
    readonly createdAt: Date;
    updatedAt: Date;
    expiresAt?: Date;
    constructor(config: SessionConfig);
    /**
     * Create a new session
     */
    static create(agentId: string, options?: Partial<SessionConfig>): SessionEntity;
    /**
     * Add a message to the session
     */
    addMessage(message: Message): void;
    /**
     * Get recent messages
     */
    getRecentMessages(count: number): Message[];
    /**
     * Get messages by role
     */
    getMessagesByRole(role: Message['role']): Message[];
    /**
     * Update session context
     */
    updateContext(updates: Partial<SessionContext>): void;
    /**
     * Add entity to context
     */
    addEntity(entity: Entity): void;
    /**
     * Add topic to context
     */
    addTopic(topic: string): void;
    /**
     * Set relevant memories from search
     */
    setRelevantMemories(memories: MemoryEntry[]): void;
    /**
     * Check if session is expired
     */
    isExpired(): boolean;
    /**
     * Extend session expiration
     */
    extend(ttl: number): void;
    /**
     * Get session duration in milliseconds
     */
    getDuration(): number;
    /**
     * Get message count
     */
    getMessageCount(): number;
    /**
     * Get conversation turns (user + assistant pairs)
     */
    getTurnCount(): number;
    /**
     * Build conversation history for LLM context
     */
    buildConversationHistory(maxMessages?: number): Array<{
        role: string;
        content: string;
    }>;
    /**
     * Serialize session to JSON
     */
    toJSON(): Record<string, unknown>;
    /**
     * Create session from JSON
     */
    static fromJSON(data: Record<string, unknown>): SessionEntity;
}
//# sourceMappingURL=Session.d.ts.map