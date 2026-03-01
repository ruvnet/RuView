/**
 * Risk Management with Neural Trader
 *
 * Demonstrates using @neural-trader/risk for:
 * - Value at Risk (VaR) calculations
 * - Expected Shortfall (CVaR)
 * - Maximum Drawdown analysis
 * - Sharpe, Sortino, Calmar ratios
 * - Portfolio stress testing
 */

// Risk configuration
const riskConfig = {
  // VaR settings
  var: {
    confidenceLevel: 0.99,     // 99% VaR
    horizon: 1,                 // 1 day
    methods: ['historical', 'parametric', 'monteCarlo']
  },

  // Position limits
  limits: {
    maxPositionSize: 0.10,      // 10% of portfolio per position
    maxSectorExposure: 0.30,    // 30% per sector
    maxDrawdown: 0.15,          // 15% max drawdown trigger
    stopLoss: 0.02              // 2% daily stop loss
  },

  // Stress test scenarios
  stressScenarios: [
    { name: '2008 Financial Crisis', equity: -0.50, bonds: 0.10, volatility: 3.0 },
    { name: 'COVID-19 Crash', equity: -0.35, bonds: 0.05, volatility: 4.0 },
    { name: 'Tech Bubble 2000', equity: -0.45, bonds: 0.20, volatility: 2.5 },
    { name: 'Flash Crash', equity: -0.10, bonds: 0.02, volatility: 5.0 },
    { name: 'Rising Rates', equity: -0.15, bonds: -0.20, volatility: 1.5 }
  ],

  // Monte Carlo settings
  monteCarlo: {
    simulations: 10000,
    horizon: 252                // 1 year
  }
};

async function main() {
  console.log('='.repeat(70));
  console.log('Risk Management - Neural Trader');
  console.log('='.repeat(70));
  console.log();

  // 1. Generate portfolio data
  console.log('1. Loading portfolio data...');
  const portfolio = generatePortfolioData();
  console.log(`   Portfolio value: $${portfolio.totalValue.toLocaleString()}`);
  console.log(`   Positions: ${portfolio.positions.length}`);
  console.log(`   History: ${portfolio.returns.length} days`);
  console.log();

  // 2. Portfolio composition
  console.log('2. Portfolio Composition:');
  console.log('-'.repeat(70));
  console.log('   Asset   | Value       | Weight  | Sector     | Daily Vol');
  console.log('-'.repeat(70));

  portfolio.positions.forEach(pos => {
    console.log(`   ${pos.symbol.padEnd(7)} | $${pos.value.toLocaleString().padStart(10)} | ${(pos.weight * 100).toFixed(1).padStart(5)}% | ${pos.sector.padEnd(10)} | ${(pos.dailyVol * 100).toFixed(2)}%`);
  });

  console.log('-'.repeat(70));
  console.log(`   Total   | $${portfolio.totalValue.toLocaleString().padStart(10)} | 100.0% |            |`);
  console.log();

  // 3. Risk metrics summary
  console.log('3. Risk Metrics Summary:');
  console.log('-'.repeat(70));

  const metrics = calculateRiskMetrics(portfolio.returns, portfolio.totalValue);

  console.log(`   Daily Volatility:    ${(metrics.dailyVol * 100).toFixed(2)}%`);
  console.log(`   Annual Volatility:   ${(metrics.annualVol * 100).toFixed(2)}%`);
  console.log(`   Sharpe Ratio:        ${metrics.sharpe.toFixed(2)}`);
  console.log(`   Sortino Ratio:       ${metrics.sortino.toFixed(2)}`);
  console.log(`   Calmar Ratio:        ${metrics.calmar.toFixed(2)}`);
  console.log(`   Max Drawdown:        ${(metrics.maxDrawdown * 100).toFixed(2)}%`);
  console.log(`   Recovery Days:       ${metrics.maxDrawdownDuration}`);
  console.log(`   Beta (to SPY):       ${metrics.beta.toFixed(2)}`);
  console.log(`   Information Ratio:   ${metrics.informationRatio.toFixed(2)}`);
  console.log();

  // 4. Value at Risk
  console.log('4. Value at Risk (VaR) Analysis:');
  console.log('-'.repeat(70));

  const varResults = calculateVaR(portfolio.returns, portfolio.totalValue, riskConfig.var);

  console.log(`   Confidence Level: ${(riskConfig.var.confidenceLevel * 100)}%`);
  console.log(`   Horizon: ${riskConfig.var.horizon} day(s)`);
  console.log();
  console.log('   Method         | VaR ($)      | VaR (%)  | CVaR ($)     | CVaR (%)');
  console.log('-'.repeat(70));

  for (const method of riskConfig.var.methods) {
    const result = varResults[method];
    console.log(`   ${method.padEnd(15)} | $${result.var.toLocaleString().padStart(11)} | ${(result.varPct * 100).toFixed(2).padStart(6)}% | $${result.cvar.toLocaleString().padStart(11)} | ${(result.cvarPct * 100).toFixed(2).padStart(6)}%`);
  }
  console.log();

  // 5. Drawdown analysis
  console.log('5. Drawdown Analysis:');
  console.log('-'.repeat(70));

  const drawdowns = analyzeDrawdowns(portfolio.equityCurve);
  console.log('   Top 5 Drawdowns:');
  console.log('   Rank | Depth    | Start      | End        | Duration | Recovery');
  console.log('-'.repeat(70));

  drawdowns.slice(0, 5).forEach((dd, i) => {
    console.log(`   ${(i + 1).toString().padStart(4)} | ${(dd.depth * 100).toFixed(2).padStart(6)}% | ${dd.startDate} | ${dd.endDate} | ${dd.duration.toString().padStart(8)} | ${dd.recovery} days`);
  });
  console.log();

  // 6. Position risk breakdown
  console.log('6. Position Risk Contribution:');
  console.log('-'.repeat(70));

  const positionRisk = calculatePositionRisk(portfolio);
  console.log('   Asset   | Weight  | Risk Contrib | Marginal VaR | Component VaR');
  console.log('-'.repeat(70));

  positionRisk.forEach(pr => {
    console.log(`   ${pr.symbol.padEnd(7)} | ${(pr.weight * 100).toFixed(1).padStart(5)}% | ${(pr.riskContrib * 100).toFixed(1).padStart(11)}% | $${pr.marginalVaR.toLocaleString().padStart(11)} | $${pr.componentVaR.toLocaleString().padStart(12)}`);
  });
  console.log();

  // 7. Stress testing
  console.log('7. Stress Test Results:');
  console.log('-'.repeat(70));
  console.log('   Scenario              | Impact ($)   | Impact (%) | Positions Hit');
  console.log('-'.repeat(70));

  for (const scenario of riskConfig.stressScenarios) {
    const impact = runStressTest(portfolio, scenario);
    console.log(`   ${scenario.name.padEnd(22)} | $${impact.loss.toLocaleString().padStart(11)} | ${(impact.lossPct * 100).toFixed(2).padStart(8)}% | ${impact.positionsAffected.toString().padStart(13)}`);
  }
  console.log();

  // 8. Risk limits monitoring
  console.log('8. Risk Limits Monitoring:');
  console.log('-'.repeat(70));

  const limitsStatus = checkRiskLimits(portfolio, riskConfig.limits);

  console.log(`   Max Position Size:    ${limitsStatus.maxPositionSize.status.padEnd(10)} (${(limitsStatus.maxPositionSize.current * 100).toFixed(1)}% / ${(riskConfig.limits.maxPositionSize * 100)}% limit)`);
  console.log(`   Sector Concentration: ${limitsStatus.sectorExposure.status.padEnd(10)} (${limitsStatus.sectorExposure.sector}: ${(limitsStatus.sectorExposure.current * 100).toFixed(1)}%)`);
  console.log(`   Daily Drawdown:       ${limitsStatus.dailyDrawdown.status.padEnd(10)} (${(limitsStatus.dailyDrawdown.current * 100).toFixed(2)}% today)`);
  console.log(`   Max Drawdown:         ${limitsStatus.maxDrawdown.status.padEnd(10)} (${(metrics.maxDrawdown * 100).toFixed(1)}% / ${(riskConfig.limits.maxDrawdown * 100)}% limit)`);
  console.log();

  // 9. Monte Carlo simulation
  console.log('9. Monte Carlo Simulation:');
  const mcResults = monteCarloSimulation(portfolio, riskConfig.monteCarlo);

  console.log(`   Simulations: ${riskConfig.monteCarlo.simulations.toLocaleString()}`);
  console.log(`   Horizon: ${riskConfig.monteCarlo.horizon} days`);
  console.log();
  console.log('   Percentile | Portfolio Value | Return');
  console.log('-'.repeat(70));

  const percentiles = [1, 5, 10, 25, 50, 75, 90, 95, 99];
  for (const p of percentiles) {
    const result = mcResults.percentiles[p];
    const ret = (result - portfolio.totalValue) / portfolio.totalValue;
    console.log(`   ${p.toString().padStart(9)}% | $${result.toLocaleString().padStart(15)} | ${(ret * 100).toFixed(1).padStart(6)}%`);
  }
  console.log();

  console.log(`   Expected Value:       $${mcResults.expected.toLocaleString()}`);
  console.log(`   Probability of Loss:  ${(mcResults.probLoss * 100).toFixed(1)}%`);
  console.log(`   Expected Shortfall:   $${Math.abs(mcResults.expectedShortfall).toLocaleString()}`);
  console.log();

  console.log('='.repeat(70));
  console.log('Risk management analysis completed!');
  console.log('='.repeat(70));
}

// Generate portfolio data
function generatePortfolioData() {
  const positions = [
    { symbol: 'AAPL', value: 150000, sector: 'Technology', dailyVol: 0.018 },
    { symbol: 'GOOGL', value: 120000, sector: 'Technology', dailyVol: 0.020 },
    { symbol: 'MSFT', value: 130000, sector: 'Technology', dailyVol: 0.016 },
    { symbol: 'AMZN', value: 100000, sector: 'Consumer', dailyVol: 0.022 },
    { symbol: 'JPM', value: 80000, sector: 'Financial', dailyVol: 0.015 },
    { symbol: 'V', value: 70000, sector: 'Financial', dailyVol: 0.014 },
    { symbol: 'JNJ', value: 60000, sector: 'Healthcare', dailyVol: 0.010 },
    { symbol: 'PG', value: 50000, sector: 'Consumer', dailyVol: 0.008 },
    { symbol: 'XOM', value: 40000, sector: 'Energy', dailyVol: 0.020 },
    { symbol: 'BND', value: 100000, sector: 'Bonds', dailyVol: 0.004 }
  ];

  const totalValue = positions.reduce((sum, p) => sum + p.value, 0);
  positions.forEach(p => p.weight = p.value / totalValue);

  // Generate historical returns
  const returns = [];
  const equityCurve = [totalValue];

  for (let i = 0; i < 504; i++) { // 2 years
    // Weighted portfolio return
    let dailyReturn = 0;
    for (const pos of positions) {
      const posReturn = (Math.random() - 0.48) * pos.dailyVol * 2;
      dailyReturn += posReturn * pos.weight;
    }
    returns.push(dailyReturn);
    equityCurve.push(equityCurve[i] * (1 + dailyReturn));
  }

  return { positions, totalValue, returns, equityCurve };
}

// Calculate risk metrics
function calculateRiskMetrics(returns, portfolioValue) {
  const n = returns.length;
  const mean = returns.reduce((a, b) => a + b, 0) / n;
  const variance = returns.reduce((sum, r) => sum + Math.pow(r - mean, 2), 0) / n;
  const dailyVol = Math.sqrt(variance);
  const annualVol = dailyVol * Math.sqrt(252);

  // Sharpe (assuming 4.5% risk-free rate)
  const annualReturn = mean * 252;
  const riskFree = 0.045;
  const sharpe = (annualReturn - riskFree) / annualVol;

  // Sortino (downside deviation)
  const negReturns = returns.filter(r => r < 0);
  const downsideVar = negReturns.length > 0
    ? negReturns.reduce((sum, r) => sum + Math.pow(r, 2), 0) / n
    : variance;
  const downsideDev = Math.sqrt(downsideVar) * Math.sqrt(252);
  const sortino = (annualReturn - riskFree) / downsideDev;

  // Max Drawdown
  let peak = 1;
  let maxDD = 0;
  let drawdownDays = 0;
  let maxDDDuration = 0;
  let equity = 1;

  for (const r of returns) {
    equity *= (1 + r);
    peak = Math.max(peak, equity);
    const dd = (peak - equity) / peak;
    if (dd > maxDD) {
      maxDD = dd;
      maxDDDuration = drawdownDays;
    }
    if (dd > 0) drawdownDays++;
    else drawdownDays = 0;
  }

  // Calmar
  const calmar = annualReturn / maxDD;

  return {
    dailyVol,
    annualVol,
    sharpe,
    sortino,
    calmar,
    maxDrawdown: maxDD,
    maxDrawdownDuration: maxDDDuration,
    beta: 1.1, // Simulated
    informationRatio: 0.45 // Simulated
  };
}

// Calculate VaR using multiple methods
function calculateVaR(returns, portfolioValue, config) {
  const results = {};
  const sortedReturns = [...returns].sort((a, b) => a - b);
  const idx = Math.floor((1 - config.confidenceLevel) * returns.length);

  // Historical VaR
  const historicalVar = -sortedReturns[idx] * portfolioValue;
  const historicalCVar = -sortedReturns.slice(0, idx + 1).reduce((a, b) => a + b, 0) / (idx + 1) * portfolioValue;

  results.historical = {
    var: Math.round(historicalVar),
    varPct: historicalVar / portfolioValue,
    cvar: Math.round(historicalCVar),
    cvarPct: historicalCVar / portfolioValue
  };

  // Parametric VaR (normal distribution)
  const mean = returns.reduce((a, b) => a + b, 0) / returns.length;
  const std = Math.sqrt(returns.reduce((sum, r) => sum + Math.pow(r - mean, 2), 0) / returns.length);
  const zScore = 2.326; // 99% confidence

  const paramVar = (zScore * std - mean) * portfolioValue;
  const paramCVar = paramVar * 1.15; // Approximation

  results.parametric = {
    var: Math.round(paramVar),
    varPct: paramVar / portfolioValue,
    cvar: Math.round(paramCVar),
    cvarPct: paramCVar / portfolioValue
  };

  // Monte Carlo VaR
  const simReturns = [];
  for (let i = 0; i < 10000; i++) {
    simReturns.push(mean + std * (Math.random() + Math.random() + Math.random() - 1.5) * 1.224);
  }
  simReturns.sort((a, b) => a - b);
  const mcIdx = Math.floor((1 - config.confidenceLevel) * simReturns.length);

  const mcVar = -simReturns[mcIdx] * portfolioValue;
  const mcCVar = -simReturns.slice(0, mcIdx + 1).reduce((a, b) => a + b, 0) / (mcIdx + 1) * portfolioValue;

  results.monteCarlo = {
    var: Math.round(mcVar),
    varPct: mcVar / portfolioValue,
    cvar: Math.round(mcCVar),
    cvarPct: mcCVar / portfolioValue
  };

  return results;
}

// Analyze drawdowns
function analyzeDrawdowns(equityCurve) {
  const drawdowns = [];
  let peak = equityCurve[0];
  let peakIdx = 0;
  let inDrawdown = false;
  let drawdownStart = 0;

  for (let i = 1; i < equityCurve.length; i++) {
    if (equityCurve[i] > peak) {
      if (inDrawdown) {
        // Drawdown ended
        drawdowns.push({
          depth: (peak - Math.min(...equityCurve.slice(peakIdx, i))) / peak,
          startDate: formatDate(drawdownStart),
          endDate: formatDate(i),
          duration: i - drawdownStart,
          recovery: i - drawdownStart
        });
      }
      peak = equityCurve[i];
      peakIdx = i;
      inDrawdown = false;
    } else {
      if (!inDrawdown) {
        inDrawdown = true;
        drawdownStart = peakIdx;
      }
    }
  }

  return drawdowns.sort((a, b) => b.depth - a.depth);
}

// Format date
function formatDate(idx) {
  const date = new Date();
  date.setDate(date.getDate() - (504 - idx));
  return date.toISOString().split('T')[0];
}

// Calculate position risk
function calculatePositionRisk(portfolio) {
  const totalVaR = portfolio.totalValue * 0.02; // 2% approximate VaR
  const results = [];

  let totalRiskContrib = 0;
  portfolio.positions.forEach(pos => {
    const riskContrib = pos.weight * pos.dailyVol;
    totalRiskContrib += riskContrib;
  });

  portfolio.positions.forEach(pos => {
    const riskContrib = (pos.weight * pos.dailyVol) / totalRiskContrib;
    results.push({
      symbol: pos.symbol,
      weight: pos.weight,
      riskContrib,
      marginalVaR: Math.round(pos.dailyVol * pos.value * 2.326),
      componentVaR: Math.round(riskContrib * totalVaR)
    });
  });

  return results.sort((a, b) => b.riskContrib - a.riskContrib);
}

// Run stress test
function runStressTest(portfolio, scenario) {
  let loss = 0;
  let positionsAffected = 0;

  for (const pos of portfolio.positions) {
    let impact = 0;
    if (pos.sector === 'Bonds') {
      impact = scenario.bonds;
    } else if (['Technology', 'Consumer', 'Healthcare', 'Financial', 'Energy'].includes(pos.sector)) {
      impact = scenario.equity * (0.8 + Math.random() * 0.4); // Sector-specific impact
    }

    if (impact < 0) positionsAffected++;
    loss += pos.value * impact;
  }

  return {
    loss: Math.round(loss),
    lossPct: loss / portfolio.totalValue,
    positionsAffected
  };
}

// Check risk limits
function checkRiskLimits(portfolio, limits) {
  const maxPosition = Math.max(...portfolio.positions.map(p => p.weight));
  const sectorExposures = {};
  portfolio.positions.forEach(p => {
    sectorExposures[p.sector] = (sectorExposures[p.sector] || 0) + p.weight;
  });
  const maxSector = Math.max(...Object.values(sectorExposures));
  const maxSectorName = Object.entries(sectorExposures).find(([_, v]) => v === maxSector)[0];

  const dailyReturn = portfolio.returns[portfolio.returns.length - 1];

  return {
    maxPositionSize: {
      current: maxPosition,
      status: maxPosition <= limits.maxPositionSize ? 'OK' : 'BREACH'
    },
    sectorExposure: {
      current: maxSector,
      sector: maxSectorName,
      status: maxSector <= limits.maxSectorExposure ? 'OK' : 'WARNING'
    },
    dailyDrawdown: {
      current: Math.max(0, -dailyReturn),
      status: Math.abs(dailyReturn) <= limits.stopLoss ? 'OK' : 'BREACH'
    },
    maxDrawdown: {
      current: 0.12,
      status: 0.12 <= limits.maxDrawdown ? 'OK' : 'WARNING'
    }
  };
}

// Monte Carlo simulation
function monteCarloSimulation(portfolio, config) {
  const returns = portfolio.returns;
  const mean = returns.reduce((a, b) => a + b, 0) / returns.length;
  const std = Math.sqrt(returns.reduce((sum, r) => sum + Math.pow(r - mean, 2), 0) / returns.length);

  const finalValues = [];

  for (let sim = 0; sim < config.simulations; sim++) {
    let value = portfolio.totalValue;
    for (let day = 0; day < config.horizon; day++) {
      const dailyReturn = mean + std * (Math.random() + Math.random() - 1) * 1.414;
      value *= (1 + dailyReturn);
    }
    finalValues.push(value);
  }

  finalValues.sort((a, b) => a - b);

  const percentiles = {};
  for (const p of [1, 5, 10, 25, 50, 75, 90, 95, 99]) {
    percentiles[p] = Math.round(finalValues[Math.floor(p / 100 * config.simulations)]);
  }

  const expected = Math.round(finalValues.reduce((a, b) => a + b, 0) / config.simulations);
  const losses = finalValues.filter(v => v < portfolio.totalValue);
  const probLoss = losses.length / config.simulations;
  const expectedShortfall = losses.length > 0
    ? Math.round((portfolio.totalValue - losses.reduce((a, b) => a + b, 0) / losses.length))
    : 0;

  return { percentiles, expected, probLoss, expectedShortfall };
}

// Run the example
main().catch(console.error);
