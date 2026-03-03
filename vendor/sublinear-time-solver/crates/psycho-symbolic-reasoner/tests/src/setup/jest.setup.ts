/**
 * Jest global setup and configuration
 */

import { jest } from '@jest/globals';

// Set up global test environment
beforeAll(async () => {
  // Initialize WASM modules if needed
  if (typeof window !== 'undefined') {
    // Browser-like environment setup
    global.console = console;
  }

  // Set up performance monitoring
  if (!global.performance) {
    const { performance } = await import('perf_hooks');
    global.performance = performance;
  }

  // Memory monitoring setup
  if (process.env.NODE_ENV === 'test') {
    // Enable garbage collection for memory tests
    if (global.gc) {
      global.gc();
    }
  }
});

// Clean up after each test
afterEach(() => {
  // Clear any timers
  jest.clearAllTimers();

  // Force garbage collection if available
  if (global.gc) {
    global.gc();
  }
});

// Global error handling
process.on('unhandledRejection', (reason, promise) => {
  console.error('Unhandled Rejection at:', promise, 'reason:', reason);
});

process.on('uncaughtException', (error) => {
  console.error('Uncaught Exception:', error);
});

// Extend Jest matchers
expect.extend({
  toBeWithinRange(received: number, floor: number, ceiling: number) {
    const pass = received >= floor && received <= ceiling;
    if (pass) {
      return {
        message: () =>
          `expected ${received} not to be within range ${floor} - ${ceiling}`,
        pass: true,
      };
    } else {
      return {
        message: () =>
          `expected ${received} to be within range ${floor} - ${ceiling}`,
        pass: false,
      };
    }
  },

  toHaveProperty(received: any, property: string) {
    const pass = Object.prototype.hasOwnProperty.call(received, property);
    if (pass) {
      return {
        message: () => `expected object not to have property ${property}`,
        pass: true,
      };
    } else {
      return {
        message: () => `expected object to have property ${property}`,
        pass: false,
      };
    }
  },

  toBeValidJSON(received: string) {
    try {
      JSON.parse(received);
      return {
        message: () => `expected ${received} not to be valid JSON`,
        pass: true,
      };
    } catch {
      return {
        message: () => `expected ${received} to be valid JSON`,
        pass: false,
      };
    }
  }
});

// Type declarations for custom matchers
declare global {
  namespace jest {
    interface Matchers<R> {
      toBeWithinRange(floor: number, ceiling: number): R;
      toHaveProperty(property: string): R;
      toBeValidJSON(): R;
    }
  }
}