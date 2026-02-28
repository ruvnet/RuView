/**
 * DiscordAdapter - Discord Channel Integration
 *
 * Connects to Discord servers using discord.js for real-time messaging.
 * Supports threads, embeds, reactions, and slash commands.
 */
import { BaseAdapter, type AdapterConfig, type UnifiedMessage, type SendOptions } from './BaseAdapter.js';
export interface DiscordCredentials {
    token: string;
    clientId?: string;
    guildId?: string;
    intents?: number[];
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
export declare class DiscordAdapter extends BaseAdapter {
    private client;
    constructor(config: Omit<AdapterConfig, 'type'> & {
        credentials: DiscordCredentials;
    });
    /**
     * Connect to Discord
     */
    connect(): Promise<void>;
    /**
     * Disconnect from Discord
     */
    disconnect(): Promise<void>;
    /**
     * Send a message to a Discord channel
     */
    send(channelId: string, content: string, options?: SendOptions): Promise<string>;
    /**
     * Reply to a Discord message
     */
    reply(message: UnifiedMessage, content: string, options?: SendOptions): Promise<string>;
    private loadDiscordJs;
    private getChannel;
    private discordToUnified;
    private getMimeCategory;
}
export declare function createDiscordAdapter(config: Omit<AdapterConfig, 'type'> & {
    credentials: DiscordCredentials;
}): DiscordAdapter;
export default DiscordAdapter;
//# sourceMappingURL=DiscordAdapter.d.ts.map