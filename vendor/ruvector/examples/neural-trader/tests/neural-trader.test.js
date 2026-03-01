/**
 * Neural-Trader Test Suite
 *
 * Comprehensive tests for all production modules
 */

import { describe, it, expect, beforeEach } from '@jest/globals';

// Mock performance for Node.js
if (typeof performance === 'undefined') {
  global.performance = { now: () => Date.now() };
}

// ============================================================================
// FRACTIONAL KELLY TESTS
// ============================================================================

describe('Fractional Kelly Engine', () => {
  let KellyCriterion, TradingKelly, SportsBettingKelly;

  beforeEach(async () => {
    const module = await import('../production/fractional-kelly.js');
    KellyCriterion = module.KellyCriterion;
    TradingKelly = module.TradingKelly;
    SportsBettingKelly = module.SportsBettingKelly;
  });

  describe('KellyCriterion', () => {
    it('should calculate full Kelly correctly', () => {
      const kelly = new KellyCriterion();
      const result = kelly.calculateFullKelly(0.55, 2.0);
      expect(result).toBeCloseTo(0.10, 2); // 10% for 55% win rate at even odds
    });

    it('should return 0 for negative edge', () => {
      const kelly = new KellyCriterion();
      const result = kelly.calculateFullKelly(0.40, 2.0);
      expect(result).toBeLessThanOrEqual(0);
    });

    it('should apply fractional Kelly correctly', () => {
      const kelly = new KellyCriterion();
      const result = kelly.calculateFractionalKelly(0.55, 2.0, 'conservative');
      expect(result.stake).toBeLessThan(kelly.calculateFullKelly(0.55, 2.0) * 10000);
    });

    it('should handle edge cases', () => {
      const kelly = new KellyCriterion();
      expect(kelly.calculateFullKelly(0, 2.0)).toBeLessThanOrEqual(0);
      expect(kelly.calculateFullKelly(1, 2.0)).toBeGreaterThan(0);
    });
  });

  describe('TradingKelly', () => {
    it('should calculate position size', () => {
      const kelly = new TradingKelly();
      const result = kelly.calculatePositionSize(100000, 0.55, 0.02, 0.015);
      expect(result.positionSize).toBeGreaterThan(0);
      expect(result.positionSize).toBeLessThan(100000);
    });

    it('should respect max position limits', () => {
      const kelly = new TradingKelly();
      const result = kelly.calculatePositionSize(100000, 0.99, 0.10, 0.01);
      expect(result.positionSize).toBeLessThanOrEqual(100000 * 0.20);
    });
  });
});

// ============================================================================
// LSTM-TRANSFORMER TESTS
// ============================================================================

describe('Hybrid LSTM-Transformer', () => {
  let HybridLSTMTransformer, FeatureExtractor, LSTMCell;

  beforeEach(async () => {
    const module = await import('../production/hybrid-lstm-transformer.js');
    HybridLSTMTransformer = module.HybridLSTMTransformer;
    FeatureExtractor = module.FeatureExtractor;
    LSTMCell = module.LSTMCell;
  });

  describe('LSTMCell', () => {
    it('should forward pass correctly', () => {
      const cell = new LSTMCell(10, 64);
      const x = new Array(10).fill(0.1);
      const h = new Array(64).fill(0);
      const c = new Array(64).fill(0);

      const result = cell.forward(x, h, c);
      expect(result.h).toHaveLength(64);
      expect(result.c).toHaveLength(64);
      expect(result.h.every(v => !isNaN(v))).toBe(true);
    });

    it('should handle zero inputs', () => {
      const cell = new LSTMCell(10, 64);
      const x = new Array(10).fill(0);
      const h = new Array(64).fill(0);
      const c = new Array(64).fill(0);

      const result = cell.forward(x, h, c);
      expect(result.h.every(v => isFinite(v))).toBe(true);
    });
  });

  describe('FeatureExtractor', () => {
    it('should extract features from candles', () => {
      const extractor = new FeatureExtractor();
      const candles = [];
      for (let i = 0; i < 100; i++) {
        candles.push({
          open: 100 + Math.random() * 10,
          high: 105 + Math.random() * 10,
          low: 95 + Math.random() * 10,
          close: 100 + Math.random() * 10,
          volume: 1000000
        });
      }

      const features = extractor.extract(candles);
      expect(features.length).toBe(99); // One less than candles
      expect(features[0].length).toBe(10); // 10 features
    });
  });

  describe('HybridLSTMTransformer', () => {
    it('should produce valid predictions', () => {
      const model = new HybridLSTMTransformer();
      const features = [];
      for (let i = 0; i < 50; i++) {
        features.push(new Array(10).fill(0).map(() => Math.random() - 0.5));
      }

      const result = model.predict(features);
      expect(result).toHaveProperty('prediction');
      expect(result).toHaveProperty('confidence');
      expect(result).toHaveProperty('signal');
      expect(['BUY', 'SELL', 'HOLD']).toContain(result.signal);
    });
  });
});

// ============================================================================
// DRL PORTFOLIO MANAGER TESTS
// ============================================================================

describe('DRL Portfolio Manager', () => {
  let EnsemblePortfolioManager, ReplayBuffer, NeuralNetwork;

  beforeEach(async () => {
    const module = await import('../production/drl-portfolio-manager.js');
    EnsemblePortfolioManager = module.EnsemblePortfolioManager;
    ReplayBuffer = module.ReplayBuffer;
    NeuralNetwork = module.NeuralNetwork;
  });

  describe('ReplayBuffer', () => {
    it('should store and sample experiences', () => {
      const buffer = new ReplayBuffer(100);

      for (let i = 0; i < 50; i++) {
        buffer.push(
          new Array(10).fill(i),
          new Array(5).fill(0.2),
          Math.random(),
          new Array(10).fill(i + 1),
          false
        );
      }

      expect(buffer.length).toBe(50);
      const batch = buffer.sample(16);
      expect(batch).toHaveLength(16);
    });

    it('should respect max size', () => {
      const buffer = new ReplayBuffer(10);

      for (let i = 0; i < 20; i++) {
        buffer.push([i], [0.5], 1, [i + 1], false);
      }

      expect(buffer.length).toBe(10);
    });
  });

  describe('NeuralNetwork', () => {
    it('should forward pass correctly', () => {
      const net = new NeuralNetwork([10, 32, 5]);
      const input = new Array(10).fill(0.5);
      const result = net.forward(input);

      expect(result.output).toHaveLength(5);
      expect(result.output.every(v => isFinite(v))).toBe(true);
    });
  });

  describe('EnsemblePortfolioManager', () => {
    it('should produce valid portfolio weights', () => {
      const manager = new EnsemblePortfolioManager(5);
      const state = new Array(62).fill(0).map(() => Math.random());
      const action = manager.getEnsembleAction(state);

      expect(action).toHaveLength(5);
      const sum = action.reduce((a, b) => a + b, 0);
      expect(sum).toBeCloseTo(1, 1); // Weights should sum to ~1
    });
  });
});

// ============================================================================
// SENTIMENT ALPHA TESTS
// ============================================================================

describe('Sentiment Alpha Pipeline', () => {
  let LexiconAnalyzer, EmbeddingAnalyzer, SentimentAggregator;

  beforeEach(async () => {
    const module = await import('../production/sentiment-alpha.js');
    LexiconAnalyzer = module.LexiconAnalyzer;
    EmbeddingAnalyzer = module.EmbeddingAnalyzer;
    SentimentAggregator = module.SentimentAggregator;
  });

  describe('LexiconAnalyzer', () => {
    it('should detect positive sentiment', () => {
      const analyzer = new LexiconAnalyzer();
      const result = analyzer.analyze('Strong growth and profit beat expectations');
      expect(result.score).toBeGreaterThan(0);
      expect(result.positiveCount).toBeGreaterThan(0);
    });

    it('should detect negative sentiment', () => {
      const analyzer = new LexiconAnalyzer();
      const result = analyzer.analyze('Losses and decline amid recession fears');
      expect(result.score).toBeLessThan(0);
      expect(result.negativeCount).toBeGreaterThan(0);
    });

    it('should handle neutral text', () => {
      const analyzer = new LexiconAnalyzer();
      const result = analyzer.analyze('The company released quarterly results');
      expect(Math.abs(result.score)).toBeLessThan(0.5);
    });

    it('should handle negators', () => {
      const analyzer = new LexiconAnalyzer();
      const positive = analyzer.analyze('Strong growth');
      const negated = analyzer.analyze('Not strong growth');
      expect(negated.score).toBeLessThan(positive.score);
    });
  });

  describe('SentimentAggregator', () => {
    it('should aggregate multiple sentiments', () => {
      const aggregator = new SentimentAggregator();

      aggregator.addSentiment('AAPL', { source: 'news', score: 0.5, confidence: 0.8 });
      aggregator.addSentiment('AAPL', { source: 'social', score: 0.3, confidence: 0.6 });

      const result = aggregator.getAggregatedSentiment('AAPL');
      expect(result.score).toBeGreaterThan(0);
      expect(result.count).toBe(2);
    });
  });
});

// ============================================================================
// BACKTESTING TESTS
// ============================================================================

describe('Backtesting Framework', () => {
  let PerformanceMetrics, BacktestEngine;

  beforeEach(async () => {
    const module = await import('../system/backtesting.js');
    PerformanceMetrics = module.PerformanceMetrics;
    BacktestEngine = module.BacktestEngine;
  });

  describe('PerformanceMetrics', () => {
    it('should calculate metrics from equity curve', () => {
      const metrics = new PerformanceMetrics(0.05);
      const equityCurve = [100000];

      // Generate random walk
      for (let i = 0; i < 252; i++) {
        const lastValue = equityCurve[equityCurve.length - 1];
        equityCurve.push(lastValue * (1 + (Math.random() - 0.48) * 0.02));
      }

      const result = metrics.calculate(equityCurve);

      expect(result).toHaveProperty('totalReturn');
      expect(result).toHaveProperty('sharpeRatio');
      expect(result).toHaveProperty('maxDrawdown');
      expect(result.tradingDays).toBe(252);
    });

    it('should handle edge cases', () => {
      const metrics = new PerformanceMetrics();

      // Empty curve
      expect(metrics.calculate([]).totalReturn).toBe(0);

      // Single value
      expect(metrics.calculate([100]).totalReturn).toBe(0);
    });

    it('should compute drawdown correctly', () => {
      const metrics = new PerformanceMetrics();
      const equityCurve = [100, 110, 105, 95, 100, 90, 95];

      const result = metrics.calculate(equityCurve);
      expect(result.maxDrawdown).toBeCloseTo(0.182, 2); // (110-90)/110
    });
  });
});

// ============================================================================
// RISK MANAGEMENT TESTS
// ============================================================================

describe('Risk Management', () => {
  let RiskManager, CircuitBreaker, StopLossManager;

  beforeEach(async () => {
    const module = await import('../system/risk-management.js');
    RiskManager = module.RiskManager;
    CircuitBreaker = module.CircuitBreaker;
    StopLossManager = module.StopLossManager;
  });

  describe('StopLossManager', () => {
    it('should set and check stops', () => {
      const manager = new StopLossManager();
      manager.setStop('AAPL', 150, 'fixed');

      const check = manager.checkStop('AAPL', 140);
      expect(check.triggered).toBe(true);
    });

    it('should update trailing stops', () => {
      const manager = new StopLossManager();
      manager.setStop('AAPL', 100, 'trailing');

      manager.updateTrailingStop('AAPL', 110);
      const stop = manager.stops.get('AAPL');
      expect(stop.highWaterMark).toBe(110);
      expect(stop.stopPrice).toBeGreaterThan(100 * 0.97);
    });
  });

  describe('CircuitBreaker', () => {
    it('should trip on consecutive losses', () => {
      const breaker = new CircuitBreaker({ consecutiveLosses: 3 });

      breaker.recordTrade(-100);
      breaker.recordTrade(-100);
      expect(breaker.canTrade().allowed).toBe(true);

      breaker.recordTrade(-100);
      expect(breaker.canTrade().allowed).toBe(false);
    });

    it('should reset after cooldown', () => {
      const breaker = new CircuitBreaker({
        consecutiveLosses: 2,
        drawdownCooldown: 100 // 100ms for testing
      });

      breaker.recordTrade(-100);
      breaker.recordTrade(-100);
      expect(breaker.canTrade().allowed).toBe(false);

      // Wait for cooldown
      return new Promise(resolve => {
        setTimeout(() => {
          expect(breaker.canTrade().allowed).toBe(true);
          resolve();
        }, 150);
      });
    });

    it('should trip on drawdown', () => {
      const breaker = new CircuitBreaker({ drawdownThreshold: 0.10 });

      breaker.updateEquity(100000);
      breaker.updateEquity(88000); // 12% drawdown

      expect(breaker.canTrade().allowed).toBe(false);
    });
  });

  describe('RiskManager', () => {
    it('should check trade limits', () => {
      const manager = new RiskManager();
      manager.startDay(100000);

      const portfolio = {
        equity: 100000,
        cash: 50000,
        positions: {}
      };

      const trade = { symbol: 'AAPL', side: 'buy', value: 5000 };
      const result = manager.canTrade('AAPL', trade, portfolio);

      expect(result.allowed).toBe(true);
    });

    it('should block oversized trades', () => {
      const manager = new RiskManager();

      const portfolio = {
        equity: 100000,
        cash: 50000,
        positions: {}
      };

      const trade = { symbol: 'AAPL', side: 'buy', value: 60000 };
      const result = manager.canTrade('AAPL', trade, portfolio);

      expect(result.checks.positionSize.violations.length).toBeGreaterThan(0);
    });
  });
});

// ============================================================================
// TRADING PIPELINE TESTS
// ============================================================================

describe('Trading Pipeline', () => {
  let TradingPipeline, TradingDag;

  beforeEach(async () => {
    const module = await import('../system/trading-pipeline.js');
    TradingPipeline = module.TradingPipeline;
    TradingDag = module.TradingDag;
  });

  describe('TradingDag', () => {
    it('should execute nodes in order', async () => {
      const dag = new TradingDag();
      const order = [];

      dag.addNode({
        id: 'a',
        name: 'Node A',
        dependencies: [],
        status: 'pending',
        executor: async () => { order.push('a'); return 'A'; }
      });

      dag.addNode({
        id: 'b',
        name: 'Node B',
        dependencies: ['a'],
        status: 'pending',
        executor: async (ctx, deps) => { order.push('b'); return deps.a + 'B'; }
      });

      await dag.execute({});

      expect(order).toEqual(['a', 'b']);
      expect(dag.results.get('b')).toBe('AB');
    });

    it('should execute parallel nodes concurrently', async () => {
      const dag = new TradingDag({ parallelExecution: true, maxConcurrency: 2 });
      const timestamps = {};

      dag.addNode({
        id: 'a',
        dependencies: [],
        status: 'pending',
        executor: async () => { timestamps.a = Date.now(); return 'A'; }
      });

      dag.addNode({
        id: 'b',
        dependencies: [],
        status: 'pending',
        executor: async () => { timestamps.b = Date.now(); return 'B'; }
      });

      await dag.execute({});

      // Both should start at nearly the same time
      expect(Math.abs(timestamps.a - timestamps.b)).toBeLessThan(50);
    });
  });
});

// ============================================================================
// INTEGRATION TESTS
// ============================================================================

describe('Integration Tests', () => {
  it('should run complete pipeline', async () => {
    const { createTradingPipeline } = await import('../system/trading-pipeline.js');
    const pipeline = createTradingPipeline();

    const generateCandles = (n) => {
      const candles = [];
      let price = 100;
      for (let i = 0; i < n; i++) {
        price *= (1 + (Math.random() - 0.5) * 0.02);
        candles.push({
          open: price * 0.99,
          high: price * 1.01,
          low: price * 0.98,
          close: price,
          volume: 1000000
        });
      }
      return candles;
    };

    const context = {
      marketData: generateCandles(100),
      newsData: [
        { symbol: 'TEST', text: 'Strong earnings growth', source: 'news' }
      ],
      symbols: ['TEST'],
      portfolio: { equity: 100000, cash: 50000, positions: {}, assets: ['TEST'] },
      prices: { TEST: 100 }
    };

    const result = await pipeline.execute(context);

    expect(result).toHaveProperty('signals');
    expect(result).toHaveProperty('positions');
    expect(result).toHaveProperty('orders');
    expect(result).toHaveProperty('metrics');
  });
});

export {};
