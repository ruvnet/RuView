/**
 * RuvBot Vitest Configuration
 *
 * Comprehensive testing setup for TDD-driven development
 * Borrowed patterns from Clawdbot and agentic-synth
 */

import { defineConfig } from 'vitest/config';
import { resolve } from 'path';

export default defineConfig({
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
      '@': resolve(__dirname, './src'),
      '@tests': resolve(__dirname, './tests'),
      '@fixtures': resolve(__dirname, './tests/fixtures'),
      '@factories': resolve(__dirname, './tests/factories'),
      '@mocks': resolve(__dirname, './tests/mocks')
    }
  }
});
