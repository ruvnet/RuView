/**
 * Type definitions for agentic-synth-examples
 */

export enum ModelProvider {
  GEMINI = 'gemini',
  CLAUDE = 'claude',
  GPT4 = 'gpt4',
  LLAMA = 'llama'
}

export interface ModelConfig {
  provider: ModelProvider;
  model: string;
  apiKey: string;
  temperature?: number;
  maxTokens?: number;
}

export interface TrainingResult {
  modelProvider: ModelProvider;
  model: string;
  iteration: number;
  quality: {
    score: number;
    metrics: Record<string, number>;
  };
  cost: number;
  duration: number;
  timestamp: Date;
}

export interface TrainingReport {
  bestModel: string;
  bestProvider: ModelProvider;
  bestScore: number;
  qualityImprovement: number;
  totalCost: number;
  totalDuration: number;
  iterations: number;
  results: TrainingResult[];
}

export interface BenchmarkResult {
  provider: ModelProvider;
  model: string;
  task: string;
  score: number;
  latency: number;
  cost: number;
  tokensUsed: number;
}

export interface LearningMetrics {
  iteration: number;
  quality: number;
  testsPassingRate?: number;
  improvement: number;
  feedback: string[];
}

export interface StockDataPoint {
  symbol: string;
  date: Date;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  sentiment?: number;
  news?: string[];
}

export interface EventEmitter {
  on(event: string, listener: (...args: any[]) => void): void;
  emit(event: string, ...args: any[]): void;
  off(event: string, listener: (...args: any[]) => void): void;
}
