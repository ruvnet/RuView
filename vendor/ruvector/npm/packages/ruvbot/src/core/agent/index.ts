/**
 * Agent Module - Conversation management and response generation
 */

export interface Agent {
  id: string;
  tenantId: string;
  name: string;
  persona: AgentPersona;
  skills: string[];
  memoryConfig: MemoryConfiguration;
  status: AgentStatus;
}

export interface AgentPersona {
  systemPrompt: string;
  temperature: number;
  maxTokens: number;
  traits: PersonalityTrait[];
  constraints: ResponseConstraint[];
}

export interface PersonalityTrait {
  name: string;
  weight: number;
}

export interface ResponseConstraint {
  type: 'must_include' | 'must_exclude' | 'format';
  value: string;
}

export interface MemoryConfiguration {
  episodicEnabled: boolean;
  semanticEnabled: boolean;
  proceduralEnabled: boolean;
  maxMemoriesPerType: number;
  importanceThreshold: number;
}

export type AgentStatus = 'active' | 'paused' | 'disabled';

export interface ConversationTurn {
  id: string;
  sessionId: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  embeddingId?: string;
  metadata: TurnMetadata;
  timestamp: Date;
}

export interface TurnMetadata {
  tokenCount?: number;
  latencyMs?: number;
  skillsInvoked?: string[];
  memoriesRetrieved?: string[];
}
