/**
 * Portfolio Optimization with Neural Trader
 *
 * Demonstrates using @neural-trader/portfolio for:
 * - Mean-Variance Optimization (Markowitz)
 * - Risk Parity Portfolio
 * - Maximum Sharpe Ratio
 * - Minimum Volatility
 * - Black-Litterman Model
 */

// Portfolio configuration
const portfolioConfig = {
  // Assets to optimize
  assets: ['AAPL', 'GOOGL', 'MSFT', 'AMZN', 'NVDA', 'META', 'TSLA', 'BRK.B', 'JPM', 'V'],

  // Risk-free rate (annual)
  riskFreeRate: 0.045,

  // Optimization constraints
  constraints: {
    minWeight: 0.02,      // Minimum 2% per asset
    maxWeight: 0.25,      // Maximum 25% per asset
    maxSectorWeight: 0.40, // Maximum 40% per sector
    turnoverLimit: 0.20   // Maximum 20% turnover per rebalance
  },

  // Lookback period for historical data
  lookbackDays: 252 * 3   // 3 years
};

// Sector mappings
const sectorMap = {
  'AAPL': 'Technology', 'GOOGL': 'Technology', 'MSFT': 'Technology',
  'AMZN': 'Consumer', 'NVDA': 'Technology', 'META': 'Technology',
  'TSLA': 'Consumer', 'BRK.B': 'Financial', 'JPM': 'Financial', 'V': 'Financial'
};

async function main() {
  console.log('='.repeat(70));
  console.log('Portfolio Optimization - Neural Trader');
  console.log('='.repeat(70));
  console.log();

  // 1. Load historical returns
  console.log('1. Loading historical data...');
  const { returns, prices, covariance, expectedReturns } = generateHistoricalData(
    portfolioConfig.assets,
    portfolioConfig.lookbackDays
  );
  console.log(`   Assets: ${portfolioConfig.assets.length}`);
  console.log(`   Data points: ${portfolioConfig.lookbackDays} days`);
  console.log();

  // 2. Display asset statistics
  console.log('2. Asset Statistics:');
  console.log('-'.repeat(70));
  console.log('   Asset   | Ann. Return | Volatility | Sharpe | Sector');
  console.log('-'.repeat(70));

  portfolioConfig.assets.forEach(asset => {
    const annReturn = expectedReturns[asset];
    const vol = Math.sqrt(covariance[asset][asset]) * Math.sqrt(252);
    const sharpe = (annReturn - portfolioConfig.riskFreeRate) / vol;

    console.log(`   ${asset.padEnd(7)} | ${(annReturn * 100).toFixed(1).padStart(10)}% | ${(vol * 100).toFixed(1).padStart(9)}% | ${sharpe.toFixed(2).padStart(6)} | ${sectorMap[asset]}`);
  });
  console.log();

  // 3. Calculate different portfolio optimizations
  console.log('3. Portfolio Optimization Results:');
  console.log('='.repeat(70));

  // Equal Weight (benchmark)
  const equalWeight = equalWeightPortfolio(portfolioConfig.assets);
  displayPortfolio('Equal Weight (Benchmark)', equalWeight, expectedReturns, covariance);

  // Minimum Variance
  const minVar = minimumVariancePortfolio(expectedReturns, covariance, portfolioConfig.constraints);
  displayPortfolio('Minimum Variance', minVar, expectedReturns, covariance);

  // Maximum Sharpe Ratio
  const maxSharpe = maximumSharpePortfolio(expectedReturns, covariance, portfolioConfig.riskFreeRate, portfolioConfig.constraints);
  displayPortfolio('Maximum Sharpe Ratio', maxSharpe, expectedReturns, covariance);

  // Risk Parity
  const riskParity = riskParityPortfolio(covariance);
  displayPortfolio('Risk Parity', riskParity, expectedReturns, covariance);

  // Black-Litterman
  const bl = blackLittermanPortfolio(expectedReturns, covariance, portfolioConfig.constraints);
  displayPortfolio('Black-Litterman', bl, expectedReturns, covariance);

  // 4. Efficient Frontier
  console.log('4. Efficient Frontier:');
  console.log('-'.repeat(70));
  console.log('   Target Vol | Exp. Return | Sharpe | Weights Summary');
  console.log('-'.repeat(70));

  const targetVols = [0.10, 0.12, 0.15, 0.18, 0.20, 0.25];
  for (const targetVol of targetVols) {
    const portfolio = efficientFrontierPoint(expectedReturns, covariance, targetVol, portfolioConfig.constraints);
    const ret = calculatePortfolioReturn(portfolio, expectedReturns);
    const vol = calculatePortfolioVolatility(portfolio, covariance);
    const sharpe = (ret - portfolioConfig.riskFreeRate) / vol;

    // Summarize weights
    const topWeights = Object.entries(portfolio)
      .sort((a, b) => b[1] - a[1])
      .slice(0, 3)
      .map(([asset, weight]) => `${asset}:${(weight * 100).toFixed(0)}%`)
      .join(', ');

    console.log(`   ${(targetVol * 100).toFixed(0).padStart(9)}% | ${(ret * 100).toFixed(1).padStart(10)}% | ${sharpe.toFixed(2).padStart(6)} | ${topWeights}`);
  }
  console.log();

  // 5. Sector allocation analysis
  console.log('5. Sector Allocation Analysis:');
  console.log('-'.repeat(70));

  const portfolios = {
    'Equal Weight': equalWeight,
    'Min Variance': minVar,
    'Max Sharpe': maxSharpe,
    'Risk Parity': riskParity
  };

  const sectors = [...new Set(Object.values(sectorMap))];
  console.log(`   Portfolio      | ${sectors.map(s => s.padEnd(10)).join(' | ')}`);
  console.log('-'.repeat(70));

  for (const [name, portfolio] of Object.entries(portfolios)) {
    const sectorWeights = {};
    sectors.forEach(s => sectorWeights[s] = 0);

    for (const [asset, weight] of Object.entries(portfolio)) {
      sectorWeights[sectorMap[asset]] += weight;
    }

    const row = sectors.map(s => (sectorWeights[s] * 100).toFixed(1).padStart(8) + '%').join(' | ');
    console.log(`   ${name.padEnd(14)} | ${row}`);
  }
  console.log();

  // 6. Rebalancing analysis
  console.log('6. Rebalancing Analysis (from Equal Weight):');
  console.log('-'.repeat(70));

  for (const [name, portfolio] of Object.entries(portfolios)) {
    if (name === 'Equal Weight') continue;

    let turnover = 0;
    for (const asset of portfolioConfig.assets) {
      turnover += Math.abs((portfolio[asset] || 0) - equalWeight[asset]);
    }
    turnover /= 2; // One-way turnover

    const numTrades = Object.keys(portfolio).filter(a =>
      Math.abs((portfolio[a] || 0) - equalWeight[a]) > 0.01
    ).length;

    console.log(`   ${name.padEnd(15)}: ${(turnover * 100).toFixed(1)}% turnover, ${numTrades} trades required`);
  }
  console.log();

  // 7. Risk decomposition
  console.log('7. Risk Decomposition (Max Sharpe Portfolio):');
  console.log('-'.repeat(70));

  const riskContrib = calculateRiskContribution(maxSharpe, covariance);
  console.log('   Asset   | Weight  | Risk Contrib | Marginal Risk');
  console.log('-'.repeat(70));

  Object.entries(riskContrib)
    .sort((a, b) => b[1].contribution - a[1].contribution)
    .forEach(([asset, { weight, contribution, marginal }]) => {
      console.log(`   ${asset.padEnd(7)} | ${(weight * 100).toFixed(1).padStart(5)}% | ${(contribution * 100).toFixed(1).padStart(11)}% | ${(marginal * 100).toFixed(2).padStart(12)}%`);
    });
  console.log();

  console.log('='.repeat(70));
  console.log('Portfolio optimization completed!');
  console.log('='.repeat(70));
}

// Generate historical data
function generateHistoricalData(assets, days) {
  const prices = {};
  const returns = {};
  const expectedReturns = {};
  const covariance = {};

  // Initialize covariance matrix
  assets.forEach(a => {
    covariance[a] = {};
    assets.forEach(b => covariance[a][b] = 0);
  });

  // Generate correlated returns
  for (const asset of assets) {
    prices[asset] = [100 + Math.random() * 200];
    returns[asset] = [];

    // Generate random returns with realistic characteristics
    const annualReturn = 0.08 + Math.random() * 0.15; // 8-23% annual return
    const dailyReturn = annualReturn / 252;
    const dailyVol = (0.15 + Math.random() * 0.25) / Math.sqrt(252);

    for (let i = 0; i < days; i++) {
      const r = dailyReturn + dailyVol * (Math.random() - 0.5) * 2;
      returns[asset].push(r);
      prices[asset].push(prices[asset][i] * (1 + r));
    }

    // Calculate expected return (annualized)
    const avgReturn = returns[asset].reduce((a, b) => a + b, 0) / returns[asset].length;
    expectedReturns[asset] = avgReturn * 252;
  }

  // Calculate covariance matrix
  for (const a of assets) {
    for (const b of assets) {
      if (a === b) {
        // Variance
        const mean = returns[a].reduce((s, r) => s + r, 0) / returns[a].length;
        covariance[a][b] = returns[a].reduce((s, r) => s + Math.pow(r - mean, 2), 0) / returns[a].length;
      } else {
        // Covariance with correlation factor
        const meanA = returns[a].reduce((s, r) => s + r, 0) / returns[a].length;
        const meanB = returns[b].reduce((s, r) => s + r, 0) / returns[b].length;

        let cov = 0;
        for (let i = 0; i < days; i++) {
          cov += (returns[a][i] - meanA) * (returns[b][i] - meanB);
        }
        cov /= days;

        // Add sector correlation
        const sameSecter = sectorMap[a] === sectorMap[b];
        const corrFactor = sameSecter ? 1.5 : 0.8;
        covariance[a][b] = cov * corrFactor;
      }
    }
  }

  return { returns, prices, covariance, expectedReturns };
}

// Equal weight portfolio
function equalWeightPortfolio(assets) {
  const weight = 1 / assets.length;
  const portfolio = {};
  assets.forEach(a => portfolio[a] = weight);
  return portfolio;
}

// Minimum variance portfolio (simplified)
function minimumVariancePortfolio(expectedReturns, covariance, constraints) {
  const assets = Object.keys(expectedReturns);
  const n = assets.length;

  // Simple optimization: inversely proportional to variance
  const invVariances = assets.map(a => 1 / covariance[a][a]);
  const sum = invVariances.reduce((a, b) => a + b, 0);

  const portfolio = {};
  assets.forEach((a, i) => {
    let weight = invVariances[i] / sum;
    weight = Math.max(constraints.minWeight, Math.min(constraints.maxWeight, weight));
    portfolio[a] = weight;
  });

  // Normalize to sum to 1
  const totalWeight = Object.values(portfolio).reduce((a, b) => a + b, 0);
  Object.keys(portfolio).forEach(a => portfolio[a] /= totalWeight);

  return portfolio;
}

// Maximum Sharpe ratio portfolio (simplified)
function maximumSharpePortfolio(expectedReturns, covariance, riskFreeRate, constraints) {
  const assets = Object.keys(expectedReturns);

  // Simple optimization: proportional to excess return / variance
  const scores = assets.map(a => {
    const excessReturn = expectedReturns[a] - riskFreeRate;
    const vol = Math.sqrt(covariance[a][a]) * Math.sqrt(252);
    return Math.max(0, excessReturn / vol);
  });

  const sum = scores.reduce((a, b) => a + b, 0);

  const portfolio = {};
  assets.forEach((a, i) => {
    let weight = sum > 0 ? scores[i] / sum : 1 / assets.length;
    weight = Math.max(constraints.minWeight, Math.min(constraints.maxWeight, weight));
    portfolio[a] = weight;
  });

  // Normalize
  const totalWeight = Object.values(portfolio).reduce((a, b) => a + b, 0);
  Object.keys(portfolio).forEach(a => portfolio[a] /= totalWeight);

  return portfolio;
}

// Risk parity portfolio
function riskParityPortfolio(covariance) {
  const assets = Object.keys(covariance);

  // Target: equal risk contribution
  // Simplified: inversely proportional to volatility
  const invVols = assets.map(a => 1 / Math.sqrt(covariance[a][a]));
  const sum = invVols.reduce((a, b) => a + b, 0);

  const portfolio = {};
  assets.forEach((a, i) => portfolio[a] = invVols[i] / sum);

  return portfolio;
}

// Black-Litterman portfolio (simplified)
function blackLittermanPortfolio(expectedReturns, covariance, constraints) {
  const assets = Object.keys(expectedReturns);

  // Views: slight adjustment to expected returns based on "views"
  const adjustedReturns = {};
  assets.forEach(a => {
    // Simulate analyst view adjustment
    const viewAdjustment = (Math.random() - 0.5) * 0.02;
    adjustedReturns[a] = expectedReturns[a] + viewAdjustment;
  });

  return maximumSharpePortfolio(adjustedReturns, covariance, portfolioConfig.riskFreeRate, constraints);
}

// Efficient frontier point
function efficientFrontierPoint(expectedReturns, covariance, targetVol, constraints) {
  // Simplified: interpolate between min variance and max return
  const minVar = minimumVariancePortfolio(expectedReturns, covariance, constraints);
  const maxSharpe = maximumSharpePortfolio(expectedReturns, covariance, portfolioConfig.riskFreeRate, constraints);

  const minVol = calculatePortfolioVolatility(minVar, covariance);
  const maxVol = calculatePortfolioVolatility(maxSharpe, covariance);

  const alpha = Math.min(1, Math.max(0, (targetVol - minVol) / (maxVol - minVol)));

  const portfolio = {};
  Object.keys(minVar).forEach(a => {
    portfolio[a] = minVar[a] * (1 - alpha) + maxSharpe[a] * alpha;
  });

  return portfolio;
}

// Calculate portfolio return
function calculatePortfolioReturn(portfolio, expectedReturns) {
  let ret = 0;
  for (const [asset, weight] of Object.entries(portfolio)) {
    ret += weight * expectedReturns[asset];
  }
  return ret;
}

// Calculate portfolio volatility
function calculatePortfolioVolatility(portfolio, covariance) {
  const assets = Object.keys(portfolio);
  let variance = 0;

  for (const a of assets) {
    for (const b of assets) {
      variance += portfolio[a] * portfolio[b] * covariance[a][b] * 252;
    }
  }

  return Math.sqrt(variance);
}

// Calculate risk contribution
function calculateRiskContribution(portfolio, covariance) {
  const assets = Object.keys(portfolio);
  const totalVol = calculatePortfolioVolatility(portfolio, covariance);

  const result = {};

  for (const asset of assets) {
    // Marginal contribution to risk
    let marginal = 0;
    for (const b of assets) {
      marginal += portfolio[b] * covariance[asset][b] * 252;
    }
    marginal /= totalVol;

    // Total contribution
    const contribution = portfolio[asset] * marginal / totalVol;

    result[asset] = {
      weight: portfolio[asset],
      contribution,
      marginal
    };
  }

  return result;
}

// Display portfolio summary
function displayPortfolio(name, portfolio, expectedReturns, covariance) {
  console.log(`\n   ${name}:`);
  console.log('-'.repeat(70));

  // Sort by weight
  const sorted = Object.entries(portfolio).sort((a, b) => b[1] - a[1]);

  console.log('   Weights: ' + sorted.slice(0, 5).map(([a, w]) => `${a}:${(w * 100).toFixed(1)}%`).join(', ') + (sorted.length > 5 ? '...' : ''));

  const ret = calculatePortfolioReturn(portfolio, expectedReturns);
  const vol = calculatePortfolioVolatility(portfolio, covariance);
  const sharpe = (ret - portfolioConfig.riskFreeRate) / vol;

  console.log(`   Expected Return: ${(ret * 100).toFixed(2)}%`);
  console.log(`   Volatility:      ${(vol * 100).toFixed(2)}%`);
  console.log(`   Sharpe Ratio:    ${sharpe.toFixed(2)}`);
}

// Run the example
main().catch(console.error);
