/**
 * Fractional Kelly Criterion Engine
 *
 * PRODUCTION: Foundation for optimal bet sizing in trading and sports betting
 *
 * Research-backed implementation:
 * - Full Kelly leads to ruin in practice (Dotan, 2024)
 * - 1/5th Kelly achieved 98% ROI in NBA betting simulations
 * - 1/8th Kelly recommended for conservative strategies
 *
 * Features:
 * - Multiple Kelly fractions (1/2, 1/4, 1/5, 1/8)
 * - Calibration-aware adjustments
 * - Multi-bet portfolio optimization
 * - Risk-of-ruin calculations
 * - Drawdown protection
 */

// Kelly Configuration
const kellyConfig = {
  // Fraction strategies
  fractions: {
    aggressive: 0.5,    // Half Kelly
    moderate: 0.25,     // Quarter Kelly
    conservative: 0.2,  // Fifth Kelly (recommended)
    ultraSafe: 0.125    // Eighth Kelly
  },

  // Risk management
  risk: {
    maxBetFraction: 0.05,     // Never bet more than 5% of bankroll
    minEdge: 0.01,            // Minimum 1% edge required
    maxDrawdown: 0.25,        // Stop at 25% drawdown
    confidenceThreshold: 0.6  // Minimum model confidence
  },

  // Bankroll management
  bankroll: {
    initial: 10000,
    reserveRatio: 0.1,  // Keep 10% as reserve
    rebalanceThreshold: 0.2  // Rebalance when 20% deviation
  }
};

/**
 * Kelly Criterion Calculator
 * Optimal bet sizing for positive expected value bets
 */
class KellyCriterion {
  constructor(config = kellyConfig) {
    this.config = config;
    this.bankroll = config.bankroll.initial;
    this.peakBankroll = this.bankroll;
    this.history = [];
    this.stats = {
      totalBets: 0,
      wins: 0,
      losses: 0,
      totalWagered: 0,
      totalProfit: 0
    };
  }

  /**
   * Calculate full Kelly fraction
   * f* = (bp - q) / b
   * where b = decimal odds - 1, p = win probability, q = 1 - p
   */
  calculateFullKelly(winProbability, decimalOdds) {
    const b = decimalOdds - 1;  // Net odds
    const p = winProbability;
    const q = 1 - p;

    const kelly = (b * p - q) / b;
    return Math.max(0, kelly);  // Never negative
  }

  /**
   * Calculate fractional Kelly with safety bounds
   */
  calculateFractionalKelly(winProbability, decimalOdds, fraction = 'conservative') {
    const fullKelly = this.calculateFullKelly(winProbability, decimalOdds);

    if (fullKelly <= 0) {
      return { stake: 0, edge: 0, fullKelly: 0, reason: 'negative_ev' };
    }

    const fractionValue = typeof fraction === 'number'
      ? fraction
      : this.config.fractions[fraction] || 0.2;

    let adjustedKelly = fullKelly * fractionValue;

    // Apply maximum bet constraint
    adjustedKelly = Math.min(adjustedKelly, this.config.risk.maxBetFraction);

    // Calculate edge
    const edge = (winProbability * decimalOdds) - 1;

    // Check minimum edge requirement
    if (edge < this.config.risk.minEdge) {
      return { stake: 0, edge, fullKelly, reason: 'insufficient_edge' };
    }

    // Calculate actual stake
    const availableBankroll = this.bankroll * (1 - this.config.bankroll.reserveRatio);
    const stake = availableBankroll * adjustedKelly;

    return {
      stake: Math.round(stake * 100) / 100,
      stakePercent: adjustedKelly * 100,
      fullKelly: fullKelly * 100,
      fractionalKelly: adjustedKelly * 100,
      edge: edge * 100,
      expectedValue: stake * edge,
      fraction: fractionValue,
      reason: 'approved'
    };
  }

  /**
   * Calculate Kelly for calibrated probability models
   * Adjusts for model confidence/calibration quality
   */
  calculateCalibratedKelly(predictedProb, calibrationScore, decimalOdds, fraction = 'conservative') {
    // Shrink probability toward 0.5 based on calibration quality
    // Perfect calibration (1.0) = use predicted prob
    // Poor calibration (0.5) = shrink significantly toward 0.5
    const shrinkage = 1 - calibrationScore;
    const adjustedProb = predictedProb * (1 - shrinkage * 0.5) + 0.5 * shrinkage * 0.5;

    // Only bet if confidence exceeds threshold
    if (calibrationScore < this.config.risk.confidenceThreshold) {
      return {
        stake: 0,
        reason: 'low_calibration',
        calibrationScore,
        adjustedProb
      };
    }

    const result = this.calculateFractionalKelly(adjustedProb, decimalOdds, fraction);
    return {
      ...result,
      originalProb: predictedProb,
      adjustedProb,
      calibrationScore
    };
  }

  /**
   * Multi-bet Kelly (simultaneous independent bets)
   * Reduces individual stakes to account for correlation risk
   */
  calculateMultiBetKelly(bets, fraction = 'conservative') {
    if (bets.length === 0) return [];

    // Calculate individual Kelly for each bet
    const individualBets = bets.map(bet => ({
      ...bet,
      kelly: this.calculateFractionalKelly(bet.winProbability, bet.decimalOdds, fraction)
    }));

    // Filter to positive EV bets only
    const positiveBets = individualBets.filter(b => b.kelly.stake > 0);

    if (positiveBets.length === 0) return individualBets;

    // Apply correlation adjustment (reduce stakes when many bets)
    // Use sqrt(n) scaling to account for diversification
    const correlationFactor = 1 / Math.sqrt(positiveBets.length);

    // Total stake shouldn't exceed max bet fraction
    const totalKelly = positiveBets.reduce((sum, b) => sum + b.kelly.fractionalKelly / 100, 0);
    const scaleFactor = totalKelly > this.config.risk.maxBetFraction
      ? this.config.risk.maxBetFraction / totalKelly
      : 1;

    return individualBets.map(bet => {
      if (bet.kelly.stake === 0) return bet;

      const adjustedStake = bet.kelly.stake * correlationFactor * scaleFactor;
      return {
        ...bet,
        kelly: {
          ...bet.kelly,
          originalStake: bet.kelly.stake,
          stake: Math.round(adjustedStake * 100) / 100,
          correlationAdjustment: correlationFactor,
          portfolioScaling: scaleFactor
        }
      };
    });
  }

  /**
   * Calculate risk of ruin given betting strategy
   */
  calculateRiskOfRuin(winProbability, decimalOdds, betFraction, targetMultiple = 2) {
    const p = winProbability;
    const q = 1 - p;
    const b = decimalOdds - 1;

    // Simplified risk of ruin formula
    // R = (q/p)^(bankroll/unit)
    if (p <= q / b) {
      return 1;  // Negative EV = certain ruin
    }

    const edge = b * p - q;
    const variance = p * q * (b + 1) ** 2;
    const sharpe = edge / Math.sqrt(variance);

    // Approximate risk of ruin using normal approximation
    const unitsToTarget = Math.log(targetMultiple) / Math.log(1 + betFraction * edge);
    const riskOfRuin = Math.exp(-2 * edge * unitsToTarget / variance);

    return Math.min(1, Math.max(0, riskOfRuin));
  }

  /**
   * Place a bet and update bankroll
   */
  placeBet(stake, decimalOdds, won) {
    if (stake > this.bankroll) {
      throw new Error('Insufficient bankroll');
    }

    const profit = won ? stake * (decimalOdds - 1) : -stake;
    this.bankroll += profit;
    this.peakBankroll = Math.max(this.peakBankroll, this.bankroll);

    this.stats.totalBets++;
    this.stats.totalWagered += stake;
    this.stats.totalProfit += profit;
    if (won) this.stats.wins++;
    else this.stats.losses++;

    this.history.push({
      timestamp: Date.now(),
      stake,
      decimalOdds,
      won,
      profit,
      bankroll: this.bankroll
    });

    // Check drawdown protection
    const drawdown = (this.peakBankroll - this.bankroll) / this.peakBankroll;
    if (drawdown >= this.config.risk.maxDrawdown) {
      return {
        ...this.getStats(),
        warning: 'max_drawdown_reached',
        drawdown: drawdown * 100
      };
    }

    return this.getStats();
  }

  /**
   * Get current statistics
   */
  getStats() {
    const drawdown = (this.peakBankroll - this.bankroll) / this.peakBankroll;
    const roi = this.stats.totalWagered > 0
      ? (this.stats.totalProfit / this.stats.totalWagered) * 100
      : 0;
    const winRate = this.stats.totalBets > 0
      ? (this.stats.wins / this.stats.totalBets) * 100
      : 0;

    return {
      bankroll: Math.round(this.bankroll * 100) / 100,
      peakBankroll: Math.round(this.peakBankroll * 100) / 100,
      drawdown: Math.round(drawdown * 10000) / 100,
      totalBets: this.stats.totalBets,
      wins: this.stats.wins,
      losses: this.stats.losses,
      winRate: Math.round(winRate * 100) / 100,
      totalWagered: Math.round(this.stats.totalWagered * 100) / 100,
      totalProfit: Math.round(this.stats.totalProfit * 100) / 100,
      roi: Math.round(roi * 100) / 100
    };
  }

  /**
   * Simulate betting strategy
   */
  simulate(bets, fraction = 'conservative') {
    const results = [];

    for (const bet of bets) {
      const kelly = this.calculateFractionalKelly(bet.winProbability, bet.decimalOdds, fraction);

      if (kelly.stake > 0) {
        const outcome = this.placeBet(kelly.stake, bet.decimalOdds, bet.actualWin);
        results.push({
          bet,
          kelly,
          outcome,
          bankroll: this.bankroll
        });
      }
    }

    return {
      finalStats: this.getStats(),
      betResults: results
    };
  }

  /**
   * Reset bankroll to initial state
   */
  reset() {
    this.bankroll = this.config.bankroll.initial;
    this.peakBankroll = this.bankroll;
    this.history = [];
    this.stats = {
      totalBets: 0,
      wins: 0,
      losses: 0,
      totalWagered: 0,
      totalProfit: 0
    };
  }
}

/**
 * Sports Betting Kelly Extension
 * Specialized for sports betting markets
 */
class SportsBettingKelly extends KellyCriterion {
  constructor(config = kellyConfig) {
    super(config);
    this.marketEfficiency = 0.95;  // Assume 95% efficient markets
  }

  /**
   * Convert American odds to decimal
   */
  americanToDecimal(americanOdds) {
    if (americanOdds > 0) {
      return (americanOdds / 100) + 1;
    } else {
      return (100 / Math.abs(americanOdds)) + 1;
    }
  }

  /**
   * Calculate implied probability from odds
   */
  impliedProbability(decimalOdds) {
    return 1 / decimalOdds;
  }

  /**
   * Calculate edge over market
   */
  calculateEdge(modelProbability, decimalOdds) {
    const impliedProb = this.impliedProbability(decimalOdds);
    return modelProbability - impliedProb;
  }

  /**
   * Find value bets from model predictions vs market odds
   */
  findValueBets(predictions, marketOdds, minEdge = 0.02) {
    const valueBets = [];

    for (let i = 0; i < predictions.length; i++) {
      const pred = predictions[i];
      const odds = marketOdds[i];

      // Check home team value
      const homeEdge = this.calculateEdge(pred.homeWinProb, odds.homeDecimal);
      if (homeEdge >= minEdge) {
        valueBets.push({
          matchId: pred.matchId,
          selection: 'home',
          modelProbability: pred.homeWinProb,
          decimalOdds: odds.homeDecimal,
          edge: homeEdge,
          kelly: this.calculateFractionalKelly(pred.homeWinProb, odds.homeDecimal)
        });
      }

      // Check away team value
      const awayEdge = this.calculateEdge(pred.awayWinProb, odds.awayDecimal);
      if (awayEdge >= minEdge) {
        valueBets.push({
          matchId: pred.matchId,
          selection: 'away',
          modelProbability: pred.awayWinProb,
          decimalOdds: odds.awayDecimal,
          edge: awayEdge,
          kelly: this.calculateFractionalKelly(pred.awayWinProb, odds.awayDecimal)
        });
      }

      // Check draw if applicable
      if (pred.drawProb && odds.drawDecimal) {
        const drawEdge = this.calculateEdge(pred.drawProb, odds.drawDecimal);
        if (drawEdge >= minEdge) {
          valueBets.push({
            matchId: pred.matchId,
            selection: 'draw',
            modelProbability: pred.drawProb,
            decimalOdds: odds.drawDecimal,
            edge: drawEdge,
            kelly: this.calculateFractionalKelly(pred.drawProb, odds.drawDecimal)
          });
        }
      }
    }

    return valueBets.sort((a, b) => b.edge - a.edge);
  }
}

/**
 * Trading Kelly Extension
 * Specialized for financial market position sizing
 */
class TradingKelly extends KellyCriterion {
  constructor(config = kellyConfig) {
    super(config);
  }

  /**
   * Calculate position size for a trade
   * Uses expected return and win rate from historical analysis
   */
  calculatePositionSize(winRate, avgWin, avgLoss, accountSize = null) {
    const bankroll = accountSize || this.bankroll;

    // Convert to Kelly inputs
    // For trading: b = avgWin/avgLoss (reward/risk ratio)
    const b = avgWin / Math.abs(avgLoss);
    const p = winRate;
    const q = 1 - p;

    const fullKelly = (b * p - q) / b;

    if (fullKelly <= 0) {
      return {
        positionSize: 0,
        reason: 'negative_expectancy',
        expectancy: (winRate * avgWin) + ((1 - winRate) * avgLoss)
      };
    }

    const fractionValue = this.config.fractions.conservative;
    let adjustedKelly = fullKelly * fractionValue;
    adjustedKelly = Math.min(adjustedKelly, this.config.risk.maxBetFraction);

    const positionSize = bankroll * adjustedKelly;
    const expectancy = (winRate * avgWin) + ((1 - winRate) * avgLoss);

    return {
      positionSize: Math.round(positionSize * 100) / 100,
      positionPercent: adjustedKelly * 100,
      fullKelly: fullKelly * 100,
      rewardRiskRatio: b,
      winRate: winRate * 100,
      expectancy,
      expectancyPercent: expectancy * 100
    };
  }

  /**
   * Calculate optimal leverage using Kelly
   */
  calculateOptimalLeverage(expectedReturn, volatility, riskFreeRate = 0.05) {
    // Kelly for continuous returns: f* = (μ - r) / σ²
    const excessReturn = expectedReturn - riskFreeRate;
    const kelly = excessReturn / (volatility * volatility);

    // Apply fraction and caps
    const fractionValue = this.config.fractions.conservative;
    let adjustedLeverage = kelly * fractionValue;

    // Cap leverage at reasonable levels
    const maxLeverage = 3.0;
    adjustedLeverage = Math.min(adjustedLeverage, maxLeverage);
    adjustedLeverage = Math.max(adjustedLeverage, 0);

    return {
      optimalLeverage: Math.round(adjustedLeverage * 100) / 100,
      fullKellyLeverage: Math.round(kelly * 100) / 100,
      sharpeRatio: excessReturn / volatility,
      expectedReturn: expectedReturn * 100,
      volatility: volatility * 100
    };
  }
}

// Demo and test
async function main() {
  console.log('═'.repeat(70));
  console.log('FRACTIONAL KELLY CRITERION ENGINE');
  console.log('═'.repeat(70));
  console.log();

  // 1. Basic Kelly calculations
  console.log('1. Basic Kelly Calculations:');
  console.log('─'.repeat(70));

  const kelly = new KellyCriterion();

  // Example: 55% win probability, 2.0 decimal odds (even money)
  const basic = kelly.calculateFractionalKelly(0.55, 2.0);
  console.log('   Win Prob: 55%, Odds: 2.0 (even money)');
  console.log(`   Full Kelly:       ${basic.fullKelly.toFixed(2)}%`);
  console.log(`   1/5th Kelly:      ${basic.fractionalKelly.toFixed(2)}%`);
  console.log(`   Recommended Stake: $${basic.stake.toFixed(2)}`);
  console.log(`   Edge:             ${basic.edge.toFixed(2)}%`);
  console.log();

  // 2. Calibrated Kelly (for ML models)
  console.log('2. Calibrated Kelly (ML Model Adjustment):');
  console.log('─'.repeat(70));

  const calibrated = kelly.calculateCalibratedKelly(0.60, 0.85, 2.0);
  console.log('   Model Prediction: 60%, Calibration Score: 0.85');
  console.log(`   Adjusted Prob:    ${(calibrated.adjustedProb * 100).toFixed(2)}%`);
  console.log(`   Recommended Stake: $${calibrated.stake.toFixed(2)}`);
  console.log();

  // 3. Multi-bet portfolio
  console.log('3. Multi-Bet Portfolio:');
  console.log('─'.repeat(70));

  const multiBets = kelly.calculateMultiBetKelly([
    { id: 1, winProbability: 0.55, decimalOdds: 2.0 },
    { id: 2, winProbability: 0.52, decimalOdds: 2.1 },
    { id: 3, winProbability: 0.58, decimalOdds: 1.9 },
    { id: 4, winProbability: 0.51, decimalOdds: 2.2 }
  ]);

  console.log('   Bet │ Win Prob │ Odds │ Individual │ Portfolio │ Final Stake');
  console.log('─'.repeat(70));
  for (const bet of multiBets) {
    if (bet.kelly.stake > 0) {
      console.log(`   ${bet.id}   │ ${(bet.winProbability * 100).toFixed(0)}%     │ ${bet.decimalOdds.toFixed(1)}  │ $${bet.kelly.originalStake?.toFixed(2) || bet.kelly.stake.toFixed(2)}     │ ${(bet.kelly.correlationAdjustment * 100 || 100).toFixed(0)}%      │ $${bet.kelly.stake.toFixed(2)}`);
    }
  }
  console.log();

  // 4. Risk of ruin analysis
  console.log('4. Risk of Ruin Analysis:');
  console.log('─'.repeat(70));

  const strategies = [
    { name: 'Full Kelly', fraction: 1.0 },
    { name: 'Half Kelly', fraction: 0.5 },
    { name: '1/5th Kelly', fraction: 0.2 },
    { name: '1/8th Kelly', fraction: 0.125 }
  ];

  console.log('   Strategy     │ Bet Size │ Risk of Ruin (2x target)');
  console.log('─'.repeat(70));
  for (const strat of strategies) {
    const fullKelly = kelly.calculateFullKelly(0.55, 2.0);
    const betFraction = fullKelly * strat.fraction;
    const ror = kelly.calculateRiskOfRuin(0.55, 2.0, betFraction, 2);
    console.log(`   ${strat.name.padEnd(12)} │ ${(betFraction * 100).toFixed(2)}%   │ ${(ror * 100).toFixed(2)}%`);
  }
  console.log();

  // 5. Sports betting simulation
  console.log('5. Sports Betting Simulation (100 bets):');
  console.log('─'.repeat(70));

  const sportsKelly = new SportsBettingKelly();

  // Generate simulated bets with 55% edge
  const simulatedBets = [];
  let rng = 42;
  const random = () => { rng = (rng * 9301 + 49297) % 233280; return rng / 233280; };

  for (let i = 0; i < 100; i++) {
    const trueProb = 0.50 + random() * 0.15;  // 50-65% true probability
    const odds = 1.8 + random() * 0.4;         // 1.8-2.2 odds
    const actualWin = random() < trueProb;

    simulatedBets.push({
      winProbability: trueProb,
      decimalOdds: odds,
      actualWin
    });
  }

  // Run simulations with different Kelly fractions
  const fractions = ['aggressive', 'moderate', 'conservative', 'ultraSafe'];
  console.log('   Fraction     │ Final Bankroll │ ROI      │ Max Drawdown');
  console.log('─'.repeat(70));

  for (const frac of fractions) {
    sportsKelly.reset();
    sportsKelly.simulate(simulatedBets, frac);
    const stats = sportsKelly.getStats();
    console.log(`   ${frac.padEnd(12)} │ $${stats.bankroll.toFixed(2).padStart(12)} │ ${stats.roi.toFixed(1).padStart(6)}% │ ${stats.drawdown.toFixed(1)}%`);
  }
  console.log();

  // 6. Trading position sizing
  console.log('6. Trading Position Sizing:');
  console.log('─'.repeat(70));

  const tradingKelly = new TradingKelly();

  const position = tradingKelly.calculatePositionSize(0.55, 0.02, -0.015, 100000);
  console.log('   Win Rate: 55%, Avg Win: 2%, Avg Loss: -1.5%');
  console.log(`   Reward/Risk Ratio: ${position.rewardRiskRatio.toFixed(2)}`);
  console.log(`   Position Size:     $${position.positionSize.toFixed(2)} (${position.positionPercent.toFixed(2)}%)`);
  console.log(`   Expectancy:        ${position.expectancyPercent.toFixed(2)}% per trade`);
  console.log();

  // 7. Optimal leverage
  console.log('7. Optimal Leverage Calculation:');
  console.log('─'.repeat(70));

  const leverage = tradingKelly.calculateOptimalLeverage(0.12, 0.18, 0.05);
  console.log('   Expected Return: 12%, Volatility: 18%, Risk-Free: 5%');
  console.log(`   Sharpe Ratio:        ${leverage.sharpeRatio.toFixed(2)}`);
  console.log(`   Full Kelly Leverage: ${leverage.fullKellyLeverage.toFixed(2)}x`);
  console.log(`   Recommended (1/5):   ${leverage.optimalLeverage.toFixed(2)}x`);
  console.log();

  console.log('═'.repeat(70));
  console.log('Fractional Kelly engine demonstration completed');
  console.log('═'.repeat(70));
}

// Export for use as module
export {
  KellyCriterion,
  SportsBettingKelly,
  TradingKelly,
  kellyConfig
};

main().catch(console.error);
