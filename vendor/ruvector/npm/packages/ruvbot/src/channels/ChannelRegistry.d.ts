/**
 * ChannelRegistry - Multi-Channel Management
 *
 * Manages multiple channel adapters with unified message routing,
 * multi-tenant isolation, and rate limiting.
 */
import type { BaseAdapter, ChannelType, MessageHandler, AdapterConfig } from './adapters/BaseAdapter.js';
export interface ChannelFilter {
    types?: ChannelType[];
    tenantIds?: string[];
    channelIds?: string[];
}
export interface ChannelRegistryConfig {
    defaultRateLimit?: {
        requests: number;
        windowMs: number;
    };
}
export interface AdapterFactory {
    (config: AdapterConfig): BaseAdapter;
}
export declare class ChannelRegistry {
    private adapters;
    private adaptersByType;
    private adaptersByTenant;
    private globalHandlers;
    private config;
    private rateLimitWindows;
    constructor(config?: ChannelRegistryConfig);
    /**
     * Generate unique adapter key
     */
    private getAdapterKey;
    /**
     * Register a channel adapter
     */
    register(adapter: BaseAdapter): void;
    /**
     * Unregister a channel adapter
     */
    unregister(type: ChannelType, tenantId: string): boolean;
    /**
     * Get a specific adapter
     */
    get(type: ChannelType, tenantId: string): BaseAdapter | undefined;
    /**
     * Get all adapters for a type
     */
    getByType(type: ChannelType): BaseAdapter[];
    /**
     * Get all adapters for a tenant
     */
    getByTenant(tenantId: string): BaseAdapter[];
    /**
     * Get all registered adapters
     */
    getAll(): BaseAdapter[];
    /**
     * Register a global message handler
     */
    onMessage(handler: MessageHandler): void;
    /**
     * Remove a global message handler
     */
    offMessage(handler: MessageHandler): void;
    /**
     * Start all adapters
     */
    start(): Promise<void>;
    /**
     * Stop all adapters
     */
    stop(): Promise<void>;
    /**
     * Broadcast a message to multiple channels
     */
    broadcast(message: string, channelIds: string[], filter?: ChannelFilter): Promise<Map<string, string>>;
    /**
     * Get registry statistics
     */
    getStats(): {
        totalAdapters: number;
        byType: Record<ChannelType, number>;
        byTenant: Record<string, number>;
        connected: number;
        totalMessages: number;
    };
    private handleMessage;
    private filterAdapters;
    private checkRateLimit;
}
export declare function createChannelRegistry(config?: ChannelRegistryConfig): ChannelRegistry;
export default ChannelRegistry;
//# sourceMappingURL=ChannelRegistry.d.ts.map