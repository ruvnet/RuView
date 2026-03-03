/** @type {import('jest').Config} */
export default {
  preset: 'ts-jest/presets/default-esm',
  extensionsToTreatAsEsm: ['.ts'],
  globals: {
    'ts-jest': {
      useESM: true,
    },
  },
  testEnvironment: 'node',
  roots: ['<rootDir>/src'],
  testMatch: [
    '**/__tests__/**/*.ts',
    '**/?(*.)+(spec|test).ts'
  ],
  transform: {
    '^.+\\.ts$': ['ts-jest', {
      useESM: true,
    }],
  },
  collectCoverageFrom: [
    'src/**/*.ts',
    '!src/**/*.d.ts',
    '!src/**/index.ts',
    '!src/test-utils/**',
    '!src/setup/**'
  ],
  coverageDirectory: 'coverage',
  coverageReporters: [
    'text',
    'lcov',
    'html',
    'json-summary'
  ],
  coverageThreshold: {
    global: {
      branches: 90,
      functions: 90,
      lines: 90,
      statements: 90
    }
  },
  setupFilesAfterEnv: [
    '<rootDir>/src/setup/jest.setup.ts'
  ],
  moduleNameMapping: {
    '^@/(.*)$': '<rootDir>/src/$1',
    '^@test/(.*)$': '<rootDir>/src/test-utils/$1'
  },
  testTimeout: 30000,
  maxWorkers: '50%',
  // Enable memory leak detection
  detectOpenHandles: true,
  detectLeaks: true,
  // WASM configuration
  moduleFileExtensions: ['ts', 'js', 'json', 'wasm'],
  projects: [
    {
      displayName: 'unit',
      testMatch: ['<rootDir>/src/unit/**/*.test.ts']
    },
    {
      displayName: 'integration',
      testMatch: ['<rootDir>/src/integration/**/*.test.ts']
    },
    {
      displayName: 'e2e',
      testMatch: ['<rootDir>/src/e2e/**/*.test.ts'],
      testEnvironment: 'node'
    },
    {
      displayName: 'performance',
      testMatch: ['<rootDir>/src/performance/**/*.test.ts'],
      testTimeout: 60000
    },
    {
      displayName: 'memory',
      testMatch: ['<rootDir>/src/memory/**/*.test.ts'],
      testTimeout: 120000
    },
    {
      displayName: 'cli',
      testMatch: ['<rootDir>/src/cli/**/*.test.ts']
    },
    {
      displayName: 'mcp',
      testMatch: ['<rootDir>/src/mcp/**/*.test.ts']
    },
    {
      displayName: 'regression',
      testMatch: ['<rootDir>/src/regression/**/*.test.ts']
    }
  ]
};