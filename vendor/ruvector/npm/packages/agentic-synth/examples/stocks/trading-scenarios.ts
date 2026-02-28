/**
 * Trading Scenarios Generation
 *
 * Generate realistic market scenarios for testing trading systems:
 * - Bull/bear markets
 * - Volatility patterns
 * - Flash crashes
 * - Earnings announcements
 * - Market correlations
 */

import { AgenticSynth } from '../../../src';

// ============================================================================
// Market Regime Types
// ============================================================================

type MarketRegime = 'bull' | 'bear' | 'sideways' | 'volatile' | 'crisis';

interface MarketScenario {
  timestamp: Date;
  price: number;
  volume: number;
  regime: MarketRegime;
  volatility: number;
  trend: number; // -1 to 1
  momentum: number;
  symbol: string;
}

// ============================================================================
// Bull Market Scenario
// ============================================================================

/**
 * Generate sustained uptrend with occasional pullbacks
 */
async function generateBullMarket() {
  const synth = new AgenticSynth();

  const basePrice = 100;
  const days = 252; // One trading year
  const barsPerDay = 390; // 1-minute bars

  const bullMarket = await synth.generate<MarketScenario>({
    count: days * barsPerDay,
    template: {
      timestamp: '{{date.recent}}',
      price: 0,
      volume: '{{number.int(1000000, 5000000)}}',
      regime: 'bull',
      volatility: '{{number.float(0.01, 0.03, 4)}}',
      trend: '{{number.float(0.5, 1, 2)}}',
      momentum: '{{number.float(0.3, 0.9, 2)}}',
      symbol: 'BULL',
    },
  });

  // Generate price series with upward drift
  let currentPrice = basePrice;
  const enrichedData = bullMarket.map((bar, idx) => {
    // Daily drift: ~0.08% per bar (20% annual return)
    const drift = 0.0008;
    const volatility = bar.volatility;
    const random = (Math.random() - 0.5) * 2; // -1 to 1

    // Occasional pullbacks (10% chance)
    const pullback = Math.random() < 0.1 ? -0.002 : 0;

    currentPrice *= 1 + drift + volatility * random + pullback;

    // Increase volume on breakouts
    const volumeMultiplier = currentPrice > basePrice * 1.1 ? 1.5 : 1;

    return {
      ...bar,
      price: Number(currentPrice.toFixed(2)),
      volume: Math.floor(bar.volume * volumeMultiplier),
    };
  });

  console.log('Bull Market Scenario:');
  console.log({
    initialPrice: enrichedData[0].price,
    finalPrice: enrichedData[enrichedData.length - 1].price,
    totalReturn: (
      ((enrichedData[enrichedData.length - 1].price - enrichedData[0].price) /
        enrichedData[0].price) *
      100
    ).toFixed(2) + '%',
    avgVolatility:
      enrichedData.reduce((sum, b) => sum + b.volatility, 0) /
      enrichedData.length,
  });

  return enrichedData;
}

// ============================================================================
// Bear Market Scenario
// ============================================================================

/**
 * Generate sustained downtrend with sharp selloffs
 */
async function generateBearMarket() {
  const synth = new AgenticSynth();

  const basePrice = 100;
  const days = 126; // Six months of bear market

  const bearMarket = await synth.generate<MarketScenario>({
    count: days * 390,
    template: {
      timestamp: '{{date.recent}}',
      price: 0,
      volume: '{{number.int(2000000, 8000000)}}', // Higher volume in bear
      regime: 'bear',
      volatility: '{{number.float(0.02, 0.06, 4)}}', // Higher volatility
      trend: '{{number.float(-1, -0.4, 2)}}',
      momentum: '{{number.float(-0.9, -0.3, 2)}}',
      symbol: 'BEAR',
    },
  });

  let currentPrice = basePrice;
  const enrichedData = bearMarket.map((bar, idx) => {
    // Daily drift: -0.1% per bar (-25% over 6 months)
    const drift = -0.001;
    const volatility = bar.volatility;
    const random = (Math.random() - 0.5) * 2;

    // Sharp selloffs (5% chance of -2% move)
    const selloff = Math.random() < 0.05 ? -0.02 : 0;

    // Dead cat bounces (3% chance of +1.5% move)
    const bounce = Math.random() < 0.03 ? 0.015 : 0;

    currentPrice *= 1 + drift + volatility * random + selloff + bounce;

    // Volume spikes on panic selling
    const volumeMultiplier = selloff < 0 ? 2.5 : 1;

    return {
      ...bar,
      price: Number(currentPrice.toFixed(2)),
      volume: Math.floor(bar.volume * volumeMultiplier),
    };
  });

  console.log('Bear Market Scenario:');
  console.log({
    initialPrice: enrichedData[0].price,
    finalPrice: enrichedData[enrichedData.length - 1].price,
    totalReturn: (
      ((enrichedData[enrichedData.length - 1].price - enrichedData[0].price) /
        enrichedData[0].price) *
      100
    ).toFixed(2) + '%',
    avgVolatility:
      enrichedData.reduce((sum, b) => sum + b.volatility, 0) /
      enrichedData.length,
  });

  return enrichedData;
}

// ============================================================================
// Volatility Patterns
// ============================================================================

interface VolatilityRegime {
  timestamp: Date;
  price: number;
  realizedVol: number;
  impliedVol: number;
  vix: number;
  regime: 'low' | 'medium' | 'high' | 'extreme';
  symbol: string;
}

/**
 * Generate varying volatility regimes
 */
async function generateVolatilityPatterns() {
  const synth = new AgenticSynth();

  const scenarios = [
    { regime: 'low', vixRange: [10, 15], volRange: [0.005, 0.015] },
    { regime: 'medium', vixRange: [15, 25], volRange: [0.015, 0.03] },
    { regime: 'high', vixRange: [25, 40], volRange: [0.03, 0.05] },
    { regime: 'extreme', vixRange: [40, 80], volRange: [0.05, 0.15] },
  ] as const;

  const allScenarios: VolatilityRegime[] = [];

  for (const scenario of scenarios) {
    const data = await synth.generate<VolatilityRegime>({
      count: 390, // One trading day
      template: {
        timestamp: '{{date.recent}}',
        price: '{{number.float(100, 200, 2)}}',
        realizedVol: `{{number.float(${scenario.volRange[0]}, ${scenario.volRange[1]}, 4)}}`,
        impliedVol: `{{number.float(${scenario.volRange[0] * 1.1}, ${scenario.volRange[1] * 1.2}, 4)}}`,
        vix: `{{number.float(${scenario.vixRange[0]}, ${scenario.vixRange[1]}, 2)}}`,
        regime: scenario.regime,
        symbol: 'SPY',
      },
      constraints: [
        'bar.impliedVol >= bar.realizedVol', // IV typically > RV
      ],
    });

    allScenarios.push(...data);
  }

  console.log('Volatility Patterns Generated:');
  scenarios.forEach((s) => {
    const filtered = allScenarios.filter((d) => d.regime === s.regime);
    console.log(`${s.regime}: ${filtered.length} bars, avg VIX: ${
      (filtered.reduce((sum, b) => sum + b.vix, 0) / filtered.length).toFixed(2)
    }`);
  });

  return allScenarios;
}

// ============================================================================
// Flash Crash Simulation
// ============================================================================

interface FlashCrashEvent {
  phase: 'normal' | 'crash' | 'recovery';
  timestamp: Date;
  price: number;
  volume: number;
  spread: number;
  liquidityScore: number;
  symbol: string;
}

/**
 * Simulate flash crash with rapid price decline and recovery
 */
async function generateFlashCrash() {
  const synth = new AgenticSynth();

  const basePrice = 150;
  const phases = [
    { phase: 'normal', duration: 100, priceChange: 0 },
    { phase: 'crash', duration: 20, priceChange: -0.15 }, // 15% drop
    { phase: 'recovery', duration: 50, priceChange: 0.12 }, // Recover 12%
  ] as const;

  const allData: FlashCrashEvent[] = [];
  let currentPrice = basePrice;

  for (const phase of phases) {
    const phaseData = await synth.generate<FlashCrashEvent>({
      count: phase.duration,
      template: {
        phase: phase.phase,
        timestamp: '{{date.recent}}',
        price: 0,
        volume: '{{number.int(1000000, 10000000)}}',
        spread: '{{number.float(0.01, 0.5, 4)}}',
        liquidityScore: '{{number.float(0, 1, 2)}}',
        symbol: 'FLASH',
      },
    });

    const pricePerBar = phase.priceChange / phase.duration;

    const enrichedPhase = phaseData.map((bar, idx) => {
      if (phase.phase === 'crash') {
        // Exponential decay during crash
        const crashIntensity = Math.pow(idx / phase.duration, 2);
        currentPrice *= 1 + pricePerBar * (1 + crashIntensity);

        return {
          ...bar,
          price: Number(currentPrice.toFixed(2)),
          volume: bar.volume * 5, // Massive volume spike
          spread: bar.spread * 10, // Wide spreads
          liquidityScore: 0.1, // Liquidity evaporates
        };
      } else if (phase.phase === 'recovery') {
        // Quick recovery
        currentPrice *= 1 + pricePerBar * 1.5;

        return {
          ...bar,
          price: Number(currentPrice.toFixed(2)),
          volume: bar.volume * 2,
          spread: bar.spread * 3,
          liquidityScore: 0.4,
        };
      } else {
        // Normal trading
        currentPrice *= 1 + (Math.random() - 0.5) * 0.0002;

        return {
          ...bar,
          price: Number(currentPrice.toFixed(2)),
          liquidityScore: 0.9,
        };
      }
    });

    allData.push(...enrichedPhase);
  }

  console.log('Flash Crash Simulation:');
  console.log({
    precrashPrice: allData[99].price,
    crashLowPrice: Math.min(...allData.slice(100, 120).map((d) => d.price)),
    postRecoveryPrice: allData[allData.length - 1].price,
    maxDrawdown: (
      ((Math.min(...allData.map((d) => d.price)) - allData[0].price) /
        allData[0].price) *
      100
    ).toFixed(2) + '%',
  });

  return allData;
}

// ============================================================================
// Earnings Announcement Impact
// ============================================================================

interface EarningsEvent {
  phase: 'pre-announcement' | 'announcement' | 'post-announcement';
  timestamp: Date;
  price: number;
  volume: number;
  impliedVolatility: number;
  optionVolume: number;
  surprise: 'beat' | 'miss' | 'inline';
  symbol: string;
}

/**
 * Simulate earnings announcement with volatility crush
 */
async function generateEarningsScenario() {
  const synth = new AgenticSynth();

  const surpriseType = ['beat', 'miss', 'inline'][Math.floor(Math.random() * 3)] as 'beat' | 'miss' | 'inline';

  const phases = [
    { phase: 'pre-announcement', duration: 200, ivLevel: 0.8 },
    { phase: 'announcement', duration: 10, ivLevel: 1.2 },
    { phase: 'post-announcement', duration: 180, ivLevel: 0.3 },
  ] as const;

  const allData: EarningsEvent[] = [];
  let basePrice = 100;

  // Determine price reaction based on surprise
  const priceReaction = {
    beat: 0.08, // 8% pop
    miss: -0.12, // 12% drop
    inline: 0.02, // 2% drift
  }[surpriseType];

  for (const phase of phases) {
    const phaseData = await synth.generate<EarningsEvent>({
      count: phase.duration,
      template: {
        phase: phase.phase,
        timestamp: '{{date.recent}}',
        price: 0,
        volume: '{{number.int(1000000, 5000000)}}',
        impliedVolatility: 0,
        optionVolume: '{{number.int(10000, 100000)}}',
        surprise: surpriseType,
        symbol: 'EARN',
      },
    });

    const enrichedPhase = phaseData.map((bar, idx) => {
      if (phase.phase === 'pre-announcement') {
        // Building anticipation
        basePrice *= 1 + (Math.random() - 0.5) * 0.001;

        return {
          ...bar,
          price: Number(basePrice.toFixed(2)),
          impliedVolatility: Number((phase.ivLevel * (0.3 + idx / phase.duration * 0.2)).toFixed(4)),
          optionVolume: bar.optionVolume * 2, // Heavy options activity
        };
      } else if (phase.phase === 'announcement') {
        // Immediate reaction
        if (idx === 0) {
          basePrice *= 1 + priceReaction;
        }

        return {
          ...bar,
          price: Number(basePrice.toFixed(2)),
          volume: bar.volume * 10, // Massive volume spike
          impliedVolatility: Number((phase.ivLevel * 0.5).toFixed(4)),
          optionVolume: bar.optionVolume * 5,
        };
      } else {
        // Volatility crush
        basePrice *= 1 + (Math.random() - 0.5) * 0.0005;

        return {
          ...bar,
          price: Number(basePrice.toFixed(2)),
          impliedVolatility: Number((phase.ivLevel * (1 - idx / phase.duration * 0.7)).toFixed(4)),
          volume: Math.floor(bar.volume * (2 - idx / phase.duration)),
        };
      }
    });

    allData.push(...enrichedPhase);
  }

  console.log('Earnings Announcement Scenario:');
  console.log({
    surprise: surpriseType,
    preEarningsPrice: allData[199].price,
    postEarningsPrice: allData[210].price,
    priceChange: (
      ((allData[210].price - allData[199].price) / allData[199].price) *
      100
    ).toFixed(2) + '%',
    preIV: allData[199].impliedVolatility,
    postIV: allData[allData.length - 1].impliedVolatility,
    ivCrush: (
      ((allData[allData.length - 1].impliedVolatility - allData[199].impliedVolatility) /
        allData[199].impliedVolatility) *
      100
    ).toFixed(2) + '%',
  });

  return allData;
}

// ============================================================================
// Market Correlation Data
// ============================================================================

interface CorrelationData {
  timestamp: Date;
  spy: number; // S&P 500
  qqq: number; // Nasdaq
  iwm: number; // Russell 2000
  vix: number; // Volatility index
  dxy: number; // Dollar index
  correlation_spy_qqq: number;
  correlation_spy_vix: number;
}

/**
 * Generate correlated multi-asset data
 */
async function generateCorrelatedMarkets() {
  const synth = new AgenticSynth();

  const count = 390;
  const baseData = await synth.generate<{ timestamp: Date }>({
    count,
    template: {
      timestamp: '{{date.recent}}',
    },
  });

  // Generate correlated returns
  const returns = Array.from({ length: count }, () => {
    const marketFactor = (Math.random() - 0.5) * 0.02; // Common market movement

    return {
      spy: marketFactor + (Math.random() - 0.5) * 0.005,
      qqq: marketFactor * 1.3 + (Math.random() - 0.5) * 0.008, // Higher beta
      iwm: marketFactor * 1.5 + (Math.random() - 0.5) * 0.01, // Even higher beta
      vix: -marketFactor * 3 + (Math.random() - 0.5) * 0.05, // Negative correlation
      dxy: -marketFactor * 0.5 + (Math.random() - 0.5) * 0.003, // Slight negative
    };
  });

  // Convert returns to prices
  let prices = { spy: 400, qqq: 350, iwm: 180, vix: 15, dxy: 100 };

  const correlationData: CorrelationData[] = baseData.map((bar, idx) => {
    prices.spy *= 1 + returns[idx].spy;
    prices.qqq *= 1 + returns[idx].qqq;
    prices.iwm *= 1 + returns[idx].iwm;
    prices.vix *= 1 + returns[idx].vix;
    prices.dxy *= 1 + returns[idx].dxy;

    // Calculate rolling correlation (simplified)
    const window = 20;
    const start = Math.max(0, idx - window);
    const spyReturns = returns.slice(start, idx + 1).map((r) => r.spy);
    const qqqReturns = returns.slice(start, idx + 1).map((r) => r.qqq);
    const vixReturns = returns.slice(start, idx + 1).map((r) => r.vix);

    const correlation = (arr1: number[], arr2: number[]): number => {
      const n = arr1.length;
      const mean1 = arr1.reduce((a, b) => a + b, 0) / n;
      const mean2 = arr2.reduce((a, b) => a + b, 0) / n;

      const numerator = arr1.reduce(
        (sum, val, i) => sum + (val - mean1) * (arr2[i] - mean2),
        0
      );
      const denom1 = Math.sqrt(
        arr1.reduce((sum, val) => sum + Math.pow(val - mean1, 2), 0)
      );
      const denom2 = Math.sqrt(
        arr2.reduce((sum, val) => sum + Math.pow(val - mean2, 2), 0)
      );

      return numerator / (denom1 * denom2);
    };

    return {
      timestamp: bar.timestamp,
      spy: Number(prices.spy.toFixed(2)),
      qqq: Number(prices.qqq.toFixed(2)),
      iwm: Number(prices.iwm.toFixed(2)),
      vix: Number(prices.vix.toFixed(2)),
      dxy: Number(prices.dxy.toFixed(2)),
      correlation_spy_qqq: Number(correlation(spyReturns, qqqReturns).toFixed(4)),
      correlation_spy_vix: Number(correlation(spyReturns, vixReturns).toFixed(4)),
    };
  });

  console.log('Market Correlation Data:');
  console.log({
    avgCorrelation_SPY_QQQ:
      correlationData.reduce((sum, d) => sum + d.correlation_spy_qqq, 0) /
      correlationData.length,
    avgCorrelation_SPY_VIX:
      correlationData.reduce((sum, d) => sum + d.correlation_spy_vix, 0) /
      correlationData.length,
  });

  return correlationData;
}

// ============================================================================
// Main Execution
// ============================================================================

async function main() {
  console.log('='.repeat(80));
  console.log('Trading Scenarios Generation');
  console.log('='.repeat(80));
  console.log();

  try {
    console.log('1. Generating Bull Market Scenario...');
    await generateBullMarket();
    console.log();

    console.log('2. Generating Bear Market Scenario...');
    await generateBearMarket();
    console.log();

    console.log('3. Generating Volatility Patterns...');
    await generateVolatilityPatterns();
    console.log();

    console.log('4. Generating Flash Crash...');
    await generateFlashCrash();
    console.log();

    console.log('5. Generating Earnings Scenario...');
    await generateEarningsScenario();
    console.log();

    console.log('6. Generating Correlated Markets...');
    await generateCorrelatedMarkets();
    console.log();

    console.log('All trading scenarios generated successfully!');
  } catch (error) {
    console.error('Error generating trading scenarios:', error);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

export {
  generateBullMarket,
  generateBearMarket,
  generateVolatilityPatterns,
  generateFlashCrash,
  generateEarningsScenario,
  generateCorrelatedMarkets,
};
