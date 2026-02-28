/**
 * Message entity - represents a single message in a conversation
 */

import { v4 as uuid } from 'uuid';
import type { Message, MessageRole, Attachment, MessageMetadata } from '../types.js';

export interface CreateMessageOptions {
  sessionId: string;
  role: MessageRole;
  content: string;
  attachments?: Attachment[];
  metadata?: MessageMetadata;
}

export class MessageEntity implements Message {
  public readonly id: string;
  public readonly sessionId: string;
  public readonly role: MessageRole;
  public content: string;
  public attachments?: Attachment[];
  public embedding?: Float32Array;
  public metadata?: MessageMetadata;
  public readonly createdAt: Date;

  constructor(options: CreateMessageOptions & { id?: string }) {
    this.id = options.id || uuid();
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
  static user(sessionId: string, content: string, attachments?: Attachment[]): MessageEntity {
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
  static assistant(
    sessionId: string,
    content: string,
    metadata?: MessageMetadata
  ): MessageEntity {
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
  static system(sessionId: string, content: string): MessageEntity {
    return new MessageEntity({
      sessionId,
      role: 'system',
      content,
    });
  }

  /**
   * Create a function result message
   */
  static functionResult(sessionId: string, name: string, result: unknown): MessageEntity {
    return new MessageEntity({
      sessionId,
      role: 'function',
      content: JSON.stringify({ name, result }),
    });
  }

  /**
   * Set embedding vector
   */
  setEmbedding(embedding: Float32Array): void {
    this.embedding = embedding;
  }

  /**
   * Update metadata
   */
  updateMetadata(updates: Partial<MessageMetadata>): void {
    this.metadata = {
      ...this.metadata,
      ...updates,
    };
  }

  /**
   * Get token count if available
   */
  getTokenCount(): number | undefined {
    return this.metadata?.tokens;
  }

  /**
   * Get latency if available
   */
  getLatency(): number | undefined {
    return this.metadata?.latency;
  }

  /**
   * Check if message has attachments
   */
  hasAttachments(): boolean {
    return (this.attachments?.length ?? 0) > 0;
  }

  /**
   * Get content length
   */
  getContentLength(): number {
    return this.content.length;
  }

  /**
   * Check if message is from user
   */
  isFromUser(): boolean {
    return this.role === 'user';
  }

  /**
   * Check if message is from assistant
   */
  isFromAssistant(): boolean {
    return this.role === 'assistant';
  }

  /**
   * Truncate content to max length
   */
  truncate(maxLength: number): string {
    if (this.content.length <= maxLength) {
      return this.content;
    }
    return this.content.slice(0, maxLength - 3) + '...';
  }

  /**
   * Serialize message to JSON
   */
  toJSON(): Record<string, unknown> {
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
  static fromJSON(data: Record<string, unknown>): MessageEntity {
    const message = new MessageEntity({
      id: data.id as string,
      sessionId: data.sessionId as string,
      role: data.role as MessageRole,
      content: data.content as string,
      attachments: data.attachments as Attachment[] | undefined,
      metadata: data.metadata as MessageMetadata | undefined,
    });
    return message;
  }
}
