#!/usr/bin/env node
/**
 * Neural-Trader CLI
 *
 * Command-line interface for running trading strategies
 *
 * Usage:
 *   npx neural-trader run --strategy=hybrid --symbol=AAPL
 *   npx neural-trader backtest --data=./data.json --days=252
 *   npx neural-trader paper --capital=100000
 */

import { createTradingPipeline } from './system/trading-pipeline.js';
import { BacktestEngine, PerformanceMetrics } from './system/backtesting.js';
import { DataManager } from './system/data-connectors.js';
import { RiskManager } from './system/risk-management.js';

// CLI Configuration
const CLI_VERSION = '1.0.0';

// Parse command line arguments
function parseArgs(args) {
  const parsed = {
    command: args[0] || 'help',
    options: {}
  };

  for (let i = 1; i < args.length; i++) {
    const arg = args[i];
    if (arg.startsWith('--')) {
      const [key, value] = arg.slice(2).split('=');
      parsed.options[key] = value || true;
    } else if (arg.startsWith('-')) {
      parsed.options[arg.slice(1)] = args[++i] || true;
    }
  }

  return parsed;
}

// Generate synthetic data for demo
function generateSyntheticData(days = 252, startPrice = 100) {
  const data = [];
  let price = startPrice;

  for (let i = 0; i < days; i++) {
    const trend = Math.sin(i / 50) * 0.001;
    const noise = (Math.random() - 0.5) * 0.02;
    price *= (1 + trend + noise);

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
}

// Commands
const commands = {
  help: () => {
    console.log(`
Neural-Trader CLI v${CLI_VERSION}

USAGE:
  neural-trader <command> [options]

COMMANDS:
  run        Execute trading strategy in real-time mode
  backtest   Run historical backtest simulation
  paper      Start paper trading session
  analyze    Analyze market data and generate signals
  benchmark  Run performance benchmarks
  help       Show this help message

OPTIONS:
  --strategy=<name>    Strategy: hybrid, lstm, drl, sentiment (default: hybrid)
  --symbol=<ticker>    Stock/crypto symbol (default: AAPL)
  --capital=<amount>   Initial capital (default: 100000)
  --days=<n>           Number of trading days (default: 252)
  --data=<path>        Path to historical data file
  --output=<path>      Path for output results
  --verbose            Enable verbose output
  --json               Output in JSON format

EXAMPLES:
  neural-trader run --strategy=hybrid --symbol=AAPL
  neural-trader backtest --days=500 --capital=50000
  neural-trader paper --capital=100000 --strategy=drl
  neural-trader analyze --symbol=TSLA --verbose
`);
  },

  run: async (options) => {
    console.log('═'.repeat(70));
    console.log('NEURAL-TRADER: REAL-TIME MODE');
    console.log('═'.repeat(70));
    console.log();

    const strategy = options.strategy || 'hybrid';
    const symbol = options.symbol || 'AAPL';
    const capital = parseFloat(options.capital) || 100000;

    console.log(`Strategy: ${strategy}`);
    console.log(`Symbol: ${symbol}`);
    console.log(`Capital: $${capital.toLocaleString()}`);
    console.log();

    const pipeline = createTradingPipeline();
    const riskManager = new RiskManager();
    riskManager.startDay(capital);

    // Generate sample data for demo
    const marketData = generateSyntheticData(100);
    const currentPrice = marketData[marketData.length - 1].close;

    const context = {
      marketData,
      newsData: [
        { symbol, text: 'Market showing positive momentum today', source: 'news' },
        { symbol, text: 'Analysts maintain buy rating', source: 'analyst' }
      ],
      symbols: [symbol],
      portfolio: {
        equity: capital,
        cash: capital,
        positions: {},
        assets: [symbol]
      },
      prices: { [symbol]: currentPrice },
      riskManager
    };

    console.log('Executing pipeline...');
    const result = await pipeline.execute(context);

    console.log();
    console.log('RESULTS:');
    console.log('─'.repeat(70));

    if (result.signals) {
      for (const [sym, signal] of Object.entries(result.signals)) {
        console.log(`${sym}: ${signal.direction.toUpperCase()}`);
        console.log(`  Strength: ${(signal.strength * 100).toFixed(1)}%`);
        console.log(`  Confidence: ${(signal.confidence * 100).toFixed(1)}%`);
      }
    }

    if (result.orders && result.orders.length > 0) {
      console.log();
      console.log('ORDERS:');
      for (const order of result.orders) {
        console.log(`  ${order.side.toUpperCase()} ${order.quantity} ${order.symbol} @ $${order.price.toFixed(2)}`);
      }
    } else {
      console.log();
      console.log('No orders generated');
    }

    console.log();
    console.log(`Pipeline latency: ${result.metrics.totalLatency.toFixed(2)}ms`);

    if (options.json) {
      console.log();
      console.log('JSON OUTPUT:');
      console.log(JSON.stringify(result, null, 2));
    }
  },

  backtest: async (options) => {
    console.log('═'.repeat(70));
    console.log('NEURAL-TRADER: BACKTEST MODE');
    console.log('═'.repeat(70));
    console.log();

    const days = parseInt(options.days) || 252;
    const capital = parseFloat(options.capital) || 100000;
    const symbol = options.symbol || 'TEST';

    console.log(`Period: ${days} trading days`);
    console.log(`Initial Capital: $${capital.toLocaleString()}`);
    console.log();

    const engine = new BacktestEngine({
      simulation: { initialCapital: capital, warmupPeriod: 50 }
    });

    const historicalData = generateSyntheticData(days);

    console.log('Running backtest...');
    const results = await engine.run(historicalData, {
      symbols: [symbol],
      newsData: [
        { symbol, text: 'Positive market sentiment', source: 'news' }
      ]
    });

    console.log(engine.generateReport(results));

    if (options.output) {
      const fs = await import('fs');
      fs.writeFileSync(options.output, JSON.stringify(results, null, 2));
      console.log(`Results saved to ${options.output}`);
    }
  },

  paper: async (options) => {
    console.log('═'.repeat(70));
    console.log('NEURAL-TRADER: PAPER TRADING MODE');
    console.log('═'.repeat(70));
    console.log();

    const capital = parseFloat(options.capital) || 100000;
    const symbol = options.symbol || 'AAPL';
    const interval = parseInt(options.interval) || 5000; // 5 seconds default

    console.log(`Starting paper trading session...`);
    console.log(`Capital: $${capital.toLocaleString()}`);
    console.log(`Symbol: ${symbol}`);
    console.log(`Update interval: ${interval}ms`);
    console.log();
    console.log('Press Ctrl+C to stop');
    console.log();

    const pipeline = createTradingPipeline();
    const riskManager = new RiskManager();
    riskManager.startDay(capital);

    let portfolio = {
      equity: capital,
      cash: capital,
      positions: {},
      assets: [symbol]
    };

    let priceHistory = generateSyntheticData(100);
    let iteration = 0;

    const tick = async () => {
      iteration++;

      // Simulate price movement
      const lastPrice = priceHistory[priceHistory.length - 1].close;
      const newPrice = lastPrice * (1 + (Math.random() - 0.48) * 0.01);

      priceHistory.push({
        date: new Date(),
        open: lastPrice,
        high: Math.max(lastPrice, newPrice) * 1.002,
        low: Math.min(lastPrice, newPrice) * 0.998,
        close: newPrice,
        volume: 1000000
      });

      if (priceHistory.length > 200) {
        priceHistory = priceHistory.slice(-200);
      }

      const context = {
        marketData: priceHistory,
        newsData: [],
        symbols: [symbol],
        portfolio,
        prices: { [symbol]: newPrice },
        riskManager
      };

      try {
        const result = await pipeline.execute(context);

        // Update portfolio based on positions
        portfolio.equity = portfolio.cash;
        for (const [sym, qty] of Object.entries(portfolio.positions)) {
          portfolio.equity += qty * newPrice;
        }

        const pnl = portfolio.equity - capital;
        const pnlPercent = (pnl / capital) * 100;

        console.log(`[${new Date().toLocaleTimeString()}] Tick #${iteration}`);
        console.log(`  Price: $${newPrice.toFixed(2)} | Equity: $${portfolio.equity.toFixed(2)} | P&L: ${pnl >= 0 ? '+' : ''}$${pnl.toFixed(2)} (${pnlPercent.toFixed(2)}%)`);

        if (result.signals?.[symbol]) {
          const signal = result.signals[symbol];
          if (signal.direction !== 'neutral') {
            console.log(`  Signal: ${signal.direction.toUpperCase()} (${(signal.strength * 100).toFixed(0)}%)`);
          }
        }

        console.log();
      } catch (error) {
        console.error(`  Error: ${error.message}`);
      }
    };

    // Run paper trading loop
    const intervalId = setInterval(tick, interval);

    // Initial tick
    await tick();

    // Handle graceful shutdown
    process.on('SIGINT', () => {
      clearInterval(intervalId);
      console.log();
      console.log('─'.repeat(70));
      console.log('Paper trading session ended');
      console.log(`Final equity: $${portfolio.equity.toFixed(2)}`);
      console.log(`Total P&L: $${(portfolio.equity - capital).toFixed(2)}`);
      process.exit(0);
    });
  },

  analyze: async (options) => {
    console.log('═'.repeat(70));
    console.log('NEURAL-TRADER: ANALYSIS MODE');
    console.log('═'.repeat(70));
    console.log();

    const symbol = options.symbol || 'AAPL';

    console.log(`Analyzing ${symbol}...`);
    console.log();

    // Import modules
    const { LexiconAnalyzer, EmbeddingAnalyzer } = await import('./production/sentiment-alpha.js');
    const { FeatureExtractor, HybridLSTMTransformer } = await import('./production/hybrid-lstm-transformer.js');

    const lexicon = new LexiconAnalyzer();
    const embedding = new EmbeddingAnalyzer();
    const featureExtractor = new FeatureExtractor();
    const lstm = new HybridLSTMTransformer();

    // Generate sample data
    const marketData = generateSyntheticData(100);
    const features = featureExtractor.extract(marketData);

    console.log('TECHNICAL ANALYSIS:');
    console.log('─'.repeat(70));

    const prediction = lstm.predict(features);
    console.log(`LSTM Prediction: ${prediction.signal}`);
    console.log(`Direction: ${prediction.direction}`);
    console.log(`Confidence: ${(prediction.confidence * 100).toFixed(1)}%`);

    console.log();
    console.log('SENTIMENT ANALYSIS:');
    console.log('─'.repeat(70));

    const sampleNews = [
      'Strong earnings beat analyst expectations with revenue growth',
      'Company faces regulatory headwinds',
      'Quarterly results in line with market estimates'
    ];

    for (const text of sampleNews) {
      const result = lexicon.analyze(text);
      const sentiment = result.score > 0.2 ? 'Positive' : result.score < -0.2 ? 'Negative' : 'Neutral';
      console.log(`"${text.slice(0, 50)}..."`);
      console.log(`  → ${sentiment} (score: ${result.score.toFixed(2)})`);
    }

    console.log();
    console.log('RISK METRICS:');
    console.log('─'.repeat(70));

    const metrics = new PerformanceMetrics();
    const equityCurve = marketData.map(d => d.close * 1000);
    const perf = metrics.calculate(equityCurve);

    console.log(`Volatility (Ann.): ${(perf.annualizedVolatility * 100).toFixed(2)}%`);
    console.log(`Max Drawdown: ${(perf.maxDrawdown * 100).toFixed(2)}%`);
    console.log(`Sharpe Ratio: ${perf.sharpeRatio.toFixed(2)}`);
  },

  benchmark: async (options) => {
    console.log('═'.repeat(70));
    console.log('NEURAL-TRADER: BENCHMARK MODE');
    console.log('═'.repeat(70));
    console.log();

    const iterations = parseInt(options.iterations) || 100;

    console.log(`Running ${iterations} iterations...`);
    console.log();

    // Import all modules
    const { KellyCriterion } = await import('./production/fractional-kelly.js');
    const { LSTMCell, HybridLSTMTransformer } = await import('./production/hybrid-lstm-transformer.js');
    const { NeuralNetwork, ReplayBuffer } = await import('./production/drl-portfolio-manager.js');
    const { LexiconAnalyzer } = await import('./production/sentiment-alpha.js');
    const { PerformanceMetrics } = await import('./system/backtesting.js');

    const results = {};

    // Benchmark Kelly
    const kelly = new KellyCriterion();
    let start = performance.now();
    for (let i = 0; i < iterations; i++) {
      kelly.calculateFractionalKelly(0.55 + Math.random() * 0.1, 2.0);
    }
    results.kelly = (performance.now() - start) / iterations;

    // Benchmark LSTM Cell
    const cell = new LSTMCell(10, 64);
    const x = new Array(10).fill(0.1);
    const h = new Array(64).fill(0);
    const c = new Array(64).fill(0);
    start = performance.now();
    for (let i = 0; i < iterations; i++) {
      cell.forward(x, h, c);
    }
    results.lstmCell = (performance.now() - start) / iterations;

    // Benchmark Neural Network
    const net = new NeuralNetwork([62, 128, 10]);
    const state = new Array(62).fill(0.5);
    start = performance.now();
    for (let i = 0; i < iterations; i++) {
      net.forward(state);
    }
    results.neuralNet = (performance.now() - start) / iterations;

    // Benchmark Lexicon
    const lexicon = new LexiconAnalyzer();
    const text = 'Strong earnings growth beat analyst expectations with positive revenue outlook';
    start = performance.now();
    for (let i = 0; i < iterations; i++) {
      lexicon.analyze(text);
    }
    results.lexicon = (performance.now() - start) / iterations;

    // Benchmark Metrics
    const metrics = new PerformanceMetrics();
    const equityCurve = new Array(252).fill(100000).map((v, i) => v * (1 + (Math.random() - 0.5) * 0.02 * i / 252));
    start = performance.now();
    for (let i = 0; i < iterations; i++) {
      metrics.calculate(equityCurve);
    }
    results.metrics = (performance.now() - start) / iterations;

    console.log('BENCHMARK RESULTS:');
    console.log('─'.repeat(70));
    console.log(`Kelly Criterion:    ${results.kelly.toFixed(3)}ms (${(1000 / results.kelly).toFixed(0)}/s)`);
    console.log(`LSTM Cell:          ${results.lstmCell.toFixed(3)}ms (${(1000 / results.lstmCell).toFixed(0)}/s)`);
    console.log(`Neural Network:     ${results.neuralNet.toFixed(3)}ms (${(1000 / results.neuralNet).toFixed(0)}/s)`);
    console.log(`Lexicon Analyzer:   ${results.lexicon.toFixed(3)}ms (${(1000 / results.lexicon).toFixed(0)}/s)`);
    console.log(`Metrics Calculator: ${results.metrics.toFixed(3)}ms (${(1000 / results.metrics).toFixed(0)}/s)`);
  }
};

// Main entry point
async function main() {
  const args = process.argv.slice(2);
  const { command, options } = parseArgs(args);

  if (commands[command]) {
    try {
      await commands[command](options);
    } catch (error) {
      console.error(`Error: ${error.message}`);
      if (options.verbose) {
        console.error(error.stack);
      }
      process.exit(1);
    }
  } else {
    console.error(`Unknown command: ${command}`);
    commands.help();
    process.exit(1);
  }
}

// Run if executed directly
const isMainModule = import.meta.url === `file://${process.argv[1]}`;
if (isMainModule) {
  main();
}

export { commands, parseArgs };
