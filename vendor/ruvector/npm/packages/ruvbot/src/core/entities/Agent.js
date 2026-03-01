"use strict";
/**
 * Agent entity - represents an AI agent instance
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.AgentEntity = void 0;
const uuid_1 = require("uuid");
class AgentEntity {
    constructor(config) {
        this.id = config.id || (0, uuid_1.v4)();
        this.name = config.name;
        this.config = {
            ...config,
            id: this.id,
        };
        this.status = 'idle';
        this.createdAt = new Date();
        this.lastActiveAt = new Date();
    }
    /**
     * Create a new agent with default configuration
     */
    static create(name, options) {
        return new AgentEntity({
            id: (0, uuid_1.v4)(),
            name,
            model: options?.model ?? 'claude-sonnet-4-20250514',
            temperature: options?.temperature ?? 0.7,
            maxTokens: options?.maxTokens ?? 4096,
            skills: options?.skills ?? ['search', 'summarize', 'memory'],
            ...options,
        });
    }
    /**
     * Update agent status
     */
    setStatus(status) {
        this.status = status;
        this.lastActiveAt = new Date();
    }
    /**
     * Check if agent is available for processing
     */
    isAvailable() {
        return this.status === 'idle';
    }
    /**
     * Check if agent has a specific skill
     */
    hasSkill(skillName) {
        return this.config.skills?.includes(skillName) ?? false;
    }
    /**
     * Get agent uptime in milliseconds
     */
    getUptime() {
        return Date.now() - this.createdAt.getTime();
    }
    /**
     * Serialize agent to JSON
     */
    toJSON() {
        return {
            id: this.id,
            name: this.name,
            config: this.config,
            status: this.status,
            createdAt: this.createdAt.toISOString(),
            lastActiveAt: this.lastActiveAt.toISOString(),
        };
    }
    /**
     * Create agent from JSON
     */
    static fromJSON(data) {
        const agent = new AgentEntity(data.config);
        agent.status = data.status;
        return agent;
    }
}
exports.AgentEntity = AgentEntity;
//# sourceMappingURL=Agent.js.map