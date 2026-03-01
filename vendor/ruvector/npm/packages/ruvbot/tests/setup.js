"use strict";
/**
 * Test Setup Configuration
 *
 * Global setup for all RuvBot tests
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.delay = exports.waitFor = void 0;
const vitest_1 = require("vitest");
// Global test timeout
vitest_1.vi.setConfig({ testTimeout: 30000 });
// Environment setup
(0, vitest_1.beforeAll)(async () => {
    // Set test environment variables
    process.env.NODE_ENV = 'test';
    process.env.RUVBOT_TEST_MODE = 'true';
    process.env.RUVBOT_LOG_LEVEL = 'error';
    process.env.DATABASE_URL = 'postgresql://test:test@localhost:5432/ruvbot_test';
    process.env.SLACK_BOT_TOKEN = 'xoxb-test-token';
    process.env.SLACK_SIGNING_SECRET = 'test-signing-secret';
    // Suppress console output during tests unless DEBUG is set
    if (!process.env.DEBUG) {
        vitest_1.vi.spyOn(console, 'log').mockImplementation(() => { });
        vitest_1.vi.spyOn(console, 'info').mockImplementation(() => { });
        vitest_1.vi.spyOn(console, 'debug').mockImplementation(() => { });
    }
});
(0, vitest_1.afterAll)(async () => {
    // Cleanup any global resources
    vitest_1.vi.restoreAllMocks();
});
(0, vitest_1.beforeEach)(() => {
    // Reset any per-test state
    vitest_1.vi.clearAllMocks();
});
(0, vitest_1.afterEach)(() => {
    // Clean up after each test
    vitest_1.vi.useRealTimers();
});
// Global error handler for unhandled rejections in tests
process.on('unhandledRejection', (reason, promise) => {
    console.error('Unhandled Rejection in test:', reason);
});
// Export test utilities
const waitFor = async (condition, timeout = 5000) => {
    const start = Date.now();
    while (Date.now() - start < timeout) {
        if (await condition())
            return;
        await new Promise(resolve => setTimeout(resolve, 50));
    }
    throw new Error(`waitFor timeout after ${timeout}ms`);
};
exports.waitFor = waitFor;
const delay = (ms) => new Promise(resolve => setTimeout(resolve, ms));
exports.delay = delay;
//# sourceMappingURL=setup.js.map