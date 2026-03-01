/**
 * Portfolio Simulation and Management
 *
 * Generate realistic portfolio data for:
 * - Multi-asset portfolios
 * - Rebalancing scenarios
 * - Risk-adjusted returns
 * - Drawdown analysis
 * - Performance attribution
 */

import { AgenticSynth } from '../../../src';

// ============================================================================
// Portfolio Types and Interfaces
// ============================================================================

interface Asset {
  symbol: string;
  assetClass: 'equity' | 'fixedIncome' | 'commodity' | 'crypto' | 'alternative';
  weight: number;
  expectedReturn: number;
  volatility: number;
}

interface PortfolioHolding {
  timestamp: Date;
  symbol: string;
  shares: number;
  price: number;
  marketValue: number;
  weight: number;
  dayReturn: number;
  totalReturn: number;
}

interface PortfolioMetrics {
  timestamp: Date;
  totalValue: number;
  cashBalance: number;
  totalReturn: number;
  dailyReturn: number;
  volatility: number;
  sharpeRatio: number;
  maxDrawdown: number;
  beta: number;
  alpha: number;
}

// ============================================================================
// Multi-Asset Portfolio Generation
// ============================================================================

/**
 * Generate a diversified multi-asset portfolio
 */
async function generateMultiAssetPortfolio() {
  const synth = new AgenticSynth();

  // Define portfolio composition
  const assets: Asset[] = [
    // Equities (60%)
    { symbol: 'SPY', assetClass: 'equity', weight: 0.30, expectedReturn: 0.10, volatility: 0.15 },
    { symbol: 'QQQ', assetClass: 'equity', weight: 0.15, expectedReturn: 0.12, volatility: 0.20 },
    { symbol: 'IWM', assetClass: 'equity', weight: 0.10, expectedReturn: 0.11, volatility: 0.18 },
    { symbol: 'EFA', assetClass: 'equity', weight: 0.05, expectedReturn: 0.08, volatility: 0.16 },

    // Fixed Income (30%)
    { symbol: 'AGG', assetClass: 'fixedIncome', weight: 0.20, expectedReturn: 0.04, volatility: 0.05 },
    { symbol: 'TLT', assetClass: 'fixedIncome', weight: 0.10, expectedReturn: 0.03, volatility: 0.12 },

    // Alternatives (10%)
    { symbol: 'GLD', assetClass: 'commodity', weight: 0.05, expectedReturn: 0.05, volatility: 0.15 },
    { symbol: 'VNQ', assetClass: 'alternative', weight: 0.05, expectedReturn: 0.08, volatility: 0.17 },
  ];

  const days = 252; // One year
  const initialValue = 1000000; // $1M portfolio

  // Generate price series for each asset
  const portfolioData: Map<string, PortfolioHolding[]> = new Map();

  for (const asset of assets) {
    const initialPrice = 100;
    let currentPrice = initialPrice;
    const shares = (initialValue * asset.weight) / initialPrice;

    const holdings: PortfolioHolding[] = [];

    for (let day = 0; day < days; day++) {
      // Generate correlated returns
      const marketReturn = (Math.random() - 0.5) * 0.02;
      const idiosyncraticReturn = (Math.random() - 0.5) * asset.volatility * 0.5;
      const dailyReturn = marketReturn * 0.7 + idiosyncraticReturn + asset.expectedReturn / 252;

      currentPrice *= 1 + dailyReturn;
      const marketValue = shares * currentPrice;
      const totalReturn = (currentPrice - initialPrice) / initialPrice;

      holdings.push({
        timestamp: new Date(Date.now() - (days - day) * 86400000),
        symbol: asset.symbol,
        shares,
        price: Number(currentPrice.toFixed(2)),
        marketValue: Number(marketValue.toFixed(2)),
        weight: asset.weight, // Will be updated later
        dayReturn: Number(dailyReturn.toFixed(6)),
        totalReturn: Number(totalReturn.toFixed(6)),
      });
    }

    portfolioData.set(asset.symbol, holdings);
  }

  // Calculate portfolio-level metrics
  const portfolioMetrics: PortfolioMetrics[] = [];

  for (let day = 0; day < days; day++) {
    let totalValue = 0;
    let dailyReturn = 0;

    assets.forEach((asset) => {
      const holding = portfolioData.get(asset.symbol)![day];
      totalValue += holding.marketValue;
      dailyReturn += holding.dayReturn * asset.weight;
    });

    // Update weights based on current market values
    assets.forEach((asset) => {
      const holding = portfolioData.get(asset.symbol)![day];
      holding.weight = holding.marketValue / totalValue;
    });

    // Calculate metrics
    const returns = Array.from({ length: Math.min(day + 1, 60) }, (_, i) => {
      let ret = 0;
      assets.forEach((asset) => {
        ret += portfolioData.get(asset.symbol)![day - i]?.dayReturn || 0;
      });
      return ret;
    });

    const volatility = Math.sqrt(
      returns.reduce((sum, r) => sum + Math.pow(r, 2), 0) / returns.length
    ) * Math.sqrt(252);

    const totalReturn = (totalValue - initialValue) / initialValue;
    const sharpeRatio = (totalReturn * 252) / (volatility + 0.0001);

    // Calculate max drawdown
    const portfolioValues = Array.from({ length: day + 1 }, (_, i) => {
      let val = 0;
      assets.forEach((asset) => {
        val += portfolioData.get(asset.symbol)![i].marketValue;
      });
      return val;
    });

    const maxDrawdown = portfolioValues.reduce((maxDD, val, i) => {
      const peak = Math.max(...portfolioValues.slice(0, i + 1));
      const dd = (val - peak) / peak;
      return Math.min(maxDD, dd);
    }, 0);

    portfolioMetrics.push({
      timestamp: new Date(Date.now() - (days - day) * 86400000),
      totalValue: Number(totalValue.toFixed(2)),
      cashBalance: 0,
      totalReturn: Number(totalReturn.toFixed(6)),
      dailyReturn: Number(dailyReturn.toFixed(6)),
      volatility: Number(volatility.toFixed(4)),
      sharpeRatio: Number(sharpeRatio.toFixed(2)),
      maxDrawdown: Number(maxDrawdown.toFixed(4)),
      beta: 0.95, // Simplified
      alpha: Number(((totalReturn - 0.08 * (day / 252)) / (day / 252 + 0.0001)).toFixed(4)),
    });
  }

  console.log('Multi-Asset Portfolio:');
  console.log({
    initialValue,
    finalValue: portfolioMetrics[portfolioMetrics.length - 1].totalValue,
    totalReturn: (portfolioMetrics[portfolioMetrics.length - 1].totalReturn * 100).toFixed(2) + '%',
    sharpeRatio: portfolioMetrics[portfolioMetrics.length - 1].sharpeRatio,
    maxDrawdown: (portfolioMetrics[portfolioMetrics.length - 1].maxDrawdown * 100).toFixed(2) + '%',
    volatility: (portfolioMetrics[portfolioMetrics.length - 1].volatility * 100).toFixed(2) + '%',
  });

  return { portfolioData, portfolioMetrics, assets };
}

// ============================================================================
// Rebalancing Scenarios
// ============================================================================

interface RebalanceEvent {
  timestamp: Date;
  type: 'calendar' | 'threshold' | 'opportunistic';
  holdings: PortfolioHolding[];
  targetWeights: Record<string, number>;
  actualWeights: Record<string, number>;
  trades: Trade[];
  transactionCosts: number;
}

interface Trade {
  symbol: string;
  action: 'buy' | 'sell';
  shares: number;
  price: number;
  value: number;
  commission: number;
}

/**
 * Generate portfolio rebalancing scenarios
 */
async function generateRebalancingScenarios() {
  const synth = new AgenticSynth();

  // Start with portfolio from previous function
  const { portfolioData, assets } = await generateMultiAssetPortfolio();

  const rebalanceEvents: RebalanceEvent[] = [];
  const rebalanceThreshold = 0.05; // Rebalance if drift > 5%
  const rebalanceFrequency = 63; // Quarterly (every 63 trading days)

  const days = 252;
  for (let day = 0; day < days; day++) {
    const currentHoldings: PortfolioHolding[] = [];
    let totalValue = 0;

    // Get current holdings
    assets.forEach((asset) => {
      const holding = portfolioData.get(asset.symbol)![day];
      currentHoldings.push(holding);
      totalValue += holding.marketValue;
    });

    // Calculate current weights
    const actualWeights: Record<string, number> = {};
    currentHoldings.forEach((holding) => {
      actualWeights[holding.symbol] = holding.marketValue / totalValue;
    });

    // Check if rebalancing is needed
    const targetWeights: Record<string, number> = {};
    assets.forEach((asset) => {
      targetWeights[asset.symbol] = asset.weight;
    });

    let maxDrift = 0;
    Object.keys(targetWeights).forEach((symbol) => {
      const drift = Math.abs(actualWeights[symbol] - targetWeights[symbol]);
      maxDrift = Math.max(maxDrift, drift);
    });

    const shouldRebalance =
      maxDrift > rebalanceThreshold || day % rebalanceFrequency === 0;

    if (shouldRebalance) {
      const trades: Trade[] = [];
      let transactionCosts = 0;

      // Generate trades to rebalance
      Object.keys(targetWeights).forEach((symbol) => {
        const currentWeight = actualWeights[symbol];
        const targetWeight = targetWeights[symbol];
        const targetValue = totalValue * targetWeight;
        const currentValue = totalValue * currentWeight;
        const deltaValue = targetValue - currentValue;

        const holding = currentHoldings.find((h) => h.symbol === symbol)!;
        const deltaShares = deltaValue / holding.price;

        if (Math.abs(deltaShares) > 1) {
          const commission = Math.abs(deltaValue) * 0.0005; // 5 bps
          transactionCosts += commission;

          trades.push({
            symbol,
            action: deltaShares > 0 ? 'buy' : 'sell',
            shares: Math.abs(Math.floor(deltaShares)),
            price: holding.price,
            value: Math.abs(deltaValue),
            commission,
          });
        }
      });

      rebalanceEvents.push({
        timestamp: new Date(Date.now() - (days - day) * 86400000),
        type: maxDrift > rebalanceThreshold * 2 ? 'threshold' : 'calendar',
        holdings: currentHoldings,
        targetWeights,
        actualWeights,
        trades,
        transactionCosts,
      });
    }
  }

  console.log('Rebalancing Scenarios:');
  console.log({
    totalRebalances: rebalanceEvents.length,
    avgTransactionCosts:
      rebalanceEvents.reduce((sum, e) => sum + e.transactionCosts, 0) /
      rebalanceEvents.length,
    totalTransactionCosts: rebalanceEvents.reduce(
      (sum, e) => sum + e.transactionCosts,
      0
    ),
  });

  return rebalanceEvents;
}

// ============================================================================
// Risk-Adjusted Returns Analysis
// ============================================================================

interface RiskMetrics {
  timestamp: Date;
  portfolioReturn: number;
  benchmarkReturn: number;
  excessReturn: number;
  trackingError: number;
  informationRatio: number;
  sharpeRatio: number;
  sortinoRatio: number;
  calmarRatio: number;
  beta: number;
  alpha: number;
  correlation: number;
}

/**
 * Calculate comprehensive risk-adjusted return metrics
 */
async function generateRiskAdjustedReturns() {
  const { portfolioMetrics } = await generateMultiAssetPortfolio();

  const riskFreeRate = 0.04; // 4% annual
  const dailyRfRate = riskFreeRate / 252;

  const riskMetrics: RiskMetrics[] = portfolioMetrics.map((metrics, idx) => {
    // Simulate benchmark (S&P 500)
    const benchmarkReturn = metrics.dailyReturn * 0.95 + (Math.random() - 0.5) * 0.005;
    const excessReturn = metrics.dailyReturn - dailyRfRate;

    // Calculate rolling metrics
    const window = Math.min(60, idx + 1);
    const recentReturns = portfolioMetrics
      .slice(Math.max(0, idx - window), idx + 1)
      .map((m) => m.dailyReturn);

    const recentBenchmarkReturns = Array.from(
      { length: window },
      (_, i) => portfolioMetrics[Math.max(0, idx - window + i)].dailyReturn * 0.95
    );

    // Tracking error
    const trackingDiffs = recentReturns.map(
      (r, i) => r - recentBenchmarkReturns[i]
    );
    const trackingError =
      Math.sqrt(
        trackingDiffs.reduce((sum, d) => sum + Math.pow(d, 2), 0) / window
      ) * Math.sqrt(252);

    // Information ratio
    const avgExcessReturn =
      trackingDiffs.reduce((sum, d) => sum + d, 0) / window;
    const informationRatio = (avgExcessReturn * 252) / (trackingError + 0.0001);

    // Sortino ratio (downside deviation)
    const downsideReturns = recentReturns.filter((r) => r < dailyRfRate);
    const downsideDeviation = downsideReturns.length > 0
      ? Math.sqrt(
          downsideReturns.reduce(
            (sum, r) => sum + Math.pow(r - dailyRfRate, 2),
            0
          ) / downsideReturns.length
        ) * Math.sqrt(252)
      : 0.0001;

    const avgReturn = recentReturns.reduce((sum, r) => sum + r, 0) / window;
    const sortinoRatio = ((avgReturn - dailyRfRate) * 252) / downsideDeviation;

    // Calmar ratio
    const calmarRatio = (avgReturn * 252) / (Math.abs(metrics.maxDrawdown) + 0.0001);

    // Beta and alpha
    const benchmarkVar =
      recentBenchmarkReturns.reduce(
        (sum, r) => sum + Math.pow(r - avgReturn, 2),
        0
      ) / window;
    const covariance =
      recentReturns.reduce(
        (sum, r, i) => sum + (r - avgReturn) * (recentBenchmarkReturns[i] - avgReturn),
        0
      ) / window;
    const beta = covariance / (benchmarkVar + 0.0001);
    const alpha = (avgReturn - dailyRfRate - beta * (avgReturn * 0.95 - dailyRfRate)) * 252;

    // Correlation
    const correlation = covariance / (Math.sqrt(benchmarkVar) * metrics.volatility / Math.sqrt(252) + 0.0001);

    return {
      timestamp: metrics.timestamp,
      portfolioReturn: metrics.dailyReturn,
      benchmarkReturn,
      excessReturn,
      trackingError: Number(trackingError.toFixed(4)),
      informationRatio: Number(informationRatio.toFixed(2)),
      sharpeRatio: metrics.sharpeRatio,
      sortinoRatio: Number(sortinoRatio.toFixed(2)),
      calmarRatio: Number(calmarRatio.toFixed(2)),
      beta: Number(beta.toFixed(2)),
      alpha: Number(alpha.toFixed(4)),
      correlation: Number(correlation.toFixed(2)),
    };
  });

  console.log('Risk-Adjusted Returns (final metrics):');
  const finalMetrics = riskMetrics[riskMetrics.length - 1];
  console.log({
    sharpeRatio: finalMetrics.sharpeRatio,
    sortinoRatio: finalMetrics.sortinoRatio,
    calmarRatio: finalMetrics.calmarRatio,
    informationRatio: finalMetrics.informationRatio,
    beta: finalMetrics.beta,
    alpha: (finalMetrics.alpha * 100).toFixed(2) + '%',
    correlation: finalMetrics.correlation,
  });

  return riskMetrics;
}

// ============================================================================
// Drawdown Analysis
// ============================================================================

interface DrawdownPeriod {
  startDate: Date;
  troughDate: Date;
  endDate: Date | null;
  peakValue: number;
  troughValue: number;
  recoveryValue: number | null;
  drawdown: number;
  duration: number;
  recoveryDuration: number | null;
  underwater: boolean;
}

/**
 * Analyze portfolio drawdowns
 */
async function generateDrawdownAnalysis() {
  const { portfolioMetrics } = await generateMultiAssetPortfolio();

  const drawdowns: DrawdownPeriod[] = [];
  let currentPeak = portfolioMetrics[0].totalValue;
  let currentPeakDate = portfolioMetrics[0].timestamp;
  let inDrawdown = false;
  let troughValue = currentPeak;
  let troughDate = currentPeakDate;
  let startDate = currentPeakDate;

  portfolioMetrics.forEach((metrics, idx) => {
    if (metrics.totalValue > currentPeak) {
      // New peak
      if (inDrawdown) {
        // End of drawdown - recovery complete
        drawdowns[drawdowns.length - 1].endDate = metrics.timestamp;
        drawdowns[drawdowns.length - 1].recoveryValue = metrics.totalValue;
        drawdowns[drawdowns.length - 1].recoveryDuration =
          (metrics.timestamp.getTime() - troughDate.getTime()) / 86400000;
        drawdowns[drawdowns.length - 1].underwater = false;
        inDrawdown = false;
      }
      currentPeak = metrics.totalValue;
      currentPeakDate = metrics.timestamp;
    } else if (metrics.totalValue < currentPeak) {
      // In drawdown
      if (!inDrawdown) {
        // Start of new drawdown
        startDate = currentPeakDate;
        troughValue = metrics.totalValue;
        troughDate = metrics.timestamp;
        inDrawdown = true;
      }

      if (metrics.totalValue < troughValue) {
        troughValue = metrics.totalValue;
        troughDate = metrics.timestamp;
      }

      // Update or create drawdown record
      const dd = (metrics.totalValue - currentPeak) / currentPeak;
      const duration = (troughDate.getTime() - startDate.getTime()) / 86400000;

      if (drawdowns.length === 0 || !drawdowns[drawdowns.length - 1].underwater) {
        drawdowns.push({
          startDate,
          troughDate,
          endDate: null,
          peakValue: currentPeak,
          troughValue,
          recoveryValue: null,
          drawdown: dd,
          duration,
          recoveryDuration: null,
          underwater: true,
        });
      } else {
        drawdowns[drawdowns.length - 1].troughDate = troughDate;
        drawdowns[drawdowns.length - 1].troughValue = troughValue;
        drawdowns[drawdowns.length - 1].drawdown = dd;
        drawdowns[drawdowns.length - 1].duration = duration;
      }
    }
  });

  // Sort by drawdown magnitude
  const sortedDrawdowns = drawdowns.sort((a, b) => a.drawdown - b.drawdown);

  console.log('Drawdown Analysis:');
  console.log({
    totalDrawdowns: drawdowns.length,
    maxDrawdown: (sortedDrawdowns[0].drawdown * 100).toFixed(2) + '%',
    avgDrawdown: (
      (drawdowns.reduce((sum, dd) => sum + dd.drawdown, 0) / drawdowns.length) *
      100
    ).toFixed(2) + '%',
    longestDrawdown: Math.max(...drawdowns.map((dd) => dd.duration)),
    currentlyUnderwater: drawdowns[drawdowns.length - 1]?.underwater || false,
  });

  console.log('\nTop 5 Drawdowns:');
  sortedDrawdowns.slice(0, 5).forEach((dd, idx) => {
    console.log(
      `${idx + 1}. ${(dd.drawdown * 100).toFixed(2)}% over ${dd.duration} days, ` +
        `recovered in ${dd.recoveryDuration || 'N/A'} days`
    );
  });

  return drawdowns;
}

// ============================================================================
// Main Execution
// ============================================================================

async function main() {
  console.log('='.repeat(80));
  console.log('Portfolio Simulation and Management');
  console.log('='.repeat(80));
  console.log();

  try {
    console.log('1. Generating Multi-Asset Portfolio...');
    await generateMultiAssetPortfolio();
    console.log();

    console.log('2. Generating Rebalancing Scenarios...');
    await generateRebalancingScenarios();
    console.log();

    console.log('3. Calculating Risk-Adjusted Returns...');
    await generateRiskAdjustedReturns();
    console.log();

    console.log('4. Analyzing Drawdowns...');
    await generateDrawdownAnalysis();
    console.log();

    console.log('All portfolio simulations completed successfully!');
  } catch (error) {
    console.error('Error generating portfolio simulations:', error);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

export {
  generateMultiAssetPortfolio,
  generateRebalancingScenarios,
  generateRiskAdjustedReturns,
  generateDrawdownAnalysis,
};
