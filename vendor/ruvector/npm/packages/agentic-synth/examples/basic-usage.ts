/**
 * Basic usage examples for agentic-synth
 */

import { AgenticSynth, createSynth } from '../src/index.js';

// Example 1: Basic time-series generation
async function basicTimeSeries() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const result = await synth.generateTimeSeries({
    count: 100,
    interval: '1h',
    metrics: ['temperature', 'humidity'],
    trend: 'up',
    seasonality: true
  });

  console.log('Generated time-series data:');
  console.log(result.data.slice(0, 5));
  console.log('Metadata:', result.metadata);
}

// Example 2: Event generation
async function generateEvents() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const result = await synth.generateEvents({
    count: 50,
    eventTypes: ['page_view', 'button_click', 'form_submit'],
    distribution: 'poisson',
    userCount: 25,
    timeRange: {
      start: new Date(Date.now() - 24 * 60 * 60 * 1000),
      end: new Date()
    }
  });

  console.log('Generated events:');
  console.log(result.data.slice(0, 5));
}

// Example 3: Structured data with schema
async function generateStructured() {
  const synth = createSynth();

  const schema = {
    id: { type: 'string', required: true },
    name: { type: 'string', required: true },
    email: { type: 'string', required: true },
    age: { type: 'number', required: true },
    address: {
      type: 'object',
      required: false,
      properties: {
        street: { type: 'string' },
        city: { type: 'string' },
        country: { type: 'string' }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 20,
    schema,
    format: 'json'
  });

  console.log('Generated user data:');
  console.log(result.data.slice(0, 3));
}

// Example 4: Streaming generation
async function streamingGeneration() {
  const synth = createSynth({
    streaming: true
  });

  console.log('Streaming time-series data:');

  for await (const dataPoint of synth.generateStream('timeseries', {
    count: 50,
    interval: '5m',
    metrics: ['cpu', 'memory']
  })) {
    console.log('Received:', dataPoint);
  }
}

// Example 5: Batch generation
async function batchGeneration() {
  const synth = createSynth();

  const batches = [
    { count: 10, schema: { id: { type: 'string' }, value: { type: 'number' } } },
    { count: 15, schema: { id: { type: 'string' }, value: { type: 'number' } } },
    { count: 20, schema: { id: { type: 'string' }, value: { type: 'number' } } }
  ];

  const results = await synth.generateBatch('structured', batches, 2);

  console.log('Batch results:');
  results.forEach((result, i) => {
    console.log(`Batch ${i + 1}: ${result.metadata.count} records in ${result.metadata.duration}ms`);
  });
}

// Example 6: Using OpenRouter
async function useOpenRouter() {
  const synth = createSynth({
    provider: 'openrouter',
    apiKey: process.env.OPENROUTER_API_KEY,
    model: 'anthropic/claude-3.5-sonnet'
  });

  const result = await synth.generateTimeSeries({
    count: 30,
    interval: '10m',
    metrics: ['requests_per_second']
  });

  console.log('Generated with OpenRouter:');
  console.log(result.metadata);
}

// Example 7: With caching
async function withCaching() {
  const synth = createSynth({
    cacheStrategy: 'memory',
    cacheTTL: 600 // 10 minutes
  });

  // First call - will generate
  console.time('First call');
  const result1 = await synth.generateTimeSeries({
    count: 50,
    interval: '1h',
    metrics: ['value']
  });
  console.timeEnd('First call');
  console.log('Cached:', result1.metadata.cached);

  // Second call with same params - should hit cache
  console.time('Second call');
  const result2 = await synth.generateTimeSeries({
    count: 50,
    interval: '1h',
    metrics: ['value']
  });
  console.timeEnd('Second call');
  console.log('Cached:', result2.metadata.cached);
}

// Example 8: Error handling
async function errorHandling() {
  const synth = createSynth();

  try {
    await synth.generateStructured({
      count: 10
      // Missing schema - will throw ValidationError
    });
  } catch (error) {
    if (error.name === 'ValidationError') {
      console.error('Validation error:', error.message);
    } else {
      console.error('Unexpected error:', error);
    }
  }
}

// Run examples
async function runExamples() {
  console.log('=== Example 1: Basic Time-Series ===');
  await basicTimeSeries();

  console.log('\n=== Example 2: Events ===');
  await generateEvents();

  console.log('\n=== Example 3: Structured Data ===');
  await generateStructured();

  console.log('\n=== Example 5: Batch Generation ===');
  await batchGeneration();

  console.log('\n=== Example 7: Caching ===');
  await withCaching();

  console.log('\n=== Example 8: Error Handling ===');
  await errorHandling();
}

// Uncomment to run
// runExamples().catch(console.error);
