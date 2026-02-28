/**
 * Backtesting Framework
 *
 * Historical simulation with comprehensive performance metrics:
 * - Sharpe Ratio, Sortino Ratio
 * - Maximum Drawdown, Calmar Ratio
 * - Win Rate, Profit Factor
 * - Value at Risk (VaR), Expected Shortfall
 * - Rolling statistics and regime analysis
 */

import { TradingPipeline, createTradingPipeline } from './trading-pipeline.js';

// Backtesting Configuration
const backtestConfig = {
  // Simulation settings
  simulation: {
    initialCapital: 100000,
    startDate: null,      // Use all available data if null
    endDate: null,
    rebalanceFrequency: 'daily',  // daily, weekly, monthly
    warmupPeriod: 50      // Days for indicator warmup
  },

  // Execution assumptions
  execution: {
    slippage: 0.001,      // 0.1%
    commission: 0.001,    // 0.1%
    marketImpact: 0.0005, // 0.05% for large orders
    fillRate: 1.0         // 100% fill rate assumed
  },

  // Risk-free rate for Sharpe calculation
  riskFreeRate: 0.05,     // 5% annual

  // Benchmark
  benchmark: 'buyAndHold'  // buyAndHold, equalWeight, or custom
};

/**
 * Performance Metrics Calculator
 */
class PerformanceMetrics {
  constructor(riskFreeRate = 0.05) {
    this.riskFreeRate = riskFreeRate;
    this.dailyRiskFreeRate = Math.pow(1 + riskFreeRate, 1/252) - 1;
  }

  // Optimized: Calculate all metrics with minimal passes over data
  calculate(equityCurve, benchmark = null) {
    if (equityCurve.length < 2) {
      return this.emptyMetrics();
    }

    // Single pass: compute returns and statistics together
    const n = equityCurve.length;
    const returns = new Array(n - 1);
    let sum = 0, sumSq = 0;
    let positiveSum = 0, negativeSum = 0;
    let positiveCount = 0, negativeCount = 0;
    let compoundReturn = 1;

    for (let i = 1; i < n; i++) {
      const r = (equityCurve[i] - equityCurve[i-1]) / equityCurve[i-1];
      returns[i-1] = r;
      sum += r;
      sumSq += r * r;
      compoundReturn *= (1 + r);
      if (r > 0) { positiveSum += r; positiveCount++; }
      else if (r < 0) { negativeSum += r; negativeCount++; }
    }

    const mean = sum / returns.length;
    const variance = sumSq / returns.length - mean * mean;
    const volatility = Math.sqrt(variance);
    const annualizedVol = volatility * Math.sqrt(252);

    // Single pass: drawdown metrics
    const ddMetrics = this.computeDrawdownMetrics(equityCurve);

    // Pre-computed stats for Sharpe/Sortino
    const excessMean = mean - this.dailyRiskFreeRate;
    const sharpe = volatility > 0 ? (excessMean / volatility) * Math.sqrt(252) : 0;

    // Downside deviation (single pass)
    let downsideVariance = 0;
    for (let i = 0; i < returns.length; i++) {
      const excess = returns[i] - this.dailyRiskFreeRate;
      if (excess < 0) downsideVariance += excess * excess;
    }
    const downsideDeviation = Math.sqrt(downsideVariance / returns.length);
    const sortino = downsideDeviation > 0 ? (excessMean / downsideDeviation) * Math.sqrt(252) : 0;

    // Annualized return
    const years = returns.length / 252;
    const annualizedReturn = Math.pow(compoundReturn, 1 / years) - 1;

    // CAGR
    const cagr = Math.pow(equityCurve[n-1] / equityCurve[0], 1 / years) - 1;

    // Calmar
    const calmar = ddMetrics.maxDrawdown > 0 ? annualizedReturn / ddMetrics.maxDrawdown : 0;

    // Trade metrics (using pre-computed counts)
    const winRate = returns.length > 0 ? positiveCount / returns.length : 0;
    const avgWin = positiveCount > 0 ? positiveSum / positiveCount : 0;
    const avgLoss = negativeCount > 0 ? negativeSum / negativeCount : 0;
    const profitFactor = negativeSum !== 0 ? positiveSum / Math.abs(negativeSum) : Infinity;
    const payoffRatio = avgLoss !== 0 ? avgWin / Math.abs(avgLoss) : Infinity;
    const expectancy = winRate * avgWin - (1 - winRate) * Math.abs(avgLoss);

    // VaR (requires sort - do lazily)
    const sortedReturns = [...returns].sort((a, b) => a - b);
    const var95 = -sortedReturns[Math.floor(0.05 * sortedReturns.length)];
    const var99 = -sortedReturns[Math.floor(0.01 * sortedReturns.length)];

    // CVaR
    const tailIndex = Math.floor(0.05 * sortedReturns.length);
    let cvarSum = 0;
    for (let i = 0; i <= tailIndex; i++) cvarSum += sortedReturns[i];
    const cvar95 = tailIndex > 0 ? -cvarSum / (tailIndex + 1) : 0;

    // Skewness and Kurtosis (using pre-computed mean/variance)
    let m3 = 0, m4 = 0;
    for (let i = 0; i < returns.length; i++) {
      const d = returns[i] - mean;
      const d2 = d * d;
      m3 += d * d2;
      m4 += d2 * d2;
    }
    m3 /= returns.length;
    m4 /= returns.length;
    const std = volatility;
    const skewness = std > 0 ? m3 / (std * std * std) : 0;
    const kurtosis = std > 0 ? m4 / (std * std * std * std) - 3 : 0;

    // Best/worst day
    let bestDay = returns[0], worstDay = returns[0];
    for (let i = 1; i < returns.length; i++) {
      if (returns[i] > bestDay) bestDay = returns[i];
      if (returns[i] < worstDay) worstDay = returns[i];
    }

    // Benchmark metrics
    let informationRatio = null;
    if (benchmark) {
      informationRatio = this.informationRatioFast(returns, benchmark);
    }

    return {
      totalReturn: compoundReturn - 1,
      annualizedReturn,
      cagr,
      volatility,
      annualizedVolatility: annualizedVol,
      maxDrawdown: ddMetrics.maxDrawdown,
      averageDrawdown: ddMetrics.averageDrawdown,
      drawdownDuration: ddMetrics.maxDuration,
      sharpeRatio: sharpe,
      sortinoRatio: sortino,
      calmarRatio: calmar,
      informationRatio,
      winRate,
      profitFactor,
      averageWin: avgWin,
      averageLoss: avgLoss,
      payoffRatio,
      expectancy,
      var95,
      var99,
      cvar95,
      skewness,
      kurtosis,
      tradingDays: returns.length,
      bestDay,
      worstDay,
      positiveMonths: this.positiveMonthsFast(returns),
      returns,
      equityCurve
    };
  }

  // Optimized: Single pass drawdown computation
  computeDrawdownMetrics(equityCurve) {
    let maxDrawdown = 0;
    let peak = equityCurve[0];
    let ddSum = 0;
    let maxDuration = 0;
    let currentDuration = 0;

    for (let i = 0; i < equityCurve.length; i++) {
      const value = equityCurve[i];
      if (value > peak) {
        peak = value;
        currentDuration = 0;
      } else {
        currentDuration++;
        if (currentDuration > maxDuration) maxDuration = currentDuration;
      }
      const dd = (peak - value) / peak;
      ddSum += dd;
      if (dd > maxDrawdown) maxDrawdown = dd;
    }

    return {
      maxDrawdown,
      averageDrawdown: ddSum / equityCurve.length,
      maxDuration
    };
  }

  // Optimized information ratio
  informationRatioFast(returns, benchmark) {
    const benchmarkReturns = this.calculateReturns(benchmark);
    const minLen = Math.min(returns.length, benchmarkReturns.length);
    let sum = 0, sumSq = 0;

    for (let i = 0; i < minLen; i++) {
      const te = returns[i] - benchmarkReturns[i];
      sum += te;
      sumSq += te * te;
    }

    const mean = sum / minLen;
    const variance = sumSq / minLen - mean * mean;
    const vol = Math.sqrt(variance);
    return vol > 0 ? (mean / vol) * Math.sqrt(252) : 0;
  }

  // Optimized positive months
  positiveMonthsFast(returns) {
    let positiveMonths = 0;
    let totalMonths = 0;
    let monthReturn = 1;

    for (let i = 0; i < returns.length; i++) {
      monthReturn *= (1 + returns[i]);
      if ((i + 1) % 21 === 0 || i === returns.length - 1) {
        if (monthReturn > 1) positiveMonths++;
        totalMonths++;
        monthReturn = 1;
      }
    }

    return totalMonths > 0 ? positiveMonths / totalMonths : 0;
  }

  calculateReturns(equityCurve) {
    const returns = new Array(equityCurve.length - 1);
    for (let i = 1; i < equityCurve.length; i++) {
      returns[i-1] = (equityCurve[i] - equityCurve[i-1]) / equityCurve[i-1];
    }
    return returns;
  }

  emptyMetrics() {
    return {
      totalReturn: 0, annualizedReturn: 0, cagr: 0,
      volatility: 0, annualizedVolatility: 0, maxDrawdown: 0,
      sharpeRatio: 0, sortinoRatio: 0, calmarRatio: 0,
      winRate: 0, profitFactor: 0, expectancy: 0,
      var95: 0, var99: 0, cvar95: 0,
      tradingDays: 0, returns: [], equityCurve: []
    };
  }
}

/**
 * Backtest Engine
 */
class BacktestEngine {
  constructor(config = backtestConfig) {
    this.config = config;
    this.metricsCalculator = new PerformanceMetrics(config.riskFreeRate);
    this.pipeline = createTradingPipeline();
  }

  // Run backtest on historical data
  async run(historicalData, options = {}) {
    const {
      symbols = ['DEFAULT'],
      newsData = [],
      riskManager = null
    } = options;

    const results = {
      equityCurve: [this.config.simulation.initialCapital],
      benchmarkCurve: [this.config.simulation.initialCapital],
      trades: [],
      dailyReturns: [],
      positions: [],
      signals: []
    };

    // Initialize portfolio
    let portfolio = {
      equity: this.config.simulation.initialCapital,
      cash: this.config.simulation.initialCapital,
      positions: {},
      assets: symbols
    };

    // Skip warmup period
    const startIndex = this.config.simulation.warmupPeriod;
    const prices = {};

    // Process each day
    for (let i = startIndex; i < historicalData.length; i++) {
      const dayData = historicalData[i];
      const currentPrice = dayData.close || dayData.price || 100;

      // Update prices
      for (const symbol of symbols) {
        prices[symbol] = currentPrice;
      }

      // Get historical window for pipeline
      const windowStart = Math.max(0, i - 100);
      const marketWindow = historicalData.slice(windowStart, i + 1);

      // Get news for this day (simplified - would filter by date in production)
      const dayNews = newsData.filter((n, idx) => idx < 3);

      // Execute pipeline
      const context = {
        marketData: marketWindow,
        newsData: dayNews,
        symbols,
        portfolio,
        prices,
        riskManager
      };

      try {
        const pipelineResult = await this.pipeline.execute(context);

        // Store signals
        if (pipelineResult.signals) {
          results.signals.push({
            day: i,
            signals: pipelineResult.signals
          });
        }

        // Execute orders
        if (pipelineResult.orders && pipelineResult.orders.length > 0) {
          for (const order of pipelineResult.orders) {
            const trade = this.executeTrade(order, portfolio, prices);
            if (trade) {
              results.trades.push({ day: i, ...trade });
            }
          }
        }
      } catch (error) {
        // Pipeline error - skip this day
        console.warn(`Day ${i} pipeline error:`, error.message);
      }

      // Update portfolio value
      portfolio.equity = portfolio.cash;
      for (const [symbol, qty] of Object.entries(portfolio.positions)) {
        portfolio.equity += qty * (prices[symbol] || 0);
      }

      results.equityCurve.push(portfolio.equity);
      results.positions.push({ ...portfolio.positions });

      // Update benchmark (buy and hold)
      const benchmarkReturn = i > startIndex
        ? (currentPrice / historicalData[i - 1].close) - 1
        : 0;
      const lastBenchmark = results.benchmarkCurve[results.benchmarkCurve.length - 1];
      results.benchmarkCurve.push(lastBenchmark * (1 + benchmarkReturn));

      // Daily return
      if (results.equityCurve.length >= 2) {
        const prev = results.equityCurve[results.equityCurve.length - 2];
        const curr = results.equityCurve[results.equityCurve.length - 1];
        results.dailyReturns.push((curr - prev) / prev);
      }
    }

    // Calculate performance metrics
    results.metrics = this.metricsCalculator.calculate(
      results.equityCurve,
      results.benchmarkCurve
    );

    results.benchmarkMetrics = this.metricsCalculator.calculate(
      results.benchmarkCurve
    );

    // Trade statistics
    results.tradeStats = this.calculateTradeStats(results.trades);

    return results;
  }

  // Execute a trade
  executeTrade(order, portfolio, prices) {
    const price = prices[order.symbol] || order.price;
    const value = order.quantity * price;
    const costs = value * (this.config.execution.slippage + this.config.execution.commission);

    if (order.side === 'buy') {
      if (portfolio.cash < value + costs) {
        return null;  // Insufficient funds
      }
      portfolio.cash -= value + costs;
      portfolio.positions[order.symbol] = (portfolio.positions[order.symbol] || 0) + order.quantity;
    } else {
      const currentQty = portfolio.positions[order.symbol] || 0;
      if (currentQty < order.quantity) {
        return null;  // Insufficient shares
      }
      portfolio.cash += value - costs;
      portfolio.positions[order.symbol] = currentQty - order.quantity;
    }

    return {
      symbol: order.symbol,
      side: order.side,
      quantity: order.quantity,
      price,
      value,
      costs,
      timestamp: Date.now()
    };
  }

  // Calculate trade statistics
  calculateTradeStats(trades) {
    if (trades.length === 0) {
      return { totalTrades: 0, buyTrades: 0, sellTrades: 0, totalVolume: 0, totalCosts: 0 };
    }

    return {
      totalTrades: trades.length,
      buyTrades: trades.filter(t => t.side === 'buy').length,
      sellTrades: trades.filter(t => t.side === 'sell').length,
      totalVolume: trades.reduce((a, t) => a + t.value, 0),
      totalCosts: trades.reduce((a, t) => a + t.costs, 0),
      avgTradeSize: trades.reduce((a, t) => a + t.value, 0) / trades.length
    };
  }

  // Generate backtest report
  generateReport(results) {
    const m = results.metrics;
    const b = results.benchmarkMetrics;
    const t = results.tradeStats;

    return `
══════════════════════════════════════════════════════════════════════
BACKTEST REPORT
══════════════════════════════════════════════════════════════════════

PERFORMANCE SUMMARY
──────────────────────────────────────────────────────────────────────
                        Strategy      Benchmark     Difference
Total Return:           ${(m.totalReturn * 100).toFixed(2)}%        ${(b.totalReturn * 100).toFixed(2)}%         ${((m.totalReturn - b.totalReturn) * 100).toFixed(2)}%
Annualized Return:      ${(m.annualizedReturn * 100).toFixed(2)}%        ${(b.annualizedReturn * 100).toFixed(2)}%         ${((m.annualizedReturn - b.annualizedReturn) * 100).toFixed(2)}%
CAGR:                   ${(m.cagr * 100).toFixed(2)}%        ${(b.cagr * 100).toFixed(2)}%         ${((m.cagr - b.cagr) * 100).toFixed(2)}%

RISK METRICS
──────────────────────────────────────────────────────────────────────
Volatility (Ann.):      ${(m.annualizedVolatility * 100).toFixed(2)}%        ${(b.annualizedVolatility * 100).toFixed(2)}%
Max Drawdown:           ${(m.maxDrawdown * 100).toFixed(2)}%        ${(b.maxDrawdown * 100).toFixed(2)}%
Avg Drawdown:           ${(m.averageDrawdown * 100).toFixed(2)}%
DD Duration (days):     ${m.drawdownDuration}

RISK-ADJUSTED RETURNS
──────────────────────────────────────────────────────────────────────
Sharpe Ratio:           ${m.sharpeRatio.toFixed(2)}           ${b.sharpeRatio.toFixed(2)}
Sortino Ratio:          ${m.sortinoRatio.toFixed(2)}           ${b.sortinoRatio.toFixed(2)}
Calmar Ratio:           ${m.calmarRatio.toFixed(2)}           ${b.calmarRatio.toFixed(2)}
Information Ratio:      ${m.informationRatio?.toFixed(2) || 'N/A'}

TRADE STATISTICS
──────────────────────────────────────────────────────────────────────
Win Rate:               ${(m.winRate * 100).toFixed(1)}%
Profit Factor:          ${m.profitFactor.toFixed(2)}
Avg Win:                ${(m.averageWin * 100).toFixed(2)}%
Avg Loss:               ${(m.averageLoss * 100).toFixed(2)}%
Payoff Ratio:           ${m.payoffRatio.toFixed(2)}
Expectancy:             ${(m.expectancy * 100).toFixed(3)}%

TAIL RISK
──────────────────────────────────────────────────────────────────────
VaR (95%):              ${(m.var95 * 100).toFixed(2)}%
VaR (99%):              ${(m.var99 * 100).toFixed(2)}%
CVaR (95%):             ${(m.cvar95 * 100).toFixed(2)}%
Skewness:               ${m.skewness.toFixed(2)}
Kurtosis:               ${m.kurtosis.toFixed(2)}

TRADING ACTIVITY
──────────────────────────────────────────────────────────────────────
Total Trades:           ${t.totalTrades}
Buy Trades:             ${t.buyTrades}
Sell Trades:            ${t.sellTrades}
Total Volume:           $${t.totalVolume.toFixed(2)}
Total Costs:            $${t.totalCosts.toFixed(2)}
Avg Trade Size:         $${(t.avgTradeSize || 0).toFixed(2)}

ADDITIONAL METRICS
──────────────────────────────────────────────────────────────────────
Trading Days:           ${m.tradingDays}
Best Day:               ${(m.bestDay * 100).toFixed(2)}%
Worst Day:              ${(m.worstDay * 100).toFixed(2)}%
Positive Months:        ${(m.positiveMonths * 100).toFixed(1)}%

══════════════════════════════════════════════════════════════════════
`;
  }
}

/**
 * Walk-Forward Analysis
 */
class WalkForwardAnalyzer {
  constructor(config = {}) {
    this.trainRatio = config.trainRatio || 0.7;
    this.numFolds = config.numFolds || 5;
    this.engine = new BacktestEngine();
  }

  async analyze(historicalData, options = {}) {
    const foldSize = Math.floor(historicalData.length / this.numFolds);
    const results = [];

    for (let i = 0; i < this.numFolds; i++) {
      const testStart = i * foldSize;
      const testEnd = (i + 1) * foldSize;
      const trainEnd = Math.floor(testStart * this.trainRatio);

      // In-sample (training) period
      const trainData = historicalData.slice(0, trainEnd);

      // Out-of-sample (test) period
      const testData = historicalData.slice(testStart, testEnd);

      // Run backtest on test period
      const foldResult = await this.engine.run(testData, options);

      results.push({
        fold: i + 1,
        trainPeriod: { start: 0, end: trainEnd },
        testPeriod: { start: testStart, end: testEnd },
        metrics: foldResult.metrics
      });
    }

    // Aggregate results
    const avgSharpe = results.reduce((a, r) => a + r.metrics.sharpeRatio, 0) / results.length;
    const avgReturn = results.reduce((a, r) => a + r.metrics.totalReturn, 0) / results.length;

    return {
      folds: results,
      aggregate: {
        avgSharpe,
        avgReturn,
        consistency: this.calculateConsistency(results)
      }
    };
  }

  calculateConsistency(results) {
    const profitableFolds = results.filter(r => r.metrics.totalReturn > 0).length;
    return profitableFolds / results.length;
  }
}

// Exports
export {
  BacktestEngine,
  PerformanceMetrics,
  WalkForwardAnalyzer,
  backtestConfig
};

// Demo if run directly
const isMainModule = import.meta.url === `file://${process.argv[1]}`;
if (isMainModule) {
  console.log('══════════════════════════════════════════════════════════════════════');
  console.log('BACKTESTING FRAMEWORK DEMO');
  console.log('══════════════════════════════════════════════════════════════════════\n');

  // Generate synthetic historical data
  const generateHistoricalData = (days) => {
    const data = [];
    let price = 100;

    for (let i = 0; i < days; i++) {
      const trend = Math.sin(i / 50) * 0.001;  // Cyclical trend
      const noise = (Math.random() - 0.5) * 0.02;  // Random noise
      const change = trend + noise;

      price *= (1 + change);

      data.push({
        date: new Date(Date.now() - (days - i) * 24 * 60 * 60 * 1000),
        open: price * (1 - Math.random() * 0.005),
        high: price * (1 + Math.random() * 0.01),
        low: price * (1 - Math.random() * 0.01),
        close: price,
        volume: 1000000 * (0.5 + Math.random())
      });
    }

    return data;
  };

  const historicalData = generateHistoricalData(500);

  console.log('1. Data Summary:');
  console.log('──────────────────────────────────────────────────────────────────────');
  console.log(`   Days: ${historicalData.length}`);
  console.log(`   Start: ${historicalData[0].date.toISOString().split('T')[0]}`);
  console.log(`   End: ${historicalData[historicalData.length-1].date.toISOString().split('T')[0]}`);
  console.log(`   Start Price: $${historicalData[0].close.toFixed(2)}`);
  console.log(`   End Price: $${historicalData[historicalData.length-1].close.toFixed(2)}`);
  console.log();

  const engine = new BacktestEngine();

  console.log('2. Running Backtest...');
  console.log('──────────────────────────────────────────────────────────────────────');

  engine.run(historicalData, {
    symbols: ['TEST'],
    newsData: [
      { symbol: 'TEST', text: 'Strong growth reported in quarterly earnings', source: 'news' },
      { symbol: 'TEST', text: 'Analyst upgrades stock to buy rating', source: 'analyst' }
    ]
  }).then(results => {
    console.log(engine.generateReport(results));

    console.log('3. Equity Curve Summary:');
    console.log('──────────────────────────────────────────────────────────────────────');
    console.log(`   Initial: $${results.equityCurve[0].toFixed(2)}`);
    console.log(`   Final: $${results.equityCurve[results.equityCurve.length-1].toFixed(2)}`);
    console.log(`   Peak: $${Math.max(...results.equityCurve).toFixed(2)}`);
    console.log(`   Trough: $${Math.min(...results.equityCurve).toFixed(2)}`);

    console.log();
    console.log('══════════════════════════════════════════════════════════════════════');
    console.log('Backtesting demo completed');
    console.log('══════════════════════════════════════════════════════════════════════');
  }).catch(err => {
    console.error('Backtest error:', err);
  });
}
