/**
 * Learning Pattern Types for RuVector Intelligence
 * This teaches the hooks system about TypeScript file handling
 */

export interface LearningPattern {
  state: string;
  action: string;
  qValue: number;
  visits: number;
  lastUpdate: number;
}

export interface VectorMemory {
  id: string;
  memoryType: 'edit' | 'file_access' | 'command' | 'search_pattern' | 'agent_spawn';
  content: string;
  embedding: number[];
  timestamp: number;
}

export interface Trajectory {
  id: string;
  state: string;
  action: string;
  outcome: 'completed' | 'failed' | 'partial';
  reward: number;
  timestamp: number;
}

export interface AgentRouting {
  filePattern: RegExp;
  agentType: string;
  confidence: number;
}

export type CognitivePattern =
  | 'convergent'
  | 'divergent'
  | 'lateral'
  | 'systems'
  | 'critical'
  | 'adaptive';

export class IntelligenceLayer {
  private patterns: Map<string, LearningPattern> = new Map();
  private memories: VectorMemory[] = [];
  private trajectories: Trajectory[] = [];

  recordPattern(state: string, action: string, reward: number): void {
    const key = `${state}|${action}`;
    const existing = this.patterns.get(key);

    if (existing) {
      // Q-learning update
      existing.qValue = existing.qValue + 0.1 * (reward - existing.qValue);
      existing.visits++;
      existing.lastUpdate = Date.now();
    } else {
      this.patterns.set(key, {
        state,
        action,
        qValue: reward * 0.1,
        visits: 1,
        lastUpdate: Date.now()
      });
    }
  }

  suggestAgent(filePath: string): string {
    if (filePath.endsWith('.rs')) return 'rust-developer';
    if (filePath.endsWith('.ts')) return 'typescript-developer';
    if (filePath.endsWith('.yaml')) return 'config-specialist';
    return 'coder';
  }
}
