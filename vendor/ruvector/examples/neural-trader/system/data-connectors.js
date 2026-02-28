/**
 * Real Data Connectors
 *
 * APIs for market data from multiple sources:
 * - Yahoo Finance (free, delayed)
 * - Alpha Vantage (free tier available)
 * - Binance (crypto, real-time)
 * - Polygon.io (stocks, options)
 * - IEX Cloud (stocks)
 *
 * Features:
 * - Rate limiting
 * - Caching
 * - Error handling
 * - Data normalization
 */

// Connector Configuration
const connectorConfig = {
  // API Keys (set via environment or constructor)
  apiKeys: {
    alphaVantage: process.env.ALPHA_VANTAGE_KEY || '',
    polygon: process.env.POLYGON_KEY || '',
    iex: process.env.IEX_KEY || '',
    binance: process.env.BINANCE_KEY || ''
  },

  // Rate limits (requests per minute)
  rateLimits: {
    yahoo: 100,
    alphaVantage: 5,
    binance: 1200,
    polygon: 100,
    iex: 100
  },

  // Cache settings
  cache: {
    enabled: true,
    ttl: 60000,  // 1 minute default
    maxSize: 1000
  },

  // Retry settings
  retry: {
    maxRetries: 3,
    backoffMs: 1000
  }
};

/**
 * Simple LRU Cache
 */
class LRUCache {
  constructor(maxSize = 1000, ttl = 60000) {
    this.maxSize = maxSize;
    this.ttl = ttl;
    this.cache = new Map();
  }

  get(key) {
    const entry = this.cache.get(key);
    if (!entry) return null;
    if (Date.now() - entry.timestamp > this.ttl) {
      this.cache.delete(key);
      return null;
    }
    // Move to end (most recent)
    this.cache.delete(key);
    this.cache.set(key, entry);
    return entry.value;
  }

  set(key, value) {
    if (this.cache.size >= this.maxSize) {
      // Remove oldest entry
      const firstKey = this.cache.keys().next().value;
      this.cache.delete(firstKey);
    }
    this.cache.set(key, { value, timestamp: Date.now() });
  }

  clear() {
    this.cache.clear();
  }
}

/**
 * Rate Limiter
 */
class RateLimiter {
  constructor(requestsPerMinute) {
    this.requestsPerMinute = requestsPerMinute;
    this.requests = [];
  }

  async acquire() {
    const now = Date.now();
    // Remove requests older than 1 minute
    this.requests = this.requests.filter(t => now - t < 60000);

    if (this.requests.length >= this.requestsPerMinute) {
      const waitTime = 60000 - (now - this.requests[0]);
      await new Promise(resolve => setTimeout(resolve, waitTime));
      return this.acquire();
    }

    this.requests.push(now);
    return true;
  }
}

/**
 * Base Data Connector
 */
class BaseConnector {
  constructor(config = {}) {
    this.config = { ...connectorConfig, ...config };
    this.cache = new LRUCache(
      this.config.cache.maxSize,
      this.config.cache.ttl
    );
    this.rateLimiters = {};
  }

  getRateLimiter(source) {
    if (!this.rateLimiters[source]) {
      this.rateLimiters[source] = new RateLimiter(
        this.config.rateLimits[source] || 100
      );
    }
    return this.rateLimiters[source];
  }

  async fetchWithRetry(url, options = {}, source = 'default') {
    const cacheKey = `${source}:${url}`;

    // Check cache
    if (this.config.cache.enabled) {
      const cached = this.cache.get(cacheKey);
      if (cached) return cached;
    }

    // Rate limit
    await this.getRateLimiter(source).acquire();

    let lastError;
    for (let i = 0; i < this.config.retry.maxRetries; i++) {
      try {
        const response = await fetch(url, options);

        if (!response.ok) {
          throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }

        const data = await response.json();

        // Cache result
        if (this.config.cache.enabled) {
          this.cache.set(cacheKey, data);
        }

        return data;
      } catch (error) {
        lastError = error;
        await new Promise(r => setTimeout(r, this.config.retry.backoffMs * (i + 1)));
      }
    }

    throw lastError;
  }

  // Normalize OHLCV data to common format
  normalizeOHLCV(data, source) {
    return data.map(d => ({
      timestamp: new Date(d.timestamp || d.date || d.t).getTime(),
      open: parseFloat(d.open || d.o || d['1. open'] || 0),
      high: parseFloat(d.high || d.h || d['2. high'] || 0),
      low: parseFloat(d.low || d.l || d['3. low'] || 0),
      close: parseFloat(d.close || d.c || d['4. close'] || 0),
      volume: parseFloat(d.volume || d.v || d['5. volume'] || 0),
      source
    }));
  }
}

/**
 * Yahoo Finance Connector (via unofficial API)
 */
class YahooFinanceConnector extends BaseConnector {
  constructor(config = {}) {
    super(config);
    this.baseUrl = 'https://query1.finance.yahoo.com/v8/finance';
  }

  async getQuote(symbol) {
    const url = `${this.baseUrl}/chart/${symbol}?interval=1d&range=1d`;
    const data = await this.fetchWithRetry(url, {}, 'yahoo');

    if (!data.chart?.result?.[0]) {
      throw new Error(`No data for symbol: ${symbol}`);
    }

    const result = data.chart.result[0];
    const quote = result.indicators.quote[0];
    const meta = result.meta;

    return {
      symbol: meta.symbol,
      price: meta.regularMarketPrice,
      previousClose: meta.previousClose,
      change: meta.regularMarketPrice - meta.previousClose,
      changePercent: ((meta.regularMarketPrice - meta.previousClose) / meta.previousClose) * 100,
      volume: quote.volume?.[quote.volume.length - 1] || 0,
      timestamp: Date.now()
    };
  }

  async getHistorical(symbol, period = '1y', interval = '1d') {
    const url = `${this.baseUrl}/chart/${symbol}?interval=${interval}&range=${period}`;
    const data = await this.fetchWithRetry(url, {}, 'yahoo');

    if (!data.chart?.result?.[0]) {
      throw new Error(`No data for symbol: ${symbol}`);
    }

    const result = data.chart.result[0];
    const timestamps = result.timestamp;
    const quote = result.indicators.quote[0];

    const candles = [];
    for (let i = 0; i < timestamps.length; i++) {
      if (quote.open[i] !== null) {
        candles.push({
          timestamp: timestamps[i] * 1000,
          open: quote.open[i],
          high: quote.high[i],
          low: quote.low[i],
          close: quote.close[i],
          volume: quote.volume[i],
          source: 'yahoo'
        });
      }
    }

    return candles;
  }

  async search(query) {
    const url = `https://query2.finance.yahoo.com/v1/finance/search?q=${encodeURIComponent(query)}`;
    const data = await this.fetchWithRetry(url, {}, 'yahoo');
    return data.quotes?.map(q => ({
      symbol: q.symbol,
      name: q.shortname || q.longname,
      type: q.quoteType,
      exchange: q.exchange
    })) || [];
  }
}

/**
 * Alpha Vantage Connector
 */
class AlphaVantageConnector extends BaseConnector {
  constructor(config = {}) {
    super(config);
    this.baseUrl = 'https://www.alphavantage.co/query';
    this.apiKey = config.apiKey || this.config.apiKeys.alphaVantage;
  }

  async getQuote(symbol) {
    if (!this.apiKey) throw new Error('Alpha Vantage API key required');

    const url = `${this.baseUrl}?function=GLOBAL_QUOTE&symbol=${symbol}&apikey=${this.apiKey}`;
    const data = await this.fetchWithRetry(url, {}, 'alphaVantage');

    const quote = data['Global Quote'];
    if (!quote) throw new Error(`No data for symbol: ${symbol}`);

    return {
      symbol: quote['01. symbol'],
      price: parseFloat(quote['05. price']),
      previousClose: parseFloat(quote['08. previous close']),
      change: parseFloat(quote['09. change']),
      changePercent: parseFloat(quote['10. change percent'].replace('%', '')),
      volume: parseInt(quote['06. volume']),
      timestamp: Date.now()
    };
  }

  async getHistorical(symbol, outputSize = 'compact') {
    if (!this.apiKey) throw new Error('Alpha Vantage API key required');

    const url = `${this.baseUrl}?function=TIME_SERIES_DAILY&symbol=${symbol}&outputsize=${outputSize}&apikey=${this.apiKey}`;
    const data = await this.fetchWithRetry(url, {}, 'alphaVantage');

    const timeSeries = data['Time Series (Daily)'];
    if (!timeSeries) throw new Error(`No data for symbol: ${symbol}`);

    return Object.entries(timeSeries).map(([date, values]) => ({
      timestamp: new Date(date).getTime(),
      open: parseFloat(values['1. open']),
      high: parseFloat(values['2. high']),
      low: parseFloat(values['3. low']),
      close: parseFloat(values['4. close']),
      volume: parseInt(values['5. volume']),
      source: 'alphaVantage'
    })).sort((a, b) => a.timestamp - b.timestamp);
  }

  async getIntraday(symbol, interval = '5min') {
    if (!this.apiKey) throw new Error('Alpha Vantage API key required');

    const url = `${this.baseUrl}?function=TIME_SERIES_INTRADAY&symbol=${symbol}&interval=${interval}&apikey=${this.apiKey}`;
    const data = await this.fetchWithRetry(url, {}, 'alphaVantage');

    const key = `Time Series (${interval})`;
    const timeSeries = data[key];
    if (!timeSeries) throw new Error(`No data for symbol: ${symbol}`);

    return Object.entries(timeSeries).map(([datetime, values]) => ({
      timestamp: new Date(datetime).getTime(),
      open: parseFloat(values['1. open']),
      high: parseFloat(values['2. high']),
      low: parseFloat(values['3. low']),
      close: parseFloat(values['4. close']),
      volume: parseInt(values['5. volume']),
      source: 'alphaVantage'
    })).sort((a, b) => a.timestamp - b.timestamp);
  }

  async getSentiment(tickers) {
    if (!this.apiKey) throw new Error('Alpha Vantage API key required');

    const tickerList = Array.isArray(tickers) ? tickers.join(',') : tickers;
    const url = `${this.baseUrl}?function=NEWS_SENTIMENT&tickers=${tickerList}&apikey=${this.apiKey}`;
    const data = await this.fetchWithRetry(url, {}, 'alphaVantage');

    return data.feed?.map(item => ({
      title: item.title,
      url: item.url,
      source: item.source,
      summary: item.summary,
      sentiment: item.overall_sentiment_score,
      sentimentLabel: item.overall_sentiment_label,
      tickers: item.ticker_sentiment,
      timestamp: new Date(item.time_published).getTime()
    })) || [];
  }
}

/**
 * Binance Connector (Crypto)
 */
class BinanceConnector extends BaseConnector {
  constructor(config = {}) {
    super(config);
    this.baseUrl = 'https://api.binance.com/api/v3';
    this.wsUrl = 'wss://stream.binance.com:9443/ws';
  }

  async getQuote(symbol) {
    const url = `${this.baseUrl}/ticker/24hr?symbol=${symbol}`;
    const data = await this.fetchWithRetry(url, {}, 'binance');

    return {
      symbol: data.symbol,
      price: parseFloat(data.lastPrice),
      previousClose: parseFloat(data.prevClosePrice),
      change: parseFloat(data.priceChange),
      changePercent: parseFloat(data.priceChangePercent),
      volume: parseFloat(data.volume),
      quoteVolume: parseFloat(data.quoteVolume),
      high24h: parseFloat(data.highPrice),
      low24h: parseFloat(data.lowPrice),
      timestamp: data.closeTime
    };
  }

  async getHistorical(symbol, interval = '1d', limit = 500) {
    const url = `${this.baseUrl}/klines?symbol=${symbol}&interval=${interval}&limit=${limit}`;
    const data = await this.fetchWithRetry(url, {}, 'binance');

    return data.map(candle => ({
      timestamp: candle[0],
      open: parseFloat(candle[1]),
      high: parseFloat(candle[2]),
      low: parseFloat(candle[3]),
      close: parseFloat(candle[4]),
      volume: parseFloat(candle[5]),
      closeTime: candle[6],
      quoteVolume: parseFloat(candle[7]),
      trades: candle[8],
      source: 'binance'
    }));
  }

  async getOrderBook(symbol, limit = 100) {
    const url = `${this.baseUrl}/depth?symbol=${symbol}&limit=${limit}`;
    const data = await this.fetchWithRetry(url, {}, 'binance');

    return {
      lastUpdateId: data.lastUpdateId,
      bids: data.bids.map(([price, qty]) => ({
        price: parseFloat(price),
        quantity: parseFloat(qty)
      })),
      asks: data.asks.map(([price, qty]) => ({
        price: parseFloat(price),
        quantity: parseFloat(qty)
      }))
    };
  }

  async getTrades(symbol, limit = 100) {
    const url = `${this.baseUrl}/trades?symbol=${symbol}&limit=${limit}`;
    const data = await this.fetchWithRetry(url, {}, 'binance');

    return data.map(trade => ({
      id: trade.id,
      price: parseFloat(trade.price),
      quantity: parseFloat(trade.qty),
      time: trade.time,
      isBuyerMaker: trade.isBuyerMaker
    }));
  }

  // WebSocket subscription for real-time data
  subscribeToTrades(symbol, callback) {
    const ws = new WebSocket(`${this.wsUrl}/${symbol.toLowerCase()}@trade`);

    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      callback({
        symbol: data.s,
        price: parseFloat(data.p),
        quantity: parseFloat(data.q),
        time: data.T,
        isBuyerMaker: data.m
      });
    };

    return {
      close: () => ws.close()
    };
  }

  subscribeToKlines(symbol, interval, callback) {
    const ws = new WebSocket(`${this.wsUrl}/${symbol.toLowerCase()}@kline_${interval}`);

    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      const k = data.k;
      callback({
        symbol: k.s,
        interval: k.i,
        open: parseFloat(k.o),
        high: parseFloat(k.h),
        low: parseFloat(k.l),
        close: parseFloat(k.c),
        volume: parseFloat(k.v),
        isClosed: k.x,
        timestamp: k.t
      });
    };

    return {
      close: () => ws.close()
    };
  }
}

/**
 * Unified Data Manager
 */
class DataManager {
  constructor(config = {}) {
    this.config = { ...connectorConfig, ...config };
    this.connectors = {
      yahoo: new YahooFinanceConnector(config),
      alphaVantage: new AlphaVantageConnector(config),
      binance: new BinanceConnector(config)
    };
    this.preferredSource = config.preferredSource || 'yahoo';
  }

  // Get connector by name
  getConnector(name) {
    return this.connectors[name];
  }

  // Smart quote - try preferred source, fallback to others
  async getQuote(symbol, source = null) {
    const sources = source ? [source] : [this.preferredSource, 'yahoo', 'alphaVantage'];

    for (const src of sources) {
      try {
        const connector = this.connectors[src];
        if (connector) {
          return await connector.getQuote(symbol);
        }
      } catch (error) {
        console.warn(`Quote failed for ${symbol} from ${src}:`, error.message);
      }
    }

    throw new Error(`Failed to get quote for ${symbol} from all sources`);
  }

  // Get historical data with source selection
  async getHistorical(symbol, options = {}) {
    const {
      source = this.preferredSource,
      period = '1y',
      interval = '1d'
    } = options;

    const connector = this.connectors[source];
    if (!connector) throw new Error(`Unknown source: ${source}`);

    if (source === 'yahoo') {
      return connector.getHistorical(symbol, period, interval);
    } else if (source === 'alphaVantage') {
      return connector.getHistorical(symbol, period === '1y' ? 'full' : 'compact');
    } else if (source === 'binance') {
      return connector.getHistorical(symbol, interval);
    }
  }

  // Get multiple symbols in parallel
  async getQuotes(symbols) {
    const promises = symbols.map(s => this.getQuote(s).catch(e => ({ symbol: s, error: e.message })));
    return Promise.all(promises);
  }

  // Get news sentiment
  async getSentiment(symbols, source = 'alphaVantage') {
    const connector = this.connectors[source];
    if (connector?.getSentiment) {
      return connector.getSentiment(symbols);
    }
    return [];
  }

  // Clear all caches
  clearCache() {
    for (const connector of Object.values(this.connectors)) {
      connector.cache?.clear();
    }
  }
}

// Exports
export {
  DataManager,
  YahooFinanceConnector,
  AlphaVantageConnector,
  BinanceConnector,
  BaseConnector,
  LRUCache,
  RateLimiter,
  connectorConfig
};

// Demo if run directly
const isMainModule = import.meta.url === `file://${process.argv[1]}`;
if (isMainModule) {
  console.log('══════════════════════════════════════════════════════════════════════');
  console.log('DATA CONNECTORS DEMO');
  console.log('══════════════════════════════════════════════════════════════════════\n');

  console.log('Available Connectors:');
  console.log('──────────────────────────────────────────────────────────────────────');
  console.log('  • Yahoo Finance  - Free, delayed quotes, historical data');
  console.log('  • Alpha Vantage  - Free tier (5 req/min), sentiment analysis');
  console.log('  • Binance        - Real-time crypto, WebSocket support');
  console.log();

  console.log('Features:');
  console.log('──────────────────────────────────────────────────────────────────────');
  console.log('  • Rate limiting per source');
  console.log('  • LRU caching with TTL');
  console.log('  • Automatic retry with backoff');
  console.log('  • Data normalization to OHLCV format');
  console.log('  • Multi-source fallback');
  console.log();

  console.log('Example Usage:');
  console.log('──────────────────────────────────────────────────────────────────────');
  console.log(`
  import { DataManager } from './data-connectors.js';

  const data = new DataManager({
    apiKeys: { alphaVantage: 'YOUR_KEY' }
  });

  // Get quote
  const quote = await data.getQuote('AAPL');

  // Get historical data
  const history = await data.getHistorical('AAPL', { period: '1y' });

  // Get crypto data
  const btc = await data.getQuote('BTCUSDT', 'binance');
  const klines = await data.getHistorical('BTCUSDT', {
    source: 'binance',
    interval: '1h'
  });

  // Get sentiment
  const sentiment = await data.getSentiment(['AAPL', 'MSFT']);
`);

  // Test with mock data (no actual API calls)
  console.log('\nSimulated Output:');
  console.log('──────────────────────────────────────────────────────────────────────');

  const mockQuote = {
    symbol: 'AAPL',
    price: 178.50,
    previousClose: 177.25,
    change: 1.25,
    changePercent: 0.71,
    volume: 52847300,
    timestamp: Date.now()
  };

  console.log('Quote (AAPL):');
  console.log(`  Price: $${mockQuote.price}`);
  console.log(`  Change: $${mockQuote.change} (${mockQuote.changePercent.toFixed(2)}%)`);
  console.log(`  Volume: ${mockQuote.volume.toLocaleString()}`);

  console.log();
  console.log('══════════════════════════════════════════════════════════════════════');
  console.log('Data connectors ready for integration');
  console.log('══════════════════════════════════════════════════════════════════════');
}
