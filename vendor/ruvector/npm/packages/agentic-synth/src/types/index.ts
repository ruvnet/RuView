/**
 * Core type definitions for agentic-synth
 */

export interface GenerationResult {
  data: string;
  tokensUsed: number;
  latencyMs: number;
  cached: boolean;
  modelUsed: string;
  timestamp: number;
}

export interface BatchGenerationResult {
  results: GenerationResult[];
  totalTokens: number;
  avgLatencyMs: number;
  cacheHitRate: number;
  totalDurationMs: number;
}

export interface StreamingResult {
  chunks: string[];
  totalChunks: number;
  totalTokens: number;
  streamDurationMs: number;
  firstChunkLatencyMs: number;
}

export interface CachedResult {
  hit: boolean;
  key: string;
  data?: string;
  ttl?: number;
}

export interface PerformanceMetrics {
  throughput: number; // requests per second
  p50LatencyMs: number;
  p95LatencyMs: number;
  p99LatencyMs: number;
  avgLatencyMs: number;
  cacheHitRate: number;
  memoryUsageMB: number;
  cpuUsagePercent: number;
  concurrentRequests: number;
  errorRate: number;
}

export interface OptimizationRecommendation {
  category: 'cache' | 'routing' | 'memory' | 'concurrency' | 'compilation';
  severity: 'low' | 'medium' | 'high' | 'critical';
  issue: string;
  recommendation: string;
  estimatedImprovement: string;
  implementationEffort: 'low' | 'medium' | 'high';
}

export interface BenchmarkConfig {
  name: string;
  iterations: number;
  concurrency: number;
  warmupIterations: number;
  timeout: number;
  outputPath?: string;
}

export interface BenchmarkResult {
  config: BenchmarkConfig;
  metrics: PerformanceMetrics;
  recommendations: OptimizationRecommendation[];
  timestamp: number;
  duration: number;
  success: boolean;
}
