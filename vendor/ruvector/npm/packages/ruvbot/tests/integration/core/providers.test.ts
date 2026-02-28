/**
 * Provider Integration Tests
 *
 * Tests the AnthropicProvider and OpenRouterProvider API contracts
 * and implementation correctness without making real API calls.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  AnthropicProvider,
  createAnthropicProvider,
  type AnthropicConfig,
} from '../../../src/integration/providers/AnthropicProvider.js';
import {
  OpenRouterProvider,
  createOpenRouterProvider,
  createQwQProvider,
  createDeepSeekR1Provider,
  type OpenRouterConfig,
} from '../../../src/integration/providers/OpenRouterProvider.js';
import type {
  Message,
  CompletionOptions,
  Completion,
  ModelInfo,
  LLMProvider,
} from '../../../src/integration/providers/index.js';

// Mock fetch for API testing
const mockFetch = vi.fn();
global.fetch = mockFetch;

describe('AnthropicProvider Integration Tests', () => {
  let provider: AnthropicProvider;

  beforeEach(() => {
    vi.resetAllMocks();
    provider = createAnthropicProvider({
      apiKey: 'test-api-key',
    });
  });

  describe('Configuration', () => {
    it('should create provider with default configuration', () => {
      const p = createAnthropicProvider({ apiKey: 'key' });
      const model = p.getModel();

      expect(model.id).toBe('claude-3-5-sonnet-20241022');
      expect(model.name).toBe('Claude 3.5 Sonnet');
      expect(model.maxTokens).toBe(8192);
      expect(model.contextWindow).toBe(200000);
    });

    it('should accept custom model', () => {
      const p = createAnthropicProvider({
        apiKey: 'key',
        model: 'claude-3-opus-20240229',
      });

      const model = p.getModel();
      expect(model.id).toBe('claude-3-opus-20240229');
      expect(model.name).toBe('Claude 3 Opus');
    });

    it('should accept custom base URL', () => {
      const p = createAnthropicProvider({
        apiKey: 'key',
        baseUrl: 'https://custom.api.example.com',
      });

      expect(p).toBeDefined();
    });

    it('should support all Claude models', () => {
      const models = [
        'claude-opus-4-20250514',
        'claude-sonnet-4-20250514',
        'claude-3-5-sonnet-20241022',
        'claude-3-5-haiku-20241022',
        'claude-3-opus-20240229',
        'claude-3-sonnet-20240229',
        'claude-3-haiku-20240307',
      ];

      for (const modelId of models) {
        const p = createAnthropicProvider({ apiKey: 'key', model: modelId });
        expect(p.getModel().id).toBe(modelId);
      }
    });
  });

  describe('LLMProvider Interface', () => {
    it('should implement complete method', async () => {
      const mockResponse = {
        id: 'msg_123',
        type: 'message',
        role: 'assistant',
        content: [{ type: 'text', text: 'Hello!' }],
        model: 'claude-3-5-sonnet-20241022',
        stop_reason: 'end_turn',
        usage: { input_tokens: 10, output_tokens: 5 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      const messages: Message[] = [
        { role: 'user', content: 'Say hello' },
      ];

      const completion = await provider.complete(messages);

      expect(completion.content).toBe('Hello!');
      expect(completion.finishReason).toBe('stop');
      expect(completion.usage.inputTokens).toBe(10);
      expect(completion.usage.outputTokens).toBe(5);
    });

    it('should implement stream method', async () => {
      // The stream method returns an AsyncGenerator
      const stream = provider.stream([{ role: 'user', content: 'Hello' }]);

      expect(stream).toBeDefined();
      expect(typeof stream[Symbol.asyncIterator]).toBe('function');
    });

    it('should implement countTokens method', async () => {
      const count = await provider.countTokens('Hello, world!');

      expect(typeof count).toBe('number');
      expect(count).toBeGreaterThan(0);
      // Approximate: 13 chars / 4 = ~4 tokens
      expect(count).toBeLessThan(10);
    });

    it('should implement getModel method', () => {
      const model = provider.getModel();

      expect(model).toHaveProperty('id');
      expect(model).toHaveProperty('name');
      expect(model).toHaveProperty('maxTokens');
      expect(model).toHaveProperty('contextWindow');
    });

    it('should implement isHealthy method', async () => {
      const healthy = await provider.isHealthy();
      expect(typeof healthy).toBe('boolean');
    });
  });

  describe('Message Handling', () => {
    it('should handle system messages', async () => {
      const mockResponse = {
        id: 'msg_123',
        type: 'message',
        role: 'assistant',
        content: [{ type: 'text', text: 'Response' }],
        model: 'claude-3-5-sonnet-20241022',
        stop_reason: 'end_turn',
        usage: { input_tokens: 20, output_tokens: 5 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      const messages: Message[] = [
        { role: 'system', content: 'You are helpful' },
        { role: 'user', content: 'Hello' },
      ];

      await provider.complete(messages);

      // Verify fetch was called with correct body
      const callArgs = mockFetch.mock.calls[0];
      const body = JSON.parse(callArgs[1].body);

      // System message should be prepended to first user message
      expect(body.messages[0].role).toBe('user');
      expect(body.messages[0].content).toContain('You are helpful');
    });

    it('should handle multi-turn conversations', async () => {
      const mockResponse = {
        id: 'msg_123',
        type: 'message',
        role: 'assistant',
        content: [{ type: 'text', text: 'Response' }],
        model: 'claude-3-5-sonnet-20241022',
        stop_reason: 'end_turn',
        usage: { input_tokens: 30, output_tokens: 5 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      const messages: Message[] = [
        { role: 'user', content: 'First message' },
        { role: 'assistant', content: 'First response' },
        { role: 'user', content: 'Second message' },
      ];

      await provider.complete(messages);

      const callArgs = mockFetch.mock.calls[0];
      const body = JSON.parse(callArgs[1].body);

      expect(body.messages.length).toBe(3);
    });
  });

  describe('Tool Use', () => {
    it('should handle tool calls in response', async () => {
      const mockResponse = {
        id: 'msg_123',
        type: 'message',
        role: 'assistant',
        content: [
          { type: 'text', text: 'Let me search' },
          {
            type: 'tool_use',
            id: 'tool_123',
            name: 'web_search',
            input: { query: 'weather' },
          },
        ],
        model: 'claude-3-5-sonnet-20241022',
        stop_reason: 'tool_use',
        usage: { input_tokens: 15, output_tokens: 20 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      const completion = await provider.complete([
        { role: 'user', content: 'What is the weather?' },
      ], {
        tools: [{
          name: 'web_search',
          description: 'Search the web',
          parameters: { type: 'object', properties: { query: { type: 'string' } } },
        }],
      });

      expect(completion.finishReason).toBe('tool_use');
      expect(completion.toolCalls).toBeDefined();
      expect(completion.toolCalls?.length).toBe(1);
      expect(completion.toolCalls?.[0].name).toBe('web_search');
      expect(completion.toolCalls?.[0].input).toEqual({ query: 'weather' });
    });

    it('should send tools in request', async () => {
      const mockResponse = {
        id: 'msg_123',
        type: 'message',
        role: 'assistant',
        content: [{ type: 'text', text: 'Response' }],
        model: 'claude-3-5-sonnet-20241022',
        stop_reason: 'end_turn',
        usage: { input_tokens: 10, output_tokens: 5 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      await provider.complete([{ role: 'user', content: 'Hello' }], {
        tools: [{
          name: 'calculator',
          description: 'Perform calculations',
          parameters: { type: 'object' },
        }],
      });

      const callArgs = mockFetch.mock.calls[0];
      const body = JSON.parse(callArgs[1].body);

      expect(body.tools).toBeDefined();
      expect(body.tools[0].name).toBe('calculator');
    });
  });

  describe('Completion Options', () => {
    it('should apply maxTokens option', async () => {
      const mockResponse = {
        id: 'msg_123',
        type: 'message',
        role: 'assistant',
        content: [{ type: 'text', text: 'Response' }],
        model: 'claude-3-5-sonnet-20241022',
        stop_reason: 'end_turn',
        usage: { input_tokens: 10, output_tokens: 5 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      await provider.complete([{ role: 'user', content: 'Hello' }], {
        maxTokens: 100,
      });

      const body = JSON.parse(mockFetch.mock.calls[0][1].body);
      expect(body.max_tokens).toBe(100);
    });

    it('should apply temperature option', async () => {
      const mockResponse = {
        id: 'msg_123',
        type: 'message',
        role: 'assistant',
        content: [{ type: 'text', text: 'Response' }],
        model: 'claude-3-5-sonnet-20241022',
        stop_reason: 'end_turn',
        usage: { input_tokens: 10, output_tokens: 5 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      await provider.complete([{ role: 'user', content: 'Hello' }], {
        temperature: 0.5,
      });

      const body = JSON.parse(mockFetch.mock.calls[0][1].body);
      expect(body.temperature).toBe(0.5);
    });
  });

  describe('Error Handling', () => {
    it('should throw on API error', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 401,
        text: () => Promise.resolve('Invalid API key'),
      });

      await expect(
        provider.complete([{ role: 'user', content: 'Hello' }])
      ).rejects.toThrow('Anthropic API error: 401');
    });

    it('should handle max_tokens finish reason', async () => {
      const mockResponse = {
        id: 'msg_123',
        type: 'message',
        role: 'assistant',
        content: [{ type: 'text', text: 'Truncated...' }],
        model: 'claude-3-5-sonnet-20241022',
        stop_reason: 'max_tokens',
        usage: { input_tokens: 10, output_tokens: 100 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      const completion = await provider.complete([{ role: 'user', content: 'Long text' }]);
      expect(completion.finishReason).toBe('length');
    });
  });
});

describe('OpenRouterProvider Integration Tests', () => {
  let provider: OpenRouterProvider;

  beforeEach(() => {
    vi.resetAllMocks();
    provider = createOpenRouterProvider({
      apiKey: 'test-openrouter-key',
    });
  });

  describe('Configuration', () => {
    it('should create provider with default model (QwQ)', () => {
      const p = createOpenRouterProvider({ apiKey: 'key' });
      const model = p.getModel();

      expect(model.id).toBe('qwen/qwq-32b');
      expect(model.name).toContain('QwQ');
    });

    it('should accept custom model', () => {
      const p = createOpenRouterProvider({
        apiKey: 'key',
        model: 'anthropic/claude-3.5-sonnet',
      });

      const model = p.getModel();
      expect(model.id).toBe('anthropic/claude-3.5-sonnet');
    });

    it('should accept site information', () => {
      const p = createOpenRouterProvider({
        apiKey: 'key',
        siteUrl: 'https://myapp.com',
        siteName: 'MyApp',
      });

      expect(p).toBeDefined();
    });
  });

  describe('Factory Functions', () => {
    it('should create QwQ provider', () => {
      const p = createQwQProvider('key');
      expect(p.getModel().id).toBe('qwen/qwq-32b');
    });

    it('should create free QwQ provider', () => {
      const p = createQwQProvider('key', true);
      expect(p.getModel().id).toBe('qwen/qwq-32b:free');
    });

    it('should create DeepSeek R1 provider', () => {
      const p = createDeepSeekR1Provider('key');
      expect(p.getModel().id).toBe('deepseek/deepseek-r1');
    });
  });

  describe('LLMProvider Interface', () => {
    it('should implement complete method', async () => {
      const mockResponse = {
        id: 'gen_123',
        model: 'qwen/qwq-32b',
        choices: [{
          index: 0,
          message: {
            role: 'assistant',
            content: 'Hello from QwQ!',
          },
          finish_reason: 'stop',
        }],
        usage: {
          prompt_tokens: 10,
          completion_tokens: 5,
          total_tokens: 15,
        },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      const completion = await provider.complete([
        { role: 'user', content: 'Hello' },
      ]);

      expect(completion.content).toBe('Hello from QwQ!');
      expect(completion.finishReason).toBe('stop');
    });

    it('should implement stream method', () => {
      const stream = provider.stream([{ role: 'user', content: 'Hello' }]);

      expect(stream).toBeDefined();
      expect(typeof stream[Symbol.asyncIterator]).toBe('function');
    });

    it('should implement countTokens method', async () => {
      const count = await provider.countTokens('Test text');
      expect(typeof count).toBe('number');
      expect(count).toBeGreaterThan(0);
    });

    it('should implement getModel method', () => {
      const model = provider.getModel();

      expect(model).toHaveProperty('id');
      expect(model).toHaveProperty('name');
      expect(model).toHaveProperty('maxTokens');
      expect(model).toHaveProperty('contextWindow');
    });

    it('should implement isHealthy method', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
      });

      const healthy = await provider.isHealthy();
      expect(healthy).toBe(true);
    });
  });

  describe('Model Info', () => {
    const modelTests = [
      { id: 'qwen/qwq-32b', name: 'Qwen QwQ 32B', context: 32768 },
      { id: 'anthropic/claude-3.5-sonnet', name: 'Claude 3.5 Sonnet', context: 200000 },
      { id: 'openai/gpt-4o', name: 'GPT-4o', context: 128000 },
      { id: 'deepseek/deepseek-r1', name: 'DeepSeek R1', context: 64000 },
    ];

    for (const test of modelTests) {
      it(`should have correct info for ${test.id}`, () => {
        const p = createOpenRouterProvider({ apiKey: 'key', model: test.id });
        const model = p.getModel();

        expect(model.id).toBe(test.id);
        expect(model.name).toContain(test.name.split(' ')[0]);
        expect(model.contextWindow).toBe(test.context);
      });
    }

    it('should handle unknown models gracefully', () => {
      const p = createOpenRouterProvider({
        apiKey: 'key',
        model: 'unknown/model-xyz',
      });

      const model = p.getModel();
      expect(model.id).toBe('unknown/model-xyz');
      expect(model.maxTokens).toBe(4096); // default
    });
  });

  describe('Message Handling', () => {
    it('should preserve system messages', async () => {
      const mockResponse = {
        id: 'gen_123',
        model: 'qwen/qwq-32b',
        choices: [{
          index: 0,
          message: { role: 'assistant', content: 'Response' },
          finish_reason: 'stop',
        }],
        usage: { prompt_tokens: 20, completion_tokens: 5, total_tokens: 25 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      await provider.complete([
        { role: 'system', content: 'Be helpful' },
        { role: 'user', content: 'Hello' },
      ]);

      const body = JSON.parse(mockFetch.mock.calls[0][1].body);
      expect(body.messages[0].role).toBe('system');
      expect(body.messages[0].content).toBe('Be helpful');
    });
  });

  describe('Tool Use', () => {
    it('should handle tool calls', async () => {
      const mockResponse = {
        id: 'gen_123',
        model: 'qwen/qwq-32b',
        choices: [{
          index: 0,
          message: {
            role: 'assistant',
            content: null,
            tool_calls: [{
              id: 'call_123',
              type: 'function',
              function: {
                name: 'get_weather',
                arguments: '{"city": "London"}',
              },
            }],
          },
          finish_reason: 'tool_calls',
        }],
        usage: { prompt_tokens: 10, completion_tokens: 15, total_tokens: 25 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      const completion = await provider.complete([
        { role: 'user', content: 'Weather in London?' },
      ], {
        tools: [{
          name: 'get_weather',
          description: 'Get weather',
          parameters: { type: 'object' },
        }],
      });

      expect(completion.finishReason).toBe('tool_use');
      expect(completion.toolCalls).toHaveLength(1);
      expect(completion.toolCalls?.[0].name).toBe('get_weather');
      expect(completion.toolCalls?.[0].input).toEqual({ city: 'London' });
    });
  });

  describe('Headers', () => {
    it('should include site headers when configured', async () => {
      const p = createOpenRouterProvider({
        apiKey: 'key',
        siteUrl: 'https://myapp.com',
        siteName: 'MyApp',
      });

      const mockResponse = {
        id: 'gen_123',
        model: 'qwen/qwq-32b',
        choices: [{
          index: 0,
          message: { role: 'assistant', content: 'Response' },
          finish_reason: 'stop',
        }],
        usage: { prompt_tokens: 10, completion_tokens: 5, total_tokens: 15 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      await p.complete([{ role: 'user', content: 'Hello' }]);

      const headers = mockFetch.mock.calls[0][1].headers;
      expect(headers['HTTP-Referer']).toBe('https://myapp.com');
      expect(headers['X-Title']).toBe('MyApp');
    });
  });

  describe('List Models', () => {
    it('should list available models', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({
          data: [
            { id: 'model1' },
            { id: 'model2' },
          ],
        }),
      });

      const models = await provider.listModels();

      expect(models).toContain('model1');
      expect(models).toContain('model2');
    });

    it('should return default models on API failure', async () => {
      mockFetch.mockRejectedValueOnce(new Error('Network error'));

      const models = await provider.listModels();

      expect(models.length).toBeGreaterThan(0);
      expect(models).toContain('qwen/qwq-32b');
    });
  });

  describe('Error Handling', () => {
    it('should throw on API error', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 429,
        text: () => Promise.resolve('Rate limited'),
      });

      await expect(
        provider.complete([{ role: 'user', content: 'Hello' }])
      ).rejects.toThrow('OpenRouter API error: 429');
    });

    it('should handle null content in response', async () => {
      const mockResponse = {
        id: 'gen_123',
        model: 'qwen/qwq-32b',
        choices: [{
          index: 0,
          message: { role: 'assistant', content: null },
          finish_reason: 'stop',
        }],
        usage: { prompt_tokens: 10, completion_tokens: 0, total_tokens: 10 },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(mockResponse),
      });

      const completion = await provider.complete([{ role: 'user', content: 'Hello' }]);
      expect(completion.content).toBe('');
    });
  });
});

describe('Provider Contract Compliance', () => {
  const providers: Array<{ name: string; create: () => LLMProvider }> = [
    {
      name: 'AnthropicProvider',
      create: () => createAnthropicProvider({ apiKey: 'test' }),
    },
    {
      name: 'OpenRouterProvider',
      create: () => createOpenRouterProvider({ apiKey: 'test' }),
    },
  ];

  for (const { name, create } of providers) {
    describe(`${name} Contract`, () => {
      let provider: LLMProvider;

      beforeEach(() => {
        provider = create();
      });

      it('should implement complete method', () => {
        expect(typeof provider.complete).toBe('function');
      });

      it('should implement stream method', () => {
        expect(typeof provider.stream).toBe('function');
      });

      it('should implement countTokens method', () => {
        expect(typeof provider.countTokens).toBe('function');
      });

      it('should implement getModel method', () => {
        expect(typeof provider.getModel).toBe('function');
      });

      it('should implement isHealthy method', () => {
        expect(typeof provider.isHealthy).toBe('function');
      });

      it('should return valid ModelInfo from getModel', () => {
        const model = provider.getModel();

        expect(model).toHaveProperty('id');
        expect(model).toHaveProperty('name');
        expect(model).toHaveProperty('maxTokens');
        expect(model).toHaveProperty('contextWindow');

        expect(typeof model.id).toBe('string');
        expect(typeof model.name).toBe('string');
        expect(typeof model.maxTokens).toBe('number');
        expect(typeof model.contextWindow).toBe('number');

        expect(model.maxTokens).toBeGreaterThan(0);
        expect(model.contextWindow).toBeGreaterThan(0);
      });

      it('should return number from countTokens', async () => {
        const count = await provider.countTokens('test');
        expect(typeof count).toBe('number');
        expect(count).toBeGreaterThan(0);
      });

      it('should return boolean from isHealthy', async () => {
        const healthy = await provider.isHealthy();
        expect(typeof healthy).toBe('boolean');
      });
    });
  }
});
