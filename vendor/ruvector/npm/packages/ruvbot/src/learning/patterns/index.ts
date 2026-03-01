/**
 * Patterns Module - HNSW-indexed pattern matching
 */

export interface PatternIndex {
  initialize(): Promise<void>;
  findMatches(query: Float32Array, options?: PatternMatchOptions): Promise<PatternMatch[]>;
  addPattern(pattern: LearnedPattern): Promise<void>;
  updateStats(patternId: string, used: boolean, successful: boolean): Promise<void>;
  deactivate(patternId: string, reason: string): Promise<void>;
}

export interface LearnedPattern {
  id: string;
  tenantId: string;
  workspaceId?: string;
  patternType: PatternType;
  embedding: Float32Array;
  exemplarTrajectoryIds: string[];
  suggestedResponse?: string;
  suggestedSkills?: string[];
  confidence: number;
  usageCount: number;
  successCount: number;
  successRate: number;
  isActive: boolean;
  createdAt: Date;
  lastUsedAt: Date;
  supersededBy?: string;
}

export type PatternType =
  | 'response'
  | 'skill_selection'
  | 'memory_retrieval'
  | 'conversation_flow';

export interface PatternMatchOptions {
  limit?: number;
  threshold?: number;
  patternTypes?: PatternType[];
  activeOnly?: boolean;
}

export interface PatternMatch {
  pattern: LearnedPattern;
  score: number;
  rawSimilarity: number;
}

export interface PatternOptimizer {
  analyze(patterns: LearnedPattern[]): OptimizationReport;
  prune(threshold: number): Promise<PruneResult>;
  merge(patterns: string[]): Promise<LearnedPattern>;
  cluster(patterns: LearnedPattern[]): Promise<PatternCluster[]>;
}

export interface OptimizationReport {
  totalPatterns: number;
  activePatterns: number;
  lowConfidenceCount: number;
  duplicateCandidates: string[][];
  recommendations: string[];
}

export interface PruneResult {
  prunedCount: number;
  prunedPatternIds: string[];
  reason: string;
}

export interface PatternCluster {
  centroid: Float32Array;
  members: LearnedPattern[];
  cohesion: number;
}
