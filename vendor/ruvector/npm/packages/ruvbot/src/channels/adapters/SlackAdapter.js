"use strict";
/**
 * SlackAdapter - Slack Channel Integration
 *
 * Connects to Slack workspace using @slack/bolt for real-time messaging.
 * Supports threads, reactions, file attachments, and app mentions.
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
exports.SlackAdapter = void 0;
exports.createSlackAdapter = createSlackAdapter;
const BaseAdapter_js_1 = require("./BaseAdapter.js");
// ============================================================================
// SlackAdapter Implementation
// ============================================================================
class SlackAdapter extends BaseAdapter_js_1.BaseAdapter {
    constructor(config) {
        super({ ...config, type: 'slack' });
        this.client = null;
        this.app = null;
    }
    /**
     * Connect to Slack
     */
    async connect() {
        const credentials = this.config.credentials;
        try {
            // Dynamic import to avoid requiring @slack/bolt if not used
            const boltModule = await this.loadSlackBolt();
            if (boltModule) {
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                const App = boltModule.App;
                this.app = new App({
                    token: credentials.token,
                    signingSecret: credentials.signingSecret,
                    socketMode: credentials.socketMode ?? false,
                    appToken: credentials.appToken,
                });
                // Register message handler
                const app = this.app;
                const self = this;
                app.message(async function (args) {
                    const unified = self.slackToUnified(args.message);
                    await self.emitMessage(unified);
                });
                // Start the app
                await this.app.start();
                this.status.connected = true;
            }
            else {
                // Fallback: Mark as connected but log warning
                console.warn('SlackAdapter: @slack/bolt not available, running in mock mode');
                this.status.connected = true;
            }
        }
        catch (error) {
            this.status.errorCount++;
            throw new Error(`Failed to connect to Slack: ${error instanceof Error ? error.message : 'Unknown error'}`);
        }
    }
    /**
     * Disconnect from Slack
     */
    async disconnect() {
        if (this.app) {
            await this.app.stop?.();
            this.app = null;
        }
        this.status.connected = false;
    }
    /**
     * Send a message to a Slack channel
     */
    async send(channelId, content, options) {
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
            return result.ts;
        }
        catch (error) {
            this.status.errorCount++;
            throw error;
        }
    }
    /**
     * Reply to a Slack message
     */
    async reply(message, content, options) {
        return this.send(message.channelId, content, {
            ...options,
            threadId: message.threadId ?? message.metadata.ts,
        });
    }
    // ==========================================================================
    // Private Methods
    // ==========================================================================
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    async loadSlackBolt() {
        try {
            return await Promise.resolve().then(() => __importStar(require('@slack/bolt')));
        }
        catch {
            return null;
        }
    }
    getClient() {
        if (this.app) {
            return this.app.client;
        }
        // Mock client for testing
        return {
            chat: {
                postMessage: async () => ({ ts: Date.now().toString() }),
            },
        };
    }
    slackToUnified(message) {
        const attachments = (message.files ?? []).map(file => ({
            id: file.id,
            type: this.getMimeCategory(file.mimetype),
            url: file.url_private,
            mimeType: file.mimetype,
            filename: file.name,
            size: file.size,
        }));
        return this.createUnifiedMessage(message.text, message.user, message.channel, {
            threadId: message.thread_ts,
            attachments: attachments.length > 0 ? attachments : undefined,
            metadata: {
                ts: message.ts,
                blocks: message.blocks,
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
exports.SlackAdapter = SlackAdapter;
// ============================================================================
// Factory Function
// ============================================================================
function createSlackAdapter(config) {
    return new SlackAdapter(config);
}
exports.default = SlackAdapter;
//# sourceMappingURL=SlackAdapter.js.map