const { EventEmitter } = require('events');
const { Worker, isMainThread, parentPort, workerData } = require('worker_threads');
const { v4: uuidv4 } = require('uuid');
const path = require('path');

class StreamingManager extends EventEmitter {
  constructor(config = {}) {
    super();

    this.config = {
      maxConcurrentStreams: config.maxConcurrentStreams || 100,
      workerPoolSize: config.workers || 1,
      streamTimeout: config.streamTimeout || 300000, // 5 minutes
      heartbeatInterval: config.heartbeatInterval || 15000, // 15 seconds
      ...config
    };

    this.activeStreams = new Map();
    this.workerPool = [];
    this.availableWorkers = [];
    this.jobQueue = [];

    this.initializeWorkerPool();
  }

  initializeWorkerPool() {
    for (let i = 0; i < this.config.workerPoolSize; i++) {
      this.createWorker();
    }
  }

  createWorker() {
    const worker = new Worker(path.join(__dirname, 'solver-worker.js'));

    worker.on('message', (message) => {
      this.handleWorkerMessage(worker, message);
    });

    worker.on('error', (error) => {
      console.error('Worker error:', error);
      this.replaceWorker(worker);
    });

    worker.on('exit', (code) => {
      if (code !== 0) {
        console.error(`Worker stopped with exit code ${code}`);
        this.replaceWorker(worker);
      }
    });

    worker.id = uuidv4();
    this.workerPool.push(worker);
    this.availableWorkers.push(worker);

    return worker;
  }

  replaceWorker(deadWorker) {
    // Remove dead worker
    this.workerPool = this.workerPool.filter(w => w.id !== deadWorker.id);
    this.availableWorkers = this.availableWorkers.filter(w => w.id !== deadWorker.id);

    // Create replacement
    this.createWorker();
  }

  async startSolve(session) {
    if (this.activeStreams.size >= this.config.maxConcurrentStreams) {
      throw new Error('Maximum concurrent streams reached');
    }

    const stream = new SolverStream(session, this);
    this.activeStreams.set(session.id, stream);

    // Set up cleanup timeout
    setTimeout(() => {
      if (this.activeStreams.has(session.id)) {
        this.activeStreams.delete(session.id);
        stream.destroy();
      }
    }, this.config.streamTimeout);

    return stream.start();
  }

  getWorker() {
    if (this.availableWorkers.length === 0) {
      return null;
    }

    return this.availableWorkers.shift();
  }

  releaseWorker(worker) {
    if (this.workerPool.includes(worker)) {
      this.availableWorkers.push(worker);
    }
  }

  handleWorkerMessage(worker, message) {
    const stream = this.activeStreams.get(message.sessionId);
    if (stream) {
      stream.handleWorkerMessage(message);
    }
  }

  stopStream(sessionId) {
    const stream = this.activeStreams.get(sessionId);
    if (stream) {
      stream.stop();
      this.activeStreams.delete(sessionId);
    }
  }

  getStreamStatus(sessionId) {
    const stream = this.activeStreams.get(sessionId);
    return stream ? stream.getStatus() : null;
  }

  getStats() {
    return {
      activeStreams: this.activeStreams.size,
      availableWorkers: this.availableWorkers.length,
      totalWorkers: this.workerPool.length,
      queuedJobs: this.jobQueue.length
    };
  }
}

class SolverStream extends EventEmitter {
  constructor(session, manager) {
    super();

    this.session = session;
    this.manager = manager;
    this.worker = null;
    this.status = 'pending';
    this.startTime = Date.now();
    this.lastUpdate = Date.now();

    this.updates = [];
    this.currentIteration = 0;
    this.currentResidual = Infinity;
    this.converged = false;
    this.error = null;

    // Heartbeat to detect stalled streams
    this.heartbeatTimer = setInterval(() => {
      this.checkHeartbeat();
    }, manager.config.heartbeatInterval);
  }

  async start() {
    try {
      this.status = 'starting';
      this.worker = this.manager.getWorker();

      if (!this.worker) {
        throw new Error('No available workers');
      }

      // Send solve job to worker
      this.worker.postMessage({
        type: 'solve',
        sessionId: this.session.id,
        matrix: this.session.matrix,
        vector: this.session.vector,
        method: this.session.method,
        options: this.session.options
      });

      this.status = 'running';

      return this.createAsyncIterator();

    } catch (error) {
      this.error = error;
      this.status = 'error';
      throw error;
    }
  }

  async *createAsyncIterator() {
    let done = false;

    while (!done) {
      // Wait for next update
      const update = await this.waitForUpdate();

      if (update.type === 'iteration') {
        this.currentIteration = update.iteration;
        this.currentResidual = update.residual;
        this.lastUpdate = Date.now();

        yield {
          iteration: update.iteration,
          residual: update.residual,
          convergence_rate: update.convergenceRate,
          memory_usage: update.memoryUsage,
          estimated_time_remaining: this.estimateTimeRemaining(update),
          verified: update.verified || false,
          timestamp: new Date().toISOString()
        };

      } else if (update.type === 'converged') {
        this.converged = true;
        this.status = 'completed';
        done = true;

        yield {
          iteration: this.currentIteration,
          residual: this.currentResidual,
          converged: true,
          solution: update.solution,
          stats: update.stats,
          timestamp: new Date().toISOString()
        };

      } else if (update.type === 'error') {
        this.error = new Error(update.error);
        this.status = 'error';
        done = true;

        yield {
          error: update.error,
          iteration: this.currentIteration,
          timestamp: new Date().toISOString()
        };
      }
    }

    this.cleanup();
  }

  waitForUpdate() {
    return new Promise((resolve) => {
      this.once('update', resolve);
    });
  }

  handleWorkerMessage(message) {
    this.emit('update', message);
  }

  estimateTimeRemaining(update) {
    if (this.currentIteration === 0) return null;

    const elapsed = Date.now() - this.startTime;
    const iterationsPerMs = this.currentIteration / elapsed;

    if (update.convergenceRate && update.convergenceRate > 0) {
      // Estimate based on convergence rate
      const iterationsToConverge = Math.log(this.session.options.tolerance || 1e-10) /
                                  Math.log(update.convergenceRate);
      const remainingIterations = iterationsToConverge - this.currentIteration;

      return Math.max(0, remainingIterations / iterationsPerMs);
    }

    // Fallback to linear estimation
    const maxIterations = this.session.options.maxIterations || 1000;
    const remainingIterations = maxIterations - this.currentIteration;

    return remainingIterations / iterationsPerMs;
  }

  checkHeartbeat() {
    const timeSinceUpdate = Date.now() - this.lastUpdate;

    if (timeSinceUpdate > this.manager.config.heartbeatInterval * 3) {
      // Stream appears stalled
      console.warn(`Stream ${this.session.id} appears stalled`);
      this.stop();
    }
  }

  stop() {
    if (this.worker) {
      this.worker.postMessage({
        type: 'stop',
        sessionId: this.session.id
      });
    }

    this.status = 'stopped';
    this.cleanup();
  }

  cleanup() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }

    if (this.worker) {
      this.manager.releaseWorker(this.worker);
      this.worker = null;
    }

    this.removeAllListeners();
  }

  destroy() {
    this.stop();
    this.status = 'destroyed';
  }

  getStatus() {
    return {
      status: this.status,
      sessionId: this.session.id,
      currentIteration: this.currentIteration,
      currentResidual: this.currentResidual,
      converged: this.converged,
      error: this.error ? this.error.message : null,
      startTime: this.startTime,
      lastUpdate: this.lastUpdate,
      elapsed: Date.now() - this.startTime
    };
  }
}

class VerificationLoop {
  constructor(options = {}) {
    this.enabled = options.enabled || false;
    this.interval = options.interval || 100;
    this.probeCount = options.probeCount || 10;
    this.tolerance = options.tolerance || 1e-8;

    this.lastVerification = 0;
    this.verificationHistory = [];
  }

  shouldVerify(iteration) {
    if (!this.enabled) return false;
    return iteration - this.lastVerification >= this.interval;
  }

  async verify(matrix, solution, vector) {
    this.lastVerification = Date.now();

    try {
      // Generate random probes
      const probes = this.generateProbes(solution.length);
      const errors = [];

      // Test each probe
      for (const probe of probes) {
        const computed = this.computeMatrixVectorProbe(matrix, solution, probe.indices);
        const expected = probe.indices.map(idx => vector[idx]);

        const error = this.computeError(computed, expected);
        errors.push(error);
      }

      const maxError = Math.max(...errors);
      const meanError = errors.reduce((a, b) => a + b) / errors.length;
      const verified = maxError < this.tolerance;

      const result = {
        verified,
        maxError,
        meanError,
        probeCount: probes.length,
        timestamp: new Date().toISOString()
      };

      this.verificationHistory.push(result);

      // Keep only recent history
      if (this.verificationHistory.length > 100) {
        this.verificationHistory.shift();
      }

      return result;

    } catch (error) {
      console.error('Verification error:', error);
      return {
        verified: false,
        error: error.message,
        timestamp: new Date().toISOString()
      };
    }
  }

  generateProbes(vectorLength) {
    const probes = [];

    for (let i = 0; i < this.probeCount; i++) {
      const probeSize = Math.min(50, Math.floor(vectorLength * 0.1));
      const indices = [];

      for (let j = 0; j < probeSize; j++) {
        const idx = Math.floor(Math.random() * vectorLength);
        if (!indices.includes(idx)) {
          indices.push(idx);
        }
      }

      probes.push({ indices: indices.sort() });
    }

    return probes;
  }

  computeMatrixVectorProbe(matrix, vector, indices) {
    // Simplified matrix-vector multiplication for probe indices
    const result = [];

    for (const rowIdx of indices) {
      let sum = 0;

      if (matrix.format === 'coo') {
        // Coordinate format
        for (let i = 0; i < matrix.data.values.length; i++) {
          if (matrix.data.rowIndices[i] === rowIdx) {
            const col = matrix.data.colIndices[i];
            const val = matrix.data.values[i];
            sum += val * vector[col];
          }
        }
      } else if (matrix.format === 'dense') {
        // Dense format
        for (let col = 0; col < matrix.cols; col++) {
          sum += matrix.data[rowIdx][col] * vector[col];
        }
      }

      result.push(sum);
    }

    return result;
  }

  computeError(computed, expected) {
    let maxError = 0;

    for (let i = 0; i < computed.length; i++) {
      const error = Math.abs(computed[i] - expected[i]);
      maxError = Math.max(maxError, error);
    }

    return maxError;
  }

  getVerificationStats() {
    if (this.verificationHistory.length === 0) {
      return null;
    }

    const recent = this.verificationHistory.slice(-10);
    const verificationRate = recent.filter(v => v.verified).length / recent.length;
    const avgError = recent.reduce((sum, v) => sum + (v.meanError || 0), 0) / recent.length;

    return {
      verificationRate,
      avgError,
      recentVerifications: recent.length,
      totalVerifications: this.verificationHistory.length
    };
  }
}

// Session management utilities
class SessionManager {
  constructor(config = {}) {
    this.sessions = new Map();
    this.config = config;
  }

  async createSession(sessionId, sessionData) {
    const session = {
      id: sessionId,
      ...sessionData,
      status: 'created',
      createdAt: new Date().toISOString(),
      lastActivity: new Date().toISOString(),
      metrics: {
        iterations: 0,
        residual: Infinity,
        memoryUsage: 0
      }
    };

    this.sessions.set(sessionId, session);
    return session;
  }

  getSession(sessionId) {
    return this.sessions.get(sessionId);
  }

  updateSession(sessionId, updates) {
    const session = this.sessions.get(sessionId);
    if (session) {
      Object.assign(session, updates);
      session.lastActivity = new Date().toISOString();
    }
  }

  deleteSession(sessionId) {
    this.sessions.delete(sessionId);
  }

  getStats() {
    return {
      totalSessions: this.sessions.size,
      activeSessions: Array.from(this.sessions.values())
        .filter(s => s.status === 'running').length
    };
  }
}

module.exports = {
  StreamingManager,
  SolverStream,
  VerificationLoop,
  SessionManager
};