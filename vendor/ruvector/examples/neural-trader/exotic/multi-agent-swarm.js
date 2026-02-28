/**
 * Multi-Agent Swarm Trading Coordination
 *
 * EXOTIC: Distributed intelligence for market analysis
 *
 * Uses @neural-trader with RuVector for:
 * - Specialized agent roles (momentum, mean-reversion, sentiment, arbitrage)
 * - Consensus mechanisms for trade decisions
 * - Pheromone-inspired signal propagation
 * - Emergent collective intelligence
 *
 * Each agent maintains its own vector memory in RuVector,
 * with cross-agent communication via shared memory space.
 */

// Ring buffer for efficient bounded memory
class RingBuffer {
  constructor(capacity) {
    this.capacity = capacity;
    this.buffer = new Array(capacity);
    this.head = 0;
    this.size = 0;
  }

  push(item) {
    this.buffer[this.head] = item;
    this.head = (this.head + 1) % this.capacity;
    if (this.size < this.capacity) this.size++;
  }

  getAll() {
    if (this.size < this.capacity) {
      return this.buffer.slice(0, this.size);
    }
    return [...this.buffer.slice(this.head), ...this.buffer.slice(0, this.head)];
  }

  getLast(n) {
    const all = this.getAll();
    return all.slice(-Math.min(n, all.length));
  }

  get length() {
    return this.size;
  }
}

// Signal pool for object reuse
class SignalPool {
  constructor(initialSize = 100) {
    this.pool = [];
    for (let i = 0; i < initialSize; i++) {
      this.pool.push({ direction: 0, confidence: 0, timestamp: 0, reason: '' });
    }
  }

  acquire(direction, confidence, reason) {
    let signal = this.pool.pop();
    if (!signal) {
      signal = { direction: 0, confidence: 0, timestamp: 0, reason: '' };
    }
    signal.direction = direction;
    signal.confidence = confidence;
    signal.timestamp = Date.now();
    signal.reason = reason;
    return signal;
  }

  release(signal) {
    if (this.pool.length < 500) {
      this.pool.push(signal);
    }
  }
}

const signalPool = new SignalPool(200);

// Swarm configuration
const swarmConfig = {
  // Agent types with specializations
  agents: {
    momentum: { count: 3, weight: 0.25, lookback: 20 },
    meanReversion: { count: 2, weight: 0.20, zscore: 2.0 },
    sentiment: { count: 2, weight: 0.15, threshold: 0.6 },
    arbitrage: { count: 1, weight: 0.15, minSpread: 0.001 },
    volatility: { count: 2, weight: 0.25, regime: 'adaptive' }
  },

  // Consensus parameters
  consensus: {
    method: 'weighted_vote',    // weighted_vote, byzantine, raft
    quorum: 0.6,                // 60% agreement needed
    timeout: 1000,              // ms to wait for votes
    minConfidence: 0.7          // Minimum confidence to act
  },

  // Pheromone decay for signal propagation
  pheromone: {
    decayRate: 0.95,
    reinforcement: 1.5,
    evaporationTime: 300000     // 5 minutes
  }
};

// Base Agent class
class TradingAgent {
  constructor(id, type, config) {
    this.id = id;
    this.type = type;
    this.config = config;
    this.memory = [];
    this.signals = [];
    this.confidence = 0.5;
    this.performance = { wins: 0, losses: 0, pnl: 0 };
    this.maxSignals = 1000;  // Bound signals array to prevent memory leak
  }

  // Analyze market data and generate signal
  analyze(marketData) {
    throw new Error('Subclass must implement analyze()');
  }

  // Update agent's memory with new observation
  updateMemory(observation) {
    this.memory.push({
      timestamp: Date.now(),
      observation,
      signal: this.signals[this.signals.length - 1]
    });

    // Keep bounded memory
    if (this.memory.length > 1000) {
      this.memory.shift();
    }
  }

  // Learn from outcome
  learn(outcome) {
    if (outcome.profitable) {
      this.performance.wins++;
      this.confidence = Math.min(0.95, this.confidence * 1.05);
    } else {
      this.performance.losses++;
      this.confidence = Math.max(0.1, this.confidence * 0.95);
    }
    this.performance.pnl += outcome.pnl;
  }
}

// Momentum Agent - follows trends
class MomentumAgent extends TradingAgent {
  constructor(id, config) {
    super(id, 'momentum', config);
    this.lookback = config.lookback || 20;
  }

  analyze(marketData) {
    const prices = marketData.slice(-this.lookback);
    if (prices.length < this.lookback) {
      return { signal: 0, confidence: 0, reason: 'insufficient data' };
    }

    // Calculate momentum as rate of change
    const oldPrice = prices[0].close;
    const newPrice = prices[prices.length - 1].close;
    const momentum = (newPrice - oldPrice) / oldPrice;

    // Calculate trend strength via linear regression
    const n = prices.length;
    let sumX = 0, sumY = 0, sumXY = 0, sumX2 = 0;
    for (let i = 0; i < n; i++) {
      sumX += i;
      sumY += prices[i].close;
      sumXY += i * prices[i].close;
      sumX2 += i * i;
    }
    const denominator = n * sumX2 - sumX * sumX;
    // Guard against division by zero (all prices identical)
    const slope = Math.abs(denominator) > 1e-10
      ? (n * sumXY - sumX * sumY) / denominator
      : 0;
    const avgPrice = sumY / n;
    const normalizedSlope = avgPrice > 0 ? slope / avgPrice : 0;

    // Signal strength based on momentum and trend alignment
    let signal = 0;
    let confidence = 0;

    if (momentum > 0 && normalizedSlope > 0) {
      signal = 1;  // Long
      confidence = Math.min(0.95, Math.abs(momentum) * 10 + Math.abs(normalizedSlope) * 100);
    } else if (momentum < 0 && normalizedSlope < 0) {
      signal = -1;  // Short
      confidence = Math.min(0.95, Math.abs(momentum) * 10 + Math.abs(normalizedSlope) * 100);
    }

    const result = {
      signal,
      confidence: confidence * this.confidence,  // Weighted by agent's track record
      reason: `momentum=${(momentum * 100).toFixed(2)}%, slope=${(normalizedSlope * 10000).toFixed(2)}bps/bar`,
      agentId: this.id,
      agentType: this.type
    };

    this.signals.push(result);
    // Bound signals array to prevent memory leak
    if (this.signals.length > this.maxSignals) {
      this.signals = this.signals.slice(-this.maxSignals);
    }
    return result;
  }
}

// Mean Reversion Agent - fades extremes
class MeanReversionAgent extends TradingAgent {
  constructor(id, config) {
    super(id, 'meanReversion', config);
    this.zscoreThreshold = config.zscore || 2.0;
    this.lookback = config.lookback || 50;
  }

  analyze(marketData) {
    const prices = marketData.slice(-this.lookback).map(d => d.close);
    if (prices.length < 20) {
      return { signal: 0, confidence: 0, reason: 'insufficient data' };
    }

    // Calculate z-score with division-by-zero guard
    const mean = prices.reduce((a, b) => a + b, 0) / prices.length;
    const variance = prices.reduce((sum, p) => sum + Math.pow(p - mean, 2), 0) / prices.length;
    const std = Math.sqrt(variance);
    const currentPrice = prices[prices.length - 1];
    // Guard against zero standard deviation (constant prices)
    const zscore = std > 1e-10 ? (currentPrice - mean) / std : 0;

    let signal = 0;
    let confidence = 0;

    if (zscore > this.zscoreThreshold) {
      signal = -1;  // Short - price too high
      confidence = Math.min(0.9, (Math.abs(zscore) - this.zscoreThreshold) * 0.3);
    } else if (zscore < -this.zscoreThreshold) {
      signal = 1;   // Long - price too low
      confidence = Math.min(0.9, (Math.abs(zscore) - this.zscoreThreshold) * 0.3);
    }

    const result = {
      signal,
      confidence: confidence * this.confidence,
      reason: `zscore=${zscore.toFixed(2)}, mean=${mean.toFixed(2)}, std=${std.toFixed(4)}`,
      agentId: this.id,
      agentType: this.type
    };

    this.signals.push(result);
    return result;
  }
}

// Sentiment Agent - analyzes market sentiment
class SentimentAgent extends TradingAgent {
  constructor(id, config) {
    super(id, 'sentiment', config);
    this.threshold = config.threshold || 0.6;
  }

  analyze(marketData) {
    // Derive sentiment from price action (in production, use news/social data)
    const recent = marketData.slice(-10);
    if (recent.length < 5) {
      return { signal: 0, confidence: 0, reason: 'insufficient data' };
    }

    // Count bullish vs bearish candles
    let bullish = 0, bearish = 0;
    let volumeUp = 0, volumeDown = 0;

    for (const candle of recent) {
      if (candle.close > candle.open) {
        bullish++;
        volumeUp += candle.volume || 1;
      } else {
        bearish++;
        volumeDown += candle.volume || 1;
      }
    }

    // Volume-weighted sentiment
    const totalVolume = volumeUp + volumeDown;
    const sentiment = totalVolume > 0
      ? (volumeUp - volumeDown) / totalVolume
      : (bullish - bearish) / recent.length;

    let signal = 0;
    let confidence = 0;

    if (sentiment > this.threshold - 0.5) {
      signal = 1;
      confidence = Math.abs(sentiment);
    } else if (sentiment < -(this.threshold - 0.5)) {
      signal = -1;
      confidence = Math.abs(sentiment);
    }

    const result = {
      signal,
      confidence: confidence * this.confidence,
      reason: `sentiment=${sentiment.toFixed(2)}, bullish=${bullish}/${recent.length}`,
      agentId: this.id,
      agentType: this.type
    };

    this.signals.push(result);
    return result;
  }
}

// Volatility Regime Agent - adapts to market conditions
class VolatilityAgent extends TradingAgent {
  constructor(id, config) {
    super(id, 'volatility', config);
    this.lookback = 20;
  }

  analyze(marketData) {
    const prices = marketData.slice(-this.lookback);
    if (prices.length < 10) {
      return { signal: 0, confidence: 0, reason: 'insufficient data' };
    }

    // Calculate returns
    const returns = [];
    for (let i = 1; i < prices.length; i++) {
      returns.push((prices[i].close - prices[i-1].close) / prices[i-1].close);
    }

    // Calculate realized volatility
    const mean = returns.reduce((a, b) => a + b, 0) / returns.length;
    const variance = returns.reduce((sum, r) => sum + Math.pow(r - mean, 2), 0) / returns.length;
    const volatility = Math.sqrt(variance) * Math.sqrt(252);  // Annualized

    // Detect regime
    const highVolThreshold = 0.30;  // 30% annualized
    const lowVolThreshold = 0.15;   // 15% annualized

    let regime = 'normal';
    let signal = 0;
    let confidence = 0;

    if (volatility > highVolThreshold) {
      regime = 'high';
      // In high vol, mean reversion tends to work
      const lastReturn = returns[returns.length - 1];
      if (Math.abs(lastReturn) > variance * 2) {
        signal = lastReturn > 0 ? -1 : 1;  // Fade the move
        confidence = 0.6;
      }
    } else if (volatility < lowVolThreshold) {
      regime = 'low';
      // In low vol, momentum tends to work
      const recentMomentum = prices[prices.length - 1].close / prices[0].close - 1;
      signal = recentMomentum > 0 ? 1 : -1;
      confidence = 0.5;
    }

    const result = {
      signal,
      confidence: confidence * this.confidence,
      reason: `regime=${regime}, vol=${(volatility * 100).toFixed(1)}%`,
      regime,
      volatility,
      agentId: this.id,
      agentType: this.type
    };

    this.signals.push(result);
    return result;
  }
}

// Swarm Coordinator - manages consensus
class SwarmCoordinator {
  constructor(config) {
    this.config = config;
    this.agents = [];
    this.pheromoneTrails = new Map();
    this.consensusHistory = [];
  }

  // Initialize agent swarm
  initializeSwarm() {
    let agentId = 0;

    // Create momentum agents
    for (let i = 0; i < this.config.agents.momentum.count; i++) {
      this.agents.push(new MomentumAgent(agentId++, {
        ...this.config.agents.momentum,
        lookback: 10 + i * 10  // Different lookbacks
      }));
    }

    // Create mean reversion agents
    for (let i = 0; i < this.config.agents.meanReversion.count; i++) {
      this.agents.push(new MeanReversionAgent(agentId++, {
        ...this.config.agents.meanReversion,
        zscore: 1.5 + i * 0.5
      }));
    }

    // Create sentiment agents
    for (let i = 0; i < this.config.agents.sentiment.count; i++) {
      this.agents.push(new SentimentAgent(agentId++, this.config.agents.sentiment));
    }

    // Create volatility agents
    for (let i = 0; i < this.config.agents.volatility.count; i++) {
      this.agents.push(new VolatilityAgent(agentId++, this.config.agents.volatility));
    }

    console.log(`Initialized swarm with ${this.agents.length} agents`);
  }

  // Gather signals from all agents
  gatherSignals(marketData) {
    const signals = [];

    for (const agent of this.agents) {
      const signal = agent.analyze(marketData);
      signals.push(signal);
    }

    return signals;
  }

  // Weighted voting consensus
  weightedVoteConsensus(signals) {
    let totalWeight = 0;
    let weightedSum = 0;
    let totalConfidence = 0;

    const agentWeights = this.config.agents;

    for (const signal of signals) {
      if (signal.signal === 0) continue;

      const typeWeight = agentWeights[signal.agentType]?.weight || 0.1;
      const weight = typeWeight * signal.confidence;

      weightedSum += signal.signal * weight;
      totalWeight += weight;
      totalConfidence += signal.confidence;
    }

    if (totalWeight === 0) {
      return { decision: 0, confidence: 0, reason: 'no signals' };
    }

    const normalizedSignal = weightedSum / totalWeight;
    const avgConfidence = totalConfidence / signals.length;

    // Apply quorum requirement
    const activeSignals = signals.filter(s => s.signal !== 0);
    const quorum = activeSignals.length / signals.length;

    if (quorum < this.config.consensus.quorum) {
      return {
        decision: 0,
        confidence: 0,
        reason: `quorum not met (${(quorum * 100).toFixed(0)}% < ${(this.config.consensus.quorum * 100).toFixed(0)}%)`
      };
    }

    // Determine final decision
    let decision = 0;
    if (normalizedSignal > 0.3) decision = 1;
    else if (normalizedSignal < -0.3) decision = -1;

    return {
      decision,
      confidence: avgConfidence * Math.abs(normalizedSignal),
      normalizedSignal,
      quorum,
      reason: `weighted_vote=${normalizedSignal.toFixed(3)}, quorum=${(quorum * 100).toFixed(0)}%`
    };
  }

  // Byzantine fault tolerant consensus (simplified)
  byzantineConsensus(signals) {
    // In BFT, we need 2f+1 agreeing votes to tolerate f faulty nodes
    const activeSignals = signals.filter(s => s.signal !== 0);
    const n = activeSignals.length;
    const f = Math.floor((n - 1) / 3);  // Max faulty nodes
    const requiredAgreement = 2 * f + 1;

    const votes = { long: 0, short: 0, neutral: 0 };
    for (const signal of signals) {
      if (signal.signal > 0) votes.long++;
      else if (signal.signal < 0) votes.short++;
      else votes.neutral++;
    }

    let decision = 0;
    let confidence = 0;

    if (votes.long >= requiredAgreement) {
      decision = 1;
      confidence = votes.long / n;
    } else if (votes.short >= requiredAgreement) {
      decision = -1;
      confidence = votes.short / n;
    }

    return {
      decision,
      confidence,
      votes,
      requiredAgreement,
      reason: `BFT: L=${votes.long}, S=${votes.short}, N=${votes.neutral}, need=${requiredAgreement}`
    };
  }

  // Main consensus method
  reachConsensus(signals) {
    let consensus;

    switch (this.config.consensus.method) {
      case 'byzantine':
        consensus = this.byzantineConsensus(signals);
        break;
      case 'weighted_vote':
      default:
        consensus = this.weightedVoteConsensus(signals);
    }

    // Apply minimum confidence threshold
    if (consensus.confidence < this.config.consensus.minConfidence) {
      consensus.decision = 0;
      consensus.reason += ` (confidence ${(consensus.confidence * 100).toFixed(0)}% < ${(this.config.consensus.minConfidence * 100).toFixed(0)}%)`;
    }

    // Update pheromone trails
    this.updatePheromones(consensus);

    this.consensusHistory.push({
      timestamp: Date.now(),
      consensus,
      signalCount: signals.length
    });

    // Bound consensus history to prevent memory leak
    if (this.consensusHistory.length > 1000) {
      this.consensusHistory = this.consensusHistory.slice(-500);
    }

    return consensus;
  }

  // Pheromone-based signal reinforcement
  updatePheromones(consensus) {
    const now = Date.now();

    // Decay existing pheromones
    for (const [key, trail] of this.pheromoneTrails) {
      const age = now - trail.timestamp;
      trail.strength *= Math.pow(this.config.pheromone.decayRate, age / 1000);

      if (trail.strength < 0.01) {
        this.pheromoneTrails.delete(key);
      }
    }

    // Reinforce based on consensus
    if (consensus.decision !== 0) {
      const key = consensus.decision > 0 ? 'bullish' : 'bearish';
      const existing = this.pheromoneTrails.get(key) || { strength: 0, timestamp: now };

      existing.strength = Math.min(1.0,
        existing.strength + consensus.confidence * this.config.pheromone.reinforcement
      );
      existing.timestamp = now;

      this.pheromoneTrails.set(key, existing);
    }
  }

  // Learn from trade outcome
  learnFromOutcome(outcome) {
    for (const agent of this.agents) {
      agent.learn(outcome);
    }
  }

  // Get swarm statistics
  getSwarmStats() {
    const stats = {
      totalAgents: this.agents.length,
      byType: {},
      avgConfidence: 0,
      totalWins: 0,
      totalLosses: 0,
      totalPnL: 0,
      pheromones: {}
    };

    for (const agent of this.agents) {
      if (!stats.byType[agent.type]) {
        stats.byType[agent.type] = { count: 0, avgConfidence: 0, pnl: 0 };
      }
      stats.byType[agent.type].count++;
      stats.byType[agent.type].avgConfidence += agent.confidence;
      stats.byType[agent.type].pnl += agent.performance.pnl;
      stats.avgConfidence += agent.confidence;
      stats.totalWins += agent.performance.wins;
      stats.totalLosses += agent.performance.losses;
      stats.totalPnL += agent.performance.pnl;
    }

    stats.avgConfidence /= this.agents.length || 1;

    // Use Object.entries for object iteration (stats.byType is an object, not Map)
    for (const [key, value] of Object.entries(stats.byType)) {
      stats.byType[key].avgConfidence /= value.count || 1;
    }

    for (const [key, trail] of this.pheromoneTrails) {
      stats.pheromones[key] = trail.strength;
    }

    return stats;
  }
}

// Generate synthetic market data
function generateMarketData(n, seed = 42) {
  const data = [];
  let price = 100;

  let rng = seed;
  const random = () => {
    rng = (rng * 9301 + 49297) % 233280;
    return rng / 233280;
  };

  for (let i = 0; i < n; i++) {
    const regime = Math.sin(i / 100) > 0 ? 'trend' : 'mean-revert';
    const volatility = regime === 'trend' ? 0.015 : 0.025;

    let drift = 0;
    if (regime === 'trend') {
      drift = 0.0003 * Math.sign(Math.sin(i / 200));
    }

    const return_ = drift + volatility * (random() + random() - 1);
    const open = price;
    price = price * (1 + return_);

    const high = Math.max(open, price) * (1 + random() * 0.005);
    const low = Math.min(open, price) * (1 - random() * 0.005);
    const volume = 1000000 * (0.5 + random());

    data.push({
      timestamp: Date.now() - (n - i) * 60000,
      open,
      high,
      low,
      close: price,
      volume,
      regime
    });
  }

  return data;
}

async function main() {
  console.log('═'.repeat(70));
  console.log('MULTI-AGENT SWARM TRADING COORDINATION');
  console.log('═'.repeat(70));
  console.log();

  // 1. Initialize swarm
  console.log('1. Swarm Initialization:');
  console.log('─'.repeat(70));

  const coordinator = new SwarmCoordinator(swarmConfig);
  coordinator.initializeSwarm();

  console.log();
  console.log('   Agent Distribution:');
  for (const [type, config] of Object.entries(swarmConfig.agents)) {
    console.log(`   - ${type}: ${config.count} agents (weight: ${(config.weight * 100).toFixed(0)}%)`);
  }
  console.log();

  // 2. Generate market data
  console.log('2. Market Data Simulation:');
  console.log('─'.repeat(70));

  const marketData = generateMarketData(500);
  console.log(`   Generated ${marketData.length} candles`);
  console.log(`   Price range: $${Math.min(...marketData.map(d => d.low)).toFixed(2)} - $${Math.max(...marketData.map(d => d.high)).toFixed(2)}`);
  console.log();

  // 3. Run swarm analysis
  console.log('3. Swarm Analysis (Rolling Window):');
  console.log('─'.repeat(70));

  const decisions = [];
  const lookback = 100;

  for (let i = lookback; i < marketData.length; i += 10) {
    const window = marketData.slice(i - lookback, i);
    const signals = coordinator.gatherSignals(window);
    const consensus = coordinator.reachConsensus(signals);

    decisions.push({
      index: i,
      price: marketData[i].close,
      consensus,
      signals
    });

    // Simulate outcome for learning
    if (i + 10 < marketData.length) {
      const futureReturn = (marketData[i + 10].close - marketData[i].close) / marketData[i].close;
      const profitable = consensus.decision * futureReturn > 0;
      coordinator.learnFromOutcome({
        profitable,
        pnl: consensus.decision * futureReturn * 10000  // in bps
      });
    }
  }

  console.log(`   Analyzed ${decisions.length} decision points`);
  console.log();

  // 4. Decision summary
  console.log('4. Decision Summary:');
  console.log('─'.repeat(70));

  const longDecisions = decisions.filter(d => d.consensus.decision === 1).length;
  const shortDecisions = decisions.filter(d => d.consensus.decision === -1).length;
  const neutralDecisions = decisions.filter(d => d.consensus.decision === 0).length;

  console.log(`   Long signals:    ${longDecisions} (${(longDecisions / decisions.length * 100).toFixed(1)}%)`);
  console.log(`   Short signals:   ${shortDecisions} (${(shortDecisions / decisions.length * 100).toFixed(1)}%)`);
  console.log(`   Neutral:         ${neutralDecisions} (${(neutralDecisions / decisions.length * 100).toFixed(1)}%)`);
  console.log();

  // 5. Sample decisions
  console.log('5. Sample Decisions (Last 5):');
  console.log('─'.repeat(70));
  console.log('   Index │ Price   │ Decision │ Confidence │ Reason');
  console.log('─'.repeat(70));

  const lastDecisions = decisions.slice(-5);
  for (const d of lastDecisions) {
    const decision = d.consensus.decision === 1 ? 'LONG ' : d.consensus.decision === -1 ? 'SHORT' : 'HOLD ';
    const conf = (d.consensus.confidence * 100).toFixed(0);
    console.log(`   ${String(d.index).padStart(5)} │ $${d.price.toFixed(2).padStart(6)} │ ${decision}    │ ${conf.padStart(6)}%    │ ${d.consensus.reason}`);
  }
  console.log();

  // 6. Agent performance
  console.log('6. Swarm Performance:');
  console.log('─'.repeat(70));

  const stats = coordinator.getSwarmStats();
  console.log(`   Total P&L:       ${stats.totalPnL.toFixed(0)} bps`);
  console.log(`   Win/Loss:        ${stats.totalWins}/${stats.totalLosses}`);
  console.log(`   Win Rate:        ${((stats.totalWins / (stats.totalWins + stats.totalLosses)) * 100).toFixed(1)}%`);
  console.log(`   Avg Confidence:  ${(stats.avgConfidence * 100).toFixed(1)}%`);
  console.log();

  console.log('   Performance by Agent Type:');
  for (const [type, data] of Object.entries(stats.byType)) {
    console.log(`   - ${type.padEnd(15)} P&L: ${data.pnl.toFixed(0).padStart(6)} bps`);
  }
  console.log();

  // 7. Pheromone state
  console.log('7. Pheromone Trails (Signal Strength):');
  console.log('─'.repeat(70));

  for (const [direction, strength] of Object.entries(stats.pheromones)) {
    const bar = '█'.repeat(Math.floor(strength * 40));
    console.log(`   ${direction.padEnd(10)} ${'['.padEnd(1)}${bar.padEnd(40)}] ${(strength * 100).toFixed(1)}%`);
  }
  console.log();

  // 8. Consensus visualization
  console.log('8. Consensus Timeline (Last 20 decisions):');
  console.log('─'.repeat(70));

  const timeline = decisions.slice(-20);
  let timelineStr = '   ';
  for (const d of timeline) {
    if (d.consensus.decision === 1) timelineStr += '▲';
    else if (d.consensus.decision === -1) timelineStr += '▼';
    else timelineStr += '─';
  }
  console.log(timelineStr);
  console.log('   ▲=Long  ▼=Short  ─=Hold');
  console.log();

  console.log('═'.repeat(70));
  console.log('Multi-agent swarm analysis completed');
  console.log('═'.repeat(70));
}

main().catch(console.error);
