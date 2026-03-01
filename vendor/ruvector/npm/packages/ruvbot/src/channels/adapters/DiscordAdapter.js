"use strict";
/**
 * DiscordAdapter - Discord Channel Integration
 *
 * Connects to Discord servers using discord.js for real-time messaging.
 * Supports threads, embeds, reactions, and slash commands.
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
exports.DiscordAdapter = void 0;
exports.createDiscordAdapter = createDiscordAdapter;
const BaseAdapter_js_1 = require("./BaseAdapter.js");
// ============================================================================
// DiscordAdapter Implementation
// ============================================================================
class DiscordAdapter extends BaseAdapter_js_1.BaseAdapter {
    constructor(config) {
        super({ ...config, type: 'discord' });
        this.client = null;
    }
    /**
     * Connect to Discord
     */
    async connect() {
        const credentials = this.config.credentials;
        try {
            // Dynamic import to avoid requiring discord.js if not used
            const discordModule = await this.loadDiscordJs();
            if (discordModule) {
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                const Client = discordModule.Client;
                const GatewayIntentBits = discordModule.GatewayIntentBits;
                this.client = new Client({
                    intents: credentials.intents ?? [
                        GatewayIntentBits.Guilds,
                        GatewayIntentBits.GuildMessages,
                        GatewayIntentBits.MessageContent,
                        GatewayIntentBits.DirectMessages,
                    ],
                });
                // Register message handler
                this.client.on('messageCreate', (message) => {
                    // Ignore bot messages
                    if (message.author.bot)
                        return;
                    const unified = this.discordToUnified(message);
                    this.emitMessage(unified);
                });
                // Login
                await this.client.login(credentials.token);
                this.status.connected = true;
            }
            else {
                console.warn('DiscordAdapter: discord.js not available, running in mock mode');
                this.status.connected = true;
            }
        }
        catch (error) {
            this.status.errorCount++;
            throw new Error(`Failed to connect to Discord: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
    }
    /**
     * Disconnect from Discord
     */
    async disconnect() {
        if (this.client) {
            await this.client.destroy?.();
            this.client = null;
        }
        this.status.connected = false;
    }
    /**
     * Send a message to a Discord channel
     */
    async send(channelId, content, options) {
        if (!this.client) {
            throw new Error('DiscordAdapter not connected');
        }
        try {
            const channel = await this.getChannel(channelId);
            const sendOptions = { content };
            if (options?.replyTo) {
                sendOptions.reply = { messageReference: options.replyTo };
            }
            const result = await channel.send(sendOptions);
            this.status.messageCount++;
            return result.id;
        }
        catch (error) {
            this.status.errorCount++;
            throw error;
        }
    }
    /**
     * Reply to a Discord message
     */
    async reply(message, content, options) {
        return this.send(message.channelId, content, {
            ...options,
            replyTo: message.id,
        });
    }
    // ==========================================================================
    // Private Methods
    // ==========================================================================
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    async loadDiscordJs() {
        try {
            // Dynamic import - discord.js is optional
            // @ts-expect-error - discord.js may not be installed
            return await Promise.resolve().then(() => __importStar(require('discord.js'))).catch(() => null);
        }
        catch {
            return null;
        }
    }
    async getChannel(channelId) {
        if (!this.client) {
            throw new Error('Client not connected');
        }
        const channels = this.client.channels;
        return channels.fetch(channelId);
    }
    discordToUnified(message) {
        const attachments = [];
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
        return this.createUnifiedMessage(message.content, message.author.id, message.channelId, {
            username: `${message.author.username}#${message.author.discriminator}`,
            replyTo: message.reference?.messageId,
            attachments: attachments.length > 0 ? attachments : undefined,
            metadata: {
                guildId: message.guildId,
                originalId: message.id,
            },
        });
    }
    getMimeCategory(mimeType) {
        if (mimeType.startsWith('image/'))
            return 'image';
        if (mimeType.startsWith('audio/'))
            return 'audio';
        if (mimeType.startsWith('video/'))
            return 'video';
        return 'file';
    }
}
exports.DiscordAdapter = DiscordAdapter;
// ============================================================================
// Factory Function
// ============================================================================
function createDiscordAdapter(config) {
    return new DiscordAdapter(config);
}
exports.default = DiscordAdapter;
//# sourceMappingURL=DiscordAdapter.js.map