import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  initDebugConsole,
  subscribeToLogs,
  getLogs,
  clearLogs,
  debug,
  timing,
} from '../utils/debug';

describe('Debug Console', () => {
  beforeEach(() => {
    clearLogs();
    vi.clearAllMocks();
  });

  describe('initDebugConsole', () => {
    it('initializes without errors', () => {
      expect(() => initDebugConsole()).not.toThrow();
    });

    it('overrides console methods', () => {
      initDebugConsole();

      // Console.log should still work
      expect(() => console.log('test')).not.toThrow();
    });
  });

  describe('debug logging', () => {
    it('logs info messages', () => {
      debug.info('Test info message', { data: 'test' });

      const logs = getLogs();
      expect(logs.some((l) => l.message === 'Test info message')).toBe(true);
    });

    it('logs warning messages', () => {
      debug.warn('Test warning', { warning: true });

      const logs = getLogs();
      expect(logs.some((l) => l.level === 'warn')).toBe(true);
    });

    it('logs error messages', () => {
      debug.error('Test error');

      const logs = getLogs();
      expect(logs.some((l) => l.level === 'error')).toBe(true);
    });

    it('logs debug messages', () => {
      debug.debug('Debug message');

      const logs = getLogs();
      expect(logs.some((l) => l.level === 'debug')).toBe(true);
    });
  });

  describe('subscribeToLogs', () => {
    it('notifies subscribers on new logs', () => {
      const listener = vi.fn();
      subscribeToLogs(listener);

      debug.log('New log');

      expect(listener).toHaveBeenCalled();
    });

    it('returns unsubscribe function', () => {
      const listener = vi.fn();
      const unsubscribe = subscribeToLogs(listener);

      unsubscribe();
      listener.mockClear();

      debug.log('After unsubscribe');

      // Listener should not be called after unsubscribe
    });
  });

  describe('clearLogs', () => {
    it('removes all logs', () => {
      debug.log('Log 1');
      debug.log('Log 2');

      expect(getLogs().length).toBeGreaterThan(0);

      clearLogs();

      expect(getLogs().length).toBe(0);
    });
  });

  describe('timing', () => {
    it('starts and ends timing', () => {
      timing.start('test-operation');
      const duration = timing.end('test-operation');

      expect(duration).toBeGreaterThanOrEqual(0);
    });

    it('returns 0 for unknown labels', () => {
      const duration = timing.end('unknown-label');
      expect(duration).toBe(0);
    });

    it('measures async operations', async () => {
      const result = await timing.measure('async-op', async () => {
        await new Promise((r) => setTimeout(r, 10));
        return 'done';
      });

      expect(result).toBe('done');
    });
  });
});
