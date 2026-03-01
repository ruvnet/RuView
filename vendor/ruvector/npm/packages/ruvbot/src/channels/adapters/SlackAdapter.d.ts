/**
 * SlackAdapter - Slack Channel Integration
 *
 * Connects to Slack workspace using @slack/bolt for real-time messaging.
 * Supports threads, reactions, file attachments, and app mentions.
 */
import { BaseAdapter, type AdapterConfig, type UnifiedMessage, type SendOptions } from './BaseAdapter.js';
export interface SlackCredentials {
    token: string;
    signingSecret: string;
    appToken?: string;
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
export declare class SlackAdapter extends BaseAdapter {
    private client;
    private app;
    constructor(config: Omit<AdapterConfig, 'type'> & {
        credentials: SlackCredentials;
    });
    /**
     * Connect to Slack
     */
    connect(): Promise<void>;
    /**
     * Disconnect from Slack
     */
    disconnect(): Promise<void>;
    /**
     * Send a message to a Slack channel
     */
    send(channelId: string, content: string, options?: SendOptions): Promise<string>;
    /**
     * Reply to a Slack message
     */
    reply(message: UnifiedMessage, content: string, options?: SendOptions): Promise<string>;
    private loadSlackBolt;
    private getClient;
    private slackToUnified;
    private getMimeCategory;
}
export declare function createSlackAdapter(config: Omit<AdapterConfig, 'type'> & {
    credentials: SlackCredentials;
}): SlackAdapter;
export default SlackAdapter;
//# sourceMappingURL=SlackAdapter.d.ts.map