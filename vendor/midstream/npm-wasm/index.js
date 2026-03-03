/**
 * @midstream/wasm - WebAssembly bindings for Midstream
 *
 * Browser and Node.js compatible wrapper for temporal comparison,
 * nanosecond scheduling, meta-learning, and QUIC multistream.
 */

let wasm;
let initialized = false;

/**
 * Initialize the WASM module
 * @param {string} [wasmPath] - Optional custom path to WASM file
 * @returns {Promise<void>}
 */
async function init(wasmPath) {
  if (initialized) {
    return;
  }

  try {
    // Detect environment
    const isNode = typeof process !== 'undefined' && process.versions && process.versions.node;
    const isBrowser = typeof window !== 'undefined';

    if (isBrowser) {
      // Browser environment - use bundler target (works in browsers)
      const wasmModule = await import('./pkg-bundler/midstream_wasm.js');
      await wasmModule.default();
      wasm = wasmModule;
    } else if (isNode) {
      // Node.js environment - use nodejs target (package.json uses pkg-node)
      const wasmModule = await import('./pkg-node/midstream_wasm.js');
      wasm = wasmModule;
    } else {
      throw new Error('Unsupported environment');
    }

    initialized = true;
    console.log('[Midstream WASM] Initialized successfully');
  } catch (error) {
    console.error('[Midstream WASM] Initialization failed:', error);
    throw error;
  }
}

/**
 * Ensure WASM is initialized
 * @private
 */
function ensureInitialized() {
  if (!initialized) {
    throw new Error('WASM not initialized. Call init() first.');
  }
}

// ============================================================================
// TEMPORAL COMPARISON API
// ============================================================================

/**
 * Temporal comparison utilities (DTW, LCS, Edit Distance)
 */
class TemporalCompare {
  constructor(windowSize = 100) {
    ensureInitialized();
    this.instance = new wasm.TemporalCompare(windowSize);
  }

  /**
   * Calculate Dynamic Time Warping distance
   * @param {number[]} seq1 - First sequence
   * @param {number[]} seq2 - Second sequence
   * @returns {number} DTW distance
   */
  dtw(seq1, seq2) {
    return this.instance.dtw(new Float64Array(seq1), new Float64Array(seq2));
  }

  /**
   * Calculate Longest Common Subsequence length
   * @param {number[]} seq1 - First sequence
   * @param {number[]} seq2 - Second sequence
   * @returns {number} LCS length
   */
  lcs(seq1, seq2) {
    return this.instance.lcs(new Int32Array(seq1), new Int32Array(seq2));
  }

  /**
   * Calculate Levenshtein edit distance
   * @param {string} s1 - First string
   * @param {string} s2 - Second string
   * @returns {number} Edit distance
   */
  editDistance(s1, s2) {
    return this.instance.edit_distance(s1, s2);
  }

  /**
   * Comprehensive temporal analysis
   * @param {number[]} seq1 - First sequence
   * @param {number[]} seq2 - Second sequence
   * @returns {Object} Analysis results with dtw, lcs, edit distance, and similarity
   */
  analyze(seq1, seq2) {
    const result = this.instance.analyze(new Float64Array(seq1), new Float64Array(seq2));
    return {
      dtwDistance: result.dtw_distance,
      lcsLength: result.lcs_length,
      editDistance: result.edit_distance,
      similarityScore: result.similarity_score
    };
  }
}

// ============================================================================
// NANOSECOND SCHEDULER API
// ============================================================================

/**
 * Nanosecond-precision task scheduler
 */
class NanoScheduler {
  constructor() {
    ensureInitialized();
    this.instance = new wasm.NanoScheduler();
    this.animationFrameId = null;
    this.running = false;
  }

  /**
   * Schedule a one-time task
   * @param {Function} callback - Function to execute
   * @param {number} delayNs - Delay in nanoseconds
   * @returns {number} Task ID
   */
  schedule(callback, delayNs) {
    return this.instance.schedule(callback, delayNs);
  }

  /**
   * Schedule a repeating task
   * @param {Function} callback - Function to execute
   * @param {number} intervalNs - Interval in nanoseconds
   * @returns {number} Task ID
   */
  scheduleRepeating(callback, intervalNs) {
    return this.instance.schedule_repeating(callback, intervalNs);
  }

  /**
   * Cancel a scheduled task
   * @param {number} taskId - Task ID to cancel
   * @returns {boolean} Success status
   */
  cancel(taskId) {
    return this.instance.cancel(taskId);
  }

  /**
   * Get current time in nanoseconds
   * @returns {number} Current time in nanoseconds
   */
  nowNs() {
    return this.instance.now_ns();
  }

  /**
   * Start the scheduler (begins processing tasks)
   */
  start() {
    if (this.running) return;
    this.running = true;

    const tick = () => {
      if (!this.running) return;
      this.instance.tick();
      this.animationFrameId = requestAnimationFrame(tick);
    };

    tick();
  }

  /**
   * Stop the scheduler
   */
  stop() {
    this.running = false;
    if (this.animationFrameId !== null) {
      cancelAnimationFrame(this.animationFrameId);
      this.animationFrameId = null;
    }
  }

  /**
   * Get number of pending tasks
   * @returns {number} Pending task count
   */
  get pendingCount() {
    return this.instance.pending_count;
  }
}

// ============================================================================
// STRANGE LOOP META-LEARNING API
// ============================================================================

/**
 * Meta-learning and pattern recognition
 */
class StrangeLoop {
  constructor(learningRate = 0.1) {
    ensureInitialized();
    this.instance = new wasm.StrangeLoop(learningRate);
  }

  /**
   * Observe a pattern and learn from it
   * @param {string} patternId - Pattern identifier
   * @param {number} performance - Performance metric (0.0 to 1.0)
   */
  observe(patternId, performance) {
    this.instance.observe(patternId, performance);
  }

  /**
   * Get confidence for a pattern
   * @param {string} patternId - Pattern identifier
   * @returns {number|null} Confidence score (0.0 to 1.0)
   */
  getConfidence(patternId) {
    return this.instance.get_confidence(patternId);
  }

  /**
   * Get the best pattern learned so far
   * @returns {Object|null} Best pattern with id, confidence, iteration, improvement
   */
  bestPattern() {
    const pattern = this.instance.best_pattern();
    if (!pattern) return null;

    return {
      patternId: pattern.pattern_id,
      confidence: pattern.confidence,
      iteration: pattern.iteration,
      improvement: pattern.improvement
    };
  }

  /**
   * Reflect on learning progress (meta-cognition)
   * @returns {Object} All learned patterns
   */
  reflect() {
    return this.instance.reflect();
  }

  /**
   * Get iteration count
   * @returns {number} Total iterations
   */
  get iterationCount() {
    return this.instance.iteration_count;
  }

  /**
   * Get pattern count
   * @returns {number} Number of learned patterns
   */
  get patternCount() {
    return this.instance.pattern_count;
  }
}

// ============================================================================
// QUIC MULTISTREAM API
// ============================================================================

/**
 * QUIC multistream (WebTransport compatible)
 */
class QuicMultistream {
  constructor() {
    ensureInitialized();
    this.instance = new wasm.QuicMultistream();
  }

  /**
   * Open a new stream with priority
   * @param {number} priority - Stream priority (0-255)
   * @returns {number} Stream ID
   */
  openStream(priority = 128) {
    return this.instance.open_stream(priority);
  }

  /**
   * Close a stream
   * @param {number} streamId - Stream ID
   * @returns {boolean} Success status
   */
  closeStream(streamId) {
    return this.instance.close_stream(streamId);
  }

  /**
   * Send data on a stream
   * @param {number} streamId - Stream ID
   * @param {Uint8Array} data - Data to send
   * @returns {number} Bytes sent
   */
  send(streamId, data) {
    return this.instance.send(streamId, data);
  }

  /**
   * Receive data on a stream
   * @param {number} streamId - Stream ID
   * @param {number} size - Bytes to receive
   * @returns {Uint8Array} Received data
   */
  receive(streamId, size) {
    return this.instance.receive(streamId, size);
  }

  /**
   * Get stream statistics
   * @param {number} streamId - Stream ID
   * @returns {Object} Stream stats
   */
  getStats(streamId) {
    return this.instance.get_stats(streamId);
  }

  /**
   * Get stream count
   * @returns {number} Number of active streams
   */
  get streamCount() {
    return this.instance.stream_count;
  }
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/**
 * Get WASM module version
 * @returns {string} Version string
 */
function version() {
  ensureInitialized();
  return wasm.version();
}

/**
 * Benchmark DTW performance
 * @param {number} size - Sequence size
 * @param {number} iterations - Number of iterations
 * @returns {number} Average time per iteration (ms)
 */
function benchmarkDtw(size = 100, iterations = 100) {
  ensureInitialized();
  return wasm.benchmark_dtw(size, iterations);
}

// ============================================================================
// EXPORTS
// ============================================================================

export {
  init,
  TemporalCompare,
  NanoScheduler,
  StrangeLoop,
  QuicMultistream,
  version,
  benchmarkDtw
};

// Default export for convenience
export default {
  init,
  TemporalCompare,
  NanoScheduler,
  StrangeLoop,
  QuicMultistream,
  version,
  benchmarkDtw
};
