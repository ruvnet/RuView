/**
 * Session entity - represents a conversation session
 */

import { v4 as uuid } from 'uuid';
import type {
  Session,
  SessionConfig,
  SessionContext,
  Message,
  Platform,
  Entity,
  MemoryEntry,
} from '../types.js';

export class SessionEntity implements Session {
  public readonly id: string;
  public readonly agentId: string;
  public readonly userId?: string;
  public readonly channelId?: string;
  public readonly platform: Platform;
  public messages: Message[];
  public context: SessionContext;
  public metadata: Record<string, unknown>;
  public readonly createdAt: Date;
  public updatedAt: Date;
  public expiresAt?: Date;

  constructor(config: SessionConfig) {
    this.id = config.id || uuid();
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
  static create(agentId: string, options?: Partial<SessionConfig>): SessionEntity {
    return new SessionEntity({
      agentId,
      ...options,
    });
  }

  /**
   * Add a message to the session
   */
  addMessage(message: Message): void {
    this.messages.push(message);
    this.updatedAt = new Date();
  }

  /**
   * Get recent messages
   */
  getRecentMessages(count: number): Message[] {
    return this.messages.slice(-count);
  }

  /**
   * Get messages by role
   */
  getMessagesByRole(role: Message['role']): Message[] {
    return this.messages.filter((m) => m.role === role);
  }

  /**
   * Update session context
   */
  updateContext(updates: Partial<SessionContext>): void {
    this.context = {
      ...this.context,
      ...updates,
    };
    this.updatedAt = new Date();
  }

  /**
   * Add entity to context
   */
  addEntity(entity: Entity): void {
    // Avoid duplicates
    const exists = this.context.entities.some(
      (e) => e.type === entity.type && e.value === entity.value
    );
    if (!exists) {
      this.context.entities.push(entity);
    }
    this.updatedAt = new Date();
  }

  /**
   * Add topic to context
   */
  addTopic(topic: string): void {
    if (!this.context.topics.includes(topic)) {
      this.context.topics.push(topic);
    }
    this.updatedAt = new Date();
  }

  /**
   * Set relevant memories from search
   */
  setRelevantMemories(memories: MemoryEntry[]): void {
    this.context.relevantMemories = memories;
    this.updatedAt = new Date();
  }

  /**
   * Check if session is expired
   */
  isExpired(): boolean {
    if (!this.expiresAt) return false;
    return Date.now() > this.expiresAt.getTime();
  }

  /**
   * Extend session expiration
   */
  extend(ttl: number): void {
    this.expiresAt = new Date(Date.now() + ttl);
    this.updatedAt = new Date();
  }

  /**
   * Get session duration in milliseconds
   */
  getDuration(): number {
    return Date.now() - this.createdAt.getTime();
  }

  /**
   * Get message count
   */
  getMessageCount(): number {
    return this.messages.length;
  }

  /**
   * Get conversation turns (user + assistant pairs)
   */
  getTurnCount(): number {
    return Math.floor(this.messages.filter((m) => m.role === 'user').length);
  }

  /**
   * Build conversation history for LLM context
   */
  buildConversationHistory(maxMessages?: number): Array<{ role: string; content: string }> {
    const messages = maxMessages ? this.getRecentMessages(maxMessages) : this.messages;
    return messages.map((m) => ({
      role: m.role,
      content: m.content,
    }));
  }

  /**
   * Serialize session to JSON
   */
  toJSON(): Record<string, unknown> {
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
  static fromJSON(data: Record<string, unknown>): SessionEntity {
    const session = new SessionEntity({
      id: data.id as string,
      agentId: data.agentId as string,
      userId: data.userId as string | undefined,
      channelId: data.channelId as string | undefined,
      platform: data.platform as Platform,
    });

    session.messages = data.messages as Message[];
    session.context = data.context as SessionContext;
    session.metadata = data.metadata as Record<string, unknown>;

    return session;
  }
}
