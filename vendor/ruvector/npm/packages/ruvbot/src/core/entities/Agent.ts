/**
 * Agent entity - represents an AI agent instance
 */

import { v4 as uuid } from 'uuid';
import type { Agent, AgentConfig, AgentStatus } from '../types.js';

export class AgentEntity implements Agent {
  public readonly id: string;
  public readonly name: string;
  public readonly config: AgentConfig;
  public status: AgentStatus;
  public readonly createdAt: Date;
  public lastActiveAt: Date;

  constructor(config: AgentConfig) {
    this.id = config.id || uuid();
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
  static create(name: string, options?: Partial<AgentConfig>): AgentEntity {
    return new AgentEntity({
      id: uuid(),
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
  setStatus(status: AgentStatus): void {
    this.status = status;
    this.lastActiveAt = new Date();
  }

  /**
   * Check if agent is available for processing
   */
  isAvailable(): boolean {
    return this.status === 'idle';
  }

  /**
   * Check if agent has a specific skill
   */
  hasSkill(skillName: string): boolean {
    return this.config.skills?.includes(skillName) ?? false;
  }

  /**
   * Get agent uptime in milliseconds
   */
  getUptime(): number {
    return Date.now() - this.createdAt.getTime();
  }

  /**
   * Serialize agent to JSON
   */
  toJSON(): Record<string, unknown> {
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
  static fromJSON(data: Record<string, unknown>): AgentEntity {
    const agent = new AgentEntity(data.config as AgentConfig);
    agent.status = data.status as AgentStatus;
    return agent;
  }
}
