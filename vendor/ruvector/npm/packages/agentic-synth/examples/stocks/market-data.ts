/**
 * Stock Market Data Generation Examples
 *
 * Demonstrates realistic OHLCV data generation, technical indicators,
 * multi-timeframe data, market depth, and tick-by-tick simulation.
 */

import { AgenticSynth } from '../../../src';

// ============================================================================
// OHLCV Data Generation
// ============================================================================

interface OHLCVBar {
  timestamp: Date;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  symbol: string;
}

/**
 * Generate realistic OHLCV (candlestick) data with proper market microstructure
 */
async function generateOHLCVData() {
  const synth = new AgenticSynth();

  const ohlcvData = await synth.generate<OHLCVBar>({
    count: 390, // One trading day (6.5 hours * 60 minutes)
    template: {
      timestamp: '{{date.recent}}',
      open: '{{number.float(100, 200, 2)}}',
      high: '{{number.float(100, 200, 2)}}',
      low: '{{number.float(100, 200, 2)}}',
      close: '{{number.float(100, 200, 2)}}',
      volume: '{{number.int(100000, 10000000)}}',
      symbol: 'AAPL',
    },
    constraints: [
      // High must be >= max(open, close)
      'bar.high >= Math.max(bar.open, bar.close)',
      // Low must be <= min(open, close)
      'bar.low <= Math.min(bar.open, bar.close)',
      // Volume must be positive
      'bar.volume > 0',
    ],
    relationships: [
      {
        type: 'temporal',
        field: 'timestamp',
        interval: '1m', // 1-minute bars
      },
      {
        type: 'continuity',
        sourceField: 'close',
        targetField: 'open',
        description: 'Next bar opens at previous close',
      },
    ],
  });

  // Post-process to ensure OHLCV validity
  const validatedData = ohlcvData.map((bar, idx) => {
    if (idx > 0) {
      bar.open = ohlcvData[idx - 1].close; // Open = previous close
    }
    bar.high = Math.max(bar.open, bar.close, bar.high);
    bar.low = Math.min(bar.open, bar.close, bar.low);
    return bar;
  });

  console.log('Generated OHLCV Data (first 5 bars):');
  console.log(validatedData.slice(0, 5));

  return validatedData;
}

// ============================================================================
// Technical Indicators
// ============================================================================

interface TechnicalIndicators {
  timestamp: Date;
  price: number;
  sma_20: number;
  sma_50: number;
  rsi_14: number;
  macd: number;
  macd_signal: number;
  bb_upper: number;
  bb_middle: number;
  bb_lower: number;
  volume: number;
  symbol: string;
}

/**
 * Generate price data with technical indicators pre-calculated
 */
async function generateTechnicalIndicators() {
  const synth = new AgenticSynth();

  // First generate base price series
  const priceData = await synth.generate<{ price: number; volume: number }>({
    count: 100,
    template: {
      price: '{{number.float(150, 160, 2)}}',
      volume: '{{number.int(1000000, 5000000)}}',
    },
  });

  // Calculate indicators (simplified for demonstration)
  const calculateSMA = (data: number[], period: number): number[] => {
    const sma: number[] = [];
    for (let i = 0; i < data.length; i++) {
      if (i < period - 1) {
        sma.push(data[i]);
      } else {
        const sum = data.slice(i - period + 1, i + 1).reduce((a, b) => a + b, 0);
        sma.push(sum / period);
      }
    }
    return sma;
  };

  const calculateRSI = (data: number[], period: number = 14): number[] => {
    const rsi: number[] = [];
    for (let i = 0; i < data.length; i++) {
      if (i < period) {
        rsi.push(50); // Neutral RSI for initial period
      } else {
        const gains: number[] = [];
        const losses: number[] = [];
        for (let j = i - period + 1; j <= i; j++) {
          const change = data[j] - data[j - 1];
          if (change > 0) gains.push(change);
          else losses.push(Math.abs(change));
        }
        const avgGain = gains.reduce((a, b) => a + b, 0) / period;
        const avgLoss = losses.reduce((a, b) => a + b, 0) / period;
        const rs = avgLoss === 0 ? 100 : avgGain / avgLoss;
        rsi.push(100 - 100 / (1 + rs));
      }
    }
    return rsi;
  };

  const prices = priceData.map((d) => d.price);
  const sma20 = calculateSMA(prices, 20);
  const sma50 = calculateSMA(prices, 50);
  const rsi = calculateRSI(prices);

  const technicalData: TechnicalIndicators[] = priceData.map((bar, idx) => ({
    timestamp: new Date(Date.now() - (priceData.length - idx) * 60000),
    price: bar.price,
    sma_20: Number(sma20[idx].toFixed(2)),
    sma_50: Number(sma50[idx].toFixed(2)),
    rsi_14: Number(rsi[idx].toFixed(2)),
    macd: Number((sma20[idx] - sma50[idx]).toFixed(2)),
    macd_signal: Number((sma20[idx] - sma50[idx]) * 0.9).toFixed(2), // Simplified
    bb_upper: Number((sma20[idx] * 1.02).toFixed(2)),
    bb_middle: Number(sma20[idx].toFixed(2)),
    bb_lower: Number((sma20[idx] * 0.98).toFixed(2)),
    volume: bar.volume,
    symbol: 'AAPL',
  }));

  console.log('Technical Indicators (last 5 bars):');
  console.log(technicalData.slice(-5));

  return technicalData;
}

// ============================================================================
// Multi-Timeframe Data
// ============================================================================

interface MultiTimeframeData {
  '1m': OHLCVBar[];
  '5m': OHLCVBar[];
  '1h': OHLCVBar[];
  '1d': OHLCVBar[];
}

/**
 * Generate data across multiple timeframes with proper aggregation
 */
async function generateMultiTimeframeData(): Promise<MultiTimeframeData> {
  const synth = new AgenticSynth();

  // Generate 1-minute bars (base timeframe)
  const bars1m = await synth.generate<OHLCVBar>({
    count: 1560, // 4 trading days worth of 1-minute data
    template: {
      timestamp: '{{date.recent}}',
      open: '{{number.float(100, 200, 2)}}',
      high: '{{number.float(100, 200, 2)}}',
      low: '{{number.float(100, 200, 2)}}',
      close: '{{number.float(100, 200, 2)}}',
      volume: '{{number.int(10000, 100000)}}',
      symbol: 'AAPL',
    },
  });

  // Aggregate to 5-minute bars
  const bars5m: OHLCVBar[] = [];
  for (let i = 0; i < bars1m.length; i += 5) {
    const chunk = bars1m.slice(i, i + 5);
    if (chunk.length === 5) {
      bars5m.push({
        timestamp: chunk[0].timestamp,
        open: chunk[0].open,
        high: Math.max(...chunk.map((b) => b.high)),
        low: Math.min(...chunk.map((b) => b.low)),
        close: chunk[4].close,
        volume: chunk.reduce((sum, b) => sum + b.volume, 0),
        symbol: 'AAPL',
      });
    }
  }

  // Aggregate to 1-hour bars
  const bars1h: OHLCVBar[] = [];
  for (let i = 0; i < bars1m.length; i += 60) {
    const chunk = bars1m.slice(i, i + 60);
    if (chunk.length === 60) {
      bars1h.push({
        timestamp: chunk[0].timestamp,
        open: chunk[0].open,
        high: Math.max(...chunk.map((b) => b.high)),
        low: Math.min(...chunk.map((b) => b.low)),
        close: chunk[59].close,
        volume: chunk.reduce((sum, b) => sum + b.volume, 0),
        symbol: 'AAPL',
      });
    }
  }

  // Aggregate to 1-day bars
  const bars1d: OHLCVBar[] = [];
  for (let i = 0; i < bars1m.length; i += 390) {
    const chunk = bars1m.slice(i, i + 390);
    if (chunk.length === 390) {
      bars1d.push({
        timestamp: chunk[0].timestamp,
        open: chunk[0].open,
        high: Math.max(...chunk.map((b) => b.high)),
        low: Math.min(...chunk.map((b) => b.low)),
        close: chunk[389].close,
        volume: chunk.reduce((sum, b) => sum + b.volume, 0),
        symbol: 'AAPL',
      });
    }
  }

  console.log('Multi-timeframe data generated:');
  console.log(`1m bars: ${bars1m.length}`);
  console.log(`5m bars: ${bars5m.length}`);
  console.log(`1h bars: ${bars1h.length}`);
  console.log(`1d bars: ${bars1d.length}`);

  return {
    '1m': bars1m,
    '5m': bars5m,
    '1h': bars1h,
    '1d': bars1d,
  };
}

// ============================================================================
// Market Depth Data (Order Book)
// ============================================================================

interface OrderBookLevel {
  price: number;
  size: number;
  orders: number;
}

interface MarketDepth {
  timestamp: Date;
  symbol: string;
  bids: OrderBookLevel[];
  asks: OrderBookLevel[];
  spread: number;
  midPrice: number;
}

/**
 * Generate realistic Level 2 market depth data (order book)
 */
async function generateMarketDepth() {
  const synth = new AgenticSynth();

  const midPrice = 150.0;
  const tickSize = 0.01;
  const depth = 20; // 20 levels on each side

  const marketDepth = await synth.generate<MarketDepth>({
    count: 100,
    template: {
      timestamp: '{{date.recent}}',
      symbol: 'AAPL',
      bids: [],
      asks: [],
      spread: 0,
      midPrice: midPrice,
    },
  });

  // Generate order book levels
  const enrichedDepth = marketDepth.map((snapshot) => {
    const bids: OrderBookLevel[] = [];
    const asks: OrderBookLevel[] = [];

    // Generate bid side (below mid price)
    for (let i = 0; i < depth; i++) {
      bids.push({
        price: Number((midPrice - i * tickSize).toFixed(2)),
        size: Math.floor(Math.random() * 10000) + 100,
        orders: Math.floor(Math.random() * 50) + 1,
      });
    }

    // Generate ask side (above mid price)
    for (let i = 0; i < depth; i++) {
      asks.push({
        price: Number((midPrice + (i + 1) * tickSize).toFixed(2)),
        size: Math.floor(Math.random() * 10000) + 100,
        orders: Math.floor(Math.random() * 50) + 1,
      });
    }

    const bestBid = bids[0].price;
    const bestAsk = asks[0].price;

    return {
      ...snapshot,
      bids,
      asks,
      spread: Number((bestAsk - bestBid).toFixed(2)),
      midPrice: Number(((bestBid + bestAsk) / 2).toFixed(2)),
    };
  });

  console.log('Market Depth (first snapshot):');
  console.log({
    timestamp: enrichedDepth[0].timestamp,
    bestBid: enrichedDepth[0].bids[0],
    bestAsk: enrichedDepth[0].asks[0],
    spread: enrichedDepth[0].spread,
    totalBidVolume: enrichedDepth[0].bids.reduce((sum, b) => sum + b.size, 0),
    totalAskVolume: enrichedDepth[0].asks.reduce((sum, a) => sum + a.size, 0),
  });

  return enrichedDepth;
}

// ============================================================================
// Tick-by-Tick Data
// ============================================================================

interface Tick {
  timestamp: Date;
  symbol: string;
  price: number;
  size: number;
  side: 'buy' | 'sell';
  exchange: string;
  conditions: string[];
}

/**
 * Generate high-frequency tick-by-tick trade data
 */
async function generateTickData() {
  const synth = new AgenticSynth();

  const tickData = await synth.generate<Tick>({
    count: 10000, // 10k ticks (typical for a few minutes of active trading)
    template: {
      timestamp: '{{date.recent}}',
      symbol: 'AAPL',
      price: '{{number.float(149.5, 150.5, 2)}}',
      size: '{{number.int(1, 1000)}}',
      side: '{{random.arrayElement(["buy", "sell"])}}',
      exchange: '{{random.arrayElement(["NASDAQ", "NYSE", "BATS", "IEX"])}}',
      conditions: [],
    },
    constraints: [
      'tick.size > 0',
      'tick.price > 0',
    ],
    relationships: [
      {
        type: 'temporal',
        field: 'timestamp',
        interval: '10ms', // High-frequency ticks
      },
    ],
  });

  // Add trade conditions (regulatory tags)
  const enrichedTicks = tickData.map((tick) => {
    const conditions: string[] = [];

    if (tick.size >= 100) conditions.push('BLOCK');
    if (tick.size >= 10000) conditions.push('INSTITUTIONAL');
    if (Math.random() < 0.05) conditions.push('ODD_LOT');
    if (Math.random() < 0.1) conditions.push('EXTENDED_HOURS');

    return {
      ...tick,
      conditions,
    };
  });

  // Calculate tick statistics
  const buyTicks = enrichedTicks.filter((t) => t.side === 'buy');
  const sellTicks = enrichedTicks.filter((t) => t.side === 'sell');
  const avgBuyPrice =
    buyTicks.reduce((sum, t) => sum + t.price, 0) / buyTicks.length;
  const avgSellPrice =
    sellTicks.reduce((sum, t) => sum + t.price, 0) / sellTicks.length;

  console.log('Tick Data Statistics:');
  console.log({
    totalTicks: enrichedTicks.length,
    buyTicks: buyTicks.length,
    sellTicks: sellTicks.length,
    avgBuyPrice: avgBuyPrice.toFixed(2),
    avgSellPrice: avgSellPrice.toFixed(2),
    priceImbalance: (avgBuyPrice - avgSellPrice).toFixed(4),
    avgTradeSize:
      enrichedTicks.reduce((sum, t) => sum + t.size, 0) / enrichedTicks.length,
  });

  return enrichedTicks;
}

// ============================================================================
// Market Microstructure Patterns
// ============================================================================

interface MicrostructureMetrics {
  timestamp: Date;
  symbol: string;
  effectiveSpread: number;
  realizedSpread: number;
  priceImpact: number;
  toxicity: number; // Adverse selection measure
  orderImbalance: number;
  volatility: number;
}

/**
 * Generate market microstructure metrics for analysis
 */
async function generateMicrostructureMetrics() {
  const synth = new AgenticSynth();

  const metrics = await synth.generate<MicrostructureMetrics>({
    count: 390, // One trading day
    template: {
      timestamp: '{{date.recent}}',
      symbol: 'AAPL',
      effectiveSpread: '{{number.float(0.01, 0.05, 4)}}',
      realizedSpread: '{{number.float(0.005, 0.03, 4)}}',
      priceImpact: '{{number.float(0.001, 0.02, 4)}}',
      toxicity: '{{number.float(0, 1, 4)}}',
      orderImbalance: '{{number.float(-1, 1, 4)}}',
      volatility: '{{number.float(0.01, 0.05, 4)}}',
    },
    constraints: [
      'metrics.effectiveSpread >= metrics.realizedSpread',
      'metrics.toxicity >= 0 && metrics.toxicity <= 1',
      'metrics.orderImbalance >= -1 && metrics.orderImbalance <= 1',
    ],
  });

  console.log('Microstructure Metrics (sample):');
  console.log(metrics.slice(0, 5));

  return metrics;
}

// ============================================================================
// Main Execution
// ============================================================================

async function main() {
  console.log('='.repeat(80));
  console.log('Stock Market Data Generation Examples');
  console.log('='.repeat(80));
  console.log();

  try {
    console.log('1. Generating OHLCV Data...');
    await generateOHLCVData();
    console.log();

    console.log('2. Generating Technical Indicators...');
    await generateTechnicalIndicators();
    console.log();

    console.log('3. Generating Multi-Timeframe Data...');
    await generateMultiTimeframeData();
    console.log();

    console.log('4. Generating Market Depth...');
    await generateMarketDepth();
    console.log();

    console.log('5. Generating Tick Data...');
    await generateTickData();
    console.log();

    console.log('6. Generating Microstructure Metrics...');
    await generateMicrostructureMetrics();
    console.log();

    console.log('All examples completed successfully!');
  } catch (error) {
    console.error('Error generating market data:', error);
    process.exit(1);
  }
}

// Run if executed directly
if (require.main === module) {
  main();
}

export {
  generateOHLCVData,
  generateTechnicalIndicators,
  generateMultiTimeframeData,
  generateMarketDepth,
  generateTickData,
  generateMicrostructureMetrics,
};
