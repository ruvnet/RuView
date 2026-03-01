"use strict";
/**
 * Bot state management
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.BotStateManager = void 0;
// ============================================================================
// Bot State Manager
// ============================================================================
class BotStateManager {
    constructor() {
        this.status = 'initializing';
        this.agents = new Map();
        this.sessions = new Map();
        this.eventHandlers = new Map();
        this.metrics = {
            uptime: 0,
            messagesProcessed: 0,
            activesSessions: 0,
            memoryUsage: 0,
            averageLatency: 0,
            errorRate: 0,
        };
    }
    // ============================================================================
    // Status Management
    // ============================================================================
    getStatus() {
        return this.status;
    }
    setStatus(status) {
        const oldStatus = this.status;
        this.status = status;
        if (status === 'running' && !this.startedAt) {
            this.startedAt = new Date();
        }
        this.emit({
            type: 'agent:status',
            timestamp: new Date(),
            source: 'BotStateManager',
            data: { oldStatus, newStatus: status },
        });
    }
    isReady() {
        return this.status === 'ready' || this.status === 'running';
    }
    // ============================================================================
    // Agent Management
    // ============================================================================
    registerAgent(agent) {
        this.agents.set(agent.id, agent);
    }
    getAgent(id) {
        return this.agents.get(id);
    }
    getAllAgents() {
        return Array.from(this.agents.values());
    }
    updateAgentStatus(id, status) {
        const agent = this.agents.get(id);
        if (agent) {
            agent.status = status;
            agent.lastActiveAt = new Date();
        }
    }
    removeAgent(id) {
        return this.agents.delete(id);
    }
    // ============================================================================
    // Session Management
    // ============================================================================
    registerSession(session) {
        this.sessions.set(session.id, session);
        this.metrics.activesSessions = this.sessions.size;
    }
    getSession(id) {
        return this.sessions.get(id);
    }
    getAllSessions() {
        return Array.from(this.sessions.values());
    }
    getSessionsByAgent(agentId) {
        return Array.from(this.sessions.values()).filter((s) => s.agentId === agentId);
    }
    getSessionsByUser(userId) {
        return Array.from(this.sessions.values()).filter((s) => s.userId === userId);
    }
    updateSession(session) {
        if (this.sessions.has(session.id)) {
            session.updatedAt = new Date();
            this.sessions.set(session.id, session);
        }
    }
    removeSession(id) {
        const result = this.sessions.delete(id);
        if (result) {
            this.metrics.activesSessions = this.sessions.size;
            this.emit({
                type: 'session:ended',
                timestamp: new Date(),
                source: 'BotStateManager',
                data: { sessionId: id },
            });
        }
        return result;
    }
    // ============================================================================
    // Metrics
    // ============================================================================
    getMetrics() {
        // Update uptime
        if (this.startedAt) {
            this.metrics.uptime = Date.now() - this.startedAt.getTime();
        }
        // Update memory usage
        const memUsage = process.memoryUsage();
        this.metrics.memoryUsage = memUsage.heapUsed;
        return Object.freeze({ ...this.metrics });
    }
    incrementMessagesProcessed() {
        this.metrics.messagesProcessed++;
        this.lastActivityAt = new Date();
    }
    updateLatency(latencyMs) {
        // Running average
        const count = this.metrics.messagesProcessed || 1;
        this.metrics.averageLatency =
            (this.metrics.averageLatency * (count - 1) + latencyMs) / count;
    }
    recordError() {
        const total = this.metrics.messagesProcessed || 1;
        this.metrics.errorRate = (this.metrics.errorRate * total + 1) / (total + 1);
    }
    // ============================================================================
    // Event Handling
    // ============================================================================
    on(eventType, handler) {
        if (!this.eventHandlers.has(eventType)) {
            this.eventHandlers.set(eventType, new Set());
        }
        this.eventHandlers.get(eventType).add(handler);
        // Return unsubscribe function
        return () => {
            this.eventHandlers.get(eventType)?.delete(handler);
        };
    }
    off(eventType, handler) {
        this.eventHandlers.get(eventType)?.delete(handler);
    }
    emit(event) {
        // Call specific handlers
        const handlers = this.eventHandlers.get(event.type);
        if (handlers) {
            for (const handler of handlers) {
                try {
                    handler(event);
                }
                catch (error) {
                    console.error(`Event handler error for ${event.type}:`, error);
                }
            }
        }
        // Call wildcard handlers
        const wildcardHandlers = this.eventHandlers.get('*');
        if (wildcardHandlers) {
            for (const handler of wildcardHandlers) {
                try {
                    handler(event);
                }
                catch (error) {
                    console.error('Wildcard event handler error:', error);
                }
            }
        }
    }
    // ============================================================================
    // Snapshot
    // ============================================================================
    getSnapshot() {
        return {
            status: this.status,
            agents: new Map(this.agents),
            sessions: new Map(this.sessions),
            metrics: this.getMetrics(),
            startedAt: this.startedAt,
            lastActivityAt: this.lastActivityAt,
        };
    }
    // ============================================================================
    // Cleanup
    // ============================================================================
    cleanupExpiredSessions() {
        const now = Date.now();
        let cleaned = 0;
        for (const [id, session] of this.sessions) {
            if (session.expiresAt && session.expiresAt.getTime() < now) {
                this.removeSession(id);
                cleaned++;
            }
        }
        return cleaned;
    }
    reset() {
        this.agents.clear();
        this.sessions.clear();
        this.eventHandlers.clear();
        this.status = 'initializing';
        this.startedAt = undefined;
        this.lastActivityAt = undefined;
        this.metrics = {
            uptime: 0,
            messagesProcessed: 0,
            activesSessions: 0,
            memoryUsage: 0,
            averageLatency: 0,
            errorRate: 0,
        };
    }
}
exports.BotStateManager = BotStateManager;
//# sourceMappingURL=BotState.js.map