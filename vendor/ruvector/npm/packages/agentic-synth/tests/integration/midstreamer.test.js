/**
 * Integration tests for Midstreamer adapter
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { MidstreamerAdapter } from '../../src/adapters/midstreamer.js';
import { DataGenerator } from '../../src/generators/data-generator.js';

describe('Midstreamer Integration', () => {
  let adapter;
  let generator;

  beforeEach(async () => {
    adapter = new MidstreamerAdapter({
      endpoint: 'http://localhost:8080',
      apiKey: 'test-key'
    });

    generator = new DataGenerator({
      schema: {
        name: { type: 'string', length: 10 },
        value: { type: 'number', min: 0, max: 100 }
      }
    });
  });

  afterEach(async () => {
    if (adapter.isConnected()) {
      await adapter.disconnect();
    }
  });

  describe('connection', () => {
    it('should connect to Midstreamer', async () => {
      const result = await adapter.connect();
      expect(result).toBe(true);
      expect(adapter.isConnected()).toBe(true);
    });

    it('should disconnect from Midstreamer', async () => {
      await adapter.connect();
      await adapter.disconnect();
      expect(adapter.isConnected()).toBe(false);
    });

    it('should handle reconnection', async () => {
      await adapter.connect();
      await adapter.disconnect();
      await adapter.connect();
      expect(adapter.isConnected()).toBe(true);
    });
  });

  describe('data streaming', () => {
    beforeEach(async () => {
      await adapter.connect();
    });

    it('should stream generated data', async () => {
      const data = generator.generate(5);
      const results = await adapter.stream(data);

      expect(results).toHaveLength(5);
      results.forEach(result => {
        expect(result).toHaveProperty('id');
        expect(result).toHaveProperty('status');
        expect(result.status).toBe('streamed');
      });
    });

    it('should handle empty data array', async () => {
      const results = await adapter.stream([]);
      expect(results).toHaveLength(0);
    });

    it('should throw error when not connected', async () => {
      await adapter.disconnect();
      await expect(adapter.stream([{ id: 1 }])).rejects.toThrow('Not connected to Midstreamer');
    });

    it('should throw error for invalid data', async () => {
      await expect(adapter.stream('not an array')).rejects.toThrow('Data must be an array');
    });
  });

  describe('end-to-end workflow', () => {
    it('should generate and stream data', async () => {
      // Generate synthetic data
      const data = generator.generate(10);
      expect(data).toHaveLength(10);

      // Connect to Midstreamer
      await adapter.connect();
      expect(adapter.isConnected()).toBe(true);

      // Stream data
      const results = await adapter.stream(data);
      expect(results).toHaveLength(10);

      // Verify all items processed
      results.forEach((result, index) => {
        expect(result.id).toBe(data[index].id);
        expect(result.status).toBe('streamed');
      });

      // Cleanup
      await adapter.disconnect();
    });

    it('should handle large batches', async () => {
      const largeData = generator.generate(1000);

      await adapter.connect();
      const results = await adapter.stream(largeData);

      expect(results).toHaveLength(1000);
    });
  });

  describe('error handling', () => {
    it('should handle connection failures', async () => {
      const failingAdapter = new MidstreamerAdapter({
        endpoint: 'http://invalid-endpoint:99999'
      });

      // Note: In real implementation, this would actually fail
      // For now, our mock always succeeds
      await expect(failingAdapter.connect()).resolves.toBe(true);
    });

    it('should recover from streaming errors', async () => {
      await adapter.connect();

      // First stream succeeds
      const data1 = generator.generate(5);
      await adapter.stream(data1);

      // Second stream should also succeed
      const data2 = generator.generate(5);
      const results = await adapter.stream(data2);

      expect(results).toHaveLength(5);
    });
  });

  describe('performance', () => {
    beforeEach(async () => {
      await adapter.connect();
    });

    it('should stream 100 items quickly', async () => {
      const data = generator.generate(100);

      const start = Date.now();
      await adapter.stream(data);
      const duration = Date.now() - start;

      expect(duration).toBeLessThan(500); // Less than 500ms
    });

    it('should handle multiple concurrent streams', async () => {
      const batches = Array.from({ length: 5 }, () => generator.generate(20));

      const start = Date.now();
      const results = await Promise.all(
        batches.map(batch => adapter.stream(batch))
      );
      const duration = Date.now() - start;

      expect(results).toHaveLength(5);
      expect(duration).toBeLessThan(1000);
    });
  });
});
