/**
 * Session Domain Entity - Unit Tests
 *
 * Tests for Session lifecycle, context management, and conversation handling
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { createSession, createSessionWithHistory, type Session, type ConversationMessage } from '../../factories';

// Session Entity Types
interface SessionState {
  id: string;
  tenantId: string;
  userId: string;
  channelId: string;
  threadTs: string;
  status: 'active' | 'paused' | 'completed' | 'error';
  context: SessionContext;
  metadata: SessionMetadata;
}

interface SessionContext {
  conversationHistory: ConversationMessage[];
  workingDirectory: string;
  activeAgents: string[];
  variables: Map<string, unknown>;
  artifacts: Map<string, Artifact>;
}

interface Artifact {
  id: string;
  type: 'code' | 'file' | 'image' | 'document';
  content: unknown;
  createdAt: Date;
}

interface SessionMetadata {
  createdAt: Date;
  lastActiveAt: Date;
  messageCount: number;
  tokenUsage: number;
  estimatedCost: number;
}

// Mock Session class for testing
class SessionEntity {
  private state: SessionState;
  private eventLog: Array<{ type: string; payload: unknown; timestamp: Date }> = [];
  private readonly maxHistoryLength = 100;

  constructor(initialState: Partial<SessionState>) {
    this.state = {
      id: initialState.id || `session-${Date.now()}`,
      tenantId: initialState.tenantId || 'default-tenant',
      userId: initialState.userId || 'unknown-user',
      channelId: initialState.channelId || 'unknown-channel',
      threadTs: initialState.threadTs || `${Date.now()}.000000`,
      status: initialState.status || 'active',
      context: {
        conversationHistory: initialState.context?.conversationHistory || [],
        workingDirectory: initialState.context?.workingDirectory || '/workspace',
        activeAgents: initialState.context?.activeAgents || [],
        variables: new Map(Object.entries(initialState.context?.variables || {})),
        artifacts: new Map()
      },
      metadata: {
        createdAt: initialState.metadata?.createdAt || new Date(),
        lastActiveAt: initialState.metadata?.lastActiveAt || new Date(),
        messageCount: initialState.metadata?.messageCount || 0,
        tokenUsage: initialState.metadata?.tokenUsage || 0,
        estimatedCost: initialState.metadata?.estimatedCost || 0
      }
    };
  }

  getId(): string {
    return this.state.id;
  }

  getTenantId(): string {
    return this.state.tenantId;
  }

  getUserId(): string {
    return this.state.userId;
  }

  getChannelId(): string {
    return this.state.channelId;
  }

  getThreadTs(): string {
    return this.state.threadTs;
  }

  getStatus(): SessionState['status'] {
    return this.state.status;
  }

  isActive(): boolean {
    return this.state.status === 'active';
  }

  getConversationHistory(): ConversationMessage[] {
    return [...this.state.context.conversationHistory];
  }

  getActiveAgents(): string[] {
    return [...this.state.context.activeAgents];
  }

  getWorkingDirectory(): string {
    return this.state.context.workingDirectory;
  }

  getVariable(key: string): unknown {
    return this.state.context.variables.get(key);
  }

  getMetadata(): SessionMetadata {
    return { ...this.state.metadata };
  }

  async addMessage(message: Omit<ConversationMessage, 'timestamp'>): Promise<void> {
    if (this.state.status !== 'active') {
      throw new Error(`Cannot add message to ${this.state.status} session`);
    }

    const fullMessage: ConversationMessage = {
      ...message,
      timestamp: new Date()
    };

    this.state.context.conversationHistory.push(fullMessage);
    this.state.metadata.messageCount++;
    this.state.metadata.lastActiveAt = new Date();

    // Trim history if too long
    if (this.state.context.conversationHistory.length > this.maxHistoryLength) {
      this.state.context.conversationHistory.shift();
    }

    this.logEvent('message_added', { role: message.role });
  }

  async addUserMessage(content: string): Promise<void> {
    await this.addMessage({ role: 'user', content });
  }

  async addAssistantMessage(content: string, agentId?: string): Promise<void> {
    await this.addMessage({ role: 'assistant', content, agentId });
  }

  async addSystemMessage(content: string): Promise<void> {
    await this.addMessage({ role: 'system', content });
  }

  getLastMessage(): ConversationMessage | undefined {
    const history = this.state.context.conversationHistory;
    return history.length > 0 ? history[history.length - 1] : undefined;
  }

  getMessageCount(): number {
    return this.state.metadata.messageCount;
  }

  async attachAgent(agentId: string): Promise<void> {
    if (!this.state.context.activeAgents.includes(agentId)) {
      this.state.context.activeAgents.push(agentId);
      this.logEvent('agent_attached', { agentId });
    }
  }

  async detachAgent(agentId: string): Promise<void> {
    const index = this.state.context.activeAgents.indexOf(agentId);
    if (index !== -1) {
      this.state.context.activeAgents.splice(index, 1);
      this.logEvent('agent_detached', { agentId });
    }
  }

  setVariable(key: string, value: unknown): void {
    this.state.context.variables.set(key, value);
    this.logEvent('variable_set', { key });
  }

  deleteVariable(key: string): boolean {
    const deleted = this.state.context.variables.delete(key);
    if (deleted) {
      this.logEvent('variable_deleted', { key });
    }
    return deleted;
  }

  setWorkingDirectory(path: string): void {
    this.state.context.workingDirectory = path;
    this.logEvent('working_directory_changed', { path });
  }

  addArtifact(artifact: Omit<Artifact, 'createdAt'>): void {
    const fullArtifact: Artifact = {
      ...artifact,
      createdAt: new Date()
    };
    this.state.context.artifacts.set(artifact.id, fullArtifact);
    this.logEvent('artifact_added', { artifactId: artifact.id, type: artifact.type });
  }

  getArtifact(id: string): Artifact | undefined {
    return this.state.context.artifacts.get(id);
  }

  listArtifacts(): Artifact[] {
    return Array.from(this.state.context.artifacts.values());
  }

  updateTokenUsage(tokens: number, cost: number): void {
    this.state.metadata.tokenUsage += tokens;
    this.state.metadata.estimatedCost += cost;
  }

  async pause(): Promise<void> {
    if (this.state.status !== 'active') {
      throw new Error(`Cannot pause ${this.state.status} session`);
    }
    this.state.status = 'paused';
    this.logEvent('paused', {});
  }

  async resume(): Promise<void> {
    if (this.state.status !== 'paused') {
      throw new Error(`Cannot resume ${this.state.status} session`);
    }
    this.state.status = 'active';
    this.state.metadata.lastActiveAt = new Date();
    this.logEvent('resumed', {});
  }

  async complete(): Promise<void> {
    if (this.state.status === 'completed') {
      return; // Already completed
    }
    this.state.status = 'completed';
    this.state.context.activeAgents = [];
    this.logEvent('completed', {});
  }

  async fail(error: Error): Promise<void> {
    this.state.status = 'error';
    this.logEvent('failed', { error: error.message });
  }

  clearHistory(): void {
    this.state.context.conversationHistory = [];
    this.state.metadata.messageCount = 0;
    this.logEvent('history_cleared', {});
  }

  getEventLog(): Array<{ type: string; payload: unknown; timestamp: Date }> {
    return [...this.eventLog];
  }

  toJSON(): SessionState {
    return {
      ...this.state,
      context: {
        ...this.state.context,
        variables: Object.fromEntries(this.state.context.variables) as unknown as Map<string, unknown>,
        artifacts: Object.fromEntries(this.state.context.artifacts) as unknown as Map<string, Artifact>
      }
    };
  }

  private logEvent(type: string, payload: unknown): void {
    this.eventLog.push({ type, payload, timestamp: new Date() });
  }
}

// Tests
describe('Session Domain Entity', () => {
  describe('Construction', () => {
    it('should create session with default values', () => {
      const session = new SessionEntity({});

      expect(session.getId()).toBeDefined();
      expect(session.getStatus()).toBe('active');
      expect(session.getConversationHistory()).toEqual([]);
      expect(session.getActiveAgents()).toEqual([]);
    });

    it('should create session with provided values', () => {
      const session = new SessionEntity({
        id: 'session-001',
        tenantId: 'tenant-001',
        userId: 'user-001',
        channelId: 'C12345',
        threadTs: '1234567890.123456'
      });

      expect(session.getId()).toBe('session-001');
      expect(session.getTenantId()).toBe('tenant-001');
      expect(session.getUserId()).toBe('user-001');
      expect(session.getChannelId()).toBe('C12345');
      expect(session.getThreadTs()).toBe('1234567890.123456');
    });

    it('should initialize metadata correctly', () => {
      const session = new SessionEntity({});
      const metadata = session.getMetadata();

      expect(metadata.messageCount).toBe(0);
      expect(metadata.tokenUsage).toBe(0);
      expect(metadata.estimatedCost).toBe(0);
      expect(metadata.createdAt).toBeInstanceOf(Date);
    });
  });

  describe('Status Management', () => {
    it('should be active by default', () => {
      const session = new SessionEntity({});
      expect(session.isActive()).toBe(true);
    });

    it('should pause active session', async () => {
      const session = new SessionEntity({ status: 'active' });

      await session.pause();

      expect(session.getStatus()).toBe('paused');
      expect(session.isActive()).toBe(false);
    });

    it('should resume paused session', async () => {
      const session = new SessionEntity({ status: 'paused' });

      await session.resume();

      expect(session.getStatus()).toBe('active');
      expect(session.isActive()).toBe(true);
    });

    it('should complete session', async () => {
      const session = new SessionEntity({ status: 'active' });
      await session.attachAgent('agent-001');

      await session.complete();

      expect(session.getStatus()).toBe('completed');
      expect(session.getActiveAgents()).toEqual([]);
    });

    it('should fail session', async () => {
      const session = new SessionEntity({ status: 'active' });

      await session.fail(new Error('Something went wrong'));

      expect(session.getStatus()).toBe('error');
    });

    it('should throw when pausing non-active session', async () => {
      const session = new SessionEntity({ status: 'paused' });

      await expect(session.pause()).rejects.toThrow('Cannot pause');
    });

    it('should throw when resuming non-paused session', async () => {
      const session = new SessionEntity({ status: 'active' });

      await expect(session.resume()).rejects.toThrow('Cannot resume');
    });
  });

  describe('Conversation History', () => {
    it('should add user message', async () => {
      const session = new SessionEntity({});

      await session.addUserMessage('Hello!');

      const history = session.getConversationHistory();
      expect(history).toHaveLength(1);
      expect(history[0].role).toBe('user');
      expect(history[0].content).toBe('Hello!');
    });

    it('should add assistant message', async () => {
      const session = new SessionEntity({});

      await session.addAssistantMessage('Hi there!', 'agent-001');

      const history = session.getConversationHistory();
      expect(history).toHaveLength(1);
      expect(history[0].role).toBe('assistant');
      expect(history[0].agentId).toBe('agent-001');
    });

    it('should add system message', async () => {
      const session = new SessionEntity({});

      await session.addSystemMessage('System initialized');

      const history = session.getConversationHistory();
      expect(history).toHaveLength(1);
      expect(history[0].role).toBe('system');
    });

    it('should get last message', async () => {
      const session = new SessionEntity({});

      await session.addUserMessage('First');
      await session.addUserMessage('Second');
      await session.addAssistantMessage('Third');

      const lastMessage = session.getLastMessage();
      expect(lastMessage?.content).toBe('Third');
      expect(lastMessage?.role).toBe('assistant');
    });

    it('should return undefined for empty history', () => {
      const session = new SessionEntity({});
      expect(session.getLastMessage()).toBeUndefined();
    });

    it('should increment message count', async () => {
      const session = new SessionEntity({});

      await session.addUserMessage('Message 1');
      await session.addAssistantMessage('Message 2');

      expect(session.getMessageCount()).toBe(2);
    });

    it('should update last active time on message', async () => {
      const session = new SessionEntity({});
      const before = session.getMetadata().lastActiveAt;

      await new Promise(resolve => setTimeout(resolve, 10));
      await session.addUserMessage('Test');

      const after = session.getMetadata().lastActiveAt;
      expect(after.getTime()).toBeGreaterThan(before.getTime());
    });

    it('should throw when adding message to non-active session', async () => {
      const session = new SessionEntity({ status: 'completed' });

      await expect(session.addUserMessage('Test'))
        .rejects.toThrow('Cannot add message');
    });

    it('should clear history', async () => {
      const session = new SessionEntity({});
      await session.addUserMessage('Test 1');
      await session.addUserMessage('Test 2');

      session.clearHistory();

      expect(session.getConversationHistory()).toHaveLength(0);
      expect(session.getMessageCount()).toBe(0);
    });
  });

  describe('Agent Management', () => {
    it('should attach agent', async () => {
      const session = new SessionEntity({});

      await session.attachAgent('agent-001');

      expect(session.getActiveAgents()).toContain('agent-001');
    });

    it('should not duplicate attached agent', async () => {
      const session = new SessionEntity({});

      await session.attachAgent('agent-001');
      await session.attachAgent('agent-001');

      expect(session.getActiveAgents()).toEqual(['agent-001']);
    });

    it('should detach agent', async () => {
      const session = new SessionEntity({});
      await session.attachAgent('agent-001');
      await session.attachAgent('agent-002');

      await session.detachAgent('agent-001');

      expect(session.getActiveAgents()).toEqual(['agent-002']);
    });

    it('should handle detaching non-existent agent gracefully', async () => {
      const session = new SessionEntity({});

      await expect(session.detachAgent('non-existent')).resolves.not.toThrow();
    });
  });

  describe('Variables', () => {
    it('should set and get variable', () => {
      const session = new SessionEntity({});

      session.setVariable('key', 'value');

      expect(session.getVariable('key')).toBe('value');
    });

    it('should handle complex variable values', () => {
      const session = new SessionEntity({});
      const complexValue = { nested: { data: [1, 2, 3] } };

      session.setVariable('complex', complexValue);

      expect(session.getVariable('complex')).toEqual(complexValue);
    });

    it('should delete variable', () => {
      const session = new SessionEntity({});
      session.setVariable('toDelete', 'value');

      const deleted = session.deleteVariable('toDelete');

      expect(deleted).toBe(true);
      expect(session.getVariable('toDelete')).toBeUndefined();
    });

    it('should return false when deleting non-existent variable', () => {
      const session = new SessionEntity({});

      const deleted = session.deleteVariable('nonExistent');

      expect(deleted).toBe(false);
    });
  });

  describe('Working Directory', () => {
    it('should get default working directory', () => {
      const session = new SessionEntity({});
      expect(session.getWorkingDirectory()).toBe('/workspace');
    });

    it('should set working directory', () => {
      const session = new SessionEntity({});

      session.setWorkingDirectory('/new/path');

      expect(session.getWorkingDirectory()).toBe('/new/path');
    });
  });

  describe('Artifacts', () => {
    it('should add artifact', () => {
      const session = new SessionEntity({});

      session.addArtifact({
        id: 'artifact-001',
        type: 'code',
        content: 'console.log("Hello")'
      });

      const artifact = session.getArtifact('artifact-001');
      expect(artifact).toBeDefined();
      expect(artifact?.type).toBe('code');
      expect(artifact?.content).toBe('console.log("Hello")');
    });

    it('should list all artifacts', () => {
      const session = new SessionEntity({});

      session.addArtifact({ id: 'a1', type: 'code', content: 'code' });
      session.addArtifact({ id: 'a2', type: 'file', content: 'file' });

      const artifacts = session.listArtifacts();
      expect(artifacts).toHaveLength(2);
    });

    it('should return undefined for non-existent artifact', () => {
      const session = new SessionEntity({});
      expect(session.getArtifact('non-existent')).toBeUndefined();
    });
  });

  describe('Token Usage', () => {
    it('should update token usage', () => {
      const session = new SessionEntity({});

      session.updateTokenUsage(1000, 0.01);

      const metadata = session.getMetadata();
      expect(metadata.tokenUsage).toBe(1000);
      expect(metadata.estimatedCost).toBe(0.01);
    });

    it('should accumulate token usage', () => {
      const session = new SessionEntity({});

      session.updateTokenUsage(500, 0.005);
      session.updateTokenUsage(500, 0.005);

      const metadata = session.getMetadata();
      expect(metadata.tokenUsage).toBe(1000);
      expect(metadata.estimatedCost).toBeCloseTo(0.01, 5);
    });
  });

  describe('Event Logging', () => {
    it('should log events during lifecycle', async () => {
      const session = new SessionEntity({});

      await session.addUserMessage('Hello');
      await session.attachAgent('agent-001');
      await session.pause();
      await session.resume();

      const events = session.getEventLog();
      expect(events.length).toBeGreaterThanOrEqual(4);
      expect(events.some(e => e.type === 'message_added')).toBe(true);
      expect(events.some(e => e.type === 'agent_attached')).toBe(true);
      expect(events.some(e => e.type === 'paused')).toBe(true);
      expect(events.some(e => e.type === 'resumed')).toBe(true);
    });
  });

  describe('Serialization', () => {
    it('should serialize to JSON', async () => {
      const session = new SessionEntity({
        id: 'session-001',
        tenantId: 'tenant-001'
      });
      await session.addUserMessage('Test');
      session.setVariable('key', 'value');

      const json = session.toJSON();

      expect(json.id).toBe('session-001');
      expect(json.tenantId).toBe('tenant-001');
      expect(json.context.conversationHistory).toHaveLength(1);
    });
  });
});

describe('Session Factory Integration', () => {
  it('should create session from factory data', () => {
    const factorySession = createSession({
      tenantId: 'tenant-factory',
      userId: 'user-factory'
    });

    const session = new SessionEntity({
      id: factorySession.id,
      tenantId: factorySession.tenantId,
      userId: factorySession.userId,
      channelId: factorySession.channelId,
      threadTs: factorySession.threadTs,
      context: {
        conversationHistory: factorySession.context.conversationHistory,
        workingDirectory: factorySession.context.workingDirectory,
        activeAgents: factorySession.context.activeAgents
      }
    });

    expect(session.getTenantId()).toBe('tenant-factory');
    expect(session.getUserId()).toBe('user-factory');
  });

  it('should create session with history from factory', () => {
    const factorySession = createSessionWithHistory(5);

    const session = new SessionEntity({
      id: factorySession.id,
      context: {
        conversationHistory: factorySession.context.conversationHistory
      },
      metadata: {
        messageCount: factorySession.metadata.messageCount
      }
    });

    expect(session.getConversationHistory()).toHaveLength(5);
    expect(session.getMessageCount()).toBe(5);
  });
});
