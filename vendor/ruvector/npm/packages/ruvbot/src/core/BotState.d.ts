/**
 * Bot state management
 */
import type { Agent, AgentStatus, Session, BotEvent, BotEventType } from './types.js';
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
type EventHandler<T = unknown> = (event: BotEvent<T>) => void | Promise<void>;
export declare class BotStateManager {
    private status;
    private agents;
    private sessions;
    private metrics;
    private startedAt?;
    private lastActivityAt?;
    private eventHandlers;
    constructor();
    getStatus(): BotStatus;
    setStatus(status: BotStatus): void;
    isReady(): boolean;
    registerAgent(agent: Agent): void;
    getAgent(id: string): Agent | undefined;
    getAllAgents(): Agent[];
    updateAgentStatus(id: string, status: AgentStatus): void;
    removeAgent(id: string): boolean;
    registerSession(session: Session): void;
    getSession(id: string): Session | undefined;
    getAllSessions(): Session[];
    getSessionsByAgent(agentId: string): Session[];
    getSessionsByUser(userId: string): Session[];
    updateSession(session: Session): void;
    removeSession(id: string): boolean;
    getMetrics(): Readonly<BotMetrics>;
    incrementMessagesProcessed(): void;
    updateLatency(latencyMs: number): void;
    recordError(): void;
    on<T>(eventType: BotEventType | '*', handler: EventHandler<T>): () => void;
    off(eventType: BotEventType | '*', handler: EventHandler): void;
    emit<T>(event: BotEvent<T>): void;
    getSnapshot(): BotStateSnapshot;
    cleanupExpiredSessions(): number;
    reset(): void;
}
export {};
//# sourceMappingURL=BotState.d.ts.map