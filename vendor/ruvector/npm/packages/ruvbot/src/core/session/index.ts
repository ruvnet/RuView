/**
 * Session Module - State machine and context window management
 */

export interface Session {
  id: string;
  agentId: string;
  userId: string;
  tenantId: string;
  workspaceId: string;
  state: SessionState;
  contextWindow: ContextWindow;
  channel: SessionChannel;
  channelId?: string;
  createdAt: Date;
  lastActiveAt: Date;
  expiresAt: Date;
}

export type SessionState =
  | { type: 'idle' }
  | { type: 'processing'; turnId: string }
  | { type: 'awaiting_input'; prompt: string }
  | { type: 'executing_skill'; skillId: string; step: number }
  | { type: 'terminated'; reason: string };

export interface ContextWindow {
  maxTokens: number;
  currentTokens: number;
  turns: ContextTurn[];
  retrievedMemories: RetrievedMemory[];
  activeSkillContext: SkillContext | null;
}

export interface ContextTurn {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  tokenCount: number;
}

export interface RetrievedMemory {
  memoryId: string;
  content: string;
  relevanceScore: number;
  memoryType: 'episodic' | 'semantic' | 'procedural';
}

export interface SkillContext {
  skillId: string;
  state: Record<string, unknown>;
  step: number;
}

export type SessionChannel = 'api' | 'slack' | 'webhook' | 'cli';
