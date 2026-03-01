/**
 * RuvBot - Self-learning AI Assistant with RuVector Backend
 *
 * Main entry point for the RuvBot framework.
 * Combines Clawdbot-style personal AI with RuVector's WASM vector operations.
 */
import { EventEmitter } from 'eventemitter3';
import { type BotConfig } from './core/BotConfig.js';
import { type BotStatus } from './core/BotState.js';
import type { Agent, AgentConfig, Session, Message } from './core/types.js';
type BotState = BotStatus;
export interface RuvBotOptions {
    config?: Partial<BotConfig>;
    configPath?: string;
    autoStart?: boolean;
}
export interface RuvBotEvents {
    ready: () => void;
    shutdown: () => void;
    error: (error: Error) => void;
    message: (message: Message, session: Session) => void;
    'agent:spawn': (agent: Agent) => void;
    'agent:stop': (agentId: string) => void;
    'session:create': (session: Session) => void;
    'session:end': (sessionId: string) => void;
    'memory:store': (entryId: string) => void;
    'skill:invoke': (skillName: string, params: Record<string, unknown>) => void;
}
export declare class RuvBot extends EventEmitter<RuvBotEvents> {
    private readonly id;
    private readonly configManager;
    private readonly stateManager;
    private readonly logger;
    private agents;
    private sessions;
    private isRunning;
    private startTime?;
    private llmProvider;
    private httpServer;
    constructor(options?: RuvBotOptions);
    /**
     * Start the bot and all configured services
     */
    start(): Promise<void>;
    /**
     * Stop the bot and cleanup resources
     */
    stop(): Promise<void>;
    /**
     * Spawn a new agent with the given configuration
     */
    spawnAgent(config: AgentConfig): Promise<Agent>;
    /**
     * Stop an agent by ID
     */
    stopAgent(agentId: string): Promise<void>;
    /**
     * Get an agent by ID
     */
    getAgent(agentId: string): Agent | undefined;
    /**
     * List all active agents
     */
    listAgents(): Agent[];
    /**
     * Create a new session for an agent
     */
    createSession(agentId: string, options?: {
        userId?: string;
        channelId?: string;
        platform?: Session['platform'];
        metadata?: Record<string, unknown>;
    }): Promise<Session>;
    /**
     * End a session by ID
     */
    endSession(sessionId: string): Promise<void>;
    /**
     * Get a session by ID
     */
    getSession(sessionId: string): Session | undefined;
    /**
     * List all active sessions
     */
    listSessions(): Session[];
    /**
     * Send a message to an agent in a session
     */
    chat(sessionId: string, content: string, options?: {
        userId?: string;
        attachments?: Message['attachments'];
        metadata?: Message['metadata'];
    }): Promise<Message>;
    /**
     * Get the current bot status
     */
    getStatus(): {
        id: string;
        name: string;
        state: BotState;
        isRunning: boolean;
        uptime?: number;
        agents: number;
        sessions: number;
    };
    /**
     * Get the current configuration
     */
    getConfig(): Readonly<BotConfig>;
    private initializeServices;
    private startIntegrations;
    private stopIntegrations;
    private startApiServer;
    private stopApiServer;
    private handleApiRequest;
    private parseRequestBody;
    private generateResponse;
}
/**
 * Create a new RuvBot instance
 */
export declare function createRuvBot(options?: RuvBotOptions): RuvBot;
/**
 * Create a RuvBot instance from environment variables
 */
export declare function createRuvBotFromEnv(): RuvBot;
export default RuvBot;
//# sourceMappingURL=RuvBot.d.ts.map