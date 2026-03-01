/**
 * Cryptocurrency Exchange Data Generation
 *
 * Examples for generating realistic crypto exchange data including:
 * - OHLCV (Open, High, Low, Close, Volume) data
 * - Order book snapshots and updates
 * - Trade execution data
 * - Liquidity pool metrics
 * - CEX (Centralized Exchange) and DEX (Decentralized Exchange) patterns
 */

import { createSynth } from '../../src/index.js';

/**
 * Example 1: Generate OHLCV data for multiple cryptocurrencies
 * Simulates 24/7 crypto market with realistic price movements
 */
async function generateOHLCV() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const cryptocurrencies = ['BTC', 'ETH', 'SOL', 'AVAX', 'MATIC'];
  const results = [];

  for (const symbol of cryptocurrencies) {
    const result = await synth.generateTimeSeries({
      count: 288, // 24 hours of 5-minute candles
      interval: '5m',
      startDate: new Date(Date.now() - 24 * 60 * 60 * 1000),
      metrics: ['open', 'high', 'low', 'close', 'volume', 'vwap'],
      trend: symbol === 'BTC' ? 'up' : symbol === 'SOL' ? 'volatile' : 'stable',
      seasonality: true, // Include daily trading patterns
      noise: 0.15, // 15% volatility
      schema: {
        timestamp: { type: 'datetime', format: 'iso8601' },
        symbol: { type: 'string', enum: [symbol] },
        open: { type: 'number', min: 0 },
        high: { type: 'number', min: 0 },
        low: { type: 'number', min: 0 },
        close: { type: 'number', min: 0 },
        volume: { type: 'number', min: 1000 },
        vwap: { type: 'number', min: 0 },
        trades: { type: 'integer', min: 1 }
      },
      constraints: {
        // Ensure high >= open, close, low
        // Ensure low <= open, close, high
        custom: [
          'high >= Math.max(open, close, low)',
          'low <= Math.min(open, close, high)',
          'vwap >= low && vwap <= high',
          'volume > 0'
        ]
      }
    });

    results.push({ symbol, data: result.data });
    console.log(`Generated ${symbol} OHLCV data: ${result.data.length} candles`);
  }

  return results;
}

/**
 * Example 2: Generate realistic order book data
 * Includes bid/ask spreads, market depth, and price levels
 */
async function generateOrderBook() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateStructured({
    count: 100, // 100 order book snapshots
    schema: {
      timestamp: { type: 'datetime', required: true },
      exchange: { type: 'string', enum: ['binance', 'coinbase', 'kraken', 'okx'] },
      symbol: { type: 'string', enum: ['BTC/USDT', 'ETH/USDT', 'SOL/USDT'] },
      bids: {
        type: 'array',
        required: true,
        minItems: 20,
        maxItems: 50,
        items: {
          type: 'object',
          properties: {
            price: { type: 'number', min: 0 },
            quantity: { type: 'number', min: 0.001 },
            total: { type: 'number' }
          }
        }
      },
      asks: {
        type: 'array',
        required: true,
        minItems: 20,
        maxItems: 50,
        items: {
          type: 'object',
          properties: {
            price: { type: 'number', min: 0 },
            quantity: { type: 'number', min: 0.001 },
            total: { type: 'number' }
          }
        }
      },
      spread: { type: 'number', min: 0 },
      spreadPercent: { type: 'number', min: 0, max: 5 },
      midPrice: { type: 'number', min: 0 },
      liquidity: {
        type: 'object',
        properties: {
          bidDepth: { type: 'number' },
          askDepth: { type: 'number' },
          totalDepth: { type: 'number' }
        }
      }
    },
    constraints: {
      custom: [
        'bids sorted by price descending',
        'asks sorted by price ascending',
        'spread = asks[0].price - bids[0].price',
        'midPrice = (bids[0].price + asks[0].price) / 2',
        'realistic market microstructure'
      ]
    }
  });

  console.log('Generated order book snapshots:', result.data.length);
  console.log('Sample order book:', JSON.stringify(result.data[0], null, 2));

  return result;
}

/**
 * Example 3: Generate trade execution data
 * Simulates actual trades with realistic patterns
 */
async function generateTrades() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateEvents({
    count: 10000, // 10k trades
    eventTypes: ['market_buy', 'market_sell', 'limit_buy', 'limit_sell'],
    distribution: 'poisson', // Realistic trade arrival times
    timeRange: {
      start: new Date(Date.now() - 24 * 60 * 60 * 1000),
      end: new Date()
    },
    userCount: 5000, // 5k unique traders
    schema: {
      tradeId: { type: 'string', format: 'uuid' },
      timestamp: { type: 'datetime', format: 'iso8601' },
      exchange: { type: 'string', enum: ['binance', 'coinbase', 'kraken'] },
      symbol: { type: 'string', enum: ['BTC/USDT', 'ETH/USDT', 'SOL/USDT', 'AVAX/USDT'] },
      side: { type: 'string', enum: ['buy', 'sell'] },
      orderType: { type: 'string', enum: ['market', 'limit', 'stop', 'stop_limit'] },
      price: { type: 'number', min: 0 },
      quantity: { type: 'number', min: 0.001 },
      total: { type: 'number' },
      fee: { type: 'number', min: 0 },
      feeAsset: { type: 'string', enum: ['USDT', 'BTC', 'ETH', 'BNB'] },
      userId: { type: 'string' },
      makerTaker: { type: 'string', enum: ['maker', 'taker'] },
      latency: { type: 'number', min: 1, max: 500 } // ms
    }
  });

  console.log('Generated trades:', result.data.length);
  console.log('Metadata:', result.metadata);

  // Analyze trade patterns
  const buyTrades = result.data.filter((t: any) => t.side === 'buy').length;
  const sellTrades = result.data.filter((t: any) => t.side === 'sell').length;
  console.log(`Buy/Sell ratio: ${buyTrades}/${sellTrades}`);

  return result;
}

/**
 * Example 4: Generate liquidity pool data (DEX)
 * Simulates AMM (Automated Market Maker) pools
 */
async function generateLiquidityPools() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateTimeSeries({
    count: 1440, // 24 hours, 1-minute intervals
    interval: '1m',
    startDate: new Date(Date.now() - 24 * 60 * 60 * 1000),
    metrics: ['reserveA', 'reserveB', 'totalLiquidity', 'volume24h', 'fees24h', 'apy'],
    trend: 'stable',
    seasonality: true,
    noise: 0.08,
    schema: {
      timestamp: { type: 'datetime', format: 'iso8601' },
      dex: { type: 'string', enum: ['uniswap', 'sushiswap', 'pancakeswap', 'curve'] },
      poolAddress: { type: 'string', format: 'ethereum_address' },
      tokenA: { type: 'string', enum: ['WETH', 'USDC', 'DAI', 'WBTC'] },
      tokenB: { type: 'string', enum: ['USDC', 'USDT', 'DAI'] },
      reserveA: { type: 'number', min: 100000 },
      reserveB: { type: 'number', min: 100000 },
      totalLiquidity: { type: 'number', min: 200000 },
      price: { type: 'number', min: 0 },
      volume24h: { type: 'number', min: 0 },
      fees24h: { type: 'number', min: 0 },
      txCount: { type: 'integer', min: 0 },
      uniqueWallets: { type: 'integer', min: 0 },
      apy: { type: 'number', min: 0, max: 500 },
      impermanentLoss: { type: 'number', min: -50, max: 0 }
    },
    constraints: {
      custom: [
        'price = reserveB / reserveA',
        'totalLiquidity = reserveA + reserveB (in USD)',
        'fees24h = volume24h * 0.003', // 0.3% fee
        'apy based on fees and liquidity',
        'maintain constant product formula (k = reserveA * reserveB)'
      ]
    }
  });

  console.log('Generated liquidity pool data:', result.data.length);
  console.log('Sample pool state:', result.data[0]);

  return result;
}

/**
 * Example 5: Generate cross-exchange arbitrage opportunities
 * Simulates price differences across exchanges
 */
async function generateArbitrageOpportunities() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const exchanges = ['binance', 'coinbase', 'kraken', 'okx'];
  const symbols = ['BTC/USDT', 'ETH/USDT', 'SOL/USDT'];

  const result = await synth.generateStructured({
    count: 500, // 500 arbitrage opportunities over 24 hours
    schema: {
      timestamp: { type: 'datetime', required: true },
      symbol: { type: 'string', enum: symbols },
      buyExchange: { type: 'string', enum: exchanges },
      sellExchange: { type: 'string', enum: exchanges },
      buyPrice: { type: 'number', min: 0 },
      sellPrice: { type: 'number', min: 0 },
      spread: { type: 'number', min: 0.001, max: 5 }, // 0.1% to 5%
      spreadPercent: { type: 'number' },
      volume: { type: 'number', min: 0 },
      profitUSD: { type: 'number', min: 0 },
      profitPercent: { type: 'number', min: 0 },
      executionTime: { type: 'number', min: 100, max: 5000 }, // ms
      feasible: { type: 'boolean' },
      fees: {
        type: 'object',
        properties: {
          buyFee: { type: 'number' },
          sellFee: { type: 'number' },
          networkFee: { type: 'number' },
          totalFee: { type: 'number' }
        }
      },
      netProfit: { type: 'number' }
    },
    constraints: {
      custom: [
        'buyExchange !== sellExchange',
        'sellPrice > buyPrice',
        'spreadPercent = (sellPrice - buyPrice) / buyPrice * 100',
        'profitUSD = volume * spread - fees.totalFee',
        'netProfit = profitUSD - fees.totalFee',
        'feasible = netProfit > 0 && executionTime < 3000'
      ]
    }
  });

  console.log('Generated arbitrage opportunities:', result.data.length);

  const feasibleOpps = result.data.filter((opp: any) => opp.feasible);
  console.log('Feasible opportunities:', feasibleOpps.length);
  console.log('Average profit:',
    feasibleOpps.reduce((sum: number, opp: any) => sum + opp.netProfit, 0) / feasibleOpps.length
  );

  return result;
}

/**
 * Example 6: Generate 24/7 market data with realistic patterns
 * Includes timezone effects and global trading sessions
 */
async function generate24x7MarketData() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateTimeSeries({
    count: 168 * 12, // 1 week, 5-minute intervals
    interval: '5m',
    startDate: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000),
    metrics: ['price', 'volume', 'volatility', 'momentum'],
    trend: 'up',
    seasonality: true,
    noise: 0.12,
    schema: {
      timestamp: { type: 'datetime', format: 'iso8601' },
      symbol: { type: 'string', default: 'BTC/USDT' },
      price: { type: 'number', min: 0 },
      volume: { type: 'number', min: 0 },
      volatility: { type: 'number', min: 0, max: 100 },
      momentum: { type: 'number', min: -100, max: 100 },
      tradingSession: {
        type: 'string',
        enum: ['asian', 'european', 'american', 'overlap']
      },
      marketCap: { type: 'number' },
      dominance: { type: 'number', min: 0, max: 100 },
      fearGreedIndex: { type: 'integer', min: 0, max: 100 }
    },
    constraints: {
      custom: [
        'Higher volume during US and European hours',
        'Increased volatility during Asian session opens',
        'Weekend volumes typically 30% lower',
        'Fear & Greed index correlates with momentum',
        'Price movements respect support/resistance levels'
      ]
    }
  });

  console.log('Generated 24/7 market data:', result.data.length);
  console.log('Time range:', {
    start: result.data[0].timestamp,
    end: result.data[result.data.length - 1].timestamp
  });

  return result;
}

/**
 * Example 7: Generate funding rate data (perpetual futures)
 * Important for derivatives trading
 */
async function generateFundingRates() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateTimeSeries({
    count: 720, // 30 days, 8-hour funding periods
    interval: '8h',
    startDate: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000),
    metrics: ['fundingRate', 'predictedRate', 'openInterest', 'markPrice', 'indexPrice'],
    trend: 'stable',
    seasonality: false,
    noise: 0.05,
    schema: {
      timestamp: { type: 'datetime', format: 'iso8601' },
      exchange: { type: 'string', enum: ['binance', 'bybit', 'okx', 'deribit'] },
      symbol: { type: 'string', enum: ['BTC-PERP', 'ETH-PERP', 'SOL-PERP'] },
      fundingRate: { type: 'number', min: -0.05, max: 0.05 }, // -5% to 5%
      predictedRate: { type: 'number', min: -0.05, max: 0.05 },
      openInterest: { type: 'number', min: 1000000 },
      markPrice: { type: 'number', min: 0 },
      indexPrice: { type: 'number', min: 0 },
      premium: { type: 'number' },
      longShortRatio: { type: 'number', min: 0.5, max: 2 }
    },
    constraints: {
      custom: [
        'premium = markPrice - indexPrice',
        'fundingRate based on premium and time',
        'positive rate means longs pay shorts',
        'negative rate means shorts pay longs',
        'extreme rates indicate strong directional bias'
      ]
    }
  });

  console.log('Generated funding rate data:', result.data.length);
  console.log('Average funding rate:',
    result.data.reduce((sum: any, d: any) => sum + d.fundingRate, 0) / result.data.length
  );

  return result;
}

/**
 * Example 8: Generate streaming real-time market data
 * Simulates WebSocket-like continuous data feed
 */
async function streamMarketData() {
  const synth = createSynth({
    provider: 'gemini',
    streaming: true
  });

  console.log('Streaming real-time market data (30 updates)...');
  let count = 0;

  for await (const tick of synth.generateStream('timeseries', {
    count: 30,
    interval: '1s',
    metrics: ['price', 'volume'],
    schema: {
      timestamp: { type: 'datetime' },
      symbol: { type: 'string', default: 'BTC/USDT' },
      price: { type: 'number' },
      volume: { type: 'number' },
      lastUpdate: { type: 'number' }
    }
  })) {
    console.log(`[${++count}] ${tick.timestamp} - ${tick.symbol}: $${tick.price} (Vol: ${tick.volume})`);
  }
}

/**
 * Run all examples
 */
export async function runExchangeDataExamples() {
  console.log('=== Cryptocurrency Exchange Data Generation Examples ===\n');

  console.log('Example 1: OHLCV Data Generation');
  await generateOHLCV();
  console.log('\n---\n');

  console.log('Example 2: Order Book Generation');
  await generateOrderBook();
  console.log('\n---\n');

  console.log('Example 3: Trade Execution Data');
  await generateTrades();
  console.log('\n---\n');

  console.log('Example 4: Liquidity Pool Data (DEX)');
  await generateLiquidityPools();
  console.log('\n---\n');

  console.log('Example 5: Arbitrage Opportunities');
  await generateArbitrageOpportunities();
  console.log('\n---\n');

  console.log('Example 6: 24/7 Market Data');
  await generate24x7MarketData();
  console.log('\n---\n');

  console.log('Example 7: Funding Rate Data');
  await generateFundingRates();
  console.log('\n---\n');

  console.log('Example 8: Streaming Market Data');
  await streamMarketData();
}

// Export individual examples
export {
  generateOHLCV,
  generateOrderBook,
  generateTrades,
  generateLiquidityPools,
  generateArbitrageOpportunities,
  generate24x7MarketData,
  generateFundingRates,
  streamMarketData
};

// Uncomment to run
// runExchangeDataExamples().catch(console.error);
