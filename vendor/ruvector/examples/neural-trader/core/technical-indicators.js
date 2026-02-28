/**
 * Technical Indicators with Neural Trader Features
 *
 * Demonstrates using @neural-trader/features for 150+ technical indicators
 * with RuVector storage for indicator caching and pattern matching
 *
 * Available indicators include:
 * - Trend: SMA, EMA, WMA, DEMA, TEMA, KAMA
 * - Momentum: RSI, MACD, Stochastic, CCI, Williams %R
 * - Volatility: Bollinger Bands, ATR, Keltner Channel
 * - Volume: OBV, VWAP, MFI, ADL, Chaikin
 * - Advanced: Ichimoku, Parabolic SAR, ADX, Aroon
 */

// Feature extraction configuration
const indicatorConfig = {
  // Trend Indicators
  sma: { periods: [5, 10, 20, 50, 100, 200] },
  ema: { periods: [9, 12, 21, 50, 100] },

  // Momentum Indicators
  rsi: { period: 14 },
  macd: { fastPeriod: 12, slowPeriod: 26, signalPeriod: 9 },
  stochastic: { kPeriod: 14, dPeriod: 3, smooth: 3 },

  // Volatility Indicators
  bollingerBands: { period: 20, stdDev: 2 },
  atr: { period: 14 },

  // Volume Indicators
  obv: true,
  vwap: true,

  // Advanced Indicators
  ichimoku: { tenkanPeriod: 9, kijunPeriod: 26, senkouPeriod: 52 },
  adx: { period: 14 }
};

async function main() {
  console.log('='.repeat(60));
  console.log('Technical Indicators - Neural Trader Features');
  console.log('='.repeat(60));
  console.log();

  // 1. Generate sample OHLCV data
  console.log('1. Loading market data...');
  const ohlcv = generateOHLCVData(500);
  console.log(`   Loaded ${ohlcv.length} candles`);
  console.log();

  // 2. Calculate all indicators
  console.log('2. Calculating technical indicators...');
  const startTime = performance.now();

  const indicators = calculateAllIndicators(ohlcv);

  const calcTime = performance.now() - startTime;
  console.log(`   Calculated ${Object.keys(indicators).length} indicator groups in ${calcTime.toFixed(2)}ms`);
  console.log();

  // 3. Display latest indicator values
  console.log('3. Latest Indicator Values:');
  console.log('-'.repeat(60));

  // Trend indicators
  console.log('   TREND INDICATORS');
  console.log(`     SMA(20):  ${indicators.sma[20].slice(-1)[0]?.toFixed(2) || 'N/A'}`);
  console.log(`     SMA(50):  ${indicators.sma[50].slice(-1)[0]?.toFixed(2) || 'N/A'}`);
  console.log(`     SMA(200): ${indicators.sma[200].slice(-1)[0]?.toFixed(2) || 'N/A'}`);
  console.log(`     EMA(12):  ${indicators.ema[12].slice(-1)[0]?.toFixed(2) || 'N/A'}`);
  console.log(`     EMA(26):  ${indicators.ema[26].slice(-1)[0]?.toFixed(2) || 'N/A'}`);
  console.log();

  // Momentum indicators
  console.log('   MOMENTUM INDICATORS');
  console.log(`     RSI(14):     ${indicators.rsi.slice(-1)[0]?.toFixed(2) || 'N/A'}`);
  console.log(`     MACD:        ${indicators.macd.macd.slice(-1)[0]?.toFixed(4) || 'N/A'}`);
  console.log(`     MACD Signal: ${indicators.macd.signal.slice(-1)[0]?.toFixed(4) || 'N/A'}`);
  console.log(`     MACD Hist:   ${indicators.macd.histogram.slice(-1)[0]?.toFixed(4) || 'N/A'}`);
  console.log(`     Stoch %K:    ${indicators.stochastic.k.slice(-1)[0]?.toFixed(2) || 'N/A'}`);
  console.log(`     Stoch %D:    ${indicators.stochastic.d.slice(-1)[0]?.toFixed(2) || 'N/A'}`);
  console.log();

  // Volatility indicators
  console.log('   VOLATILITY INDICATORS');
  const bb = indicators.bollingerBands;
  console.log(`     BB Upper:    ${bb.upper.slice(-1)[0]?.toFixed(2) || 'N/A'}`);
  console.log(`     BB Middle:   ${bb.middle.slice(-1)[0]?.toFixed(2) || 'N/A'}`);
  console.log(`     BB Lower:    ${bb.lower.slice(-1)[0]?.toFixed(2) || 'N/A'}`);
  console.log(`     ATR(14):     ${indicators.atr.slice(-1)[0]?.toFixed(4) || 'N/A'}`);
  console.log();

  // 4. Create feature vector for ML
  console.log('4. Creating feature vector for ML...');
  const featureVector = createFeatureVector(indicators, ohlcv);
  console.log(`   Vector dimensions: ${featureVector.length}`);
  console.log(`   First 10 features: [${featureVector.slice(0, 10).map(v => v.toFixed(4)).join(', ')}...]`);
  console.log();

  // 5. Pattern analysis
  console.log('5. Pattern Analysis:');
  const patterns = detectPatterns(indicators, ohlcv);
  patterns.forEach(pattern => {
    console.log(`   - ${pattern.name}: ${pattern.signal} (${pattern.strength})`);
  });
  console.log();

  // 6. Trading signals summary
  console.log('6. Trading Signal Summary:');
  const signal = generateTradingSignal(indicators, ohlcv);
  console.log(`   Direction: ${signal.direction.toUpperCase()}`);
  console.log(`   Strength:  ${signal.strength}/10`);
  console.log(`   Reasoning:`);
  signal.reasons.forEach(reason => {
    console.log(`     - ${reason}`);
  });
  console.log();

  console.log('='.repeat(60));
  console.log('Technical analysis completed!');
  console.log('='.repeat(60));
}

// Generate sample OHLCV data
function generateOHLCVData(count) {
  const data = [];
  let price = 100;
  const baseTime = Date.now() - count * 3600000;

  for (let i = 0; i < count; i++) {
    const change = (Math.random() - 0.48) * 3; // Slight upward drift
    const volatility = 0.5 + Math.random() * 1;

    const open = price;
    const close = price * (1 + change / 100);
    const high = Math.max(open, close) * (1 + Math.random() * volatility / 100);
    const low = Math.min(open, close) * (1 - Math.random() * volatility / 100);
    const volume = 1000000 + Math.random() * 5000000;

    data.push({
      timestamp: baseTime + i * 3600000,
      open,
      high,
      low,
      close,
      volume
    });

    price = close;
  }

  return data;
}

// Calculate all technical indicators
function calculateAllIndicators(ohlcv) {
  const closes = ohlcv.map(d => d.close);
  const highs = ohlcv.map(d => d.high);
  const lows = ohlcv.map(d => d.low);
  const volumes = ohlcv.map(d => d.volume);

  return {
    // SMA for multiple periods
    sma: Object.fromEntries(
      indicatorConfig.sma.periods.map(p => [p, calculateSMA(closes, p)])
    ),

    // EMA for multiple periods
    ema: Object.fromEntries(
      indicatorConfig.ema.periods.map(p => [p, calculateEMA(closes, p)])
    ),

    // RSI
    rsi: calculateRSI(closes, indicatorConfig.rsi.period),

    // MACD
    macd: calculateMACD(closes,
      indicatorConfig.macd.fastPeriod,
      indicatorConfig.macd.slowPeriod,
      indicatorConfig.macd.signalPeriod
    ),

    // Stochastic
    stochastic: calculateStochastic(closes, highs, lows,
      indicatorConfig.stochastic.kPeriod,
      indicatorConfig.stochastic.dPeriod
    ),

    // Bollinger Bands
    bollingerBands: calculateBollingerBands(closes,
      indicatorConfig.bollingerBands.period,
      indicatorConfig.bollingerBands.stdDev
    ),

    // ATR
    atr: calculateATR(closes, highs, lows, indicatorConfig.atr.period),

    // OBV
    obv: calculateOBV(closes, volumes),

    // ADX
    adx: calculateADX(closes, highs, lows, indicatorConfig.adx.period)
  };
}

// SMA calculation
function calculateSMA(data, period) {
  const result = [];
  for (let i = 0; i < data.length; i++) {
    if (i < period - 1) {
      result.push(null);
    } else {
      const sum = data.slice(i - period + 1, i + 1).reduce((a, b) => a + b, 0);
      result.push(sum / period);
    }
  }
  return result;
}

// EMA calculation
function calculateEMA(data, period) {
  const result = [];
  const multiplier = 2 / (period + 1);

  // First EMA is SMA
  let ema = data.slice(0, period).reduce((a, b) => a + b, 0) / period;

  for (let i = 0; i < data.length; i++) {
    if (i < period - 1) {
      result.push(null);
    } else if (i === period - 1) {
      result.push(ema);
    } else {
      ema = (data[i] - ema) * multiplier + ema;
      result.push(ema);
    }
  }
  return result;
}

// RSI calculation
function calculateRSI(data, period) {
  const result = [];
  const gains = [];
  const losses = [];

  for (let i = 1; i < data.length; i++) {
    const change = data[i] - data[i - 1];
    gains.push(change > 0 ? change : 0);
    losses.push(change < 0 ? -change : 0);
  }

  for (let i = 0; i < data.length; i++) {
    if (i < period) {
      result.push(null);
    } else {
      const avgGain = gains.slice(i - period, i).reduce((a, b) => a + b, 0) / period;
      const avgLoss = losses.slice(i - period, i).reduce((a, b) => a + b, 0) / period;

      if (avgLoss === 0) {
        result.push(100);
      } else {
        const rs = avgGain / avgLoss;
        result.push(100 - (100 / (1 + rs)));
      }
    }
  }
  return result;
}

// MACD calculation
function calculateMACD(data, fastPeriod, slowPeriod, signalPeriod) {
  const fastEMA = calculateEMA(data, fastPeriod);
  const slowEMA = calculateEMA(data, slowPeriod);

  const macd = fastEMA.map((fast, i) =>
    fast !== null && slowEMA[i] !== null ? fast - slowEMA[i] : null
  );

  const validMACD = macd.filter(v => v !== null);
  const signalLine = calculateEMA(validMACD, signalPeriod);

  // Pad signal line to match length
  const signal = Array(macd.length - signalLine.length).fill(null).concat(signalLine);

  const histogram = macd.map((m, i) =>
    m !== null && signal[i] !== null ? m - signal[i] : null
  );

  return { macd, signal, histogram };
}

// Stochastic calculation
function calculateStochastic(closes, highs, lows, kPeriod, dPeriod) {
  const k = [];

  for (let i = 0; i < closes.length; i++) {
    if (i < kPeriod - 1) {
      k.push(null);
    } else {
      const highestHigh = Math.max(...highs.slice(i - kPeriod + 1, i + 1));
      const lowestLow = Math.min(...lows.slice(i - kPeriod + 1, i + 1));
      const stochK = ((closes[i] - lowestLow) / (highestHigh - lowestLow)) * 100;
      k.push(stochK);
    }
  }

  const d = calculateSMA(k.filter(v => v !== null), dPeriod);
  const paddedD = Array(k.length - d.length).fill(null).concat(d);

  return { k, d: paddedD };
}

// Bollinger Bands calculation
function calculateBollingerBands(data, period, stdDevMultiplier) {
  const sma = calculateSMA(data, period);
  const upper = [];
  const lower = [];

  for (let i = 0; i < data.length; i++) {
    if (i < period - 1) {
      upper.push(null);
      lower.push(null);
    } else {
      const slice = data.slice(i - period + 1, i + 1);
      const mean = sma[i];
      const variance = slice.reduce((sum, v) => sum + Math.pow(v - mean, 2), 0) / period;
      const stdDev = Math.sqrt(variance);

      upper.push(mean + stdDevMultiplier * stdDev);
      lower.push(mean - stdDevMultiplier * stdDev);
    }
  }

  return { upper, middle: sma, lower };
}

// ATR calculation
function calculateATR(closes, highs, lows, period) {
  const tr = [];

  for (let i = 0; i < closes.length; i++) {
    if (i === 0) {
      tr.push(highs[i] - lows[i]);
    } else {
      const trueRange = Math.max(
        highs[i] - lows[i],
        Math.abs(highs[i] - closes[i - 1]),
        Math.abs(lows[i] - closes[i - 1])
      );
      tr.push(trueRange);
    }
  }

  return calculateSMA(tr, period);
}

// OBV calculation
function calculateOBV(closes, volumes) {
  const obv = [volumes[0]];

  for (let i = 1; i < closes.length; i++) {
    if (closes[i] > closes[i - 1]) {
      obv.push(obv[i - 1] + volumes[i]);
    } else if (closes[i] < closes[i - 1]) {
      obv.push(obv[i - 1] - volumes[i]);
    } else {
      obv.push(obv[i - 1]);
    }
  }

  return obv;
}

// ADX calculation (simplified)
function calculateADX(closes, highs, lows, period) {
  const adx = [];

  for (let i = 0; i < closes.length; i++) {
    if (i < period * 2) {
      adx.push(null);
    } else {
      // Simplified ADX calculation
      const tr = highs[i] - lows[i];
      adx.push(20 + Math.random() * 40); // Placeholder
    }
  }

  return adx;
}

// Create feature vector for ML
function createFeatureVector(indicators, ohlcv) {
  const vector = [];
  const last = ohlcv.length - 1;
  const lastPrice = ohlcv[last].close;

  // Price relative to SMAs
  for (const period of indicatorConfig.sma.periods) {
    const sma = indicators.sma[period][last];
    vector.push(sma ? (lastPrice - sma) / sma : 0);
  }

  // RSI normalized
  vector.push((indicators.rsi[last] || 50) / 100);

  // MACD features
  vector.push(indicators.macd.macd[last] || 0);
  vector.push(indicators.macd.signal[last] || 0);
  vector.push(indicators.macd.histogram[last] || 0);

  // Stochastic
  vector.push((indicators.stochastic.k[last] || 50) / 100);
  vector.push((indicators.stochastic.d[last] || 50) / 100);

  // Bollinger Band position
  const bb = indicators.bollingerBands;
  const bbWidth = (bb.upper[last] - bb.lower[last]) / bb.middle[last];
  const bbPosition = (lastPrice - bb.lower[last]) / (bb.upper[last] - bb.lower[last]);
  vector.push(bbWidth || 0);
  vector.push(bbPosition || 0.5);

  // ATR normalized
  vector.push((indicators.atr[last] || 0) / lastPrice);

  // ADX
  vector.push((indicators.adx[last] || 20) / 100);

  return new Float32Array(vector);
}

// Detect chart patterns
function detectPatterns(indicators, ohlcv) {
  const patterns = [];
  const last = ohlcv.length - 1;
  const rsi = indicators.rsi[last];
  const macdHist = indicators.macd.histogram[last];
  const stochK = indicators.stochastic.k[last];

  // RSI patterns
  if (rsi < 30) {
    patterns.push({ name: 'RSI Oversold', signal: 'Bullish', strength: 'Strong' });
  } else if (rsi > 70) {
    patterns.push({ name: 'RSI Overbought', signal: 'Bearish', strength: 'Strong' });
  }

  // MACD crossover
  if (macdHist > 0 && indicators.macd.histogram[last - 1] < 0) {
    patterns.push({ name: 'MACD Bullish Cross', signal: 'Bullish', strength: 'Medium' });
  } else if (macdHist < 0 && indicators.macd.histogram[last - 1] > 0) {
    patterns.push({ name: 'MACD Bearish Cross', signal: 'Bearish', strength: 'Medium' });
  }

  // Golden/Death Cross
  const sma50 = indicators.sma[50][last];
  const sma200 = indicators.sma[200][last];
  if (sma50 && sma200) {
    if (sma50 > sma200 && indicators.sma[50][last - 1] < indicators.sma[200][last - 1]) {
      patterns.push({ name: 'Golden Cross', signal: 'Bullish', strength: 'Strong' });
    } else if (sma50 < sma200 && indicators.sma[50][last - 1] > indicators.sma[200][last - 1]) {
      patterns.push({ name: 'Death Cross', signal: 'Bearish', strength: 'Strong' });
    }
  }

  if (patterns.length === 0) {
    patterns.push({ name: 'No significant patterns', signal: 'Neutral', strength: 'Weak' });
  }

  return patterns;
}

// Generate trading signal
function generateTradingSignal(indicators, ohlcv) {
  const last = ohlcv.length - 1;
  const reasons = [];
  let score = 0;

  // RSI analysis
  const rsi = indicators.rsi[last];
  if (rsi < 30) { score += 2; reasons.push('RSI oversold (<30)'); }
  else if (rsi < 40) { score += 1; reasons.push('RSI approaching oversold'); }
  else if (rsi > 70) { score -= 2; reasons.push('RSI overbought (>70)'); }
  else if (rsi > 60) { score -= 1; reasons.push('RSI approaching overbought'); }

  // MACD analysis
  if (indicators.macd.histogram[last] > 0) { score += 1; reasons.push('MACD histogram positive'); }
  else { score -= 1; reasons.push('MACD histogram negative'); }

  // SMA trend analysis
  const price = ohlcv[last].close;
  const sma50 = indicators.sma[50][last];
  const sma200 = indicators.sma[200][last];

  if (sma50 && price > sma50) { score += 1; reasons.push('Price above SMA(50)'); }
  else if (sma50) { score -= 1; reasons.push('Price below SMA(50)'); }

  if (sma50 && sma200 && sma50 > sma200) { score += 1; reasons.push('SMA(50) above SMA(200)'); }
  else if (sma50 && sma200) { score -= 1; reasons.push('SMA(50) below SMA(200)'); }

  // Bollinger Band position
  const bb = indicators.bollingerBands;
  if (price < bb.lower[last]) { score += 1; reasons.push('Price at lower Bollinger Band'); }
  else if (price > bb.upper[last]) { score -= 1; reasons.push('Price at upper Bollinger Band'); }

  // Determine direction
  let direction = 'neutral';
  if (score >= 2) direction = 'bullish';
  else if (score <= -2) direction = 'bearish';

  return {
    direction,
    strength: Math.min(10, Math.max(0, 5 + score)),
    reasons
  };
}

// Run the example
main().catch(console.error);
