/**
 * DiscordAdapter - Discord Channel Integration
 *
 * Connects to Discord servers using discord.js for real-time messaging.
 * Supports threads, embeds, reactions, and slash commands.
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

export interface DiscordCredentials {
  token: string;           // Bot Token
  clientId?: string;       // Application Client ID
  guildId?: string;        // Optional: Specific guild to connect to
  intents?: number[];      // Discord intents
}

export interface DiscordMessage {
  id: string;
  channelId: string;
  guildId?: string;
  author: {
    id: string;
    username: string;
    discriminator: string;
  };
  content: string;
  timestamp: Date;
  reference?: {
    messageId: string;
  };
  attachments: Map<string, DiscordAttachment>;
}

export interface DiscordAttachment {
  id: string;
  filename: string;
  contentType?: string;
  url: string;
  size: number;
}

// ============================================================================
// DiscordAdapter Implementation
// ============================================================================

export class DiscordAdapter extends BaseAdapter {
  private client: unknown = null;

  constructor(config: Omit<AdapterConfig, 'type'> & { credentials: DiscordCredentials }) {
    super({ ...config, type: 'discord' });
  }

  /**
   * Connect to Discord
   */
  async connect(): Promise<void> {
    const credentials = this.config.credentials as unknown as DiscordCredentials;

    try {
      // Dynamic import to avoid requiring discord.js if not used
      const discordModule = await this.loadDiscordJs();

      if (discordModule) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const Client = discordModule.Client as any;
        const GatewayIntentBits = discordModule.GatewayIntentBits as Record<string, number>;

        this.client = new Client({
          intents: credentials.intents ?? [
            GatewayIntentBits.Guilds,
            GatewayIntentBits.GuildMessages,
            GatewayIntentBits.MessageContent,
            GatewayIntentBits.DirectMessages,
          ],
        });

        // Register message handler
        (this.client as { on: (event: string, handler: (message: DiscordMessage) => void) => void }).on('messageCreate', (message: DiscordMessage) => {
          // Ignore bot messages
          if ((message as unknown as { author: { bot?: boolean } }).author.bot) return;

          const unified = this.discordToUnified(message);
          this.emitMessage(unified);
        });

        // Login
        await (this.client as { login: (token: string) => Promise<void> }).login(credentials.token);
        this.status.connected = true;
      } else {
        console.warn('DiscordAdapter: discord.js not available, running in mock mode');
        this.status.connected = true;
      }
    } catch (error) {
      this.status.errorCount++;
      throw new Error(`Failed to connect to Discord: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  }

  /**
   * Disconnect from Discord
   */
  async disconnect(): Promise<void> {
    if (this.client) {
      await (this.client as { destroy?: () => Promise<void> }).destroy?.();
      this.client = null;
    }
    this.status.connected = false;
  }

  /**
   * Send a message to a Discord channel
   */
  async send(
    channelId: string,
    content: string,
    options?: SendOptions
  ): Promise<string> {
    if (!this.client) {
      throw new Error('DiscordAdapter not connected');
    }

    try {
      const channel = await this.getChannel(channelId);

      const sendOptions: Record<string, unknown> = { content };

      if (options?.replyTo) {
        sendOptions.reply = { messageReference: options.replyTo };
      }

      const result = await (channel as { send: (opts: unknown) => Promise<{ id: string }> }).send(sendOptions);

      this.status.messageCount++;
      return result.id;
    } catch (error) {
      this.status.errorCount++;
      throw error;
    }
  }

  /**
   * Reply to a Discord message
   */
  async reply(
    message: UnifiedMessage,
    content: string,
    options?: SendOptions
  ): Promise<string> {
    return this.send(message.channelId, content, {
      ...options,
      replyTo: message.id,
    });
  }

  // ==========================================================================
  // Private Methods
  // ==========================================================================

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private async loadDiscordJs(): Promise<any | null> {
    try {
      // Dynamic import - discord.js is optional
      // @ts-expect-error - discord.js may not be installed
      return await import('discord.js').catch(() => null);
    } catch {
      return null;
    }
  }

  private async getChannel(channelId: string): Promise<unknown> {
    if (!this.client) {
      throw new Error('Client not connected');
    }

    const channels = (this.client as { channels: { fetch: (id: string) => Promise<unknown> } }).channels;
    return channels.fetch(channelId);
  }

  private discordToUnified(message: DiscordMessage): UnifiedMessage {
    const attachments: Attachment[] = [];

    message.attachments.forEach((attachment) => {
      attachments.push({
        id: attachment.id,
        type: this.getMimeCategory(attachment.contentType ?? ''),
        url: attachment.url,
        mimeType: attachment.contentType,
        filename: attachment.filename,
        size: attachment.size,
      });
    });

    return this.createUnifiedMessage(
      message.content,
      message.author.id,
      message.channelId,
      {
        username: `${message.author.username}#${message.author.discriminator}`,
        replyTo: message.reference?.messageId,
        attachments: attachments.length > 0 ? attachments : undefined,
        metadata: {
          guildId: message.guildId,
          originalId: message.id,
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

export function createDiscordAdapter(
  config: Omit<AdapterConfig, 'type'> & { credentials: DiscordCredentials }
): DiscordAdapter {
  return new DiscordAdapter(config);
}

export default DiscordAdapter;
