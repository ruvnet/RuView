const { EventEmitter } = require('events');
const { v4: uuidv4 } = require('uuid');
const { StreamingManager } = require('./streaming');

class SessionManager extends EventEmitter {
  constructor(config = {}) {
    super();

    this.config = {
      maxSessions: config.maxSessions || 100,
      sessionTimeout: config.sessionTimeout || 3600000, // 1 hour
      cleanupInterval: config.cleanupInterval || 300000, // 5 minutes
      ...config
    };

    this.sessions = new Map();
    this.jobQueue = [];
    this.streaming = new StreamingManager(config);

    // Start cleanup timer
    this.cleanupTimer = setInterval(() => {
      this.cleanupStaleSessions();
    }, this.config.cleanupInterval);
  }

  async createSession(sessionId, sessionData) {
    if (this.sessions.size >= this.config.maxSessions) {
      throw new Error('Maximum number of sessions reached');
    }

    const session = {
      id: sessionId,
      ...sessionData,
      status: 'created',
      createdAt: new Date().toISOString(),
      lastActivity: new Date().toISOString(),
      metrics: {
        iterations: 0,
        residual: Infinity,
        memoryUsage: 0,
        convergenceRate: null
      },
      swarmNodes: new Set(),
      verificationHistory: [],
      costUpdates: []
    };

    this.sessions.set(sessionId, session);
    this.emit('session_created', session);

    console.log(`Session created: ${sessionId}`);
    return session;
  }

  getSession(sessionId) {
    const session = this.sessions.get(sessionId);
    if (session) {
      session.lastActivity = new Date().toISOString();
    }
    return session;
  }

  updateSession(sessionId, updates) {
    const session = this.sessions.get(sessionId);
    if (session) {
      Object.assign(session, updates);
      session.lastActivity = new Date().toISOString();
      this.emit('session_updated', session);
    }
    return session;
  }

  deleteSession(sessionId) {
    const session = this.sessions.get(sessionId);
    if (session) {
      this.sessions.delete(sessionId);
      this.streaming.stopStream(sessionId);
      this.emit('session_deleted', session);
      console.log(`Session deleted: ${sessionId}`);
    }
  }

  async submitJob(jobData) {
    const jobId = uuidv4();
    const session = await this.createSession(jobId, {
      ...jobData,
      type: 'job',
      status: 'queued'
    });

    this.jobQueue.push(jobId);
    this.processJobQueue();

    return jobId;
  }

  async processJobQueue() {
    if (this.jobQueue.length === 0) return;

    const jobId = this.jobQueue.shift();
    const session = this.getSession(jobId);

    if (!session) return;

    try {
      session.status = 'running';
      const stream = await this.streaming.startSolve(session);

      // Store stream reference for later access
      session.stream = stream;

    } catch (error) {
      session.status = 'error';
      session.error = error.message;
      console.error(`Job ${jobId} failed:`, error.message);
    }
  }

  async getJobStatus(jobId) {
    const session = this.getSession(jobId);
    if (!session) return null;

    const streamStatus = this.streaming.getStreamStatus(jobId);

    return {
      job_id: jobId,
      status: session.status,
      created_at: session.createdAt,
      last_activity: session.lastActivity,
      metrics: session.metrics,
      stream: streamStatus,
      swarm_nodes: Array.from(session.swarmNodes || []),
      verification_count: session.verificationHistory?.length || 0
    };
  }

  async getJobStream(jobId) {
    const session = this.getSession(jobId);
    if (!session) return null;

    if (session.stream) {
      return session.stream;
    }

    // If no active stream, try to start one
    if (session.status === 'created' || session.status === 'queued') {
      return await this.streaming.startSolve(session);
    }

    return null;
  }

  async verifySession(sessionId, options = {}) {
    const session = this.getSession(sessionId);
    if (!session) {
      throw new Error('Session not found');
    }

    const { probeCount = 10, tolerance = 1e-8 } = options;

    try {
      // Implement verification logic
      const verificationResult = await this.performVerification(session, {
        probeCount,
        tolerance
      });

      // Store verification history
      session.verificationHistory = session.verificationHistory || [];
      session.verificationHistory.push({
        timestamp: new Date().toISOString(),
        ...verificationResult
      });

      // Keep only recent history
      if (session.verificationHistory.length > 100) {
        session.verificationHistory = session.verificationHistory.slice(-100);
      }

      return verificationResult;

    } catch (error) {
      console.error(`Verification failed for session ${sessionId}:`, error);
      throw error;
    }
  }

  async performVerification(session, options) {
    // This would integrate with the actual solver verification
    // For now, return a mock result
    const errors = [];

    for (let i = 0; i < options.probeCount; i++) {
      // Generate random probe error (mock)
      const error = Math.random() * 1e-8;
      errors.push(error);
    }

    const maxError = Math.max(...errors);
    const meanError = errors.reduce((a, b) => a + b) / errors.length;

    return {
      verified: maxError < options.tolerance,
      max_error: maxError,
      mean_error: meanError,
      probe_count: options.probeCount,
      tolerance: options.tolerance,
      verification_time_ms: Math.random() * 5 // Mock timing
    };
  }

  async updateCosts(sessionId, costUpdate) {
    const session = this.getSession(sessionId);
    if (!session) {
      throw new Error('Session not found');
    }

    // Store cost update
    session.costUpdates = session.costUpdates || [];
    session.costUpdates.push({
      timestamp: new Date().toISOString(),
      ...costUpdate
    });

    // Apply cost update to solver (if running)
    if (session.stream && session.status === 'running') {
      // This would integrate with the actual solver cost update mechanism
      console.log(`Cost update applied to session ${sessionId}`);
    }

    this.emit('cost_update', { sessionId, costUpdate });
  }

  async joinSwarm(sessionId, nodeData) {
    const session = this.getSession(sessionId);
    if (!session) {
      throw new Error('Session not found');
    }

    session.swarmNodes = session.swarmNodes || new Set();
    session.swarmNodes.add(nodeData.nodeId);

    console.log(`Node ${nodeData.nodeId} joined swarm for session ${sessionId}`);
    this.emit('swarm_join', { sessionId, nodeData });
  }

  cleanupStaleSession(sessionId) {
    const session = this.sessions.get(sessionId);
    if (!session) return;

    const now = Date.now();
    const lastActivity = new Date(session.lastActivity).getTime();
    const age = now - lastActivity;

    if (age > this.config.sessionTimeout) {
      console.log(`Cleaning up stale session: ${sessionId} (age: ${Math.round(age / 1000)}s)`);
      this.deleteSession(sessionId);
      return true;
    }

    return false;
  }

  cleanupStaleSession() {
    const staleSessions = [];

    for (const [sessionId, session] of this.sessions) {
      if (this.cleanupStaleSession(sessionId)) {
        staleSessions.push(sessionId);
      }
    }

    if (staleSessions.length > 0) {
      console.log(`Cleaned up ${staleSessions.length} stale sessions`);
    }
  }

  getStats() {
    const sessions = Array.from(this.sessions.values());

    return {
      total_sessions: this.sessions.size,
      active_sessions: sessions.filter(s => s.status === 'running').length,
      queued_jobs: this.jobQueue.length,
      sessions_by_status: {
        created: sessions.filter(s => s.status === 'created').length,
        queued: sessions.filter(s => s.status === 'queued').length,
        running: sessions.filter(s => s.status === 'running').length,
        completed: sessions.filter(s => s.status === 'completed').length,
        error: sessions.filter(s => s.status === 'error').length
      },
      streaming_stats: this.streaming.getStats(),
      memory_usage: process.memoryUsage()
    };
  }

  async shutdown() {
    console.log('Shutting down session manager...');

    // Clear cleanup timer
    if (this.cleanupTimer) {
      clearInterval(this.cleanupTimer);
    }

    // Stop all active streams
    for (const sessionId of this.sessions.keys()) {
      this.streaming.stopStream(sessionId);
    }

    // Clear all sessions
    this.sessions.clear();
    this.jobQueue.length = 0;

    this.emit('shutdown');
    console.log('Session manager shutdown complete');
  }
}

// Session data structure
class SolverSession {
  constructor(sessionId, config) {
    this.id = sessionId;
    this.matrix = config.matrix;
    this.vector = config.vector;
    this.method = config.method || 'adaptive';
    this.options = config.options || {};

    this.status = 'created';
    this.createdAt = new Date().toISOString();
    this.lastActivity = new Date().toISOString();

    this.metrics = {
      iterations: 0,
      residual: Infinity,
      convergenceRate: null,
      memoryUsage: 0
    };

    this.swarmNodes = new Set();
    this.verificationHistory = [];
    this.costUpdates = [];

    // Flow-Nexus specific data
    this.flowNexus = config.flowNexus || {};
  }

  serialize() {
    return {
      id: this.id,
      matrix: this.matrix,
      vector: this.vector,
      method: this.method,
      options: this.options,
      status: this.status,
      createdAt: this.createdAt,
      lastActivity: this.lastActivity,
      metrics: this.metrics,
      swarmNodes: Array.from(this.swarmNodes),
      verificationHistory: this.verificationHistory,
      costUpdates: this.costUpdates,
      flowNexus: this.flowNexus
    };
  }

  static deserialize(data) {
    const session = new SolverSession(data.id, {
      matrix: data.matrix,
      vector: data.vector,
      method: data.method,
      options: data.options,
      flowNexus: data.flowNexus
    });

    session.status = data.status;
    session.createdAt = data.createdAt;
    session.lastActivity = data.lastActivity;
    session.metrics = data.metrics;
    session.swarmNodes = new Set(data.swarmNodes || []);
    session.verificationHistory = data.verificationHistory || [];
    session.costUpdates = data.costUpdates || [];

    return session;
  }

  updateMetrics(metrics) {
    Object.assign(this.metrics, metrics);
    this.lastActivity = new Date().toISOString();
  }

  addSwarmNode(nodeId) {
    this.swarmNodes.add(nodeId);
    this.lastActivity = new Date().toISOString();
  }

  removeSwarmNode(nodeId) {
    this.swarmNodes.delete(nodeId);
    this.lastActivity = new Date().toISOString();
  }

  addVerificationResult(result) {
    this.verificationHistory.push({
      timestamp: new Date().toISOString(),
      ...result
    });

    // Keep only recent history
    if (this.verificationHistory.length > 100) {
      this.verificationHistory.shift();
    }

    this.lastActivity = new Date().toISOString();
  }

  addCostUpdate(update) {
    this.costUpdates.push({
      timestamp: new Date().toISOString(),
      ...update
    });

    // Keep only recent updates
    if (this.costUpdates.length > 1000) {
      this.costUpdates = this.costUpdates.slice(-1000);
    }

    this.lastActivity = new Date().toISOString();
  }

  getAge() {
    return Date.now() - new Date(this.createdAt).getTime();
  }

  getInactiveTime() {
    return Date.now() - new Date(this.lastActivity).getTime();
  }
}

module.exports = {
  SessionManager,
  SolverSession
};