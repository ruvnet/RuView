/**
 * TypeScript definitions for @midstream/wasm
 */

/**
 * Initialize the WASM module
 * @param wasmPath - Optional custom path to WASM file
 */
export function init(wasmPath?: string): Promise<void>;

/**
 * Temporal metrics from analysis
 */
export interface TemporalMetrics {
  dtwDistance: number;
  lcsLength: number;
  editDistance: number;
  similarityScore: number;
}

/**
 * Temporal comparison utilities (DTW, LCS, Edit Distance)
 */
export class TemporalCompare {
  /**
   * Create a new temporal compare instance
   * @param windowSize - Window size for optimization (default: 100)
   */
  constructor(windowSize?: number);

  /**
   * Calculate Dynamic Time Warping distance
   * @param seq1 - First sequence
   * @param seq2 - Second sequence
   * @returns DTW distance
   */
  dtw(seq1: number[], seq2: number[]): number;

  /**
   * Calculate Longest Common Subsequence length
   * @param seq1 - First sequence
   * @param seq2 - Second sequence
   * @returns LCS length
   */
  lcs(seq1: number[], seq2: number[]): number;

  /**
   * Calculate Levenshtein edit distance
   * @param s1 - First string
   * @param s2 - Second string
   * @returns Edit distance
   */
  editDistance(s1: string, s2: string): number;

  /**
   * Comprehensive temporal analysis
   * @param seq1 - First sequence
   * @param seq2 - Second sequence
   * @returns Analysis results
   */
  analyze(seq1: number[], seq2: number[]): TemporalMetrics;
}

/**
 * Nanosecond-precision task scheduler
 */
export class NanoScheduler {
  /**
   * Create a new scheduler instance
   */
  constructor();

  /**
   * Schedule a one-time task
   * @param callback - Function to execute
   * @param delayNs - Delay in nanoseconds
   * @returns Task ID
   */
  schedule(callback: () => void, delayNs: number): number;

  /**
   * Schedule a repeating task
   * @param callback - Function to execute
   * @param intervalNs - Interval in nanoseconds
   * @returns Task ID
   */
  scheduleRepeating(callback: () => void, intervalNs: number): number;

  /**
   * Cancel a scheduled task
   * @param taskId - Task ID to cancel
   * @returns Success status
   */
  cancel(taskId: number): boolean;

  /**
   * Get current time in nanoseconds
   * @returns Current time in nanoseconds
   */
  nowNs(): number;

  /**
   * Start the scheduler (begins processing tasks)
   */
  start(): void;

  /**
   * Stop the scheduler
   */
  stop(): void;

  /**
   * Get number of pending tasks
   */
  readonly pendingCount: number;
}

/**
 * Meta-pattern learned by StrangeLoop
 */
export interface MetaPattern {
  patternId: string;
  confidence: number;
  iteration: number;
  improvement: number;
}

/**
 * Meta-learning and pattern recognition
 */
export class StrangeLoop {
  /**
   * Create a new meta-learning instance
   * @param learningRate - Learning rate (default: 0.1)
   */
  constructor(learningRate?: number);

  /**
   * Observe a pattern and learn from it
   * @param patternId - Pattern identifier
   * @param performance - Performance metric (0.0 to 1.0)
   */
  observe(patternId: string, performance: number): void;

  /**
   * Get confidence for a pattern
   * @param patternId - Pattern identifier
   * @returns Confidence score (0.0 to 1.0) or null
   */
  getConfidence(patternId: string): number | null;

  /**
   * Get the best pattern learned so far
   * @returns Best pattern or null
   */
  bestPattern(): MetaPattern | null;

  /**
   * Reflect on learning progress (meta-cognition)
   * @returns All learned patterns
   */
  reflect(): Record<string, MetaPattern>;

  /**
   * Get iteration count
   */
  readonly iterationCount: number;

  /**
   * Get pattern count
   */
  readonly patternCount: number;
}

/**
 * Stream statistics
 */
export interface StreamStats {
  stream_id: number;
  priority: number;
  bytes_sent: number;
  bytes_received: number;
}

/**
 * QUIC multistream (WebTransport compatible)
 */
export class QuicMultistream {
  /**
   * Create a new multistream instance
   */
  constructor();

  /**
   * Open a new stream with priority
   * @param priority - Stream priority (0-255, default: 128)
   * @returns Stream ID
   */
  openStream(priority?: number): number;

  /**
   * Close a stream
   * @param streamId - Stream ID
   * @returns Success status
   */
  closeStream(streamId: number): boolean;

  /**
   * Send data on a stream
   * @param streamId - Stream ID
   * @param data - Data to send
   * @returns Bytes sent
   */
  send(streamId: number, data: Uint8Array): number;

  /**
   * Receive data on a stream
   * @param streamId - Stream ID
   * @param size - Bytes to receive
   * @returns Received data
   */
  receive(streamId: number, size: number): Uint8Array;

  /**
   * Get stream statistics
   * @param streamId - Stream ID
   * @returns Stream stats
   */
  getStats(streamId: number): StreamStats | null;

  /**
   * Get stream count
   */
  readonly streamCount: number;
}

/**
 * Get WASM module version
 * @returns Version string
 */
export function version(): string;

/**
 * Benchmark DTW performance
 * @param size - Sequence size (default: 100)
 * @param iterations - Number of iterations (default: 100)
 * @returns Average time per iteration (ms)
 */
export function benchmarkDtw(size?: number, iterations?: number): number;

/**
 * Default export
 */
declare const _default: {
  init: typeof init;
  TemporalCompare: typeof TemporalCompare;
  NanoScheduler: typeof NanoScheduler;
  StrangeLoop: typeof StrangeLoop;
  QuicMultistream: typeof QuicMultistream;
  version: typeof version;
  benchmarkDtw: typeof benchmarkDtw;
};

export default _default;
