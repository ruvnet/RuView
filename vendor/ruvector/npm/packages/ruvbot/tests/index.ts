/**
 * RuvBot Test Utilities Index
 *
 * Central exports for test utilities, fixtures, factories, and mocks
 */

// Re-export fixtures
export * from './fixtures';

// Re-export factories
export * from './factories';

// Re-export mocks
export * from './mocks';

// Test utilities
export { waitFor, delay } from './utils/setup';

// Test type definitions
export interface TestContext {
  tenantId: string;
  userId: string;
  sessionId: string;
  channelId: string;
}

export interface MockServices {
  pool: import('./mocks/postgres.mock').MockPool;
  slackApp: import('./mocks/slack.mock').MockSlackBoltApp;
  ruvector: ReturnType<typeof import('./mocks/wasm.mock').createMockRuVectorBindings>;
}

/**
 * Create a complete mock services setup for testing
 */
export function createMockServices(): MockServices {
  const { createMockPool } = require('./mocks/postgres.mock');
  const { createMockSlackApp } = require('./mocks/slack.mock');
  const { createMockRuVectorBindings } = require('./mocks/wasm.mock');

  return {
    pool: createMockPool(),
    slackApp: createMockSlackApp(),
    ruvector: createMockRuVectorBindings()
  };
}

/**
 * Create a default test context
 */
export function createTestContext(overrides: Partial<TestContext> = {}): TestContext {
  return {
    tenantId: overrides.tenantId || 'test-tenant',
    userId: overrides.userId || 'U12345678',
    sessionId: overrides.sessionId || `session-${Date.now()}`,
    channelId: overrides.channelId || 'C12345678'
  };
}
