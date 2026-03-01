/**
 * Agent entity - represents an AI agent instance
 */
import type { Agent, AgentConfig, AgentStatus } from '../types.js';
export declare class AgentEntity implements Agent {
    readonly id: string;
    readonly name: string;
    readonly config: AgentConfig;
    status: AgentStatus;
    readonly createdAt: Date;
    lastActiveAt: Date;
    constructor(config: AgentConfig);
    /**
     * Create a new agent with default configuration
     */
    static create(name: string, options?: Partial<AgentConfig>): AgentEntity;
    /**
     * Update agent status
     */
    setStatus(status: AgentStatus): void;
    /**
     * Check if agent is available for processing
     */
    isAvailable(): boolean;
    /**
     * Check if agent has a specific skill
     */
    hasSkill(skillName: string): boolean;
    /**
     * Get agent uptime in milliseconds
     */
    getUptime(): number;
    /**
     * Serialize agent to JSON
     */
    toJSON(): Record<string, unknown>;
    /**
     * Create agent from JSON
     */
    static fromJSON(data: Record<string, unknown>): AgentEntity;
}
//# sourceMappingURL=Agent.d.ts.map