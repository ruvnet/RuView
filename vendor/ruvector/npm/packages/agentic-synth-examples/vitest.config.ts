/**
 * Vitest Configuration for agentic-synth-examples
 */

import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

export default defineConfig({
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
      '@': resolve(__dirname, './src'),
      '@tests': resolve(__dirname, './tests')
    }
  }
});
