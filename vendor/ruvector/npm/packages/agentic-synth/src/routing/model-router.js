/**
 * Model Router for intelligent model selection
 */

export class ModelRouter {
  constructor(options = {}) {
    this.models = options.models || [];
    this.strategy = options.strategy || 'round-robin';
    this.currentIndex = 0;
    this.modelStats = new Map();

    // Initialize stats for provided models
    this.models.forEach(model => {
      this.modelStats.set(model.id, {
        requests: 0,
        errors: 0,
        totalLatency: 0,
        avgLatency: 0
      });
    });
  }

  /**
   * Route request to appropriate model
   * @param {Object} request - Request object
   * @returns {string} Selected model ID
   */
  route(request) {
    if (this.models.length === 0) {
      throw new Error('No models available for routing');
    }

    switch (this.strategy) {
      case 'round-robin':
        return this._roundRobin();
      case 'least-latency':
        return this._leastLatency();
      case 'cost-optimized':
        return this._costOptimized(request);
      case 'capability-based':
        return this._capabilityBased(request);
      default:
        return this.models[0];
    }
  }

  /**
   * Register model
   */
  registerModel(model) {
    if (!model.id || !model.endpoint) {
      throw new Error('Model must have id and endpoint');
    }

    this.models.push(model);
    this.modelStats.set(model.id, {
      requests: 0,
      errors: 0,
      totalLatency: 0,
      avgLatency: 0
    });
  }

  /**
   * Record model performance
   */
  recordMetrics(modelId, latency, success = true) {
    const stats = this.modelStats.get(modelId);
    if (!stats) return;

    stats.requests++;
    if (!success) stats.errors++;
    stats.totalLatency += latency;
    stats.avgLatency = stats.totalLatency / stats.requests;
  }

  /**
   * Get model statistics
   */
  getStats(modelId = null) {
    if (modelId) {
      return this.modelStats.get(modelId);
    }
    return Object.fromEntries(this.modelStats);
  }

  /**
   * Round-robin routing
   * @private
   */
  _roundRobin() {
    const model = this.models[this.currentIndex];
    this.currentIndex = (this.currentIndex + 1) % this.models.length;
    return model.id;
  }

  /**
   * Route to model with least latency
   * @private
   */
  _leastLatency() {
    let bestModel = this.models[0];
    let lowestLatency = Infinity;

    for (const model of this.models) {
      const stats = this.modelStats.get(model.id);
      if (stats && stats.avgLatency < lowestLatency) {
        lowestLatency = stats.avgLatency;
        bestModel = model;
      }
    }

    return bestModel.id;
  }

  /**
   * Cost-optimized routing
   * @private
   */
  _costOptimized(request) {
    const requestSize = JSON.stringify(request).length;

    // Route small requests to cheaper models
    if (requestSize < 1000) {
      return this.models[0].id;
    }

    return this.models[this.models.length - 1].id;
  }

  /**
   * Capability-based routing
   * @private
   */
  _capabilityBased(request) {
    const requiredCapability = request.capability || 'general';

    const capableModels = this.models.filter(model =>
      model.capabilities?.includes(requiredCapability)
    );

    if (capableModels.length === 0) {
      return this.models[0].id;
    }

    return capableModels[0].id;
  }
}
