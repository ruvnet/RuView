/**
 * Stock Market Simulator
 * Generate realistic OHLCV financial data
 */

import type { StockDataPoint } from '../types/index.js';

export interface StockSimulatorConfig {
  symbols: string[];
  startDate: string | Date;
  endDate: string | Date;
  volatility: 'low' | 'medium' | 'high';
  includeWeekends?: boolean;
}

export interface GenerateOptions {
  includeNews?: boolean;
  includeSentiment?: boolean;
  marketConditions?: 'bearish' | 'neutral' | 'bullish';
}

export class StockMarketSimulator {
  private config: StockSimulatorConfig;
  private volatilityMultiplier: number;

  constructor(config: StockSimulatorConfig) {
    this.config = config;
    this.volatilityMultiplier = this.getVolatilityMultiplier(config.volatility);
  }

  /**
   * Generate stock market data
   */
  async generate(options: GenerateOptions = {}): Promise<StockDataPoint[]> {
    const startDate = new Date(this.config.startDate);
    const endDate = new Date(this.config.endDate);
    const data: StockDataPoint[] = [];

    for (const symbol of this.config.symbols) {
      const symbolData = await this.generateSymbol(symbol, startDate, endDate, options);
      data.push(...symbolData);
    }

    return data.sort((a, b) => a.date.getTime() - b.date.getTime());
  }

  /**
   * Generate data for a single symbol
   */
  private async generateSymbol(
    symbol: string,
    startDate: Date,
    endDate: Date,
    options: GenerateOptions
  ): Promise<StockDataPoint[]> {
    const data: StockDataPoint[] = [];
    let currentDate = new Date(startDate);
    let lastClose = this.getInitialPrice(symbol);

    const trendMultiplier = this.getTrendMultiplier(options.marketConditions);

    while (currentDate <= endDate) {
      // Skip weekends unless explicitly included
      if (!this.config.includeWeekends && this.isWeekend(currentDate)) {
        currentDate.setDate(currentDate.getDate() + 1);
        continue;
      }

      const dataPoint = this.generateDataPoint(
        symbol,
        currentDate,
        lastClose,
        trendMultiplier,
        options
      );

      data.push(dataPoint);
      lastClose = dataPoint.close;

      currentDate.setDate(currentDate.getDate() + 1);
    }

    return data;
  }

  /**
   * Generate a single data point (day)
   */
  private generateDataPoint(
    symbol: string,
    date: Date,
    lastClose: number,
    trendMultiplier: number,
    options: GenerateOptions
  ): StockDataPoint {
    // Generate realistic OHLCV data
    const trend = (Math.random() - 0.5) * 0.02 * trendMultiplier;
    const volatility = this.volatilityMultiplier * (Math.random() * 0.015);

    const open = lastClose * (1 + (Math.random() - 0.5) * 0.005);
    const close = open * (1 + trend + (Math.random() - 0.5) * volatility);

    const high = Math.max(open, close) * (1 + Math.random() * volatility);
    const low = Math.min(open, close) * (1 - Math.random() * volatility);

    const baseVolume = this.getBaseVolume(symbol);
    const volume = Math.floor(baseVolume * (0.5 + Math.random() * 1.5));

    const dataPoint: StockDataPoint = {
      symbol,
      date: new Date(date),
      open: parseFloat(open.toFixed(2)),
      high: parseFloat(high.toFixed(2)),
      low: parseFloat(low.toFixed(2)),
      close: parseFloat(close.toFixed(2)),
      volume
    };

    // Add optional features
    if (options.includeSentiment) {
      dataPoint.sentiment = this.generateSentiment(trend);
    }

    if (options.includeNews && Math.random() < 0.1) { // 10% chance of news
      dataPoint.news = this.generateNews(symbol, trend);
    }

    return dataPoint;
  }

  /**
   * Get initial price for symbol
   */
  private getInitialPrice(symbol: string): number {
    const prices: Record<string, number> = {
      AAPL: 150,
      GOOGL: 140,
      MSFT: 350,
      AMZN: 130,
      TSLA: 200
    };

    return prices[symbol] || 100;
  }

  /**
   * Get base trading volume for symbol
   */
  private getBaseVolume(symbol: string): number {
    const volumes: Record<string, number> = {
      AAPL: 50000000,
      GOOGL: 25000000,
      MSFT: 30000000,
      AMZN: 40000000,
      TSLA: 100000000
    };

    return volumes[symbol] || 10000000;
  }

  /**
   * Get volatility multiplier
   */
  private getVolatilityMultiplier(volatility: 'low' | 'medium' | 'high'): number {
    const multipliers = {
      low: 0.5,
      medium: 1.0,
      high: 2.0
    };

    return multipliers[volatility];
  }

  /**
   * Get trend multiplier based on market conditions
   */
  private getTrendMultiplier(conditions?: 'bearish' | 'neutral' | 'bullish'): number {
    if (!conditions) return 1.0;

    const multipliers = {
      bearish: -1.5,
      neutral: 1.0,
      bullish: 1.5
    };

    return multipliers[conditions];
  }

  /**
   * Check if date is weekend
   */
  private isWeekend(date: Date): boolean {
    const day = date.getDay();
    return day === 0 || day === 6; // Sunday = 0, Saturday = 6
  }

  /**
   * Generate sentiment score based on price movement
   */
  private generateSentiment(trend: number): number {
    // Sentiment from -1 (very negative) to 1 (very positive)
    const baseSentiment = trend * 50; // Scale trend
    const noise = (Math.random() - 0.5) * 0.3;
    return Math.max(-1, Math.min(1, baseSentiment + noise));
  }

  /**
   * Generate realistic news headlines
   */
  private generateNews(symbol: string, trend: number): string[] {
    const newsTemplates = {
      positive: [
        `${symbol} reports strong quarterly earnings`,
        `${symbol} announces new product launch`,
        `Analysts upgrade ${symbol} to "buy"`,
        `${symbol} expands into new markets`
      ],
      negative: [
        `${symbol} faces regulatory challenges`,
        `${symbol} misses earnings expectations`,
        `Concerns grow over ${symbol}'s market position`,
        `${symbol} announces layoffs`
      ],
      neutral: [
        `${symbol} holds annual shareholder meeting`,
        `${symbol} updates corporate strategy`,
        `Market watches ${symbol} closely`,
        `${symbol} maintains steady performance`
      ]
    };

    let category: 'positive' | 'negative' | 'neutral';
    if (trend > 0.01) {
      category = 'positive';
    } else if (trend < -0.01) {
      category = 'negative';
    } else {
      category = 'neutral';
    }

    const templates = newsTemplates[category];
    const selectedNews = templates[Math.floor(Math.random() * templates.length)];

    return [selectedNews];
  }

  /**
   * Get market statistics
   */
  getStatistics(data: StockDataPoint[]): Record<string, any> {
    if (data.length === 0) return {};

    const closes = data.map(d => d.close);
    const volumes = data.map(d => d.volume);

    return {
      totalDays: data.length,
      avgPrice: closes.reduce((a, b) => a + b, 0) / closes.length,
      minPrice: Math.min(...closes),
      maxPrice: Math.max(...closes),
      avgVolume: volumes.reduce((a, b) => a + b, 0) / volumes.length,
      priceChange: ((closes[closes.length - 1] - closes[0]) / closes[0]) * 100,
      volatility: this.calculateVolatility(closes)
    };
  }

  /**
   * Calculate price volatility (standard deviation)
   */
  private calculateVolatility(prices: number[]): number {
    const mean = prices.reduce((a, b) => a + b, 0) / prices.length;
    const variance = prices.reduce((sum, price) => sum + Math.pow(price - mean, 2), 0) / prices.length;
    return Math.sqrt(variance);
  }
}
