/**
 * DAG-Based Trading Pipeline
 *
 * Orchestrates all production modules into a unified system:
 * - LSTM-Transformer for price prediction
 * - Sentiment Alpha for news signals
 * - DRL Ensemble for portfolio decisions
 * - Fractional Kelly for position sizing
 *
 * Uses DAG topology for parallel execution and critical path optimization.
 */

import { KellyCriterion, TradingKelly } from '../production/fractional-kelly.js';
import { HybridLSTMTransformer, FeatureExtractor } from '../production/hybrid-lstm-transformer.js';
import { EnsemblePortfolioManager, PortfolioEnvironment } from '../production/drl-portfolio-manager.js';
import { SentimentAggregator, AlphaFactorCalculator, LexiconAnalyzer, EmbeddingAnalyzer } from '../production/sentiment-alpha.js';

// Pipeline Configuration
const pipelineConfig = {
  // DAG execution settings
  dag: {
    parallelExecution: true,
    maxConcurrency: 4,
    timeout: 5000,  // ms per node
    retryOnFailure: true,
    maxRetries: 2
  },

  // Signal combination weights
  signalWeights: {
    lstm: 0.35,
    sentiment: 0.25,
    drl: 0.40
  },

  // Position sizing
  sizing: {
    kellyFraction: 'conservative',  // 1/5th Kelly
    maxPositionSize: 0.20,          // Max 20% per position
    minPositionSize: 0.01,          // Min 1% position
    maxTotalExposure: 0.80          // Max 80% invested
  },

  // Execution settings
  execution: {
    slippage: 0.001,      // 0.1% slippage assumption
    commission: 0.001,    // 0.1% commission
    minOrderSize: 100     // Minimum $100 order
  }
};

/**
 * DAG Node - Represents a computation unit in the pipeline
 */
class DagNode {
  constructor(id, name, executor, dependencies = []) {
    this.id = id;
    this.name = name;
    this.executor = executor;
    this.dependencies = dependencies;
    this.status = 'pending';  // pending, running, completed, failed
    this.result = null;
    this.error = null;
    this.startTime = null;
    this.endTime = null;
  }

  get latency() {
    if (!this.startTime || !this.endTime) return null;
    return this.endTime - this.startTime;
  }
}

/**
 * Trading DAG - Manages pipeline execution
 */
class TradingDag {
  constructor(config = pipelineConfig.dag) {
    this.config = config;
    this.nodes = new Map();
    this.edges = new Map();  // node -> dependencies
    this.results = new Map();
    this.executionOrder = [];
    this.metrics = {
      totalLatency: 0,
      criticalPath: [],
      parallelEfficiency: 0
    };
  }

  addNode(node) {
    this.nodes.set(node.id, node);
    this.edges.set(node.id, node.dependencies);
  }

  // Topological sort for execution order
  topologicalSort() {
    const visited = new Set();
    const result = [];
    const visiting = new Set();

    const visit = (nodeId) => {
      if (visited.has(nodeId)) return;
      if (visiting.has(nodeId)) {
        throw new Error(`Cycle detected at node: ${nodeId}`);
      }

      visiting.add(nodeId);
      const deps = this.edges.get(nodeId) || [];
      for (const dep of deps) {
        visit(dep);
      }
      visiting.delete(nodeId);
      visited.add(nodeId);
      result.push(nodeId);
    };

    for (const nodeId of this.nodes.keys()) {
      visit(nodeId);
    }

    this.executionOrder = result;
    return result;
  }

  // Find nodes that can execute in parallel
  getReadyNodes(completed) {
    const ready = [];
    for (const [nodeId, deps] of this.edges) {
      const node = this.nodes.get(nodeId);
      if (node.status === 'pending') {
        const allDepsCompleted = deps.every(d => completed.has(d));
        if (allDepsCompleted) {
          ready.push(nodeId);
        }
      }
    }
    return ready;
  }

  // Execute a single node
  async executeNode(nodeId, context) {
    const node = this.nodes.get(nodeId);
    if (!node) throw new Error(`Node not found: ${nodeId}`);

    node.status = 'running';
    node.startTime = performance.now();

    try {
      // Gather dependency results
      const depResults = {};
      for (const dep of node.dependencies) {
        depResults[dep] = this.results.get(dep);
      }

      // Execute with timeout
      const timeoutPromise = new Promise((_, reject) =>
        setTimeout(() => reject(new Error('Timeout')), this.config.timeout)
      );

      const result = await Promise.race([
        node.executor(context, depResults),
        timeoutPromise
      ]);

      node.result = result;
      node.status = 'completed';
      this.results.set(nodeId, result);
    } catch (error) {
      node.error = error;
      node.status = 'failed';

      if (this.config.retryOnFailure && node.retries < this.config.maxRetries) {
        node.retries = (node.retries || 0) + 1;
        node.status = 'pending';
        return this.executeNode(nodeId, context);
      }
    }

    node.endTime = performance.now();
    return node;
  }

  // Execute entire DAG
  async execute(context) {
    const startTime = performance.now();
    this.topologicalSort();

    const completed = new Set();
    const running = new Map();

    while (completed.size < this.nodes.size) {
      // Get nodes ready to execute
      const ready = this.getReadyNodes(completed);

      if (ready.length === 0 && running.size === 0) {
        // Check for failures
        const failed = [...this.nodes.values()].filter(n => n.status === 'failed');
        if (failed.length > 0) {
          throw new Error(`Pipeline failed: ${failed.map(n => n.name).join(', ')}`);
        }
        break;
      }

      // Execute ready nodes (parallel or sequential)
      if (this.config.parallelExecution) {
        const toExecute = ready.slice(0, this.config.maxConcurrency - running.size);
        const promises = toExecute.map(async nodeId => {
          running.set(nodeId, true);
          await this.executeNode(nodeId, context);
          running.delete(nodeId);
          completed.add(nodeId);
        });
        await Promise.all(promises);
      } else {
        for (const nodeId of ready) {
          await this.executeNode(nodeId, context);
          completed.add(nodeId);
        }
      }
    }

    this.metrics.totalLatency = performance.now() - startTime;
    this.computeCriticalPath();

    return this.results;
  }

  // Compute critical path for optimization insights
  computeCriticalPath() {
    const depths = new Map();
    const latencies = new Map();

    for (const nodeId of this.executionOrder) {
      const node = this.nodes.get(nodeId);
      const deps = this.edges.get(nodeId) || [];

      let maxDepth = 0;
      let maxLatency = 0;
      for (const dep of deps) {
        maxDepth = Math.max(maxDepth, (depths.get(dep) || 0) + 1);
        maxLatency = Math.max(maxLatency, (latencies.get(dep) || 0) + (node.latency || 0));
      }

      depths.set(nodeId, maxDepth);
      latencies.set(nodeId, maxLatency + (node.latency || 0));
    }

    // Find critical path (longest latency)
    let maxLatency = 0;
    let criticalEnd = null;
    for (const [nodeId, latency] of latencies) {
      if (latency > maxLatency) {
        maxLatency = latency;
        criticalEnd = nodeId;
      }
    }

    // Trace back critical path
    this.metrics.criticalPath = [criticalEnd];
    // Simplified - in production would trace back through dependencies
  }

  getMetrics() {
    const nodeMetrics = {};
    for (const [id, node] of this.nodes) {
      nodeMetrics[id] = {
        name: node.name,
        status: node.status,
        latency: node.latency,
        error: node.error?.message
      };
    }

    return {
      ...this.metrics,
      nodes: nodeMetrics
    };
  }
}

/**
 * Unified Trading Signal
 */
class TradingSignal {
  constructor(symbol, direction, strength, confidence, sources) {
    this.symbol = symbol;
    this.direction = direction;  // 'long', 'short', 'neutral'
    this.strength = strength;    // 0-1
    this.confidence = confidence; // 0-1
    this.sources = sources;      // { lstm, sentiment, drl }
    this.timestamp = Date.now();
  }

  get score() {
    return this.direction === 'long' ? this.strength :
           this.direction === 'short' ? -this.strength : 0;
  }
}

/**
 * Trading Pipeline - Main integration class
 */
class TradingPipeline {
  constructor(config = pipelineConfig) {
    this.config = config;

    // Initialize production modules
    this.kelly = new TradingKelly();
    this.featureExtractor = new FeatureExtractor();
    this.lstmTransformer = null;  // Lazy init with correct dimensions
    this.sentimentAggregator = new SentimentAggregator();
    this.alphaCalculator = new AlphaFactorCalculator(this.sentimentAggregator);
    this.lexicon = new LexiconAnalyzer();
    this.embedding = new EmbeddingAnalyzer();

    // DRL initialized per portfolio
    this.drlManager = null;

    // Pipeline state
    this.positions = new Map();
    this.signals = new Map();
    this.orders = [];
  }

  // Build the execution DAG
  buildDag() {
    const dag = new TradingDag(this.config.dag);

    // Node 1: Data Preparation
    dag.addNode(new DagNode('data_prep', 'Data Preparation', async (ctx) => {
      const { marketData, newsData } = ctx;
      return {
        candles: marketData,
        news: newsData,
        features: this.featureExtractor.extract(marketData)
      };
    }, []));

    // Node 2: LSTM-Transformer Prediction (depends on data_prep)
    dag.addNode(new DagNode('lstm_predict', 'LSTM Prediction', async (ctx, deps) => {
      const { features } = deps.data_prep;
      if (!features || features.length === 0) {
        return { prediction: 0, confidence: 0, signal: 'HOLD' };
      }

      // Lazy init LSTM with correct input size
      if (!this.lstmTransformer) {
        const inputSize = features[0]?.length || 10;
        this.lstmTransformer = new HybridLSTMTransformer({
          lstm: { inputSize, hiddenSize: 64, numLayers: 2 },
          transformer: { dModel: 64, numHeads: 4, numLayers: 2, ffDim: 128 }
        });
      }

      const prediction = this.lstmTransformer.predict(features);
      return prediction;
    }, ['data_prep']));

    // Node 3: Sentiment Analysis (depends on data_prep, parallel with LSTM)
    dag.addNode(new DagNode('sentiment_analyze', 'Sentiment Analysis', async (ctx, deps) => {
      const { news } = deps.data_prep;
      if (!news || news.length === 0) {
        return { score: 0, confidence: 0, signal: 'HOLD' };
      }

      // Analyze each news item
      for (const item of news) {
        const lexiconResult = this.lexicon.analyze(item.text);
        const embeddingResult = this.embedding.analyze(item.text);

        this.sentimentAggregator.addSentiment(item.symbol, {
          source: item.source || 'news',
          score: (lexiconResult.score + embeddingResult.score) / 2,
          confidence: (lexiconResult.confidence + embeddingResult.confidence) / 2,
          timestamp: item.timestamp || Date.now()
        });
      }

      // Get aggregated sentiment per symbol
      const symbols = [...new Set(news.map(n => n.symbol))];
      const sentiments = {};
      for (const symbol of symbols) {
        sentiments[symbol] = this.sentimentAggregator.getAggregatedSentiment(symbol);
        sentiments[symbol].alpha = this.alphaCalculator.calculateAlpha(symbol);
      }

      return sentiments;
    }, ['data_prep']));

    // Node 4: DRL Portfolio Decision (depends on data_prep, parallel with LSTM/Sentiment)
    dag.addNode(new DagNode('drl_decide', 'DRL Decision', async (ctx, deps) => {
      const { candles } = deps.data_prep;
      const { portfolio } = ctx;

      if (!portfolio || !candles || candles.length === 0) {
        return { weights: [], action: 'hold' };
      }

      // Initialize DRL if needed
      if (!this.drlManager) {
        const numAssets = portfolio.assets?.length || 1;
        this.drlManager = new EnsemblePortfolioManager(numAssets, {
          lookbackWindow: 30,
          transactionCost: 0.001
        });
      }

      // Get state from environment
      const state = this.buildDrlState(candles, portfolio);
      const action = this.drlManager.getEnsembleAction(state);

      return {
        weights: action,
        action: this.interpretDrlAction(action)
      };
    }, ['data_prep']));

    // Node 5: Signal Fusion (depends on lstm, sentiment, drl)
    dag.addNode(new DagNode('signal_fusion', 'Signal Fusion', async (ctx, deps) => {
      const lstmResult = deps.lstm_predict;
      const sentimentResult = deps.sentiment_analyze;
      const drlResult = deps.drl_decide;
      const { symbols } = ctx;

      const signals = {};

      for (const symbol of (symbols || ['DEFAULT'])) {
        // Get individual signals
        const lstmSignal = this.normalizeSignal(lstmResult);
        const sentimentSignal = this.normalizeSentiment(sentimentResult[symbol]);
        const drlSignal = this.normalizeDrl(drlResult, symbol);

        // Weighted combination
        const w = this.config.signalWeights;
        const combinedScore =
          w.lstm * lstmSignal.score +
          w.sentiment * sentimentSignal.score +
          w.drl * drlSignal.score;

        const combinedConfidence =
          w.lstm * lstmSignal.confidence +
          w.sentiment * sentimentSignal.confidence +
          w.drl * drlSignal.confidence;

        const direction = combinedScore > 0.1 ? 'long' :
                         combinedScore < -0.1 ? 'short' : 'neutral';

        signals[symbol] = new TradingSignal(
          symbol,
          direction,
          Math.abs(combinedScore),
          combinedConfidence,
          { lstm: lstmSignal, sentiment: sentimentSignal, drl: drlSignal }
        );
      }

      return signals;
    }, ['lstm_predict', 'sentiment_analyze', 'drl_decide']));

    // Node 6: Position Sizing with Kelly (depends on signal_fusion)
    dag.addNode(new DagNode('position_sizing', 'Position Sizing', async (ctx, deps) => {
      const signals = deps.signal_fusion;
      const { portfolio, riskManager } = ctx;

      const positions = {};

      for (const [symbol, signal] of Object.entries(signals)) {
        if (signal.direction === 'neutral') {
          positions[symbol] = { size: 0, action: 'hold' };
          continue;
        }

        // Check risk limits first
        if (riskManager && !riskManager.canTrade(symbol)) {
          positions[symbol] = { size: 0, action: 'blocked', reason: 'risk_limit' };
          continue;
        }

        // Calculate Kelly position size
        const winProb = 0.5 + signal.strength * signal.confidence * 0.2;  // Map to 0.5-0.7
        const avgWin = 0.02;   // 2% average win
        const avgLoss = 0.015; // 1.5% average loss

        const kellyResult = this.kelly.calculatePositionSize(
          portfolio?.equity || 10000,
          winProb,
          avgWin,
          avgLoss,
          this.config.sizing.kellyFraction
        );

        // Apply position limits
        let size = kellyResult.positionSize;
        size = Math.min(size, portfolio?.equity * this.config.sizing.maxPositionSize);
        size = Math.max(size, portfolio?.equity * this.config.sizing.minPositionSize);

        // Check total exposure
        const currentExposure = this.calculateExposure(positions, portfolio);
        if (currentExposure + size / portfolio?.equity > this.config.sizing.maxTotalExposure) {
          size = (this.config.sizing.maxTotalExposure - currentExposure) * portfolio?.equity;
        }

        positions[symbol] = {
          size: signal.direction === 'short' ? -size : size,
          action: signal.direction === 'long' ? 'buy' : 'sell',
          kelly: kellyResult,
          signal
        };
      }

      return positions;
    }, ['signal_fusion']));

    // Node 7: Order Generation (depends on position_sizing)
    dag.addNode(new DagNode('order_gen', 'Order Generation', async (ctx, deps) => {
      const positions = deps.position_sizing;
      const { portfolio, prices } = ctx;

      const orders = [];

      for (const [symbol, position] of Object.entries(positions)) {
        if (position.action === 'hold' || position.action === 'blocked') {
          continue;
        }

        const currentPosition = portfolio?.positions?.[symbol] || 0;
        const targetPosition = position.size;
        const delta = targetPosition - currentPosition;

        if (Math.abs(delta) < this.config.execution.minOrderSize) {
          continue;  // Skip small orders
        }

        const price = prices?.[symbol] || 100;
        const shares = Math.floor(Math.abs(delta) / price);

        if (shares > 0) {
          orders.push({
            symbol,
            side: delta > 0 ? 'buy' : 'sell',
            quantity: shares,
            type: 'market',
            price,
            estimatedValue: shares * price,
            slippage: shares * price * this.config.execution.slippage,
            commission: shares * price * this.config.execution.commission,
            signal: position.signal,
            timestamp: Date.now()
          });
        }
      }

      return orders;
    }, ['position_sizing']));

    return dag;
  }

  // Helper: Build DRL state vector
  buildDrlState(candles, portfolio) {
    const state = [];

    // Price features (last 30 returns)
    const returns = [];
    for (let i = 1; i < Math.min(31, candles.length); i++) {
      returns.push((candles[i].close - candles[i-1].close) / candles[i-1].close);
    }
    state.push(...returns);

    // Portfolio features
    if (portfolio) {
      state.push(portfolio.cash / portfolio.equity);
      state.push(portfolio.exposure || 0);
    }

    // Pad to expected size
    while (state.length < 62) state.push(0);

    return state.slice(0, 62);
  }

  // Helper: Normalize LSTM output to signal
  normalizeSignal(lstmResult) {
    if (!lstmResult) return { score: 0, confidence: 0 };
    return {
      score: lstmResult.prediction || 0,
      confidence: lstmResult.confidence || 0
    };
  }

  // Helper: Normalize sentiment to signal
  normalizeSentiment(sentiment) {
    if (!sentiment) return { score: 0, confidence: 0 };
    return {
      score: sentiment.score || sentiment.alpha?.factor || 0,
      confidence: sentiment.confidence || 0.5
    };
  }

  // Helper: Normalize DRL output to signal
  normalizeDrl(drlResult, symbol) {
    if (!drlResult || !drlResult.weights) return { score: 0, confidence: 0 };
    // Map weight to signal (-1 to 1)
    const weight = drlResult.weights[0] || 0;
    return {
      score: weight * 2 - 1,  // Map 0-1 to -1 to 1
      confidence: 0.6
    };
  }

  // Helper: Calculate current exposure
  calculateExposure(positions, portfolio) {
    if (!portfolio?.equity) return 0;
    let exposure = 0;
    for (const pos of Object.values(positions)) {
      exposure += Math.abs(pos.size || 0);
    }
    return exposure / portfolio.equity;
  }

  // Main execution method
  async execute(context) {
    const dag = this.buildDag();
    const results = await dag.execute(context);

    return {
      signals: results.get('signal_fusion'),
      positions: results.get('position_sizing'),
      orders: results.get('order_gen'),
      metrics: dag.getMetrics()
    };
  }
}

/**
 * Pipeline Factory
 */
function createTradingPipeline(config) {
  return new TradingPipeline({ ...pipelineConfig, ...config });
}

export {
  TradingPipeline,
  TradingDag,
  DagNode,
  TradingSignal,
  createTradingPipeline,
  pipelineConfig
};

// Demo if run directly
const isMainModule = import.meta.url === `file://${process.argv[1]}`;
if (isMainModule) {
  console.log('══════════════════════════════════════════════════════════════════════');
  console.log('DAG-BASED TRADING PIPELINE');
  console.log('══════════════════════════════════════════════════════════════════════\n');

  const pipeline = createTradingPipeline();

  // Generate sample data
  const generateCandles = (n) => {
    const candles = [];
    let price = 100;
    for (let i = 0; i < n; i++) {
      const change = (Math.random() - 0.5) * 2;
      price *= (1 + change / 100);
      candles.push({
        open: price * (1 - Math.random() * 0.01),
        high: price * (1 + Math.random() * 0.02),
        low: price * (1 - Math.random() * 0.02),
        close: price,
        volume: 1000000 * (0.5 + Math.random())
      });
    }
    return candles;
  };

  const context = {
    marketData: generateCandles(100),
    newsData: [
      { symbol: 'AAPL', text: 'Strong earnings beat expectations with record revenue growth', source: 'news', timestamp: Date.now() },
      { symbol: 'AAPL', text: 'Analysts upgrade rating after impressive quarterly results', source: 'analyst', timestamp: Date.now() },
      { symbol: 'AAPL', text: 'New product launch receives positive market reception', source: 'social', timestamp: Date.now() }
    ],
    symbols: ['AAPL'],
    portfolio: {
      equity: 100000,
      cash: 50000,
      positions: {},
      exposure: 0,
      assets: ['AAPL']
    },
    prices: { AAPL: 150 },
    riskManager: null
  };

  console.log('1. Pipeline Configuration:');
  console.log('──────────────────────────────────────────────────────────────────────');
  console.log(`   Parallel execution: ${pipelineConfig.dag.parallelExecution}`);
  console.log(`   Max concurrency: ${pipelineConfig.dag.maxConcurrency}`);
  console.log(`   Signal weights: LSTM=${pipelineConfig.signalWeights.lstm}, Sentiment=${pipelineConfig.signalWeights.sentiment}, DRL=${pipelineConfig.signalWeights.drl}`);
  console.log(`   Kelly fraction: ${pipelineConfig.sizing.kellyFraction}`);
  console.log();

  console.log('2. Executing Pipeline:');
  console.log('──────────────────────────────────────────────────────────────────────');

  pipeline.execute(context).then(result => {
    console.log(`   Total latency: ${result.metrics.totalLatency.toFixed(2)}ms`);
    console.log();

    console.log('3. Node Execution:');
    console.log('──────────────────────────────────────────────────────────────────────');
    for (const [id, node] of Object.entries(result.metrics.nodes)) {
      const status = node.status === 'completed' ? '✓' : '✗';
      console.log(`   ${status} ${node.name.padEnd(20)} ${(node.latency || 0).toFixed(2)}ms`);
    }
    console.log();

    console.log('4. Trading Signals:');
    console.log('──────────────────────────────────────────────────────────────────────');
    for (const [symbol, signal] of Object.entries(result.signals || {})) {
      console.log(`   ${symbol}: ${signal.direction.toUpperCase()} (strength: ${(signal.strength * 100).toFixed(1)}%, confidence: ${(signal.confidence * 100).toFixed(1)}%)`);
      console.log(`      LSTM: ${(signal.sources.lstm.score * 100).toFixed(1)}%`);
      console.log(`      Sentiment: ${(signal.sources.sentiment.score * 100).toFixed(1)}%`);
      console.log(`      DRL: ${(signal.sources.drl.score * 100).toFixed(1)}%`);
    }
    console.log();

    console.log('5. Position Sizing:');
    console.log('──────────────────────────────────────────────────────────────────────');
    for (const [symbol, pos] of Object.entries(result.positions || {})) {
      if (pos.action !== 'hold') {
        console.log(`   ${symbol}: ${pos.action.toUpperCase()} $${Math.abs(pos.size).toFixed(2)}`);
        if (pos.kelly) {
          console.log(`      Kelly: ${(pos.kelly.kellyFraction * 100).toFixed(2)}%`);
        }
      }
    }
    console.log();

    console.log('6. Generated Orders:');
    console.log('──────────────────────────────────────────────────────────────────────');
    if (result.orders && result.orders.length > 0) {
      for (const order of result.orders) {
        console.log(`   ${order.side.toUpperCase()} ${order.quantity} ${order.symbol} @ $${order.price.toFixed(2)}`);
        console.log(`      Value: $${order.estimatedValue.toFixed(2)}, Costs: $${(order.slippage + order.commission).toFixed(2)}`);
      }
    } else {
      console.log('   No orders generated');
    }

    console.log();
    console.log('══════════════════════════════════════════════════════════════════════');
    console.log('Trading pipeline execution completed');
    console.log('══════════════════════════════════════════════════════════════════════');
  }).catch(err => {
    console.error('Pipeline error:', err);
  });
}
