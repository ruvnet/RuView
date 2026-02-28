/**
 * TelegramAdapter - Telegram Channel Integration
 *
 * Connects to Telegram using telegraf for real-time messaging.
 * Supports inline keyboards, commands, and rich media.
 */
import { BaseAdapter, type AdapterConfig, type UnifiedMessage, type SendOptions } from './BaseAdapter.js';
export interface TelegramCredentials {
    token: string;
    webhookUrl?: string;
    pollingTimeout?: number;
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
export declare class TelegramAdapter extends BaseAdapter {
    private bot;
    constructor(config: Omit<AdapterConfig, 'type'> & {
        credentials: TelegramCredentials;
    });
    /**
     * Connect to Telegram
     */
    connect(): Promise<void>;
    /**
     * Disconnect from Telegram
     */
    disconnect(): Promise<void>;
    /**
     * Send a message to a Telegram chat
     */
    send(channelId: string, content: string, options?: SendOptions): Promise<string>;
    /**
     * Reply to a Telegram message
     */
    reply(message: UnifiedMessage, content: string, options?: SendOptions): Promise<string>;
    private loadTelegraf;
    private telegramToUnified;
}
export declare function createTelegramAdapter(config: Omit<AdapterConfig, 'type'> & {
    credentials: TelegramCredentials;
}): TelegramAdapter;
export default TelegramAdapter;
//# sourceMappingURL=TelegramAdapter.d.ts.map