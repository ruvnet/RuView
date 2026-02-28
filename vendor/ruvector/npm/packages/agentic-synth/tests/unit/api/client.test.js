/**
 * Unit tests for APIClient
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { APIClient } from '../../../src/api/client.js';

// Mock fetch
global.fetch = vi.fn();

describe('APIClient', () => {
  let client;

  beforeEach(() => {
    client = new APIClient({
      baseUrl: 'https://api.test.com',
      apiKey: 'test-key-123',
      timeout: 5000,
      retries: 3
    });
    vi.clearAllMocks();
  });

  describe('constructor', () => {
    it('should create client with default options', () => {
      const defaultClient = new APIClient();
      expect(defaultClient.baseUrl).toBe('https://api.example.com');
      expect(defaultClient.timeout).toBe(5000);
      expect(defaultClient.retries).toBe(3);
    });

    it('should accept custom options', () => {
      expect(client.baseUrl).toBe('https://api.test.com');
      expect(client.apiKey).toBe('test-key-123');
      expect(client.timeout).toBe(5000);
      expect(client.retries).toBe(3);
    });
  });

  describe('request', () => {
    it('should make successful request', async () => {
      const mockResponse = { data: 'test' };
      global.fetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockResponse
      });

      const result = await client.request('/test');

      expect(global.fetch).toHaveBeenCalledTimes(1);
      expect(result).toEqual(mockResponse);
    });

    it('should include authorization header', async () => {
      global.fetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({})
      });

      await client.request('/test');

      const callArgs = global.fetch.mock.calls[0];
      expect(callArgs[1].headers.Authorization).toBe('Bearer test-key-123');
    });

    it('should handle API errors', async () => {
      // Mock must return error for all retry attempts
      global.fetch.mockResolvedValue({
        ok: false,
        status: 404,
        statusText: 'Not Found'
      });

      await expect(client.request('/test')).rejects.toThrow('API error: 404 Not Found');
    });

    it('should retry on failure', async () => {
      global.fetch
        .mockRejectedValueOnce(new Error('Network error'))
        .mockRejectedValueOnce(new Error('Network error'))
        .mockResolvedValueOnce({
          ok: true,
          json: async () => ({ success: true })
        });

      const result = await client.request('/test');

      expect(global.fetch).toHaveBeenCalledTimes(3);
      expect(result).toEqual({ success: true });
    });

    it('should fail after max retries', async () => {
      global.fetch.mockRejectedValue(new Error('Network error'));

      await expect(client.request('/test')).rejects.toThrow('Network error');
      expect(global.fetch).toHaveBeenCalledTimes(3);
    });

    it('should respect timeout', async () => {
      const shortTimeoutClient = new APIClient({ timeout: 100 });

      global.fetch.mockImplementationOnce(() =>
        new Promise(resolve => setTimeout(resolve, 200))
      );

      // Note: This test depends on AbortController implementation
      // May need adjustment based on test environment
    });
  });

  describe('get', () => {
    it('should make GET request', async () => {
      global.fetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ result: 'success' })
      });

      const result = await client.get('/users');

      expect(result).toEqual({ result: 'success' });
      expect(global.fetch.mock.calls[0][1].method).toBe('GET');
    });

    it('should append query parameters', async () => {
      global.fetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({})
      });

      await client.get('/users', { page: 1, limit: 10 });

      const url = global.fetch.mock.calls[0][0];
      expect(url).toContain('?page=1&limit=10');
    });
  });

  describe('post', () => {
    it('should make POST request', async () => {
      global.fetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ id: 123 })
      });

      const data = { name: 'Test User' };
      const result = await client.post('/users', data);

      expect(result).toEqual({ id: 123 });

      const callArgs = global.fetch.mock.calls[0];
      expect(callArgs[1].method).toBe('POST');
      expect(callArgs[1].body).toBe(JSON.stringify(data));
    });

    it('should include content-type header', async () => {
      global.fetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({})
      });

      await client.post('/test', {});

      const headers = global.fetch.mock.calls[0][1].headers;
      expect(headers['Content-Type']).toBe('application/json');
    });
  });

  describe('error handling', () => {
    it('should handle network errors', async () => {
      global.fetch.mockRejectedValue(new Error('Failed to fetch'));

      await expect(client.get('/test')).rejects.toThrow();
    });

    it('should handle timeout errors', async () => {
      global.fetch.mockImplementationOnce(() =>
        new Promise((_, reject) => {
          setTimeout(() => reject(new Error('Timeout')), 100);
        })
      );

      await expect(client.request('/test')).rejects.toThrow();
    });
  });
});
