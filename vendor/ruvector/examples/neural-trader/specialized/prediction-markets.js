/**
 * Prediction Markets with Neural Trader
 *
 * Demonstrates using @neural-trader/prediction-markets for:
 * - Polymarket integration
 * - Expected value calculations
 * - Market making strategies
 * - Arbitrage across platforms
 * - Event probability analysis
 */

// Prediction market configuration
const marketConfig = {
  platforms: ['Polymarket', 'Kalshi', 'PredictIt'],
  initialCapital: 10000,
  maxPositionSize: 0.10,
  minEdge: 0.03,
  fees: {
    Polymarket: 0.02,
    Kalshi: 0.02,
    PredictIt: 0.10
  }
};

// Sample market data
const predictionMarkets = [
  {
    id: 'fed-rate-jan-2025',
    question: 'Will the Fed cut rates in January 2025?',
    category: 'Economics',
    endDate: '2025-01-31',
    platforms: {
      Polymarket: { yes: 0.22, no: 0.78, volume: 1250000 },
      Kalshi: { yes: 0.24, no: 0.76, volume: 850000 }
    },
    modelProbability: 0.18
  },
  {
    id: 'btc-100k-jan-2025',
    question: 'Will Bitcoin reach $100,000 by January 31, 2025?',
    category: 'Crypto',
    endDate: '2025-01-31',
    platforms: {
      Polymarket: { yes: 0.65, no: 0.35, volume: 5200000 },
      Kalshi: { yes: 0.62, no: 0.38, volume: 2100000 }
    },
    modelProbability: 0.70
  },
  {
    id: 'sp500-6000-q1-2025',
    question: 'Will S&P 500 close above 6000 in Q1 2025?',
    category: 'Markets',
    endDate: '2025-03-31',
    platforms: {
      Polymarket: { yes: 0.58, no: 0.42, volume: 980000 },
      Kalshi: { yes: 0.55, no: 0.45, volume: 1450000 }
    },
    modelProbability: 0.62
  },
  {
    id: 'ai-regulation-2025',
    question: 'Will the US pass major AI regulation in 2025?',
    category: 'Politics',
    endDate: '2025-12-31',
    platforms: {
      Polymarket: { yes: 0.35, no: 0.65, volume: 750000 },
      PredictIt: { yes: 0.38, no: 0.62, volume: 420000 }
    },
    modelProbability: 0.28
  },
  {
    id: 'eth-merge-upgrade-2025',
    question: 'Will Ethereum complete Pectra upgrade by March 2025?',
    category: 'Crypto',
    endDate: '2025-03-31',
    platforms: {
      Polymarket: { yes: 0.72, no: 0.28, volume: 890000 }
    },
    modelProbability: 0.75
  }
];

async function main() {
  console.log('='.repeat(70));
  console.log('Prediction Markets Analysis - Neural Trader');
  console.log('='.repeat(70));
  console.log();

  // 1. Display configuration
  console.log('1. Configuration:');
  console.log('-'.repeat(70));
  console.log(`   Capital:          $${marketConfig.initialCapital.toLocaleString()}`);
  console.log(`   Max Position:     ${marketConfig.maxPositionSize * 100}%`);
  console.log(`   Min Edge:         ${marketConfig.minEdge * 100}%`);
  console.log(`   Platforms:        ${marketConfig.platforms.join(', ')}`);
  console.log();

  // 2. Market overview
  console.log('2. Market Overview:');
  console.log('-'.repeat(70));
  console.log('   Market                                 | Category   | End Date   | Volume');
  console.log('-'.repeat(70));

  predictionMarkets.forEach(market => {
    const totalVolume = Object.values(market.platforms)
      .reduce((sum, p) => sum + p.volume, 0);
    console.log(`   ${market.question.substring(0, 40).padEnd(40)} | ${market.category.padEnd(10)} | ${market.endDate} | $${(totalVolume / 1e6).toFixed(2)}M`);
  });
  console.log();

  // 3. Analyze each market
  console.log('3. Market Analysis:');
  console.log('='.repeat(70));

  const opportunities = [];

  for (const market of predictionMarkets) {
    console.log();
    console.log(`   📊 ${market.question}`);
    console.log('-'.repeat(70));
    console.log(`   Category: ${market.category} | End: ${market.endDate} | Model P(Yes): ${(market.modelProbability * 100).toFixed(0)}%`);
    console.log();

    // Platform comparison
    console.log('   Platform     | Yes Price | No Price  | Implied P | Spread | Volume');
    console.log('   ' + '-'.repeat(60));

    for (const [platform, data] of Object.entries(market.platforms)) {
      const impliedYes = data.yes;
      const spread = Math.abs(data.yes + data.no - 1);
      console.log(`   ${platform.padEnd(12)} | $${data.yes.toFixed(2).padStart(8)} | $${data.no.toFixed(2).padStart(8)} | ${(impliedYes * 100).toFixed(1).padStart(8)}% | ${(spread * 100).toFixed(1)}% | $${(data.volume / 1e6).toFixed(2)}M`);
    }
    console.log();

    // Calculate EV opportunities
    console.log('   Expected Value Analysis:');
    console.log('   Position    | Platform     | Price  | Model P | EV       | Action');
    console.log('   ' + '-'.repeat(60));

    for (const [platform, data] of Object.entries(market.platforms)) {
      const fee = marketConfig.fees[platform];

      // YES position
      const yesEV = calculateEV(market.modelProbability, data.yes, fee);
      const yesAction = yesEV > marketConfig.minEdge ? '✅ BUY YES' : 'PASS';

      console.log(`   ${'YES'.padEnd(11)} | ${platform.padEnd(12)} | $${data.yes.toFixed(2).padStart(5)} | ${(market.modelProbability * 100).toFixed(0).padStart(6)}% | ${formatEV(yesEV).padStart(8)} | ${yesAction}`);

      // NO position
      const noEV = calculateEV(1 - market.modelProbability, data.no, fee);
      const noAction = noEV > marketConfig.minEdge ? '✅ BUY NO' : 'PASS';

      console.log(`   ${'NO'.padEnd(11)} | ${platform.padEnd(12)} | $${data.no.toFixed(2).padStart(5)} | ${((1 - market.modelProbability) * 100).toFixed(0).padStart(6)}% | ${formatEV(noEV).padStart(8)} | ${noAction}`);

      // Track opportunities
      if (yesEV > marketConfig.minEdge) {
        opportunities.push({
          market: market.question,
          platform,
          position: 'YES',
          price: data.yes,
          ev: yesEV,
          modelProb: market.modelProbability
        });
      }
      if (noEV > marketConfig.minEdge) {
        opportunities.push({
          market: market.question,
          platform,
          position: 'NO',
          price: data.no,
          ev: noEV,
          modelProb: 1 - market.modelProbability
        });
      }
    }
    console.log();

    // Cross-platform arbitrage check
    if (Object.keys(market.platforms).length > 1) {
      const arbResult = checkCrossArbitrage(market);
      if (arbResult.hasArbitrage) {
        console.log(`   🎯 ARBITRAGE: Buy YES on ${arbResult.yesPlatform} ($${arbResult.yesPrice.toFixed(2)})`);
        console.log(`                Buy NO on ${arbResult.noPlatform} ($${arbResult.noPrice.toFixed(2)})`);
        console.log(`                Guaranteed profit: ${(arbResult.profit * 100).toFixed(2)}%`);
      }
    }
  }

  // 4. Portfolio recommendations
  console.log();
  console.log('4. Portfolio Recommendations:');
  console.log('='.repeat(70));

  if (opportunities.length === 0) {
    console.log('   No positions currently meet the minimum edge criteria.');
  } else {
    // Sort by EV
    opportunities.sort((a, b) => b.ev - a.ev);

    console.log('   Rank | Market                                 | Position | Platform     | EV      | Size');
    console.log('-'.repeat(70));

    let totalAllocation = 0;
    opportunities.slice(0, 5).forEach((opp, i) => {
      const kelly = calculateKelly(opp.modelProb, opp.price);
      const size = Math.min(kelly * 0.25, marketConfig.maxPositionSize) * marketConfig.initialCapital;
      totalAllocation += size;

      console.log(`   ${(i + 1).toString().padStart(4)} | ${opp.market.substring(0, 38).padEnd(38)} | ${opp.position.padEnd(8)} | ${opp.platform.padEnd(12)} | ${formatEV(opp.ev).padStart(7)} | $${size.toFixed(0)}`);
    });

    console.log('-'.repeat(70));
    console.log(`   Total Allocation: $${totalAllocation.toFixed(0)} (${(totalAllocation / marketConfig.initialCapital * 100).toFixed(1)}% of capital)`);
  }
  console.log();

  // 5. Market making opportunities
  console.log('5. Market Making Opportunities:');
  console.log('-'.repeat(70));

  console.log('   Markets with high spread (>5%):');
  predictionMarkets.forEach(market => {
    for (const [platform, data] of Object.entries(market.platforms)) {
      const spread = Math.abs(data.yes + data.no - 1);
      if (spread > 0.05) {
        console.log(`   - ${market.question.substring(0, 45)} (${platform}): ${(spread * 100).toFixed(1)}% spread`);
      }
    }
  });
  console.log();

  // 6. Risk analysis
  console.log('6. Risk Analysis:');
  console.log('-'.repeat(70));

  const categoryExposure = {};
  opportunities.forEach(opp => {
    const market = predictionMarkets.find(m => m.question === opp.market);
    if (market) {
      categoryExposure[market.category] = (categoryExposure[market.category] || 0) + 1;
    }
  });

  console.log('   Category concentration:');
  Object.entries(categoryExposure).forEach(([cat, count]) => {
    console.log(`   - ${cat}: ${count} positions`);
  });
  console.log();

  console.log('   Correlation warnings:');
  console.log('   - BTC $100K and S&P 6000 may be correlated (risk-on assets)');
  console.log('   - Consider hedging or reducing combined exposure');
  console.log();

  console.log('='.repeat(70));
  console.log('Prediction markets analysis completed!');
  console.log('='.repeat(70));
}

// Calculate Expected Value
function calculateEV(trueProb, price, fee) {
  const netProfit = (1 - fee) / price - 1;
  const ev = trueProb * netProfit - (1 - trueProb);
  return ev;
}

// Format EV for display
function formatEV(ev) {
  const pct = ev * 100;
  return pct >= 0 ? `+${pct.toFixed(1)}%` : `${pct.toFixed(1)}%`;
}

// Calculate Kelly Criterion for prediction markets
function calculateKelly(prob, price) {
  const b = 1 / price - 1; // Potential profit per dollar
  const p = prob;
  const q = 1 - prob;

  const kelly = (b * p - q) / b;
  return Math.max(0, kelly);
}

// Check for cross-platform arbitrage
function checkCrossArbitrage(market) {
  const platforms = Object.entries(market.platforms);
  if (platforms.length < 2) return { hasArbitrage: false };

  let bestYes = { price: 1, platform: '' };
  let bestNo = { price: 1, platform: '' };

  platforms.forEach(([platform, data]) => {
    if (data.yes < bestYes.price) {
      bestYes = { price: data.yes, platform };
    }
    if (data.no < bestNo.price) {
      bestNo = { price: data.no, platform };
    }
  });

  const totalCost = bestYes.price + bestNo.price;
  if (totalCost < 1) {
    return {
      hasArbitrage: true,
      yesPlatform: bestYes.platform,
      yesPrice: bestYes.price,
      noPlatform: bestNo.platform,
      noPrice: bestNo.price,
      profit: 1 / totalCost - 1
    };
  }

  return { hasArbitrage: false };
}

// Run the example
main().catch(console.error);
