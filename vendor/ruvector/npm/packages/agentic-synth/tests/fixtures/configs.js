/**
 * Test fixtures - Sample configurations
 */

export const defaultConfig = {
  api: {
    baseUrl: 'https://api.test.com',
    apiKey: 'test-key-123',
    timeout: 5000,
    retries: 3
  },
  cache: {
    maxSize: 100,
    ttl: 3600000
  },
  generator: {
    seed: 12345,
    format: 'json'
  },
  router: {
    strategy: 'round-robin',
    models: []
  }
};

export const productionConfig = {
  api: {
    baseUrl: 'https://api.production.com',
    apiKey: process.env.API_KEY || '',
    timeout: 10000,
    retries: 5
  },
  cache: {
    maxSize: 1000,
    ttl: 7200000
  },
  generator: {
    seed: Date.now(),
    format: 'json'
  },
  router: {
    strategy: 'least-latency',
    models: [
      { id: 'model-1', endpoint: 'https://model1.com' },
      { id: 'model-2', endpoint: 'https://model2.com' }
    ]
  }
};

export const testConfig = {
  api: {
    baseUrl: 'http://localhost:3000',
    apiKey: 'test',
    timeout: 1000,
    retries: 1
  },
  cache: {
    maxSize: 10,
    ttl: 1000
  },
  generator: {
    seed: 12345,
    format: 'json'
  },
  router: {
    strategy: 'round-robin',
    models: []
  }
};

export const minimalConfig = {
  api: {
    baseUrl: 'https://api.example.com'
  }
};
