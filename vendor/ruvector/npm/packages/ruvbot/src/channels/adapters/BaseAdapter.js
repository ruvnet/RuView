"use strict";
/**
 * BaseAdapter - Abstract Channel Adapter
 *
 * Base class for all channel adapters providing a unified interface
 * for multi-channel messaging support.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.BaseAdapter = void 0;
const uuid_1 = require("uuid");
// ============================================================================
// BaseAdapter Abstract Class
// ============================================================================
class BaseAdapter {
    constructor(config) {
        this.messageHandlers = [];
        this.config = {
            ...config,
            enabled: config.enabled ?? true,
        };
        this.status = {
            connected: false,
            errorCount: 0,
            messageCount: 0,
        };
    }
    /**
     * Get channel type
     */
    get type() {
        return this.config.type;
    }
    /**
     * Get tenant ID
     */
    get tenantId() {
        return this.config.tenantId;
    }
    /**
     * Check if adapter is enabled
     */
    get enabled() {
        return this.config.enabled ?? true;
    }
    /**
     * Get adapter status
     */
    getStatus() {
        return { ...this.status };
    }
    /**
     * Register a message handler
     */
    onMessage(handler) {
        this.messageHandlers.push(handler);
    }
    /**
     * Remove a message handler
     */
    offMessage(handler) {
        const index = this.messageHandlers.indexOf(handler);
        if (index > -1) {
            this.messageHandlers.splice(index, 1);
        }
    }
    /**
     * Emit a received message to all handlers
     */
    async emitMessage(message) {
        this.status.messageCount++;
        this.status.lastActivity = new Date();
        for (const handler of this.messageHandlers) {
            try {
                await handler(message);
            }
            catch (error) {
                this.status.errorCount++;
                console.error(`Message handler error in ${this.type}:`, error);
            }
        }
    }
    /**
     * Create a unified message from raw input
     */
    createUnifiedMessage(content, userId, channelId, extra = {}) {
        return {
            id: (0, uuid_1.v4)(),
            channelId,
            channelType: this.config.type,
            tenantId: this.config.tenantId,
            userId,
            content,
            timestamp: new Date(),
            metadata: {},
            ...extra,
        };
    }
}
exports.BaseAdapter = BaseAdapter;
exports.default = BaseAdapter;
//# sourceMappingURL=BaseAdapter.js.map