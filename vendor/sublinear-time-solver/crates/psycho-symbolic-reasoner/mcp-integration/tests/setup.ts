// Jest setup file for integration tests

// Mock console methods to reduce noise in tests
const originalConsoleLog = console.log;
const originalConsoleError = console.error;
const originalConsoleWarn = console.warn;

beforeAll(() => {
  // Only show errors in tests
  console.log = jest.fn();
  console.warn = jest.fn();
  console.error = jest.fn();
});

afterAll(() => {
  // Restore console methods
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
  console.warn = originalConsoleWarn;
});

// Global test timeout
jest.setTimeout(30000);

// Mock environment variables
process.env.NODE_ENV = 'test';
process.env.LOG_LEVEL = 'error';

// Global error handler for unhandled rejections in tests
process.on('unhandledRejection', (reason, promise) => {
  console.error('Unhandled Rejection at:', promise, 'reason:', reason);
});

// Global WebAssembly mock if needed
global.WebAssembly = global.WebAssembly || {
  Memory: class MockMemory {
    constructor(descriptor: any) {
      // Mock WebAssembly Memory
    }
  },
  instantiate: async (bytes: any, imports: any) => {
    return {
      instance: {
        exports: {}
      },
      module: {}
    };
  }
};

export {};