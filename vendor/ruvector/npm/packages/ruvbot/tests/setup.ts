/**
 * Test Setup Configuration
 *
 * Global setup for all RuvBot tests
 */

import { beforeAll, afterAll, beforeEach, afterEach, vi } from 'vitest';

// Global test timeout
vi.setConfig({ testTimeout: 30000 });

// Environment setup
beforeAll(async () => {
  // Set test environment variables
  process.env.NODE_ENV = 'test';
  process.env.RUVBOT_TEST_MODE = 'true';
  process.env.RUVBOT_LOG_LEVEL = 'error';
  process.env.DATABASE_URL = 'postgresql://test:test@localhost:5432/ruvbot_test';
  process.env.SLACK_BOT_TOKEN = 'xoxb-test-token';
  process.env.SLACK_SIGNING_SECRET = 'test-signing-secret';

  // Suppress console output during tests unless DEBUG is set
  if (!process.env.DEBUG) {
    vi.spyOn(console, 'log').mockImplementation(() => {});
    vi.spyOn(console, 'info').mockImplementation(() => {});
    vi.spyOn(console, 'debug').mockImplementation(() => {});
  }
});

afterAll(async () => {
  // Cleanup any global resources
  vi.restoreAllMocks();
});

beforeEach(() => {
  // Reset any per-test state
  vi.clearAllMocks();
});

afterEach(() => {
  // Clean up after each test
  vi.useRealTimers();
});

// Global error handler for unhandled rejections in tests
process.on('unhandledRejection', (reason, promise) => {
  console.error('Unhandled Rejection in test:', reason);
});

// Export test utilities
export const waitFor = async (condition: () => boolean | Promise<boolean>, timeout = 5000): Promise<void> => {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    if (await condition()) return;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`waitFor timeout after ${timeout}ms`);
};

export const delay = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms));
