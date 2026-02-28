/**
 * Sports Betting with Neural Trader
 *
 * Demonstrates using @neural-trader/sports-betting for:
 * - Arbitrage detection across sportsbooks
 * - Kelly Criterion position sizing
 * - Expected Value (EV) calculations
 * - Odds comparison and analysis
 * - Bankroll management
 */

// Sports betting configuration
const bettingConfig = {
  // Bankroll settings
  initialBankroll: 10000,
  maxBetPercent: 0.05,      // 5% max per bet (conservative Kelly)
  minEdge: 0.02,            // 2% minimum edge to bet
  fractionKelly: 0.25,      // Quarter Kelly for safety

  // Sportsbooks
  sportsbooks: ['DraftKings', 'FanDuel', 'BetMGM', 'Caesars', 'PointsBet'],

  // Sports to analyze
  sports: ['NFL', 'NBA', 'MLB', 'NHL', 'Soccer', 'UFC']
};

// Sample odds data (American format)
const sampleOdds = {
  'NFL_Week17_Chiefs_Raiders': {
    event: 'Kansas City Chiefs vs Las Vegas Raiders',
    sport: 'NFL',
    date: '2024-12-29',
    time: '16:25 ET',
    odds: {
      'DraftKings':  { moneyline: { home: -280, away: +230 }, spread: { home: -6.5, homeOdds: -110, away: +6.5, awayOdds: -110 }, total: { over: 44.5, overOdds: -110, under: 44.5, underOdds: -110 } },
      'FanDuel':     { moneyline: { home: -285, away: +235 }, spread: { home: -6.5, homeOdds: -112, away: +6.5, awayOdds: -108 }, total: { over: 44.5, overOdds: -108, under: 44.5, underOdds: -112 } },
      'BetMGM':      { moneyline: { home: -275, away: +225 }, spread: { home: -6.5, homeOdds: -108, away: +6.5, awayOdds: -112 }, total: { over: 45.0, overOdds: -110, under: 45.0, underOdds: -110 } },
      'Caesars':     { moneyline: { home: -290, away: +240 }, spread: { home: -7.0, homeOdds: -110, away: +7.0, awayOdds: -110 }, total: { over: 44.5, overOdds: -105, under: 44.5, underOdds: -115 } },
      'PointsBet':   { moneyline: { home: -270, away: +220 }, spread: { home: -6.5, homeOdds: -115, away: +6.5, awayOdds: -105 }, total: { over: 44.5, overOdds: -112, under: 44.5, underOdds: -108 } }
    },
    trueProbability: { home: 0.72, away: 0.28 } // Model estimate
  },
  'NBA_Lakers_Warriors': {
    event: 'Los Angeles Lakers vs Golden State Warriors',
    sport: 'NBA',
    date: '2024-12-30',
    time: '19:30 ET',
    odds: {
      'DraftKings':  { moneyline: { home: +145, away: -170 }, spread: { home: +4.5, homeOdds: -110, away: -4.5, awayOdds: -110 }, total: { over: 225.5, overOdds: -110, under: 225.5, underOdds: -110 } },
      'FanDuel':     { moneyline: { home: +150, away: -175 }, spread: { home: +4.5, homeOdds: -108, away: -4.5, awayOdds: -112 }, total: { over: 226.0, overOdds: -110, under: 226.0, underOdds: -110 } },
      'BetMGM':      { moneyline: { home: +140, away: -165 }, spread: { home: +4.0, homeOdds: -110, away: -4.0, awayOdds: -110 }, total: { over: 225.5, overOdds: -108, under: 225.5, underOdds: -112 } },
      'Caesars':     { moneyline: { home: +155, away: -180 }, spread: { home: +5.0, homeOdds: -110, away: -5.0, awayOdds: -110 }, total: { over: 225.0, overOdds: -115, under: 225.0, underOdds: -105 } },
      'PointsBet':   { moneyline: { home: +160, away: -185 }, spread: { home: +5.0, homeOdds: -105, away: -5.0, awayOdds: -115 }, total: { over: 226.5, overOdds: -110, under: 226.5, underOdds: -110 } }
    },
    trueProbability: { home: 0.42, away: 0.58 }
  }
};

async function main() {
  console.log('='.repeat(70));
  console.log('Sports Betting Analysis - Neural Trader');
  console.log('='.repeat(70));
  console.log();

  // 1. Display configuration
  console.log('1. Betting Configuration:');
  console.log('-'.repeat(70));
  console.log(`   Initial Bankroll:  $${bettingConfig.initialBankroll.toLocaleString()}`);
  console.log(`   Max Bet Size:      ${bettingConfig.maxBetPercent * 100}% ($${bettingConfig.initialBankroll * bettingConfig.maxBetPercent})`);
  console.log(`   Kelly Fraction:    ${bettingConfig.fractionKelly * 100}%`);
  console.log(`   Minimum Edge:      ${bettingConfig.minEdge * 100}%`);
  console.log(`   Sportsbooks:       ${bettingConfig.sportsbooks.join(', ')}`);
  console.log();

  // 2. Analyze each event
  for (const [eventId, eventData] of Object.entries(sampleOdds)) {
    console.log(`2. Event Analysis: ${eventData.event}`);
    console.log('-'.repeat(70));
    console.log(`   Sport: ${eventData.sport} | Date: ${eventData.date} ${eventData.time}`);
    console.log();

    // Display odds comparison
    console.log('   Moneyline Odds Comparison:');
    console.log('   Sportsbook    | Home      | Away      | Home Prob | Away Prob | Vig');
    console.log('   ' + '-'.repeat(60));

    for (const [book, odds] of Object.entries(eventData.odds)) {
      const homeProb = americanToImpliedProb(odds.moneyline.home);
      const awayProb = americanToImpliedProb(odds.moneyline.away);
      const vig = (homeProb + awayProb - 1) * 100;

      console.log(`   ${book.padEnd(13)} | ${formatOdds(odds.moneyline.home).padStart(9)} | ${formatOdds(odds.moneyline.away).padStart(9)} | ${(homeProb * 100).toFixed(1).padStart(8)}% | ${(awayProb * 100).toFixed(1).padStart(8)}% | ${vig.toFixed(1)}%`);
    }
    console.log();

    // Find best odds
    const bestHomeOdds = findBestOdds(eventData.odds, 'moneyline', 'home');
    const bestAwayOdds = findBestOdds(eventData.odds, 'moneyline', 'away');

    console.log(`   Best Home Odds: ${formatOdds(bestHomeOdds.odds)} at ${bestHomeOdds.book}`);
    console.log(`   Best Away Odds: ${formatOdds(bestAwayOdds.odds)} at ${bestAwayOdds.book}`);
    console.log();

    // Check for arbitrage
    console.log('   Arbitrage Analysis:');
    const arbResult = checkArbitrage(eventData.odds);

    if (arbResult.hasArbitrage) {
      console.log(`   🎯 ARBITRAGE OPPORTUNITY FOUND!`);
      console.log(`   Guaranteed profit: ${(arbResult.profit * 100).toFixed(2)}%`);
      console.log(`   Bet ${arbResult.homeBook} Home: $${arbResult.homeBet.toFixed(2)}`);
      console.log(`   Bet ${arbResult.awayBook} Away: $${arbResult.awayBet.toFixed(2)}`);
    } else {
      console.log(`   No pure arbitrage available (combined implied: ${(arbResult.combinedImplied * 100).toFixed(1)}%)`);
    }
    console.log();

    // EV calculations
    console.log('   Expected Value Analysis (using model probabilities):');
    console.log(`   Model: Home ${(eventData.trueProbability.home * 100).toFixed(0)}% | Away ${(eventData.trueProbability.away * 100).toFixed(0)}%`);
    console.log();
    console.log('   Bet             | Book          | Odds      | EV       | Kelly   | Recommended');
    console.log('   ' + '-'.repeat(65));

    const evAnalysis = calculateEVForAllBets(eventData);
    evAnalysis.forEach(bet => {
      const evStr = bet.ev >= 0 ? `+${(bet.ev * 100).toFixed(2)}%` : `${(bet.ev * 100).toFixed(2)}%`;
      const kellyStr = bet.kelly > 0 ? `${(bet.kelly * 100).toFixed(2)}%` : '-';
      const recBet = bet.recommendedBet > 0 ? `$${bet.recommendedBet.toFixed(0)}` : 'PASS';

      console.log(`   ${bet.type.padEnd(16)} | ${bet.book.padEnd(13)} | ${formatOdds(bet.odds).padStart(9)} | ${evStr.padStart(8)} | ${kellyStr.padStart(7)} | ${recBet.padStart(11)}`);
    });
    console.log();

    // Top recommended bets
    const topBets = evAnalysis.filter(b => b.recommendedBet > 0).sort((a, b) => b.ev - a.ev);
    if (topBets.length > 0) {
      console.log(`   📊 Top Recommended Bet:`);
      const best = topBets[0];
      console.log(`      ${best.type} at ${best.book}`);
      console.log(`      Odds: ${formatOdds(best.odds)} | EV: +${(best.ev * 100).toFixed(2)}% | Bet Size: $${best.recommendedBet.toFixed(0)}`);
    }
    console.log();
  }

  // 3. Bankroll simulation
  console.log('3. Bankroll Growth Simulation:');
  console.log('-'.repeat(70));

  const simulation = simulateBankrollGrowth(1000, 0.03, 0.55, bettingConfig);
  console.log(`   Starting Bankroll: $${bettingConfig.initialBankroll.toLocaleString()}`);
  console.log(`   Bets Placed:       ${simulation.totalBets}`);
  console.log(`   Win Rate:          ${(simulation.winRate * 100).toFixed(1)}%`);
  console.log(`   Final Bankroll:    $${simulation.finalBankroll.toLocaleString()}`);
  console.log(`   ROI:               ${((simulation.finalBankroll / bettingConfig.initialBankroll - 1) * 100).toFixed(1)}%`);
  console.log(`   Max Drawdown:      ${(simulation.maxDrawdown * 100).toFixed(1)}%`);
  console.log();

  // 4. Syndicate management (advanced)
  console.log('4. Syndicate Management:');
  console.log('-'.repeat(70));
  console.log('   Account Diversification Strategy:');
  console.log('   - Spread bets across multiple sportsbooks');
  console.log('   - Maximum 20% of action per book');
  console.log('   - Rotate accounts to avoid limits');
  console.log('   - Track CLV (Closing Line Value) per book');
  console.log();

  console.log('='.repeat(70));
  console.log('Sports betting analysis completed!');
  console.log('='.repeat(70));
}

// Convert American odds to implied probability
function americanToImpliedProb(odds) {
  if (odds > 0) {
    return 100 / (odds + 100);
  } else {
    return Math.abs(odds) / (Math.abs(odds) + 100);
  }
}

// Convert implied probability to American odds
function probToAmerican(prob) {
  if (prob >= 0.5) {
    return Math.round(-100 * prob / (1 - prob));
  } else {
    return Math.round(100 * (1 - prob) / prob);
  }
}

// Format American odds
function formatOdds(odds) {
  return odds > 0 ? `+${odds}` : `${odds}`;
}

// Find best odds across sportsbooks
function findBestOdds(odds, market, side) {
  let best = { odds: -Infinity, book: '' };

  for (const [book, bookOdds] of Object.entries(odds)) {
    const odd = bookOdds[market][side];
    if (odd > best.odds) {
      best = { odds: odd, book };
    }
  }

  return best;
}

// Check for arbitrage opportunity
function checkArbitrage(odds) {
  const bestHome = findBestOdds(odds, 'moneyline', 'home');
  const bestAway = findBestOdds(odds, 'moneyline', 'away');

  const homeProb = americanToImpliedProb(bestHome.odds);
  const awayProb = americanToImpliedProb(bestAway.odds);
  const combinedImplied = homeProb + awayProb;

  if (combinedImplied < 1) {
    // Arbitrage exists!
    const profit = 1 / combinedImplied - 1;
    const totalStake = 1000;
    const homeBet = totalStake * (homeProb / combinedImplied);
    const awayBet = totalStake * (awayProb / combinedImplied);

    return {
      hasArbitrage: true,
      profit,
      combinedImplied,
      homeBook: bestHome.book,
      awayBook: bestAway.book,
      homeBet,
      awayBet
    };
  }

  return { hasArbitrage: false, combinedImplied };
}

// Calculate EV for all betting options
function calculateEVForAllBets(eventData) {
  const results = [];
  const bankroll = bettingConfig.initialBankroll;

  for (const [book, odds] of Object.entries(eventData.odds)) {
    // Home moneyline
    const homeOdds = odds.moneyline.home;
    const homeEV = calculateEV(eventData.trueProbability.home, homeOdds);
    const homeKelly = calculateKelly(eventData.trueProbability.home, homeOdds);
    const homeRec = homeEV >= bettingConfig.minEdge
      ? Math.min(homeKelly * bettingConfig.fractionKelly, bettingConfig.maxBetPercent) * bankroll
      : 0;

    results.push({
      type: 'Home Moneyline',
      book,
      odds: homeOdds,
      ev: homeEV,
      kelly: homeKelly,
      recommendedBet: homeRec
    });

    // Away moneyline
    const awayOdds = odds.moneyline.away;
    const awayEV = calculateEV(eventData.trueProbability.away, awayOdds);
    const awayKelly = calculateKelly(eventData.trueProbability.away, awayOdds);
    const awayRec = awayEV >= bettingConfig.minEdge
      ? Math.min(awayKelly * bettingConfig.fractionKelly, bettingConfig.maxBetPercent) * bankroll
      : 0;

    results.push({
      type: 'Away Moneyline',
      book,
      odds: awayOdds,
      ev: awayEV,
      kelly: awayKelly,
      recommendedBet: awayRec
    });
  }

  return results.sort((a, b) => b.ev - a.ev);
}

// Calculate Expected Value
function calculateEV(trueProb, americanOdds) {
  const impliedProb = americanToImpliedProb(americanOdds);
  const decimalOdds = americanOdds > 0 ? (americanOdds / 100) + 1 : (100 / Math.abs(americanOdds)) + 1;

  return (trueProb * decimalOdds) - 1;
}

// Calculate Kelly Criterion
function calculateKelly(trueProb, americanOdds) {
  const decimalOdds = americanOdds > 0 ? (americanOdds / 100) + 1 : (100 / Math.abs(americanOdds)) + 1;
  const b = decimalOdds - 1;
  const p = trueProb;
  const q = 1 - p;

  const kelly = (b * p - q) / b;
  return Math.max(0, kelly);
}

// Simulate bankroll growth
function simulateBankrollGrowth(numBets, avgEdge, winRate, config) {
  let bankroll = config.initialBankroll;
  let peak = bankroll;
  let maxDrawdown = 0;
  let wins = 0;

  for (let i = 0; i < numBets; i++) {
    const betSize = bankroll * config.maxBetPercent * config.fractionKelly;
    const isWin = Math.random() < winRate;

    if (isWin) {
      bankroll += betSize * (1 + avgEdge);
      wins++;
    } else {
      bankroll -= betSize;
    }

    peak = Math.max(peak, bankroll);
    maxDrawdown = Math.max(maxDrawdown, (peak - bankroll) / peak);
  }

  return {
    totalBets: numBets,
    winRate: wins / numBets,
    finalBankroll: Math.round(bankroll),
    maxDrawdown
  };
}

// Run the example
main().catch(console.error);
