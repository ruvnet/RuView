/**
 * Stock Market Simulator - Realistic financial market data generation
 *
 * Generates OHLCV (Open, High, Low, Close, Volume) data with realistic market
 * dynamics, news events, and sentiment analysis. Perfect for backtesting trading
 * strategies and financial ML models.
 *
 * @packageDocumentation
 */

import { EventEmitter } from 'events';
import { AgenticSynth, SynthConfig, GenerationResult, TimeSeriesOptions } from '@ruvector/agentic-synth';

/**
 * OHLCV candlestick data point
 */
export interface OHLCVData {
  timestamp: Date;
  symbol: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  vwap?: number; // Volume-weighted average price
}

/**
 * Market news event
 */
export interface MarketNewsEvent {
  timestamp: Date;
  headline: string;
  sentiment: 'bullish' | 'bearish' | 'neutral';
  impact: 'low' | 'medium' | 'high';
  affectedSymbols: string[];
}

/**
 * Market condition type
 */
export type MarketCondition = 'bullish' | 'bearish' | 'sideways' | 'volatile' | 'crash' | 'rally';

/**
 * Stock market simulation configuration
 */
export interface StockMarketConfig extends Partial<SynthConfig> {
  symbols?: string[]; // Stock symbols to simulate
  startPrice?: number; // Starting price for simulation
  volatility?: number; // Price volatility (0-1)
  marketCondition?: MarketCondition;
  includeNews?: boolean; // Generate news events
  newsFrequency?: number; // News events per day
  tradingHours?: boolean; // Only generate during market hours
}

/**
 * Market statistics
 */
export interface MarketStatistics {
  totalCandles: number;
  avgVolume: number;
  priceChange: number;
  priceChangePercent: number;
  volatility: number;
  newsEvents: number;
}

/**
 * Stock Market Simulator with realistic OHLCV generation
 *
 * Features:
 * - Realistic OHLCV candlestick data
 * - Multiple market conditions (bull, bear, sideways, etc.)
 * - News event generation with sentiment
 * - Volume patterns and trends
 * - Trading hours simulation
 * - Statistical analysis
 *
 * @example
 * ```typescript
 * const simulator = new StockMarketSimulator({
 *   provider: 'gemini',
 *   apiKey: process.env.GEMINI_API_KEY,
 *   symbols: ['AAPL', 'GOOGL', 'MSFT'],
 *   marketCondition: 'bullish',
 *   includeNews: true
 * });
 *
 * // Generate market data
 * const result = await simulator.generateMarketData({
 *   startDate: new Date('2024-01-01'),
 *   endDate: new Date('2024-12-31'),
 *   interval: '1h'
 * });
 *
 * // Get news events
 * const news = await simulator.generateNewsEvents(10);
 *
 * // Analyze statistics
 * const stats = simulator.getStatistics();
 * console.log(`Total candles: ${stats.totalCandles}`);
 * ```
 */
export class StockMarketSimulator extends EventEmitter {
  private synth: AgenticSynth;
  private config: StockMarketConfig;
  private generatedCandles: OHLCVData[] = [];
  private newsEvents: MarketNewsEvent[] = [];
  private currentPrice: Map<string, number> = new Map();

  constructor(config: StockMarketConfig = {}) {
    super();

    this.config = {
      provider: config.provider || 'gemini',
      apiKey: config.apiKey || process.env.GEMINI_API_KEY || '',
      ...(config.model && { model: config.model }),
      cacheStrategy: config.cacheStrategy || 'memory',
      cacheTTL: config.cacheTTL || 3600,
      maxRetries: config.maxRetries || 3,
      timeout: config.timeout || 30000,
      streaming: config.streaming || false,
      automation: config.automation || false,
      vectorDB: config.vectorDB || false,
      symbols: config.symbols || ['STOCK'],
      startPrice: config.startPrice ?? 100,
      volatility: config.volatility ?? 0.02,
      marketCondition: config.marketCondition || 'sideways',
      includeNews: config.includeNews ?? false,
      newsFrequency: config.newsFrequency ?? 3,
      tradingHours: config.tradingHours ?? true
    };

    this.synth = new AgenticSynth(this.config);

    // Initialize starting prices
    this.config.symbols.forEach(symbol => {
      this.currentPrice.set(symbol, this.config.startPrice);
    });
  }

  /**
   * Generate realistic OHLCV market data
   */
  async generateMarketData(options: {
    startDate?: Date;
    endDate?: Date;
    interval?: string;
    symbol?: string;
  } = {}): Promise<GenerationResult<OHLCVData>> {
    const symbol = options.symbol || this.config.symbols[0];

    this.emit('generation:start', { symbol, options });

    try {
      // Generate synthetic time series data
      const timeSeriesOptions: Partial<TimeSeriesOptions> = {
        startDate: options.startDate || new Date(Date.now() - 30 * 24 * 60 * 60 * 1000),
        endDate: options.endDate || new Date(),
        interval: options.interval || '1h',
        metrics: ['price', 'volume'],
        trend: this.mapMarketConditionToTrend(this.config.marketCondition),
        seasonality: true,
        noise: this.config.volatility
      };

      const result = await this.synth.generateTimeSeries<{ price: number; volume: number }>(
        timeSeriesOptions
      );

      // Convert to OHLCV format
      const candles = this.convertToOHLCV(result.data, symbol);

      // Filter for trading hours if enabled
      const filteredCandles = this.config.tradingHours
        ? this.filterTradingHours(candles)
        : candles;

      this.generatedCandles.push(...filteredCandles);

      this.emit('generation:complete', {
        symbol,
        candleCount: filteredCandles.length,
        priceRange: {
          min: Math.min(...filteredCandles.map(c => c.low)),
          max: Math.max(...filteredCandles.map(c => c.high))
        }
      });

      return {
        data: filteredCandles,
        metadata: result.metadata
      };
    } catch (error) {
      this.emit('generation:error', { error, symbol });
      throw error;
    }
  }

  /**
   * Generate market news events with sentiment
   */
  async generateNewsEvents(count: number = 10): Promise<MarketNewsEvent[]> {
    this.emit('news:generating', { count });

    try {
      const result = await this.synth.generateEvents<{
        headline: string;
        sentiment: string;
        impact: string;
        symbols: string[];
      }>({
        count,
        eventTypes: ['earnings', 'merger', 'regulation', 'product-launch', 'executive-change'],
        distribution: 'poisson'
      });

      const newsEvents: MarketNewsEvent[] = result.data.map(event => ({
        timestamp: new Date(),
        headline: event.headline,
        sentiment: this.parseSentiment(event.sentiment),
        impact: this.parseImpact(event.impact),
        affectedSymbols: event.symbols.filter(s => this.config.symbols.includes(s))
      }));

      this.newsEvents.push(...newsEvents);

      this.emit('news:generated', { count: newsEvents.length });

      return newsEvents;
    } catch (error) {
      this.emit('news:error', { error });
      throw error;
    }
  }

  /**
   * Generate multi-symbol market data in parallel
   */
  async generateMultiSymbolData(options: {
    startDate?: Date;
    endDate?: Date;
    interval?: string;
  } = {}): Promise<Map<string, OHLCVData[]>> {
    this.emit('multi-symbol:start', { symbols: this.config.symbols });

    const results = new Map<string, OHLCVData[]>();

    // Generate for all symbols in parallel
    const promises = this.config.symbols.map(async symbol => {
      const result = await this.generateMarketData({ ...options, symbol });
      return { symbol, data: result.data };
    });

    const symbolResults = await Promise.all(promises);

    symbolResults.forEach(({ symbol, data }) => {
      results.set(symbol, data);
    });

    this.emit('multi-symbol:complete', {
      symbols: this.config.symbols.length,
      totalCandles: Array.from(results.values()).reduce((sum, candles) => sum + candles.length, 0)
    });

    return results;
  }

  /**
   * Get market statistics
   */
  getStatistics(symbol?: string): MarketStatistics {
    const candles = symbol
      ? this.generatedCandles.filter(c => c.symbol === symbol)
      : this.generatedCandles;

    if (candles.length === 0) {
      return {
        totalCandles: 0,
        avgVolume: 0,
        priceChange: 0,
        priceChangePercent: 0,
        volatility: 0,
        newsEvents: this.newsEvents.length
      };
    }

    const volumes = candles.map(c => c.volume);
    const avgVolume = volumes.reduce((a, b) => a + b, 0) / volumes.length;

    const firstPrice = candles[0].open;
    const lastPrice = candles[candles.length - 1].close;
    const priceChange = lastPrice - firstPrice;
    const priceChangePercent = (priceChange / firstPrice) * 100;

    // Calculate volatility as standard deviation of returns
    const returns = candles.slice(1).map((c, i) =>
      (c.close - candles[i].close) / candles[i].close
    );
    const avgReturn = returns.reduce((a, b) => a + b, 0) / returns.length;
    const variance = returns.reduce((sum, r) => sum + Math.pow(r - avgReturn, 2), 0) / returns.length;
    const volatility = Math.sqrt(variance);

    return {
      totalCandles: candles.length,
      avgVolume,
      priceChange,
      priceChangePercent,
      volatility,
      newsEvents: this.newsEvents.length
    };
  }

  /**
   * Export market data to CSV format
   */
  exportToCSV(symbol?: string): string {
    const candles = symbol
      ? this.generatedCandles.filter(c => c.symbol === symbol)
      : this.generatedCandles;

    const headers = ['timestamp', 'symbol', 'open', 'high', 'low', 'close', 'volume', 'vwap'];
    const rows = candles.map(c => [
      c.timestamp.toISOString(),
      c.symbol,
      c.open,
      c.high,
      c.low,
      c.close,
      c.volume,
      c.vwap || ''
    ].join(','));

    return [headers.join(','), ...rows].join('\n');
  }

  /**
   * Reset simulator state
   */
  reset(): void {
    this.generatedCandles = [];
    this.newsEvents = [];
    this.config.symbols.forEach(symbol => {
      this.currentPrice.set(symbol, this.config.startPrice);
    });

    this.emit('reset', { timestamp: new Date() });
  }

  /**
   * Convert generated data to OHLCV format
   */
  private convertToOHLCV(data: { price: number; volume: number }[], symbol: string): OHLCVData[] {
    return data.map((point, i) => {
      const basePrice = point.price;
      const dailyVolatility = this.config.volatility * basePrice;

      // Generate realistic OHLC from base price
      const open = i === 0 ? basePrice : basePrice * (1 + (Math.random() - 0.5) * 0.01);
      const close = basePrice;
      const high = Math.max(open, close) * (1 + Math.random() * (dailyVolatility / basePrice));
      const low = Math.min(open, close) * (1 - Math.random() * (dailyVolatility / basePrice));

      // Calculate VWAP
      const vwap = (high + low + close) / 3;

      return {
        timestamp: new Date(Date.now() - (data.length - i) * 60 * 60 * 1000),
        symbol,
        open,
        high,
        low,
        close,
        volume: point.volume,
        vwap
      };
    });
  }

  /**
   * Filter candles to trading hours only (9:30 AM - 4:00 PM ET)
   */
  private filterTradingHours(candles: OHLCVData[]): OHLCVData[] {
    return candles.filter(candle => {
      const hour = candle.timestamp.getHours();
      const minute = candle.timestamp.getMinutes();
      const timeInMinutes = hour * 60 + minute;

      // 9:30 AM = 570 minutes, 4:00 PM = 960 minutes
      return timeInMinutes >= 570 && timeInMinutes <= 960;
    });
  }

  /**
   * Map market condition to trend direction
   */
  private mapMarketConditionToTrend(condition: MarketCondition): 'up' | 'down' | 'stable' | 'random' {
    switch (condition) {
      case 'bullish':
      case 'rally':
        return 'up';
      case 'bearish':
      case 'crash':
        return 'down';
      case 'sideways':
        return 'stable';
      case 'volatile':
        return 'random';
      default:
        return 'stable';
    }
  }

  /**
   * Parse sentiment string to typed value
   */
  private parseSentiment(sentiment: string): 'bullish' | 'bearish' | 'neutral' {
    const lower = sentiment.toLowerCase();
    if (lower.includes('bull') || lower.includes('positive')) return 'bullish';
    if (lower.includes('bear') || lower.includes('negative')) return 'bearish';
    return 'neutral';
  }

  /**
   * Parse impact string to typed value
   */
  private parseImpact(impact: string): 'low' | 'medium' | 'high' {
    const lower = impact.toLowerCase();
    if (lower.includes('high') || lower.includes('major')) return 'high';
    if (lower.includes('medium') || lower.includes('moderate')) return 'medium';
    return 'low';
  }
}

/**
 * Create a new stock market simulator instance
 */
export function createStockMarketSimulator(config?: StockMarketConfig): StockMarketSimulator {
  return new StockMarketSimulator(config);
}
