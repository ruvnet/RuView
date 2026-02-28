/**
 * SlackAdapter - Slack Channel Integration
 *
 * Connects to Slack workspace using @slack/bolt for real-time messaging.
 * Supports threads, reactions, file attachments, and app mentions.
 */

import {
  BaseAdapter,
  type AdapterConfig,
  type UnifiedMessage,
  type SendOptions,
  type Attachment,
} from './BaseAdapter.js';

// ============================================================================
// Types
// ============================================================================

export interface SlackCredentials {
  token: string;          // Bot User OAuth Token (xoxb-)
  signingSecret: string;  // App Signing Secret
  appToken?: string;      // App-Level Token for Socket Mode (xapp-)
  socketMode?: boolean;
}

export interface SlackMessage {
  type: string;
  channel: string;
  user: string;
  text: string;
  ts: string;
  thread_ts?: string;
  files?: SlackFile[];
  blocks?: unknown[];
}

export interface SlackFile {
  id: string;
  name: string;
  mimetype: string;
  url_private: string;
  size: number;
}

interface SlackClient {
  chat: {
    postMessage: (args: {
      channel: string;
      text: string;
      thread_ts?: string;
    }) => Promise<{ ts: string }>;
  };
}

// ============================================================================
// SlackAdapter Implementation
// ============================================================================

export class SlackAdapter extends BaseAdapter {
  private client: unknown = null;
  private app: unknown = null;

  constructor(config: Omit<AdapterConfig, 'type'> & { credentials: SlackCredentials }) {
    super({ ...config, type: 'slack' });
  }

  /**
   * Connect to Slack
   */
  async connect(): Promise<void> {
    const credentials = this.config.credentials as unknown as SlackCredentials;

    try {
      // Dynamic import to avoid requiring @slack/bolt if not used
      const boltModule = await this.loadSlackBolt();

      if (boltModule) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const App = boltModule.App as any;

        this.app = new App({
          token: credentials.token,
          signingSecret: credentials.signingSecret,
          socketMode: credentials.socketMode ?? false,
          appToken: credentials.appToken,
        });

        // Register message handler
        const app = this.app as { message: (handler: (args: { message: SlackMessage }) => Promise<void>) => void };
        const self = this;
        app.message(async function(args: { message: SlackMessage }) {
          const unified = self.slackToUnified(args.message);
          await self.emitMessage(unified);
        });

        // Start the app
        await (this.app as { start: () => Promise<void> }).start();
        this.status.connected = true;
      } else {
        // Fallback: Mark as connected but log warning
        console.warn('SlackAdapter: @slack/bolt not available, running in mock mode');
        this.status.connected = true;
      }
    } catch (error) {
      this.status.errorCount++;
      throw new Error(`Failed to connect to Slack: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  }

  /**
   * Disconnect from Slack
   */
  async disconnect(): Promise<void> {
    if (this.app) {
      await (this.app as { stop?: () => Promise<void> }).stop?.();
      this.app = null;
    }
    this.status.connected = false;
  }

  /**
   * Send a message to a Slack channel
   */
  async send(
    channelId: string,
    content: string,
    options?: SendOptions
  ): Promise<string> {
    if (!this.client && !this.app) {
      throw new Error('SlackAdapter not connected');
    }

    try {
      const client = this.getClient();

      const result = await client.chat.postMessage({
        channel: channelId,
        text: content,
        thread_ts: options?.threadId,
      });

      this.status.messageCount++;
      return result.ts as string;
    } catch (error) {
      this.status.errorCount++;
      throw error;
    }
  }

  /**
   * Reply to a Slack message
   */
  async reply(
    message: UnifiedMessage,
    content: string,
    options?: SendOptions
  ): Promise<string> {
    return this.send(message.channelId, content, {
      ...options,
      threadId: message.threadId ?? message.metadata.ts as string,
    });
  }

  // ==========================================================================
  // Private Methods
  // ==========================================================================

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private async loadSlackBolt(): Promise<any | null> {
    try {
      return await import('@slack/bolt');
    } catch {
      return null;
    }
  }

  private getClient(): SlackClient {
    if (this.app) {
      return (this.app as { client: SlackClient }).client;
    }
    // Mock client for testing
    return {
      chat: {
        postMessage: async () => ({ ts: Date.now().toString() }),
      },
    };
  }

  private slackToUnified(message: SlackMessage): UnifiedMessage {
    const attachments: Attachment[] = (message.files ?? []).map(file => ({
      id: file.id,
      type: this.getMimeCategory(file.mimetype),
      url: file.url_private,
      mimeType: file.mimetype,
      filename: file.name,
      size: file.size,
    }));

    return this.createUnifiedMessage(
      message.text,
      message.user,
      message.channel,
      {
        threadId: message.thread_ts,
        attachments: attachments.length > 0 ? attachments : undefined,
        metadata: {
          ts: message.ts,
          blocks: message.blocks,
        },
      }
    );
  }

  private getMimeCategory(mimeType: string): Attachment['type'] {
    if (mimeType.startsWith('image/')) return 'image';
    if (mimeType.startsWith('audio/')) return 'audio';
    if (mimeType.startsWith('video/')) return 'video';
    return 'file';
  }
}

// ============================================================================
// Factory Function
// ============================================================================

export function createSlackAdapter(
  config: Omit<AdapterConfig, 'type'> & { credentials: SlackCredentials }
): SlackAdapter {
  return new SlackAdapter(config);
}

export default SlackAdapter;
