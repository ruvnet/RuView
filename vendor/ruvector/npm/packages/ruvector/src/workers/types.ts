/**
 * Native Worker Types for RuVector
 *
 * Deep integration with ONNX embeddings, VectorDB, and intelligence engine.
 */

export interface WorkerConfig {
  name: string;
  description?: string;
  phases: PhaseConfig[];
  capabilities?: WorkerCapabilities;
  timeout?: number;
  parallel?: boolean;
}

export interface PhaseConfig {
  type: PhaseType;
  config?: Record<string, any>;
}

export type PhaseType =
  | 'file-discovery'
  | 'pattern-extraction'
  | 'embedding-generation'
  | 'vector-storage'
  | 'similarity-search'
  | 'security-scan'
  | 'complexity-analysis'
  | 'summarization';

export interface WorkerCapabilities {
  onnxEmbeddings?: boolean;
  vectorDb?: boolean;
  intelligenceMemory?: boolean;
  parallelProcessing?: boolean;
}

export interface PhaseResult {
  phase: PhaseType;
  success: boolean;
  data: any;
  timeMs: number;
  error?: string;
}

export interface WorkerResult {
  worker: string;
  success: boolean;
  phases: PhaseResult[];
  totalTimeMs: number;
  summary?: WorkerSummary;
}

export interface WorkerSummary {
  filesAnalyzed: number;
  patternsFound: number;
  embeddingsGenerated: number;
  vectorsStored: number;
  findings: Finding[];
}

export interface Finding {
  type: 'info' | 'warning' | 'error' | 'security';
  message: string;
  file?: string;
  line?: number;
  severity?: number;
}

export interface BenchmarkResult {
  name: string;
  iterations: number;
  results: {
    min: number;
    max: number;
    avg: number;
    p50: number;
    p95: number;
    p99: number;
  };
  throughput?: {
    itemsPerSecond: number;
    mbPerSecond?: number;
  };
}
