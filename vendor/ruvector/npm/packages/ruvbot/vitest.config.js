"use strict";
/**
 * RuvBot Vitest Configuration
 *
 * Comprehensive testing setup for TDD-driven development
 * Borrowed patterns from Clawdbot and agentic-synth
 */
Object.defineProperty(exports, "__esModule", { value: true });
const config_1 = require("vitest/config");
const path_1 = require("path");
exports.default = (0, config_1.defineConfig)({
    test: {
        globals: true,
        environment: 'node',
        // Test file patterns
        include: [
            'tests/**/*.test.ts',
            'tests/**/*.spec.ts'
        ],
        exclude: [
            'node_modules/**',
            'dist/**'
        ],
        // Coverage configuration
        coverage: {
            provider: 'v8',
            reporter: ['text', 'json', 'html', 'lcov'],
            exclude: [
                'node_modules/**',
                'dist/**',
                'coverage/**',
                'tests/**',
                '**/*.test.ts',
                '**/*.spec.ts',
                '**/*.config.ts',
                '**/*.config.js',
                'benchmarks/**',
                'examples/**',
                'docs/**'
            ],
            include: ['src/**/*.ts'],
            all: true,
            lines: 80,
            functions: 80,
            branches: 75,
            statements: 80
        },
        // Timeouts
        testTimeout: 30000,
        hookTimeout: 15000,
        // Setup files
        setupFiles: ['./tests/setup.ts'],
        // Parallel execution
        pool: 'threads',
        poolOptions: {
            threads: {
                singleThread: false,
                maxThreads: 4,
                minThreads: 1
            }
        },
        // Reporter configuration
        reporters: ['verbose'],
        // Retry configuration for flaky tests
        retry: 1,
        // Sequence configuration
        sequence: {
            shuffle: false,
            seed: 42
        },
        // Mock configuration
        clearMocks: true,
        restoreMocks: true,
        mockReset: true,
        // Type checking
        typecheck: {
            enabled: false, // Enable when type definitions are stable
            include: ['tests/**/*.ts']
        }
    },
    resolve: {
        alias: {
            '@': (0, path_1.resolve)(__dirname, './src'),
            '@tests': (0, path_1.resolve)(__dirname, './tests'),
            '@fixtures': (0, path_1.resolve)(__dirname, './tests/fixtures'),
            '@factories': (0, path_1.resolve)(__dirname, './tests/factories'),
            '@mocks': (0, path_1.resolve)(__dirname, './tests/mocks')
        }
    }
});
//# sourceMappingURL=vitest.config.js.map