/**
 * Bot state management
 */

import type { Agent, AgentStatus, Session, BotEvent, BotEventType } from './types.js';

// ============================================================================
// Bot State Types
// ============================================================================

export type BotStatus = 'initializing' | 'starting' | 'ready' | 'running' | 'stopping' | 'stopped' | 'error';

export interface BotMetrics {
  uptime: number;
  messagesProcessed: number;
  activesSessions: number;
  memoryUsage: number;
  averageLatency: number;
  errorRate: number;
}

export interface BotStateSnapshot {
  status: BotStatus;
  agents: Map<string, Agent>;
  sessions: Map<string, Session>;
  metrics: BotMetrics;
  startedAt?: Date;
  lastActivityAt?: Date;
}

// ============================================================================
// Event Emitter Interface
// ============================================================================

type EventHandler<T = unknown> = (event: BotEvent<T>) => void | Promise<void>;

// ============================================================================
// Bot State Manager
// ============================================================================

export class BotStateManager {
  private status: BotStatus = 'initializing';
  private agents: Map<string, Agent> = new Map();
  private sessions: Map<string, Session> = new Map();
  private metrics: BotMetrics;
  private startedAt?: Date;
  private lastActivityAt?: Date;
  private eventHandlers: Map<BotEventType | '*', Set<EventHandler>> = new Map();

  constructor() {
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

  getStatus(): BotStatus {
    return this.status;
  }

  setStatus(status: BotStatus): void {
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

  isReady(): boolean {
    return this.status === 'ready' || this.status === 'running';
  }

  // ============================================================================
  // Agent Management
  // ============================================================================

  registerAgent(agent: Agent): void {
    this.agents.set(agent.id, agent);
  }

  getAgent(id: string): Agent | undefined {
    return this.agents.get(id);
  }

  getAllAgents(): Agent[] {
    return Array.from(this.agents.values());
  }

  updateAgentStatus(id: string, status: AgentStatus): void {
    const agent = this.agents.get(id);
    if (agent) {
      agent.status = status;
      agent.lastActiveAt = new Date();
    }
  }

  removeAgent(id: string): boolean {
    return this.agents.delete(id);
  }

  // ============================================================================
  // Session Management
  // ============================================================================

  registerSession(session: Session): void {
    this.sessions.set(session.id, session);
    this.metrics.activesSessions = this.sessions.size;
  }

  getSession(id: string): Session | undefined {
    return this.sessions.get(id);
  }

  getAllSessions(): Session[] {
    return Array.from(this.sessions.values());
  }

  getSessionsByAgent(agentId: string): Session[] {
    return Array.from(this.sessions.values()).filter((s) => s.agentId === agentId);
  }

  getSessionsByUser(userId: string): Session[] {
    return Array.from(this.sessions.values()).filter((s) => s.userId === userId);
  }

  updateSession(session: Session): void {
    if (this.sessions.has(session.id)) {
      session.updatedAt = new Date();
      this.sessions.set(session.id, session);
    }
  }

  removeSession(id: string): boolean {
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

  getMetrics(): Readonly<BotMetrics> {
    // Update uptime
    if (this.startedAt) {
      this.metrics.uptime = Date.now() - this.startedAt.getTime();
    }

    // Update memory usage
    const memUsage = process.memoryUsage();
    this.metrics.memoryUsage = memUsage.heapUsed;

    return Object.freeze({ ...this.metrics });
  }

  incrementMessagesProcessed(): void {
    this.metrics.messagesProcessed++;
    this.lastActivityAt = new Date();
  }

  updateLatency(latencyMs: number): void {
    // Running average
    const count = this.metrics.messagesProcessed || 1;
    this.metrics.averageLatency =
      (this.metrics.averageLatency * (count - 1) + latencyMs) / count;
  }

  recordError(): void {
    const total = this.metrics.messagesProcessed || 1;
    this.metrics.errorRate = (this.metrics.errorRate * total + 1) / (total + 1);
  }

  // ============================================================================
  // Event Handling
  // ============================================================================

  on<T>(eventType: BotEventType | '*', handler: EventHandler<T>): () => void {
    if (!this.eventHandlers.has(eventType)) {
      this.eventHandlers.set(eventType, new Set());
    }
    this.eventHandlers.get(eventType)!.add(handler as EventHandler);

    // Return unsubscribe function
    return () => {
      this.eventHandlers.get(eventType)?.delete(handler as EventHandler);
    };
  }

  off(eventType: BotEventType | '*', handler: EventHandler): void {
    this.eventHandlers.get(eventType)?.delete(handler);
  }

  emit<T>(event: BotEvent<T>): void {
    // Call specific handlers
    const handlers = this.eventHandlers.get(event.type);
    if (handlers) {
      for (const handler of handlers) {
        try {
          handler(event);
        } catch (error) {
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
        } catch (error) {
          console.error('Wildcard event handler error:', error);
        }
      }
    }
  }

  // ============================================================================
  // Snapshot
  // ============================================================================

  getSnapshot(): BotStateSnapshot {
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

  cleanupExpiredSessions(): number {
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

  reset(): void {
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
