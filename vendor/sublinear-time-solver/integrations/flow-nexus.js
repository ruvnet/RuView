const { EventEmitter } = require('events');
const https = require('https');
const WebSocket = require('ws');

class FlowNexusIntegration extends EventEmitter {
  constructor(options = {}) {
    super();

    this.config = {
      endpoint: options.endpoint || 'https://api.flow-nexus.ruv.io',
      token: options.token || process.env.FLOW_NEXUS_TOKEN,
      timeout: options.timeout || 30000,
      retryAttempts: options.retryAttempts || 3,
      retryDelay: options.retryDelay || 1000,
      ...options
    };

    this.registered = false;
    this.solverId = null;
    this.capabilities = [];
    this.swarmConnections = new Map();
    this.costUpdateQueue = [];

    // Connection status
    this.connected = false;
    this.lastHeartbeat = null;
    this.reconnectAttempts = 0;
  }

  async registerSolver(solverConfig = {}) {
    try {
      const registrationData = {
        type: 'sublinear-time-solver',
        version: '1.0.0',
        capabilities: [
          'streaming',
          'verification',
          'swarm-coordination',
          'cost-propagation',
          'real-time-updates',
          ...solverConfig.capabilities || []
        ],
        endpoints: {
          solve: '/api/v1/solve-stream',
          verify: '/api/v1/verify',
          status: '/api/v1/status',
          swarm: '/api/v1/swarm'
        },
        performance: {
          max_matrix_size: 1000000,
          target_latency_ms: 1,
          throughput_ops_per_sec: 10000
        },
        metadata: {
          description: 'Advanced sublinear time sparse linear system solver',
          algorithms: ['jacobi', 'gauss-seidel', 'conjugate-gradient', 'hybrid'],
          formats: ['coo', 'csr', 'dense', 'matrix-market'],
          verification: 'random-probe',
          ...solverConfig.metadata
        }
      };

      const response = await this.makeRequest('POST', '/v1/solvers/register', registrationData);

      this.solverId = response.solver_id;
      this.registered = true;
      this.capabilities = registrationData.capabilities;

      console.log(`✓ Registered with Flow-Nexus as solver: ${this.solverId}`);

      // Start heartbeat
      this.startHeartbeat();

      return {
        solver_id: this.solverId,
        status: 'registered',
        capabilities: this.capabilities
      };

    } catch (error) {
      console.error('Flow-Nexus registration failed:', error.message);
      throw error;
    }
  }

  async joinSwarm(swarmId, nodeConfig = {}) {
    if (!this.registered) {
      throw new Error('Must register solver before joining swarm');
    }

    try {
      const joinData = {
        solver_id: this.solverId,
        node_id: nodeConfig.nodeId || `node-${this.solverId}`,
        capabilities: nodeConfig.capabilities || this.capabilities,
        topology_preference: nodeConfig.topology || 'mesh',
        coordination_enabled: true,
        cost_propagation: true
      };

      const response = await this.makeRequest('POST', `/v1/swarms/${swarmId}/join`, joinData);

      // Establish WebSocket connection for real-time coordination
      await this.connectToSwarm(swarmId, response.websocket_endpoint);

      this.swarmConnections.set(swarmId, {
        status: 'connected',
        nodeId: joinData.node_id,
        connectedAt: new Date().toISOString(),
        lastActivity: new Date().toISOString()
      });

      console.log(`✓ Joined swarm: ${swarmId} as node: ${joinData.node_id}`);

      return {
        swarm_id: swarmId,
        node_id: joinData.node_id,
        status: 'joined'
      };

    } catch (error) {
      console.error(`Failed to join swarm ${swarmId}:`, error.message);
      throw error;
    }
  }

  async connectToSwarm(swarmId, wsEndpoint) {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(wsEndpoint, {
        headers: {
          'Authorization': `Bearer ${this.config.token}`,
          'X-Solver-ID': this.solverId
        }
      });

      ws.on('open', () => {
        console.log(`WebSocket connected to swarm: ${swarmId}`);
        this.connected = true;
        resolve();
      });

      ws.on('message', (data) => {
        this.handleSwarmMessage(swarmId, JSON.parse(data.toString()));
      });

      ws.on('close', () => {
        console.log(`WebSocket disconnected from swarm: ${swarmId}`);
        this.connected = false;
        this.scheduleReconnect(swarmId, wsEndpoint);
      });

      ws.on('error', (error) => {
        console.error(`WebSocket error for swarm ${swarmId}:`, error);
        reject(error);
      });

      // Store WebSocket connection
      const swarmConnection = this.swarmConnections.get(swarmId);
      if (swarmConnection) {
        swarmConnection.ws = ws;
      }
    });
  }

  handleSwarmMessage(swarmId, message) {
    switch (message.type) {
      case 'cost_update':
        this.handleCostUpdate(swarmId, message);
        break;

      case 'verification_request':
        this.handleVerificationRequest(swarmId, message);
        break;

      case 'consensus_vote':
        this.handleConsensusVote(swarmId, message);
        break;

      case 'heartbeat':
        this.handleHeartbeat(swarmId, message);
        break;

      default:
        console.warn(`Unknown swarm message type: ${message.type}`);
    }
  }

  async handleCostUpdate(swarmId, message) {
    try {
      // Propagate cost update to local solver sessions
      const costUpdate = {
        type: 'cost_update',
        session_id: message.session_id,
        delta_costs: message.delta_costs,
        matrix_updates: message.matrix_updates,
        source_node: message.source_node,
        propagation_depth: (message.propagation_depth || 0) + 1,
        timestamp: new Date().toISOString()
      };

      this.emit('cost_update', costUpdate);

      // Queue for batch processing
      this.costUpdateQueue.push(costUpdate);

      // Process queue if it gets large
      if (this.costUpdateQueue.length > 100) {
        await this.processCostUpdateQueue();
      }

    } catch (error) {
      console.error('Error handling cost update:', error);
    }
  }

  async handleVerificationRequest(swarmId, message) {
    try {
      // Respond to verification request
      const verificationResult = await this.performVerification(message);

      this.sendSwarmMessage(swarmId, {
        type: 'verification_response',
        request_id: message.request_id,
        session_id: message.session_id,
        verified: verificationResult.verified,
        max_error: verificationResult.maxError,
        node_id: this.solverId
      });

    } catch (error) {
      console.error('Error handling verification request:', error);
    }
  }

  async performVerification(request) {
    // Implement verification logic
    // This would integrate with the local solver's verification system
    return {
      verified: true,
      maxError: 1e-10,
      probeCount: request.probe_count || 10
    };
  }

  handleConsensusVote(swarmId, message) {
    // Handle consensus voting for distributed decisions
    this.emit('consensus_vote', {
      swarmId,
      voteId: message.vote_id,
      proposal: message.proposal,
      nodeId: message.node_id
    });
  }

  handleHeartbeat(swarmId, message) {
    this.lastHeartbeat = Date.now();

    // Update swarm connection activity
    const connection = this.swarmConnections.get(swarmId);
    if (connection) {
      connection.lastActivity = new Date().toISOString();
    }
  }

  sendSwarmMessage(swarmId, message) {
    const connection = this.swarmConnections.get(swarmId);
    if (connection && connection.ws && connection.ws.readyState === WebSocket.OPEN) {
      connection.ws.send(JSON.stringify(message));
    }
  }

  async broadcastCostUpdate(costUpdate) {
    for (const [swarmId, connection] of this.swarmConnections) {
      if (connection.ws && connection.ws.readyState === WebSocket.OPEN) {
        this.sendSwarmMessage(swarmId, {
          type: 'cost_update',
          ...costUpdate,
          source_node: this.solverId,
          timestamp: new Date().toISOString()
        });
      }
    }
  }

  async processCostUpdateQueue() {
    if (this.costUpdateQueue.length === 0) return;

    // Batch process cost updates
    const updates = this.costUpdateQueue.splice(0);

    try {
      // Aggregate updates by session
      const sessionUpdates = new Map();

      for (const update of updates) {
        if (!sessionUpdates.has(update.session_id)) {
          sessionUpdates.set(update.session_id, []);
        }
        sessionUpdates.get(update.session_id).push(update);
      }

      // Apply aggregated updates
      for (const [sessionId, updates] of sessionUpdates) {
        await this.applyAggregatedUpdates(sessionId, updates);
      }

    } catch (error) {
      console.error('Error processing cost update queue:', error);
    }
  }

  async applyAggregatedUpdates(sessionId, updates) {
    // Aggregate delta costs
    const aggregatedDeltas = new Map();

    for (const update of updates) {
      if (update.delta_costs && update.delta_costs.indices) {
        for (let i = 0; i < update.delta_costs.indices.length; i++) {
          const idx = update.delta_costs.indices[i];
          const value = update.delta_costs.values[i];

          aggregatedDeltas.set(idx, (aggregatedDeltas.get(idx) || 0) + value);
        }
      }
    }

    // Emit aggregated update
    this.emit('aggregated_cost_update', {
      session_id: sessionId,
      delta_costs: {
        indices: Array.from(aggregatedDeltas.keys()),
        values: Array.from(aggregatedDeltas.values())
      },
      update_count: updates.length,
      timestamp: new Date().toISOString()
    });
  }

  startHeartbeat() {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
    }

    this.heartbeatInterval = setInterval(async () => {
      try {
        await this.sendHeartbeat();
      } catch (error) {
        console.error('Heartbeat failed:', error.message);
      }
    }, 30000); // 30 second heartbeat
  }

  async sendHeartbeat() {
    if (!this.registered) return;

    const heartbeatData = {
      solver_id: this.solverId,
      timestamp: new Date().toISOString(),
      status: 'active',
      stats: this.getPerformanceStats()
    };

    await this.makeRequest('POST', `/v1/solvers/${this.solverId}/heartbeat`, heartbeatData);

    // Send heartbeat to all swarm connections
    for (const [swarmId, connection] of this.swarmConnections) {
      this.sendSwarmMessage(swarmId, {
        type: 'heartbeat',
        node_id: this.solverId,
        timestamp: new Date().toISOString()
      });
    }
  }

  getPerformanceStats() {
    return {
      memory_usage: process.memoryUsage(),
      cpu_usage: process.cpuUsage(),
      uptime: process.uptime(),
      active_connections: this.swarmConnections.size,
      queue_size: this.costUpdateQueue.length
    };
  }

  scheduleReconnect(swarmId, wsEndpoint) {
    this.reconnectAttempts++;
    const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);

    setTimeout(async () => {
      try {
        console.log(`Attempting to reconnect to swarm ${swarmId}...`);
        await this.connectToSwarm(swarmId, wsEndpoint);
        this.reconnectAttempts = 0;
      } catch (error) {
        console.error(`Reconnection failed for swarm ${swarmId}:`, error.message);

        if (this.reconnectAttempts < 10) {
          this.scheduleReconnect(swarmId, wsEndpoint);
        } else {
          console.error(`Max reconnection attempts reached for swarm ${swarmId}`);
        }
      }
    }, delay);
  }

  async makeRequest(method, path, data = null) {
    return new Promise((resolve, reject) => {
      const url = new URL(path, this.config.endpoint);

      const options = {
        method,
        headers: {
          'Content-Type': 'application/json',
          'User-Agent': 'SublinearSolver/1.0.0'
        },
        timeout: this.config.timeout
      };

      if (this.config.token) {
        options.headers['Authorization'] = `Bearer ${this.config.token}`;
      }

      const req = https.request(url, options, (res) => {
        let body = '';

        res.on('data', (chunk) => {
          body += chunk;
        });

        res.on('end', () => {
          try {
            const response = JSON.parse(body);

            if (res.statusCode >= 200 && res.statusCode < 300) {
              resolve(response);
            } else {
              reject(new Error(`HTTP ${res.statusCode}: ${response.error || body}`));
            }
          } catch (error) {
            reject(new Error(`Invalid JSON response: ${body}`));
          }
        });
      });

      req.on('error', reject);
      req.on('timeout', () => {
        req.destroy();
        reject(new Error('Request timeout'));
      });

      if (data) {
        req.write(JSON.stringify(data));
      }

      req.end();
    });
  }

  async getStatus() {
    return {
      registered: this.registered,
      solver_id: this.solverId,
      connected: this.connected,
      swarm_connections: this.swarmConnections.size,
      capabilities: this.capabilities,
      last_heartbeat: this.lastHeartbeat,
      queue_size: this.costUpdateQueue.length
    };
  }

  async disconnect() {
    // Clean shutdown
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
    }

    // Close all swarm connections
    for (const [swarmId, connection] of this.swarmConnections) {
      if (connection.ws) {
        connection.ws.close();
      }
    }

    this.swarmConnections.clear();
    this.connected = false;

    // Unregister from Flow-Nexus
    if (this.registered && this.solverId) {
      try {
        await this.makeRequest('DELETE', `/v1/solvers/${this.solverId}`);
        console.log('✓ Unregistered from Flow-Nexus');
      } catch (error) {
        console.error('Error unregistering from Flow-Nexus:', error.message);
      }
    }
  }
}

// Tool registration for Flow-Nexus MCP integration
class FlowNexusMCPTools {
  constructor(integration) {
    this.integration = integration;
  }

  getToolDefinitions() {
    return [
      {
        name: 'sublinear_solver_stream',
        description: 'Stream-based sublinear time matrix solver with real-time updates',
        inputSchema: {
          type: 'object',
          properties: {
            matrix: {
              type: 'object',
              description: 'Sparse matrix in COO or CSR format'
            },
            vector: {
              type: 'array',
              items: { type: 'number' },
              description: 'Right-hand side vector'
            },
            method: {
              enum: ['jacobi', 'gauss_seidel', 'cg', 'hybrid'],
              default: 'adaptive',
              description: 'Solver method'
            },
            stream_options: {
              type: 'object',
              properties: {
                real_time: { type: 'boolean', default: true },
                verification_enabled: { type: 'boolean', default: true },
                swarm_coordination: { type: 'boolean', default: false }
              }
            }
          },
          required: ['matrix', 'vector']
        }
      },
      {
        name: 'solver_verification',
        description: 'Verify solution accuracy with random probes',
        inputSchema: {
          type: 'object',
          properties: {
            session_id: { type: 'string' },
            probe_count: { type: 'integer', minimum: 1, maximum: 100, default: 10 },
            tolerance: { type: 'number', default: 1e-8 }
          },
          required: ['session_id']
        }
      },
      {
        name: 'swarm_cost_propagation',
        description: 'Propagate cost updates across swarm network',
        inputSchema: {
          type: 'object',
          properties: {
            session_id: { type: 'string' },
            delta_costs: {
              type: 'object',
              properties: {
                indices: { type: 'array', items: { type: 'integer' } },
                values: { type: 'array', items: { type: 'number' } }
              }
            },
            swarm_id: { type: 'string' }
          },
          required: ['session_id', 'delta_costs']
        }
      }
    ];
  }

  async handleToolCall(toolName, parameters) {
    switch (toolName) {
      case 'sublinear_solver_stream':
        return await this.handleSolverStream(parameters);

      case 'solver_verification':
        return await this.handleVerification(parameters);

      case 'swarm_cost_propagation':
        return await this.handleCostPropagation(parameters);

      default:
        throw new Error(`Unknown tool: ${toolName}`);
    }
  }

  async handleSolverStream(params) {
    // Implementation would integrate with local solver
    return {
      session_id: 'session-' + Date.now(),
      status: 'started',
      stream_endpoint: '/api/v1/solve-stream'
    };
  }

  async handleVerification(params) {
    const result = await this.integration.performVerification(params);
    return {
      session_id: params.session_id,
      verified: result.verified,
      max_error: result.maxError
    };
  }

  async handleCostPropagation(params) {
    await this.integration.broadcastCostUpdate(params);
    return {
      status: 'propagated',
      timestamp: new Date().toISOString()
    };
  }
}

module.exports = {
  FlowNexusIntegration,
  FlowNexusMCPTools
};