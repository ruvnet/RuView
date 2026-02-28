/**
 * Integrations module exports
 *
 * Provides Slack, Discord, and webhook integrations.
 */

// Placeholder exports - to be implemented
export const INTEGRATIONS_MODULE_VERSION = '0.1.0';

// Chat adapter interface
export interface ChatAdapter {
  name: string;
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  sendMessage(channelId: string, content: string): Promise<void>;
  onMessage(handler: (message: IncomingMessage) => void): void;
}

export interface IncomingMessage {
  id: string;
  channelId: string;
  userId: string;
  content: string;
  timestamp: Date;
  platform: string;
  metadata?: Record<string, unknown>;
}

// Slack adapter options
export interface SlackAdapterOptions {
  botToken: string;
  signingSecret: string;
  appToken?: string;
  socketMode?: boolean;
}

// Discord adapter options
export interface DiscordAdapterOptions {
  token: string;
  clientId?: string;
  guildId?: string;
}

// Webhook options
export interface WebhookOptions {
  secret?: string;
  endpoints?: string[];
}
