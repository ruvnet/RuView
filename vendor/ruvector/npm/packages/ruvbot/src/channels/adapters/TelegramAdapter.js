"use strict";
/**
 * TelegramAdapter - Telegram Channel Integration
 *
 * Connects to Telegram using telegraf for real-time messaging.
 * Supports inline keyboards, commands, and rich media.
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.TelegramAdapter = void 0;
exports.createTelegramAdapter = createTelegramAdapter;
const BaseAdapter_js_1 = require("./BaseAdapter.js");
// ============================================================================
// TelegramAdapter Implementation
// ============================================================================
class TelegramAdapter extends BaseAdapter_js_1.BaseAdapter {
    constructor(config) {
        super({ ...config, type: 'telegram' });
        this.bot = null;
    }
    /**
     * Connect to Telegram
     */
    async connect() {
        const credentials = this.config.credentials;
        try {
            // Dynamic import to avoid requiring telegraf if not used
            const telegrafModule = await this.loadTelegraf();
            if (telegrafModule) {
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                const Telegraf = telegrafModule.Telegraf;
                this.bot = new Telegraf(credentials.token);
                // Register message handler
                this.bot.on('message', (ctx) => {
                    const unified = this.telegramToUnified(ctx.message);
                    this.emitMessage(unified);
                });
                // Start polling or webhook
                if (credentials.webhookUrl) {
                    await this.bot.telegram.setWebhook(credentials.webhookUrl);
                }
                else {
                    this.bot.launch();
                }
                this.status.connected = true;
            }
            else {
                console.warn('TelegramAdapter: telegraf not available, running in mock mode');
                this.status.connected = true;
            }
        }
        catch (error) {
            this.status.errorCount++;
            throw new Error(`Failed to connect to Telegram: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
    }
    /**
     * Disconnect from Telegram
     */
    async disconnect() {
        if (this.bot) {
            this.bot.stop?.('SIGTERM');
            this.bot = null;
        }
        this.status.connected = false;
    }
    /**
     * Send a message to a Telegram chat
     */
    async send(channelId, content, options) {
        if (!this.bot) {
            throw new Error('TelegramAdapter not connected');
        }
        try {
            const telegram = this.bot.telegram;
            const extra = {};
            if (options?.replyTo) {
                extra.reply_to_message_id = parseInt(options.replyTo, 10);
            }
            const result = await telegram.sendMessage(channelId, content, Object.keys(extra).length > 0 ? extra : undefined);
            this.status.messageCount++;
            return result.message_id.toString();
        }
        catch (error) {
            this.status.errorCount++;
            throw error;
        }
    }
    /**
     * Reply to a Telegram message
     */
    async reply(message, content, options) {
        return this.send(message.channelId, content, {
            ...options,
            replyTo: message.metadata.messageId,
        });
    }
    // ==========================================================================
    // Private Methods
    // ==========================================================================
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    async loadTelegraf() {
        try {
            // Dynamic import - telegraf is optional
            // @ts-expect-error - telegraf may not be installed
            return await Promise.resolve().then(() => __importStar(require('telegraf'))).catch(() => null);
        }
        catch {
            return null;
        }
    }
    telegramToUnified(message) {
        const attachments = [];
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
        return this.createUnifiedMessage(message.text ?? '[media]', message.from.id.toString(), message.chat.id.toString(), {
            username,
            replyTo: message.reply_to_message?.message_id.toString(),
            timestamp: new Date(message.date * 1000),
            attachments: attachments.length > 0 ? attachments : undefined,
            metadata: {
                messageId: message.message_id.toString(),
                chatType: message.chat.type,
                chatTitle: message.chat.title,
            },
        });
    }
}
exports.TelegramAdapter = TelegramAdapter;
// ============================================================================
// Factory Function
// ============================================================================
function createTelegramAdapter(config) {
    return new TelegramAdapter(config);
}
exports.default = TelegramAdapter;
//# sourceMappingURL=TelegramAdapter.js.map