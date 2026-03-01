"use strict";
/**
 * Vitest Configuration for agentic-synth-examples
 */
Object.defineProperty(exports, "__esModule", { value: true });
const config_1 = require("vitest/config");
const url_1 = require("url");
const path_1 = require("path");
const __filename = (0, url_1.fileURLToPath)(import.meta.url);
const __dirname = (0, path_1.dirname)(__filename);
exports.default = (0, config_1.defineConfig)({
    test: {
        // Test environment
        environment: 'node',
        // Test files
        include: ['tests/**/*.test.ts'],
        exclude: ['node_modules', 'dist', 'build'],
        // Coverage configuration
        coverage: {
            provider: 'v8',
            reporter: ['text', 'json', 'html', 'lcov'],
            include: ['src/**/*.ts'],
            exclude: [
                'src/**/*.d.ts',
                'src/index.ts', // Re-export file
                'src/dspy/index.ts', // Re-export file
                'src/types/index.ts', // Type definitions
                'tests/**',
                'node_modules/**',
                'dist/**'
            ],
            // Coverage thresholds (80%+ target)
            thresholds: {
                lines: 80,
                functions: 80,
                branches: 75,
                statements: 80
            }
        },
        // Timeouts
        testTimeout: 10000, // 10 seconds for async operations
        hookTimeout: 10000,
        // Reporters
        reporters: ['verbose'],
        // Run tests in sequence to avoid race conditions
        // with event emitters and shared state
        sequence: {
            concurrent: false
        },
        // Globals
        globals: true,
        // Mock options
        mockReset: true,
        restoreMocks: true,
        clearMocks: true,
        // Retry failed tests once
        retry: 1
    },
    resolve: {
        alias: {
            '@': (0, path_1.resolve)(__dirname, './src'),
            '@tests': (0, path_1.resolve)(__dirname, './tests')
        }
    }
});
//# sourceMappingURL=vitest.config.js.map