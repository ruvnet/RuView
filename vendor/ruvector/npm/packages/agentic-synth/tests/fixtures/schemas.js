/**
 * Test fixtures - Sample schemas
 */

export const basicSchema = {
  name: { type: 'string', length: 10 },
  value: { type: 'number', min: 0, max: 100 }
};

export const complexSchema = {
  id: { type: 'string', length: 8 },
  title: { type: 'string', length: 50 },
  description: { type: 'string', length: 200 },
  priority: { type: 'number', min: 1, max: 5 },
  active: { type: 'boolean' },
  tags: { type: 'array', items: 10 },
  metadata: {
    created: { type: 'number' },
    updated: { type: 'number' }
  }
};

export const vectorSchema = {
  document_id: { type: 'string', length: 16 },
  text: { type: 'string', length: 100 },
  embedding: { type: 'vector', dimensions: 128 },
  score: { type: 'number', min: 0, max: 1 }
};

export const roboticsSchema = {
  command: { type: 'string', length: 16 },
  x: { type: 'number', min: -100, max: 100 },
  y: { type: 'number', min: -100, max: 100 },
  z: { type: 'number', min: 0, max: 50 },
  velocity: { type: 'number', min: 0, max: 10 }
};

export const streamingSchema = {
  event_id: { type: 'string', length: 12 },
  timestamp: { type: 'number' },
  event_type: { type: 'string', length: 20 },
  payload: { type: 'string', length: 500 },
  priority: { type: 'number', min: 1, max: 10 }
};
