/**
 * ChannelRegistry - Multi-Channel Management
 *
 * Manages multiple channel adapters with unified message routing,
 * multi-tenant isolation, and rate limiting.
 */

import type {
  BaseAdapter,
  ChannelType,
  UnifiedMessage,
  MessageHandler,
  AdapterConfig,
} from './adapters/BaseAdapter.js';

// ============================================================================
// Types
// ============================================================================

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

// ============================================================================
// ChannelRegistry Implementation
// ============================================================================

export class ChannelRegistry {
  private adapters: Map<string, BaseAdapter> = new Map();
  private adaptersByType: Map<ChannelType, Set<string>> = new Map();
  private adaptersByTenant: Map<string, Set<string>> = new Map();
  private globalHandlers: MessageHandler[] = [];
  private config: ChannelRegistryConfig;

  // Rate limiting state
  private rateLimitWindows: Map<string, { count: number; resetAt: number }> = new Map();

  constructor(config: ChannelRegistryConfig = {}) {
    this.config = config;
  }

  /**
   * Generate unique adapter key
   */
  private getAdapterKey(type: ChannelType, tenantId: string): string {
    return `${type}:${tenantId}`;
  }

  /**
   * Register a channel adapter
   */
  register(adapter: BaseAdapter): void {
    const key = this.getAdapterKey(adapter.type, adapter.tenantId);

    // Store adapter
    this.adapters.set(key, adapter);

    // Index by type
    if (!this.adaptersByType.has(adapter.type)) {
      this.adaptersByType.set(adapter.type, new Set());
    }
    this.adaptersByType.get(adapter.type)!.add(key);

    // Index by tenant
    if (!this.adaptersByTenant.has(adapter.tenantId)) {
      this.adaptersByTenant.set(adapter.tenantId, new Set());
    }
    this.adaptersByTenant.get(adapter.tenantId)!.add(key);

    // Register global message handler on adapter
    adapter.onMessage(async (message) => {
      await this.handleMessage(message);
    });
  }

  /**
   * Unregister a channel adapter
   */
  unregister(type: ChannelType, tenantId: string): boolean {
    const key = this.getAdapterKey(type, tenantId);
    const adapter = this.adapters.get(key);

    if (!adapter) return false;

    // Remove from indices
    this.adaptersByType.get(type)?.delete(key);
    this.adaptersByTenant.get(tenantId)?.delete(key);

    // Remove adapter
    this.adapters.delete(key);

    return true;
  }

  /**
   * Get a specific adapter
   */
  get(type: ChannelType, tenantId: string): BaseAdapter | undefined {
    return this.adapters.get(this.getAdapterKey(type, tenantId));
  }

  /**
   * Get all adapters for a type
   */
  getByType(type: ChannelType): BaseAdapter[] {
    const keys = this.adaptersByType.get(type);
    if (!keys) return [];

    return Array.from(keys)
      .map(key => this.adapters.get(key))
      .filter((a): a is BaseAdapter => a !== undefined);
  }

  /**
   * Get all adapters for a tenant
   */
  getByTenant(tenantId: string): BaseAdapter[] {
    const keys = this.adaptersByTenant.get(tenantId);
    if (!keys) return [];

    return Array.from(keys)
      .map(key => this.adapters.get(key))
      .filter((a): a is BaseAdapter => a !== undefined);
  }

  /**
   * Get all registered adapters
   */
  getAll(): BaseAdapter[] {
    return Array.from(this.adapters.values());
  }

  /**
   * Register a global message handler
   */
  onMessage(handler: MessageHandler): void {
    this.globalHandlers.push(handler);
  }

  /**
   * Remove a global message handler
   */
  offMessage(handler: MessageHandler): void {
    const index = this.globalHandlers.indexOf(handler);
    if (index > -1) {
      this.globalHandlers.splice(index, 1);
    }
  }

  /**
   * Start all adapters
   */
  async start(): Promise<void> {
    const startPromises = Array.from(this.adapters.values())
      .filter(adapter => adapter.enabled)
      .map(adapter => adapter.connect());

    await Promise.all(startPromises);
  }

  /**
   * Stop all adapters
   */
  async stop(): Promise<void> {
    const stopPromises = Array.from(this.adapters.values())
      .map(adapter => adapter.disconnect());

    await Promise.all(stopPromises);
  }

  /**
   * Broadcast a message to multiple channels
   */
  async broadcast(
    message: string,
    channelIds: string[],
    filter?: ChannelFilter
  ): Promise<Map<string, string>> {
    const results = new Map<string, string>();
    const adapters = this.filterAdapters(filter);

    for (const adapter of adapters) {
      for (const channelId of channelIds) {
        try {
          if (this.checkRateLimit(adapter)) {
            const messageId = await adapter.send(channelId, message);
            results.set(`${adapter.type}:${channelId}`, messageId);
          }
        } catch (error) {
          console.error(`Failed to broadcast to ${adapter.type}:${channelId}:`, error);
        }
      }
    }

    return results;
  }

  /**
   * Get registry statistics
   */
  getStats(): {
    totalAdapters: number;
    byType: Record<ChannelType, number>;
    byTenant: Record<string, number>;
    connected: number;
    totalMessages: number;
  } {
    const byType = {} as Record<ChannelType, number>;
    const byTenant = {} as Record<string, number>;
    let connected = 0;
    let totalMessages = 0;

    for (const adapter of this.adapters.values()) {
      // By type
      byType[adapter.type] = (byType[adapter.type] ?? 0) + 1;

      // By tenant
      byTenant[adapter.tenantId] = (byTenant[adapter.tenantId] ?? 0) + 1;

      // Connected status
      const status = adapter.getStatus();
      if (status.connected) connected++;
      totalMessages += status.messageCount;
    }

    return {
      totalAdapters: this.adapters.size,
      byType,
      byTenant,
      connected,
      totalMessages,
    };
  }

  // ==========================================================================
  // Private Methods
  // ==========================================================================

  private async handleMessage(message: UnifiedMessage): Promise<void> {
    for (const handler of this.globalHandlers) {
      try {
        await handler(message);
      } catch (error) {
        console.error('Global message handler error:', error);
      }
    }
  }

  private filterAdapters(filter?: ChannelFilter): BaseAdapter[] {
    let adapters = Array.from(this.adapters.values());

    if (filter?.types) {
      adapters = adapters.filter(a => filter.types!.includes(a.type));
    }

    if (filter?.tenantIds) {
      adapters = adapters.filter(a => filter.tenantIds!.includes(a.tenantId));
    }

    return adapters.filter(a => a.enabled);
  }

  private checkRateLimit(adapter: BaseAdapter): boolean {
    const config = this.config.defaultRateLimit;
    if (!config) return true;

    const key = this.getAdapterKey(adapter.type, adapter.tenantId);
    const now = Date.now();

    let window = this.rateLimitWindows.get(key);
    if (!window || now > window.resetAt) {
      window = { count: 0, resetAt: now + config.windowMs };
      this.rateLimitWindows.set(key, window);
    }

    if (window.count >= config.requests) {
      return false;
    }

    window.count++;
    return true;
  }
}

// ============================================================================
// Factory Function
// ============================================================================

export function createChannelRegistry(config?: ChannelRegistryConfig): ChannelRegistry {
  return new ChannelRegistry(config);
}

export default ChannelRegistry;
