/**
 * BaseAdapter - Abstract Channel Adapter
 *
 * Base class for all channel adapters providing a unified interface
 * for multi-channel messaging support.
 */

import { v4 as uuidv4 } from 'uuid';
import type { EventEmitter } from 'events';

// ============================================================================
// Types
// ============================================================================

export type ChannelType =
  | 'slack'
  | 'discord'
  | 'telegram'
  | 'signal'
  | 'whatsapp'
  | 'line'
  | 'imessage'
  | 'web'
  | 'api'
  | 'cli';

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

// ============================================================================
// Message Handler Type
// ============================================================================

export type MessageHandler = (message: UnifiedMessage) => Promise<void>;

// ============================================================================
// BaseAdapter Abstract Class
// ============================================================================

export abstract class BaseAdapter {
  protected readonly config: AdapterConfig;
  protected status: AdapterStatus;
  protected messageHandlers: MessageHandler[] = [];
  protected eventEmitter?: EventEmitter;

  constructor(config: AdapterConfig) {
    this.config = {
      ...config,
      enabled: config.enabled ?? true,
    };
    this.status = {
      connected: false,
      errorCount: 0,
      messageCount: 0,
    };
  }

  /**
   * Get channel type
   */
  get type(): ChannelType {
    return this.config.type;
  }

  /**
   * Get tenant ID
   */
  get tenantId(): string {
    return this.config.tenantId;
  }

  /**
   * Check if adapter is enabled
   */
  get enabled(): boolean {
    return this.config.enabled ?? true;
  }

  /**
   * Get adapter status
   */
  getStatus(): AdapterStatus {
    return { ...this.status };
  }

  /**
   * Register a message handler
   */
  onMessage(handler: MessageHandler): void {
    this.messageHandlers.push(handler);
  }

  /**
   * Remove a message handler
   */
  offMessage(handler: MessageHandler): void {
    const index = this.messageHandlers.indexOf(handler);
    if (index > -1) {
      this.messageHandlers.splice(index, 1);
    }
  }

  /**
   * Emit a received message to all handlers
   */
  protected async emitMessage(message: UnifiedMessage): Promise<void> {
    this.status.messageCount++;
    this.status.lastActivity = new Date();

    for (const handler of this.messageHandlers) {
      try {
        await handler(message);
      } catch (error) {
        this.status.errorCount++;
        console.error(`Message handler error in ${this.type}:`, error);
      }
    }
  }

  /**
   * Create a unified message from raw input
   */
  protected createUnifiedMessage(
    content: string,
    userId: string,
    channelId: string,
    extra: Partial<UnifiedMessage> = {}
  ): UnifiedMessage {
    return {
      id: uuidv4(),
      channelId,
      channelType: this.config.type,
      tenantId: this.config.tenantId,
      userId,
      content,
      timestamp: new Date(),
      metadata: {},
      ...extra,
    };
  }

  // ==========================================================================
  // Abstract Methods (must be implemented by subclasses)
  // ==========================================================================

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
  abstract send(
    channelId: string,
    content: string,
    options?: SendOptions
  ): Promise<string>;

  /**
   * Reply to a message
   */
  abstract reply(
    message: UnifiedMessage,
    content: string,
    options?: SendOptions
  ): Promise<string>;
}

export default BaseAdapter;
