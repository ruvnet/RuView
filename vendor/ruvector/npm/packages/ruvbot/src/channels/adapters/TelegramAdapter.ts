/**
 * TelegramAdapter - Telegram Channel Integration
 *
 * Connects to Telegram using telegraf for real-time messaging.
 * Supports inline keyboards, commands, and rich media.
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

export interface TelegramCredentials {
  token: string;           // Bot Token from @BotFather
  webhookUrl?: string;     // Optional webhook URL for production
  pollingTimeout?: number; // Long polling timeout
}

export interface TelegramMessage {
  message_id: number;
  chat: {
    id: number;
    type: string;
    title?: string;
    username?: string;
  };
  from: {
    id: number;
    username?: string;
    first_name: string;
    last_name?: string;
  };
  text?: string;
  date: number;
  reply_to_message?: TelegramMessage;
  photo?: TelegramPhoto[];
  document?: TelegramDocument;
  audio?: TelegramAudio;
  video?: TelegramVideo;
}

export interface TelegramPhoto {
  file_id: string;
  file_unique_id: string;
  width: number;
  height: number;
  file_size?: number;
}

export interface TelegramDocument {
  file_id: string;
  file_unique_id: string;
  file_name?: string;
  mime_type?: string;
  file_size?: number;
}

export interface TelegramAudio {
  file_id: string;
  file_unique_id: string;
  duration: number;
  performer?: string;
  title?: string;
  file_size?: number;
}

export interface TelegramVideo {
  file_id: string;
  file_unique_id: string;
  width: number;
  height: number;
  duration: number;
  file_size?: number;
}

// ============================================================================
// TelegramAdapter Implementation
// ============================================================================

export class TelegramAdapter extends BaseAdapter {
  private bot: unknown = null;

  constructor(config: Omit<AdapterConfig, 'type'> & { credentials: TelegramCredentials }) {
    super({ ...config, type: 'telegram' });
  }

  /**
   * Connect to Telegram
   */
  async connect(): Promise<void> {
    const credentials = this.config.credentials as unknown as TelegramCredentials;

    try {
      // Dynamic import to avoid requiring telegraf if not used
      const telegrafModule = await this.loadTelegraf();

      if (telegrafModule) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const Telegraf = telegrafModule.Telegraf as any;

        this.bot = new Telegraf(credentials.token);

        // Register message handler
        (this.bot as { on: (event: string, handler: (ctx: { message: TelegramMessage }) => void) => void }).on('message', (ctx: { message: TelegramMessage }) => {
          const unified = this.telegramToUnified(ctx.message);
          this.emitMessage(unified);
        });

        // Start polling or webhook
        if (credentials.webhookUrl) {
          await (this.bot as {
            telegram: {
              setWebhook: (url: string) => Promise<void>
            }
          }).telegram.setWebhook(credentials.webhookUrl);
        } else {
          (this.bot as { launch: () => void }).launch();
        }

        this.status.connected = true;
      } else {
        console.warn('TelegramAdapter: telegraf not available, running in mock mode');
        this.status.connected = true;
      }
    } catch (error) {
      this.status.errorCount++;
      throw new Error(`Failed to connect to Telegram: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  }

  /**
   * Disconnect from Telegram
   */
  async disconnect(): Promise<void> {
    if (this.bot) {
      (this.bot as { stop?: (signal?: string) => void }).stop?.('SIGTERM');
      this.bot = null;
    }
    this.status.connected = false;
  }

  /**
   * Send a message to a Telegram chat
   */
  async send(
    channelId: string,
    content: string,
    options?: SendOptions
  ): Promise<string> {
    if (!this.bot) {
      throw new Error('TelegramAdapter not connected');
    }

    try {
      const telegram = (this.bot as { telegram: unknown }).telegram as {
        sendMessage: (chatId: string | number, text: string, extra?: unknown) => Promise<{ message_id: number }>;
      };

      const extra: Record<string, unknown> = {};

      if (options?.replyTo) {
        extra.reply_to_message_id = parseInt(options.replyTo, 10);
      }

      const result = await telegram.sendMessage(
        channelId,
        content,
        Object.keys(extra).length > 0 ? extra : undefined
      );

      this.status.messageCount++;
      return result.message_id.toString();
    } catch (error) {
      this.status.errorCount++;
      throw error;
    }
  }

  /**
   * Reply to a Telegram message
   */
  async reply(
    message: UnifiedMessage,
    content: string,
    options?: SendOptions
  ): Promise<string> {
    return this.send(message.channelId, content, {
      ...options,
      replyTo: message.metadata.messageId as string,
    });
  }

  // ==========================================================================
  // Private Methods
  // ==========================================================================

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private async loadTelegraf(): Promise<any | null> {
    try {
      // Dynamic import - telegraf is optional
      // @ts-expect-error - telegraf may not be installed
      return await import('telegraf').catch(() => null);
    } catch {
      return null;
    }
  }

  private telegramToUnified(message: TelegramMessage): UnifiedMessage {
    const attachments: Attachment[] = [];

    // Handle photos (get largest)
    if (message.photo && message.photo.length > 0) {
      const photo = message.photo[message.photo.length - 1];
      attachments.push({
        id: photo.file_id,
        type: 'image',
        size: photo.file_size,
      });
    }

    // Handle document
    if (message.document) {
      attachments.push({
        id: message.document.file_id,
        type: 'file',
        filename: message.document.file_name,
        mimeType: message.document.mime_type,
        size: message.document.file_size,
      });
    }

    // Handle audio
    if (message.audio) {
      attachments.push({
        id: message.audio.file_id,
        type: 'audio',
        size: message.audio.file_size,
      });
    }

    // Handle video
    if (message.video) {
      attachments.push({
        id: message.video.file_id,
        type: 'video',
        size: message.video.file_size,
      });
    }

    const username = message.from.username ??
      `${message.from.first_name}${message.from.last_name ? ' ' + message.from.last_name : ''}`;

    return this.createUnifiedMessage(
      message.text ?? '[media]',
      message.from.id.toString(),
      message.chat.id.toString(),
      {
        username,
        replyTo: message.reply_to_message?.message_id.toString(),
        timestamp: new Date(message.date * 1000),
        attachments: attachments.length > 0 ? attachments : undefined,
        metadata: {
          messageId: message.message_id.toString(),
          chatType: message.chat.type,
          chatTitle: message.chat.title,
        },
      }
    );
  }
}

// ============================================================================
// Factory Function
// ============================================================================

export function createTelegramAdapter(
  config: Omit<AdapterConfig, 'type'> & { credentials: TelegramCredentials }
): TelegramAdapter {
  return new TelegramAdapter(config);
}

export default TelegramAdapter;
