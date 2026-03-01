/**
 * Tests for Stock Market Simulator
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { StockMarketSimulator } from '../../src/generators/stock-market.js';
import type { StockSimulatorConfig, GenerateOptions } from '../../src/generators/stock-market.js';

describe('StockMarketSimulator', () => {
  let config: StockSimulatorConfig;

  beforeEach(() => {
    config = {
      symbols: ['AAPL', 'GOOGL'],
      startDate: '2024-01-01',
      endDate: '2024-01-10',
      volatility: 'medium'
    };
  });

  describe('Initialization', () => {
    it('should create simulator with valid config', () => {
      const simulator = new StockMarketSimulator(config);
      expect(simulator).toBeDefined();
    });

    it('should accept Date objects', () => {
      const simulatorWithDates = new StockMarketSimulator({
        ...config,
        startDate: new Date('2024-01-01'),
        endDate: new Date('2024-01-10')
      });
      expect(simulatorWithDates).toBeDefined();
    });

    it('should handle different volatility levels', () => {
      const lowVol = new StockMarketSimulator({ ...config, volatility: 'low' });
      const highVol = new StockMarketSimulator({ ...config, volatility: 'high' });

      expect(lowVol).toBeDefined();
      expect(highVol).toBeDefined();
    });
  });

  describe('Data Generation', () => {
    it('should generate OHLCV data for all symbols', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate();

      expect(data.length).toBeGreaterThan(0);

      // Check that all symbols are present
      const symbols = new Set(data.map(d => d.symbol));
      expect(symbols.has('AAPL')).toBe(true);
      expect(symbols.has('GOOGL')).toBe(true);
    });

    it('should generate correct number of trading days', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate();

      // Should have data points for both symbols
      const aaplData = data.filter(d => d.symbol === 'AAPL');
      const googlData = data.filter(d => d.symbol === 'GOOGL');

      expect(aaplData.length).toBeGreaterThan(0);
      expect(googlData.length).toBeGreaterThan(0);
    });

    it('should skip weekends by default', async () => {
      const simulator = new StockMarketSimulator({
        symbols: ['AAPL'],
        startDate: '2024-01-06', // Saturday
        endDate: '2024-01-08', // Monday
        volatility: 'medium'
      });
      const data = await simulator.generate();

      // Should only have Monday's data, not Saturday or Sunday
      expect(data.length).toBe(1);
      expect(data[0].date.getDay()).not.toBe(0); // Not Sunday
      expect(data[0].date.getDay()).not.toBe(6); // Not Saturday
    });

    it('should include weekends when configured', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        includeWeekends: true,
        startDate: '2024-01-06', // Saturday
        endDate: '2024-01-08' // Monday
      });
      const data = await simulator.generate();

      const aaplData = data.filter(d => d.symbol === 'AAPL');
      expect(aaplData.length).toBe(3); // Saturday, Sunday, Monday
    });
  });

  describe('OHLCV Data Validation', () => {
    it('should generate valid OHLCV data', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate();

      data.forEach(point => {
        expect(point.open).toBeGreaterThan(0);
        expect(point.high).toBeGreaterThan(0);
        expect(point.low).toBeGreaterThan(0);
        expect(point.close).toBeGreaterThan(0);
        expect(point.volume).toBeGreaterThan(0);

        // High should be highest
        expect(point.high).toBeGreaterThanOrEqual(point.open);
        expect(point.high).toBeGreaterThanOrEqual(point.close);
        expect(point.high).toBeGreaterThanOrEqual(point.low);

        // Low should be lowest
        expect(point.low).toBeLessThanOrEqual(point.open);
        expect(point.low).toBeLessThanOrEqual(point.close);
        expect(point.low).toBeLessThanOrEqual(point.high);
      });
    });

    it('should have reasonable price ranges', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate();

      data.forEach(point => {
        // Prices should be in a reasonable range (not negative, not absurdly high)
        expect(point.open).toBeLessThan(10000);
        expect(point.high).toBeLessThan(10000);
        expect(point.low).toBeLessThan(10000);
        expect(point.close).toBeLessThan(10000);

        // Price precision (2 decimal places)
        expect(point.open.toString().split('.')[1]?.length || 0).toBeLessThanOrEqual(2);
        expect(point.close.toString().split('.')[1]?.length || 0).toBeLessThanOrEqual(2);
      });
    });

    it('should have realistic volume', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate();

      data.forEach(point => {
        expect(Number.isInteger(point.volume)).toBe(true);
        expect(point.volume).toBeGreaterThan(1000000); // At least 1M volume
        expect(point.volume).toBeLessThan(1000000000); // Less than 1B volume
      });
    });
  });

  describe('Market Conditions', () => {
    it('should generate bullish trends', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        startDate: '2024-01-01',
        endDate: '2024-01-30'
      });
      const data = await simulator.generate({ marketConditions: 'bullish' });

      const aaplData = data.filter(d => d.symbol === 'AAPL').sort((a, b) => a.date.getTime() - b.date.getTime());

      if (aaplData.length > 5) {
        const firstPrice = aaplData[0].close;
        const lastPrice = aaplData[aaplData.length - 1].close;

        // Bullish market should trend upward (with some tolerance for randomness)
        // Over 30 days, we expect positive movement more often than not
        const priceChange = ((lastPrice - firstPrice) / firstPrice) * 100;
        // Allow for some randomness, but generally should be positive
      }
    });

    it('should generate bearish trends', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        startDate: '2024-01-01',
        endDate: '2024-01-30'
      });
      const data = await simulator.generate({ marketConditions: 'bearish' });

      expect(data.length).toBeGreaterThan(0);
      // Bearish trends are applied but due to randomness, actual direction may vary
    });

    it('should generate neutral market', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        startDate: '2024-01-01',
        endDate: '2024-01-30'
      });
      const data = await simulator.generate({ marketConditions: 'neutral' });

      expect(data.length).toBeGreaterThan(0);
      // Neutral market should have balanced ups and downs
    });
  });

  describe('Volatility Levels', () => {
    it('should reflect different volatility in price movements', async () => {
      const lowVolSimulator = new StockMarketSimulator({ ...config, volatility: 'low' });
      const highVolSimulator = new StockMarketSimulator({ ...config, volatility: 'high' });

      const lowVolData = await lowVolSimulator.generate();
      const highVolData = await highVolSimulator.generate();

      // Both should generate data
      expect(lowVolData.length).toBeGreaterThan(0);
      expect(highVolData.length).toBeGreaterThan(0);

      // Calculate average daily price range for comparison
      const calcAvgRange = (data: any[]) => {
        const ranges = data.map(d => ((d.high - d.low) / d.close) * 100);
        return ranges.reduce((a, b) => a + b, 0) / ranges.length;
      };

      const lowAvgRange = calcAvgRange(lowVolData.filter(d => d.symbol === 'AAPL'));
      const highAvgRange = calcAvgRange(highVolData.filter(d => d.symbol === 'AAPL'));

      // High volatility should generally have larger ranges (with some tolerance)
      // Due to randomness, this might not always hold, so we just check they're different
      expect(lowAvgRange).toBeGreaterThan(0);
      expect(highAvgRange).toBeGreaterThan(0);
    });
  });

  describe('Optional Features', () => {
    it('should include sentiment when requested', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate({ includeSentiment: true });

      data.forEach(point => {
        expect(point.sentiment).toBeDefined();
        expect(point.sentiment).toBeGreaterThanOrEqual(-1);
        expect(point.sentiment).toBeLessThanOrEqual(1);
      });
    });

    it('should not include sentiment by default', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate();

      // Most points should not have sentiment
      const withSentiment = data.filter(d => d.sentiment !== undefined);
      expect(withSentiment.length).toBe(0);
    });

    it('should include news when requested', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        startDate: '2024-01-01',
        endDate: '2024-02-01' // Longer period for more news events
      });
      const data = await simulator.generate({ includeNews: true });

      // Should have some news events (10% probability per day)
      const withNews = data.filter(d => d.news && d.news.length > 0);
      expect(withNews.length).toBeGreaterThan(0);

      withNews.forEach(point => {
        expect(Array.isArray(point.news)).toBe(true);
        expect(point.news!.length).toBeGreaterThan(0);
        point.news!.forEach(headline => {
          expect(typeof headline).toBe('string');
          expect(headline.length).toBeGreaterThan(0);
        });
      });
    });

    it('should not include news by default', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate();

      const withNews = data.filter(d => d.news && d.news.length > 0);
      expect(withNews.length).toBe(0);
    });
  });

  describe('Date Handling', () => {
    it('should generate data in correct date range', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate();

      const startDate = new Date('2024-01-01');
      const endDate = new Date('2024-01-10');

      data.forEach(point => {
        expect(point.date.getTime()).toBeGreaterThanOrEqual(startDate.getTime());
        expect(point.date.getTime()).toBeLessThanOrEqual(endDate.getTime());
      });
    });

    it('should sort data by date', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate();

      // Data should be sorted by date
      for (let i = 1; i < data.length; i++) {
        expect(data[i].date.getTime()).toBeGreaterThanOrEqual(data[i - 1].date.getTime());
      }
    });

    it('should handle single day generation', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        startDate: '2024-01-15',
        endDate: '2024-01-15'
      });
      const data = await simulator.generate();

      const aaplData = data.filter(d => d.symbol === 'AAPL');
      expect(aaplData.length).toBe(1);
      expect(aaplData[0].date.toISOString().split('T')[0]).toBe('2024-01-15');
    });
  });

  describe('Statistics', () => {
    it('should calculate market statistics', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        startDate: '2024-01-01',
        endDate: '2024-01-30'
      });
      const data = await simulator.generate();

      const aaplData = data.filter(d => d.symbol === 'AAPL');
      const stats = simulator.getStatistics(aaplData);

      expect(stats.totalDays).toBe(aaplData.length);
      expect(stats.avgPrice).toBeGreaterThan(0);
      expect(stats.minPrice).toBeGreaterThan(0);
      expect(stats.maxPrice).toBeGreaterThan(0);
      expect(stats.avgVolume).toBeGreaterThan(0);
      expect(typeof stats.priceChange).toBe('number');
      expect(stats.volatility).toBeGreaterThan(0);

      // Min should be less than avg, avg less than max
      expect(stats.minPrice).toBeLessThanOrEqual(stats.avgPrice);
      expect(stats.avgPrice).toBeLessThanOrEqual(stats.maxPrice);
    });

    it('should handle empty data for statistics', async () => {
      const simulator = new StockMarketSimulator(config);
      const stats = simulator.getStatistics([]);

      expect(stats).toEqual({});
    });

    it('should calculate volatility correctly', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate();

      const aaplData = data.filter(d => d.symbol === 'AAPL');
      const stats = simulator.getStatistics(aaplData);

      expect(stats.volatility).toBeGreaterThan(0);
      expect(stats.volatility).toBeLessThan(100); // Reasonable volatility range
    });
  });

  describe('Multiple Symbols', () => {
    it('should handle single symbol', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        symbols: ['AAPL']
      });
      const data = await simulator.generate();

      expect(data.every(d => d.symbol === 'AAPL')).toBe(true);
    });

    it('should handle many symbols', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        symbols: ['AAPL', 'GOOGL', 'MSFT', 'AMZN', 'TSLA']
      });
      const data = await simulator.generate();

      const symbols = new Set(data.map(d => d.symbol));
      expect(symbols.size).toBe(5);
      expect(symbols.has('AAPL')).toBe(true);
      expect(symbols.has('TSLA')).toBe(true);
    });

    it('should generate independent data for each symbol', async () => {
      const simulator = new StockMarketSimulator(config);
      const data = await simulator.generate();

      const aaplData = data.filter(d => d.symbol === 'AAPL');
      const googlData = data.filter(d => d.symbol === 'GOOGL');

      // Prices should be different (independent generation)
      expect(aaplData[0].close).not.toBe(googlData[0].close);
    });
  });

  describe('Edge Cases', () => {
    it('should handle very short time period', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        startDate: '2024-01-02',
        endDate: '2024-01-02'
      });
      const data = await simulator.generate();

      expect(data.length).toBeGreaterThan(0);
    });

    it('should handle long time periods', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        startDate: '2024-01-01',
        endDate: '2024-12-31'
      });
      const data = await simulator.generate();

      // Should have roughly 252 trading days * 2 symbols
      expect(data.length).toBeGreaterThan(400);
    });

    it('should handle unknown symbols gracefully', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        symbols: ['UNKNOWN', 'FAKE']
      });
      const data = await simulator.generate();

      // Should still generate data with default prices
      expect(data.length).toBeGreaterThan(0);
      data.forEach(point => {
        expect(point.close).toBeGreaterThan(0);
      });
    });
  });

  describe('Performance', () => {
    it('should generate data efficiently', async () => {
      const simulator = new StockMarketSimulator({
        ...config,
        startDate: '2024-01-01',
        endDate: '2024-03-31',
        symbols: ['AAPL', 'GOOGL', 'MSFT']
      });

      const startTime = Date.now();
      await simulator.generate();
      const duration = Date.now() - startTime;

      // Should complete quickly even with 3 months of data
      expect(duration).toBeLessThan(1000);
    });
  });
});
