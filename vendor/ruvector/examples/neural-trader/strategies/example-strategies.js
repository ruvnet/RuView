/**
 * Example Trading Strategies
 *
 * Ready-to-run combined strategies using all production modules
 */

import { createTradingPipeline } from '../system/trading-pipeline.js';
import { BacktestEngine } from '../system/backtesting.js';
import { RiskManager } from '../system/risk-management.js';
import { KellyCriterion, TradingKelly } from '../production/fractional-kelly.js';
import { HybridLSTMTransformer } from '../production/hybrid-lstm-transformer.js';
import { LexiconAnalyzer, SentimentAggregator, AlphaFactorCalculator } from '../production/sentiment-alpha.js';
import { Dashboard, viz } from '../system/visualization.js';

// ============================================================================
// STRATEGY 1: Hybrid Momentum
// Combines LSTM predictions with sentiment for trend following
// ============================================================================

class HybridMomentumStrategy {
  constructor(config = {}) {
    this.config = {
      lookback: 50,
      signalThreshold: 0.15,
      kellyFraction: 'conservative',
      maxPosition: 0.15,
      ...config
    };

    this.lstm = new HybridLSTMTransformer();
    this.lexicon = new LexiconAnalyzer();
    this.kelly = new TradingKelly();
  }

  analyze(marketData, newsData = []) {
    // Get LSTM prediction (predict() internally extracts features from candles)
    const lstmPrediction = this.lstm.predict(marketData);

    // Handle insufficient data
    if (lstmPrediction.error) {
      return {
        signal: 'HOLD',
        strength: 0,
        confidence: 0,
        components: { lstm: 0, sentiment: 0 },
        error: lstmPrediction.error
      };
    }

    // Get sentiment signal
    let sentimentScore = 0;
    for (const news of newsData) {
      const result = this.lexicon.analyze(news.text);
      sentimentScore += result.score * result.confidence;
    }
    sentimentScore = newsData.length > 0 ? sentimentScore / newsData.length : 0;

    // Combine signals
    const combinedSignal = lstmPrediction.prediction * 0.6 + sentimentScore * 0.4;

    return {
      signal: combinedSignal > this.config.signalThreshold ? 'BUY' :
              combinedSignal < -this.config.signalThreshold ? 'SELL' : 'HOLD',
      strength: Math.abs(combinedSignal),
      confidence: lstmPrediction.confidence,
      components: {
        lstm: lstmPrediction.prediction,
        sentiment: sentimentScore
      }
    };
  }

  getPositionSize(equity, signal) {
    if (signal.signal === 'HOLD') return 0;

    const winProb = 0.5 + signal.strength * signal.confidence * 0.15;
    const result = this.kelly.calculatePositionSize(
      equity, winProb, 0.02, 0.015, this.config.kellyFraction
    );

    return Math.min(result.positionSize, equity * this.config.maxPosition);
  }
}

// ============================================================================
// STRATEGY 2: Mean Reversion with Sentiment Filter
// Buys oversold conditions when sentiment is not extremely negative
// ============================================================================

class MeanReversionStrategy {
  constructor(config = {}) {
    this.config = {
      rsiPeriod: 14,
      oversoldLevel: 30,
      overboughtLevel: 70,
      sentimentFilter: -0.5,  // Block trades if sentiment below this
      ...config
    };

    this.lexicon = new LexiconAnalyzer();
    this.kelly = new KellyCriterion();
  }

  calculateRSI(prices, period = 14) {
    if (prices.length < period + 1) return 50;

    let gains = 0, losses = 0;
    for (let i = prices.length - period; i < prices.length; i++) {
      const change = prices[i] - prices[i - 1];
      if (change > 0) gains += change;
      else losses -= change;
    }

    const avgGain = gains / period;
    const avgLoss = losses / period;
    const rs = avgLoss === 0 ? 100 : avgGain / avgLoss;
    return 100 - (100 / (1 + rs));
  }

  analyze(marketData, newsData = []) {
    const prices = marketData.map(d => d.close);
    const rsi = this.calculateRSI(prices, this.config.rsiPeriod);

    // Get sentiment filter
    let sentiment = 0;
    for (const news of newsData) {
      const result = this.lexicon.analyze(news.text);
      sentiment += result.score;
    }
    sentiment = newsData.length > 0 ? sentiment / newsData.length : 0;

    // Generate signal
    let signal = 'HOLD';
    let strength = 0;

    if (rsi < this.config.oversoldLevel && sentiment > this.config.sentimentFilter) {
      signal = 'BUY';
      strength = (this.config.oversoldLevel - rsi) / this.config.oversoldLevel;
    } else if (rsi > this.config.overboughtLevel) {
      signal = 'SELL';
      strength = (rsi - this.config.overboughtLevel) / (100 - this.config.overboughtLevel);
    }

    return {
      signal,
      strength,
      confidence: Math.min(strength, 0.8),
      components: {
        rsi,
        sentiment,
        sentimentBlocked: sentiment <= this.config.sentimentFilter
      }
    };
  }

  getPositionSize(equity, signal) {
    if (signal.signal === 'HOLD') return 0;

    const kellyResult = this.kelly.calculateFractionalKelly(
      0.52 + signal.strength * 0.08,
      2.0,
      'conservative'
    );

    return Math.min(kellyResult.stake, equity * 0.10);
  }
}

// ============================================================================
// STRATEGY 3: Sentiment Momentum
// Pure sentiment-based trading with momentum confirmation
// ============================================================================

class SentimentMomentumStrategy {
  constructor(config = {}) {
    this.config = {
      sentimentThreshold: 0.3,
      momentumWindow: 10,
      momentumThreshold: 0.02,
      ...config
    };

    this.aggregator = new SentimentAggregator();
    this.alphaCalc = new AlphaFactorCalculator(this.aggregator);
    this.lexicon = new LexiconAnalyzer();
    this.kelly = new TradingKelly();
  }

  analyze(marketData, newsData = [], symbol = 'DEFAULT') {
    // Process news sentiment
    for (const news of newsData) {
      this.aggregator.addObservation(
        symbol,
        news.source || 'news',
        news.text,
        Date.now()
      );
    }

    const sentiment = this.aggregator.getAggregatedSentiment(symbol);
    const alpha = this.alphaCalc.calculateAlpha(symbol, this.aggregator);

    // Calculate price momentum
    const prices = marketData.slice(-this.config.momentumWindow).map(d => d.close);
    const momentum = prices.length >= 2
      ? (prices[prices.length - 1] - prices[0]) / prices[0]
      : 0;

    // Generate signal
    let signal = 'HOLD';
    let strength = 0;

    const sentimentStrong = Math.abs(sentiment.score) > this.config.sentimentThreshold;
    const momentumConfirms = (sentiment.score > 0 && momentum > this.config.momentumThreshold) ||
                             (sentiment.score < 0 && momentum < -this.config.momentumThreshold);

    if (sentimentStrong && momentumConfirms) {
      signal = sentiment.score > 0 ? 'BUY' : 'SELL';
      strength = Math.min(Math.abs(sentiment.score), 1);
    }

    return {
      signal,
      strength,
      confidence: sentiment.confidence,
      components: {
        sentimentScore: sentiment.score,
        sentimentConfidence: sentiment.confidence,
        momentum,
        alpha: alpha.factor
      }
    };
  }

  getPositionSize(equity, signal) {
    if (signal.signal === 'HOLD') return 0;

    const winProb = 0.5 + signal.strength * 0.1;
    const result = this.kelly.calculatePositionSize(
      equity, winProb, 0.025, 0.018, 'moderate'
    );

    return result.positionSize;
  }
}

// ============================================================================
// STRATEGY RUNNER
// ============================================================================

class StrategyRunner {
  constructor(strategy, config = {}) {
    this.strategy = strategy;
    this.config = {
      initialCapital: 100000,
      riskManager: new RiskManager(),
      ...config
    };

    this.portfolio = {
      equity: this.config.initialCapital,
      cash: this.config.initialCapital,
      positions: {}
    };

    this.trades = [];
    this.equityCurve = [this.config.initialCapital];
  }

  run(marketData, newsData = [], symbol = 'DEFAULT') {
    this.config.riskManager.startDay(this.portfolio.equity);

    // Get strategy signal
    const analysis = this.strategy.analyze(marketData, newsData, symbol);

    // Check risk limits
    const riskCheck = this.config.riskManager.canTrade(symbol, {
      symbol,
      side: analysis.signal === 'BUY' ? 'buy' : 'sell',
      value: this.strategy.getPositionSize(this.portfolio.equity, analysis)
    }, this.portfolio);

    if (!riskCheck.allowed && analysis.signal !== 'HOLD') {
      analysis.blocked = true;
      analysis.blockReason = riskCheck.checks;
    }

    // Execute if allowed
    if (!analysis.blocked && analysis.signal !== 'HOLD') {
      const positionSize = this.strategy.getPositionSize(this.portfolio.equity, analysis);
      const currentPrice = marketData[marketData.length - 1].close;
      const shares = Math.floor(positionSize / currentPrice);

      if (shares > 0) {
        const trade = {
          symbol,
          side: analysis.signal.toLowerCase(),
          shares,
          price: currentPrice,
          value: shares * currentPrice,
          timestamp: Date.now(),
          signal: analysis
        };

        // Update portfolio
        if (trade.side === 'buy') {
          this.portfolio.cash -= trade.value;
          this.portfolio.positions[symbol] = (this.portfolio.positions[symbol] || 0) + shares;
        } else {
          this.portfolio.cash += trade.value;
          this.portfolio.positions[symbol] = (this.portfolio.positions[symbol] || 0) - shares;
        }

        this.trades.push(trade);
      }
    }

    // Update equity
    let positionValue = 0;
    const currentPrice = marketData[marketData.length - 1].close;
    for (const [sym, qty] of Object.entries(this.portfolio.positions)) {
      positionValue += qty * currentPrice;
    }
    this.portfolio.equity = this.portfolio.cash + positionValue;
    this.equityCurve.push(this.portfolio.equity);

    return analysis;
  }

  getStats() {
    const { PerformanceMetrics } = require('../system/backtesting.js');
    const metrics = new PerformanceMetrics();
    return {
      portfolio: this.portfolio,
      trades: this.trades,
      metrics: metrics.calculate(this.equityCurve)
    };
  }
}

// ============================================================================
// DEMO
// ============================================================================

async function demo() {
  console.log('═'.repeat(70));
  console.log('EXAMPLE STRATEGIES DEMO');
  console.log('═'.repeat(70));
  console.log();

  // Generate sample data
  const generateMarketData = (days) => {
    const data = [];
    let price = 100;
    for (let i = 0; i < days; i++) {
      price *= (1 + (Math.random() - 0.48) * 0.02);
      data.push({
        open: price * 0.995,
        high: price * 1.01,
        low: price * 0.99,
        close: price,
        volume: 1000000
      });
    }
    return data;
  };

  const marketData = generateMarketData(100);
  const newsData = [
    { text: 'Strong quarterly earnings beat analyst expectations', source: 'news' },
    { text: 'New product launch receives positive reception', source: 'social' }
  ];

  // Test each strategy
  const strategies = [
    { name: 'Hybrid Momentum', instance: new HybridMomentumStrategy() },
    { name: 'Mean Reversion', instance: new MeanReversionStrategy() },
    { name: 'Sentiment Momentum', instance: new SentimentMomentumStrategy() }
  ];

  for (const { name, instance } of strategies) {
    console.log(`\n${name} Strategy:`);
    console.log('─'.repeat(50));

    const analysis = instance.analyze(marketData, newsData);
    console.log(`  Signal: ${analysis.signal}`);
    console.log(`  Strength: ${(analysis.strength * 100).toFixed(1)}%`);
    console.log(`  Confidence: ${(analysis.confidence * 100).toFixed(1)}%`);

    if (analysis.components) {
      console.log('  Components:');
      for (const [key, value] of Object.entries(analysis.components)) {
        if (typeof value === 'number') {
          console.log(`    ${key}: ${value.toFixed(4)}`);
        } else {
          console.log(`    ${key}: ${value}`);
        }
      }
    }

    const positionSize = instance.getPositionSize(100000, analysis);
    console.log(`  Position Size: $${positionSize.toFixed(2)}`);
  }

  console.log();
  console.log('═'.repeat(70));
  console.log('Strategies demo completed');
  console.log('═'.repeat(70));
}

// Export
export {
  HybridMomentumStrategy,
  MeanReversionStrategy,
  SentimentMomentumStrategy,
  StrategyRunner
};

// Run demo if executed directly
const isMainModule = import.meta.url === `file://${process.argv[1]}`;
if (isMainModule) {
  demo().catch(console.error);
}
